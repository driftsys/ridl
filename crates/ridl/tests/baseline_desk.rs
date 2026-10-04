//! Integration tests for the baseline-aware desk check (general form §6.3): `ridl baseline` writing one `.ir.json` snapshot per
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
    ridl_in(None, args)
}

/// Runs `ridl` with `args` from `current_dir` when one is given.
fn ridl_in(current_dir: Option<&Path>, args: &[&std::ffi::OsStr]) -> (i32, String, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ridl"));
    command.args(args);
    if let Some(dir) = current_dir {
        command.current_dir(dir);
    }
    let output = command.output().expect("the ridl binary must run");
    let code = output.status.code().expect("the process exits with a code");
    (
        code,
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The fixtures leave their items undocumented, so the manifest sets
/// `missing-docs` (TYPL-406) to `allow`: a clean check stays silent.
const MANIFEST: &str = "[package]\nname = \"veh.cluster\"\nversion = \"1.0.0\"\n\n\
                        [lints]\nmissing-docs = \"allow\"\n";

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

/// A named-form service composing two interfaces at slots 1 and 2 (ridl
/// §14.5, ADR-0015 decision 15) — the container the shape-list drift
/// fixtures below edit.
const SVC_BASE: &str = "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
interface DoorBlock {
  signal locked: Speed @10ms
}
interface HealthBlock {
  signal uptime: Speed @10ms
}
service veh.cluster.doors : DoorBlock, HealthBlock
";

/// The two shapes swap places — the shape-list reading of the §6.3 tidying:
/// both interface ids move.
const SVC_REORDERED: &str = "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
interface DoorBlock {
  signal locked: Speed @10ms
}
interface HealthBlock {
  signal uptime: Speed @10ms
}
service veh.cluster.doors : HealthBlock, DoorBlock
";

/// A shape inserted ahead of the two the baseline numbers.
const SVC_INSERTED: &str = "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
interface DoorBlock {
  signal locked: Speed @10ms
}
interface HealthBlock {
  signal uptime: Speed @10ms
}
interface NewBlock {
  signal fresh: Speed @10ms
}
service veh.cluster.doors : NewBlock, DoorBlock, HealthBlock
";

/// `HealthBlock` dropped from the list without a tombstone. The interface
/// declaration itself stays, so the removal is purely a shape-list edit.
const SVC_REMOVED: &str = "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
interface DoorBlock {
  signal locked: Speed @10ms
}
interface HealthBlock {
  signal uptime: Speed @10ms
}
service veh.cluster.doors : DoorBlock
";

/// A shape whose field names a type from the standard package. Every other
/// fixture in this file names only its own declarations, so this is the only
/// one whose compile reaches `ridl.std`.
const NAMES_A_STANDARD_TYPE: &str = "package veh.cluster
type DoorState: integer [0..1]
struct DoorReport {
  observedAt: Timestamp
  door: DoorState
}
interface VehicleStatus {
  event doorOpened: DoorState @[100ms..1s]
}
";

/// Lays out a one-package workspace holding `source`, records its interface
/// numbers with plain `ridl lock` — `ridl baseline` refuses a provisional
/// number (RIDL-411), so a fixture that publishes needs its lock first — and
/// returns its root.
fn package_workspace(dir: &TempDir, source: &str) -> PathBuf {
    dir.write("ridl.toml", MANIFEST);
    dir.write("cluster.ridl", source);
    let root = dir.path().to_path_buf();
    lock(&root);
    root
}

/// Records the interface numbers of the workspace at `root` with plain
/// `ridl lock`.
fn lock(root: &Path) {
    let (code, _, stderr) = ridl(&["lock".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the fixture's lock is allocated: {stderr}");
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
    lock(root);

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

/// A service's list is a set (ADR-0015 decision 19 as amended on
/// 2026-09-15): its order is not an identity, so a reorder, an insertion and
/// a removal in the list move nothing on the wire and draw no RIDL-407 — a
/// service's list carries no ordinal for the desk check to report at all.
#[test]
fn check_is_silent_for_a_service_set_change() {
    for (label, source) in [
        ("svcreorder", SVC_REORDERED),
        ("svcinsert", SVC_INSERTED),
        ("svcremoval", SVC_REMOVED),
    ] {
        let dir = TempDir::new(label);
        let root = package_workspace(&dir, SVC_BASE);
        let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
        assert_eq!(code, 0, "the baseline is written: {stderr}");

        dir.write("cluster.ridl", source);
        let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);
        assert!(
            !stderr.contains("RIDL-407"),
            "{label}: a set change draws no desk warning:\n{stderr}"
        );
        assert_eq!(code, 0, "{label}: exit code:\n{stderr}");
    }
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

/// The published composites: a struct and a union whose members take their
/// wire identity from their ordinal (typl §7.4), and an enum and an enum set
/// whose members carry an explicit number instead (typl §8, §9).
const COMPOSITES: &str = "package veh.cluster
type DoorState: integer [0..1]
type LatchState: integer [0..3]
struct Report {
  door: DoorState
  latch: LatchState
}
union Reading {
  door: DoorState
  latch: LatchState
}
enum Gear {
  PARK = 0
  DRIVE = 1
}
enumset Warnings {
  LOW_FUEL = 0
  DOOR_OPEN = 1
}
";

/// `COMPOSITES` with the two fields of `Report` swapped.
const STRUCT_FIELDS_SWAPPED: &str = "package veh.cluster
type DoorState: integer [0..1]
type LatchState: integer [0..3]
struct Report {
  latch: LatchState
  door: DoorState
}
union Reading {
  door: DoorState
  latch: LatchState
}
enum Gear {
  PARK = 0
  DRIVE = 1
}
enumset Warnings {
  LOW_FUEL = 0
  DOOR_OPEN = 1
}
";

/// `COMPOSITES` with the two arms of `Reading` swapped.
const UNION_ARMS_SWAPPED: &str = "package veh.cluster
type DoorState: integer [0..1]
type LatchState: integer [0..3]
struct Report {
  door: DoorState
  latch: LatchState
}
union Reading {
  latch: LatchState
  door: DoorState
}
enum Gear {
  PARK = 0
  DRIVE = 1
}
enumset Warnings {
  LOW_FUEL = 0
  DOOR_OPEN = 1
}
";

/// `COMPOSITES` with the values of `Gear` and the bits of `Warnings` swapped,
/// each keeping its explicit number.
const ENUM_MEMBERS_SWAPPED: &str = "package veh.cluster
type DoorState: integer [0..1]
type LatchState: integer [0..3]
struct Report {
  door: DoorState
  latch: LatchState
}
union Reading {
  door: DoorState
  latch: LatchState
}
enum Gear {
  DRIVE = 1
  PARK = 0
}
enumset Warnings {
  DOOR_OPEN = 1
  LOW_FUEL = 0
}
";

/// Asserts the RIDL-407 block about `member` of `container`: it names both,
/// states the two ordinals in the typl §7.4 word, cites that rule, names the
/// remedy, and points its span at `location` — `declaration`'s text alone
/// does not pin the span, because `Report` and `Reading` in `COMPOSITES`
/// declare the same two members with the same text, so a span on the wrong
/// container's copy would still pass a text-only check.
fn assert_member_moved(
    stderr: &str,
    member: &str,
    container: &str,
    was: u32,
    now: u32,
    declaration: &str,
    location: &str,
) {
    let block = ridl_407_block(stderr, member);
    assert!(
        block.contains(&format!("`{member}` has moved in `{container}`")),
        "the message names the member and the body it is declared in:\n{stderr}",
    );
    assert!(
        block.contains(&format!("(ordinal {was} there, ordinal {now} here)")),
        "a struct field or union arm is placed by its ordinal, the word typl §7.4 \
         uses for it:\n{stderr}",
    );
    assert!(
        !block.contains("position"),
        "an ordinal is not worded as a position:\n{stderr}",
    );
    assert!(
        !block.contains("interaction"),
        "a composite member is not an interaction:\n{stderr}",
    );
    assert!(
        block.contains("typl §7.4"),
        "the message cites the rule that makes the order a wire identity:\n{stderr}",
    );
    assert!(
        block.contains("add new ones at the end"),
        "the message names the remedy that keeps the baseline intact:\n{stderr}",
    );
    assert!(
        !block.contains("reserved"),
        "a reorder never comes with a removal, so the message offers no \
         tombstone:\n{stderr}",
    );
    // `location` pins the span to {container}'s own copy of the declaration,
    // not the other composite's identical member text.
    assert_underlines(block, declaration, location, stderr);
}

/// A struct field swap moves two ordinals, which are wire identity (typl
/// §7.4). `ridl diff` reports both as `member_reordered`, and the desk check
/// warns once for each, without moving the exit code (driftsys/ridl#335).
#[test]
fn check_flags_a_struct_field_reorder_against_the_baseline() {
    let dir = TempDir::new("struct-reorder");
    let root = package_workspace(&dir, COMPOSITES);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the baseline is written: {stderr}");

    dir.write("cluster.ridl", STRUCT_FIELDS_SWAPPED);
    let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);

    assert_eq!(
        stderr.matches("warning[RIDL-407]").count(),
        2,
        "one warning per moved field:\n{stderr}",
    );
    assert_member_moved(
        &stderr,
        "door",
        "Report",
        1,
        2,
        "door: DoorState",
        "cluster.ridl:6:3",
    );
    assert_member_moved(
        &stderr,
        "latch",
        "Report",
        2,
        1,
        "latch: LatchState",
        "cluster.ridl:5:3",
    );
    assert_eq!(
        code, 0,
        "a warning never moves the exit code of an otherwise clean check:\n{stderr}",
    );
}

/// A union arm swap is the same wire break as a struct field swap (typl §7.4)
/// and draws the same warning, once per moved arm.
#[test]
fn check_flags_a_union_arm_reorder_against_the_baseline() {
    let dir = TempDir::new("union-reorder");
    let root = package_workspace(&dir, COMPOSITES);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the baseline is written: {stderr}");

    dir.write("cluster.ridl", UNION_ARMS_SWAPPED);
    let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);

    assert_eq!(
        stderr.matches("warning[RIDL-407]").count(),
        2,
        "one warning per moved arm:\n{stderr}",
    );
    assert_member_moved(
        &stderr,
        "door",
        "Reading",
        1,
        2,
        "door: DoorState",
        "cluster.ridl:10:3",
    );
    assert_member_moved(
        &stderr,
        "latch",
        "Reading",
        2,
        1,
        "latch: LatchState",
        "cluster.ridl:9:3",
    );
    assert_eq!(code, 0, "the warning leaves the exit code alone:\n{stderr}");
}

/// Runs `ridl diff` from the published baseline to the workspace, returning
/// its exit code and report. A test that asserts what a struct field's or
/// union arm's change draws reads this first: the desk warning and the gate
/// must never disagree about whether such a change is a wire break.
fn diff_against_baseline(root: &Path) -> (i32, String) {
    let (code, stdout, _) = ridl(&[
        "diff".as_ref(),
        root.join(".ridl/baseline").as_os_str(),
        root.as_os_str(),
    ]);
    (code, stdout)
}

/// Asserts that a RIDL-407 block's span underlines exactly `declaration`, at
/// `location`: the snippet line carries the declaration's text, the location
/// line carries `file:line:column`, and the underline holds one caret per
/// byte of the declaration and no more. This pins the width a reader sees.
/// It does not exercise the trailing-whitespace trim in `declaration_range`:
/// the parser's range for a field, an arm or a `reserved` entry in these
/// fixtures already ends at the declaration's last byte.
fn assert_underlines(block: &str, declaration: &str, location: &str, stderr: &str) {
    assert!(
        block.contains(declaration),
        "the span underlines `{declaration}`:\n{stderr}",
    );
    assert!(
        block.contains(location),
        "the span is at {location}:\n{stderr}",
    );
    let underline = block
        .lines()
        .find(|line| line.contains('^'))
        .unwrap_or_else(|| panic!("no underline in the block:\n{stderr}"));
    assert_eq!(
        underline.chars().filter(|c| *c == '^').count(),
        declaration.len(),
        "the underline is exactly as wide as `{declaration}`:\n{stderr}",
    );
}

/// A field or arm added above an existing one shifts the ordinals after it,
/// a wire break under typl §7.4. `ridl diff` reports the addition alone, as
/// `decl_added`, and classifies it breaking; the desk check reads that
/// verdict and warns once, for the added member, without moving the exit
/// code. The third case inserts between two members, so the ordinal the
/// message states is the inserted member's own and not the body's first.
#[test]
fn check_flags_a_struct_field_or_union_arm_insertion_as_the_gate_does() {
    for (label, container, edit, ordinal, holder, moved, location) in [
        (
            "struct-insert",
            "Report",
            "struct Report {\n",
            1,
            "door",
            "`door` and `latch` also changed ordinal",
            "cluster.ridl:5:3",
        ),
        (
            "union-insert",
            "Reading",
            "union Reading {\n",
            1,
            "door",
            "`door` and `latch` also changed ordinal",
            "cluster.ridl:9:3",
        ),
        (
            "struct-insert-middle",
            "Report",
            "struct Report {\n  door: DoorState\n",
            2,
            "latch",
            "`latch` also changed ordinal",
            "cluster.ridl:6:3",
        ),
    ] {
        let dir = TempDir::new(label);
        let root = package_workspace(&dir, COMPOSITES);
        let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
        assert_eq!(code, 0, "{label}: the baseline is written: {stderr}");

        dir.write(
            "cluster.ridl",
            &COMPOSITES.replacen(edit, &format!("{edit}  hinge: DoorState\n"), 1),
        );
        let (code, diff) = diff_against_baseline(&root);
        assert_eq!(
            code, 1,
            "{label}: `ridl diff` gates on the insertion:\n{diff}"
        );
        assert!(
            diff.contains(&format!(
                "[breaking] decl_added veh.cluster/{container}/hinge"
            )) && !diff.contains("member_reordered"),
            "{label}: the diff reports the added member as breaking and no reorder:\n{diff}",
        );

        let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);
        assert_eq!(
            stderr.matches("warning[RIDL-407]").count(),
            1,
            "{label}: one warning, for the inserted member:\n{stderr}",
        );
        let block = ridl_407_block(&stderr, "hinge");
        assert!(
            block.starts_with(&format!(
                "warning[RIDL-407]: `hinge` takes ordinal {ordinal} in `{container}`"
            )),
            "{label}: the message names the member, its ordinal and the body:\n{stderr}",
        );
        assert!(
            block.contains(&format!(
                "which the published baseline assigns to `{holder}`"
            )),
            "{label}: the message names the member the baseline declares at that ordinal:\n{stderr}",
        );
        assert!(
            block.contains(moved),
            "{label}: the message names the members the insertion shifted:\n{stderr}",
        );
        assert!(
            block.contains("typl §7.4") && block.contains("declare it at the end of the body"),
            "{label}: the message cites the rule and names the remedy:\n{stderr}",
        );
        assert!(
            !block.contains("interaction") && !block.contains("has moved"),
            "{label}: an inserted member is neither an interaction nor a reorder:\n{stderr}",
        );
        assert_underlines(block, "hinge: DoorState", location, &stderr);
        assert_eq!(
            code, 0,
            "{label}: the warning leaves the exit code alone:\n{stderr}",
        );
    }
}

/// A field or arm deleted without a `reserved` tombstone frees its ordinal,
/// and every later member slides into a wire identity that is not its own
/// (typl §7.4). `ridl diff` reports `decl_removed`, breaking; the desk check
/// warns once, names the tombstone that keeps the slot, and points at the
/// container's name, since the member's own declaration is gone.
#[test]
fn check_flags_a_struct_field_or_union_arm_removal_as_the_gate_does() {
    for (label, container, keyword, edit, location) in [
        (
            "struct-remove",
            "Report",
            "struct",
            "struct Report {\n  door: DoorState\n",
            "cluster.ridl:4:8",
        ),
        (
            "union-remove",
            "Reading",
            "union",
            "union Reading {\n  door: DoorState\n",
            "cluster.ridl:8:7",
        ),
    ] {
        let dir = TempDir::new(label);
        let root = package_workspace(&dir, COMPOSITES);
        let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
        assert_eq!(code, 0, "{label}: the baseline is written: {stderr}");

        dir.write(
            "cluster.ridl",
            &COMPOSITES.replacen(edit, &format!("{keyword} {container} {{\n"), 1),
        );
        let (code, diff) = diff_against_baseline(&root);
        assert_eq!(
            code, 1,
            "{label}: `ridl diff` gates on the removal:\n{diff}"
        );
        assert!(
            diff.contains(&format!(
                "[breaking] decl_removed veh.cluster/{container}/door"
            )),
            "{label}: the diff reports the removed member as breaking:\n{diff}",
        );

        let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);
        assert_eq!(
            stderr.matches("warning[RIDL-407]").count(),
            1,
            "{label}: one warning, for the removed member:\n{stderr}",
        );
        let block = ridl_407_block(&stderr, "door");
        assert!(
            block.starts_with(&format!(
                "warning[RIDL-407]: `door` is gone in `{container}`"
            )),
            "{label}: the message names the member and the body:\n{stderr}",
        );
        assert!(
            block.contains("typl §7.4") && block.contains("`reserved door`"),
            "{label}: the message cites the rule and names the tombstone:\n{stderr}",
        );
        assert!(
            !block.contains("interaction"),
            "{label}: a composite member is not an interaction:\n{stderr}",
        );
        assert_underlines(block, container, location, &stderr);
        assert_eq!(
            code, 0,
            "{label}: the warning leaves the exit code alone:\n{stderr}",
        );
    }
}

/// A field or arm retired in place with `reserved` keeps its slot, the typl
/// §7.4 discipline — but `ridl diff` matches a removal by name and does not
/// read the body's `reserved` entries (the limit `diff_composite` records),
/// so it gates the retirement as breaking like a bare deletion. The desk
/// check repeats the gate rather than disagree with it, says so, and points
/// at the tombstone.
#[test]
fn check_repeats_the_gate_on_a_tombstoned_struct_field_or_union_arm_removal() {
    for (label, container, location) in [
        ("struct-tombstone", "Report", "cluster.ridl:5:3"),
        ("union-tombstone", "Reading", "cluster.ridl:9:3"),
    ] {
        let dir = TempDir::new(label);
        let root = package_workspace(&dir, COMPOSITES);
        let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
        assert_eq!(code, 0, "{label}: the baseline is written: {stderr}");

        let opening = if container == "Report" {
            "struct Report {\n"
        } else {
            "union Reading {\n"
        };
        dir.write(
            "cluster.ridl",
            &COMPOSITES.replacen(
                &format!("{opening}  door: DoorState\n"),
                &format!("{opening}  reserved door\n"),
                1,
            ),
        );
        let (code, diff) = diff_against_baseline(&root);
        assert_eq!(
            code, 1,
            "{label}: `ridl diff` still gates a tombstoned removal:\n{diff}",
        );
        assert!(
            diff.contains(&format!(
                "[breaking] decl_removed veh.cluster/{container}/door"
            )),
            "{label}: the diff reports the retired member as removed:\n{diff}",
        );

        let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);
        assert_eq!(
            stderr.matches("warning[RIDL-407]").count(),
            1,
            "{label}: one warning, agreeing with the gate:\n{stderr}",
        );
        let block = ridl_407_block(&stderr, "door");
        assert!(
            block.starts_with(&format!(
                "warning[RIDL-407]: `door` is retired in `{container}`"
            )) && block.contains("`ridl diff`"),
            "{label}: the message says the member is retired and that the gate still \
             reports it:\n{stderr}",
        );
        assert!(
            !block.contains("retire it in place"),
            "{label}: a retired member is not told to retire itself:\n{stderr}",
        );
        assert_underlines(block, "reserved door", location, &stderr);
        assert_eq!(
            code, 0,
            "{label}: the warning leaves the exit code alone:\n{stderr}",
        );
    }
}

