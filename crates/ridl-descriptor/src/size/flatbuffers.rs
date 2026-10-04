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
/// all reach here as unbounded rows, so `ridl describe` and
/// `--emit codegen-model` agree.
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
    use ridl_ir::v2::Package;

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
        let package = fixture();
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert!(matches!(
            size_state("Point", &ctx, Encoding::FlatBuffers),
            Some(SizeState::Bounded(_))
        ));
        assert_eq!(size_state("Point", &ctx, Encoding::ReprC), None);
    }
}
