//! `ridl check --format json` prints the JSON diagnostic contract to stdout.

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A unique directory under the system temp dir, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let mut path = std::env::temp_dir();
        path.push(format!(
            "ridl-check-json-{label}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst),
        ));
        std::fs::create_dir_all(&path).expect("create the temp dir");
        Self(path)
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

#[test]
fn json_format_prints_the_contract_and_keeps_the_exit_code() {
    let dir = TempDir::new("broken");
    // No trailing newline: a trailing `\n` moves the parser's EOF diagnostic
    // to a phantom empty line 3, and the assertion below pins line 2.
    let path = dir.write("broken.typl", "package p\ntype X:");

    let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
        .args(["check", "--format", "json"])
        .arg(&path)
        .output()
        .expect("run ridl check");

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stdout = String::from_utf8(output.stdout).expect("utf-8 stdout");
    let diagnostics: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is JSON");
    let diagnostics = diagnostics.as_array().expect("a JSON array");
    let error = diagnostics
        .iter()
        .find(|diagnostic| diagnostic["severity"] == "error")
        .expect("an error diagnostic");
    assert!(error["code"].as_str().is_some_and(|code| !code.is_empty()));
    assert_eq!(error["span"]["start"]["line"], 2);
    assert!(error["labels"].is_array());
    assert!(error["fixes"].is_array());
}

#[test]
fn json_format_prints_an_empty_array_for_a_clean_file() {
    let dir = TempDir::new("clean");
    let path = dir.write("clean.typl", "package p\n");

    let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
        .args(["check", "--format", "json"])
        .arg(&path)
        .output()
        .expect("run ridl check");

    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "[]");
}
