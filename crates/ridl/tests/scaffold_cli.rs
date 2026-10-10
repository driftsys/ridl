//! Integration tests for `ridl init` and `ridl new`, each against the built
//! binary: a scaffold must check and format clean, and every refusal must
//! exit 2 and write nothing.

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
            "ridl-scaffold-{label}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst),
        ));
        std::fs::create_dir_all(&path).expect("create the temp dir");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn write(&self, relative: &str, text: &str) {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().expect("a relative path has a parent"))
            .expect("create parent directories");
        std::fs::write(path, text).expect("write the fixture file");
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Runs `ridl` in `dir` with `args`, returning `(exit_code, stdout, stderr)`.
fn ridl(dir: &Path, args: &[&str]) -> (i32, String, String) {
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

/// Every file under `dir` with its bytes, sorted, directories included.
fn snapshot(dir: &Path) -> Vec<(PathBuf, Option<Vec<u8>>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        for entry in std::fs::read_dir(&next).expect("read a directory") {
            let path = entry.expect("read an entry").path();
            let kind = path.symlink_metadata().expect("stat an entry").file_type();
            if kind.is_dir() {
                out.push((path.clone(), None));
                stack.push(path);
            } else if kind.is_symlink() {
                let target = std::fs::read_link(&path).expect("read a link");
                out.push((
                    path,
                    Some(target.to_string_lossy().into_owned().into_bytes()),
                ));
            } else {
                let bytes = std::fs::read(&path).expect("read a file");
                out.push((path, Some(bytes)));
            }
        }
    }
    out.sort();
    out
}

fn assert_clean(dir: &Path, args: &[&str]) {
    let (code, stdout, stderr) = ridl(dir, args);
    assert_eq!(code, 0, "{args:?} stdout:\n{stdout}\nstderr:\n{stderr}");
    assert_eq!(stderr, "", "{args:?} must draw nothing on stderr");
}

/// Asserts that `ridl args` run in `dir` refuses: exit 2, nothing on stdout,
/// an `error:` line on stderr that holds `reason`, and `watched` unchanged.
fn assert_refused(dir: &Path, args: &[&str], reason: &str, watched: &Path) {
    let before = snapshot(watched);
    let (code, stdout, stderr) = ridl(dir, args);
    assert_eq!(code, 2, "{args:?} stderr:\n{stderr}");
    assert_eq!(stdout, "", "{args:?} must print nothing on stdout");
    assert!(stderr.starts_with("error: "), "stderr:\n{stderr}");
    assert!(stderr.contains(reason), "expected `{reason}` in:\n{stderr}");
    assert_eq!(snapshot(watched), before, "{args:?} must write nothing");
}

const PACKAGE_MANIFEST: &str = "[package]\nname = \"other\"\nversion = \"1.0.0\"\n";

#[test]
fn new_creates_a_package_that_checks_clean() {
    let tmp = TempDir::new("new");
    let (code, stdout, stderr) = ridl(tmp.path(), &["new", "demo"]);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(stdout, "Created package demo\n");
    let manifest = std::fs::read_to_string(tmp.path().join("demo/ridl.toml")).expect("manifest");
    assert_eq!(
        manifest,
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n"
    );
    let source = std::fs::read_to_string(tmp.path().join("demo/demo.ridl")).expect("source");
    assert!(source.starts_with("package demo\n"), "source:\n{source}");
    assert_clean(&tmp.path().join("demo"), &["check"]);
}

#[test]
fn new_creates_missing_parent_directories() {
    let tmp = TempDir::new("new-parents");
    let (code, _, stderr) = ridl(tmp.path(), &["new", "a/b/demo"]);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert!(tmp.path().join("a/b/demo/ridl.toml").is_file());
}

#[test]
fn new_resolves_a_parent_component_before_creating() {
    let tmp = TempDir::new("new-dotdot");
    assert_clean(tmp.path(), &["new", "a/../fresh"]);
    assert!(tmp.path().join("fresh/ridl.toml").is_file());
    assert!(!tmp.path().join("a").exists(), "`a` must not be created");
}

#[test]
fn init_in_an_empty_directory_checks_clean() {
    let tmp = TempDir::new("init");
    std::fs::create_dir(tmp.path().join("demo")).expect("mkdir");
    let dir = tmp.path().join("demo");
    let (code, stdout, stderr) = ridl(&dir, &["init"]);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(stdout, "Created package demo\n");
    assert_clean(&dir, &["check"]);
}

