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
//! enum), a map as `map<K, V>`, a tuple as an induced message, a nested array
//! or map refused (`resolve_field_type`). proto3 has no unbounded state: typl
//! bounds every collection, so a message is bounded or, when the projection
//! refuses a member, absent. A bound above `u32::MAX` is absent, as in the
//! FlatBuffers projection's `MAX_ENCODABLE`.

use ridl_ir::v2::{
    ArrayType, FieldType, MapType, Package, StructDef, TupleType, UnionDef, struct_member,
};

use super::{Ctx, Leaf, SizeState, leaf_of_field_type, leaf_of_name};

/// Nesting deeper than this is treated as unsizable; typl rejects recursion
/// (§7.3), so this only guards against an IR the checker did not see.
const MAX_DEPTH: u32 = 64;

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

/// A length-delimited field: tag, length varint, payload.
fn delimited(number: u32, payload: u64) -> u64 {
    tag_len(number) + varint_len(payload) + payload
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
/// ten-byte varint, otherwise the largest magnitude decides.
fn enum_len(negative: bool, max_magnitude: u64) -> u64 {
    if negative {
        10
    } else {
        varint_len(max_magnitude)
    }
}

fn leaf_field(number: u32, leaf: Leaf<'_>, ctx: &Ctx<'_>, depth: u32) -> Option<u64> {
    if depth > MAX_DEPTH {
        return None;
    }
    Some(match leaf {
        Leaf::Scalar(s) => tag_len(number) + s.max_encoded_len()?,
        Leaf::Blob(bytes) => delimited(number, bytes),
        Leaf::Enum {
            negative,
            max_magnitude,
        } => tag_len(number) + enum_len(negative, max_magnitude),
        Leaf::Struct { def, home } => delimited(number, struct_size(def, home, ctx, depth + 1)?),
        Leaf::Union { def, home } => delimited(number, union_size(def, home, ctx, depth + 1)?),
        Leaf::Tuple { def, home } => delimited(number, tuple_size(def, home, ctx, depth + 1)?),
        Leaf::Array { def, home } => array_field(number, def, home, ctx, depth + 1)?,
        Leaf::Map { def, home } => map_field(number, def, home, ctx, depth + 1)?,
    })
}

fn struct_size<'a>(
    def: &'a StructDef,
    home: &'a Package,
    ctx: &Ctx<'a>,
    depth: u32,
) -> Option<u64> {
    def.members
        .iter()
        .map(|member| match &member.member {
            Some(struct_member::Member::Field(field)) => {
                field_size(field.ordinal, field.r#type.as_ref()?, home, ctx, depth)
            }
            Some(struct_member::Member::Reserved(_)) | None => Some(0),
        })
        .sum()
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
    tuple
        .fields
        .iter()
        .enumerate()
        .map(|(i, f)| {
            field_size(
                u32::try_from(i + 1).ok()?,
                f.r#type.as_ref()?,
                home,
                ctx,
                depth,
            )
        })
        .sum()
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
    Some(match element {
        Leaf::Scalar(s) => delimited(number, n.checked_mul(s.max_encoded_len()?)?),
        Leaf::Enum {
            negative,
            max_magnitude,
        } => delimited(number, n.checked_mul(enum_len(negative, max_magnitude))?),
        Leaf::Array { .. } | Leaf::Map { .. } => return None,
        other => n.checked_mul(leaf_field(number, other, ctx, depth)?)?,
    })
}

/// `map<K, V>`: `max` entries, each a message with the key at 1 and the
/// value at 2.
fn map_field<'a>(
    number: u32,
    map: &'a MapType,
    home: &'a Package,
    ctx: &Ctx<'a>,
    depth: u32,
) -> Option<u64> {
    let entry = field_size(1, map.key.as_ref()?, home, ctx, depth)?.checked_add(field_size(
        2,
        map.value.as_ref()?,
        home,
        ctx,
        depth,
    )?)?;
    map.max.checked_mul(delimited(number, entry))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::size::tests_support::*;
    use ridl_ir::v2::{
        Backing, Constraint, Decl, EnumDef, EnumValue, Field, Package, PrimitiveType, StructMember,
        TupleField, TypeDef, backing, decl, field_type,
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
}
