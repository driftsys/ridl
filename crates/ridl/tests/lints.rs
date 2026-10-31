//! The `[lints]` levels as the `ridl` command shows and applies them (ADR-0024
//! decision 6, docs/book/cli-reference.md and docs/book/lints.md): the `lint`
//! field of the JSON report, the lint note of the text report, the exit code of
//! `check` and `build` under `deny`, the commands that do not apply levels
//! (ADR-0024 decision 8), and the `--baseline` path, whose RIDL-407 is raised
//! by the CLI after `ridlc` returns.

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

/// RIDL-416 (`lock-in-subdirectory`) takes its level from `[lints]`: at
/// `deny` the ignored lock is an error and `ridl check` exits 1; at `allow`
/// it is not reported.
#[test]
fn lock_in_subdirectory_takes_its_level_from_the_lints_table() {
    for (lints, expected_code, reported) in [
        ("\n[lints]\nlock-in-subdirectory = \"deny\"\n", 1, true),
        ("\n[lints]\nlock-in-subdirectory = \"allow\"\n", 0, false),
    ] {
        let dir = TempDir::new("lock-in-subdirectory");
        let root = member_workspace(&dir, lints);
        dir.write("sensor/sub/interfaces.lock", "next 1\n");

        let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);
        assert_eq!(code, expected_code, "{lints}stderr:\n{stderr}");
        assert_eq!(
            stderr.contains("error[RIDL-416]"),
            reported,
            "{lints}stderr:\n{stderr}"
        );
        assert!(
            reported || !stderr.contains("RIDL-416"),
            "{lints}stderr:\n{stderr}"
        );
    }
}

/// RIDL-417 (`unsupported-source-file`): `ridl check` on a package directory
/// reports one warning per `.rxdl` file and exits 0; the level comes from
/// `[lints]`, so at `deny` the warning is an error and `ridl check` exits 1,
/// and at `allow` it is not reported.
#[test]
fn unsupported_source_file_takes_its_level_from_the_lints_table() {
    for (lints, expected_code, rendered) in [
        ("", 0, Some("warning[RIDL-417]")),
        (
            "\n[lints]\nunsupported-source-file = \"deny\"\n",
            1,
            Some("error[RIDL-417]"),
        ),
        ("\n[lints]\nunsupported-source-file = \"allow\"\n", 0, None),
    ] {
        let dir = TempDir::new("unsupported-source-file");
        let root = member_workspace(&dir, lints);
        dir.write("sensor/extra.rxdl", "package demo\n");
        dir.write("sensor/sub/more.rxdl", "package demo.sub\n");

        let (code, _, stderr) = ridl(&["check".as_ref(), root.join("sensor").as_os_str()]);
        assert_eq!(code, expected_code, "{lints}stderr:\n{stderr}");
        match rendered {
            Some(header) => assert_eq!(
                stderr.matches(header).count(),
                2,
                "{lints}stderr:\n{stderr}"
            ),
            None => assert!(!stderr.contains("RIDL-417"), "{lints}stderr:\n{stderr}"),
        }
    }
}

/// A lone `.rxdl` or `.rmdl` file, with no `ridl.toml` above it, is a load
/// error in every command that loads source: `ridl check`, `ridl build`,
/// `ridl diff`, `ridl lock`, `ridl test` and `ridl baseline` exit 2 with an
/// error that names the extension as unsupported and report no RIDL-417. For
/// `ridl lock` the exit code and the message are what the test pins; `ridl
/// build` and `ridl baseline` are also checked to write nothing. With a
/// warning instead, `ridl diff` printed
/// `identical` over two different files and `ridl build` wrote nothing and
/// exited 0.
#[test]
fn a_bare_rxdl_or_rmdl_file_is_refused_by_every_command() {
    for extension in ["rxdl", "rmdl"] {
        let dir = TempDir::new("bare-unsupported");
        let old = dir.write(
            &format!("old/hmi.{extension}"),
            "package veh.hmi\ntype B: m\n",
        );
        let new = dir.write(
            &format!("new/hmi.{extension}"),
            "package veh.hmi\ntype C: m\n",
        );
        let out = dir.path().join("out");
        let baseline_out = dir.path().join("baseline-out");
        let commands: [Vec<&OsStr>; 6] = [
            vec!["check".as_ref(), old.as_os_str()],
            vec![
                "build".as_ref(),
                old.as_os_str(),
                "--out-dir".as_ref(),
                out.as_os_str(),
            ],
            vec!["diff".as_ref(), old.as_os_str(), new.as_os_str()],
            vec!["lock".as_ref(), old.as_os_str()],
            vec!["test".as_ref(), old.as_os_str()],
            vec![
                "baseline".as_ref(),
                old.as_os_str(),
                "--out".as_ref(),
                baseline_out.as_os_str(),
            ],
        ];
        for args in commands {
            let (code, stdout, stderr) = ridl(&args);
            let context = format!("{args:?}\nstdout:\n{stdout}\nstderr:\n{stderr}");
            assert_eq!(code, 2, "{context}");
            assert!(
                stderr.contains(&format!(
                    "`.{extension}` is not a supported source file extension"
                )),
                "{context}"
            );
            assert!(!stderr.contains("RIDL-417"), "{context}");
            assert!(!stdout.contains("identical"), "{context}");
        }
        assert!(!out.exists(), "`ridl build` wrote {}", out.display());
        assert!(
            !baseline_out.exists(),
            "`ridl baseline` wrote {}",
            baseline_out.display()
        );

        // A supported old side does not hide an unsupported new side.
        let supported = dir.write("supported/hmi.ridl", "package veh.hmi\ntype B: m\n");
        let (code, stdout, stderr) =
            ridl(&["diff".as_ref(), supported.as_os_str(), new.as_os_str()]);
        let context = format!("stdout:\n{stdout}\nstderr:\n{stderr}");
        assert_eq!(code, 2, "{context}");
        assert!(
            stderr.contains(&format!(
                "`.{extension}` is not a supported source file extension"
            )),
            "{context}"
        );
        assert!(!stdout.contains("identical"), "{context}");
    }
}

/// A workspace with one member whose interface declares one `command`, with
/// the timing annotation `timing` (empty for none), under
/// `missing-response-bound = "deny"`. Returns the workspace root.
fn response_bound_workspace(dir: &TempDir, timing: &str) -> PathBuf {
    dir.write("ridl.toml", "[workspace]\nmembers = [\"actuator\"]\n");
    dir.write(
        "actuator/ridl.toml",
        "[package]\nname = \"demo\"\nversion = \"1.0.0\"\n\n[lints]\n\
         missing-response-bound = \"deny\"\n",
    );
    dir.write(
        "actuator/actuator.ridl",
        &format!(
            "package demo\n\ntype Speed: integer [0..300]\n\ninterface Actuator {{\n  \
             command setTarget(speed: Speed){timing}\n}}\n"
        ),
    );
    dir.path().to_path_buf()
}

