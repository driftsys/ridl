//! Package coupling and interface cohesion metrics over checked workspace IR.
use crate::snapshot::{Snapshot, ToolError};
use crate::types::WorkspaceStatus;
use ridlc::cohesion_groups;
use ridlc::deps::{package_edges, workspace_package_edges};
use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct MetricsInput {
    /// The workspace root, a package directory or a source file, relative to the server's working directory unless absolute.
    pub path: String,
}

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct MetricsOutput {
    pub packages: Vec<PackageMetrics>,
    pub interfaces: Vec<InterfaceMetrics>,
    pub workspace: WorkspaceStatus,
}

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(rename_all = "camelCase")]
pub struct PackageMetrics {
    pub name: String,
    pub fan_in: usize,
    pub fan_out: usize,
    pub instability: Option<f64>,
    pub depends_on: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct InterfaceMetrics {
    pub name: String,
    pub members: usize,
    pub groups: Vec<Vec<String>>,
}

/// Reports every workspace package and declared interface without thresholds.
pub fn metrics(snap: &Snapshot, _input: &MetricsInput) -> Result<MetricsOutput, ToolError> {
    let edges = workspace_package_edges(&package_edges(
        &snap.output.checked,
        snap.output.system.as_ref(),
    ));
    let mut incoming: BTreeMap<&str, usize> = BTreeMap::new();
    for targets in edges.values() {
        for target in targets {
            *incoming.entry(target).or_default() += 1;
        }
    }
    let packages = edges
        .iter()
        .map(|(name, targets)| {
            let fan_in = incoming.get(name.as_str()).copied().unwrap_or_default();
            let fan_out = targets.len();
            PackageMetrics {
                name: name.clone(),
                fan_in,
                fan_out,
                instability: (fan_in + fan_out != 0)
                    .then(|| fan_out as f64 / (fan_in + fan_out) as f64),
                depends_on: targets.iter().cloned().collect(),
            }
        })
        .collect();
    let mut interfaces = Vec::new();
    for checked in &snap.output.checked {
        let pkg = &checked.ir;
        if pkg.name == "ridl.std" {
            continue;
        }
        for iface in &pkg.interfaces {
            interfaces.push(InterfaceMetrics {
                name: format!("{}.{}", pkg.name, iface.name),
                members: iface.interactions.len(),
                groups: cohesion_groups(pkg, iface),
            });
        }
    }
    interfaces.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(MetricsOutput {
        packages,
        interfaces,
        workspace: snap.status(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::refs::{DependenciesInput, dependencies};
    use crate::snapshot::{
        snapshot,
        tests::{TempWorkspace, fixture},
    };
    use crate::types::OverlayInput;
    use serde_json::json;
    use std::{
        collections::BTreeMap,
        fs,
        path::{Path, PathBuf},
        time::SystemTime,
    };

    fn value(snap: &Snapshot, path: &str) -> serde_json::Value {
        serde_json::to_value(metrics(snap, &MetricsInput { path: path.into() }).unwrap()).unwrap()
    }

    #[test]
    fn metrics_reports_fan_in_fan_out_and_instability() {
        let path = fixture("ws");
        let snap = snapshot(&path, &[]).unwrap();
        let deps = dependencies(
            &snap,
            &DependenciesInput {
                path: path.clone(),
                overlays: None,
                package: None,
            },
        )
        .unwrap();
        let result = value(&snap, &path);
        assert_eq!(
            result["packages"],
            json!([
                {"name":"fx.a", "fanIn":1, "fanOut":0, "instability":0.0, "dependsOn":[]},
                {"name":"fx.a.sub", "fanIn":0, "fanOut":0, "instability":null, "dependsOn":[]},
                {"name":"fx.b", "fanIn":0, "fanOut":1, "instability":1.0, "dependsOn":["fx.a"]}
            ])
        );
        for (metric, dep) in result["packages"]
            .as_array()
            .unwrap()
            .iter()
            .zip(&deps.packages)
        {
            let targets: Vec<_> = dep
                .depends_on
                .iter()
                .filter(|target| deps.packages.iter().any(|p| &p.name == *target))
                .collect();
            assert_eq!(metric["dependsOn"], json!(targets));
            assert_eq!(metric["fanOut"], targets.len());
            assert_eq!(metric["fanIn"], dep.dependents.len());
        }
        assert_eq!(
            result["workspace"],
            serde_json::to_value(snap.status()).unwrap()
        );
    }

    #[test]
    fn metrics_excludes_external_dependency_targets() {
        let path = fixture("ws");
        let file = format!("{path}/a/a.ridl");
        let source = fs::read_to_string(&file).unwrap()
            + "\nstruct MissingRefs {\n  external: ext.Thing\n  nested: foreign.deep.Thing\n}\n";
        let snap = snapshot(&path, &[OverlayInput { path: file, source }]).unwrap();
        let deps = dependencies(
            &snap,
            &DependenciesInput {
                path: path.clone(),
                overlays: None,
                package: None,
            },
        )
        .unwrap();
        assert_eq!(deps.packages[0].depends_on, ["ext", "foreign.deep"]);
        assert_eq!(
            value(&snap, &path)["packages"],
            json!([
                {"name":"fx.a", "fanIn":1, "fanOut":0, "instability":0.0, "dependsOn":[]},
                {"name":"fx.a.sub", "fanIn":0, "fanOut":0, "instability":null, "dependsOn":[]},
                {"name":"fx.b", "fanIn":0, "fanOut":1, "instability":1.0, "dependsOn":["fx.a"]}
            ])
        );
    }

    #[test]
    fn metrics_reports_cohesion_groups() {
        let copy = TempWorkspace::copy("ws");
        fs::write(copy.0.join("a/a.ridl"), "package fx.a\ntype X: integer [0..10]\ntype Z: integer [0..20]\ninterface Split {\n command z(x: X) @[..1s]\n command y(x: X) @[..1s]\n command a(z: Z) @[..1s]\n command reset() @[..1s]\n}\ninterface Empty {}\n").unwrap();
        let path = copy.0.join("a").to_str().unwrap().to_string();
        let snap = snapshot(&path, &[]).unwrap();
        assert_eq!(snap.status().errors, 0);
        assert_eq!(
            value(&snap, &path)["interfaces"],
            json!([
                {"name":"fx.a.Empty", "members":0, "groups":[]},
                {"name":"fx.a.Split", "members":4, "groups":[["y","z"],["a"]]}
            ])
        );
    }

    #[test]
    fn metrics_reports_instability_with_incoming_and_outgoing_edges() {
        let path = fixture("ws");
        let snap = snapshot(&path, &[OverlayInput {
            path: format!("{path}/a/sub/sub.ridl"),
            source: "package fx.a.sub\nimport fx.a.Reading\nimport fx.b.Level as Window\nstruct Sample {\n reading: Reading\n window: Window\n}\n".into(),
        }]).unwrap();
        assert_eq!(snap.status().errors, 0);
        assert_eq!(
            value(&snap, &path)["packages"],
            json!([
                {"name":"fx.a", "fanIn":2, "fanOut":0, "instability":0.0, "dependsOn":[]},
                {"name":"fx.a.sub", "fanIn":0, "fanOut":2, "instability":1.0, "dependsOn":["fx.a", "fx.b"]},
                {"name":"fx.b", "fanIn":1, "fanOut":1, "instability":0.5, "dependsOn":["fx.a"]}
            ])
        );
    }

    #[test]
    fn metrics_reports_a_package_with_no_applicable_findings() {
        let copy = TempWorkspace::copy("ws");
        fs::remove_dir_all(copy.0.join("a/sub")).unwrap();
        let file = copy.0.join("a/a.ridl");
        fs::write(&file, "package fx.a\ntype Count: integer [0..10]\n").unwrap();
        let path = file.to_str().unwrap();
        let snap = snapshot(path, &[]).unwrap();
        assert!(snap.output.diagnostics.is_empty());
        assert_eq!(
            value(&snap, path)["packages"],
            json!([
                {"name":"fx.a", "fanIn":0, "fanOut":0, "instability":null, "dependsOn":[]}
            ])
        );
        assert_eq!(value(&snap, path)["interfaces"], json!([]));
    }

    #[tokio::test]
    async fn metrics_reports_path_errors_as_tool_errors() {
        let copy = TempWorkspace::copy("ws");
        let path = copy.0.join("missing").to_str().unwrap().to_string();
        let result = crate::RidlMcp::new()
            .ridl_metrics(rmcp::handler::server::wrapper::Parameters(MetricsInput {
                path: path.clone(),
            }))
            .await
            .unwrap();
        assert_eq!(result.is_error, Some(true));
        assert_eq!(
            result.content[0].as_text().unwrap().text,
            format!("`{path}` does not exist")
        );
    }

    fn tree(root: &Path) -> BTreeMap<PathBuf, (Vec<u8>, u64, SystemTime)> {
        let mut files = BTreeMap::new();
        for entry in fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                files.extend(tree(&path));
            } else {
                let meta = fs::metadata(&path).unwrap();
                files.insert(
                    path.clone(),
                    (
                        fs::read(path).unwrap(),
                        meta.len(),
                        meta.modified().unwrap(),
                    ),
                );
            }
        }
        files
    }

    #[tokio::test]
    async fn metrics_writes_nothing() {
        let copy = TempWorkspace::copy("ws");
        let before = tree(&copy.0);
        let result = crate::RidlMcp::new()
            .ridl_metrics(rmcp::handler::server::wrapper::Parameters(MetricsInput {
                path: copy.0.to_str().unwrap().into(),
            }))
            .await
            .unwrap();
        assert_eq!(result.is_error, Some(false));
        let content = result.structured_content.unwrap();
        assert_eq!(content["packages"].as_array().unwrap().len(), 3);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(
                result.content[0].as_text().unwrap().text.as_str()
            )
            .unwrap(),
            content
        );
        assert_eq!(tree(&copy.0), before);
    }
}
