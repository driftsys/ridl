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

/// Installs a complete release whose binary records what it receives.
fn fake_release(root: &Path, version: &str) -> PathBuf {
    let dir = root.join("versions").join(version);
    fs::create_dir_all(&dir).unwrap();
    let script = format!(
        "#!/bin/sh\n\
         r=\"$RIDL_TEST_RECORD\"\n\
         printf '%s\\n' \"{version}\" > \"$r/version\"\n\
         for a in \"$@\"; do printf '%s\\0' \"$a\"; done > \"$r/argv\"\n\
         pwd -P > \"$r/cwd\"\n\
         cat > \"$r/stdin\"\n\
         printf '%s' \"${{RIDL_TOOLCHAIN_HANDOFF-unset}}\" > \"$r/guard\"\n\
         echo \"child stdout\"; echo \"child stderr\" >&2\n\
         exit 7\n"
    );
    let binary = dir.join("ridl");
    fs::write(&binary, script).unwrap();
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
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in envs {
            command.env(key, value);
        }
        let mut child = command.spawn().unwrap();
        let mut stdin = child.stdin.take().unwrap();
        // The refused run never reads standard input; a closed pipe is fine.
        let _ = stdin.write_all(b"stdin probe\n");
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
