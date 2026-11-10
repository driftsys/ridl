//! Integration tests for the `instances.lock` checks of `ridl check` (rsdl
//! reference §7): an absent file is not an error, a live entry with no
//! instance is RSDL-314, and a malformed file is RSDL-315. Each test runs the
//! built binary.

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
            "ridl-instance-lock-{label}-{}-{}",
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

const MANIFEST: &str = "[package]\nname = \"veh\"\nversion = \"1.0.0\"\n";
const SYSTEM: &str = "package veh\n\
                      component Cruise [ instances = (primary, backup) ] {}\n\
                      system Vehicle { Cruise }\n";
const HEADER: &str = "# instances.lock — written by ridl lock; do not edit by hand.\n";

/// A one-package workspace whose system has one component of two instances,
/// and an `instances.lock` with the given body, or none.
fn workspace(label: &str, lock: Option<&str>) -> TempDir {
    let dir = TempDir::new(label);
    dir.write("ridl.toml", MANIFEST);
    dir.write("system.rsdl", SYSTEM);
    if let Some(body) = lock {
        dir.write("instances.lock", body);
    }
    dir
}

fn check(dir: &TempDir) -> (i32, String, String) {
    ridl(&["check".as_ref(), dir.path().as_os_str()])
}

#[test]
fn a_missing_instance_lock_is_not_an_error() {
    let dir = workspace("missing", None);
    let (code, stdout, stderr) = check(&dir);
    assert_eq!(code, 0, "stdout: {stdout}\nstderr: {stderr}");
}

#[test]
fn check_reports_an_orphan_instance_entry_as_rsdl_314() {
    let lock = format!("{HEADER}next 4\nveh.Cruise/spare 3\n");
    let dir = workspace("orphan", Some(&lock));
    let (code, _, stderr) = check(&dir);
    assert_eq!(code, 1, "stderr: {stderr}");
    assert!(stderr.contains("error[RSDL-314]"), "{stderr}");
    assert!(stderr.contains("instances.lock"), "{stderr}");
    assert!(stderr.contains("--retire veh.Cruise/spare"), "{stderr}");
}

#[test]
fn check_reports_a_malformed_instance_lock_as_rsdl_315() {
    let dir = workspace("malformed", Some("next x\n"));
    let (code, _, stderr) = check(&dir);
    assert_eq!(code, 1, "stderr: {stderr}");
    assert!(stderr.contains("error[RSDL-315]"), "{stderr}");
}

#[test]
fn check_inside_a_member_still_reports_the_root_instance_lock() {
    let dir = TempDir::new("member");
    dir.write("ridl.toml", "[workspace]\nmembers = [\"veh\"]\n");
    dir.write("veh/ridl.toml", MANIFEST);
    dir.write("veh/system.rsdl", SYSTEM);
    dir.write(
        "instances.lock",
        &format!("{HEADER}next 4\nveh.Cruise/spare 3\n"),
    );
    let member = dir.path().join("veh");
    let (code, _, stderr) = ridl(&["check".as_ref(), member.as_os_str()]);
    assert_eq!(code, 1, "stderr: {stderr}");
    assert!(stderr.contains("error[RSDL-314]"), "{stderr}");
}

// ---- `ridl lock` and `ridl lock merge` over `instances.lock` ----

/// `SYSTEM` with a lone service, which numbers as `veh.status/Unit`.
const LOCK_SYSTEM: &str = "package veh\n\
                           component Cruise [ instances = (primary, backup) ] {}\n\
                           system Vehicle { Cruise, veh.status }\n";

/// The lone service of `LOCK_SYSTEM`.
const STATUS_SERVICE: &str =
    "package veh\ntype Flag: integer [0..1]\nservice veh.status { signal a: Flag @[100ms..1s] }\n";

/// A one-package workspace on `LOCK_SYSTEM`, with the given `instances.lock`
/// body, or none.
fn lock_workspace(label: &str, lock: Option<&str>) -> TempDir {
    let dir = TempDir::new(label);
    dir.write("ridl.toml", MANIFEST);
    dir.write("system.rsdl", LOCK_SYSTEM);
    dir.write("status.ridl", STATUS_SERVICE);
    if let Some(body) = lock {
        dir.write("instances.lock", body);
    }
    dir
}

