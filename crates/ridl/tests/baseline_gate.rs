//! Integration tests for the baseline publication gate
//! (docs/archive/2026-09-13-baseline-gate-design.md, general form §6.3): `ridl
//! baseline` refusing to replace a published snapshot when the replacement
//! drops an interaction the snapshot still declares and the source does not
//! retire it with a `reserved` tombstone at its own ordinal, or declares a
//! live interaction under a name the snapshot retires.

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
/// ordinal 2, the slot `doorClosed` held in `THREE`. `diff_interface` in
/// `ridl-diff`'s `walk.rs` emits this as `InteractionRemoved` with a
/// `change.after` carrying the tombstone's own ordinal (3), which is the one
/// `InteractionRemoved` emission that carries an `after` at all — the
/// structural signal the misplaced-tombstone wording is selected on.
const MISPLACED_TOMBSTONE: &str = "package veh.cluster
type DoorState: integer [0..1]
interface VehicleStatus {
  event doorOpened: DoorState @[100ms..1s]
  event doorLocked: DoorState @[100ms..1s]
  reserved doorClosed
}
";

/// `THREE` behind a tombstone for a name no event carries. Removing
/// `doorClosed` outright from this baseline must draw the bare-removal
/// wording: the published IR reserves `legacyTemp`, not `doorClosed`. The
/// tombstone comes first so that a lookup which stops at any tombstone,
/// whatever its name, meets it before the live `doorClosed`.
const THREE_WITH_ANOTHER_TOMBSTONE: &str = "package veh.cluster
type DoorState: integer [0..1]
interface VehicleStatus {
  reserved legacyTemp
  event doorOpened: DoorState @[100ms..1s]
  event doorClosed: DoorState @[100ms..1s]
  event doorLocked: DoorState @[100ms..1s]
}
";

/// `THREE_WITH_ANOTHER_TOMBSTONE` with `doorClosed` deleted outright.
const BARE_REMOVAL_WITH_ANOTHER_TOMBSTONE: &str = "package veh.cluster
type DoorState: integer [0..1]
interface VehicleStatus {
  reserved legacyTemp
  event doorOpened: DoorState @[100ms..1s]
  event doorLocked: DoorState @[100ms..1s]
}
";

/// A sibling interface, declared first, retires `doorClosed`; `VehicleStatus`
/// declares the same name live. The tombstone that decides the wording must
/// be looked up in the interface the diff path names, never in the first one.
const SIBLING_RESERVES_THE_NAME: &str = "package veh.cluster
type DoorState: integer [0..1]
interface Legacy {
  event ping: DoorState @[100ms..1s]
  reserved doorClosed
}
interface VehicleStatus {
  event doorOpened: DoorState @[100ms..1s]
  event doorClosed: DoorState @[100ms..1s]
  event doorLocked: DoorState @[100ms..1s]
}
";

/// `SIBLING_RESERVES_THE_NAME` with `doorClosed` deleted from `VehicleStatus`
/// outright; `Legacy` is unchanged.
const SIBLING_RESERVES_THE_NAME_REMOVED: &str = "package veh.cluster
type DoorState: integer [0..1]
interface Legacy {
  event ping: DoorState @[100ms..1s]
  reserved doorClosed
}
interface VehicleStatus {
  event doorOpened: DoorState @[100ms..1s]
  event doorLocked: DoorState @[100ms..1s]
}
";

/// An inline-form service whose body retires `doorClosed` at ordinal 2. Its
/// shape is reached through `Package::shapes`, not `Package::interfaces`.
const INLINE_SERVICE_TOMBSTONED: &str = "package veh.cluster
type DoorState: integer [0..1]
service veh.cluster.doors {
  event doorOpened: DoorState @[100ms..1s]
  reserved doorClosed
  event doorLocked: DoorState @[100ms..1s]
}
";

