//! Integration tests for protocol A in `ridl check` and `ridl build` (lock
//! design §4): one test per row of the §4 table's `ridl check` / `ridl build`
//! column, the desk check running with RIDL-409 as the only error, and the
//! rename hint — a label on RIDL-409 naming the one `ridl lock --rename`
//! when the published baseline shows exactly one declaration without an
//! entry with the orphan entry's shape (plan decision PD-5). Every test that
//! must write nothing asserts the lock file byte-identical afterwards.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A unique directory under the system temp dir, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let mut path = std::env::temp_dir();
        path.push(format!(
            "ridl-lock-protocol-{label}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst),
        ));
        std::fs::create_dir_all(&path).expect("create the temp dir");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn write(&self, relative: &str, text: &str) -> PathBuf {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().expect("a relative path has a parent"))
            .expect("create parent directories");
        std::fs::write(&path, text).expect("write the fixture file");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

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

const MANIFEST: &str = "[package]\nname = \"veh.hvac\"\nversion = \"1.0.0\"\n";
const HEADER: &str = "# interfaces.lock — written by ridl lock; do not edit by hand.\n";

/// The lock every fixture starts from: `Cabin` and `Zone` frozen.
const LOCK: &str = "next 3\nCabin 1\nZone 2\n";

/// The published shape: `Cabin` with one member, `Zone` with two.
const BASE: &str = "package veh.hvac
type State: integer [0..1]
interface Cabin { signal c : State @[100ms..1s] }
interface Zone {
  signal level : State @[100ms..1s]
  event opened : State @[100ms..1s]
}
";

/// `Zone` renamed to `Lane`, member for member, with a doc comment on one
/// member that the baseline's `Zone` does not carry.
const ZONE_RENAMED: &str = "package veh.hvac
type State: integer [0..1]
interface Cabin { signal c : State @[100ms..1s] }
interface Lane {
  /// The zone's level.
  signal level : State @[100ms..1s]
  event opened : State @[100ms..1s]
}
";

/// `Zone` gone and two declarations with its shape.
const TWO_CANDIDATES: &str = "package veh.hvac
type State: integer [0..1]
interface Cabin { signal c : State @[100ms..1s] }
interface Lane {
  signal level : State @[100ms..1s]
  event opened : State @[100ms..1s]
}
interface Strip {
  signal level : State @[100ms..1s]
  event opened : State @[100ms..1s]
}
";

/// `Zone` gone and one declaration of a different shape: `opened` is a
/// signal here, an event in the baseline.
const OTHER_SHAPE: &str = "package veh.hvac
type State: integer [0..1]
interface Cabin { signal c : State @[100ms..1s] }
interface Lane {
  signal level : State @[100ms..1s]
  signal opened : State @[100ms..1s]
}
";

/// `Zone` renamed to `Lane` beside a type whose bounds are inverted
/// (TYPL-104), a second error next to RIDL-409.
const ZONE_RENAMED_WITH_AN_ERROR: &str = "package veh.hvac
type State: integer [0..1]
type Bad: integer [10..0]
interface Cabin { signal c : State @[100ms..1s] }
interface Lane {
  signal level : State @[100ms..1s]
  event opened : State @[100ms..1s]
}
";

/// `Cabin` alone: `Zone` removed with no candidate.
const ZONE_REMOVED: &str = "package veh.hvac
type State: integer [0..1]
interface Cabin { signal c : State @[100ms..1s] }
";

/// `Zone`'s two members swapped — an ordinal drift the desk check reports
/// as RIDL-407.
const ZONE_REORDERED: &str = "package veh.hvac
type State: integer [0..1]
interface Cabin { signal c : State @[100ms..1s] }
interface Zone {
  event opened : State @[100ms..1s]
  signal level : State @[100ms..1s]
}
";

/// Lays out a one-package workspace holding `source` and `lock` (when given)
/// and returns its root.
fn package_workspace(dir: &TempDir, source: &str, lock: Option<&str>) -> PathBuf {
    dir.write("ridl.toml", MANIFEST);
    dir.write("hvac.ridl", source);
    if let Some(lock) = lock {
        dir.write("interfaces.lock", &format!("{HEADER}{lock}"));
    }
    dir.path().to_path_buf()
}

/// Publishes the workspace as its own baseline, then rewrites the source.
fn publish_then_edit(dir: &TempDir, root: &Path, edited: &str) {
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the baseline is published: {stderr}");
    dir.write("hvac.ridl", edited);
}

