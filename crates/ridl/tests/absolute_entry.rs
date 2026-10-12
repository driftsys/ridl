//! An absolute entry is normalised lexically before the search for its
//! `ridl.toml`, as a relative entry is (driftsys/ridl#846). Without that,
//! `ridl check /ws/pkg/../other` found `/ws/pkg/ridl.toml` and loaded the
//! unrelated package `pkg`.

use std::path::PathBuf;
use std::process::Command;

const RIDL: &str = env!("CARGO_BIN_EXE_ridl");

/// A unique temp directory, removed on drop.
struct Fixture(PathBuf);

impl Fixture {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("ridl-absolute-entry-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create the fixture");
        Fixture(root)
    }

    fn write(&self, relative: &str, text: &str) {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().expect("relative paths have a parent"))
            .expect("create parent directories");
        std::fs::write(path, text).expect("write the fixture file");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn an_absolute_entry_with_dotdot_does_not_load_a_sibling_package() {
    let fixture = Fixture::new("dotdot");
    fixture.write(
        "pkg/ridl.toml",
        "[package]\nname = \"pkg\"\nversion = \"1.0.0\"\n",
    );
    fixture.write(
        "pkg/a.ridl",
        "package pkg\n\nstruct S {\n    field: Unknown,\n}\n",
    );
    fixture.write("other/b.ridl", "package other\n");

    let entry = fixture.0.join("pkg/../other");
    let output = Command::new(RIDL)
        .arg("check")
        .arg(&entry)
        .output()
        .expect("the binary must run");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(
        stderr.contains("no `ridl.toml` found at or above"),
        "{stderr}"
    );
}
