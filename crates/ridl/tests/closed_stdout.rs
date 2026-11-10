//! Reports end with one newline; a stdout write failure exits 2, overriding a verdict.

#![cfg(unix)]

mod remote_support;

use std::io;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;

// On macOS, pipe creation and setting close-on-exec are separate operations.
// Serialize pipe creation through spawn so another child cannot inherit a reader.
static PIPE_LOCK: Mutex<()> = Mutex::new(());

fn command(dir: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ridl"));
    command.current_dir(dir).env("HOME", dir).args(args);
    command
}

fn assert_closed_stdout(mut command: Command) {
    let guard = PIPE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (reader, writer) = io::pipe().expect("create stdout pipe");
    drop(reader);
    let child = command
        .stdout(writer)
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn ridl with no stdout reader");
    drop(guard);
    let output = child
        .wait_with_output()
        .expect("run ridl with no stdout reader");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "stderr:\n{stderr}");
    let cause = io::Error::from_raw_os_error(32);
    assert!(
        stderr.ends_with(&format!("error: {cause}\n")),
        "the write failure is reported without a panic: {stderr}"
    );
}

fn workspace(source: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("create workspace");
    std::fs::write(
        dir.path().join("ridl.toml"),
        "[package]\nname = \"app\"\nversion = \"1.0.0\"\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("app.ridl"), source).unwrap();
    dir
}

const SOURCE: &str = "package app\n/// A level.\ntype Level: integer [0..100]\n";
const INTERFACE: &str =
    "package app\ntype Level: integer [0..100]\ninterface Status { signal level: Level @10ms }\n";

fn successful_stdout(mut command: Command) -> Vec<u8> {
    let guard = PIPE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn ridl with captured stdout");
    drop(guard);
    let output = child.wait_with_output().expect("capture ridl report");
    assert!(
        output.status.success(),
        "status: {}; stderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.ends_with(b"\n"), "missing final newline");
    assert!(
        !output.stdout.ends_with(b"\n\n"),
        "more than one final newline"
    );
    output.stdout
}

#[test]
fn explain_stdout_has_exactly_one_final_newline() {
    let dir = tempfile::tempdir().unwrap();
    let category = ridl_diff::Category::TimingChanged;
    let word = ridl_diff::category_word(category);
    let stdout = successful_stdout(command(dir.path(), &["diff", "--explain", word]));
    assert_eq!(
        stdout,
        format!("{word}\n{}\n", ridl_diff::explain(category)).as_bytes()
    );
}

#[test]
fn diff_json_stdout_has_exactly_one_final_newline() {
    let dir = workspace(SOURCE);
    successful_stdout(command(
        dir.path(),
        &["diff", "app.ridl", "app.ridl", "--format", "json"],
    ));
}

#[test]
fn check_json_stdout_has_exactly_one_final_newline() {
    let dir = workspace(SOURCE);
    successful_stdout(command(dir.path(), &["check", "--format", "json"]));
}

#[test]
fn check_sarif_stdout_has_exactly_one_final_newline() {
    let dir = workspace(SOURCE);
    successful_stdout(command(dir.path(), &["check", "--format", "sarif"]));
}

#[test]
fn test_json_stdout_has_exactly_one_final_newline() {
    let dir = workspace(SOURCE);
    successful_stdout(command(dir.path(), &["test", "--format", "json"]));
}

#[test]
fn explain_exits_2_when_stdout_reader_is_gone() {
    let dir = tempfile::tempdir().unwrap();
    assert_closed_stdout(command(
        dir.path(),
        &["diff", "--explain", "timing_changed"],
    ));
}

#[test]
fn explain_exits_2_when_both_output_readers_are_gone() {
    let dir = tempfile::tempdir().unwrap();
    let guard = PIPE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (stdout_reader, stdout_writer) = io::pipe().unwrap();
    let (stderr_reader, stderr_writer) = io::pipe().unwrap();
    drop(stdout_reader);
    drop(stderr_reader);
    let mut child = command(dir.path(), &["diff", "--explain", "timing_changed"])
        .stdout(stdout_writer)
        .stderr(stderr_writer)
        .spawn()
        .expect("spawn ridl with no output readers");
    drop(guard);
    let status = child.wait().expect("run ridl with no output readers");
    assert_eq!(status.code(), Some(2));
}

fn diff(format: &str) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("app.ridl"), SOURCE).unwrap();
    std::fs::write(
        dir.path().join("narrow.ridl"),
        "package app\n/// A level.\ntype Level: integer [0..50]\n",
    )
    .unwrap();
    assert_closed_stdout(command(
        dir.path(),
        &["diff", "app.ridl", "narrow.ridl", "--format", format],
    ));
}

