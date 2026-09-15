//! Integration tests for `ridl lock merge`, the git merge driver for
//! `interfaces.lock` (lock design §6): one test per row of the design's §6
//! table, each run in both directions (OURS and THEIRS swapped), asserting the
//! bytes written to OURS and the exit code; the conflict form and that
//! `ridl check` then reports RIDL-410; an empty BASE; an unreadable input; an
//! input that does not parse; and one end-to-end merge through git with the
//! driver registered as the CLI reference documents it.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A unique directory under the system temp dir, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let mut path = std::env::temp_dir();
        path.push(format!(
            "ridl-lock-merge-{label}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst),
        ));
        std::fs::create_dir_all(&path).expect("create the temp dir");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
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

/// Runs `ridl` with `args` in `dir`, returning `(exit_code, stdout, stderr)`.
fn ridl_in(dir: &Path, args: &[&OsStr]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
        .current_dir(dir)
        .args(args)
        .output()
        .expect("the ridl binary must run");
    let code = output.status.code().expect("the process exits with a code");
    (
        code,
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

const HEADER: &str = "# interfaces.lock — written by ridl lock; do not edit by hand.\n";

/// A lock file text: the header, `next N`, then `entries` (one line each,
/// each ending in `\n`).
fn lock(next: u32, entries: &str) -> String {
    format!("{HEADER}next {next}\n{entries}")
}

/// Runs `ridl lock merge BASE OURS THEIRS MARKER_SIZE` over the three texts,
/// returning the exit code, the bytes OURS holds afterwards, and stderr.
fn merge_with(base: &str, ours: &str, theirs: &str, marker_size: &str) -> (i32, String, String) {
    let dir = TempDir::new("merge");
    let base_path = dir.write("base.lock", base);
    let ours_path = dir.write("ours.lock", ours);
    let theirs_path = dir.write("theirs.lock", theirs);
    let (code, stdout, stderr) = ridl_in(
        dir.path(),
        &[
            OsStr::new("lock"),
            OsStr::new("merge"),
            base_path.as_os_str(),
            ours_path.as_os_str(),
            theirs_path.as_os_str(),
            OsStr::new(marker_size),
        ],
    );
    assert_eq!(stdout, "", "the driver prints nothing to stdout");
    let written = std::fs::read_to_string(&ours_path).expect("OURS stays readable");
    (code, written, stderr)
}

/// The driver with git's default marker size, 7.
fn merge(base: &str, ours: &str, theirs: &str) -> (i32, String, String) {
    merge_with(base, ours, theirs, "7")
}

/// The base of every §6 row but the last: `A 1, B 2`.
fn base_ab() -> String {
    lock(3, "A 1\nB 2\n")
}

/// §6 row 1, both directions: both sides allocated number 3; ours keeps the
/// number, theirs is renumbered to the next free one, and `next` follows.
#[test]
fn both_add_one_number_renumbers_theirs() {
    let ours = lock(4, "A 1\nB 2\nC 3\n");
    let theirs = lock(4, "A 1\nB 2\nD 3\n");

    let (code, written, stderr) = merge(&base_ab(), &ours, &theirs);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(written, lock(5, "A 1\nB 2\nC 3\nD 4\n"));

    let (code, written, stderr) = merge(&base_ab(), &theirs, &ours);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(written, lock(5, "A 1\nB 2\nD 3\nC 4\n"));
}

/// §6 row 2, both directions: each entry changed on one side only, so both
/// changes are taken.
#[test]
fn rename_plus_add_merges_clean() {
    let ours = lock(3, "A 1\nBee 2\n");
    let theirs = lock(4, "A 1\nB 2\nD 3\n");
    let expected = lock(4, "A 1\nBee 2\nD 3\n");

    let (code, written, stderr) = merge(&base_ab(), &ours, &theirs);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(written, expected);

    let (code, written, stderr) = merge(&base_ab(), &theirs, &ours);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(written, expected);
}

/// §6 row 3, both directions: entry 2 changed on both sides in two ways, so
/// it is a conflict; entry 1 is written plain.
#[test]
fn retire_versus_rename_conflicts_on_the_entry() {
    let ours = lock(3, "A 1\nB 2 retired\n");
    let theirs = lock(3, "A 1\nBee 2\n");

    let (code, written, stderr) = merge(&base_ab(), &ours, &theirs);
    assert_eq!(code, 1, "stderr:\n{stderr}");
    assert_eq!(
        written,
        lock(
            3,
            "A 1\n<<<<<<< ours\nB 2 retired\n=======\nBee 2\n>>>>>>> theirs\n"
        )
    );

    let (code, written, stderr) = merge(&base_ab(), &theirs, &ours);
    assert_eq!(code, 1, "stderr:\n{stderr}");
    assert_eq!(
        written,
        lock(
            3,
            "A 1\n<<<<<<< ours\nBee 2\n=======\nB 2 retired\n>>>>>>> theirs\n"
        )
    );
}

/// §6 row 4, both directions: two different renames of entry 2 conflict.
#[test]
fn two_different_renames_conflict() {
    let ours = lock(3, "A 1\nBee 2\n");
    let theirs = lock(3, "A 1\nBea 2\n");

    let (code, written, stderr) = merge(&base_ab(), &ours, &theirs);
    assert_eq!(code, 1, "stderr:\n{stderr}");
    assert_eq!(
        written,
        lock(
            3,
            "A 1\n<<<<<<< ours\nBee 2\n=======\nBea 2\n>>>>>>> theirs\n"
        )
    );

    let (code, written, stderr) = merge(&base_ab(), &theirs, &ours);
    assert_eq!(code, 1, "stderr:\n{stderr}");
    assert_eq!(
        written,
        lock(
            3,
            "A 1\n<<<<<<< ours\nBea 2\n=======\nBee 2\n>>>>>>> theirs\n"
        )
    );
}

/// §6 row 5, both directions: the same retire on both sides is kept once.
#[test]
fn the_same_change_on_both_sides_is_kept_once() {
    let side = lock(3, "A 1\nB 2 retired\n");

    let (code, written, stderr) = merge(&base_ab(), &side, &side);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(written, side);
}

/// §6 row 6, both directions: both sides retired `B 2` and both allocated
/// the re-added `B` at 3 the same way, so the lock merges clean. (The two
/// provisional `B` declarations behind it are the checker's TYPL-009, not
/// the driver's.)
#[test]
fn both_retire_and_both_re_add_merges_clean() {
    let side = lock(4, "A 1\nB 2 retired\nB 3\n");

    let (code, written, stderr) = merge(&base_ab(), &side, &side);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(written, side);
}

/// §6 row 7, both directions: a retire on one side and an addition on the
/// other touch different entries.
#[test]
fn retire_versus_add_merges_clean() {
    let ours = lock(3, "A 1\nB 2 retired\n");
    let theirs = lock(4, "A 1\nB 2\nD 3\n");
    let expected = lock(4, "A 1\nB 2 retired\nD 3\n");

    let (code, written, stderr) = merge(&base_ab(), &ours, &theirs);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(written, expected);

    let (code, written, stderr) = merge(&base_ab(), &theirs, &ours);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(written, expected);
}

/// §6 row 8, both directions: each side allocated `B` on its own number.
/// Every entry merges on its number, but the result would hold one live key
/// twice, so the two entries are a conflict, placed at the lower number.
#[test]
fn a_live_name_on_two_numbers_conflicts() {
    let base = lock(2, "A 1\n");
    let ours = lock(3, "A 1\nB 2\n");
    let theirs = lock(4, "A 1\nB 3\n");

    let (code, written, stderr) = merge(&base, &ours, &theirs);
    assert_eq!(code, 1, "stderr:\n{stderr}");
    assert_eq!(
        written,
        lock(4, "A 1\n<<<<<<< ours\nB 2\n=======\nB 3\n>>>>>>> theirs\n")
    );

    let (code, written, stderr) = merge(&base, &theirs, &ours);
    assert_eq!(code, 1, "stderr:\n{stderr}");
    assert_eq!(
        written,
        lock(4, "A 1\n<<<<<<< ours\nB 3\n=======\nB 2\n>>>>>>> theirs\n")
    );
}

/// §6 "on a conflict ... markers of MARKER_SIZE around only the disagreeing
/// entries": entries that agree, before and after the conflict, are written
/// plain, the markers are as long as MARKER_SIZE says, and the written file
/// is RIDL-410 under `ridl check` until an author resolves it.
#[test]
fn a_conflict_marks_only_the_disagreeing_entries_and_is_ridl_410_until_resolved() {
    let base = lock(4, "A 1\nB 2\nC 3\n");
    let ours = lock(5, "A 1\nB 2 retired\nC 3\nD 4\n");
    let theirs = lock(4, "A 1\nBee 2\nC 3\n");

    let (code, written, stderr) = merge_with(&base, &ours, &theirs, "3");
    assert_eq!(code, 1, "stderr:\n{stderr}");
    assert_eq!(
        written,
        lock(
            5,
            "A 1\n<<< ours\nB 2 retired\n===\nBee 2\n>>> theirs\nC 3\nD 4\n"
        )
    );
    assert!(
        stderr.contains("conflict markers") && stderr.contains("RIDL-410"),
        "stderr:\n{stderr}"
    );

    let dir = TempDir::new("conflicted-package");
    dir.write(
        "ridl.toml",
        "[package]\nname = \"veh.hvac\"\nversion = \"1.0.0\"\n",
    );
    dir.write(
        "hvac.ridl",
        "package veh.hvac\ntype State: integer [0..1]\ninterface A { signal a : State @[100ms..1s] }\n",
    );
    dir.write("interfaces.lock", &written);
    let (code, _, stderr) = ridl_in(dir.path(), &[OsStr::new("check"), OsStr::new(".")]);
    assert_eq!(code, 1, "stderr:\n{stderr}");
    assert!(stderr.contains("error[RIDL-410]"), "stderr:\n{stderr}");
}

/// §6: an empty BASE — what git passes as `%O` when both sides created the
/// file — reads as `next 1` with no entries, so both sides' entries are
/// additions.
#[test]
fn an_empty_base_reads_as_next_one() {
    let ours = lock(2, "A 1\n");
    let theirs = lock(2, "B 1\n");

    let (code, written, stderr) = merge("", &ours, &theirs);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(written, lock(3, "A 1\nB 2\n"));

    let (code, written, stderr) = merge("", &theirs, &ours);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(written, lock(3, "B 1\nA 2\n"));
}

/// §6: `next` is the maximum of the three sides — a side whose `next` was
/// raised by hand keeps it — plus any renumbering, which allocates from that
/// maximum upward.
#[test]
fn next_is_the_maximum_of_the_three_sides_plus_renumbering() {
    let ours = lock(9, "A 1\nB 2\nC 3\n");
    let theirs = lock(4, "A 1\nB 2\nD 3\n");

    let (code, written, stderr) = merge(&base_ab(), &ours, &theirs);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(written, lock(10, "A 1\nB 2\nC 3\nD 9\n"));

    let (code, written, stderr) = merge(&base_ab(), &theirs, &ours);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(written, lock(10, "A 1\nB 2\nD 3\nC 9\n"));
}

/// §6: an input that cannot be read is exit 2, and OURS is left as it was.
#[test]
fn an_unreadable_input_exits_two() {
    let dir = TempDir::new("unreadable");
    let base_path = dir.write("base.lock", &base_ab());
    let ours = lock(4, "A 1\nB 2\nC 3\n");
    let ours_path = dir.write("ours.lock", &ours);
    let missing = dir.path().join("theirs.lock");

    let (code, stdout, stderr) = ridl_in(
        dir.path(),
        &[
            OsStr::new("lock"),
            OsStr::new("merge"),
            base_path.as_os_str(),
            ours_path.as_os_str(),
            missing.as_os_str(),
            OsStr::new("7"),
        ],
    );
    assert_eq!(code, 2, "stderr:\n{stderr}");
    assert_eq!(stdout, "");
    assert!(
        stderr.contains("error: cannot read") && stderr.contains("theirs.lock"),
        "stderr:\n{stderr}"
    );
    assert_eq!(
        std::fs::read_to_string(&ours_path).expect("OURS is readable"),
        ours
    );
}

/// A readable input that does not parse is exit 2 too, naming the file and
/// the line, and OURS is left as it was: the driver merges tables, and a side
/// that is not a table is an input it cannot answer over.
#[test]
fn an_input_that_does_not_parse_exits_two_and_leaves_ours_unchanged() {
    let ours = lock(4, "A 1\nB 2\nC 3\n");
    let theirs = format!("{HEADER}next 2\nA 1\nB 2\n");

    let (code, written, stderr) = merge(&base_ab(), &ours, &theirs);
    assert_eq!(code, 2, "stderr:\n{stderr}");
    assert_eq!(written, ours);
    assert!(
        stderr
            .contains("theirs.lock:4: `next` is 2, which is not greater than the number 2 of `B`"),
        "stderr:\n{stderr}"
    );
}

/// A package directory literally named `merge` is a path, not the
/// subcommand, when it is spelled `./merge`.
#[test]
fn a_package_directory_named_merge_is_a_path_when_spelled_with_a_dot() {
    let dir = TempDir::new("named-merge");
    dir.write(
        "merge/ridl.toml",
        "[package]\nname = \"veh.hvac\"\nversion = \"1.0.0\"\n",
    );
    dir.write(
        "merge/hvac.ridl",
        "package veh.hvac\ntype State: integer [0..1]\ninterface Cabin { signal c : State @[100ms..1s] }\n",
    );

    let (code, stdout, stderr) = ridl_in(dir.path(), &[OsStr::new("lock"), OsStr::new("./merge")]);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(stdout, "allocated Cabin 1\n");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("merge/interfaces.lock")).expect("written"),
        lock(2, "Cabin 1\n")
    );
}