/// `INLINE_SERVICE_TOMBSTONED` with the tombstone dropped.
const INLINE_SERVICE_TOMBSTONE_DROPPED: &str = "package veh.cluster
type DoorState: integer [0..1]
service veh.cluster.doors {
  event doorOpened: DoorState @[100ms..1s]
  event doorLocked: DoorState @[100ms..1s]
}
";

/// `doorClosed` declared live again, at the end, in a source whose baseline
/// (`TOMBSTONED_REMOVAL`) retires that name at ordinal 2. `diff_interface`
/// emits this as `ReservedNameRedeclared`, not `InteractionRemoved`: the
/// name is live on the new side, so the tombstone loop skips it.
const TOMBSTONE_REDECLARED: &str = "package veh.cluster
type DoorState: integer [0..1]
interface VehicleStatus {
  event doorOpened: DoorState @[100ms..1s]
  event doorLocked: DoorState @[100ms..1s]
  event doorClosed: DoorState @[100ms..1s]
}
";

/// `THREE` under the package name `veh.other`, with the manifest to match:
/// a republish of this workspace over a `veh.cluster` baseline writes a
/// fresh snapshot under a new file name and drops the stale one.
const RENAMED_MANIFEST: &str = "[package]\nname = \"veh.other\"\nversion = \"1.0.0\"\n";
const RENAMED_THREE: &str = "package veh.other
type DoorState: integer [0..1]
interface VehicleStatus {
  event doorOpened: DoorState @[100ms..1s]
  event doorClosed: DoorState @[100ms..1s]
  event doorLocked: DoorState @[100ms..1s]
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
        "a bare removal draws the bare-removal wording, not the misplaced- or \
         dropped-tombstone one:\n{stderr}",
    );
    assert_eq!(
        stderr.matches("RIDL-408").count(),
        1,
        "one refusal for the one removed interaction — `doorLocked` sliding into the freed \
         slot is the removal's consequence, not a second refusal:\n{stderr}",
    );
    assert!(
        stderr.contains("┌─") && stderr.contains("interface VehicleStatus {"),
        "the interaction is gone from the source, so the span falls back to the interface's \
         name:\n{stderr}",
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

/// A publication that fails part-way must not leave the output directory
/// holding no snapshot: an empty directory is a first publication to the
/// gate, so the next run would compare against nothing and publish whatever
/// it was handed. The fresh snapshots move in before the stale ones are
/// removed, so the failure here — the fresh snapshot's destination blocked by
/// a directory of the same name, which refuses the rename — leaves the stale
/// `veh.cluster.ir.json` where it was.
#[test]
fn a_failed_publication_keeps_the_stale_snapshot() {
    let dir = TempDir::new("gate-partial-publish");
    let root = package_workspace(&dir, THREE);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the first baseline is written: {stderr}");
    let published = root.join(".ridl").join("baseline");

    dir.write("ridl.toml", RENAMED_MANIFEST);
    dir.write("cluster.ridl", RENAMED_THREE);
    std::fs::create_dir_all(published.join("veh.other.ir.json"))
        .expect("block the fresh snapshot's destination with a directory");

    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(
        code, 2,
        "a snapshot that cannot be moved into place is a tool failure:\n{stderr}",
    );
    assert!(
        stderr.contains("cannot publish the baseline"),
        "the failure is reported:\n{stderr}",
    );
    assert!(
        published.join("veh.cluster.ir.json").is_file(),
        "the stale snapshot survives a publication that failed before it was replaced; \
         without it the next run reads an empty directory as a first publication",
    );
    assert!(
        !root.join(".ridl").join(".baseline.staging").exists(),
        "the staging directory is removed after the failure",
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
/// distinct from the first: an *existing, empty* `.ridl/baseline/` passes
/// `untombstoned_removals`'s `out_dir.is_dir()` check, which the test above
/// never reaches because it never creates the directory at all. This pins
/// that the empty-but-present shape publishes into the directory that is
/// there, at exit 0 and with no diagnostic. It does not pin the
/// `published.is_empty()` early return on that path: a diff of an empty
/// snapshot set against the fresh one carries no refused change either, so
/// deleting the early return would change nothing this test can see.
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
    assert_eq!(
        stderr.matches("RIDL-408").count(),
        2,
        "two refusals for two removed interactions, and no third for `doorAjar` sliding:\n\
         {stderr}",
    );
}

/// A tombstone that retires the right name at the wrong ordinal is still a
/// refusal: `reserved doorClosed` in `MISPLACED_TOMBSTONE` holds ordinal 3,
/// not the ordinal 2 `doorClosed` held in the published baseline, so the
/// surviving interactions would slide into the freed slot (ridl §11). This
/// draws the misplaced-tombstone wording, never the bare-removal one — the
/// source does retire the name, just not in the slot it must hold.
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
    assert_eq!(
        stderr.matches("RIDL-408").count(),
        1,
        "one refusal for the one misplaced tombstone; `doorLocked` moving up is its \
         consequence, not a second refusal:\n{stderr}",
    );
    assert!(
        stderr.contains("not at the ordinal the interaction held (ordinal 2)")
            && stderr.contains("Move `reserved doorClosed` to ordinal 2"),
        "the misplaced-tombstone wording is drawn and names the ordinal to move the \
         tombstone to:\n{stderr}",
    );
    assert!(
        !stderr.contains("is gone from the source"),
        "the bare-removal wording must not fire when the source does retire the name:\n{stderr}",
    );
    assert!(
        stderr.contains("┌─") && stderr.contains("reserved doorClosed"),
        "the tombstone is in the source, so the span points at it:\n{stderr}",
    );
}

/// A tombstone already recorded in the baseline being replaced, dropped from
/// the source, is a refusal too: a tombstone is a permanent reservation
/// (ridl §11), so deleting the `reserved` line does not un-retire the slot.
/// This draws the dropped-tombstone wording, never the bare-removal one — the
/// source never held the interaction live at all here, only its retirement,
/// and the retirement is what was dropped.
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
    assert_eq!(
        stderr.matches("RIDL-408").count(),
        1,
        "one refusal for the one dropped tombstone:\n{stderr}",
    );
    assert!(
        stderr.contains("records `doorClosed` in `VehicleStatus` as retired (ordinal 2)")
            && stderr.contains("dropped the tombstone")
            && stderr.contains("Put `reserved doorClosed` back at ordinal 2"),
        "the dropped-tombstone wording is drawn, naming the interface and the ordinal the \
         tombstone held:\n{stderr}",
    );
    assert!(
        !stderr.contains("is gone from the source"),
        "the bare-removal wording must not fire when the baseline already retired the \
         name:\n{stderr}",
    );
    assert!(
        stderr.contains("┌─") && stderr.contains("interface VehicleStatus {"),
        "the tombstone is gone from the source, so the span falls back to the interface's \
         name:\n{stderr}",
    );
}

