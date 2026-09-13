//! Integration tests for the baseline publication gate (docs/ROADMAP.md epic
//! E2.9, general form §6.3): `ridl baseline` refusing to replace a published
//! snapshot when the replacement drops an interaction the snapshot still
//! declares and the source does not retire with a `reserved` tombstone.

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

/// Lays out a one-package workspace holding `source` and returns its root.
fn package_workspace(dir: &TempDir, source: &str) -> PathBuf {
    dir.write("ridl.toml", MANIFEST);
    dir.write("cluster.ridl", source);
    dir.path().to_path_buf()
}

/// Three events. The second is the one the later fixtures remove.
/// The shape mirrors `BASE` in `baseline_desk.rs:62` — a `type` declaration
/// rather than an enum, and a timing bound on every event — because that
/// fixture is known to compile.
const THREE: &str = "package veh.cluster
type DoorState: integer [0..1]
interface VehicleStatus {
  event doorOpened: DoorState @[100ms..1s]
  event doorClosed: DoorState @[100ms..1s]
  event doorLocked: DoorState @[100ms..1s]
}
";

/// `doorClosed` deleted outright. `doorLocked` slides onto ordinal 2.
const BARE_REMOVAL: &str = "package veh.cluster
type DoorState: integer [0..1]
interface VehicleStatus {
  event doorOpened: DoorState @[100ms..1s]
  event doorLocked: DoorState @[100ms..1s]
}
";

/// `doorClosed` retired in place. `doorLocked` keeps ordinal 3.
const TOMBSTONED_REMOVAL: &str = "package veh.cluster
type DoorState: integer [0..1]
interface VehicleStatus {
  event doorOpened: DoorState @[100ms..1s]
  reserved doorClosed
  event doorLocked: DoorState @[100ms..1s]
}
";

/// Four events, published as the baseline for the two-removal test below.
const FOUR: &str = "package veh.cluster
type DoorState: integer [0..1]
interface VehicleStatus {
  event doorOpened: DoorState @[100ms..1s]
  event doorClosed: DoorState @[100ms..1s]
  event doorLocked: DoorState @[100ms..1s]
  event doorAjar: DoorState @[100ms..1s]
}
";

/// `doorClosed` and `doorLocked` both deleted outright, neither with a
/// tombstone. Two refusals from one run, not one.
const TWO_BARE_REMOVALS: &str = "package veh.cluster
type DoorState: integer [0..1]
interface VehicleStatus {
  event doorOpened: DoorState @[100ms..1s]
  event doorAjar: DoorState @[100ms..1s]
}
";

/// The published baseline is the only record that a removed interaction's
/// ordinal was ever taken. Replacing it with a snapshot that drops the
/// interaction with no tombstone destroys that record, so publication refuses.
#[test]
fn baseline_refuses_to_publish_an_untombstoned_removal() {
    let dir = TempDir::new("gate-refuses");
    let root = package_workspace(&dir, THREE);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the first baseline is written: {stderr}");

    dir.write("cluster.ridl", BARE_REMOVAL);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(
        code, 1,
        "a refused publication is a negative answer, not a tool failure:\n{stderr}",
    );
    assert!(
        stderr.contains("RIDL-408"),
        "the refusal carries its code:\n{stderr}",
    );
    assert!(
        stderr.contains("doorClosed"),
        "the message names the interaction that would be dropped:\n{stderr}",
    );
    assert!(
        stderr.contains("reserved doorClosed"),
        "the message names the line that would sanction the removal:\n{stderr}",
    );
}

/// The refused run must leave the published record exactly as it was. An exit
/// code alone does not prove that: the refusal happens after a successful
/// compile, one step before the call that deletes the published files.
#[test]
fn a_refused_publication_leaves_the_baseline_byte_identical() {
    let dir = TempDir::new("gate-preserves");
    let root = package_workspace(&dir, THREE);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the first baseline is written: {stderr}");

    let snapshot = root
        .join(".ridl")
        .join("baseline")
        .join("veh.cluster.ir.json");
    let before = std::fs::read(&snapshot).expect("the published snapshot is readable");

    dir.write("cluster.ridl", BARE_REMOVAL);
    let (code, _, _) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 1, "the publication is refused");

    let after = std::fs::read(&snapshot).expect("the published snapshot survives the refusal");
    assert_eq!(before, after, "a refused publication rewrites nothing",);
}

/// The sanctioned retirement is the whole point of the tombstone, so it must
/// publish. `doorLocked` keeps ordinal 3 because the tombstone holds slot 2.
#[test]
fn baseline_publishes_a_tombstoned_removal() {
    let dir = TempDir::new("gate-admits-tombstone");
    let root = package_workspace(&dir, THREE);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the first baseline is written: {stderr}");

    dir.write("cluster.ridl", TOMBSTONED_REMOVAL);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(
        code, 0,
        "a removal that leaves a tombstone is the sanctioned path:\n{stderr}",
    );
    assert!(
        !stderr.contains("RIDL-408"),
        "the sanctioned path draws no refusal:\n{stderr}",
    );
}

/// With nothing published there is nothing to compare, so the first
/// publication is never refused and draws no diagnostic.
#[test]
fn the_first_publication_is_never_refused() {
    let dir = TempDir::new("gate-first");
    let root = package_workspace(&dir, BARE_REMOVAL);

    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(
        code, 0,
        "a first publication has no prior record:\n{stderr}"
    );
    assert!(
        !stderr.contains("RIDL-408"),
        "a first publication draws no refusal:\n{stderr}",
    );
}

/// The gate reports every untombstoned removal it finds in one run, not just
/// the first, so one run tells the author the whole correction.
#[test]
fn a_refusal_names_every_untombstoned_removal_in_one_run() {
    let dir = TempDir::new("gate-reports-all");
    let root = package_workspace(&dir, FOUR);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the first baseline is written: {stderr}");

    dir.write("cluster.ridl", TWO_BARE_REMOVALS);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(
        code, 1,
        "a refused publication is a negative answer, not a tool failure:\n{stderr}",
    );
    assert!(
        stderr.contains("doorClosed"),
        "the refusal names the first removed interaction:\n{stderr}",
    );
    assert!(
        stderr.contains("doorLocked"),
        "the refusal names the second removed interaction, not just the first:\n{stderr}",
    );
}
