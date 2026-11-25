const BIN: &str = env!("CARGO_BIN_EXE_ridl");
fn commands() -> Vec<Vec<&'static str>> {
    vec![
        vec!["check", "."],
        vec!["build", ".", "--out-dir", "out"],
        vec!["fmt", "."],
        vec!["fmt", ".", "--check"],
        vec!["lock", "."],
        vec!["baseline", ".", "--out", "out"],
        vec!["test", "."],
        vec!["vendor", "."],
        vec!["vendor", ".", "--check"],
        vec!["diff", ".", "a.typl"],
        vec!["diff", "a.typl", "."],
    ]
}
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
use tempfile::TempDir;

fn workspace(pin: &str) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("ridl.toml"), format!("[package]\nname = \"p\"\nversion = \"1.0.0\"\nridl-version = {pin}\nunknown = true\n[codegen]\nheader-file = \"missing\"\n[lints]\nunknown-lint = \"deny\"\n")).unwrap();
    fs::write(
        dir.path().join("a.typl"),
        "package p\ntype A:integer[0..1]\n",
    )
    .unwrap();
    fs::write(dir.path().join("instances.lock"), "broken [ TOML").unwrap();
    fs::write(dir.path().join("ridl.lock"), "broken [ TOML").unwrap();
    fs::create_dir(dir.path().join("out")).unwrap();
    fs::write(dir.path().join("out/sentinel"), "retain output").unwrap();
    dir
}

fn tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, path: &Path, result: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            result.insert(
                path.strip_prefix(root).unwrap().to_path_buf(),
                if path.is_file() {
                    fs::read(&path).unwrap()
                } else {
                    vec![]
                },
            );
            if path.is_dir() {
                walk(root, &path, result);
            }
        }
    }
    let mut result = BTreeMap::new();
    walk(root, root, &mut result);
    result
}
fn run(root: &Path, args: &[&str], running: &str) -> Output {
    // An empty toolchain directory keeps a release installed on the machine
    // from receiving the command, so a refusal is always printed.
    let toolchain = tempfile::tempdir().unwrap();
    Command::new(BIN)
        .current_dir(root)
        .args(args)
        .env("RIDL_TEST_RUNNING_RELEASE", running)
        .env("RIDL_TOOLCHAIN_DIR", toolchain.path())
        .env_remove("RIDL_TOOLCHAIN_HANDOFF")
        .env_remove("RIDL_TOOLCHAIN_AUTO_INSTALL")
        .output()
        .unwrap()
}
fn assert_refusal(output: &Output, code: &str) {
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let text = String::from_utf8_lossy(&output.stderr);
    let codes: Vec<_> = text
        .lines()
        .filter(|line| line.starts_with("error[") || line.starts_with("warning["))
        .collect();
    assert_eq!(codes.len(), 1, "{output:?}");
    assert!(codes[0].contains(code), "{output:?}");
}

#[test]
fn release_member_directory_refuses_incompatible_root() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("member")).unwrap();
    fs::write(
        dir.path().join("ridl.toml"),
        "[workspace]\nmembers = [\"member\"]\nridl-version = \"0.24.0\"\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("member/ridl.toml"),
        "[package]\nname = \"member\"\nversion = \"1.0.0\"\n",
    )
    .unwrap();
    let before = tree(dir.path());
    assert_refusal(&run(dir.path(), &["check", "member"], "0.23.2"), "MANI-026");
    assert_eq!(tree(dir.path()), before);
}

