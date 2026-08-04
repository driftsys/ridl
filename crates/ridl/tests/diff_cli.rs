//! Integration tests for `ridl diff` (docs/ROADMAP.md epic E2.8a): the exit
//! contract (0 compatible/identical, 1 breaking, 2 error), source and
//! `.ir.json` inputs, in-process compilation via `ridlc::compile_workspace`,
//! and the stable machine-readable JSON schema.

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
            "ridl-diff-{label}-{}-{}",
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

const BASE: &str = "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
type DoorState: integer [0..1]
interface VehicleStatus {
  signal currentSpeed: Speed @10ms
  event doorOpened: DoorState
}
";

/// A breaking change: the signal payload type changes (Speed -> Speed2).
const BREAKING: &str = "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
type Speed2: km/h [0.0..300.0 step 0.5]
type DoorState: integer [0..1]
interface VehicleStatus {
  signal currentSpeed: Speed2 @10ms
  event doorOpened: DoorState
}
";

/// A compatible change: a new interaction is appended at the end.
const COMPATIBLE: &str = "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
type DoorState: integer [0..1]
interface VehicleStatus {
  signal currentSpeed: Speed @10ms
  event doorOpened: DoorState
  event hoodOpened: DoorState
}
";

/// A source that fails to compile: an unknown payload type.
const BROKEN: &str = "package veh.cluster
interface VehicleStatus {
  signal currentSpeed: NoSuchType @10ms
}
";

/// `ridl diff <file> <file>` over two identical single-file sources exits 0.
#[test]
fn source_files_identical_exits_zero() {
    let dir = TempDir::new("same");
    let old = dir.write("old.ridl", BASE);
    let new = dir.write("new.ridl", BASE);
    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), new.as_os_str()]);
    assert_eq!(code, 0, "identical sources exit 0, stderr:\n{stderr}");
    // `render_text` terminates every line itself, so the text output must not
    // pick up a trailing blank line.
    assert_eq!(
        stdout, "identical\n",
        "the text rendering must not gain a blank line, got:\n{stdout:?}"
    );
}

/// `ridl diff` refuses the two non-JSON IR encodings by name — diffs and
/// baselines stay `.ir.json` (ADR-0014 decision 5). Before the refusal the
/// file fell through to the source compiler, which parsed the artifact as
/// `.typl` and reported FORM-104 into it — a misdiagnosis of the actual
/// mistake.
#[test]
fn non_json_ir_snapshots_are_refused() {
    let dir = TempDir::new("refuse");
    let source = dir.write("new.ridl", BASE);
    for name in ["old.ir.txtpb", "old.ir.binpb"] {
        let artifact = dir.write(name, "name: \"veh.cluster\"\n");
        let (code, _, stderr) = ridl(&["diff".as_ref(), artifact.as_os_str(), source.as_os_str()]);
        assert_eq!(code, 2, "`{name}` is an input error, stderr:\n{stderr}");
        assert!(
            stderr.contains(".ir.json"),
            "the refusal must name the accepted encoding:\n{stderr}"
        );
        assert!(
            !stderr.contains("FORM-104"),
            "the artifact must not be parsed as source:\n{stderr}"
        );
    }
}

/// Retiring an interaction but writing its `reserved` tombstone at the end of
/// the body frees the retired ordinal, so a surviving interaction slides down
/// into it (ridl §11). Driven through real source so the compiler assigns the
/// ordinals: a retirement is compatible, but this one shifts a wire identity
/// and must gate.
#[test]
fn a_tombstone_written_out_of_its_slot_exits_one() {
    const HEADER: &str = "package veh.cluster\ntype T: integer [0..10]\n";
    let dir = TempDir::new("tombstone");
    // a=1, b=2, c=3
    let old = dir.write(
        "old.ridl",
        &format!(
            "{HEADER}interface I {{\n  signal a: T @10ms\n  signal b: T @10ms\n  signal c: T @10ms\n}}\n"
        ),
    );
    // a=1, c=2 (slid down from 3), reserved b=3
    let new = dir.write(
        "new.ridl",
        &format!(
            "{HEADER}interface I {{\n  signal a: T @10ms\n  signal c: T @10ms\n  reserved b\n}}\n"
        ),
    );

    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), new.as_os_str()]);
    assert_eq!(
        code, 1,
        "an out-of-slot tombstone frees a wire slot, stdout:\n{stdout}stderr:\n{stderr}"
    );
    assert!(stdout.starts_with("breaking"), "stdout:\n{stdout}");
}

