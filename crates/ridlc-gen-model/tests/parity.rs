//! The process host's parity test (the lane P driver, stage P3; ADR-0020
//! decision 11 as amended 2026-09-22): the reference plugin, run by
//! `ridlc`'s process host, is byte-identical to the in-process path over
//! every corpus package.
//!
//! Two levels. At the contract's level, one request per corpus package is
//! answered by `ModelBackend` in process and by `ridlc-gen-model` across the
//! pipe, and the two responses are equal. At the command's level, every
//! corpus entry is built twice, `--emit codegen-model` and
//! `--plugin model=<the built binary>`, and the two output directories hold
//! the same files with the same bytes. The binary is the one cargo built
//! for this test (`CARGO_BIN_EXE_ridlc-gen-model`): no installation, no
//! `PATH`, no network.
//!
//! The Rust backend is not what runs here, because it still reads the raw
//! IR and a plugin has none (the model backend is the one backend a plugin
//! can wrap today); stage P4 ports it, and this file then gains
//! `ridlc-gen-rust` on the same two levels.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ridl_core::RidlDatabase;
use ridl_ir::codegen::{self, Backend as _, ModelBackend, v1};
use ridl_ir::v2;
use ridlc::plugin::{Plugin, PluginSpec, run};

const TIMEOUT: Duration = Duration::from_secs(60);

fn plugin() -> Plugin {
    Plugin {
        language: "model".to_string(),
        executable: PathBuf::from(env!("CARGO_BIN_EXE_ridlc-gen-model")),
    }
}

fn spec() -> PluginSpec {
    PluginSpec {
        language: "model".to_string(),
        executable: Some(PathBuf::from(env!("CARGO_BIN_EXE_ridlc-gen-model"))),
    }
}

/// Every corpus entry of `crates/ridlc/tests/corpus`, in directory order.
fn corpus_entries() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("ridlc")
        .join("tests")
        .join("corpus");
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&root)
        .expect("the corpus directory is readable")
        .map(|entry| entry.expect("a corpus directory entry").path())
        .filter(|path| path.is_dir())
        .collect();
    entries.sort();
    assert!(!entries.is_empty(), "the corpus must not be empty");
    entries
}

/// Every package of every corpus entry, with the scope the CLI hands the
/// backends: every sibling of the build plus `ridl.std`.
fn corpus_packages() -> Vec<(String, Vec<v2::Package>)> {
    corpus_entries()
        .into_iter()
        .map(|entry| {
            let name = entry
                .file_name()
                .expect("a named entry")
                .to_string_lossy()
                .into_owned();
            let mut db = RidlDatabase::default();
            let output = ridlc::compile_workspace(&mut db, &entry).expect("a corpus entry loads");
            let mut packages: Vec<v2::Package> = output
                .checked
                .iter()
                .map(|checked| checked.ir.clone())
                .collect();
            packages.push(output.std_ir.clone());
            (name, packages)
        })
        .collect()
}

#[test]
fn the_plugin_answers_every_corpus_request_as_the_in_process_backend_does() {
    let plugin = plugin();
    let mut compared = 0usize;
    for (entry, packages) in corpus_packages() {
        let others: Vec<&v2::Package> = packages.iter().collect();
        for package in &packages {
            let request = ridlc::codegen_request(&package.name, package, &others, Vec::new());
            let label = format!("{entry}: package {}", package.name);

            let in_process = ModelBackend.generate(&request);
            let through_the_host = run(&plugin, &request, TIMEOUT)
                .unwrap_or_else(|err| panic!("{label}: the process host failed: {err}"));

            assert_eq!(
                through_the_host, in_process,
                "{label}: the response across the pipe differs from the response in process"
            );
            assert_eq!(
                in_process.files.len(),
                1,
                "{label}: the model backend writes one file"
            );
            let file = &in_process.files[0];
            assert_eq!(file.path, format!("{}.codegen.json", package.name));
            let expected = codegen::to_json_pretty(request.model.as_ref().expect("a model"))
                .expect("the model renders");
            assert_eq!(
                file.content,
                Some(v1::generated_file::Content::Text(expected)),
                "{label}: the file is the model written back"
            );
            compared += 1;
        }
    }
    assert!(compared > 0, "the corpus yielded no package");
}