/// Runs `git` with `args` in `dir`; panics with the output when it fails.
///
/// A git hook runs with `GIT_DIR`, `GIT_WORK_TREE` and `GIT_INDEX_FILE` in its
/// environment, and each of those outranks the child process's working
/// directory. This repository's pre-push hook runs the test suite, so the
/// inherited git environment is removed here: without that, this helper would
/// drive the repository being pushed from instead of the temporary one.
fn git(dir: &Path, args: &[&str]) -> String {
    let mut command = Command::new("git");
    command
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_EDITOR", "true");
    for inherited in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_COMMON_DIR",
        "GIT_NAMESPACE",
        "GIT_PREFIX",
    ] {
        command.env_remove(inherited);
    }
    let output = command.args(args).output().expect("git runs");
    assert!(
        output.status.success(),
        "git {} failed:\nstdout:\n{}\nstderr:\n{}",
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// The end-to-end row: a repository registers the driver exactly as the CLI
/// reference documents it — `.gitattributes` with `interfaces.lock
/// merge=ridl-lock`, and `git config merge.ridl-lock.driver` — and merges the
/// two branches of §6 row 1; git runs the driver and the merged file is the
/// row's result. Skipped, with the reason printed, when `git` is not on PATH.
#[test]
fn the_registered_driver_merges_two_branches_through_git() {
    if Command::new("git").arg("--version").output().is_err() {
        eprintln!("skipped: `git` is not on PATH");
        return;
    }
    let dir = TempDir::new("git");
    let repo = dir.path();
    git(repo, &["init", "--quiet", "--initial-branch=main"]);
    // Every later command must reach this repository and no other. The check
    // runs before the first commit, so a git environment that outranked the
    // working directory would stop the test here rather than write into the
    // repository the test suite runs in.
    let reached = git(repo, &["rev-parse", "--absolute-git-dir"]);
    assert_eq!(
        std::fs::canonicalize(reached.trim()).expect("the git directory git reached exists"),
        std::fs::canonicalize(repo.join(".git")).expect("the temporary git directory exists"),
        "git must reach the temporary repository"
    );
    git(repo, &["config", "user.name", "ridl tests"]);
    git(
        repo,
        &["config", "user.email", "ridl-tests@example.invalid"],
    );
    git(repo, &["config", "commit.gpgsign", "false"]);
    git(
        repo,
        &[
            "config",
            "merge.ridl-lock.driver",
            &format!("\"{}\" lock merge %O %A %B %L", env!("CARGO_BIN_EXE_ridl")),
        ],
    );
    dir.write(".gitattributes", "interfaces.lock merge=ridl-lock\n");
    dir.write("interfaces.lock", &base_ab());
    git(repo, &["add", "."]);
    git(repo, &["commit", "--quiet", "-m", "base"]);

    git(repo, &["checkout", "--quiet", "-b", "ours"]);
    dir.write("interfaces.lock", &lock(4, "A 1\nB 2\nC 3\n"));
    git(repo, &["commit", "--quiet", "-am", "ours adds C"]);

    git(repo, &["checkout", "--quiet", "-b", "theirs", "main"]);
    dir.write("interfaces.lock", &lock(4, "A 1\nB 2\nD 3\n"));
    git(repo, &["commit", "--quiet", "-am", "theirs adds D"]);

    git(repo, &["checkout", "--quiet", "ours"]);
    git(repo, &["merge", "--quiet", "--no-edit", "theirs"]);

    assert_eq!(
        std::fs::read_to_string(repo.join("interfaces.lock")).expect("the merged file"),
        lock(5, "A 1\nB 2\nC 3\nD 4\n")
    );
    assert_eq!(
        git(repo, &["status", "--porcelain"]),
        "",
        "the merge committed cleanly"
    );
}
