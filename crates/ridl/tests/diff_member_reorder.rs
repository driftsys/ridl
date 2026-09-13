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
