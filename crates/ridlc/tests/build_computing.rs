//! `run_build_computing` computes the earlier catalogs from the build's own
//! compile: the function is called once, with the build's checked packages,
//! and never for a compile with an error diagnostic — a remote unit that
//! does not materialize included.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use ridl_core::Frozen;
use ridlc::{ApplyLints, Emit};

const SOURCE: &str = "package demo\n\ntype Speed: integer [0..300]\n";

fn workspace(source: &str) -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("a temp dir");
    std::fs::write(
        root.path().join("ridl.toml"),
        "[package]\nname = \"demo\"\nversion = \"1.0.0\"\n",
    )
    .expect("the manifest is written");
    std::fs::write(root.path().join("demo.typl"), source).expect("the source is written");
    root
}

fn build(
    root: &Path,
    compute: &mut ridlc::CompatibleCatalogsFn<'_>,
) -> std::io::Result<ridlc::CliRun> {
    let out = tempfile::tempdir().expect("a temp dir");
    ridlc::run_build_computing(
        root,
        out.path(),
        &[Emit::Catalog],
        &[],
        Duration::from_secs(10),
        Frozen::No,
        ApplyLints::No,
        None,
        Some(compute),
    )
}

#[test]
fn the_function_is_called_once_with_the_checked_packages() {
    let root = workspace(SOURCE);
    let mut calls = Vec::new();
    let run = build(
        root.path(),
        &mut |packages, _std, names_imports, _remotes| {
            calls.push((
                packages.iter().map(|p| p.name.clone()).collect::<Vec<_>>(),
                names_imports,
            ));
            Ok(BTreeMap::new())
        },
    )
    .expect("the build runs");
    assert!(!run.has_error(), "unexpected error: {:?}", run.diagnostics);
    assert_eq!(calls, vec![(vec!["demo".to_string()], false)]);
}

#[test]
fn a_build_with_an_error_diagnostic_never_calls_it() {
    let root = workspace("package demo\n\nstruct S {\n  x: Missing\n}\n");
    let mut called = false;
    let run = build(root.path(), &mut |_, _, _, _| {
        called = true;
        Ok(BTreeMap::new())
    })
    .expect("the build runs");
    assert!(run.has_error(), "the source draws an error");
    assert!(!called, "no list is computed for a build with an error");
}

#[test]
fn an_error_of_the_function_is_the_error_of_the_build() {
    let root = workspace(SOURCE);
    let error = build(root.path(), &mut |_, _, _, _| {
        Err(std::io::Error::other("the baseline cannot be read"))
    })
    .err()
    .expect("the build fails");
    assert_eq!(error.to_string(), "the baseline cannot be read");
}

/// A frozen build of `root`, so that a pinned `[imports]` URL with no
/// `ridl.lock` is MANI-103 without a fetch.
fn frozen_build(
    root: &Path,
    compute: &mut ridlc::CompatibleCatalogsFn<'_>,
) -> std::io::Result<ridlc::CliRun> {
    let out = tempfile::tempdir().expect("a temp dir");
    ridlc::run_build_computing(
        root,
        out.path(),
        &[Emit::Catalog],
        &[],
        Duration::from_secs(10),
        Frozen::Yes,
        ApplyLints::No,
        None,
        Some(compute),
    )
}

/// The remote units are materialized by the load, before the function is
/// called: a package whose `[imports]` cannot be materialized has an error
/// diagnostic, so the function is not called. The `names_imports` flag of a
/// build whose units load is pinned against the `ridl` binary, in
/// `crates/ridl/tests/catalog_chain.rs`.
#[test]
fn a_package_whose_imports_do_not_materialize_never_calls_it() {
    let root = workspace(SOURCE);
    std::fs::write(
        root.path().join("ridl.toml"),
        "[package]\nname = \"demo\"\nversion = \"1.0.0\"\n\n[imports]\next = \"https://example.invalid/ext.git\"\n",
    )
    .expect("the manifest is written");
    let mut flags = Vec::new();
    let run = frozen_build(root.path(), &mut |_, _, names_imports, _| {
        flags.push(names_imports);
        Ok(BTreeMap::new())
    })
    .expect("the build runs");
    assert!(
        run.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_str() == "MANI-103"),
        "{:?}",
        run.diagnostics
    );
    assert_eq!(flags, Vec::<bool>::new());
}

