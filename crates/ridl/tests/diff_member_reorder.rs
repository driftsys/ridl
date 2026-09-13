//! Integration tests for the composite body reorder category
//! (docs/wip/2026-09-13-baseline-gate-design.md, driftsys/ridl#314): `ridl
//! diff` reporting a swapped struct field as `member_reordered` rather than a
//! whole-container `constraint_changed`.

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

fn workspace(dir: &TempDir, relative: &str, source: &str) -> PathBuf {
    dir.write(&format!("{relative}/ridl.toml"), MANIFEST);
    dir.write(&format!("{relative}/report.ridl"), source);
    dir.path().join(relative)
}

const FIELDS: &str = "package veh.cluster
type DoorState: integer [0..1]
type Count: integer [0..10]
struct Report {
  door: DoorState
  latch: DoorState
}
";

/// The two fields swap places. Names and types are untouched.
const SWAPPED: &str = "package veh.cluster
type DoorState: integer [0..1]
type Count: integer [0..10]
struct Report {
  latch: DoorState
  door: DoorState
}
";

/// `latch` changes type in place. The names and their order are untouched, so
/// this is the branch `ConstraintChanged` must keep.
const CHANGED_IN_PLACE: &str = "package veh.cluster
type DoorState: integer [0..1]
type Count: integer [0..10]
struct Report {
  door: DoorState
  latch: Count
}
";

/// A body gives one order and that order is wire identity, so a swap is as
/// much a wire change as an interaction reorder and deserves the same kind of
/// category. Reporting it as `constraint_changed` sends the reader looking for
/// a constraint that did not change (driftsys/ridl#314).
#[test]
fn a_swapped_struct_field_reports_member_reordered() {
    let dir = TempDir::new("reorder");
    let old = workspace(&dir, "old", FIELDS);
    let new = workspace(&dir, "new", SWAPPED);

    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), new.as_os_str()]);
    let out = format!("{stdout}{stderr}");

    assert_eq!(code, 1, "a reorder is a wire break:\n{out}");
    assert!(
        out.contains("member_reordered"),
        "the reorder has its own category:\n{out}",
    );
    assert!(
        !out.contains("constraint_changed"),
        "the fallback no longer stands in for it:\n{out}",
    );
    assert!(
        out.contains("door") && out.contains("latch"),
        "both moved members are named:\n{out}",
    );
}

/// A member changed in place, with the order untouched, is still
/// `constraint_changed`. This pins the branch the new category must not take
/// over — asserting only the absence of `member_reordered` would pass even if
/// the whole branch stopped reporting anything.
#[test]
fn an_in_place_change_still_reports_constraint_changed() {
    let dir = TempDir::new("in-place");
    let old = workspace(&dir, "old", FIELDS);
    let new = workspace(&dir, "new", CHANGED_IN_PLACE);

    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), new.as_os_str()]);
    let out = format!("{stdout}{stderr}");

    assert_eq!(code, 1, "narrowing a member in place is breaking:\n{out}");
    assert!(
        out.contains("constraint_changed"),
        "an in-place change keeps the category it always had:\n{out}",
    );
    assert!(
        !out.contains("member_reordered"),
        "nothing moved, so nothing reorders:\n{out}",
    );
}

/// Three fields. `latch` sits second on both sides.
const THREE_FIELDS: &str = "package veh.cluster
type DoorState: integer [0..1]
type Count: integer [0..10]
struct Report {
  door: DoorState
  latch: DoorState
  window: DoorState
}
";

/// The outer two fields swap places; `latch` keeps its position.
const OUTER_SWAPPED: &str = "package veh.cluster
type DoorState: integer [0..1]
type Count: integer [0..10]
struct Report {
  window: DoorState
  latch: DoorState
  door: DoorState
}
";

/// `door` and `latch` swap places and, in the same edit, `window` changes type
/// in place.
const SWAPPED_AND_RETYPED: &str = "package veh.cluster
type DoorState: integer [0..1]
type Count: integer [0..10]
struct Report {
  latch: DoorState
  door: DoorState
  window: Count
}
";

const GEARS: &str = "package veh.cluster
enum GearPosition {
  PARK = 0
  DRIVE = 1
  REVERSE = 2
}
";

/// `PARK` and `DRIVE` swap places in the text. Every value is unchanged.
const GEARS_REORDERED: &str = "package veh.cluster
enum GearPosition {
  DRIVE = 1
  PARK = 0
  REVERSE = 2
}
";

/// `PARK` and `DRIVE` swap places in the text and, in the same edit, `REVERSE`
/// takes a new value while keeping its position.
const GEARS_REORDERED_AND_REVALUED: &str = "package veh.cluster
enum GearPosition {
  DRIVE = 1
  PARK = 0
  REVERSE = 5
}
";

/// One `member_reordered` per member that moved, and none for a member that
/// kept its position. The swap test cannot pin this: both of its members move,
/// so a loop reporting every member would pass it.
#[test]
fn only_the_members_that_moved_are_reported() {
    let dir = TempDir::new("outer-swap");
    let old = workspace(&dir, "old", THREE_FIELDS);
    let new = workspace(&dir, "new", OUTER_SWAPPED);

    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), new.as_os_str()]);
    let out = format!("{stdout}{stderr}");

    assert_eq!(code, 1, "a reorder is a wire break:\n{out}");
    assert!(
        out.contains("member_reordered veh.cluster/Report/door")
            && out.contains("member_reordered veh.cluster/Report/window"),
        "both moved members are reported:\n{out}",
    );
    assert!(
        !out.contains("member_reordered veh.cluster/Report/latch"),
        "a member that kept its position is not reported:\n{out}",
    );
}

