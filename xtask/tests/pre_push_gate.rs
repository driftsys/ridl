//! The check on `.githooks/pre-push-gate.sh`, the pre-push hook command.
//!
//! git gives a pre-push hook one line per ref on stdin:
//! `<local ref> <local sha> <remote ref> <remote sha>`, and git-std forwards
//! them to the command. The script skips `just pre-push` when every remote ref
//! is a tag, or when there is no line, and runs it otherwise. Each test runs
//! the script with a fake `just` first on `PATH`, which writes a marker file
//! and exits 7.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const SHA: &str = "1111111111111111111111111111111111111111";
const ZEROS: &str = "0000000000000000000000000000000000000000";

/// Runs the script with `stdin`; returns its exit code and whether the fake
/// `just` ran.
fn run_gate(test: &str, stdin: &str) -> (i32, bool) {
    let dir = std::env::temp_dir().join(format!("xtask-pre-push-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let marker = dir.join("ran");
    let fake = dir.join("just");
    std::fs::write(
        &fake,
        format!("#!/bin/sh\ntouch '{}'\nexit 7\n", marker.display()),
    )
    .unwrap();
    make_executable(&fake);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let path = format!(
        "{}:{}",
        dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut child = Command::new("bash")
        .arg(root.join(".githooks/pre-push-gate.sh"))
        .env("PATH", path)
        .stdin(Stdio::piped())
        .spawn()
        .expect("bash must run");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let status = child.wait().unwrap();
    (status.code().unwrap_or(-1), marker.exists())
}

#[cfg(unix)]
fn make_executable(path: &PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn tag_only_push_skips_the_gate() {
    let stdin = format!("refs/tags/v0.15.0 {SHA} refs/tags/v0.15.0 {ZEROS}\n");
    assert_eq!(run_gate("tag", &stdin), (0, false));
}

#[test]
fn tag_deletion_skips_the_gate() {
    let stdin = format!("(delete) {ZEROS} refs/tags/v0.15.0 {SHA}\n");
    assert_eq!(run_gate("delete", &stdin), (0, false));
}

#[test]
fn branch_push_runs_the_gate() {
    let stdin = format!("refs/heads/main {SHA} refs/heads/main {SHA}\n");
    assert_eq!(run_gate("branch", &stdin), (7, true));
}

#[test]
fn mixed_push_runs_the_gate() {
    let stdin = format!(
        "refs/tags/v0.15.0 {SHA} refs/tags/v0.15.0 {ZEROS}\n\
         refs/heads/main {SHA} refs/heads/main {SHA}\n"
    );
    assert_eq!(run_gate("mixed", &stdin), (7, true));
}

#[test]
fn empty_push_skips_the_gate() {
    assert_eq!(run_gate("empty", ""), (0, false));
}