/// A field or arm appended at the end takes a fresh ordinal and moves no
/// other: `ridl diff` classifies it compatible, and the desk check stays
/// silent — the same verdict, read from the same change.
#[test]
fn check_is_silent_for_a_struct_field_or_union_arm_append() {
    for (label, container, edit) in [
        ("struct-append", "Report", "  latch: LatchState\n}\nunion"),
        ("union-append", "Reading", "  latch: LatchState\n}\nenum"),
    ] {
        let dir = TempDir::new(label);
        let root = package_workspace(&dir, COMPOSITES);
        let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
        assert_eq!(code, 0, "{label}: the baseline is written: {stderr}");

        dir.write(
            "cluster.ridl",
            &COMPOSITES.replacen(edit, &edit.replacen("\n}", "\n  hinge: DoorState\n}", 1), 1),
        );
        let (code, diff) = diff_against_baseline(&root);
        assert_eq!(code, 0, "{label}: `ridl diff` passes an append:\n{diff}");
        assert!(
            diff.contains(&format!(
                "[compatible] decl_added veh.cluster/{container}/hinge"
            )),
            "{label}: the diff reports the appended member as compatible:\n{diff}",
        );

        let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);
        assert!(
            !stderr.contains("RIDL-407"),
            "{label}: an appended member draws no desk warning:\n{stderr}",
        );
        assert_eq!(
            code, 0,
            "{label}: a compatible change stays clean:\n{stderr}"
        );
    }
}

