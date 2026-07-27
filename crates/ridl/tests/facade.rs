//! Integration tests for the `ridl` porcelain facade (docs/ROADMAP.md epic
//! E1.13, E1.14): `check` / `build` delegating to the compiler, humane default
//! paths, and `ridl fmt` (rewrite in place, `--check`, refuse a broken file).

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
            "ridl-facade-{label}-{}-{}",
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

/// Runs `ridl` with `args`, returning `(exit_code, stderr)`.
fn ridl(args: &[&std::ffi::OsStr]) -> (i32, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
        .args(args)
        .output()
        .expect("the ridl binary must run");
    let code = output.status.code().expect("the process exits with a code");
    (code, String::from_utf8_lossy(&output.stderr).into_owned())
}

const SPEED_SOURCE: &str = "package veh.common\ntype Speed: km/h [0.0..250.0 step 0.5]\n";
const PACKAGE_MANIFEST: &str = "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n";

/// `ridl check <file>` on a clean single file exits 0.
#[test]
fn check_clean_file_exits_zero() {
    let dir = TempDir::new("check");
    let file = dir.write("speed.typl", SPEED_SOURCE);
    let (code, stderr) = ridl(&["check".as_ref(), file.as_os_str()]);
    assert_eq!(code, 0, "a clean file must exit 0, stderr:\n{stderr}");
}

/// `ridl build <file> --out-dir <tmp>` writes the single-file `<stem>.rs`.
#[test]
fn build_single_file_writes_stem_rs() {
    let dir = TempDir::new("build");
    let file = dir.write("speed.typl", SPEED_SOURCE);
    let out = TempDir::new("build-out");
    let (code, stderr) = ridl(&[
        "build".as_ref(),
        file.as_os_str(),
        "--out-dir".as_ref(),
        out.path().as_os_str(),
    ]);
    assert_eq!(code, 0, "a clean build must exit 0, stderr:\n{stderr}");
    let generated = std::fs::read_to_string(out.path().join("speed.rs"))
        .expect("single-file mode writes <input-stem>.rs");
    assert!(generated.contains("pub struct Speed"));
}

/// `ridl check` with no PATH defaults to the current directory.
#[test]
fn check_defaults_to_current_directory() {
    let dir = TempDir::new("default-path");
    dir.write("ridl.toml", PACKAGE_MANIFEST);
    dir.write("speed.typl", SPEED_SOURCE);
    let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
        .arg("check")
        .current_dir(dir.path())
        .output()
        .expect("the ridl binary must run");
    assert_eq!(
        output.status.code(),
        Some(0),
        "a clean package in the current directory must check clean, stderr:\n{}",
        String::from_utf8_lossy(&output.stderr),
    );
}

/// `ridl fmt <file>` rewrites a non-canonical file in place to the tight style.
#[test]
fn fmt_rewrites_in_place() {
    let dir = TempDir::new("fmt-rewrite");
    let input = "package p\ntype Speed  :  km/h [0.0..250.0 step 0.5]\n";
    let file = dir.write("speed.typl", input);

    let (code, stderr) = ridl(&["fmt".as_ref(), file.as_os_str()]);
    assert_eq!(
        code, 0,
        "formatting a clean file exits 0, stderr:\n{stderr}"
    );

    let formatted = std::fs::read_to_string(&file).expect("the file is rewritten");
    assert_ne!(formatted, input, "the non-canonical file must be rewritten");
    assert!(
        formatted.contains("type Speed: km/h"),
        "the colon must be tightened, got:\n{formatted}"
    );

    // A second `--check` pass is now a fixed point: nothing left to change.
    let (recheck, _) = ridl(&["fmt".as_ref(), "--check".as_ref(), file.as_os_str()]);
    assert_eq!(
        recheck, 0,
        "the formatted file is a fixed point under --check"
    );
}

/// `ridl fmt --check` detects a file that would change: exit 1, no rewrite.
#[test]
fn fmt_check_detects_and_does_not_write() {
    let dir = TempDir::new("fmt-check");
    let input = "package p\ntype Speed  :  km/h [0.0..250.0 step 0.5]\n";
    let file = dir.write("speed.typl", input);

    let (code, _) = ridl(&["fmt".as_ref(), "--check".as_ref(), file.as_os_str()]);
    assert_eq!(code, 1, "a would-change file exits 1 under --check");

    let unchanged = std::fs::read_to_string(&file).expect("the file is still readable");
    assert_eq!(unchanged, input, "--check must never rewrite the file");
}

