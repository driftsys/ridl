//! The proto3 state of a named-type payload under ADR-0017's projection:
//! bounded for a struct (a message) and a union (a message with a `oneof`);
//! absent for a named scalar and an enum set, which ADR-0017 decision 1
//! inlines into their field (decision 2 rejects a wrapper message), and for
//! an enum, a declared `enum` with no message of its own.
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
//! outside the integral and string scalars (`map_key_text`), an optional
//! array or map field (`emit_struct`, ADR-0013 decision 7), an enum live or
//! retired value outside int32 (`emit_enum`), and a field number protobuf
//! reserves or exceeds (`check_field_number`). The backend's name-collision
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
//! wire size does not depend on the names. Moving the refusal rules into
//! `ridl_ir::projection::proto3`, where the backend and this sizer share
//! them, is tracked by driftsys/ridl#690.
//!
//! proto3 has no unbounded state: typl bounds every collection, so a message
//! is bounded or, when the projection refuses a member, absent. Every
//! addition and multiplication is checked, so a bound the `u64` arithmetic
//! cannot hold is absent, and so is a bound above `u32::MAX`, as in the
//! FlatBuffers projection's `MAX_ENCODABLE`.

use std::collections::HashMap;

use ridl_ir::projection::proto3::{
    self as proto3_projection, PROTO_MAX_FIELD_NUMBER, PROTO_RESERVED, Scalar,
};
use ridl_ir::v2::{
    ArrayType, FieldType, MapType, Package, StructDef, TupleType, UnionDef, decl, field_type,
    struct_member,
};

use super::{Ctx, Leaf, SizeState, leaf_of_field_type, leaf_of_name};

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
}

/// The proto3 state of the named type `type_name`: its message's bound for
/// a struct or a union; absent for everything else (ADR-0017 decision 1
/// inlines a named scalar and an enum set, decision 2 rejects a wrapper
/// message, and an enum is a declared `enum`, not a message), for a name that
/// does not resolve, for a member the projection refuses, and for a bound
/// above `u32::MAX`.
pub(crate) fn state(type_name: &str, ctx: &Ctx<'_>) -> Option<SizeState> {
    let mut walk = Walk {
        ctx,
        memo: HashMap::new(),
        #[cfg(test)]
        bodies: 0,
    };
    let bytes = match leaf_of_name(type_name, ctx.packages().package, ctx)? {
        Leaf::Struct { name, def, home } => struct_size(name, def, home, &mut walk, 0)?,
        Leaf::Union { name, def, home } => union_size(name, def, home, &mut walk, 0)?,
        Leaf::Scalar(_)
        | Leaf::Blob(_)
        | Leaf::Enum { .. }
        | Leaf::Tuple { .. }
        | Leaf::Array { .. }
        | Leaf::Map { .. } => return None,
    };
    Some(SizeState::Bounded(u32::try_from(bytes).ok()?))
}

fn field_size<'a>(
    number: u32,
    ty: &'a FieldType,
    home: &'a Package,
    walk: &mut Walk<'a, '_>,
    depth: u32,
) -> Option<u64> {
    leaf_field(number, leaf_of_field_type(ty, home, walk.ctx)?, walk, depth)
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
    if depth > MAX_DEPTH || PROTO_RESERVED.contains(&number) || number > PROTO_MAX_FIELD_NUMBER {
        return None;
    }
    match leaf {
        Leaf::Scalar(s) => plain(number, s.max_encoded_len().expect(SCALAR_IS_BOUNDED)),
        Leaf::Blob(bytes) => delimited(number, bytes),
        Leaf::Enum {
            min,
            max,
            retired_in_int32,
        } => plain(number, enum_len(min, max, retired_in_int32)?),
        Leaf::Struct { name, def, home } => {
            delimited(number, struct_size(name, def, home, walk, depth + 1)?)
        }
        Leaf::Union { name, def, home } => {
            delimited(number, union_size(name, def, home, walk, depth + 1)?)
        }
        Leaf::Tuple { def, home } => delimited(number, tuple_size(def, home, walk, depth + 1)?),
        Leaf::Array { def, home } => array_field(number, def, home, walk, depth + 1),
        Leaf::Map { def, home } => map_field(number, def, home, walk, depth + 1),
    }
}

