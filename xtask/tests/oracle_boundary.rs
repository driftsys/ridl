//! The schema-compiler dependency-boundary guard.
//!
//! `ridl-backend-proto` and `ridl-backend-flatbuffers` each carry a schema
//! compiler — `protox`, `planus-translation` — as a test-time validity
//! oracle: it compiles a backend's emitted schema to prove the backend
//! produced something the real target toolchain accepts, and it plays no
//! part in emission itself. Both crates' own `Cargo.toml` files say so in a
//! doc comment above `[dev-dependencies]`. Promoting either oracle to
//! `[dependencies]` would drag a parser generator and its whole dependency
//! tree (`lalrpop`, `sha3`, `term`, and the rest `cargo metadata` resolves
//! under it) into every downstream build — a real cost paid by everything
//! that depends on `ridlc`, for a guarantee those callers never asked for.
//!
//! `just wasm-check` was believed to catch this and does not: it runs
//! `cargo check --target wasm32-unknown-unknown`, and that target compiles
//! `std::fs` as a stub whose calls fail only at runtime, never at compile
//! time. `protox` and `planus-translation` both use ordinary `std`
//! filesystem calls, so moving either into `[dependencies]` still passes
//! `wasm-check` — proven directly: temporarily promoting either one and
//! running the check still succeeds (recorded in the task report, not
//! repeated here as an automated check, because reproducing it would mean
//! shipping the very promotion this guard exists to prevent).
//!
//! This guard reads the resolved dependency graph instead, via
//! `cargo metadata --format-version 1`, because that is the one place the
//! *kind* of a dependency edge — normal, dev, or build — is recorded
//! unambiguously. The `Cargo.toml` text is not enough on its own: a
//! workspace-inherited dependency (`protox.workspace = true`) reads the
//! same whether it sits under `[dependencies]` or `[dev-dependencies]`, so a
//! textual scan in the `shape_walk.rs` style cannot tell the two apart —
//! only the resolved graph can.

use std::collections::HashMap;
use std::process::Command;

/// One boundary this guard protects: `package` may depend on `oracle` only
/// as a dev-dependency, never as a normal one.
struct Boundary {
    /// The backend crate under the constraint.
    package: &'static str,
    /// The schema-compiler crate `package` may reach only through
    /// `[dev-dependencies]`.
    oracle: &'static str,
}

const BOUNDARIES: &[Boundary] = &[
    Boundary {
        package: "ridl-backend-flatbuffers",
        oracle: "planus-translation",
    },
    Boundary {
        package: "ridl-backend-proto",
        oracle: "protox",
    },
];

/// Runs `cargo metadata --format-version 1 --locked` and parses its stdout
/// as JSON. `--locked` matches every other cargo invocation this workspace's
/// gate makes (`justfile`): the lockfile is already the resolved graph, and
/// this guard must read that graph, not silently re-resolve a different one.
fn cargo_metadata() -> serde_json::Value {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--locked"])
        .output()
        .expect("`cargo metadata` must run");
    assert!(
        output.status.success(),
        "`cargo metadata` failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("`cargo metadata` must print valid JSON")
}

/// A package id (`cargo metadata`'s pkgid string, e.g.
/// `path+file:///.../ridl-ir#0.0.0`) to the plain crate name it resolves to.
fn package_names(metadata: &serde_json::Value) -> HashMap<&str, &str> {
    metadata["packages"]
        .as_array()
        .expect("cargo metadata carries a `packages` array")
        .iter()
        .map(|pkg| {
            let id = pkg["id"].as_str().expect("a package id is a string");
            let name = pkg["name"].as_str().expect("a package name is a string");
            (id, name)
        })
        .collect()
}

/// The dependency kinds of every resolved edge from `package` to `oracle`:
/// `cargo metadata`'s `dep_kinds[].kind`, where `None` marks a NORMAL
/// dependency (`Some("dev")` and `Some("build")` are the other two). Empty
/// when `package` does not depend on `oracle` at all.
fn dependency_kinds(
    metadata: &serde_json::Value,
    package: &str,
    oracle: &str,
) -> Vec<Option<String>> {
    let names = package_names(metadata);
    let nodes = metadata["resolve"]["nodes"]
        .as_array()
        .expect("cargo metadata carries `resolve.nodes`");
    let node = nodes
        .iter()
        .find(|node| {
            let id = node["id"].as_str().expect("a node id is a string");
            names.get(id) == Some(&package)
        })
        .unwrap_or_else(|| panic!("cargo metadata's resolved graph has no node for `{package}` — is it still a workspace member?"));

    node["deps"]
        .as_array()
        .expect("a resolved node carries a `deps` array")
        .iter()
        .filter(|dep| {
            let dep_id = dep["pkg"].as_str().expect("a dep's `pkg` is a string");
            names.get(dep_id) == Some(&oracle)
        })
        .flat_map(|dep| {
            dep["dep_kinds"]
                .as_array()
                .expect("a dependency edge carries a `dep_kinds` array")
                .iter()
                .map(|entry| entry["kind"].as_str().map(str::to_string))
        })
        .collect()
}

/// Every [`Boundary`] holds: neither schema-compiler oracle reaches its
/// backend crate as a normal dependency in the resolved graph.
#[test]
fn schema_compilers_stay_dev_dependencies() {
    let metadata = cargo_metadata();
    for boundary in BOUNDARIES {
        let kinds = dependency_kinds(&metadata, boundary.package, boundary.oracle);
        assert!(
            !kinds.iter().any(Option::is_none),
            "\n\
             `{package}` depends on `{oracle}` as a NORMAL dependency in the \
             resolved graph (resolved kinds: {kinds:?}).\n\
             \n\
             `{oracle}` is a test-time validity oracle for `{package}`, not \
             part of emission — see `{package}`'s own `Cargo.toml` doc \
             comment. Promoting it out of `[dev-dependencies]` drags a \
             schema compiler and its whole dependency tree into every \
             downstream build. Move `{oracle}` back to \
             `[dev-dependencies]`.\n",
            package = boundary.package,
            oracle = boundary.oracle,
            kinds = kinds,
        );
    }
}