/// The text of `<root>/interfaces.lock`.
fn read_lock(root: &Path) -> String {
    std::fs::read_to_string(root.join("interfaces.lock")).expect("the lock file exists")
}

/// `ridl check <root>`'s exit code and stderr.
fn check(root: &Path) -> (i32, String) {
    let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);
    (code, stderr)
}

/// The label text the rename hint carries for `old` and `new` at `root`.
fn hint(root: &Path, old: &str, new: &str) -> String {
    format!(
        "same shape as `{old}` in the published baseline: run `ridl lock {} --rename {old}={new}`",
        root.display()
    )
}

// --- §4 table, the `ridl check` / `ridl build` column -----------------------

/// Row 1: a declaration with no entry compiles with a provisional number and
/// draws no diagnostic; nothing writes the lock file.
#[test]
fn a_declaration_with_no_entry_checks_clean_with_a_provisional_number() {
    let dir = TempDir::new("provisional");
    let root = package_workspace(&dir, BASE, None);

    let (code, stderr) = check(&root);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert!(!stderr.contains("RIDL-409"), "stderr:\n{stderr}");
    assert!(
        !root.join("interfaces.lock").exists(),
        "`ridl check` never writes the lock"
    );
}

/// Row 2: an entry with no declaration and no candidate is RIDL-409, exit 1,
/// naming `--retire Old` alone — there is no declaration without an entry —
/// and the lock is untouched.
#[test]
fn an_orphan_entry_with_no_candidate_names_retire() {
    let dir = TempDir::new("no-candidate");
    let root = package_workspace(&dir, ZONE_REMOVED, Some(LOCK));
    let text = read_lock(&root);

    let (code, stderr) = check(&root);
    assert_eq!(code, 1, "stderr:\n{stderr}");
    assert!(stderr.contains("error[RIDL-409]"), "stderr:\n{stderr}");
    assert!(
        stderr.contains(&format!(
            "run `ridl lock {} --retire Zone` to record",
            root.display()
        )),
        "stderr:\n{stderr}"
    );
    assert!(!stderr.contains("--rename"), "stderr:\n{stderr}");
    assert!(!stderr.contains("same shape"), "stderr:\n{stderr}");
    assert_eq!(read_lock(&root), text, "byte-identical");
}

/// Row 3 (§12 bullet 2): with a published baseline, exactly one declaration
/// without an entry that has `Old`'s shape — member for member, the doc
/// comments blanked — draws the label naming the single `--rename`. The
/// baseline's `Zone` is frozen (number 2) and the candidate `Lane` is
/// provisional; the comparison is over the interactions alone.
#[test]
fn one_same_shape_candidate_adds_the_rename_label() {
    let dir = TempDir::new("one-candidate");
    let root = package_workspace(&dir, BASE, Some(LOCK));
    publish_then_edit(&dir, &root, ZONE_RENAMED);
    let text = read_lock(&root);

    let (code, stderr) = check(&root);
    assert_eq!(code, 1, "RIDL-409 keeps its exit code: {stderr}");
    assert!(stderr.contains("error[RIDL-409]"), "stderr:\n{stderr}");
    assert!(
        stderr.contains("--rename Zone=New` when a declaration"),
        "the message still names both commands: {stderr}"
    );
    assert!(
        stderr.contains(&hint(&root, "Zone", "Lane")),
        "the label names the one rename: {stderr}"
    );
    assert!(
        stderr.contains("interface Lane"),
        "the label points at the candidate's declaration: {stderr}"
    );
    assert_eq!(read_lock(&root), text, "byte-identical");
}

/// The desk check itself runs with RIDL-409 as the only error: an ordinal
/// drift beside an orphan entry still draws its RIDL-407 warning.
#[test]
fn the_desk_check_runs_with_ridl_409_present() {
    let dir = TempDir::new("desk-check-runs");
    let root = package_workspace(&dir, BASE, Some(LOCK));
    publish_then_edit(&dir, &root, ZONE_REORDERED);
    dir.write(
        "interfaces.lock",
        &format!("{HEADER}next 4\nCabin 1\nZone 2\nLegacy 3\n"),
    );

    let (code, stderr) = check(&root);
    assert_eq!(code, 1, "stderr:\n{stderr}");
    assert!(stderr.contains("error[RIDL-409]"), "stderr:\n{stderr}");
    assert!(
        stderr.contains("warning[RIDL-407]"),
        "the desk check ran beside RIDL-409: {stderr}"
    );
}

