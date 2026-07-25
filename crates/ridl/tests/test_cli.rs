//! `ridl test` — the property runner over a workspace (E2.11a).
//!
//! Drives the binary the way a user does and pins the three report sections
//! (range self-corpora, `require` satisfiability sampling, `ensure` observer
//! stubs), the exit contract (0 clean, 1 a self-corpus failure or an evaluation
//! error, 2 a compile or I/O error), the JSON rendering, and the determinism
//! that seeding per contract buys.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let mut path = std::env::temp_dir();
        path.push(format!(
            "ridl-test-{label}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&path).expect("create the temp dir");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn write(&self, relative: &str, text: &str) -> PathBuf {
        let target = self.0.join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).expect("create the parent directory");
        }
        std::fs::write(&target, text).expect("write the fixture");
        target
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Runs `ridl` with `args`, returning `(exit_code, stdout, stderr)`.
fn ridl(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
        .args(args)
        .output()
        .expect("the ridl binary must run");
    (
        output.status.code().expect("the process exits with a code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

const MANIFEST: &str = "[package]\nname = \"app\"\nversion = \"1.0.0\"\n";

/// The fixture the report assertions read.
///
/// Every clause is here for one reason:
///
/// - `setRange` — a satisfiable precondition over two generatable parameters;
/// - `overshoot` — `require desired > 300.0` over `Speed [0.0..250.0]`, which no
///   sampled input can satisfy: the "suspect" finding;
/// - `guard` — reads the interface's own signal, so it is skipped;
/// - `average` — a satisfiable integer precondition plus an `ensure`, which is
///   listed as an observer stub and never evaluated.
const SOURCE: &str = "\
package app

type Speed : km/h [0.0..250.0 step 0.5]
type Count : integer [0..1000]

const MAX_SPEED : Speed = 200.0

interface Cruise {
  signal currentSpeed : Speed @10ms
  command setRange(min: Speed, max: Speed) [
    require min < max
  ]
  command overshoot(desired: Speed) [
    require desired > 300.0
  ]
  command guard(desired: Speed) [
    require desired != 0.0 || currentSpeed == 0.0
  ]
  query average(window: Count): Speed [
    require window > 0
    ensure result >= 0.0
  ]
}
";

fn fixture(label: &str) -> TempDir {
    let dir = TempDir::new(label);
    dir.write("ridl.toml", MANIFEST);
    dir.write("app.ridl", SOURCE);
    dir
}

// ==========================================================================
// The three report sections
// ==========================================================================

#[test]
fn the_report_carries_the_three_sections() {
    let dir = fixture("sections");
    let (code, stdout, stderr) = ridl(&["test", dir.path().to_str().expect("utf-8 path")]);
    assert_eq!(code, 0, "a clean workspace exits 0; stderr: {stderr}");

    // 1. Range self-corpora — every constrained named type is reported.
    assert!(stdout.contains("ranges"), "{stdout}");
    assert!(stdout.contains("Speed"), "{stdout}");
    assert!(stdout.contains("Count"), "{stdout}");

    // 2. Require satisfiability sampling.
    assert!(stdout.contains("requires"), "{stdout}");
    assert!(
        stdout.contains("Cruise.setRange.require[0]"),
        "the observer id names the clause: {stdout}"
    );

    // 3. `ensure` clauses, listed as observer stubs only.
    assert!(stdout.contains("ensures"), "{stdout}");
    assert!(
        stdout.contains("Cruise.average.ensure[0]"),
        "the ensure is listed: {stdout}"
    );
    assert!(
        stdout.contains("observer stub"),
        "an ensure is listed, never evaluated: {stdout}"
    );
}

#[test]
fn a_signal_reading_require_is_skipped_with_the_live_state_reason() {
    let dir = fixture("skip");
    let (code, stdout, _) = ridl(&["test", dir.path().to_str().expect("utf-8 path")]);
    assert_eq!(code, 0);
    let line = stdout
        .lines()
        .find(|line| line.contains("Cruise.guard.require[0]"))
        .unwrap_or_else(|| panic!("the guard clause is reported: {stdout}"));
    assert!(
        line.contains("skipped: reads live state — observer territory (E5)"),
        "the skip names the reason: {line}"
    );
}

#[test]
fn a_precondition_no_sample_satisfies_is_reported_as_suspect() {
    let dir = fixture("suspect");
    let (code, stdout, _) = ridl(&["test", dir.path().to_str().expect("utf-8 path")]);
    // The finding lives in the test plane: it is reported, and it does not
    // fail the run and does not raise a compile diagnostic.
    assert_eq!(code, 0, "a suspect finding is not a failure: {stdout}");
    let line = stdout
        .lines()
        .find(|line| line.contains("Cruise.overshoot.require[0]"))
        .unwrap_or_else(|| panic!("the overshoot clause is reported: {stdout}"));
    assert!(
        line.contains("suspect: no sampled input satisfies this precondition"),
        "the suspect wording is the plan's: {line}"
    );
}

#[test]
fn the_suspect_finding_raises_no_compile_diagnostic() {
    // The same workspace checks clean: the finding is a test-plane report and
    // burns no diagnostic code.
    let dir = fixture("no-diagnostic");
    let (code, _, stderr) = ridl(&["check", dir.path().to_str().expect("utf-8 path")]);
    assert_eq!(code, 0, "the workspace checks clean: {stderr}");
    assert!(
        !stderr.contains("RIDL-"),
        "no diagnostic is raised for an unsatisfiable precondition: {stderr}"
    );
}

#[test]
fn a_satisfiable_require_reports_a_satisfied_count() {
    let dir = fixture("satisfied");
    let (code, stdout, _) = ridl(&["test", dir.path().to_str().expect("utf-8 path")]);
    assert_eq!(code, 0);
    let line = stdout
        .lines()
        .find(|line| line.contains("Cruise.average.require[0]"))
        .unwrap_or_else(|| panic!("the average clause is reported: {stdout}"));
    assert!(
        line.contains("/256"),
        "the default sample count is 256: {line}"
    );
}

// ==========================================================================
// Exit codes
// ==========================================================================

#[test]
fn a_clean_workspace_exits_zero() {
    let dir = fixture("exit-zero");
    let (code, _, stderr) = ridl(&["test", dir.path().to_str().expect("utf-8 path")]);
    assert_eq!(code, 0, "stderr: {stderr}");
}

#[test]
fn an_evaluation_error_exits_one() {
    let dir = TempDir::new("exit-one");
    dir.write("ridl.toml", MANIFEST);
    // `d - d` is zero for every sampled `d`, so the fault is reached on every
    // run rather than on a lucky draw — the report must be deterministic.
    dir.write(
        "app.ridl",
        "package app\n\
type Divisor : integer [0..10]\n\
interface I {\n  command c(d: Divisor) [\n    require 100 / (d - d) > 1\n  ]\n}\n",
    );
    let (code, stdout, _) = ridl(&["test", dir.path().to_str().expect("utf-8 path")]);
    assert_eq!(code, 1, "an evaluation error fails the run: {stdout}");
    assert!(
        stdout.contains("division by zero"),
        "the fault is named: {stdout}"
    );
}

#[test]
fn a_compile_error_exits_two() {
    let dir = TempDir::new("exit-two-compile");
    dir.write("ridl.toml", MANIFEST);
    dir.write("app.ridl", "package app\ntype Broken : km/h [10.0..0.0]\n");
    let (code, _, stderr) = ridl(&["test", dir.path().to_str().expect("utf-8 path")]);
    assert_eq!(
        code, 2,
        "a workspace that does not compile exits 2: {stderr}"
    );
}

#[test]
fn a_missing_path_exits_two() {
    let (code, _, _) = ridl(&["test", "/nonexistent/ridl/workspace/for/testing"]);
    assert_eq!(code, 2, "an unreadable input exits 2");
}

// ==========================================================================
// Determinism, sample count, and the JSON rendering
// ==========================================================================

#[test]
fn two_runs_of_one_workspace_agree() {
    // Seeding is derived from the contract's identity and text, so the run is
    // reproducible rather than merely repeatable within one process.
    let dir = fixture("determinism");
    let path = dir.path().to_str().expect("utf-8 path");
    let (first_code, first, _) = ridl(&["test", path, "--format", "json"]);
    let (second_code, second, _) = ridl(&["test", path, "--format", "json"]);
    assert_eq!(first_code, second_code);
    assert_eq!(first, second, "two runs produce the same report");
}

#[test]
fn the_sample_count_is_configurable() {
    let dir = fixture("samples");
    let (code, stdout, _) = ridl(&[
        "test",
        dir.path().to_str().expect("utf-8 path"),
        "--samples",
        "16",
        "--format",
        "json",
    ]);
    assert_eq!(code, 0);
    let report: serde_json::Value = serde_json::from_str(&stdout).expect("the report is JSON");
    let sampled = report[0]["contracts"]
        .as_array()
        .expect("contracts is an array")
        .iter()
        .find(|contract| contract["id"] == "Cruise.average.require[0]")
        .expect("the average clause is reported")
        .clone();
    assert_eq!(sampled["samples"], 16);
}

#[test]
fn the_json_report_carries_the_documented_shape() {
    let dir = fixture("json");
    let (code, stdout, _) = ridl(&[
        "test",
        dir.path().to_str().expect("utf-8 path"),
        "--format",
        "json",
    ]);
    assert_eq!(code, 0);
    let report: serde_json::Value = serde_json::from_str(&stdout).expect("the report is JSON");
    let package = &report[0];
    assert_eq!(package["package"], "app");

    // ranges: [{ type, status }]
    let ranges = package["ranges"].as_array().expect("ranges is an array");
    let speed = ranges
        .iter()
        .find(|range| range["type"] == "Speed")
        .expect("Speed is reported");
    assert_eq!(speed["status"], "ok");

    // contracts: [{ id, status, satisfied, samples }]
    let contracts = package["contracts"]
        .as_array()
        .expect("contracts is an array");
    let status_of = |id: &str| -> String {
        contracts
            .iter()
            .find(|contract| contract["id"] == id)
            .unwrap_or_else(|| panic!("`{id}` is reported"))["status"]
            .as_str()
            .expect("status is a string")
            .to_string()
    };
    assert_eq!(status_of("Cruise.overshoot.require[0]"), "suspect");
    assert_eq!(status_of("Cruise.guard.require[0]"), "skipped");
    assert_eq!(status_of("Cruise.average.ensure[0]"), "observer-stub");
    assert_eq!(status_of("Cruise.average.require[0]"), "ok");
}

// ==========================================================================
// The local gate (plan decision 11): the shipped corpus runs green
// ==========================================================================

#[test]
fn the_shipped_corpus_workspace_runs_green() {
    // `cargo test --workspace` carries the property runs by driving `ridl test`
    // over the reviewed corpus, so a range whose generators and checker
    // disagree fails the build.
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ridlc/tests/corpus/veh-common")
        .canonicalize()
        .expect("the corpus workspace exists");
    let (code, stdout, stderr) = ridl(&["test", corpus.to_str().expect("utf-8 path")]);
    assert_eq!(code, 0, "the corpus runs green\nstdout: {stdout}\n{stderr}");
    assert!(
        stdout.contains("ranges"),
        "the corpus exercises the range self-corpora: {stdout}"
    );
}
