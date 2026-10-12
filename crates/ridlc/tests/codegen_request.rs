//! The `codegen-request` emit: the whole codegen request a build would hand
//! a plugin, written as canonical protobuf JSON.
//!
//! The emit exists so that a plugin author can read the request their plugin
//! would receive without running one (driftsys/ridl#725). Three facts are
//! pinned here: the artifact parses back into the request it was written
//! from, its `model` is the same value the `codegen-model` emit writes, and
//! the options of `[backend.codegen-request]` reach the written request
//! rather than being refused.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A unique directory under the system temp dir, removed on drop. The same
/// helper `cli.rs` carries, kept beside the tests that use it.
struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let mut path = std::env::temp_dir();
        path.push(format!(
            "ridlc-request-{label}-{}-{}",
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

/// Runs `ridlc` with `args` and returns `(exit_code, stderr)`.
fn ridlc(args: &[&std::ffi::OsStr]) -> (i32, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ridlc"))
        .args(args)
        .output()
        .expect("the ridlc binary must run");
    let code = output.status.code().expect("the process exits with a code");
    (code, String::from_utf8_lossy(&output.stderr).into_owned())
}

/// Builds `entry` into a fresh output directory with `emit` and returns that
/// directory.
fn build(entry: &Path, label: &str, emit: &str) -> TempDir {
    let out = TempDir::new(label);
    let (code, stderr) = ridlc(&[
        "build".as_ref(),
        entry.as_os_str(),
        "--out-dir".as_ref(),
        out.path().as_os_str(),
        "--emit".as_ref(),
        emit.as_ref(),
    ]);
    assert_eq!(code, 0, "the fixture builds, stderr:\n{stderr}");
    out
}

const CORPUS_ENTRY: &str = "tests/corpus/veh-cluster";
const PACKAGE: &str = "veh.cluster";

/// The artifact is canonical JSON of a `CodegenRequest`: reading it back
/// through `request_from_json` and rendering it again gives the same bytes,
/// so the file is the request and not a lossy rendering of it.
#[test]
fn the_emit_writes_a_request_that_parses_back_into_an_equal_request() {
    let out = build(Path::new(CORPUS_ENTRY), "round-trip", "codegen-request");
    let text = std::fs::read_to_string(out.path().join(format!("{PACKAGE}.codegen-request.json")))
        .expect("codegen-request writes <pkg-name>.codegen-request.json");

    let request =
        ridl_ir::codegen::request_from_json(&text).expect("the artifact is canonical request JSON");
    let rendered = ridl_ir::codegen::request_to_json(&request).expect("the request renders");
    assert_eq!(
        rendered.trim_end(),
        text.trim_end(),
        "the artifact and the request it parses into are the same value"
    );
    assert_eq!(request.schema, ridl_ir::codegen::SCHEMA);
    assert_eq!(request.artifact_base, PACKAGE);
}

/// The request's `model` is the value `--emit codegen-model` writes for the
/// same build. The two emits read one lowered model, and this test is what
/// keeps them from becoming two sources of truth.
#[test]
fn the_request_model_equals_what_the_codegen_model_emit_writes() {
    let out = build(
        Path::new(CORPUS_ENTRY),
        "model-parity",
        "codegen-model,codegen-request",
    );
    let request_text =
        std::fs::read_to_string(out.path().join(format!("{PACKAGE}.codegen-request.json")))
            .expect("the request artifact");
    let model_text = std::fs::read_to_string(out.path().join(format!("{PACKAGE}.codegen.json")))
        .expect("the model artifact");

    let request = ridl_ir::codegen::request_from_json(&request_text)
        .expect("the artifact is canonical request JSON");
    let carried = request.model.expect("the request carries a model");
    let written = ridl_ir::codegen::from_json(&model_text).expect("the model is canonical JSON");
    assert_eq!(carried, written);
}

/// The request emit accepts the options of its own `[backend.codegen-request]`
/// table and writes them into the request, where the model emit refuses every
/// option: the point of the artifact is to show the options a plugin receives,
/// and a request with no option is not the request most plugins are handed.
#[test]
fn an_option_of_the_request_table_reaches_the_written_request() {
    let dir = TempDir::new("options-fixture");
    dir.write(
        "ridl.toml",
        "[package]\nname = \"veh.options\"\nversion = \"1.0.0\"\n\n\
         [backend.codegen-request]\nencodings = [\"flatbuffers\", \"proto3\"]\n",
    );
    dir.write(
        "speed.typl",
        "package veh.options\ntype Speed: km/h [0.0..250.0 step 0.5]\n",
    );

    let out = build(dir.path(), "options-out", "codegen-request");
    let text = std::fs::read_to_string(out.path().join("veh.options.codegen-request.json"))
        .expect("the request artifact");
    let request =
        ridl_ir::codegen::request_from_json(&text).expect("the artifact is canonical request JSON");

    let options: Vec<(&str, &str)> = request
        .options
        .iter()
        .map(|option| (option.key.as_str(), option.value.as_str()))
        .collect();
    assert_eq!(options, [("encodings", "flatbuffers,proto3")]);
}