/// Row 4, several candidates: two declarations without an entry share
/// `Old`'s shape, so no label is added — the author chooses.
#[test]
fn several_same_shape_candidates_add_no_label() {
    let dir = TempDir::new("several-candidates");
    let root = package_workspace(&dir, BASE, Some(LOCK));
    publish_then_edit(&dir, &root, TWO_CANDIDATES);

    let (code, stderr) = check(&root);
    assert_eq!(code, 1, "stderr:\n{stderr}");
    assert!(stderr.contains("error[RIDL-409]"), "stderr:\n{stderr}");
    assert!(stderr.contains("--rename Zone=New"), "stderr:\n{stderr}");
    assert!(!stderr.contains("same shape"), "stderr:\n{stderr}");
}

/// Row 4, no candidate of that shape: the one declaration without an entry
/// differs from `Old` member for member, so no label is added.
#[test]
fn no_same_shape_candidate_adds_no_label() {
    let dir = TempDir::new("no-same-shape");
    let root = package_workspace(&dir, BASE, Some(LOCK));
    publish_then_edit(&dir, &root, OTHER_SHAPE);

    let (code, stderr) = check(&root);
    assert_eq!(code, 1, "stderr:\n{stderr}");
    assert!(stderr.contains("error[RIDL-409]"), "stderr:\n{stderr}");
    assert!(stderr.contains("--rename Zone=New"), "stderr:\n{stderr}");
    assert!(!stderr.contains("same shape"), "stderr:\n{stderr}");
}

/// Row 5: with no baseline published there is no shape to compare, so
/// RIDL-409 names both commands and carries no label.
#[test]
fn no_baseline_means_no_label() {
    let dir = TempDir::new("no-baseline");
    let root = package_workspace(&dir, ZONE_RENAMED, Some(LOCK));
    let text = read_lock(&root);

    let (code, stderr) = check(&root);
    assert_eq!(code, 1, "stderr:\n{stderr}");
    assert!(stderr.contains("error[RIDL-409]"), "stderr:\n{stderr}");
    assert!(stderr.contains("--rename Zone=New"), "stderr:\n{stderr}");
    assert!(stderr.contains("--retire Zone"), "stderr:\n{stderr}");
    assert!(!stderr.contains("same shape"), "stderr:\n{stderr}");
    assert_eq!(read_lock(&root), text, "byte-identical");
}

/// Row 6: a lock line deleted by hand. The build cannot see it: the kept
/// declaration is provisional, and the package checks clean.
#[test]
fn a_hand_deleted_line_is_invisible_to_check() {
    let dir = TempDir::new("hand-deleted");
    let root = package_workspace(&dir, BASE, Some("next 3\nCabin 1\n"));
    let text = read_lock(&root);

    let (code, stderr) = check(&root);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert!(!stderr.contains("RIDL-4"), "stderr:\n{stderr}");
    assert_eq!(read_lock(&root), text, "byte-identical");
}

/// §4 "and `ridl build`": the compiler's RIDL-409 stops a build too, and no
/// artifact is written.
#[test]
fn build_fails_on_an_orphan_entry_too() {
    let dir = TempDir::new("build");
    let root = package_workspace(&dir, ZONE_REMOVED, Some(LOCK));
    let out = TempDir::new("build-out");

    let (code, _, stderr) = ridl(&[
        "build".as_ref(),
        root.as_os_str(),
        "--out-dir".as_ref(),
        out.path().as_os_str(),
    ]);
    assert_eq!(code, 1, "stderr:\n{stderr}");
    assert!(stderr.contains("error[RIDL-409]"), "stderr:\n{stderr}");
    assert!(
        std::fs::read_dir(out.path())
            .expect("the out dir exists")
            .next()
            .is_none(),
        "nothing is written"
    );
}

/// §4, `run_check`'s condition: a second error beside RIDL-409 skips the desk
/// check, so the same-shape candidate draws no label.
#[test]
fn a_second_error_beside_ridl_409_skips_the_desk_check() {
    let dir = TempDir::new("second-error");
    let root = package_workspace(&dir, BASE, Some(LOCK));
    publish_then_edit(&dir, &root, ZONE_RENAMED_WITH_AN_ERROR);

    let (code, stderr) = check(&root);
    assert_eq!(code, 1, "stderr:\n{stderr}");
    assert!(stderr.contains("error[RIDL-409]"), "stderr:\n{stderr}");
    assert!(stderr.contains("error[TYPL-104]"), "stderr:\n{stderr}");
    assert!(!stderr.contains("same shape"), "stderr:\n{stderr}");
}
