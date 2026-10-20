//! The proto3 state of a named-type payload under ADR-0017's projection:
//! bounded for a struct (a message), a union (a message with a `oneof`), and
//! a named scalar, an enum and an enum set, each of which the proto backend
//! gives a root box message `<Name>Box` holding one field `value = 1` of the
//! declaration's own type (`emit_root_boxes`). ADR-0017 decision 2 rejects a
//! wrapper message at a field position; no field names a box, so a field of
//! one of those three types still inlines (decision 1).
//!
//! The rules are the proto3 wire format over the projection the proto backend
//! emits: a field number from the IR ordinal (`emit_struct`, `emit_union`), a
//! named scalar inlined into its field, an enum as a varint, an enum set as
//! its width's scalar, an array as `repeated` (packed for a scalar and an
//! enum), a map as `map<K, V>`, a tuple as an induced message.
//!
//! A member whose shape the backend refuses makes the payload absent. The
//! refusals reproduced here, each citing the backend function that holds it,
//! include: a field with no type and a stream type (`resolve_field_type`), a
//! name that does not resolve (`resolve_named`), a nested array or map and a
//! map value that is an array or a map (`resolve_field_type`), a map key
//! outside the integral and string scalars (`map_key_text`), an enum live or
//! retired value outside int32 (`emit_enum`), and a field number protobuf
//! reserves or exceeds (`check_field_number`). One refusal is not reproduced
//! but shared: an optional array or map field of a struct or a tuple
//! (`emit_struct`, `emit_induced_tuple`, ADR-0013 decision 7) is the rule
//! [`proto3_projection::optional_repeated`], which the backend and this sizer
//! both call. The backend's name-collision
//! refusals, at member and package scope, are **not** reproduced: two struct
//! field names that coincide after `snake_case` (`emit_struct`); two enum
//! values that coincide after `SCREAMING_SNAKE_CASE`, one that spells the
//! synthesized `<PREFIX>_UNSPECIFIED`, or a reserved enum name equal to a
//! live value (`emit_enum`); a union arm named `value`, the name of the
//! `oneof` every union message carries, or two arms that project to one name
//! (`emit_union`); an induced tuple message named as another tuple or as a
//! declared type (`emit_induced_tuples`); and every other package-scope
//! claim that clashes, such as a declared type named as a generated
//! `<Interface>Ordinal` table (`SymbolScope::claim`). A payload in a package
//! the backend refuses on a name collision still gets a proto3 bound here: the
//! wire size does not depend on the names. Moving the other refusal rules
//! reproduced here into [`crate::projection::proto3`], where the backend and
//! this sizer would share them as they share `optional_repeated`, is tracked
//! by driftsys/ridl#690.
//!
//! proto3 has no unbounded state: typl bounds every collection, so a message
//! is bounded or absent, and the cause of an absence says why. A member the
//! projection refuses is [`AbsentCause::RefusedMember`]. A member that has
//! no finite bound — a `string` or `bytes` with no length maximum, a type def
//! with no width, or a collection of such a member — is
//! [`AbsentCause::NoBound`]. Every addition and multiplication is checked, so
//! a bound the `u64` arithmetic cannot hold is [`AbsentCause::Overflow`], and
//! so is a bound above `u32::MAX`, as in the FlatBuffers projection's
//! `MAX_ENCODABLE`, and a nesting past the depth limit. The walk stops at its
//! first failure, so the cause is that of the first member, in field order,
//! that has no bound.

use std::collections::HashMap;

use crate::projection::proto3::{
    self as proto3_projection, PROTO_MAX_FIELD_NUMBER, PROTO_RESERVED, Scalar,
};
use crate::v2::{
    ArrayType, FieldType, MapType, Package, StructDef, TupleType, UnionDef, decl, field_type,
    struct_member,
};

use super::{AbsentCause, Ctx, Leaf, SizeState, leaf_of_decl, leaf_of_field_type, leaf_of_name};

/// Nesting deeper than this is treated as unsizable; typl rejects recursion
/// (§7.3), so this only guards against an IR the checker did not see.
const MAX_DEPTH: u32 = 64;

const SCALAR_IS_BOUNDED: &str = "a `Leaf::Scalar` is never `string` or `bytes`: `leaf_of_type_def` and `leaf_of_primitive` \
     make those a `Leaf::Blob` or no leaf";

/// The length of `v` as a base-128 varint.
pub(crate) fn varint_len(v: u64) -> u64 {
    if v == 0 {
        1
    } else {
        u64::from((64 - v.leading_zeros()).div_ceil(7))
    }
}

/// The tag of field `number`; the wire type does not change the length.
fn tag_len(number: u32) -> u64 {
    varint_len((u64::from(number) << 3) | 5)
}

/// A field whose value is `len` bytes: tag and value.
fn plain(number: u32, len: u64) -> Option<u64> {
    tag_len(number).checked_add(len)
}

/// A length-delimited field: tag, length varint, payload.
fn delimited(number: u32, payload: u64) -> Option<u64> {
    tag_len(number)
        .checked_add(varint_len(payload))?
        .checked_add(payload)
}

/// One `state` call's walk: the scope it resolves names in, and the bounds
/// it has derived so far.
struct Walk<'a, 'c> {
    ctx: &'c Ctx<'a>,
    /// The bound of each named struct and union already derived, keyed by
    /// the declaring package's name and the declaration's name, so a type
    /// reached twice is walked once. Without it a diamond — `A { b: B, c: B }`,
    /// `B { d: C, e: C }`, and so on — costs time exponential in its depth,
    /// and typl admits one: TYPL-206 rejects a cycle, not sharing. The
    /// FlatBuffers projection's `Sizer::computed` is the same device.
    ///
    /// Only a bound is remembered. A `None` may be the depth guard answering
    /// for the path that reached it rather than a property of the type, and
    /// that answer does not generalize to another path.
    memo: HashMap<(&'a str, &'a str), u64>,
    /// The cause of the first failure the walk met, when that failure is a
    /// size problem: `NoBound` for a member with no finite bound, `Overflow`
    /// for arithmetic that does not fit or a nesting past `MAX_DEPTH`. A
    /// refusal records nothing, so a walk that fails with no cause failed on
    /// a refused member. The walk stops at its first failure, so the first
    /// cause recorded is the cause of the whole.
    cause: Option<AbsentCause>,
    /// How many times `remembered` ran its body: the number of struct and
    /// union messages the walk derived rather than reused. A test counts
    /// these, because a memo regression on a deep diamond would otherwise
    /// show as a walk that does not return.
    #[cfg(test)]
    bodies: u32,
}

impl<'a> Walk<'a, '_> {
    /// The bound of the declaration `name` of `home`: the one already
    /// derived if there is one, otherwise what `body` derives, remembered
    /// when it is a bound.
    fn remembered(
        &mut self,
        home: &'a Package,
        name: &'a str,
        body: impl FnOnce(&mut Self) -> Option<u64>,
    ) -> Option<u64> {
        let key = (home.name.as_str(), name);
        if let Some(bound) = self.memo.get(&key) {
            return Some(*bound);
        }
        #[cfg(test)]
        {
            self.bodies += 1;
        }
        let bound = body(self)?;
        self.memo.insert(key, bound);
        Some(bound)
    }

    /// Fails the walk for a size problem, recording `cause` when no cause is
    /// recorded yet.
    fn fail(&mut self, cause: AbsentCause) -> Option<u64> {
        self.cause.get_or_insert(cause);
        None
    }

    /// `value`, or the walk failed with `Overflow`: the checked arithmetic
    /// of a bound did not fit.
    fn fits(&mut self, value: Option<u64>) -> Option<u64> {
        match value {
            Some(value) => Some(value),
            None => self.fail(AbsentCause::Overflow),
        }
    }

    /// The leaf at a field position ([`leaf_of_field_type`]), or the walk
    /// failed: `NoBound` for a bare `string` or `bytes` and for an inline or
    /// named type def no leaf bounds; a refusal for a field with no type, a
    /// stream, a name that does not resolve or a declaration with no leaf.
    fn leaf_of(&mut self, ty: &'a FieldType, home: &'a Package) -> Option<Leaf<'a>> {
        let leaf = leaf_of_field_type(ty, home, self.ctx);
        if leaf.is_none() {
            let no_bound = match ty.kind.as_ref() {
                Some(field_type::Kind::Primitive(primitive)) => matches!(
                    proto3_projection::primitive(*primitive),
                    Scalar::String | Scalar::Bytes
                ),
                Some(field_type::Kind::InlineScalar(_)) => true,
                Some(field_type::Kind::Named(name)) => self.names_a_type_def(name, home),
                _ => false,
            };
            if no_bound {
                self.fail(AbsentCause::NoBound);
            }
        }
        leaf
    }

    /// The leaf of the name `name` written in `home` ([`leaf_of_name`]),
    /// with the causes [`Walk::leaf_of`] gives.
    fn leaf_of_named(&mut self, name: &str, home: &'a Package) -> Option<Leaf<'a>> {
        let leaf = leaf_of_name(name, home, self.ctx);
        if leaf.is_none() && self.names_a_type_def(name, home) {
            self.fail(AbsentCause::NoBound);
        }
        leaf
    }

    /// Whether `name` resolves to a type def: the one declaration kind whose
    /// missing leaf means no bound rather than a refusal.
    fn names_a_type_def(&self, name: &str, home: &'a Package) -> bool {
        self.ctx
            .resolve(home, name)
            .is_some_and(|(decl, _)| matches!(decl.kind, Some(decl::Kind::TypeDef(_))))
    }
}

