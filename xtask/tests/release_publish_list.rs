//! The check on the crates.io release workflow's publish list.
//!
//! `.github/workflows/crates-io-release.yml` publishes crates from a
//! hand-written list of `publish <crate>` lines. Without this test, deleting a
//! line leaves that crate unpublished, and listing a crate before one it
//! depends on makes the release job fail after earlier crates are already on
//! crates.io. Neither change fails any other gate.
//!
//! The test checks two properties against `cargo metadata`:
//!
//! 1. The listed crates are exactly the workspace members whose manifest does
//!    not set `publish = false`.
//! 2. Each listed crate comes after every workspace crate it depends on
//!    through a dependency that `cargo publish` keeps, and no listed crate
//!    depends on a workspace crate that is not published.
//!
//! The kept dependencies are the normal and build dependencies, and a
//! dev-dependency that carries a version requirement. `cargo publish`
//! removes a dev-dependency that names a path and no version from the
//! packaged manifest, and this repository writes most internal
//! dev-dependencies that way (`ridlc = { path = "../ridlc" }`), so those need
//! not be on crates.io first; several of them form cycles with a normal
//! dependency, which no publish order could satisfy. `cargo metadata` reports
//! a dependency without a version requirement as `*`. A dev-dependency with a
//! version (for example `name.workspace = true`) stays in the packaged
//! manifest, so the order check includes it.
//!
//! The checking logic takes plain data, so the unit tests at the end feed it
//! small synthetic workflow texts and `cargo metadata` documents.
//!
//! `oracle_boundary.rs` has its own `cargo metadata` helper. This test does
//! not share it: that helper reads the resolved graph (no `--no-deps`) and
//! takes a manifest path and a feature switch, while this test reads only the
//! member list, so one helper would need those parameters and a common module
//! compiled into every test binary.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::process::Command;

const WORKFLOW: &str = ".github/workflows/crates-io-release.yml";

/// A workspace member.
struct Member {
    name: String,
    /// Whether `cargo publish` can send the crate to crates.io.
    publishable: bool,
    /// Workspace crates that `cargo publish` keeps as dependencies of the
    /// packaged manifest.
    internal_deps: BTreeSet<String>,
}

/// What the workflow's `publish` lines hold.
struct PublishList {
    /// The crate names of the `publish <crate>` calls, in file order.
    calls: Vec<String>,
    /// Lines that start with the word `publish` and are not a plain call.
    unreadable: Vec<String>,
}

/// Reads the `publish <crate>` calls. A call is a line that holds exactly the
/// word `publish` and one plain crate name. The function definition
/// (`publish() {`) and comments do not start with the word `publish`. Any
/// other line that does is recorded as unreadable, so a trailing comment or a
/// quoted name fails loudly instead of hiding a crate.
fn parse_publish_list(workflow: &str) -> PublishList {
    let mut list = PublishList {
        calls: Vec::new(),
        unreadable: Vec::new(),
    };
    for line in workflow.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        if words.first() != Some(&"publish") {
            continue;
        }
        match words.as_slice() {
            [_, name]
                if name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') =>
            {
                list.calls.push((*name).to_string());
            }
            _ => list.unreadable.push(line.trim().to_string()),
        }
    }
    list
}

/// Every problem found between the publish list and the workspace members.
/// An empty result means the list is complete and in dependency order.
fn problems(list: &PublishList, members: &[Member]) -> Vec<String> {
    let mut found = Vec::new();
    for line in &list.unreadable {
        found.push(format!("cannot read this publish line: `{line}`"));
    }
    let calls = &list.calls;
    if calls.is_empty() {
        found.push("the workflow holds no `publish <crate>` line".to_string());
        return found;
    }

    let publishable: Vec<&Member> = members.iter().filter(|m| m.publishable).collect();
    let listed: BTreeSet<&str> = calls.iter().map(String::as_str).collect();
    let expected: BTreeSet<&str> = publishable.iter().map(|m| m.name.as_str()).collect();
    for name in expected.difference(&listed) {
        found.push(format!("`{name}` is publishable but has no `publish` line"));
    }
    for name in listed.difference(&expected) {
        found.push(format!(
            "`publish {name}` names a crate that is not a publishable workspace member"
        ));
    }
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for name in calls {
        if !seen.insert(name) {
            found.push(format!("`publish {name}` appears more than once"));
        }
    }

    // The first occurrence of a crate decides its position.
    let mut position: HashMap<&str, usize> = HashMap::new();
    for (index, name) in calls.iter().enumerate() {
        position.entry(name.as_str()).or_insert(index);
    }
    let unpublished: BTreeSet<&str> = members
        .iter()
        .filter(|m| !m.publishable)
        .map(|m| m.name.as_str())
        .collect();
    for member in publishable {
        let own = position.get(member.name.as_str()).copied();
        for dependency in &member.internal_deps {
            if unpublished.contains(dependency.as_str()) {
                found.push(format!(
                    "`{}` depends on `{dependency}`, which is not published to crates.io",
                    member.name
                ));
                continue;
            }
            // A publishable dependency with no `publish` line is reported by
            // the completeness check above.
            if let (Some(own), Some(&before)) = (own, position.get(dependency.as_str()))
                && before > own
            {
                found.push(format!(
                    "`{}` depends on `{dependency}` but is published before it",
                    member.name
                ));
            }
        }
    }
    found
}