#[test]
fn init_takes_a_path() {
    let tmp = TempDir::new("init-path");
    std::fs::create_dir(tmp.path().join("demo")).expect("mkdir");
    assert_clean(tmp.path(), &["init", "demo"]);
    assert!(tmp.path().join("demo/demo.ridl").is_file());
}

#[test]
fn the_scaffold_formats_clean() {
    let tmp = TempDir::new("fmt");
    assert_clean(tmp.path(), &["new", "demo"]);
    assert_clean(&tmp.path().join("demo"), &["fmt", "--check"]);
}

#[test]
fn workspace_checks_clean_from_the_root_and_from_the_member() {
    let tmp = TempDir::new("workspace");
    let (code, stdout, stderr) = ridl(tmp.path(), &["new", "--workspace", "proj"]);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(stdout, "Created workspace proj\n");
    let root = tmp.path().join("proj");
    let manifest = std::fs::read_to_string(root.join("ridl.toml")).expect("root manifest");
    assert_eq!(manifest, "[workspace]\nmembers = [\"proj\"]\n");
    assert!(root.join("proj/ridl.toml").is_file());
    assert!(root.join("proj/proj.ridl").is_file());
    assert_clean(&root, &["check"]);
    assert_clean(&root.join("proj"), &["check"]);
    assert_clean(&root, &["fmt", "--check"]);
}

#[test]
fn workspace_name_flag_names_the_member() {
    let tmp = TempDir::new("workspace-name");
    assert_clean(
        tmp.path(),
        &["new", "--workspace", "--name", "hvac", "proj"],
    );
    let root = tmp.path().join("proj");
    let manifest = std::fs::read_to_string(root.join("ridl.toml")).expect("root manifest");
    assert_eq!(manifest, "[workspace]\nmembers = [\"hvac\"]\n");
    assert!(root.join("hvac/hvac.ridl").is_file());
    assert_clean(&root, &["check"]);
}

#[test]
fn init_workspace_scaffolds_into_an_existing_directory() {
    let tmp = TempDir::new("init-workspace");
    std::fs::create_dir(tmp.path().join("proj")).expect("mkdir");
    let root = tmp.path().join("proj");
    let (code, stdout, stderr) = ridl(&root, &["init", "--workspace"]);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(stdout, "Created workspace proj\n");
    assert_clean(&root, &["check"]);
}

#[test]
fn name_flag_names_the_package() {
    let tmp = TempDir::new("name");
    std::fs::create_dir(tmp.path().join("Not Legal")).expect("mkdir");
    let dir = tmp.path().join("Not Legal");
    let (code, stdout, stderr) = ridl(&dir, &["init", "--name", "demo"]);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(stdout, "Created package demo\n");
    assert_clean(&dir, &["check"]);
}

#[test]
fn a_dotted_basename_is_a_legal_name() {
    let tmp = TempDir::new("dotted-new");
    let (code, stdout, stderr) = ridl(tmp.path(), &["new", "veh.common"]);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(stdout, "Created package veh.common\n");
    assert_clean(&tmp.path().join("veh.common"), &["check"]);
}

#[test]
fn a_dotted_name_flag_is_a_legal_name() {
    let tmp = TempDir::new("dotted-name");
    std::fs::create_dir(tmp.path().join("body")).expect("mkdir");
    let dir = tmp.path().join("body");
    let (code, stdout, stderr) = ridl(&dir, &["init", "--name", "veh.body"]);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(stdout, "Created package veh.body\n");
    assert_clean(&dir, &["check"]);
}

#[cfg(unix)]
#[test]
fn init_names_the_package_after_the_path_as_written() {
    let tmp = TempDir::new("link-name");
    std::fs::create_dir_all(tmp.path().join("builds/run42")).expect("mkdir");
    std::os::unix::fs::symlink("builds/run42", tmp.path().join("demo")).expect("symlink");
    let (code, stdout, stderr) = ridl(tmp.path(), &["init", "demo"]);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(stdout, "Created package demo\n");
    assert!(tmp.path().join("builds/run42/demo.ridl").is_file());
}

#[test]
fn new_on_an_existing_path_exits_2_and_changes_nothing() {
    let tmp = TempDir::new("new-exists");
    tmp.write("demo/keep.txt", "keep");
    assert_refused(tmp.path(), &["new", "demo"], "already exists", tmp.path());
}

#[test]
fn new_refuses_an_existing_directory_reached_through_a_parent_component() {
    let tmp = TempDir::new("new-dotdot-exists");
    tmp.write("demo/keep.txt", "keep");
    assert_refused(
        tmp.path(),
        &["new", "nope/../demo"],
        "already exists",
        tmp.path(),
    );
}

