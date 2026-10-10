//! A relative entry whose package root is the current directory or above it
//! loads the package (driftsys/ridl#795): `ridl check`, and with a bare file
//! name `ridl lock`, `ridl baseline`, `ridl build` and `ridl diff`. The tests
//! pin the package load, not only the exit code: `a.ridl` uses a type that
//! only the sibling file `types.typl` declares, a broken sibling file is
//! reported for an entry elsewhere in the package, and the lockfile and the
//! published baseline land at the package root, also when the command runs
//! from a subdirectory of the package (driftsys/ridl#830). `ridl build`
//! writes where `--out-dir` points, relative to the current directory.

use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

/// A package directory holding `files`, each a path relative to the package
/// root and its text. The directory is removed when the value is dropped,
/// even when the test panics.
fn package(files: &[(&str, &str)]) -> TempDir {
    let dir = tempfile::Builder::new()
        .prefix("ridl-bare-")
        .tempdir()
        .expect("create the fixture");
    for (path, text) in files {
        let path = dir.path().join(path);
        std::fs::create_dir_all(path.parent().expect("a file has a parent"))
            .expect("create the fixture directory");
        std::fs::write(&path, text).expect("write the fixture file");
    }
    dir
}

/// A package whose `a.ridl` uses a type that only `types.typl` declares, with
/// a nested source `src/b.ridl`.
fn struct_package() -> TempDir {
    package(&[
        (
            "ridl.toml",
            "[package]\nname = \"p\"\nversion = \"1.0.0\"\n",
        ),
        ("types.typl", "package p\n\ntype Speed: km/h [0..250]\n"),
        ("a.ridl", "package p\n\nstruct Gauge {\n  speed: Speed\n}\n"),
        ("src/b.ridl", "package p.src\n"),
    ])
}

/// A package whose `a.ridl` declares an interface over a type that only the
/// sibling file `types.typl` declares, so a load of `a.ridl` alone fails to
/// resolve it, with a nested source `src/b.ridl`.
fn interface_package() -> TempDir {
    package(&[
        (
            "ridl.toml",
            "[package]\nname = \"veh.cluster\"\nversion = \"1.0.0\"\n",
        ),
        (
            "types.typl",
            "package veh.cluster\n\ntype Flag: integer [0..1]\n",
        ),
        (
            "a.ridl",
            "package veh.cluster\n\ninterface Status {\n  event opened: Flag @[100ms..1s]\n}\n",
        ),
        ("src/b.ridl", "package veh.cluster.src\n"),
    ])
}

/// Runs `ridl` with `args` from the directory `cwd` of `dir`, returning the
/// exit code and the combined output.
fn ridl_in(dir: &TempDir, cwd: &str, args: &[&str]) -> (Option<i32>, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
        .current_dir(dir.path().join(cwd))
        .args(args)
        .output()
        .expect("run ridl");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stderr).into_owned()
            + &String::from_utf8_lossy(&output.stdout),
    )
}

/// Whether `path` below the fixture root is a file.
fn is_file(dir: &TempDir, path: &str) -> bool {
    Path::new(dir.path()).join(path).is_file()
}

#[test]
fn a_bare_file_name_checks_the_package_it_sits_in() {
    let dir = struct_package();
    let (code, text) = ridl_in(&dir, ".", &["check", "a.ridl"]);
    assert_eq!(code, Some(0), "{text}");
}

#[test]
fn a_relative_path_below_the_package_root_checks_the_package() {
    let dir = struct_package();
    std::fs::write(
        dir.path().join("bad.ridl"),
        "package p\n\nstruct Bad {\n  x: Nope\n}\n",
    )
    .expect("write the broken source");
    // The broken sibling is in the package, so a package load reports it
    // for an entry in any directory of the package.
    let results = [
        ridl_in(&dir, ".", &["check", "src/b.ridl"]),
        ridl_in(&dir, ".", &["check", "src"]),
        ridl_in(&dir, "src", &["check", "b.ridl"]),
    ];
    for (code, text) in &results {
        assert_eq!(*code, Some(1), "{text}");
        assert!(text.contains("TYPL-011"), "{text}");
    }
    // A root above the current directory keeps its relative form.
    assert!(results[2].1.contains("../bad.ridl"), "{}", results[2].1);
}

/// `ridl lock` with a bare file name locks the package it sits in and writes
/// the lockfile at the package root.
#[test]
fn lock_with_a_bare_file_name_locks_the_package() {
    let dir = interface_package();
    let (code, text) = ridl_in(&dir, ".", &["lock", "a.ridl"]);
    assert_eq!(code, Some(0), "{text}");
    assert!(
        is_file(&dir, "interfaces.lock"),
        "the lockfile is at the package root: {text}"
    );
}

