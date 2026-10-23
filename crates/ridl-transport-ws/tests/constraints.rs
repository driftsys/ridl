//! The constraints of the crate's design that a build does not check on its
//! own, read from the sources and the manifests: the codec needs `core`
//! only, with no `alloc`; the crate's one unconditional dependency is
//! `ridl-rt` with no feature, which the `server` feature does not change; and
//! the `descriptor` feature adds `ridl-descriptor` with its default features
//! off. The checks
//! are textual, so a dependency or an `alloc` use written in a form they do
//! not read is a change to make here too.

use std::fs;
use std::path::Path;

/// A file of the crate, read whole.
fn crate_file(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The lines of the `[name]` table of `manifest`, trimmed, without comments
/// and blank lines.
fn table(manifest: &str, name: &str) -> Vec<String> {
    let header = format!("[{name}]");
    let mut lines = Vec::new();
    let mut inside = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            inside = line == header;
            continue;
        }
        if inside && !line.is_empty() && !line.starts_with('#') {
            lines.push(line.to_owned());
        }
    }
    lines
}

/// The codec and the crate root name no `alloc` crate: no `extern crate
/// alloc`, no `alloc::` path, outside comments.
#[test]
fn the_codec_names_no_alloc_crate() {
    for file in ["src/frame.rs", "src/lib.rs"] {
        let source = crate_file(file);
        for (i, line) in source.lines().enumerate() {
            let code = line.split("//").next().unwrap_or_default();
            assert!(!code.contains("alloc"), "{file}:{}: {line}", i + 1);
        }
    }
}

/// The one unconditional dependency is `ridl-rt`, through a workspace entry
/// that names no feature; the one optional dependency is `ridl-descriptor`,
/// through a workspace entry with its default features off, under the
/// `descriptor` feature. No dependency table of another form exists.
#[test]
fn the_dependencies_are_ridl_rt_and_an_optional_ridl_descriptor() {
    let manifest = crate_file("Cargo.toml");
    assert_eq!(
        table(&manifest, "dependencies"),
        [
            "ridl-rt.workspace = true",
            "ridl-descriptor = { workspace = true, optional = true }"
        ]
    );
    for form in ["[dependencies.", "[target.", "[build-dependencies"] {
        assert!(!manifest.contains(form), "a {form} table in the manifest");
    }
    let workspace = crate_file("../../Cargo.toml");
    let entries = table(&workspace, "workspace.dependencies");
    let rt = entries
        .iter()
        .find(|line| line.starts_with("ridl-rt "))
        .expect("the workspace manifest names ridl-rt");
    assert!(!rt.contains("features"), "the workspace entry: {rt}");
    let descriptor = entries
        .iter()
        .find(|line| line.starts_with("ridl-descriptor "))
        .expect("the workspace manifest names ridl-descriptor");
    assert!(
        descriptor.contains("default-features = false"),
        "the workspace entry: {descriptor}"
    );
}

/// The `server` feature turns on nothing in `ridl-rt`, and nothing else; the
/// `descriptor` feature turns on `server` and the optional `ridl-descriptor`.
#[test]
fn the_features_are_server_and_descriptor() {
    let manifest = crate_file("Cargo.toml");
    assert_eq!(
        table(&manifest, "features"),
        [
            "default = [\"server\"]",
            "server = []",
            "descriptor = [\"server\", \"dep:ridl-descriptor\"]"
        ]
    );
}
