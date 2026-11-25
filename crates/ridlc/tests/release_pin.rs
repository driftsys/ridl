const BIN: &str = env!("CARGO_BIN_EXE_ridlc");
fn commands() -> Vec<Vec<&'static str>> {
    vec![vec!["check", "."], vec!["build", ".", "--out-dir", "out"]]
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
    Command::new(BIN)
        .current_dir(root)
        .args(args)
        .env("RIDL_TEST_RUNNING_RELEASE", running)
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
fn release_member_path_refuses_incompatible_root() {
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
fn release_refusal_does_not_change_unpinned_or_non_source_commands() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("ridl.toml"),
        "[package]\nname = \"p\"\nversion = \"1.0.0\"\n",
    )
    .unwrap();
    assert!(run(dir.path(), &["check", "."], "9.0.0").status.success());
    let refusing = workspace("\"0.23.0\"");
    assert!(
        run(refusing.path(), &["--version"], "0.24.0")
            .status
            .success()
    );
}

#[test]
fn release_refusal_in_json_and_sarif_is_the_only_diagnostic() {
    for (pin, code) in [
        ("42", "MANI-024"),
        ("\"0.22.0\"", "MANI-025"),
        ("\"9.23.0\"", "MANI-026"),
    ] {
        let dir = workspace(pin);
        let run = ridlc::run_check(dir.path(), ridl_core::Frozen::Yes).unwrap();
        let json = ridl_core::diag::to_json(&run.diagnostics, &run.sources);
        assert_eq!(json.len(), 1);
        assert_eq!(run.diagnostics[0].code.as_str(), code);
        let sarif =
            ridl_core::diag::sarif::to_sarif(&run.diagnostics, &run.sources, None, "0.23.0");
        let value = serde_json::to_value(sarif).unwrap();
        assert_eq!(value["runs"][0]["results"].as_array().unwrap().len(), 1);
    }
}

#[test]
fn release_diff_root_overlay_replaces_non_utf8_disk_manifest() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = dir.path().join("ridl.toml");
    fs::write(&manifest, [0xff]).unwrap();
    let overlay = ridl_core::Overlay {
        path: manifest,
        text: "[package]\nname = \"p\"\nversion = \"1.0.0\"\nridl-version = \"9.0.0\"\n"
            .to_string(),
    };
    let result = ridlc::load_diff_side(
        &mut ridl_core::RidlDatabase::default(),
        dir.path(),
        &[overlay],
    );
    assert!(
        matches!(result,Err(ridlc::DiffSideError::Compile {diagnostics,..}) if diagnostics.len()==1 && diagnostics[0].code==ridl_core::DiagCode::MANI_026)
    );
}
