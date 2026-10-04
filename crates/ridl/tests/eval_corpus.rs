//! The public evaluation corpus must compile, fit its source budget, and
//! record the origin and interaction rule of every workspace.

use std::path::{Path, PathBuf};
use std::process::Command;

fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../evals/corpus")
}

fn corpus_dirs() -> Vec<PathBuf> {
    let root = corpus_root();
    if !root.exists() {
        return Vec::new();
    }
    let mut dirs: Vec<_> = std::fs::read_dir(&root)
        .expect("read the corpus root")
        .map(|entry| entry.expect("read a corpus entry"))
        .filter(|entry| entry.file_type().expect("read a corpus file type").is_dir())
        .map(|entry| entry.path())
        .collect();
    dirs.sort();
    dirs
}

/// Counts all physical source lines, including comments and blank lines.
fn source_lines(dir: &Path) -> usize {
    std::fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("read {}: {error}", dir.display()))
        .map(|entry| {
            let entry = entry.expect("read a corpus entry");
            let path = entry.path();
            let kind = entry.file_type().expect("read a corpus file type");
            if kind.is_dir() {
                source_lines(&path)
            } else if kind.is_file()
                && matches!(
                    path.extension().and_then(|extension| extension.to_str()),
                    Some("typl" | "ridl" | "rsdl")
                )
            {
                std::fs::read_to_string(&path)
                    .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
                    .lines()
                    .count()
            } else {
                0
            }
        })
        .sum()
}

#[test]
fn every_corpus_workspace_checks_without_an_error() {
    let dirs = corpus_dirs();
    assert!(!dirs.is_empty(), "the evaluation corpus must not be empty");
    for dir in dirs {
        let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
            .args(["check", "--format", "json"])
            .arg(&dir)
            .output()
            .expect("run ridl check on a corpus workspace");
        let diagnostics: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|error| {
                panic!(
                    "{}: invalid diagnostic JSON: {error}\nstdout: {}\nstderr: {}",
                    dir.display(),
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr),
                )
            });
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic["severity"] != "error"),
            "{} has error diagnostics: {diagnostics:#?}",
            dir.display(),
        );
        assert!(
            output.status.success(),
            "{}: ridl check exited {}\nstderr: {}",
            dir.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr),
        );
    }
}

#[test]
fn the_corpus_stays_inside_its_budget() {
    let root = corpus_root();
    for (name, budget) in [("ros2", 2_500), ("mavlink", 2_500), ("vss", 1_000)] {
        let dir = root.join(name);
        if dir.exists() {
            let lines = source_lines(&dir);
            assert!(
                lines <= budget,
                "{name}: {lines} source lines exceed {budget}"
            );
        }
    }
    let total: usize = corpus_dirs().iter().map(|dir| source_lines(dir)).sum();
    assert!(
        total <= 6_000,
        "{total} source lines exceed the 6,000-line corpus budget"
    );
}

#[test]
fn every_corpus_workspace_records_its_provenance() {
    for dir in corpus_dirs() {
        for filename in ["PROVENANCE.md", "LICENSE"] {
            let path = dir.join(filename);
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            assert!(
                !text.trim().is_empty(),
                "{} must not be empty",
                path.display()
            );
            if filename == "PROVENANCE.md" {
                for header in ["Upstream:", "Revision:", "Licence:", "Kind rule:"] {
                    assert!(
                        text.lines().any(|line| line.starts_with(header)),
                        "{} is missing {header}",
                        path.display(),
                    );
                }
            }
        }
    }
}