/// `missing-response-bound = "deny"` makes the default response bound an
/// error: an unannotated command reports RIDL-112 as an error and exits 1,
/// and the same command with a written bound exits 0.
#[test]
fn missing_response_bound_deny_exits_1_until_the_bound_is_written() {
    let dir = TempDir::new("response-bound-deny");
    let root = response_bound_workspace(&dir, "");
    let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);
    assert_eq!(code, 1, "an untimed command at deny exits 1:\n{stderr}");
    assert!(
        stderr.contains("error[RIDL-112]"),
        "the lint is rendered as an error:\n{stderr}"
    );

    let dir = TempDir::new("response-bound-written");
    let root = response_bound_workspace(&dir, " @[..1s]");
    let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "a written response bound exits 0:\n{stderr}");
    assert!(!stderr.contains("RIDL-112"), "no RIDL-112:\n{stderr}");
}

/// An RPC annotation that keeps a readable minimum but did not parse whole is
/// reported by the parser's FORM-101 alone: the maximum the default supplies
/// draws no RIDL-101 or RIDL-108, although each minimum is longer than the
/// built-in default response bound (`1s` for a command, `3s` for a query).
#[test]
fn unreadable_rpc_range_draws_form_101_and_no_ridl_101_or_108() {
    let dir = TempDir::new("unreadable-rpc-range");
    dir.write(
        "demo/ridl.toml",
        "[package]\nname = \"demo\"\nversion = \"1.0.0\"\n",
    );
    dir.write(
        "demo/actuator.ridl",
        "package demo\n\ntype Speed: integer [0..300]\n\ninterface Actuator {\n  \
         query getSpeed(): Speed @[5s 10s]\n  command setTarget(speed: Speed) @[5s..10xs]\n}\n",
    );
    let (code, _, stderr) = ridl(&["check".as_ref(), dir.path().join("demo").as_os_str()]);
    assert_eq!(code, 1, "FORM-101 is an error:\n{stderr}");
    assert_eq!(
        stderr.matches("error[FORM-101]").count(),
        2,
        "one FORM-101 for each annotation:\n{stderr}"
    );
    assert!(!stderr.contains("RIDL-101"), "no RIDL-101:\n{stderr}");
    assert!(!stderr.contains("RIDL-108"), "no RIDL-108:\n{stderr}");
    assert!(!stderr.contains("RIDL-112"), "no RIDL-112:\n{stderr}");
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

/// `ridl diff` compiles both sides without applying levels (ADR-0024 decision
/// 8): a lint at `deny` on either side does not change its result.
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

/// `ridl baseline` publishes the snapshot without applying levels (ADR-0024
/// decision 8): a lint at `deny` does not block the publication.
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

/// `SOURCE` with one more interaction: the `signal` with no timing
/// annotation stays, and a command with a satisfiable precondition, which
/// `ridl test` samples, is declared beside it.
const SOURCE_WITH_CONTRACT: &str = "package demo\n\ntype Speed: integer [0..300]\n\n\
                                    interface Sensor {\n  signal speed: Speed\n  \
                                    command setRange(min: Speed, max: Speed) [\n    \
                                    require min < max\n  ] @[..50ms]\n}\n";

/// `ridl test` compiles the workspace without applying levels (ADR-0024
/// decision 8): a lint at `deny` does not stop the run, which exits 2 on a
/// compile error.
#[test]
fn test_ignores_deny() {
    let dir = TempDir::new("test-deny");
    dir.write(
        "ridl.toml",
        &format!("[package]\nname = \"demo\"\nversion = \"1.0.0\"\n{DENY}"),
    );
    dir.write("sensor.ridl", SOURCE_WITH_CONTRACT);
    let root = dir.path();
    // The fixture draws the denied lint: `ridl check` exits 1 over it.
    let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);
    assert_eq!(code, 1, "the fixture draws the denied lint:\n{stderr}");

    let (code, stdout, stderr) = ridl(&["test".as_ref(), root.as_os_str()]);

    assert_eq!(
        code, 0,
        "a lint at deny does not change the test run:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("Sensor.setRange.require[0]"),
        "the contract is sampled:\n{stdout}"
    );
}

/// Entering at a workspace member loads its workspace (ADR-0024 decision 9,
/// as ADR-0026 amends it), so the root's `[lints]` applies to the member: the
/// root's `deny` is an exit 1 from the root and from the member alike.
#[test]
fn member_entry_applies_root_lints() {
    let dir = TempDir::new("member-entry");
    dir.write(
        "ridl.toml",
        &format!("[workspace]\nmembers = [\"sensor\"]\n{DENY}"),
    );
    dir.write(
        "sensor/ridl.toml",
        "[package]\nname = \"demo\"\nversion = \"1.0.0\"\n",
    );
    dir.write("sensor/sensor.ridl", SOURCE);
    let root = dir.path();
    let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);
    assert_eq!(code, 1, "from the root, the root's deny applies:\n{stderr}");

    let member = root.join("sensor");
    let (code, _, stderr) = ridl(&["check".as_ref(), member.as_os_str()]);

    assert_eq!(
        code, 1,
        "from the member, the root's deny applies:\n{stderr}"
    );
    assert!(
        stderr.contains("error[RIDL-100]"),
        "the lint is an error at the root's level:\n{stderr}"
    );
}

/// A two-member workspace: `a` imports `Speed` from `b`, and `b` also holds
/// a file with an error. Returns the workspace root.
fn sibling_workspace(dir: &TempDir) -> PathBuf {
    dir.write("ridl.toml", "[workspace]\nmembers = [\"a\", \"b\"]\n");
    dir.write(
        "a/ridl.toml",
        "[package]\nname = \"a\"\nversion = \"1.0.0\"\n",
    );
    dir.write(
        "a/a.typl",
        "package a\nimport b.Speed\nstruct Cabin { primary: Speed }\n",
    );
    dir.write(
        "b/ridl.toml",
        "[package]\nname = \"b\"\nversion = \"1.0.0\"\n",
    );
    dir.write("b/speed.typl", "package b\ntype Speed: integer [0..300]\n");
    dir.write(
        "b/broken.typl",
        "package b\nstruct Broken { field: Missing }\n",
    );
    dir.path().to_path_buf()
}

/// Checking a member reports only the diagnostics of files under the member:
/// the error in member `b` is not shown when checking `a`, and the exit code
/// is 0. From the root, the same error is shown and the exit code is 1.
#[test]
fn member_entry_reports_only_the_member() {
    let dir = TempDir::new("member-scope");
    let root = sibling_workspace(&dir);

    let (code, _, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);
    assert_eq!(code, 1, "from the root, b's error is reported:\n{stderr}");
    assert!(stderr.contains("broken.typl"), "{stderr}");

    let (code, _, stderr) = ridl(&["check".as_ref(), root.join("a").as_os_str()]);
    assert_eq!(code, 0, "from a, b's error is not reported:\n{stderr}");
    assert!(!stderr.contains("broken.typl"), "{stderr}");
}

