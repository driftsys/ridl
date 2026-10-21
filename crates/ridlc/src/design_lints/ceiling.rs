//! RIDL-150 `payload-above-ceiling`: a payload carried by an interaction whose
//! largest encoded size, in an encoding the package enables, is above the
//! payload ceiling, or has no finite bound (ADR-0030 decision 7).
//!
//! The sizes are the ones the codegen model carries for each payload, which
//! are the sizes the backends write as `MAX_SIZE`, so the lint and the
//! generated code never disagree about a bound. The diagnostic is reported on
//! the interaction that carries the payload; a type that no interaction
//! carries is not reported.

use ridl_core::diag::{DiagCode, Diagnostic};
use ridl_core::lint::lint_by_name;
use ridl_ir::codegen::{self, v1};

use super::Ctx;

/// The payload ceiling in bytes: the largest single transfer of 16,384 bytes
/// minus the 128 bytes a transport reserves for its header (ADR-0030
/// decisions 1 and 2). It is a platform rule, changed only by amending that
/// record.
pub(crate) const PAYLOAD_CEILING: u32 = 16_256;

/// Why one payload is reported under one encoding.
fn problem(state: &v1::SizeState) -> Option<String> {
    match state.state.as_ref()? {
        v1::size_state::State::Bounded(bytes) if *bytes > PAYLOAD_CEILING => {
            Some(format!("up to {bytes} bytes"))
        }
        v1::size_state::State::Bounded(_) => None,
        v1::size_state::State::Unbounded(_) => Some("no finite bound".to_string()),
        v1::size_state::State::Absent(absent) => match v1::AbsentCause::try_from(absent.cause) {
            Ok(v1::AbsentCause::NoBound) => Some("no finite bound".to_string()),
            Ok(v1::AbsentCause::Overflow) => Some("a bound too large to represent".to_string()),
            // Every other absent state means the payload has no encoding in
            // this codec, which is not a size.
            _ => None,
        },
    }
}

/// The payloads one interaction carries on the wire, with the role of each.
/// A fixed is provisioned and never in flight, so it carries none.
fn payloads(interaction: &v1::Interaction) -> Vec<(&'static str, &v1::PayloadSizes)> {
    match interaction.shape.as_ref() {
        Some(v1::interaction::Shape::Signal(signal)) => signal
            .payload
            .as_ref()
            .and_then(|payload| payload.sizes.as_ref())
            .map(|sizes| ("payload", sizes))
            .into_iter()
            .collect(),
        Some(v1::interaction::Shape::Event(event)) => event
            .payload
            .as_ref()
            .and_then(|payload| payload.sizes.as_ref())
            .map(|sizes| ("payload", sizes))
            .into_iter()
            .collect(),
        Some(v1::interaction::Shape::Command(command)) => command
            .request_sizes
            .as_ref()
            .map(|sizes| ("request", sizes))
            .into_iter()
            .collect(),
        Some(v1::interaction::Shape::Query(query)) => [
            query.request_sizes.as_ref().map(|sizes| ("request", sizes)),
            query.reply_sizes.as_ref().map(|sizes| ("reply", sizes)),
        ]
        .into_iter()
        .flatten()
        .collect(),
        _ => Vec::new(),
    }
}

/// The encodings to check: the names in `encodings` that have a size state,
/// or FlatBuffers when `encodings` names none.
fn checked_encodings(encodings: &[String]) -> (bool, bool) {
    if encodings.is_empty() {
        return (true, false);
    }
    (
        encodings.iter().any(|name| name == "flatbuffers"),
        encodings.iter().any(|name| name == "proto3"),
    )
}

pub(crate) fn check(
    ctx: &Ctx<'_>,
    std_ir: &ridl_ir::v2::Package,
    encodings: &[String],
) -> Vec<Diagnostic> {
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
        let others: Vec<&ridl_ir::v2::Package> = ctx
            .checked
            .iter()
            .filter(|other| other.ir.name != package.name)
            .map(|other| &other.ir)
            .chain(std::iter::once(std_ir))
            .collect();
        let model = codegen::lower(package, &others);
        for (shape, interface) in package.shapes().zip(&model.interfaces) {
            for (decl, slot) in shape.interface.interactions.iter().zip(&interface.slots) {
                let Some(v1::interaction_slot::Occupant::Interaction(interaction)) =
                    slot.occupant.as_ref()
                else {
                    continue;
                };
                let Some(span) = ctx.sites.member(&package.name, shape.name, &decl.name) else {
                    continue;
                };
                for (role, sizes) in payloads(interaction) {
                    let mut found = Vec::new();
                    if flatbuffers && let Some(why) = sizes.flatbuffers.as_ref().and_then(problem) {
                        found.push(format!("{why} in flatbuffers"));
                    }
                    if proto3 && let Some(why) = sizes.proto3.as_ref().and_then(problem) {
                        found.push(format!("{why} in proto3"));
                    }
                    if found.is_empty() {
                        continue;
                    }
                    diagnostics.push(Diagnostic {
                        code: DiagCode::RIDL_150,
                        severity,
                        message: format!(
                            "the {role} of `{}.{}.{}` has {}; a payload is capped at \
                             {PAYLOAD_CEILING} bytes and a larger value is refused at run time",
                            package.name,
                            shape.name,
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

    fn state(state: v1::size_state::State) -> v1::SizeState {
        v1::SizeState { state: Some(state) }
    }

    fn absent(cause: v1::AbsentCause) -> v1::SizeState {
        state(v1::size_state::State::Absent(v1::SizeAbsent {
            cause: cause as i32,
            detail: None,
        }))
    }

    #[test]
    fn a_bound_is_a_problem_only_above_the_ceiling() {
        let bounded = |bytes| state(v1::size_state::State::Bounded(bytes));
        assert_eq!(problem(&bounded(PAYLOAD_CEILING)), None);
        assert_eq!(
            problem(&bounded(PAYLOAD_CEILING + 1)).as_deref(),
            Some("up to 16257 bytes")
        );
    }

    #[test]
    fn an_unbounded_state_is_a_problem() {
        let unbounded = state(v1::size_state::State::Unbounded(v1::FbUnbounded::default()));
        assert_eq!(problem(&unbounded).as_deref(), Some("no finite bound"));
    }

    #[test]
    fn only_the_absent_causes_about_size_are_problems() {
        assert_eq!(
            problem(&absent(v1::AbsentCause::NoBound)).as_deref(),
            Some("no finite bound")
        );
        assert_eq!(
            problem(&absent(v1::AbsentCause::Overflow)).as_deref(),
            Some("a bound too large to represent")
        );
        for cause in [
            v1::AbsentCause::EncodingUndefined,
            v1::AbsentCause::NoMessage,
            v1::AbsentCause::RefusedMember,
            v1::AbsentCause::Unresolved,
        ] {
            assert_eq!(problem(&absent(cause)), None, "{cause:?}");
        }
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
        assert_eq!(checked_encodings(&names(&["repr-c"])), (false, false));
    }
}
