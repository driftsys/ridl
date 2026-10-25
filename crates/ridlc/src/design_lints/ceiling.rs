//! RIDL-150 `payload-above-ceiling`: a payload carried by an interaction whose
//! largest encoded size, in an encoding that the workspace manifest enables, is above the
//! payload ceiling, or has no finite bound (ADR-0030 decision 7).
//!
//! The sizes are the ones the codegen model carries for each payload, which
//! are the sizes the backends write as `MAX_SIZE`, so the lint and the
//! generated code never disagree about a bound. A fallible `T | E` reply has
//! no size of its own in the model, so its success and error types are sized
//! separately with the same sizer, and the larger problem is reported. The
//! diagnostic is reported on the interaction that carries the payload; a type
//! that no interaction carries is not reported.

use ridl_core::diag::{DiagCode, Diagnostic};
use ridl_core::limits::PAYLOAD_CEILING;
use ridl_core::lint::lint_by_name;
use ridl_ir::codegen::{self, v1};
use ridl_ir::projection::size::{self, Ctx as SizeCtx, PayloadShape};
use ridl_ir::v2;

use super::Ctx;

/// What one payload's size state says, ordered from the least to the most
/// serious so that the worst of several is their maximum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Size {
    /// No size is stated: the payload has no encoding in this codec, which is
    /// not a size problem.
    Other,
    Bytes(u64),
    /// The bound does not fit a `u32`, or the nesting is past the sizer's
    /// depth limit of 64 levels.
    Overflow,
    NoBound,
}

fn from_model(state: &v1::SizeState) -> Size {
    match state.state.as_ref() {
        Some(v1::size_state::State::Bounded(bytes)) => Size::Bytes(u64::from(*bytes)),
        Some(v1::size_state::State::Unbounded(_)) => Size::NoBound,
        Some(v1::size_state::State::Absent(absent)) => {
            match v1::AbsentCause::try_from(absent.cause) {
                Ok(v1::AbsentCause::NoBound) => Size::NoBound,
                Ok(v1::AbsentCause::Overflow) => Size::Overflow,
                _ => Size::Other,
            }
        }
        None => Size::Other,
    }
}

fn from_sizer(state: size::SizeState) -> Size {
    match state {
        size::SizeState::Bounded(bytes) => Size::Bytes(u64::from(bytes)),
        size::SizeState::Unbounded(_) => Size::NoBound,
        size::SizeState::Absent(size::AbsentCause::NoBound) => Size::NoBound,
        size::SizeState::Absent(size::AbsentCause::Overflow) => Size::Overflow,
        size::SizeState::Absent(_) => Size::Other,
    }
}

/// Why one payload breaks `limit` bytes under one encoding, or `None` when it
/// does not: its bound is above `limit`, or it has no finite bound.
fn problem_above(size: Size, limit: u64) -> Option<String> {
    match size {
        Size::Bytes(bytes) if bytes > limit => Some(format!("up to {bytes} bytes")),
        Size::Bytes(_) | Size::Other => None,
        Size::NoBound => Some("no finite bound".to_string()),
        Size::Overflow => Some(
            "a bound too large to represent, or, in proto3, nesting deeper than 64 levels"
                .to_string(),
        ),
    }
}

/// The size of one payload under each checked encoding, with the type that
/// has it when the payload is a fallible reply.
#[derive(Clone)]
pub(super) struct Side {
    pub(super) size: Size,
    pub(super) ty: Option<String>,
}

pub(super) struct Entry {
    pub(super) role: &'static str,
    pub(super) flatbuffers: Side,
    pub(super) proto3: Side,
}

fn entry(role: &'static str, sizes: &v1::PayloadSizes) -> Entry {
    let side = |state: Option<&v1::SizeState>| Side {
        size: state.map_or(Size::Other, from_model),
        ty: None,
    };
    Entry {
        role,
        flatbuffers: side(sizes.flatbuffers.as_ref()),
        proto3: side(sizes.proto3.as_ref()),
    }
}