/// The wording is decided by asking the published IR whether it retires *this*
/// name in *this* interface, so a tombstone for another name in the same body
/// must not turn a bare removal into a dropped tombstone.
#[test]
fn a_tombstone_for_another_name_does_not_change_the_wording() {
    let dir = TempDir::new("gate-other-tombstone");
    let root = package_workspace(&dir, THREE_WITH_ANOTHER_TOMBSTONE);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the first baseline is written: {stderr}");

    dir.write("cluster.ridl", BARE_REMOVAL_WITH_ANOTHER_TOMBSTONE);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(code, 1, "the bare removal is refused:\n{stderr}");
    assert!(
        stderr.contains("`doorClosed` is gone from the source") && stderr.contains("(ordinal 3)"),
        "the published IR reserves `legacyTemp`, not `doorClosed`, so this is a bare removal \
         of the interaction at ordinal 3:\n{stderr}",
    );
    assert!(
        !stderr.contains("dropped the tombstone"),
        "another name's tombstone is not this name's:\n{stderr}",
    );
}

/// The same lookup must read the interface the diff path names: a sibling
/// interface that happens to retire the same name, declared first, is not the
/// one the removal happened in.
#[test]
fn a_sibling_interface_reserving_the_name_does_not_change_the_wording() {
    let dir = TempDir::new("gate-sibling-tombstone");
    let root = package_workspace(&dir, SIBLING_RESERVES_THE_NAME);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the first baseline is written: {stderr}");

    dir.write("cluster.ridl", SIBLING_RESERVES_THE_NAME_REMOVED);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(code, 1, "the bare removal is refused:\n{stderr}");
    assert_eq!(
        stderr.matches("RIDL-408").count(),
        1,
        "`Legacy` is unchanged, so only `VehicleStatus` draws a refusal:\n{stderr}",
    );
    assert!(
        stderr.contains("`doorClosed` is gone from the source")
            && stderr.contains("in `VehicleStatus`"),
        "`VehicleStatus` never retired `doorClosed`; `Legacy` did, and it is not the \
         interface the removal happened in:\n{stderr}",
    );
    assert!(
        !stderr.contains("dropped the tombstone"),
        "a sibling's tombstone is not this interface's:\n{stderr}",
    );
}

