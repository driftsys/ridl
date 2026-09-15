//! Integration tests for `ridl lock` (lock design §5): plain allocation,
//! `--rename` and `--retire`, one test per exit-code clause of the design's
//! §5 row and per `ridl lock` cell of its §4 table, each against the built
//! binary — the way ADR-0010 decision 1's cells are verified. Every test that
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
            "ridl-lock-{label}-{}-{}",
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
const DOOR_MANIFEST: &str = "[package]\nname = \"veh.door\"\nversion = \"1.0.0\"\n";
const HEADER: &str = "# interfaces.lock — written by ridl lock; do not edit by hand.\n";

/// Two interfaces whose file order (`Zone`, `Cabin`) is not the byte order
/// of their names, so the allocation order shows.
const ZONE_AND_CABIN: &str = "package veh.hvac
type State: integer [0..1]
interface Zone { signal z : State @[100ms..1s] }
interface Cabin { signal c : State @[100ms..1s] }
";

/// `Cabin` alone.
const CABIN: &str = "package veh.hvac
type State: integer [0..1]
interface Cabin { signal c : State @[100ms..1s] }
";

/// `Cabin` and a service with an inline shape.
const CABIN_AND_REAR: &str = "package veh.hvac
type State: integer [0..1]
interface Cabin { signal c : State @[100ms..1s] }
service veh.hvac.rear { signal r : State @[100ms..1s] }
";

/// `Cabin` beside a type whose bounds are inverted — TYPL-104, a compile
/// error that is not RIDL-409.
const CABIN_WITH_AN_ERROR: &str = "package veh.hvac
type State: integer [0..1]
type Bad: integer [10..0]
interface Cabin { signal c : State @[100ms..1s] }
";

/// A second member for the workspace fixtures.
const DOOR: &str = "package veh.door
type Latch: integer [0..1]
interface Door { signal d : Latch @[100ms..1s] }
";

/// The second member with a compile error (TYPL-104).
const DOOR_WITH_AN_ERROR: &str = "package veh.door
type Latch: integer [10..0]
interface Door { signal d : Latch @[100ms..1s] }
";

/// Lays out a one-package workspace holding `source` and returns its root.
fn package_workspace(dir: &TempDir, source: &str) -> PathBuf {
    dir.write("ridl.toml", MANIFEST);
    dir.write("hvac.ridl", source);
    dir.path().to_path_buf()
}

/// Lays out a two-member workspace (`hvac`, then `door`) and returns its root.
fn two_member_workspace(dir: &TempDir, hvac: &str, door: &str) -> PathBuf {
    dir.write("ridl.toml", "[workspace]\nmembers = [\"hvac\", \"door\"]\n");
    dir.write("hvac/ridl.toml", MANIFEST);
    dir.write("hvac/hvac.ridl", hvac);
    dir.write("door/ridl.toml", DOOR_MANIFEST);
    dir.write("door/door.ridl", door);
    dir.path().to_path_buf()
}

/// The text of `<root>/interfaces.lock`.
fn read_lock(root: &Path) -> String {
    std::fs::read_to_string(root.join("interfaces.lock")).expect("the lock file exists")
}

/// `ridl lock <root> [flags]`.
fn lock(root: &Path, flags: &[&str]) -> (i32, String, String) {
    let mut args: Vec<&std::ffi::OsStr> = vec!["lock".as_ref(), root.as_os_str()];
    args.extend(
        flags
            .iter()
            .map(|flag| -> &std::ffi::OsStr { flag.as_ref() }),
    );
    ridl(&args)
}

/// `ridl check <root>`'s exit code and stderr.
fn check(root: &Path) -> (i32, String) {
    let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);
    (code, stderr)
}

// --- §5 exit 0: the file is written, or there is nothing to change ---------

/// §4 row 1, `ridl lock` column: plain `ridl lock` allocates every
/// declaration with no entry, in byte order of the name from `next` — the
/// numbers the checker showed as provisional — and writes the file. A second
/// run finds nothing to change.
#[test]
fn plain_lock_allocates_every_provisional_interface_and_exits_zero() {
    let dir = TempDir::new("allocate");
    let root = package_workspace(&dir, ZONE_AND_CABIN);
    assert!(!root.join("interfaces.lock").exists());

    let (code, stdout, stderr) = lock(&root, &[]);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(stdout, "allocated Cabin 1\nallocated Zone 2\n");
    assert_eq!(
        read_lock(&root),
        format!("{HEADER}next 3\nCabin 1\nZone 2\n")
    );

    let (code, stderr) = check(&root);
    assert_eq!(code, 0, "the package checks clean with its lock: {stderr}");

    let (code, stdout, stderr) = lock(&root, &[]);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(stdout, "", "nothing to allocate on the second run");
    assert_eq!(
        read_lock(&root),
        format!("{HEADER}next 3\nCabin 1\nZone 2\n")
    );
}