#[test]
fn release_fmt_reports_both_refusing_roots_before_writing() {
    let dir = tempfile::tempdir().unwrap();
    for (name, pin) in [("a", "0.24.0"), ("b", "0.25.0")] {
        let root = dir.path().join(name);
        fs::create_dir(&root).unwrap();
        fs::write(
            root.join("ridl.toml"),
            format!(
                "[package]\nname = \"{name}\"\nversion = \"1.0.0\"\nridl-version = \"{pin}\"\n"
            ),
        )
        .unwrap();
        fs::write(
            root.join("a.typl"),
            format!("package {name}\ntype A:integer[0..1]\n"),
        )
        .unwrap();
    }
    let before = tree(dir.path());
    let output = run(dir.path(), &["fmt", "."], "0.23.2");
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let text = String::from_utf8_lossy(&output.stderr);
    assert_eq!(text.matches("error[MANI-026]").count(), 2, "{output:?}");
    for name in ["a", "b"] {
        assert!(text.contains(&format!("{name}/ridl.toml")), "{output:?}");
    }
    assert_eq!(tree(dir.path()), before);
}
#[test]
fn release_refusal_exits_two_and_preserves_outputs() {
    for (pin, code) in [
        ("42", "MANI-024"),
        ("\"0.22.0\"", "MANI-025"),
        ("\"0.23.0\"", "MANI-026"),
    ] {
        let dir = workspace(pin);
        for args in commands() {
            let before = tree(dir.path());
            let output = run(dir.path(), &args, "0.24.0");
            assert_refusal(&output, code);
            assert_eq!(tree(dir.path()), before, "{args:?}");
        }
    }
}
#[test]
fn release_override_is_debug_only() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("ridl.toml"),
        "[package]\nname = \"p\"\nversion = \"1.0.0\"\nridl-version = \"0.24.0\"\n",
    )
    .unwrap();
    let actual = Command::new(BIN)
        .current_dir(dir.path())
        .args(["check", "."])
        .env_remove("RIDL_TEST_RUNNING_RELEASE")
        .output()
        .unwrap();
    let invalid = run(dir.path(), &["check", "."], "invalid");
    assert_eq!(invalid.status.code(), actual.status.code());
    assert_eq!(invalid.stderr, actual.stderr);
    let output = run(dir.path(), &["check", "."], "0.24.0");
    assert_eq!(
        output.status.code(),
        if cfg!(debug_assertions) {
            Some(0)
        } else {
            actual.status.code()
        },
        "{output:?}"
    );
}

