//! `--emit rust` writes a compiling crate (Task 7,
//! docs/wip/typl-value-objects-plan.md): a `Cargo.toml` and `lib.rs` alongside
//! the per-package `.rs` files, so the flat emitted files resolve at the
//! `crate::…` paths `ridl_backend_rust::type_path` already generates.

use std::path::Path;

use ridlc::Emit;

/// `run_build` over a multi-package workspace with `&[Emit::Rust]` writes a
/// `Cargo.toml` and a `lib.rs` alongside the per-package sources, and the
/// manifest carries the `validate-pattern`/`std` feature set and the `regex`
/// and `ridl-rt` dependencies the plan specifies.
#[test]
fn rust_emit_writes_a_compiling_crate() {
    let out = tempfile::tempdir().expect("temp dir");
    let run = ridlc::run_build(
        Path::new("tests/corpus/workspace-two-members"),
        out.path(),
        &[Emit::Rust],
        false.into(),
    )
    .expect("build runs");
    assert!(
        !run.has_error(),
        "expected no error, got: {:?}",
        run.diagnostics
    );

    assert!(out.path().join("Cargo.toml").exists());
    assert!(out.path().join("lib.rs").exists());

    let manifest = std::fs::read_to_string(out.path().join("Cargo.toml")).unwrap();
    assert!(manifest.contains("default = [\"validate-pattern\", \"std\"]"));
    assert!(manifest.contains("regex = { version = \"1\", optional = true }"));
    assert!(manifest.contains("ridl-rt = "));

    let lib = std::fs::read_to_string(out.path().join("lib.rs")).unwrap();
    assert!(lib.contains("pub mod veh"), "lib.rs was:\n{lib}");
}

/// THE REAL PROOF: the emitted crate root actually compiles with `rustc`,
/// which is the only thing that proves the module tree resolves — including
/// every `crate::…` cross-package reference the packages make to each other.
/// This deliberately does not pass `--extern ridl_rt`: nothing generated names
/// the runtime crate until Task 3 lands, so the argument would be inert and
/// would hide an `E0433` if generated code named `ridl_rt` when it should not
/// yet. Task 3 adds `--extern ridl_rt` to this test.
#[test]
fn emitted_lib_rs_compiles_with_rustc() {
    let out = tempfile::tempdir().expect("temp dir");
    let run = ridlc::run_build(
        Path::new("tests/corpus/veh-cluster"),
        out.path(),
        &[Emit::Rust],
        false.into(),
    )
    .expect("build runs");
    assert!(
        !run.has_error(),
        "expected no error, got: {:?}",
        run.diagnostics
    );

    let rmeta = out.path().join("out.rmeta");
    let status = std::process::Command::new("rustc")
        .current_dir(out.path())
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--emit",
            "metadata",
            "-o",
        ])
        .arg(&rmeta)
        .arg("lib.rs")
        .status()
        .expect("rustc must run");
    assert!(
        status.success(),
        "rustc failed to compile the emitted crate root"
    );
}

/// A package referencing `ridl.std` makes the build write `ridl.std.rs`
/// (issue #190); `lib.rs` must make it reachable as `crate::ridl::std`, or the
/// crate the other packages reference from does not compile.
#[test]
fn ridl_std_is_reachable_from_lib_rs() {
    let out = tempfile::tempdir().expect("temp dir");
    let run = ridlc::run_build(
        Path::new("tests/corpus/veh-common"),
        out.path(),
        &[Emit::Rust],
        false.into(),
    )
    .expect("build runs");
    assert!(
        !run.has_error(),
        "expected no error, got: {:?}",
        run.diagnostics
    );

    assert!(
        out.path().join("ridl.std.rs").exists(),
        "the fixture references ridl.std, so the build must write ridl.std.rs"
    );

    let lib = std::fs::read_to_string(out.path().join("lib.rs")).unwrap();
    assert!(
        lib.contains("#[path = \"ridl.std.rs\"]") && lib.contains("pub mod std"),
        "lib.rs must make ridl.std.rs reachable as crate::ridl::std, was:\n{lib}"
    );
}