/// A non-optional struct field appended at the end whose type excludes 0 —
/// here `Level`, `[1..3]` — is breaking for its type: a reader of the new
/// version refuses every payload of the old one, which does not carry the
/// field (driftsys/ridl#598). `ridl diff` exits 1 on it, and the desk check
/// stays silent, because no ordinal moved. Declared optional, the same field
/// is compatible.
#[test]
fn an_appended_struct_field_whose_type_excludes_zero_fails_the_gate_and_not_the_desk() {
    for (label, field, code_wanted, verdict) in [
        ("append-level", "hinge: Level", 1, "breaking"),
        ("append-optional-level", "hinge: Level?", 0, "compatible"),
    ] {
        let dir = TempDir::new(label);
        let source = COMPOSITES.replacen(
            "struct Report {",
            "type Level: integer [1..3]\nstruct Report {",
            1,
        );
        let root = package_workspace(&dir, &source);
        let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
        assert_eq!(code, 0, "{label}: the baseline is written: {stderr}");

        dir.write(
            "cluster.ridl",
            &source.replacen(
                "  latch: LatchState\n}\nunion",
                &format!("  latch: LatchState\n  {field}\n}}\nunion"),
                1,
            ),
        );
        let (code, diff) = diff_against_baseline(&root);
        assert!(
            diff.contains(&format!("[{verdict}] decl_added veh.cluster/Report/hinge")),
            "{label}: the diff reports the appended field as {verdict}:\n{diff}",
        );
        assert_eq!(code, code_wanted, "{label}: the gate's exit code:\n{diff}");

        let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);
        assert!(
            !stderr.contains("RIDL-407"),
            "{label}: an append moves no ordinal, so it draws no desk warning:\n{stderr}",
        );
        assert_eq!(code, 0, "{label}: the desk check stays clean:\n{stderr}");
    }

    let (code, stdout, _) = ridl(&["diff".as_ref(), "--explain".as_ref(), "decl_added".as_ref()]);
    assert_eq!(
        code, 0,
        "--explain decl_added prints the rule row:\n{stdout}"
    );
    assert!(
        stdout.contains("(`field : T?`)"),
        "the rule row tells the author to declare the new field optional:\n{stdout}",
    );
}

/// A struct field appended with a `ridl.std` type is judged by that type's
/// declaration, although no snapshot carries `ridl.std`: `ridl diff` resolves
/// it against the built-in package (driftsys/ridl#598). `Timestamp` and
/// `Duration` allow 0, so appending one is compatible; `Label` is a string,
/// which has no default, so appending it is breaking.
#[test]
fn an_appended_struct_field_with_a_standard_type_is_judged_by_its_declaration() {
    for (label, field, code_wanted, verdict) in [
        ("append-timestamp", "stamp: Timestamp", 0, "compatible"),
        ("append-duration", "stamp: Duration", 0, "compatible"),
        ("append-label", "stamp: Label", 1, "breaking"),
    ] {
        let dir = TempDir::new(label);
        let root = package_workspace(&dir, COMPOSITES);
        let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
        assert_eq!(code, 0, "{label}: the baseline is written: {stderr}");

        dir.write(
            "cluster.ridl",
            &COMPOSITES.replacen(
                "  latch: LatchState\n}\nunion",
                &format!("  latch: LatchState\n  {field}\n}}\nunion"),
                1,
            ),
        );
        let (code, diff) = diff_against_baseline(&root);
        assert!(
            diff.contains(&format!("[{verdict}] decl_added veh.cluster/Report/stamp")),
            "{label}: the diff reports `{field}` as {verdict}:\n{diff}",
        );
        assert_eq!(code, code_wanted, "{label}: the gate's exit code:\n{diff}");
    }
}

/// A `reserved` entry added above the live members shifts every ordinal
/// after it while no declaration moves. `ridl diff` reports one
/// `member_reordered` per shifted member; the desk check warns once for each
/// and says the ordinal changed because of a tombstone, not that the member
/// moved — the member's own line is where it was (driftsys/ridl#533).
#[test]
fn check_names_the_tombstone_that_shifted_a_struct_field_or_union_arm() {
    for (label, container, edit, door_at, latch_at) in [
        (
            "struct-shift",
            "Report",
            "struct Report {\n",
            "cluster.ridl:6:3",
            "cluster.ridl:7:3",
        ),
        (
            "union-shift",
            "Reading",
            "union Reading {\n",
            "cluster.ridl:10:3",
            "cluster.ridl:11:3",
        ),
    ] {
        let dir = TempDir::new(label);
        let root = package_workspace(&dir, COMPOSITES);
        let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
        assert_eq!(code, 0, "{label}: the baseline is written: {stderr}");

        dir.write(
            "cluster.ridl",
            &COMPOSITES.replacen(edit, &format!("{edit}  reserved gone\n"), 1),
        );
        let (code, diff) = diff_against_baseline(&root);
        assert_eq!(code, 1, "{label}: `ridl diff` gates on the shift:\n{diff}");
        assert_eq!(
            diff.matches("member_reordered").count(),
            2,
            "{label}: one reorder per shifted member:\n{diff}",
        );

        let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);
        assert_eq!(
            stderr.matches("warning[RIDL-407]").count(),
            2,
            "{label}: one warning per shifted member:\n{stderr}",
        );
        for (member, was, now, declaration, location) in [
            ("door", 1, 2, "door: DoorState", door_at),
            ("latch", 2, 3, "latch: LatchState", latch_at),
        ] {
            let block = ridl_407_block(&stderr, member);
            assert!(
                block.starts_with(&format!(
                    "warning[RIDL-407]: `{member}` has not moved in `{container}`"
                )),
                "{label}: the message says the member's own declaration did not \
                 move:\n{stderr}",
            );
            assert!(
                block.contains(&format!("(ordinal {was} there, ordinal {now} here)")),
                "{label}: the message states both ordinals:\n{stderr}",
            );
            assert!(
                block.contains("`reserved`") && block.contains("typl §7.4"),
                "{label}: the message names the tombstone as the cause and cites the \
                 rule:\n{stderr}",
            );
            assert!(
                !block.contains("has moved in"),
                "{label}: a shifted member is not reported as moved:\n{stderr}",
            );
            assert_underlines(block, declaration, location, &stderr);
        }
        assert_eq!(
            code, 0,
            "{label}: the warning leaves the exit code alone:\n{stderr}",
        );
    }
}

