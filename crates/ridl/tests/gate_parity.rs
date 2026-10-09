//! Tests of the `gate-parity` recipe: a member removed from, or added to, the
//! `build` dependency line has to fail the recipe and be named in its output.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "ridl-gate-parity-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn write(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

const WORKFLOW: &str = ".github/workflows/ci.yml";

fn read_repo(path: &str) -> String {
    std::fs::read_to_string(repo().join(path)).unwrap()
}

/// The members on the `build:` line of the real justfile.
fn build_members(justfile: &str) -> Vec<String> {
    let line = justfile
        .lines()
        .find(|line| line.starts_with("build:"))
        .expect("the justfile has a build: line");
    line["build:".len()..]
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

/// Replaces the `build:` line with one that holds exactly `members`.
fn with_members(justfile: &str, members: &[String]) -> String {
    let mut out = String::new();
    for line in justfile.lines() {
        if line.starts_with("build:") {
            out.push_str(&format!("build: {}\n", members.join(" ")));
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// Copies the justfile and the workflow into a fresh directory, with the given
/// replacements, and runs `just gate-parity` there.
fn run_gate_parity(justfile: &str, workflow: &str) -> Output {
    let dir = TestDir::new();
    dir.write("justfile", justfile);
    dir.write(WORKFLOW, workflow);
    Command::new("just")
        .arg("gate-parity")
        .current_dir(&dir.0)
        .output()
        .expect("just is installed")
}

/// The words after `prefix` on the first stderr line that starts with it.
fn named_after(output: &Output, prefix: &str) -> Vec<String> {
    let stderr = String::from_utf8_lossy(&output.stderr);
    stderr
        .lines()
        .find_map(|line| line.strip_prefix(prefix))
        .map(|rest| rest.split_whitespace().map(str::to_string).collect())
        .unwrap_or_default()
}

#[test]
fn the_unmodified_gate_passes() {
    let output = run_gate_parity(&read_repo("justfile"), &read_repo(WORKFLOW));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_member_removed_from_build_fails_and_is_named() {
    let justfile = read_repo("justfile");
    let workflow = read_repo(WORKFLOW);
    let members = build_members(&justfile);
    assert!(members.len() > 10, "members read: {members:?}");
    for member in &members {
        let kept: Vec<String> = members.iter().filter(|m| *m != member).cloned().collect();
        let output = run_gate_parity(&with_members(&justfile, &kept), &workflow);
        assert!(
            !output.status.success(),
            "removing {member} from build left gate-parity green"
        );
        assert_eq!(
            named_after(&output, "gate-parity: removed from 'build':"),
            vec![member.clone()],
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn a_member_added_to_build_and_to_the_workflow_fails_and_is_named() {
    let justfile = read_repo("justfile");
    let workflow = read_repo(WORKFLOW);
    let mut members = build_members(&justfile);
    // `book` is a recipe that exists and is not a member of build.
    assert!(!members.iter().any(|m| m == "book"));
    members.push("book".to_string());
    let workflow = format!("{workflow}\n      - run: just book\n");
    let output = run_gate_parity(&with_members(&justfile, &members), &workflow);
    assert!(
        !output.status.success(),
        "adding book left gate-parity green"
    );
    assert_eq!(
        named_after(&output, "gate-parity: added to 'build':"),
        vec!["book".to_string()],
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
