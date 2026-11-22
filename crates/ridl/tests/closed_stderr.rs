//! A command whose stderr reader has gone still returns the exit code it
//! intended. A write error on stderr is ignored: it never panics (exit 101)
//! and never replaces the exit code of the command.

#![cfg(unix)]

use std::io;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;

// On macOS, pipe creation and setting close-on-exec are separate operations.
// Serialize pipe creation through spawn so another child cannot inherit a reader.
static PIPE_LOCK: Mutex<()> = Mutex::new(());

fn exit_code_with_closed_stderr(dir: &Path, args: &[&str]) -> Option<i32> {
    let guard = PIPE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (reader, writer) = io::pipe().expect("create stderr pipe");
    drop(reader);
    let mut child = Command::new(env!("CARGO_BIN_EXE_ridl"))
        .current_dir(dir)
        .env("HOME", dir)
        .args(args)
        .stdout(Stdio::null())
        .stderr(writer)
        .spawn()
        .expect("spawn ridl with no stderr reader");
    drop(guard);
    child.wait().expect("run ridl with no stderr reader").code()
}

fn missing_path_exits_2(subcommand: &str) {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("missing");
    let code = exit_code_with_closed_stderr(dir.path(), &[subcommand, missing.to_str().unwrap()]);
    assert_eq!(code, Some(2), "`ridl {subcommand}` on a missing path");
}

#[test]
fn check_exits_2_when_stderr_reader_is_gone() {
    missing_path_exits_2("check");
}

#[test]
fn describe_exits_2_when_stderr_reader_is_gone() {
    missing_path_exits_2("describe");
}

#[test]
fn fmt_exits_2_when_stderr_reader_is_gone() {
    missing_path_exits_2("fmt");
}