/// §5 exit 0, "there is nothing to change": a lock that already holds every
/// declaration is left byte for byte as it is — the writer is not run over
/// it, so a hand-formatted file (entries out of number order, a comment) is
/// not rewritten.
#[test]
fn plain_lock_with_nothing_to_change_exits_zero_and_writes_nothing() {
    let dir = TempDir::new("nothing");
    let root = package_workspace(&dir, ZONE_AND_CABIN);
    let text = format!("{HEADER}# kept as written\nnext 3\nZone 2\nCabin 1\n");
    dir.write("interfaces.lock", &text);

    let (code, stdout, stderr) = lock(&root, &[]);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(stdout, "");
    assert_eq!(read_lock(&root), text, "byte-identical");
}

/// Design §2: a package with no interfaces never gets a lock file.
#[test]
fn a_package_with_no_interfaces_gets_no_lock_file() {
    let dir = TempDir::new("no-interfaces");
    let root = package_workspace(&dir, "package veh.hvac\ntype State: integer [0..1]\n");

    let (code, stdout, stderr) = lock(&root, &[]);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(stdout, "");
    assert!(!root.join("interfaces.lock").exists(), "no file is created");
}

// --- §5 exit 1: a diagnostic error over the source, nothing written --------

/// §5 exit 1 and §4 row 2, `ridl lock` column: plain `ridl lock` over a live
/// entry with no declaration is RIDL-409, exit 1, and nothing is written —
/// a retire is a recorded departure, never inferred.
#[test]
fn plain_lock_refuses_an_orphan_entry_with_ridl_409_and_writes_nothing() {
    let dir = TempDir::new("orphan");
    let root = package_workspace(&dir, ZONE_AND_CABIN);
    let text = format!("{HEADER}next 3\nCabin 1\nLegacy 2\n");
    dir.write("interfaces.lock", &text);

    let (code, stdout, stderr) = lock(&root, &[]);
    assert_eq!(code, 1, "stderr:\n{stderr}");
    assert_eq!(stdout, "");
    assert!(stderr.contains("error[RIDL-409]"), "stderr:\n{stderr}");
    assert!(stderr.contains("--retire Legacy"), "stderr:\n{stderr}");
    assert_eq!(
        read_lock(&root),
        text,
        "byte-identical: `Zone` was not allocated"
    );
}

/// §5 exit 1: a malformed lock file — conflict markers included — is
/// RIDL-410, exit 1, and nothing is written.
#[test]
fn a_malformed_lock_is_ridl_410_and_nothing_is_written() {
    let dir = TempDir::new("malformed");
    let root = package_workspace(&dir, ZONE_AND_CABIN);
    let text = format!("{HEADER}next 3\n<<<<<<< HEAD\nCabin 1\n=======\nZone 1\n>>>>>>> feature\n");
    dir.write("interfaces.lock", &text);

    let (code, stdout, stderr) = lock(&root, &[]);
    assert_eq!(code, 1, "stderr:\n{stderr}");
    assert_eq!(stdout, "");
    assert!(stderr.contains("error[RIDL-410]"), "stderr:\n{stderr}");
    assert_eq!(read_lock(&root), text, "byte-identical");
}

/// PD-6: `ridl lock` over a workspace compiles every package first. A
/// compile error in one member exits 1 and writes no member's file — the
/// clean member is not allocated either.
#[test]
fn a_compile_error_in_one_package_writes_no_package_file() {
    let dir = TempDir::new("one-bad-member");
    let root = two_member_workspace(&dir, ZONE_AND_CABIN, DOOR_WITH_AN_ERROR);

    let (code, stdout, stderr) = lock(&root, &[]);
    assert_eq!(code, 1, "stderr:\n{stderr}");
    assert_eq!(stdout, "");
    assert!(stderr.contains("error[TYPL-104]"), "stderr:\n{stderr}");
    assert!(
        !root.join("hvac/interfaces.lock").exists(),
        "the clean member's file is not written"
    );
    assert!(!root.join("door/interfaces.lock").exists());
}

