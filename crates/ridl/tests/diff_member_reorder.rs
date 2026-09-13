//! Integration tests for the composite body reorder category
//! (driftsys/ridl#314; typl §7.4 in
//! `docs/specification/typl-language-reference.md` for the ordinal rule, and
//! its §17.14 for the enum and enum-set treatment): `ridl diff` reporting a
//! swapped struct field as `member_reordered` rather than a whole-container
//! `constraint_changed`.

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
            "ridl-reorder-{label}-{}-{}",
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
        "the fallback is not reported in its place:\n{out}",
    );
    assert!(
        out.contains("door") && out.contains("latch"),
        "both moved members are named:\n{out}",
    );
}

/// The detail carries the field's old and new ordinal, 1-based, old first: the
/// number typl §7.4 makes the wire identity, so the reader can see which slot
/// the field left and which it took.
#[test]
fn a_moved_field_reports_its_old_and_new_ordinal() {
    let dir = TempDir::new("ordinals");
    let old = workspace(&dir, "old", FIELDS);
    let new = workspace(&dir, "new", SWAPPED);

    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), new.as_os_str()]);
    let out = format!("{stdout}{stderr}");

    assert_eq!(code, 1, "a reorder is a wire break:\n{out}");
    assert!(
        out.contains("member_reordered veh.cluster/Report/door: ordinal 1 -> ordinal 2\n"),
        "door left ordinal 1 for ordinal 2:\n{out}",
    );
    assert!(
        out.contains("member_reordered veh.cluster/Report/latch: ordinal 2 -> ordinal 1\n"),
        "latch left ordinal 2 for ordinal 1:\n{out}",
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
/// `constraint_changed` for the content that changed, after the member lines.
/// Reporting the reorder alone would hide the retype.
#[test]
fn a_reorder_with_an_in_place_change_reports_both() {
    let dir = TempDir::new("reorder-and-retype");
    let old = workspace(&dir, "old", THREE_FIELDS);
    let new = workspace(&dir, "new", SWAPPED_AND_RETYPED);

    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), new.as_os_str()]);
    let out = format!("{stdout}{stderr}");

    assert_eq!(code, 1, "a reorder is a wire break:\n{out}");
    let door = out.find("member_reordered veh.cluster/Report/door");
    let latch = out.find("member_reordered veh.cluster/Report/latch");
    let container = out.find("constraint_changed veh.cluster/Report\n");
    assert!(
        door.is_some() && latch.is_some(),
        "both moved members are reported:\n{out}",
    );
    assert!(
        container.is_some(),
        "the in-place retype is reported on the container as well:\n{out}",
    );
    assert!(
        container > door && container > latch,
        "the container line follows the member lines:\n{out}",
    );
}

/// An enum value's number is explicit content, not a position. A reorder that
/// also changes a value reports the moved values as `member_reordered`, each
/// with its 1-based position in the body, and the container as
/// `constraint_changed`.
#[test]
fn an_enum_reorder_with_a_changed_value_reports_constraint_changed() {
    let dir = TempDir::new("enum-reorder-revalue");
    let old = workspace(&dir, "old", GEARS);
    let new = workspace(&dir, "new", GEARS_REORDERED_AND_REVALUED);

    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), new.as_os_str()]);
    let out = format!("{stdout}{stderr}");

    assert_eq!(code, 1, "a changed enum value is a wire break:\n{out}");
    assert!(
        out.contains("member_reordered veh.cluster/GearPosition/PARK: position 1 -> position 2\n")
            && out.contains(
                "member_reordered veh.cluster/GearPosition/DRIVE: position 2 -> position 1\n"
            ),
        "both moved values are reported with their positions:\n{out}",
    );
    assert!(
        !out.contains("member_reordered veh.cluster/GearPosition/REVERSE"),
        "a value that kept its position is not reported:\n{out}",
    );
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
/// as equal. The bodies differ, no value moved, and the walk does not read the
/// reserved list as identities, so the report is conservative: the container's
/// `constraint_changed`, breaking, never `identical` with exit 0.
#[test]
fn a_reordered_enum_reserved_list_is_not_identical() {
    let dir = TempDir::new("enum-reserved-reordered");
    let old = workspace(&dir, "old", GEARS_WITH_RESERVED);
    let new = workspace(&dir, "new", GEARS_RESERVED_REORDERED);

    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), new.as_os_str()]);
    let out = format!("{stdout}{stderr}");

    assert_eq!(
        code, 1,
        "a changed reserved list is reported breaking:\n{out}"
    );
    assert!(
        stdout.starts_with("breaking"),
        "the report verdict is breaking, not identical:\n{out}",
    );
    assert!(
        out.contains("constraint_changed veh.cluster/GearPosition\n"),
        "the change is reported on the container:\n{out}",
    );
    assert!(!out.contains("member_reordered"), "no value moved:\n{out}",);
}