/// The proto3 state of the named type `type_name`, read from `home`: its
/// message's bound for a struct or a union, its root box message's bound for
/// a named scalar, an enum or an enum set, and absent otherwise, with the
/// cause of the absence. No declaration kind gives `NoMessage`.
///
/// `NoBound` for a named scalar that no proto3 leaf bounds — a `string` or
/// `bytes` with no length bound, a type def with no width — and for a message
/// whose first failing member is one, or a collection of one. `Unresolved`
/// for a name that does not resolve and for a declaration that has no message
/// at all, such as a constant or an interaction. `Overflow` when the nesting
/// passes `MAX_DEPTH`, the bound is above `u32::MAX`, or a `u64` sum or
/// product inside the walk does not fit. `RefusedMember` for every other
/// failure inside the message: a member the proto backend refuses, a name
/// inside the message that does not resolve, and an enum value outside int32.
pub(crate) fn state<'a>(home: &'a Package, type_name: &str, ctx: &Ctx<'a>) -> SizeState {
    let Some((decl, declaring)) = ctx.resolve(home, type_name) else {
        return SizeState::Absent(AbsentCause::Unresolved);
    };
    let mut walk = Walk {
        ctx,
        memo: HashMap::new(),
        cause: None,
        #[cfg(test)]
        bodies: 0,
    };
    let name = decl.name.as_str();
    let bytes = match decl.kind.as_ref() {
        Some(decl::Kind::StructDef(def)) => struct_size(name, def, declaring, &mut walk, 0),
        Some(decl::Kind::UnionDef(def)) => union_size(name, def, declaring, &mut walk, 0),
        // The root box message `<Name>Box { <type> value = 1; }` the proto
        // backend emits (`emit_root_boxes`): one field numbered 1 whose type
        // is the declaration itself, sized by the same rules as a struct
        // field of that type. A named scalar without a leaf has no size at
        // all; an enum and an enum set always have a leaf.
        Some(decl::Kind::TypeDef(_) | decl::Kind::EnumDef(_) | decl::Kind::EnumSetDef(_)) => {
            let Some(leaf) = leaf_of_decl(decl, declaring) else {
                return SizeState::Absent(AbsentCause::NoBound);
            };
            leaf_field(1, leaf, &mut walk, 0)
        }
        Some(
            decl::Kind::ConstDef(_)
            | decl::Kind::SignalDef(_)
            | decl::Kind::EventDef(_)
            | decl::Kind::CommandDef(_)
            | decl::Kind::QueryDef(_)
            | decl::Kind::FixedDef(_)
            | decl::Kind::ReservedSlot(_),
        )
        | None => return SizeState::Absent(AbsentCause::Unresolved),
    };
    match bytes {
        // A bound the message derived but a `u32` cannot hold is an overflow.
        Some(bytes) => match u32::try_from(bytes) {
            Ok(bytes) => SizeState::Bounded(bytes),
            Err(_) => SizeState::Absent(AbsentCause::Overflow),
        },
        None => SizeState::Absent(walk.cause.unwrap_or(AbsentCause::RefusedMember)),
    }
}

/// The proto3 state of an induced argument or reply struct, read from
/// `home`: the bound of its message, with the causes [`state`] gives.
pub(crate) fn tuple_state<'a>(home: &'a Package, tuple: &'a TupleType, ctx: &Ctx<'a>) -> SizeState {
    let mut walk = Walk {
        ctx,
        memo: HashMap::new(),
        cause: None,
        #[cfg(test)]
        bodies: 0,
    };
    match tuple_size(tuple, home, &mut walk, 0) {
        Some(bytes) => match u32::try_from(bytes) {
            Ok(bytes) => SizeState::Bounded(bytes),
            Err(_) => SizeState::Absent(AbsentCause::Overflow),
        },
        None => SizeState::Absent(walk.cause.unwrap_or(AbsentCause::RefusedMember)),
    }
}

/// The field `number` of type `ty`. A field number the proto backend refuses
/// is `RefusedMember` whatever the field's type: it is checked before the
/// type is read, so a leaf with no bound behind a refused number does not
/// turn the refusal into `NoBound`.
fn field_size<'a>(
    number: u32,
    ty: &'a FieldType,
    home: &'a Package,
    walk: &mut Walk<'a, '_>,
    depth: u32,
) -> Option<u64> {
    if PROTO_RESERVED.contains(&number) || number > PROTO_MAX_FIELD_NUMBER {
        return None;
    }
    let leaf = walk.leaf_of(ty, home)?;
    leaf_field(number, leaf, walk, depth)
}

/// The largest encoding of one enum value: a negative member encodes as a
/// ten-byte varint, otherwise the largest value decides. `None` when a live
/// member or a retired value is outside proto3's int32 range, which
/// `emit_enum` in the proto backend refuses.
fn enum_len(min: i64, max: i64, retired_in_int32: bool) -> Option<u64> {
    if !retired_in_int32 {
        return None;
    }
    i32::try_from(min).ok()?;
    i32::try_from(max).ok()?;
    Some(if min < 0 {
        10
    } else {
        varint_len(max.unsigned_abs())
    })
}

fn leaf_field<'a>(number: u32, leaf: Leaf<'a>, walk: &mut Walk<'a, '_>, depth: u32) -> Option<u64> {
    if depth > MAX_DEPTH {
        return walk.fail(AbsentCause::Overflow);
    }
    if PROTO_RESERVED.contains(&number) || number > PROTO_MAX_FIELD_NUMBER {
        return None;
    }
    match leaf {
        Leaf::Scalar(s) => {
            let len = plain(number, s.max_encoded_len().expect(SCALAR_IS_BOUNDED));
            walk.fits(len)
        }
        Leaf::Blob(bytes) => walk.fits(delimited(number, bytes)),
        Leaf::Enum {
            min,
            max,
            retired_in_int32,
        } => {
            let len = plain(number, enum_len(min, max, retired_in_int32)?);
            walk.fits(len)
        }
        Leaf::Struct { name, def, home } => {
            let payload = struct_size(name, def, home, walk, depth + 1)?;
            walk.fits(delimited(number, payload))
        }
        Leaf::Union { name, def, home } => {
            let payload = union_size(name, def, home, walk, depth + 1)?;
            walk.fits(delimited(number, payload))
        }
        Leaf::Tuple { def, home } => {
            let payload = tuple_size(def, home, walk, depth + 1)?;
            walk.fits(delimited(number, payload))
        }
        Leaf::Array { def, home } => array_field(number, def, home, walk, depth + 1),
        Leaf::Map { def, home } => map_field(number, def, home, walk, depth + 1),
    }
}

/// The message of the struct `name` declared in `home`: the sum of its
/// fields. A reserved member is a retired field number and costs nothing.
fn struct_size<'a>(
    name: &'a str,
    def: &'a StructDef,
    home: &'a Package,
    walk: &mut Walk<'a, '_>,
    depth: u32,
) -> Option<u64> {
    walk.remembered(home, name, |walk| {
        let mut sum = 0u64;
        for member in &def.members {
            let field = match &member.member {
                Some(struct_member::Member::Field(field)) => {
                    let ty = field.r#type.as_ref()?;
                    // The proto backend refuses an optional array or map
                    // (`emit_struct`, ADR-0013 decision 7).
                    if proto3_projection::optional_repeated(ty).is_some() {
                        return None;
                    }
                    field_size(field.ordinal, ty, home, walk, depth)?
                }
                Some(struct_member::Member::Reserved(_)) | None => 0,
            };
            sum = walk.fits(sum.checked_add(field))?;
        }
        Some(sum)
    })
}

