//! The `[lints]` levels as the `ridl` command shows and applies them (lint
//! foundation spec §6.2, §7.1 and §7.2): the `lint` field of the JSON report,
//! the lint note of the text report, the exit code of `check` and `build`
//! under `deny`, the commands that do not apply levels (D-8), and the
//! `--baseline` path, whose RIDL-407 is raised by the CLI after `ridlc`
//! returns.

use std::ffi::OsStr;
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
            "ridl-lints-{label}-{}-{}",
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
fn ridl(args: &[&OsStr]) -> (i32, String, String) {
    ridl_in(None, args)
}

/// Runs `ridl` with `args` from `current_dir` when one is given.
fn ridl_in(current_dir: Option<&Path>, args: &[&OsStr]) -> (i32, String, String) {
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

/// A `signal` with no timing annotation, which draws RIDL-100
/// (`missing-timing`, a Warning by default). Seven lines, so the rendered
/// gutter is two spaces wide.
const SOURCE: &str = "package demo\n\ntype Speed: integer [0..300]\n\ninterface Sensor {\n  \
                      signal speed: Speed\n}\n";

const DENY: &str = "\n[lints]\nmissing-timing = \"deny\"\n";
const ALLOW: &str = "\n[lints]\nmissing-timing = \"allow\"\n";

/// A workspace with one member, `sensor`, whose manifest ends with `lints`:
/// the text of a `[lints]` table, or an empty string for none. Returns the
/// workspace root.
fn member_workspace(dir: &TempDir, lints: &str) -> PathBuf {
    dir.write("ridl.toml", "[workspace]\nmembers = [\"sensor\"]\n");
    dir.write(
        "sensor/ridl.toml",
        &format!("[package]\nname = \"demo\"\nversion = \"1.0.0\"\n{lints}"),
    );
    dir.write("sensor/sensor.ridl", SOURCE);
    dir.path().to_path_buf()
}

#[test]
fn json_carries_lint_field_and_deny_exits_1() {
    let dir = TempDir::new("json-deny");
    let root = member_workspace(&dir, DENY);
    // A second member file with an Error-code diagnostic, to check that an
    // Error code has no `lint` key. The file is broken on its own: no
    // trailing newline, so the parser's EOF diagnostic stays on line 2.
    dir.write("sensor/broken.ridl", "package demo\ntype X:");

    let (code, stdout, stderr) = ridl(&[
        "check".as_ref(),
        "--format".as_ref(),
        "json".as_ref(),
        root.as_os_str(),
    ]);

    assert_eq!(
        code, 1,
        "a lint at deny exits 1:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    let diagnostics: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is JSON");
    let diagnostics = diagnostics.as_array().expect("a JSON array");
    let ridl_100 = diagnostics
        .iter()
        .find(|diagnostic| diagnostic["code"] == "RIDL-100")
        .expect("a RIDL-100 element");
    assert_eq!(ridl_100["severity"], "error", "{ridl_100}");
    assert_eq!(ridl_100["lint"], "missing-timing", "{ridl_100}");

    let error_code = diagnostics
        .iter()
        .find(|diagnostic| diagnostic["code"] != "RIDL-100" && diagnostic["severity"] == "error")
        .expect("an Error-code element");
    assert!(
        error_code.get("lint").is_none(),
        "an Error code has no `lint` key: {error_code}"
    );
}

#[test]
fn text_deny_exits_1() {
    let dir = TempDir::new("text-deny");
    let root = member_workspace(&dir, DENY);

    let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);

    assert_eq!(code, 1, "a lint at deny exits 1:\n{stderr}");
    assert!(
        stderr.contains("error[RIDL-100]"),
        "the lint is rendered as an error:\n{stderr}"
    );
    assert!(
        !stderr.contains("warning[RIDL-100]"),
        "the lint is not rendered as a warning:\n{stderr}"
    );
}

