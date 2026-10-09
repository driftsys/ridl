//! A path that cannot be read is reported with the operating system's cause
//! and the path that failed (driftsys/ridl#196). The exit code stays 2.
//!
//! Unix only: the fixtures use `chmod 000`, which does not bind the root user,
//! so every test that uses it returns early when it runs as root. The test of
//! a manifest that links to itself needs no permission change and always runs.

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
            std::env::temp_dir().join(format!("ridl-unreadable-{name}-{}", std::process::id()));
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

/// Whether the test must return early because it runs as root, with a notice
/// on stderr so that the skip is visible in the test output.
fn skip_as_root() -> bool {
    let root = is_root();
    if root {
        eprintln!("skipped: the process is root, and `chmod 000` does not bind root");
    }
    root
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

const RIDL: &str = env!("CARGO_BIN_EXE_ridl");
const COMMANDS: &[&[&str]] = &[&["check"], &["build"], &["baseline"], &["test"]];

#[test]
fn an_unreadable_root_reports_the_os_error_and_the_path() {
    if skip_as_root() {
        return;
    }
    let fixture = unreadable_root();
    for args in COMMANDS {
        let (code, stderr) = run(RIDL, args, &fixture.root);
        let root = fixture.root.display().to_string();
        assert_eq!(code, 2, "ridl {args:?}: {stderr}");
        assert!(
            stderr.contains(&root),
            "ridl {args:?} names the path: {stderr}"
        );
        assert!(
            stderr.contains("Permission denied"),
            "ridl {args:?} names the cause: {stderr}"
        );
        assert!(
            !stderr.contains("no `ridl.toml` found"),
            "ridl {args:?} does not claim the manifest is missing: {stderr}"
        );
    }
}

#[test]
fn an_unreadable_subdirectory_is_named_with_the_os_error() {
    if skip_as_root() {
        return;
    }
    let fixture = unreadable_subdirectory();
    let sub = fixture.root.join("sub").display().to_string();
    for args in COMMANDS {
        let (code, stderr) = run(RIDL, args, &fixture.root);
        assert_eq!(code, 2, "ridl {args:?}: {stderr}");
        assert!(stderr.contains(&sub), "ridl {args:?} names `sub`: {stderr}");
        assert!(
            stderr.contains("Permission denied"),
            "ridl {args:?} names the cause: {stderr}"
        );
    }
}

/// Runs every command over `fixture`, expecting exit code 2 with `path` and the
/// operating system's cause on stderr.
fn assert_names_path_and_cause(fixture: &Fixture, path: &Path) {
    let path = path.display().to_string();
    for args in COMMANDS {
        let (code, stderr) = run(RIDL, args, &fixture.root);
        assert_eq!(code, 2, "ridl {args:?}: {stderr}");
        assert!(
            stderr.contains(&format!("cannot read `{path}`")),
            "ridl {args:?} names `{path}`: {stderr}"
        );
        assert!(
            stderr.contains("Permission denied"),
            "ridl {args:?} names the cause: {stderr}"
        );
    }
}

#[test]
fn an_unreadable_source_file_is_named_with_the_os_error() {
    if skip_as_root() {
        return;
    }
    let mut fixture = Fixture::new("source");
    std::fs::write(fixture.root.join("ridl.toml"), MANIFEST).unwrap();
    let source = fixture.root.join("a.ridl");
    std::fs::write(&source, SOURCE).unwrap();
    fixture.lock_down(&source);
    assert_names_path_and_cause(&fixture, &source);
}

#[test]
fn an_unreadable_member_manifest_is_named_with_the_os_error() {
    if skip_as_root() {
        return;
    }
    let mut fixture = Fixture::new("member");
    std::fs::write(
        fixture.root.join("ridl.toml"),
        "[workspace]\nmembers = [\"m\"]\n",
    )
    .unwrap();
    std::fs::create_dir(fixture.root.join("m")).unwrap();
    let manifest = fixture.root.join("m").join("ridl.toml");
    std::fs::write(&manifest, MANIFEST).unwrap();
    std::fs::write(fixture.root.join("m").join("a.ridl"), SOURCE).unwrap();
    fixture.lock_down(&manifest);
    assert_names_path_and_cause(&fixture, &manifest);
}

#[test]
fn an_unreadable_nested_manifest_is_named_with_the_os_error() {
    if skip_as_root() {
        return;
    }
    let mut fixture = Fixture::new("nested");
    std::fs::write(fixture.root.join("ridl.toml"), MANIFEST).unwrap();
    std::fs::write(fixture.root.join("a.ridl"), SOURCE).unwrap();
    std::fs::create_dir(fixture.root.join("sub")).unwrap();
    let nested = fixture.root.join("sub").join("ridl.toml");
    std::fs::write(&nested, MANIFEST).unwrap();
    fixture.lock_down(&nested);
    assert_names_path_and_cause(&fixture, &nested);
}

#[test]
fn a_manifest_that_links_to_itself_in_an_ancestor_is_named() {
    let fixture = Fixture::new("loop");
    let manifest = fixture.root.join("ridl.toml");
    std::os::unix::fs::symlink("ridl.toml", &manifest).unwrap();
    let entry = fixture.root.join("sub");
    std::fs::create_dir(&entry).unwrap();
    for args in COMMANDS {
        let (code, stderr) = run(RIDL, args, &entry);
        assert_eq!(code, 2, "ridl {args:?}: {stderr}");
        assert!(
            stderr.contains(&format!("cannot read `{}`", manifest.display())),
            "ridl {args:?} names the ancestor manifest: {stderr}"
        );
        assert!(
            stderr.contains("Too many levels of symbolic links"),
            "ridl {args:?} names the cause: {stderr}"
        );
    }
}
