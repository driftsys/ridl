//! `ridl check` with a relative entry whose package root is the current
//! directory or above it checks the package (driftsys/ridl#795). The tests
//! pin the package load, not only the exit code: `a.ridl` uses a type that
//! only the sibling file `types.typl` declares, and a broken sibling file is
//! reported for an entry elsewhere in the package.

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
