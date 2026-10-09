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
//!    through a normal or build dependency.
//!
//! Dev-dependencies are left out of the second property. `cargo publish`
//! removes a dev-dependency that names a path and no version from the
//! packaged manifest, and this repository writes its internal
//! dev-dependencies that way (`ridlc = { path = "../ridlc" }`), so a
//! dev-dependency never has to be on crates.io first. Several of them also
//! form cycles with a normal dependency, which no publish order could satisfy.
//!
//! The checking logic takes plain data, so the unit tests at the end feed it
//! small synthetic workflow texts and member lists.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::process::Command;

const WORKFLOW: &str = ".github/workflows/crates-io-release.yml";

/// A workspace member that can be published, with its internal dependencies
/// that must be on the registry first.
struct Member {
    name: String,
    internal_deps: BTreeSet<String>,
}

/// The crate names of the `publish <crate>` calls, in file order.
///
/// A call is a line that holds exactly the word `publish` and one crate name.
/// The function definition (`publish() {`) and comments do not match.
fn publish_calls(workflow: &str) -> Vec<String> {
    workflow
        .lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            match (words.next(), words.next(), words.next()) {
                (Some("publish"), Some(name), None) if !name.starts_with('#') => {
                    Some(name.to_string())
                }
                _ => None,
            }
        })
        .collect()
}

/// Every problem found between the publish list and the publishable members.
/// An empty result means the list is complete and in dependency order.
fn problems(calls: &[String], members: &[Member]) -> Vec<String> {
    let mut found = Vec::new();
    if calls.is_empty() {
        found.push("the workflow holds no `publish <crate>` line".to_string());
        return found;
    }

    let listed: BTreeSet<&str> = calls.iter().map(String::as_str).collect();
    let expected: BTreeSet<&str> = members.iter().map(|m| m.name.as_str()).collect();
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

    let position: HashMap<&str, usize> = calls
        .iter()
        .enumerate()
        .rev()
        .map(|(index, name)| (name.as_str(), index))
        .collect();
    for member in members {
        let Some(&own) = position.get(member.name.as_str()) else {
            continue;
        };
        for dependency in &member.internal_deps {
            // A dependency with no `publish` line is reported above.
            if let Some(&before) = position.get(dependency.as_str())
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

/// The publishable members of the workspace, read from `cargo metadata`.
fn publishable_members() -> Vec<Member> {
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
    let packages = metadata["packages"]
        .as_array()
        .expect("cargo metadata carries a `packages` array");

    let names: BTreeSet<&str> = packages
        .iter()
        .map(|p| p["name"].as_str().expect("a package has a name"))
        .collect();
    packages
        .iter()
        .filter(|p| {
            // `publish = false` is reported as an empty list; no `publish`
            // key is reported as null.
            p["publish"]
                .as_array()
                .map(|registries| !registries.is_empty())
                .unwrap_or(true)
        })
        .map(|p| Member {
            name: p["name"].as_str().unwrap().to_string(),
            internal_deps: p["dependencies"]
                .as_array()
                .expect("a package carries `dependencies`")
                .iter()
                // `kind` is null for a normal dependency, "build", or "dev".
                .filter(|d| d["kind"].as_str() != Some("dev"))
                .filter_map(|d| d["name"].as_str())
                .filter(|name| names.contains(name))
                .map(str::to_string)
                .collect(),
        })
        .collect()
}

#[test]
fn the_release_publish_list_is_complete_and_in_dependency_order() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(WORKFLOW);
    let workflow = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    let found = problems(&publish_calls(&workflow), &publishable_members());
    assert!(
        found.is_empty(),
        "{WORKFLOW} publish list is wrong:\n  {}",
        found.join("\n  ")
    );
}

#[cfg(test)]
mod unit {
    use super::*;

    fn member(name: &str, deps: &[&str]) -> Member {
        Member {
            name: name.to_string(),
            internal_deps: deps.iter().map(|d| d.to_string()).collect(),
        }
    }

    fn calls(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    fn members() -> Vec<Member> {
        vec![
            member("a", &[]),
            member("b", &["a"]),
            member("c", &["a", "b"]),
        ]
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
          publish c # trailing
          echo publish d
";
        assert_eq!(publish_calls(text), calls(&["a", "b"]));
    }

    #[test]
    fn a_correct_list_passes() {
        assert!(problems(&calls(&["a", "b", "c"]), &members()).is_empty());
    }

    #[test]
    fn no_calls_fails() {
        let found = problems(&[], &members());
        assert!(found[0].contains("no `publish"), "{found:?}");
    }

    #[test]
    fn a_missing_line_is_named() {
        let found = problems(&calls(&["a", "c"]), &members());
        assert!(
            found.iter().any(|f| f.contains("`b` is publishable")),
            "{found:?}"
        );
    }

    #[test]
    fn an_extra_line_is_named() {
        let found = problems(&calls(&["a", "b", "c", "xtask"]), &members());
        assert!(
            found.iter().any(|f| f.contains("publish xtask")),
            "{found:?}"
        );
    }

    #[test]
    fn a_duplicate_line_is_named() {
        let found = problems(&calls(&["a", "b", "b", "c"]), &members());
        assert!(
            found
                .iter()
                .any(|f| f.contains("`publish b` appears more than once")),
            "{found:?}"
        );
    }

    #[test]
    fn a_dependent_before_its_dependency_is_named() {
        let found = problems(&calls(&["b", "a", "c"]), &members());
        assert!(
            found
                .iter()
                .any(|f| f.contains("`b` depends on `a` but is published before it")),
            "{found:?}"
        );
        assert_eq!(found.len(), 1, "{found:?}");
    }
}
