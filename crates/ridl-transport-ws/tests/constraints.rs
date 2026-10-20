//! The constraints of the crate's design that a build does not check on its
//! own, read from the sources and the manifests: the codec needs `core`
//! only, with no `alloc`; and the crate's one normal dependency is `ridl-rt`
//! with no feature, which the `server` feature does not change. The checks
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

/// The only normal dependency is `ridl-rt`, through the workspace entry,
/// which names no feature; no dependency table of another form exists.
#[test]
fn the_only_normal_dependency_is_ridl_rt_with_no_feature() {
    let manifest = crate_file("Cargo.toml");
    assert_eq!(
        table(&manifest, "dependencies"),
        ["ridl-rt.workspace = true"]
    );
    for form in ["[dependencies.", "[target.", "[build-dependencies"] {
        assert!(!manifest.contains(form), "a {form} table in the manifest");
    }
    let workspace = crate_file("../../Cargo.toml");
    let entry = table(&workspace, "workspace.dependencies")
        .into_iter()
        .find(|line| line.starts_with("ridl-rt "))
        .expect("the workspace manifest names ridl-rt");
    assert!(!entry.contains("features"), "the workspace entry: {entry}");
}

/// The `server` feature turns on nothing in `ridl-rt`, and nothing else.
#[test]
fn the_server_feature_turns_on_no_ridl_rt_feature() {
    let manifest = crate_file("Cargo.toml");
    assert_eq!(
        table(&manifest, "features"),
        ["default = [\"server\"]", "server = []"]
    );
}
