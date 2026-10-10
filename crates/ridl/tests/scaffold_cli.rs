//! Integration tests for `ridl init` and `ridl new`, each against the built
//! binary: a scaffold must check and format clean, and every refusal must
//! exit 2 and write nothing.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A unique directory under the system temp dir, removed on drop. A `.git`
/// directory marks it as a repository root, so the search for an enclosing
/// unit never leaves it.
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
        std::fs::create_dir_all(path.join(".git")).expect("create the temp dir");
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
            if path.is_dir() {
                out.push((path.clone(), None));
                stack.push(path);
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
fn new_on_an_existing_path_exits_2_and_changes_nothing() {
    let tmp = TempDir::new("new-exists");
    tmp.write("demo/keep.txt", "keep");
    let before = snapshot(tmp.path());
    let (code, stdout, stderr) = ridl(tmp.path(), &["new", "demo"]);
    assert_eq!(code, 2, "stdout:\n{stdout}");
    assert!(stderr.starts_with("error: "), "stderr:\n{stderr}");
    assert_eq!(snapshot(tmp.path()), before);
}

#[test]
fn init_with_an_existing_manifest_exits_2_and_writes_nothing() {
    let tmp = TempDir::new("init-exists");
    tmp.write(
        "demo/ridl.toml",
        "[package]\nname = \"other\"\nversion = \"1.0.0\"\n",
    );
    let before = snapshot(tmp.path());
    let (code, stdout, stderr) = ridl(&tmp.path().join("demo"), &["init"]);
    assert_eq!(code, 2, "stdout:\n{stdout}");
    assert!(stderr.starts_with("error: "), "stderr:\n{stderr}");
    assert_eq!(snapshot(tmp.path()), before);
}

#[test]
fn init_with_an_existing_source_file_exits_2_and_writes_nothing() {
    let tmp = TempDir::new("init-source-exists");
    tmp.write("demo/demo.ridl", "package demo\n");
    let before = snapshot(tmp.path());
    let (code, _, stderr) = ridl(&tmp.path().join("demo"), &["init"]);
    assert_eq!(code, 2);
    assert!(stderr.starts_with("error: "), "stderr:\n{stderr}");
    assert_eq!(snapshot(tmp.path()), before);
}

#[test]
fn workspace_with_an_existing_member_file_exits_2_and_writes_nothing() {
    let tmp = TempDir::new("workspace-exists");
    tmp.write(
        "proj/proj/ridl.toml",
        "[package]\nname = \"proj\"\nversion = \"1.0.0\"\n",
    );
    let before = snapshot(tmp.path());
    let (code, _, stderr) = ridl(&tmp.path().join("proj"), &["init", "--workspace"]);
    assert_eq!(code, 2);
    assert!(stderr.starts_with("error: "), "stderr:\n{stderr}");
    assert_eq!(snapshot(tmp.path()), before);
}

#[test]
fn init_inside_an_existing_unit_exits_2_and_writes_nothing() {
    let tmp = TempDir::new("inside-unit");
    tmp.write(
        "outer/ridl.toml",
        "[package]\nname = \"outer\"\nversion = \"1.0.0\"\n",
    );
    std::fs::create_dir(tmp.path().join("outer/inner")).expect("mkdir");
    let before = snapshot(tmp.path());
    let (code, _, stderr) = ridl(&tmp.path().join("outer/inner"), &["init"]);
    assert_eq!(code, 2);
    assert!(stderr.starts_with("error: "), "stderr:\n{stderr}");
    assert_eq!(snapshot(tmp.path()), before);
}

#[test]
fn new_inside_an_existing_workspace_exits_2_and_writes_nothing() {
    let tmp = TempDir::new("inside-workspace");
    tmp.write("ws/ridl.toml", "[workspace]\nmembers = []\n");
    let before = snapshot(tmp.path());
    let (code, _, stderr) = ridl(&tmp.path().join("ws"), &["new", "demo"]);
    assert_eq!(code, 2);
    assert!(stderr.starts_with("error: "), "stderr:\n{stderr}");
    assert_eq!(snapshot(tmp.path()), before);
}

#[test]
fn an_illegal_basename_exits_2_and_asks_for_name() {
    let tmp = TempDir::new("basename");
    std::fs::create_dir(tmp.path().join("My-Dir")).expect("mkdir");
    let before = snapshot(tmp.path());
    let (code, _, stderr) = ridl(&tmp.path().join("My-Dir"), &["init"]);
    assert_eq!(code, 2);
    assert!(stderr.starts_with("error: "), "stderr:\n{stderr}");
    assert!(stderr.contains("--name"), "stderr:\n{stderr}");
    assert_eq!(snapshot(tmp.path()), before);

    let (code, _, stderr) = ridl(tmp.path(), &["new", "My-Dir2"]);
    assert_eq!(code, 2);
    assert!(stderr.contains("--name"), "stderr:\n{stderr}");
    assert!(!tmp.path().join("My-Dir2").exists());
}

#[test]
fn an_illegal_name_flag_exits_2_and_writes_nothing() {
    let tmp = TempDir::new("bad-name");
    std::fs::create_dir(tmp.path().join("demo")).expect("mkdir");
    let before = snapshot(tmp.path());
    let (code, _, stderr) = ridl(&tmp.path().join("demo"), &["init", "--name", "Bad-Name"]);
    assert_eq!(code, 2);
    assert!(stderr.starts_with("error: "), "stderr:\n{stderr}");
    assert_eq!(snapshot(tmp.path()), before);

    let (code, _, _) = ridl(tmp.path(), &["new", "--name", "Bad-Name", "other"]);
    assert_eq!(code, 2);
    assert!(!tmp.path().join("other").exists());
}