#[test]
fn release_refusal_in_json_and_sarif_is_the_only_diagnostic() {
    for (pin, code) in [
        ("42", "MANI-024"),
        ("\"0.22.0\"", "MANI-025"),
        ("\"0.23.0\"", "MANI-026"),
    ] {
        let dir = workspace(pin);
        for format in ["json", "sarif"] {
            let output = run(dir.path(), &["check", ".", "--format", format], "0.24.0");
            assert_eq!(output.status.code(), Some(2), "{output:?}");
            let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            let rows = if format == "json" {
                json.as_array().unwrap()
            } else {
                json["runs"][0]["results"].as_array().unwrap()
            };
            assert_eq!(rows.len(), 1, "{json}");
            assert_eq!(
                rows[0][if format == "json" { "code" } else { "ruleId" }],
                code
            );
        }
    }
}
#[test]
fn release_fmt_preflights_all_roots_before_writing() {
    let dir = tempfile::tempdir().unwrap();
    for (name, pin) in [("a", "0.23.0"), ("b", "0.24.0")] {
        let root = dir.path().join(name);
        fs::create_dir(&root).unwrap();
        fs::write(
            root.join("ridl.toml"),
            format!(
                "[package]\nname = \"{name}\"\nversion = \"1.0.0\"\nridl-version = \"{pin}\"\n"
            ),
        )
        .unwrap();
        fs::write(
            root.join("a.typl"),
            format!("package {name}\ntype A:integer[0..1]\n"),
        )
        .unwrap();
    }
    let before = tree(dir.path());
    assert_refusal(&run(dir.path(), &["fmt", "."], "0.23.2"), "MANI-026");
    assert_eq!(tree(dir.path()), before);
}
#[test]
fn release_describe_checks_input_root_without_analysing_sources() {
    let dir = workspace("\"0.23.0\"");
    // Generate a real catalog from a separate unpinned workspace.
    let standalone = tempfile::tempdir().unwrap();
    fs::write(
        standalone.path().join("ridl.toml"),
        "[package]\nname = \"p\"\nversion = \"1.0.0\"\n",
    )
    .unwrap();
    fs::write(
        standalone.path().join("a.ridl"),
        "package p\ntype A: integer [0..1]\ninterface I { signal x: A }\n",
    )
    .unwrap();
    let output = run(
        standalone.path(),
        &["build", ".", "--emit", "catalog", "--out-dir", "out"],
        "0.24.0",
    );
    assert!(output.status.success(), "{output:?}");
    let actual = standalone.path().join("out/p.catalog.binfb");
    let inside = dir.path().join("p.catalog");
    fs::copy(&actual, &inside).unwrap();
    fs::write(dir.path().join("a.typl"), "broken source").unwrap();
    assert_refusal(
        &run(
            dir.path(),
            &["describe", inside.to_str().unwrap()],
            "0.24.0",
        ),
        "MANI-026",
    );
    let manifestless = tempfile::tempdir().unwrap();
    let descriptor = manifestless.path().join("standalone.catalog.binfb");
    fs::copy(&actual, &descriptor).unwrap();
    assert!(
        ridl_core::read_root_release(&descriptor, None)
            .unwrap()
            .manifest
            .is_none()
    );
    assert!(
        run(
            dir.path(),
            &["describe", descriptor.to_str().unwrap()],
            "0.24.0"
        )
        .status
        .success()
    );
}
#[test]
fn release_refusal_does_not_change_unpinned_or_non_source_commands() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("ridl.toml"),
        "[package]\nname = \"p\"\nversion = \"1.0.0\"\n",
    )
    .unwrap();
    assert!(run(dir.path(), &["check", "."], "9.0.0").status.success());
    let refusing = workspace("\"0.23.0\"");
    let scaffold = tempfile::tempdir().unwrap();
    let init = scaffold.path().join("initial");
    fs::create_dir(&init).unwrap();
    let new = scaffold.path().join("created");
    for args in [
        vec!["--version"],
        vec!["init", init.to_str().unwrap(), "--name", "p"],
        vec!["new", new.to_str().unwrap(), "--name", "p"],
    ] {
        let output = run(refusing.path(), &args, "0.24.0");
        assert!(output.status.success(), "{args:?}: {output:?}");
    }
    let output = run(
        dir.path(),
        &["build", ".", "--emit", "ir-json", "--out-dir", "snapshots"],
        "9.0.0",
    );
    assert!(output.status.success(), "{output:?}");
    // An empty unit emits no package: add a real source before generating snapshots.
    fs::write(
        dir.path().join("a.typl"),
        "package p\ntype A: integer [0..1]\n",
    )
    .unwrap();
    assert!(
        run(
            dir.path(),
            &["build", ".", "--emit", "ir-json", "--out-dir", "snapshots"],
            "9.0.0"
        )
        .status
        .success()
    );
    let snapshot = fs::read(dir.path().join("snapshots/p.ir.json")).unwrap();
    for side in ["old.ir.json", "new.ir.json"] {
        fs::write(refusing.path().join(side), &snapshot).unwrap();
    }
    let output = run(
        refusing.path(),
        &["diff", "old.ir.json", "new.ir.json"],
        "0.24.0",
    );
    assert!(output.status.success(), "{output:?}");
    for side in ["base.lock", "ours.lock", "theirs.lock"] {
        fs::write(refusing.path().join(side), "next 1\n").unwrap();
    }
    let output = run(
        refusing.path(),
        &[
            "lock",
            "merge",
            "base.lock",
            "ours.lock",
            "theirs.lock",
            "7",
        ],
        "0.24.0",
    );
    assert!(output.status.success(), "{output:?}");
}
#[test]
#[ignore = "requires separately built release binaries stamped 0.23.0"]
fn release_override_is_ignored_by_release_binaries() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("ridl.toml"),
        "[package]\nname = \"p\"\nversion = \"1.0.0\"\nridl-version = \"0.24.0\"\n",
    )
    .unwrap();
    for key in ["RIDL_RELEASE_BIN", "RIDLC_RELEASE_BIN"] {
        let bin = std::env::var(key).expect("release binary path");
        let output = Command::new(&bin).arg("--version").output().unwrap();
        assert!(
            String::from_utf8_lossy(&output.stdout)
                .trim()
                .ends_with(" 0.23.0"),
            "{output:?}"
        );
        let output = Command::new(&bin)
            .args(["check", dir.path().to_str().unwrap()])
            .env("RIDL_TEST_RUNNING_RELEASE", "0.24.0")
            .output()
            .unwrap();
        assert_refusal(&output, "MANI-026");
    }
}

#[test]
fn release_fmt_checks_empty_roots() {
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("root");
    fs::create_dir(&root).unwrap();
    fs::write(
        root.join("ridl.toml"),
        "[package]\nname = \"p\"\nversion = \"1.0.0\"\nridl-version = \"0.24.0\"\n",
    )
    .unwrap();
    assert_refusal(&run(parent.path(), &["fmt", "."], "0.23.2"), "MANI-026");
}

#[test]
fn release_diff_precedes_publication_validation() {
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("root");
    fs::create_dir(&root).unwrap();
    fs::write(
        root.join("ridl.toml"),
        "[package]\nname = \"p\"\nversion = \"1.0.0\"\nridl-version = \"0.24.0\"\n",
    )
    .unwrap();
    let marker = ridlc::baseline_context::transaction_marker(&root).unwrap();
    fs::write(marker, "interrupted").unwrap();
    assert_refusal(
        &run(parent.path(), &["diff", "root", "root"], "0.23.2"),
        "MANI-026",
    );
}

