//! The FlatBuffers state of a named-type payload: the projection's own bound
//! (`ridl_ir::projection::flatbuffers::max_size`, the one implementation of
//! the bound — `docs/design/flatbuffers-codec.md`, from E11.7's design D-6),
//! and the codegen model's cause when there is none. Nothing is derived here:
//! a second derivation would be two implementations of one rule, and a silent
//! disagreement the moment either changed.

use ridl_ir::codegen::fb_unbounded;
use ridl_ir::codegen::v1::FbUnboundedCause;
use ridl_ir::projection::flatbuffers::{MAX_ENCODABLE, max_size, root_table};

use super::{Ctx, SizeState};
use crate::UnboundedCause;

/// The state of the named type `type_name`: absent when the name does not
/// resolve or the declaration has no FlatBuffers root (a constant, ADR-0013
/// decision 5, or an interaction); bounded with the projection's bound;
/// unbounded, with the cause the codegen model reports for the same
/// declaration, when the projection answers `None` — an unresolved
/// reference, a cycle, a `u64` overflow or a bound above `MAX_ENCODABLE`
/// all reach here as unbounded rows, so the descriptor's column and the
/// model `--emit codegen-model` writes agree on which payloads have a bound.
pub(crate) fn state(type_name: &str, ctx: &Ctx<'_>) -> Option<SizeState> {
    let (decl, declaring) = ctx.resolve(ctx.packages().package, type_name)?;
    root_table(decl)?;
    // Rooted at the declaring package: a bare name inside an imported
    // declaration resolves in that package, as the codegen lowering does.
    Some(match max_size(ctx.packages_for(declaring)?, decl) {
        Some(bytes) => {
            debug_assert!(bytes <= MAX_ENCODABLE);
            SizeState::Bounded(
                u32::try_from(bytes).expect("max_size refuses a bound above MAX_ENCODABLE"),
            )
        }
        None => SizeState::Unbounded(cause_of(fb_unbounded(declaring, decl).cause)),
    })
}

