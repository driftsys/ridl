//! A command that the running release refuses (MANI-026) is handed to a
//! compatible installed release. The installed releases here are shell scripts
//! that record what they were given.
#![cfg(unix)]

use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};
use tempfile::TempDir;

#[allow(dead_code)]
mod toolchain_support;

const BIN: &str = env!("CARGO_BIN_EXE_ridl");

/// The shell script that records what it receives, as a release binary.
fn recording_script(version: &str) -> String {
    format!(
        "#!/bin/sh\n\
         r=\"$RIDL_TEST_RECORD\"\n\
         printf '%s\\n' \"{version}\" > \"$r/version\"\n\
         for a in \"$@\"; do printf '%s\\0' \"$a\"; done > \"$r/argv\"\n\
         pwd -P > \"$r/cwd\"\n\
         cat > \"$r/stdin\"\n\
         printf '%s' \"${{RIDL_TOOLCHAIN_HANDOFF-unset}}\" > \"$r/guard\"\n\
         echo \"child stdout\"; echo \"child stderr\" >&2\n\
         exit 7\n"
    )
}

/// Installs a complete release whose binary records what it receives.
fn fake_release(root: &Path, version: &str) -> PathBuf {
    let dir = root.join("versions").join(version);
    fs::create_dir_all(&dir).unwrap();
    let binary = dir.join("ridl");
    fs::write(&binary, recording_script(version)).unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(dir.join("LICENSE"), b"license").unwrap();
    fs::write(dir.join("THIRD-PARTY-NOTICES.txt"), b"notices").unwrap();
    fs::write(dir.join(".complete"), b"").unwrap();
    binary.canonicalize().unwrap()
}

/// A release directory that is not usable: the completion marker is absent.
fn damaged_release(root: &Path, version: &str) {
    fake_release(root, version);
    fs::remove_file(root.join("versions").join(version).join(".complete")).unwrap();
}

/// A workspace pinned to `pin` whose lockfiles and imports would fail or fetch
/// if any later stage ran.
fn workspace(dir: &Path, pin: &str) {
    fs::create_dir_all(dir.join("out")).unwrap();
    fs::write(
        dir.join("ridl.toml"),
        format!(
            "[package]\nname = \"p\"\nversion = \"1.0.0\"\nridl-version = \"{pin}\"\n\
             [imports]\n\"remote.unit\" = \"file:///nonexistent/ridl-handoff-fixture\"\n"
        ),
    )
    .unwrap();
    fs::write(dir.join("a.typl"), "package p\ntype A:integer[0..1]\n").unwrap();
    fs::write(dir.join("instances.lock"), "broken [ TOML").unwrap();
    fs::write(dir.join("ridl.lock"), "broken [ TOML").unwrap();
    fs::write(dir.join("out/sentinel"), "retain output").unwrap();
}

fn tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, path: &Path, result: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            let is_dir = path.is_dir();
            result.insert(
                path.strip_prefix(root).unwrap().to_path_buf(),
                if is_dir {
                    vec![]
                } else {
                    fs::read(&path).unwrap()
                },
            );
            if is_dir {
                walk(root, &path, result);
            }
        }
    }
    let mut result = BTreeMap::new();
    walk(root, root, &mut result);
    result
}

