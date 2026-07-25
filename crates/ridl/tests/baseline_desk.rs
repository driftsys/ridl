//! Integration tests for the baseline-aware desk check (docs/ROADMAP.md epic
//! E2.9, general form §6.3): `ridl baseline` writing one `.ir.json` snapshot per
//! package, and `ridl check` rendering an ordinal-affecting drift against that
//! baseline as a RIDL-407 warning that never moves the exit code.

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
            "ridl-baseline-{label}-{}-{}",
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

const MANIFEST: &str = "[package]\nname = \"veh.cluster\"\nversion = \"1.0.0\"\n";

/// The published shape: three interactions at ordinals 1, 2, 3.
const BASE: &str = "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
type DoorState: integer [0..1]
interface VehicleStatus {
  signal currentSpeed: Speed @10ms
  event doorOpened: DoorState @[100ms..1s]
  event doorClosed: DoorState @[100ms..1s]
}
";

/// `doorClosed` moved ahead of `doorOpened` — the ordinals of both shift, the
/// general form §6.3 tidying that looks harmless in a diff.
const REORDERED: &str = "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
type DoorState: integer [0..1]
interface VehicleStatus {
  signal currentSpeed: Speed @10ms
  event doorClosed: DoorState @[100ms..1s]
  event doorOpened: DoorState @[100ms..1s]
}
";

/// A fourth interaction appended at the end — no surviving ordinal moves.
const APPENDED: &str = "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
type DoorState: integer [0..1]
interface VehicleStatus {
  signal currentSpeed: Speed @10ms
  event doorOpened: DoorState @[100ms..1s]
  event doorClosed: DoorState @[100ms..1s]
  event hoodOpened: DoorState @[100ms..1s]
}
";

/// `doorOpened` deleted without a `reserved` tombstone — every later ordinal
/// slides down.
const REMOVED: &str = "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
type DoorState: integer [0..1]
interface VehicleStatus {
  signal currentSpeed: Speed @10ms
  event doorClosed: DoorState @[100ms..1s]
}
";

/// A breaking change that moves no ordinal: the payload type narrows and the
/// staleness bound is raised. `ridl diff` gates on both; the desk check must
/// stay silent, because general form §6.3 asks it for ordinal drift only.
const NON_ORDINAL: &str = "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
type DoorState: integer [0..1]
type NarrowState: integer [0..0]
interface VehicleStatus {
  signal currentSpeed: Speed @10ms
  event doorOpened: NarrowState @[100ms..2s]
  event doorClosed: DoorState @[100ms..1s]
}
";

/// Lays out a one-package workspace holding `source` and returns its root.
fn package_workspace(dir: &TempDir, source: &str) -> PathBuf {
    dir.write("ridl.toml", MANIFEST);
    dir.write("cluster.ridl", source);
    dir.path().to_path_buf()
}

/// `ridl baseline` writes one `<pkg-name>.ir.json` per package into
/// `.ridl/baseline/` at the workspace root.
#[test]
fn baseline_writes_one_snapshot_per_package() {
    let dir = TempDir::new("write");
    let root = package_workspace(&dir, BASE);

    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(code, 0, "a clean workspace snapshots cleanly: {stderr}");
    assert!(
        root.join(".ridl/baseline/veh.cluster.ir.json").is_file(),
        "the package snapshot lands under `.ridl/baseline/`",
    );
}

