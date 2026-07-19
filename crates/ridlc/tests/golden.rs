//! The walking-skeleton golden test (docs/ROADMAP.md epic E0.9, the exit
//! criterion): compile the fixture end to end, pin the generated Rust and the
//! lowered IR against committed snapshots, and drive the same pipeline through
//! the `ridlc` binary.

use std::path::Path;
use std::process::Command;

const FIXTURE: &str = include_str!("../../ridl-syntax/fixtures/walking_skeleton.typl");

/// Compiling the fixture produces no diagnostics, and its generated Rust and
/// lowered IR v1 package match the committed snapshots.
#[test]
fn fixture_compiles_to_committed_snapshots() {
    let output = ridlc::compile("walking_skeleton.typl", FIXTURE);

    assert!(
        output.diagnostics.is_empty(),
        "expected no diagnostics, got: {:?}",
        output.diagnostics,
    );

    insta::assert_snapshot!("generated_rust", output.rust_source);
    insta::assert_snapshot!("ir_package", ridl_ir::v1::to_json_pretty(&output.package));
}

/// A typl name that is a Rust keyword (`fn`) lexes as a valid identifier and
/// passes parse, resolve, and check, then the Rust backend rejects it. `compile`
/// stays total: it reports the failure as a diagnostic naming `fn` and leaves
/// `rust_source` empty instead of panicking.
#[test]
fn keyword_type_name_is_a_backend_diagnostic_not_a_panic() {
    let output = ridlc::compile("bad.typl", "type fn: m [0.0..1.0]\n");

    assert!(
        output.diagnostics.iter().any(|d| d.message.contains("fn")),
        "expected a diagnostic naming the offending identifier, got: {:?}",
        output.diagnostics,
    );
    assert!(
        output.rust_source.is_empty(),
        "a failed backend must leave rust_source empty, got:\n{}",
        output.rust_source,
    );
}

/// A duration literal inside a constraint fails a positional parser check and
/// crosses the profile boundary at the same token, so `compile` surfaces both a
/// FORM-101 and a TYPL-302 at the same offset. The JSON snapshot pins the
/// structured diagnostics — codes, severities, and exact byte offsets — and the
/// render snapshot pins the terminal output: two clean blocks, one caret each.
#[test]
fn duration_in_constraint_yields_two_coded_diagnostics() {
    let output = ridlc::compile("example.typl", "package p\ntype X: integer [0..10ms]\n");

    let codes: Vec<&str> = output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect();
    assert_eq!(codes, vec!["FORM-101", "TYPL-302"]);

    insta::assert_json_snapshot!("duration_in_constraint_diagnostics", output.diagnostics);
    insta::assert_snapshot!(
        "duration_in_constraint_render",
        ridl_core::diag::render(&output.diagnostics, &output.sources),
    );
}

/// `ridlc build <fixture> --out-dir <tmp>` exits 0 and writes a Rust file that
/// declares the generated `Speed` newtype.
#[test]
fn cli_build_writes_generated_rust() {
    let fixture_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../ridl-syntax/fixtures/walking_skeleton.typl");

    let out_dir = std::env::temp_dir().join(format!(
        "ridlc_cli_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock must read a time after the unix epoch")
            .as_nanos()
    ));

    let status = Command::new(env!("CARGO_BIN_EXE_ridlc"))
        .arg("build")
        .arg(&fixture_path)
        .arg("--out-dir")
        .arg(&out_dir)
        .status()
        .expect("the ridlc binary must run");

    assert!(status.success(), "ridlc build must exit 0 on the fixture");

    let out_file = out_dir.join("walking_skeleton.rs");
    let generated =
        std::fs::read_to_string(&out_file).expect("ridlc build must write the generated Rust file");
    assert!(
        generated.contains("pub struct Speed"),
        "generated file must declare the Speed newtype, got:\n{generated}"
    );

    std::fs::remove_dir_all(&out_dir).ok();
}