/// `ridl lock` with a bare file name run from a subdirectory of the package
/// writes the lockfile at the package root, not in the current directory.
#[test]
fn lock_with_a_bare_file_name_below_the_root_locks_the_package() {
    let dir = interface_package();
    let (code, text) = ridl_in(&dir, "src", &["lock", "b.ridl"]);
    assert_eq!(code, Some(0), "{text}");
    assert!(
        is_file(&dir, "interfaces.lock"),
        "the lockfile is at the package root: {text}"
    );
    assert!(
        !is_file(&dir, "src/interfaces.lock"),
        "no lockfile in the current directory: {text}"
    );
    let lockfile = std::fs::read_to_string(dir.path().join("interfaces.lock")).unwrap_or_default();
    assert!(
        lockfile.contains("Status 1"),
        "the lockfile numbers the interface of the whole package:\n{lockfile}"
    );
}

/// `ridl baseline` with a bare file name publishes the package it sits in
/// under the package root's `.ridl/baseline/`.
#[test]
fn baseline_with_a_bare_file_name_publishes_the_package() {
    let dir = interface_package();
    let (lock_code, lock_text) = ridl_in(&dir, ".", &["lock", "a.ridl"]);
    assert_eq!(lock_code, Some(0), "{lock_text}");
    let (code, text) = ridl_in(&dir, ".", &["baseline", "a.ridl"]);
    assert_eq!(code, Some(0), "{text}");
    assert!(
        is_file(&dir, ".ridl/baseline/veh.cluster.ir.json"),
        "the snapshot is at the package root: {text}"
    );
}

/// `ridl baseline` with a bare file name run from a subdirectory of the
/// package publishes under the package root's `.ridl/baseline/` (the
/// default baseline directory), not under the current directory.
#[test]
fn baseline_with_a_bare_file_name_below_the_root_publishes_at_the_root() {
    let dir = interface_package();
    let (lock_code, lock_text) = ridl_in(&dir, "src", &["lock", "b.ridl"]);
    assert_eq!(lock_code, Some(0), "{lock_text}");
    let (code, text) = ridl_in(&dir, "src", &["baseline", "b.ridl"]);
    assert_eq!(code, Some(0), "{text}");
    assert!(
        is_file(&dir, ".ridl/baseline/veh.cluster.ir.json"),
        "the snapshot is at the package root: {text}"
    );
    assert!(
        !dir.path().join("src/.ridl").exists(),
        "nothing is published under the current directory: {text}"
    );
}

/// `ridl build` with a bare file name builds the package it sits in: the
/// emitted crate takes its name from the package manifest, which the build
/// finds through the root of the entry.
#[test]
fn build_with_a_bare_file_name_builds_the_package() {
    let dir = interface_package();
    let (code, text) = ridl_in(&dir, ".", &["build", "a.ridl", "--out-dir", "out"]);
    assert_eq!(code, Some(0), "{text}");
    let manifest = std::fs::read_to_string(dir.path().join("out/Cargo.toml")).unwrap_or_default();
    assert!(
        manifest.contains("name = \"veh_cluster\""),
        "the crate is named after the package:\n{manifest}"
    );
}

/// `ridl build` with a bare file name run from a subdirectory of the package
/// finds the package root above the current directory and emits a crate, not
/// the single file of a manifest-less entry.
#[test]
fn build_with_a_bare_file_name_below_the_root_builds_a_crate() {
    let dir = interface_package();
    let (code, text) = ridl_in(&dir, "src", &["build", "b.ridl", "--out-dir", "out"]);
    assert_eq!(code, Some(0), "{text}");
    let manifest =
        std::fs::read_to_string(dir.path().join("src/out/Cargo.toml")).unwrap_or_default();
    assert!(
        manifest.contains("name = \"veh_cluster\""),
        "a crate named after the package is written:\n{manifest}\n{text}"
    );
    assert!(
        is_file(&dir, "src/out/veh.cluster.rs"),
        "the crate holds the module of the whole package: {text}"
    );
}

/// `ridl diff` with a bare file name on each side compares the package the
/// file sits in.
#[test]
fn diff_with_a_bare_file_name_compares_the_package() {
    let dir = interface_package();
    let (code, text) = ridl_in(&dir, ".", &["diff", "a.ridl", "a.ridl"]);
    assert_eq!(code, Some(0), "{text}");
    assert!(text.contains("identical"), "{text}");
}