/// A build entered at a member writes the whole workspace, so an error in
/// another member still blocks every artifact. The build says why and exits
/// 1, without showing the other member's diagnostics.
#[test]
fn member_build_writes_nothing_when_another_member_has_an_error() {
    let dir = TempDir::new("member-build");
    let root = sibling_workspace(&dir);
    let out = TempDir::new("member-build-out");

    let (code, _, stderr) = ridl(&[
        "build".as_ref(),
        "--out-dir".as_ref(),
        out.path().as_os_str(),
        "--emit".as_ref(),
        "ir-json".as_ref(),
        root.join("a").as_os_str(),
    ]);

    assert_eq!(code, 1, "{stderr}");
    assert!(
        stderr.contains("another member of the workspace has an error"),
        "{stderr}"
    );
    assert!(!stderr.contains("broken.typl"), "{stderr}");
    let written: Vec<PathBuf> = std::fs::read_dir(out.path())
        .expect("the out dir is readable")
        .map(|entry| entry.expect("a readable entry").path())
        .collect();
    assert!(written.is_empty(), "the build wrote: {written:?}");
}

/// A build entered at a member, while another member holds an RSDL-7xx
/// error, writes the packages and the system without the blocked deployment
/// (rsdl reference §13). The build exits 1 and says that another member has
/// an error, without showing it.
#[test]
fn member_build_exits_1_when_another_member_leaves_out_a_deployment() {
    let dir = TempDir::new("member-build-rsdl");
    dir.write("ridl.toml", "[workspace]\nmembers = [\"a\", \"demo\"]\n");
    dir.write(
        "a/ridl.toml",
        "[package]\nname = \"a\"\nversion = \"1.0.0\"\n",
    );
    dir.write("a/a.typl", "package a\ntype Level: integer [0..3]\n");
    dir.write(
        "demo/ridl.toml",
        "[package]\nname = \"veh.demo\"\nversion = \"1.0.0\"\n",
    );
    dir.write(
        "demo/lane.ridl",
        "package veh.demo\n\ntype Flag: boolean\n\n\
         interface LaneAssist {\n  signal active: Flag @[100ms..1s]\n}\n\n\
         service veh.demo.lane : LaneAssist\n",
    );
    // `Panel` is not placed in `Bad`: RSDL-701 blocks that deployment only.
    dir.write(
        "demo/topology.rsdl",
        "package veh.demo\n\n\
         component Lane { offers veh.demo.lane }\n\
         component Panel { requires LaneAssist }\n\
         system Vehicle { Lane, Panel }\n\
         deployment Good for Vehicle { machine A { Lane, Panel } }\n\
         deployment Bad for Vehicle { machine A { Lane } }\n",
    );
    let out = TempDir::new("member-build-rsdl-out");

    let (code, _, stderr) = ridl(&[
        "build".as_ref(),
        "--out-dir".as_ref(),
        out.path().as_os_str(),
        "--emit".as_ref(),
        "ir-json".as_ref(),
        dir.path().join("a").as_os_str(),
    ]);

    assert_eq!(code, 1, "{stderr}");
    assert!(
        stderr.contains("another member of the workspace has an error"),
        "{stderr}"
    );
    assert!(!stderr.contains("RSDL-701"), "{stderr}");
    assert!(
        out.path().join("a.ir.json").is_file(),
        "the packages are written"
    );
    let json = std::fs::read_to_string(out.path().join("veh.demo.Vehicle.system.json"))
        .expect("the system is written");
    assert!(json.contains("Good") && !json.contains("\"Bad\""), "{json}");
}

/// A member entry resolves an import of a sibling member, from a relative
/// entry inside the member as well (`ridl check .` from `a`).
#[test]
fn member_entry_resolves_a_sibling_import() {
    let dir = TempDir::new("member-sibling");
    let root = sibling_workspace(&dir);
    std::fs::remove_file(root.join("b/broken.typl")).expect("remove the broken file");

    let (code, _, stderr) = ridl(&["check".as_ref(), root.join("a").as_os_str()]);
    assert_eq!(code, 0, "b.Speed resolves from a:\n{stderr}");

    let (code, _, stderr) = ridl_in(Some(&root.join("a")), &["check".as_ref(), ".".as_ref()]);
    assert_eq!(code, 0, "b.Speed resolves from `.` inside a:\n{stderr}");
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
/// (docs/book/lints.md).
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
const DENY_ORDINAL: &str = "\n[lints]\nordinal-changed = \"deny\"\n";

/// `ordinal-changed = "deny"` reports RIDL-407 as an error, which exits 1
/// (cli-reference, "The baseline desk check").
#[test]
fn ordinal_changed_deny_fails_baseline() {
    let dir = TempDir::new("ordinal-deny");
    let (root, baseline) = published_package(&dir, DENY_ORDINAL, BASE);
    dir.write("cluster.ridl", REORDERED);

    let (code, _, stderr) = ridl(&[
        "check".as_ref(),
        "--baseline".as_ref(),
        baseline.as_os_str(),
        root.as_os_str(),
    ]);

    assert_eq!(
        code, 1,
        "the denied desk warning moves the exit code:\n{stderr}"
    );
    assert!(
        stderr.contains("error[RIDL-407]"),
        "`ordinal-changed = \"deny\"` reports the desk warning as an error:\n{stderr}"
    );
}

/// The entry names one file of the package, and the moved interaction is in
/// another. The compile covers the whole package, so the declaration index
/// must too: the RIDL-407 points at `cluster.ridl`, and the package's
/// `[lints]` scope reaches it. With a detached span the registry default
/// would apply instead, and `deny` would not change the exit code.
#[test]
fn ordinal_changed_deny_applies_through_a_file_entry() {
    let dir = TempDir::new("ordinal-deny-file-entry");
    let other = dir.write(
        "other.ridl",
        "package veh.cluster\ntype Other: integer [0..1]\n",
    );
    let (_, baseline) = published_package(&dir, DENY_ORDINAL, BASE);
    dir.write("cluster.ridl", REORDERED);

    let (code, _, stderr) = ridl(&[
        "check".as_ref(),
        "--baseline".as_ref(),
        baseline.as_os_str(),
        other.as_os_str(),
    ]);

    assert_eq!(
        code, 1,
        "the denied desk warning moves the exit code:\n{stderr}"
    );
    assert!(
        stderr.contains("error[RIDL-407]"),
        "the package's `[lints]` scope reaches the desk warning:\n{stderr}"
    );
    assert!(
        stderr.contains("cluster.ridl:6:3"),
        "the desk warning points at the moved declaration:\n{stderr}"
    );
}

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

#[test]
fn inconsistent_unit_levels_reach_compiler_and_cli_reports() {
    let source = "package a\ntype Speed: km/h [0.0..250.0 step 0.5]\ntype SpeedMs: m/s [0.0..100.0 step 0.5]\nstruct First { speed: Speed }\nstruct Second { speed: Speed }\nstruct Third { speed: SpeedMs }\n";
    for (level, expected_count, expected_severity, expected_exit) in [
        ("info", 1, "info", 0),
        ("allow", 0, "info", 0),
        ("deny", 1, "error", 1),
    ] {
        let dir = TempDir::new("unit-levels");
        dir.write("ridl.toml", &format!("[package]\nname = \"a\"\nversion = \"1.0.0\"\n[lints]\ninconsistent-unit = \"{level}\"\n"));
        dir.write("source.ridl", source);
        // Compilation preserves the catalogue severity for every level.
        let compiled =
            ridlc::compile_workspace(&mut ridl_core::RidlDatabase::default(), dir.path()).unwrap();
        // Preserve the incoming allowance for undocumented fixture items.
        let compiled_diagnostics = compiled
            .diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.code != ridl_core::lint::lint_by_name("missing-docs").unwrap().code
            })
            .collect::<Vec<_>>();
        assert_eq!(compiled_diagnostics.len(), 1, "{:?}", compiled_diagnostics);
        assert_eq!(compiled_diagnostics[0].code.as_str(), "TYPL-222");
        assert_eq!(
            compiled_diagnostics[0].severity,
            ridl_core::diag::Severity::Info
        );

        // The shared command driver applies levels; the binary must agree.
        let run = ridlc::run_check(dir.path(), ridl_core::Frozen::Yes).unwrap();
        // Preserve the incoming allowance for undocumented fixture items.
        let run_diagnostics = run
            .diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.code != ridl_core::lint::lint_by_name("missing-docs").unwrap().code
            })
            .collect::<Vec<_>>();
        assert_eq!(
            run_diagnostics.len(),
            expected_count,
            "{level}: {:?}",
            run_diagnostics
        );
        assert_eq!(run.has_error(), expected_exit == 1);
        if let Some(diagnostic) = run_diagnostics.first() {
            assert_eq!(diagnostic.code.as_str(), "TYPL-222");
            assert_eq!(
                diagnostic.severity,
                if level == "deny" {
                    ridl_core::diag::Severity::Error
                } else {
                    ridl_core::diag::Severity::Info
                }
            );
        }
        let (exit, stdout, stderr) = ridl(&[
            "check".as_ref(),
            "--format".as_ref(),
            "json".as_ref(),
            dir.path().as_os_str(),
        ]);
        assert_eq!(exit, expected_exit, "{level}: {stderr}");
        let diagnostics: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        let diagnostics = diagnostics
            .as_array()
            .unwrap()
            .iter()
            .filter(|diagnostic| diagnostic["lint"] != "missing-docs")
            .collect::<Vec<_>>();
        assert_eq!(
            diagnostics.len(),
            expected_count,
            "{level}: {diagnostics:?}"
        );
        if let Some(diagnostic) = diagnostics.first() {
            assert_eq!(diagnostic["code"], "TYPL-222");
            assert_eq!(diagnostic["lint"], "inconsistent-unit");
            assert_eq!(diagnostic["severity"], expected_severity);
            assert_eq!(
                diagnostic["message"],
                "`speed` uses `m/s` here; elsewhere `speed` uses `km/h`"
            );
        }
    }
}