/// A swap and a tombstone in one edit: `door` and `latch` change places, and
/// a `reserved` entry goes in above `hinge`, whose own place among the live
/// members does not change. The two swapped members are reported as moved
/// and `hinge` as shifted — the decision is per member, not per body.
#[test]
fn check_tells_a_moved_member_from_a_shifted_one_in_the_same_edit() {
    let three = COMPOSITES.replace(
        "struct Report {\n  door: DoorState\n  latch: LatchState\n}",
        "struct Report {\n  door: DoorState\n  latch: LatchState\n  hinge: DoorState\n}",
    );
    let dir = TempDir::new("swap-and-shift");
    let root = package_workspace(&dir, &three);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the baseline is written: {stderr}");

    dir.write(
        "cluster.ridl",
        &three.replace(
            "struct Report {\n  door: DoorState\n  latch: LatchState\n  hinge: DoorState\n}",
            "struct Report {\n  latch: LatchState\n  door: DoorState\n  reserved gone\n  hinge: \
             DoorState\n}",
        ),
    );
    let (code, diff) = diff_against_baseline(&root);
    assert_eq!(code, 1, "`ridl diff` gates on the edit:\n{diff}");
    assert_eq!(
        diff.matches("member_reordered").count(),
        3,
        "every ordinal moved:\n{diff}",
    );

    let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);
    assert_eq!(
        stderr.matches("warning[RIDL-407]").count(),
        3,
        "one warning per member:\n{stderr}",
    );
    assert_member_moved(
        &stderr,
        "door",
        "Report",
        1,
        2,
        "door: DoorState",
        "cluster.ridl:6:3",
    );
    assert_member_moved(
        &stderr,
        "latch",
        "Report",
        2,
        1,
        "latch: LatchState",
        "cluster.ridl:5:3",
    );
    let block = ridl_407_block(&stderr, "hinge");
    assert!(
        block.starts_with("warning[RIDL-407]: `hinge` has not moved in `Report`")
            && block.contains("(ordinal 3 there, ordinal 4 here)"),
        "the member the tombstone shifted is not reported as moved:\n{stderr}",
    );
    assert_underlines(block, "hinge: DoorState", "cluster.ridl:8:3", &stderr);
    assert_eq!(code, 0, "the warnings leave the exit code alone:\n{stderr}");
}

/// A member inserted beside a `reserved` entry. After the tombstone, the new
/// member takes the ordinal of the first live member, whose place among the
/// live members it also takes, so the ordinal the message states is not the
/// live position. In the tombstone's own slot, it revives a retired ordinal
/// and no live member moves.
#[test]
fn check_names_the_baseline_holder_of_an_inserted_member_ordinal() {
    let tombstoned = COMPOSITES.replace(
        "struct Report {\n  door: DoorState\n",
        "struct Report {\n  reserved gone\n  door: DoorState\n",
    );
    for (label, body, ordinal, holder, moved, location) in [
        (
            "after-tombstone",
            "struct Report {\n  reserved gone\n  hinge: DoorState\n  door: DoorState\n  latch: \
             LatchState\n}",
            2,
            "assigns to `door`",
            Some("`door` and `latch` also changed ordinal"),
            "cluster.ridl:6:3",
        ),
        (
            "into-retired-slot",
            "struct Report {\n  hinge: DoorState\n  door: DoorState\n  latch: LatchState\n}",
            1,
            "retires",
            None,
            "cluster.ridl:5:3",
        ),
    ] {
        let dir = TempDir::new(label);
        let root = package_workspace(&dir, &tombstoned);
        let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
        assert_eq!(code, 0, "{label}: the baseline is written: {stderr}");

        dir.write(
            "cluster.ridl",
            &tombstoned.replace(
                "struct Report {\n  reserved gone\n  door: DoorState\n  latch: LatchState\n}",
                body,
            ),
        );
        let (code, diff) = diff_against_baseline(&root);
        assert_eq!(
            code, 1,
            "{label}: `ridl diff` gates on the insertion:\n{diff}"
        );
        assert!(
            diff.contains("[breaking] decl_added veh.cluster/Report/hinge")
                && !diff.contains("member_reordered"),
            "{label}: the diff reports the added member as breaking and no reorder:\n{diff}",
        );

        let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);
        assert_eq!(
            stderr.matches("warning[RIDL-407]").count(),
            1,
            "{label}: one warning, for the inserted member:\n{stderr}",
        );
        let block = ridl_407_block(&stderr, "hinge");
        assert!(
            block.starts_with(&format!(
                "warning[RIDL-407]: `hinge` takes ordinal {ordinal} in `Report`, which the \
                 published baseline {holder}"
            )),
            "{label}: the message states the ordinal and what the baseline holds there:\n{stderr}",
        );
        match moved {
            Some(moved) => assert!(
                block.contains(moved),
                "{label}: the message names the members the insertion shifted:\n{stderr}",
            ),
            None => assert!(
                !block.contains("also changed ordinal"),
                "{label}: no live member moved, so none is named:\n{stderr}",
            ),
        }
        assert_underlines(block, "hinge: DoorState", location, &stderr);
        assert_eq!(
            code, 0,
            "{label}: the warning leaves the exit code alone:\n{stderr}",
        );
    }
}

/// `ridl diff` reports a member appended at the end as breaking when another
/// member moved or left the body in the same edit, and reports no reorder
/// beside an addition or a removal. The desk check warns on the appended
/// member, says it is at the end, and names the sibling change the verdict
/// stands for. When the appended struct field is also breaking on its own —
/// its type does not allow 0, so a reader of the new version refuses a
/// payload of the baseline (driftsys/ridl#598) — the message says so and
/// tells the author to declare it optional; otherwise it does not.
#[test]
fn check_explains_an_append_the_gate_reports_as_breaking() {
    for (label, body, warnings, reason, field, by_type) in [
        (
            "swap-and-append",
            "struct Report {\n  latch: LatchState\n  door: DoorState\n  hinge: DoorState\n}",
            1,
            "because `latch` and `door` changed ordinal in the same edit",
            "hinge: DoorState",
            false,
        ),
        (
            "retire-and-append",
            "struct Report {\n  door: DoorState\n  reserved latch\n  hinge: DoorState\n}",
            2,
            "because `latch` is no longer declared in the same edit",
            "hinge: DoorState",
            false,
        ),
        (
            "swap-and-append-excluding-zero",
            "struct Report {\n  latch: LatchState\n  door: DoorState\n  hinge: integer [1..3]\n}",
            1,
            "because `latch` and `door` changed ordinal in the same edit",
            "hinge: integer [1..3]",
            true,
        ),
    ] {
        let location = "cluster.ridl:7:3";
        let dir = TempDir::new(label);
        let root = package_workspace(&dir, COMPOSITES);
        let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
        assert_eq!(code, 0, "{label}: the baseline is written: {stderr}");

        dir.write(
            "cluster.ridl",
            &COMPOSITES.replace(
                "struct Report {\n  door: DoorState\n  latch: LatchState\n}",
                body,
            ),
        );
        let (code, diff) = diff_against_baseline(&root);
        assert_eq!(code, 1, "{label}: `ridl diff` gates on the edit:\n{diff}");
        assert!(
            diff.contains("[breaking] decl_added veh.cluster/Report/hinge")
                && !diff.contains("member_reordered"),
            "{label}: the diff reports the append as breaking and no reorder:\n{diff}",
        );

        let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);
        assert_eq!(
            stderr.matches("warning[RIDL-407]").count(),
            warnings,
            "{label}: one warning per change the gate reports:\n{stderr}",
        );
        let block = ridl_407_block(&stderr, "hinge");
        assert!(
            block.starts_with(
                "warning[RIDL-407]: `hinge` is declared in `Report` after every ordinal the \
                 published baseline assigns or retires"
            ) && block.contains(reason),
            "{label}: the message says the member is at the end and names the cause:\n{stderr}",
        );
        assert!(
            !block.contains("takes ordinal") && block.contains("this one stays at the end"),
            "{label}: an append is not told to move:\n{stderr}",
        );
        assert_eq!(
            block.contains("`hinge` is also breaking on its own")
                && block.contains("declare it optional, with `?` after its type"),
            by_type,
            "{label}: the message says the field is breaking for its type exactly when it \
             is:\n{stderr}",
        );
        assert_underlines(block, field, location, &stderr);
        assert_eq!(
            code, 0,
            "{label}: the warnings leave the exit code alone:\n{stderr}",
        );
    }
}

