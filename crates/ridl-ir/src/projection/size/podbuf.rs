//! The podbuf state of a named-type payload: the projection's own bound
//! ([`crate::projection::podbuf::max_size`], the one implementation of the
//! bound). Nothing is derived here: a second derivation would be two
//! implementations of one rule.
//!
//! podbuf has no unbounded case, because every typl string, bytes and
//! collection carries a bound. A name that does not resolve, or whose
//! declaration is not a type, is absent with [`AbsentCause::Unresolved`], as
//! under FlatBuffers. A projection that answers `None` is absent with
//! [`AbsentCause::Overflow`]: the bound is above `u32::MAX`, an array or map
//! is past a descriptor limit, or the layout is an error.

use crate::projection::podbuf::{max_size, tuple_max_size};
use crate::v2::{Decl, Package, TupleType, decl};

use super::{AbsentCause, Ctx, SizeState};

/// Whether `decl` has a podbuf root: a struct, a union, a named scalar, an
/// enum or an enum set.
fn has_root(decl: &Decl) -> bool {
    matches!(
        decl.kind,
        Some(
            decl::Kind::StructDef(_)
                | decl::Kind::UnionDef(_)
                | decl::Kind::TypeDef(_)
                | decl::Kind::EnumDef(_)
                | decl::Kind::EnumSetDef(_)
        )
    )
}

/// The state of the named type `type_name`, read from `home`: absent with
/// [`AbsentCause::Unresolved`] when the name does not resolve or the
/// declaration has no podbuf root; bounded with the projection's bound;
/// absent with [`AbsentCause::Overflow`] when the projection answers `None`.
pub(crate) fn state<'a>(home: &'a Package, type_name: &str, ctx: &Ctx<'a>) -> SizeState {
    let Some((decl, declaring)) = ctx.resolve(home, type_name) else {
        return SizeState::Absent(AbsentCause::Unresolved);
    };
    if !has_root(decl) {
        return SizeState::Absent(AbsentCause::Unresolved);
    }
    // Rooted at the declaring package: a bare name inside an imported
    // declaration resolves in that package.
    let Some(packages) = ctx.packages_for(declaring) else {
        return SizeState::Absent(AbsentCause::Unresolved);
    };
    bounded(max_size(packages, decl))
}

/// The state of an induced argument or reply struct, read from `home`.
pub(crate) fn tuple_state<'a>(home: &'a Package, tuple: &TupleType, ctx: &Ctx<'a>) -> SizeState {
    let Some(packages) = ctx.packages_for(home) else {
        return SizeState::Absent(AbsentCause::Unresolved);
    };
    bounded(tuple_max_size(packages, tuple))
}