#[test]
fn build_fails_on_deny() {
    let dir = TempDir::new("build-deny");
    let root = member_workspace(&dir, DENY);
    let out = TempDir::new("build-deny-out");

    let (code, _, stderr) = ridl(&[
        "build".as_ref(),
        "--out-dir".as_ref(),
        out.path().as_os_str(),
        "--emit".as_ref(),
        "ir-json".as_ref(),
        root.as_os_str(),
    ]);

    assert_eq!(code, 1, "a lint at deny fails the build:\n{stderr}");
    let written: Vec<PathBuf> = std::fs::read_dir(out.path())
        .expect("the out dir is readable")
        .map(|entry| entry.expect("a readable entry").path())
        .collect();
    assert!(written.is_empty(), "the build wrote: {written:?}");
}

/// `ridl diff` compiles both sides without applying levels (spec D-8): a lint
/// at `deny` on either side does not change its result.
#[test]
fn diff_ignores_deny() {
    let old = TempDir::new("diff-old");
    let old_root = member_workspace(&old, DENY);
    let new = TempDir::new("diff-new");
    let new_root = member_workspace(&new, DENY);

    let (code, stdout, stderr) =
        ridl(&["diff".as_ref(), old_root.as_os_str(), new_root.as_os_str()]);

    assert_eq!(
        code, 0,
        "a lint at deny does not change the diff:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

/// `ridl baseline` publishes the snapshot without applying levels (spec
/// D-8): a lint at `deny` does not block the publication.
#[test]
fn baseline_ignores_deny() {
    let dir = TempDir::new("baseline-deny");
    let root = member_workspace(&dir, DENY);
    let (code, _, stderr) = ridl(&["lock".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the fixture's lock is allocated: {stderr}");

    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);

    assert_eq!(
        code, 0,
        "a lint at deny does not block the baseline:\n{stderr}"
    );
    let snapshot = root.join(".ridl").join("baseline").join("demo.ir.json");
    assert!(
        snapshot.is_file(),
        "the snapshot is written at {}",
        snapshot.display()
    );
}

#[test]
fn allow_is_absent_in_text_and_json() {
    let dir = TempDir::new("allow");
    let root = member_workspace(&dir, ALLOW);

    let (code, stdout, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "stdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(
        !stderr.contains("RIDL-100"),
        "an allowed lint is absent from the text report:\n{stderr}"
    );

    let (code, stdout, stderr) = ridl(&[
        "check".as_ref(),
        "--format".as_ref(),
        "json".as_ref(),
        root.as_os_str(),
    ]);
    assert_eq!(code, 0, "stdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(
        !stdout.contains("RIDL-100"),
        "an allowed lint is absent from the JSON report:\n{stdout}"
    );
}

/// The SARIF 2.1.0 schema, downloaded from
/// `https://json.schemastore.org/sarif-2.1.0.json`. Every reference in it is
/// internal, so the validator needs no network.
const SARIF_SCHEMA: &str = include_str!("fixtures/sarif-2.1.0.json");

/// Runs `ridl check --format sarif .` from `root` and returns the exit code
/// and the parsed log, after validating the log against the vendored schema.
fn sarif_check(root: &Path) -> (i32, serde_json::Value) {
    sarif_check_in(root, ".".as_ref())
}

/// Runs `ridl check --format sarif <entry>` from `current_dir` and returns
/// the exit code and the parsed log, after validating the log against the
/// vendored schema.
fn sarif_check_in(current_dir: &Path, entry: &OsStr) -> (i32, serde_json::Value) {
    let (code, stdout, stderr) = ridl_in(
        Some(current_dir),
        &[
            "check".as_ref(),
            "--format".as_ref(),
            "sarif".as_ref(),
            entry,
        ],
    );
    let log: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|err| {
        panic!("stdout is JSON ({err}):\nstdout:\n{stdout}\nstderr:\n{stderr}")
    });
    let schema: serde_json::Value =
        serde_json::from_str(SARIF_SCHEMA).expect("the vendored schema is JSON");
    let validator = jsonschema::validator_for(&schema).expect("the vendored schema compiles");
    let errors: Vec<String> = validator
        .iter_errors(&log)
        .map(|err| format!("{} at {}", err, err.instance_path()))
        .collect();
    assert!(
        errors.is_empty(),
        "the log validates against the SARIF 2.1.0 schema:\n{}\nlog:\n{stdout}",
        errors.join("\n")
    );
    (code, log)
}

/// The results of the one run whose `ruleId` is `code`.
fn sarif_results<'a>(log: &'a serde_json::Value, code: &str) -> Vec<&'a serde_json::Value> {
    log["runs"][0]["results"]
        .as_array()
        .expect("results is an array")
        .iter()
        .filter(|result| result["ruleId"] == code)
        .collect()
}

