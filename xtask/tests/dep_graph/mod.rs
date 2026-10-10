//! The resolved-dependency-graph helpers that `oracle_boundary.rs` and
//! `ridlc_diff_boundary.rs` share.
//!
//! Nodes are keyed by package id (`cargo metadata`'s pkgid string), not by
//! crate name: two resolved versions of one crate are two nodes, and a walk
//! must follow both.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

/// Runs `cargo metadata --format-version 1 --locked` for the workspace whose
/// root manifest is `manifest`, or for this workspace when it is `None`, and
/// parses its stdout as JSON. `--locked` makes the guard read the graph that
/// `Cargo.lock` records, and fail when the lockfile is out of date, rather
/// than silently re-resolve a different one. With `all_features`, every feature of every workspace
/// member is on, so the resolved graph holds every optional dependency,
/// including one that no feature in the workspace turns on.
pub fn cargo_metadata(manifest: Option<&Path>, all_features: bool) -> serde_json::Value {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let mut command = Command::new(cargo);
    command.args(["metadata", "--format-version", "1", "--locked"]);
    if all_features {
        command.arg("--all-features");
    }
    if let Some(manifest) = manifest {
        command.arg("--manifest-path").arg(manifest);
    }
    let output = command.output().expect("`cargo metadata` must run");
    assert!(
        output.status.success(),
        "`cargo metadata` failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("`cargo metadata` must print valid JSON")
}

/// A package id (`cargo metadata`'s pkgid string, e.g.
/// `path+file:///.../ridl-ir#0.0.0`) to the plain crate name it resolves to.
pub fn package_names(metadata: &serde_json::Value) -> HashMap<&str, &str> {
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

/// The package id of the workspace member named `name`.
pub fn member_id<'a>(metadata: &'a serde_json::Value, name: &str) -> &'a str {
    let names = package_names(metadata);
    metadata["workspace_members"]
        .as_array()
        .expect("cargo metadata carries `workspace_members`")
        .iter()
        .map(|id| id.as_str().expect("a member id is a string"))
        .find(|id| names.get(id) == Some(&name))
        .unwrap_or_else(|| panic!("`{name}` is not a member of this workspace"))
}

/// Every resolved node's dependencies, by package id, keeping an edge when
/// `keep` accepts any of its kinds. A kind is `None` for a normal edge (JSON
/// null), otherwise `"dev"` or `"build"`; an edge can carry several kinds at
/// once, which is why the check is "does any kind pass".
pub fn edges(
    metadata: &serde_json::Value,
    keep: impl Fn(Option<&str>) -> bool,
) -> HashMap<&str, Vec<&str>> {
    let nodes = metadata["resolve"]["nodes"]
        .as_array()
        .expect("cargo metadata carries `resolve.nodes`");

    let mut edges: HashMap<&str, Vec<&str>> = HashMap::new();
    for node in nodes {
        let id = node["id"].as_str().expect("a node id is a string");
        let deps = node["deps"]
            .as_array()
            .expect("a resolved node carries a `deps` array")
            .iter()
            .filter(|dep| {
                dep["dep_kinds"]
                    .as_array()
                    .expect("a dependency edge carries a `dep_kinds` array")
                    .iter()
                    .any(|entry| keep(entry["kind"].as_str()))
            })
            .map(|dep| dep["pkg"].as_str().expect("a dep's `pkg` is a string"))
            .collect();
        edges.insert(id, deps);
    }
    edges
}

/// Walks from the package id `start`: its own edges in `first`, every later
/// step's in `rest`. Returns each package id reached, mapped to the id it was
/// first reached from.
pub fn reach<'a>(
    first: &HashMap<&'a str, Vec<&'a str>>,
    rest: &HashMap<&'a str, Vec<&'a str>>,
    start: &'a str,
) -> HashMap<&'a str, &'a str> {
    assert!(
        first.contains_key(start),
        "cargo metadata's resolved graph has no node for `{start}`"
    );

    let mut reached: HashMap<&str, &str> = HashMap::new();
    let mut queue: Vec<&str> = vec![start];
    while let Some(from) = queue.pop() {
        let step = if from == start { first } else { rest };
        for &to in step.get(from).into_iter().flatten() {
            if reached.contains_key(to) || to == start {
                continue;
            }
            reached.insert(to, from);
            queue.push(to);
        }
    }
    reached
}

/// The chain of edges from the walk's start to the first reached package
/// named `target`, as `a -> b -> target`, or `None` when no package of that
/// name was reached. The start is the one package on the chain that
/// `reached` has no entry for, which ends the walk back.
pub fn path_to(
    names: &HashMap<&str, &str>,
    reached: &HashMap<&str, &str>,
    target: &str,
) -> Option<String> {
    let mut at = *reached.keys().find(|id| names.get(*id) == Some(&target))?;
    let mut chain = vec![target];
    while let Some(&from) = reached.get(at) {
        chain.push(names.get(from).copied().unwrap_or(from));
        at = from;
    }
    chain.reverse();
    Some(chain.join(" -> "))
}
