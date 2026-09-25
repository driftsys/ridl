//! Integration tests for the baseline publication gate
//! (docs/archive/2026-09-13-baseline-gate-design.md, general form §6.3): `ridl
//! baseline` refusing to replace a published snapshot when the replacement
//! drops an interaction the snapshot still declares and the source does not
//! retire it with a `reserved` tombstone at its own ordinal, or declares a
//! live interaction under a name the snapshot retires (RIDL-408); and the
//! interface level of the same gate, the lock's (lock design §8): a
//! provisional interface number is refused (RIDL-411), and so is a published
//! number the fresh snapshot neither carries nor retires (RIDL-412).

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

/// Lays out a one-package workspace holding `source`, records its interface
/// numbers with plain `ridl lock` — `ridl baseline` refuses a provisional
/// number (RIDL-411), so a fixture that publishes needs its lock first — and
/// returns its root.
fn package_workspace(dir: &TempDir, source: &str) -> PathBuf {
    dir.write("ridl.toml", MANIFEST);
    dir.write("cluster.ridl", source);
    let root = dir.path().to_path_buf();
    let (code, _, stderr) = ridl(&["lock".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the fixture's lock is allocated: {stderr}");
    root
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

/// `THREE` plus a second interface, `Lights`, whose lock entry the
/// interface-level tests allocate, delete by hand, or retire. Plain
/// `ridl lock` numbers a fresh workspace in byte order of the name:
/// `Lights 1`, `VehicleStatus 2`, `next 3`.
const THREE_AND_LIGHTS: &str = "package veh.cluster
type DoorState: integer [0..1]
interface VehicleStatus {
  event doorOpened: DoorState @[100ms..1s]
  event doorClosed: DoorState @[100ms..1s]
  event doorLocked: DoorState @[100ms..1s]
}
interface Lights {
  event lampOn: DoorState @[100ms..1s]
}
";

const LOCK_HEADER: &str = "# interfaces.lock — written by ridl lock; do not edit by hand.\n";

/// `THREE_AND_LIGHTS`'s lock with the `Lights 1` line deleted by hand: `next`
/// is not lowered, so the number is not reused.
const LOCK_WITHOUT_LIGHTS: &str = "next 3\nVehicleStatus 2\n";

/// `VehicleStatus` behind a service, so an rsdl `component` can `offer` it.
const SERVICE_SOURCE: &str = "package veh.cluster
type DoorState: integer [0..1]
interface VehicleStatus {
  event doorOpened: DoorState @[100ms..1s]
}
service veh.cluster.status : VehicleStatus
";

/// Two components, fully placed: `Cluster` offers `veh.cluster.status`,
/// `Panel` requires `VehicleStatus`, and both are placed on a machine of
/// `Desk` (rsdl reference §9).
const PLACED_TOPOLOGY: &str = "package veh.cluster
component Cluster { offers veh.cluster.status }
component Panel { requires VehicleStatus }
system Vehicle { Cluster, Panel }
deployment Desk for Vehicle {
  machine Top { Cluster }
  machine Front { Panel }
}
";

/// `PLACED_TOPOLOGY` with `Panel`'s machine line removed: `Panel.Unit` is
/// placed on no machine of `Desk`, RSDL-701 (rsdl reference §9).
const UNPLACED_TOPOLOGY: &str = "package veh.cluster
component Cluster { offers veh.cluster.status }
component Panel { requires VehicleStatus }
system Vehicle { Cluster, Panel }
deployment Desk for Vehicle {
  machine Top { Cluster }
}
";

/// The published snapshot of `veh.cluster` under `<root>/.ridl/baseline`.
fn snapshot(root: &Path) -> PathBuf {
    root.join(".ridl")
        .join("baseline")
        .join("veh.cluster.ir.json")
}

/// Publishes the workspace at `root` and asserts the publication went
/// through.
fn publish(root: &Path) {
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the baseline is written: {stderr}");
}

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

/// P-B8 made the staging cleanup on an RSDL-7xx-only run load-bearing: such a
/// run writes every artifact (ADR-0022) — including the package staging
/// otherwise holds nothing to publish over — so `run_baseline` must still
/// publish nothing and discard the staging directory, exactly as a refused
/// publication does above.
#[test]
fn baseline_discards_staging_on_an_rsdl_error() {
    let dir = TempDir::new("rsdl-staging");
    dir.write("ridl.toml", MANIFEST);
    dir.write("cluster.ridl", SERVICE_SOURCE);
    dir.write("topology.rsdl", PLACED_TOPOLOGY);
    let root = dir.path().to_path_buf();
    let (code, _, stderr) = ridl(&["lock".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the fixture's lock is allocated: {stderr}");
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the first baseline is written: {stderr}");

    let snapshot = root
        .join(".ridl")
        .join("baseline")
        .join("veh.cluster.ir.json");
    let before = std::fs::read(&snapshot).expect("the published snapshot is readable");

    dir.write("topology.rsdl", UNPLACED_TOPOLOGY);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(
        code, 1,
        "an RSDL-7xx error is a negative answer, not a tool failure:\n{stderr}"
    );
    assert!(
        stderr.contains("RSDL-701"),
        "the placement error is reported:\n{stderr}"
    );
    let after = std::fs::read(&snapshot).expect("the published snapshot survives the error");
    assert_eq!(
        before, after,
        "an RSDL-7xx-only run publishes nothing, even though it writes every artifact",
    );
    let staging = root.join(".ridl").join(".baseline.staging");
    assert!(
        !staging.exists(),
        "the staging directory the RSDL-7xx run wrote into is discarded, not left: {}",
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

// --- The interface level: the lock's publication rules (lock design §8) -----

/// Lock design §4, row 1, the `ridl baseline` column: a declaration with no
/// lock entry compiles with a provisional number, and publication refuses it
/// — RIDL-411, exit 1, nothing written, the span at the declaration — until
/// plain `ridl lock` records the number, after which the same source
/// publishes.
#[test]
fn baseline_refuses_a_provisional_number() {
    let dir = TempDir::new("gate-provisional");
    let root = package_workspace(&dir, THREE);
    dir.write("cluster.ridl", THREE_AND_LIGHTS);

    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(code, 1, "a provisional number is refused:\n{stderr}");
    assert!(
        stderr.contains("error[RIDL-411]"),
        "the refusal carries its code:\n{stderr}",
    );
    assert!(
        stderr.contains("`Lights` has a provisional interface number (2)"),
        "the message names the interface and the number the checker showed:\n{stderr}",
    );
    assert!(
        stderr.contains(&format!("Run `ridl lock {}`", root.display())),
        "the message names the command that records the number:\n{stderr}",
    );
    assert!(
        stderr.contains("┌─") && stderr.contains("interface Lights"),
        "the span points at the declaration:\n{stderr}",
    );
    assert!(
        !stderr.contains("RIDL-408"),
        "the interaction level has nothing to refuse:\n{stderr}",
    );
    assert!(
        !root.join(".ridl").join("baseline").exists(),
        "nothing is published",
    );
    assert!(
        !root.join(".ridl").join(".baseline.staging").exists(),
        "the staging directory is removed",
    );

    let (code, _, stderr) = ridl(&["lock".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the number is recorded: {stderr}");
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(
        code, 0,
        "with the number recorded, the same source publishes:\n{stderr}",
    );
    assert!(snapshot(&root).is_file(), "the snapshot is written");
}

/// Lock design §4, row 5: with no baseline published yet, an entry with no
/// declaration beside a declaration with no entry is RIDL-409 at the compile,
/// so `ridl baseline` never reaches its gate; once `ridl lock --rename`
/// records the fix, the first publication has nothing to compare against and
/// goes through with no diagnostic.
#[test]
fn the_first_publication_after_a_lock_fix_compares_nothing() {
    let dir = TempDir::new("gate-first-after-fix");
    dir.write("ridl.toml", MANIFEST);
    dir.write("cluster.ridl", THREE_AND_LIGHTS);
    dir.write(
        "interfaces.lock",
        &format!("{LOCK_HEADER}next 3\nVehicleStatus 1\nLamps 2\n"),
    );
    let root = dir.path().to_path_buf();

    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 1, "the compile fails on the orphan entry:\n{stderr}");
    assert!(
        stderr.contains("error[RIDL-409]"),
        "the compile's diagnostic:\n{stderr}",
    );
    assert!(
        !stderr.contains("RIDL-41"),
        "the gate does not run after a failed compile:\n{stderr}",
    );
    assert!(!root.join(".ridl").exists(), "nothing is published");

    let (code, _, stderr) = ridl(&[
        "lock".as_ref(),
        root.as_os_str(),
        "--rename".as_ref(),
        "Lamps=Lights".as_ref(),
    ]);
    assert_eq!(code, 0, "the rename is recorded: {stderr}");
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the first publication after the fix:\n{stderr}");
    assert!(
        !stderr.contains("RIDL-"),
        "nothing to compare against, nothing provisional:\n{stderr}",
    );
    assert!(snapshot(&root).is_file(), "the snapshot is written");
}

/// Lock design §4, row 6, first phase: a lock line deleted by hand leaves
/// its declaration provisional. The build cannot see that, and publication
/// refuses it with RIDL-411 — the provisional number is taken from `next`,
/// so the deleted number is not reused — leaving the published snapshot
/// byte-identical.
#[test]
fn a_hand_deleted_lock_line_is_ridl_411_while_its_declaration_is_provisional() {
    let dir = TempDir::new("gate-deleted-line-provisional");
    let root = package_workspace(&dir, THREE_AND_LIGHTS);
    publish(&root);
    let before = std::fs::read(snapshot(&root)).expect("the published snapshot is readable");

    dir.write(
        "interfaces.lock",
        &format!("{LOCK_HEADER}{LOCK_WITHOUT_LIGHTS}"),
    );
    let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the build cannot see a deleted line:\n{stderr}");

    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(code, 1, "the provisional number is refused:\n{stderr}");
    assert!(
        stderr.contains("error[RIDL-411]")
            && stderr.contains("`Lights` has a provisional interface number (3)"),
        "the number comes from `next`, not from the deleted line:\n{stderr}",
    );
    let after = std::fs::read(snapshot(&root)).expect("the published snapshot survives");
    assert_eq!(before, after, "a refused publication rewrites nothing");
    assert!(
        !root.join(".ridl").join(".baseline.staging").exists(),
        "the staging directory is removed",
    );
}

/// Lock design §4, row 6, second phase, and §7 row 6 at publication: after
/// plain `ridl lock` gives the kept declaration a fresh number, the number
/// the baseline holds is absent from the fresh side and not retired —
/// RIDL-412, exit 1, the published snapshot byte-identical, the staging
/// directory removed. Nothing here is RIDL-408: the interaction level is
/// untouched.
#[test]
fn baseline_refuses_a_number_dropped_without_a_retired_entry() {
    let dir = TempDir::new("gate-dropped-number");
    let root = package_workspace(&dir, THREE_AND_LIGHTS);
    publish(&root);
    let before = std::fs::read(snapshot(&root)).expect("the published snapshot is readable");

    dir.write(
        "interfaces.lock",
        &format!("{LOCK_HEADER}{LOCK_WITHOUT_LIGHTS}"),
    );
    let (code, stdout, stderr) = ridl(&["lock".as_ref(), root.as_os_str()]);
    assert_eq!(
        code, 0,
        "the kept declaration gets a fresh number: {stderr}"
    );
    assert_eq!(stdout, "allocated Lights 3\n", "`next` was never lowered");

    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(code, 1, "the dropped number is refused:\n{stderr}");
    assert_eq!(
        stderr.matches("RIDL-412").count(),
        1,
        "one refusal for the one dropped number:\n{stderr}",
    );
    assert!(
        stderr.contains("`Lights` holds interface number 1 in the baseline being replaced")
            && stderr.contains("Restore the line `Lights 1`")
            && stderr.contains("`Lights 1 retired` when the interface is gone"),
        "the message names the number and the line that restores its record:\n{stderr}",
    );
    assert!(
        !stderr.contains("RIDL-408") && !stderr.contains("RIDL-411"),
        "the interaction level is untouched and nothing is provisional:\n{stderr}",
    );
    let after = std::fs::read(snapshot(&root)).expect("the published snapshot survives");
    assert_eq!(before, after, "a refused publication rewrites nothing");
    assert!(
        !root.join(".ridl").join(".baseline.staging").exists(),
        "the staging directory is removed",
    );
}

/// Lock design §7, row 5, at publication: an interface removed with its entry
/// retired — `ridl lock --retire` — is the sanctioned removal. It publishes,
/// and the new snapshot records the retired number.
#[test]
fn baseline_publishes_a_retired_number() {
    let dir = TempDir::new("gate-retired-number");
    let root = package_workspace(&dir, THREE_AND_LIGHTS);
    publish(&root);

    dir.write("cluster.ridl", THREE);
    let (code, stdout, stderr) = ridl(&[
        "lock".as_ref(),
        root.as_os_str(),
        "--retire".as_ref(),
        "Lights".as_ref(),
    ]);
    assert_eq!(code, 0, "the retirement is recorded: {stderr}");
    assert_eq!(stdout, "retired Lights 1\n");

    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(code, 0, "a retired number publishes:\n{stderr}");
    assert!(
        !stderr.contains("RIDL-"),
        "the sanctioned removal draws no refusal:\n{stderr}",
    );
    let text = std::fs::read_to_string(snapshot(&root)).expect("the republished snapshot");
    assert!(
        text.contains("\"retired\"") && text.contains("\"name\": \"Lights\""),
        "the snapshot records the retired entry:\n{text}",
    );
}

/// Plan decision PD-9: an interface published before the lock existed
/// carries `number` 0, which is never allocated, so it never had an entry.
/// `ridl diff` matches it by name and reports its removal as breaking, and
/// RIDL-412 does not refuse it. The pre-lock snapshot is made by publishing
/// and rewriting the number to 0 — the shape a snapshot from before the lock
/// has.
#[test]
fn a_pre_lock_baseline_is_not_refused_for_an_interface_removal() {
    let dir = TempDir::new("gate-pre-lock");
    let root = package_workspace(&dir, THREE_AND_LIGHTS);
    publish(&root);
    let path = snapshot(&root);
    let text = std::fs::read_to_string(&path).expect("the published snapshot is readable");
    assert!(
        text.contains("\"number\": 1"),
        "`Lights` was published under number 1:\n{text}",
    );
    std::fs::write(&path, text.replace("\"number\": 1", "\"number\": 0"))
        .expect("rewrite the snapshot as a pre-lock one");

    dir.write("cluster.ridl", THREE);
    dir.write(
        "interfaces.lock",
        &format!("{LOCK_HEADER}{LOCK_WITHOUT_LIGHTS}"),
    );
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(
        code, 0,
        "a number the lock never allocated has no record to lose:\n{stderr}",
    );
    assert!(
        !stderr.contains("RIDL-412") && !stderr.contains("RIDL-408"),
        "neither gate refuses:\n{stderr}",
    );
}

/// The interface level is outside the RIDL-408 gate — the claim the retired
/// service-tombstone test used to state. A whole interface removed is
/// refused by the lock's own rules instead: with its entry still live the
/// build fails first (RIDL-409, the compile), so publication never reaches
/// its gate, and only a hand-deleted line reaches RIDL-412.
#[test]
fn a_whole_interface_removed_is_refused_by_the_lock_not_the_tombstone_gate() {
    let dir = TempDir::new("gate-whole-interface");
    let root = package_workspace(&dir, THREE_AND_LIGHTS);
    publish(&root);
    let before = std::fs::read(snapshot(&root)).expect("the published snapshot is readable");

    dir.write("cluster.ridl", THREE);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(code, 1, "the live entry fails the compile:\n{stderr}");
    assert!(
        stderr.contains("error[RIDL-409]"),
        "the build's diagnostic, naming the retire:\n{stderr}",
    );
    assert!(
        !stderr.contains("RIDL-408") && !stderr.contains("RIDL-412"),
        "no gate runs after a failed compile:\n{stderr}",
    );
    let after = std::fs::read(snapshot(&root)).expect("the published snapshot survives");
    assert_eq!(before, after, "a failed compile rewrites nothing");
}

// --- The published side the gate cannot resolve (driftsys/ridl#339) --------

/// A declared `interface doors` beside an inline-form `service doors`: the
/// service name is not dotted, so the two shapes carry the same identity
/// name. The service retires `doorClosed` at ordinal 2; the interface holds
/// no tombstone at all.
const INTERFACE_AND_SERVICE_SHARING_A_NAME: &str = "package veh.cluster
type DoorState: integer [0..1]
interface doors {
  event locked: DoorState @[100ms..1s]
}
service doors {
  event doorOpened: DoorState @[100ms..1s]
  reserved doorClosed
  event doorLocked: DoorState @[100ms..1s]
}
";

/// `INTERFACE_AND_SERVICE_SHARING_A_NAME` with `doorClosed` declared live
/// again in the service, at the end.
const INTERFACE_AND_SERVICE_SHARING_A_NAME_REDECLARED: &str = "package veh.cluster
type DoorState: integer [0..1]
interface doors {
  event locked: DoorState @[100ms..1s]
}
service doors {
  event doorOpened: DoorState @[100ms..1s]
  event doorLocked: DoorState @[100ms..1s]
  event doorClosed: DoorState @[100ms..1s]
}
";

/// `TOMBSTONED_REMOVAL`'s interface renamed to `Status`, with `doorClosed`
/// declared live again at the end. The lock keeps the number under the new
/// key (`ridl lock --rename`), so `ridl_diff` matches the two shapes and
/// emits `ReservedNameRedeclared` on a path carrying the new name — one the
/// published IR does not hold.
const RENAMED_INTERFACE_TOMBSTONE_REDECLARED: &str = "package veh.cluster
type DoorState: integer [0..1]
interface Status {
  event doorOpened: DoorState @[100ms..1s]
  event doorLocked: DoorState @[100ms..1s]
  event doorClosed: DoorState @[100ms..1s]
}
";

/// The mirror of `INTERFACE_AND_SERVICE_SHARING_A_NAME`: the declared
/// `interface doors` holds the tombstone, at ordinal 2, and the inline-form
/// `service doors` beside it holds no tombstone at all.
const INTERFACE_TOMBSTONED_BESIDE_SERVICE_SHARING_ITS_NAME: &str = "package veh.cluster
type DoorState: integer [0..1]
interface doors {
  event doorOpened: DoorState @[100ms..1s]
  reserved doorClosed
  event doorLocked: DoorState @[100ms..1s]
}
service doors {
  event locked: DoorState @[100ms..1s]
}
";

/// `INTERFACE_TOMBSTONED_BESIDE_SERVICE_SHARING_ITS_NAME` with `doorClosed`
/// declared live again in the interface, at the end.
const INTERFACE_TOMBSTONED_BESIDE_SERVICE_SHARING_ITS_NAME_REDECLARED: &str = "package veh.cluster
type DoorState: integer [0..1]
interface doors {
  event doorOpened: DoorState @[100ms..1s]
  event doorLocked: DoorState @[100ms..1s]
  event doorClosed: DoorState @[100ms..1s]
}
service doors {
  event locked: DoorState @[100ms..1s]
}
";

/// `TOMBSTONED_REMOVAL` behind a named-form service: the service carries no
/// body of its own, only a reference to `VehicleStatus`.
const TOMBSTONED_REMOVAL_BEHIND_A_NAMED_SERVICE: &str = "package veh.cluster
type DoorState: integer [0..1]
interface VehicleStatus {
  event doorOpened: DoorState @[100ms..1s]
  reserved doorClosed
  event doorLocked: DoorState @[100ms..1s]
}
service status : VehicleStatus
";

/// `TOMBSTONED_REMOVAL_BEHIND_A_NAMED_SERVICE` with the interface renamed to
/// `status` — the named-form service's own name — and `doorClosed` declared
/// live again at the end. After `ridl lock --rename`, the change's path names
/// a container that is, in the published IR, a named-form service and no
/// shape at all.
const RENAMED_TO_ITS_SERVICE_NAME_TOMBSTONE_REDECLARED: &str = "package veh.cluster
type DoorState: integer [0..1]
interface status {
  event doorOpened: DoorState @[100ms..1s]
  event doorLocked: DoorState @[100ms..1s]
  event doorClosed: DoorState @[100ms..1s]
}
service status : status
";

/// Two published snapshots declaring one package are refused, exit 2, and
/// both are left as they are (driftsys/ridl#339 case 1). `ridl_diff`
/// compares against the one that sorts last while the gate's own lookup read
/// the one that sorts first, so a copy that already lacked `doorClosed` let a
/// bare removal publish — and the publication deleted the copy.
#[test]
fn two_published_snapshots_for_one_package_are_refused_and_left_as_they_are() {
    let dir = TempDir::new("gate-duplicate-package");
    let root = package_workspace(&dir, THREE);
    publish(&root);

    // A same-package snapshot that already lacks `doorClosed`, published to
    // a separate directory as a first publication and copied in beside the
    // real record under a name that sorts after it.
    dir.write("cluster.ridl", BARE_REMOVAL);
    let elsewhere = dir.path().join("elsewhere");
    let (code, _, stderr) = ridl(&[
        "baseline".as_ref(),
        root.as_os_str(),
        "--out".as_ref(),
        elsewhere.as_os_str(),
    ]);
    assert_eq!(code, 0, "the copy is a first publication: {stderr}");
    let copy = snapshot(&root).with_file_name("zz-copy.ir.json");
    std::fs::copy(elsewhere.join("veh.cluster.ir.json"), &copy).expect("copy the snapshot in");

    let published = std::fs::read(snapshot(&root)).expect("the published snapshot is readable");
    let copied = std::fs::read(&copy).expect("the copy is readable");

    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(
        code, 2,
        "a published side that names one package twice cannot be compared against:\n{stderr}",
    );
    assert!(
        stderr.contains("package `veh.cluster`")
            && stderr.contains("veh.cluster.ir.json")
            && stderr.contains("zz-copy.ir.json"),
        "the message names the package and both files:\n{stderr}",
    );
    assert!(
        !stderr.contains("RIDL-408"),
        "the run stops before the gate, which has nothing sound to compare:\n{stderr}",
    );
    let after = std::fs::read(snapshot(&root)).expect("the published snapshot survives");
    assert_eq!(published, after, "the published record is not rewritten");
    let after = std::fs::read(&copy).expect("the copy survives");
    assert_eq!(copied, after, "the copy is neither rewritten nor deleted");
    let staging = root.join(".ridl").join(".baseline.staging");
    assert!(
        !staging.exists(),
        "an exit-2 refusal removes the staging directory: {}",
        staging.display(),
    );
}

/// An interface and an inline-form service sharing a name are two shapes
/// under one identity name. Redeclaring a name the service retires is
/// `ReservedNameRedeclared` on the service's path, and the gate must refuse
/// it by reading the shape that holds the tombstone — not the interface that
/// happens to sort first under the same name (driftsys/ridl#339 case 2).
#[test]
fn a_tombstone_in_a_service_sharing_its_interface_name_still_refuses_the_redeclaration() {
    let dir = TempDir::new("gate-shared-shape-name");
    let root = package_workspace(&dir, INTERFACE_AND_SERVICE_SHARING_A_NAME);
    let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);
    assert_eq!(
        code, 0,
        "an interface and a service may share a name: {stderr}"
    );
    publish(&root);
    let before = std::fs::read(snapshot(&root)).expect("the published snapshot is readable");

    dir.write(
        "cluster.ridl",
        INTERFACE_AND_SERVICE_SHARING_A_NAME_REDECLARED,
    );
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(
        code, 1,
        "redeclaring a name the service retires is a refusal:\n{stderr}",
    );
    assert_eq!(
        stderr.matches("RIDL-408").count(),
        1,
        "one refusal for the one redeclared name:\n{stderr}",
    );
    assert!(
        stderr.contains("`doorClosed` is declared again in `doors`")
            && stderr.contains("keep `reserved doorClosed` at ordinal 2"),
        "the ordinal comes from the service's shape, the one that holds the tombstone:\n\
         {stderr}",
    );
    let after = std::fs::read(snapshot(&root)).expect("the published snapshot survives");
    assert_eq!(before, after, "a refused publication rewrites nothing");
}

/// A snapshot-named entry whose metadata cannot be read — a symlink to a
/// file that is gone — is exit 2, not an absent snapshot (driftsys/ridl#339
/// case 3). Skipping it read the published side as empty, and a bare removal
/// then published as a first publication, replacing the link with a snapshot
/// that had dropped the ordinal record.
#[cfg(unix)]
#[test]
fn a_published_snapshot_that_cannot_be_stated_refuses_the_publication() {
    let dir = TempDir::new("gate-dangling-symlink");
    let root = package_workspace(&dir, THREE);
    publish(&root);

    let path = snapshot(&root);
    std::fs::remove_file(&path).expect("remove the published snapshot");
    let target = PathBuf::from("missing.ir.json");
    std::os::unix::fs::symlink(&target, &path).expect("replace it with a dangling symlink");

    dir.write("cluster.ridl", BARE_REMOVAL);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(
        code, 2,
        "a snapshot entry that cannot be read is not an absent one:\n{stderr}",
    );
    assert!(
        stderr.contains(&path.display().to_string()),
        "the message names the entry it could not read:\n{stderr}",
    );
    assert!(
        !stderr.contains("cannot read the snapshot directory"),
        "the directory itself was listed; the entry is what failed:\n{stderr}",
    );
    let link = std::fs::symlink_metadata(&path).expect("the entry is still there");
    assert!(
        link.file_type().is_symlink(),
        "the symlink is neither replaced nor removed",
    );
    assert_eq!(
        std::fs::read_link(&path).expect("the symlink is readable"),
        target,
        "the symlink still points where it did",
    );
    let staging = root.join(".ridl").join(".baseline.staging");
    assert!(
        !staging.exists(),
        "an exit-2 refusal removes the staging directory: {}",
        staging.display(),
    );
}

/// A `ReservedNameRedeclared` change whose container the published IR cannot
/// resolve is refused, not published: the walk emitted it from a tombstone
/// it read on the published side, so a lookup that finds nothing is the
/// lookup's failure (driftsys/ridl#339 case 2, the fail-closed rule). A
/// renamed interface is such a container — the change's path carries the new
/// name — and the old rule, which refused only when the lookup found the
/// tombstone, published the redeclaration.
#[test]
fn a_redeclaration_in_a_renamed_interface_is_refused_although_the_lookup_finds_nothing() {
    let dir = TempDir::new("gate-renamed-container");
    let root = package_workspace(&dir, TOMBSTONED_REMOVAL);
    publish(&root);
    let before = std::fs::read(snapshot(&root)).expect("the published snapshot is readable");

    dir.write("cluster.ridl", RENAMED_INTERFACE_TOMBSTONE_REDECLARED);
    let (code, _, stderr) = ridl(&[
        "lock".as_ref(),
        root.as_os_str(),
        "--rename".as_ref(),
        "VehicleStatus=Status".as_ref(),
    ]);
    assert_eq!(code, 0, "the rename keeps the number: {stderr}");
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(
        code, 1,
        "a redeclared retired name is refused whether or not the lookup resolves its \
         container:\n{stderr}",
    );
    assert_eq!(
        stderr.matches("RIDL-408").count(),
        1,
        "one refusal for the one redeclared name:\n{stderr}",
    );
    assert!(
        stderr.contains("`doorClosed` is declared again in `Status`")
            && stderr.contains("keep `reserved doorClosed` at that ordinal"),
        "the message names the new container; the ordinal is unknown to a lookup by the new \
         name, so the remedy names no number:\n{stderr}",
    );
    let after = std::fs::read(snapshot(&root)).expect("the published snapshot survives");
    assert_eq!(before, after, "a refused publication rewrites nothing");
}

/// The other direction of the shared-name case: the declared interface holds
/// the tombstone and the same-named inline service does not. The lookup must
/// answer from the shape that declares the name whichever of the two it is,
/// so the message carries the tombstone's ordinal here as well.
#[test]
fn a_tombstone_in_an_interface_sharing_its_service_name_still_names_its_ordinal() {
    let dir = TempDir::new("gate-shared-shape-name-interface");
    let root = package_workspace(&dir, INTERFACE_TOMBSTONED_BESIDE_SERVICE_SHARING_ITS_NAME);
    publish(&root);
    let before = std::fs::read(snapshot(&root)).expect("the published snapshot is readable");

    dir.write(
        "cluster.ridl",
        INTERFACE_TOMBSTONED_BESIDE_SERVICE_SHARING_ITS_NAME_REDECLARED,
    );
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(
        code, 1,
        "redeclaring a name the interface retires is a refusal:\n{stderr}",
    );
    assert_eq!(
        stderr.matches("RIDL-408").count(),
        1,
        "one refusal for the one redeclared name:\n{stderr}",
    );
    assert!(
        stderr.contains("`doorClosed` is declared again in `doors`")
            && stderr.contains("keep `reserved doorClosed` at ordinal 2"),
        "the ordinal comes from the interface's shape, the one that holds the tombstone:\n\
         {stderr}",
    );
    let after = std::fs::read(snapshot(&root)).expect("the published snapshot survives");
    assert_eq!(before, after, "a refused publication rewrites nothing");
}

/// A redeclared retired name is refused whatever the published IR says the
/// container is. The walk emitted the change from a tombstone it read on the
/// published side, so no second lookup can sanction it — and a rule that
/// admitted a container resolving to a named-form service published this
/// case: the interface renamed to its service's own name, so the change's
/// path names a service that is no shape in the published IR.
#[test]
fn a_redeclaration_in_an_interface_renamed_to_its_service_name_is_refused() {
    let dir = TempDir::new("gate-renamed-to-service-name");
    let root = package_workspace(&dir, TOMBSTONED_REMOVAL_BEHIND_A_NAMED_SERVICE);
    publish(&root);
    let before = std::fs::read(snapshot(&root)).expect("the published snapshot is readable");

    dir.write(
        "cluster.ridl",
        RENAMED_TO_ITS_SERVICE_NAME_TOMBSTONE_REDECLARED,
    );
    let (code, _, stderr) = ridl(&[
        "lock".as_ref(),
        root.as_os_str(),
        "--rename".as_ref(),
        "VehicleStatus=status".as_ref(),
    ]);
    assert_eq!(code, 0, "the rename keeps the number: {stderr}");
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(
        code, 1,
        "a redeclared retired name is refused whatever the container resolves to:\n{stderr}",
    );
    assert_eq!(
        stderr.matches("RIDL-408").count(),
        1,
        "one refusal for the one redeclared name:\n{stderr}",
    );
    assert!(
        stderr.contains("`doorClosed` is declared again in `status`"),
        "the message names the new container:\n{stderr}",
    );
    let after = std::fs::read(snapshot(&root)).expect("the published snapshot survives");
    assert_eq!(before, after, "a refused publication rewrites nothing");
}

/// A symlink to a readable snapshot is that snapshot: the metadata read
/// follows the link, so the gate compares against the file it names and
/// refuses an untombstoned removal through it, exactly as through the file
/// itself.
#[cfg(unix)]
#[test]
fn a_symlink_to_a_readable_snapshot_is_read_as_that_snapshot() {
    let dir = TempDir::new("gate-readable-symlink");
    let root = package_workspace(&dir, THREE);
    publish(&root);

    let path = snapshot(&root);
    let elsewhere = root.join(".ridl").join("elsewhere");
    std::fs::create_dir_all(&elsewhere).expect("create the directory the link points into");
    let real = elsewhere.join("veh.cluster.ir.json");
    std::fs::rename(&path, &real).expect("move the published snapshot out of the directory");
    let target = PathBuf::from("../elsewhere/veh.cluster.ir.json");
    std::os::unix::fs::symlink(&target, &path).expect("link to it from the published directory");
    let before = std::fs::read(&real).expect("the snapshot behind the link is readable");

    dir.write("cluster.ridl", BARE_REMOVAL);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(
        code, 1,
        "the removal is refused against the snapshot the link names:\n{stderr}",
    );
    assert!(
        stderr.contains("RIDL-408") && stderr.contains("`doorClosed` is gone from the source"),
        "the gate read the snapshot through the link:\n{stderr}",
    );
    assert!(
        std::fs::symlink_metadata(&path)
            .expect("the entry is still there")
            .file_type()
            .is_symlink()
            && std::fs::read_link(&path).expect("the symlink is readable") == target,
        "the link is left as it is",
    );
    let after = std::fs::read(&real).expect("the snapshot behind the link survives");
    assert_eq!(before, after, "a refused publication rewrites nothing");
}
