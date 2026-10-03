//! The `[lints]` levels applied by the `ridlc` command drivers (lint
//! foundation spec D-8, §6.2): `run_check` and `run_build` apply them,
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

#[test]
fn defaults_normalise_severity() {
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

#[test]
fn check_source_uses_defaults() {
    let run = ridlc::check_source("sensor.ridl", SOURCE);
    assert_eq!(the_ridl_100(&run.diagnostics).severity, Severity::Warning);
    assert!(!run.has_error());
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
    assert_eq!(levels.level(missing_timing), LintLevel::Allow);
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