#[test]
fn sarif_validates_and_denies() {
    let dir = TempDir::new("sarif-deny");
    let root = member_workspace(&dir, DENY);
    let (code, log) = sarif_check(&root);
    assert_eq!(code, 1, "a lint at deny exits 1:\n{log}");
    let ridl_100 = sarif_results(&log, "RIDL-100");
    assert_eq!(ridl_100.len(), 1, "{log}");
    assert_eq!(ridl_100[0]["level"], "error", "{}", ridl_100[0]);
    assert_eq!(
        ridl_100[0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
        "sensor/sensor.ridl",
        "{}",
        ridl_100[0]
    );

    let dir = TempDir::new("sarif-allow");
    let root = member_workspace(&dir, ALLOW);
    let (code, log) = sarif_check(&root);
    assert_eq!(code, 0, "{log}");
    assert!(
        sarif_results(&log, "RIDL-100").is_empty(),
        "an allowed lint is absent from the SARIF log:\n{log}"
    );
}

/// A second file of the `demo` package, in its `sub` directory, with one
/// untimed signal: it draws one RIDL-100 and nothing else.
const SUB_SOURCE: &str = "package demo.sub\n\ntype Level: integer [0..10]\n\ninterface Gauge {\n  \
                          signal level: Level\n}\n";

/// A package at `ws/` with `ws/sensor.ridl` and `ws/sub/<sub_file>`, each
/// drawing one RIDL-100. The fixture for the artifact URI tests, which run
/// the binary from `dir` so that `ws/` is one level below the working
/// directory.
fn two_level_package(dir: &TempDir, sub_file: &str) {
    dir.write(
        "ws/ridl.toml",
        "[package]\nname = \"demo\"\nversion = \"1.0.0\"\n",
    );
    dir.write("ws/sensor.ridl", SOURCE);
    dir.write(&format!("ws/sub/{sub_file}"), SUB_SOURCE);
}

/// The `file://` URI of an absolute path, percent-encoding every byte outside
/// the RFC 3986 unreserved set and `/`, as the binary is expected to write
/// the `%SRCROOT%` base.
fn file_uri(path: &Path) -> String {
    let mut uri = String::from("file://");
    for byte in path.to_str().expect("the temp dir is UTF-8").bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                uri.push(byte as char);
            }
            _ => uri.push_str(&format!("%{byte:02X}")),
        }
    }
    uri
}

/// The `(uri, uriBaseId)` pair of each RIDL-100 result's primary location,
/// sorted by URI.
fn ridl_100_locations(log: &serde_json::Value) -> Vec<(String, Option<String>)> {
    let mut locations: Vec<(String, Option<String>)> = sarif_results(log, "RIDL-100")
        .iter()
        .map(|result| {
            let location = &result["locations"][0]["physicalLocation"]["artifactLocation"];
            (
                location["uri"]
                    .as_str()
                    .expect("uri is a string")
                    .to_string(),
                location["uriBaseId"].as_str().map(str::to_string),
            )
        })
        .collect();
    locations.sort();
    locations
}

/// Asserts that `log` names the working directory `cwd` as `%SRCROOT%`, as a
/// `file://` URI that ends with `/`, and that both RIDL-100 results carry
/// `sub_uri` and `ws/sensor.ridl` relative to that base.
fn assert_cwd_relative_uris(log: &serde_json::Value, cwd: &Path, sub_uri: &str) {
    let cwd = cwd.canonicalize().expect("the temp dir resolves");
    let expected_base = format!("{}/", file_uri(&cwd));
    assert_eq!(
        log["runs"][0]["originalUriBaseIds"]["%SRCROOT%"]["uri"], expected_base,
        "{log}"
    );
    let base = Some("%SRCROOT%".to_string());
    assert_eq!(
        ridl_100_locations(log),
        vec![
            ("ws/sensor.ridl".to_string(), base.clone()),
            (sub_uri.to_string(), base),
        ],
        "{log}"
    );
}