/// §5 text: `--rename` and `--retire` run with RIDL-409 present, but any
/// other compile error still exits 1, and nothing is written.
#[test]
fn rename_with_another_compile_error_exits_one() {
    let dir = TempDir::new("retire-with-error");
    let root = package_workspace(&dir, CABIN_WITH_AN_ERROR);
    let text = format!("{HEADER}next 3\nCabin 1\nLegacy 2\n");
    dir.write("interfaces.lock", &text);

    let (code, stdout, stderr) = lock(&root, &["--retire", "Legacy"]);
    assert_eq!(code, 1, "stderr:\n{stderr}");
    assert_eq!(stdout, "");
    assert!(stderr.contains("error[TYPL-104]"), "stderr:\n{stderr}");
    assert_eq!(read_lock(&root), text, "byte-identical");
}

// --- §5 exit 2: the tool could not answer ----------------------------------

/// §5 exit 2: the path is missing.
#[test]
fn a_missing_path_exits_two() {
    let dir = TempDir::new("missing");
    let missing = dir.path().join("nowhere");
    let (code, stdout, stderr) = lock(&missing, &[]);
    assert_eq!(code, 2, "stderr:\n{stderr}");
    assert_eq!(stdout, "");
    assert!(stderr.contains("does not exist"), "stderr:\n{stderr}");
}

/// §5 exit 2, "a bad flag": a `--rename` that is not `OLD=NEW`, or a key
/// that is not a lock key, is refused before anything is compiled or written.
#[test]
fn a_flag_that_does_not_parse_exits_two() {
    let dir = TempDir::new("bad-flag");
    let root = package_workspace(&dir, ZONE_AND_CABIN);
    let text = format!("{HEADER}next 3\nCabin 1\nLegacy 2\n");
    dir.write("interfaces.lock", &text);

    let (code, _, stderr) = lock(&root, &["--rename", "Legacy"]);
    assert_eq!(code, 2, "stderr:\n{stderr}");
    assert!(stderr.contains("takes OLD=NEW"), "stderr:\n{stderr}");

    let (code, _, stderr) = lock(&root, &["--rename", "Legacy=zo-ne"]);
    assert_eq!(code, 2, "stderr:\n{stderr}");
    assert!(stderr.contains("is not a lock key"), "stderr:\n{stderr}");

    let (code, _, stderr) = lock(&root, &["--retire", "service:"]);
    assert_eq!(code, 2, "stderr:\n{stderr}");
    assert!(stderr.contains("is not a lock key"), "stderr:\n{stderr}");

    assert_eq!(read_lock(&root), text, "byte-identical");
}

/// §5 exit 2: `--rename` naming no live entry.
#[test]
fn rename_naming_no_live_entry_exits_two() {
    let dir = TempDir::new("rename-absent");
    let root = package_workspace(&dir, ZONE_AND_CABIN);
    let text = format!("{HEADER}next 3\nCabin 1\nOld 2 retired\n");
    dir.write("interfaces.lock", &text);

    let (code, stdout, stderr) = lock(&root, &["--rename", "Legacy=Zone"]);
    assert_eq!(code, 2, "stderr:\n{stderr}");
    assert_eq!(stdout, "");
    assert!(
        stderr.contains("no live entry `Legacy`"),
        "stderr:\n{stderr}"
    );

    let (code, _, stderr) = lock(&root, &["--rename", "Old=Zone"]);
    assert_eq!(code, 2, "a retired entry is not live: {stderr}");
    assert!(stderr.contains("no live entry `Old`"), "stderr:\n{stderr}");
    assert_eq!(read_lock(&root), text, "byte-identical");
}

