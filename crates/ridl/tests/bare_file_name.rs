//! A relative entry whose package root is the current directory or above it
//! loads the package (driftsys/ridl#795): `ridl check`, and with a bare file
//! name `ridl lock`, `ridl baseline`, `ridl build` and `ridl diff`. The tests
//! pin the package load, not only the exit code: `a.ridl` uses a type that
//! only the sibling file `types.typl` declares, a broken sibling file is
//! reported for an entry elsewhere in the package, and the files a command
//! writes land at the package root.

use std::path::{Path, PathBuf};
use std::process::Command;

fn package_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ridl-bare-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).expect("create the fixture");
    std::fs::write(
        dir.join("ridl.toml"),
        "[package]\nname = \"p\"\nversion = \"1.0.0\"\n",
    )
    .expect("write the manifest");
    std::fs::write(
        dir.join("types.typl"),
        "package p\n\ntype Speed: km/h [0..250]\n",
    )
    .expect("write the type file");
    std::fs::write(
        dir.join("a.ridl"),
        "package p\n\nstruct Gauge {\n  speed: Speed\n}\n",
    )
    .expect("write the source");
    std::fs::write(dir.join("src").join("b.ridl"), "package p.src\n")
        .expect("write the nested source");
    dir
}

fn check_in(dir: &Path, cwd: &str, entry: &str) -> (Option<i32>, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
        .current_dir(dir.join(cwd))
        .args(["check", entry])
        .output()
        .expect("run ridl check");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stderr).into_owned()
            + &String::from_utf8_lossy(&output.stdout),
    )
}

#[test]
fn a_bare_file_name_checks_the_package_it_sits_in() {
    let dir = package_dir("bare");
    let (code, text) = check_in(&dir, ".", "a.ridl");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(code, Some(0), "{text}");
}

#[test]
fn a_relative_path_below_the_package_root_checks_the_package() {
    let dir = package_dir("below");
    std::fs::write(
        dir.join("bad.ridl"),
        "package p\n\nstruct Bad {\n  x: Nope\n}\n",
    )
    .expect("write the broken source");
    // The broken sibling is in the package, so a package load reports it
    // for an entry in any directory of the package.
    let results = [
        check_in(&dir, ".", "src/b.ridl"),
        check_in(&dir, ".", "src"),
        check_in(&dir, "src", "b.ridl"),
    ];
    let _ = std::fs::remove_dir_all(&dir);
    for (code, text) in &results {
        assert_eq!(*code, Some(1), "{text}");
        assert!(text.contains("TYPL-011"), "{text}");
    }
    // A root above the current directory keeps its relative form.
    assert!(results[2].1.contains("../bad.ridl"), "{}", results[2].1);
}

/// A package whose `a.ridl` declares an interface over a type that only the
/// sibling file `types.typl` declares, so a load of `a.ridl` alone fails to
/// resolve it.
fn interface_package(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ridl-bare-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create the fixture");
    std::fs::write(
        dir.join("ridl.toml"),
        "[package]\nname = \"veh.cluster\"\nversion = \"1.0.0\"\n",
    )
    .expect("write the manifest");
    std::fs::write(
        dir.join("types.typl"),
        "package veh.cluster\n\ntype Flag: integer [0..1]\n",
    )
    .expect("write the type file");
    std::fs::write(
        dir.join("a.ridl"),
        "package veh.cluster\n\ninterface Status {\n  event opened: Flag @[100ms..1s]\n}\n",
    )
    .expect("write the source");
    dir
}

/// Runs `ridl` with `args` from `dir`, returning the exit code and the
/// combined output.
fn ridl_in(dir: &Path, args: &[&str]) -> (Option<i32>, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
        .current_dir(dir)
        .args(args)
        .output()
        .expect("run ridl");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stderr).into_owned()
            + &String::from_utf8_lossy(&output.stdout),
    )
}

/// `ridl lock` with a bare file name locks the package it sits in and writes
/// the lockfile at the package root.
#[test]
fn lock_with_a_bare_file_name_locks_the_package() {
    let dir = interface_package("lock");
    let (code, text) = ridl_in(&dir, &["lock", "a.ridl"]);
    let locked = dir.join("interfaces.lock").is_file();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(code, Some(0), "{text}");
    assert!(locked, "the lockfile is at the package root: {text}");
}

/// `ridl baseline` with a bare file name publishes the package it sits in
/// under the package root's `.ridl/baseline/`.
#[test]
fn baseline_with_a_bare_file_name_publishes_the_package() {
    let dir = interface_package("baseline");
    let (lock_code, lock_text) = ridl_in(&dir, &["lock", "a.ridl"]);
    let (code, text) = ridl_in(&dir, &["baseline", "a.ridl"]);
    let published = dir.join(".ridl/baseline/veh.cluster.ir.json").is_file();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(lock_code, Some(0), "{lock_text}");
    assert_eq!(code, Some(0), "{text}");
    assert!(published, "the snapshot is at the package root: {text}");
}

/// `ridl build` with a bare file name builds the package it sits in: the
/// emitted crate takes its name from the package manifest, which the build
/// finds through the root of the entry.
#[test]
fn build_with_a_bare_file_name_builds_the_package() {
    let dir = interface_package("build");
    let (code, text) = ridl_in(&dir, &["build", "a.ridl", "--out-dir", "out"]);
    let manifest = std::fs::read_to_string(dir.join("out/Cargo.toml")).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(code, Some(0), "{text}");
    assert!(
        manifest.contains("name = \"veh_cluster\""),
        "the crate is named after the package:\n{manifest}"
    );
}

/// `ridl diff` with a bare file name on each side compares the package the
/// file sits in.
#[test]
fn diff_with_a_bare_file_name_compares_the_package() {
    let dir = interface_package("diff");
    let (code, text) = ridl_in(&dir, &["diff", "a.ridl", "a.ridl"]);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(code, Some(0), "{text}");
    assert!(text.contains("identical"), "{text}");
}