/// The tombstone moves after both fields: `door` is now ordinal 1 and `latch`
/// ordinal 2.
const TOMBSTONE_LAST: &str = "package veh.cluster
type DoorState: integer [0..1]
type Count: integer [0..10]
struct Report {
  door: DoorState
  latch: DoorState
  reserved gone
}
";

/// The tombstone moves after both fields and the fields swap: `door` keeps
/// ordinal 2, `latch` takes ordinal 1.
const TOMBSTONE_LAST_AND_SWAPPED: &str = "package veh.cluster
type DoorState: integer [0..1]
type Count: integer [0..10]
struct Report {
  latch: DoorState
  door: DoorState
  reserved gone
}
";

/// A tombstone between the second and third field: `window` is ordinal 4.
const TOMBSTONE_BETWEEN: &str = "package veh.cluster
type DoorState: integer [0..1]
type Count: integer [0..10]
struct Report {
  door: DoorState
  latch: DoorState
  reserved gone
  window: DoorState
}
";

/// The first two fields swap and the tombstone moves after `window`, which
/// keeps its place among the live names while its ordinal drops from 4 to 3.
const SWAPPED_AROUND_TOMBSTONE: &str = "package veh.cluster
type DoorState: integer [0..1]
type Count: integer [0..10]
struct Report {
  latch: DoorState
  door: DoorState
  window: DoorState
  reserved gone
}
";

/// A moved tombstone shifts the ordinal of a field without changing the order
/// of the live names. The report names the field and its ordinals; it does not
/// fall back to a bare `constraint_changed` on the container.
#[test]
fn a_moved_struct_tombstone_reports_the_field_it_shifted() {
    let dir = TempDir::new("tombstone-shift");
    let old = workspace(&dir, "old", TOMBSTONE_FIRST);
    let new = workspace(&dir, "new", TOMBSTONE_MOVED);

    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), new.as_os_str()]);
    let out = format!("{stdout}{stderr}");

    assert_eq!(code, 1, "a shifted field ordinal is a wire break:\n{out}");
    assert!(
        out.contains("member_reordered veh.cluster/Report/door: ordinal 2 -> ordinal 1\n"),
        "door slid into the slot the tombstone left:\n{out}",
    );
    assert!(
        !out.contains("member_reordered veh.cluster/Report/latch"),
        "latch kept ordinal 3:\n{out}",
    );
    assert!(
        !out.contains("constraint_changed"),
        "nothing changed in place, so nothing is reported on the container:\n{out}",
    );
}

/// A field that keeps its place among the live names still moves when a
/// tombstone ahead of it moves: the reported number is the ordinal, not the
/// index among the live names, so `window` is reported alongside the swap.
#[test]
fn a_field_shifted_by_a_moved_tombstone_is_reported_by_ordinal() {
    let dir = TempDir::new("swap-around-tombstone");
    let old = workspace(&dir, "old", TOMBSTONE_BETWEEN);
    let new = workspace(&dir, "new", SWAPPED_AROUND_TOMBSTONE);

    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), new.as_os_str()]);
    let out = format!("{stdout}{stderr}");

    assert_eq!(code, 1, "a reorder is a wire break:\n{out}");
    assert!(
        out.contains("member_reordered veh.cluster/Report/door: ordinal 1 -> ordinal 2\n")
            && out.contains("member_reordered veh.cluster/Report/latch: ordinal 2 -> ordinal 1\n"),
        "the swapped fields are reported with their ordinals:\n{out}",
    );
    assert!(
        out.contains("member_reordered veh.cluster/Report/window: ordinal 4 -> ordinal 3\n"),
        "window kept its place among the live names but not its ordinal:\n{out}",
    );
    assert!(
        !out.contains("constraint_changed"),
        "nothing changed in place, so nothing is reported on the container:\n{out}",
    );
}

