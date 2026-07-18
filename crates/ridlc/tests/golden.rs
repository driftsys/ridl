//! The walking-skeleton golden test (docs/ROADMAP.md epic E0.9, the exit
//! criterion): compile the fixture end to end, pin the generated Rust and the
//! lowered IR against committed snapshots, and drive the same pipeline through
//! the `ridlc` binary.

use std::path::Path;
use std::process::Command;

const FIXTURE: &str = include_str!("../../ridl-syntax/fixtures/walking_skeleton.typl");

/// Compiling the fixture produces no diagnostics, and its generated Rust and
/// lowered IR match the committed snapshots.
#[test]
fn fixture_compiles_to_committed_snapshots() {
    let output = ridlc::compile("walking_skeleton.typl", FIXTURE);

    assert!(
        output.diagnostics.is_empty(),
        "expected no diagnostics, got: {:?}",
        output.diagnostics,
    );

    insta::assert_snapshot!("generated_rust", output.rust_source);
    insta::assert_json_snapshot!("ir_module", output.module);
}

/// A typl name that is a Rust keyword (`fn`) lexes as a valid identifier and
/// passes parse, resolve, and check, then the Rust backend rejects it. `compile`
/// stays total: it reports the failure as a diagnostic naming `fn` and leaves
/// `rust_source` empty instead of panicking.
#[test]
fn keyword_type_name_is_a_backend_diagnostic_not_a_panic() {
    let output = ridlc::compile("bad.typl", "type fn: m [0.0..1.0]\n");

    assert!(
        output.diagnostics.iter().any(|d| d.contains("fn")),
        "expected a diagnostic naming the offending identifier, got: {:?}",
        output.diagnostics,
    );
    assert!(
        output.rust_source.is_empty(),
        "a failed backend must leave rust_source empty, got:\n{}",
        output.rust_source,
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