/// The message of the union `name` declared in `home`: its largest arm, as
/// a field of the `oneof` at the arm's ordinal. The backend's name-collision
/// refusals in `emit_union` — an arm named `value`, the `oneof`'s own name,
/// and two arms that project to one name — are not reproduced (module doc,
/// driftsys/ridl#690).
fn union_size<'a>(
    name: &'a str,
    def: &'a UnionDef,
    home: &'a Package,
    walk: &mut Walk<'a, '_>,
    depth: u32,
) -> Option<u64> {
    walk.remembered(home, name, |walk| {
        let mut largest = 0;
        for arm in &def.arms {
            let leaf = walk.leaf_of_named(&arm.type_ref, home)?;
            largest = largest.max(leaf_field(arm.ordinal, leaf, walk, depth)?);
        }
        Some(largest)
    })
}

/// A tuple is an induced message with positional fields 1..n. The proto
/// backend refuses an optional array or map tuple field, as it does a struct
/// field (`emit_induced_tuple`, ADR-0013 decision 7).
fn tuple_size<'a>(
    tuple: &'a TupleType,
    home: &'a Package,
    walk: &mut Walk<'a, '_>,
    depth: u32,
) -> Option<u64> {
    let mut sum = 0u64;
    for (i, f) in tuple.fields.iter().enumerate() {
        let ty = f.r#type.as_ref()?;
        if proto3_projection::optional_repeated(ty).is_some() {
            return None;
        }
        let field = field_size(u32::try_from(i + 1).ok()?, ty, home, walk, depth)?;
        sum = walk.fits(sum.checked_add(field))?;
    }
    Some(sum)
}

/// `repeated`: a scalar and an enum are packed (one tag, one length, the
/// values); every other element repeats tag and length. A nested array or
/// map is refused by the projection (`resolve_field_type` in the proto
/// backend).
fn array_field<'a>(
    number: u32,
    array: &'a ArrayType,
    home: &'a Package,
    walk: &mut Walk<'a, '_>,
    depth: u32,
) -> Option<u64> {
    let element = walk.leaf_of(array.element.as_ref()?, home)?;
    let n = array.max;
    match element {
        Leaf::Scalar(s) => {
            let run = walk.fits(n.checked_mul(s.max_encoded_len().expect(SCALAR_IS_BOUNDED)))?;
            walk.fits(delimited(number, run))
        }
        Leaf::Enum {
            min,
            max,
            retired_in_int32,
        } => {
            let run = walk.fits(n.checked_mul(enum_len(min, max, retired_in_int32)?))?;
            walk.fits(delimited(number, run))
        }
        Leaf::Array { .. } | Leaf::Map { .. } => None,
        other => {
            let each = leaf_field(number, other, walk, depth)?;
            walk.fits(n.checked_mul(each))
        }
    }
}

/// The proto3 scalar a map key projects to, as `map_key_text` in the proto
/// backend resolves it: a primitive, an inline scalar, a named scalar or an
/// enum set. A named struct, enum or union is a message or enum name, which
/// proto3 does not admit as a key; neither does any other key kind.
fn map_key_scalar(key: &FieldType, home: &Package, ctx: &Ctx<'_>) -> Option<Scalar> {
    match key.kind.as_ref()? {
        field_type::Kind::Primitive(primitive) => Some(proto3_projection::primitive(*primitive)),
        field_type::Kind::InlineScalar(td) => Some(proto3_projection::scalar(td)),
        field_type::Kind::Named(name) => match ctx.resolve(home, name)?.0.kind.as_ref()? {
            decl::Kind::TypeDef(td) => Some(proto3_projection::scalar(td)),
            decl::Kind::EnumSetDef(def) => Some(proto3_projection::enum_set_scalar(def.width)),
            _ => None,
        },
        _ => None,
    }
}

/// `map<K, V>`: `max` entries, each a message with the key at 1 and the
/// value at 2. The proto backend refuses a key outside the integral and
/// string scalars (`map_key_text`) and a value that is an array or a map
/// (`resolve_field_type`).
fn map_field<'a>(
    number: u32,
    map: &'a MapType,
    home: &'a Package,
    walk: &mut Walk<'a, '_>,
    depth: u32,
) -> Option<u64> {
    let key = map.key.as_ref()?;
    let value = map.value.as_ref()?;
    if !map_key_scalar(key, home, walk.ctx)?.admitted_as_map_key()
        || matches!(
            value.kind,
            Some(field_type::Kind::Array(_) | field_type::Kind::Map(_))
        )
    {
        return None;
    }
    let key = field_size(1, key, home, walk, depth)?;
    let value = field_size(2, value, home, walk, depth)?;
    let entry = walk.fits(key.checked_add(value))?;
    let each = walk.fits(delimited(number, entry))?;
    walk.fits(map.max.checked_mul(each))
}

#[cfg(test)]
mod tests {
    use super::super::tests_support::*;
    use super::super::{AbsentCause, Encoding, PayloadShape, size_state};
    use super::*;
    use crate::v2::{
        Backing, Constraint, Decl, EnumDef, EnumSetDef, EnumValue, Field, FloatWidth, IntWidth,
        Package, PrimitiveType, Reserved, StructMember, TupleField, TypeDef, UnionArm, UnionDef,
        backing, decl, field_type, type_def,
    };

    #[test]
    fn varint_lengths() {
        assert_eq!(varint_len(0), 1);
        assert_eq!(varint_len(127), 1);
        assert_eq!(varint_len(128), 2);
        assert_eq!(varint_len(u64::MAX), 10);
    }

    #[test]
    fn a_struct_payload_is_the_message_itself() {
        // Point { x: Coord, y: Coord }, Coord = integer i16: two sint32
        // fields, tag 1 byte + value 5 bytes each.
        let package = fixture();
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(state_in("Point", &ctx), SizeState::Bounded(12));
    }

    #[test]
    fn a_named_scalar_is_sized_as_its_root_box_message() {
        // The proto backend gives a named scalar the root message
        // `<Name>Box { <type> value = 1; }`, so a payload of one is the size
        // of that one field:
        //   Coord, an i16 `sint32`:          1 + 5                    = 6
        //   Flt, an f32 `float`:             1 + 4                    = 5
        //   Vin, `string [17..17]`, at most 17 * 4 = 68 bytes:
        //                                    1 + varint_len(68) + 68  = 70
        //   Long, `string [0..200]`, at most 200 * 4 = 800 bytes:
        //                                    1 + varint_len(800) + 800
        //                                    = 1 + 2 + 800            = 803
        let mut package = fixture();
        package.decls.push(Decl {
            name: "Flt".to_owned(),
            kind: Some(decl::Kind::TypeDef(TypeDef {
                backing: Some(Backing {
                    kind: Some(backing::Kind::Primitive(PrimitiveType::Float as i32)),
                }),
                width: Some(type_def::Width::FloatWidth(FloatWidth::F32 as i32)),
                ..Default::default()
            })),
            ..Default::default()
        });
        package.decls.push(Decl {
            name: "Long".to_owned(),
            kind: Some(decl::Kind::TypeDef(TypeDef {
                backing: Some(Backing {
                    kind: Some(backing::Kind::Primitive(PrimitiveType::String as i32)),
                }),
                constraint: Some(Constraint {
                    len_max: Some(200),
                    ..Default::default()
                }),
                ..Default::default()
            })),
            ..Default::default()
        });
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(state_in("Coord", &ctx), SizeState::Bounded(6));
        assert_eq!(state_in("Flt", &ctx), SizeState::Bounded(5));
        assert_eq!(
            state_in("Vin", &ctx),
            SizeState::Bounded(u32::try_from(1 + varint_len(68) + 68).unwrap())
        );
        assert_eq!(state_in("Long", &ctx), SizeState::Bounded(803));
    }

    #[test]
    fn a_named_scalar_no_leaf_bounds_has_no_bound() {
        // `Loose = string` carries no length bound, so no proto3 leaf bounds
        // the value at all, and its root box message has no bound either.
        let mut package = fixture();
        package.decls.push(Decl {
            name: "Loose".to_owned(),
            kind: Some(decl::Kind::TypeDef(TypeDef {
                backing: Some(Backing {
                    kind: Some(backing::Kind::Primitive(PrimitiveType::String as i32)),
                }),
                ..Default::default()
            })),
            ..Default::default()
        });
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(
            state_in("Loose", &ctx),
            SizeState::Absent(AbsentCause::NoBound)
        );
    }

