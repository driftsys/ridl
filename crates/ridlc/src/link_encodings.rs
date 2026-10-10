//! RSDL-807 `link-encoding-not-emitted`: a consumer link of the selected
//! deployment carries an encoding that the emitted backend's
//! `[backend.<name>] encodings` list does not name.
//!
//! The check is a build's, not the system query's: only a build knows which
//! backends it emits, so `run_build` raises it once per emitted backend, and
//! `check` never does. It walks the checked system rather than the lowered
//! one because the lowered `Deployment` carries no span: each link's crossing
//! comes from [`link_crossing`], the rule the lowering writes on the link, and
//! its encoding from [`encoding_of`], the rule the codegen model writes
//! (`docs/design/codegen-plugins.md`, "The encoding rule"). The diagnostic
//! points at the consumer's `requires` line.

use ridl_core::RidlDatabase;
use ridl_core::diag::{DiagCode, Diagnostic, Severity, SourceMap, Span};
use ridl_ir::codegen::{encoding_of, v1};
use ridl_sem::CheckedSystem;
use ridl_sem::rsdl::{ClosureComponent, UNIT_INSTANCE, link_crossing};

/// One RSDL-807 warning per consumer link of the deployment named
/// `deployment` whose encoding `encodings`, the `[backend.<backend>]` list,
/// does not name. A link with an unspecified encoding draws nothing. Nothing
/// is drawn when the system declares no deployment of that name, or when an
/// RSDL-7xx error blocked its placement (rsdl reference §13).
pub(crate) fn link_encoding_not_emitted(
    db: &RidlDatabase,
    system: &CheckedSystem,
    deployment: &str,
    backend: &str,
    encodings: &[String],
    sources: &mut SourceMap,
) -> Vec<Diagnostic> {
    let Some(closure) = &system.closure else {
        return Vec::new();
    };
    let Some((decl, placement)) = system
        .deployments
        .iter()
        .zip(&system.placements)
        .find(|(decl, placement)| decl.name.name == deployment && !placement.has_errors)
    else {
        return Vec::new();
    };
    let placed = |component: usize, instance: &str| {
        placement
            .placements
            .iter()
            .find(|placed| placed.component == component && placed.instance == instance)
    };
    let mut diagnostics = Vec::new();
    for require in &closure.requires {
        let consumer = &closure.components[require.consumer];
        let producer = &closure.components[require.producer];
        // rsdl §10: a link with two external endpoints is not lowered, so it
        // carries no encoding.
        if consumer.external && producer.external {
            continue;
        }
        // An implicit component writes no `requires` line, so it never
        // consumes.
        let Some(consumer_decl) = consumer.decl else {
            continue;
        };
        let site = system.components[consumer_decl].requires[require.line]
            .reference
            .site;
        for consumer_instance in &consumer.instances {
            for producer_instance in &producer.instances {
                let (Some(from), Some(to)) = (
                    placed(require.consumer, consumer_instance),
                    placed(require.producer, producer_instance),
                ) else {
                    continue;
                };
                let crossing = link_crossing(decl, from, to);
                let Some(encoding) = encoding_name(encoding_of(crossing as i32)) else {
                    continue;
                };
                if encodings.iter().any(|name| name == encoding) {
                    continue;
                }
                diagnostics.push(Diagnostic {
                    code: DiagCode::RSDL_807,
                    severity: Severity::Warning,
                    message: format!(
                        "link `{} -> {}` of deployment `{deployment}` carries `{encoding}`, which \
                         `[backend.{backend}] encodings` does not name",
                        instance_name(consumer, consumer_instance),
                        instance_name(producer, producer_instance),
                    ),
                    primary: Span {
                        file: sources.file_id(site.file.path(db), site.file.text(db)),
                        range: site.range,
                    },
                    labels: Vec::new(),
                    fixits: Vec::new(),
                });
            }
        }
    }
    diagnostics
}

/// The name a `[backend.<name>] encodings` list writes for a codegen-model
/// encoding (`ridl_core::manifest::ENCODING_NAMES`), or `None` for the
/// unspecified encoding, which no list can name.
fn encoding_name(encoding: i32) -> Option<&'static str> {
    match v1::Encoding::try_from(encoding) {
        Ok(v1::Encoding::Flatbuffers) => Some("flatbuffers"),
        Ok(v1::Encoding::Proto3) => Some("proto3"),
        Ok(v1::Encoding::ReprC) => Some("repr-c"),
        Ok(v1::Encoding::Unspecified) | Err(_) => None,
    }
}

/// The name a diagnostic prints for an instance: the component's name for
/// the unit instance, `Component.instance` otherwise, as RSDL-805 prints it.
fn instance_name(component: &ClosureComponent, instance: &str) -> String {
    if instance == UNIT_INSTANCE {
        component.id.text().to_string()
    } else {
        format!("{}.{instance}", component.id.text())
    }
}

#[cfg(test)]
mod tests {
    use ridl_ir::v2;

    use super::*;

    /// The crossing the lowering writes decides the encoding the list must
    /// name: within one machine FlatBuffers, across machines proto3.
    #[test]
    fn each_crossing_maps_to_one_encoding_name() {
        assert_eq!(
            encoding_name(encoding_of(v2::Crossing::SameMachine as i32)),
            Some("flatbuffers")
        );
        assert_eq!(
            encoding_name(encoding_of(v2::Crossing::DifferentMachine as i32)),
            Some("proto3")
        );
        assert_eq!(
            encoding_name(encoding_of(v2::Crossing::OffBoard as i32)),
            Some("proto3")
        );
        assert_eq!(
            encoding_name(encoding_of(v2::Crossing::Unspecified as i32)),
            None
        );
    }
}