/// The workspace members described by a `cargo metadata --no-deps` document.
fn members_from_metadata(metadata: &serde_json::Value) -> Vec<Member> {
    let packages = metadata["packages"]
        .as_array()
        .expect("cargo metadata carries a `packages` array");
    let names: BTreeSet<&str> = packages
        .iter()
        .map(|p| p["name"].as_str().expect("a package has a name"))
        .collect();
    packages
        .iter()
        .map(|p| Member {
            name: p["name"].as_str().unwrap().to_string(),
            // `publish = false` is reported as an empty list and no
            // `publish` key as null. A list of registries publishes to
            // crates.io only when it names it.
            publishable: match p["publish"].as_array() {
                None => true,
                Some(registries) => registries.iter().any(|r| r.as_str() == Some("crates-io")),
            },
            internal_deps: p["dependencies"]
                .as_array()
                .expect("a package carries `dependencies`")
                .iter()
                // A workspace member is a path dependency; a crates.io
                // dependency that shares a member's name has no `path`.
                .filter(|d| d["path"].is_string())
                .filter(|d| match d["kind"].as_str() {
                    // `kind` is null for a normal dependency.
                    None | Some("build") => true,
                    // `req` is `*` when the dependency names no version.
                    Some("dev") => d["req"].as_str() != Some("*"),
                    Some(_) => false,
                })
                .filter_map(|d| d["name"].as_str())
                .filter(|name| names.contains(name))
                .map(str::to_string)
                .collect(),
        })
        .collect()
}

/// The members of this workspace, read from `cargo metadata`.
fn workspace_members() -> Vec<Member> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--no-deps", "--locked"])
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
        .output()
        .expect("`cargo metadata` must run");
    assert!(
        output.status.success(),
        "`cargo metadata` failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("`cargo metadata` must print valid JSON");
    members_from_metadata(&metadata)
}

#[test]
fn the_release_publish_list_is_complete_and_in_dependency_order() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(WORKFLOW);
    let workflow = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    let found = problems(&parse_publish_list(&workflow), &workspace_members());
    assert!(
        found.is_empty(),
        "{WORKFLOW} publish list is wrong:\n  {}",
        found.join("\n  ")
    );
}

mod unit {
    use super::*;
    use serde_json::json;

    fn member(name: &str, deps: &[&str]) -> Member {
        Member {
            name: name.to_string(),
            publishable: true,
            internal_deps: deps.iter().map(|d| d.to_string()).collect(),
        }
    }

    fn list(names: &[&str]) -> PublishList {
        PublishList {
            calls: names.iter().map(|n| n.to_string()).collect(),
            unreadable: Vec::new(),
        }
    }

    fn members() -> Vec<Member> {
        vec![
            member("a", &[]),
            member("b", &["a"]),
            member("c", &["a", "b"]),
        ]
    }

    /// A metadata document for packages given as `(name, publish, deps)`,
    /// where a dependency is `(name, kind, req, has_path)`.
    type Dep = (&'static str, Option<&'static str>, &'static str, bool);
    fn metadata(packages: &[(&str, serde_json::Value, Vec<Dep>)]) -> serde_json::Value {
        let packages: Vec<_> = packages
            .iter()
            .map(|(name, publish, deps)| {
                let deps: Vec<_> = deps
                    .iter()
                    .map(|(dep, kind, req, has_path)| {
                        let mut d = json!({"name": dep, "kind": kind, "req": req});
                        if *has_path {
                            d["path"] = json!(format!("/ws/{dep}"));
                        }
                        d
                    })
                    .collect();
                json!({"name": name, "publish": publish, "dependencies": deps})
            })
            .collect();
        json!({ "packages": packages })
    }

    fn deps_of(metadata: &serde_json::Value, name: &str) -> Vec<String> {
        members_from_metadata(metadata)
            .into_iter()
            .find(|m| m.name == name)
            .unwrap()
            .internal_deps
            .into_iter()
            .collect()
    }

    #[test]
    fn parses_only_calls() {
        let text = "\
          publish() {
            local crate=$1
          }
          # publish commented
          publish a
            publish b
          echo publish d
";
        let parsed = parse_publish_list(text);
        assert_eq!(parsed.calls, ["a", "b"]);
        assert!(parsed.unreadable.is_empty());
    }

