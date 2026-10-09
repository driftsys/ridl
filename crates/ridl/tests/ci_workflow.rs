//! Tests of the `changes` job in `.github/workflows/ci.yml`: the path filter
//! script that decides which gate jobs a pull request runs, and the `needs` and
//! `if:` lines that act on its outputs.
//!
//! The filter test runs the real `run:` script of the `filter` step with `bash`
//! in a temporary git repository. The structure test reads the workflow text.
//! Both read the workflow through `workflow_text`, which takes the path in the
//! `CI_WORKFLOW_PATH` environment variable when it is set, so that a modified
//! copy of the file can be checked.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "ridl-ci-workflow-{}-{}",
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

/// The text of the workflow file under test.
fn workflow_text() -> String {
    let path = match std::env::var_os("CI_WORKFLOW_PATH") {
        Some(path) => PathBuf::from(path),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/workflows/ci.yml"),
    };
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// The lines of one job, from its `  name:` line to the line before the next
/// job. Jobs are the two-space-indented keys of the `jobs:` mapping.
fn job_block(workflow: &str, name: &str) -> Vec<String> {
    let lines: Vec<&str> = workflow.lines().collect();
    let jobs = lines
        .iter()
        .position(|l| *l == "jobs:")
        .expect("the workflow has a jobs: key");
    let header = format!("  {name}:");
    let start = lines[jobs..]
        .iter()
        .position(|l| *l == header)
        .unwrap_or_else(|| panic!("no job {name}"))
        + jobs;
    let mut block = vec![lines[start].to_string()];
    for line in &lines[start + 1..] {
        let is_job_key = indent_of(line) == 2 && !line.trim_start().starts_with('#');
        if is_job_key && !line.trim().is_empty() {
            break;
        }
        block.push(line.to_string());
    }
    block
}

/// The value of the job's own `key:` line (four-space indent), if present.
fn job_key(block: &[String], key: &str) -> Option<String> {
    let prefix = format!("    {key}:");
    block
        .iter()
        .find(|l| l.starts_with(&prefix))
        .map(|l| l[prefix.len()..].trim().to_string())
}

/// The script of the `run: |` key of the step whose id is `filter`, with the
/// block indentation removed.
fn filter_script(workflow: &str) -> String {
    let lines: Vec<&str> = workflow.lines().collect();
    let id = lines
        .iter()
        .position(|l| l.trim() == "- id: filter")
        .expect("the workflow has a step with id: filter");
    let run = lines[id..]
        .iter()
        .position(|l| l.trim() == "run: |")
        .expect("the filter step has a run: | key")
        + id;
    let body_indent = indent_of(lines[run]) + 2;
    let mut script = String::new();
    for line in &lines[run + 1..] {
        if !line.trim().is_empty() && indent_of(line) < body_indent {
            break;
        }
        script.push_str(line.get(body_indent..).unwrap_or(""));
        script.push('\n');
    }
    script
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(dir)
        .env_remove("GIT_DIR")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_WORK_TREE")
        .args(["-c", "user.name=test", "-c", "user.email=test@example.com"])
        .args(["-c", "commit.gpgsign=false"])
        // No host hook runs on the temporary repository, and a rename is
        // detected unless the script under test asks for it not to be.
        .args(["-c", "core.hooksPath=/dev/null", "-c", "diff.renames=true"])
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// What the second commit of the temporary repository does.
enum Change<'a> {
    /// Adds these files.
    Add(&'a [&'a str]),
    /// Renames `from` (present in the first commit) to `to`.
    Rename(&'a str, &'a str),
    /// Commits nothing.
    Empty,
}

