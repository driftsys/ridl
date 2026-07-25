//! The walking-skeleton golden test (docs/ROADMAP.md epic E0.9, the exit
//! criterion): compile the fixture end to end, pin the generated Rust and the
//! lowered IR against committed snapshots, and drive the same pipeline through
//! the `ridlc` binary.

use std::path::Path;
use std::process::Command;

const FIXTURE: &str = include_str!("../../ridl-syntax/fixtures/walking_skeleton.typl");

/// Compiling the fixture produces no diagnostics, and its generated Rust and
/// lowered IR v2 package match the committed snapshots.
#[test]
fn fixture_compiles_to_committed_snapshots() {
    let output = ridlc::compile("walking_skeleton.typl", FIXTURE);

    assert!(
        output.diagnostics.is_empty(),
        "expected no diagnostics, got: {:?}",
        output.diagnostics,
    );

    insta::assert_snapshot!("generated_rust", output.rust_source);
    insta::assert_snapshot!("ir_package", ridl_ir::v2::to_json_pretty(&output.package));
}

/// A typl name that is a Rust keyword (`fn`) lexes as a valid identifier and
/// passes parse, resolve, and check. The E1.12 Rust backend escapes it as a raw
/// identifier (`r#fn`) rather than rejecting it — the raw-escaping decision the
/// walking-skeleton backend deferred to E1.12. `compile` stays total and emits
/// valid Rust with no diagnostic.
#[test]
fn keyword_type_name_is_raw_escaped() {
    let output = ridlc::compile(
        "keyword.typl",
        "package p\ntype fn: m [0.0..1.0 step 0.1]\n",
    );

    assert!(
        output.diagnostics.is_empty(),
        "a keyword type name must not raise a diagnostic, got: {:?}",
        output.diagnostics,
    );
    assert!(
        output.rust_source.contains("r#fn"),
        "the keyword type name must be raw-escaped, got:\n{}",
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

/// The malformed interaction sources that reach the lowering with no name:
/// FORM-101 (no name token) and FORM-105 (a family reserved word used as a
/// name — `view` belongs to uxdl, so it is reserved family-wide but is not an
/// active ridl keyword), at both interaction sites (an `interface` body and a
/// service's inline shape).
const NAMELESS_INTERACTIONS: &[&str] = &[
    "interface I {\n  signal : Speed @10ms\n  signal after : Speed @10ms\n}\n",
    "interface I {\n  event : Speed\n  signal after : Speed @10ms\n}\n",
    "interface I {\n  command (g: Speed)\n  signal after : Speed @10ms\n}\n",
    "interface I {\n  query (): Speed\n  signal after : Speed @10ms\n}\n",
    "interface I {\n  final : Speed = 1.0\n  signal after : Speed @10ms\n}\n",
    "interface I {\n  signal view : Speed @10ms\n  signal after : Speed @10ms\n}\n",
    "service veh.a.b {\n  signal : Speed @10ms\n  signal after : Speed @10ms\n}\n",
    "service veh.a.b {\n  signal view : Speed @10ms\n  signal after : Speed @10ms\n}\n",
    "interface I {\n  command setGear(view: Speed)\n  signal after : Speed @10ms\n}\n",
];

/// `compile` is total on a nameless interaction. Each source here is reported
/// by the parser and still finishes its member node, so the member reaches the
/// lowering; before the fix it lowered to a `Decl` with an empty name, which
/// the Rust backend cannot turn into an identifier. `compile` runs the backend
/// unconditionally — it does not gate on diagnostics — so an empty name there
/// breaks the never-panics contract this module's crate documents.
#[test]
fn compile_is_total_on_a_nameless_interaction() {
    for body in NAMELESS_INTERACTIONS {
        let source = format!("package app\ntype Speed: km/h [0.0..300.0 step 0.5]\n{body}");
        // A panic here fails the test, which is the point: `compile` must
        // return for every one of these.
        let output = ridlc::compile("app.ridl", &source);

        let mut interactions: Vec<&ridl_ir::v2::Decl> = output
            .package
            .interfaces
            .iter()
            .flat_map(|interface| interface.interactions.iter())
            .collect();
        for service in &output.package.services {
            if let Some(ridl_ir::v2::service::Shape::Inline(inline)) = &service.shape {
                interactions.extend(inline.interactions.iter());
            }
        }
        for decl in interactions {
            // A `reserved` tombstone's `Decl` name is empty by design — the
            // retired name lives in `Reserved.name`.
            if matches!(decl.kind, Some(ridl_ir::v2::decl::Kind::ReservedSlot(_))) {
                continue;
            }
            assert!(
                !decl.name.is_empty(),
                "an empty-named interaction reached the IR from:\n{body}",
            );
        }
    }
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
