//! The checker compiles every `match` pattern with the `regex` crate and its
//! default features (TYPL-220): the `unicode` feature, on by default, is what
//! makes `\w`, `\d` and `\p{L}` compile, and the generated crate's
//! `regex = "1.13"` dependency has it on too. A unit test on `\p{L}` cannot
//! catch `ridl-sem` turning the default features off, because cargo unifies
//! features across the workspace and other members (lalrpop, proptest,
//! logos-codegen) turn `unicode` back on, while an isolated build of
//! `ridl-sem` with the same manifest fails on `\p{L}` and `\w`
//! (driftsys/ridl#605). So this test reads the dependency spec itself:
//! `ridl-sem`'s `regex` entry and, when that entry is inherited, the
//! workspace's.

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
         checker needs them (TYPL-220 compiles `\\w` and `\\p{{..}}` only with `unicode`)"
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
             ridl-sem inherits that entry; the checker needs them (TYPL-220 compiles `\\w` and \
             `\\p{{..}}` only with `unicode`)"
        );
    }
}