/// The projection's bound as a state. The projection answers `None` above
/// `u32::MAX`, so a bound it gives always fits a `u32`.
fn bounded(bound: Option<u64>) -> SizeState {
    match bound.map(u32::try_from) {
        Some(Ok(bytes)) => SizeState::Bounded(bytes),
        Some(Err(_)) | None => SizeState::Absent(AbsentCause::Overflow),
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Encoding, PayloadShape, size_state};
    use super::*;
    use crate::v2::{
        Backing, ConstDef, Constraint, Field, FieldType, IntWidth, PrimitiveType, StructDef,
        StructMember, TypeDef, backing, field_type, struct_member, type_def,
    };

    fn field(name: &str, ordinal: u32, ty: FieldType) -> StructMember {
        StructMember {
            member: Some(struct_member::Member::Field(Box::new(Field {
                name: name.to_owned(),
                ordinal,
                r#type: Some(ty),
                ..Default::default()
            }))),
        }
    }

    fn scalar(primitive: PrimitiveType, constraint: Constraint, width: IntWidth) -> FieldType {
        FieldType {
            optional: false,
            kind: Some(field_type::Kind::InlineScalar(Box::new(TypeDef {
                backing: Some(Backing {
                    kind: Some(backing::Kind::Primitive(primitive as i32)),
                }),
                constraint: Some(constraint),
                width: Some(type_def::Width::IntWidth(width as i32)),
                ..Default::default()
            }))),
        }
    }

    fn string(max: u64) -> FieldType {
        FieldType {
            optional: false,
            kind: Some(field_type::Kind::InlineScalar(Box::new(TypeDef {
                backing: Some(Backing {
                    kind: Some(backing::Kind::Primitive(PrimitiveType::String as i32)),
                }),
                constraint: Some(Constraint {
                    len_min: Some(0),
                    len_max: Some(max),
                    ..Default::default()
                }),
                ..Default::default()
            }))),
        }
    }

    fn named(name: &str) -> FieldType {
        FieldType {
            optional: false,
            kind: Some(field_type::Kind::Named(name.to_owned())),
        }
    }

    fn array(element: FieldType, min: u64, max: u64) -> FieldType {
        FieldType {
            optional: false,
            kind: Some(field_type::Kind::Array(Box::new(crate::v2::ArrayType {
                element: Some(Box::new(element)),
                min,
                max,
            }))),
        }
    }

    fn struct_decl(name: &str, members: Vec<StructMember>) -> Decl {
        Decl {
            name: name.to_owned(),
            kind: Some(decl::Kind::StructDef(StructDef {
                members,
                fixed_layout: false,
            })),
            ..Default::default()
        }
    }

    fn package() -> Package {
        let u16_field = scalar(PrimitiveType::Integer, Constraint::default(), IntWidth::U16);
        let i64_field = scalar(PrimitiveType::Integer, Constraint::default(), IntWidth::I64);
        Package {
            name: "veh".to_owned(),
            decls: vec![
                struct_decl(
                    "FaultEvent",
                    vec![
                        field("code", 1, u16_field),
                        field("message", 2, string(256)),
                        field("timestamp", 3, i64_field),
                    ],
                ),
                struct_decl(
                    "FaultPage",
                    vec![field("faults", 1, array(named("FaultEvent"), 0, 64))],
                ),
                struct_decl(
                    "TooMany",
                    vec![field("items", 1, array(named("FaultEvent"), 0, 65_536))],
                ),
                Decl {
                    name: "LIMIT".to_owned(),
                    kind: Some(decl::Kind::ConstDef(ConstDef::default())),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn a_struct_is_bounded_at_its_podbuf_max_size() {
        let package = package();
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(
            state(&package, "FaultPage", &ctx),
            SizeState::Bounded(8 + 8 + 64 * 24 + 64 * 1025)
        );
    }

    #[test]
    fn size_state_routes_the_podbuf_column() {
        let package = package();
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(
            size_state(
                &package,
                &PayloadShape::Named("FaultPage"),
                &ctx,
                Encoding::Podbuf
            ),
            SizeState::Bounded(8 + 8 + 64 * 24 + 64 * 1025)
        );
    }

    #[test]
    fn a_tuple_is_bounded_as_a_struct_of_its_fields() {
        let package = package();
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        let tuple = TupleType {
            fields: vec![crate::v2::TupleField {
                name: "faults".to_owned(),
                r#type: Some(array(named("FaultEvent"), 0, 64)),
            }],
        };
        assert_eq!(
            size_state(
                &package,
                &PayloadShape::Tuple(&tuple),
                &ctx,
                Encoding::Podbuf
            ),
            SizeState::Bounded(8 + 8 + 64 * 24 + 64 * 1025)
        );
    }

    #[test]
    fn a_projection_with_no_bound_is_overflow() {
        let package = package();
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(
            state(&package, "TooMany", &ctx),
            SizeState::Absent(AbsentCause::Overflow)
        );
    }

    #[test]
    fn a_name_with_no_root_or_no_declaration_is_unresolved() {
        let package = package();
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        for name in ["Missing", "LIMIT"] {
            assert_eq!(
                state(&package, name, &ctx),
                SizeState::Absent(AbsentCause::Unresolved),
                "{name}"
            );
        }
    }
}