#[test]
fn release_member_warning_at_deny_is_exit_one() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("p")).unwrap();
    fs::write(dir.path().join("ridl.toml"),"[workspace]\nmembers = [\"p\"]\nridl-version = \"0.23.0\"\n[lints]\nmember-ridl-version = \"deny\"\n").unwrap();
    fs::write(
        dir.path().join("p/ridl.toml"),
        "[package]\nname = \"p\"\nversion = \"1.0.0\"\nridl-version = 42\n",
    )
    .unwrap();
    let output = run(dir.path(), &["check", "p"], "0.23.2");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("MANI-027"),
        "{output:?}"
    );
}

#[test]
fn release_diff_subdirectory_precedes_publication_validation() {
    let dir = workspace("\"0.23.0\"");
    let empty = dir.path().join("empty");
    fs::create_dir(&empty).unwrap();
    fs::write(
        ridlc::baseline_context::transaction_marker(&empty).unwrap(),
        "interrupted",
    )
    .unwrap();
    assert_refusal(
        &run(dir.path(), &["diff", "empty", "empty"], "0.24.0"),
        "MANI-026",
    );
}

#[test]
fn release_diff_checks_a_refusing_new_side_after_a_valid_old_side() {
    let refusing = workspace("\"0.23.0\"");
    let valid = tempfile::tempdir().unwrap();
    let old = valid.path().join("old.typl");
    fs::write(&old, "package old\ntype A: integer [0..1]\n").unwrap();
    let before = tree(refusing.path());
    assert_refusal(
        &run(
            refusing.path(),
            &["diff", old.to_str().unwrap(), "."],
            "0.24.0",
        ),
        "MANI-026",
    );
    assert_eq!(tree(refusing.path()), before);
}

#[test]
fn release_diff_snapshot_directory_with_stray_source_ignores_parent_pin() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("original.typl"),
        "package p\ntype A: integer [0..1]\n",
    )
    .unwrap();
    let built = run(
        dir.path(),
        &[
            "build",
            "original.typl",
            "--emit",
            "ir-json",
            "--out-dir",
            "snapshots",
        ],
        "0.23.2",
    );
    assert!(built.status.success(), "{built:?}");
    fs::rename(
        dir.path().join("original.typl"),
        dir.path().join("snapshots/original.typl"),
    )
    .unwrap();
    fs::write(
        dir.path().join("ridl.toml"),
        "[workspace]\nmembers = []\nridl-version = \"0.24.0\"\n",
    )
    .unwrap();
    let before = tree(dir.path());
    let output = run(dir.path(), &["diff", "snapshots", "snapshots"], "0.23.2");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(output.stdout, b"identical\n");
    assert!(output.stderr.is_empty(), "{output:?}");
    assert_eq!(tree(dir.path()), before);
}

#[test]
fn release_fmt_reports_one_refusal_for_a_root_with_a_member() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("workspace");
    let member = root.join("member");
    fs::create_dir_all(&member).unwrap();
    fs::write(
        root.join("ridl.toml"),
        "[workspace]\nmembers = [\"member\"]\nridl-version = \"0.24.0\"\n",
    )
    .unwrap();
    fs::write(
        member.join("ridl.toml"),
        "[package]\nname = \"member\"\nversion = \"1.0.0\"\n",
    )
    .unwrap();
    fs::write(
        member.join("a.typl"),
        "package member\ntype A:integer[0..1]\n",
    )
    .unwrap();
    let before = tree(dir.path());
    let output = run(dir.path(), &["fmt", "."], "0.23.2");
    assert_refusal(&output, "MANI-026");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr)
            .matches("error[MANI-026]")
            .count(),
        1,
        "{output:?}"
    );
    assert_eq!(tree(dir.path()), before);
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn release_describe_refusal_precedes_a_missing_input() {
    let pinned = workspace("\"0.24.0\"");
    let control_dir = tempfile::tempdir().unwrap();
    for input in ["missing.catalog.binfb", "gone/missing.catalog.binfb"] {
        let refused = run(pinned.path(), &["describe", input], "0.23.2");
        assert_refusal(&refused, "MANI-026");
        assert!(!stderr(&refused).contains("No such file"), "{refused:?}");
        let control = run(control_dir.path(), &["describe", input], "0.23.2");
        assert_eq!(control.status.code(), Some(2));
        assert!(
            stderr(&control).starts_with(&format!("error: {input}: ")),
            "{control:?}"
        );
    }
}

