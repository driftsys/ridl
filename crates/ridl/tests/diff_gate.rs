//! The `ridl diff` gating contract (docs/ROADMAP.md epic E2.8b).
//!
//! These tests run the real binary over real source trees, so the local merge
//! gate exercises the CI gating contract itself (ADR-0008 decision 11): a
//! breaking change exits 1, a compatible one exits 0.
//!
//! They also carry the two cases that only exist end to end — the
//! `[defaults].timing` rule, which is only a timing change once the compiler has
//! resolved the manifest default into every untimed interaction (ADR-0008
//! decision 12), and `--explain`, which is the classifier's rule table rendered
//! for a human.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Runs `ridl` with `args`, returning `(exit_code, stdout, stderr)`.
fn ridl(args: &[&std::ffi::OsStr]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
        .args(args)
        .output()
        .expect("the ridl binary must run");
    let code = output.status.code().expect("the process exits with a code");
    (
        code,
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// One of the checked-in gate trees under `tools/diff/test_data/gate/`.
fn gate(variant: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/diff/test_data/gate")
        .join(variant)
}

/// The `breaking` tree is the `base` tree with its two interactions swapped: a
/// reorder, which shifts both wire identities.
#[test]
fn the_breaking_gate_tree_exits_one() {
    let (code, stdout, stderr) = ridl(&[
        "diff".as_ref(),
        gate("base").as_os_str(),
        gate("breaking").as_os_str(),
    ]);
    assert_eq!(code, 1, "a breaking tree gates, stderr:\n{stderr}");
    assert!(stdout.starts_with("breaking"), "stdout:\n{stdout}");
    assert!(
        stdout.contains("interaction_reordered"),
        "the reorder must be named, stdout:\n{stdout}"
    );
}

/// The `compatible` tree is the `base` tree with one interaction appended at the
/// end of the body.
#[test]
fn the_compatible_gate_tree_exits_zero() {
    let (code, stdout, stderr) = ridl(&[
        "diff".as_ref(),
        gate("base").as_os_str(),
        gate("compatible").as_os_str(),
    ]);
    assert_eq!(code, 0, "a compatible tree passes, stderr:\n{stderr}");
    assert!(stdout.starts_with("compatible"), "stdout:\n{stdout}");
    assert!(
        stdout.contains("[compatible] interaction_appended veh.cluster/VehicleStatus/hoodOpened"),
        "stdout:\n{stdout}"
    );
}

// --------------------------------------------------------------------------
// `[defaults].timing` — end to end over two trees differing only in ridl.toml.
// --------------------------------------------------------------------------

/// A unique directory under the system temp dir, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let mut path = std::env::temp_dir();
        path.push(format!(
            "ridl-diff-gate-{label}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst),
        ));
        std::fs::create_dir_all(&path).expect("create the temp dir");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn write(&self, relative: &str, text: &str) {
        std::fs::write(self.0.join(relative), text).expect("write the fixture file");
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// One untimed signal, so the package's whole timing contract comes from the
/// manifest default.
const UNTIMED: &str = "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
interface VehicleStatus {
  signal currentSpeed: Speed
}
";

/// A package tree whose only variable is the `[defaults].timing` string.
fn defaults_tree(label: &str, timing: &str) -> TempDir {
    let dir = TempDir::new(label);
    dir.write(
        "ridl.toml",
        &format!(
            "[package]\nname = \"veh.cluster\"\nversion = \"1.0.0\"\n\n\
             [defaults]\ntiming = \"{timing}\"\n"
        ),
    );
    dir.write("cluster.ridl", UNTIMED);
    dir
}

/// The rule needs no special case in the classifier: because diff compares the
/// *resolved* IR (ADR-0008 decision 12), editing `[defaults].timing` arrives as
/// an ordinary `timing_changed` on every defaulted interaction and is decided by
/// the ordinary bound rules. Loosening the staleness bound is breaking (ridl
/// §9.1: "the default is a convenience, not a loophole").
#[test]
fn loosening_the_configured_timing_default_is_breaking() {
    let old = defaults_tree("defaults-tight", "[100ms..1000ms]");
    let new = defaults_tree("defaults-loose", "[100ms..2000ms]");

    let (code, stdout, stderr) = ridl(&[
        "diff".as_ref(),
        old.path().as_os_str(),
        new.path().as_os_str(),
    ]);
    assert_eq!(
        code, 1,
        "a loosened staleness bound gates, stdout:\n{stdout}stderr:\n{stderr}"
    );
    assert!(
        stdout.contains("[breaking] timing_changed veh.cluster/VehicleStatus/currentSpeed"),
        "the manifest edit must surface as a timing change on the defaulted \
         interaction, stdout:\n{stdout}"
    );
    // The rendered values are the resolved microsecond bounds, not the manifest
    // text — proof that the classifier judged resolved IR.
    assert!(
        stdout.contains("1000000us") && stdout.contains("2000000us"),
        "the resolved bounds must render, stdout:\n{stdout}"
    );
}

/// The other direction of the same rule: tightening the configured default
/// strengthens every consumer-visible guarantee, so it passes.
#[test]
fn tightening_the_configured_timing_default_is_compatible() {
    let old = defaults_tree("defaults-loose-2", "[100ms..2000ms]");
    let new = defaults_tree("defaults-tight-2", "[100ms..1000ms]");

    let (code, stdout, stderr) = ridl(&[
        "diff".as_ref(),
        old.path().as_os_str(),
        new.path().as_os_str(),
    ]);
    assert_eq!(
        code, 0,
        "a tightened staleness bound passes, stdout:\n{stdout}stderr:\n{stderr}"
    );
    assert!(
        stdout.contains("[compatible] timing_changed veh.cluster/VehicleStatus/currentSpeed"),
        "stdout:\n{stdout}"
    );
}

// --------------------------------------------------------------------------
// `--explain`.
// --------------------------------------------------------------------------

/// `--explain` prints the category's rule row, both verdict directions
/// included — the CI-facing documentation of record until the E4 error index.
#[test]
fn explain_prints_the_rule_row_for_a_category() {
    let (code, stdout, stderr) = ridl(&[
        "diff".as_ref(),
        "--explain".as_ref(),
        "timing_changed".as_ref(),
    ]);
    assert_eq!(code, 0, "explaining a known category exits 0:\n{stderr}");
    assert!(
        stdout.starts_with("timing_changed\n"),
        "the row is headed by the category as the report prints it, stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("compatible") && stdout.contains("breaking"),
        "both directions must be documented, stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("min raised") && stdout.contains("min lowered"),
        "the directional rule must be spelled out, stdout:\n{stdout}"
    );
}

/// Every category a report can print is explainable, so a reader can always take
/// a word out of the output and ask what it means.
#[test]
fn every_reported_category_is_explainable() {
    for category in [
        "decl_added",
        "decl_removed",
        "interaction_appended",
        "interaction_inserted",
        "interaction_reordered",
        "interaction_removed",
        "interaction_retired",
        "kind_changed",
        "payload_changed",
        "return_changed",
        "params_changed",
        "timing_changed",
        "contract_changed",
        "width_changed",
        "constraint_changed",
        "init_changed",
        "reserved_name_redeclared",
        "service_changed",
        "doc_only",
        "visibility_changed",
    ] {
        let (code, stdout, stderr) =
            ridl(&["diff".as_ref(), "--explain".as_ref(), category.as_ref()]);
        assert_eq!(code, 0, "{category} must be explainable, stderr:\n{stderr}");
        assert!(
            stdout.starts_with(&format!("{category}\n")),
            "{category} printed the wrong row, stdout:\n{stdout}"
        );
    }
}

/// An unknown category is a usage error, and the message lists what is valid.
#[test]
fn explain_rejects_an_unknown_category() {
    let (code, _, stderr) = ridl(&[
        "diff".as_ref(),
        "--explain".as_ref(),
        "no_such_category".as_ref(),
    ]);
    assert_eq!(code, 2, "an unknown category is a usage error");
    assert!(
        stderr.contains("timing_changed"),
        "the valid categories must be listed, stderr:\n{stderr}"
    );
}

/// Without `--explain`, both inputs are still required.
#[test]
fn a_diff_without_inputs_is_a_usage_error() {
    let (code, _, stderr) = ridl(&["diff".as_ref()]);
    assert_eq!(code, 2, "a diff with no inputs exits 2");
    assert!(stderr.contains("--explain"), "stderr:\n{stderr}");
}