fn run_lock(dir: &TempDir, args: &[&str]) -> (i32, String, String) {
    let mut all: Vec<&std::ffi::OsStr> = vec!["lock".as_ref(), dir.path().as_os_str()];
    all.extend(args.iter().map(std::ffi::OsStr::new));
    ridl(&all)
}

fn lock_text(dir: &TempDir) -> String {
    std::fs::read_to_string(dir.path().join("instances.lock")).expect("instances.lock exists")
}

/// The positions of `needles` in `haystack`, which must all be present and in
/// the order given.
fn assert_in_order(haystack: &str, needles: &[&str]) {
    let mut from = 0;
    for needle in needles {
        let at = haystack[from..]
            .find(needle)
            .unwrap_or_else(|| panic!("`{needle}` not found in order in:\n{haystack}"));
        from += at + needle.len();
    }
}

#[test]
fn plain_lock_allocates_every_instance_in_key_byte_order() {
    let dir = lock_workspace("alloc", None);
    let (code, stdout, stderr) = run_lock(&dir, &[]);
    assert_eq!(code, 0, "stdout: {stdout}\nstderr: {stderr}");
    assert_in_order(
        &stdout,
        &[
            "allocated veh.Cruise/backup 1",
            "allocated veh.Cruise/primary 2",
            "allocated veh.status/Unit 3",
        ],
    );
    assert_eq!(
        lock_text(&dir),
        format!("{HEADER}next 4\nveh.Cruise/backup 1\nveh.Cruise/primary 2\nveh.status/Unit 3\n")
    );
}

/// Two lone services whose keys sort one way as text and the other way field
/// by field: `veh.cruise.x/Unit` comes first as text, because `.` sorts
/// before `/`, but component `veh.cruise` comes before `veh.cruise.x`. The
/// checker numbers in text order, and `ridl lock` must write those numbers.
#[test]
fn plain_lock_writes_the_numbers_in_key_text_order() {
    let dir = TempDir::new("text-order");
    dir.write("ridl.toml", MANIFEST);
    dir.write(
        "services.ridl",
        "package veh\n\
         type Flag: integer [0..1]\n\
         service veh.cruise { signal a: Flag @[100ms..1s] }\n\
         service veh.cruise.x { signal b: Flag @[100ms..1s] }\n",
    );
    dir.write(
        "system.rsdl",
        "package veh\nsystem Vehicle { veh.cruise, veh.cruise.x }\n",
    );
    let (code, stdout, stderr) = run_lock(&dir, &[]);
    assert_eq!(code, 0, "stdout: {stdout}\nstderr: {stderr}");
    assert_in_order(
        &stdout,
        &[
            "allocated veh.cruise.x/Unit 1",
            "allocated veh.cruise/Unit 2",
        ],
    );
    assert_eq!(
        lock_text(&dir),
        format!("{HEADER}next 3\nveh.cruise.x/Unit 1\nveh.cruise/Unit 2\n")
    );
}

#[test]
fn plain_lock_with_nothing_to_change_writes_nothing() {
    let dir = lock_workspace("nothing", None);
    let (code, _, stderr) = run_lock(&dir, &[]);
    assert_eq!(code, 0, "{stderr}");
    let before = std::fs::read(dir.path().join("instances.lock")).unwrap();
    let (code, stdout, stderr) = run_lock(&dir, &[]);
    assert_eq!(code, 0, "{stderr}");
    assert!(!stdout.contains("allocated"), "{stdout}");
    assert_eq!(
        std::fs::read(dir.path().join("instances.lock")).unwrap(),
        before
    );
}

#[test]
fn a_workspace_with_no_system_gets_no_instance_lock() {
    let dir = TempDir::new("nosystem");
    dir.write("ridl.toml", MANIFEST);
    dir.write("a.ridl", "package veh\n/// A.\ntype A: u8\n");
    let (code, _, stderr) = run_lock(&dir, &[]);
    assert_eq!(code, 0, "{stderr}");
    assert!(!dir.path().join("instances.lock").exists());
}

#[test]
fn plain_lock_refuses_an_orphan_instance_entry() {
    let lock = format!("{HEADER}next 5\nveh.Cruise/spare 4\n");
    let dir = lock_workspace("refuse", Some(&lock));
    let (code, _, stderr) = run_lock(&dir, &[]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("RSDL-314"), "{stderr}");
    assert_eq!(lock_text(&dir), lock);
}

