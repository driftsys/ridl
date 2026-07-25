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
        line.contains("boundary +") && line.contains("random of"),
        "the split is reported so a reader can tell an endpoint hit from an \
         interior one: {line}"
    );
}

#[test]
fn the_boundary_corpus_is_sampled_so_endpoint_clauses_are_not_called_suspect() {
    // A uniform draw reaches an endpoint far too rarely to be relied on: 256
    // draws over `[0..1000]` hit either end only about a fifth of the time, so
    // these perfectly satisfiable clauses were reported as unsatisfiable before
    // the boundary corpus was injected. Each one turns on a declared endpoint.
    let dir = TempDir::new("boundary");
    dir.write("ridl.toml", MANIFEST);
    dir.write(
        "app.ridl",
        "package app\n\
type Speed : km/h [0.0..250.0 step 0.5]\n\
type Count : integer [0..1000]\n\
interface I {\n\
  command atMax(c: Count) [ require c == 1000 ]\n\
  command atMin(s: Speed) [ require s == 0.0 ]\n\
  command atTop(s: Speed) [ require s == 250.0 ]\n\
  command nearTop(s: Speed) [ require s > 249.0 ]\n\
}\n",
    );
    let (code, stdout, _) = ridl(&["test", dir.path().to_str().expect("utf-8 path")]);
    assert_eq!(code, 0, "{stdout}");
    for id in [
        "I.atMax.require[0]",
        "I.atMin.require[0]",
        "I.atTop.require[0]",
        "I.nearTop.require[0]",
    ] {
        let line = stdout
            .lines()
            .find(|line| line.contains(id))
            .unwrap_or_else(|| panic!("`{id}` is reported: {stdout}"));
        assert!(
            !line.contains("suspect"),
            "`{id}` is satisfiable at its own declared endpoint and must not be \
             called suspect: {line}"
        );
    }
}

