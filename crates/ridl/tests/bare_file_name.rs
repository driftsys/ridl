//! `ridl check a.ridl` with a bare relative file name, run from inside a
//! package directory, checks the package (driftsys/ridl#795). The parent of a
//! bare name is the empty path, which stands for the current directory.

use std::path::PathBuf;
use std::process::Command;

fn package_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ridl-bare-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create the fixture");
    std::fs::write(
        dir.join("ridl.toml"),
        "[package]\nname = \"p\"\nversion = \"1.0.0\"\n",
    )
    .expect("write the manifest");
    std::fs::write(dir.join("a.ridl"), "package p\n").expect("write the source");
    dir
}

#[test]
fn check_accepts_a_bare_file_name_inside_a_package_directory() {
    let dir = package_dir("check");
    let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
        .current_dir(&dir)
        .args(["check", "a.ridl"])
        .output()
        .expect("run ridl check");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(output.status.code(), Some(0), "{stderr}");
}
