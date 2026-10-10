//! A path that cannot be read is reported with the operating system's cause
//! and the path that failed (driftsys/ridl#196). The exit code stays 2.
//!
//! Unix only: the fixtures use `chmod 000`, which does not bind the root user,
//! so every test returns early when it runs as root.

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const MANIFEST: &str = "[package]\nname = \"veh.cluster\"\nversion = \"1.0.0\"\n";
const SOURCE: &str = "package veh.cluster\n";

/// A temp directory whose permissions are restored before it is removed.
struct Fixture {
    root: PathBuf,
    restore: Vec<PathBuf>,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("ridlc-unreadable-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create the fixture");
        Fixture {
            root,
            restore: Vec::new(),
        }
    }

    fn lock_down(&mut self, path: &Path) {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o000)).expect("chmod 000");
        self.restore.push(path.to_path_buf());
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for path in &self.restore {
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755));
        }
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Whether the process is root, for which `chmod 000` binds nothing.
fn is_root() -> bool {
    Command::new("id")
        .arg("-u")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "0")
        .unwrap_or(false)
}

/// A readable package whose root directory is then made unreadable.
fn unreadable_root() -> Fixture {
    let mut fixture = Fixture::new("root");
    std::fs::write(fixture.root.join("ridl.toml"), MANIFEST).unwrap();
    std::fs::write(fixture.root.join("a.ridl"), SOURCE).unwrap();
    let root = fixture.root.clone();
    fixture.lock_down(&root);
    fixture
}

/// A readable package with an unreadable subdirectory `sub`.
fn unreadable_subdirectory() -> Fixture {
    let mut fixture = Fixture::new("sub");
    std::fs::write(fixture.root.join("ridl.toml"), MANIFEST).unwrap();
    std::fs::write(fixture.root.join("a.ridl"), SOURCE).unwrap();
    std::fs::create_dir(fixture.root.join("sub")).unwrap();
    let sub = fixture.root.join("sub");
    fixture.lock_down(&sub);
    fixture
}

fn run(exe: &str, args: &[&str], root: &Path) -> (i32, String) {
    let output = Command::new(exe)
        .args(args)
        .arg(root)
        .output()
        .expect("the binary must run");
    (
        output.status.code().expect("the process exits with a code"),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

const RIDLC: &str = env!("CARGO_BIN_EXE_ridlc");
const COMMANDS: &[&[&str]] = &[
    &["check"],
    &[
        "build",
        "--out-dir",
        "/nonexistent-out",
        "--emit",
        "ir-json",
    ],
];

#[test]
fn an_unreadable_root_reports_the_os_error_and_the_path() {
    if is_root() {
        return;
    }
    let fixture = unreadable_root();
    for args in COMMANDS {
        let (code, stderr) = run(RIDLC, args, &fixture.root);
        let root = fixture.root.display().to_string();
        assert_eq!(code, 2, "ridlc {args:?}: {stderr}");
        assert!(
            stderr.contains(&root),
            "ridlc {args:?} names the path: {stderr}"
        );
        assert!(
            stderr.contains("Permission denied"),
            "ridlc {args:?} names the cause: {stderr}"
        );
        assert!(
            !stderr.contains("no `ridl.toml` found"),
            "ridlc {args:?} does not claim the manifest is missing: {stderr}"
        );
    }
}

#[test]
fn an_unreadable_subdirectory_is_named_with_the_os_error() {
    if is_root() {
        return;
    }
    let fixture = unreadable_subdirectory();
    let sub = fixture.root.join("sub").display().to_string();
    for args in COMMANDS {
        let (code, stderr) = run(RIDLC, args, &fixture.root);
        assert_eq!(code, 2, "ridlc {args:?}: {stderr}");
        assert!(
            stderr.contains(&sub),
            "ridlc {args:?} names `sub`: {stderr}"
        );
        assert!(
            stderr.contains("Permission denied"),
            "ridlc {args:?} names the cause: {stderr}"
        );
    }
}
