//! A contract clause on a parameter or a reply whose named scalar is declared
//! in another package is translated, and the interface keeps its face
//! (driftsys/ridl#1414).

use ridl_backend_rust::{WireEncoding, generate_pipeline};

const PA: &str = "package pa\n\ntype Level: integer [0..100] = 7\n";

const PB: &str = r#"package pb

import pa.Level

type Local: integer [0..100] = 7

interface Svc {
  command set(level: Level) @[..50ms] [ require level < 100 ]
  command own(level: Local) @[..50ms] [ require level < 90 ]
  query peek(): Level @[..50ms] [ ensure result < 80 ]
}
"#;

#[test]
fn a_clause_on_a_foreign_named_scalar_keeps_the_face() {
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
    let packages: Vec<_> = output.checked.into_iter().map(|c| c.ir).collect();
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
    assert!(
        source.contains("args.0 < 100"),
        "foreign require:\n{source}"
    );
    assert!(source.contains("args.0 < 90"), "local require:\n{source}");
    assert!(source.contains("< 80"), "foreign ensure:\n{source}");
}
