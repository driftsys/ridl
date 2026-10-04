//! Diagnostic catalogue and diff-category explanations.
use crate::snapshot::ToolError;
use ridl_core::diag::{ALL_CATALOGS, Severity};
use ridl_core::lint::default_level;
use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct ExplainInput {
    pub code: String,
}
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExplainOutput {
    Diagnostic {
        code: String,
        severity: String,
        summary: String,
        /// The lint name, when the code is a lint: its key in `[lints]` in ridl.toml.
        #[serde(skip_serializing_if = "Option::is_none")]
        lint: Option<String>,
        /// The lint's default level, `warn` or `info`; present when `lint` is.
        #[serde(skip_serializing_if = "Option::is_none")]
        default_level: Option<String>,
    },
    DiffCategory {
        category: String,
        text: String,
    },
}
pub fn explain(input: &ExplainInput) -> Result<ExplainOutput, ToolError> {
    if let Some(entry) = ALL_CATALOGS
        .iter()
        .flat_map(|(_, catalog)| catalog.iter())
        .find(|entry| entry.code.as_str() == input.code)
    {
        let severity = match entry.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        };
        return Ok(ExplainOutput::Diagnostic {
            code: entry.code.as_str().into(),
            severity: severity.into(),
            summary: entry.summary.into(),
            lint: entry.lint.map(str::to_string),
            default_level: default_level(entry).map(|level| level.as_str().to_string()),
        });
    }
    if let Some(category) = ridl_diff::category_from_word(&input.code) {
        return Ok(ExplainOutput::DiffCategory {
            category: ridl_diff::category_word(category).into(),
            text: ridl_diff::explain(category).into(),
        });
    }
    Err(ToolError::Request(format!(
        "`{}` is neither a diagnostic code (FORM-, TYPL-, RIDL-, RSDL-, MANI-) nor a ridl diff category word",
        input.code
    )))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::ToolError;
    #[test]
    fn explain_a_diagnostic_code() {
        let result = explain(&ExplainInput {
            code: "TYPL-002".into(),
        })
        .unwrap();
        let entry = ridl_core::diag::ALL_CATALOGS
            .iter()
            .flat_map(|(_, c)| c.iter())
            .find(|e| e.code.as_str() == "TYPL-002")
            .unwrap();
        match result {
            ExplainOutput::Diagnostic {
                code,
                severity,
                summary,
                ..
            } => {
                assert_eq!(code, "TYPL-002");
                assert_eq!(severity, "error");
                assert_eq!(summary, entry.summary);
            }
            _ => panic!("expected a diagnostic"),
        }
    }
    // A lint code names its lint and its default level (lint foundation spec
    // §7.4); an Error code, which is never a lint, carries neither field.
    #[test]
    fn explain_a_lint_code() {
        let value = |code: &str| {
            serde_json::to_value(explain(&ExplainInput { code: code.into() }).unwrap()).unwrap()
        };
        let lint = value("RIDL-100");
        assert_eq!(lint["kind"], "diagnostic");
        assert_eq!(lint["severity"], "warning");
        assert_eq!(lint["lint"], "missing-timing");
        assert_eq!(lint["default_level"], "warn");
        let info = value("RIDL-405");
        assert_eq!(info["kind"], "diagnostic");
        assert_eq!(info["severity"], "info");
        assert_eq!(info["lint"], "shared-error-type");
        assert_eq!(info["default_level"], "info");
        let error = value("RIDL-101");
        assert_eq!(error["kind"], "diagnostic");
        assert_eq!(error["severity"], "error");
        assert!(error.get("lint").is_none(), "{error}");
        assert!(error.get("default_level").is_none(), "{error}");
    }
    #[test]
    fn explain_a_diff_category() {
        match explain(&ExplainInput {
            code: "payload_changed".into(),
        })
        .unwrap()
        {
            ExplainOutput::DiffCategory { category, text } => {
                assert_eq!(category, "payload_changed");
                assert_eq!(
                    text,
                    ridl_diff::explain(ridl_diff::category_from_word("payload_changed").unwrap())
                );
            }
            _ => panic!("expected a diff category"),
        }
    }
    #[test]
    fn explain_an_unknown_code_is_an_error() {
        for code in ["TYPL-9999", "nonsense"] {
            match explain(&ExplainInput { code: code.into() }).unwrap_err() {
                ToolError::Request(message) => assert_eq!(
                    message,
                    format!(
                        "`{code}` is neither a diagnostic code (FORM-, TYPL-, RIDL-, RSDL-, MANI-) nor a ridl diff category word"
                    )
                ),
                _ => panic!("expected a request error"),
            }
        }
    }
}
