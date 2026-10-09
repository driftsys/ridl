//! Integration tests for the `<unit>.catalogs` files `ridl baseline` publishes
//! beside the snapshots: the catalog hash of the baseline being published,
//! then the earlier hashes a consumer built against still works with, back to
//! the last breaking change.

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
            "ridl-catalog-chain-{label}-{}-{}",
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

/// The unit of the one package every fixture declares.
const UNIT: &str = "veh.cluster";

/// Writes `source` as the workspace's one source file and records its
/// interface numbers with plain `ridl lock`, since `ridl baseline` refuses a
/// provisional number (RIDL-411).
fn set_source(dir: &TempDir, source: &str) -> PathBuf {
    dir.write("ridl.toml", MANIFEST);
    dir.write("cluster.ridl", source);
    let root = dir.path().to_path_buf();
    let (code, _, stderr) = ridl(&["lock".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the fixture's lock is allocated: {stderr}");
    root
}

/// Runs `ridl baseline` on `root`, returning `(exit_code, stderr)`.
fn baseline(root: &Path) -> (i32, String) {
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    (code, stderr)
}

/// Runs `ridl baseline` on `root` and asserts that it publishes.
fn publish(root: &Path) {
    let (code, stderr) = baseline(root);
    assert_eq!(code, 0, "the baseline publishes: {stderr}");
}

fn baseline_dir(root: &Path) -> PathBuf {
    root.join(".ridl").join("baseline")
}

/// The hash lines of the published `<UNIT>.catalogs`, newest first.
fn history_lines(root: &Path) -> Vec<String> {
    let path = baseline_dir(root).join(format!("{UNIT}.catalogs"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    text.lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// Builds `root` with `--emit catalog` into `out`, runs `ridl describe` on
/// `<unit>.catalog.binfb`, and returns the descriptor's `hash` as lowercase
/// hex.
fn describe_hash(root: &Path, out: &Path, unit: &str) -> String {
    let (code, _, stderr) = ridl(&[
        "build".as_ref(),
        root.as_os_str(),
        "--out-dir".as_ref(),
        out.as_os_str(),
        "--emit".as_ref(),
        "catalog".as_ref(),
    ]);
    assert_eq!(code, 0, "the catalog builds: {stderr}");
    let file = out.join(format!("{unit}.catalog.binfb"));
    let (code, stdout, stderr) = ridl(&["describe".as_ref(), file.as_os_str()]);
    assert_eq!(code, 0, "the descriptor is described: {stderr}");
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is JSON");
    json["hash"]
        .as_array()
        .expect("the descriptor has a hash array")
        .iter()
        .map(|byte| format!("{:02x}", byte.as_u64().expect("a hash byte is a number")))
        .collect()
}

const BASE: &str = "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
type DoorState: integer [0..1]
interface VehicleStatus {
  signal currentSpeed: Speed @10ms
  event doorOpened: DoorState @[100ms..1s]
}
";

/// `BASE` with one event appended: a compatible change.
const APPENDED: &str = "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
type DoorState: integer [0..1]
interface VehicleStatus {
  signal currentSpeed: Speed @10ms
  event doorOpened: DoorState @[100ms..1s]
  event doorClosed: DoorState @[100ms..1s]
}
";

/// `BASE` with the signal's payload type changed: a breaking change.
const RETYPED: &str = "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
type DoorState: integer [0..1]
interface VehicleStatus {
  signal currentSpeed: DoorState @10ms
  event doorOpened: DoorState @[100ms..1s]
}
";

/// `BASE` with `doorOpened` deleted and no `reserved` tombstone: refused at
/// publication (RIDL-408).
const BARE_REMOVAL: &str = "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
type DoorState: integer [0..1]
interface VehicleStatus {
  signal currentSpeed: Speed @10ms
}
";

/// `BASE` with the event's payload a struct that names the standard type
/// `Timestamp`, so the catalog reaches a declaration of `ridl.std`.
const NAMES_A_STANDARD_TYPE: &str = "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
struct DoorReport {
  observedAt: Timestamp
  open: boolean
}
interface VehicleStatus {
  signal currentSpeed: Speed @10ms
  event doorOpened: DoorReport @[100ms..1s]
}
";

/// A package that declares types only, so its unit has no interface shape.
const TYPES_ONLY: &str = "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
";

#[test]
fn the_first_publication_records_the_catalog_hash() {
    let dir = TempDir::new("first");
    let out = TempDir::new("first-out");
    let root = set_source(&dir, BASE);
    publish(&root);
    assert_eq!(
        history_lines(&root),
        vec![describe_hash(&root, out.path(), UNIT)]
    );
}

#[test]
fn a_compatible_publication_carries_the_earlier_hash_over() {
    let dir = TempDir::new("compatible");
    let out = TempDir::new("compatible-out");
    let root = set_source(&dir, BASE);
    publish(&root);
    let old_hash = describe_hash(&root, out.path(), UNIT);
    set_source(&dir, APPENDED);
    publish(&root);
    let new_hash = describe_hash(&root, out.path(), UNIT);
    assert_ne!(new_hash, old_hash, "the appended event changes the catalog");
    assert_eq!(history_lines(&root), vec![new_hash, old_hash]);
}

#[test]
fn a_breaking_publication_restarts_the_chain() {
    let dir = TempDir::new("breaking");
    let out = TempDir::new("breaking-out");
    let root = set_source(&dir, BASE);
    publish(&root);
    set_source(&dir, RETYPED);
    publish(&root);
    let new_hash = describe_hash(&root, out.path(), UNIT);
    assert_eq!(history_lines(&root), vec![new_hash]);
}

#[test]
fn a_hash_is_never_written_twice() {
    let dir = TempDir::new("identical");
    let out = TempDir::new("identical-out");
    let root = set_source(&dir, BASE);
    publish(&root);
    let old_hash = describe_hash(&root, out.path(), UNIT);
    set_source(&dir, APPENDED);
    publish(&root);
    // Published again with no change: the verdict is identical, the earlier
    // hashes are carried, and the new hash, already first in the replaced
    // file, is not listed a second time.
    publish(&root);
    let new_hash = describe_hash(&root, out.path(), UNIT);
    assert_eq!(history_lines(&root), vec![new_hash, old_hash]);
}

#[test]
fn a_replaced_baseline_without_a_history_file_starts_a_chain() {
    let dir = TempDir::new("no-history");
    let out = TempDir::new("no-history-out");
    let root = set_source(&dir, BASE);
    publish(&root);
    std::fs::remove_file(baseline_dir(&root).join(format!("{UNIT}.catalogs")))
        .expect("remove the published history");
    set_source(&dir, APPENDED);
    publish(&root);
    let new_hash = describe_hash(&root, out.path(), UNIT);
    assert_eq!(history_lines(&root), vec![new_hash]);
}

#[test]
fn a_unit_without_a_shape_gets_no_file() {
    let dir = TempDir::new("no-shape");
    let root = set_source(&dir, TYPES_ONLY);
    publish(&root);
    let names: Vec<String> = std::fs::read_dir(baseline_dir(&root))
        .expect("the baseline directory exists")
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        names.iter().any(|name| name.ends_with(".ir.json")),
        "the snapshot is published: {names:?}"
    );
    assert!(
        !names.iter().any(|name| name.ends_with(".catalogs")),
        "no history file is published: {names:?}"
    );
}

#[test]
fn a_stale_catalogs_file_is_removed() {
    let dir = TempDir::new("stale");
    let root = set_source(&dir, BASE);
    publish(&root);
    let baseline = baseline_dir(&root);
    std::fs::write(
        baseline.join("gone.catalogs"),
        format!("{}\n", "00".repeat(32)),
    )
    .expect("write a stale history file");
    publish(&root);
    assert!(!baseline.join("gone.catalogs").exists());
    assert!(baseline.join(format!("{UNIT}.catalogs")).exists());
}

#[test]
fn a_refused_publication_writes_no_history() {
    let dir = TempDir::new("refused");
    let root = set_source(&dir, BASE);
    publish(&root);
    let path = baseline_dir(&root).join(format!("{UNIT}.catalogs"));
    let before = std::fs::read(&path).expect("read the published history");
    set_source(&dir, BARE_REMOVAL);
    let (code, stderr) = baseline(&root);
    assert_eq!(code, 1, "the bare removal is refused: {stderr}");
    assert!(stderr.contains("RIDL-408"), "{stderr}");
    assert_eq!(
        std::fs::read(&path).expect("read the history again"),
        before
    );
}

#[test]
fn an_unreadable_history_file_refuses_the_publication() {
    let dir = TempDir::new("unreadable");
    let root = set_source(&dir, BASE);
    publish(&root);
    let published = baseline_dir(&root);
    let snapshot = published.join(format!("{UNIT}.ir.json"));
    let snapshot_before = std::fs::read(&snapshot).expect("read the published snapshot");
    std::fs::write(published.join(format!("{UNIT}.catalogs")), "garbage\n")
        .expect("overwrite the published history");
    // A breaking change: the earlier hashes are not carried, and the file is
    // still read, so it still refuses the publication.
    set_source(&dir, RETYPED);
    let (code, stderr) = baseline(&root);
    assert_eq!(code, 2, "an unreadable history is an error: {stderr}");
    assert!(stderr.contains(".catalogs"), "{stderr}");
    assert_eq!(
        std::fs::read(&snapshot).expect("read the snapshot again"),
        snapshot_before,
        "the snapshot is not replaced"
    );
}

#[test]
fn a_catalog_that_names_a_standard_type_records_the_built_hash() {
    let dir = TempDir::new("std");
    let out = TempDir::new("std-out");
    let root = set_source(&dir, NAMES_A_STANDARD_TYPE);
    publish(&root);
    assert_eq!(
        history_lines(&root),
        vec![describe_hash(&root, out.path(), UNIT)]
    );
}

#[test]
fn a_history_of_a_unit_with_no_published_snapshot_is_not_carried() {
    let dir = TempDir::new("unpublished-unit");
    let out = TempDir::new("unpublished-unit-out");
    dir.write(
        "common/ridl.toml",
        "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n",
    );
    dir.write(
        "common/common.typl",
        "package veh.common\ntype Speed: km/h [0.0..250.0 step 0.5]\n",
    );
    dir.write("ridl.toml", "[workspace]\nmembers = [\"common\"]\n");
    let root = dir.path().to_path_buf();
    publish(&root);
    // A history left under the name of a unit the published baseline does
    // not hold.
    let marker = "ab".repeat(32);
    std::fs::write(
        baseline_dir(&root).join(format!("{UNIT}.catalogs")),
        format!("{marker}\n"),
    )
    .expect("write the leftover history");

    // The unit is added: every change is an addition, a compatible verdict.
    dir.write(
        "ridl.toml",
        "[workspace]\nmembers = [\"common\", \"cluster\"]\n",
    );
    dir.write("cluster/ridl.toml", MANIFEST);
    dir.write("cluster/cluster.ridl", BASE);
    let (code, _, stderr) = ridl(&["lock".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the added unit's lock is allocated: {stderr}");
    publish(&root);

    let hash = describe_hash(&root, out.path(), UNIT);
    assert_eq!(history_lines(&root), vec![hash]);
}