/// A type documented with a `/** */` block, which draws TYPL-410
/// (`doc-comment-style`, `allow` by default) and nothing else.
const BLOCK_DOC_SOURCE: &str = "package demo\n\n/** A speed. */\ntype Speed: integer [0..300]\n";

/// A workspace with one member whose only file is [`BLOCK_DOC_SOURCE`] and
/// whose manifest ends with `lints`. Returns the workspace root.
fn block_doc_workspace(dir: &TempDir, lints: &str) -> PathBuf {
    dir.write("ridl.toml", "[workspace]\nmembers = [\"speed\"]\n");
    dir.write(
        "speed/ridl.toml",
        &format!("[package]\nname = \"demo\"\nversion = \"1.0.0\"\n{lints}"),
    );
    dir.write("speed/speed.typl", BLOCK_DOC_SOURCE);
    dir.path().to_path_buf()
}

#[test]
fn doc_comment_style_is_silent_by_default() {
    let dir = TempDir::new("doc-style-default");
    let root = block_doc_workspace(&dir, "");

    let (code, stdout, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);

    assert_eq!(code, 0, "stdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(
        !stderr.contains("TYPL-410") && !stdout.contains("TYPL-410"),
        "a lint that is allowed by default is not reported:\n{stderr}"
    );
}

#[test]
fn doc_comment_style_at_warn_is_reported() {
    let dir = TempDir::new("doc-style-warn");
    let root = block_doc_workspace(&dir, "\n[lints]\ndoc-comment-style = \"warn\"\n");

    let (code, stdout, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);

    assert_eq!(code, 0, "stdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(
        stderr.contains("warning[TYPL-410]"),
        "the lint at warn is rendered as a warning:\n{stderr}"
    );
}

#[test]
fn doc_comment_style_at_deny_exits_1() {
    let dir = TempDir::new("doc-style-deny");
    let root = block_doc_workspace(&dir, "\n[lints]\ndoc-comment-style = \"deny\"\n");

    let (code, stdout, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);

    assert_eq!(code, 1, "stdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(
        stderr.contains("error[TYPL-410]"),
        "the lint at deny is rendered as an error:\n{stderr}"
    );
}