/// Control for the case above: the same retirement written in the retired
/// interaction's own slot keeps every later ordinal and stays compatible.
#[test]
fn a_tombstone_written_in_its_slot_exits_zero() {
    const HEADER: &str = "package veh.cluster\ntype T: integer [0..10]\n";
    let dir = TempDir::new("tombstone-ok");
    let old = dir.write(
        "old.ridl",
        &format!(
            "{HEADER}interface I {{\n  signal a: T @10ms\n  signal b: T @10ms\n  signal c: T @10ms\n}}\n"
        ),
    );
    // reserved b holds ordinal 2, so c keeps ordinal 3.
    let new = dir.write(
        "new.ridl",
        &format!(
            "{HEADER}interface I {{\n  signal a: T @10ms\n  reserved b\n  signal c: T @10ms\n}}\n"
        ),
    );

    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), new.as_os_str()]);
    assert_eq!(
        code, 0,
        "an in-slot retirement is compatible, stdout:\n{stdout}stderr:\n{stderr}"
    );
    assert!(stdout.contains("interaction_retired"), "stdout:\n{stdout}");
}

/// `ridl diff <dir> <dir>` over two package directories: a breaking change
/// exits 1 (in-process compilation via `ridlc::compile_workspace`).
#[test]
fn source_dir_vs_source_dir_breaking_exits_one() {
    let old = TempDir::new("dir-old");
    old.write("ridl.toml", MANIFEST);
    old.write("iface.ridl", BASE);
    let new = TempDir::new("dir-new");
    new.write("ridl.toml", MANIFEST);
    new.write("iface.ridl", BREAKING);

    let (code, stdout, stderr) = ridl(&[
        "diff".as_ref(),
        old.path().as_os_str(),
        new.path().as_os_str(),
    ]);
    assert_eq!(code, 1, "a breaking change exits 1, stderr:\n{stderr}");
    assert!(stdout.starts_with("breaking"), "stdout:\n{stdout}");
    assert!(
        stdout.contains("payload_changed veh.cluster/VehicleStatus/currentSpeed"),
        "the honest path must surface, stdout:\n{stdout}"
    );
}

/// A compatible source change (an appended interaction) exits 0.
#[test]
fn source_dir_vs_source_dir_compatible_exits_zero() {
    let old = TempDir::new("dir-old-ok");
    old.write("ridl.toml", MANIFEST);
    old.write("iface.ridl", BASE);
    let new = TempDir::new("dir-new-ok");
    new.write("ridl.toml", MANIFEST);
    new.write("iface.ridl", COMPATIBLE);

    let (code, stdout, stderr) = ridl(&[
        "diff".as_ref(),
        old.path().as_os_str(),
        new.path().as_os_str(),
    ]);
    assert_eq!(code, 0, "a compatible change exits 0, stderr:\n{stderr}");
    assert!(stdout.starts_with("compatible"), "stdout:\n{stdout}");
    assert!(
        stdout.contains("interaction_appended veh.cluster/VehicleStatus/hoodOpened"),
        "stdout:\n{stdout}"
    );
}

