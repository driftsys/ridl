//! Pure type-reference and dependency queries over checked IR.
use crate::query::{self, Item, NameInput, find};
use crate::snapshot::{Snapshot, ToolError};
use crate::types::{Location, OverlayInput, WorkspaceStatus};
use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct ReferencesOutput {
    pub target: String,
    pub references: Vec<Reference>,
    pub workspace: WorkspaceStatus,
}
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct Reference {
    pub package: String,
    pub declaration: String,
    pub interaction: Option<String>,
    pub location: Option<Location>,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct DependenciesInput {
    pub path: String,
    pub overlays: Option<Vec<OverlayInput>>,
    pub package: Option<String>,
}
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct DependenciesOutput {
    pub packages: Vec<PackageDeps>,
    pub workspace: WorkspaceStatus,
}
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct PackageDeps {
    pub name: String,
    pub imports: Vec<String>,
    pub depends_on: Vec<String>,
    pub dependents: Vec<String>,
}

/// Walks the IR's canonical protobuf JSON reference fields recursively.
/// Interaction boundaries retain their names; units and documentation are not references.
pub fn references_of(own_package: &str, item: Item<'_>) -> Vec<(Option<String>, String)> {
    let value = match item {
        Item::Decl(d) => serde_json::to_value(d),
        Item::Interface(i) => serde_json::to_value(i),
        Item::Service(s) => serde_json::to_value(s),
    }
    .expect("IR items serialize");
    fn walk(
        own: &str,
        value: &serde_json::Value,
        interaction: Option<&str>,
        out: &mut Vec<(Option<String>, String)>,
    ) {
        match value {
            serde_json::Value::Object(object) => {
                for (key, value) in object {
                    if key == "interactions" {
                        if let Some(interactions) = value.as_array() {
                            for item in interactions {
                                walk(own, item, item["name"].as_str(), out);
                            }
                        }
                    } else if key == "fallible" {
                        if let Some(fallible) = value.as_object() {
                            for key in ["ok", "err"] {
                                if let Some(name) = fallible
                                    .get(key)
                                    .and_then(serde_json::Value::as_str)
                                    .filter(|n| !n.is_empty())
                                {
                                    let canonical = if name.contains('.') {
                                        name.to_string()
                                    } else {
                                        format!("{own}.{name}")
                                    };
                                    out.push((interaction.map(str::to_string), canonical));
                                }
                            }
                        }
                    } else if matches!(
                        key.as_str(),
                        "named"
                            | "typeRef"
                            | "backingEnum"
                            | "patternConst"
                            | "interfaceRef"
                            | "payload"
                    ) {
                        if let Some(name) = value.as_str().filter(|n| !n.is_empty()) {
                            let canonical = if name.contains('.') {
                                name.to_string()
                            } else {
                                format!("{own}.{name}")
                            };
                            out.push((interaction.map(str::to_string), canonical));
                        } else {
                            walk(own, value, interaction, out);
                        }
                    } else {
                        walk(own, value, interaction, out);
                    }
                }
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    walk(own, value, interaction, out);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(own_package, &value, None, &mut out);
    out
}
fn items(package: &ridl_ir::v2::Package) -> impl Iterator<Item = Item<'_>> {
    package
        .decls
        .iter()
        .map(Item::Decl)
        .chain(
            package
                .shapes()
                .filter(|shape| !shape.is_inline())
                .map(|shape| Item::Interface(shape.interface)),
        )
        .chain(package.services.iter().map(Item::Service))
}
fn canonical(snap: &Snapshot, own: &str, reference: String) -> String {
    let prefix = reference.rsplit_once('.').map(|(p, _)| p);
    if prefix == Some("ridl.std")
        || snap
            .output
            .checked
            .iter()
            .any(|c| Some(c.ir.name.as_str()) == prefix)
    {
        reference
    } else {
        format!("{own}.{reference}")
    }
}
pub fn references(snap: &Snapshot, input: &NameInput) -> Result<ReferencesOutput, ToolError> {
    let found = find(snap, &input.name, input.from.as_deref())?;
    let target = format!("{}.{}", found.package, found.item.name());
    let mut references = Vec::new();
    for (i, checked) in snap.output.checked.iter().enumerate() {
        for item in items(&checked.ir) {
            let pairs: BTreeSet<_> = references_of(&checked.ir.name, item)
                .into_iter()
                .filter_map(|(interaction, reference)| {
                    (canonical(snap, &checked.ir.name, reference) == target).then_some(interaction)
                })
                .collect();
            for interaction in pairs {
                let location = if matches!(item, Item::Service(_)) {
                    None
                } else {
                    snap.output.resolutions[i]
                        .symbols
                        .get(item.name())
                        .filter(|s| s.package == checked.ir.name)
                        .map(|s| snap.location(s.file, s.range))
                };
                references.push(Reference {
                    package: checked.ir.name.clone(),
                    declaration: item.name().into(),
                    interaction,
                    location,
                });
            }
        }
    }
    references.sort_by(|a, b| {
        (&a.package, &a.declaration, &a.interaction).cmp(&(
            &b.package,
            &b.declaration,
            &b.interaction,
        ))
    });
    Ok(ReferencesOutput {
        target,
        references,
        workspace: snap.status(),
    })
}
pub fn dependencies(
    snap: &Snapshot,
    input: &DependenciesInput,
) -> Result<DependenciesOutput, ToolError> {
    if let Some(package) = &input.package
        && !snap.output.checked.iter().any(|c| &c.ir.name == package)
    {
        return Err(ToolError::Request(format!(
            "no package `{package}` in this workspace; packages: {}",
            query::packages(snap)
        )));
    }
    let mut packages: Vec<_> = snap
        .output
        .checked
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let depends_on: BTreeSet<String> = items(&c.ir)
                .flat_map(|item| references_of(&c.ir.name, item))
                .filter_map(|(_, reference)| {
                    let reference = canonical(snap, &c.ir.name, reference);
                    let (prefix, _) = reference.rsplit_once('.')?;
                    (prefix != c.ir.name && prefix != "ridl.std").then(|| prefix.to_string())
                })
                .collect();
            PackageDeps {
                name: c.ir.name.clone(),
                imports: snap.output.imports[i].keys().cloned().collect(),
                depends_on: depends_on.into_iter().collect(),
                dependents: Vec::new(),
            }
        })
        .collect();
    for i in 0..packages.len() {
        let mut dependents = packages
            .iter()
            .filter(|p| p.depends_on.contains(&packages[i].name))
            .map(|p| p.name.clone())
            .collect::<Vec<_>>();
        dependents.sort();
        dependents.dedup();
        packages[i].dependents = dependents;
    }
    packages.sort_by(|a, b| a.name.cmp(&b.name));
    if let Some(package) = &input.package {
        packages.retain(|p| &p.name == package);
    }
    Ok(DependenciesOutput {
        packages,
        workspace: snap.status(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{snapshot, tests::fixture};
    use crate::types::OverlayInput;
    fn input(name: &str) -> NameInput {
        NameInput {
            path: fixture("ws"),
            overlays: None,
            name: name.into(),
            from: None,
        }
    }
    fn snap() -> Snapshot {
        snapshot(&fixture("ws"), &[]).unwrap()
    }
    fn pairs(output: &ReferencesOutput) -> Vec<(&str, &str, Option<&str>)> {
        output
            .references
            .iter()
            .map(|r| {
                (
                    r.package.as_str(),
                    r.declaration.as_str(),
                    r.interaction.as_deref(),
                )
            })
            .collect()
    }
    #[test]
    fn references_to_a_struct() {
        let out = references(&snap(), &input("Reading")).unwrap();
        assert_eq!(
            pairs(&out),
            [
                ("fx.a", "Outcome", None),
                ("fx.b", "Status", Some("reading"))
            ]
        );
    }
    #[test]
    fn references_through_an_alias() {
        let out = references(&snap(), &input("fx.a.Level")).unwrap();
        assert_eq!(
            pairs(&out),
            [
                ("fx.a", "Reading", None),
                ("fx.b", "Status", Some("setLevel"))
            ]
        );
        let out = references(&snap(), &input("fx.b.Level")).unwrap();
        assert_eq!(pairs(&out), [("fx.b", "Status", Some("outcome"))]);
    }
    #[test]
    fn a_declaration_referring_twice_is_reported_once() {
        let path = format!("{}/a/a.ridl", fixture("ws"));
        let source = std::fs::read_to_string(&path).unwrap()
            + "\nstruct Pair {\n  x: Health\n  y: Health\n}\n";
        let snap = snapshot(&fixture("ws"), &[OverlayInput { path, source }]).unwrap();
        let out = references(&snap, &input("Health")).unwrap();
        assert_eq!(
            out.references
                .iter()
                .filter(|r| r.declaration == "Pair")
                .count(),
            1
        );
    }
    #[test]
    fn references_from_services() {
        let out = references(&snap(), &input("Status")).unwrap();
        assert_eq!(pairs(&out), [("fx.b", "fx.b.status", None)]);
        assert!(out.references[0].location.is_none());
        let out = references(&snap(), &input("Speed")).unwrap();
        assert_eq!(
            pairs(&out),
            [
                ("fx.b", "Status", Some("speed")),
                ("fx.b", "fx.b.diag", Some("readSpeed"))
            ]
        );
        assert!(out.references[1].location.is_none());
    }
    #[test]
    fn interactions_referring_to_the_same_target_are_separate_pairs() {
        let path = format!("{}/b/b.ridl", fixture("ws"));
        let source = std::fs::read_to_string(&path).unwrap().replace(
            "  fixed softwareVersion: Version",
            "  query readSpeed(input: Speed): Speed @[..100ms]\n  fixed softwareVersion: Version",
        );
        let snap = snapshot(&fixture("ws"), &[OverlayInput { path, source }]).unwrap();
        let out = references(&snap, &input("Speed")).unwrap();
        assert_eq!(
            pairs(&out),
            [
                ("fx.b", "Status", Some("readSpeed")),
                ("fx.b", "Status", Some("speed")),
                ("fx.b", "fx.b.diag", Some("readSpeed"))
            ]
        );
    }
    #[test]
    fn fallible_returns_report_both_named_types() {
        let interface: ridl_ir::v2::Interface = serde_json::from_value(serde_json::json!({
            "name": "Status",
            "interactions": [{"name":"read", "queryDef": {"returnType":{"fallible":{"ok":"fx.a.Speed", "err":"Failure"}}}}]
        })).unwrap();
        let mut found = references_of("fx.b", Item::Interface(&interface));
        found.sort();
        assert_eq!(
            found,
            [
                (Some("read".into()), "fx.a.Speed".into()),
                (Some("read".into()), "fx.b.Failure".into())
            ]
        );
    }

    #[test]
    fn dependencies_of_the_fixture() {
        let out = dependencies(
            &snap(),
            &DependenciesInput {
                path: fixture("ws"),
                overlays: None,
                package: None,
            },
        )
        .unwrap();
        let a = out.packages.iter().find(|p| p.name == "fx.a").unwrap();
        assert_eq!(a.dependents, ["fx.b"]);
        let b = out.packages.iter().find(|p| p.name == "fx.b").unwrap();
        assert_eq!(b.depends_on, ["fx.a"]);
        let sub = out.packages.iter().find(|p| p.name == "fx.a.sub").unwrap();
        assert!(sub.depends_on.is_empty());
        assert!(sub.dependents.is_empty());
        assert!(
            out.packages
                .iter()
                .all(|p| !p.depends_on.iter().any(|n| n == "ridl.std"))
        );
    }
    #[test]
    fn dependencies_of_an_unknown_package_is_an_error() {
        let error = dependencies(
            &snap(),
            &DependenciesInput {
                path: fixture("ws"),
                overlays: None,
                package: Some("missing".into()),
            },
        )
        .err()
        .unwrap();
        let ToolError::Request(message) = error else {
            panic!("request error")
        };
        assert_eq!(
            message,
            "no package `missing` in this workspace; packages: fx.a, fx.a.sub, fx.b"
        );
    }
}
