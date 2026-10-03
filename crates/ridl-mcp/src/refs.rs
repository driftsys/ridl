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
#[derive(Serialize, JsonSchema, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[schemars(crate = "rmcp::schemars")]
#[serde(rename_all = "snake_case")]
pub enum ReferenceKind {
    Declaration,
    Interface,
    Service,
    Component,
}

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct Reference {
    pub package: String,
    pub declaration: String,
    pub kind: ReferenceKind,
    pub interaction: Option<String>,
    pub location: Option<Location>,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct DependenciesInput {
    /// The workspace root, a package directory or a source file, relative to the server's working directory unless absolute.
    pub path: String,
    /// Optional unsaved source files to apply without writing them to disk.
    pub overlays: Option<Vec<OverlayInput>>,
    /// An optional package name that filters the result to that workspace package.
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
/// Every (component package, component name, required interface) triple of
/// the lowered system, per spec §4.4: declared components only, required
/// interfaces that are present and not inline, as `{catalog}.{name}`.
fn component_requires(system: &ridl_ir::v2::System) -> Vec<(String, String, String)> {
    system
        .components
        .iter()
        .filter(|c| !c.package.is_empty())
        .flat_map(|component| {
            component.requires.iter().filter_map(|require| {
                let interface = require.interface.as_ref()?;
                (!interface.inline).then(|| {
                    (
                        component.package.clone(),
                        component.name.clone(),
                        format!("{}.{}", interface.catalog, interface.name),
                    )
                })
            })
        })
        .collect()
}

/// (system package, member component package) pairs, per spec §4.4.
fn system_member_packages(system: &ridl_ir::v2::System) -> Vec<(String, String)> {
    system
        .members
        .iter()
        .filter_map(|member| {
            let component = system.components.iter().find(|c| {
                !c.package.is_empty() && format!("{}.{}", c.package, c.name) == member.component
            })?;
            Some((system.package.clone(), component.package.clone()))
        })
        .collect()
}

pub fn references(snap: &Snapshot, input: &NameInput) -> Result<ReferencesOutput, ToolError> {
    let found = find(snap, &input.name, input.from.as_deref())?;
    let target = format!("{}.{}", found.package, found.item.name());
    let mut references = Vec::new();
    for (i, checked) in snap.output.checked.iter().enumerate() {
        for item in items(&checked.ir) {
            let pairs: BTreeSet<_> = references_of(&checked.ir.name, item)
                .into_iter()
                .filter_map(|(interaction, reference)| (reference == target).then_some(interaction))
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
                    kind: match item {
                        Item::Decl(_) => ReferenceKind::Declaration,
                        Item::Interface(_) => ReferenceKind::Interface,
                        Item::Service(_) => ReferenceKind::Service,
                    },
                    interaction,
                    location,
                });
            }
        }
    }
    if let Some(system) = &snap.output.system {
        let components: BTreeSet<_> = component_requires(system)
            .into_iter()
            .filter_map(|(package, declaration, reference)| {
                (reference == target).then_some((package, declaration))
            })
            .collect();
        for (package, declaration) in components {
            references.push(Reference {
                package,
                declaration,
                kind: ReferenceKind::Component,
                interaction: None,
                location: None,
            });
        }
    }
    references.sort_by(|a, b| {
        (&a.package, &a.declaration, &a.interaction, &a.kind).cmp(&(
            &b.package,
            &b.declaration,
            &b.interaction,
            &b.kind,
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
    if let Some(system) = &snap.output.system {
        let edges = component_requires(system)
            .into_iter()
            .filter_map(|(package, _, reference)| {
                let (catalog, _) = reference.rsplit_once('.')?;
                Some((package, catalog.to_string()))
            })
            .chain(system_member_packages(system));
        for (own, target) in edges {
            if target != own
                && target != "ridl.std"
                && let Some(package) = packages.iter_mut().find(|p| p.name == own)
            {
                package.depends_on.push(target);
            }
        }
        for package in &mut packages {
            package.depends_on.sort();
            package.depends_on.dedup();
        }
    }
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
        assert_eq!(out.references[1].kind, ReferenceKind::Service);
    }
    #[test]
    fn interactions_referring_to_the_same_target_are_separate_pairs() {
        let path = format!("{}/b/b.ridl", fixture("ws"));
        let source = std::fs::read_to_string(&path).unwrap().replace(
            "  fixed softwareVersion: Version",
            "  query readSpeed(sample: Speed): Speed @[..100ms]\n  fixed softwareVersion: Version",
        );
        let snap = snapshot(&fixture("ws"), &[OverlayInput { path, source }]).unwrap();
        assert_eq!(snap.status().errors, 0);
        assert_eq!(snap.status().warnings, 0);
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
    fn dependencies_report_manifest_imports_and_preserve_the_filtered_graph() {
        let copy = crate::snapshot::tests::TempWorkspace::copy("ws");
        for (member, imports) in [
            ("a", "alpha = \"https://192.0.2.1/alpha.git\""),
            (
                "b",
                "zeta = \"https://192.0.2.1/zeta.git\"\nbeta = \"https://192.0.2.1/beta.git\"",
            ),
        ] {
            let manifest = copy.0.join(member).join("ridl.toml");
            let source = std::fs::read_to_string(&manifest).unwrap();
            std::fs::write(manifest, format!("{source}\n[imports]\n{imports}\n")).unwrap();
        }
        let path = copy.0.to_str().unwrap();
        let snap = snapshot(path, &[]).unwrap();
        let out = dependencies(
            &snap,
            &DependenciesInput {
                path: path.into(),
                overlays: None,
                package: None,
            },
        )
        .unwrap();
        assert_eq!(
            out.packages
                .iter()
                .map(|p| (
                    &*p.name,
                    p.imports.iter().map(String::as_str).collect::<Vec<_>>()
                ))
                .collect::<Vec<_>>(),
            [
                ("fx.a", vec!["alpha"]),
                ("fx.a.sub", vec!["alpha"]),
                ("fx.b", vec!["beta", "zeta"])
            ]
        );
        let filtered = dependencies(
            &snap,
            &DependenciesInput {
                path: path.into(),
                overlays: None,
                package: Some("fx.a".into()),
            },
        )
        .unwrap();
        assert_eq!(filtered.packages.len(), 1);
        assert_eq!(filtered.packages[0].name, "fx.a");
        assert_eq!(filtered.packages[0].dependents, ["fx.b"]);
        assert_eq!(filtered.packages[0].imports, ["alpha"]);
    }
    #[test]
    fn dependencies_preserve_unresolved_qualified_references() {
        let path = format!("{}/a/a.ridl", fixture("ws"));
        let source = std::fs::read_to_string(&path).unwrap()
            + "\nstruct MissingRefs {\n  external: ext.Thing\n  nested: foreign.deep.Thing\n  local: Missing\n}\n";
        let snap = snapshot(&fixture("ws"), &[OverlayInput { path, source }]).unwrap();
        assert!(snap.status().errors > 0);
        assert!(
            snap.output
                .diagnostics
                .iter()
                .any(|d| d.code.as_str() == "TYPL-011")
        );
        let out = dependencies(
            &snap,
            &DependenciesInput {
                path: fixture("ws"),
                overlays: None,
                package: Some("fx.a".into()),
            },
        )
        .unwrap();
        assert_eq!(out.packages[0].depends_on, ["ext", "foreign.deep"]);
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
    fn rsdl_snap() -> Snapshot {
        let snap = snapshot(&fixture("ws-rsdl"), &[]).unwrap();
        assert_eq!(snap.status().errors, 0);
        assert_eq!(snap.status().warnings, 2);
        assert!(snap.output.system.is_some());
        assert!(snap.notes.is_empty());
        snap
    }
    fn deps(snap: &Snapshot, package: Option<&str>) -> DependenciesOutput {
        dependencies(
            snap,
            &DependenciesInput {
                path: snap.root.to_string_lossy().into_owned(),
                overlays: None,
                package: package.map(str::to_string),
            },
        )
        .unwrap()
    }
    #[test]
    fn references_from_components() {
        let out = references(&rsdl_snap(), &input("veh.climate.Seats")).unwrap();
        let rows = serde_json::to_value(&out.references).unwrap();
        assert_eq!(
            rows.as_array()
                .unwrap()
                .iter()
                .map(|r| (
                    r["package"].as_str().unwrap(),
                    r["declaration"].as_str().unwrap(),
                    r["kind"].as_str(),
                    r["interaction"].as_str(),
                ))
                .collect::<Vec<_>>(),
            [
                ("veh.cabin", "ClimateControl", Some("component"), None),
                ("veh.cabin", "Dashboard", Some("component"), None),
                ("veh.climate", "veh.climate.seats", Some("service"), None),
            ]
        );
        assert!(out.references.iter().all(|r| r.location.is_none()));
    }
    #[test]
    fn an_inline_require_is_not_a_reference() {
        let snap = rsdl_snap();
        let mut requires = component_requires(snap.output.system.as_ref().unwrap());
        requires.sort();
        assert_eq!(
            requires,
            [
                (
                    "veh.cabin".into(),
                    "ClimateControl".into(),
                    "veh.climate.Seats".into()
                ),
                (
                    "veh.cabin".into(),
                    "Dashboard".into(),
                    "veh.climate.Climate".into()
                ),
                (
                    "veh.cabin".into(),
                    "Dashboard".into(),
                    "veh.climate.Seats".into()
                ),
                (
                    "veh.cabin".into(),
                    "PhoneApp".into(),
                    "veh.climate.Climate".into()
                ),
            ]
        );
        let out = references(&snap, &input("veh.climate.Temperature")).unwrap();
        assert!(out.references.iter().all(|r| r.declaration != "PhoneApp"));
        assert!(
            out.references
                .iter()
                .any(|r| r.declaration == "veh.climate.diag")
        );
    }
    #[test]
    fn one_package_keeps_its_workspace_dependents() {
        let out = deps(&rsdl_snap(), Some("veh.climate"));
        assert_eq!(out.packages.len(), 1);
        assert_eq!(out.packages[0].name, "veh.climate");
        assert_eq!(out.packages[0].dependents, ["veh.cabin"]);
    }
    #[test]
    fn dependencies_count_component_requires() {
        let out = deps(&rsdl_snap(), None);
        assert_eq!(
            out.packages
                .iter()
                .find(|p| p.name == "veh.cabin")
                .unwrap()
                .depends_on,
            ["veh.climate"]
        );
        assert_eq!(
            out.packages
                .iter()
                .find(|p| p.name == "veh.climate")
                .unwrap()
                .dependents,
            ["veh.cabin"]
        );
    }
    #[test]
    fn system_package_depends_on_member_component_packages() {
        use std::fs;
        let copy = crate::snapshot::tests::TempWorkspace::copy("ws-rsdl");
        fs::write(
            copy.0.join("ridl.toml"),
            "[workspace]\nmembers = [\"climate\", \"cabin\", \"ops\"]\n",
        )
        .unwrap();
        let cabin = copy.0.join("cabin/cabin.rsdl");
        let text = fs::read_to_string(&cabin).unwrap();
        fs::write(cabin, text.split("system Cabin").next().unwrap()).unwrap();
        fs::create_dir(copy.0.join("ops")).unwrap();
        fs::write(
            copy.0.join("ops/ridl.toml"),
            "[package]\nname = \"veh.ops\"\nversion = \"1.0.0\"\n",
        )
        .unwrap();
        fs::write(copy.0.join("ops/ops.rsdl"), "package veh.ops\nimport veh.climate.Climate\nimport veh.cabin.Dashboard\nimport veh.cabin.ClimateControl\nimport veh.cabin.SeatHeating\n\ncomponent Monitor {\n  requires Climate\n}\n\nsystem Ops {\n  Monitor\n  Dashboard\n  ClimateControl\n  SeatHeating\n}\n").unwrap();
        let snap = snapshot(copy.0.to_str().unwrap(), &[]).unwrap();
        assert_eq!(snap.status().errors, 0);
        assert_eq!(snap.status().warnings, 2);
        let references = references(&snap, &input("veh.climate.Seats")).unwrap();
        assert_eq!(
            pairs(&references),
            [
                ("veh.cabin", "ClimateControl", None),
                ("veh.cabin", "Dashboard", None),
                ("veh.climate", "veh.climate.seats", None),
            ]
        );
        assert!(references.workspace.notes.is_empty());
        let out = deps(&snap, Some("veh.ops"));
        assert!(out.workspace.notes.is_empty());
        assert_eq!(out.packages[0].depends_on, ["veh.cabin", "veh.climate"]);
    }
    #[test]
    fn no_lowered_system_draws_the_rsdl_note() {
        let path = format!("{}/cabin/cabin.rsdl", fixture("ws-rsdl"));
        let disk = std::fs::read_to_string(&path).unwrap();
        let snap = snapshot(
            &fixture("ws-rsdl"),
            &[OverlayInput {
                path,
                source: disk.split("system Cabin").next().unwrap().into(),
            }],
        )
        .unwrap();
        assert!(snap.output.system.is_none());
        let out = references(&snap, &input("veh.climate.Seats")).unwrap();
        assert!(out.workspace.notes.iter().any(|n| n == "rsdl uses were not counted, because no system was lowered: the workspace declares no `system`, or an error in its closure blocked the lowering; run ridl_check on the same path to see which"));
        assert_eq!(deps(&snap, None).workspace.notes, out.workspace.notes);
        let rows = serde_json::to_value(&out.references).unwrap();
        assert!(
            rows.as_array()
                .unwrap()
                .iter()
                .all(|r| r["kind"] != "component")
        );
    }
    #[test]
    fn existing_references_carry_their_kind() {
        let out = references(&snap(), &input("Reading")).unwrap();
        let rows = serde_json::to_value(&out.references).unwrap();
        assert_eq!(rows[0]["package"], "fx.a");
        assert_eq!(rows[0]["declaration"], "Outcome");
        assert_eq!(rows[0]["kind"], "declaration");
        assert_eq!(rows[1]["package"], "fx.b");
        assert_eq!(rows[1]["declaration"], "Status");
        assert_eq!(rows[1]["kind"], "interface");
    }

    #[test]
    fn declaration_references_have_locations() {
        let out = references(&snap(), &input("Reading")).unwrap();
        assert_eq!(
            serde_json::to_value(&out.references[0].location).unwrap(),
            crate::snapshot::tests::name_location("a/a.ridl", "union Outcome", "Outcome")
        );
        assert_eq!(
            serde_json::to_value(&out.references[1].location).unwrap(),
            crate::snapshot::tests::name_location("b/b.ridl", "interface Status", "Status")
        );
    }
    #[test]
    fn references_through_backing_enums_and_pattern_consts() {
        let out = references(&snap(), &input("Health")).unwrap();
        assert!(pairs(&out).contains(&("fx.a", "HealthSet", None)));
        let out = references(&snap(), &input("HEALTH_PATTERN")).unwrap();
        assert_eq!(pairs(&out), [("fx.a", "HealthCode", None)]);
    }
}