/// The reply of a fallible `T | E` query: each of the two types sized on its
/// own, and the worse of them kept for each encoding.
fn fallible_entry(
    package: &v2::Package,
    others: &[&v2::Package],
    fallible: &v2::FallibleType,
) -> Entry {
    let sizes = SizeCtx::new(package, others);
    let worst = |encoding: size::Encoding| {
        [&fallible.ok, &fallible.err]
            .into_iter()
            .map(|name| {
                let state = size::size_state(package, &PayloadShape::Named(name), &sizes, encoding);
                Side {
                    size: from_sizer(state),
                    ty: Some(name.clone()),
                }
            })
            .max_by_key(|side| side.size)
            .expect("two types")
    };
    Entry {
        role: "reply",
        flatbuffers: worst(size::Encoding::FlatBuffers),
        proto3: worst(size::Encoding::Proto3),
    }
}

/// The value of a fixed, sized under each encoding. RIDL-150 never sizes a
/// fixed, because it is provisioned and never in flight; the payload limits
/// of a unit apply to it all the same (ADR-0030 decision 14).
pub(super) fn fixed_entry(
    package: &v2::Package,
    others: &[&v2::Package],
    fixed: &v2::FixedDef,
) -> Option<Entry> {
    let ty = fixed.payload.as_ref()?;
    let sizes = SizeCtx::new(package, others);
    let side = |encoding: size::Encoding| Side {
        size: from_sizer(size::size_state(
            package,
            &PayloadShape::Field(ty),
            &sizes,
            encoding,
        )),
        ty: None,
    };
    Some(Entry {
        role: "value",
        flatbuffers: side(size::Encoding::FlatBuffers),
        proto3: side(size::Encoding::Proto3),
    })
}

/// The payloads one interaction carries on the wire, with the role of each.
/// A fixed is provisioned and never in flight, so it carries none.
pub(super) fn payloads(
    interaction: &v1::Interaction,
    decl: &v2::Decl,
    package: &v2::Package,
    others: &[&v2::Package],
) -> Vec<Entry> {
    match interaction.shape.as_ref() {
        Some(v1::interaction::Shape::Signal(signal)) => signal
            .payload
            .as_ref()
            .and_then(|payload| payload.sizes.as_ref())
            .map(|sizes| entry("payload", sizes))
            .into_iter()
            .collect(),
        Some(v1::interaction::Shape::Event(event)) => event
            .payload
            .as_ref()
            .and_then(|payload| payload.sizes.as_ref())
            .map(|sizes| entry("payload", sizes))
            .into_iter()
            .collect(),
        Some(v1::interaction::Shape::Command(command)) => command
            .request_sizes
            .as_ref()
            .map(|sizes| entry("request", sizes))
            .into_iter()
            .collect(),
        Some(v1::interaction::Shape::Query(query)) => {
            let fallible = match decl.kind.as_ref() {
                Some(v2::decl::Kind::QueryDef(def)) => {
                    def.return_type
                        .as_ref()
                        .and_then(|ret| match ret.kind.as_ref() {
                            Some(v2::return_type::Kind::Fallible(fallible)) => Some(fallible),
                            _ => None,
                        })
                }
                _ => None,
            };
            let reply = match fallible {
                Some(fallible) => Some(fallible_entry(package, others, fallible)),
                None => query
                    .reply_sizes
                    .as_ref()
                    .map(|sizes| entry("reply", sizes)),
            };
            query
                .request_sizes
                .as_ref()
                .map(|sizes| entry("request", sizes))
                .into_iter()
                .chain(reply)
                .collect()
        }
        _ => Vec::new(),
    }
}

/// The encodings to check: the names in `encodings` that have a size state,
/// or FlatBuffers when `encodings` names none.
pub(super) fn checked_encodings(encodings: &[String]) -> (bool, bool) {
    if encodings.is_empty() {
        return (true, false);
    }
    (
        encodings.iter().any(|name| name == "flatbuffers"),
        encodings.iter().any(|name| name == "proto3"),
    )
}

