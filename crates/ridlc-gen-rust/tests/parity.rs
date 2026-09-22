//! The process host's parity test over the Rust backend (the lane P driver,
//! stage P4; ADR-0020 decision 11 as amended 2026-09-22): the Rust backend,
//! run by `ridlc`'s process host, is byte-identical to the in-process path
//! over every corpus package.
//!
//! This is the exit test of story E4.5b and of the driver's decision D-P1.
//! The preceding stage could only run it over `ridlc-gen-model`, because the
//! Rust backend still read the raw IR and a plugin has none; stage P4 ported
//! it onto the lowered model, so the backend a plugin can wrap is now the
//! one a user's `--emit rust` reaches.
//!
//! Two levels. At the contract's level, one request per corpus package is
//! answered by `ridl_backend_rust::Backend` in process and by
//! `ridlc-gen-rust` across the pipe, and the two responses are equal. At the
//! command's level, every corpus entry is built twice, `--emit rust` and
//! `--plugin rust=<the built binary>`, and the two output directories hold
//! the same `<package>.rs` files with the same bytes. The binary is the one
//! cargo built for this test (`CARGO_BIN_EXE_ridlc-gen-rust`): no
//! installation, no `PATH`, no network.
//!
//! The command's level compares the per-package sources only. A build with
//! `--emit rust` also writes the crate root and the manifest, which `ridlc`
//! writes for itself and no backend produces (ADR-0020 decision 9), and a
//! build that names only a plugin writes neither.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ridl_core::RidlDatabase;
use ridl_ir::codegen::Backend as _;
use ridl_ir::v2;
use ridlc::plugin::{Plugin, PluginSpec, run};

const TIMEOUT: Duration = Duration::from_secs(120);

fn plugin() -> Plugin {
    Plugin {
        language: "rust".to_string(),
        executable: PathBuf::from(env!("CARGO_BIN_EXE_ridlc-gen-rust")),
    }
}

fn spec() -> PluginSpec {
    PluginSpec {
        language: "rust".to_string(),
        executable: Some(PathBuf::from(env!("CARGO_BIN_EXE_ridlc-gen-rust"))),
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

            let in_process = ridl_backend_rust::Backend.generate(&request);
            let through_the_host = run(&plugin, &request, TIMEOUT)
                .unwrap_or_else(|err| panic!("{label}: the process host failed: {err}"));

            assert_eq!(
                through_the_host, in_process,
                "{label}: the response across the pipe differs from the response in process"
            );
            compared += 1;
        }
    }
    assert!(compared > 0, "the corpus yielded no package");
}

/// Every `.rs` file under `dir`, keyed by its path relative to `dir`.
fn sources_under(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).expect("the directory is readable") {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                walk(root, &path, out);
                continue;
            }
            if path.extension().is_none_or(|extension| extension != "rs") {
                continue;
            }
            let relative = path
                .strip_prefix(root)
                .expect("under the root")
                .to_string_lossy()
                .replace('\\', "/");
            // `lib.rs` is the crate root `ridlc` writes for itself; no
            // backend produces it, and a plugin-only build writes none.
            if relative == "lib.rs" {
                continue;
            }
            out.insert(
                relative,
                std::fs::read(&path).expect("the file is readable"),
            );
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
            &[ridlc::Emit::Rust],
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

        let expected = sources_under(in_process.path());
        let actual = sources_under(through_the_host.path());
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
    assert!(wrote > 0, "no corpus entry wrote a source file");
}
