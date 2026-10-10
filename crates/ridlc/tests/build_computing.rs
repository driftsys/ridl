//! `run_build_computing` computes the earlier catalogs from the build's own
//! compile: the function is called once, with the build's checked packages,
//! and never for a build with an error diagnostic.

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
    let run = build(root.path(), &mut |packages, _std, names_imports| {
        calls.push((
            packages.iter().map(|p| p.name.clone()).collect::<Vec<_>>(),
            names_imports,
        ));
        Ok(BTreeMap::new())
    })
    .expect("the build runs");
    assert!(!run.has_error(), "unexpected error: {:?}", run.diagnostics);
    assert_eq!(calls, vec![(vec!["demo".to_string()], false)]);
}

#[test]
fn a_build_with_an_error_diagnostic_never_calls_it() {
    let root = workspace("package demo\n\nstruct S {\n  x: Missing\n}\n");
    let mut called = false;
    let run = build(root.path(), &mut |_, _, _| {
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
    let error = build(root.path(), &mut |_, _, _| {
        Err(std::io::Error::other("the baseline cannot be read"))
    })
    .err()
    .expect("the build fails");
    assert_eq!(error.to_string(), "the baseline cannot be read");
}

#[test]
fn a_workspace_that_names_imports_is_reported_to_the_function() {
    let root = workspace(SOURCE);
    std::fs::write(
        root.path().join("ridl.toml"),
        "[package]\nname = \"demo\"\nversion = \"1.0.0\"\n\n[imports]\next = \"https://example.invalid/ext.git\"\n",
    )
    .expect("the manifest is written");
    let mut flags = Vec::new();
    let _ = build(root.path(), &mut |_, _, names_imports| {
        flags.push(names_imports);
        Ok(BTreeMap::new())
    });
    assert_eq!(flags, vec![true]);
}