/// The same for `[imports]` named by a workspace root.
#[test]
fn a_root_whose_imports_do_not_materialize_never_calls_it() {
    let root = tempfile::tempdir().expect("a temp dir");
    std::fs::write(
        root.path().join("ridl.toml"),
        "[workspace]\nmembers = [\"a\"]\n\n[imports]\next = \"https://example.invalid/ext.git\"\n",
    )
    .expect("the root manifest is written");
    let member = root.path().join("a");
    std::fs::create_dir(&member).expect("the member directory is created");
    std::fs::write(
        member.join("ridl.toml"),
        "[package]\nname = \"demo\"\nversion = \"1.0.0\"\n",
    )
    .expect("the member manifest is written");
    std::fs::write(member.join("demo.typl"), SOURCE).expect("the source is written");
    let mut flags = Vec::new();
    let run = frozen_build(root.path(), &mut |_, _, names_imports, _| {
        flags.push(names_imports);
        Ok(BTreeMap::new())
    })
    .expect("the build runs");
    assert!(
        run.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_str() == "MANI-103"),
        "{:?}",
        run.diagnostics
    );
    assert_eq!(flags, Vec::<bool>::new());
}

const GIVEN_HASH: &str = "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=";

/// Builds the codegen model of `root` with `run_build_with` given a list for
/// unit `unit`, and returns the model's `catalog.compatible` and the run.
fn model_compatible_given(root: &Path, unit: &str) -> (Vec<String>, ridlc::CliRun) {
    let out = tempfile::tempdir().expect("a temp dir");
    let mut given = BTreeMap::new();
    given.insert(unit.to_string(), vec![[1u8; 32]]);
    let run = ridlc::run_build_with(
        root,
        out.path(),
        &[Emit::CodegenModel],
        &[],
        Duration::from_secs(10),
        Frozen::No,
        ApplyLints::No,
        None,
        &given,
    )
    .expect("the build runs");
    let text = std::fs::read_to_string(out.path().join(format!("{unit}.codegen.json")))
        .expect("the model is written");
    let json: serde_json::Value = serde_json::from_str(&text).expect("the model is JSON");
    let hashes = json["catalog"]["compatible"]
        .as_array()
        .expect("the model's catalog has a compatible array")
        .iter()
        .map(|hash| hash.as_str().expect("a base64 string").to_string())
        .collect();
    (hashes, run)
}

#[test]
fn run_build_with_writes_the_given_list() {
    let root = workspace(SOURCE);
    let (hashes, run) = model_compatible_given(root.path(), "demo");
    assert!(!run.has_error(), "unexpected error: {:?}", run.diagnostics);
    assert_eq!(hashes, vec![GIVEN_HASH.to_string()]);
}

/// An RSDL-7xx error still writes the package artifacts, and the given list
/// is written with them.
#[test]
fn run_build_with_writes_the_given_list_beside_an_rsdl_7xx_error() {
    let root = tempfile::tempdir().expect("a temp dir");
    std::fs::write(
        root.path().join("ridl.toml"),
        "[package]\nname = \"veh.demo\"\nversion = \"1.0.0\"\n",
    )
    .expect("the manifest is written");
    std::fs::write(
        root.path().join("lane.ridl"),
        "package veh.demo\n\ntype Flag: boolean\n\n\
         interface LaneAssist {\n  signal active: Flag @[100ms..1s]\n}\n\n\
         service veh.demo.lane : LaneAssist\n",
    )
    .expect("the source is written");
    std::fs::write(
        root.path().join("topology.rsdl"),
        "package veh.demo\n\n\
         component Lane { offers veh.demo.lane }\n\
         component Panel { requires LaneAssist }\n\
         system Vehicle { Lane, Panel }\n\
         deployment Good for Vehicle { machine A { Lane, Panel } }\n\
         deployment Bad for Vehicle { machine A { Lane } }\n",
    )
    .expect("the topology is written");
    let (hashes, run) = model_compatible_given(root.path(), "veh.demo");
    assert!(run.has_error(), "the placement error is reported");
    assert_eq!(hashes, vec![GIVEN_HASH.to_string()]);
}