/// A tombstone written after the fields it used to precede frees its slot, and
/// every field after it slides down one ordinal. The order-insensitive
/// comparison clears the tombstone's ordinal as well as the fields', so no
/// `constraint_changed` is reported for what is only a reorder.
#[test]
fn a_tombstone_moved_to_the_end_shifts_every_field_after_it() {
    let dir = TempDir::new("tombstone-last");
    let old = workspace(&dir, "old", TOMBSTONE_FIRST);
    let new = workspace(&dir, "new", TOMBSTONE_LAST);

    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), new.as_os_str()]);
    let out = format!("{stdout}{stderr}");

    assert_eq!(code, 1, "a shifted field ordinal is a wire break:\n{out}");
    assert!(
        out.contains("member_reordered veh.cluster/Report/door: ordinal 2 -> ordinal 1\n")
            && out.contains("member_reordered veh.cluster/Report/latch: ordinal 3 -> ordinal 2\n"),
        "both fields slid down one ordinal:\n{out}",
    );
    assert!(
        !out.contains("constraint_changed"),
        "nothing changed in place, so nothing is reported on the container:\n{out}",
    );
}

/// A tombstone move and a field swap in one edit: the report is one line per
/// field whose ordinal changed. `door` swapped places among the live names but
/// kept ordinal 2, so it is not reported; the tombstone's move is visible
/// through `latch`, which took the slot the tombstone left.
#[test]
fn a_tombstone_move_with_a_swap_reports_only_the_ordinals_that_changed() {
    let dir = TempDir::new("tombstone-last-and-swap");
    let old = workspace(&dir, "old", TOMBSTONE_FIRST);
    let new = workspace(&dir, "new", TOMBSTONE_LAST_AND_SWAPPED);

    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), new.as_os_str()]);
    let out = format!("{stdout}{stderr}");

    assert_eq!(code, 1, "a shifted field ordinal is a wire break:\n{out}");
    assert!(
        out.contains("member_reordered veh.cluster/Report/latch: ordinal 3 -> ordinal 1\n"),
        "latch took the slot the tombstone left:\n{out}",
    );
    assert!(
        !out.contains("member_reordered veh.cluster/Report/door"),
        "door kept ordinal 2:\n{out}",
    );
    assert!(
        !out.contains("constraint_changed"),
        "nothing changed in place, so nothing is reported on the container:\n{out}",
    );
}

/// `PARK` and `DRIVE` swap places in the text, and so do the two tombstones.
/// Every value and every retired number is unchanged.
const GEARS_AND_RESERVED_REORDERED: &str = "package veh.cluster
enum GearPosition {
  DRIVE = 1
  PARK = 0
  reserved 4
  reserved 3
}
";

/// With a value moved, the order-insensitive comparison decides whether
/// anything else changed, and it removes the order of the reserved list along
/// with the order of the values: the report is the two moved values and
/// nothing on the container.
#[test]
fn an_enum_reorder_with_its_reserved_list_reordered_reports_only_the_values() {
    let dir = TempDir::new("enum-and-reserved-reordered");
    let old = workspace(&dir, "old", GEARS_WITH_RESERVED);
    let new = workspace(&dir, "new", GEARS_AND_RESERVED_REORDERED);

    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), new.as_os_str()]);
    let out = format!("{stdout}{stderr}");

    assert_eq!(code, 1, "a reorder is reported breaking:\n{out}");
    assert!(
        out.contains("member_reordered veh.cluster/GearPosition/PARK: position 1 -> position 2\n")
            && out.contains(
                "member_reordered veh.cluster/GearPosition/DRIVE: position 2 -> position 1\n"
            ),
        "both moved values are reported:\n{out}",
    );
    assert!(
        !out.contains("constraint_changed"),
        "the order of the reserved list is not content:\n{out}",
    );
}

const ARMS: &str = "package veh.cluster
type DoorState: integer [0..1]
type Count: integer [0..10]
type Level: integer [0..100]
union Reading {
  door: DoorState
  count: Count
}
";

/// The two arms swap places. Names and types are untouched.
const ARMS_SWAPPED: &str = "package veh.cluster
type DoorState: integer [0..1]
type Count: integer [0..10]
type Level: integer [0..100]
union Reading {
  count: Count
  door: DoorState
}
";

/// The two arms swap places and, in the same edit, `door` changes type.
const ARMS_SWAPPED_AND_RETYPED: &str = "package veh.cluster
type DoorState: integer [0..1]
type Count: integer [0..10]
type Level: integer [0..100]
union Reading {
  count: Count
  door: Level
}
";