/// The schema's enum mirrors the model's, member for member.
fn cause_of(cause: i32) -> UnboundedCause {
    match FbUnboundedCause::try_from(cause) {
        Ok(FbUnboundedCause::Member) => UnboundedCause::Member,
        Ok(FbUnboundedCause::Untyped) => UnboundedCause::Untyped,
        Ok(FbUnboundedCause::Layout) => UnboundedCause::Layout,
        Ok(FbUnboundedCause::Aggregate) => UnboundedCause::Aggregate,
        Ok(FbUnboundedCause::Exempt) => UnboundedCause::Exempt,
        Ok(FbUnboundedCause::Unspecified) | Err(_) => UnboundedCause::Unspecified,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::size::tests_support::*;
    use crate::size::{Ctx, SizeState, size_state};
    use crate::{Encoding, UnboundedCause};
    use ridl_ir::projection::flatbuffers::{Packages, max_size};
    use ridl_ir::v2::{
        Backing, Constraint, Decl, Field, FieldType, Package, PrimitiveType, StructDef,
        StructMember, TypeDef, backing, decl, struct_member,
    };

    #[test]
    fn a_bounded_type_advertises_the_projections_number() {
        let package = fixture();
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        for name in ["Point", "Vin", "Coord", "Bag", "Shape"] {
            let (decl, _) = ctx.resolve(&package, name).unwrap();
            let expected = max_size(
                Packages {
                    package: &package,
                    others: &[],
                },
                decl,
            )
            .unwrap_or_else(|| panic!("{name} is bounded in the fixture"));
            assert_eq!(
                state(name, &ctx),
                Some(SizeState::Bounded(u32::try_from(expected).unwrap())),
                "{name}"
            );
        }
    }

    #[test]
    fn an_unbounded_type_carries_the_models_cause() {
        // The fixture's `Open` holds a bare `string` with no bound, the shape
        // the codegen model's own tests refuse with the `Member` cause
        // (`flatbuffers_bound_names_the_unbounded_member_beside_an_exempt_one`
        // in `ridl-backend-rust`).
        let package = unbounded_fixture();
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(
            state("Open", &ctx),
            Some(SizeState::Unbounded(UnboundedCause::Member))
        );
    }

    #[test]
    fn a_name_with_no_root_table_or_no_declaration_is_absent() {
        let package = fixture();
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(state("Missing", &ctx), None);
        // A constant projects no FlatBuffers declaration (ADR-0013 decision 5).
        assert_eq!(state("LIMIT", &ctx), None);
    }

    #[test]
    fn size_state_routes_the_flatbuffers_column() {
        // `Point`'s FlatBuffers bound is 55 bytes; its proto3 bound is 12,
        // so a column routed to the other sizer is caught here.
        let package = fixture();
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(
            size_state("Point", &ctx, Encoding::FlatBuffers),
            Some(SizeState::Bounded(55))
        );
        assert_eq!(size_state("Point", &ctx, Encoding::ReprC), None);
    }

    #[test]
    fn an_imported_declaration_is_sized_in_its_own_package() {
        // `q.Thing { f: Inner }` names q's one-boolean `Inner`; rooted at
        // the root p, the bare `Inner` would be p's wider struct. `q.Loose`
        // names q's `Hole`, a bare `string`, so it is unbounded by that
        // member; attributed against p, whose `Hole` is bounded, every
        // member would judge as bounded and the cause would be `Aggregate`.
        let (root, imported) = two_package_fixture();
        let others = [&imported];
        let ctx = Ctx::new(&root, &others);
        let (thing, _) = ctx.resolve(&root, "q.Thing").expect("q.Thing resolves");
        let in_q = max_size(
            Packages {
                package: &imported,
                others: &[&root],
            },
            thing,
        )
        .expect("bounded in q");
        let in_p = max_size(
            Packages {
                package: &root,
                others: &[&imported],
            },
            thing,
        )
        .expect("bounded in p too, with the wider `Inner`");
        assert_ne!(in_q, in_p, "the two `Inner`s differ in size");
        assert_eq!(
            state("q.Thing", &ctx),
            Some(SizeState::Bounded(u32::try_from(in_q).unwrap()))
        );
        assert_eq!(
            state("q.Loose", &ctx),
            Some(SizeState::Unbounded(UnboundedCause::Member))
        );
    }

    #[test]
    fn every_model_cause_maps_to_its_schema_member() {
        use ridl_ir::codegen::v1::FbUnboundedCause as Model;
        for (model, schema) in [
            (Model::Member, UnboundedCause::Member),
            (Model::Untyped, UnboundedCause::Untyped),
            (Model::Layout, UnboundedCause::Layout),
            (Model::Aggregate, UnboundedCause::Aggregate),
            (Model::Exempt, UnboundedCause::Exempt),
            (Model::Unspecified, UnboundedCause::Unspecified),
        ] {
            assert_eq!(cause_of(model as i32), schema, "{model:?}");
        }
        assert_eq!(
            cause_of(99),
            UnboundedCause::Unspecified,
            "a cause tag the model does not define"
        );
    }

    fn one_member_struct(name: &str, members: Vec<StructMember>) -> Decl {
        Decl {
            name: name.to_owned(),
            kind: Some(decl::Kind::StructDef(StructDef {
                members,
                fixed_layout: false,
            })),
            ..Default::default()
        }
    }

    fn member(name: &str, ordinal: u32, ty: Option<FieldType>) -> StructMember {
        StructMember {
            member: Some(struct_member::Member::Field(Field {
                name: name.to_owned(),
                ordinal,
                r#type: ty,
                ..Default::default()
            })),
        }
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

    #[test]
    fn each_unbounded_path_carries_its_own_cause() {
        // The shapes the codegen model attributes to each cause other than
        // `Member` (`an_unbounded_type_carries_the_models_cause`):
        // `Untyped`, a field with no type in the IR; `Layout`, two fields on
        // one ordinal, which `struct_table` refuses; `Aggregate`, two
        // `bytes [0..2^31]` fields each under `MAX_ENCODABLE` alone and over
        // it together; `Exempt`, a cross-package reference the attribution
        // (over the package alone) cannot judge, to a type the projection
        // (over the scope) finds unbounded.
        let (root, imported) = two_package_fixture();
        let mut root = root;
        root.decls
            .push(one_member_struct("NoType", vec![member("f", 1, None)]));
        root.decls.push(one_member_struct(
            "Clash",
            vec![
                member("a", 1, Some(named("Hole"))),
                member("b", 1, Some(named("Hole"))),
            ],
        ));
        root.decls.push(one_member_struct(
            "TooBig",
            vec![
                member("a", 1, Some(bytes_max(1 << 31))),
                member("b", 2, Some(bytes_max(1 << 31))),
            ],
        ));
        root.decls.push(one_member_struct(
            "Far",
            vec![member("hole", 1, Some(named("q.Hole")))],
        ));
        let others = [&imported];
        let ctx = Ctx::new(&root, &others);
        assert_eq!(
            state("NoType", &ctx),
            Some(SizeState::Unbounded(UnboundedCause::Untyped))
        );
        assert_eq!(
            state("Clash", &ctx),
            Some(SizeState::Unbounded(UnboundedCause::Layout))
        );
        assert_eq!(
            state("TooBig", &ctx),
            Some(SizeState::Unbounded(UnboundedCause::Aggregate))
        );
        assert_eq!(
            state("Far", &ctx),
            Some(SizeState::Unbounded(UnboundedCause::Exempt))
        );
    }
}
