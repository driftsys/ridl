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
    history_lines_of(root, UNIT)
}

/// The hash lines of the published `<unit>.catalogs`, newest first.
fn history_lines_of(root: &Path, unit: &str) -> Vec<String> {
    let path = baseline_dir(root).join(format!("{unit}.catalogs"));
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
    assert!(
        !root.join(".ridl").join(".baseline.staging").exists(),
        "the staging directory is removed"
    );
}

/// A history path that is not a readable file (here a directory) is an error
/// other than a missing file: the build fails with exit 2, and the
/// publication is refused with exit 2 and the staging directory is removed.
#[test]
fn a_history_path_that_cannot_be_read_refuses_the_publication() {
    let dir = TempDir::new("history-is-a-directory");
    let out = TempDir::new("history-is-a-directory-out");
    let root = set_source(&dir, BASE);
    publish(&root);
    let history = baseline_dir(&root).join(format!("{UNIT}.catalogs"));
    std::fs::remove_file(&history).expect("remove the published history");
    std::fs::create_dir(&history).expect("put a directory in its place");
    set_source(&dir, APPENDED);
    let (code, stderr) = build(&root, out.path(), "catalog");
    assert_eq!(code, 2, "an unreadable history fails the build: {stderr}");
    assert!(
        !stderr.contains("could not be computed"),
        "the cause is reported once, by name: {stderr}"
    );
    assert!(
        stderr.contains(&history.display().to_string()),
        "stderr names the file: {stderr}"
    );
    let (code, stderr) = baseline(&root);
    assert_eq!(code, 2, "an unreadable history is an error: {stderr}");
    assert!(
        stderr.contains(&history.display().to_string()),
        "stderr names the file: {stderr}"
    );
    assert!(
        !root.join(".ridl").join(".baseline.staging").exists(),
        "the staging directory is removed"
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

/// `NAMES_A_STANDARD_TYPE` with a second `Timestamp` field appended to the
/// struct: a compatible change to a type that reaches `ridl.std`.
const NAMES_A_STANDARD_TYPE_TWICE: &str = "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
struct DoorReport {
  observedAt: Timestamp
  open: boolean
  closedAt: Timestamp
}
interface VehicleStatus {
  signal currentSpeed: Speed @10ms
  event doorOpened: DoorReport @[100ms..1s]
}
";

/// A catalog that reaches `ridl.std` is compared and hashed with the
/// standard package in scope, at publication and at build time: the
/// published tree lists nothing, a compatible change lists the baseline, and
/// the chain is carried at the second publication.
#[test]
fn a_chain_that_reaches_a_standard_type_is_carried() {
    let dir = TempDir::new("std-chain");
    let first = TempDir::new("std-chain-first");
    let second = TempDir::new("std-chain-second");
    let published_tree = TempDir::new("std-chain-published");
    let changed_tree = TempDir::new("std-chain-changed");
    let republished_tree = TempDir::new("std-chain-republished");
    let root = set_source(&dir, NAMES_A_STANDARD_TYPE);
    publish(&root);
    let first_hash = describe_hash(&root, first.path(), UNIT);
    build_catalog(&root, published_tree.path());
    assert_eq!(
        compatible_of(published_tree.path(), UNIT),
        Vec::<String>::new(),
        "the published tree lists nothing"
    );
    set_source(&dir, NAMES_A_STANDARD_TYPE_TWICE);
    build_catalog(&root, changed_tree.path());
    assert_eq!(
        compatible_of(changed_tree.path(), UNIT),
        vec![first_hash.clone()],
        "the compatible change lists the baseline"
    );
    publish(&root);
    let second_hash = describe_hash(&root, second.path(), UNIT);
    assert_eq!(history_lines(&root), vec![second_hash, first_hash.clone()]);
    build_catalog(&root, republished_tree.path());
    assert_eq!(
        compatible_of(republished_tree.path(), UNIT),
        vec![first_hash]
    );
}

/// `BASE` with a type that no interface reaches.
const BASE_WITH_UNREACHED: &str = "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
type DoorState: integer [0..1]
type Unreached: integer [0..9]
interface VehicleStatus {
  signal currentSpeed: Speed @10ms
  event doorOpened: DoorState @[100ms..1s]
}
";

/// `APPENDED` with a type that no interface reaches.
const APPENDED_WITH_UNREACHED: &str = "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
type DoorState: integer [0..1]
type Unreached: integer [0..9]
interface VehicleStatus {
  signal currentSpeed: Speed @10ms
  event doorOpened: DoorState @[100ms..1s]
  event doorClosed: DoorState @[100ms..1s]
}
";

/// Removing a declaration no interface reaches is a breaking change of the
/// unit, but it leaves the catalog hash as published: the catalog did not
/// change, so the chain is carried at build time and at publication.
#[test]
fn an_unchanged_catalog_carries_the_chain_whatever_the_verdict() {
    let dir = TempDir::new("unchanged-catalog");
    let first = TempDir::new("unchanged-catalog-first");
    let second = TempDir::new("unchanged-catalog-second");
    let out = TempDir::new("unchanged-catalog-out");
    let root = set_source(&dir, BASE_WITH_UNREACHED);
    publish(&root);
    let first_hash = describe_hash(&root, first.path(), UNIT);
    set_source(&dir, APPENDED_WITH_UNREACHED);
    publish(&root);
    let second_hash = describe_hash(&root, second.path(), UNIT);

    set_source(&dir, APPENDED);
    build_catalog(&root, out.path());
    assert_eq!(
        hex_of_bytes(&describe(out.path(), UNIT)["hash"]),
        second_hash,
        "the removal leaves the catalog hash as published"
    );
    assert_eq!(
        compatible_of(out.path(), UNIT),
        vec![first_hash.clone()],
        "the build carries the chain"
    );
    publish(&root);
    assert_eq!(
        history_lines(&root),
        vec![second_hash, first_hash],
        "the publication carries the chain"
    );
}

/// A two-unit workspace: `veh.cluster` with `cluster_source` and `veh.body`
/// with `body_source`.
fn write_two_units(dir: &TempDir, cluster_source: &str, body_source: &str) -> PathBuf {
    dir.write(
        "ridl.toml",
        "[workspace]\nmembers = [\"cluster\", \"body\"]\n",
    );
    dir.write("cluster/ridl.toml", MANIFEST);
    dir.write("cluster/cluster.ridl", cluster_source);
    dir.write(
        "body/ridl.toml",
        "[package]\nname = \"veh.body\"\nversion = \"1.0.0\"\n",
    );
    dir.write("body/body.ridl", body_source);
    let root = dir.path().to_path_buf();
    lock(&root);
    root
}

const BODY: &str = "package veh.body
type Level: integer [0..100]
interface Lights {
  signal level: Level @10ms
}
";

/// `BODY` with one signal appended: a compatible change.
const BODY_APPENDED: &str = "package veh.body
type Level: integer [0..100]
interface Lights {
  signal level: Level @10ms
  signal dimmed: Level @10ms
}
";

/// Each unit is judged by its own verdict: a breaking change in one unit
/// restarts that unit's chain and leaves the other unit's chain carried, at
/// build time and at publication.
#[test]
fn a_break_in_one_unit_leaves_the_other_chain_carried() {
    let dir = TempDir::new("two-units");
    let before = TempDir::new("two-units-before");
    let out = TempDir::new("two-units-out");
    let after = TempDir::new("two-units-after");
    let root = write_two_units(&dir, BASE, BODY);
    publish(&root);
    let body_hash = describe_hash(&root, before.path(), "veh.body");

    write_two_units(&dir, RETYPED, BODY_APPENDED);
    build_catalog(&root, out.path());
    assert_eq!(
        compatible_of(out.path(), UNIT),
        Vec::<String>::new(),
        "the broken unit lists nothing"
    );
    assert_eq!(
        compatible_of(out.path(), "veh.body"),
        vec![body_hash.clone()],
        "the compatible unit lists its baseline"
    );

    publish(&root);
    let cluster_hash = describe_hash(&root, after.path(), UNIT);
    let new_body_hash = hex_of_bytes(&describe(after.path(), "veh.body")["hash"]);
    assert_eq!(history_lines(&root), vec![cluster_hash]);
    assert_eq!(
        history_lines_of(&root, "veh.body"),
        vec![new_body_hash, body_hash]
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

/// A history left under the name of a unit the published baseline holds no
/// snapshot of is never carried, so it is not read: a malformed one does not
/// refuse the publication.
#[test]
fn a_malformed_history_of_a_unit_with_no_published_snapshot_is_not_read() {
    let dir = TempDir::new("unpublished-unit-malformed");
    let out = TempDir::new("unpublished-unit-malformed-out");
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
    std::fs::write(
        baseline_dir(&root).join(format!("{UNIT}.catalogs")),
        "garbage\n",
    )
    .expect("write the malformed leftover history");

    dir.write(
        "ridl.toml",
        "[workspace]\nmembers = [\"common\", \"cluster\"]\n",
    );
    dir.write("cluster/ridl.toml", MANIFEST);
    dir.write("cluster/cluster.ridl", BASE);
    lock(&root);
    publish(&root);

    let hash = describe_hash(&root, out.path(), UNIT);
    assert_eq!(history_lines(&root), vec![hash]);
}

// ==========================================================================
// What `ridl build` writes from the chain
// ==========================================================================

/// Runs `ridl build` on `root` into `out` with `--emit <emits>`, returning
/// `(exit_code, stderr)`.
fn build(root: &Path, out: &Path, emits: &str) -> (i32, String) {
    let (code, _, stderr) = ridl(&[
        "build".as_ref(),
        root.as_os_str(),
        "--out-dir".as_ref(),
        out.as_os_str(),
        "--emit".as_ref(),
        emits.as_ref(),
    ]);
    (code, stderr)
}

/// Runs `ridl build --emit catalog` on `root` into `out` and asserts that it
/// exits 0.
fn build_catalog(root: &Path, out: &Path) {
    let (code, stderr) = build(root, out, "catalog");
    assert_eq!(code, 0, "the catalog builds: {stderr}");
}

/// `ridl describe` of `out/<unit>.catalog.binfb`.
fn describe(out: &Path, unit: &str) -> serde_json::Value {
    let file = out.join(format!("{unit}.catalog.binfb"));
    let (code, stdout, stderr) = ridl(&["describe".as_ref(), file.as_os_str()]);
    assert_eq!(code, 0, "the descriptor is described: {stderr}");
    serde_json::from_str(&stdout).expect("stdout is JSON")
}

fn hex_of_bytes(bytes: &serde_json::Value) -> String {
    bytes
        .as_array()
        .expect("a hash is an array of bytes")
        .iter()
        .map(|byte| format!("{:02x}", byte.as_u64().expect("a hash byte is a number")))
        .collect()
}

/// The `compatible` hashes of the descriptor `out/<unit>.catalog.binfb`, as
/// lowercase hex, in the descriptor's order.
fn compatible_of(out: &Path, unit: &str) -> Vec<String> {
    describe(out, unit)["compatible"]
        .as_array()
        .expect("the descriptor has a compatible array")
        .iter()
        .map(hex_of_bytes)
        .collect()
}

/// The `catalog.compatible` hashes of the codegen model `out/<pkg>.codegen.json`,
/// as lowercase hex, in the model's order. The model serializes a byte
/// string as standard base64.
fn model_compatible_of(out: &Path, pkg: &str) -> Vec<String> {
    let path = out.join(format!("{pkg}.codegen.json"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    let json: serde_json::Value = serde_json::from_str(&text).expect("the model is JSON");
    json["catalog"]["compatible"]
        .as_array()
        .expect("the model's catalog has a compatible array")
        .iter()
        .map(|hash| base64_to_hex(hash.as_str().expect("a model hash is a base64 string")))
        .collect()
}

/// Decodes standard base64 with `=` padding into lowercase hex.
fn base64_to_hex(text: &str) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut bytes = Vec::new();
    let mut buffer: u32 = 0;
    let mut bits = 0;
    for symbol in text.bytes().filter(|symbol| *symbol != b'=') {
        let value = ALPHABET
            .iter()
            .position(|candidate| *candidate == symbol)
            .expect("a base64 symbol") as u32;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            bytes.push(((buffer >> bits) & 0xff) as u8);
        }
    }
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn a_build_after_a_compatible_change_lists_the_baseline_hash() {
    let dir = TempDir::new("build-compatible");
    let before = TempDir::new("build-compatible-before");
    let out = TempDir::new("build-compatible-out");
    let root = set_source(&dir, BASE);
    publish(&root);
    let baseline_hash = describe_hash(&root, before.path(), UNIT);
    set_source(&dir, APPENDED);
    let (code, stderr) = build(&root, out.path(), "catalog,codegen-model");
    assert_eq!(code, 0, "the build succeeds: {stderr}");
    assert_eq!(compatible_of(out.path(), UNIT), vec![baseline_hash.clone()]);
    assert_eq!(model_compatible_of(out.path(), UNIT), vec![baseline_hash]);
}

#[test]
fn a_build_after_a_breaking_change_lists_nothing() {
    let dir = TempDir::new("build-breaking");
    let out = TempDir::new("build-breaking-out");
    let root = set_source(&dir, BASE);
    publish(&root);
    set_source(&dir, APPENDED);
    publish(&root);
    assert_eq!(history_lines(&root).len(), 2, "the chain holds two hashes");
    set_source(&dir, RETYPED);
    build_catalog(&root, out.path());
    assert_eq!(compatible_of(out.path(), UNIT), Vec::<String>::new());
}

#[test]
fn a_build_with_no_baseline_lists_nothing() {
    let dir = TempDir::new("build-no-baseline");
    let out = TempDir::new("build-no-baseline-out");
    let root = set_source(&dir, BASE);
    build_catalog(&root, out.path());
    assert_eq!(compatible_of(out.path(), UNIT), Vec::<String>::new());
}

#[test]
fn an_empty_baseline_directory_is_no_baseline() {
    let dir = TempDir::new("build-empty-baseline");
    let out = TempDir::new("build-empty-baseline-out");
    let root = set_source(&dir, BASE);
    std::fs::create_dir_all(baseline_dir(&root)).expect("create the empty baseline directory");
    let (code, stderr) = build(&root, out.path(), "catalog");
    assert_eq!(code, 0, "an empty directory is no baseline: {stderr}");
    assert_eq!(compatible_of(out.path(), UNIT), Vec::<String>::new());
}

/// A directory with no snapshot is no baseline, so a history file in it is
/// not read: there is no earlier baseline for the hashes to name.
#[test]
fn a_history_file_without_a_snapshot_is_not_read() {
    let dir = TempDir::new("build-history-no-snapshot");
    let out = TempDir::new("build-history-no-snapshot-out");
    let root = set_source(&dir, BASE);
    std::fs::create_dir_all(baseline_dir(&root)).expect("create the baseline directory");
    std::fs::write(
        baseline_dir(&root).join(format!("{UNIT}.catalogs")),
        format!("{}\n", "ab".repeat(32)),
    )
    .expect("write the stray history");
    let (code, stderr) = build(&root, out.path(), "catalog");
    assert_eq!(
        code, 0,
        "a directory with no snapshot is no baseline: {stderr}"
    );
    assert_eq!(compatible_of(out.path(), UNIT), Vec::<String>::new());
}

#[test]
fn the_current_hash_is_never_listed() {
    let dir = TempDir::new("build-current");
    let before = TempDir::new("build-current-before");
    let out = TempDir::new("build-current-out");
    let root = set_source(&dir, BASE);
    publish(&root);
    let older_hash = describe_hash(&root, before.path(), UNIT);
    set_source(&dir, APPENDED);
    publish(&root);
    // The tree is the published baseline: the file's first hash is the
    // current one, and the list holds the earlier one only.
    build_catalog(&root, out.path());
    assert_eq!(compatible_of(out.path(), UNIT), vec![older_hash]);
}

#[test]
fn a_baseline_that_cannot_be_loaded_fails_the_build() {
    let dir = TempDir::new("build-unloadable");
    let out = TempDir::new("build-unloadable-out");
    let root = set_source(&dir, BASE);
    publish(&root);
    let snapshot = baseline_dir(&root).join(format!("{UNIT}.ir.json"));
    std::fs::write(&snapshot, "{").expect("damage the snapshot");
    let (code, stderr) = build(&root, out.path(), "catalog");
    assert_eq!(code, 2, "stderr:\n{stderr}");
    assert!(
        stderr.contains(&snapshot.display().to_string()),
        "stderr names the snapshot:\n{stderr}"
    );
}

#[test]
fn a_malformed_history_file_fails_the_build() {
    let dir = TempDir::new("build-malformed-history");
    let out = TempDir::new("build-malformed-history-out");
    let root = set_source(&dir, BASE);
    publish(&root);
    let history = baseline_dir(&root).join(format!("{UNIT}.catalogs"));
    std::fs::write(&history, "garbage\n").expect("damage the history");
    let (code, stderr) = build(&root, out.path(), "catalog");
    assert_eq!(code, 2, "stderr:\n{stderr}");
    assert!(
        stderr.contains(&history.display().to_string()) && stderr.contains("line 1"),
        "stderr names the file and the line:\n{stderr}"
    );
}

/// The list is carried, never hashed: the same tree built with a chain
/// behind it and with none has the same catalog hash.
#[test]
fn the_compatible_list_does_not_feed_the_hash() {
    let dir = TempDir::new("build-hash-independent");
    let with_chain = TempDir::new("build-hash-independent-with");
    let without_chain = TempDir::new("build-hash-independent-without");
    let root = set_source(&dir, BASE);
    publish(&root);
    set_source(&dir, APPENDED);
    build_catalog(&root, with_chain.path());
    assert_eq!(
        compatible_of(with_chain.path(), UNIT).len(),
        1,
        "the first build lists the baseline"
    );
    std::fs::remove_dir_all(baseline_dir(&root)).expect("remove the baseline");
    build_catalog(&root, without_chain.path());
    assert_eq!(
        compatible_of(without_chain.path(), UNIT),
        Vec::<String>::new()
    );
    assert_eq!(
        hex_of_bytes(&describe(with_chain.path(), UNIT)["hash"]),
        hex_of_bytes(&describe(without_chain.path(), UNIT)["hash"])
    );
}

/// An IR dump carries no list, so the build does not read the baseline: a
/// damaged one fails a catalog build and not an `ir-json` one.
#[test]
fn an_ir_dump_build_reads_no_baseline() {
    let dir = TempDir::new("build-ir-dump");
    let out = TempDir::new("build-ir-dump-out");
    let root = set_source(&dir, BASE);
    publish(&root);
    std::fs::write(baseline_dir(&root).join(format!("{UNIT}.ir.json")), "{")
        .expect("damage the snapshot");
    let (code, stderr) = build(&root, out.path(), "ir-json");
    assert_eq!(code, 0, "stderr:\n{stderr}");
}

/// `APPENDED` with a second event appended: a compatible change on top of a
/// compatible change.
const APPENDED_TWICE: &str = "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
type DoorState: integer [0..1]
interface VehicleStatus {
  signal currentSpeed: Speed @10ms
  event doorOpened: DoorState @[100ms..1s]
  event doorClosed: DoorState @[100ms..1s]
  event doorLocked: DoorState @[100ms..1s]
}
";

/// Runs `ridl lock` on `root` and asserts that it allocates.
fn lock(root: &Path) {
    let (code, _, stderr) = ridl(&["lock".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the lock is allocated: {stderr}");
}

/// A history file for a unit the baseline holds no snapshot of is not read
/// at build time either: the baseline holds other units, so it is a
/// baseline, and the stray file still names no earlier baseline of this unit.
#[test]
fn a_stray_history_for_a_unit_with_no_snapshot_is_not_listed() {
    let dir = TempDir::new("build-stray-history");
    let out = TempDir::new("build-stray-history-out");
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
    std::fs::write(
        baseline_dir(&root).join(format!("{UNIT}.catalogs")),
        format!("{}\n", "ab".repeat(32)),
    )
    .expect("write the stray history");

    // The unit is added: every change is an addition, a compatible verdict.
    dir.write(
        "ridl.toml",
        "[workspace]\nmembers = [\"common\", \"cluster\"]\n",
    );
    dir.write("cluster/ridl.toml", MANIFEST);
    dir.write("cluster/cluster.ridl", BASE);
    lock(&root);
    build_catalog(&root, out.path());
    assert_eq!(compatible_of(out.path(), UNIT), Vec::<String>::new());
}

/// The list is keyed by unit, not by package: a sub-package of the unit gets
/// the unit's list in its codegen model, and another shaped unit with no
/// history gets an empty list in its descriptor and its model.
#[test]
fn the_list_is_keyed_by_unit() {
    let dir = TempDir::new("build-keyed-by-unit");
    let before = TempDir::new("build-keyed-by-unit-before");
    let out = TempDir::new("build-keyed-by-unit-out");
    dir.write(
        "ridl.toml",
        "[workspace]\nmembers = [\"cluster\", \"body\"]\n",
    );
    dir.write("cluster/ridl.toml", MANIFEST);
    dir.write("cluster/cluster.ridl", BASE);
    dir.write(
        "cluster/doors/doors.ridl",
        "package veh.cluster.doors
type Count: integer [0..4]
interface Doors {
  signal open: Count @100ms
}
",
    );
    dir.write(
        "body/ridl.toml",
        "[package]\nname = \"veh.body\"\nversion = \"1.0.0\"\n",
    );
    dir.write(
        "body/body.ridl",
        "package veh.body
type Level: integer [0..100]
interface Lights {
  signal level: Level @10ms
}
",
    );
    let root = dir.path().to_path_buf();
    lock(&root);
    publish(&root);
    let cluster_hash = describe_hash(&root, before.path(), UNIT);
    // The second unit keeps its snapshot and loses its history.
    std::fs::remove_file(baseline_dir(&root).join("veh.body.catalogs"))
        .expect("remove the second unit's history");

    dir.write("cluster/cluster.ridl", APPENDED);
    let (code, stderr) = build(&root, out.path(), "catalog,codegen-model");
    assert_eq!(code, 0, "the build succeeds: {stderr}");
    assert_eq!(
        compatible_of(out.path(), UNIT),
        vec![cluster_hash.clone()],
        "the unit's descriptor"
    );
    assert_eq!(
        compatible_of(out.path(), "veh.body"),
        Vec::<String>::new(),
        "the other unit's descriptor"
    );
    assert_eq!(
        model_compatible_of(out.path(), UNIT),
        vec![cluster_hash.clone()],
        "the unit's package"
    );
    assert_eq!(
        model_compatible_of(out.path(), "veh.cluster.doors"),
        vec![cluster_hash],
        "the unit's sub-package"
    );
    assert_eq!(
        model_compatible_of(out.path(), "veh.body"),
        Vec::<String>::new(),
        "the other unit's package"
    );
}

/// A chain of three lists the two earlier hashes newest first, the order of
/// the history file, in the descriptor and in the codegen model.
#[test]
fn a_list_of_several_hashes_is_newest_first() {
    let dir = TempDir::new("build-chain-of-three");
    let first = TempDir::new("build-chain-of-three-first");
    let second = TempDir::new("build-chain-of-three-second");
    let out = TempDir::new("build-chain-of-three-out");
    let root = set_source(&dir, BASE);
    publish(&root);
    let first_hash = describe_hash(&root, first.path(), UNIT);
    set_source(&dir, APPENDED);
    publish(&root);
    let second_hash = describe_hash(&root, second.path(), UNIT);
    set_source(&dir, APPENDED_TWICE);
    publish(&root);
    let (code, stderr) = build(&root, out.path(), "catalog,codegen-model");
    assert_eq!(code, 0, "the build succeeds: {stderr}");
    let expected = vec![second_hash, first_hash];
    assert_eq!(compatible_of(out.path(), UNIT), expected, "the descriptor");
    assert_eq!(
        model_compatible_of(out.path(), UNIT),
        expected,
        "the codegen model"
    );
}

/// A plugin generates code, so a build with a plugin and an IR dump alone
/// carries the list and reads the baseline: a damaged one fails that build
/// with exit 2 before the plugin is resolved, where a build with no plugin
/// does not read it.
#[test]
fn a_build_with_a_plugin_reads_the_baseline() {
    let dir = TempDir::new("build-plugin-gate");
    let out = TempDir::new("build-plugin-gate-out");
    let root = set_source(&dir, BASE);
    publish(&root);
    let snapshot = baseline_dir(&root).join(format!("{UNIT}.ir.json"));
    std::fs::write(&snapshot, "{").expect("damage the snapshot");
    let (code, _, stderr) = ridl(&[
        "build".as_ref(),
        root.as_os_str(),
        "--out-dir".as_ref(),
        out.path().as_os_str(),
        "--emit".as_ref(),
        "ir-json".as_ref(),
        "--plugin".as_ref(),
        "no-such-language-for-this-test".as_ref(),
    ]);
    assert_eq!(code, 2, "stderr:\n{stderr}");
    assert!(
        stderr.contains(&snapshot.display().to_string()),
        "stderr names the snapshot:\n{stderr}"
    );
}

/// A build with an error diagnostic reports the diagnostic and exits 1, as
/// it does with no baseline: the published chain is not an error source.
#[test]
fn a_build_with_a_source_error_reports_the_error() {
    let dir = TempDir::new("build-source-error");
    let out = TempDir::new("build-source-error-out");
    let root = set_source(&dir, BASE);
    publish(&root);
    dir.write(
        "cluster.ridl",
        "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
interface VehicleStatus {
  signal currentSpeed: Missing @10ms
}
",
    );
    let (code, stderr) = build(&root, out.path(), "catalog");
    assert_eq!(code, 1, "stderr:\n{stderr}");
    assert!(
        stderr.contains("Missing"),
        "stderr names the error:\n{stderr}"
    );
}

/// The error gate comes before the chain is read: a build with an error
/// diagnostic exits 1 even when the published history file is malformed,
/// which would otherwise fail the build with exit 2.
#[test]
fn a_build_with_a_source_error_does_not_read_the_history() {
    let dir = TempDir::new("build-source-error-malformed");
    let out = TempDir::new("build-source-error-malformed-out");
    let root = set_source(&dir, BASE);
    publish(&root);
    let history = baseline_dir(&root).join(format!("{UNIT}.catalogs"));
    std::fs::write(&history, "garbage\n").expect("damage the history");
    dir.write(
        "cluster.ridl",
        "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
interface VehicleStatus {
  signal currentSpeed: Missing @10ms
}
",
    );
    let (code, stderr) = build(&root, out.path(), "catalog");
    assert_eq!(code, 1, "stderr:\n{stderr}");
    assert!(
        stderr.contains("Missing"),
        "stderr names the error:\n{stderr}"
    );
    assert!(
        !stderr.contains(".catalogs"),
        "stderr does not name the history file:\n{stderr}"
    );
}

/// Only the first hash of the history carries the chain whatever the
/// verdict. A tree whose catalog hash is an older hash of the history, here
/// the source returned to `BASE` after `APPENDED` was published, is a
/// breaking change from the baseline, so its list is empty.
#[test]
fn only_the_head_of_the_history_carries_the_chain() {
    let dir = TempDir::new("build-older-hash");
    let first = TempDir::new("build-older-hash-first");
    let out = TempDir::new("build-older-hash-out");
    let root = set_source(&dir, BASE);
    publish(&root);
    let base_hash = describe_hash(&root, first.path(), UNIT);
    set_source(&dir, APPENDED);
    publish(&root);
    assert_eq!(history_lines(&root).len(), 2, "the chain holds two hashes");
    set_source(&dir, BASE);
    build_catalog(&root, out.path());
    assert_eq!(
        hex_of_bytes(&describe(out.path(), UNIT)["hash"]),
        base_hash,
        "the tree has the older hash of the history"
    );
    assert_eq!(compatible_of(out.path(), UNIT), Vec::<String>::new());
}

/// A struct with one required field: the payload of `STRUCT_WITH_OPTIONAL`
/// before its optional field is added.
const STRUCT_BASE: &str = "package veh.cluster
struct DoorReport {
  open: boolean
}
interface VehicleStatus {
  event doorOpened: DoorReport @[100ms..1s]
}
";

/// `STRUCT_BASE` with an optional field appended to the struct: a compatible
/// change. Removing the field again is a breaking change that no publication
/// check refuses.
const STRUCT_WITH_OPTIONAL: &str = "package veh.cluster
struct DoorReport {
  open: boolean
  ajar: boolean?
}
interface VehicleStatus {
  event doorOpened: DoorReport @[100ms..1s]
}
";

/// At publication too, only the first hash of the history carries the chain
/// whatever the verdict. Removing the optional field returns the catalog to
/// an older hash of the history, and the removal is breaking, so the chain
/// starts again from that hash alone.
#[test]
fn a_publication_back_to_an_older_hash_restarts_the_chain() {
    let dir = TempDir::new("publish-older-hash");
    let first = TempDir::new("publish-older-hash-first");
    let second = TempDir::new("publish-older-hash-second");
    let root = set_source(&dir, STRUCT_BASE);
    publish(&root);
    let first_hash = describe_hash(&root, first.path(), UNIT);
    set_source(&dir, STRUCT_WITH_OPTIONAL);
    publish(&root);
    let second_hash = describe_hash(&root, second.path(), UNIT);
    assert_eq!(
        history_lines(&root),
        vec![second_hash, first_hash.clone()],
        "the optional field is a compatible change"
    );
    set_source(&dir, STRUCT_BASE);
    publish(&root);
    assert_eq!(history_lines(&root), vec![first_hash]);
}

/// With no `<unit>.catalogs` file there is no history to carry, so a build
/// does not load the snapshots: a snapshot that cannot be parsed does not
/// fail it.
#[test]
fn a_baseline_with_no_history_file_is_not_loaded_by_a_build() {
    let dir = TempDir::new("build-no-history");
    let out = TempDir::new("build-no-history-out");
    let root = set_source(&dir, BASE);
    publish(&root);
    let baseline = baseline_dir(&root);
    std::fs::remove_file(baseline.join(format!("{UNIT}.catalogs"))).expect("remove the history");
    std::fs::write(baseline.join(format!("{UNIT}.ir.json")), "{").expect("damage the snapshot");
    let (code, stderr) = build(&root, out.path(), "catalog");
    assert_eq!(code, 0, "the build never loads the snapshot: {stderr}");
    assert_eq!(compatible_of(out.path(), UNIT), Vec::<String>::new());
}

/// A workspace that names `[imports]` gets a note that a unit's list covers
/// the workspace's own packages only, when there is a history to carry. The
/// frozen build fails on the missing lockfile entry, offline, after the note.
#[test]
fn a_workspace_with_imports_gets_a_note_beside_the_list() {
    let dir = TempDir::new("build-imports-note");
    let out = TempDir::new("build-imports-note-out");
    let root = set_source(&dir, BASE);
    publish(&root);
    let frozen_build = |out: &Path| {
        let (code, _, stderr) = ridl(&[
            "build".as_ref(),
            root.as_os_str(),
            "--out-dir".as_ref(),
            out.as_os_str(),
            "--emit".as_ref(),
            "catalog".as_ref(),
            "--frozen".as_ref(),
        ]);
        (code, stderr)
    };
    let (code, stderr) = frozen_build(out.path());
    assert_eq!(code, 0, "no imports, no failure: {stderr}");
    assert!(
        !stderr.contains("[imports]"),
        "no note without imports: {stderr}"
    );
    dir.write(
        "ridl.toml",
        &format!(
            "{MANIFEST}\n[imports]\n\"other.dep\" = \"https://registry.example.com/other/dep@v1.0.0\"\n"
        ),
    );
    let (code, stderr) = frozen_build(out.path());
    assert_eq!(
        code, 1,
        "the missing lockfile entry fails the build: {stderr}"
    );
    assert!(
        stderr.contains("note: the workspace names `[imports]`"),
        "the note is printed: {stderr}"
    );
}

const IMPORTS: &str =
    "\n[imports]\n\"other.dep\" = \"https://registry.example.com/other/dep@v1.0.0\"\n";

/// Runs a frozen `ridl build --emit catalog` on `root`, returning
/// `(exit_code, stderr)`.
fn frozen_catalog_build(root: &Path, out: &Path) -> (i32, String) {
    let (code, _, stderr) = ridl(&[
        "build".as_ref(),
        root.as_os_str(),
        "--out-dir".as_ref(),
        out.as_os_str(),
        "--emit".as_ref(),
        "catalog".as_ref(),
        "--frozen".as_ref(),
    ]);
    (code, stderr)
}

/// The note is printed once a list is computed, so a baseline that cannot be
/// loaded fails the build before it.
#[test]
fn a_workspace_with_imports_and_an_unloadable_baseline_gets_no_note() {
    let dir = TempDir::new("build-imports-bad-baseline");
    let out = TempDir::new("build-imports-bad-baseline-out");
    let root = set_source(&dir, BASE);
    publish(&root);
    std::fs::write(baseline_dir(&root).join(format!("{UNIT}.ir.json")), "{")
        .expect("damage the snapshot");
    dir.write("ridl.toml", &format!("{MANIFEST}{IMPORTS}"));
    let (code, stderr) = frozen_catalog_build(&root, out.path());
    assert_eq!(code, 2, "the baseline fails the build: {stderr}");
    assert!(!stderr.contains("note: the workspace names"), "{stderr}");
}

/// A unit with no published package gets no list, so no list is computed and
/// there is no note.
#[test]
fn a_workspace_with_imports_and_no_list_gets_no_note() {
    let dir = TempDir::new("build-imports-no-list");
    let out = TempDir::new("build-imports-no-list-out");
    let root = set_source(&dir, BASE);
    publish(&root);
    dir.write(
        "ridl.toml",
        &format!("[package]\nname = \"veh.other\"\nversion = \"1.0.0\"\n{IMPORTS}"),
    );
    dir.write("cluster.ridl", &BASE.replace("veh.cluster", "veh.other"));
    let (code, stderr) = frozen_catalog_build(&root, out.path());
    assert_eq!(
        code, 1,
        "the missing lockfile entry fails the build: {stderr}"
    );
    assert!(stderr.contains("MANI-103"), "{stderr}");
    assert!(!stderr.contains("note: the workspace names"), "{stderr}");
}