/// A removal is told from a retirement by the tombstone's ordinal, not its
/// name alone: `reserved door` at another ordinal does not hold `door`'s
/// slot. The message names the member that slid into the freed ordinal, or
/// says none did when the last member went.
#[test]
fn check_tells_a_misplaced_tombstone_and_a_last_member_from_a_retirement() {
    for (label, body, member, opening, consequence, declaration, location) in [
        (
            "misplaced-tombstone",
            "struct Report {\n  latch: LatchState\n  reserved door\n}",
            "door",
            "`door` is gone in `Report` but the published baseline declares it at ordinal 1, \
             and the `reserved door` entry sits at ordinal 2, not there",
            "`latch` slid into a wire identity that is not its own",
            "reserved door",
            "cluster.ridl:6:3",
        ),
        (
            "last-member",
            "struct Report {\n  door: DoorState\n}",
            "latch",
            "`latch` is gone in `Report` but the published baseline declares it at ordinal 2.",
            "which a later member could take",
            "Report",
            "cluster.ridl:4:8",
        ),
    ] {
        let dir = TempDir::new(label);
        let root = package_workspace(&dir, COMPOSITES);
        let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
        assert_eq!(code, 0, "{label}: the baseline is written: {stderr}");

        dir.write(
            "cluster.ridl",
            &COMPOSITES.replace(
                "struct Report {\n  door: DoorState\n  latch: LatchState\n}",
                body,
            ),
        );
        let (code, diff) = diff_against_baseline(&root);
        assert_eq!(
            code, 1,
            "{label}: `ridl diff` gates on the removal:\n{diff}"
        );
        assert!(
            diff.contains(&format!(
                "[breaking] decl_removed veh.cluster/Report/{member}"
            )) && !diff.contains("member_reordered"),
            "{label}: the diff reports the removal as breaking and no reorder:\n{diff}",
        );

        let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);
        assert_eq!(
            stderr.matches("warning[RIDL-407]").count(),
            1,
            "{label}: one warning, for the removed member:\n{stderr}",
        );
        let block = ridl_407_block(&stderr, member);
        assert!(
            block.starts_with(&format!("warning[RIDL-407]: {opening}"))
                && block.contains(consequence),
            "{label}: the message states the baseline ordinal and the consequence:\n{stderr}",
        );
        assert!(
            !block.contains("is retired")
                && block.contains(&format!("`reserved {member}` at ordinal")),
            "{label}: the member is not reported as retired, and the remedy names the \
             ordinal:\n{stderr}",
        );
        assert_underlines(block, declaration, location, &stderr);
        assert_eq!(
            code, 0,
            "{label}: the warning leaves the exit code alone:\n{stderr}",
        );
    }
}

/// An enum value added or removed is `ridl diff`'s alone: RIDL-407 is about
/// ordinals, and an enum value's identity is its explicit number (typl §8),
/// so the desk check stays silent whatever the gate says about it.
#[test]
fn check_is_silent_for_an_enum_value_added_or_removed() {
    for (label, edit, change) in [
        ("value-removed", "", "decl_removed veh.cluster/Gear/DRIVE"),
        (
            "value-added",
            "  DRIVE = 1\n  REVERSE = 2\n",
            "decl_added veh.cluster/Gear/REVERSE",
        ),
    ] {
        let dir = TempDir::new(label);
        let root = package_workspace(&dir, COMPOSITES);
        let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
        assert_eq!(code, 0, "{label}: the baseline is written: {stderr}");

        dir.write("cluster.ridl", &COMPOSITES.replace("  DRIVE = 1\n", edit));
        let (_, diff) = diff_against_baseline(&root);
        assert!(
            diff.contains(change),
            "{label}: the diff reports the enum value change:\n{diff}"
        );

        let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);
        assert!(
            !stderr.contains("RIDL-407"),
            "{label}: an enum value change draws no desk warning:\n{stderr}",
        );
        assert_eq!(code, 0, "{label}: the check passes:\n{stderr}");
    }
}

/// An arm appended to a union that becomes a result union is breaking on its
/// own: a result union's arms are its transport identity (ADR-0008 decision
/// 4). No sibling moved, so the desk check does not name one; it says why the
/// gate reports the append and that the warning repeats the gate.
#[test]
fn check_repeats_the_gate_on_an_arm_added_to_a_result_union() {
    let fixture = format!(
        "{COMPOSITES}error struct Fault {{\n  code: DoorState\n}}\nunion Outcome {{\n  ok: \
         Reading\n}}\n"
    );
    let dir = TempDir::new("result-arm");
    let root = package_workspace(&dir, &fixture);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the baseline is written: {stderr}");

    let edited = fixture.replace("  ok: Reading\n", "  ok: Reading\n  err: Fault\n");
    dir.write("cluster.ridl", &edited);
    let (code, diff) = diff_against_baseline(&root);
    assert_eq!(code, 1, "`ridl diff` gates on the arm:\n{diff}");
    assert!(
        diff.contains("[breaking] decl_added veh.cluster/Outcome/err")
            && !diff.contains("member_reordered"),
        "the diff reports the added arm as breaking and no reorder:\n{diff}",
    );

    let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);
    assert_eq!(
        stderr.matches("warning[RIDL-407]").count(),
        1,
        "one warning, for the added arm:\n{stderr}",
    );
    let block = ridl_407_block(&stderr, "err");
    assert!(
        block.starts_with(
            "warning[RIDL-407]: `err` is declared in `Outcome` after every ordinal the \
             published baseline assigns or retires, and `ridl diff` still reports the \
             addition as breaking: a result union's arms are its transport identity \
             (ADR-0008 decision 4)"
        ) && block.contains("repeats the gate"),
        "the message names the rule instead of a sibling:\n{stderr}",
    );
    assert!(
        !block.contains("because ") && !block.contains("put the other members back"),
        "no sibling is blamed and no move is asked for:\n{stderr}",
    );
    let line = edited
        .lines()
        .position(|line| line.trim_start().starts_with("err: Fault"))
        .expect("the arm is in the edited file")
        + 1;
    assert_underlines(
        block,
        "err: Fault",
        &format!("cluster.ridl:{line}:3"),
        &stderr,
    );
    assert_eq!(code, 0, "the warning leaves the exit code alone:\n{stderr}");
}

/// A `reserved` entry at the removed member's ordinal without its name — a
/// bare `reserved 2`, or an entry under another name — keeps the slot but
/// not the name (typl §7.4). The desk check says so, asks for the member's
/// own name, and points at the container, since no entry carries the name
/// the index would find.
#[test]
fn check_tells_a_tombstone_that_keeps_the_slot_but_not_the_name() {
    for (label, body) in [
        (
            "bare-tombstone",
            "struct Report {\n  door: DoorState\n  reserved 2\n}",
        ),
        (
            "tombstone-for-another-name",
            "struct Report {\n  door: DoorState\n  reserved other\n}",
        ),
    ] {
        let dir = TempDir::new(label);
        let root = package_workspace(&dir, COMPOSITES);
        let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
        assert_eq!(code, 0, "{label}: the baseline is written: {stderr}");

        dir.write(
            "cluster.ridl",
            &COMPOSITES.replace(
                "struct Report {\n  door: DoorState\n  latch: LatchState\n}",
                body,
            ),
        );
        let (code, diff) = diff_against_baseline(&root);
        assert_eq!(
            code, 1,
            "{label}: `ridl diff` gates on the removal:\n{diff}"
        );
        assert!(
            diff.contains("[breaking] decl_removed veh.cluster/Report/latch"),
            "{label}: the diff reports the removal as breaking:\n{diff}",
        );

        let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);
        assert_eq!(
            stderr.matches("warning[RIDL-407]").count(),
            1,
            "{label}: one warning, for the removed member:\n{stderr}",
        );
        let block = ridl_407_block(&stderr, "latch");
        assert!(
            block.starts_with(
                "warning[RIDL-407]: `latch` is gone in `Report`, and a `reserved` entry holds \
                 its ordinal without its name"
            ) && block.contains("write `reserved latch` instead")
                && block.contains("repeats the gate"),
            "{label}: the message says the slot is kept and the name is not:\n{stderr}",
        );
        assert!(
            !block.contains("could take") && !block.contains("is retired"),
            "{label}: the slot is neither free nor retired under the name:\n{stderr}",
        );
        assert_underlines(block, "Report", "cluster.ridl:4:8", &stderr);
        assert_eq!(
            code, 0,
            "{label}: the warning leaves the exit code alone:\n{stderr}",
        );
    }
}

