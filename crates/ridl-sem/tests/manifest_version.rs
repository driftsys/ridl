//! `ridl-sem`'s own `Cargo.toml` cannot write `ridl-core = { workspace = true,
//! default-features = false }`: cargo refuses a `default-features = false`
//! override on a workspace-inherited dependency outright, so that one
//! dependency carries a literal `version` instead, with a comment saying to
//! bump it alongside `[workspace.package].version`. Nothing enforces that by
//! itself — this test is what keeps the literal from drifting silently the
//! next time the workspace version moves.

#[test]
fn ridl_core_dependency_version_matches_the_workspace() {
    #[derive(serde::Deserialize)]
    struct CargoManifest {
        dependencies: Dependencies,
    }
    #[derive(serde::Deserialize)]
    struct Dependencies {
        #[serde(rename = "ridl-core")]
        ridl_core: RidlCoreDependency,
    }
    #[derive(serde::Deserialize)]
    struct RidlCoreDependency {
        version: String,
    }

    let manifest_text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
        .expect("crates/ridl-sem/Cargo.toml must exist");
    let manifest: CargoManifest =
        toml::from_str(&manifest_text).expect("crates/ridl-sem/Cargo.toml must be valid TOML");

    #[derive(serde::Deserialize)]
    struct RootManifest {
        workspace: RootWorkspace,
    }
    #[derive(serde::Deserialize)]
    struct RootWorkspace {
        package: RootWorkspacePackage,
    }
    #[derive(serde::Deserialize)]
    struct RootWorkspacePackage {
        version: String,
    }

    let root_manifest_text =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml"))
            .expect("the workspace root Cargo.toml must exist");
    let root_manifest: RootManifest = toml::from_str(&root_manifest_text)
        .expect("the workspace root Cargo.toml must be valid TOML");

    assert_eq!(
        manifest.dependencies.ridl_core.version, root_manifest.workspace.package.version,
        "crates/ridl-sem/Cargo.toml's ridl-core dependency version must be bumped alongside \
         [workspace.package].version in the root Cargo.toml"
    );
}
