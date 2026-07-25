//! `ridl test` — the property runner over a workspace (E2.11a).
//!
//! Drives the binary the way a user does and pins the three report sections
//! (range self-corpora, `require` satisfiability sampling, `ensure` observer
//! stubs), the exit contract (0 clean, 1 a self-corpus failure or an evaluation
//! error, 2 a compile or I/O error), the JSON rendering, and the determinism
//! that seeding per contract buys.

use std::collections::BTreeMap;
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
    // `Spans` is deliberately WIDE. On a narrow range such as `[-3..3]` a
    // uniform draw hits zero roughly 35 times in 256, so the second assertion
    // would hold whether or not zero was injected and the test could not fail.
    // Across two million values a random hit is vanishingly unlikely, so only
    // the injected value can satisfy the clause.
    dir.write(
        "app.ridl",
        "package app\n\
type Pos : integer [5..9]\n\
type Spans : integer [-1000000..1000000]\n\
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

#[test]
fn a_draw_outside_its_own_range_is_discarded_rather_than_evaluated() {
    // The float sampler rounds both bounds to `f64` to build its strategy, so
    // on a range whose bounds need more than about fifteen significant digits
    // the reconstructed value can land outside the declared interval. Feeding
    // such a value to the evaluator inverts verdicts: `require v < min`, which
    // no legal value satisfies, was reported `ok` before the guard. The
    // discarded count is asserted as well as the verdict, so removing the guard
    // fails here rather than merely changing a number nobody checks.
    let dir = TempDir::new("discard");
    dir.write("ridl.toml", MANIFEST);
    dir.write(
        "app.ridl",
        "package app\n\
type Fine : float [1.0000000000000001..1.0000000000000009 step 0.0000000000000001]\n\
interface I {\n\
  command below(v: Fine) [ require v < 1.0000000000000001 ]\n\
}\n",
    );
    let (code, stdout, _) = ridl(&[
        "test",
        dir.path().to_str().expect("utf-8 path"),
        "--format",
        "json",
    ]);
    assert_eq!(code, 0, "{stdout}");
    let report: serde_json::Value = serde_json::from_str(&stdout).expect("the report is JSON");
    let clause = report[0]["contracts"]
        .as_array()
        .expect("contracts is an array")
        .iter()
        .find(|contract| contract["id"] == "I.below.require[0]")
        .expect("the clause is reported")
        .clone();

    assert_eq!(
        clause["status"], "suspect",
        "nothing in `Fine` is below its own minimum, so no input can satisfy \
         this clause: {clause}"
    );
    assert!(
        clause["discarded_samples"]
            .as_u64()
            .expect("a discarded count")
            > 0,
        "the out-of-range draws are discarded and counted, not evaluated: {clause}"
    );
    assert_eq!(
        clause["satisfied"], 0,
        "an out-of-range value must never be counted as satisfying: {clause}"
    );
}

#[test]
fn a_multi_parameter_suspect_carries_the_combination_caveat() {
    // Boundary values are zipped, not combined, so the corpus is the diagonal
    // and `a == 0 && b == 1000` is never tried even though both values are in
    // their parameters' corpora. The finding must say so, or it reads as a
    // claim about the model rather than a limit of the search.
    let dir = TempDir::new("combination");
    dir.write("ridl.toml", MANIFEST);
    dir.write(
        "app.ridl",
        "package app\n\
type Count : integer [0..1000]\n\
interface I {\n\
  command corner(a: Count, b: Count) [ require a == 0 && b == 1000 ]\n\
  command single(a: Count) [ require a > 2000 ]\n\
}\n",
    );
    let (code, stdout, _) = ridl(&["test", dir.path().to_str().expect("utf-8 path")]);
    assert_eq!(code, 0, "{stdout}");

    let corner = stdout
        .lines()
        .find(|line| line.contains("I.corner.require[0]"))
        .unwrap_or_else(|| panic!("the clause is reported: {stdout}"));
    assert!(
        corner.contains("boundary combinations across parameters are not explored"),
        "a multi-parameter finding explains what was not tried: {corner}"
    );

    // A single-parameter clause has no combinations to miss, so the caveat
    // would be noise — its absence is what keeps the caveat meaningful.
    let single = stdout
        .lines()
        .find(|line| line.contains("I.single.require[0]"))
        .unwrap_or_else(|| panic!("the clause is reported: {stdout}"));
    assert!(
        single.contains("suspect") && !single.contains("boundary combinations"),
        "a one-parameter finding carries no combination caveat: {single}"
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
    // — or a CI job checking only the exit code — takes for a clean pass.
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
fn the_summary_counts_constant_false_clauses_as_findings() {
    // A `suspect` is counted in the summary; a constant-false clause is the
    // same news reached by a different route — every input, all one of them,
    // failed the precondition. Left uncounted, a package whose every clause is
    // unsatisfiable printed "2 total, 2 evaluated" and read like a pass.
    let dir = TempDir::new("constant-false");
    dir.write("ridl.toml", MANIFEST);
    dir.write(
        "app.ridl",
        "package app\n\
type Speed : km/h [0.0..250.0 step 0.5]\n\
const MAX_SPEED : Speed = 200.0\n\
interface I {\n\
  command a(s: Speed) [ require MAX_SPEED < 0.0 ]\n\
  command b(s: Speed) [ require MAX_SPEED > 300.0 ]\n\
}\n",
    );
    let (code, stdout, _) = ridl(&["test", dir.path().to_str().expect("utf-8 path")]);
    assert_eq!(code, 0, "{stdout}");
    let line = stdout
        .lines()
        .find(|line| line.contains("summary —"))
        .unwrap_or_else(|| panic!("the run is summarized: {stdout}"));
    assert!(
        line.contains("2 constant-false"),
        "both clauses are unsatisfiable and the summary must say so rather than \
         reporting only that they were evaluated: {line}"
    );

    let (_, json, _) = ridl(&[
        "test",
        dir.path().to_str().expect("utf-8 path"),
        "--format",
        "json",
    ]);
    let report: serde_json::Value = serde_json::from_str(&json).expect("the report is JSON");
    assert_eq!(report[0]["summary"]["requires_constant_false"], 2);
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
// Name resolution across a workspace
//
// Every fixture above this line is ONE package: no member list, no import, no
// alias, no `ridl.std` type in a sampled position, and every constant and enum
// a clause reads declared in the same file. The runner resolved names against
// that shape — one package's `decls` — and the whole suite stayed green on a
// layout no shipped corpus entry uses. The fixtures below are the layout the
// corpus does use (`crates/ridlc/tests/corpus/services-workspace` and
// `veh-cluster`): several members, one direction of dependency, and a real
// name collision that makes an alias required rather than needless.
// ==========================================================================

/// A three-member workspace: a types member, a retired-generation member that
/// collides with it on every exported name, and an interface member that
/// imports from both — the second under an alias, because the collision makes
/// the alias necessary (the `services-workspace` corpus entry's shape).
///
/// Every colliding pair is deliberately given values that put a clause on
/// OPPOSITE sides of its verdict, so binding the wrong one of the two flips a
/// reported status rather than changing a number nobody reads:
///
/// - `veh.common.Level` is `[0..7]`, `veh.legacy.Level` is `[1000..2000]`;
/// - `veh.common.LIMIT` is 5, `veh.legacy.LIMIT` is 1500;
/// - `veh.common.GearPosition` has no `REVERSE`, so naming one under the wrong
///   binding is an unbound reference and not a wrong answer.
fn cross_package(label: &str) -> TempDir {
    let dir = TempDir::new(label);
    dir.write(
        "ridl.toml",
        "[workspace]\nmembers = [\"common\", \"legacy\", \"cluster\"]\n",
    );

    dir.write(
        "common/ridl.toml",
        "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n",
    );
    dir.write(
        "common/common.typl",
        "package veh.common\n\
type Speed : km/h [0.0..250.0 step 0.5]\n\
type Level : integer [0..7]\n\
type Gain : float [0.0..10.0 step 0.5]\n\
const LIMIT : Level = 5\n\
const TWO : Gain = 2.0\n\
enum GearPosition {\n  PARK = 0\n  DRIVE = 1\n}\n",
    );

    dir.write(
        "legacy/ridl.toml",
        "[package]\nname = \"veh.legacy\"\nversion = \"1.0.0\"\n",
    );
    dir.write(
        "legacy/legacy.typl",
        "package veh.legacy\n\
type Level : integer [1000..2000]\n\
const LIMIT : Level = 1500\n\
enum GearPosition {\n  PARK = 0\n  REVERSE = 1\n}\n",
    );

    dir.write(
        "cluster/ridl.toml",
        "[package]\nname = \"veh.cluster\"\nversion = \"1.0.0\"\n",
    );
    dir.write(
        "cluster/cluster.ridl",
        "package veh.cluster\n\
import veh.common.Speed\n\
import veh.common.Level\n\
import veh.common.LIMIT\n\
import veh.common.TWO\n\
import veh.common.GearPosition\n\
import veh.legacy.Level as LegacyLevel\n\
import veh.legacy.LIMIT as LEGACY_LIMIT\n\
import veh.legacy.GearPosition as LegacyGear\n\
interface Cruise {\n\
  command setSpeed(s: Speed) [ require s > 0.0 ]\n\
  command own(l: Level) [ require l > LIMIT ]\n\
  command aliased(l: LegacyLevel) [ require l < LEGACY_LIMIT ]\n\
  command ownBounds(l: Level) [ require l > 7 ]\n\
  command legacyBounds(l: LegacyLevel) [ require l > 7 ]\n\
  command ratio(l: Level) [ require 5 / TWO > 2 ]\n\
  command gear(l: Level) [ require GearPosition.PARK != GearPosition.DRIVE ]\n\
  command legacyGear(l: Level) [ require LegacyGear.PARK != LegacyGear.REVERSE ]\n\
  command stamp(t: Timestamp) [ require t > 0 ]\n\
  query avg(window: Duration): Speed [ require window == 1ms ]\n\
}\n",
    );
    dir
}

/// The `veh.cluster` clause statuses of [`cross_package`], keyed by observer id.
fn cross_package_statuses(label: &str) -> (i32, String, BTreeMap<String, String>) {
    let dir = cross_package(label);
    let (code, stdout, stderr) = ridl(&[
        "test",
        dir.path().to_str().expect("utf-8 path"),
        "--format",
        "json",
    ]);
    assert_eq!(code, 0, "the workspace runs clean; stderr: {stderr}");
    let report: serde_json::Value = serde_json::from_str(&stdout).expect("the report is JSON");
    let cluster = report
        .as_array()
        .expect("the report is an array")
        .iter()
        .find(|package| package["package"] == "veh.cluster")
        .unwrap_or_else(|| panic!("the interface member is reported: {stdout}"))
        .clone();
    let statuses = cluster["contracts"]
        .as_array()
        .expect("contracts is an array")
        .iter()
        .map(|contract| {
            (
                contract["id"].as_str().expect("an id").to_string(),
                contract["status"].as_str().expect("a status").to_string(),
            )
        })
        .collect();
    (code, stdout, statuses)
}

#[test]
fn a_parameter_typed_from_another_package_is_sampled_rather_than_skipped() {
    // The headline defect. `Speed` is declared in the sibling member, so the
    // runner resolved nothing for it and reported "has no generatable range" —
    // on the layout every shipped corpus entry uses, which meant essentially
    // every clause was skipped while the command exited 0.
    let (_, stdout, statuses) = cross_package_statuses("cross-param");
    assert_eq!(
        statuses
            .get("Cruise.setSpeed.require[0]")
            .map(String::as_str),
        Some("ok"),
        "a parameter typed from another package is drawn: {stdout}"
    );
    assert!(
        !stdout.contains("has no generatable range"),
        "no clause of this workspace is skipped for an unresolvable type: {stdout}"
    );

    // And the same in the report a human reads, not only in the JSON field.
    let dir = cross_package("cross-param-text");
    let (code, text, _) = ridl(&["test", dir.path().to_str().expect("utf-8 path")]);
    assert_eq!(code, 0, "{text}");
    let line = text
        .lines()
        .find(|line| line.contains("Cruise.setSpeed.require[0]"))
        .unwrap_or_else(|| panic!("the clause is reported: {text}"));
    assert!(
        line.contains("boundary +") && line.contains("random of"),
        "the rendered line reports a real sample count: {line}"
    );
    assert!(
        !text.contains("WARNING"),
        "this run tested its preconditions, so the alarm stays silent: {text}"
    );
}

#[test]
fn a_clause_reading_an_imported_constant_evaluates_instead_of_failing_the_run() {
    // The second defect, and the sharper one: an imported constant was unbound,
    // so evaluation raised `\`LIMIT\` is not bound in this environment` and the
    // command exited 1 — on a workspace `ridl check` accepts. Exit 1 means the
    // toolchain disagrees with itself, so the exit code is asserted first.
    let dir = cross_package("cross-const");
    let path = dir.path().to_str().expect("utf-8 path");
    let (check, _, check_err) = ridl(&["check", path]);
    assert_eq!(check, 0, "the workspace is legal: {check_err}");

    let (code, stdout, _) = ridl(&["test", path]);
    assert_eq!(
        code, 0,
        "a legal workspace must not make the property runner fail: {stdout}"
    );
    assert!(
        !stdout.contains("is not bound in this environment"),
        "every name the checker resolved is bound here too: {stdout}"
    );
    let line = stdout
        .lines()
        .find(|line| line.contains("Cruise.own.require[0]"))
        .unwrap_or_else(|| panic!("the clause is reported: {stdout}"));
    assert!(
        line.contains("ok —"),
        "`l > LIMIT` is satisfiable over `[0..7]` with `LIMIT` = 5: {line}"
    );
}

#[test]
fn an_import_alias_binds_the_declaration_it_names_not_the_colliding_one() {
    // `LIMIT` is exported by both members. The importing package binds
    // `veh.common.LIMIT` as `LIMIT` and `veh.legacy.LIMIT` as `LEGACY_LIMIT`,
    // and the two hold 5 and 1500. Indexing constants by their declared name
    // across packages — the shortcut this fix deliberately does not take —
    // cannot tell them apart, and each clause is written so that binding the
    // other one flips its verdict rather than changing a count.
    let (_, stdout, statuses) = cross_package_statuses("alias-const");

    assert_eq!(
        statuses.get("Cruise.own.require[0]").map(String::as_str),
        Some("ok"),
        "`l > LIMIT` over `[0..7]` is satisfiable at 5 and unsatisfiable at \
         1500, so `ok` is evidence the local name bound `veh.common.LIMIT`: \
         {stdout}"
    );
    assert_eq!(
        statuses
            .get("Cruise.aliased.require[0]")
            .map(String::as_str),
        Some("ok"),
        "`l < LEGACY_LIMIT` over `[1000..2000]` is satisfiable at 1500 and \
         unsatisfiable at 5, so `ok` is evidence the alias bound \
         `veh.legacy.LIMIT`: {stdout}"
    );
}

#[test]
fn a_parameter_is_sampled_against_the_bounds_of_its_own_packages_type() {
    // The same collision one layer down: both members export `Level`, with
    // disjoint ranges. `require l > 7` is unsatisfiable over `veh.common`'s
    // `[0..7]` and satisfied by every value of `veh.legacy`'s `[1000..2000]`,
    // so resolving the wrong `Level` inverts both verdicts at once.
    let (_, stdout, statuses) = cross_package_statuses("alias-type");
    assert_eq!(
        statuses
            .get("Cruise.ownBounds.require[0]")
            .map(String::as_str),
        Some("suspect"),
        "nothing in `veh.common.Level [0..7]` exceeds 7: {stdout}"
    );
    assert_eq!(
        statuses
            .get("Cruise.legacyBounds.require[0]")
            .map(String::as_str),
        Some("ok"),
        "every value of `veh.legacy.Level [1000..2000]` exceeds 7: {stdout}"
    );
}

#[test]
fn an_imported_constants_type_is_read_in_the_package_that_declares_it() {
    // A constant's lowered `type_ref` is canonical in ITS OWN package's view:
    // `TWO` lowers with `type_ref = "Gain"`, and `veh.cluster` imports `TWO`
    // without importing `Gain`. Resolving that reference in the importing
    // package finds nothing and falls back to the value's spelling — and the
    // IR normalizes `2.0` to `"2"`, so the fallback reads integer.
    //
    // `5 / TWO > 2` is exactly where that matters: float-backed operands divide
    // exactly (2.5 > 2, true), integer-backed operands truncate (2 > 2, false).
    // The clause reads no parameter, so the two verdicts are `constant-true`
    // and `constant-false` — a status flip, not a count.
    let (_, stdout, statuses) = cross_package_statuses("const-backing");
    assert_eq!(
        statuses.get("Cruise.ratio.require[0]").map(String::as_str),
        Some("constant-true"),
        "`TWO` is float-backed through `veh.common.Gain`, so `5 / TWO` is 2.5 \
         and not 2: {stdout}"
    );
}

#[test]
fn enum_members_bind_under_the_local_name_including_an_alias() {
    // The evaluator asks the environment for the dotted spelling the clause
    // WRITES, so members must be keyed by the local name — `LegacyGear.PARK`,
    // not `GearPosition.PARK`. Keying by the declared name leaves the aliased
    // clause naming an unbound reference, which is an evaluation error and
    // exit 1.
    //
    // `REVERSE` exists only in `veh.legacy.GearPosition`, so the aliased clause
    // also fails if the alias resolves to the colliding `veh.common` enum.
    let (_, stdout, statuses) = cross_package_statuses("alias-enum");
    assert_eq!(
        statuses.get("Cruise.gear.require[0]").map(String::as_str),
        Some("constant-true"),
        "a plainly imported enum's members bind: {stdout}"
    );
    assert_eq!(
        statuses
            .get("Cruise.legacyGear.require[0]")
            .map(String::as_str),
        Some("constant-true"),
        "an aliased enum's members bind under the alias: {stdout}"
    );
}

#[test]
fn a_ridl_std_parameter_type_is_sampled() {
    // `ridl.std` is deliberately absent from the workspace's package list and
    // threaded through the passes as a parameter, so it is not among the
    // checked packages a consumer receives — while every package implicitly
    // imports all of it. `Timestamp` is `integer [0..9223372036854775807]`:
    // an ordinary generatable range that resolved to nothing.
    let (_, stdout, statuses) = cross_package_statuses("std-type");
    assert_eq!(
        statuses.get("Cruise.stamp.require[0]").map(String::as_str),
        Some("ok"),
        "a `ridl.std.Timestamp` parameter is drawn: {stdout}"
    );
}

#[test]
fn a_duration_parameter_is_drawn_in_the_duration_domain() {
    // `ridl.std.Duration` is the one inhabitant of the duration domain
    // (expr-core §5.1) and the evaluator refuses to order a duration against a
    // number, so making `ridl.std` reachable without honouring that would turn
    // `require window > 0ms` — a clause the checker accepts — into an
    // evaluation error and exit 1.
    //
    // The clause is `window == 1ms` rather than `> 0ms` because it also pins
    // the SCALE: `Duration` is declared in milliseconds and `Value::Dur` is an
    // exact microsecond count. The boundary corpus of `[0..i64::MAX]` contains
    // 1, which is 1ms only after the thousandfold scale; unscaled it is one
    // microsecond, and a random draw hitting exactly 1000 out of 9.2e18 values
    // never happens, so the clause reports `suspect` instead.
    let (_, stdout, statuses) = cross_package_statuses("std-duration");
    assert_eq!(
        statuses.get("Cruise.avg.require[0]").map(String::as_str),
        Some("ok"),
        "the boundary value 1 is one millisecond, so `window == 1ms` is \
         satisfied: {stdout}"
    );
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

#[test]
fn the_shipped_multi_member_corpus_evaluates_its_cross_package_clauses() {
    // `veh-common` is one package with no interface, so the entry above cannot
    // see a resolution defect at all. These two entries are the reviewed
    // multi-member workspaces — `veh-cluster` is the ridl reference Appendix A
    // over a `veh.common` types member, `services-workspace` is three members
    // with an inline shape that owns none of its vocabulary — and every clause
    // named here was reported `skipped: … has no generatable range` before the
    // runner resolved names the way the checker does.
    //
    // Naming the clauses rather than counting them is deliberate: a count moves
    // whenever the corpus grows, while a clause that stops being evaluated is
    // the regression this guards.
    for (entry, evaluated) in [
        (
            "veh-cluster",
            vec![
                // A `ridl.std.Duration` parameter, from Appendix A verbatim.
                "VehicleStatus.getAverageSpeed.require[0]",
                // A `veh.common.Temperature` parameter inside an inline shape.
                "veh.hvac.cabin.setTarget.require[0]",
            ],
        ),
        (
            "services-workspace",
            // A `fleet.contracts.DoorId` parameter inside an inline shape in a
            // package that owns none of its vocabulary.
            vec!["fleet.vehicle.mirrors.readFold.require[0]"],
        ),
    ] {
        let corpus = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../ridlc/tests/corpus")
            .join(entry)
            .canonicalize()
            .unwrap_or_else(|err| panic!("the `{entry}` corpus workspace exists: {err}"));
        let (code, stdout, stderr) = ridl(&[
            "test",
            corpus.to_str().expect("utf-8 path"),
            "--format",
            "json",
        ]);
        assert_eq!(code, 0, "`{entry}` runs green\nstdout: {stdout}\n{stderr}");

        let report: serde_json::Value = serde_json::from_str(&stdout).expect("the report is JSON");
        let contracts: Vec<&serde_json::Value> = report
            .as_array()
            .expect("the report is an array")
            .iter()
            .flat_map(|package| {
                package["contracts"]
                    .as_array()
                    .expect("contracts is an array")
            })
            .collect();
        for id in evaluated {
            let clause = contracts
                .iter()
                .find(|contract| contract["id"] == id)
                .unwrap_or_else(|| panic!("`{id}` is reported in `{entry}`: {stdout}"));
            assert_eq!(
                clause["status"], "ok",
                "`{id}` reads a parameter this runner can draw, so it is \
                 evaluated rather than skipped: {clause}"
            );
        }

        // Nothing anywhere in either entry may report an evaluation error: an
        // error is exit 1, which claims the toolchain disagrees with itself.
        for contract in &contracts {
            assert_ne!(
                contract["status"], "error",
                "a reviewed corpus entry must not fault the evaluator: {contract}"
            );
        }
    }
}