    #[test]
    fn an_enum_set_is_sized_as_its_root_box_message() {
        // `Flags` of width u8 is a `uint32` at a field position, so its root
        // box message `FlagsBox { uint32 value = 1; }` is 1 + 5 = 6.
        let mut package = fixture();
        package.decls.push(Decl {
            name: "Flags".to_owned(),
            kind: Some(decl::Kind::EnumSetDef(EnumSetDef {
                width: IntWidth::U8 as i32,
                ..Default::default()
            })),
            ..Default::default()
        });
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(state_in("Flags", &ctx), SizeState::Bounded(6));
    }

    #[test]
    fn string_array_and_map_fields_inside_a_message() {
        // Bag { tag: Vin @1, flags: [u8; 0..4] @2, index: {string [0..8]: u32; 0..2} @3 }:
        //   tag:   1 + 1 + 17 * 4            = 70
        //   flags: packed, 1 + 1 + 4 * 5     = 22
        //   index: entry (1+1+32) + (1+5) = 40; 2 * (1 + 1 + 40) = 84
        let package = fixture();
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(state_in("Bag", &ctx), SizeState::Bounded(176));
    }

    #[test]
    fn a_union_payload_is_its_message_with_the_largest_arm() {
        // Shape = Point @1 | Coord @2: arm a 1 + 1 + 12 = 14, arm b 1 + 5 = 6.
        let package = fixture();
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(state_in("Shape", &ctx), SizeState::Bounded(14));
    }

    #[test]
    fn an_unresolved_name_is_absent() {
        let package = fixture();
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(
            state_in("Missing", &ctx),
            SizeState::Absent(AbsentCause::Unresolved)
        );
    }

    fn struct_decl(name: &str, fields: Vec<(&str, u32, FieldType)>) -> Decl {
        Decl {
            name: name.to_owned(),
            kind: Some(decl::Kind::StructDef(StructDef {
                members: fields
                    .into_iter()
                    .map(|(name, ordinal, ty)| StructMember {
                        member: Some(struct_member::Member::Field(Box::new(Field {
                            name: name.to_owned(),
                            ordinal,
                            r#type: Some(ty),
                            ..Default::default()
                        }))),
                    })
                    .collect(),
                fixed_layout: false,
            })),
            ..Default::default()
        }
    }

    fn field(kind: field_type::Kind) -> FieldType {
        FieldType {
            optional: false,
            kind: Some(kind),
        }
    }

    fn map_of(key: FieldType, value: FieldType, max: u64) -> FieldType {
        field(field_type::Kind::Map(Box::new(MapType {
            key: Some(Box::new(key)),
            value: Some(Box::new(value)),
            min: 0,
            max,
        })))
    }

    fn bytes_max(len_max: u64) -> FieldType {
        inline(TypeDef {
            backing: Some(Backing {
                kind: Some(backing::Kind::Primitive(PrimitiveType::Bytes as i32)),
            }),
            constraint: Some(Constraint {
                len_max: Some(len_max),
                ..Default::default()
            }),
            ..Default::default()
        })
    }

    fn enum_decl(name: &str, values: &[i64]) -> Decl {
        Decl {
            name: name.to_owned(),
            kind: Some(decl::Kind::EnumDef(EnumDef {
                values: values
                    .iter()
                    .map(|&value| EnumValue {
                        name: format!("V{}", value.unsigned_abs()),
                        value,
                        doc: String::new(),
                        links: Vec::new(),
                        see: Vec::new(),
                        since: Vec::new(),
                    })
                    .collect(),
                reserved: Vec::new(),
            })),
            ..Default::default()
        }
    }

    fn array_of(element: FieldType, max: u64) -> FieldType {
        field(field_type::Kind::Array(Box::new(ArrayType {
            element: Some(Box::new(element)),
            min: 0,
            max,
        })))
    }

