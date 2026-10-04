//! Package dependency edges over checked IR and the lowered system.
use ridl_ir::v2::System;
use ridl_sem::CheckedPackage;
use std::collections::{BTreeMap, BTreeSet};

/// Dependencies of each checked workspace package, including external qualifiers.
/// The package itself and `ridl.std` are excluded. Component requires and system
/// member uses follow ADR-0025 decision 8.
pub fn package_edges(
    checked: &[CheckedPackage],
    system: Option<&System>,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut edges: BTreeMap<_, _> = checked
        .iter()
        .map(|checked| {
            let package = &checked.ir;
            let references = package
                .decls
                .iter()
                .flat_map(|decl| references_of(&package.name, decl))
                .chain(
                    package
                        .shapes()
                        .filter(|shape| !shape.is_inline())
                        .flat_map(|shape| references_of(&package.name, shape.interface)),
                )
                .chain(
                    package
                        .services
                        .iter()
                        .flat_map(|service| references_of(&package.name, service)),
                );
            let targets: BTreeSet<String> = references
                .filter_map(|(_, reference)| {
                    let (prefix, _) = reference.rsplit_once('.')?;
                    (prefix != package.name && prefix != "ridl.std").then(|| prefix.to_string())
                })
                .collect();
            (package.name.clone(), targets)
        })
        .collect();
    if let Some(system) = system {
        let system_edges = component_requires(system)
            .into_iter()
            .filter_map(|(package, _, reference)| {
                let (catalog, _) = reference.rsplit_once('.')?;
                Some((package, catalog.to_string()))
            })
            .chain(system_member_packages(system));
        for (own, target) in system_edges {
            if target != own
                && target != "ridl.std"
                && let Some(targets) = edges.get_mut(&own)
            {
                targets.insert(target);
            }
        }
    }
    edges
}

/// Retains only targets that are workspace packages in an already computed graph.
/// Packages with no workspace dependencies remain as keys with empty target sets.
pub fn workspace_package_edges(
    edges: &BTreeMap<String, BTreeSet<String>>,
) -> BTreeMap<String, BTreeSet<String>> {
    edges
        .iter()
        .map(|(package, targets)| {
            (
                package.clone(),
                targets
                    .iter()
                    .filter(|target| edges.contains_key(*target))
                    .cloned()
                    .collect(),
            )
        })
        .collect()
}

/// Walks the IR's canonical protobuf JSON reference fields recursively.
/// Interaction boundaries retain their names; units and documentation are not references.
pub fn references_of<T: serde::Serialize + ?Sized>(
    own_package: &str,
    item: &T,
) -> Vec<(Option<String>, String)> {
    let value = serde_json::to_value(item).expect("IR items serialize");
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
/// Every (component package, component name, required interface) triple of
/// the lowered system, per spec §4.4: declared components only, required
/// interfaces that are present and not inline, as `{catalog}.{name}`.
pub fn component_requires(system: &ridl_ir::v2::System) -> Vec<(String, String, String)> {
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

#[cfg(test)]
mod tests {
    use ridl_core::RidlDatabase;
    use std::collections::{BTreeMap, BTreeSet};

    fn workspace(external_references: bool) -> crate::WorkspaceOutput {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("ridl.toml"),
            "[workspace]\nmembers = [\"a\", \"b\", \"c\"]\n",
        )
        .unwrap();
        for name in ["a", "b", "c"] {
            std::fs::create_dir(dir.path().join(name)).unwrap();
            std::fs::write(
                dir.path().join(name).join("ridl.toml"),
                format!("[package]\nname = \"{name}\"\nversion = \"1.0.0\"\n"),
            )
            .unwrap();
        }
        std::fs::write(
            dir.path().join("a/a.ridl"),
            "package a\ntype Reading: integer [0..10]\ninterface Status {\n  signal reading: Reading @[100ms..1s]\n}\nservice a.status: Status\n",
        )
        .unwrap();
        let mut source = "package b\nimport a.Reading\nstruct Sample {\n  reading: Reading\n  id: ridl.std.Uuid\n}\nstruct Pair {\n  first: Sample\n  second: Sample\n}\n".to_string();
        if external_references {
            source.push_str("struct MissingRefs {\n  external: ext.Thing\n  nested: foreign.deep.Thing\n  local: Missing\n}\n");
        }
        std::fs::write(dir.path().join("b/b.typl"), source).unwrap();
        std::fs::write(
            dir.path().join("c/c.rsdl"),
            "package c\nimport a.Status\ncomponent Consumer [ external ] {\n  requires Status\n}\nsystem Example {\n  Consumer\n  a.status\n}\n",
        )
        .unwrap();
        let output = crate::compile_workspace(&mut RidlDatabase::default(), dir.path()).unwrap();
        if !external_references {
            assert!(
                !output
                    .diagnostics
                    .iter()
                    .any(|d| d.severity == ridl_core::diag::Severity::Error),
                "unexpected diagnostics: {:?}",
                output.diagnostics
            );
        }
        assert!(output.system.is_some());
        output
    }

    fn edges(rows: &[(&str, &[&str])]) -> BTreeMap<String, BTreeSet<String>> {
        rows.iter()
            .map(|(name, targets)| {
                (
                    name.to_string(),
                    targets.iter().map(|target| target.to_string()).collect(),
                )
            })
            .collect()
    }

    #[test]
    fn package_edges_matches_imports_and_component_uses() {
        let output = workspace(false);
        assert_eq!(
            super::package_edges(&output.checked, output.system.as_ref()),
            edges(&[("a", &[]), ("b", &["a"]), ("c", &["a"])])
        );
    }

    #[test]
    fn workspace_package_edges_filters_external_qualifiers() {
        let output = workspace(true);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|d| d.code.as_str() == "TYPL-011")
        );
        let complete = super::package_edges(&output.checked, output.system.as_ref());
        assert_eq!(
            complete,
            edges(&[
                ("a", &[]),
                ("b", &["a", "ext", "foreign.deep"]),
                ("c", &["a"])
            ])
        );
        assert_eq!(
            super::workspace_package_edges(&complete),
            edges(&[("a", &[]), ("b", &["a"]), ("c", &["a"])])
        );
        assert_eq!(
            complete["b"],
            edges(&[("b", &["a", "ext", "foreign.deep"])])["b"]
        );
    }
}
