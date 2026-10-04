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
//! enum), a map as `map<K, V>`, a tuple as an induced message. A member the
//! backend refuses makes the payload absent, and every refusal is reproduced
//! here, citing the backend function that holds it: a nested array or map and
//! a map value that is an array or a map (`resolve_field_type`), a map key
//! outside the integral and string scalars (`map_key_text`), an optional array
//! or map field (`emit_struct`, ADR-0013 decision 7), an enum live or
//! retired value outside int32 (`emit_enum`), and a field number protobuf
//! reserves or exceeds (`check_field_number`). proto3 has no unbounded state:
//! typl bounds every collection, so a message is bounded or, when the
//! projection refuses a member, absent. Every addition and multiplication is
//! checked, so a bound the `u64` arithmetic cannot hold is absent, and so is a
//! bound above `u32::MAX`, as in the FlatBuffers projection's `MAX_ENCODABLE`.

use ridl_ir::projection::proto3::{self as proto3_projection, Scalar};
use ridl_ir::v2::{
    ArrayType, FieldType, MapType, Package, StructDef, TupleType, UnionDef, decl, field_type,
    struct_member,
};

use super::{Ctx, Leaf, SizeState, leaf_of_field_type, leaf_of_name};

/// Nesting deeper than this is treated as unsizable; typl rejects recursion
/// (§7.3), so this only guards against an IR the checker did not see.
const MAX_DEPTH: u32 = 64;

/// The field numbers protobuf reserves for itself, and its largest field
/// number: the two bounds `check_field_number` in the proto backend refuses.
const PROTO_RESERVED: std::ops::RangeInclusive<u32> = 19_000..=19_999;
const PROTO_MAX_FIELD_NUMBER: u32 = 536_870_911;

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

/// The proto3 state of the named type `type_name`: its message's bound for
/// a struct or a union; absent for everything else (ADR-0017 decision 1
/// inlines a named scalar and an enum set, decision 2 rejects a wrapper
/// message, and an enum is a declared `enum`, not a message), for a name that
/// does not resolve, for a member the projection refuses, and for a bound
/// above `u32::MAX`.
pub(crate) fn state(type_name: &str, ctx: &Ctx<'_>) -> Option<SizeState> {
    let bytes = match leaf_of_name(type_name, ctx.packages().package, ctx)? {
        Leaf::Struct { def, home } => struct_size(def, home, ctx, 0)?,
        Leaf::Union { def, home } => union_size(def, home, ctx, 0)?,
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
    ctx: &Ctx<'a>,
    depth: u32,
) -> Option<u64> {
    leaf_field(number, leaf_of_field_type(ty, home, ctx)?, ctx, depth)
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

fn leaf_field(number: u32, leaf: Leaf<'_>, ctx: &Ctx<'_>, depth: u32) -> Option<u64> {
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
        Leaf::Struct { def, home } => delimited(number, struct_size(def, home, ctx, depth + 1)?),
        Leaf::Union { def, home } => delimited(number, union_size(def, home, ctx, depth + 1)?),
        Leaf::Tuple { def, home } => delimited(number, tuple_size(def, home, ctx, depth + 1)?),
        Leaf::Array { def, home } => array_field(number, def, home, ctx, depth + 1),
        Leaf::Map { def, home } => map_field(number, def, home, ctx, depth + 1),
    }
}

/// The sum of the fields of a message; `None` when a field is, or when the
/// sum does not fit.
fn message_size(mut fields: impl Iterator<Item = Option<u64>>) -> Option<u64> {
    fields.try_fold(0u64, |sum, field| sum.checked_add(field?))
}

fn struct_size<'a>(
    def: &'a StructDef,
    home: &'a Package,
    ctx: &Ctx<'a>,
    depth: u32,
) -> Option<u64> {
    message_size(def.members.iter().map(|member| match &member.member {
        Some(struct_member::Member::Field(field)) => {
            let ty = field.r#type.as_ref()?;
            // proto3 cannot mark a repeated or map field absent, so the proto
            // backend refuses an optional array or map (`emit_struct`,
            // ADR-0013 decision 7).
            if ty.optional
                && matches!(
                    ty.kind,
                    Some(field_type::Kind::Array(_) | field_type::Kind::Map(_))
                )
            {
                return None;
            }
            field_size(field.ordinal, ty, home, ctx, depth)
        }
        Some(struct_member::Member::Reserved(_)) | None => Some(0),
    }))
}

/// The largest arm, as a field of the `oneof` at the arm's ordinal.
fn union_size<'a>(def: &'a UnionDef, home: &'a Package, ctx: &Ctx<'a>, depth: u32) -> Option<u64> {
    let mut largest = 0;
    for arm in &def.arms {
        let leaf = leaf_of_name(&arm.type_ref, home, ctx)?;
        largest = largest.max(leaf_field(arm.ordinal, leaf, ctx, depth)?);
    }
    Some(largest)
}

/// A tuple is an induced message with positional fields 1..n.
fn tuple_size<'a>(
    tuple: &'a TupleType,
    home: &'a Package,
    ctx: &Ctx<'a>,
    depth: u32,
) -> Option<u64> {
    message_size(tuple.fields.iter().enumerate().map(|(i, f)| {
        field_size(
            u32::try_from(i + 1).ok()?,
            f.r#type.as_ref()?,
            home,
            ctx,
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
    ctx: &Ctx<'a>,
    depth: u32,
) -> Option<u64> {
    let element = leaf_of_field_type(array.element.as_ref()?, home, ctx)?;
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
        other => n.checked_mul(leaf_field(number, other, ctx, depth)?),
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
    ctx: &Ctx<'a>,
    depth: u32,
) -> Option<u64> {
    let key = map.key.as_ref()?;
    let value = map.value.as_ref()?;
    if !map_key_scalar(key, home, ctx)?.admitted_as_map_key()
        || matches!(
            value.kind,
            Some(field_type::Kind::Array(_) | field_type::Kind::Map(_))
        )
    {
        return None;
    }
    let entry = field_size(1, key, home, ctx, depth)?
        .checked_add(field_size(2, value, home, ctx, depth)?)?;
    map.max.checked_mul(delimited(number, entry)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::size::tests_support::*;
    use ridl_ir::v2::{
        Backing, Constraint, Decl, EnumDef, EnumValue, Field, FloatWidth, IntWidth, Package,
        PrimitiveType, Reserved, StructMember, TupleField, TypeDef, UnionArm, UnionDef, backing,
        decl, field_type, type_def,
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
}