/// The sum of the fields of a message; `None` when a field is, or when the
/// sum does not fit.
fn message_size(mut fields: impl Iterator<Item = Option<u64>>) -> Option<u64> {
    fields.try_fold(0u64, |sum, field| sum.checked_add(field?))
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
        message_size(def.members.iter().map(|member| match &member.member {
            Some(struct_member::Member::Field(field)) => {
                let ty = field.r#type.as_ref()?;
                // proto3 cannot mark a repeated or map field absent, so the
                // proto backend refuses an optional array or map
                // (`emit_struct`, ADR-0013 decision 7).
                if ty.optional
                    && matches!(
                        ty.kind,
                        Some(field_type::Kind::Array(_) | field_type::Kind::Map(_))
                    )
                {
                    return None;
                }
                field_size(field.ordinal, ty, home, walk, depth)
            }
            Some(struct_member::Member::Reserved(_)) | None => Some(0),
        }))
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
            let leaf = leaf_of_name(&arm.type_ref, home, walk.ctx)?;
            largest = largest.max(leaf_field(arm.ordinal, leaf, walk, depth)?);
        }
        Some(largest)
    })
}

/// A tuple is an induced message with positional fields 1..n.
fn tuple_size<'a>(
    tuple: &'a TupleType,
    home: &'a Package,
    walk: &mut Walk<'a, '_>,
    depth: u32,
) -> Option<u64> {
    message_size(tuple.fields.iter().enumerate().map(|(i, f)| {
        field_size(
            u32::try_from(i + 1).ok()?,
            f.r#type.as_ref()?,
            home,
            walk,
            depth,
        )
    }))
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
    let element = leaf_of_field_type(array.element.as_ref()?, home, walk.ctx)?;
    let n = array.max;
    match element {
        Leaf::Scalar(s) => delimited(
            number,
            n.checked_mul(s.max_encoded_len().expect(SCALAR_IS_BOUNDED))?,
        ),
        Leaf::Enum {
            min,
            max,
            retired_in_int32,
        } => delimited(
            number,
            n.checked_mul(enum_len(min, max, retired_in_int32)?)?,
        ),
        Leaf::Array { .. } | Leaf::Map { .. } => None,
        other => n.checked_mul(leaf_field(number, other, walk, depth)?),
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
    let entry = field_size(1, key, home, walk, depth)?
        .checked_add(field_size(2, value, home, walk, depth)?)?;
    map.max.checked_mul(delimited(number, entry)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Encoding;
    use crate::size::size_state;
    use crate::size::tests_support::*;
    use ridl_ir::v2::{
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
        assert_eq!(state("Point", &ctx), Some(SizeState::Bounded(12)));
    }

    #[test]
    fn a_named_scalar_has_no_proto3_root_form() {
        // ADR-0017 decision 1 inlines a named scalar and decision 2 rejects a
        // wrapper message, so a payload of one is absent (driver §4 answer 6).
        let package = fixture();
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(state("Vin", &ctx), None);
        assert_eq!(state("Coord", &ctx), None);
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
        assert_eq!(state("Bag", &ctx), Some(SizeState::Bounded(176)));
    }

    #[test]
    fn a_union_payload_is_its_message_with_the_largest_arm() {
        // Shape = Point @1 | Coord @2: arm a 1 + 1 + 12 = 14, arm b 1 + 5 = 6.
        let package = fixture();
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(state("Shape", &ctx), Some(SizeState::Bounded(14)));
    }

    #[test]
    fn an_unresolved_name_is_absent() {
        let package = fixture();
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(state("Missing", &ctx), None);
    }

    fn struct_decl(name: &str, fields: Vec<(&str, u32, FieldType)>) -> Decl {
        Decl {
            name: name.to_owned(),
            kind: Some(decl::Kind::StructDef(StructDef {
                members: fields
                    .into_iter()
                    .map(|(name, ordinal, ty)| StructMember {
                        member: Some(struct_member::Member::Field(Field {
                            name: name.to_owned(),
                            ordinal,
                            r#type: Some(ty),
                            ..Default::default()
                        })),
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
        assert_eq!(state("Mix", &ctx), Some(SizeState::Bounded(63)));
        assert_eq!(state("Gear", &ctx), None, "an enum is not a message");
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
        assert_eq!(state("Grid", &ctx), None);
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
        assert_eq!(state("Wide", &ctx), None);
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
        assert_eq!(state("Huge", &ctx), None);
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
        assert_eq!(state("Vast", &ctx), None);
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
        assert_eq!(state("FloatKeyed", &ctx), None);
        assert_eq!(state("BytesKeyed", &ctx), None);
        assert_eq!(state("MessageKeyed", &ctx), None);
        // The control: `Bag` carries a string-keyed map and is bounded.
        assert_eq!(state("Bag", &ctx), Some(SizeState::Bounded(176)));
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
        assert_eq!(state("ArrayValued", &ctx), None);
        assert_eq!(state("MapValued", &ctx), None);
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
        assert_eq!(state("OptionalArray", &ctx), None);
        assert_eq!(state("OptionalMap", &ctx), None);
        assert_eq!(
            state("OptionalScalar", &ctx),
            Some(SizeState::Bounded(6)),
            "an optional scalar is admitted and costs no more than a required one"
        );
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
        assert_eq!(state("HoldsWide", &ctx), None);
        assert_eq!(state("HoldsDeep", &ctx), None);
        assert_eq!(
            state("HoldsEdge", &ctx),
            Some(SizeState::Bounded(11)),
            "the int32 bounds themselves are admitted; a negative member is a 10-byte varint"
        );
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
        assert_eq!(state("HoldsRetired", &ctx), None);
        assert_eq!(
            state("HoldsRetiredEdge", &ctx),
            Some(SizeState::Bounded(2)),
            "a retired value inside int32 is admitted and does not change the live values' size"
        );
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
        assert_eq!(state("Reserved", &ctx), None);
        assert_eq!(state("Reserved2", &ctx), None);
        assert_eq!(state("TooHigh", &ctx), None);
        assert_eq!(state("ArmReserved", &ctx), None);
        // The largest field number takes a 5-byte tag: 5 + 5.
        assert_eq!(state("Highest", &ctx), Some(SizeState::Bounded(10)));
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

    /// The bound of the struct or union `type_name`, and how many struct
    /// and union messages the walk derived rather than reused from its memo.
    fn walked(type_name: &str, ctx: &Ctx<'_>) -> (Option<u64>, u32) {
        let mut walk = Walk {
            ctx,
            memo: HashMap::new(),
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
        assert_eq!(state("Both", &ctx), Some(SizeState::Bounded(410)));
        assert_eq!(state("Swapped", &ctx), Some(SizeState::Bounded(410)));
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
        assert_eq!(state("Both", &ctx), None);
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
        assert_eq!(state("Wrapped", &ctx), None);
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
        assert_eq!(state("Wrapped", &ctx), None);
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
        assert_eq!(state("Wrapped", &ctx), None);
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
        assert_eq!(state("Wrapped", &ctx), None);
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
        assert_eq!(state("Wrapped", &ctx), None);
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
        assert_eq!(state("Flipped", &ctx), Some(SizeState::Bounded(14)));
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
        assert_eq!(state("HoldsBig", &ctx), Some(SizeState::Bounded(3)));
        assert_eq!(state("HoldsBigs", &ctx), Some(SizeState::Bounded(8)));
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
        assert_eq!(state("NamedKeyed", &ctx), Some(SizeState::Bounded(28)));
        assert_eq!(state("NamedFloatKeyed", &ctx), None, "a named float key");
        // A bare `integer` key is `int64`: entry (1 + 10) + (1 + 5) = 17; 2 * (1 + 1 + 17).
        assert_eq!(state("IntegerKeyed", &ctx), Some(SizeState::Bounded(38)));
        assert_eq!(
            state("FloatKeyed", &ctx),
            None,
            "a bare float key is `double`"
        );
        // An enum set of width u8 is a `uint32` key: entry (1 + 5) + (1 + 5).
        assert_eq!(state("EnumSetKeyed", &ctx), Some(SizeState::Bounded(28)));
        assert_eq!(
            state("EnumKeyed", &ctx),
            None,
            "an enum key is an enum name"
        );
        assert_eq!(
            state("UnionKeyed", &ctx),
            None,
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
        assert_eq!(state("Long", &ctx), Some(SizeState::Bounded(203)));
        assert_eq!(state("Outer", &ctx), Some(SizeState::Bounded(206)));
    }

    #[test]
    fn size_state_routes_the_proto3_column() {
        let package = fixture();
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(
            size_state("Point", &ctx, Encoding::Proto3),
            Some(SizeState::Bounded(12))
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
        assert_eq!(state("q.Thing", &ctx), Some(SizeState::Bounded(4)));
        assert_eq!(state("q.Loose", &ctx), None);
        assert_eq!(
            state("Inner", &ctx),
            Some(SizeState::Bounded(403)),
            "the root's own `Inner`"
        );
        assert_eq!(state("Hole", &ctx), Some(SizeState::Bounded(2)));
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
        assert_eq!(state("WithRetired", &ctx), Some(SizeState::Bounded(6)));
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
        assert_eq!(state("Sixteen", &ctx), Some(SizeState::Bounded(99)));
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
        assert!(matches!(state("C0", &ctx), Some(SizeState::Bounded(_))));
        let beyond = chain(MAX_DEPTH + 1);
        let ctx = Ctx::new(&beyond, &others);
        assert_eq!(state("C0", &ctx), None);
    }
}