/// `ridl diff` over two `.ir.json` snapshots produced by `ridl build`: a
/// breaking change exits 1.
#[test]
fn ir_json_vs_ir_json_breaking_exits_one() {
    let dir = TempDir::new("irjson");
    let old_src = dir.write("base.ridl", BASE);
    let new_src = dir.write("candidate.ridl", BREAKING);
    let out_old = TempDir::new("irjson-out-old");
    let out_new = TempDir::new("irjson-out-new");

    let (build_old, _, err_old) = ridl(&[
        "build".as_ref(),
        old_src.as_os_str(),
        "--emit".as_ref(),
        "ir-json".as_ref(),
        "--out-dir".as_ref(),
        out_old.path().as_os_str(),
    ]);
    assert_eq!(build_old, 0, "baseline builds, stderr:\n{err_old}");
    let (build_new, _, err_new) = ridl(&[
        "build".as_ref(),
        new_src.as_os_str(),
        "--emit".as_ref(),
        "ir-json".as_ref(),
        "--out-dir".as_ref(),
        out_new.path().as_os_str(),
    ]);
    assert_eq!(build_new, 0, "candidate builds, stderr:\n{err_new}");

    let old_ir = out_old.path().join("base.ir.json");
    let new_ir = out_new.path().join("candidate.ir.json");
    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old_ir.as_os_str(), new_ir.as_os_str()]);
    assert_eq!(code, 1, "the snapshot diff is breaking, stderr:\n{stderr}");
    assert!(stdout.starts_with("breaking"), "stdout:\n{stdout}");
}

/// A source that fails to compile on either side yields exit 2 with the
/// compiler diagnostics rendered to stderr — `ridlc` stays untouched, but its
/// errors gate the diff.
#[test]
fn a_broken_source_exits_two() {
    let dir = TempDir::new("broken");
    let old = dir.write("good.ridl", BASE);
    let new = dir.write("broken.ridl", BROKEN);
    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), new.as_os_str()]);
    assert_eq!(code, 2, "a compile error exits 2, stdout:\n{stdout}");
    assert!(
        stderr.contains("NoSuchType"),
        "the compiler diagnostic must render to stderr, got:\n{stderr}"
    );
    assert!(
        stdout.is_empty(),
        "no diff report is written when a side fails to compile, stdout:\n{stdout}"
    );
}

/// A missing input path is a usage error: exit 2.
#[test]
fn a_missing_input_exits_two() {
    let dir = TempDir::new("missing");
    let old = dir.write("old.ridl", BASE);
    let missing = dir.path().join("does-not-exist.ridl");
    let (code, _, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), missing.as_os_str()]);
    assert_eq!(code, 2, "a missing input exits 2, stderr:\n{stderr}");
}

/// `--format json` prints the stable schema and still keys the exit code on
/// the verdict.
#[test]
fn format_json_matches_the_schema_and_exit_code() {
    let dir = TempDir::new("json");
    let old = dir.write("old.ridl", BASE);
    let new = dir.write("new.ridl", BREAKING);
    let (code, stdout, stderr) = ridl(&[
        "diff".as_ref(),
        old.as_os_str(),
        new.as_os_str(),
        "--format".as_ref(),
        "json".as_ref(),
    ]);
    assert_eq!(code, 1, "a breaking change exits 1, stderr:\n{stderr}");

    let value: serde_json::Value =
        serde_json::from_str(&stdout).expect("--format json emits valid JSON");
    assert_eq!(value["verdict"], "breaking");
    let changes = value["changes"].as_array().expect("changes is an array");
    assert!(!changes.is_empty(), "there is at least one change");
    for change in changes {
        assert!(change.get("path").is_some(), "every change has a path");
        assert!(
            change.get("category").is_some(),
            "every change has a category"
        );
        assert!(
            change.get("verdict").is_some(),
            "every change has a verdict"
        );
        // before/after are always present (null when not applicable).
        assert!(change.as_object().unwrap().contains_key("before"));
        assert!(change.as_object().unwrap().contains_key("after"));
    }
    assert!(
        changes.iter().any(
            |change| change["category"] == "payload_changed" && change["verdict"] == "breaking"
        ),
        "the payload change is present and breaking, stdout:\n{stdout}"
    );
}
