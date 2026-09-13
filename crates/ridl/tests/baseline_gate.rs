//! Integration tests for the baseline publication gate
//! (docs/wip/2026-09-13-baseline-gate-design.md, general form §6.3): `ridl
//! baseline` refusing to replace a published snapshot when the replacement
//! drops an interaction the snapshot still declares and the source does not
//! retire it with a `reserved` tombstone at its own ordinal.

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

/// `doorClosed` retired, but `reserved doorClosed` sits last rather than at
/// ordinal 2, the slot `doorClosed` held in `THREE`. `diff_interface`
/// (ridl-diff/src/walk.rs:339-363) emits this as `InteractionRemoved` with a
/// `change.after` carrying the tombstone's own ordinal (3), which is the one
/// `InteractionRemoved` emission that carries an `after` at all — the
/// structural signal message (b) is selected on.
const MISPLACED_TOMBSTONE: &str = "package veh.cluster
type DoorState: integer [0..1]
interface VehicleStatus {
  event doorOpened: DoorState @[100ms..1s]
  event doorLocked: DoorState @[100ms..1s]
  reserved doorClosed
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
    assert!(
        stderr.contains("is gone from the source"),
        "a bare removal draws message (a), not the misplaced- or dropped-tombstone wording:\n\
         {stderr}",
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

    let staging = root.join(".ridl").join(".baseline.staging");
    assert!(
        !staging.exists(),
        "a refused publication removes the staging directory it built, not just the files it \
         declines to move: {}",
        staging.display(),
    );
}

/// The sanctioned retirement is the whole point of the tombstone, so it must
/// publish. `doorLocked` keeps ordinal 3 because the tombstone holds slot 2.
#[test]
fn baseline_publishes_a_tombstoned_removal() {
    let dir = TempDir::new("gate-admits-tombstone");
    let root = package_workspace(&dir, THREE);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the first baseline is written: {stderr}");

    let snapshot = root
        .join(".ridl")
        .join("baseline")
        .join("veh.cluster.ir.json");
    let before = std::fs::read(&snapshot).expect("the first published snapshot is readable");

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

    let after = std::fs::read(&snapshot).expect("the republished snapshot is readable");
    assert_ne!(
        before, after,
        "the sanctioned path still publishes: the snapshot must now record `doorClosed` as \
         retired, not read back as the untouched first publication",
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

/// Design section 3 defines a first publication as "the output directory is
/// absent, or holds no snapshot" — the second half of that sentence is
/// distinct from the first: an *existing, empty* `.ridl/baseline/` reaches
/// `untombstoned_removals`'s `out_dir.is_dir()` check as `true` and only then
/// meets `published.is_empty()`, which the test above never exercises because
/// it never creates the directory at all. This pins that the empty-but-present
/// shape is still a first publication, not something to compare against.
#[test]
fn an_existing_empty_baseline_directory_is_still_a_first_publication() {
    let dir = TempDir::new("gate-empty-dir-first");
    let root = package_workspace(&dir, THREE);
    let published = root.join(".ridl").join("baseline");
    std::fs::create_dir_all(&published).expect("create the empty baseline directory");

    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(
        code, 0,
        "an empty output directory is a first publication, not something to compare against:\n\
         {stderr}",
    );
    assert!(
        !stderr.contains("RIDL-408"),
        "a first publication draws no refusal:\n{stderr}",
    );
    assert!(
        published.join("veh.cluster.ir.json").is_file(),
        "the snapshot is written into the directory that was already there",
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

/// A tombstone that retires the right name at the wrong ordinal is still a
/// refusal: `reserved doorClosed` in `MISPLACED_TOMBSTONE` holds ordinal 3,
/// not the ordinal 2 `doorClosed` held in the published baseline, so the
/// surviving interactions would slide into the freed slot (ridl §11). This is
/// message (b), never message (a) — the source does retire the name, just
/// not in the slot it must hold.
#[test]
fn baseline_refuses_a_tombstone_at_the_wrong_ordinal() {
    let dir = TempDir::new("gate-misplaced-tombstone");
    let root = package_workspace(&dir, THREE);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the first baseline is written: {stderr}");

    dir.write("cluster.ridl", MISPLACED_TOMBSTONE);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(
        code, 1,
        "a tombstone at the wrong ordinal is still a refusal:\n{stderr}",
    );
    assert!(
        stderr.contains("RIDL-408"),
        "the refusal carries its code:\n{stderr}",
    );
    assert!(
        stderr.contains("not at the ordinal the interaction held"),
        "message (b) is drawn — the source retires the name, just not in place:\n{stderr}",
    );
    assert!(
        !stderr.contains("is gone from the source"),
        "message (a)'s wording must not fire when the source does retire the name:\n{stderr}",
    );
}

/// A tombstone already recorded in the baseline being replaced, dropped from
/// the source, is a refusal too: a tombstone is a permanent reservation
/// (ridl §11), so deleting the `reserved` line does not un-retire the slot.
/// This is message (c), never message (a) — the source never held the
/// interaction live at all here, only its retirement, and the retirement is
/// what was dropped.
#[test]
fn baseline_refuses_a_dropped_tombstone() {
    let dir = TempDir::new("gate-dropped-tombstone");
    let root = package_workspace(&dir, TOMBSTONED_REMOVAL);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the first baseline is written: {stderr}");

    dir.write("cluster.ridl", BARE_REMOVAL);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(
        code, 1,
        "dropping an already-published tombstone is still a refusal:\n{stderr}",
    );
    assert!(
        stderr.contains("RIDL-408"),
        "the refusal carries its code:\n{stderr}",
    );
    assert!(
        stderr.contains("dropped the tombstone"),
        "message (c) is drawn — the baseline being replaced already retired the name:\n{stderr}",
    );
}

/// A published `.ir.json` that cannot be parsed stays fail-closed: it cannot
/// be shown safe to replace, and replacing it would destroy whatever ordinal
/// record it held with no one seeing it — the exact failure the gate exists
/// to prevent (Ruling 18). Republishing over it exits 2, names the remedy,
/// and touches the corrupt file not at all.
#[test]
fn baseline_refuses_to_republish_over_a_corrupt_snapshot() {
    let dir = TempDir::new("gate-corrupt-snapshot");
    let root = package_workspace(&dir, THREE);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the first baseline is written: {stderr}");

    let snapshot = root
        .join(".ridl")
        .join("baseline")
        .join("veh.cluster.ir.json");
    let corrupt = "<<<<<<< HEAD\n";
    std::fs::write(&snapshot, corrupt).expect("overwrite the published snapshot with garbage");

    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(
        code, 2,
        "a published snapshot that cannot be parsed cannot be shown safe to replace:\n{stderr}",
    );
    assert!(
        stderr.contains("restore the file") && stderr.contains("delete it and run `ridl baseline`"),
        "the message names the remedy:\n{stderr}",
    );
    let after =
        std::fs::read_to_string(&snapshot).expect("the corrupt file is still readable, untouched");
    assert_eq!(
        after, corrupt,
        "a refused publication rewrites nothing, corrupt or not",
    );
}