/// Every regular file under `dir`, keyed by its path relative to `dir`.
fn files_under(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).expect("the directory is readable") {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let relative = path
                    .strip_prefix(root)
                    .expect("under the root")
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(
                    relative,
                    std::fs::read(&path).expect("the file is readable"),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

#[test]
fn a_build_through_the_plugin_writes_what_a_build_through_the_emit_writes() {
    let mut wrote = 0usize;
    for entry in corpus_entries() {
        let label = entry
            .file_name()
            .expect("a named entry")
            .to_string_lossy()
            .into_owned();
        let in_process = tempfile::tempdir().expect("a temp dir");
        let through_the_host = tempfile::tempdir().expect("a temp dir");

        let emit_run = ridlc::run_build_with(
            &entry,
            in_process.path(),
            &[ridlc::Emit::CodegenModel],
            &[],
            TIMEOUT,
            ridl_core::Frozen::No,
        )
        .expect("the build runs");
        let plugin_run = ridlc::run_build_with(
            &entry,
            through_the_host.path(),
            &[],
            &[spec()],
            TIMEOUT,
            ridl_core::Frozen::No,
        )
        .expect("the build runs");

        assert_eq!(
            emit_run.has_error(),
            plugin_run.has_error(),
            "{label}: the two builds disagree on whether the entry builds"
        );
        let plugin_messages: Vec<&str> = plugin_run
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .filter(|message| message.starts_with("plugin `"))
            .collect();
        assert!(
            plugin_messages.is_empty(),
            "{label}: the plugin drew a diagnostic: {plugin_messages:?}"
        );

        let expected = files_under(in_process.path());
        let actual = files_under(through_the_host.path());
        assert_eq!(
            actual.keys().collect::<Vec<_>>(),
            expected.keys().collect::<Vec<_>>(),
            "{label}: the two builds wrote different files"
        );
        for (path, bytes) in &expected {
            assert!(
                actual[path] == *bytes,
                "{label}: `{path}` differs between the emit and the plugin"
            );
        }
        wrote += expected.len();
    }
    assert!(wrote > 0, "no corpus entry wrote a model");
}

#[test]
fn the_plugin_refuses_a_schema_it_does_not_know_with_a_diagnostic_naming_both() {
    let request = v1::CodegenRequest {
        schema: "ridl.codegen.v2".to_string(),
        toolchain: "9.9.9".to_string(),
        model: Some(v1::Model::default()),
        options: Vec::new(),
        artifact_base: "p".to_string(),
    };
    let response = run(&plugin(), &request, TIMEOUT).expect("a response, not a host failure");
    assert!(codegen::has_error(&response));
    assert!(response.files.is_empty());
    let message = &response.diagnostics[0].message;
    assert!(message.contains("ridl.codegen.v1"), "{message}");
    assert!(message.contains("ridl.codegen.v2"), "{message}");
}

#[test]
fn the_plugin_refuses_an_option_it_does_not_know() {
    let request = v1::CodegenRequest {
        schema: codegen::SCHEMA.to_string(),
        toolchain: "0.0.0".to_string(),
        model: Some(v1::Model::default()),
        options: vec![v1::BackendOption {
            key: "indent".to_string(),
            value: "2".to_string(),
        }],
        artifact_base: "p".to_string(),
    };
    let response = run(&plugin(), &request, TIMEOUT).expect("a response, not a host failure");
    assert!(codegen::has_error(&response));
    assert!(response.diagnostics[0].message.contains("`indent`"));
}

#[test]
fn input_that_is_not_a_request_is_a_non_zero_exit_the_host_reports() {
    use std::io::Write as _;
    use std::process::{Command, Stdio};

    let mut child = Command::new(env!("CARGO_BIN_EXE_ridlc-gen-model"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the plugin starts");
    child
        .stdin
        .take()
        .expect("piped")
        .write_all(b"{\"schema\": 1}")
        .expect("the write succeeds");
    let output = child.wait_with_output().expect("the plugin exits");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("CodegenRequest"), "{stderr}");
}