/// `ridl fmt` refuses to reformat a file with parse errors: exit 1, no rewrite,
/// and the parse diagnostics render.
#[test]
fn fmt_refuses_a_broken_file() {
    let dir = TempDir::new("fmt-broken");
    let input = "package p\ntype X: integer [0..10ms]\n";
    let file = dir.write("broken.typl", input);

    let (code, stderr) = ridl(&["fmt".as_ref(), file.as_os_str()]);
    assert_eq!(code, 1, "a broken file must not format cleanly");
    assert!(
        stderr.contains("TYPL-302"),
        "the parse diagnostics must render, got:\n{stderr}"
    );

    let unchanged = std::fs::read_to_string(&file).expect("the file is still readable");
    assert_eq!(unchanged, input, "a broken file must never be rewritten");
}

/// `ridl fmt` on a path that does not exist is a usage error, exit 2, naming
/// the cause (driftsys/ridl#194). Before the fix, `collect_source_files`
/// walked the missing path, found nothing, and `ridl fmt` reported success —
/// the one subcommand that did not fail closed on a bad path.
#[test]
fn fmt_on_a_missing_path_exits_two() {
    let dir = TempDir::new("fmt-missing");
    let missing = dir.path().join("does-not-exist");

    let (code, stderr) = ridl(&["fmt".as_ref(), missing.as_os_str()]);
    assert_eq!(
        code, 2,
        "a missing path is a usage error, stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("cannot read"),
        "the message must name the cause, got:\n{stderr}"
    );
}

/// `ridl fmt --check` does not treat an unreadable directory reached mid-walk
/// as zero files (driftsys/ridl#194). Before the fix, `chmod 000` on a
/// subdirectory made a would-reformat tree exit 0 with no output on either
/// stream — the defect that matters, because it is what let a permissions
/// change flip `ridl fmt --check` from failing the CI gate to passing it
/// silently.
#[cfg(unix)]
#[test]
fn fmt_on_an_unreadable_subdirectory_exits_two() {
    use std::os::unix::fs::PermissionsExt;

    // Restores the subdirectory's permissions on scope exit, panic or not, so
    // a failed assertion below never leaves an unreadable directory behind for
    // the `TempDir`'s own `Drop` to trip over.
    struct RestorePermissions(PathBuf);
    impl Drop for RestorePermissions {
        fn drop(&mut self) {
            let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
        }
    }

    let dir = TempDir::new("fmt-unreadable");
    dir.write(
        "a.typl",
        "package p\n\ntype Speed: km/h [0.0..250.0 step 0.5]\n",
    );
    let sub = dir.path().join("sub");
    std::fs::create_dir_all(&sub).expect("create the subdirectory");
    dir.write("sub/b.typl", "package q\n\ntype Level: integer [0..10]\n");

    std::fs::set_permissions(&sub, std::fs::Permissions::from_mode(0o000))
        .expect("chmod the subdirectory unreadable");
    let _restore = RestorePermissions(sub);

    let (code, stderr) = ridl(&["fmt".as_ref(), "--check".as_ref(), dir.path().as_os_str()]);

    assert_eq!(
        code, 2,
        "an unreadable directory reached mid-walk must not read as zero files, stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("cannot read"),
        "the message must name the cause, got:\n{stderr}"
    );
    assert!(
        stderr.contains("sub"),
        "the message must name the unreadable directory itself, got:\n{stderr}"
    );
}

/// `ridl --version` and its short form `-V` both report the binary's own name
/// and version and exit 0 (driftsys/ridl#194); before the fix both flags were
/// unrecognised arguments and exited 2.
#[test]
fn version_flag_and_short_form_both_exit_zero() {
    for flag in ["--version", "-V"] {
        let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
            .arg(flag)
            .output()
            .expect("the ridl binary must run");
        assert_eq!(
            output.status.code(),
            Some(0),
            "`ridl {flag}` must exit 0, stderr:\n{}",
            String::from_utf8_lossy(&output.stderr),
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.trim_start().starts_with("ridl "),
            "`ridl {flag}` must report the binary's own name, got:\n{stdout}"
        );
    }
}
