//! The SARIF 2.1.0 projection of a diagnostic list (lint foundation spec
//! §7.3), which `ridl check --format sarif` writes to stdout.
//!
//! The log holds one run. Its `tool.driver.rules` lists every row of
//! [`ALL_CATALOGS`], Error rows included, so a viewer can describe every
//! result; each result points at its rule by `ruleId` and `ruleIndex`. The
//! structs below are the subset of SARIF the log uses; no SARIF crate is
//! added. Fix-its are not emitted: the JSON output already carries them.

use std::path::{Component, Path};

use serde::Serialize;

use super::{ALL_CATALOGS, CatalogEntry, Diagnostic, Label, Severity, SourceMap, Span, line_col};

/// The `$schema` of every log.
pub const SCHEMA: &str = "https://json.schemastore.org/sarif-2.1.0.json";

/// The SARIF `version` of every log.
pub const VERSION: &str = "2.1.0";

/// RIDL columns count characters, not the UTF-16 code units SARIF defaults to.
const COLUMN_KIND: &str = "unicodeCodePoints";

/// A SARIF 2.1.0 log with one run.
#[derive(Debug, Serialize)]
pub struct SarifLog {
    #[serde(rename = "$schema")]
    pub schema: &'static str,
    pub version: &'static str,
    pub runs: Vec<Run>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub tool: Tool,
    pub results: Vec<SarifResult>,
    pub column_kind: &'static str,
}

#[derive(Debug, Serialize)]
pub struct Tool {
    pub driver: Driver,
}

#[derive(Debug, Serialize)]
pub struct Driver {
    pub name: &'static str,
    pub version: String,
    pub rules: Vec<Rule>,
}

/// One catalogue row. `name` is the lint name, absent for an Error row.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub id: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<&'static str>,
    pub short_description: Message,
    pub default_configuration: Configuration,
}

#[derive(Debug, Serialize)]
pub struct Message {
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct Configuration {
    pub level: &'static str,
}

/// One diagnostic. `rule_id` and `rule_index` are absent for an uncoded
/// diagnostic; `locations` is absent when the primary span has no file in the
/// source map ([`super::FileId::DETACHED`]).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_id: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_index: Option<usize>,
    pub level: &'static str,
    pub message: Message,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locations: Option<Vec<Location>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub related_locations: Vec<Location>,
}

/// A location. `physical_location` is absent for a label on a detached span,
/// which then carries only its message.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub physical_location: Option<PhysicalLocation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<Message>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicalLocation {
    pub artifact_location: ArtifactLocation,
    pub region: Region,
}

#[derive(Debug, Serialize)]
pub struct ArtifactLocation {
    pub uri: String,
}

/// A 1-based region. `end_column` is the column one past the last character,
/// as in SARIF and in the JSON contract's exclusive `end`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Region {
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

/// The SARIF level of a severity: a rule's default from its catalogue
/// severity, a result's from its effective severity (spec §7.3).
fn sarif_level(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "note",
    }
}

fn rule(entry: &CatalogEntry) -> Rule {
    Rule {
        id: entry.code.as_str(),
        name: entry.lint,
        short_description: Message {
            text: entry.summary.to_string(),
        },
        default_configuration: Configuration {
            level: sarif_level(entry.severity),
        },
    }
}