/// The commands that apply no `[lints]` levels (ADR-0024 decision 8) still
/// leave out a lint that is `allow` by default: `ridl baseline` and
/// `ridl lock` render the warnings of the compile.
#[test]
fn doc_comment_style_is_silent_in_baseline_and_lock() {
    let dir = TempDir::new("doc-style-baseline");
    let root = block_doc_workspace(&dir, "");

    let (code, stdout, stderr) = ridl(&["lock".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "stdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(
        !stderr.contains("TYPL-410"),
        "`ridl lock` does not report an allow-by-default lint:\n{stderr}"
    );

    let (code, stdout, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "stdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(
        !stderr.contains("TYPL-410"),
        "`ridl baseline` does not report an allow-by-default lint:\n{stderr}"
    );
}

/// `ridl diff` renders the diagnostics of a side that does not compile, with
/// no `[lints]` levels; an allow-by-default lint is left out of them.
#[test]
fn doc_comment_style_is_silent_in_a_diff_compile_error() {
    let dir = TempDir::new("doc-style-diff");
    let old = dir.write("old/speed.typl", BLOCK_DOC_SOURCE);
    let new = dir.write(
        "new/speed.typl",
        &format!("{BLOCK_DOC_SOURCE}type Broken: Missing\n"),
    );

    let (code, stdout, stderr) = ridl(&["diff".as_ref(), old.as_os_str(), new.as_os_str()]);

    assert_eq!(code, 2, "stdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(
        stderr.contains("error"),
        "the new side does not compile:\n{stderr}"
    );
    assert!(
        !stderr.contains("TYPL-410"),
        "`ridl diff` does not report an allow-by-default lint:\n{stderr}"
    );
}

/// A two-member workspace for `missing-docs`: member `a` is documented and
/// imports `Speed` from member `b`, whose declarations have no docs. Returns
/// the workspace root.
fn undocumented_dependency_workspace(dir: &TempDir) -> PathBuf {
    dir.write("ridl.toml", "[workspace]\nmembers = [\"a\", \"b\"]\n");
    dir.write(
        "a/ridl.toml",
        "[package]\nname = \"a\"\nversion = \"1.0.0\"\n",
    );
    dir.write(
        "a/a.typl",
        "package a\nimport b.Speed\n/// The cabin.\nstruct Cabin {\n  /// The primary speed.\n  \
         primary: Speed\n}\n",
    );
    dir.write(
        "b/ridl.toml",
        "[package]\nname = \"b\"\nversion = \"1.0.0\"\n",
    );
    dir.write(
        "b/speed.typl",
        "package b\ntype Speed: integer [0..300]\nstruct Pair {\n  left: Speed\n}\n",
    );
    dir.path().to_path_buf()
}

/// TYPL-406 obeys ADR-0024 decision 10: checking member `a` does not report
/// the undocumented items of member `b`, which is outside the entry's
/// directory tree. From the root, the same items are reported.
#[test]
fn missing_docs_skips_dependencies() {
    let dir = TempDir::new("missing-docs-deps");
    let root = undocumented_dependency_workspace(&dir);

    let (code, stdout, stderr) = ridl(&["check".as_ref(), root.join("a").as_os_str()]);
    assert_eq!(code, 0, "stdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(
        !stderr.contains("TYPL-406") && !stdout.contains("TYPL-406"),
        "the dependency's undocumented items are not reported:\n{stderr}"
    );

    let (code, stdout, stderr) = ridl(&["check".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "stdout:\n{stdout}\nstderr:\n{stderr}");
    assert_eq!(
        stderr.matches("warning[TYPL-406]").count(),
        3,
        "from the root, `Speed`, `Pair` and `left` are reported:\n{stderr}"
    );
}

#[test]
fn missing_docs_at_allow_is_silent() {
    let dir = TempDir::new("missing-docs-allow");
    dir.write("ridl.toml", "[workspace]\nmembers = [\"b\"]\n");
    dir.write(
        "b/ridl.toml",
        "[package]\nname = \"b\"\nversion = \"1.0.0\"\n\n[lints]\nmissing-docs = \"allow\"\n",
    );
    dir.write(
        "b/speed.typl",
        "package b\ntype Speed: integer [0..300]\nstruct Pair {\n  left: Speed\n}\n",
    );

    let (code, stdout, stderr) = ridl(&["check".as_ref(), dir.path().as_os_str()]);

    assert_eq!(code, 0, "stdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(
        !stderr.contains("TYPL-406") && !stdout.contains("TYPL-406"),
        "a lint at allow is not reported:\n{stderr}"
    );
}

// ==========================================================================
// podbuf-padding-order (RIDL-156)
// ==========================================================================

const PODBUF_MANIFEST: &str = "[backend.rust]\nencodings = [\"podbuf\"]\n";
const FLATBUFFERS_MANIFEST: &str = "[backend.rust]\nencodings = [\"flatbuffers\"]\n";

/// `a` is one byte, `b` is eight bytes at alignment 8, `c` is twelve bytes at
/// alignment 4. The declared order places `a` at 0, `b` at 8 and `c` at 16, a
/// head of 32 bytes; the order `b, c, a` is a head of 24 bytes.
const TYPES: &str = "package demo\n\ntype Byte: integer [0..255]\n\
    type Half: integer [0..65535]\n\
    type Word: integer [0..4294967295]\n\
    type Wide: integer [-9223372036854775808..9223372036854775807]\n\n";

const PADDED: &str = "struct Padded {\n  a: Byte\n  b: Wide\n  c: [Word; 3]\n}\n";

/// 18 data bytes in every order, so every order is a head of 24 bytes.
const FAULT_EVENT: &str = "struct FaultEvent {\n  code: Half\n  \
    message: string [0..256]\n  timestamp: Wide\n}\n";

/// A one-member workspace whose root manifest ends with `root_extra` and whose
/// member manifest ends with `member_extra`. Returns the workspace root.
fn podbuf_workspace(dir: &TempDir, source: &str, root_extra: &str, member_extra: &str) -> PathBuf {
    dir.write(
        "ridl.toml",
        &format!("[workspace]\nmembers = [\"demo\"]\n{root_extra}"),
    );
    dir.write(
        "demo/ridl.toml",
        &format!(
            "[package]\nname = \"demo\"\nversion = \"1.0.0\"\n{member_extra}\n\
             [lints]\nmissing-docs = \"allow\"\nno-init-value = \"allow\"\n"
        ),
    );
    dir.write("demo/m.ridl", &format!("{TYPES}{source}"));
    dir.path().to_path_buf()
}

/// The RIDL-156 elements of `ridl check --format json` over `root`, with the
/// exit code.
fn padding_diagnostics(root: &Path, extra: &[&OsStr]) -> (i32, Vec<serde_json::Value>) {
    let mut args: Vec<&OsStr> = vec!["check".as_ref(), "--format".as_ref(), "json".as_ref()];
    args.extend_from_slice(extra);
    args.push(root.as_os_str());
    let (code, stdout, stderr) = ridl(&args);
    let all: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|err| panic!("stdout is JSON ({err}):\n{stdout}\n{stderr}"));
    let found = all
        .as_array()
        .expect("a JSON array")
        .iter()
        .filter(|diagnostic| diagnostic["code"] == "RIDL-156")
        .cloned()
        .collect();
    (code, found)
}

#[test]
fn a_struct_whose_order_wastes_padding_draws_ridl_156_with_both_sizes_and_the_saving_order() {
    let dir = TempDir::new("padding-draws");
    let root = podbuf_workspace(&dir, PADDED, PODBUF_MANIFEST, "");
    let (code, found) = padding_diagnostics(&root, &[]);
    assert_eq!(code, 0, "an info lint does not move the exit code");
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0]["severity"], "info", "{}", found[0]);
    assert_eq!(found[0]["lint"], "podbuf-padding-order", "{}", found[0]);
    let message = found[0]["message"].as_str().expect("a message");
    assert!(message.contains("demo.Padded"), "{message}");
    assert!(message.contains("32 bytes"), "{message}");
    assert!(message.contains("24 bytes"), "{message}");
    assert!(message.contains("b, c, a"), "{message}");
}

#[test]
fn a_struct_that_no_order_improves_draws_nothing() {
    let dir = TempDir::new("padding-fault-event");
    let root = podbuf_workspace(&dir, FAULT_EVENT, PODBUF_MANIFEST, "");
    assert!(padding_diagnostics(&root, &[]).1.is_empty());
}

#[test]
fn the_padding_lint_is_silent_when_no_backend_names_podbuf() {
    let dir = TempDir::new("padding-no-podbuf");
    let root = podbuf_workspace(&dir, PADDED, FLATBUFFERS_MANIFEST, "");
    assert!(padding_diagnostics(&root, &[]).1.is_empty());
    let dir = TempDir::new("padding-no-backend");
    let root = podbuf_workspace(&dir, PADDED, "", "");
    assert!(padding_diagnostics(&root, &[]).1.is_empty());
}

#[test]
fn a_struct_declared_in_the_baseline_draws_nothing() {
    let dir = TempDir::new("padding-released");
    let root = podbuf_workspace(&dir, PADDED, PODBUF_MANIFEST, "");
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(padding_diagnostics(&root, &[]).1.is_empty());
}