fn describe(side: &Side, encoding: &str) -> Option<String> {
    describe_above(side, encoding, u64::from(PAYLOAD_CEILING))
}

/// [`describe`] for a payload limit of `limit` bytes.
pub(super) fn describe_above(side: &Side, encoding: &str, limit: u64) -> Option<String> {
    let why = problem_above(side.size, limit)?;
    Some(match &side.ty {
        Some(ty) => format!("{why} in {encoding} for `{ty}`"),
        None => format!("{why} in {encoding}"),
    })
}

pub(crate) fn check(ctx: &Ctx<'_>, std_ir: &v2::Package, encodings: &[String]) -> Vec<Diagnostic> {
    let (flatbuffers, proto3) = checked_encodings(encodings);
    let severity = lint_by_name("payload-above-ceiling")
        .expect("registered lint")
        .severity;
    let mut diagnostics = Vec::new();
    for checked in ctx.subjects() {
        let package = &checked.ir;
        if package.shapes().next().is_none() {
            continue;
        }
        let others: Vec<&v2::Package> = ctx
            .checked
            .iter()
            .filter(|other| other.ir.name != package.name)
            .map(|other| &other.ir)
            .chain(std::iter::once(std_ir))
            .collect();
        let model = codegen::lower(package, &others);
        for (shape, interface) in package.shapes().zip(&model.interfaces) {
            // An inline shape is named by its service, whose name already
            // carries the package.
            let owner = if shape.service.is_some() {
                shape.name.to_string()
            } else {
                format!("{}.{}", package.name, shape.name)
            };
            for (decl, slot) in shape.interface.interactions.iter().zip(&interface.slots) {
                let Some(v1::interaction_slot::Occupant::Interaction(interaction)) =
                    slot.occupant.as_ref()
                else {
                    continue;
                };
                let Some(span) = ctx.sites.member(&package.name, shape.name, &decl.name) else {
                    continue;
                };
                for payload in payloads(interaction, decl, package, &others) {
                    let mut found = Vec::new();
                    if flatbuffers {
                        found.extend(describe(&payload.flatbuffers, "flatbuffers"));
                    }
                    if proto3 {
                        found.extend(describe(&payload.proto3, "proto3"));
                    }
                    if found.is_empty() {
                        continue;
                    }
                    diagnostics.push(Diagnostic {
                        code: DiagCode::RIDL_150,
                        severity,
                        message: format!(
                            "the {} of `{owner}.{}` has {}; the payload ceiling is \
                             {PAYLOAD_CEILING} bytes, which the codecs do not yet enforce \
                             (driftsys/ridl#996)",
                            payload.role,
                            decl.name,
                            found.join(" and "),
                        ),
                        primary: span,
                        labels: Vec::new(),
                        fixits: Vec::new(),
                    });
                }
            }
        }
    }
    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Why a payload is reported against the payload ceiling.
    fn problem(size: Size) -> Option<String> {
        problem_above(size, u64::from(PAYLOAD_CEILING))
    }

    #[test]
    fn a_bound_is_a_problem_only_above_the_ceiling() {
        assert_eq!(problem(Size::Bytes(u64::from(PAYLOAD_CEILING))), None);
        assert_eq!(
            problem(Size::Bytes(u64::from(PAYLOAD_CEILING) + 1)).as_deref(),
            Some("up to 16257 bytes")
        );
    }

    #[test]
    fn a_limit_moves_the_boundary_and_keeps_the_unbounded_states() {
        assert_eq!(problem_above(Size::Bytes(10), 10), None);
        assert_eq!(
            problem_above(Size::Bytes(11), 10).as_deref(),
            Some("up to 11 bytes")
        );
        // The language reaches no payload with the `NoBound` state today: an
        // unbounded `string`, `bytes` or `integer` takes the default bound or
        // is a different error. The state stays in the match for the sizers.
        assert_eq!(
            problem_above(Size::NoBound, 10).as_deref(),
            Some("no finite bound")
        );
        assert!(
            problem_above(Size::Overflow, 10)
                .unwrap()
                .contains("64 levels")
        );
        assert_eq!(problem_above(Size::Other, 10), None);
    }

    #[test]
    fn a_fallible_reply_type_is_named_in_the_description() {
        let side = Side {
            size: Size::Bytes(20),
            ty: Some("Err".to_string()),
        };
        assert_eq!(
            describe_above(&side, "proto3", 10).as_deref(),
            Some("up to 20 bytes in proto3 for `Err`")
        );
    }

    #[test]
    fn only_the_states_about_size_are_problems() {
        assert_eq!(problem(Size::NoBound).as_deref(), Some("no finite bound"));
        assert!(problem(Size::Overflow).unwrap().contains("64 levels"));
        assert_eq!(problem(Size::Other), None);
    }

    fn absent(cause: v1::AbsentCause) -> v1::SizeState {
        v1::SizeState {
            state: Some(v1::size_state::State::Absent(v1::SizeAbsent {
                cause: cause as i32,
                detail: None,
            })),
        }
    }

    #[test]
    fn model_states_map_to_sizes() {
        let bounded = v1::SizeState {
            state: Some(v1::size_state::State::Bounded(7)),
        };
        assert_eq!(from_model(&bounded), Size::Bytes(7));
        let unbounded = v1::SizeState {
            state: Some(v1::size_state::State::Unbounded(v1::FbUnbounded::default())),
        };
        assert_eq!(from_model(&unbounded), Size::NoBound);
        assert_eq!(from_model(&absent(v1::AbsentCause::NoBound)), Size::NoBound);
        assert_eq!(
            from_model(&absent(v1::AbsentCause::Overflow)),
            Size::Overflow
        );
        for cause in [
            v1::AbsentCause::EncodingUndefined,
            v1::AbsentCause::NoMessage,
            v1::AbsentCause::RefusedMember,
            v1::AbsentCause::Unresolved,
        ] {
            assert_eq!(from_model(&absent(cause)), Size::Other, "{cause:?}");
        }
    }

    #[test]
    fn sizer_states_map_to_sizes() {
        assert_eq!(from_sizer(size::SizeState::Bounded(9)), Size::Bytes(9));
        assert_eq!(
            from_sizer(size::SizeState::Unbounded(size::UnboundedCause::Member)),
            Size::NoBound
        );
        assert_eq!(
            from_sizer(size::SizeState::Absent(size::AbsentCause::NoBound)),
            Size::NoBound
        );
        assert_eq!(
            from_sizer(size::SizeState::Absent(size::AbsentCause::Overflow)),
            Size::Overflow
        );
        for cause in [
            size::AbsentCause::EncodingUndefined,
            size::AbsentCause::NoMessage,
            size::AbsentCause::RefusedMember,
            size::AbsentCause::Unresolved,
        ] {
            assert_eq!(from_sizer(size::SizeState::Absent(cause)), Size::Other);
        }
    }

    #[test]
    fn the_worst_size_is_the_largest_problem() {
        assert!(Size::Other < Size::Bytes(0));
        assert!(Size::Bytes(20_000) < Size::Bytes(30_000));
        assert!(Size::Bytes(u64::MAX) < Size::Overflow);
        assert!(Size::Overflow < Size::NoBound);
    }

    #[test]
    fn no_encoding_named_checks_flatbuffers_alone() {
        assert_eq!(checked_encodings(&[]), (true, false));
        let names = |names: &[&str]| {
            names
                .iter()
                .map(|name| name.to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(checked_encodings(&names(&["proto3"])), (false, true));
        assert_eq!(
            checked_encodings(&names(&["flatbuffers", "proto3"])),
            (true, true)
        );
        assert_eq!(checked_encodings(&names(&["podbuf"])), (false, false));
    }
}
