//! Integration tests for `ridl diff` over an interface number that `ridl lock`
//! froze or that a sibling interface moved (driftsys/ridl#700). The catalog
//! hash covers each interface's number and its provisional flag, so neither
//! change may be reported as `identical`.

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
            "ridl-diff-number-{label}-{}-{}",
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

/// Runs `ridl` with `args`, returning `(exit_code, stdout, stderr)`.
fn ridl(args: &[&std::ffi::OsStr]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
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

const MANIFEST: &str = "[package]\nname = \"veh.cluster\"\nversion = \"1.0.0\"\n";

const DOORS: &str = "package veh.cluster
type DoorState: integer [0..1]
interface Doors {
  event doorOpened: DoorState
}
";

/// Writes a workspace with no lock file under `side`, so every interface
/// number is provisional.
fn workspace(dir: &TempDir, side: &str, source: &str) -> PathBuf {
    dir.write(&format!("{side}/ridl.toml"), MANIFEST);
    dir.write(&format!("{side}/cluster.ridl"), source);
    dir.path().join(side)
}

/// `ridl lock` freezes the provisional number in place. The old side is the
/// workspace before the lock, the new side the same workspace after it: the
/// change is `interface_frozen`, compatible (exit 0), under the heading it
/// shares with `enum_reordered`, and not `identical`.
#[test]
fn a_lock_that_freezes_a_provisional_number_is_interface_frozen() {
    let dir = TempDir::new("frozen");
    let old = workspace(&dir, "old", DOORS);
    let new = workspace(&dir, "new", DOORS);
    let (code, _, stderr) = ridl(&["lock".as_ref(), new.as_os_str()]);
    assert_eq!(code, 0, "ridl lock writes the lock: {stderr}");

    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), new.as_os_str()]);
    assert_eq!(code, 0, "a frozen number is compatible, stderr:\n{stderr}");
    assert_eq!(
        stdout,
        "compatible\ncompatible on the wire, may change the catalog hash:\n  [compatible] interface_frozen veh.cluster/Doors: 1 (provisional) -> 1\n"
    );
}

/// A sibling interface added before `Doors` in name byte order moves its
/// provisional number from 1 to 2. The number is the routing key, so the
/// change is `interface_number_changed`, breaking (exit 1).
#[test]
fn a_sibling_that_moves_a_provisional_number_is_interface_number_changed() {
    let dir = TempDir::new("moved");
    let old = workspace(&dir, "old", DOORS);
    let new = workspace(
        &dir,
        "new",
        &format!("{DOORS}interface Aux {{\n  event on: DoorState\n}}\n"),
    );

    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), new.as_os_str()]);
    assert_eq!(code, 1, "a moved number is breaking, stderr:\n{stderr}");
    assert_eq!(
        stdout,
        "breaking\n  [breaking] interface_number_changed veh.cluster/Doors: 1 (provisional) -> 2 (provisional)\n  [compatible] decl_added veh.cluster/Aux: (absent) -> interface\n"
    );
}