    #[test]
    fn enum_tuple_and_repeated_message_fields() {
        // Mix { gear: Gear @1, pair: (Coord, boolean) @2, pts: [Point; 0..3] @3 }
        // with Gear = enum { R = -1, N = 0, D = 7 }:
        //   gear: a negative member, so a 10-byte varint: 1 + 10   = 11
        //   pair: an induced message, sint32 at 1 (1 + 5) and bool at 2
        //         (1 + 1) = 8; delimited: 1 + 1 + 8                 = 10
        //   pts:  a message element is not packed: 3 * (1 + 1 + 12) = 42
        let mut package = fixture();
        package.decls.push(Decl {
            name: "Gear".to_owned(),
            kind: Some(decl::Kind::EnumDef(EnumDef {
                values: [("R", -1), ("N", 0), ("D", 7)]
                    .into_iter()
                    .map(|(name, value)| EnumValue {
                        name: name.to_owned(),
                        value,
                        doc: String::new(),
                        links: Vec::new(),
                        see: Vec::new(),
                        since: Vec::new(),
                    })
                    .collect(),
                reserved: Vec::new(),
            })),
            ..Default::default()
        });
        let pair = field(field_type::Kind::Tuple(TupleType {
            fields: vec![
                TupleField {
                    name: "a".to_owned(),
                    r#type: Some(named("Coord")),
                },
                TupleField {
                    name: "b".to_owned(),
                    r#type: Some(field(field_type::Kind::Primitive(
                        PrimitiveType::Boolean as i32,
                    ))),
                },
            ],
        }));
        package.decls.push(struct_decl(
            "Mix",
            vec![
                ("gear", 1, named("Gear")),
                ("pair", 2, pair),
                ("pts", 3, array_of(named("Point"), 3)),
            ],
        ));
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(state_in("Mix", &ctx), SizeState::Bounded(63));
        // The root box message `GearBox { Gear value = 1; }`: a negative
        // member, so a 10-byte varint: 1 + 10 = 11.
        assert_eq!(state_in("Gear", &ctx), SizeState::Bounded(11));
    }

    #[test]
    fn a_member_the_projection_refuses_makes_the_message_absent() {
        // The proto backend refuses an array of arrays (`resolve_field_type`),
        // so a message that holds one has no proto3 form.
        let mut package = fixture();
        package.decls.push(struct_decl(
            "Grid",
            vec![("rows", 1, array_of(array_u8_max4(), 2))],
        ));
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(
            state_in("Grid", &ctx),
            SizeState::Absent(AbsentCause::RefusedMember)
        );
    }

    /// A size problem is `NoBound` or `Overflow`, never `RefusedMember`: a
    /// struct holding a string with no length bound, through a named scalar
    /// or inline, an array of such strings, and an array of a bounded
    /// element with no useful count maximum. The walk stops at its first
    /// failing member, so a refused member before one with no bound decides,
    /// and one after it does not.
    #[test]
    fn a_member_with_no_bound_is_no_bound_and_not_a_refused_member() {
        let mut package = fixture();
        let free = TypeDef {
            backing: Some(Backing {
                kind: Some(backing::Kind::Primitive(PrimitiveType::String as i32)),
            }),
            ..Default::default()
        };
        package.decls.push(Decl {
            name: "Free".to_owned(),
            kind: Some(decl::Kind::TypeDef(free.clone())),
            ..Default::default()
        });
        package
            .decls
            .push(struct_decl("Note", vec![("text", 1, named("Free"))]));
        package.decls.push(struct_decl(
            "Inline",
            vec![("text", 1, inline(free.clone()))],
        ));
        package.decls.push(struct_decl(
            "Tags",
            vec![("tags", 1, array_of(named("Free"), u64::MAX))],
        ));
        package.decls.push(struct_decl(
            "Many",
            vec![("pts", 1, array_of(named("Point"), u64::MAX))],
        ));
        package.decls.push(struct_decl(
            "Holder",
            vec![("note", 1, named("Note")), ("pt", 2, named("Point"))],
        ));
        package.decls.push(struct_decl(
            "RefusedFirst",
            vec![
                ("rows", 1, array_of(array_u8_max4(), 2)),
                ("text", 2, named("Free")),
            ],
        ));
        package.decls.push(struct_decl(
            "NoBoundFirst",
            vec![
                ("text", 1, named("Free")),
                ("rows", 2, array_of(array_u8_max4(), 2)),
            ],
        ));
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        for name in ["Free", "Note", "Inline", "Tags", "Holder", "NoBoundFirst"] {
            assert_eq!(
                state_in(name, &ctx),
                SizeState::Absent(AbsentCause::NoBound),
                "{name}"
            );
        }
        assert_eq!(
            state_in("Many", &ctx),
            SizeState::Absent(AbsentCause::Overflow)
        );
        assert_eq!(
            state_in("RefusedFirst", &ctx),
            SizeState::Absent(AbsentCause::RefusedMember)
        );
    }

    #[test]
    fn a_bound_above_u32_max_is_absent() {
        // `bytes [0..u32::MAX]` plus its tag and length exceeds `u32::MAX`.
        let mut package = fixture();
        let wide = inline(TypeDef {
            backing: Some(Backing {
                kind: Some(backing::Kind::Primitive(PrimitiveType::Bytes as i32)),
            }),
            constraint: Some(Constraint {
                len_max: Some(u64::from(u32::MAX)),
                ..Default::default()
            }),
            ..Default::default()
        });
        package
            .decls
            .push(struct_decl("Wide", vec![("blob", 1, wide)]));
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(
            state_in("Wide", &ctx),
            SizeState::Absent(AbsentCause::Overflow)
        );
    }

    #[test]
    fn a_near_u64_max_array_bound_is_absent() {
        // `[u8; 0..u64::MAX / 5]`: 5 bytes per packed `uint32` value gives
        // exactly `u64::MAX`, and the tag and length then do not fit. A
        // wrapping addition would publish a small false bound.
        let mut package = fixture();
        let u8_def = TypeDef {
            backing: Some(Backing {
                kind: Some(backing::Kind::Primitive(PrimitiveType::Integer as i32)),
            }),
            width: Some(type_def::Width::IntWidth(IntWidth::U8 as i32)),
            ..Default::default()
        };
        package.decls.push(struct_decl(
            "Huge",
            vec![("items", 1, array_of(inline(u8_def), u64::MAX / 5))],
        ));
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(
            state_in("Huge", &ctx),
            SizeState::Absent(AbsentCause::Overflow)
        );
    }

    #[test]
    fn a_near_u64_max_bytes_bound_is_absent() {
        let mut package = fixture();
        package.decls.push(struct_decl(
            "Vast",
            vec![("blob", 1, bytes_max(u64::MAX - 1))],
        ));
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(
            state_in("Vast", &ctx),
            SizeState::Absent(AbsentCause::Overflow)
        );
    }

    #[test]
    fn a_map_key_outside_the_integral_and_string_scalars_is_refused() {
        // `map_key_text` in the proto backend admits bool, the integral
        // scalars and string; a float or bytes key is refused.
        let mut package = fixture();
        let f32_def = TypeDef {
            backing: Some(Backing {
                kind: Some(backing::Kind::Primitive(PrimitiveType::Float as i32)),
            }),
            width: Some(type_def::Width::FloatWidth(FloatWidth::F32 as i32)),
            ..Default::default()
        };
        package.decls.push(struct_decl(
            "FloatKeyed",
            vec![("m", 1, map_of(inline(f32_def), named("Coord"), 2))],
        ));
        package.decls.push(struct_decl(
            "BytesKeyed",
            vec![("m", 1, map_of(bytes_max(8), named("Coord"), 2))],
        ));
        package.decls.push(struct_decl(
            "MessageKeyed",
            vec![("m", 1, map_of(named("Point"), named("Coord"), 2))],
        ));
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(
            state_in("FloatKeyed", &ctx),
            SizeState::Absent(AbsentCause::RefusedMember)
        );
        assert_eq!(
            state_in("BytesKeyed", &ctx),
            SizeState::Absent(AbsentCause::RefusedMember)
        );
        assert_eq!(
            state_in("MessageKeyed", &ctx),
            SizeState::Absent(AbsentCause::RefusedMember)
        );
        // The control: `Bag` carries a string-keyed map and is bounded.
        assert_eq!(state_in("Bag", &ctx), SizeState::Bounded(176));
    }

    #[test]
    fn a_map_value_that_is_an_array_or_a_map_is_refused() {
        // `resolve_field_type` in the proto backend refuses both.
        let mut package = fixture();
        package.decls.push(struct_decl(
            "ArrayValued",
            vec![("m", 1, map_of(named("Coord"), array_u8_max4(), 2))],
        ));
        package.decls.push(struct_decl(
            "MapValued",
            vec![("m", 1, map_of(named("Coord"), map_str8_u32_max2(), 2))],
        ));
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(
            state_in("ArrayValued", &ctx),
            SizeState::Absent(AbsentCause::RefusedMember)
        );
        assert_eq!(
            state_in("MapValued", &ctx),
            SizeState::Absent(AbsentCause::RefusedMember)
        );
    }

    #[test]
    fn an_optional_array_or_map_field_is_refused() {
        // `emit_struct` in the proto backend refuses both (ADR-0013
        // decision 7): proto3 cannot mark a repeated or map field absent.
        let optional = |mut ty: FieldType| {
            ty.optional = true;
            ty
        };
        let mut package = fixture();
        package.decls.push(struct_decl(
            "OptionalArray",
            vec![("flags", 1, optional(array_u8_max4()))],
        ));
        package.decls.push(struct_decl(
            "OptionalMap",
            vec![("index", 1, optional(map_str8_u32_max2()))],
        ));
        package.decls.push(struct_decl(
            "OptionalScalar",
            vec![("x", 1, optional(named("Coord")))],
        ));
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(
            state_in("OptionalArray", &ctx),
            SizeState::Absent(AbsentCause::RefusedMember)
        );
        assert_eq!(
            state_in("OptionalMap", &ctx),
            SizeState::Absent(AbsentCause::RefusedMember)
        );
        assert_eq!(
            state_in("OptionalScalar", &ctx),
            SizeState::Bounded(6),
            "an optional scalar is admitted and costs no more than a required one"
        );
    }

    #[test]
    fn an_optional_array_or_map_tuple_field_is_refused() {
        // `emit_induced_tuple` in the proto backend refuses both, as
        // `emit_struct` does for a struct field (ADR-0013 decision 7).
        let optional = |mut ty: FieldType| {
            ty.optional = true;
            ty
        };
        let tuple = |ty: FieldType| {
            field(field_type::Kind::Tuple(TupleType {
                fields: vec![TupleField {
                    name: "x".to_owned(),
                    r#type: Some(ty),
                }],
            }))
        };
        let mut package = fixture();
        package.decls.push(struct_decl(
            "TupleOptionalArray",
            vec![("t", 1, tuple(optional(array_u8_max4())))],
        ));
        package.decls.push(struct_decl(
            "TupleOptionalMap",
            vec![("t", 1, tuple(optional(map_str8_u32_max2())))],
        ));
        package.decls.push(struct_decl(
            "TupleOptionalScalar",
            vec![("t", 1, tuple(optional(named("Coord"))))],
        ));
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(
            state_in("TupleOptionalArray", &ctx),
            SizeState::Absent(AbsentCause::RefusedMember)
        );
        assert_eq!(
            state_in("TupleOptionalMap", &ctx),
            SizeState::Absent(AbsentCause::RefusedMember)
        );
        // The control: an optional scalar tuple field is admitted. The
        // induced message is 1 + 5 = 6, and the field holding it 1 + 1 + 6.
        assert_eq!(state_in("TupleOptionalScalar", &ctx), SizeState::Bounded(8));
    }

    #[test]
    fn an_enum_value_outside_int32_is_refused() {
        // `emit_enum` in the proto backend refuses the value.
        let mut package = fixture();
        package
            .decls
            .push(enum_decl("Wide", &[0, i64::from(i32::MAX) + 1]));
        package
            .decls
            .push(enum_decl("Deep", &[i64::from(i32::MIN) - 1, 0]));
        package.decls.push(enum_decl(
            "Edge",
            &[i64::from(i32::MIN), i64::from(i32::MAX)],
        ));
        package
            .decls
            .push(struct_decl("HoldsWide", vec![("e", 1, named("Wide"))]));
        package
            .decls
            .push(struct_decl("HoldsDeep", vec![("e", 1, named("Deep"))]));
        package
            .decls
            .push(struct_decl("HoldsEdge", vec![("e", 1, named("Edge"))]));
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(
            state_in("HoldsWide", &ctx),
            SizeState::Absent(AbsentCause::RefusedMember)
        );
        assert_eq!(
            state_in("HoldsDeep", &ctx),
            SizeState::Absent(AbsentCause::RefusedMember)
        );
        assert_eq!(
            state_in("HoldsEdge", &ctx),
            SizeState::Bounded(11),
            "the int32 bounds themselves are admitted; a negative member is a 10-byte varint"
        );
        // The root box message of each enum holds it as a field, so the same
        // refusal makes the enum's own payload absent.
        assert_eq!(
            state_in("Wide", &ctx),
            SizeState::Absent(AbsentCause::RefusedMember)
        );
        assert_eq!(
            state_in("Deep", &ctx),
            SizeState::Absent(AbsentCause::RefusedMember)
        );
        assert_eq!(state_in("Edge", &ctx), SizeState::Bounded(11));
    }

    #[test]
    fn a_retired_enum_value_outside_int32_is_refused() {
        // `emit_enum` in the proto backend refuses a retired value outside
        // int32 even when every live value is in range.
        let with_retired = |name: &str, retired: i64| {
            let mut decl = enum_decl(name, &[0, 1]);
            if let Some(decl::Kind::EnumDef(def)) = decl.kind.as_mut() {
                def.reserved.push(Reserved {
                    value: Some(retired),
                    ..Default::default()
                });
            }
            decl
        };
        let mut package = fixture();
        package
            .decls
            .push(with_retired("Retired", i64::from(i32::MAX) + 1));
        package
            .decls
            .push(with_retired("RetiredEdge", i64::from(i32::MIN)));
        package.decls.push(struct_decl(
            "HoldsRetired",
            vec![("e", 1, named("Retired"))],
        ));
        package.decls.push(struct_decl(
            "HoldsRetiredEdge",
            vec![("e", 1, named("RetiredEdge"))],
        ));
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(
            state_in("HoldsRetired", &ctx),
            SizeState::Absent(AbsentCause::RefusedMember)
        );
        assert_eq!(
            state_in("HoldsRetiredEdge", &ctx),
            SizeState::Bounded(2),
            "a retired value inside int32 is admitted and does not change the live values' size"
        );
        assert_eq!(
            state_in("Retired", &ctx),
            SizeState::Absent(AbsentCause::RefusedMember),
            "the root box message holds the enum as a field, so the same refusal applies"
        );
        assert_eq!(state_in("RetiredEdge", &ctx), SizeState::Bounded(2));
    }

    #[test]
    fn a_field_number_the_backend_refuses_makes_the_message_absent() {
        // `check_field_number` in the proto backend refuses 19000..=19999 and
        // anything above 536870911, in a struct, a union arm and a tuple.
        let mut package = fixture();
        package
            .decls
            .push(struct_decl("Reserved", vec![("x", 19_000, named("Coord"))]));
        package.decls.push(struct_decl(
            "Reserved2",
            vec![("x", 19_999, named("Coord"))],
        ));
        package.decls.push(struct_decl(
            "TooHigh",
            vec![("x", 536_870_912, named("Coord"))],
        ));
        package.decls.push(struct_decl(
            "Highest",
            vec![("x", 536_870_911, named("Coord"))],
        ));
        package.decls.push(Decl {
            name: "ArmReserved".to_owned(),
            kind: Some(decl::Kind::UnionDef(UnionDef {
                arms: vec![UnionArm {
                    name: "a".to_owned(),
                    ordinal: 19_500,
                    type_ref: "Coord".to_owned(),
                    ..Default::default()
                }],
                ..Default::default()
            })),
            ..Default::default()
        });
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(
            state_in("Reserved", &ctx),
            SizeState::Absent(AbsentCause::RefusedMember)
        );
        assert_eq!(
            state_in("Reserved2", &ctx),
            SizeState::Absent(AbsentCause::RefusedMember)
        );
        assert_eq!(
            state_in("TooHigh", &ctx),
            SizeState::Absent(AbsentCause::RefusedMember)
        );
        assert_eq!(
            state_in("ArmReserved", &ctx),
            SizeState::Absent(AbsentCause::RefusedMember)
        );
        // The largest field number takes a 5-byte tag: 5 + 5.
        assert_eq!(state_in("Highest", &ctx), SizeState::Bounded(10));
    }

    /// A member proto3 cannot represent is `RefusedMember` whatever else the
    /// message holds: a stream field, a field naming a declaration that does
    /// not resolve, and a field naming a constant. A refused field number is
    /// checked before the field's type is read, so a field with no bound
    /// behind a reserved number is `RefusedMember` rather than `NoBound`.
    #[test]
    fn a_stream_an_unresolved_name_a_constant_and_a_refused_number_are_refused_members() {
        let mut package = fixture();
        let free = TypeDef {
            backing: Some(Backing {
                kind: Some(backing::Kind::Primitive(PrimitiveType::String as i32)),
            }),
            ..Default::default()
        };
        package.decls.push(Decl {
            name: "Free".to_owned(),
            kind: Some(decl::Kind::TypeDef(free)),
            ..Default::default()
        });
        let stream = field(field_type::Kind::Stream(crate::v2::StreamType {
            element: Some(crate::v2::stream_type::Element::Named("Point".to_owned())),
        }));
        package
            .decls
            .push(struct_decl("Streamed", vec![("pts", 1, stream)]));
        package
            .decls
            .push(struct_decl("Dangling", vec![("x", 1, named("Missing"))]));
        // `LIMIT` is the fixture's constant: it projects no type.
        package
            .decls
            .push(struct_decl("Limited", vec![("x", 1, named("LIMIT"))]));
        package.decls.push(struct_decl(
            "ReservedFree",
            vec![("text", 19_000, named("Free"))],
        ));
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        for name in ["Streamed", "Dangling", "Limited", "ReservedFree"] {
            assert_eq!(
                state_in(name, &ctx),
                SizeState::Absent(AbsentCause::RefusedMember),
                "{name}"
            );
        }
        // The same leaf behind an admitted number is the size problem.
        package
            .decls
            .push(struct_decl("PlainFree", vec![("text", 1, named("Free"))]));
        let ctx = Ctx::new(&package, &others);
        assert_eq!(
            state_in("PlainFree", &ctx),
            SizeState::Absent(AbsentCause::NoBound)
        );
    }

    fn union_decl(name: &str, arms: &[(&str, u32, &str)]) -> Decl {
        Decl {
            name: name.to_owned(),
            kind: Some(decl::Kind::UnionDef(UnionDef {
                arms: arms
                    .iter()
                    .map(|&(name, ordinal, type_ref)| UnionArm {
                        name: name.to_owned(),
                        ordinal,
                        type_ref: type_ref.to_owned(),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            })),
            ..Default::default()
        }
    }

    fn string_max(len_max: u64) -> FieldType {
        inline(TypeDef {
            backing: Some(Backing {
                kind: Some(backing::Kind::Primitive(PrimitiveType::String as i32)),
            }),
            constraint: Some(Constraint {
                len_max: Some(len_max),
                ..Default::default()
            }),
            ..Default::default()
        })
    }

    fn primitive(primitive: PrimitiveType) -> FieldType {
        field(field_type::Kind::Primitive(primitive as i32))
    }

    /// [`state`] with the root of `ctx` as the home `type_name` is read
    /// from. A test that needs another home calls [`state`] directly.
    fn state_in<'a>(type_name: &str, ctx: &Ctx<'a>) -> SizeState {
        state(ctx.packages().package, type_name, ctx)
    }

    /// The bound of the struct or union `type_name`, and how many struct
    /// and union messages the walk derived rather than reused from its memo.
    fn walked(type_name: &str, ctx: &Ctx<'_>) -> (Option<u64>, u32) {
        let mut walk = Walk {
            ctx,
            memo: HashMap::new(),
            cause: None,
            bodies: 0,
        };
        let bound = match leaf_of_name(type_name, ctx.packages().package, ctx) {
            Some(Leaf::Struct { name, def, home }) => struct_size(name, def, home, &mut walk, 0),
            Some(Leaf::Union { name, def, home }) => union_size(name, def, home, &mut walk, 0),
            _ => panic!("{type_name} is not a struct or a union"),
        };
        (bound, walk.bodies)
    }

    #[test]
    fn a_deep_diamond_of_shared_structs_is_walked_once() {
        // S_k { x: S_{k+1} @1, y: S_{k+1} @2 } for k in 0..DEPTH, and
        // S_DEPTH { x: Coord @1 }. Every level reaches the next through both
        // fields, so a walk that remembers nothing derives 2^(DEPTH + 1) - 1
        // messages; one that remembers each struct derives DEPTH + 1. typl
        // admits the shape (TYPL-206 rejects a cycle, not sharing). The
        // bound doubles at each level: S_DEPTH = 6, and S_k is two
        // one-byte-tag fields of S_{k+1}, 2 * (1 + varint_len(S_{k+1}) +
        // S_{k+1}). The depth is low enough that a walk without the memo
        // ends, and the count fails instead of the walk not returning.
        const DEPTH: u32 = 16;
        let mut package = fixture();
        for k in 0..DEPTH {
            let next = format!("S{}", k + 1);
            package.decls.push(struct_decl(
                &format!("S{k}"),
                vec![("x", 1, named(&next)), ("y", 2, named(&next))],
            ));
        }
        package.decls.push(struct_decl(
            &format!("S{DEPTH}"),
            vec![("x", 1, named("Coord"))],
        ));
        let expected = (0..DEPTH).fold(6u64, |s, _| 2 * (1 + varint_len(s) + s));
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(walked("S0", &ctx), (Some(expected), DEPTH + 1));
    }

    #[test]
    fn a_deep_diamond_of_shared_unions_is_walked_once() {
        // U_k = a: U_{k+1} @1 | b: U_{k+1} @2 for k in 0..DEPTH, and
        // U_DEPTH = a: Coord @1. Both arms reach the next level, so a walk
        // that remembers nothing derives 2^(DEPTH + 1) - 1 messages; one
        // that remembers each union derives DEPTH + 1. The largest arm is
        // the one at ordinal 1: U_DEPTH = 6, U_k = 1 + 1 + U_{k+1} while the
        // length is one byte, so U_0 = 6 + 2 * DEPTH.
        const DEPTH: u32 = 16;
        let mut package = fixture();
        for k in 0..DEPTH {
            let next = format!("U{}", k + 1);
            package.decls.push(union_decl(
                &format!("U{k}"),
                &[("a", 1, &next), ("b", 2, &next)],
            ));
        }
        package
            .decls
            .push(union_decl(&format!("U{DEPTH}"), &[("a", 1, "Coord")]));
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(
            walked("U0", &ctx),
            (Some(6 + 2 * u64::from(DEPTH)), DEPTH + 1)
        );
    }

    #[test]
    fn the_memo_keys_on_the_declaring_package_as_well_as_the_name() {
        // p's `Inner { s: string [0..100] }` is 403 bytes and q's
        // `Inner { b: boolean }` is 2. `Both { a: Inner @1, b: q.Inner @2 }`
        // in p reaches the two in one walk: (1 + 2 + 403) + (1 + 1 + 2) =
        // 410, in either field order (`Swapped`). A memo keyed on the name
        // alone would answer the second `Inner` with the first one's bound:
        // 812 for `Both` and 8 for `Swapped`.
        let (mut root, imported) = two_package_fixture();
        root.decls.push(struct_decl(
            "Both",
            vec![("a", 1, named("Inner")), ("b", 2, named("q.Inner"))],
        ));
        root.decls.push(struct_decl(
            "Swapped",
            vec![("a", 1, named("q.Inner")), ("b", 2, named("Inner"))],
        ));
        let others = [&imported];
        let ctx = Ctx::new(&root, &others);
        assert_eq!(state_in("Both", &ctx), SizeState::Bounded(410));
        assert_eq!(state_in("Swapped", &ctx), SizeState::Bounded(410));
    }

    #[test]
    fn a_foreign_named_scalar_is_sized_as_the_root_box_of_its_own_package() {
        // q's `Speed` is an i16, a `sint32`: its box is 1 + 5 = 6. p declares
        // a `Speed` of its own, a boolean, whose box is 1 + 1 = 2. Sized from
        // p as `q.Speed`, the box must be q's; read in p by mistake, it would
        // be 2.
        let speed = |primitive: PrimitiveType, width: Option<IntWidth>| Decl {
            name: "Speed".to_owned(),
            kind: Some(decl::Kind::TypeDef(TypeDef {
                backing: Some(Backing {
                    kind: Some(backing::Kind::Primitive(primitive as i32)),
                }),
                width: width.map(|w| type_def::Width::IntWidth(w as i32)),
                ..Default::default()
            })),
            ..Default::default()
        };
        let (mut root, mut imported) = two_package_fixture();
        root.decls.push(speed(PrimitiveType::Boolean, None));
        imported
            .decls
            .push(speed(PrimitiveType::Integer, Some(IntWidth::I16)));
        let others = [&imported];
        let ctx = Ctx::new(&root, &others);
        assert_eq!(state_in("q.Speed", &ctx), SizeState::Bounded(6));
        assert_eq!(state_in("Speed", &ctx), SizeState::Bounded(2));
    }

    #[test]
    fn two_fields_whose_sum_wraps_are_absent() {
        // Each `bytes [0..2^63 - 1]` field is 1 + 9 + 2^63 - 1 = 2^63 + 9
        // bytes; the two together are 2^64 + 18, which wraps to 18.
        let mut package = fixture();
        let half = u64::MAX / 2;
        package.decls.push(struct_decl(
            "Both",
            vec![("a", 1, bytes_max(half)), ("b", 2, bytes_max(half))],
        ));
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(
            state_in("Both", &ctx),
            SizeState::Absent(AbsentCause::Overflow)
        );
    }

    #[test]
    fn a_packed_scalar_array_whose_count_wraps_is_absent() {
        // `[u8; 0..u64::MAX / 5 + 1]`: 5 bytes per packed `uint32` value is
        // 2^64 + 4, which wraps to 4 and would publish `Bounded(6)`.
        let mut package = fixture();
        let u8_def = TypeDef {
            backing: Some(Backing {
                kind: Some(backing::Kind::Primitive(PrimitiveType::Integer as i32)),
            }),
            width: Some(type_def::Width::IntWidth(IntWidth::U8 as i32)),
            ..Default::default()
        };
        package.decls.push(struct_decl(
            "Wrapped",
            vec![("items", 1, array_of(inline(u8_def), u64::MAX / 5 + 1))],
        ));
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(
            state_in("Wrapped", &ctx),
            SizeState::Absent(AbsentCause::Overflow)
        );
    }

    #[test]
    fn a_packed_enum_array_whose_count_wraps_is_absent() {
        // A negative member makes every value a 10-byte varint;
        // `u64::MAX / 10 + 1` of them is 2^64 + 4, which wraps to 4.
        let mut package = fixture();
        package.decls.push(enum_decl("Signed", &[-1, 0]));
        package.decls.push(struct_decl(
            "Wrapped",
            vec![("items", 1, array_of(named("Signed"), u64::MAX / 10 + 1))],
        ));
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(
            state_in("Wrapped", &ctx),
            SizeState::Absent(AbsentCause::Overflow)
        );
    }

    #[test]
    fn a_message_array_whose_count_wraps_is_absent() {
        // A `Point` element is 1 + 1 + 12 = 14 bytes; `u64::MAX / 14 + 1`
        // of them is 2^64 + 12, which wraps to 12.
        let mut package = fixture();
        package.decls.push(struct_decl(
            "Wrapped",
            vec![("pts", 1, array_of(named("Point"), u64::MAX / 14 + 1))],
        ));
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(
            state_in("Wrapped", &ctx),
            SizeState::Absent(AbsentCause::Overflow)
        );
    }

    #[test]
    fn a_map_entry_whose_key_and_value_sum_wraps_is_absent() {
        // A `string [0..2^61 - 1]` is 2^63 - 4 bytes; as a key or a value it
        // is 1 + 9 + 2^63 - 4 = 2^63 + 6 bytes, and the entry's two together
        // are 2^64 + 12, which wraps to 12.
        let mut package = fixture();
        let wide = (1u64 << 61) - 1;
        package.decls.push(struct_decl(
            "Wrapped",
            vec![("m", 1, map_of(string_max(wide), string_max(wide), 1))],
        ));
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(
            state_in("Wrapped", &ctx),
            SizeState::Absent(AbsentCause::Overflow)
        );
    }

    #[test]
    fn a_map_whose_entry_count_wraps_is_absent() {
        // `Bag`'s map entry is 1 + 1 + 40 = 42 bytes; `u64::MAX / 42 + 1`
        // entries is 2^64 + 26, which wraps to 26.
        let mut package = fixture();
        let mut index = map_str8_u32_max2();
        if let Some(field_type::Kind::Map(map)) = index.kind.as_mut() {
            map.max = u64::MAX / 42 + 1;
        }
        package
            .decls
            .push(struct_decl("Wrapped", vec![("index", 1, index)]));
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(
            state_in("Wrapped", &ctx),
            SizeState::Absent(AbsentCause::Overflow)
        );
    }

    #[test]
    fn the_largest_union_arm_need_not_be_first() {
        // Flipped = Coord @1 | Point @2: arm a 1 + 5 = 6, arm b 1 + 1 + 12 = 14.
        let mut package = fixture();
        package.decls.push(union_decl(
            "Flipped",
            &[("a", 1, "Coord"), ("b", 2, "Point")],
        ));
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(state_in("Flipped", &ctx), SizeState::Bounded(14));
    }

    #[test]
    fn a_non_negative_enum_is_sized_by_its_largest_value() {
        // Big = enum { A = 0, B = 200 }: 200 is a two-byte varint, so a
        // field is 1 + 2; sized by the smallest value, or by one byte, it
        // would be 2. An array of enums is packed: one tag, one length,
        // 3 * 2 values = 8; unpacked it would be 3 * 3 = 9.
        let mut package = fixture();
        package.decls.push(enum_decl("Big", &[0, 200]));
        package
            .decls
            .push(struct_decl("HoldsBig", vec![("e", 1, named("Big"))]));
        package.decls.push(struct_decl(
            "HoldsBigs",
            vec![("es", 1, array_of(named("Big"), 3))],
        ));
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(state_in("HoldsBig", &ctx), SizeState::Bounded(3));
        assert_eq!(state_in("HoldsBigs", &ctx), SizeState::Bounded(8));
        // The root box message `BigBox { Big value = 1; }` is one such field:
        // 1 + varint_len(200) = 1 + 2 = 3.
        assert_eq!(
            state_in("Big", &ctx),
            SizeState::Bounded(u32::try_from(1 + varint_len(200)).unwrap())
        );
    }

    #[test]
    fn a_map_key_is_resolved_through_every_key_kind() {
        // `map_key_scalar` admits a key by the scalar it projects to, through
        // a named scalar, a bare primitive and an enum set; a named struct,
        // enum or union is a message or enum name and is refused.
        let mut package = fixture();
        package.decls.push(Decl {
            name: "Flt".to_owned(),
            kind: Some(decl::Kind::TypeDef(TypeDef {
                backing: Some(Backing {
                    kind: Some(backing::Kind::Primitive(PrimitiveType::Float as i32)),
                }),
                width: Some(type_def::Width::FloatWidth(FloatWidth::F32 as i32)),
                ..Default::default()
            })),
            ..Default::default()
        });
        package.decls.push(Decl {
            name: "Flags".to_owned(),
            kind: Some(decl::Kind::EnumSetDef(EnumSetDef {
                width: IntWidth::U8 as i32,
                ..Default::default()
            })),
            ..Default::default()
        });
        package.decls.push(enum_decl("Gear", &[0, 1]));
        let keyed = |name: &str, key: FieldType| {
            struct_decl(name, vec![("m", 1, map_of(key, named("Coord"), 2))])
        };
        package.decls.push(keyed("NamedKeyed", named("Coord")));
        package.decls.push(keyed("NamedFloatKeyed", named("Flt")));
        package
            .decls
            .push(keyed("IntegerKeyed", primitive(PrimitiveType::Integer)));
        package
            .decls
            .push(keyed("FloatKeyed", primitive(PrimitiveType::Float)));
        package.decls.push(keyed("EnumSetKeyed", named("Flags")));
        package.decls.push(keyed("EnumKeyed", named("Gear")));
        package.decls.push(keyed("UnionKeyed", named("Shape")));
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        // A `sint32` key and value: entry (1 + 5) + (1 + 5) = 12; 2 * (1 + 1 + 12).
        assert_eq!(state_in("NamedKeyed", &ctx), SizeState::Bounded(28));
        assert_eq!(
            state_in("NamedFloatKeyed", &ctx),
            SizeState::Absent(AbsentCause::RefusedMember),
            "a named float key"
        );
        // A bare `integer` key is `sint64`: entry (1 + 10) + (1 + 5) = 17;
        // 2 * (1 + 1 + 17) = 38.
        assert_eq!(state_in("IntegerKeyed", &ctx), SizeState::Bounded(38));
        assert_eq!(
            state_in("FloatKeyed", &ctx),
            SizeState::Absent(AbsentCause::RefusedMember),
            "a bare float key is `double`"
        );
        // An enum set of width u8 is a `uint32` key: entry (1 + 5) + (1 + 5).
        assert_eq!(state_in("EnumSetKeyed", &ctx), SizeState::Bounded(28));
        assert_eq!(
            state_in("EnumKeyed", &ctx),
            SizeState::Absent(AbsentCause::RefusedMember),
            "an enum key is an enum name"
        );
        assert_eq!(
            state_in("UnionKeyed", &ctx),
            SizeState::Absent(AbsentCause::RefusedMember),
            "a union key is a message name"
        );
    }

    #[test]
    fn a_length_of_128_bytes_or_more_takes_a_two_byte_length_varint() {
        // Long { blob: bytes [0..200] }: 1 + 2 + 200; Outer { inner: Long }:
        // 1 + 2 + 203. A one-byte length would under-state each by one.
        let mut package = fixture();
        package
            .decls
            .push(struct_decl("Long", vec![("blob", 1, bytes_max(200))]));
        package
            .decls
            .push(struct_decl("Outer", vec![("inner", 1, named("Long"))]));
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(state_in("Long", &ctx), SizeState::Bounded(203));
        assert_eq!(state_in("Outer", &ctx), SizeState::Bounded(206));
    }

    #[test]
    fn size_state_routes_the_proto3_column() {
        let package = fixture();
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(
            size_state(
                &package,
                &PayloadShape::Named("Point"),
                &ctx,
                Encoding::Proto3
            ),
            SizeState::Bounded(12)
        );
    }

    #[test]
    fn an_imported_declaration_is_sized_in_its_own_package() {
        // `q.Thing { f: Inner }` names q's one-boolean `Inner` (1 + 1), so
        // `Thing` is 1 + 1 + 2; rooted at p, the bare `Inner` would be p's
        // `string [0..100]` struct (1 + 2 + 400) and `Thing` 406. `q.Loose`
        // holds q's `Hole`, a bare `string` with no leaf, so it is absent;
        // rooted at p, `Hole` would be a one-boolean struct and `Loose`
        // bounded.
        let (root, imported) = two_package_fixture();
        let others = [&imported];
        let ctx = Ctx::new(&root, &others);
        assert_eq!(state_in("q.Thing", &ctx), SizeState::Bounded(4));
        assert_eq!(
            state_in("q.Loose", &ctx),
            SizeState::Absent(AbsentCause::NoBound)
        );
        assert_eq!(
            state_in("Inner", &ctx),
            SizeState::Bounded(403),
            "the root's own `Inner`"
        );
        assert_eq!(state_in("Hole", &ctx), SizeState::Bounded(2));
    }

    #[test]
    fn a_reserved_member_costs_nothing_and_keeps_the_message() {
        // `WithRetired { x: Coord @1, reserved @2 }`: the retired slot is
        // never encoded, and the message is still sized.
        let mut package = fixture();
        let mut decl = struct_decl("WithRetired", vec![("x", 1, named("Coord"))]);
        if let Some(decl::Kind::StructDef(def)) = decl.kind.as_mut() {
            def.members.push(StructMember {
                member: Some(struct_member::Member::Reserved(Reserved {
                    ordinal: 2,
                    ..Default::default()
                })),
            });
        }
        package.decls.push(decl);
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(state_in("WithRetired", &ctx), SizeState::Bounded(6));
    }

    #[test]
    fn tuple_fields_past_the_fifteenth_take_a_two_byte_tag() {
        // A tuple of sixteen `Coord`: fields 1..=15 take a one-byte tag
        // (6 each), field 16 a two-byte tag (7): 97, delimited 1 + 1 + 97.
        // Numbered from 0 the sixteen tags would all be one byte: 96.
        let mut package = fixture();
        let wide = field(field_type::Kind::Tuple(TupleType {
            fields: (0..16)
                .map(|i| TupleField {
                    name: format!("f{i}"),
                    r#type: Some(named("Coord")),
                })
                .collect(),
        }));
        package
            .decls
            .push(struct_decl("Sixteen", vec![("t", 1, wide)]));
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(state_in("Sixteen", &ctx), SizeState::Bounded(99));
    }

    #[test]
    fn nesting_past_max_depth_is_absent() {
        // C0 { n: C1 }, ..., C_last { x: Coord }: C_k is sized at depth k,
        // and its field at the same depth, so a chain of MAX_DEPTH + 1
        // structs is sized and one more is not. typl rejects a cycle, so
        // the guard only answers for an IR the checker did not see.
        let chain = |last: u32| {
            let mut package = fixture();
            for k in 0..last {
                package.decls.push(struct_decl(
                    &format!("C{k}"),
                    vec![("n", 1, named(&format!("C{}", k + 1)))],
                ));
            }
            package.decls.push(struct_decl(
                &format!("C{last}"),
                vec![("x", 1, named("Coord"))],
            ));
            package
        };
        let within = chain(MAX_DEPTH);
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&within, &others);
        assert!(matches!(state_in("C0", &ctx), SizeState::Bounded(_)));
        let beyond = chain(MAX_DEPTH + 1);
        let ctx = Ctx::new(&beyond, &others);
        assert_eq!(
            state_in("C0", &ctx),
            SizeState::Absent(AbsentCause::Overflow)
        );
    }
}
