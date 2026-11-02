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
