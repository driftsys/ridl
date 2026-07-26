//! Integration tests for the baseline-aware desk check (docs/ROADMAP.md epic
//! E2.9, general form §6.3): `ridl baseline` writing one `.ir.json` snapshot per
//! package, and `ridl check` rendering an ordinal-affecting drift against that
//! baseline as a RIDL-407 warning that never moves the exit code.

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
            "ridl-baseline-{label}-{}-{}",
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

/// The published shape: three interactions at ordinals 1, 2, 3.
const BASE: &str = "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
type DoorState: integer [0..1]
interface VehicleStatus {
  signal currentSpeed: Speed @10ms
  event doorOpened: DoorState @[100ms..1s]
  event doorClosed: DoorState @[100ms..1s]
}
";

/// `doorClosed` moved ahead of `doorOpened` — the ordinals of both shift, the
/// general form §6.3 tidying that looks harmless in a diff.
const REORDERED: &str = "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
type DoorState: integer [0..1]
interface VehicleStatus {
  signal currentSpeed: Speed @10ms
  event doorClosed: DoorState @[100ms..1s]
  event doorOpened: DoorState @[100ms..1s]
}
";

/// A fourth interaction appended at the end — no surviving ordinal moves.
const APPENDED: &str = "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
type DoorState: integer [0..1]
interface VehicleStatus {
  signal currentSpeed: Speed @10ms
  event doorOpened: DoorState @[100ms..1s]
  event doorClosed: DoorState @[100ms..1s]
  event hoodOpened: DoorState @[100ms..1s]
}
";

/// `doorOpened` deleted without a `reserved` tombstone — every later ordinal
/// slides down.
const REMOVED: &str = "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
type DoorState: integer [0..1]
interface VehicleStatus {
  signal currentSpeed: Speed @10ms
  event doorClosed: DoorState @[100ms..1s]
}
";

/// A breaking change that moves no ordinal: the payload type narrows and the
/// staleness bound is raised. `ridl diff` gates on both; the desk check must
/// stay silent, because general form §6.3 asks it for ordinal drift only.
const NON_ORDINAL: &str = "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
type DoorState: integer [0..1]
type NarrowState: integer [0..0]
interface VehicleStatus {
  signal currentSpeed: Speed @10ms
  event doorOpened: NarrowState @[100ms..2s]
  event doorClosed: DoorState @[100ms..1s]
}
";

/// Lays out a one-package workspace holding `source` and returns its root.
fn package_workspace(dir: &TempDir, source: &str) -> PathBuf {
    dir.write("ridl.toml", MANIFEST);
    dir.write("cluster.ridl", source);
    dir.path().to_path_buf()
}

/// `ridl baseline` writes one `<pkg-name>.ir.json` per package into
/// `.ridl/baseline/` at the workspace root.
#[test]
fn baseline_writes_one_snapshot_per_package() {
    let dir = TempDir::new("write");
    let root = package_workspace(&dir, BASE);

    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(code, 0, "a clean workspace snapshots cleanly: {stderr}");
    assert!(
        root.join(".ridl/baseline/veh.cluster.ir.json").is_file(),
        "the package snapshot lands under `.ridl/baseline/`",
    );
}