/// A tombstone ahead of both arms: `door` is ordinal 2 and `count` ordinal 3.
const ARMS_TOMBSTONE_FIRST: &str = "package veh.cluster
type DoorState: integer [0..1]
type Count: integer [0..10]
type Level: integer [0..100]
union Reading {
  reserved oldArm
  door: DoorState
  count: Count
}
";

/// The tombstone moves after both arms: `door` is now ordinal 1 and `count`
/// ordinal 2.
const ARMS_TOMBSTONE_LAST: &str = "package veh.cluster
type DoorState: integer [0..1]
type Count: integer [0..10]
type Level: integer [0..100]
union Reading {
  door: DoorState
  count: Count
  reserved oldArm
}
";

/// Two tombstones between the arms.
const ARMS_TWO_TOMBSTONES: &str = "package veh.cluster
type DoorState: integer [0..1]
type Count: integer [0..10]
type Level: integer [0..100]
union Reading {
  door: DoorState
  reserved legacyA
  reserved legacyB
  count: Count
}
";

/// The arms swap places and so do the two tombstones between them.
const ARMS_TWO_TOMBSTONES_SWAPPED: &str = "package veh.cluster
type DoorState: integer [0..1]
type Count: integer [0..10]
type Level: integer [0..100]
union Reading {
  count: Count
  reserved legacyB
  reserved legacyA
  door: DoorState
}
";

/// A union arm takes its wire identity from its ordinal as a struct field does
/// (typl §7.4): a swap is one `member_reordered` per arm, with the ordinals,
/// and nothing on the container.
#[test]
fn a_swapped_union_arm_reports_member_reordered() {
    let dir = TempDir::new("union-swap");
    let old = workspace(&dir, "old", ARMS);
    let new = workspace(&dir, "new", ARMS_SWAPPED);

    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), new.as_os_str()]);
    let out = format!("{stdout}{stderr}");

    assert_eq!(code, 1, "a reorder is a wire break:\n{out}");
    assert!(
        out.contains("member_reordered veh.cluster/Reading/door: ordinal 1 -> ordinal 2\n")
            && out.contains("member_reordered veh.cluster/Reading/count: ordinal 2 -> ordinal 1\n"),
        "both moved arms are reported with their ordinals:\n{out}",
    );
    assert!(
        !out.contains("constraint_changed"),
        "nothing changed in place, so nothing is reported on the container:\n{out}",
    );
}

/// A union reorder that arrives with a retyped arm reports both, as a struct
/// does: the moved arms, then the container's `constraint_changed`.
#[test]
fn a_union_reorder_with_a_retyped_arm_reports_both() {
    let dir = TempDir::new("union-swap-and-retype");
    let old = workspace(&dir, "old", ARMS);
    let new = workspace(&dir, "new", ARMS_SWAPPED_AND_RETYPED);

    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), new.as_os_str()]);
    let out = format!("{stdout}{stderr}");

    assert_eq!(code, 1, "a reorder is a wire break:\n{out}");
    assert!(
        out.contains("member_reordered veh.cluster/Reading/door")
            && out.contains("member_reordered veh.cluster/Reading/count"),
        "both moved arms are reported:\n{out}",
    );
    assert!(
        out.contains("constraint_changed veh.cluster/Reading\n"),
        "the retype is reported on the container as well:\n{out}",
    );
}

/// A union tombstone written after the arms it used to precede shifts every
/// arm after it, and the order-insensitive comparison clears the tombstone's
/// ordinal as it clears the arms', so nothing is reported on the container.
#[test]
fn a_union_tombstone_moved_to_the_end_shifts_every_arm_after_it() {
    let dir = TempDir::new("union-tombstone-last");
    let old = workspace(&dir, "old", ARMS_TOMBSTONE_FIRST);
    let new = workspace(&dir, "new", ARMS_TOMBSTONE_LAST);

    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), new.as_os_str()]);
    let out = format!("{stdout}{stderr}");

    assert_eq!(code, 1, "a shifted arm ordinal is a wire break:\n{out}");
    assert!(
        out.contains("member_reordered veh.cluster/Reading/door: ordinal 2 -> ordinal 1\n")
            && out.contains("member_reordered veh.cluster/Reading/count: ordinal 3 -> ordinal 2\n"),
        "both arms slid down one ordinal:\n{out}",
    );
    assert!(
        !out.contains("constraint_changed"),
        "nothing changed in place, so nothing is reported on the container:\n{out}",
    );
}