/// Runs the filter script for `event` over a repository whose first commit
/// holds `base` and whose second commit applies `change`. Returns the contents
/// of the `GITHUB_OUTPUT` file.
fn run_filter(event: &str, base: &[&str], change: Change) -> String {
    let dir = TestDir::new();
    dir.write("base.txt", "base\n");
    for path in base {
        dir.write(path, "base\n");
    }
    git(&dir.0, &["init", "-q"]);
    git(&dir.0, &["add", "-A"]);
    git(&dir.0, &["commit", "-q", "-m", "base"]);
    match change {
        Change::Add(paths) => {
            for path in paths {
                dir.write(path, "changed\n");
            }
            git(&dir.0, &["add", "-A"]);
            git(&dir.0, &["commit", "-q", "-m", "change"]);
        }
        Change::Rename(from, to) => {
            std::fs::create_dir_all(dir.0.join(to).parent().unwrap()).unwrap();
            git(&dir.0, &["mv", from, to]);
            git(&dir.0, &["commit", "-q", "-m", "change"]);
        }
        Change::Empty => git(&dir.0, &["commit", "-q", "--allow-empty", "-m", "change"]),
    }

    dir.write("filter.sh", &filter_script(&workflow_text()));
    let output_file = dir.0.join("github-output");
    std::fs::write(&output_file, "").unwrap();
    let out = Command::new("bash")
        .current_dir(&dir.0)
        .env_remove("GIT_DIR")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_WORK_TREE")
        .env("EVENT", event)
        .env("GITHUB_OUTPUT", &output_file)
        .arg("filter.sh")
        .output()
        .expect("bash runs");
    assert!(
        out.status.success(),
        "the filter script failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    std::fs::read_to_string(output_file).unwrap()
}

fn pr(change: Change) -> String {
    run_filter("pull_request", &[], change)
}

#[test]
fn documentation_only_skips_rust_and_runs_markdown() {
    assert_eq!(
        pr(Change::Add(&["docs/wip/x.md"])),
        "rust=false\nmarkdown=true\n"
    );
}

#[test]
fn rust_source_only_runs_rust_and_skips_markdown() {
    assert_eq!(
        pr(Change::Add(&["crates/ridl/src/main.rs"])),
        "rust=true\nmarkdown=false\n"
    );
}

#[test]
fn a_mixed_change_runs_both() {
    assert_eq!(
        pr(Change::Add(&["docs/wip/x.md", "crates/ridl/src/main.rs"])),
        "rust=true\nmarkdown=true\n"
    );
}

#[test]
fn a_rename_out_of_a_skipped_path_counts_the_old_name() {
    // With rename detection the diff would list only the new name, which is a
    // skipped path, and the Rust jobs would not run.
    let out = run_filter(
        "pull_request",
        &["crates/ridl/src/old.rs"],
        Change::Rename("crates/ridl/src/old.rs", "docs/wip/old.rs"),
    );
    assert_eq!(out, "rust=true\nmarkdown=false\n");
}

#[test]
fn a_rename_into_a_skipped_path_from_a_skipped_path_skips_rust() {
    let out = run_filter(
        "pull_request",
        &["docs/wip/old.md"],
        Change::Rename("docs/wip/old.md", "docs/archive/old.md"),
    );
    assert_eq!(out, "rust=false\nmarkdown=true\n");
}

#[test]
fn editorconfig_runs_both() {
    assert_eq!(
        pr(Change::Add(&[".editorconfig"])),
        "rust=true\nmarkdown=true\n"
    );
}

#[test]
fn license_runs_both() {
    assert_eq!(pr(Change::Add(&["LICENSE"])), "rust=true\nmarkdown=true\n");
}

#[test]
fn an_empty_diff_runs_both() {
    assert_eq!(pr(Change::Empty), "rust=true\nmarkdown=true\n");
}

#[test]
fn the_eval_corpus_runs_rust() {
    assert_eq!(
        pr(Change::Add(&["evals/corpus/x.md"])),
        "rust=true\nmarkdown=true\n"
    );
}

#[test]
fn a_list_over_64_kib_is_read_whole() {
    // 4000 skipped paths of about 200 characters each make a list of about
    // 800 KiB. A `printf | grep -q` pipeline breaks once the list is past the
    // 64 KiB pipe buffer, but with only a little more than 64 KiB the break
    // depends on timing; a list this long makes it certain.
    let names: Vec<String> = (0..4000)
        .map(|i| format!("docs/wip/{}-{i}.md", "n".repeat(190)))
        .collect();
    let mut paths: Vec<&str> = names.iter().map(String::as_str).collect();
    assert!(paths.iter().map(|p| p.len() + 1).sum::<usize>() > 65_536);

    assert_eq!(
        pr(Change::Add(&paths)),
        "rust=false\nmarkdown=true\n",
        "a list of only skipped paths skips Rust"
    );

    // One Rust path sorts first in the list: grep finds its match at once.
    paths.push("a/first.rs");
    assert_eq!(
        pr(Change::Add(&paths)),
        "rust=true\nmarkdown=true\n",
        "one path that is not skipped runs Rust"
    );
}

#[test]
fn a_push_event_runs_both() {
    let out = run_filter("push", &[], Change::Add(&["docs/wip/x.md"]));
    assert_eq!(out, "rust=true\nmarkdown=true\n");
}

#[test]
fn gate_jobs_depend_on_changes_and_gate_on_its_outputs() {
    let workflow = workflow_text();
    for (job, output) in [("rust", "rust"), ("wasm", "rust"), ("markdown", "markdown")] {
        let block = job_block(&workflow, job);
        assert_eq!(
            job_key(&block, "needs").as_deref(),
            Some("changes"),
            "{job} needs changes"
        );
        assert_eq!(
            job_key(&block, "if").as_deref(),
            Some(format!("needs.changes.outputs.{output} == 'true'").as_str()),
            "{job} runs on the {output} output"
        );
    }

    let book = job_block(&workflow, "book");
    assert_eq!(job_key(&book, "if"), None, "book has no if:");

    let ci = job_block(&workflow, "ci");
    let needs = job_key(&ci, "needs").expect("ci has needs");
    let listed: Vec<&str> = needs
        .trim_matches(|c| c == '[' || c == ']')
        .split(',')
        .map(str::trim)
        .collect();
    assert!(listed.contains(&"changes"), "ci needs {listed:?}");

    let changes = job_block(&workflow, "changes");
    assert!(
        changes.iter().any(|l| l.trim() == "fetch-depth: 2"),
        "the changes checkout has fetch-depth: 2"
    );
}