/// The lock for `LOCK_SYSTEM` after plain `ridl lock`.
fn allocated_lock() -> String {
    format!("{HEADER}next 4\nveh.Cruise/backup 1\nveh.Cruise/primary 2\nveh.status/Unit 3\n")
}

#[test]
fn retire_an_instance_marks_its_line_retired() {
    let dir = lock_workspace("retire", Some(&allocated_lock()));
    dir.write(
        "system.rsdl",
        &LOCK_SYSTEM.replace("(primary, backup)", "(primary)"),
    );
    let (code, stdout, stderr) = run_lock(&dir, &["--retire", "veh.Cruise/backup"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("retired veh.Cruise/backup 1"), "{stdout}");
    assert_eq!(
        lock_text(&dir),
        format!(
            "{HEADER}next 4\nveh.Cruise/backup 1 retired\nveh.Cruise/primary 2\nveh.status/Unit 3\n"
        )
    );
}

#[test]
fn retire_of_an_instance_still_in_the_system_exits_two() {
    let dir = lock_workspace("retire-live", Some(&allocated_lock()));
    let (code, _, stderr) = run_lock(&dir, &["--retire", "veh.Cruise/backup"]);
    assert_eq!(code, 2, "{stderr}");
    assert_eq!(lock_text(&dir), allocated_lock());
}

#[test]
fn rename_an_instance_keeps_its_number() {
    let dir = lock_workspace("rename", Some(&allocated_lock()));
    dir.write(
        "system.rsdl",
        &LOCK_SYSTEM.replace("(primary, backup)", "(primary, spare)"),
    );
    let (code, stdout, stderr) =
        run_lock(&dir, &["--rename", "veh.Cruise/backup=veh.Cruise/spare"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stdout.contains("renamed veh.Cruise/backup veh.Cruise/spare 1"),
        "{stdout}"
    );
    assert!(lock_text(&dir).contains("veh.Cruise/spare 1\n"));
}

#[cfg(target_os = "macos")]
#[test]
fn plain_allocation_failure_restores_interfaces_and_removes_temporaries() {
    struct ImmutableFile(PathBuf);
    impl Drop for ImmutableFile {
        fn drop(&mut self) {
            let _ = Command::new("chflags").arg("nouchg").arg(&self.0).status();
        }
    }

    let dir = lock_workspace("allocation-write-failure", Some("next 1\n"));
    let interface_path = dir.write("interfaces.lock", "# preserve this comment\nnext 1\n");
    let interface_before = std::fs::read(&interface_path).unwrap();
    let instance_path = dir.path().join("instances.lock");
    let instance_before = std::fs::read(&instance_path).unwrap();
    let files = || {
        let mut files: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        files.sort();
        files
    };
    let files_before = files();
    let _immutable = ImmutableFile(instance_path.clone());
    assert!(
        Command::new("chflags")
            .arg("uchg")
            .arg(&instance_path)
            .status()
            .unwrap()
            .success()
    );

    let (code, stdout, stderr) = run_lock(&dir, &[]);
    assert_eq!(code, 2, "stdout: {stdout}\nstderr: {stderr}");
    assert_eq!(std::fs::read(&interface_path).unwrap(), interface_before);
    assert_eq!(std::fs::read(&instance_path).unwrap(), instance_before);
    assert_eq!(files(), files_before, "no temporary file remains");
    assert!(stdout.is_empty(), "{stdout}");
    assert!(
        stderr.contains(&format!(
            "error: cannot write {}: ",
            instance_path.display()
        )),
        "{stderr}"
    );
}

#[cfg(target_os = "macos")]
#[test]
fn second_lock_write_failure_restores_interfaces_and_removes_temporaries() {
    struct ImmutableFile(PathBuf);
    impl Drop for ImmutableFile {
        fn drop(&mut self) {
            let _ = Command::new("chflags").arg("nouchg").arg(&self.0).status();
        }
    }

    let dir = lock_workspace(
        "second-write-failure",
        Some(&format!(
            "{HEADER}next 5\nveh.Cruise/backup 1\nveh.Cruise/primary 2\nveh.status/Unit 3\nveh.Cruise/spare 4\n"
        )),
    );
    let interface_path = dir.write(
        "interfaces.lock",
        "# preserve this comment\nnext 3\nservice:veh.status 1\nUnused 2\n",
    );
    let interface_before = std::fs::read(&interface_path).unwrap();
    let instance_path = dir.path().join("instances.lock");
    let instance_before = std::fs::read(&instance_path).unwrap();
    let files = || {
        let mut files: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        files.sort();
        files
    };
    let files_before = files();
    let _immutable = ImmutableFile(instance_path.clone());
    assert!(
        Command::new("chflags")
            .arg("uchg")
            .arg(&instance_path)
            .status()
            .unwrap()
            .success()
    );

    let (code, stdout, stderr) = run_lock(
        &dir,
        &["--retire", "Unused", "--retire", "veh.Cruise/spare"],
    );
    assert_eq!(code, 2, "stdout: {stdout}\nstderr: {stderr}");
    assert_eq!(std::fs::read(&interface_path).unwrap(), interface_before);
    assert_eq!(std::fs::read(&instance_path).unwrap(), instance_before);
    assert_eq!(files(), files_before, "no temporary file remains");
    assert!(stdout.is_empty(), "{stdout}");
    assert!(
        stderr.contains(&format!(
            "error: cannot write {}: ",
            instance_path.display()
        )),
        "{stderr}"
    );
}

/// The new side of an instance rename must be an instance of the system
/// without an entry; `veh.Cruise/ghost` is no instance at all.
#[test]
fn rename_onto_a_key_that_is_no_instance_exits_two() {
    let dir = lock_workspace("rename-ghost", Some(&allocated_lock()));
    dir.write(
        "system.rsdl",
        &LOCK_SYSTEM.replace("(primary, backup)", "(primary)"),
    );
    let (code, _, stderr) = run_lock(&dir, &["--rename", "veh.Cruise/backup=veh.Cruise/ghost"]);
    assert_eq!(code, 2, "{stderr}");
    assert!(
        stderr.contains("`veh.Cruise/ghost` is not an instance without an entry"),
        "{stderr}"
    );
    assert_eq!(lock_text(&dir), allocated_lock());
}

/// A component rename needs every moved key to be an instance of the system
/// without an entry; component `veh.Ghost` is not in the system.
#[test]
fn rename_a_component_onto_one_not_in_the_system_exits_two() {
    let dir = lock_workspace("rename-component-ghost", Some(&allocated_lock()));
    dir.write(
        "system.rsdl",
        "package veh\nsystem Vehicle { veh.status }\n",
    );
    let (code, _, stderr) = run_lock(&dir, &["--rename", "veh.Cruise=veh.Ghost"]);
    assert_eq!(code, 2, "{stderr}");
    assert!(
        stderr.contains("is not an instance without an entry"),
        "{stderr}"
    );
    assert_eq!(lock_text(&dir), allocated_lock());
}

/// The rename the RSDL-314 message suggests for a component, with a fully
/// qualified new name, is accepted.
#[test]
fn the_component_rename_rsdl_314_suggests_is_accepted() {
    let dir = lock_workspace("rename-component-hint", Some(&allocated_lock()));
    dir.write("system.rsdl", &LOCK_SYSTEM.replace("Cruise", "New"));
    let (code, _, stderr) = check(&dir);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("--rename veh.Cruise=veh.New"), "{stderr}");
    let (code, stdout, stderr) = run_lock(&dir, &["--rename", "veh.Cruise=veh.New"]);
    assert_eq!(code, 0, "stdout: {stdout}\nstderr: {stderr}");
    assert!(lock_text(&dir).contains("veh.New/backup 1\n"));
}

#[test]
fn retire_of_a_component_still_in_the_system_exits_two() {
    let dir = lock_workspace("retire-component-live", Some(&allocated_lock()));
    let (code, _, stderr) = run_lock(&dir, &["--retire", "veh.Cruise"]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stderr.contains("still has instances"), "{stderr}");
    assert_eq!(lock_text(&dir), allocated_lock());
}

#[test]
fn rename_a_component_moves_every_entry_with_its_number() {
    let lock = format!(
        "{HEADER}next 5\nveh.Cruise/primary 2\nveh.Cruise/backup 1\nveh.Cruise/old 4 retired\nveh.status/Unit 3\n"
    );
    let dir = lock_workspace("rename-component", Some(&lock));
    dir.write("system.rsdl", &LOCK_SYSTEM.replace("Cruise", "Speed"));
    let (code, stdout, stderr) = run_lock(&dir, &["--rename", "veh.Cruise=veh.Speed"]);
    assert_eq!(code, 0, "stdout: {stdout}\nstderr: {stderr}");
    let text = lock_text(&dir);
    for line in [
        "veh.Speed/backup 1\n",
        "veh.Speed/primary 2\n",
        "veh.Speed/old 4 retired\n",
    ] {
        assert!(text.contains(line), "{line} missing from:\n{text}");
    }
    assert!(!text.contains("Cruise"), "{text}");
}

#[test]
fn retire_a_component_retires_all_its_instances() {
    let dir = lock_workspace("retire-component", Some(&allocated_lock()));
    dir.write(
        "system.rsdl",
        "package veh\nsystem Vehicle { veh.status }\n",
    );
    let (code, stdout, stderr) = run_lock(&dir, &["--retire", "veh.Cruise"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("retired veh.Cruise/backup 1"), "{stdout}");
    assert!(stdout.contains("retired veh.Cruise/primary 2"), "{stdout}");
    assert_eq!(
        lock_text(&dir),
        format!(
            "{HEADER}next 4\nveh.Cruise/backup 1 retired\nveh.Cruise/primary 2 retired\nveh.status/Unit 3\n"
        )
    );
}

/// A workspace of one member `veh` under a root manifest, on `LOCK_SYSTEM`.
fn member_workspace(label: &str, lock: Option<&str>) -> TempDir {
    let dir = TempDir::new(label);
    dir.write("ridl.toml", "[workspace]\nmembers = [\"veh\"]\n");
    dir.write("veh/ridl.toml", MANIFEST);
    dir.write("veh/system.rsdl", LOCK_SYSTEM);
    dir.write("veh/status.ridl", STATUS_SERVICE);
    if let Some(body) = lock {
        dir.write("instances.lock", body);
    }
    dir
}

#[test]
fn lock_on_a_member_path_writes_no_instance_lock() {
    let dir = member_workspace("member", None);
    let member = dir.path().join("veh");
    let (code, stdout, stderr) = ridl(&["lock".as_ref(), member.as_os_str()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(!stdout.contains("allocated veh.Cruise"), "{stdout}");
    assert!(!dir.path().join("instances.lock").exists());
}

#[test]
fn an_instance_edit_on_a_member_path_exits_two() {
    let dir = member_workspace("member-edit", Some(&allocated_lock()));
    let member = dir.path().join("veh");
    let (code, _, stderr) = ridl(&[
        "lock".as_ref(),
        member.as_os_str(),
        "--retire".as_ref(),
        "veh.Cruise/backup".as_ref(),
    ]);
    assert_eq!(code, 2, "{stderr}");
    assert!(
        stderr.contains("`instances.lock` belongs to the workspace root: run `ridl lock <root>`"),
        "{stderr}"
    );
}

#[test]
fn a_slash_on_one_side_of_a_rename_exits_two() {
    let dir = lock_workspace("slash", Some(&allocated_lock()));
    let (code, _, stderr) = run_lock(&dir, &["--rename", "veh.Cruise/backup=veh.Speed"]);
    assert_eq!(code, 2, "{stderr}");
}

#[test]
fn the_merge_driver_merges_two_instance_allocations() {
    let dir = TempDir::new("merge");
    let base = dir.write("base", &format!("{HEADER}next 2\nveh.C/a 1\n"));
    let ours = dir.write("ours", &format!("{HEADER}next 3\nveh.C/a 1\nveh.C/b 2\n"));
    let theirs = dir.write("theirs", &format!("{HEADER}next 3\nveh.C/a 1\nveh.C/c 2\n"));
    let (code, _, stderr) = ridl(&[
        "lock".as_ref(),
        "merge".as_ref(),
        base.as_os_str(),
        ours.as_os_str(),
        theirs.as_os_str(),
        "7".as_ref(),
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(
        std::fs::read_to_string(&ours).unwrap(),
        format!("{HEADER}next 4\nveh.C/a 1\nveh.C/b 2\nveh.C/c 3\n")
    );
}