/// A reorder that arrives in the same edit as an in-place change reports both:
/// the moved members as `member_reordered`, and the container as
/// `constraint_changed` for the content that changed. Reporting the reorder
/// alone would hide the retype.
#[test]
fn a_reorder_with_an_in_place_change_reports_both() {
    let dir = TempDir::new("reorder-and-retype");
    let old = workspace(&dir, "old", THREE_FIELDS);
    let new = workspace(&dir, "new", SWAPPED_AND_RETYPED);

    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), new.as_os_str()]);
    let out = format!("{stdout}{stderr}");

    assert_eq!(code, 1, "a reorder is a wire break:\n{out}");
    assert!(
        out.contains("member_reordered veh.cluster/Report/door")
            && out.contains("member_reordered veh.cluster/Report/latch"),
        "both moved members are reported:\n{out}",
    );
    assert!(
        out.contains("constraint_changed veh.cluster/Report\n"),
        "the in-place retype is reported on the container as well:\n{out}",
    );
}

/// An enum value's number is explicit content, not a position. A reorder that
/// also changes a value reports the container as `constraint_changed`.
#[test]
fn an_enum_reorder_with_a_changed_value_reports_constraint_changed() {
    let dir = TempDir::new("enum-reorder-revalue");
    let old = workspace(&dir, "old", GEARS);
    let new = workspace(&dir, "new", GEARS_REORDERED_AND_REVALUED);

    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), new.as_os_str()]);
    let out = format!("{stdout}{stderr}");

    assert_eq!(code, 1, "a changed enum value is a wire break:\n{out}");
    assert!(
        out.contains("constraint_changed veh.cluster/GearPosition\n"),
        "the changed value is reported on the container:\n{out}",
    );
}

/// A textual reorder of an enum with every value unchanged is reported as
/// `member_reordered` — conservatively, because the walk compares positions,
/// not the explicit values — and nothing else: the values did not change, so
/// no `constraint_changed` is reported.
#[test]
fn an_enum_reorder_with_unchanged_values_reports_only_member_reordered() {
    let dir = TempDir::new("enum-reorder");
    let old = workspace(&dir, "old", GEARS);
    let new = workspace(&dir, "new", GEARS_REORDERED);

    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), new.as_os_str()]);
    let out = format!("{stdout}{stderr}");

    assert_eq!(code, 1, "a reorder is reported breaking:\n{out}");
    assert!(
        out.contains("member_reordered veh.cluster/GearPosition/PARK")
            && out.contains("member_reordered veh.cluster/GearPosition/DRIVE"),
        "both moved values are reported:\n{out}",
    );
    assert!(
        !out.contains("constraint_changed"),
        "no value changed, so nothing is reported on the container:\n{out}",
    );
}

/// A tombstone ahead of both fields: `door` is ordinal 2 and `latch` ordinal 3.
const TOMBSTONE_FIRST: &str = "package veh.cluster
type DoorState: integer [0..1]
type Count: integer [0..10]
struct Report {
  reserved gone
  door: DoorState
  latch: DoorState
}
";

/// The tombstone moves between the fields. The live names keep their order,
/// yet `door` is now ordinal 1.
const TOMBSTONE_MOVED: &str = "package veh.cluster
type DoorState: integer [0..1]
type Count: integer [0..10]
struct Report {
  door: DoorState
  reserved gone
  latch: DoorState
}
";

const GEARS_WITH_RESERVED: &str = "package veh.cluster
enum GearPosition {
  PARK = 0
  DRIVE = 1
  reserved 3
  reserved 4
}
";

/// The two tombstones swap places in the text. Every value and every retired
/// number is unchanged.
const GEARS_RESERVED_REORDERED: &str = "package veh.cluster
enum GearPosition {
  PARK = 0
  DRIVE = 1
  reserved 4
  reserved 3
}
";

/// A tombstone that moves shifts the ordinal of a live field even though the
/// live names keep their order. The order-insensitive comparison clears every
/// ordinal, so it alone would read the two bodies as equal: the report must
/// still come out breaking, never `identical` with exit 0.
#[test]
fn a_moved_struct_tombstone_is_not_identical() {
    let dir = TempDir::new("tombstone-moved");
    let old = workspace(&dir, "old", TOMBSTONE_FIRST);
    let new = workspace(&dir, "new", TOMBSTONE_MOVED);

    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), new.as_os_str()]);
    let out = format!("{stdout}{stderr}");

    assert_eq!(code, 1, "a shifted field ordinal is a wire break:\n{out}");
    assert!(
        stdout.starts_with("breaking"),
        "the report verdict is breaking, not identical:\n{out}",
    );
}

/// The order of an enum's tombstones is removed by the order-insensitive
/// comparison, so that comparison alone would read a reordered reserved list
/// as equal. The bodies differ, and the walk does not read the reserved list
/// as identities, so the report is conservative: breaking, never `identical`
/// with exit 0.
#[test]
fn a_reordered_enum_reserved_list_is_not_identical() {
    let dir = TempDir::new("enum-reserved-reordered");
    let old = workspace(&dir, "old", GEARS_WITH_RESERVED);
    let new = workspace(&dir, "new", GEARS_RESERVED_REORDERED);

    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), new.as_os_str()]);
    let out = format!("{stdout}{stderr}");

    assert_eq!(code, 1, "a changed reserved list is reported breaking:\n{out}");
    assert!(
        stdout.starts_with("breaking"),
        "the report verdict is breaking, not identical:\n{out}",
    );
}
