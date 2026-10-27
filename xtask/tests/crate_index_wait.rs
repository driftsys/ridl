//! The check on `.github/scripts/wait-for-crate-index.sh`, which the crates.io
//! release workflow runs after each crate, so a crate publishes only once the
//! crates it depends on are in the index that cargo reads.
//!
//! Each test points `CRATES_INDEX_URL` at a `file://` directory laid out as
//! the sparse index is: `1/<name>`, `2/<name>`, `3/<first char>/<name>`, and
//! `<chars 1-2>/<chars 3-4>/<name>` for a longer name.

use std::path::{Path, PathBuf};
use std::process::Command;

/// A fresh index directory for one test.
fn index_dir(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("xtask-index-wait-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_entry(index: &Path, relative: &str, versions: &[&str]) {
    let path = index.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let lines: Vec<String> = versions
        .iter()
        .map(|version| format!("{{\"name\":\"x\",\"vers\":\"{version}\",\"deps\":[]}}"))
        .collect();
    std::fs::write(path, lines.join("\n") + "\n").unwrap();
}

/// Runs the script; returns its exit code and its stderr.
fn wait(index: &Path, name: &str, version: &str, timeout: &str) -> (i32, String) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let output = Command::new("bash")
        .arg(root.join(".github/scripts/wait-for-crate-index.sh"))
        .args([name, version])
        .env("CRATES_INDEX_URL", format!("file://{}", index.display()))
        .env("INDEX_WAIT_TIMEOUT", timeout)
        .env("INDEX_WAIT_INTERVAL", "1")
        .output()
        .expect("bash must run");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn present_version_returns_at_once() {
    let index = index_dir("present");
    write_entry(&index, "ri/dl/ridl-ir", &["0.14.0", "0.15.0"]);
    assert_eq!(wait(&index, "ridl-ir", "0.15.0", "30").0, 0);
}

#[test]
fn absent_version_times_out() {
    let index = index_dir("absent");
    write_entry(&index, "ri/dl/ridl-ir", &["0.14.0"]);
    let (code, stderr) = wait(&index, "ridl-ir", "0.15.0", "2");
    assert_eq!(code, 1);
    assert!(
        stderr.contains("ridl-ir 0.15.0 is not in the index"),
        "{stderr}"
    );
}

#[test]
fn only_the_exact_version_matches() {
    let index = index_dir("prefix");
    write_entry(&index, "ri/dl/ridl-ir", &["0.15.0-rc.1", "0a15b0"]);
    assert_eq!(wait(&index, "ridl-ir", "0.15.0", "2").0, 1);
}

#[test]
fn short_names_use_the_short_paths() {
    let index = index_dir("short");
    write_entry(&index, "1/a", &["1.0.0"]);
    write_entry(&index, "2/ab", &["1.0.0"]);
    write_entry(&index, "3/a/abc", &["1.0.0"]);
    write_entry(&index, "ab/cd/abcd", &["1.0.0"]);
    for name in ["a", "ab", "abc", "abcd"] {
        assert_eq!(wait(&index, name, "1.0.0", "2").0, 0, "{name}");
    }
}

#[test]
fn upper_case_names_are_looked_up_in_lower_case() {
    // The timeout message names the URL, so the lower-case path is checked
    // even on a case-insensitive file system, where an upper-case path would
    // also find the file.
    let index = index_dir("case");
    let (code, stderr) = wait(&index, "Ridl-IR", "0.15.0", "1");
    assert_eq!(code, 1);
    assert!(stderr.contains("/ri/dl/ridl-ir after 1 s"), "{stderr}");
}

#[test]
fn a_version_that_appears_later_is_waited_for() {
    let index = index_dir("later");
    write_entry(&index, "ri/dl/ridl-ir", &["0.14.0"]);
    let writer = {
        let index = index.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(2));
            write_entry(&index, "ri/dl/ridl-ir", &["0.14.0", "0.15.0"]);
        })
    };
    let (code, stderr) = wait(&index, "ridl-ir", "0.15.0", "30");
    writer.join().unwrap();
    assert_eq!(code, 0, "{stderr}");
}

#[test]
fn missing_file_is_waited_for_not_fatal() {
    let index = index_dir("missing");
    let (code, stderr) = wait(&index, "ridl-ir", "0.15.0", "2");
    assert_eq!(code, 1);
    assert!(
        stderr.contains("ridl-ir 0.15.0 is not in the index"),
        "{stderr}"
    );
}
