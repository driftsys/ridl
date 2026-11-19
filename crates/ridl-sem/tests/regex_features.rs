//! The checker and generated Rust code require the default `regex` features.
//! In particular, Unicode ranges and normalised dots need `unicode`. Workspace
//! feature unification can hide a dependency disabling defaults, so this test
//! reads the inherited manifest entries as well (driftsys/ridl#605).

/// Whether a dependency entry, in either manifest, writes
/// `default-features = false`.
fn turns_default_features_off(entry: &toml::Value) -> bool {
    entry.get("default-features").and_then(toml::Value::as_bool) == Some(false)
}

fn read_manifest(path: &str) -> toml::Value {
    let text = std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{path}: {error}"));
    toml::from_str(&text).unwrap_or_else(|error| panic!("{path} must be valid TOML: {error}"))
}

#[test]
fn regex_dependency_keeps_its_default_features() {
    let manifest = read_manifest(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
    let entry = manifest
        .get("dependencies")
        .and_then(|dependencies| dependencies.get("regex"))
        .expect("crates/ridl-sem/Cargo.toml declares a `regex` dependency");
    assert!(
        !turns_default_features_off(entry),
        "crates/ridl-sem/Cargo.toml turns the `regex` crate's default features off; the \
         checker needs them (TYPL-220 compiles patterns with non-ASCII characters only with `unicode`)"
    );
    if entry.get("workspace").and_then(toml::Value::as_bool) == Some(true) {
        let root = read_manifest(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml"));
        let inherited = root
            .get("workspace")
            .and_then(|workspace| workspace.get("dependencies"))
            .and_then(|dependencies| dependencies.get("regex"))
            .expect(
                "the workspace root Cargo.toml declares the `regex` dependency ridl-sem inherits",
            );
        assert!(
            !turns_default_features_off(inherited),
            "the workspace root Cargo.toml turns the `regex` crate's default features off, and \
             ridl-sem inherits that entry; the checker needs them (TYPL-220 compiles patterns with non-ASCII characters only with `unicode`)"
        );
    }
}
