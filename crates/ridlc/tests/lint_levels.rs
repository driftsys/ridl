//! The `[lints]` levels applied by the `ridlc` command drivers (ADR-0024
//! decisions 6 and 8): `run_check` and `run_build` apply them,
//! `check_source` applies the registry defaults, and `compile_workspace`
//! carries the scopes out unapplied.

use std::path::{Path, PathBuf};
use std::time::Duration;

use ridl_core::Frozen;
use ridl_core::diag::{Diagnostic, Severity};
use ridl_core::lint::{LintLevel, lint_by_name};
use ridlc::{ApplyLints, Emit};

/// A `signal` with no timing annotation, which draws RIDL-100
/// (`missing-timing`, a Warning by default).
const SOURCE: &str = "package demo\n\ntype Speed: integer [0..300]\n\ninterface Sensor {\n  \
                      signal speed: Speed\n}\n";

/// A workspace with one member, `sensor`, whose manifest ends with `lints`:
/// the text of a `[lints]` table, or an empty string for none.
struct Fixture {
    root: tempfile::TempDir,
}

impl Fixture {
    fn new(lints: &str) -> Self {
        let root = tempfile::tempdir().expect("a temp dir");
        std::fs::write(
            root.path().join("ridl.toml"),
            "[workspace]\nmembers = [\"sensor\"]\n",
        )
        .expect("the root manifest is written");
        let member = root.path().join("sensor");
        std::fs::create_dir(&member).expect("the member directory is created");
        std::fs::write(
            member.join("ridl.toml"),
            format!("[package]\nname = \"demo\"\nversion = \"1.0.0\"\n{lints}"),
        )
        .expect("the member manifest is written");
        std::fs::write(member.join("sensor.ridl"), SOURCE).expect("the source is written");
        Self { root }
    }

    fn root(&self) -> &Path {
        self.root.path()
    }

    fn source_path(&self) -> PathBuf {
        self.root.path().join("sensor").join("sensor.ridl")
    }
}

const DENY: &str = "\n[lints]\nmissing-timing = \"deny\"\n";
const ALLOW: &str = "\n[lints]\nmissing-timing = \"allow\"\n";

fn ridl_100(diagnostics: &[Diagnostic]) -> Vec<&Diagnostic> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code.as_str() == "RIDL-100")
        .collect()
}

/// The one RIDL-100 in `diagnostics`.
fn the_ridl_100(diagnostics: &[Diagnostic]) -> &Diagnostic {
    let found = ridl_100(diagnostics);
    assert_eq!(
        found.len(),
        1,
        "expected exactly one RIDL-100, got: {diagnostics:?}"
    );
    found[0]
}

#[test]
fn deny_turns_a_lint_into_an_error() {
    let fixture = Fixture::new(DENY);
    let run = ridlc::run_check(fixture.root(), Frozen::No).expect("the check runs");
    assert_eq!(the_ridl_100(&run.diagnostics).severity, Severity::Error);
    assert!(run.has_error());
}

#[test]
fn allow_removes_a_lint() {
    let fixture = Fixture::new(ALLOW);
    let run = ridlc::run_check(fixture.root(), Frozen::No).expect("the check runs");
    assert!(
        ridl_100(&run.diagnostics).is_empty(),
        "RIDL-100 is still present: {:?}",
        run.diagnostics
    );
    assert!(!run.has_error());
}

/// Pins that `run_check` leaves a lint with no `[lints]` entry at its
/// registry default: RIDL-100 is a Warning and does not fail the check. A
/// removed apply in the front end is not detected here, because today no
/// emit site draws a lint at a severity other than its catalogue row's;
/// `ridl_core::lint`'s `apply_uses_defaults_outside_scopes` pins the
/// normalisation itself.
#[test]
fn a_lint_without_lints_table_is_a_warning() {
    let fixture = Fixture::new("");
    let run = ridlc::run_check(fixture.root(), Frozen::No).expect("the check runs");
    assert_eq!(the_ridl_100(&run.diagnostics).severity, Severity::Warning);
    assert!(!run.has_error());
}

