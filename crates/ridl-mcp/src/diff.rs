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
                assert_eq!(
                    data["diagnostics"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|d| d["code"].as_str().unwrap())
                        // TYPL-406 (`missing-docs`) is left out: the fixture has no docs.
                        .filter(|code| *code != "TYPL-406")
                        .collect::<Vec<_>>(),
                    ["TYPL-103", "TYPL-011"]
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
