//! The check on `.githooks/pre-push-gate.sh`, the pre-push hook command.
//!
//! git gives a pre-push hook one line per ref on stdin:
//! `<local ref> <local sha> <remote ref> <remote sha>`, and git-std forwards
//! them to the command. The script skips `just pre-push` when every remote ref
//! is a tag, and runs it otherwise, also when there is no line: git sends a
//! line for every real push, so an empty stdin means the lines were lost, and
//! the gate runs instead of being skipped without notice. Each test runs
//! the script with a fake `just` first on `PATH`, which writes a marker file
//! and exits 7. The marker holds the arguments `just` was given.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

const SHA: &str = "1111111111111111111111111111111111111111";
const ZEROS: &str = "0000000000000000000000000000000000000000";

/// Runs the script with `stdin`; returns its exit code and the arguments the
/// fake `just` was given, or `None` when it did not run.
fn run_gate(test: &str, stdin: &str) -> (i32, Option<String>) {
    let dir = std::env::temp_dir().join(format!("xtask-pre-push-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let marker = dir.join("ran");
    let fake = dir.join("just");
    std::fs::write(
        &fake,
        format!("#!/bin/sh\necho \"$*\" > '{}'\nexit 7\n", marker.display()),
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
    (
        status.code().unwrap_or(-1),
        std::fs::read_to_string(&marker)
            .ok()
            .map(|args| args.trim().to_string()),
    )
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn tag_only_push_skips_the_gate() {
    let stdin = format!("refs/tags/v0.15.0 {SHA} refs/tags/v0.15.0 {ZEROS}\n");
    assert_eq!(run_gate("tag", &stdin), (0, None));
}

#[test]
fn tag_deletion_skips_the_gate() {
    let stdin = format!("(delete) {ZEROS} refs/tags/v0.15.0 {SHA}\n");
    assert_eq!(run_gate("delete", &stdin), (0, None));
}

#[test]
fn branch_push_runs_the_gate() {
    let stdin = format!("refs/heads/main {SHA} refs/heads/main {SHA}\n");
    assert_eq!(
        run_gate("branch", &stdin),
        (7, Some("pre-push".to_string()))
    );
}

#[test]
fn mixed_push_runs_the_gate() {
    let stdin = format!(
        "refs/tags/v0.15.0 {SHA} refs/tags/v0.15.0 {ZEROS}\n\
         refs/heads/main {SHA} refs/heads/main {SHA}\n"
    );
    assert_eq!(run_gate("mixed", &stdin), (7, Some("pre-push".to_string())));
}

#[test]
fn empty_stdin_runs_the_gate() {
    assert_eq!(run_gate("empty", ""), (7, Some("pre-push".to_string())));
}

#[test]
fn branch_then_tag_runs_the_gate() {
    let stdin = format!(
        "refs/heads/main {SHA} refs/heads/main {SHA}\n\
         refs/tags/v0.15.0 {SHA} refs/tags/v0.15.0 {ZEROS}\n"
    );
    assert_eq!(
        run_gate("branch-tag", &stdin),
        (7, Some("pre-push".to_string()))
    );
}

#[test]
fn branch_deletion_runs_the_gate() {
    let stdin = format!("(delete) {ZEROS} refs/heads/topic {SHA}\n");
    assert_eq!(
        run_gate("branch-delete", &stdin),
        (7, Some("pre-push".to_string()))
    );
}