/// An inline-form service's body is an interface shape reached through
/// `Package::shapes`, not `Package::interfaces`. A tombstone dropped from it
/// draws the dropped-tombstone wording exactly as one dropped from a declared
/// interface does.
#[test]
fn baseline_refuses_a_dropped_tombstone_in_an_inline_service() {
    let dir = TempDir::new("gate-inline-dropped-tombstone");
    let root = package_workspace(&dir, INLINE_SERVICE_TOMBSTONED);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the first baseline is written: {stderr}");

    dir.write("cluster.ridl", INLINE_SERVICE_TOMBSTONE_DROPPED);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(
        code, 1,
        "dropping a tombstone from an inline body is still a refusal:\n{stderr}",
    );
    assert!(
        stderr.contains("records `doorClosed` in `veh.cluster.doors` as retired (ordinal 2)")
            && stderr.contains("dropped the tombstone"),
        "the inline body's tombstone is found under the service's dotted name:\n{stderr}",
    );
}

/// An interaction appended at the end of the body is the sanctioned way to
/// grow an interface (ridl §11): no ordinal moves and nothing is dropped, so
/// the gate must admit it (baseline gate design §4).
#[test]
fn baseline_publishes_an_append() {
    let dir = TempDir::new("gate-admits-append");
    let root = package_workspace(&dir, THREE);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the first baseline is written: {stderr}");

    let snapshot = root
        .join(".ridl")
        .join("baseline")
        .join("veh.cluster.ir.json");
    let before = std::fs::read(&snapshot).expect("the first published snapshot is readable");

    dir.write("cluster.ridl", FOUR);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(code, 0, "an append publishes:\n{stderr}");
    assert!(
        !stderr.contains("RIDL-408"),
        "an append drops nothing, so it draws no refusal:\n{stderr}",
    );
    let after = std::fs::read(&snapshot).expect("the republished snapshot is readable");
    assert_ne!(
        before, after,
        "the append is published: the snapshot must now carry `doorAjar`",
    );
}

/// A tombstone that already exists and moves is `InteractionReordered`, not
/// `InteractionRemoved`, and the gate publishes it: `ridl diff` reports the
/// move as breaking and RIDL-407 warns about it at desk time, but whether
/// publication should refuse it is open (ridl §17.12). This pins the current
/// behaviour, so that widening the refusal to `InteractionReordered` is a
/// deliberate change to a recorded open question rather than an accident.
#[test]
fn baseline_publishes_a_moved_existing_tombstone() {
    let dir = TempDir::new("gate-moved-tombstone");
    let root = package_workspace(&dir, TOMBSTONED_REMOVAL);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the first baseline is written: {stderr}");

    dir.write("cluster.ridl", MISPLACED_TOMBSTONE);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(
        code, 0,
        "a moved existing tombstone publishes today (ridl §17.12 records the question):\n\
         {stderr}",
    );
    assert!(
        !stderr.contains("RIDL-408"),
        "the gate reads no `InteractionReordered` change:\n{stderr}",
    );
}