/// §5 exit 2: `--rename` to a `NEW` that is not a declaration without an
/// entry — a declaration that already has one, or no declaration at all.
#[test]
fn rename_to_a_name_that_is_not_an_unentered_declaration_exits_two() {
    let dir = TempDir::new("rename-bad-new");
    let root = package_workspace(&dir, ZONE_AND_CABIN);
    let text = format!("{HEADER}next 3\nCabin 1\nLegacy 2\n");
    dir.write("interfaces.lock", &text);

    let (code, stdout, stderr) = lock(&root, &["--rename", "Legacy=Cabin"]);
    assert_eq!(code, 2, "`Cabin` has its entry: {stderr}");
    assert_eq!(stdout, "");
    assert!(
        stderr.contains("`Cabin` is not a declaration without an entry"),
        "stderr:\n{stderr}"
    );

    let (code, _, stderr) = lock(&root, &["--rename", "Legacy=Nope"]);
    assert_eq!(code, 2, "`Nope` is not declared: {stderr}");
    assert!(
        stderr.contains("`Nope` is not a declaration without an entry"),
        "stderr:\n{stderr}"
    );
    assert_eq!(read_lock(&root), text, "byte-identical");
}

/// §5 exit 2: `--retire` naming a still-declared interface, or no live entry.
#[test]
fn retire_of_a_still_declared_interface_exits_two() {
    let dir = TempDir::new("retire-declared");
    let root = package_workspace(&dir, ZONE_AND_CABIN);
    let text = format!("{HEADER}next 3\nCabin 1\nZone 2\n");
    dir.write("interfaces.lock", &text);

    let (code, stdout, stderr) = lock(&root, &["--retire", "Cabin"]);
    assert_eq!(code, 2, "stderr:\n{stderr}");
    assert_eq!(stdout, "");
    assert!(
        stderr.contains("`Cabin` is still declared"),
        "stderr:\n{stderr}"
    );

    let (code, _, stderr) = lock(&root, &["--retire", "Absent"]);
    assert_eq!(code, 2, "stderr:\n{stderr}");
    assert!(
        stderr.contains("no live entry `Absent`"),
        "stderr:\n{stderr}"
    );
    assert_eq!(read_lock(&root), text, "byte-identical");
}

/// §5 exit 2: either flag over more than one package. `PATH` must resolve
/// to exactly one package; nothing is written.
#[test]
fn rename_over_more_than_one_package_exits_two() {
    let dir = TempDir::new("two-packages");
    let root = two_member_workspace(&dir, ZONE_AND_CABIN, DOOR);
    let hvac = format!("{HEADER}next 3\nCabin 1\nLegacy 2\n");
    dir.write("hvac/interfaces.lock", &hvac);

    let (code, stdout, stderr) = lock(&root, &["--rename", "Legacy=Zone"]);
    assert_eq!(code, 2, "stderr:\n{stderr}");
    assert_eq!(stdout, "");
    assert!(stderr.contains("holds 2 packages"), "stderr:\n{stderr}");

    let (code, _, stderr) = lock(&root, &["--retire", "Legacy"]);
    assert_eq!(code, 2, "stderr:\n{stderr}");
    assert_eq!(read_lock(&root.join("hvac")), hvac, "byte-identical");
    assert!(!root.join("door/interfaces.lock").exists());

    // Named directly, the member is one package and the edit runs.
    let (code, stdout, stderr) = lock(&root.join("hvac"), &["--rename", "Legacy=Zone"]);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(stdout, "renamed Legacy Zone 2\n");
}

/// §5 exit 2: an I/O failure writing the file.
#[test]
fn an_unwritable_lock_file_exits_two() {
    use std::os::unix::fs::PermissionsExt;

    // Restores the directory's permissions on scope exit, panic or not, so a
    // failed assertion never leaves a read-only directory behind for the
    // `TempDir`'s own `Drop` to trip over.
    struct RestorePermissions(PathBuf);
    impl Drop for RestorePermissions {
        fn drop(&mut self) {
            let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
        }
    }

    let dir = TempDir::new("unwritable");
    let root = package_workspace(&dir, ZONE_AND_CABIN);
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o555))
        .expect("chmod the package directory read-only");
    let _restore = RestorePermissions(root.clone());

    let (code, stdout, stderr) = lock(&root, &[]);
    assert_eq!(code, 2, "stderr:\n{stderr}");
    assert_eq!(stdout, "", "nothing is reported as allocated");
    assert!(stderr.contains("cannot write"), "stderr:\n{stderr}");
    assert!(stderr.contains("interfaces.lock"), "stderr:\n{stderr}");
}

// --- §4, the `ridl lock` column ---------------------------------------------