#[test]
fn a_struct_absent_from_the_explicit_baseline_still_draws_ridl_156() {
    let dir = TempDir::new("padding-new-struct");
    let root = podbuf_workspace(&dir, PADDED, PODBUF_MANIFEST, "");
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "{stderr}");
    // A second struct, declared after the baseline was published.
    dir.write(
        "demo/m.ridl",
        &format!("{TYPES}{PADDED}\nstruct Fresh {{\n  a: Byte\n  b: Wide\n  c: [Word; 3]\n}}\n"),
    );
    let baseline = root.join(".ridl/baseline");
    let (_, found) = padding_diagnostics(&root, &["--baseline".as_ref(), baseline.as_os_str()]);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0]["message"]
            .as_str()
            .expect("a message")
            .contains("demo.Fresh")
    );
}

#[test]
fn the_padding_lint_takes_its_level_from_the_lints_table() {
    let dir = TempDir::new("padding-levels");
    let root = podbuf_workspace(&dir, PADDED, PODBUF_MANIFEST, "");
    // The member manifest already holds a `[lints]` table; set the level in
    // the root's own table instead.
    dir.write(
        "ridl.toml",
        &format!(
            "[workspace]\nmembers = [\"demo\"]\n{PODBUF_MANIFEST}\n\
             [lints]\npodbuf-padding-order = \"deny\"\n"
        ),
    );
    let (code, found) = padding_diagnostics(&root, &[]);
    assert_eq!(code, 1, "a lint at deny exits 1");
    assert_eq!(found[0]["severity"], "error", "{}", found[0]);
    dir.write(
        "ridl.toml",
        &format!(
            "[workspace]\nmembers = [\"demo\"]\n{PODBUF_MANIFEST}\n\
             [lints]\npodbuf-padding-order = \"allow\"\n"
        ),
    );
    assert!(padding_diagnostics(&root, &[]).1.is_empty());
}

#[test]
fn the_suggested_order_keeps_declaration_order_among_fields_of_equal_alignment() {
    // `c` and `d` both have alignment 4. The declared order is 40 bytes; the
    // saving order `b, c, d, a` is 32 bytes and lists `c` before `d`, as
    // declared.
    let source = "struct Ties {\n  a: Byte\n  b: Wide\n  c: [Word; 3]\n  d: [Word; 2]\n}\n";
    let dir = TempDir::new("padding-ties");
    let root = podbuf_workspace(&dir, source, PODBUF_MANIFEST, "");
    let (_, found) = padding_diagnostics(&root, &[]);
    assert_eq!(found.len(), 1, "{found:?}");
    let message = found[0]["message"].as_str().expect("a message");
    assert!(message.contains("40 bytes"), "{message}");
    assert!(message.contains("32 bytes"), "{message}");
    assert!(message.contains("`b, c, d, a`"), "{message}");
}

#[test]
fn a_struct_released_in_one_package_is_still_reported_in_another_package_of_the_same_name() {
    let dir = TempDir::new("padding-two-packages");
    let member = |name: &str| {
        dir.write(
            &format!("{name}/ridl.toml"),
            &format!(
                "[package]\nname = \"{name}\"\nversion = \"1.0.0\"\n\n\
                 [lints]\nmissing-docs = \"allow\"\n"
            ),
        );
        dir.write(
            &format!("{name}/m.ridl"),
            &format!("package {name}\n\n{}", PADDED_BODY),
        );
    };
    dir.write(
        "ridl.toml",
        &format!("[workspace]\nmembers = [\"x\"]\n{PODBUF_MANIFEST}"),
    );
    member("x");
    let root = dir.path().to_path_buf();
    let (code, _, stderr) = ridl(&["baseline".as_ref(), root.as_os_str()]);
    assert_eq!(code, 0, "{stderr}");
    // Package `y` declares a struct of the same name after the baseline.
    dir.write(
        "ridl.toml",
        &format!("[workspace]\nmembers = [\"x\", \"y\"]\n{PODBUF_MANIFEST}"),
    );
    member("y");
    let (_, found) = padding_diagnostics(&root, &[]);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0]["message"]
            .as_str()
            .expect("a message")
            .contains("`y.Padded`"),
        "{}",
        found[0]
    );
}

/// The types and the struct of [`PADDED`], with no `package` line.
const PADDED_BODY: &str = "type Byte: integer [0..255]\n\
    type Word: integer [0..4294967295]\n\
    type Wide: integer [-9223372036854775808..9223372036854775807]\n\n\
    struct Padded {\n  a: Byte\n  b: Wide\n  c: [Word; 3]\n}\n";

/// `SOURCE` with an inline directive on the `signal` line before it.
fn directive_source(directive: &str) -> String {
    SOURCE.replace("  signal speed", &format!("  {directive}\n  signal speed"))
}

/// The diagnostics of `ridl check --format json` for `root`, with the exit
/// code.
fn check_json(root: &Path) -> (i32, Vec<serde_json::Value>) {
    let (code, stdout, stderr) = ridl(&[
        "check".as_ref(),
        "--format".as_ref(),
        "json".as_ref(),
        root.as_os_str(),
    ]);
    let value: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|err| panic!("stdout is JSON ({err}):\n{stdout}\n{stderr}"));
    (code, value.as_array().expect("a JSON array").clone())
}

fn count_code(diagnostics: &[serde_json::Value], code: &str) -> usize {
    diagnostics.iter().filter(|d| d["code"] == code).count()
}

#[test]
fn directive_allows_a_lint_at_deny_and_check_exits_0() {
    let dir = TempDir::new("directive-allows-deny");
    let root = member_workspace(&dir, DENY);
    dir.write(
        "sensor/sensor.ridl",
        &directive_source("// lint: allow missing-timing -- timing comes later"),
    );

    let (code, diagnostics) = check_json(&root);

    assert_eq!(
        code, 0,
        "the directive allows the denied lint: {diagnostics:?}"
    );
    assert_eq!(count_code(&diagnostics, "RIDL-100"), 0, "{diagnostics:?}");
    assert_eq!(count_code(&diagnostics, "TYPL-411"), 0, "{diagnostics:?}");
}

#[test]
fn unused_directive_reports_typl_411_in_check_json() {
    let dir = TempDir::new("directive-unused");
    let root = member_workspace(&dir, "");
    dir.write(
        "sensor/sensor.ridl",
        &directive_source("// lint: allow unbounded-float -- nothing to allow"),
    );

    let (code, diagnostics) = check_json(&root);

    assert_eq!(code, 0, "{diagnostics:?}");
    assert_eq!(count_code(&diagnostics, "TYPL-411"), 1, "{diagnostics:?}");
}

#[test]
fn malformed_directive_reports_typl_412_and_allows_nothing() {
    let dir = TempDir::new("directive-malformed");
    let root = member_workspace(&dir, "");
    dir.write(
        "sensor/sensor.ridl",
        &directive_source("// lint: allow missing-timing"),
    );

    let (_, diagnostics) = check_json(&root);

    assert_eq!(count_code(&diagnostics, "TYPL-412"), 1, "{diagnostics:?}");
    assert_eq!(count_code(&diagnostics, "RIDL-100"), 1, "{diagnostics:?}");
}