    #[test]
    fn a_publish_line_that_is_not_a_plain_call_is_unreadable() {
        for line in [
            "publish c # note",
            "publish \"c\"",
            "publish",
            "publish a b",
        ] {
            let parsed = parse_publish_list(line);
            assert!(parsed.calls.is_empty(), "{line}");
            assert_eq!(parsed.unreadable, [line], "{line}");
            let found = problems(&parsed, &members());
            assert!(
                found[0].starts_with("cannot read this publish line"),
                "{found:?}"
            );
        }
    }

    #[test]
    fn a_comment_name_is_not_a_crate() {
        // `publish # note` has the word `publish` first and a comment second.
        let parsed = parse_publish_list("publish # note");
        assert!(parsed.calls.is_empty());
        assert_eq!(parsed.unreadable.len(), 1);
    }

    #[test]
    fn a_correct_list_passes() {
        assert!(problems(&list(&["a", "b", "c"]), &members()).is_empty());
    }

    #[test]
    fn no_calls_fails() {
        assert_eq!(
            problems(&list(&[]), &members()),
            ["the workflow holds no `publish <crate>` line"]
        );
    }

    #[test]
    fn a_missing_line_is_named() {
        let found = problems(&list(&["a", "c"]), &members());
        assert!(
            found.iter().any(|f| f.contains("`b` is publishable")),
            "{found:?}"
        );
    }

    #[test]
    fn an_extra_line_is_named() {
        let found = problems(&list(&["a", "b", "c", "xtask"]), &members());
        assert!(
            found.iter().any(|f| f.contains("publish xtask")),
            "{found:?}"
        );
    }

    #[test]
    fn a_duplicate_line_is_named() {
        let found = problems(&list(&["a", "b", "b", "c"]), &members());
        assert!(
            found
                .iter()
                .any(|f| f.contains("`publish b` appears more than once")),
            "{found:?}"
        );
    }

    #[test]
    fn a_dependent_before_its_dependency_is_named() {
        let found = problems(&list(&["b", "a", "c"]), &members());
        assert_eq!(found, ["`b` depends on `a` but is published before it"]);
    }

    #[test]
    fn only_the_second_of_two_dependencies_is_misordered() {
        // `c` depends on `a` then `b`; `b` comes after `c`.
        let found = problems(&list(&["a", "c", "b"]), &members());
        assert_eq!(found, ["`c` depends on `b` but is published before it"]);
    }

    #[test]
    fn a_dependency_on_an_unpublished_member_is_named() {
        let mut all = members();
        all.push(Member {
            publishable: false,
            ..member("t", &[])
        });
        all[0].internal_deps.insert("t".to_string());
        let found = problems(&list(&["a", "b", "c"]), &all);
        assert_eq!(
            found,
            ["`a` depends on `t`, which is not published to crates.io"]
        );
    }

    #[test]
    fn a_build_dependency_is_an_edge() {
        let m = metadata(&[
            ("a", json!(null), vec![]),
            ("b", json!(null), vec![("a", Some("build"), "^1", true)]),
        ]);
        assert_eq!(deps_of(&m, "b"), ["a"]);
    }

    #[test]
    fn a_versioned_dev_dependency_is_an_edge() {
        let m = metadata(&[
            ("a", json!(null), vec![]),
            ("b", json!(null), vec![("a", Some("dev"), "^1", true)]),
        ]);
        assert_eq!(deps_of(&m, "b"), ["a"]);
    }

    #[test]
    fn a_path_only_dev_dependency_is_not_an_edge() {
        let m = metadata(&[
            ("a", json!(null), vec![]),
            ("b", json!(null), vec![("a", Some("dev"), "*", true)]),
        ]);
        assert!(deps_of(&m, "b").is_empty());
    }

    #[test]
    fn a_normal_dependency_without_a_path_is_not_a_member() {
        let m = metadata(&[
            ("a", json!(null), vec![]),
            ("b", json!(null), vec![("a", None, "^1", false)]),
        ]);
        assert!(deps_of(&m, "b").is_empty());
    }

    #[test]
    fn publish_registries_decide_whether_a_member_is_publishable() {
        let m = metadata(&[
            ("none", json!(null), vec![]),
            ("off", json!([]), vec![]),
            ("other", json!(["internal"]), vec![]),
            ("both", json!(["internal", "crates-io"]), vec![]),
        ]);
        let got: Vec<_> = members_from_metadata(&m)
            .into_iter()
            .map(|m| (m.name, m.publishable))
            .collect();
        assert_eq!(
            got,
            [
                ("none".to_string(), true),
                ("off".to_string(), false),
                ("other".to_string(), false),
                ("both".to_string(), true),
            ]
        );
    }
}
