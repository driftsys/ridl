//! The guard on the `ridlc` -> `ridl-diff` dependency edge.
//!
//! `ridlc` reads `.ir.json` snapshots for `ridl diff`'s sides through
//! `ridl_ir::v2::load_ir_json`, and does not depend on `ridl-diff`: the
//! compiler library sits below the diff engine, and the `ridl` CLI and
//! `ridl-mcp` are the crates that combine the two (driftsys/ridl#786). A
//! normal edge from `ridlc` to `ridl-diff`, direct or through another crate,
//! would put the diff engine back under the compiler.
//!
//! This guard reads the resolved dependency graph from
//! `cargo metadata --format-version 1 --locked`, for the reason
//! `oracle_boundary.rs` gives: only the resolved graph records the kind of an
//! edge. It walks normal edges only (a `dep_kinds[].kind` of JSON null), so a
//! dev-dependency on `ridl-diff` in a `ridlc` test would pass it.

use std::collections::HashMap;
use std::process::Command;

/// The crate that must not reach [`FORBIDDEN`].
const PACKAGE: &str = "ridlc";

/// The crate [`PACKAGE`] must not reach through normal dependency edges.
const FORBIDDEN: &str = "ridl-diff";

/// Runs `cargo metadata --format-version 1 --locked` over this workspace and
/// parses its stdout as JSON.
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

/// Every resolved node's normal dependency names, by the node's own name.
fn normal_edges(metadata: &serde_json::Value) -> HashMap<&str, Vec<&str>> {
    let names: HashMap<&str, &str> = metadata["packages"]
        .as_array()
        .expect("cargo metadata carries a `packages` array")
        .iter()
        .map(|pkg| {
            let id = pkg["id"].as_str().expect("a package id is a string");
            let name = pkg["name"].as_str().expect("a package name is a string");
            (id, name)
        })
        .collect();

    let mut edges: HashMap<&str, Vec<&str>> = HashMap::new();
    for node in metadata["resolve"]["nodes"]
        .as_array()
        .expect("cargo metadata carries `resolve.nodes`")
    {
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
        edges.insert(from, deps);
    }
    edges
}

/// Every package reachable from `package` by the given edges, mapped to the
/// package it was first reached from.
fn reach<'a>(
    edges: &HashMap<&'a str, Vec<&'a str>>,
    package: &'a str,
) -> HashMap<&'a str, &'a str> {
    assert!(
        edges.contains_key(package),
        "cargo metadata's resolved graph has no node for `{package}` — is it \
         still a workspace member?"
    );
    let mut reached: HashMap<&str, &str> = HashMap::new();
    let mut queue = vec![package];
    while let Some(from) = queue.pop() {
        for &to in edges.get(from).into_iter().flatten() {
            if to != package && !reached.contains_key(to) {
                reached.insert(to, from);
                queue.push(to);
            }
        }
    }
    reached
}

/// The chain of edges from `package` to `target`, as `a -> b -> target`.
fn path(reached: &HashMap<&str, &str>, package: &str, target: &str) -> String {
    let mut chain = vec![target];
    let mut at = target;
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

#[test]
fn ridlc_does_not_depend_on_ridl_diff() {
    let metadata = cargo_metadata();
    let edges = normal_edges(&metadata);
    let reached = reach(&edges, PACKAGE);
    assert!(
        !reached.contains_key(FORBIDDEN),
        "`{FORBIDDEN}` is in `{PACKAGE}`'s normal dependency closure: {path}. \
         `{PACKAGE}` loads `.ir.json` snapshots through \
         `ridl_ir::v2::load_ir_json` and must not depend on the diff engine \
         (driftsys/ridl#786).",
        path = path(&reached, PACKAGE, FORBIDDEN),
    );
}