#[test]
fn baseline_desk_check_reports_a_directive_once() {
    let dir = TempDir::new("directive-baseline-once");
    let (root, baseline) = published_package(&dir, "", BASE);
    dir.write(
        "cluster.ridl",
        &REORDERED.replacen(
            "type Speed",
            "// lint: allow unbounded-float -- nothing to allow\ntype Speed",
            1,
        ),
    );

    let (_, stderr_json, stderr) = ridl(&[
        "check".as_ref(),
        "--format".as_ref(),
        "json".as_ref(),
        "--baseline".as_ref(),
        baseline.as_os_str(),
        root.as_os_str(),
    ]);

    let value: serde_json::Value = serde_json::from_str(&stderr_json)
        .unwrap_or_else(|err| panic!("stdout is JSON ({err}):\n{stderr_json}\n{stderr}"));
    let diagnostics = value.as_array().expect("a JSON array");
    assert_eq!(count_code(diagnostics, "TYPL-411"), 1, "{diagnostics:?}");
}

#[test]
fn build_applies_directives() {
    let dir = TempDir::new("directive-build");
    let root = member_workspace(&dir, DENY);
    dir.write(
        "sensor/sensor.ridl",
        &directive_source("// lint: allow missing-timing -- timing comes later"),
    );
    let out = TempDir::new("directive-build-out");

    let (code, _, stderr) = ridl(&[
        "build".as_ref(),
        "--out-dir".as_ref(),
        out.path().as_os_str(),
        "--emit".as_ref(),
        "ir-json".as_ref(),
        root.as_os_str(),
    ]);

    assert_eq!(code, 0, "the directive lets the build run:\n{stderr}");
    let written = std::fs::read_dir(out.path()).expect("readable").count();
    assert!(written > 0, "the build wrote its artifacts");
}

/// `ridl diff` applies no levels (ADR-0024 decision 8), so it reads no
/// directive: a lint that a directive allows is still shown beside the
/// compile error that makes the diff print diagnostics.
#[test]
fn diff_ignores_directives() {
    let old = TempDir::new("directive-diff-old");
    let old_root = member_workspace(&old, "");
    let new = TempDir::new("directive-diff-new");
    let new_root = member_workspace(&new, "");
    new.write(
        "sensor/sensor.ridl",
        &format!(
            "{}type Broken: Missing\n",
            directive_source("// lint: allow missing-timing -- timing comes later")
        ),
    );

    let (code, stdout, stderr) =
        ridl(&["diff".as_ref(), old_root.as_os_str(), new_root.as_os_str()]);

    assert_eq!(code, 2, "stdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(
        stderr.contains("RIDL-100"),
        "the allowed lint is still reported by diff:\n{stderr}"
    );
    assert!(
        !stderr.contains("TYPL-411"),
        "diff reports no directive:\n{stderr}"
    );
}

#[test]
fn directive_on_ordinal_changed_is_used_after_the_desk_check() {
    let dir = TempDir::new("directive-ordinal-used");
    let (root, baseline) = published_package(&dir, "", BASE);
    dir.write(
        "cluster.ridl",
        &REORDERED.replacen(
            "interface VehicleStatus",
            "// lint: allow ordinal-changed -- the order is intended\ninterface VehicleStatus",
            1,
        ),
    );

    let (_, stdout, stderr) = ridl(&[
        "check".as_ref(),
        "--format".as_ref(),
        "json".as_ref(),
        "--baseline".as_ref(),
        baseline.as_os_str(),
        root.as_os_str(),
    ]);

    let value: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|err| panic!("stdout is JSON ({err}):\n{stdout}\n{stderr}"));
    let diagnostics = value.as_array().expect("a JSON array");
    assert_eq!(count_code(diagnostics, "RIDL-407"), 0, "{diagnostics:?}");
    assert_eq!(count_code(diagnostics, "TYPL-411"), 0, "{diagnostics:?}");
}

#[test]
fn directive_on_ordinal_changed_is_not_unused_without_a_baseline() {
    let dir = TempDir::new("directive-ordinal-no-baseline");
    dir.write(
        "ridl.toml",
        "[package]\nname = \"veh.cluster\"\nversion = \"1.0.0\"\n",
    );
    dir.write(
        "cluster.ridl",
        &REORDERED.replacen(
            "interface VehicleStatus",
            "// lint: allow ordinal-changed -- the order is intended\ninterface VehicleStatus",
            1,
        ),
    );

    let (_, diagnostics) = check_json(dir.path());

    assert_eq!(count_code(&diagnostics, "TYPL-411"), 0, "{diagnostics:?}");
}

#[test]
fn directive_on_podbuf_padding_order_is_used_by_check() {
    let dir = TempDir::new("directive-padding-used");
    let root = podbuf_workspace(
        &dir,
        &format!("// lint: allow podbuf-padding-order -- the order is intended\n{PADDED}"),
        PODBUF_MANIFEST,
        "",
    );

    let (code, diagnostics) = check_json(&root);

    assert_eq!(code, 0, "{diagnostics:?}");
    assert_eq!(count_code(&diagnostics, "RIDL-156"), 0, "{diagnostics:?}");
    assert_eq!(count_code(&diagnostics, "TYPL-411"), 0, "{diagnostics:?}");
}

#[test]
fn directive_on_podbuf_padding_order_is_not_unused_in_build() {
    let dir = TempDir::new("directive-padding-build");
    let root = podbuf_workspace(
        &dir,
        &format!("// lint: allow podbuf-padding-order -- the order is intended\n{PADDED}"),
        &format!("{PODBUF_MANIFEST}\n[lints]\nunused-lint-directive = \"deny\"\n"),
        "",
    );
    let out = TempDir::new("directive-padding-build-out");

    let (code, _, stderr) = ridl(&[
        "build".as_ref(),
        "--out-dir".as_ref(),
        out.path().as_os_str(),
        "--emit".as_ref(),
        "ir-json".as_ref(),
        root.as_os_str(),
    ]);

    assert_eq!(code, 0, "{stderr}");
    assert!(!stderr.contains("TYPL-411"), "{stderr}");
}

#[test]
fn build_reports_an_unused_directive() {
    let dir = TempDir::new("directive-build-unused");
    let root = member_workspace(&dir, "");
    dir.write(
        "sensor/sensor.ridl",
        &directive_source("// lint: allow unbounded-float -- nothing to allow"),
    );
    let out = TempDir::new("directive-build-unused-out");

    let (code, _, stderr) = ridl(&[
        "build".as_ref(),
        "--out-dir".as_ref(),
        out.path().as_os_str(),
        "--emit".as_ref(),
        "ir-json".as_ref(),
        root.as_os_str(),
    ]);

    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stderr.matches("[TYPL-411]").count(), 1, "{stderr}");
}

