//! Behaviour of `cargo xtask freeze check`, run through the built binary
//! against temporary git repositories.
//!
//! The tests that compare a Rust API run `cargo semver-checks` and are not
//! `#[ignore]`d: `bootstrap` requires the tool, and when it is missing,
//! bootstrap prints the install command and exits with an error. These tests
//! fail when the tool is not on the `PATH`: `semver_check_flags_a_removed_item`,
//! `semver_check_passes_an_added_item`,
//! `semver_check_flags_a_lint_that_warns_by_default`,
//! `semver_check_flags_a_break_in_a_runtime_crate` and
//! `semver_check_passes_runtime_crates_with_an_added_item`.
//! `check_fails_when_the_semver_tool_fails` also starts the tool, but passes
//! without it: cargo then fails on the unknown subcommand, which the check
//! also reports as a failure of the tool. The other tests stop before the tool
//! starts.

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

    fn check_release(&self, fixture: Option<(&str, &Path)>) -> Output {
        self.run_with("check", &["--release"], fixture)
    }

    fn run(&self, subcommand: &str, fixture: Option<(&str, &Path)>) -> Output {
        self.run_with(subcommand, &[], fixture)
    }

    fn run_with(&self, subcommand: &str, extra: &[&str], fixture: Option<(&str, &Path)>) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
        command
            .args(["freeze", subcommand, "--fixtures-only"])
            .args(extra)
            .arg("--root")
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
        "required = [\"ghost\"]\n\n[[surface]]\nname = \"ghost\"\nbaseline = \"HEAD\"\n",
    );
    repo.commit();
    let output = repo.check(None);
    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(
        err.contains("has a row for `ghost`") && err.contains("no dumper or API checker"),
        "{err}"
    );
}

/// A repository whose `surfaces.toml` holds `toml`, with the dump of the
/// registered fixture surface `fixture` committed, and the fixture file.
fn repo_with_surfaces(tag: &str, toml: &str) -> (Repo, PathBuf) {
    let repo = Repo::new(tag);
    let fixture = repo.dir.join("fixture-lines.txt");
    std::fs::write(&fixture, "a\n").unwrap();
    repo.write("freeze/surfaces.toml", toml);
    repo.write("freeze/fixture.txt", "a\n");
    repo.commit();
    (repo, fixture)
}

#[test]
fn check_notes_a_required_surface_without_a_row() {
    let (repo, fixture) = repo_with_surfaces("required-note", "required = [\"fixture\"]\n");
    let output = repo.check(Some(("fixture", &fixture)));
    assert!(output.status.success(), "{}", stderr(&output));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("surface `fixture` is required but has no row"),
        "{stdout}"
    );
}

#[test]
fn release_check_fails_on_a_required_surface_without_a_row() {
    let (repo, fixture) = repo_with_surfaces("required-release", "required = [\"fixture\"]\n");
    let output = repo.check_release(Some(("fixture", &fixture)));
    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(
        err.contains("surface `fixture` is required but has no row: a release cannot leave a frozen surface without a baseline"),
        "{err}"
    );
}

#[test]
fn release_check_passes_with_an_empty_required_list() {
    let (repo, fixture) = repo_with_surfaces("required-empty", "required = []\n");
    let output = repo.check_release(Some(("fixture", &fixture)));
    assert!(output.status.success(), "{}", stderr(&output));
}

#[test]
fn check_fails_on_a_row_whose_surface_is_not_required() {
    let (repo, fixture) = repo_with_surfaces(
        "row-not-required",
        "required = []\n\n[[surface]]\nname = \"fixture\"\nbaseline = \"HEAD\"\n",
    );
    let output = repo.check(Some(("fixture", &fixture)));
    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(
        err.contains(
            "has a row for `fixture` but `fixture` is not in `required`: add it to `required`"
        ),
        "{err}"
    );
}

#[test]
fn check_passes_a_row_whose_surface_is_required() {
    let (repo, fixture) = repo_with_surfaces(
        "row-required",
        "required = [\"fixture\"]\n\n[[surface]]\nname = \"fixture\"\nbaseline = \"HEAD\"\n",
    );
    let output = repo.check(Some(("fixture", &fixture)));
    assert!(output.status.success(), "{}", stderr(&output));
    let output = repo.check_release(Some(("fixture", &fixture)));
    assert!(output.status.success(), "{}", stderr(&output));
}