#[test]
fn zero_is_injected_only_when_the_range_actually_contains_it() {
    // The boundary corpus adds zero because contracts so often turn on it, but
    // only when the range spans it. Injecting it unconditionally would feed a
    // parameter a value its own type forbids, and the verdict it produces is a
    // FALSE `ok`: `require p == 0` over `Pos [5..9]` — which no legal value
    // satisfies — would be reported satisfied. A wrong green is worse than a
    // wrong red, so the guard is pinned.
    let dir = TempDir::new("zero-guard");
    dir.write("ridl.toml", MANIFEST);
    dir.write(
        "app.ridl",
        "package app\n\
type Pos : integer [5..9]\n\
type Spans : integer [-3..3]\n\
interface I {\n\
  command outside(p: Pos) [ require p == 0 ]\n\
  command inside(s: Spans) [ require s == 0 ]\n\
}\n",
    );
    let (code, stdout, _) = ridl(&["test", dir.path().to_str().expect("utf-8 path")]);
    assert_eq!(code, 0, "{stdout}");

    let outside = stdout
        .lines()
        .find(|line| line.contains("I.outside.require[0]"))
        .unwrap_or_else(|| panic!("the clause is reported: {stdout}"));
    assert!(
        outside.contains("suspect"),
        "zero is outside `Pos [5..9]`, so nothing can satisfy `p == 0` and the \
         run must not claim otherwise: {outside}"
    );

    // The other side of the guard: a range that does span zero still gets it,
    // so the test fails if zero injection is dropped altogether rather than
    // merely made unconditional.
    let inside = stdout
        .lines()
        .find(|line| line.contains("I.inside.require[0]"))
        .unwrap_or_else(|| panic!("the clause is reported: {stdout}"));
    assert!(
        inside.contains("ok") && !inside.contains("suspect"),
        "zero is inside `Spans [-3..3]`, so `s == 0` is satisfiable: {inside}"
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

// ==========================================================================
// The run summary — "tested nothing" must not look like "all good"
// ==========================================================================

#[test]
fn the_summary_counts_what_the_run_actually_did() {
    // The fixture has four requires (one satisfiable, one suspect, one skipped
    // for reading a signal, one satisfiable) and one ensure.
    let dir = fixture("summary");
    let (code, stdout, _) = ridl(&["test", dir.path().to_str().expect("utf-8 path")]);
    assert_eq!(code, 0);
    let line = stdout
        .lines()
        .find(|line| line.contains("summary —"))
        .unwrap_or_else(|| panic!("the run is summarized: {stdout}"));
    assert!(line.contains("requires: 4 total"), "{line}");
    assert!(line.contains("3 evaluated"), "{line}");
    assert!(line.contains("1 suspect"), "{line}");
    assert!(line.contains("1 skipped"), "{line}");
    assert!(line.contains("ensures: 1 listed"), "{line}");
    // Something ran, so the alarm stays silent.
    assert!(
        !stdout.contains("WARNING"),
        "three of four requires were evaluated: {stdout}"
    );
}

#[test]
fn a_run_that_evaluated_nothing_says_so_and_cannot_read_as_success() {
    // THE case this summary exists for. Every `require` here reads the
    // interface's own signals, so all of them are skipped and the command still
    // exits 0. Without the summary the output is a list of skips that a reader
    // — or a CI job checking only the exit code — takes for a clean pass. On
    // the workspace layout most models use, imported parameter types produce
    // exactly this shape.
    let dir = TempDir::new("all-skipped");
    dir.write("ridl.toml", MANIFEST);
    dir.write(
        "app.ridl",
        "package app\n\
type Speed : km/h [0.0..250.0 step 0.5]\n\
interface I {\n\
  signal speedNow : Speed @10ms\n\
  command a(s: Speed) [ require speedNow == 0.0 ]\n\
  command b(s: Speed) [ require speedNow > 1.0 ]\n\
}\n",
    );
    let (code, stdout, _) = ridl(&["test", dir.path().to_str().expect("utf-8 path")]);
    assert_eq!(
        code, 0,
        "the exit code alone still reads as success: {stdout}"
    );

    let line = stdout
        .lines()
        .find(|line| line.contains("summary —"))
        .unwrap_or_else(|| panic!("the run is summarized: {stdout}"));
    assert!(line.contains("requires: 2 total, 0 evaluated"), "{line}");
    assert!(line.contains("2 skipped"), "{line}");

    let warning = stdout
        .lines()
        .find(|line| line.contains("WARNING"))
        .unwrap_or_else(|| {
            panic!("a run that evaluated nothing must say so unmistakably: {stdout}")
        });
    assert!(
        warning.contains("no require clause was evaluated")
            && warning.contains("tested no precondition"),
        "the warning is unambiguous about what did not happen: {warning}"
    );
}

#[test]
fn the_json_summary_lets_ci_tell_tested_nothing_from_all_good() {
    // A machine reading the report has the same problem a human does, so the
    // flag is in the JSON too.
    let dir = TempDir::new("all-skipped-json");
    dir.write("ridl.toml", MANIFEST);
    dir.write(
        "app.ridl",
        "package app\n\
type Speed : km/h [0.0..250.0 step 0.5]\n\
interface I {\n\
  signal speedNow : Speed @10ms\n\
  command a(s: Speed) [ require speedNow == 0.0 ]\n\
}\n",
    );
    let (code, stdout, _) = ridl(&[
        "test",
        dir.path().to_str().expect("utf-8 path"),
        "--format",
        "json",
    ]);
    assert_eq!(code, 0);
    let report: serde_json::Value = serde_json::from_str(&stdout).expect("the report is JSON");
    let summary = &report[0]["summary"];
    assert_eq!(summary["requires_total"], 1);
    assert_eq!(summary["requires_evaluated"], 0);
    assert_eq!(summary["requires_skipped"], 1);
    assert_eq!(
        summary["nothing_evaluated"], true,
        "the single field CI keys on: {summary}"
    );

    // And the contrasting case: a run that did evaluate its preconditions must
    // report the flag false, or the flag would be useless.
    let ok = fixture("json-summary-ok");
    let (code, stdout, _) = ridl(&[
        "test",
        ok.path().to_str().expect("utf-8 path"),
        "--format",
        "json",
    ]);
    assert_eq!(code, 0);
    let report: serde_json::Value = serde_json::from_str(&stdout).expect("the report is JSON");
    let summary = &report[0]["summary"];
    assert_eq!(summary["requires_total"], 4);
    assert_eq!(summary["requires_evaluated"], 3);
    assert_eq!(summary["requires_suspect"], 1);
    assert_eq!(summary["requires_skipped"], 1);
    assert_eq!(summary["ensures_listed"], 1);
    assert_eq!(summary["nothing_evaluated"], false);
}

#[test]
fn a_package_with_no_preconditions_is_not_warned_about() {
    // The warning means "you asked for preconditions to be tested and none
    // were", not "this package has no preconditions". A types-only package has
    // nothing it failed to test, so warning there would train readers to ignore
    // the line.
    let dir = TempDir::new("no-requires");
    dir.write("ridl.toml", MANIFEST);
    dir.write(
        "app.ridl",
        "package app\ntype Speed : km/h [0.0..250.0 step 0.5]\n",
    );
    let (code, stdout, _) = ridl(&["test", dir.path().to_str().expect("utf-8 path")]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("requires: 0 total, 0 evaluated"),
        "the summary is still printed: {stdout}"
    );
    assert!(
        !stdout.contains("WARNING"),
        "a package declaring no precondition failed to test nothing: {stdout}"
    );
}

#[test]
fn a_zero_sample_count_is_a_usage_error() {
    // Refused rather than silently clamped: a run that drew nothing would call
    // every sampled clause unsatisfiable.
    let dir = fixture("zero-samples");
    let (code, _, stderr) = ridl(&[
        "test",
        dir.path().to_str().expect("utf-8 path"),
        "--samples",
        "0",
    ]);
    assert_eq!(code, 2, "stderr: {stderr}");
    assert!(stderr.contains("--samples"), "the flag is named: {stderr}");
}

#[test]
fn a_clause_reading_no_parameter_is_evaluated_once() {
    // Reporting `256/256` for a clause with nothing to vary implies a search
    // that never happened.
    let dir = TempDir::new("constant");
    dir.write("ridl.toml", MANIFEST);
    dir.write(
        "app.ridl",
        "package app\n\
type Speed : km/h [0.0..250.0 step 0.5]\n\
const MAX_SPEED : Speed = 200.0\n\
interface I {\n  command c(s: Speed) [ require MAX_SPEED > 0.0 ]\n}\n",
    );
    let (code, stdout, _) = ridl(&["test", dir.path().to_str().expect("utf-8 path")]);
    assert_eq!(code, 0, "{stdout}");
    let line = stdout
        .lines()
        .find(|line| line.contains("I.c.require[0]"))
        .unwrap_or_else(|| panic!("the clause is reported: {stdout}"));
    assert!(
        line.contains("evaluated once") && !line.contains("/256"),
        "a constant clause does not claim a sample count: {line}"
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
    // `--samples` governs the RANDOM draws; the boundary corpus is injected on
    // top of them, so the total is larger.
    assert_eq!(sampled["random_samples"], 16);
    assert!(
        sampled["samples"].as_u64().expect("a sample total") > 16,
        "the boundary corpus is drawn as well: {sampled}"
    );
}

#[test]
fn contracts_inside_a_service_with_an_inline_shape_are_reported() {
    // A `service` declared with an inline shape carries a full interface body
    // that does NOT appear in `package.interfaces`, and the checker lowers its
    // clauses into real contracts with observer ids. Walking only
    // `package.interfaces` reported a clean run over contracts that were never
    // tested — a green report over untested contracts is the one outcome this
    // command must not produce.
    let dir = TempDir::new("inline-service");
    dir.write("ridl.toml", MANIFEST);
    dir.write(
        "app.ridl",
        "package app\n\
type Speed : km/h [0.0..250.0 step 0.5]\n\
service app.cruise {\n\
  command overshoot(desired: Speed) [ require desired > 300.0 ]\n\
  query peek(): Speed [ ensure result >= 0.0 ]\n\
}\n",
    );
    let (code, stdout, stderr) = ridl(&["test", dir.path().to_str().expect("utf-8 path")]);
    assert_eq!(code, 0, "stdout: {stdout}\nstderr: {stderr}");

    let require = stdout
        .lines()
        .find(|line| line.contains("overshoot") && line.contains("require"))
        .unwrap_or_else(|| {
            panic!("the inline service's require is reported, not silently dropped: {stdout}")
        });
    assert!(
        require.contains("suspect"),
        "an unsatisfiable clause inside an inline service is still flagged: {require}"
    );
    assert!(
        stdout.contains("peek") && stdout.contains("observer stub"),
        "the inline service's ensure is listed as an observer stub: {stdout}"
    );
}

#[test]
fn a_service_naming_an_interface_reports_its_clauses_once() {
    // The other arm of the shape oneof: the target already lives in
    // `package.interfaces`, so it must not be walked a second time.
    let dir = TempDir::new("service-ref");
    dir.write("ridl.toml", MANIFEST);
    dir.write(
        "app.ridl",
        "package app\n\
type Speed : km/h [0.0..250.0 step 0.5]\n\
interface Cruise {\n\
  command overshoot(desired: Speed) [ require desired > 300.0 ]\n\
}\n\
service app.cruise : Cruise\n",
    );
    let (code, stdout, stderr) = ridl(&["test", dir.path().to_str().expect("utf-8 path")]);
    assert_eq!(code, 0, "stdout: {stdout}\nstderr: {stderr}");
    let reported = stdout
        .lines()
        .filter(|line| line.contains("Cruise.overshoot.require[0]"))
        .count();
    assert_eq!(reported, 1, "the clause is reported exactly once: {stdout}");
}

#[test]
fn the_range_section_actually_runs_its_corpora() {
    // Teeth for section 1. Asserting only `status == "ok"` cannot tell a
    // section that ran and passed from one that ran nothing at all — replacing
    // the whole check with `Ok { boundary: 0, violations: 0 }` used to leave the
    // suite green. The corpus sizes are asserted, so an empty run fails here.
    let dir = fixture("range-teeth");
    let (code, stdout, _) = ridl(&[
        "test",
        dir.path().to_str().expect("utf-8 path"),
        "--format",
        "json",
    ]);
    assert_eq!(code, 0);
    let report: serde_json::Value = serde_json::from_str(&stdout).expect("the report is JSON");
    let ranges = report[0]["ranges"].as_array().expect("ranges is an array");
    let range = |name: &str| -> serde_json::Value {
        ranges
            .iter()
            .find(|range| range["type"] == name)
            .unwrap_or_else(|| panic!("`{name}` is reported: {stdout}"))
            .clone()
    };

    // `Count : integer [0..1000]` — min, min+1, max-1, max accepted; min-1 and
    // max+1 rejected.
    let count = range("Count");
    assert_eq!(count["boundary"], 4, "the integer boundary corpus ran");
    assert_eq!(count["violations"], 2, "the integer violation corpus ran");

    // `Speed : km/h [0.0..250.0 step 0.5]` — the float corpora are the same
    // shape, one step in and one step out on each side.
    let speed = range("Speed");
    assert_eq!(speed["boundary"], 4, "the float boundary corpus ran");
    assert_eq!(speed["violations"], 2, "the float violation corpus ran");
}

#[test]
fn a_range_whose_violations_are_accepted_fails_the_run() {
    // The failure path of section 1, driven through the one range shape that
    // can exhibit it: a single-value range `[0..0]`, whose boundary corpus is
    // `{0}` and whose violations are `{-1, 1}`. If the validator ever accepted
    // a violation, this is the run that would go red — which is what makes the
    // section a check rather than a formality.
    let dir = TempDir::new("range-failure");
    dir.write("ridl.toml", MANIFEST);
    dir.write("app.ridl", "package app\ntype Fixed : integer [0..0]\n");
    let (code, stdout, _) = ridl(&[
        "test",
        dir.path().to_str().expect("utf-8 path"),
        "--format",
        "json",
    ]);
    assert_eq!(code, 0, "a correct validator passes: {stdout}");
    let report: serde_json::Value = serde_json::from_str(&stdout).expect("the report is JSON");
    let fixed = report[0]["ranges"]
        .as_array()
        .expect("ranges is an array")
        .iter()
        .find(|range| range["type"] == "Fixed")
        .expect("Fixed is reported")
        .clone();
    assert_eq!(fixed["status"], "ok");
    assert_eq!(fixed["boundary"], 1, "the single in-range value");
    assert_eq!(fixed["violations"], 2, "one step out on each side");
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