/// One `.ir.json` holds exactly one package, so an N-package workspace needs N
/// files — the multi-package case is not a single merged snapshot.
#[test]
fn baseline_writes_one_file_per_package_in_a_multi_package_workspace() {
    let dir = TempDir::new("multi");
    dir.write(
        "ridl.toml",
        "[workspace]\nmembers = [\"common\", \"cluster\"]\n",
    );
    dir.write(
        "common/ridl.toml",
        "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n",
    );
    dir.write(
        "common/common.typl",
        "package veh.common\ntype Speed: km/h [0.0..250.0 step 0.5]\n",
    );
    dir.write("cluster/ridl.toml", MANIFEST);
    dir.write(
        "cluster/cluster.ridl",
        "package veh.cluster
type DoorState: integer [0..1]
interface VehicleStatus {
  event doorOpened: DoorState @[100ms..1s]
  event doorClosed: DoorState @[100ms..1s]
}
",
    );
    let root = dir.path();

    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(code, 0, "the workspace snapshots cleanly: {stderr}");
    let baseline = root.join(".ridl/baseline");
    let mut written: Vec<String> = std::fs::read_dir(&baseline)
        .expect("the baseline directory exists")
        .map(|entry| {
            entry
                .expect("a readable entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    written.sort();
    assert_eq!(
        written,
        vec![
            "veh.cluster.ir.json".to_string(),
            "veh.common.ir.json".to_string(),
        ],
        "two packages produce two snapshot files",
    );
}

/// `--out` redirects the snapshots away from the default directory.
#[test]
fn baseline_out_flag_selects_the_directory() {
    let dir = TempDir::new("out");
    let root = package_workspace(&dir, BASE);
    let out = dir.path().join("published");

    let (code, _, stderr) = ridl(&[
        "baseline".as_ref(),
        root.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    assert_eq!(code, 0, "the workspace snapshots cleanly: {stderr}");
    assert!(
        out.join("veh.cluster.ir.json").is_file(),
        "`--out` receives the snapshot",
    );
    assert!(
        !root.join(".ridl/baseline").exists(),
        "`--out` replaces the default directory rather than adding to it",
    );
}

/// A reorder against the baseline draws RIDL-407, names the moved interaction
/// in the diff path, and points the span at that interaction's declaration.
#[test]
fn check_flags_a_reorder_against_the_baseline() {
    let dir = TempDir::new("reorder");
    let root = package_workspace(&dir, BASE);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the baseline is written: {stderr}");

    dir.write("cluster.ridl", REORDERED);
    let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);

    assert!(
        stderr.contains("RIDL-407"),
        "the reorder draws the coded desk warning:\n{stderr}",
    );
    assert!(
        stderr.contains("`doorClosed` has moved in `VehicleStatus`"),
        "the message names the interaction and the shape it is declared in:\n{stderr}",
    );
    assert!(
        stderr.contains("wire identity") && stderr.contains("add new ones at the end"),
        "the message states the consequence and the remedy:\n{stderr}",
    );
    assert!(
        stderr.contains("event doorClosed: DoorState"),
        "the span underlines the interaction's declaration:\n{stderr}",
    );
    assert_eq!(
        code, 0,
        "a warning never moves the exit code of an otherwise clean check:\n{stderr}",
    );
}

/// A non-tombstoned removal is ordinal-affecting too, and is reported even
/// though the declaration it names is gone from the current source.
#[test]
fn check_flags_a_removal_against_the_baseline() {
    let dir = TempDir::new("removal");
    let root = package_workspace(&dir, BASE);
    ridl(&["baseline".as_ref(), root.as_os_str()]);

    dir.write("cluster.ridl", REMOVED);
    let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);

    assert!(
        stderr.contains("RIDL-407")
            && stderr.contains("`doorOpened` is gone in `VehicleStatus`")
            && stderr.contains("`reserved doorOpened`"),
        "the removal draws the desk warning, and it names the tombstone that \
         keeps the slot:\n{stderr}",
    );
    assert_eq!(code, 0, "the warning leaves the exit code alone:\n{stderr}");
}

/// An append shifts no surviving ordinal, so the desk check stays silent — it
/// is the general form §6.3 mitigation, not a second diff gate.
#[test]
fn check_is_silent_for_an_append() {
    let dir = TempDir::new("append");
    let root = package_workspace(&dir, BASE);
    ridl(&["baseline".as_ref(), root.as_os_str()]);

    dir.write("cluster.ridl", APPENDED);
    let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);

    assert!(
        !stderr.contains("RIDL-407"),
        "an appended interaction is not ordinal-affecting:\n{stderr}",
    );
    assert_eq!(code, 0, "a compatible change stays clean:\n{stderr}");
}

/// With no baseline anywhere, `ridl check` behaves exactly as before: no extra
/// output, same exit code.
#[test]
fn check_without_a_baseline_is_unchanged() {
    let dir = TempDir::new("nobaseline");
    let root = package_workspace(&dir, REORDERED);

    let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);

    assert_eq!(code, 0, "the workspace is clean:\n{stderr}");
    assert!(
        stderr.is_empty(),
        "a missing baseline is silently skipped:\n{stderr}",
    );
}

/// `--baseline` accepts a single `.ir.json` file, not only a directory.
#[test]
fn check_accepts_a_single_baseline_file() {
    let dir = TempDir::new("file");
    let root = package_workspace(&dir, BASE);
    let out = dir.path().join("published");
    ridl(&[
        "baseline".as_ref(),
        root.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    dir.write("cluster.ridl", REORDERED);
    let snapshot = out.join("veh.cluster.ir.json");
    let (code, _, stderr) = ridl(&[
        "check".as_ref(),
        root.as_os_str(),
        "--baseline".as_ref(),
        snapshot.as_os_str(),
    ]);

    assert!(
        stderr.contains("RIDL-407"),
        "a single-file baseline drives the same check:\n{stderr}",
    );
    assert_eq!(code, 0, "the exit code is untouched:\n{stderr}");
}

/// A `--baseline` path that does not exist is an input error, exit 2 — only
/// auto-discovery is silent.
#[test]
fn check_reports_a_missing_explicit_baseline() {
    let dir = TempDir::new("missing");
    let root = package_workspace(&dir, BASE);
    let absent = dir.path().join("no-such-dir");

    let (code, _, stderr) = ridl(&[
        "check".as_ref(),
        root.as_os_str(),
        "--baseline".as_ref(),
        absent.as_os_str(),
    ]);

    assert_eq!(code, 2, "a named baseline that is absent is an error");
    assert!(stderr.contains("baseline"), "the error says so:\n{stderr}");
}

/// A workspace that does not compile keeps its error exit and draws no desk
/// warning — the desk check runs only after a clean compile.
#[test]
fn check_skips_the_desk_check_when_the_compile_fails() {
    let dir = TempDir::new("broken");
    let root = package_workspace(&dir, BASE);
    ridl(&["baseline".as_ref(), root.as_os_str()]);

    dir.write(
        "cluster.ridl",
        "package veh.cluster
type DoorState: integer [0..1]
interface VehicleStatus {
  event doorClosed: Nope
}
",
    );
    let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);

    assert_eq!(code, 1, "the compile error still gates:\n{stderr}");
    assert!(
        !stderr.contains("RIDL-407"),
        "no desk warning over a broken compile:\n{stderr}",
    );
}

/// The desk check reports ordinal drift and nothing else. A payload narrowing
/// and a raised staleness bound are both breaking — `ridl diff` says so in the
/// same breath — but neither moves an ordinal, so the desk stays quiet and CI
/// keeps that job (general form §6.3).
#[test]
fn check_is_silent_on_a_non_ordinal_breaking_change() {
    let dir = TempDir::new("nonordinal");
    let root = package_workspace(&dir, BASE);
    let out = dir.path().join("published");
    ridl(&[
        "baseline".as_ref(),
        root.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    dir.write("cluster.ridl", NON_ORDINAL);

    // The control: the very same edit is breaking to `ridl diff`.
    let snapshot = out.join("veh.cluster.ir.json");
    let (diff_code, diff_stdout, _) =
        ridl(&["diff".as_ref(), snapshot.as_os_str(), root.as_os_str()]);
    assert_eq!(diff_code, 1, "the edit is breaking:\n{diff_stdout}");

    let (code, _, stderr) = ridl(&[
        "check".as_ref(),
        root.as_os_str(),
        "--baseline".as_ref(),
        out.as_os_str(),
    ]);

    assert!(
        !stderr.contains("RIDL-407"),
        "a breaking change that moves no ordinal is not the desk check's job:\n{stderr}",
    );
    assert_eq!(code, 0, "and it does not gate the check:\n{stderr}");
}

/// Snapshots are matched to packages by the package name *inside* the file, not
/// by the file's name — so a renamed or hand-managed snapshot still lines up
/// with the package it holds.
#[test]
fn check_matches_snapshots_by_package_name_not_file_name() {
    let dir = TempDir::new("byname");
    let root = package_workspace(&dir, BASE);
    let out = dir.path().join("published");
    ridl(&[
        "baseline".as_ref(),
        root.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    std::fs::rename(
        out.join("veh.cluster.ir.json"),
        out.join("zzz-totally-wrong-name.ir.json"),
    )
    .expect("rename the snapshot");

    dir.write("cluster.ridl", REORDERED);
    let (code, _, stderr) = ridl(&[
        "check".as_ref(),
        root.as_os_str(),
        "--baseline".as_ref(),
        out.as_os_str(),
    ]);

    assert!(
        stderr.contains("RIDL-407") && stderr.contains("`doorClosed` has moved"),
        "the misnamed snapshot is still attributed to its package:\n{stderr}",
    );
    assert_eq!(code, 0, "the exit code is untouched:\n{stderr}");
}

/// `ridl diff <baseline-dir> <workspace>` reads the directory as a snapshot
/// set. Compiling it as source instead would diff the current source against
/// itself and always report `identical` — a CI gate that fails open.
#[test]
fn diff_reads_a_baseline_directory_as_a_snapshot_set() {
    let dir = TempDir::new("diffdir");
    let root = package_workspace(&dir, BASE);
    ridl(&["baseline".as_ref(), root.as_os_str()]);
    let baseline = root.join(".ridl/baseline");

    dir.write("cluster.ridl", REORDERED);
    let (code, stdout, stderr) = ridl(&["diff".as_ref(), baseline.as_os_str(), root.as_os_str()]);

    assert_eq!(code, 1, "the reorder is breaking:\n{stdout}{stderr}");
    assert!(
        stdout.contains("interaction_reordered")
            && stdout.contains("veh.cluster/VehicleStatus/doorClosed"),
        "the report names the change:\n{stdout}",
    );
}

/// The same comparison over an unchanged workspace is clean.
#[test]
fn diff_of_an_unchanged_workspace_against_its_baseline_is_clean() {
    let dir = TempDir::new("diffsame");
    let root = package_workspace(&dir, BASE);
    ridl(&["baseline".as_ref(), root.as_os_str()]);
    let baseline = root.join(".ridl/baseline");

    let (code, stdout, stderr) = ridl(&["diff".as_ref(), baseline.as_os_str(), root.as_os_str()]);

    assert_eq!(code, 0, "nothing changed:\n{stdout}{stderr}");
    assert!(stdout.contains("identical"), "and it says so:\n{stdout}");
}

/// A directory holding no `.ir.json` is source, not a snapshot set: it still
/// takes the compile path, so source-tree comparison is unaffected.
#[test]
fn diff_of_a_directory_without_snapshots_compiles_it_as_source() {
    let old = TempDir::new("srcold");
    let old_root = package_workspace(&old, BASE);
    let new = TempDir::new("srcnew");
    let new_root = package_workspace(&new, REORDERED);

    let (code, stdout, stderr) =
        ridl(&["diff".as_ref(), old_root.as_os_str(), new_root.as_os_str()]);

    assert_eq!(code, 1, "two source trees still compare:\n{stdout}{stderr}");
    assert!(
        stdout.contains("interaction_reordered"),
        "and the reorder is found:\n{stdout}",
    );
}

/// Re-publishing a baseline regenerates it wholesale: a snapshot for a package
/// the workspace no longer declares is dropped rather than left to rot.
#[test]
fn baseline_drops_a_snapshot_whose_package_is_gone() {
    let dir = TempDir::new("rename");
    dir.write("ridl.toml", MANIFEST);
    dir.write("cluster.ridl", BASE);
    let root = dir.path().to_path_buf();
    ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert!(
        root.join(".ridl/baseline/veh.cluster.ir.json").is_file(),
        "the first baseline is published",
    );

    // Rename the package: the manifest and the source move together.
    dir.write(
        "ridl.toml",
        "[package]\nname = \"veh.dash\"\nversion = \"1.0.0\"\n",
    );
    dir.write("cluster.ridl", &BASE.replace("veh.cluster", "veh.dash"));
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the renamed workspace compiles:\n{stderr}");

    let baseline = root.join(".ridl/baseline");
    assert!(
        baseline.join("veh.dash.ir.json").is_file(),
        "the new package is published",
    );
    assert!(
        !baseline.join("veh.cluster.ir.json").exists(),
        "the snapshot under the old package name is gone",
    );
}

/// A workspace that does not compile leaves the published baseline untouched —
/// re-publishing must never destroy a good baseline.
#[test]
fn baseline_keeps_the_published_snapshots_when_the_compile_fails() {
    let dir = TempDir::new("keep");
    let root = package_workspace(&dir, BASE);
    ridl(&["baseline".as_ref(), root.as_os_str()]);
    let snapshot = root.join(".ridl/baseline/veh.cluster.ir.json");
    let published = std::fs::read_to_string(&snapshot).expect("read the published snapshot");

    dir.write(
        "cluster.ridl",
        "package veh.cluster
interface VehicleStatus {
  event doorClosed: Nope @[100ms..1s]
}
",
    );
    let (code, _, _) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(code, 1, "the broken workspace fails to publish");
    assert_eq!(
        std::fs::read_to_string(&snapshot).expect("the snapshot survives"),
        published,
        "the published baseline is exactly as it was",
    );
}

/// The rendered block of the RIDL-407 diagnostic about the interaction called
/// `name`: its first line plus every following line up to the next diagnostic.
/// The renderer draws a source snippet under a diagnostic that carries a span
/// and nothing under one that does not, so "does this diagnostic have a span?"
/// is answered by looking for the snippet gutter inside the block.
///
/// The message opens with the interaction's own name in backticks — it names
/// what the author wrote rather than the slash-separated diff path it used to
/// print — so that is what this looks for.
fn ridl_407_block<'a>(stderr: &'a str, name: &str) -> &'a str {
    let needle = format!("warning[RIDL-407]: `{name}` ");
    let start = stderr
        .find(&needle)
        .unwrap_or_else(|| panic!("no RIDL-407 for `{name}` in:\n{stderr}"));
    // `needle` opens the diagnostic's first line, so the scan for the next
    // diagnostic starts past it — searching from `start` would find this one.
    let after = start + needle.len();
    match stderr[after..].find("warning[RIDL-407]") {
        Some(next) => &stderr[start..after + next],
        None => &stderr[start..],
    }
}

/// The committed baseline corpus member (E2 task 22). Every other test in this
/// file builds its baseline in a temp directory from a source string, so none
/// of them exercises a baseline that was written by an earlier run and read
/// back later — which is the only way a real baseline is ever used.
/// `tests/baseline-corpus/` is that case: a package plus a committed
/// `.ridl/baseline/corpus.baseline.ir.json`, read from disk exactly as a
/// published baseline is.
///
/// The package holds both stores of interactions — a named `interface` and a
/// service's inline shape — and the source has drifted from the snapshot in
/// three ways that all look like tidying: the two events are alphabetised,
/// `setGear` is retired without a tombstone, and `setEcoMode` is retired from
/// the inline shape without a tombstone. Every one is reported and the exit
/// code stays 0 — the desk check informs, `ridl diff` gates.
///
/// The inline-shape case is pinned separately because its span falls on a
/// different construct — the service's dotted name rather than an interface
/// name: see [`inline_shape_removal_spans_the_service_name`].
#[test]
fn check_reports_ordinal_drift_against_the_committed_baseline() {
    let entry = Path::new("tests/baseline-corpus");
    assert!(
        entry
            .join(".ridl/baseline/corpus.baseline.ir.json")
            .is_file(),
        "the committed baseline snapshot is the point of this fixture",
    );

    let (code, _, stderr) = ridl(&["check".as_ref(), entry.as_os_str()]);

    assert_eq!(code, 0, "a desk-check warning never moves the exit code");

    // Pinned per diagnostic: the code, the severity word, the interaction the
    // message names, the remedy that identifies *which* drift it is, and the
    // source line the span points at.
    //
    // The remedy is what distinguishes the categories now. Pinning the category
    // word alone — `interaction_removed`, `interaction_reordered` — was pinning
    // the enum variant's spelling, which is what a reader of the diff report
    // sees and not what a reader of this warning needs; and the two categories'
    // messages could have been swapped without any assertion noticing, because
    // the rest of the line was identical between them.
    for (name, remedy, line) in [
        (
            "setGear",
            "retire it in place with `reserved setGear`",
            "interface VehicleStatus {",
        ),
        (
            "tyrePressure",
            "declare it at the end of the body instead",
            "event tyrePressure : DoorState @[100ms..1s]",
        ),
        (
            "legacyWheelPhase",
            "give this interaction a different name",
            "signal legacyWheelPhase : Speed @10ms",
        ),
        (
            "doorOpened",
            "put the declarations back in the baseline's order",
            "event doorOpened : DoorState @[100ms..1s]",
        ),
        (
            "doorClosed",
            "put the declarations back in the baseline's order",
            "event doorClosed : DoorState @[100ms..1s]",
        ),
    ] {
        let block = ridl_407_block(&stderr, name);
        assert!(
            block.starts_with(&format!("warning[RIDL-407]: `{name}` ")),
            "the message opens by naming `{name}` in:\n{stderr}"
        );
        assert!(
            block.contains(remedy),
            "the message for `{name}` must carry its own remedy — `{remedy}` — in:\n{stderr}"
        );
        assert!(
            block.contains("ridl §11"),
            "the message for `{name}` must cite the rule it enforces in:\n{stderr}"
        );
        assert!(
            block.contains(line),
            "the span for `{name}` must point at `{line}` in:\n{stderr}"
        );
    }

    // A reorder states where the interaction was and where it is now, so the
    // reader can count the declarations rather than diff the file by eye.
    assert!(
        ridl_407_block(&stderr, "doorOpened").contains("(position 2 there, position 4 here)"),
        "a reorder names both positions in:\n{stderr}"
    );
    // …and drops the parenthetical when the two coincide. A reorder is detected
    // on relative order, so `doorClosed` changed rank while keeping ordinal 3 —
    // the insertion above it shifted the others past it. "has moved (position 3
    // there, position 3 here)" contradicts itself, so the numbers are omitted.
    assert!(
        !ridl_407_block(&stderr, "doorClosed").contains("position"),
        "a reorder whose absolute ordinal is unchanged must not print it:\n{stderr}"
    );

    // No message answers in the vocabulary of the IR or of the diff report.
    // Scoped to the message lines: the source snippet under each one carries a
    // file path, and a path holds a `/` legitimately.
    let message_lines: Vec<&str> = stderr
        .lines()
        .filter(|line| line.starts_with("warning[RIDL-407]:"))
        .collect();
    assert_eq!(message_lines.len(), 6, "one line per diagnostic:\n{stderr}");
    for line in &message_lines {
        for internal in [
            "ordinal",
            "interaction_reordered",
            "interaction_removed",
            "interaction_inserted",
            "reserved_name_redeclared",
        ] {
            assert!(
                !line.contains(internal),
                "RIDL-407 must not answer in `{internal}` — that is IR or diff-report \
                 vocabulary, not the reader's:\n{line}"
            );
        }
        // A diff path, in general and not just this fixture's: the message
        // names the interaction and the shape, never `pkg/Shape/member`. No
        // RIDL-407 message has a legitimate `/` in it, so the character is the
        // check.
        assert!(
            !line.contains('/'),
            "RIDL-407 must not print a diff path — the reader wrote no `/`:\n{line}"
        );
    }

    // The six above are the whole report: a seventh RIDL-407 would mean the desk
    // check flagged an interaction whose ordinal did not move.
    assert_eq!(
        stderr.matches("RIDL-407").count(),
        6,
        "exactly six ordinal-affecting changes, no more:\n{stderr}"
    );
}

/// **This test pinned a defect until the `shapes()` refactor closed it.**
///
/// An interaction removed from a service's *inline* shape used to be reported
/// by the desk check with no span at all — no file, no line, no source snippet
/// — where the same removal from a named interface fell back to the
/// interface's own name span. `DeclIndex` recorded inline-shape *members*
/// (walking `service.inline_members()`) but populated the shape-level fallback
/// map — the one used when the named interaction no longer exists in source —
/// from `source.interfaces()` only. An inline shape is an `Interface` stored
/// under `Service.shape`, outside `Package.interfaces`, so the fallback found
/// nothing and the diagnostic was emitted detached. That was the sixth
/// instance of the inline-shape blind spot the E2 corpus set out to look for
/// (`crates/ridlc/tests/corpus/veh-cluster/NOTES`).
///
/// `DeclIndex::build` now walks `SourceFile::shapes()`, which yields an
/// `interface` declaration and a service's inline shape alike, so the fallback
/// covers both. The span for an inline shape lands on the service's dotted
/// name, which is the identity its diff paths carry.
#[test]
fn inline_shape_removal_spans_the_service_name() {
    let (_, _, stderr) = ridl(&["check".as_ref(), "tests/baseline-corpus".as_ref()]);

    let inline = ridl_407_block(&stderr, "setEcoMode");
    assert!(
        inline.starts_with(
            "warning[RIDL-407]: `setEcoMode` is gone in `corpus.baseline.hvac` but the \
             published baseline still declares it."
        ),
        "the inline-shape removal is reported, naming the service it left:\n{stderr}"
    );
    assert!(
        inline.contains("┌─"),
        "the inline-shape removal carries a span — this is the sixth-instance \
         regression, back:\n{inline}"
    );
    assert!(
        inline.contains("service corpus.baseline.hvac {"),
        "the fallback span points at the service's own declaration:\n{inline}"
    );
    // The span starts on the dotted name, not on the `service` keyword and not
    // at the head of the declaration: the name is the identity the diff path
    // carries. The COLUMN is what pins that — a caret-run assertion would
    // still pass on a span widened leftwards to the keyword, because the
    // widened run is longer and `contains` matches any prefix of it.
    assert!(
        inline.contains("cluster.ridl:46:9"),
        "the span starts at the dotted name, column 9 — not column 1, where \
         the `service` keyword is:\n{inline}"
    );
    // A caret run of at least 20, which with the column above pins the left
    // edge exactly and the width as a floor. The exact right edge is
    // `shape_identity_range_covers_the_declared_name`'s job — a two-character
    // rightward widening still satisfies a `contains` on 20 carets, and that
    // unit test does fail on it.
    assert!(
        inline.contains("^^^^^^^^^^^^^^^^^^^^"),
        "the underline covers `corpus.baseline.hvac`, the 20-character dotted \
         name:\n{inline}"
    );

    // The control: the same change to a named interface spans the interface
    // name, so the two forms differ in which construct is underlined and in
    // nothing else.
    let named = ridl_407_block(&stderr, "setGear");
    assert!(
        named.contains("┌─") && named.contains("interface VehicleStatus {"),
        "a removal from a named interface still spans its interface name:\n{named}"
    );
}