#[test]
fn a_sibling_members_unused_directive_fails_a_member_build_at_deny() {
    let dir = TempDir::new("directive-sibling-build");
    let root = sibling_workspace(&dir);
    dir.write(
        "ridl.toml",
        "[workspace]\nmembers = [\"a\", \"b\"]\n\n[lints]\nunused-lint-directive = \"deny\"\n",
    );
    std::fs::remove_file(dir.path().join("b/broken.typl")).expect("the broken file is removed");
    dir.write(
        "b/speed.typl",
        "package b\n// lint: allow unbounded-float -- nothing to allow\ntype Speed: integer [0..300]\n",
    );
    let out = TempDir::new("directive-sibling-build-out");

    let (code, _, stderr) = ridl(&[
        "build".as_ref(),
        "--out-dir".as_ref(),
        out.path().as_os_str(),
        "--emit".as_ref(),
        "ir-json".as_ref(),
        root.join("a").as_os_str(),
    ]);

    assert_eq!(code, 1, "{stderr}");
    assert!(
        !stderr.contains("speed.typl"),
        "the sibling is not printed:\n{stderr}"
    );
}

/// A sibling member's directive is checked against the sibling's own
/// diagnostics: used there, it draws no TYPL-411, so a member build at
/// `unused-lint-directive = "deny"` succeeds.
#[test]
fn a_sibling_members_used_directive_passes_a_member_build_at_deny() {
    let dir = TempDir::new("directive-sibling-used-build");
    let root = sibling_workspace(&dir);
    dir.write(
        "ridl.toml",
        "[workspace]\nmembers = [\"a\", \"b\"]\n\n[lints]\nunused-lint-directive = \"deny\"\n",
    );
    std::fs::remove_file(dir.path().join("b/broken.typl")).expect("the broken file is removed");
    dir.write(
        "b/speed.typl",
        "package b\n// lint: allow unbounded-float -- legacy\ntype Speed: float\n",
    );
    let out = TempDir::new("directive-sibling-used-build-out");

    let (code, _, stderr) = ridl(&[
        "build".as_ref(),
        "--out-dir".as_ref(),
        out.path().as_os_str(),
        "--emit".as_ref(),
        "ir-json".as_ref(),
        root.join("a").as_os_str(),
    ]);

    assert_eq!(code, 0, "{stderr}");
    assert!(
        !stderr.contains("TYPL-411") && !stderr.contains("another member"),
        "the sibling's directive is used by the sibling's TYPL-102:\n{stderr}"
    );
}

/// A lint that is `allow` by default still counts as raised: a directive
/// that names `doc-comment-style` (TYPL-410, `allow` by default) over a
/// `/** */` doc comment is used, and the lint stays silent.
#[test]
fn a_directive_for_a_default_allow_lint_is_used() {
    let dir = TempDir::new("directive-default-allow");
    let root = member_workspace(&dir, "");
    dir.write(
        "sensor/sensor.ridl",
        &SOURCE.replace(
            "type Speed",
            "// lint: allow doc-comment-style -- legacy\n/** A speed. */\ntype Speed",
        ),
    );

    let (code, diagnostics) = check_json(&root);

    assert_eq!(code, 0, "{diagnostics:?}");
    assert_eq!(count_code(&diagnostics, "TYPL-411"), 0, "{diagnostics:?}");
    assert_eq!(count_code(&diagnostics, "TYPL-410"), 0, "{diagnostics:?}");
}

/// A lint at `allow` in `[lints]` still counts as raised: a directive that
/// names it is used, not unused.
#[test]
fn a_directive_for_a_lint_at_allow_is_used() {
    let dir = TempDir::new("directive-allow-level");
    let root = member_workspace(&dir, ALLOW);
    dir.write(
        "sensor/sensor.ridl",
        &directive_source("// lint: allow missing-timing -- timing comes later"),
    );

    let (code, diagnostics) = check_json(&root);

    assert_eq!(code, 0, "{diagnostics:?}");
    assert_eq!(count_code(&diagnostics, "TYPL-411"), 0, "{diagnostics:?}");
    assert_eq!(count_code(&diagnostics, "RIDL-100"), 0, "{diagnostics:?}");
}

#[test]
fn unused_directive_on_podbuf_padding_order_is_reported_when_the_check_ran() {
    let dir = TempDir::new("directive-padding-unused");
    let root = podbuf_workspace(
        &dir,
        &format!("// lint: allow podbuf-padding-order -- nothing to allow\n{FAULT_EVENT}"),
        PODBUF_MANIFEST,
        "",
    );

    let (_, diagnostics) = check_json(&root);

    assert_eq!(count_code(&diagnostics, "TYPL-411"), 1, "{diagnostics:?}");
}

#[test]
fn unused_directive_on_ordinal_changed_is_reported_when_the_desk_check_ran() {
    let dir = TempDir::new("directive-ordinal-unused");
    let (root, baseline) = published_package(&dir, "", BASE);
    dir.write(
        "cluster.ridl",
        &BASE.replacen(
            "interface VehicleStatus",
            "// lint: allow ordinal-changed -- nothing moved\ninterface VehicleStatus",
            1,
        ),
    );

    let (_, stdout, stderr) = ridl(&[
        "check".as_ref(),
        "--format".as_ref(),
        "json".as_ref(),
        "--baseline".as_ref(),
        baseline.as_os_str(),
        root.as_os_str(),
    ]);

    let value: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|err| panic!("stdout is JSON ({err}):\n{stdout}\n{stderr}"));
    let diagnostics = value.as_array().expect("a JSON array");
    assert_eq!(count_code(diagnostics, "TYPL-411"), 1, "{diagnostics:?}");
}

#[test]
fn directive_on_podbuf_padding_order_is_not_unused_when_podbuf_is_off() {
    let dir = TempDir::new("directive-padding-podbuf-off");
    let root = podbuf_workspace(
        &dir,
        &format!("// lint: allow podbuf-padding-order -- the order is intended\n{PADDED}"),
        FLATBUFFERS_MANIFEST,
        "",
    );

    let (_, diagnostics) = check_json(&root);

    assert_eq!(count_code(&diagnostics, "TYPL-411"), 0, "{diagnostics:?}");
}

/// An error in another member stops the padding check, which runs only when
/// the compile raised nothing but RIDL-409, although a member entry does not
/// report that error. The directive is not unused.
#[test]
fn directive_on_podbuf_padding_order_is_not_unused_when_the_padding_check_is_skipped() {
    let dir = TempDir::new("directive-padding-skipped");
    podbuf_workspace(
        &dir,
        &format!("// lint: allow podbuf-padding-order -- the order is intended\n{PADDED}"),
        PODBUF_MANIFEST,
        "",
    );
    dir.write(
        "ridl.toml",
        &format!("[workspace]\nmembers = [\"demo\", \"other\"]\n{PODBUF_MANIFEST}"),
    );
    dir.write(
        "other/ridl.toml",
        "[package]\nname = \"other\"\nversion = \"1.0.0\"\n",
    );
    dir.write("other/broken.typl", "package other\ntype Broken: Missing\n");

    let (code, diagnostics) = check_json(&dir.path().join("demo"));

    assert_eq!(code, 0, "{diagnostics:?}");
    assert_eq!(count_code(&diagnostics, "RIDL-156"), 0, "{diagnostics:?}");
    assert_eq!(count_code(&diagnostics, "TYPL-411"), 0, "{diagnostics:?}");
}