/// An entry at a file: the URIs are relative to the working directory, not to
/// the file's directory.
#[test]
fn sarif_uris_are_cwd_relative_from_a_file_entry() {
    let dir = TempDir::new("sarif-uri-file");
    two_level_package(&dir, "gauge.ridl");
    let (code, log) = sarif_check_in(dir.path(), "ws/sensor.ridl".as_ref());
    assert_eq!(code, 0, "{log}");
    assert_cwd_relative_uris(&log, dir.path(), "ws/sub/gauge.ridl");
}

/// An entry at a subdirectory of the package: the file above the entry and
/// the file inside it share one base, the working directory.
#[test]
fn sarif_uris_are_cwd_relative_from_a_subdirectory_entry() {
    let dir = TempDir::new("sarif-uri-subdir");
    two_level_package(&dir, "gauge.ridl");
    let (code, log) = sarif_check_in(dir.path(), "ws/sub".as_ref());
    assert_eq!(code, 0, "{log}");
    assert_cwd_relative_uris(&log, dir.path(), "ws/sub/gauge.ridl");
}

/// A file name with a space and a `#` is percent-encoded, so the URI is a
/// valid URI reference.
#[test]
fn sarif_uris_percent_encode_a_space_and_a_hash() {
    let dir = TempDir::new("sarif-uri-encode");
    two_level_package(&dir, "b c#1.ridl");
    let (code, log) = sarif_check_in(dir.path(), "ws".as_ref());
    assert_eq!(code, 0, "{log}");
    assert_cwd_relative_uris(&log, dir.path(), "ws/sub/b%20c%231.ridl");
}

/// The rendered note line for a file whose line numbers have one digit
/// (spec §7.2).
const NOTE_LINE: &str = "  = lint: `missing-timing` (set its level in `[lints]` in ridl.toml)";

#[test]
fn text_shows_lint_note() {
    let dir = TempDir::new("note");
    let root = member_workspace(&dir, "");

    let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);

    assert_eq!(code, 0, "{stderr}");
    assert!(
        stderr.lines().any(|line| line == NOTE_LINE),
        "the text report holds the line {NOTE_LINE:?}:\n{stderr}"
    );
}

// The baseline fixtures mirror `baseline_desk.rs`: a standalone package,
// locked, published as a baseline, then edited so that an ordinal moves.

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

/// `doorClosed` moved ahead of `doorOpened`: the ordinals of both shift.
const REORDERED: &str = "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
type DoorState: integer [0..1]
interface VehicleStatus {
  signal currentSpeed: Speed @10ms
  event doorClosed: DoorState @[100ms..1s]
  event doorOpened: DoorState @[100ms..1s]
}
";

/// `BASE` with no timing on `currentSpeed`, which draws RIDL-100.
const BASE_UNTIMED: &str = "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
type DoorState: integer [0..1]
interface VehicleStatus {
  signal currentSpeed: Speed
  event doorOpened: DoorState @[100ms..1s]
  event doorClosed: DoorState @[100ms..1s]
}
";

/// `REORDERED` with no timing on `currentSpeed`.
const REORDERED_UNTIMED: &str = "package veh.cluster
type Speed: km/h [0.0..250.0 step 0.5]
type DoorState: integer [0..1]
interface VehicleStatus {
  signal currentSpeed: Speed
  event doorClosed: DoorState @[100ms..1s]
  event doorOpened: DoorState @[100ms..1s]
}
";

