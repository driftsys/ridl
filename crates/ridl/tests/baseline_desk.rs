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
