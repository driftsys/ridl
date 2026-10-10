//! Guards the version literals that prose names. `git std bump` rewrites only
//! the root `Cargo.toml`, so a version written in a README or a design
//! document goes stale at the next release unless a test compares it with the
//! workspace version.

use std::path::Path;

/// The workspace version from `[workspace.package]` in the root manifest.
fn workspace_version() -> String {
    #[derive(serde::Deserialize)]
    struct Root {
        workspace: Workspace,
    }
    #[derive(serde::Deserialize)]
    struct Workspace {
        package: Package,
    }
    #[derive(serde::Deserialize)]
    struct Package {
        version: String,
    }
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml"))
        .expect("the workspace root Cargo.toml must exist");
    let root: Root = toml::from_str(&text).expect("the root Cargo.toml must be valid TOML");
    root.workspace.package.version
}

/// Fails unless `file` (relative to the repository root) contains `pin`.
/// A missing file or a missing pin fails, so the check cannot pass vacuously.
fn assert_pin(file: &str, pin: &str, version: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(file);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{file} must exist and be readable: {e}"));
    assert!(
        text.contains(pin),
        "{file} must contain `{pin}` to match the workspace version {version}. \
         A release moves the workspace version and leaves this prose behind: \
         update the pin in {file}, and check its `[[version_files]]` entry \
         in .git-std.toml."
    );
}

#[test]
fn conformance_readme_pin_matches_the_workspace_version() {
    let version = workspace_version();
    assert_pin(
        "crates/ridl-rt-conformance/README.md",
        &format!("ridl-rt-conformance = \"={version}\""),
        &version,
    );
}

#[test]
fn writing_a_port_pins_match_the_workspace_version() {
    let version = workspace_version();
    assert_pin(
        "docs/book/writing-a-port.md",
        &format!("ridl-rt = \"{version}\""),
        &version,
    );
    assert_pin(
        "docs/book/writing-a-port.md",
        &format!("ridl-rt-conformance = \"={version}\""),
        &version,
    );
}
