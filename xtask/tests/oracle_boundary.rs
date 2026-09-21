//! The test-time dependency-boundary guard.
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
//! E11.7 stage K8 widened this guard past the two schema compilers.
//! `ridl-backend-rust`'s FlatBuffers conformance test drives planus's
//! runtime and code generator, and emits the `.fbs` it feeds them with
//! `ridl-backend-flatbuffers`. All four edges are test-time only, for the
//! same reason and one more: the E11.7 design note's D-12 keeps a
//! third-party FlatBuffers implementation out of the shipped path, and
//! ADR-0020 decision 9 makes a backend an executable rather than a library
//! other crates link.
//!
//! **The authority is D-12, not ADR-0020 decision 5.** Decision 5 *permits*
//! one FlatBuffers runtime crate, in `ridl-rt` under its `flatbuffers`
//! feature; D-12 leaves that permission unused, so ADR-0020's RA-01
//! dependency ceiling stays unspent. Citing decision 5 as the prohibition
//! would be citing a permission.
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
    /// The crate `package` may reach only through `[dev-dependencies]`.
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
    // E11.7 stage K8 added planus's runtime and code generator to
    // `ridl-backend-rust` for the FlatBuffers codec's conformance test. They
    // are the same kind of oracle under a stronger rule: the design note's
    // D-12 keeps a third-party FlatBuffers implementation out of the shipped
    // path, and this crate emits the codec rather than linking one. A
    // promotion here would put planus behind `ridlc`, and so behind the
    // `ridl` CLI.
    Boundary {
        package: "ridl-backend-rust",
        oracle: "planus",
    },
    Boundary {
        package: "ridl-backend-rust",
        oracle: "planus-codegen",
    },
    Boundary {
        package: "ridl-backend-rust",
        oracle: "planus-translation",
    },
    // Not an oracle, the same rule: the conformance test emits the `.fbs`
    // with the schema backend so that the schema and the codec come from one
    // IR. ADR-0020 decision 9 makes a backend an executable rather than a
    // library other crates link, so that edge stays inside the test binary.
    Boundary {
        package: "ridl-backend-rust",
        oracle: "ridl-backend-flatbuffers",
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

/// Every package reachable from `package` by NORMAL dependency edges only,
/// each mapped to the edge that first reached it, so a failure can print the
/// path rather than only the endpoint.
///
/// **A normal edge is one whose `dep_kinds[].kind` is JSON null.** `"dev"`
/// and `"build"` are the other two, and an edge can carry several kinds at
/// once — a crate that is both a dependency and a dev-dependency has a node
/// with both, which is why the check is "does any kind read null" rather
/// than "does the first".
///
/// **Reachability, not a direct edge.** Checking only `package`'s own edges
/// would pass a promotion one hop away: `planus` moved into `ridl-ir`'s
/// `[dependencies]` puts it in `ridl-backend-rust`'s shipped graph, and so
/// behind `ridlc` and the `ridl` CLI, while producing no direct
/// `ridl-backend-rust` → `planus` edge at all. The walk starts at
/// `package`'s normal edges, so its own dev-dependencies are excluded at the
/// first step and every step after that is normal by construction.
///
/// **An optional dependency is caught whether or not its feature is on.**
/// `cargo metadata` resolves a node's `deps` with the features of the
/// current invocation, but an optional dependency that no feature activates
/// still appears in `packages[].dependencies` and, once any feature in the
/// resolve activates it, in `resolve.nodes[].deps`. This walk reads the
/// resolved nodes, so it catches an optional normal dependency that is
/// activated in this workspace's own resolve — which is the case that would
/// actually ship — and does **not** catch one that nothing activates. That
/// second case cannot reach a downstream build either, so the gap is not a
/// hole in the property this guard states.
fn normal_closure<'a>(
    metadata: &'a serde_json::Value,
    package: &'a str,
) -> HashMap<&'a str, &'a str> {
    let names = package_names(metadata);
    let nodes = metadata["resolve"]["nodes"]
        .as_array()
        .expect("cargo metadata carries `resolve.nodes`");

    // Every node's normal dependency names, by the node's own name.
    let mut normal: HashMap<&str, Vec<&str>> = HashMap::new();
    for node in nodes {
        let id = node["id"].as_str().expect("a node id is a string");
        let from = *names.get(id).expect("every node id names a package");
        let deps = node["deps"]
            .as_array()
            .expect("a resolved node carries a `deps` array")
            .iter()
            .filter(|dep| {
                dep["dep_kinds"]
                    .as_array()
                    .expect("a dependency edge carries a `dep_kinds` array")
                    .iter()
                    .any(|entry| entry["kind"].is_null())
            })
            .map(|dep| {
                let dep_id = dep["pkg"].as_str().expect("a dep's `pkg` is a string");
                *names.get(dep_id).expect("every dep id names a package")
            })
            .collect();
        normal.insert(from, deps);
    }

    assert!(
        normal.contains_key(package),
        "cargo metadata's resolved graph has no node for `{package}` — is it \
         still a workspace member?"
    );

    let mut reached: HashMap<&str, &str> = HashMap::new();
    let mut queue: Vec<&str> = vec![package];
    while let Some(from) = queue.pop() {
        for &to in normal.get(from).into_iter().flatten() {
            if reached.contains_key(to) || to == package {
                continue;
            }
            reached.insert(to, from);
            queue.push(to);
        }
    }
    reached
}

/// The chain of normal edges from `package` to `oracle`, as
/// `a -> b -> oracle`, read back out of [`normal_closure`]'s predecessor
/// map.
fn normal_path(reached: &HashMap<&str, &str>, package: &str, oracle: &str) -> String {
    let mut chain = vec![oracle];
    let mut at = oracle;
    while let Some(&from) = reached.get(at) {
        chain.push(from);
        if from == package {
            break;
        }
        at = from;
    }
    chain.reverse();
    chain.join(" -> ")
}

/// Every [`Boundary`] holds: no oracle is reachable from its backend crate
/// through normal dependency edges, at any distance.
#[test]
fn schema_compilers_stay_dev_dependencies() {
    let metadata = cargo_metadata();
    for boundary in BOUNDARIES {
        let reached = normal_closure(&metadata, boundary.package);
        assert!(
            !reached.contains_key(boundary.oracle),
            "\n\
             `{oracle}` is in `{package}`'s NORMAL dependency closure: \
             {path}\n\
             \n\
             `{oracle}` is test-time only for `{package}` and plays no part \
             in emission — see `{package}`'s own `Cargo.toml` doc comment. A \
             normal edge anywhere on that chain drags `{oracle}` and its \
             whole dependency tree into every downstream build. Move the \
             offending edge back to `[dev-dependencies]`.\n",
            package = boundary.package,
            oracle = boundary.oracle,
            path = normal_path(&reached, boundary.package, boundary.oracle),
        );
    }
}