#[test]
fn diff_text_exits_2_when_stdout_reader_is_gone() {
    diff("text");
}

#[test]
fn diff_json_exits_2_when_stdout_reader_is_gone() {
    diff("json");
}

fn check(format: &str) {
    let dir = workspace("package app\ntype Level: integer [100..0]\n");
    assert_closed_stdout(command(dir.path(), &["check", "--format", format]));
}

#[test]
fn check_json_exits_2_when_stdout_reader_is_gone() {
    check("json");
}

#[test]
fn check_sarif_exits_2_when_stdout_reader_is_gone() {
    check("sarif");
}

#[test]
fn lock_allocation_exits_2_when_stdout_reader_is_gone() {
    let dir = workspace(INTERFACE);
    assert_closed_stdout(command(dir.path(), &["lock"]));
    assert!(
        dir.path().join("interfaces.lock").is_file(),
        "the interface lock file must be written before reporting fails"
    );
}

#[test]
fn lock_instance_allocation_exits_2_when_stdout_reader_is_gone() {
    let dir = workspace(SOURCE);
    std::fs::write(
        dir.path().join("system.rsdl"),
        "package app\ncomponent Cruise [ instances = (primary, backup) ] {}\nsystem Vehicle { Cruise }\n",
    )
    .unwrap();
    assert_closed_stdout(command(dir.path(), &["lock"]));
}

#[test]
fn lock_edit_exits_2_when_stdout_reader_is_gone() {
    let dir = workspace(SOURCE);
    std::fs::write(
        dir.path().join("interfaces.lock"),
        "# interfaces.lock — written by ridl lock; do not edit by hand.\nnext 2\nStatus 1\n",
    )
    .unwrap();
    assert_closed_stdout(command(dir.path(), &["lock", "--retire", "Status"]));
}

fn property(format: &str) {
    let dir = workspace(SOURCE);
    assert_closed_stdout(command(dir.path(), &["test", "--format", format]));
}

#[test]
fn test_text_exits_2_when_stdout_reader_is_gone() {
    property("text");
}

#[test]
fn test_json_exits_2_when_stdout_reader_is_gone() {
    property("json");
}

#[test]
fn vendor_written_exits_2_when_stdout_reader_is_gone() {
    use remote_support::{COMMON_UNIT, TempDir, consumer, unit_tar};

    let dir = TempDir::new("closed-stdout-vendor");
    let url = "https://example.invalid/common.tar";
    let root = consumer(&dir, url);
    let cache = ridl_core::Cache {
        root: dir.path().join(".ridl/cache"),
    };
    let (sha256, unpacked) = cache.store(url, &unit_tar(COMMON_UNIT)).unwrap();
    let mut lock = ridl_core::Lockfile::default();
    lock.entries.insert(
        url.to_owned(),
        ridl_core::LockEntry {
            sha256,
            tree: Some(ridl_core::tree_hash(&unpacked).unwrap()),
        },
    );
    ridl_core::write_lockfile(&root.join("ridl.lock"), &lock).unwrap();
    let mut command = command(&root, &["vendor"]);
    command.env("HOME", dir.path());
    assert_closed_stdout(command);
    assert!(
        root.join("vendor/veh.common").is_dir(),
        "the vendored package must exist after reporting fails"
    );
}

#[test]
fn vendor_removed_exits_2_when_stdout_reader_is_gone() {
    let dir = workspace(SOURCE);
    std::fs::create_dir_all(dir.path().join("vendor/unused")).unwrap();
    assert_closed_stdout(command(dir.path(), &["vendor"]));
    assert!(
        dir.path().join("vendor").is_dir() && !dir.path().join("vendor/unused").exists(),
        "the vendor directory must remain with the unused package removed after reporting fails"
    );
}

#[test]
fn vendor_unchanged_exits_2_when_stdout_reader_is_gone() {
    let dir = workspace(SOURCE);
    assert_closed_stdout(command(dir.path(), &["vendor"]));
}

#[test]
fn scaffold_exits_2_when_stdout_reader_is_gone() {
    let dir = tempfile::tempdir().unwrap();
    assert_closed_stdout(command(dir.path(), &["new", "demo"]));
    assert!(
        dir.path().join("demo/ridl.toml").is_file() && dir.path().join("demo/demo.ridl").is_file(),
        "the scaffolded manifest and source file must exist after reporting fails"
    );
}