#[test]
fn deny_blocks_build() {
    let fixture = Fixture::new(DENY);
    let out = tempfile::tempdir().expect("a temp dir");
    let run = ridlc::run_build(fixture.root(), out.path(), &[Emit::IrJson], Frozen::No)
        .expect("the build runs");
    assert!(run.has_error());
    assert_eq!(the_ridl_100(&run.diagnostics).severity, Severity::Error);
    let written: Vec<PathBuf> = std::fs::read_dir(out.path())
        .expect("the out dir is readable")
        .map(|entry| entry.expect("a readable entry").path())
        .collect();
    assert!(written.is_empty(), "the build wrote: {written:?}");
}

/// Pins that `check_source`, which has no manifest and so no `[lints]`
/// table, reports a lint at its registry default: RIDL-100 is a Warning and
/// does not fail the check. A removed apply in the front end is not detected
/// here, because today no emit site draws a lint at a severity other than
/// its catalogue row's; `ridl_core::lint`'s
/// `apply_uses_defaults_outside_scopes` pins the normalisation itself.
#[test]
fn check_source_reports_the_lint_as_a_warning() {
    let run = ridlc::check_source("sensor.ridl", SOURCE);
    assert_eq!(the_ridl_100(&run.diagnostics).severity, Severity::Warning);
    assert!(!run.has_error());
}

/// The `ridlc build` binary applies the levels (`ApplyLints::Yes` at its
/// call site): a lint at `deny` exits 1 and writes no artifact. `run_build`
/// is pinned by `deny_blocks_build`; this test pins the binary's choice.
#[test]
fn binary_build_fails_on_deny() {
    let fixture = Fixture::new(DENY);
    let out = tempfile::tempdir().expect("a temp dir");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_ridlc"))
        .arg("build")
        .arg("--out-dir")
        .arg(out.path())
        .arg("--emit")
        .arg("ir-json")
        .arg(fixture.root())
        .output()
        .expect("the ridlc binary runs");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(1),
        "a lint at deny fails the build:\n{stderr}"
    );
    assert!(
        stderr.contains("error[RIDL-100]"),
        "the lint is rendered as an error:\n{stderr}"
    );
    let written: Vec<PathBuf> = std::fs::read_dir(out.path())
        .expect("the out dir is readable")
        .map(|entry| entry.expect("a readable entry").path())
        .collect();
    assert!(written.is_empty(), "the build wrote: {written:?}");
}

#[test]
fn compile_workspace_keeps_emitted_severities() {
    let fixture = Fixture::new(ALLOW);
    let mut db = ridl_core::RidlDatabase::default();
    let output = ridlc::compile_workspace(&mut db, fixture.root()).expect("the workspace compiles");
    assert_eq!(
        the_ridl_100(&output.diagnostics).severity,
        Severity::Warning
    );

    let missing_timing = lint_by_name("missing-timing").expect("missing-timing is a lint");
    let levels = output
        .lints
        .for_path(&fixture.source_path())
        .expect("the member's file is in a scope");
    assert_eq!(levels.level(missing_timing), Some(LintLevel::Allow));
}

#[test]
fn build_without_levels_writes_artifacts() {
    let fixture = Fixture::new(DENY);
    let out = tempfile::tempdir().expect("a temp dir");
    let run = ridlc::run_build_with(
        fixture.root(),
        out.path(),
        &[Emit::IrJson],
        &[],
        Duration::from_secs(10),
        Frozen::No,
        ApplyLints::No,
    )
    .expect("the build runs");
    assert!(!run.has_error(), "unexpected error: {:?}", run.diagnostics);
    assert_eq!(the_ridl_100(&run.diagnostics).severity, Severity::Warning);
    assert!(
        out.path().join("demo.ir.json").is_file(),
        "the IR file is missing; the out dir holds: {:?}",
        std::fs::read_dir(out.path()).map(|entries| entries
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>())
    );
}