/// An enum value or enum-set bit takes its identity from its explicit number,
/// so reordering the body is not a change (typl §8, §9). `ridl diff` still
/// reports it conservatively; the desk check stays silent, because its
/// warning says declaration order is the wire identity, which is false here
/// (the decision recorded on driftsys/ridl#335).
#[test]
fn check_is_silent_for_an_enum_and_enum_set_reorder() {
    let dir = TempDir::new("enum-reorder");
    let root = package_workspace(&dir, COMPOSITES);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the baseline is written: {stderr}");

    dir.write("cluster.ridl", ENUM_MEMBERS_SWAPPED);
    let (code, diff, _) = ridl(&[
        "diff".as_ref(),
        root.join(".ridl/baseline").as_os_str(),
        root.as_os_str(),
    ]);
    assert_eq!(code, 1, "`ridl diff` gates on the reorder:\n{diff}");
    for member in [
        "Gear/PARK",
        "Gear/DRIVE",
        "Warnings/LOW_FUEL",
        "Warnings/DOOR_OPEN",
    ] {
        assert!(
            diff.contains(&format!("member_reordered veh.cluster/{member}: position")),
            "the fixture is a reorder `ridl diff` reports for `{member}`:\n{diff}",
        );
    }

    let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);
    assert!(
        !stderr.contains("RIDL-407"),
        "an enum or enum-set reorder moves no wire identity:\n{stderr}",
    );
    assert_eq!(code, 0, "the reorder leaves a clean check clean:\n{stderr}");
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
    assert_eq!(
        stderr,
        format!(
            "error: the baseline `{}` does not exist\n",
            absent.display()
        ),
        "the error names the missing path"
    );
}

/// `--baseline` pointing at a directory that holds IR artifacts but no
/// `.ir.json` snapshot is refused, exit 2 (issue #218 item 4). Before the
/// refusal the snapshot scan read it as an *empty* baseline: the desk check
/// silently ran against nothing and the command printed nothing at all —
/// false assurance, worse than a confusing message.
#[test]
fn check_refuses_a_baseline_directory_of_non_json_artifacts() {
    let dir = TempDir::new("txtpb-dir");
    let root = package_workspace(&dir, BASE);
    dir.write("artifacts/veh.cluster.ir.txtpb", "name: \"veh.cluster\"\n");
    let artifacts = dir.path().join("artifacts");

    let (code, _, stderr) = ridl(&[
        "check".as_ref(),
        root.as_os_str(),
        "--baseline".as_ref(),
        artifacts.as_os_str(),
    ]);

    assert_eq!(
        code, 2,
        "the artifact directory is an input error:\n{stderr}"
    );
    assert_eq!(
        stderr,
        format!(
            "error: {}: the directory holds IR artifacts (`veh.cluster.ir.txtpb`) but no \
             `.ir.json` snapshot; a baseline stays `.ir.json` (ADR-0014 decision 5); publish \
             one with `ridl baseline`\n",
            artifacts.display()
        ),
        "the message describes the directory, not an empty baseline"
    );
}

/// `--baseline` pointing one level above the published snapshots — `.ridl`
/// rather than `.ridl/baseline` — is refused, exit 2 (issue #230).
///
/// The directory holds no IR artifact *directly*, so the artifact refusal
/// above cannot see it, and it used to be indistinguishable from the ordinary
/// "no baseline published yet" state below: the desk check ran against
/// nothing and the command printed nothing at all. The control at the head of
/// this test is what makes that false assurance rather than a quiet no-op —
/// the very same edit warns when the flag names the right directory.
///
/// The snapshots are described where they are rather than descended into.
/// `ridl baseline` publishes one flat directory of `.ir.json` files and
/// stages into a *sibling* directory, so snapshots below a baseline directory
/// are never something the toolchain writes: descending would accept a layout
/// nothing produces, and would have to choose between subdirectories when
/// more than one holds snapshots — silently merging two unrelated baselines.
#[test]
fn check_refuses_a_baseline_directory_whose_snapshots_are_nested() {
    let dir = TempDir::new("nested");
    let root = package_workspace(&dir, BASE);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the baseline is published: {stderr}");
    dir.write("cluster.ridl", REORDERED);

    // The control: aimed at the directory that holds the snapshots, this
    // exact drift draws the desk warning.
    let published = root.join(".ridl/baseline");
    let (_, _, stderr) = ridl(&[
        "check".as_ref(),
        root.as_os_str(),
        "--baseline".as_ref(),
        published.as_os_str(),
    ]);
    assert!(
        stderr.contains("RIDL-407"),
        "the drift this run must not lose sight of:\n{stderr}",
    );

    let nest = root.join(".ridl");
    let (code, _, stderr) = ridl(&[
        "check".as_ref(),
        root.as_os_str(),
        "--baseline".as_ref(),
        nest.as_os_str(),
    ]);

    assert_eq!(code, 2, "one level too high is an input error:\n{stderr}");
    assert_eq!(
        stderr,
        format!(
            "error: {}: no `.ir.json` snapshot directly inside, but the subdirectory \
             `baseline` holds one; snapshots are read from one directory, never from the \
             directories below it; pass `--baseline {}` instead\n",
            nest.display(),
            published.display(),
        ),
        "the message names the subdirectory that holds the snapshots"
    );
}

/// A published baseline that cannot be *listed* is exit 2, not an empty
/// baseline.
///
/// The nesting scan reads a level no caller has listed, so it is the one
/// place where a directory that cannot be read could quietly become a
/// directory that holds nothing: `--baseline .ridl` over an unreadable
/// `.ridl/baseline/` used to report `Ok(0 packages)`, and `desk_check`
/// returns early on an empty baseline — a clean desk check that ran against a
/// baseline it never opened. That is the same false assurance issue #230 is
/// about, reached through permissions instead of a mistyped path, so the
/// error is reported rather than swallowed.
#[cfg(unix)]
#[test]
fn check_reports_a_baseline_subdirectory_it_cannot_read() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = TempDir::new("unreadable");
    let root = package_workspace(&dir, BASE);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the baseline is published: {stderr}");
    dir.write("cluster.ridl", REORDERED);

    let published = root.join(".ridl/baseline");
    let restore = std::fs::metadata(&published)
        .expect("the published directory exists")
        .permissions();
    std::fs::set_permissions(&published, std::fs::Permissions::from_mode(0o000))
        .expect("make the published directory unreadable");
    // A process that can read it anyway — root — cannot stage this case at
    // all, so there is nothing to assert rather than something to fail on.
    if std::fs::read_dir(&published).is_ok() {
        std::fs::set_permissions(&published, restore).expect("restore the permissions");
        return;
    }

    let nest = root.join(".ridl");
    let (code, _, stderr) = ridl(&[
        "check".as_ref(),
        root.as_os_str(),
        "--baseline".as_ref(),
        nest.as_os_str(),
    ]);
    std::fs::set_permissions(&published, restore).expect("restore the permissions");

    assert_eq!(
        code, 2,
        "a baseline that cannot be read is not an absent one:\n{stderr}"
    );
    assert!(
        stderr.starts_with(&format!("error: cannot read {}: ", published.display())),
        "the message names the directory it could not list:\n{stderr}"
    );
}

/// A directory with no IR artifacts at all — directly inside or one level
/// down — falls past both refusals above the same way a directory whose
/// snapshots sit two or more levels down does. Naming it with an explicit
/// `--baseline` is an input error (driftsys/ridl#235), not the silent skip an
/// absent flag gets — pinned separately by
/// `auto_discovery_of_an_empty_baseline_directory_stays_silent`.
#[test]
fn check_refuses_an_empty_baseline_directory() {
    let dir = TempDir::new("emptydir");
    let root = package_workspace(&dir, BASE);
    let empty = dir.path().join("published");
    std::fs::create_dir_all(&empty).expect("create the empty baseline directory");

    let (code, _, stderr) = ridl(&[
        "check".as_ref(),
        root.as_os_str(),
        "--baseline".as_ref(),
        empty.as_os_str(),
    ]);

    assert_eq!(
        code, 2,
        "an explicit baseline holding no snapshot is an input error:\n{stderr}"
    );
    assert!(
        stderr.contains(&format!(
            "the baseline `{}` holds no `.ir.json` snapshot directly inside it",
            empty.display()
        )),
        "the cause names the directory — the nested and artifact-directory refusals say \
         `no `.ir.json` snapshot` too, so the wording must be this refusal's own:\n{stderr}",
    );
    assert!(
        stderr.contains("point `--baseline` at the directory that holds the snapshots")
            && stderr.contains(&format!("`ridl baseline --out {}`", empty.display())),
        "the remedy names both ways out, the aimed-too-high one first:\n{stderr}",
    );
    // The publish suggestion names the directory it writes to. "there" would
    // read as `.ridl/baseline/`, the nearest directory the sentence names
    // before it (driftsys/ridl#340).
    assert!(
        stderr.contains(&format!("into `{}`", empty.display())) && !stderr.contains(" there "),
        "the publish suggestion names the directory it writes to:\n{stderr}",
    );
}