/// A standalone package at the root of `dir`, with `lints` appended to its
/// manifest, holding `source`, locked and published as a baseline. Returns
/// the workspace root and the baseline directory.
fn published_package(dir: &TempDir, lints: &str, source: &str) -> (PathBuf, PathBuf) {
    dir.write(
        "ridl.toml",
        &format!("[package]\nname = \"veh.cluster\"\nversion = \"1.0.0\"\n{lints}"),
    );
    dir.write("cluster.ridl", source);
    let root = dir.path().to_path_buf();
    let (code, _, stderr) = ridl(&["lock".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the fixture's lock is allocated: {stderr}");
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "the baseline is written: {stderr}");
    (root.clone(), root.join(".ridl").join("baseline"))
}

const ALLOW_ORDINAL: &str = "\n[lints]\nordinal-changed = \"allow\"\n";

/// RIDL-407 is raised by the CLI after `ridlc` returns, with its span
/// interned into the source map at that point. The levels are applied once
/// more over the whole run, and the scope lookup finds the interned path.
#[test]
fn ordinal_changed_allow_silences_baseline() {
    let dir = TempDir::new("ordinal-allow");
    let (root, baseline) = published_package(&dir, ALLOW_ORDINAL, BASE);
    dir.write("cluster.ridl", REORDERED);

    let (code, _, stderr) = ridl(&[
        "check".as_ref(),
        "--baseline".as_ref(),
        baseline.as_os_str(),
        root.as_os_str(),
    ]);

    assert_eq!(code, 0, "{stderr}");
    assert!(
        !stderr.contains("RIDL-407"),
        "`ordinal-changed = \"allow\"` silences the desk warning:\n{stderr}"
    );

    // The control: without the `[lints]` table the warning is present.
    let control = TempDir::new("ordinal-default");
    let (root, baseline) = published_package(&control, "", BASE);
    control.write("cluster.ridl", REORDERED);

    let (code, _, stderr) = ridl(&[
        "check".as_ref(),
        "--baseline".as_ref(),
        baseline.as_os_str(),
        root.as_os_str(),
    ]);

    assert_eq!(code, 0, "{stderr}");
    assert!(
        stderr.contains("warning[RIDL-407]"),
        "the default level keeps the desk warning:\n{stderr}"
    );
}

/// The same, with the workspace named by a relative entry (`.`) from its own
/// directory: the scope directory and the path the CLI interns for RIDL-407
/// are both derived from the entry as given, so they share one form.
#[test]
fn ordinal_changed_allow_silences_baseline_from_a_relative_entry() {
    let dir = TempDir::new("ordinal-allow-relative");
    let (root, _) = published_package(&dir, ALLOW_ORDINAL, BASE);
    dir.write("cluster.ridl", REORDERED);

    let (code, _, stderr) = ridl_in(
        Some(&root),
        &[
            "check".as_ref(),
            "--baseline".as_ref(),
            ".ridl/baseline".as_ref(),
            ".".as_ref(),
        ],
    );

    assert_eq!(code, 0, "{stderr}");
    assert!(
        !stderr.contains("RIDL-407"),
        "`ordinal-changed = \"allow\"` silences the desk warning from a relative entry:\n{stderr}"
    );
}

/// A lint at `deny` is an Error once the levels are applied, but it is not a
/// compile error: the desk check still runs, and the run reports both the
/// denied lint and the RIDL-407 (Review Focus 1).
#[test]
fn deny_lint_does_not_skip_desk_check() {
    let dir = TempDir::new("deny-desk");
    let (root, baseline) = published_package(&dir, DENY, BASE_UNTIMED);
    dir.write("cluster.ridl", REORDERED_UNTIMED);

    let (code, _, stderr) = ridl(&[
        "check".as_ref(),
        "--baseline".as_ref(),
        baseline.as_os_str(),
        root.as_os_str(),
    ]);

    assert_eq!(code, 1, "the denied lint moves the exit code:\n{stderr}");
    assert!(
        stderr.contains("error[RIDL-100]"),
        "the denied lint is reported as an error:\n{stderr}"
    );
    assert!(
        stderr.contains("warning[RIDL-407]"),
        "the desk check still runs beside the denied lint:\n{stderr}"
    );
}