/// One process run with an isolated home, toolchain root and record directory.
struct Fixture {
    base: TempDir,
}
impl Fixture {
    fn new() -> Self {
        let base = TempDir::new().unwrap();
        for name in ["home", "toolchain", "record"] {
            fs::create_dir(base.path().join(name)).unwrap();
        }
        Self { base }
    }
    fn home(&self) -> PathBuf {
        self.base.path().join("home")
    }
    fn toolchain(&self) -> PathBuf {
        self.base.path().join("toolchain")
    }
    fn record(&self) -> PathBuf {
        self.base.path().join("record")
    }
    fn recorded(&self, name: &str) -> Vec<u8> {
        fs::read(self.record().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
    }
    /// Runs `ridl` in `cwd`, feeding a probe to standard input.
    fn run(&self, cwd: &Path, toolchain: &Path, args: &[&str], envs: &[(&str, &str)]) -> Output {
        self.run_with_stdin(cwd, toolchain, args, envs, b"stdin probe\n")
    }
    /// Runs `ridl` in `cwd`, feeding `input` to standard input.
    fn run_with_stdin(
        &self,
        cwd: &Path,
        toolchain: &Path,
        args: &[&str],
        envs: &[(&str, &str)],
        input: &[u8],
    ) -> Output {
        let mut command = Command::new(BIN);
        command
            .current_dir(cwd)
            .args(args)
            .env("RIDL_TEST_RUNNING_RELEASE", "0.22.5")
            .env("RIDL_TOOLCHAIN_DIR", toolchain)
            .env("RIDL_TEST_RECORD", self.record())
            .env("HOME", self.home())
            .env_remove("RIDL_TOOLCHAIN_HANDOFF")
            .env_remove("RIDL_TOOLCHAIN_AUTO_INSTALL")
            .env_remove("RIDL_TEST_RELEASE_ORIGIN")
            .env_remove("RIDL_TEST_RELEASE_PUBLIC_KEY")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in envs {
            command.env(key, value);
        }
        let mut child = command.spawn().unwrap();
        let mut stdin = child.stdin.take().unwrap();
        // The refused run never reads standard input; a closed pipe is fine.
        let _ = stdin.write_all(input);
        drop(stdin);
        child.wait_with_output().unwrap()
    }
    fn record_is_empty(&self) -> bool {
        fs::read_dir(self.record()).unwrap().next().is_none()
    }
    fn home_is_empty(&self) -> bool {
        fs::read_dir(self.home()).unwrap().next().is_none()
    }
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
fn assert_refusal(output: &Output, code: &str) {
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let text = stderr(output);
    let codes: Vec<_> = text.lines().filter(|l| l.starts_with("error[")).collect();
    assert_eq!(codes.len(), 1, "{output:?}");
    assert!(codes[0].contains(code), "{output:?}");
}
fn argv(fixture: &Fixture) -> Vec<String> {
    fixture
        .recorded("argv")
        .split(|b| *b == 0)
        .filter(|a| !a.is_empty())
        .map(|a| String::from_utf8(a.to_vec()).unwrap())
        .collect()
}

#[test]
fn handoff_runs_the_selected_release_for_every_enforcing_command() {
    let commands: &[&[&str]] = &[
        &["check", "."],
        &["check", ".", "--format", "json"],
        &["build", ".", "--out-dir", "out"],
        &["fmt", "."],
        &["fmt", ".", "--check"],
        &["lock", "."],
        &["baseline", ".", "--out", "out"],
        &["test", "."],
        &["vendor", "."],
        &["vendor", ".", "--check"],
        &["diff", ".", "a.typl"],
        &["diff", "a.typl", "."],
        &["describe", "a.typl"],
    ];
    for args in commands {
        let fixture = Fixture::new();
        let ws = fixture.base.path().join("ws");
        workspace(&ws, "0.23.1");
        fake_release(&fixture.toolchain(), "0.23.4");
        let before = tree(&ws);
        let toolchain_before = tree(&fixture.toolchain());
        let out = fixture.run(&ws, &fixture.toolchain(), args, &[]);
        assert_eq!(out.status.code(), Some(7), "{args:?} {out:?}");
        assert_eq!(out.stdout, b"child stdout\n", "{args:?}");
        assert_eq!(out.stderr, b"child stderr\n", "{args:?}");
        assert_eq!(argv(&fixture), *args, "{args:?}");
        assert_eq!(
            fixture.recorded("cwd"),
            format!("{}\n", ws.canonicalize().unwrap().display()).into_bytes(),
            "{args:?}"
        );
        assert_eq!(fixture.recorded("stdin"), b"stdin probe\n", "{args:?}");
        assert_eq!(fixture.recorded("guard"), b"0.22.5", "{args:?}");
        assert_eq!(tree(&ws), before, "{args:?}");
        assert_eq!(tree(&fixture.toolchain()), toolchain_before, "{args:?}");
        assert!(fixture.home_is_empty(), "{args:?}");
    }
}

#[test]
fn handoff_runs_describe_for_a_missing_descriptor() {
    let fixture = Fixture::new();
    let ws = fixture.base.path().join("ws");
    workspace(&ws, "0.23.1");
    fake_release(&fixture.toolchain(), "0.23.4");
    let args = ["describe", "missing.catalog.binfb"];
    let out = fixture.run(&ws, &fixture.toolchain(), &args, &[]);
    assert_eq!(out.status.code(), Some(7), "{out:?}");
    assert_eq!(out.stderr, b"child stderr\n");
    assert_eq!(argv(&fixture), args);
}

#[test]
fn handoff_skips_under_the_guard() {
    let fixture = Fixture::new();
    let ws = fixture.base.path().join("ws");
    workspace(&ws, "0.23.1");
    fake_release(&fixture.toolchain(), "0.23.4");
    let out = fixture.run(
        &ws,
        &fixture.toolchain(),
        &["check", "."],
        &[("RIDL_TOOLCHAIN_HANDOFF", "0.23.4")],
    );
    assert_refusal(&out, "MANI-026");
    assert!(fixture.record_is_empty());
}

#[test]
fn handoff_selects_the_highest_release_compatible_with_every_pin() {
    let fixture = Fixture::new();
    let parent = fixture.base.path().join("parent");
    fs::create_dir(&parent).unwrap();
    workspace(&parent.join("a"), "0.23.1");
    workspace(&parent.join("b"), "0.23.4");
    let root = fixture.toolchain();
    for version in ["0.23.3", "0.23.6", "0.24.0"] {
        fake_release(&root, version);
    }
    damaged_release(&root, "0.23.9");
    let out = fixture.run(&parent, &root, &["fmt", "."], &[]);
    assert_eq!(out.status.code(), Some(7), "{out:?}");
    assert_eq!(fixture.recorded("version"), b"0.23.6\n");

    // Only 0.23.3 is installed here, which satisfies the first pin alone.
    let second = Fixture::new();
    let only_old = second.base.path().join("only-old");
    fake_release(&only_old, "0.23.3");
    let out2 = second.run(&parent, &only_old, &["fmt", "."], &[]);
    assert_eq!(out2.status.code(), Some(2), "{out2:?}");
    assert_eq!(stderr(&out2).matches("error[MANI-026]").count(), 2);
    assert!(second.record_is_empty());
}

#[test]
fn handoff_is_skipped_when_any_refusal_is_not_mani_026() {
    let fixture = Fixture::new();
    let parent = fixture.base.path().join("parent");
    fs::create_dir(&parent).unwrap();
    workspace(&parent.join("a"), "42");
    workspace(&parent.join("b"), "0.23.1");
    fake_release(&fixture.toolchain(), "0.23.4");
    let out = fixture.run(&parent, &fixture.toolchain(), &["fmt", "."], &[]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(stderr(&out).contains("error[MANI-024]"));
    assert!(stderr(&out).contains("error[MANI-026]"));
    assert!(fixture.record_is_empty());
}

#[test]
fn handoff_preserves_arguments_with_spaces_and_a_root_with_spaces() {
    let fixture = Fixture::new();
    let ws = fixture.base.path().join("work space");
    workspace(&ws, "0.23.1");
    fs::create_dir(ws.join("src dir")).unwrap();
    fs::write(
        ws.join("src dir/a.typl"),
        "package p\ntype B:integer[0..1]\n",
    )
    .unwrap();
    let root = fixture.base.path().join("tool root");
    fake_release(&root, "0.23.4");
    let args = ["check", "src dir", "--format", "json"];
    let out = fixture.run(&ws, &root, &args, &[]);
    assert_eq!(out.status.code(), Some(7), "{out:?}");
    assert_eq!(argv(&fixture), args);
}

#[test]
fn handoff_reports_an_exec_failure() {
    let fixture = Fixture::new();
    let ws = fixture.base.path().join("ws");
    workspace(&ws, "0.23.1");
    let binary = fake_release(&fixture.toolchain(), "0.23.4");
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o644)).unwrap();
    let out = fixture.run(&ws, &fixture.toolchain(), &["check", "."], &[]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(
        stderr(&out).starts_with(&format!(
            "error: could not run ridl 0.23.4 at {}: ",
            binary.display()
        )),
        "{out:?}"
    );
    assert_eq!(
        stderr(&out).lines().count(),
        1,
        "no refusal follows: {out:?}"
    );
    assert!(fixture.record_is_empty());
}

use ridl_cli::toolchain::BUILD_TARGET;

/// A release origin that serves a signed `version` whose binary records what
/// it receives. The result is the directory that holds the origin.
fn origin_with(base: &Path, versions: &[&str]) -> PathBuf {
    let origin = base.join("origin");
    fs::create_dir_all(&origin).unwrap();
    for version in versions {
        let script = recording_script(version).into_bytes();
        toolchain_support::fixture_for(
            &origin,
            version,
            BUILD_TARGET,
            &[
                ("ridl", b'0', &script),
                ("LICENSE", b'0', b"license"),
                ("THIRD-PARTY-NOTICES.txt", b'0', b"notices"),
            ],
        );
    }
    origin
}

/// The environment that points the installer at `origin` with the test key.
fn origin_env(origin: &Path) -> [(&'static str, String); 2] {
    [
        (
            "RIDL_TEST_RELEASE_ORIGIN",
            format!("file://{}", origin.display()),
        ),
        (
            "RIDL_TEST_RELEASE_PUBLIC_KEY",
            toolchain_support::test_public_key(),
        ),
    ]
}

/// Runs with the origin variables and `extra` variables.
fn run_with_origin(
    fixture: &Fixture,
    cwd: &Path,
    origin: &Path,
    args: &[&str],
    extra: &[(&str, &str)],
    input: &[u8],
) -> Output {
    let origin_vars = origin_env(origin);
    let mut envs: Vec<(&str, &str)> = origin_vars.iter().map(|(k, v)| (*k, v.as_str())).collect();
    envs.extend_from_slice(extra);
    fixture.run_with_stdin(cwd, &fixture.toolchain(), args, &envs, input)
}

fn no_versions(root: &Path) -> bool {
    fs::read_dir(root.join("versions")).map_or(true, |mut entries| entries.next().is_none())
}

#[test]
fn mcp_hands_off_before_reading_standard_input() {
    let fixture = Fixture::new();
    let ws = fixture.base.path().join("ws");
    workspace(&ws, "0.23.1");
    fake_release(&fixture.toolchain(), "0.23.4");
    let line = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}\n";
    let out = fixture.run_with_stdin(&ws, &fixture.toolchain(), &["mcp"], &[], line.as_bytes());
    assert_eq!(out.status.code(), Some(7), "{out:?}");
    assert_eq!(argv(&fixture), ["mcp"]);
    assert_eq!(fixture.recorded("stdin"), line.as_bytes());
}

#[test]
fn mcp_hands_off_from_a_subdirectory_of_the_workspace() {
    let fixture = Fixture::new();
    let ws = fixture.base.path().join("ws");
    workspace(&ws, "0.23.1");
    let sub = ws.join("out");
    fake_release(&fixture.toolchain(), "0.23.4");
    let out = fixture.run_with_stdin(&sub, &fixture.toolchain(), &["mcp"], &[], b"probe\n");
    assert_eq!(out.status.code(), Some(7), "{out:?}");
    assert_eq!(argv(&fixture), ["mcp"]);
    assert_eq!(fixture.recorded("stdin"), b"probe\n");
}

#[test]
fn mcp_never_installs_and_respects_the_guard() {
    let a = Fixture::new();
    let ws = a.base.path().join("ws");
    workspace(&ws, "0.23.1");
    let origin = origin_with(a.base.path(), &["0.23.1"]);
    let out = run_with_origin(
        &a,
        &ws,
        &origin,
        &["mcp"],
        &[("RIDL_TOOLCHAIN_AUTO_INSTALL", "1")],
        b"",
    );
    assert!(!a.toolchain().join("versions/0.23.1").exists(), "{out:?}");
    assert!(a.record_is_empty(), "{out:?}");

    let b = Fixture::new();
    let ws = b.base.path().join("ws");
    workspace(&ws, "0.23.1");
    fake_release(&b.toolchain(), "0.23.4");
    let out = b.run_with_stdin(
        &ws,
        &b.toolchain(),
        &["mcp"],
        &[("RIDL_TOOLCHAIN_HANDOFF", "0.23.4")],
        b"",
    );
    assert!(b.record_is_empty(), "{out:?}");
}

#[test]
fn auto_install_is_off_unless_the_variable_is_exactly_one() {
    for value in [
        None,
        Some(""),
        Some("0"),
        Some("true"),
        Some(" 1"),
        Some("1 "),
        Some("11"),
    ] {
        let fixture = Fixture::new();
        let ws = fixture.base.path().join("ws");
        workspace(&ws, "0.23.1");
        let origin = origin_with(fixture.base.path(), &["0.23.1"]);
        let extra: Vec<(&str, &str)> = value
            .map(|v| ("RIDL_TOOLCHAIN_AUTO_INSTALL", v))
            .into_iter()
            .collect();
        let out = run_with_origin(&fixture, &ws, &origin, &["check", "."], &extra, b"");
        assert_refusal(&out, "MANI-026");
        assert!(
            !fixture.toolchain().join("versions/0.23.1").exists(),
            "{value:?}"
        );
        assert!(!stderr(&out).contains("installing ridl"), "{value:?}");
    }
}

#[test]
fn auto_install_with_one_installs_and_hands_off() {
    let fixture = Fixture::new();
    let ws = fixture.base.path().join("ws");
    workspace(&ws, "0.23.1");
    let origin = origin_with(fixture.base.path(), &["0.23.1"]);
    let out = run_with_origin(
        &fixture,
        &ws,
        &origin,
        &["check", "."],
        &[("RIDL_TOOLCHAIN_AUTO_INSTALL", "1")],
        b"",
    );
    let root = fixture.toolchain().canonicalize().unwrap();
    let bin = root.join("versions/0.23.1/ridl");
    assert_eq!(out.status.code(), Some(7), "{out:?}");
    assert_eq!(
        stderr(&out),
        format!(
            "installing ridl 0.23.1\u{2026}\ninstalled ridl 0.23.1 at {}\nchild stderr\n",
            bin.display()
        )
    );
    assert!(root.join("versions/0.23.1/.complete").exists());
    assert_eq!(fixture.recorded("version"), b"0.23.1\n");
}

#[test]
fn auto_install_requires_exactly_one_distinct_pin() {
    let fixture = Fixture::new();
    let parent = fixture.base.path().join("parent");
    fs::create_dir(&parent).unwrap();
    workspace(&parent.join("a"), "0.23.1");
    workspace(&parent.join("b"), "0.23.4");
    let origin = origin_with(fixture.base.path(), &["0.23.1", "0.23.4"]);
    let out = run_with_origin(
        &fixture,
        &parent,
        &origin,
        &["fmt", "."],
        &[("RIDL_TOOLCHAIN_AUTO_INSTALL", "1")],
        b"",
    );
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert_eq!(stderr(&out).matches("error[MANI-026]").count(), 2);
    assert!(no_versions(&fixture.toolchain()), "{out:?}");
    assert!(!stderr(&out).contains("installing ridl"));
}

#[test]
fn auto_install_never_runs_under_the_guard() {
    let fixture = Fixture::new();
    let ws = fixture.base.path().join("ws");
    workspace(&ws, "0.23.1");
    let origin = origin_with(fixture.base.path(), &["0.23.1"]);
    let out = run_with_origin(
        &fixture,
        &ws,
        &origin,
        &["check", "."],
        &[
            ("RIDL_TOOLCHAIN_AUTO_INSTALL", "1"),
            ("RIDL_TOOLCHAIN_HANDOFF", "0.23.4"),
        ],
        b"",
    );
    assert_refusal(&out, "MANI-026");
    assert!(!fixture.toolchain().join("versions/0.23.1").exists());
}

#[test]
fn auto_install_failure_prints_the_error_before_the_refusal() {
    let fixture = Fixture::new();
    let ws = fixture.base.path().join("ws");
    workspace(&ws, "0.23.1");
    let origin = fixture.base.path().join("empty-origin");
    fs::create_dir(&origin).unwrap();
    let out = run_with_origin(
        &fixture,
        &ws,
        &origin,
        &["check", "."],
        &[("RIDL_TOOLCHAIN_AUTO_INSTALL", "1")],
        b"",
    );
    let text = stderr(&out);
    let lines: Vec<_> = text.lines().collect();
    assert_eq!(lines[0], "installing ridl 0.23.1\u{2026}");
    assert!(lines[1].starts_with("error: "), "{text}");
    assert!(
        lines[2..].iter().any(|l| l.starts_with("error[MANI-026]")),
        "{text}"
    );
    assert_eq!(out.status.code(), Some(2));
}

/// A snapshot input never hands off: an unreadable snapshot directory under a workspace
/// that pins another release reports the refusal and exits 2, runs no installed
/// release, and installs nothing, even with the opt-in set.
#[test]
fn an_unreadable_snapshot_input_never_hands_off_or_installs() {
    let fixture = Fixture::new();
    let ws = fixture.base.path().join("ws");
    workspace(&ws, "0.23.1");
    let snap = ws.join("snap");
    fs::create_dir(&snap).unwrap();
    let file = snap.join("p.ir.json");
    fs::write(&file, "{}").unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read(&file).is_ok() {
        eprintln!("skipped: the process is not bound by file permissions");
        return;
    }
    fake_release(&fixture.toolchain(), "0.23.1");
    let installable = fixture.base.path().join("installable");
    fs::create_dir(&installable).unwrap();
    let origin = origin_with(&installable, &["0.23.9"]);
    let out = run_with_origin(
        &fixture,
        &ws,
        &origin,
        &["diff", "snap", "snap"],
        &[("RIDL_TOOLCHAIN_AUTO_INSTALL", "1")],
        b"",
    );
    assert_refusal(&out, "MANI-026");
    assert!(fixture.record_is_empty(), "{out:?}");
    assert!(!stderr(&out).contains("installing ridl"), "{out:?}");
    assert!(!fixture.toolchain().join("versions/0.23.9").exists());
}

/// A selection that fails to read the toolchain directory is not an answer: the
/// opt-in install is attempted, its error is printed, and the refusal follows.
#[test]
fn a_selection_error_under_the_opt_in_attempts_the_install_then_refuses() {
    let fixture = Fixture::new();
    let ws = fixture.base.path().join("ws");
    workspace(&ws, "0.23.1");
    fs::write(fixture.toolchain().join("versions"), "not a directory").unwrap();
    let origin = origin_with(fixture.base.path(), &["0.23.1"]);
    let out = run_with_origin(
        &fixture,
        &ws,
        &origin,
        &["check", "."],
        &[("RIDL_TOOLCHAIN_AUTO_INSTALL", "1")],
        b"",
    );
    let text = stderr(&out);
    let lines: Vec<_> = text.lines().collect();
    assert_eq!(lines[0], "installing ridl 0.23.1\u{2026}", "{text}");
    assert!(lines[1].starts_with("error: "), "{text}");
    assert!(
        lines[2..].iter().any(|l| l.starts_with("error[MANI-026]")),
        "{text}"
    );
    assert_eq!(out.status.code(), Some(2));
    assert!(fixture.record_is_empty());
}