/// Single-file mode (no manifest anywhere up the tree) keeps writing only
/// `<stem>.rs`, matching the documented single-file asymmetry on
/// `Emit::TypeScript` — it never writes `Cargo.toml` or `lib.rs`.
#[test]
fn single_file_mode_writes_neither_manifest_nor_lib_rs() {
    let out = tempfile::tempdir().expect("temp dir");
    let entry_dir = tempfile::tempdir().expect("temp entry dir");
    let entry = entry_dir.path().join("standalone.typl");
    std::fs::write(
        &entry,
        "package standalone\ntype Speed: km/h [0.0..250.0 step 0.5]\n",
    )
    .unwrap();

    let run =
        ridlc::run_build(&entry, out.path(), &[Emit::Rust], false.into()).expect("build runs");
    assert!(
        !run.has_error(),
        "expected no error, got: {:?}",
        run.diagnostics
    );

    assert!(out.path().join("standalone.rs").exists());
    assert!(!out.path().join("Cargo.toml").exists());
    assert!(!out.path().join("lib.rs").exists());
}

/// The generated crate name comes from the manifest's `[package]` name with
/// every `.` replaced by `_`, since a dotted ridl package name (`veh.common`)
/// is not a legal Cargo package name.
#[test]
fn crate_name_replaces_dots_with_underscores() {
    let out = tempfile::tempdir().expect("temp dir");
    let run = ridlc::run_build(
        Path::new("tests/corpus/veh-common"),
        out.path(),
        &[Emit::Rust],
        false.into(),
    )
    .expect("build runs");
    assert!(
        !run.has_error(),
        "expected no error, got: {:?}",
        run.diagnostics
    );

    let manifest = std::fs::read_to_string(out.path().join("Cargo.toml")).unwrap();
    assert!(
        manifest.contains("name = \"veh_common\""),
        "manifest was:\n{manifest}"
    );
}

/// A `[workspace]` manifest names no package, so the generated crate falls
/// back to `ridl_generated`.
#[test]
fn crate_name_falls_back_for_a_workspace_manifest() {
    let out = tempfile::tempdir().expect("temp dir");
    let run = ridlc::run_build(
        Path::new("tests/corpus/workspace-two-members"),
        out.path(),
        &[Emit::Rust],
        false.into(),
    )
    .expect("build runs");
    assert!(
        !run.has_error(),
        "expected no error, got: {:?}",
        run.diagnostics
    );

    let manifest = std::fs::read_to_string(out.path().join("Cargo.toml")).unwrap();
    assert!(
        manifest.contains("name = \"ridl_generated\""),
        "manifest was:\n{manifest}"
    );
}

/// The `ridl-rt` dependency requirement in the emitted manifest and
/// `crates/ridl-rt`'s own version must agree on major and minor, or the
/// generated crate could pin a `ridl-rt` release that predates the API the
/// codegen it emits actually needs. `ridlc` cannot read
/// `crates/ridl-rt/Cargo.toml` at run time — it is an installed binary with no
/// access to this repository's sources — so the manifest string is a literal;
/// this test is what keeps the literal and the crate's real version from
/// drifting apart silently (Task 7's own rationale for reading the version at
/// emit time, redirected into a guard test instead).
#[test]
fn ridl_rt_version_requirement_matches_the_crate() {
    #[derive(serde::Deserialize)]
    struct CargoManifest {
        package: CargoPackage,
    }
    #[derive(serde::Deserialize)]
    struct CargoPackage {
        version: String,
    }

    let ridl_rt_manifest_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../ridl-rt/Cargo.toml"
    ))
    .expect("crates/ridl-rt/Cargo.toml must exist");
    let ridl_rt_manifest: CargoManifest = toml::from_str(&ridl_rt_manifest_text)
        .expect("crates/ridl-rt/Cargo.toml must be valid TOML");
    let version = ridl_rt_manifest.package.version;
    let mut segments = version.split('.');
    let major = segments
        .next()
        .expect("a semver version has a major segment");
    let minor = segments
        .next()
        .expect("a semver version has a minor segment");
    let expected = format!("ridl-rt = \"{major}.{minor}\"");

    let out = tempfile::tempdir().expect("temp dir");
    let run = ridlc::run_build(
        Path::new("tests/corpus/veh-common"),
        out.path(),
        &[Emit::Rust],
        false.into(),
    )
    .expect("build runs");
    assert!(
        !run.has_error(),
        "expected no error, got: {:?}",
        run.diagnostics
    );
    let manifest = std::fs::read_to_string(out.path().join("Cargo.toml")).unwrap();
    assert!(
        manifest.contains(&expected),
        "expected `{expected}` in the emitted manifest, got:\n{manifest}"
    );
}