#[test]
fn check_fails_on_an_unknown_name_in_required() {
    let (repo, fixture) = repo_with_surfaces("required-unknown", "required = [\"no-such\"]\n");
    let output = repo.check(Some(("fixture", &fixture)));
    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(
        err.contains("lists `no-such` in `required` but no dumper"),
        "{err}"
    );
    // The name has no surface, so no row can be added for it: the missing-row
    // message would be wrong and is not printed.
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("no-such"), "{stdout}");
    assert!(!err.contains("has no row"), "{err}");
    let output = repo.check_release(Some(("fixture", &fixture)));
    assert!(
        !stderr(&output).contains("has no row"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn update_rejects_the_release_flag() {
    let repo = Repo::new("update-release");
    let fixture = repo.dir.join("fixture-lines.txt");
    std::fs::write(&fixture, "a\n").unwrap();
    let output = repo.run_with("update", &["--release"], Some(("fixture", &fixture)));
    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(err.contains("--release applies to `check` only"), "{err}");
    let dumps: Vec<_> = std::fs::read_dir(repo.dir.join("freeze"))
        .into_iter()
        .flatten()
        .map(|entry| entry.unwrap().file_name())
        .filter(|name| name.to_string_lossy().ends_with(".txt"))
        .collect();
    assert!(dumps.is_empty(), "update wrote dumps: {dumps:?}");
}

#[test]
fn release_check_fails_on_a_required_api_surface_without_a_row() {
    let repo = Repo::new("required-api-release");
    repo.write("freeze/surfaces.toml", "required = [\"runtime-crates\"]\n");
    repo.commit();
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args([
            "freeze",
            "check",
            "--release",
            "--fixtures-only",
            "--api-semver",
            "runtime-crates",
        ])
        .arg("--root")
        .arg(&repo.dir)
        .env_remove("GIT_DIR")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_WORK_TREE")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(
        err.contains("surface `runtime-crates` is required but has no row"),
        "{err}"
    );
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
    let err = stderr(&output);
    assert!(
        err.contains("freeze/fixture.txt") && err.contains("`HEAD~1`"),
        "{err}"
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
    assert_unknown_ref_hint(&stderr(&output), "fixture");
}

/// The failure for the unknown baseline ref `no-such-ref` names the ref and
/// the row, and says how to fetch the ref.
fn assert_unknown_ref_hint(err: &str, surface: &str) {
    assert!(
        err.contains(&format!("surface `{surface}`"))
            && err.contains(&format!(
                "(row: name = \"{surface}\", baseline = \"no-such-ref\")"
            ))
            && err.contains("`git fetch origin no-such-ref`")
            && err.contains("fetch-depth: 0"),
        "{err}"
    );
}

const ROW: &str =
    "required = [\"fixture\"]\n\n[[surface]]\nname = \"fixture\"\nbaseline = \"base\"\n";

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

const API_ROW: &str = "required = [\"generated-crate\"]\n\n[[surface]]\nname = \"generated-crate\"\nbaseline = \"base\"\n";

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
    check_api_semver(repo, "generated-crate")
}

/// `cargo xtask freeze check` with only the semver comparison of the API
/// surface `surface`, as [`check_generated_semver`] runs it.
fn check_api_semver(repo: &Repo, surface: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["freeze", "check", "--fixtures-only", "--api-semver"])
        .arg(surface)
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
    // Both copies the check compared, the one of HEAD and the one of the
    // baseline, build outside any workspace with the denied lints.
    for copy in ["head/api", "baseline/freeze/generated/api"] {
        let manifest = repo
            .dir
            .join("target/freeze-api")
            .join(copy)
            .join("Cargo.toml");
        let text = std::fs::read_to_string(&manifest)
            .unwrap_or_else(|e| panic!("{}: {e}", manifest.display()));
        assert!(
            text.contains("[workspace]")
                && text.contains("repr_c_plain_struct_fields_reordered = \"deny\""),
            "{copy}: {text}"
        );
    }
}