#[cfg(unix)]
#[test]
fn new_refuses_a_dangling_symlink() {
    let tmp = TempDir::new("new-dangling");
    std::os::unix::fs::symlink("missing", tmp.path().join("dang")).expect("symlink");
    assert_refused(tmp.path(), &["new", "dang"], "already exists", tmp.path());
}

#[test]
fn init_refuses_a_missing_directory() {
    let tmp = TempDir::new("init-missing");
    assert_refused(
        tmp.path(),
        &["init", "missing"],
        "is not a directory",
        tmp.path(),
    );
}

#[test]
fn init_refuses_a_file() {
    let tmp = TempDir::new("init-file");
    tmp.write("some-file", "text");
    assert_refused(
        tmp.path(),
        &["init", "some-file"],
        "is not a directory",
        tmp.path(),
    );
}

#[test]
fn init_with_an_existing_manifest_exits_2_and_writes_nothing() {
    let tmp = TempDir::new("init-exists");
    tmp.write("demo/ridl.toml", PACKAGE_MANIFEST);
    assert_refused(
        &tmp.path().join("demo"),
        &["init"],
        "ridl.toml` already exists",
        tmp.path(),
    );
}

#[test]
fn init_with_an_existing_source_file_exits_2_and_writes_nothing() {
    let tmp = TempDir::new("init-source-exists");
    tmp.write("demo/demo.ridl", "package demo\n");
    assert_refused(
        &tmp.path().join("demo"),
        &["init"],
        "demo.ridl` already exists",
        tmp.path(),
    );
}

#[test]
fn workspace_with_an_existing_member_file_exits_2_and_writes_nothing() {
    let tmp = TempDir::new("workspace-exists");
    tmp.write("proj/proj/ridl.toml", PACKAGE_MANIFEST);
    assert_refused(
        &tmp.path().join("proj"),
        &["init", "--workspace"],
        "already exists",
        tmp.path(),
    );
}

#[test]
fn workspace_with_an_existing_root_manifest_creates_no_member_directory() {
    let tmp = TempDir::new("workspace-root-exists");
    tmp.write("proj/ridl.toml", "[workspace]\nmembers = []\n");
    assert_refused(
        &tmp.path().join("proj"),
        &["init", "--workspace"],
        "ridl.toml` already exists",
        tmp.path(),
    );
    assert!(!tmp.path().join("proj/proj").exists());
}

#[test]
fn workspace_with_a_file_at_the_member_path_exits_2() {
    let tmp = TempDir::new("workspace-member-file");
    tmp.write("w/w", "a file");
    assert_refused(
        &tmp.path().join("w"),
        &["init", "--workspace"],
        "is not a directory",
        tmp.path(),
    );
}

#[test]
fn init_inside_an_existing_unit_exits_2_and_writes_nothing() {
    let tmp = TempDir::new("inside-unit");
    tmp.write("outer/ridl.toml", PACKAGE_MANIFEST);
    std::fs::create_dir(tmp.path().join("outer/inner")).expect("mkdir");
    assert_refused(
        &tmp.path().join("outer/inner"),
        &["init"],
        "is inside the unit of",
        tmp.path(),
    );
}

#[test]
fn init_several_levels_below_a_unit_exits_2() {
    let tmp = TempDir::new("deep-unit");
    tmp.write("outer/ridl.toml", PACKAGE_MANIFEST);
    std::fs::create_dir_all(tmp.path().join("outer/a/b")).expect("mkdir");
    assert_refused(
        &tmp.path().join("outer/a/b"),
        &["init"],
        "is inside the unit of",
        tmp.path(),
    );
}

#[test]
fn new_inside_an_existing_workspace_exits_2_and_writes_nothing() {
    let tmp = TempDir::new("inside-workspace");
    tmp.write("ws/ridl.toml", "[workspace]\nmembers = [\"demo\"]\n");
    assert_refused(
        &tmp.path().join("ws"),
        &["new", "demo"],
        "is inside the unit of",
        tmp.path(),
    );
    assert!(!tmp.path().join("ws/demo").exists());
}

#[test]
fn the_search_for_an_enclosing_unit_does_not_stop_at_a_git_directory() {
    let tmp = TempDir::new("git-boundary");
    tmp.write("ridl.toml", PACKAGE_MANIFEST);
    std::fs::create_dir_all(tmp.path().join("x/sub")).expect("mkdir");
    std::fs::create_dir(tmp.path().join("x/.git")).expect("mkdir");
    assert_refused(
        &tmp.path().join("x/sub"),
        &["init"],
        "is inside the unit of",
        tmp.path(),
    );
}

