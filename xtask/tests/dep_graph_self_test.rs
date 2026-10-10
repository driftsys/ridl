//! Self-tests of the graph helpers in `dep_graph/mod.rs`, over a small
//! synthetic `cargo metadata` document, so that a helper bug which would
//! disable `oracle_boundary.rs` and `ridlc_diff_boundary.rs` fails here.
//!
//! The graph, by crate name (`dupe` resolves to two versions):
//!
//! ```text
//! app -> mid        normal
//! app -> dupe 1.0   normal
//! app -> buildy     build
//! app -> devonly    dev
//! app -> both       dev and normal
//! mid -> dupe 2.0   normal
//! mid -> devdeep    dev
//! dupe 2.0 -> target normal
//! ```
//!
//! `dupe 1.0` has no dependency and comes after `dupe 2.0` in
//! `resolve.nodes`, so a walk that keyed nodes by name would lose the edge
//! `dupe 2.0 -> target`.

// `cargo_metadata` runs cargo; the guards exercise it, not this file.
#[allow(dead_code)]
mod dep_graph;

use dep_graph::{edges, member_id, package_names, path_to, reach};
use serde_json::{Value, json};

fn package(id: &str, name: &str) -> Value {
    json!({ "id": id, "name": name })
}

fn dep(pkg: &str, kinds: &[Option<&str>]) -> Value {
    let kinds: Vec<Value> = kinds.iter().map(|kind| json!({ "kind": kind })).collect();
    json!({ "pkg": pkg, "dep_kinds": kinds })
}

fn node(id: &str, deps: Vec<Value>) -> Value {
    json!({ "id": id, "deps": deps })
}

fn metadata() -> Value {
    json!({
        "packages": [
            package("other#0.1.0", "other"),
            package("app#0.1.0", "app"),
            package("mid#0.1.0", "mid"),
            package("dupe#1.0.0", "dupe"),
            package("dupe#2.0.0", "dupe"),
            package("target#0.1.0", "target"),
            package("buildy#0.1.0", "buildy"),
            package("devonly#0.1.0", "devonly"),
            package("devdeep#0.1.0", "devdeep"),
            package("both#0.1.0", "both"),
        ],
        "workspace_members": ["other#0.1.0", "app#0.1.0"],
        "resolve": { "nodes": [
            node("other#0.1.0", vec![dep("app#0.1.0", &[None])]),
            node("app#0.1.0", vec![
                dep("mid#0.1.0", &[None]),
                dep("dupe#1.0.0", &[None]),
                dep("buildy#0.1.0", &[Some("build")]),
                dep("devonly#0.1.0", &[Some("dev")]),
                dep("both#0.1.0", &[Some("dev"), None]),
            ]),
            node("mid#0.1.0", vec![
                dep("dupe#2.0.0", &[None]),
                dep("devdeep#0.1.0", &[Some("dev")]),
            ]),
            node("dupe#2.0.0", vec![dep("target#0.1.0", &[None])]),
            node("dupe#1.0.0", vec![]),
            node("target#0.1.0", vec![]),
            node("buildy#0.1.0", vec![]),
            node("devonly#0.1.0", vec![]),
            node("devdeep#0.1.0", vec![]),
            node("both#0.1.0", vec![]),
        ]},
    })
}

#[test]
fn edges_keep_the_kinds_asked_for() {
    let metadata = metadata();
    let shipped = edges(&metadata, |kind| kind != Some("dev"));
    assert_eq!(
        shipped["app#0.1.0"],
        ["mid#0.1.0", "dupe#1.0.0", "buildy#0.1.0", "both#0.1.0"]
    );
    assert_eq!(shipped["mid#0.1.0"], ["dupe#2.0.0"]);
    assert_eq!(shipped["dupe#2.0.0"], ["target#0.1.0"]);
    let normal = edges(&metadata, |kind| kind.is_none());
    assert_eq!(
        normal["app#0.1.0"],
        ["mid#0.1.0", "dupe#1.0.0", "both#0.1.0"]
    );
}

#[test]
fn member_id_picks_the_member_of_that_name() {
    let metadata = metadata();
    assert_eq!(member_id(&metadata, "app"), "app#0.1.0");
    assert_eq!(member_id(&metadata, "other"), "other#0.1.0");
}

#[test]
fn reach_follows_transitive_edges_through_both_versions() {
    let metadata = metadata();
    let shipped = edges(&metadata, |kind| kind != Some("dev"));
    let reached = reach(&shipped, &shipped, member_id(&metadata, "app"));
    assert_eq!(reached.get("target#0.1.0"), Some(&"dupe#2.0.0"));
    assert_eq!(reached.get("dupe#2.0.0"), Some(&"mid#0.1.0"));
    assert_eq!(reached.get("dupe#1.0.0"), Some(&"app#0.1.0"));
    assert!(!reached.contains_key("devonly#0.1.0"));
    assert!(!reached.contains_key("app#0.1.0"));
}

#[test]
fn reach_uses_the_first_edges_only_for_the_first_step() {
    let metadata = metadata();
    let any = edges(&metadata, |_| true);
    let shipped = edges(&metadata, |kind| kind != Some("dev"));
    let reached = reach(&any, &shipped, member_id(&metadata, "app"));
    assert!(reached.contains_key("devonly#0.1.0"), "{reached:?}");
    assert!(!reached.contains_key("devdeep#0.1.0"), "{reached:?}");
    assert!(reached.contains_key("target#0.1.0"), "{reached:?}");
}

#[test]
fn path_to_names_the_whole_chain() {
    let metadata = metadata();
    let names = package_names(&metadata);
    let shipped = edges(&metadata, |kind| kind != Some("dev"));
    let reached = reach(&shipped, &shipped, member_id(&metadata, "app"));
    assert_eq!(
        path_to(&names, &reached, "target").as_deref(),
        Some("app -> mid -> dupe -> target")
    );
    assert_eq!(path_to(&names, &reached, "devonly"), None);
}