/// §4 row 2: `--retire Old` marks the line retired, keeps its number and
/// `next`, and runs with RIDL-409 present — it is that diagnostic's fix, so
/// the package checks clean afterwards.
#[test]
fn retire_marks_the_line_retired() {
    let dir = TempDir::new("retire");
    let root = package_workspace(&dir, CABIN);
    dir.write(
        "interfaces.lock",
        &format!("{HEADER}next 3\nCabin 1\nLegacy 2\n"),
    );
    let (code, stderr) = check(&root);
    assert_eq!(code, 1, "RIDL-409 before the retire: {stderr}");

    let (code, stdout, stderr) = lock(&root, &["--retire", "Legacy"]);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(stdout, "retired Legacy 2\n");
    assert!(
        !stderr.contains("RIDL-409"),
        "the fixed diagnostic is not rendered: {stderr}"
    );
    assert_eq!(
        read_lock(&root),
        format!("{HEADER}next 3\nCabin 1\nLegacy 2 retired\n")
    );

    let (code, stderr) = check(&root);
    assert_eq!(code, 0, "the retire is recorded: {stderr}");
}

/// §4 row 3: `--rename Old=New` rewrites the line in place, keeps the
/// number and `next`, allocates nothing, and runs with RIDL-409 present.
#[test]
fn rename_rewrites_the_line_in_place() {
    let dir = TempDir::new("rename");
    let root = package_workspace(&dir, ZONE_AND_CABIN);
    dir.write(
        "interfaces.lock",
        &format!("{HEADER}next 3\nCabin 1\nLegacy 2\n"),
    );
    let (code, stderr) = check(&root);
    assert_eq!(code, 1, "RIDL-409 before the rename: {stderr}");

    let (code, stdout, stderr) = lock(&root, &["--rename", "Legacy=Zone"]);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(stdout, "renamed Legacy Zone 2\n");
    assert_eq!(
        read_lock(&root),
        format!("{HEADER}next 3\nCabin 1\nZone 2\n"),
        "the line is rewritten; `next` does not move"
    );

    let (code, stderr) = check(&root);
    assert_eq!(code, 0, "the rename is recorded: {stderr}");
}

/// §4 row 6: a lock line deleted by hand. Plain `ridl lock` allocates a new
/// number to the kept declaration — `next` was never lowered, so the old
/// number is not reused.
#[test]
fn a_hand_deleted_line_gets_a_fresh_number() {
    let dir = TempDir::new("hand-deleted");
    let root = package_workspace(&dir, ZONE_AND_CABIN);
    // `Zone 2` was deleted by hand; `next 3` stands.
    dir.write("interfaces.lock", &format!("{HEADER}next 3\nCabin 1\n"));

    let (code, stdout, stderr) = lock(&root, &[]);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(stdout, "allocated Zone 3\n");
    assert_eq!(
        read_lock(&root),
        format!("{HEADER}next 4\nCabin 1\nZone 3\n")
    );
}

/// PD-13: over more than one package, each output line is prefixed with the
/// package directory relative to `PATH` and a colon; each package's own
/// file is written.
#[test]
fn workspace_output_prefixes_each_package_path() {
    let dir = TempDir::new("workspace");
    let root = two_member_workspace(&dir, ZONE_AND_CABIN, DOOR);

    let (code, stdout, stderr) = lock(&root, &[]);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(
        stdout,
        "hvac: allocated Cabin 1\nhvac: allocated Zone 2\ndoor: allocated Door 1\n"
    );
    assert_eq!(
        read_lock(&root.join("hvac")),
        format!("{HEADER}next 3\nCabin 1\nZone 2\n")
    );
    assert_eq!(
        read_lock(&root.join("door")),
        format!("{HEADER}next 2\nDoor 1\n")
    );
}

/// Design §3: a service's inline shape is allocated under the `service:`
/// key, in byte order with the interfaces.
#[test]
fn an_inline_shape_allocates_under_its_service_key() {
    let dir = TempDir::new("inline");
    let root = package_workspace(&dir, CABIN_AND_REAR);

    let (code, stdout, stderr) = lock(&root, &[]);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(
        stdout,
        "allocated Cabin 1\nallocated service:veh.hvac.rear 2\n"
    );
    assert_eq!(
        read_lock(&root),
        format!("{HEADER}next 3\nCabin 1\nservice:veh.hvac.rear 2\n")
    );
    let (code, stderr) = check(&root);
    assert_eq!(code, 0, "stderr:\n{stderr}");
}
