//! Behaviour of `cargo xtask freeze check`, run through the built binary
//! against temporary git repositories.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Repo {
    dir: PathBuf,
}

impl Repo {
    fn new(tag: &str) -> Repo {
        let dir = std::env::temp_dir().join(format!("xtask-freeze-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("freeze")).unwrap();
        let repo = Repo { dir };
        repo.git(&["init", "-q"]);
        repo.git(&["config", "user.name", "Test"]);
        repo.git(&["config", "user.email", "test@example.com"]);
        repo.git(&["config", "commit.gpgsign", "false"]);
        repo
    }

    fn write(&self, relative: &str, text: &str) {
        std::fs::write(self.dir.join(relative), text).unwrap();
    }

    fn git(&self, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(&self.dir)
            .args(args)
            .env_remove("GIT_DIR")
            .env_remove("GIT_INDEX_FILE")
            .env_remove("GIT_WORK_TREE")
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    }

    fn commit(&self) {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", "freeze"]);
    }

    fn check(&self, fixture: Option<(&str, &Path)>) -> Output {
        self.run("check", fixture)
    }

    fn run(&self, subcommand: &str, fixture: Option<(&str, &Path)>) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
        command
            .args(["freeze", subcommand, "--fixtures-only", "--root"])
            .arg(&self.dir);
        if let Some((name, file)) = fixture {
            command
                .arg("--dumper-fixture")
                .arg(format!("{name}={}", file.display()));
        }
        command
            .env_remove("GIT_DIR")
            .env_remove("GIT_INDEX_FILE")
            .env_remove("GIT_WORK_TREE")
            .output()
            .unwrap()
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn check_fails_on_a_row_without_a_dumper() {
    let repo = Repo::new("no-dumper");
    repo.write(
        "freeze/surfaces.toml",
        "[[surface]]\nname = \"ghost\"\nbaseline = \"HEAD\"\n",
    );
    repo.commit();
    let output = repo.check(None);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("ghost"), "{}", stderr(&output));
}

#[test]
fn check_fails_when_the_baseline_has_no_dump() {
    let repo = Repo::new("no-dump");
    // The first commit holds no freeze/fixture.txt; it is the baseline.
    repo.write("freeze/surfaces.toml", "");
    repo.commit();
    let fixture = repo.dir.join("fixture-lines.txt");
    std::fs::write(&fixture, "a\n").unwrap();
    repo.write(
        "freeze/surfaces.toml",
        "[[surface]]\nname = \"fixture\"\nbaseline = \"HEAD\"\n",
    );
    repo.write("freeze/fixture.txt", "a\n");
    repo.commit();
    // The working tree names HEAD~1, the first commit, as the baseline.
    repo.write(
        "freeze/surfaces.toml",
        "[[surface]]\nname = \"fixture\"\nbaseline = \"HEAD~1\"\n",
    );
    let output = repo.check(Some(("fixture", &fixture)));
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("freeze/fixture.txt"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn check_fails_when_the_baseline_ref_is_unknown() {
    let repo = Repo::new("bad-ref");
    let fixture = repo.dir.join("fixture-lines.txt");
    std::fs::write(&fixture, "a\n").unwrap();
    repo.write(
        "freeze/surfaces.toml",
        "[[surface]]\nname = \"fixture\"\nbaseline = \"no-such-ref\"\n",
    );
    repo.write("freeze/fixture.txt", "a\n");
    repo.commit();
    let output = repo.check(Some(("fixture", &fixture)));
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("no-such-ref"),
        "{}",
        stderr(&output)
    );
}

const ROW: &str = "[[surface]]\nname = \"fixture\"\nbaseline = \"base\"\n";

/// A repository whose tag `base` holds freeze/fixture.txt = `baseline`; HEAD
/// holds the row, the committed dump `committed` and the fixture lines `head`
/// (returned as the path of the fixture file).
fn repo_with_baseline(tag: &str, baseline: &str, committed: &str, head: &str) -> (Repo, PathBuf) {
    let repo = Repo::new(tag);
    repo.write("freeze/surfaces.toml", "");
    repo.write("freeze/fixture.txt", baseline);
    repo.commit();
    repo.git(&["tag", "base"]);
    repo.write("freeze/surfaces.toml", ROW);
    repo.write("freeze/fixture.txt", committed);
    repo.commit();
    let fixture = repo.dir.join("fixture-lines.txt");
    std::fs::write(&fixture, head).unwrap();
    (repo, fixture)
}

#[test]
fn check_fails_when_a_baseline_line_is_missing_at_head() {
    let (repo, fixture) = repo_with_baseline("removed", "a\nb\n", "a\n", "a\n");
    let output = repo.check(Some(("fixture", &fixture)));
    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(err.contains("`b`") && err.contains("base"), "{err}");
}

#[test]
fn check_passes_when_a_line_is_added_since_the_baseline() {
    let (repo, fixture) = repo_with_baseline("added", "a\nb\n", "a\nb\nc\n", "a\nb\nc\n");
    let output = repo.check(Some(("fixture", &fixture)));
    assert!(output.status.success(), "{}", stderr(&output));
}

#[test]
fn check_fails_when_the_committed_dump_is_stale() {
    let (repo, fixture) = repo_with_baseline("stale", "a\n", "a\n", "a\nb\n");
    let output = repo.check(Some(("fixture", &fixture)));
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("cargo xtask freeze update"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn check_fails_when_the_committed_dump_is_absent() {
    let (repo, fixture) = repo_with_baseline("absent", "a\n", "a\n", "a\n");
    std::fs::remove_file(repo.dir.join("freeze/fixture.txt")).unwrap();
    let output = repo.check(Some(("fixture", &fixture)));
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("cargo xtask freeze update"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn check_fails_on_a_dump_no_dumper_produces() {
    let (repo, fixture) = repo_with_baseline("orphan", "a\n", "a\n", "a\n");
    repo.write("freeze/orphan.txt", "x\n");
    let output = repo.check(Some(("fixture", &fixture)));
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("freeze/orphan.txt"),
        "{}",
        stderr(&output)
    );
    // README.md is not a dump.
    std::fs::remove_file(repo.dir.join("freeze/orphan.txt")).unwrap();
    repo.write("freeze/README.md", "text\n");
    let output = repo.check(Some(("fixture", &fixture)));
    assert!(output.status.success(), "{}", stderr(&output));
}

#[test]
fn update_writes_sorted_deduplicated_lines() {
    let repo = Repo::new("update");
    let fixture = repo.dir.join("fixture-lines.txt");
    std::fs::write(&fixture, "b\na\nb\n\nc\n").unwrap();
    let output = repo.run("update", Some(("fixture", &fixture)));
    assert!(output.status.success(), "{}", stderr(&output));
    let written = std::fs::read_to_string(repo.dir.join("freeze/fixture.txt")).unwrap();
    assert_eq!(written, "a\nb\nc\n");
}

/// The fixture crate of xtask/tests/freeze-fixtures/api/.
fn api_fixture(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/freeze-fixtures/api")
        .join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

const API_ROW: &str = "[[surface]]\nname = \"generated-crate\"\nbaseline = \"base\"\n";

/// A repository whose tag `base` holds the fixture crate under
/// freeze/generated/api/; HEAD holds the row for `generated-crate` and the
/// crate with `lib_rs` as its source and `cargo_toml` as its manifest.
fn repo_with_api_baseline(tag: &str, cargo_toml: &str, lib_rs: &str) -> Repo {
    let repo = Repo::new(tag);
    std::fs::create_dir_all(repo.dir.join("freeze/generated/api/src")).unwrap();
    repo.write("freeze/surfaces.toml", "");
    repo.write(
        "freeze/generated/api/Cargo.toml",
        &api_fixture("Cargo.toml"),
    );
    repo.write(
        "freeze/generated/api/src/lib.rs",
        &api_fixture("src/lib.rs"),
    );
    repo.commit();
    repo.git(&["tag", "base"]);
    repo.write("freeze/surfaces.toml", API_ROW);
    repo.write("freeze/generated/api/Cargo.toml", cargo_toml);
    repo.write("freeze/generated/api/src/lib.rs", lib_rs);
    repo.commit();
    repo
}

/// `cargo xtask freeze check` with only the semver comparison of the
/// `generated-crate` surface, and a cargo target directory of the repository's
/// own, so that tests running at the same time do not share one.
fn check_generated_semver(repo: &Repo) -> Output {
    Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["freeze", "check", "--fixtures-only", "--api-semver"])
        .arg("generated-crate")
        .arg("--root")
        .arg(&repo.dir)
        .env("CARGO_TARGET_DIR", repo.dir.join("target"))
        .env_remove("GIT_DIR")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_WORK_TREE")
        .output()
        .unwrap()
}

#[test]
fn semver_check_flags_a_removed_item() {
    // The version also moves from 0.1.0 to 0.2.0. For a crate below 1.0,
    // cargo-semver-checks accepts a break in such a release unless the check
    // states the release type itself, and a frozen surface accepts no break.
    let cargo_toml = api_fixture("Cargo.toml").replace("0.1.0", "0.2.0");
    let lib_rs = api_fixture("src/lib.rs").replace("pub fn removed() {}", "");
    assert!(
        !lib_rs.contains("removed()"),
        "the fixture changed: {lib_rs}"
    );
    let repo = repo_with_api_baseline("semver-removed", &cargo_toml, &lib_rs);
    let output = check_generated_semver(&repo);
    assert!(!output.status.success());
    let err = stderr(&output);
    // `function_missing` is the cargo-semver-checks lint for a removed
    // function; the item path is the crate name with `-` read as `_`.
    assert!(
        err.contains("surface `generated-crate`")
            && err.contains("finds a break in the crate freeze/generated/api")
            && err.contains("the baseline `base`")
            && err.contains("function_missing")
            && err.contains("freeze_api_fixture::removed"),
        "{err}"
    );
}

#[test]
fn semver_check_passes_an_added_item() {
    let lib_rs = format!(
        "{}\n/// Added.\npub fn added() {{}}\n",
        api_fixture("src/lib.rs")
    );
    let repo = repo_with_api_baseline("semver-added", &api_fixture("Cargo.toml"), &lib_rs);
    let output = check_generated_semver(&repo);
    assert!(output.status.success(), "{}", stderr(&output));
}

#[test]
fn semver_check_fails_when_a_crate_is_missing_at_head() {
    let repo = repo_with_api_baseline(
        "semver-gone",
        &api_fixture("Cargo.toml"),
        &api_fixture("src/lib.rs"),
    );
    std::fs::remove_dir_all(repo.dir.join("freeze/generated/api")).unwrap();
    let output = check_generated_semver(&repo);
    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(
        err.contains("freeze/generated/api") && err.contains("`base`"),
        "{err}"
    );
}

#[test]
fn check_fails_when_the_semver_tool_fails() {
    // A manifest cargo cannot read is a failure of the tool, not a break.
    let repo = repo_with_api_baseline("semver-tool", "[package\n", &api_fixture("src/lib.rs"));
    let output = check_generated_semver(&repo);
    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(
        err.contains("cargo semver-checks failed") && !err.contains("finds a break"),
        "{err}"
    );
}
