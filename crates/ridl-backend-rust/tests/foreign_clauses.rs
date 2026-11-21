//! A contract clause on a parameter or a reply whose named scalar is declared
//! in another package is translated when the backend is handed that package,
//! and the interface keeps its face (driftsys/ridl#1414).

use ridl_backend_rust::{WireEncoding, generate_pipeline};
use ridl_ir::v2;

// `Pad` is declared first and is never referenced, and `Ratio` is a float, so
// that a foreign lookup at the wrong index or in the wrong table changes the
// rendered backing.
const PA: &str = "package pa\n\ntype Pad: integer [0..5] = 1\n\n\
type Level: integer [0..100] = 7\n\ntype Ratio: float [0.0..1.0] = 0.0\n";

const PB: &str = r#"package pb

import pa.Level
import pa.Ratio

type Local: float [0.0..100.0] = 7.0

interface Svc {
  command set(level: Level) @[..50ms] [ require level < 100 ]
  command scale(ratio: Ratio) @[..50ms] [ require ratio <= 0.5 ]
  command own(level: Local) @[..50ms] [ require level < 90 ]
  query peek(): Level @[..50ms] [ ensure result < 80 ]
}
"#;

fn packages() -> Vec<v2::Package> {
    let dir = tempfile::tempdir().expect("a temp dir is created");
    std::fs::write(
        dir.path().join("ridl.toml"),
        "[workspace]\nmembers = [\"pa\", \"pb\"]\n",
    )
    .unwrap();
    for (name, source) in [("pa", PA), ("pb", PB)] {
        let member = dir.path().join(name);
        std::fs::create_dir(&member).unwrap();
        std::fs::write(
            member.join("ridl.toml"),
            format!("[package]\nname = \"{name}\"\nversion = \"1.0.0\"\n"),
        )
        .unwrap();
        std::fs::write(member.join(format!("{name}.ridl")), source).unwrap();
    }
    let mut db = ridl_core::RidlDatabase::default();
    let output = ridlc::compile_workspace(&mut db, dir.path()).expect("the workspace loads");
    let errors: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ridl_core::diag::Severity::Error))
        .map(|d| format!("{} {}", d.code.0, d.message))
        .collect();
    assert!(errors.is_empty(), "the workspace draws errors: {errors:?}");
    output.checked.into_iter().map(|c| c.ir).collect()
}

fn compact(source: &str) -> String {
    source.chars().filter(|c| !c.is_whitespace()).collect()
}

#[test]
fn a_clause_on_a_foreign_named_scalar_keeps_the_face() {
    let packages = packages();
    let pb = packages
        .iter()
        .find(|p| p.name == "pb")
        .expect("pb compiles");
    let others: Vec<_> = packages.iter().filter(|p| p.name != "pb").collect();
    let source = generate_pipeline(pb, &[WireEncoding::FlatBuffers], false, &others)
        .expect("pb generates")
        .rust_source;
    assert!(
        !source.contains("__RIDL_NO_FACE_Svc"),
        "the face of Svc must be emitted:\n{source}"
    );
    let text = compact(&source);
    for expected in [
        "ifargs.get()<100{",
        "ifargs.get()<=0.5{",
        "ifargs.0<90.0{",
        "ifreply.get()<80{",
    ] {
        assert!(text.contains(expected), "missing `{expected}`:\n{source}");
    }
}

#[test]
fn a_clause_on_a_foreign_scalar_not_given_to_the_backend_names_the_cause() {
    let packages = packages();
    let pb = packages
        .iter()
        .find(|p| p.name == "pb")
        .expect("pb compiles");
    let source = generate_pipeline(pb, &[WireEncoding::FlatBuffers], false, &[])
        .expect("pb generates")
        .rust_source;
    assert!(source.contains("__RIDL_NO_FACE_Svc"), "{source}");
    assert!(
        source.contains("declared in another package that was not given to the backend"),
        "the note must state the cause:\n{source}"
    );
}