/// One `.ir.json` holds exactly one package, so an N-package workspace needs N
/// files — the multi-package case is not a single merged snapshot.
#[test]
fn baseline_writes_one_file_per_package_in_a_multi_package_workspace() {
    let dir = TempDir::new("multi");
    dir.write(
        "ridl.toml",
        "[workspace]\nmembers = [\"common\", \"cluster\"]\n",
    );
    dir.write(
        "common/ridl.toml",
        "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n",
    );
    dir.write(
        "common/common.typl",
        "package veh.common\ntype Speed: km/h [0.0..250.0 step 0.5]\n",
    );
    dir.write("cluster/ridl.toml", MANIFEST);
    dir.write(
        "cluster/cluster.ridl",
        "package veh.cluster
type DoorState: integer [0..1]
interface VehicleStatus {
  event doorOpened: DoorState @[100ms..1s]
  event doorClosed: DoorState @[100ms..1s]
}
",
    );
    let root = dir.path();

    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(code, 0, "the workspace snapshots cleanly: {stderr}");
    let baseline = root.join(".ridl/baseline");
    let mut written: Vec<String> = std::fs::read_dir(&baseline)
        .expect("the baseline directory exists")
        .map(|entry| {
            entry
                .expect("a readable entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    written.sort();
    assert_eq!(
        written,
        vec![
            "veh.cluster.ir.json".to_string(),
            "veh.common.ir.json".to_string(),
        ],
        "two packages produce two snapshot files",
    );
}

/// `--out` redirects the snapshots away from the default directory.
#[test]
fn baseline_out_flag_selects_the_directory() {
    let dir = TempDir::new("out");
    let root = package_workspace(&dir, BASE);
    let out = dir.path().join("published");

    let (code, _, stderr) = ridl(&[
        "baseline".as_ref(),
        root.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    assert_eq!(code, 0, "the workspace snapshots cleanly: {stderr}");
    assert!(
        out.join("veh.cluster.ir.json").is_file(),
        "`--out` receives the snapshot",
    );
    assert!(
        !root.join(".ridl/baseline").exists(),
        "`--out` replaces the default directory rather than adding to it",
    );
}

/// A reorder against the baseline draws RIDL-407, names the moved interaction
/// in the diff path, and points the span at that interaction's declaration.
#[test]
fn check_flags_a_reorder_against_the_baseline() {
    let dir = TempDir::new("reorder");
    let root = package_workspace(&dir, BASE);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the baseline is written: {stderr}");

    dir.write("cluster.ridl", REORDERED);
    let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);

    assert!(
        stderr.contains("RIDL-407"),
        "the reorder draws the coded desk warning:\n{stderr}",
    );
    assert!(
        stderr.contains("interaction ordinal changed against the baseline"),
        "the message states what moved:\n{stderr}",
    );
    assert!(
        stderr.contains("veh.cluster/VehicleStatus/doorClosed"),
        "the diff path names the moved interaction:\n{stderr}",
    );
    assert!(
        stderr.contains("event doorClosed: DoorState"),
        "the span underlines the interaction's declaration:\n{stderr}",
    );
    assert_eq!(
        code, 0,
        "a warning never moves the exit code of an otherwise clean check:\n{stderr}",
    );
}

/// A non-tombstoned removal is ordinal-affecting too, and is reported even
/// though the declaration it names is gone from the current source.
#[test]
fn check_flags_a_removal_against_the_baseline() {
    let dir = TempDir::new("removal");
    let root = package_workspace(&dir, BASE);
    ridl(&["baseline".as_ref(), root.as_os_str()]);

    dir.write("cluster.ridl", REMOVED);
    let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);

    assert!(
        stderr.contains("RIDL-407") && stderr.contains("veh.cluster/VehicleStatus/doorOpened"),
        "the removal draws the desk warning:\n{stderr}",
    );
    assert_eq!(code, 0, "the warning leaves the exit code alone:\n{stderr}");
}

/// An append shifts no surviving ordinal, so the desk check stays silent — it
/// is the general form §6.3 mitigation, not a second diff gate.
#[test]
fn check_is_silent_for_an_append() {
    let dir = TempDir::new("append");
    let root = package_workspace(&dir, BASE);
    ridl(&["baseline".as_ref(), root.as_os_str()]);

    dir.write("cluster.ridl", APPENDED);
    let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);

    assert!(
        !stderr.contains("RIDL-407"),
        "an appended interaction is not ordinal-affecting:\n{stderr}",
    );
    assert_eq!(code, 0, "a compatible change stays clean:\n{stderr}");
}

/// With no baseline anywhere, `ridl check` behaves exactly as before: no extra
/// output, same exit code.
#[test]
fn check_without_a_baseline_is_unchanged() {
    let dir = TempDir::new("nobaseline");
    let root = package_workspace(&dir, REORDERED);

    let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);

    assert_eq!(code, 0, "the workspace is clean:\n{stderr}");
    assert!(
        stderr.is_empty(),
        "a missing baseline is silently skipped:\n{stderr}",
    );
}

/// `--baseline` accepts a single `.ir.json` file, not only a directory.
#[test]
fn check_accepts_a_single_baseline_file() {
    let dir = TempDir::new("file");
    let root = package_workspace(&dir, BASE);
    let out = dir.path().join("published");
    ridl(&[
        "baseline".as_ref(),
        root.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    dir.write("cluster.ridl", REORDERED);
    let snapshot = out.join("veh.cluster.ir.json");
    let (code, _, stderr) = ridl(&[
        "check".as_ref(),
        root.as_os_str(),
        "--baseline".as_ref(),
        snapshot.as_os_str(),
    ]);

    assert!(
        stderr.contains("RIDL-407"),
        "a single-file baseline drives the same check:\n{stderr}",
    );
    assert_eq!(code, 0, "the exit code is untouched:\n{stderr}");
}

/// A `--baseline` path that does not exist is an input error, exit 2 — only
/// auto-discovery is silent.
#[test]
fn check_reports_a_missing_explicit_baseline() {
    let dir = TempDir::new("missing");
    let root = package_workspace(&dir, BASE);
    let absent = dir.path().join("no-such-dir");

    let (code, _, stderr) = ridl(&[
        "check".as_ref(),
        root.as_os_str(),
        "--baseline".as_ref(),
        absent.as_os_str(),
    ]);

    assert_eq!(code, 2, "a named baseline that is absent is an error");
    assert!(stderr.contains("baseline"), "the error says so:\n{stderr}");
}

/// A workspace that does not compile keeps its error exit and draws no desk
/// warning — the desk check runs only after a clean compile.
#[test]
fn check_skips_the_desk_check_when_the_compile_fails() {
    let dir = TempDir::new("broken");
    let root = package_workspace(&dir, BASE);
    ridl(&["baseline".as_ref(), root.as_os_str()]);

    dir.write(
        "cluster.ridl",
        "package veh.cluster
type DoorState: integer [0..1]
interface VehicleStatus {
  event doorClosed: Nope
}
",
    );
    let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);

    assert_eq!(code, 1, "the compile error still gates:\n{stderr}");
    assert!(
        !stderr.contains("RIDL-407"),
        "no desk warning over a broken compile:\n{stderr}",
    );
}

/// The desk check reports ordinal drift and nothing else. A payload narrowing
/// and a raised staleness bound are both breaking — `ridl diff` says so in the
/// same breath — but neither moves an ordinal, so the desk stays quiet and CI
/// keeps that job (general form §6.3).
#[test]
fn check_is_silent_on_a_non_ordinal_breaking_change() {
    let dir = TempDir::new("nonordinal");
    let root = package_workspace(&dir, BASE);
    let out = dir.path().join("published");
    ridl(&[
        "baseline".as_ref(),
        root.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    dir.write("cluster.ridl", NON_ORDINAL);

    // The control: the very same edit is breaking to `ridl diff`.
    let snapshot = out.join("veh.cluster.ir.json");
    let (diff_code, diff_stdout, _) =
        ridl(&["diff".as_ref(), snapshot.as_os_str(), root.as_os_str()]);
    assert_eq!(diff_code, 1, "the edit is breaking:\n{diff_stdout}");

    let (code, _, stderr) = ridl(&[
        "check".as_ref(),
        root.as_os_str(),
        "--baseline".as_ref(),
        out.as_os_str(),
    ]);

    assert!(
        !stderr.contains("RIDL-407"),
        "a breaking change that moves no ordinal is not the desk check's job:\n{stderr}",
    );
    assert_eq!(code, 0, "and it does not gate the check:\n{stderr}");
}

/// Snapshots are matched to packages by the package name *inside* the file, not
/// by the file's name — so a renamed or hand-managed snapshot still lines up
/// with the package it holds.
#[test]
fn check_matches_snapshots_by_package_name_not_file_name() {
    let dir = TempDir::new("byname");
    let root = package_workspace(&dir, BASE);
    let out = dir.path().join("published");
    ridl(&[
        "baseline".as_ref(),
        root.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    std::fs::rename(
        out.join("veh.cluster.ir.json"),
        out.join("zzz-totally-wrong-name.ir.json"),
    )
    .expect("rename the snapshot");

    dir.write("cluster.ridl", REORDERED);
    let (code, _, stderr) = ridl(&[
        "check".as_ref(),
        root.as_os_str(),
        "--baseline".as_ref(),
        out.as_os_str(),
    ]);

    assert!(
        stderr.contains("RIDL-407") && stderr.contains("veh.cluster/VehicleStatus/doorClosed"),
        "the misnamed snapshot is still attributed to its package:\n{stderr}",
    );
    assert_eq!(code, 0, "the exit code is untouched:\n{stderr}");
}

/// `ridl diff <baseline-dir> <workspace>` reads the directory as a snapshot
/// set. Compiling it as source instead would diff the current source against
/// itself and always report `identical` — a CI gate that fails open.
#[test]
fn diff_reads_a_baseline_directory_as_a_snapshot_set() {
    let dir = TempDir::new("diffdir");
    let root = package_workspace(&dir, BASE);
    ridl(&["baseline".as_ref(), root.as_os_str()]);
    let baseline = root.join(".ridl/baseline");

    dir.write("cluster.ridl", REORDERED);
    let (code, stdout, stderr) = ridl(&["diff".as_ref(), baseline.as_os_str(), root.as_os_str()]);

    assert_eq!(code, 1, "the reorder is breaking:\n{stdout}{stderr}");
    assert!(
        stdout.contains("interaction_reordered")
            && stdout.contains("veh.cluster/VehicleStatus/doorClosed"),
        "the report names the change:\n{stdout}",
    );
}

/// The same comparison over an unchanged workspace is clean.
#[test]
fn diff_of_an_unchanged_workspace_against_its_baseline_is_clean() {
    let dir = TempDir::new("diffsame");
    let root = package_workspace(&dir, BASE);
    ridl(&["baseline".as_ref(), root.as_os_str()]);
    let baseline = root.join(".ridl/baseline");

    let (code, stdout, stderr) = ridl(&["diff".as_ref(), baseline.as_os_str(), root.as_os_str()]);

    assert_eq!(code, 0, "nothing changed:\n{stdout}{stderr}");
    assert!(stdout.contains("identical"), "and it says so:\n{stdout}");
}

/// A directory holding no `.ir.json` is source, not a snapshot set: it still
/// takes the compile path, so source-tree comparison is unaffected.
#[test]
fn diff_of_a_directory_without_snapshots_compiles_it_as_source() {
    let old = TempDir::new("srcold");
    let old_root = package_workspace(&old, BASE);
    let new = TempDir::new("srcnew");
    let new_root = package_workspace(&new, REORDERED);

    let (code, stdout, stderr) =
        ridl(&["diff".as_ref(), old_root.as_os_str(), new_root.as_os_str()]);

    assert_eq!(code, 1, "two source trees still compare:\n{stdout}{stderr}");
    assert!(
        stdout.contains("interaction_reordered"),
        "and the reorder is found:\n{stdout}",
    );
}

/// Re-publishing a baseline regenerates it wholesale: a snapshot for a package
/// the workspace no longer declares is dropped rather than left to rot.
#[test]
fn baseline_drops_a_snapshot_whose_package_is_gone() {
    let dir = TempDir::new("rename");
    dir.write("ridl.toml", MANIFEST);
    dir.write("cluster.ridl", BASE);
    let root = dir.path().to_path_buf();
    ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert!(
        root.join(".ridl/baseline/veh.cluster.ir.json").is_file(),
        "the first baseline is published",
    );

    // Rename the package: the manifest and the source move together.
    dir.write(
        "ridl.toml",
        "[package]\nname = \"veh.dash\"\nversion = \"1.0.0\"\n",
    );
    dir.write("cluster.ridl", &BASE.replace("veh.cluster", "veh.dash"));
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the renamed workspace compiles:\n{stderr}");

    let baseline = root.join(".ridl/baseline");
    assert!(
        baseline.join("veh.dash.ir.json").is_file(),
        "the new package is published",
    );
    assert!(
        !baseline.join("veh.cluster.ir.json").exists(),
        "the snapshot under the old package name is gone",
    );
}

/// A workspace that does not compile leaves the published baseline untouched —
/// re-publishing must never destroy a good baseline.
#[test]
fn baseline_keeps_the_published_snapshots_when_the_compile_fails() {
    let dir = TempDir::new("keep");
    let root = package_workspace(&dir, BASE);
    ridl(&["baseline".as_ref(), root.as_os_str()]);
    let snapshot = root.join(".ridl/baseline/veh.cluster.ir.json");
    let published = std::fs::read_to_string(&snapshot).expect("read the published snapshot");

    dir.write(
        "cluster.ridl",
        "package veh.cluster
interface VehicleStatus {
  event doorClosed: Nope @[100ms..1s]
}
",
    );
    let (code, _, _) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(code, 1, "the broken workspace fails to publish");
    assert_eq!(
        std::fs::read_to_string(&snapshot).expect("the snapshot survives"),
        published,
        "the published baseline is exactly as it was",
    );
}

/// The committed baseline corpus member (E2 task 22). Every other test in this
/// file builds its baseline in a temp directory from a source string, so none
/// of them exercises a baseline that was written by an earlier version of the
/// toolchain and read back later — which is the only way a real baseline is
/// ever used. `tests/baseline-corpus/` is that case: a package plus a
/// committed `.ridl/baseline/corpus.baseline.ir.json`, read from disk exactly
/// as a published baseline is.
///
/// The source has drifted from the snapshot in two ways that look like
/// tidying: the two events are alphabetised, and `setGear` is removed with no
/// tombstone. All three surviving-or-lost identities are reported, and the
/// exit code stays 0 — the desk check informs, `ridl diff` gates.
#[test]
fn check_reports_ordinal_drift_against_the_committed_baseline() {
    let entry = Path::new("tests/baseline-corpus");
    assert!(
        entry
            .join(".ridl/baseline/corpus.baseline.ir.json")
            .is_file(),
        "the committed baseline snapshot is the point of this fixture",
    );

    let (code, _, stderr) = ridl(&["check".as_ref(), entry.as_os_str()]);

    assert_eq!(code, 0, "a desk-check warning never moves the exit code");

    // Pinned per diagnostic: the code, the severity word, the diff path in the
    // message, and the source line the span points at.
    for (path, category, line) in [
        (
            "corpus.baseline/VehicleStatus/setGear",
            "interaction_removed",
            "interface VehicleStatus {",
        ),
        (
            "corpus.baseline/VehicleStatus/doorOpened",
            "interaction_reordered",
            "event doorOpened : DoorState @[100ms..1s]",
        ),
        (
            "corpus.baseline/VehicleStatus/doorClosed",
            "interaction_reordered",
            "event doorClosed : DoorState @[100ms..1s]",
        ),
    ] {
        let expected = format!(
            "warning[RIDL-407]: interaction ordinal changed against the baseline: {path} ({category})"
        );
        assert!(
            stderr.contains(&expected),
            "expected `{expected}` in:\n{stderr}"
        );
        assert!(
            stderr.contains(line),
            "the span for {path} must point at `{line}` in:\n{stderr}"
        );
    }

    // The three above are the whole report: a fourth RIDL-407 would mean the
    // desk check flagged an interaction whose ordinal did not move.
    assert_eq!(
        stderr.matches("RIDL-407").count(),
        3,
        "exactly three ordinal-affecting changes, no more:\n{stderr}"
    );
}
