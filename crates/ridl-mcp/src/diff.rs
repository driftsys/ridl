//! The CLI diff contract exposed without writes or remote fetches.
use crate::{snapshot::ToolError, types::OverlayInput};
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use std::path::Path;
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct DiffInput {
    /// The old side as an .ir.json file, a snapshot directory such as <root>/.ridl/baseline, or a source path.
    pub old: String,
    /// The new side as an .ir.json file, a snapshot directory such as <root>/.ridl/baseline, or a source path.
    pub new: String,
    /// Optional unsaved source files applied to the new source side only.
    pub overlays: Option<Vec<OverlayInput>>,
}
pub fn diff(input: &DiffInput) -> Result<serde_json::Value, ToolError> {
    let mut db = ridl_core::RidlDatabase::default();
    let load_error = |side: &str, error: ridlc::DiffSideError| match error {
        ridlc::DiffSideError::Compile {
            diagnostics,
            sources,
        } => ToolError::WithData {
            message: format!("the {side} side does not compile"),
            data: serde_json::json!({ "diagnostics": ridl_core::diag::to_json(&diagnostics, &sources) }),
        },
        error => ToolError::Request(error.to_string()),
    };
    let old = ridlc::load_diff_side(&mut db, Path::new(&input.old), &[])
        .map_err(|error| load_error("old", error))?;
    let overlays = input
        .overlays
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|overlay| ridl_core::Overlay {
            path: overlay.path.clone().into(),
            text: overlay.source.clone(),
        })
        .collect::<Vec<_>>();
    let new = ridlc::load_diff_side(&mut db, Path::new(&input.new), &overlays)
        .map_err(|error| load_error("new", error))?;
    let report = ridl_diff::diff_workspaces(
        &old.packages,
        old.system.as_ref(),
        &new.packages,
        new.system.as_ref(),
        &[ridlc::std_ir()],
    );
    Ok(serde_json::from_str(&ridl_diff::render_json(&report))
        .expect("render_json writes valid JSON"))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{ToolError, tests::fixture};
    use crate::types::OverlayInput;
    #[test]
    fn diff_matches_the_cli() {
        let old = fixture("ws");
        let new = fixture("ws-v2");
        let mut db = ridl_core::RidlDatabase::default();
        let a = ridlc::load_diff_side(&mut db, std::path::Path::new(&old), &[]).unwrap();
        let b = ridlc::load_diff_side(&mut db, std::path::Path::new(&new), &[]).unwrap();
        let report = ridl_diff::diff_workspaces(
            &a.packages,
            a.system.as_ref(),
            &b.packages,
            b.system.as_ref(),
            &[ridlc::std_ir()],
        );
        let expected: serde_json::Value =
            serde_json::from_str(&ridl_diff::render_json(&report)).unwrap();
        let result = diff(&DiffInput {
            old,
            new,
            overlays: None,
        })
        .unwrap();
        assert_eq!(result, expected);
        assert_eq!(result["verdict"], "breaking");
    }
    #[test]
    fn diff_with_an_overlay_on_new() {
        let root = fixture("ws");
        let path = format!("{root}/a/a.ridl");
        let source = std::fs::read_to_string(&path)
            .unwrap()
            .replace("FAIL = 2", "FAIL = 2\n  DEAD = 3");
        let result = diff(&DiffInput {
            old: root.clone(),
            new: root,
            overlays: Some(vec![OverlayInput { path, source }]),
        })
        .unwrap();
        assert_eq!(result["verdict"], "compatible");
        assert_ne!(result["verdict"], "identical");
    }
    #[test]
    fn diff_with_a_missing_old_side_is_a_tool_error() {
        let old = format!("{}/.ridl/baseline", fixture("ws"));
        let expected = format!("{old}: `{old}` does not exist");
        match diff(&DiffInput {
            old,
            new: fixture("ws"),
            overlays: None,
        })
        .unwrap_err()
        {
            ToolError::Request(message) => assert_eq!(message, expected),
            _ => panic!("expected a request error"),
        }
    }
    #[test]
    fn diff_with_a_side_that_does_not_compile_carries_diagnostics() {
        match diff(&DiffInput {
            old: fixture("ws"),
            new: fixture("ws-diag"),
            overlays: None,
        })
        .unwrap_err()
        {
            ToolError::WithData { message, data } => {
                assert_eq!(message, "the new side does not compile");
                let path = fixture("ws-diag");
                let compiled = ridlc::compile_workspace(
                    &mut ridl_core::RidlDatabase::default(),
                    std::path::Path::new(&path),
                )
                .unwrap();
                assert_eq!(
                    data["diagnostics"],
                    serde_json::to_value(ridl_core::diag::to_json(
                        &compiled.diagnostics,
                        &compiled.sources,
                    ))
                    .unwrap()
                );
                let all_diagnostics = data["diagnostics"].as_array().unwrap();
                let missing_docs_code = ridl_core::lint::lint_by_name("missing-docs")
                    .unwrap()
                    .code
                    .as_str();
                let expected_codes = std::iter::once("TYPL-103")
                    .chain(std::iter::repeat_n(missing_docs_code, 15))
                    .chain(std::iter::once("TYPL-011"))
                    .chain(std::iter::repeat_n(missing_docs_code, 8))
                    .chain(["TYPL-223", "RIDL-414"])
                    .collect::<Vec<_>>();
                assert_eq!(
                    all_diagnostics
                        .iter()
                        .map(|diagnostic| diagnostic["code"].as_str().unwrap())
                        .collect::<Vec<_>>(),
                    expected_codes
                );
                assert!(
                    all_diagnostics
                        .iter()
                        .filter(|diagnostic| diagnostic["lint"] == "missing-docs")
                        .all(|diagnostic| diagnostic["severity"] == "warning")
                );
                // Keep the incoming fixture allowance while checking every other diagnostic exactly.
                let exact_diagnostics = all_diagnostics
                    .iter()
                    .filter(|diagnostic| diagnostic["lint"] != "missing-docs")
                    .collect::<Vec<_>>();
                assert_eq!(
                    exact_diagnostics
                        .iter()
                        .map(|d| d["code"].as_str().unwrap())
                        .collect::<Vec<_>>(),
                    ["TYPL-103", "TYPL-011", "TYPL-223", "RIDL-414"]
                );
                assert_eq!(
                    serde_json::to_value(exact_diagnostics).unwrap(),
                    serde_json::json!([
                        {
                            "code": "TYPL-103",
                            "severity": "warning",
                            "lint": "unbounded-length",
                            "message": "`string` without explicit bounds; the default `[0..256]` applies",
                            "span": {
                                "path": format!("{}/a/a.ridl", fixture("ws-diag")),
                                "start": {
                                    "line": 25,
                                    "column": 11
                                },
                                "end": {
                                    "line": 25,
                                    "column": 17
                                }
                            },
                            "labels": [],
                            "fixes": []
                        },
                        {
                            "code": "TYPL-011",
                            "severity": "error",
                            "message": "unknown type name `Missing`",
                            "span": {
                                "path": format!("{}/b/b.ridl", fixture("ws-diag")),
                                "start": {
                                    "line": 18,
                                    "column": 25
                                },
                                "end": {
                                    "line": 18,
                                    "column": 32
                                }
                            },
                            "labels": [],
                            "fixes": []
                        },
                        {
                            "code": "TYPL-223",
                            "severity": "info",
                            "lint": "inconsistent-abbreviation",
                            "message": "`read` in `readSpeed` abbreviates `reading`, used in `Reading`",
                            "span": {
                                "path": format!("{}/b/b.ridl", fixture("ws-diag")),
                                "start": {
                                    "line": 25,
                                    "column": 9
                                },
                                "end": {
                                    "line": 25,
                                    "column": 18
                                }
                            },
                            "labels": [],
                            "fixes": []
                        },
                        {
                            "code": "RIDL-414",
                            "severity": "info",
                            "lint": "low-cohesion-interface",
                            "message": "interface `Status` splits into 4 groups of members that share no type: [speed], [reading], [setLevel], [outcome]",
                            "span": {
                                "path": format!("{}/b/b.ridl", fixture("ws-diag")),
                                "start": {
                                    "line": 14,
                                    "column": 11
                                },
                                "end": {
                                    "line": 14,
                                    "column": 17
                                }
                            },
                            "labels": [],
                            "fixes": []
                        }
                    ])
                );
            }
            _ => panic!("expected diagnostics"),
        }
    }

    #[test]
    fn diff_uses_std_as_context() {
        let copy = crate::snapshot::tests::TempWorkspace::copy("ws");
        let path = copy.0.join("a/a.ridl");
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(
            &path,
            text.replace(
                "  health: Health\n}",
                "  health: Health\n  stamp: Timestamp\n}",
            ),
        )
        .unwrap();
        let input = DiffInput {
            old: fixture("ws"),
            new: copy.0.to_str().unwrap().into(),
            overlays: None,
        };
        let cli = std::process::Command::new("cargo")
            .current_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .args([
                "run", "-q", "-p", "ridl-cli", "--locked", "--", "diff", "--format", "json",
                &input.old, &input.new,
            ])
            .output()
            .unwrap();
        assert_eq!(
            cli.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&cli.stderr)
        );
        let expected: serde_json::Value = serde_json::from_slice(&cli.stdout).unwrap();
        assert_eq!(expected["verdict"], "compatible");
        assert_eq!(diff(&input).unwrap(), expected);
    }
}