/// The order of a union's tombstone list is removed by the order-insensitive
/// comparison along with the order of the arms, so two tombstones that swap
/// in the same edit as the arms add nothing to the report.
#[test]
fn a_union_swap_with_its_tombstones_swapped_reports_only_the_arms() {
    let dir = TempDir::new("union-two-tombstones");
    let old = workspace(&dir, "old", ARMS_TWO_TOMBSTONES);
    let new = workspace(&dir, "new", ARMS_TWO_TOMBSTONES_SWAPPED);

    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), new.as_os_str()]);
    let out = format!("{stdout}{stderr}");

    assert_eq!(code, 1, "a reorder is a wire break:\n{out}");
    assert!(
        out.contains("member_reordered veh.cluster/Reading/door: ordinal 1 -> ordinal 4\n")
            && out.contains("member_reordered veh.cluster/Reading/count: ordinal 4 -> ordinal 1\n"),
        "both moved arms are reported with their ordinals:\n{out}",
    );
    assert!(
        !out.contains("constraint_changed"),
        "the order of the tombstone list is not content:\n{out}",
    );
}

const FLAGS: &str = "package veh.cluster
enumset WarningFlags {
  LOW_FUEL = 0
  CHECK_ENGINE = 1
  DOOR_OPEN = 2
}
";

/// `LOW_FUEL` and `CHECK_ENGINE` swap places in the text. Every bit is
/// unchanged.
const FLAGS_REORDERED: &str = "package veh.cluster
enumset WarningFlags {
  CHECK_ENGINE = 1
  LOW_FUEL = 0
  DOOR_OPEN = 2
}
";

/// `LOW_FUEL` and `CHECK_ENGINE` swap places in the text and, in the same edit,
/// `DOOR_OPEN` takes a new bit while keeping its position.
const FLAGS_REORDERED_AND_REBITTED: &str = "package veh.cluster
enumset WarningFlags {
  CHECK_ENGINE = 1
  LOW_FUEL = 0
  DOOR_OPEN = 5
}
";

/// An enum-set bit is treated as an enum value: a textual reorder with every
/// bit unchanged is reported by position, conservatively, and nothing on the
/// container.
#[test]
fn an_enum_set_reorder_with_unchanged_bits_reports_only_member_reordered() {
    let dir = TempDir::new("enumset-reorder");
    let old = workspace(&dir, "old", FLAGS);
    let new = workspace(&dir, "new", FLAGS_REORDERED);

    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), new.as_os_str()]);
    let out = format!("{stdout}{stderr}");

    assert_eq!(code, 1, "a reorder is reported breaking:\n{out}");
    assert!(
        out.contains(
            "member_reordered veh.cluster/WarningFlags/LOW_FUEL: position 1 -> position 2\n"
        ) && out.contains(
            "member_reordered veh.cluster/WarningFlags/CHECK_ENGINE: position 2 -> position 1\n"
        ),
        "both moved bits are reported with their positions:\n{out}",
    );
    assert!(
        !out.contains("constraint_changed"),
        "no bit changed, so nothing is reported on the container:\n{out}",
    );
}

/// An enum-set reorder that also changes a bit reports the container as
/// `constraint_changed` as well: a changed bit is content, and must not hide
/// behind the reorder.
#[test]
fn an_enum_set_reorder_with_a_changed_bit_reports_constraint_changed() {
    let dir = TempDir::new("enumset-reorder-rebit");
    let old = workspace(&dir, "old", FLAGS);
    let new = workspace(&dir, "new", FLAGS_REORDERED_AND_REBITTED);

    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), new.as_os_str()]);
    let out = format!("{stdout}{stderr}");

    assert_eq!(code, 1, "a changed bit is a wire break:\n{out}");
    assert!(
        out.contains("member_reordered veh.cluster/WarningFlags/LOW_FUEL")
            && out.contains("member_reordered veh.cluster/WarningFlags/CHECK_ENGINE"),
        "both moved bits are reported:\n{out}",
    );
    assert!(
        out.contains("constraint_changed veh.cluster/WarningFlags\n"),
        "the changed bit is reported on the container:\n{out}",
    );
}