/// The gate compares against the directory publication is about to replace,
/// which is `--out` when given — never the default `.ridl/baseline/`. A gate
/// reading the default directory would compare against nothing for every
/// `--out` user and publish the removal as a first publication.
#[test]
fn the_gate_reads_the_out_directory() {
    let dir = TempDir::new("gate-out");
    let root = package_workspace(&dir, THREE);
    let out = dir.path().join("published");
    let (code, _, stderr) = ridl(&[
        "baseline".as_ref(),
        root.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    assert_eq!(
        code, 0,
        "the first baseline is written to `--out`: {stderr}"
    );

    dir.write("cluster.ridl", BARE_REMOVAL);
    let (code, _, stderr) = ridl(&[
        "baseline".as_ref(),
        root.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    assert_eq!(
        code, 1,
        "the removal is refused against the snapshot in `--out`:\n{stderr}",
    );
    assert!(
        stderr.contains("RIDL-408"),
        "the refusal carries its code:\n{stderr}",
    );
    assert!(
        !root.join(".ridl").exists(),
        "the default directory is neither read nor created when `--out` is given",
    );
}

/// A live interaction declared under a name the published baseline retires
/// is a refusal: the tombstone is a permanent reservation (ridl §11), and a
/// consumer holding the old contract would read the new interaction as the
/// retired one. `ridl_diff` reports this as `ReservedNameRedeclared`, not
/// `InteractionRemoved`, so a gate reading removals alone published it.
#[test]
fn baseline_refuses_an_interaction_redeclared_over_its_tombstone() {
    let dir = TempDir::new("gate-redeclared-tombstone");
    let root = package_workspace(&dir, TOMBSTONED_REMOVAL);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the first baseline is written: {stderr}");

    dir.write("cluster.ridl", TOMBSTONE_REDECLARED);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(
        code, 1,
        "redeclaring a retired name is still a refusal:\n{stderr}",
    );
    assert_eq!(
        stderr.matches("RIDL-408").count(),
        1,
        "one refusal for the one redeclared name:\n{stderr}",
    );
    assert!(
        stderr.contains("`doorClosed` is declared again in `VehicleStatus`")
            && stderr.contains("keep `reserved doorClosed` at ordinal 2"),
        "the redeclared-name wording names the interaction, the interface and the tombstone's \
         ordinal:\n{stderr}",
    );
    assert!(
        stderr.contains("┌─") && stderr.contains("event doorClosed: DoorState @[100ms..1s]"),
        "the span points at the redeclaration, which is in the source:\n{stderr}",
    );
}

/// A published `.ir.json` that cannot be parsed stays fail-closed: it cannot
/// be shown safe to replace, and replacing it would destroy whatever ordinal
/// record it held without any report — the exact failure the gate exists to
/// prevent (baseline gate design §3, "a published snapshot that cannot be
/// parsed", with the remedy amended under D-5). Republishing over it exits 2,
/// names the remedy, touches the corrupt file not at all, and removes the
/// staging directory the compile wrote before the gate ran.
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
        stderr.contains("restore it from version control")
            && stderr.contains("check the source against it with that toolchain"),
        "the message names a remedy for each cause it cannot tell apart — a damaged file, and \
         a snapshot another toolchain wrote:\n{stderr}",
    );
    assert!(
        !stderr.contains("delete it"),
        "the remedy never tells the author to discard the record unread:\n{stderr}",
    );
    let after =
        std::fs::read_to_string(&snapshot).expect("the corrupt file is still readable, untouched");
    assert_eq!(
        after, corrupt,
        "a refused publication rewrites nothing, corrupt or not",
    );
    let staging = root.join(".ridl").join(".baseline.staging");
    assert!(
        !staging.exists(),
        "an exit-2 refusal removes the staging directory the compile wrote, exactly as an \
         exit-1 refusal does: {}",
        staging.display(),
    );
}