/// `is_source_dir` treats a directory as a source tree when it holds a
/// `ridl.toml` *or* at least one `.typl`/`.ridl`/`.rsdl` file directly inside
/// it — either half is enough on its own. The source-tree case in
/// `an_explicit_baseline_holding_no_snapshot_is_an_input_error` satisfies both
/// halves at once (`package_workspace` writes `ridl.toml` and `cluster.ridl`
/// into the same directory), so it cannot tell the two halves apart. This
/// test isolates the manifest half: an explicit `--baseline` naming a
/// directory that holds `ridl.toml` and no source file must still omit the
/// publish suggestion (driftsys/ridl#340).
#[test]
fn check_refuses_an_empty_baseline_directory_holding_only_a_manifest() {
    let dir = TempDir::new("emptydir-manifest");
    let root = package_workspace(&dir, BASE);
    let manifest_only = dir.path().join("manifest-only");
    std::fs::create_dir_all(&manifest_only).expect("create the manifest-only directory");
    std::fs::write(manifest_only.join("ridl.toml"), MANIFEST).expect("write the manifest");

    let (code, _, stderr) = ridl(&[
        "check".as_ref(),
        root.as_os_str(),
        "--baseline".as_ref(),
        manifest_only.as_os_str(),
    ]);

    assert_eq!(
        code, 2,
        "an explicit baseline holding no snapshot is an input error:\n{stderr}"
    );
    assert!(
        stderr.contains(&format!(
            "the baseline `{}` holds no `.ir.json` snapshot directly inside it",
            manifest_only.display()
        )),
        "the cause names the directory:\n{stderr}",
    );
    assert!(
        !stderr.contains("ridl baseline --out"),
        "a `ridl.toml` alone makes this a source tree, so the refusal does not \
         suggest publishing into it:\n{stderr}",
    );
}

/// The source-file half of the same case: an explicit `--baseline` naming a
/// directory that holds a `.ridl` file and no `ridl.toml` must also omit the
/// publish suggestion (driftsys/ridl#340). See
/// `check_refuses_an_empty_baseline_directory_holding_only_a_manifest` for why
/// the two halves need separate fixtures.
#[test]
fn check_refuses_an_empty_baseline_directory_holding_only_a_source_file() {
    let workspace = TempDir::new("emptydir-source-ws");
    let root = package_workspace(&workspace, BASE);
    // A separate `TempDir`, not a subdirectory of `root`: a `.ridl` file
    // nested under `root` would itself be compiled as part of the workspace
    // package (its package name would have to mirror the nested path), which
    // is not what this fixture is testing.
    let baseline_dir = TempDir::new("emptydir-source-baseline");
    let source_only = baseline_dir.path().join("source-only");
    std::fs::create_dir_all(&source_only).expect("create the source-only directory");
    std::fs::write(source_only.join("cluster.ridl"), BASE).expect("write the source file");

    let (code, _, stderr) = ridl(&[
        "check".as_ref(),
        root.as_os_str(),
        "--baseline".as_ref(),
        source_only.as_os_str(),
    ]);

    assert_eq!(
        code, 2,
        "an explicit baseline holding no snapshot is an input error:\n{stderr}"
    );
    assert!(
        stderr.contains(&format!(
            "the baseline `{}` holds no `.ir.json` snapshot directly inside it",
            source_only.display()
        )),
        "the cause names the directory:\n{stderr}",
    );
    assert!(
        !stderr.contains("ridl baseline --out"),
        "a `.ridl` file alone makes this a source tree, so the refusal does \
         not suggest publishing into it:\n{stderr}",
    );
}