#[test]
fn semver_check_flags_a_lint_that_warns_by_default() {
    // cargo-semver-checks reports `repr_c_plain_struct_fields_reordered` as a
    // warning unless the check denies it.
    let lib_rs = api_fixture("src/lib.rs").replace(
        "    pub first: u8,\n    pub second: u16,",
        "    pub second: u16,\n    pub first: u8,",
    );
    assert!(
        lib_rs.contains("pub second: u16,\n    pub first"),
        "the fixture changed"
    );
    let repo = repo_with_api_baseline("semver-lint", &api_fixture("Cargo.toml"), &lib_rs);
    let output = check_generated_semver(&repo);
    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(
        err.contains("finds a break") && err.contains("repr_c_plain_struct_fields_reordered"),
        "{err}"
    );
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
fn semver_check_names_an_unknown_baseline_ref() {
    let repo = repo_with_api_baseline(
        "semver-bad-ref",
        &api_fixture("Cargo.toml"),
        &api_fixture("src/lib.rs"),
    );
    repo.write(
        "freeze/surfaces.toml",
        "[[surface]]\nname = \"generated-crate\"\nbaseline = \"no-such-ref\"\n",
    );
    let output = check_generated_semver(&repo);
    assert!(!output.status.success());
    assert_unknown_ref_hint(&stderr(&output), "generated-crate");
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

/// The crates of the `runtime-crates` surface.
const RUNTIME_CRATES: [&str; 4] = [
    "ridl-rt",
    "ridl-rt-conformance",
    "ridl-loopback",
    "ridl-descriptor",
];

/// A repository that holds a workspace of four crates with the names of the
/// runtime crates, each with the fixture crate's source and no dependency.
/// Its tag `base` holds the crates as they are; HEAD holds the row for
/// `runtime-crates`, and the working tree holds `ridl-rt` with `lib_rs` as its
/// source.
fn repo_with_runtime_baseline(tag: &str, lib_rs: &str) -> Repo {
    let repo = Repo::new(tag);
    let members: Vec<String> = RUNTIME_CRATES
        .iter()
        .map(|name| format!("\"crates/{name}\""))
        .collect();
    repo.write(
        "Cargo.toml",
        &format!(
            "[workspace]\nresolver = \"2\"\nmembers = [{}]\n",
            members.join(", ")
        ),
    );
    for name in RUNTIME_CRATES {
        std::fs::create_dir_all(repo.dir.join("crates").join(name).join("src")).unwrap();
        repo.write(
            &format!("crates/{name}/Cargo.toml"),
            &format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"),
        );
        repo.write(
            &format!("crates/{name}/src/lib.rs"),
            &api_fixture("src/lib.rs"),
        );
    }
    let status = Command::new("cargo")
        .args(["generate-lockfile", "--offline", "--manifest-path"])
        .arg(repo.dir.join("Cargo.toml"))
        .status()
        .unwrap();
    assert!(status.success(), "cargo generate-lockfile failed");
    repo.write("freeze/surfaces.toml", "");
    repo.write(".gitignore", "/target\n");
    repo.commit();
    repo.git(&["tag", "base"]);
    repo.write(
        "freeze/surfaces.toml",
        "required = [\"runtime-crates\"]\n\n[[surface]]\nname = \"runtime-crates\"\nbaseline = \"base\"\n",
    );
    repo.commit();
    repo.write("crates/ridl-rt/src/lib.rs", lib_rs);
    repo
}

#[test]
fn semver_check_flags_a_break_in_a_runtime_crate() {
    let lib_rs = api_fixture("src/lib.rs").replace("pub fn removed() {}", "");
    let repo = repo_with_runtime_baseline("runtime-removed", &lib_rs);
    let output = check_api_semver(&repo, "runtime-crates");
    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(
        err.contains("surface `runtime-crates`")
            && err.contains("finds a break in the crates ridl-rt")
            && err.contains("the baseline `base`")
            && err.contains("function_missing")
            && err.contains("ridl_rt::removed"),
        "{err}"
    );
    // The check compares every runtime crate, not only the broken one.
    for name in RUNTIME_CRATES {
        assert!(
            err.lines()
                .any(|l| l.trim_start().starts_with(&format!("Checking {name} v"))),
            "`{name}` was not compared: {err}"
        );
    }
}

/// Makes `cargo fetch --locked` fail in `repo` without the network: the root
/// manifest names a package version that its `Cargo.lock` does not hold, so
/// the lock file would need an update.
fn make_the_locked_fetch_fail(repo: &Repo) {
    std::fs::create_dir_all(repo.dir.join("src")).unwrap();
    repo.write("src/lib.rs", "");
    repo.write(
        "Cargo.toml",
        "[package]\nname = \"stale\"\nversion = \"0.2.0\"\nedition = \"2021\"\n\n[workspace]\n",
    );
    repo.write(
        "Cargo.lock",
        "version = 3\n\n[[package]]\nname = \"stale\"\nversion = \"0.1.0\"\n",
    );
}

#[test]
fn runtime_check_fails_when_the_locked_fetch_fails() {
    let repo = repo_with_runtime_baseline("runtime-fetch", &api_fixture("src/lib.rs"));
    make_the_locked_fetch_fail(&repo);
    let output = check_api_semver(&repo, "runtime-crates");
    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(err.contains("`cargo fetch --locked` failed"), "{err}");
}

#[test]
fn generated_check_fails_when_the_locked_fetch_fails() {
    let repo = repo_with_api_baseline(
        "generated-fetch",
        &api_fixture("Cargo.toml"),
        &api_fixture("src/lib.rs"),
    );
    make_the_locked_fetch_fail(&repo);
    let output = check_generated_semver(&repo);
    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(err.contains("`cargo fetch --locked` failed"), "{err}");
}

#[test]
fn semver_check_passes_runtime_crates_with_an_added_item() {
    let lib_rs = format!(
        "{}\n/// Added.\npub fn added() {{}}\n",
        api_fixture("src/lib.rs")
    );
    let repo = repo_with_runtime_baseline("runtime-added", &lib_rs);
    let output = check_api_semver(&repo, "runtime-crates");
    assert!(output.status.success(), "{}", stderr(&output));
}