/// The refusal is found from the input's own nearest existing ancestor, not
/// from the working directory: the command runs outside the pinned workspace.
#[test]
fn release_describe_refusal_is_found_from_the_input_not_the_working_directory() {
    let pinned = workspace("\"0.24.0\"");
    fs::create_dir(pinned.path().join("gone")).unwrap();
    let outside = tempfile::tempdir().unwrap();
    for input in ["missing.catalog.binfb", "gone/missing.catalog.binfb"] {
        let path = pinned.path().join(input);
        let refused = run(
            outside.path(),
            &["describe", path.to_str().unwrap()],
            "0.23.2",
        );
        assert_refusal(&refused, "MANI-026");
    }
}

#[cfg(unix)]
#[test]
fn release_diff_refusal_precedes_snapshot_input_errors() {
    use std::os::unix::fs::PermissionsExt;
    let built = tempfile::tempdir().unwrap();
    fs::write(
        built.path().join("original.typl"),
        "package p\ntype A: integer [0..1]\n",
    )
    .unwrap();
    let output = run(
        built.path(),
        &[
            "build",
            "original.typl",
            "--emit",
            "ir-json",
            "--out-dir",
            "snapshots",
        ],
        "0.23.2",
    );
    assert!(output.status.success(), "{output:?}");
    let snapshot = fs::read_dir(built.path().join("snapshots"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.to_string_lossy().ends_with(".ir.json"))
        .expect("a built snapshot");
    // Each case is a directory of snapshot inputs that cannot be listed or read.
    let fixture = |parent: &Path| {
        let listing = parent.join("listing/snaps");
        fs::create_dir_all(&listing).unwrap();
        std::os::unix::fs::symlink("missing", listing.join("x.ir.json")).unwrap();
        let reading = parent.join("reading/snaps");
        fs::create_dir_all(&reading).unwrap();
        fs::copy(&snapshot, reading.join("a.ir.json")).unwrap();
        fs::copy(&snapshot, reading.join("b.ir.json")).unwrap();
        fs::set_permissions(reading.join("b.ir.json"), fs::Permissions::from_mode(0o000)).unwrap();
    };
    let unreadable_as_root = {
        let probe = built.path().join("probe");
        fs::write(&probe, "x").unwrap();
        fs::set_permissions(&probe, fs::Permissions::from_mode(0o000)).unwrap();
        fs::read(&probe).is_ok()
    };
    let pinned = workspace("\"0.24.0\"");
    fixture(&pinned.path().join("out"));
    let control = tempfile::tempdir().unwrap();
    fixture(control.path());
    let case =
        |root: &Path, name: &str| run(&root.join(name), &["diff", "snaps", "snaps"], "0.23.2");
    let listing = case(&pinned.path().join("out"), "listing");
    assert_refusal(&listing, "MANI-026");
    let control_listing = case(control.path(), "listing");
    assert_eq!(control_listing.status.code(), Some(2));
    assert!(
        stderr(&control_listing).starts_with("error: cannot read the snapshot "),
        "{control_listing:?}"
    );
    if !unreadable_as_root {
        let reading = case(&pinned.path().join("out"), "reading");
        assert_refusal(&reading, "MANI-026");
        let control_reading = case(control.path(), "reading");
        assert_eq!(control_reading.status.code(), Some(2));
        assert!(
            stderr(&control_reading).contains("b.ir.json: "),
            "{control_reading:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn release_diff_refusal_precedes_a_nested_snapshot_read_error() {
    use std::os::unix::fs::PermissionsExt;
    // A directory with no snapshot and no source, whose subdirectory cannot be
    // listed, is read while looking for snapshots one level too low.
    let fixture = |parent: &Path| {
        let sub = parent.join("d/sub");
        fs::create_dir_all(&sub).unwrap();
        fs::set_permissions(&sub, fs::Permissions::from_mode(0o000)).unwrap();
    };
    let restore = |parent: &Path| {
        fs::set_permissions(parent.join("d/sub"), fs::Permissions::from_mode(0o755)).unwrap();
    };
    let pinned = workspace("\"0.24.0\"");
    let control = tempfile::tempdir().unwrap();
    fixture(pinned.path());
    fixture(control.path());
    let refused = run(pinned.path(), &["diff", "d", "d"], "0.23.2");
    let control_output = run(control.path(), &["diff", "d", "d"], "0.23.2");
    let as_root = fs::read_dir(control.path().join("d/sub")).is_ok();
    restore(pinned.path());
    restore(control.path());
    if as_root {
        return;
    }
    assert_refusal(&refused, "MANI-026");
    assert_eq!(control_output.status.code(), Some(2));
    assert!(
        stderr(&control_output).starts_with("error: "),
        "{control_output:?}"
    );
    assert!(!stderr(&control_output).contains("MANI-026"));
}