/// `--baseline` refuses a prototext or binary IR artifact by name: a
/// baseline stays `.ir.json` (ADR-0014 decision 5). Before the refusal the
/// snapshot loader read the file as JSON and reported a parse error, which
/// misdiagnoses the mistake.
#[test]
fn check_refuses_a_non_json_baseline() {
    let dir = TempDir::new("refuse");
    let root = package_workspace(&dir, BASE);
    for name in ["published.ir.txtpb", "published.ir.binpb"] {
        let artifact = dir.write(name, "name: \"veh.cluster\"\n");
        let (code, _, stderr) = ridl(&[
            "check".as_ref(),
            root.as_os_str(),
            "--baseline".as_ref(),
            artifact.as_os_str(),
        ]);
        assert_eq!(code, 2, "`{name}` is an input error, stderr:\n{stderr}");
        assert!(
            stderr.contains(".ir.json"),
            "the refusal must name the accepted encoding:\n{stderr}"
        );
    }
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

/// The same comparison is clean for a workspace that names a **standard** type,
/// and the published baseline holds no `ridl.std` snapshot.
///
/// `ridl baseline` runs the build driver with `--emit ir-json`, and that driver
/// also writes the standard package whenever the workspace references it (issue
/// #190). `ridl diff` compiles its current side without `ridl.std` — the
/// standard package is not a workspace member — so a `ridl.std.ir.json` in the
/// baseline has nothing to match on the other side, and the diff of an untouched
/// workspace reported `ridl.std` as a removed package and exited 1. A baseline
/// holds the packages the workspace *declares*; the standard package is
/// version-locked to the compiler binary and is not one of them.
///
/// Every other baseline and diff fixture here names only its own declarations,
/// which is why none of them saw it. `ridl check --baseline` did not either: the
/// desk check reports ordinal drift only, and a whole removed package carries no
/// ordinal change.
#[test]
fn diff_of_an_unchanged_workspace_naming_a_standard_type_is_clean() {
    let dir = TempDir::new("diffstd");
    let root = package_workspace(&dir, NAMES_A_STANDARD_TYPE);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the workspace snapshots cleanly:\n{stderr}");

    let baseline = root.join(".ridl/baseline");
    assert!(
        baseline.join("veh.cluster.ir.json").is_file(),
        "the declared package is published",
    );
    assert!(
        !baseline.join("ridl.std.ir.json").exists(),
        "the standard package is not part of the workspace's contract snapshot",
    );

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
    let root = package_workspace(&dir, BASE);
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
            "event tyrePressure: DoorState @[100ms..1s]",
        ),
        (
            "legacyWheelPhase",
            "give this interaction a different name",
            "signal legacyWheelPhase: Speed @10ms",
        ),
        (
            "doorOpened",
            "put the declarations back in the baseline's order",
            "event doorOpened: DoorState @[100ms..1s]",
        ),
        (
            "doorClosed",
            "put the declarations back in the baseline's order",
            "event doorClosed: DoorState @[100ms..1s]",
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
        inline.contains("cluster.ridl:48:9"),
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

/// A baseline path aimed two or more levels above the snapshots falls past
/// both existing refusals and yields no snapshot. Comparing against nothing
/// reports no drift and exits 0, which reads exactly like a clean check, so it
/// is an input error instead (driftsys/ridl#235).
#[test]
fn an_explicit_baseline_holding_no_snapshot_is_an_input_error() {
    let dir = TempDir::new("empty-explicit");
    let root = package_workspace(&dir, BASE);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the baseline is written: {stderr}");

    dir.write("cluster.ridl", REORDERED);
    // The snapshots are at `<root>/.ridl/baseline/`, two levels below `root`.
    let (code, _, stderr) = ridl(&[
        "check".as_ref(),
        root.as_os_str(),
        "--baseline".as_ref(),
        root.as_os_str(),
    ]);

    assert_eq!(
        code, 2,
        "the tool could not answer, so it says so instead of passing:\n{stderr}",
    );
    assert!(
        stderr.contains(&format!(
            "the baseline `{}` holds no `.ir.json` snapshot directly inside it",
            root.display()
        )),
        "the cause names the directory the flag aimed at:\n{stderr}",
    );
    // This is #235's own case: the snapshots are at `<root>/.ridl/baseline/`,
    // two levels below `root`. The remedy names the aimed-too-high mistake —
    // point `--baseline` at the snapshot directory instead. `root` is a
    // source tree, so the refusal leaves out the publish suggestion entirely,
    // which the assertion after this one checks.
    assert!(
        stderr.contains("point `--baseline` at the directory that holds the snapshots"),
        "the remedy says to aim the flag at the snapshots:\n{stderr}",
    );
    // `root` is a source tree, so the refusal offers no publish into it:
    // following that advice would write `veh.cluster.ir.json` next to the
    // sources, and a later `ridl baseline --out <root>` would delete every
    // other `.ir.json` file there (driftsys/ridl#340).
    assert!(
        !stderr.contains("ridl baseline --out"),
        "the refusal does not suggest publishing into the source tree:\n{stderr}",
    );
}

/// Auto-discovery keeps its silent skip even once the empty-baseline refusal
/// exists. `.ridl/baseline/` — the exact location `default_baseline_dir`
/// computes — is created but never published into, so `baseline_location`
/// discovers it and hands `load_baseline` `explicit == false`. No flag
/// asserted that a baseline is there, so "no baseline published yet" stays
/// legitimate, exactly as it did before this task added the refusal — pinned separately,
/// for an explicit `--baseline` naming the same kind of empty directory, by
/// `check_refuses_an_empty_baseline_directory`.
///
/// This is the test that actually exercises `load_baseline`'s `explicit`
/// guard: unlike a workspace with no `.ridl/baseline/` directory at all
/// (`check_without_a_baseline_is_unchanged`), where `baseline_location`
/// returns `None` and `load_baseline` is never called, an empty *existing*
/// directory reaches the guard and depends on `explicit` being `false` to
/// stay silent.
#[test]
fn auto_discovery_of_an_empty_baseline_directory_stays_silent() {
    let dir = TempDir::new("empty-auto");
    let root = package_workspace(&dir, BASE);
    std::fs::create_dir_all(root.join(".ridl").join("baseline"))
        .expect("create the empty baseline directory");

    let (code, stdout, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);

    assert_eq!(
        code, 0,
        "a clean check with no baseline succeeds:\n{stderr}"
    );
    assert!(
        stdout.is_empty() && stderr.is_empty(),
        "no baseline means no drift report at all:\nstdout: {stdout}\nstderr: {stderr}",
    );
}

/// A snapshot-named entry under the auto-discovered `.ridl/baseline/` whose
/// metadata cannot be read — a symlink to a file that is gone — is exit 2,
/// not the silent "no baseline published yet" skip (driftsys/ridl#339 case
/// 3). Skipping it read the directory as empty, and the desk check ran
/// against nothing.
#[cfg(unix)]
#[test]
fn check_reports_an_auto_discovered_snapshot_it_cannot_stat() {
    let dir = TempDir::new("dangling-auto");
    let root = package_workspace(&dir, BASE);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the baseline is published: {stderr}");

    let path = root.join(".ridl/baseline/veh.cluster.ir.json");
    std::fs::remove_file(&path).expect("remove the published snapshot");
    let target = PathBuf::from("missing.ir.json");
    std::os::unix::fs::symlink(&target, &path).expect("replace it with a dangling symlink");
    dir.write("cluster.ridl", REORDERED);

    let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);

    assert_eq!(
        code, 2,
        "a snapshot entry that cannot be read is not an absent baseline:\n{stderr}",
    );
    assert!(
        stderr.contains(&path.display().to_string()),
        "the message names the entry it could not read:\n{stderr}",
    );
    assert!(
        !stderr.contains("RIDL-407"),
        "no desk check runs over a baseline that could not be read:\n{stderr}",
    );
    assert!(
        std::fs::symlink_metadata(&path)
            .expect("the entry is still there")
            .file_type()
            .is_symlink(),
        "the symlink is left as it is",
    );
}

/// A dangling entry beside the published snapshot, named with an explicit
/// `--baseline`: exit 2 naming the entry, as under auto-discovery — an
/// explicit directory is listed by the same `snapshot_files`. The valid
/// snapshot is kept, so a listing that skipped the entry would run the desk
/// check over it and exit 0 with RIDL-407 — the exit 2 here comes from the
/// entry, not from the empty-directory refusal.
#[cfg(unix)]
#[test]
fn check_reports_an_explicit_baseline_snapshot_it_cannot_stat() {
    let dir = TempDir::new("dangling-explicit");
    let root = package_workspace(&dir, BASE);
    let published = dir.path().join("published");
    let (code, _, stderr) = ridl(&[
        "baseline".as_ref(),
        root.as_os_str(),
        "--out".as_ref(),
        published.as_os_str(),
    ]);
    assert_eq!(code, 0, "the baseline is published: {stderr}");

    let path = published.join("zz-dangling.ir.json");
    std::os::unix::fs::symlink("missing.ir.json", &path)
        .expect("create a dangling symlink beside the published snapshot");
    dir.write("cluster.ridl", REORDERED);

    let (code, _, stderr) = ridl(&[
        "check".as_ref(),
        root.as_os_str(),
        "--baseline".as_ref(),
        published.as_os_str(),
    ]);

    assert_eq!(
        code, 2,
        "a snapshot entry that cannot be read is not an absent baseline:\n{stderr}",
    );
    assert!(
        stderr.contains(&path.display().to_string()),
        "the message names the entry it could not read:\n{stderr}",
    );
    assert!(
        !stderr.contains("RIDL-407"),
        "no desk check runs over a baseline that could not be read:\n{stderr}",
    );
    assert!(
        !stderr.contains("holds no `.ir.json` snapshot"),
        "the refusal is the entry's, not the empty-directory one a listing that gave up \
         would draw over the snapshot still beside the entry:\n{stderr}",
    );
    assert!(
        std::fs::symlink_metadata(&path)
            .expect("the entry is still there")
            .file_type()
            .is_symlink(),
        "the symlink is left as it is",
    );
}

// A workspace member as the entry (ADR-0002 §4, issue #529).

/// The second member's source, with its two events in `first`, `second`
/// order, or swapped.
fn other_source(first: &str, second: &str) -> String {
    format!(
        "package veh.other
type Flag: integer [0..1]
interface OtherStatus {{
  event {first}: Flag @[100ms..1s]
  event {second}: Flag @[100ms..1s]
}}
"
    )
}

/// A workspace with the members `cluster` (`veh.cluster`, [`BASE`]) and
/// `other` (`veh.other`), locked, with the baseline published from the root.
fn member_workspace(dir: &TempDir) -> PathBuf {
    dir.write(
        "ridl.toml",
        "[workspace]\nmembers = [\"cluster\", \"other\"]\n",
    );
    dir.write("cluster/ridl.toml", MANIFEST);
    dir.write("cluster/cluster.ridl", BASE);
    dir.write(
        "other/ridl.toml",
        "[package]\nname = \"veh.other\"\nversion = \"1.0.0\"\n",
    );
    dir.write("other/other.ridl", &other_source("alpha", "beta"));
    let root = dir.path().to_path_buf();
    lock(&root);
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the baseline is written: {stderr}");
    root
}

/// The desk check from a member reports the RIDL-407 of that member only;
/// from the root it reports both members'.
#[test]
fn a_member_check_reports_only_the_member_desk_warnings() {
    let dir = TempDir::new("member-desk");
    let root = member_workspace(&dir);
    dir.write("cluster/cluster.ridl", REORDERED);
    dir.write("other/other.ridl", &other_source("beta", "alpha"));

    let (_, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);
    assert!(stderr.contains("`doorClosed` has moved"), "{stderr}");
    assert!(stderr.contains("in `OtherStatus`"), "{stderr}");

    let baseline = root.join(".ridl/baseline");
    let (code, _, stderr) = ridl(&[
        "check".as_ref(),
        root.join("cluster").as_os_str(),
        "--baseline".as_ref(),
        baseline.as_os_str(),
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.contains("`doorClosed` has moved"), "{stderr}");
    assert!(!stderr.contains("OtherStatus"), "{stderr}");

    // Without `--baseline`, a member entry finds the workspace root's
    // `.ridl/baseline/`.
    let (_, _, stderr) = ridl(&["check".as_ref(), root.join("cluster").as_os_str()]);
    assert!(stderr.contains("`doorClosed` has moved"), "{stderr}");
}

/// A relative member path from the workspace root (`ridl check cluster`)
/// finds the root as the empty parent of `cluster`, and the desk check still
/// reads the workspace's files, so each RIDL-407 carries its file and line.
#[test]
fn a_relative_member_entry_from_the_root_keeps_the_desk_warning_spans() {
    let dir = TempDir::new("member-desk-relative");
    let root = member_workspace(&dir);
    dir.write("cluster/cluster.ridl", REORDERED);

    let (code, _, stderr) = ridl_in(Some(&root), &["check".as_ref(), "cluster".as_ref()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.contains("`doorClosed` has moved"), "{stderr}");
    assert!(
        stderr.contains("cluster.ridl:"),
        "the warning names its file and line:\n{stderr}"
    );
}

/// The desk check compares the whole workspace, so it does not run while
/// another member has a compile error, even though a check of the member
/// does not report that error.
#[test]
fn a_member_check_skips_the_desk_check_while_another_member_has_an_error() {
    let dir = TempDir::new("member-desk-error");
    let root = member_workspace(&dir);
    dir.write("cluster/cluster.ridl", REORDERED);
    dir.write(
        "other/broken.ridl",
        "package veh.other\ntype Broken: Missing\n",
    );

    // `--baseline` names the directory, so the test does not depend on which
    // directory a member entry finds by default.
    let baseline = root.join(".ridl/baseline");
    let (code, _, stderr) = ridl(&[
        "check".as_ref(),
        root.join("cluster").as_os_str(),
        "--baseline".as_ref(),
        baseline.as_os_str(),
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert!(!stderr.contains("RIDL-407"), "{stderr}");
    assert!(!stderr.contains("broken.ridl"), "{stderr}");
}

/// `ridl baseline` on a member publishes to the workspace root's
/// `.ridl/baseline/`, the directory a check from the root reads.
#[test]
fn a_member_baseline_is_published_at_the_workspace_root() {
    let dir = TempDir::new("member-baseline");
    let root = member_workspace(&dir);
    std::fs::remove_dir_all(root.join(".ridl")).expect("remove the root baseline");

    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.join("cluster").as_os_str()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(root.join(".ridl/baseline/veh.cluster.ir.json").is_file());
    assert!(!root.join("cluster/.ridl").exists());
}
