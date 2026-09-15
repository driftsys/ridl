//! The typed AST over the rsdl declarations (rsdl reference §3, §4, §5): the
//! accessors the rsdl checker reads, exercised on the reference's own
//! examples. The CST shape of the same sources is pinned by the rsdl ok-corpus
//! snapshots (`parser_corpus.rs`).

use ridl_syntax::ast::{
    AstNode, ComponentLineKind, HasDocComments, HasName, Reference, SourceFile,
};
use ridl_syntax::{Profile, parse};

fn source(text: &str) -> SourceFile {
    let parsed = parse(text, Profile::Rsdl);
    assert_eq!(parsed.errors(), &[], "the example parses clean");
    SourceFile::cast(parsed.syntax()).expect("the root is a SourceFile")
}

fn name_of(node: &impl HasName) -> String {
    node.name()
        .and_then(|name| name.ident_token())
        .map(|token| token.text().to_string())
        .unwrap_or_default()
}

fn text_of(reference: Option<Reference>) -> String {
    reference
        .expect("the line carries a reference")
        .syntax()
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
        .map(|token| token.text().to_string())
        .collect()
}

/// rsdl reference Appendix A, `system.rsdl`, verbatim.
const SYSTEM: &str = r#"package veh.topology

import veh.adas.CruiseControl
import veh.adas.LaneAssist

/// Adaptive cruise, two copies: one per compute node.
component Cruise [ instances = (primary, backup), labels = (ASIL_B) ] {
  offers   veh.adas.cruise
  requires LaneAssist
}

component Lane { offers veh.adas.lane }

component Panel {
  requires CruiseControl
  requires LaneAssist
}

/// The fleet backend. No implementation in this workspace.
component Backend [ external ] {
  requires CruiseControl
  requires veh.diag.access
}

system Vehicle { Cruise, Lane, Panel, Backend, veh.diag.access }

distribution Adas [ tier = PLATFORM ]    { Cruise, Lane, veh.diag.access }
distribution Hmi  [ tier = APPLICATION ] { Panel }
"#;

/// rsdl reference Appendix A, `production.rsdl`, with the same package.
const PRODUCTION: &str = r#"package veh.topology

deployment Production for Vehicle {
  machine AdasHpc [ labels = (ASIL_B) ] { Cruise.primary, Lane, veh.diag.access }
  machine Cockpit { Cruise.backup, Panel [ linux.cpuset = (2, 3) ] }
  machine Cloud   [ external ] { Backend }
}
"#;

#[test]
fn components_expose_their_lines_attributes_and_doc_comments() {
    let file = source(SYSTEM);
    let components: Vec<_> = file.components().collect();
    assert_eq!(
        components.iter().map(name_of).collect::<Vec<_>>(),
        ["Cruise", "Lane", "Panel", "Backend"],
    );

    let cruise = &components[0];
    assert_eq!(cruise.doc_comments().len(), 1, "the doc comment attaches");
    let lines: Vec<_> = cruise
        .lines()
        .map(|line| (line.kind(), text_of(line.reference())))
        .collect();
    assert_eq!(
        lines,
        [
            (
                Some(ComponentLineKind::Offers),
                "veh.adas.cruise".to_string()
            ),
            (Some(ComponentLineKind::Requires), "LaneAssist".to_string()),
        ],
    );
    let keys: Vec<String> = cruise
        .attr_block()
        .expect("Cruise has an attribute block")
        .attributes()
        .map(|attribute| attribute.syntax().text().to_string())
        .collect();
    assert_eq!(keys, ["instances = (primary, backup)", "labels = (ASIL_B)"]);

    let backend = &components[3];
    let flag = backend
        .attr_block()
        .and_then(|block| block.attributes().next())
        .expect("Backend is flagged");
    assert!(flag.value().is_none(), "`external` is a flag");
    assert_eq!(backend.lines().count(), 2);
}

#[test]
fn systems_and_distributions_hold_member_lines() {
    let file = source(SYSTEM);
    let system = file.systems().next().expect("one system");
    assert_eq!(name_of(&system), "Vehicle");
    assert_eq!(
        system
            .lines()
            .map(|line| text_of(line.reference()))
            .collect::<Vec<_>>(),
        ["Cruise", "Lane", "Panel", "Backend", "veh.diag.access"],
    );

    let distributions: Vec<_> = file.distributions().collect();
    assert_eq!(
        distributions.iter().map(name_of).collect::<Vec<_>>(),
        ["Adas", "Hmi"]
    );
    let tier = distributions[0]
        .attr_block()
        .and_then(|block| block.attributes().next())
        .and_then(|attribute| attribute.value())
        .expect("Adas has a tier value");
    assert_eq!(tier.syntax().text().to_string(), "PLATFORM");
}

#[test]
fn a_deployment_names_its_system_and_holds_machines_with_placement_lines() {
    let file = source(PRODUCTION);
    let deployment = file.deployments().next().expect("one deployment");
    assert_eq!(name_of(&deployment), "Production");
    assert_eq!(text_of(deployment.system()), "Vehicle");

    let machines: Vec<_> = deployment.machines().collect();
    assert_eq!(
        machines.iter().map(name_of).collect::<Vec<_>>(),
        ["AdasHpc", "Cockpit", "Cloud"],
    );
    assert_eq!(
        machines[0]
            .lines()
            .map(|line| text_of(line.reference()))
            .collect::<Vec<_>>(),
        ["Cruise.primary", "Lane", "veh.diag.access"],
    );

    // `Panel [ linux.cpuset = (2, 3) ]`: the backend key on a placement line.
    let panel = machines[1].lines().nth(1).expect("Cockpit's second line");
    let attribute = panel
        .attr_block()
        .and_then(|block| block.attributes().next())
        .expect("the line carries an attribute");
    let segments: Vec<String> = attribute
        .key_segments()
        .filter_map(|name| Some(name.ident_token()?.text().to_string()))
        .collect();
    assert_eq!(segments, ["linux", "cpuset"]);
    assert!(attribute.dot_token().is_some());
    let values: Vec<String> = attribute
        .value()
        .expect("an assignment")
        .values()
        .map(|value| value.syntax().text().to_string())
        .collect();
    assert_eq!(values, ["2", "3"]);
}