#[test]
fn init_beside_a_manifest_in_the_parent_of_a_git_directory_exits_2() {
    let tmp = TempDir::new("git-beside");
    tmp.write("ridl.toml", PACKAGE_MANIFEST);
    std::fs::create_dir_all(tmp.path().join(".git")).expect("mkdir");
    std::fs::create_dir(tmp.path().join("x")).expect("mkdir");
    assert_refused(
        &tmp.path().join("x"),
        &["init"],
        "is inside the unit of",
        tmp.path(),
    );
}

#[cfg(unix)]
#[test]
fn the_search_for_an_enclosing_unit_follows_a_symlink() {
    let tmp = TempDir::new("link-unit");
    tmp.write("pkg/ridl.toml", PACKAGE_MANIFEST);
    std::fs::create_dir(tmp.path().join("pkg/sub")).expect("mkdir");
    std::os::unix::fs::symlink("pkg/sub", tmp.path().join("link")).expect("symlink");
    assert_refused(
        tmp.path(),
        &["new", "link/x"],
        "is inside the unit of",
        tmp.path(),
    );
}

#[test]
fn init_over_a_directory_that_holds_a_manifest_below_it_exits_2() {
    let tmp = TempDir::new("manifest-below");
    tmp.write("outer/inner/ridl.toml", PACKAGE_MANIFEST);
    assert_refused(
        &tmp.path().join("outer"),
        &["init", "--name", "outer"],
        "holds a manifest below it",
        tmp.path(),
    );
}

#[test]
fn an_illegal_basename_exits_2_and_asks_for_name() {
    for name in ["My-Dir", "MyDir", "1abc", "a..b", "a."] {
        let tmp = TempDir::new("basename");
        std::fs::create_dir(tmp.path().join(name)).expect("mkdir");
        let (code, stdout, stderr) = ridl(&tmp.path().join(name), &["init"]);
        assert_eq!(code, 2, "{name}: {stderr}");
        assert_eq!(stdout, "", "{name}");
        assert!(
            stderr.contains("is not a legal package name"),
            "{name}: {stderr}"
        );
        assert!(stderr.contains("--name"), "{name}: {stderr}");
        assert!(!tmp.path().join(name).join("ridl.toml").exists(), "{name}");

        let (code, _, stderr) = ridl(tmp.path(), &["new", &format!("{name}2")]);
        assert_eq!(code, 2, "{name}2: {stderr}");
        assert!(stderr.contains("--name"), "{name}2: {stderr}");
        assert!(!tmp.path().join(format!("{name}2")).exists());
    }
}

#[test]
fn an_illegal_name_flag_exits_2_and_writes_nothing() {
    for name in ["Bad-Name", "MyDir", "1abc", "a..b", "a.", ""] {
        let tmp = TempDir::new("bad-name");
        std::fs::create_dir(tmp.path().join("demo")).expect("mkdir");
        assert_refused(
            &tmp.path().join("demo"),
            &["init", "--name", name],
            "is not a legal package name",
            tmp.path(),
        );
        assert_refused(
            tmp.path(),
            &["new", "--name", name, "other"],
            "is not a legal package name",
            tmp.path(),
        );
    }
}

#[test]
fn a_reserved_word_basename_exits_2_and_asks_for_name() {
    for name in ["type", "system", "interface", "veh.type"] {
        let tmp = TempDir::new("keyword-basename");
        std::fs::create_dir(tmp.path().join(name)).expect("mkdir");
        let dir = tmp.path().join(name);
        let (code, stdout, stderr) = ridl(&dir, &["init"]);
        assert_eq!(code, 2, "{name}: {stderr}");
        assert_eq!(stdout, "", "{name}");
        assert!(stderr.contains("reserved word"), "{name}: {stderr}");
        assert!(stderr.contains("--name"), "{name}: {stderr}");
        assert!(!dir.join("ridl.toml").exists(), "{name}");
    }
}

#[test]
fn a_reserved_word_name_flag_exits_2_and_writes_nothing() {
    for name in ["type", "package", "veh.signal"] {
        let tmp = TempDir::new("keyword-name");
        std::fs::create_dir(tmp.path().join("demo")).expect("mkdir");
        assert_refused(
            &tmp.path().join("demo"),
            &["init", "--name", name],
            "reserved word",
            tmp.path(),
        );
    }
}