/// The artifact URI of a source path: the path with `root` stripped, written
/// with `/` separators. A path outside `root` is kept as it is.
fn artifact_uri(path: &str, root: &Path) -> String {
    match Path::new(path).strip_prefix(root) {
        Ok(relative) => relative
            .components()
            .filter_map(|component| match component {
                Component::Normal(segment) => Some(segment.to_string_lossy()),
                // Dropping `..` would name a different file.
                Component::ParentDir => Some("..".into()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("/"),
        Err(_) => path.to_string(),
    }
}

/// The physical location of `span`, or `None` when its file is not in
/// `sources`.
fn physical_location(span: Span, sources: &SourceMap, root: &Path) -> Option<PhysicalLocation> {
    let path = sources.path(span.file)?;
    let text = sources.text(span.file)?;
    let start = line_col(text, span.range.start());
    let end = line_col(text, span.range.end());
    Some(PhysicalLocation {
        artifact_location: ArtifactLocation {
            uri: artifact_uri(path, root),
        },
        region: Region {
            start_line: start.line,
            start_column: start.column,
            end_line: end.line,
            end_column: end.column,
        },
    })
}

fn related_location(label: &Label, sources: &SourceMap, root: &Path) -> Location {
    Location {
        physical_location: physical_location(label.span, sources, root),
        message: Some(Message {
            text: label.message.clone(),
        }),
    }
}

/// Projects `diagnostics` onto one SARIF 2.1.0 log. Every artifact URI is the
/// source path relative to `root`, with `/` separators; a path outside `root`
/// stays as it is. `tool_version` is the version `tool.driver` reports.
pub fn to_sarif(
    diagnostics: &[Diagnostic],
    sources: &SourceMap,
    root: &Path,
    tool_version: &str,
) -> SarifLog {
    let entries: Vec<&CatalogEntry> = ALL_CATALOGS
        .iter()
        .flat_map(|(_, catalog)| catalog.iter())
        .collect();
    let rules = entries.iter().map(|entry| rule(entry)).collect();
    let results = diagnostics
        .iter()
        .map(|diagnostic| {
            let rule_index = (!diagnostic.code.is_empty())
                .then(|| {
                    entries
                        .iter()
                        .position(|entry| entry.code == diagnostic.code)
                })
                .flatten();
            SarifResult {
                rule_id: (!diagnostic.code.is_empty()).then(|| diagnostic.code.as_str()),
                rule_index,
                level: sarif_level(diagnostic.severity),
                message: Message {
                    text: diagnostic.message.clone(),
                },
                locations: physical_location(diagnostic.primary, sources, root).map(|location| {
                    vec![Location {
                        physical_location: Some(location),
                        message: None,
                    }]
                }),
                related_locations: diagnostic
                    .labels
                    .iter()
                    .map(|label| related_location(label, sources, root))
                    .collect(),
            }
        })
        .collect();
    SarifLog {
        schema: SCHEMA,
        version: VERSION,
        runs: vec![Run {
            tool: Tool {
                driver: Driver {
                    name: "ridl",
                    version: tool_version.to_string(),
                    rules,
                },
            },
            results,
            column_kind: COLUMN_KIND,
        }],
    }
}

#[cfg(test)]
mod tests {
    use rowan::{TextRange, TextSize};
    use std::path::Path;

    use super::to_sarif;
    use crate::diag::{
        ALL_CATALOGS, DiagCode, Diagnostic, FileId, Label, Severity, SourceMap, Span,
    };

    fn span(file: FileId, start: u32, end: u32) -> Span {
        Span {
            file,
            range: TextRange::new(TextSize::new(start), TextSize::new(end)),
        }
    }

    /// One RIDL-100 Warning with a label, one uncoded Error, and one MANI-101
    /// Error on a detached span, projected onto SARIF. The snapshot pins the
    /// wire shape; the assertions below name the properties spec §7.3 fixes.
    #[test]
    fn sarif_shape() {
        let text = "package p\ninterface S {\n  signal speed: Speed\n}\n";
        let mut sources = SourceMap::new();
        let file = sources.file_id("/ws/sensor/demo.ridl", text);
        let at_signal = span(file, 33, 38); // `speed`
        let at_interface = span(file, 20, 21); // `S`
        let diagnostics = vec![
            Diagnostic {
                code: DiagCode::RIDL_100,
                severity: Severity::Warning,
                message: "signal without a timing annotation".to_string(),
                primary: at_signal,
                labels: vec![Label {
                    span: at_interface,
                    message: "in this interface".to_string(),
                }],
                fixits: Vec::new(),
            },
            Diagnostic {
                code: DiagCode::NONE,
                severity: Severity::Error,
                message: "expected a type".to_string(),
                primary: span(file, 10, 19),
                labels: Vec::new(),
                fixits: Vec::new(),
            },
            Diagnostic {
                code: DiagCode::MANI_101,
                severity: Severity::Error,
                message: "could not fetch `demo`".to_string(),
                primary: span(FileId::DETACHED, 0, 0),
                labels: Vec::new(),
                fixits: Vec::new(),
            },
        ];

        let log = to_sarif(&diagnostics, &sources, Path::new("/ws"), "0.0.0-test");
        insta::assert_json_snapshot!("sarif_shape", log);

        let value = serde_json::to_value(&log).expect("the log serializes");
        assert_eq!(value["version"], "2.1.0");
        assert_eq!(
            value["$schema"],
            "https://json.schemastore.org/sarif-2.1.0.json"
        );
        let run = &value["runs"][0];
        assert_eq!(run["columnKind"], "unicodeCodePoints");
        assert_eq!(run["tool"]["driver"]["name"], "ridl");
        assert_eq!(run["tool"]["driver"]["version"], "0.0.0-test");

        let rules = run["tool"]["driver"]["rules"]
            .as_array()
            .expect("rules is an array");
        let row_count: usize = ALL_CATALOGS.iter().map(|(_, rows)| rows.len()).sum();
        assert_eq!(rules.len(), row_count);
        let (index, rule) = rules
            .iter()
            .enumerate()
            .find(|(_, rule)| rule["id"] == "RIDL-100")
            .expect("a RIDL-100 rule");
        assert_eq!(rule["name"], "missing-timing");
        assert_eq!(rule["defaultConfiguration"]["level"], "warning");
        assert!(rules.iter().all(|rule| rule.get("helpUri").is_none()));
        let error_rule = rules
            .iter()
            .find(|rule| rule["id"] == "MANI-101")
            .expect("a MANI-101 rule");
        assert!(error_rule.get("name").is_none(), "{error_rule}");
        assert_eq!(error_rule["defaultConfiguration"]["level"], "error");

        let results = run["results"].as_array().expect("results is an array");
        assert_eq!(results.len(), 3);
        let ridl_100 = &results[0];
        assert_eq!(ridl_100["ruleId"], "RIDL-100");
        assert_eq!(ridl_100["ruleIndex"], index);
        assert_eq!(ridl_100["level"], "warning");
        let location = &ridl_100["locations"][0]["physicalLocation"];
        assert_eq!(location["artifactLocation"]["uri"], "sensor/demo.ridl");
        assert_eq!(location["region"]["startLine"], 3);
        assert_eq!(location["region"]["startColumn"], 10);
        assert_eq!(location["region"]["endLine"], 3);
        assert_eq!(location["region"]["endColumn"], 15);
        let related = ridl_100["relatedLocations"]
            .as_array()
            .expect("relatedLocations is an array");
        assert_eq!(related.len(), 1);
        assert_eq!(related[0]["message"]["text"], "in this interface");

        let uncoded = &results[1];
        assert!(uncoded.get("ruleId").is_none(), "{uncoded}");
        assert!(uncoded.get("ruleIndex").is_none(), "{uncoded}");
        assert_eq!(uncoded["level"], "error");

        let detached = &results[2];
        assert_eq!(detached["ruleId"], "MANI-101");
        assert!(detached.get("locations").is_none(), "{detached}");

        assert!(results.iter().all(|result| result.get("fixes").is_none()));
    }

    /// A source path outside `root` is kept as it is.
    #[test]
    fn a_path_outside_the_root_is_unchanged() {
        let mut sources = SourceMap::new();
        let file = sources.file_id("/elsewhere/a.ridl", "package p\n");
        let diagnostics = vec![Diagnostic {
            code: DiagCode::RIDL_100,
            severity: Severity::Warning,
            message: "m".to_string(),
            primary: span(file, 0, 7),
            labels: Vec::new(),
            fixits: Vec::new(),
        }];
        let log = to_sarif(&diagnostics, &sources, Path::new("/ws"), "0");
        let value = serde_json::to_value(&log).expect("the log serializes");
        assert_eq!(
            value["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]
                ["uri"],
            "/elsewhere/a.ridl"
        );
    }

    /// A `..` component stays in the URI, so the URI names the same file.
    #[test]
    fn a_parent_dir_component_is_kept() {
        let mut sources = SourceMap::new();
        let file = sources.file_id("/ws/../other/a.ridl", "package p\n");
        let diagnostics = vec![Diagnostic {
            code: DiagCode::RIDL_100,
            severity: Severity::Warning,
            message: "m".to_string(),
            primary: span(file, 0, 7),
            labels: Vec::new(),
            fixits: Vec::new(),
        }];
        let log = to_sarif(&diagnostics, &sources, Path::new("/ws"), "0");
        let value = serde_json::to_value(&log).expect("the log serializes");
        assert_eq!(
            value["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]
                ["uri"],
            "../other/a.ridl"
        );
    }
}
