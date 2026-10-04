//! The SARIF 2.1.0 projection of a diagnostic list (ADR-0024
//! decision 12), which `ridl check --format sarif` writes to stdout.
//!
//! The log holds one run. Its `tool.driver.rules` lists every row of
//! [`ALL_CATALOGS`], Error rows included, so a viewer can describe every
//! result; each result points at its rule by `ruleId` and `ruleIndex`. The
//! structs below are the subset of SARIF the log uses; no SARIF crate is
//! added. Fix-its are not emitted: the JSON output already carries them.
//!
//! Every artifact URI is relative to one base, the working directory the
//! caller passes as `root`, which the run names `%SRCROOT%` in
//! `originalUriBaseIds`; a file outside it is an absolute `file://` URI.
//! One log therefore has one base whatever path was checked, which is what a
//! code-scanning upload run from the checkout root expects.

use std::collections::BTreeMap;
use std::path::{Component, Path};

use serde::Serialize;

use super::{ALL_CATALOGS, CatalogEntry, Diagnostic, Label, Severity, SourceMap, Span, line_col};

/// The `$schema` of every log.
pub const SCHEMA: &str = "https://json.schemastore.org/sarif-2.1.0.json";

/// The SARIF `version` of every log.
pub const VERSION: &str = "2.1.0";

/// RIDL columns count characters, not the UTF-16 code units SARIF defaults to.
const COLUMN_KIND: &str = "unicodeCodePoints";

/// The `uriBaseId` of every artifact under the working directory.
const SRCROOT: &str = "%SRCROOT%";

/// A SARIF 2.1.0 log with one run.
#[derive(Debug, Serialize)]
pub struct SarifLog {
    #[serde(rename = "$schema")]
    pub schema: &'static str,
    pub version: &'static str,
    pub runs: Vec<Run>,
}

/// One run. `original_uri_base_ids` names the working directory as
/// `%SRCROOT%`; it is absent when the caller gave no root.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub tool: Tool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_uri_base_ids: Option<BTreeMap<&'static str, ArtifactLocation>>,
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

/// An artifact. `uri_base_id` is `%SRCROOT%` for a file under the working
/// directory, whose `uri` is then relative to it, and absent for a file
/// outside it, whose `uri` is then an absolute `file://` URI.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactLocation {
    pub uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri_base_id: Option<&'static str>,
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
/// severity, a result's from its effective severity (ADR-0024 decision 12).
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

/// A path after lexical normalisation: `.` is dropped and `..` removes the
/// segment before it, without consulting the filesystem. `prefix` is a
/// Windows drive or UNC prefix; `rooted` is whether the path is absolute.
#[derive(Debug, Default)]
struct Normalized {
    prefix: Option<String>,
    rooted: bool,
    segments: Vec<String>,
}

impl Normalized {
    fn new(path: &Path) -> Self {
        let mut normalized = Self::default();
        for component in path.components() {
            match component {
                Component::Prefix(prefix) => {
                    normalized.prefix = Some(prefix.as_os_str().to_string_lossy().into_owned());
                }
                Component::RootDir => normalized.rooted = true,
                Component::CurDir => {}
                Component::ParentDir => match normalized.segments.last() {
                    Some(last) if last != ".." => {
                        normalized.segments.pop();
                    }
                    // `/..` is `/`; a leading `..` of a relative path stays.
                    _ if normalized.rooted => {}
                    _ => normalized.segments.push("..".to_string()),
                },
                Component::Normal(segment) => {
                    normalized
                        .segments
                        .push(segment.to_string_lossy().into_owned());
                }
            }
        }
        normalized
    }

    /// The segments after `root`, or `None` when this path is not under it.
    fn strip_prefix(&self, root: &Normalized) -> Option<&[String]> {
        (self.prefix == root.prefix
            && self.rooted == root.rooted
            && self.segments.starts_with(&root.segments))
        .then(|| &self.segments[root.segments.len()..])
    }

    /// The URI of this path: `file://` plus the absolute path when it is
    /// rooted, else the relative path. A Windows prefix is written after
    /// `file:///` with `\` turned into `/` (`file:///C:/...`); no test covers
    /// that branch, because the test suite runs on Unix only. Every other
    /// segment is percent-encoded.
    fn uri(&self) -> String {
        let mut uri = String::new();
        if self.rooted {
            uri.push_str("file://");
            if let Some(prefix) = &self.prefix {
                uri.push('/');
                uri.push_str(&prefix.replace('\\', "/"));
            }
            uri.push('/');
        }
        uri.push_str(&join_segments(&self.segments));
        uri
    }
}

/// The segments joined with `/`, each percent-encoded: the RFC 3986
/// unreserved characters pass, every other byte of the UTF-8 form is `%XX` in
/// uppercase hex.
fn join_segments(segments: &[String]) -> String {
    let mut uri = String::new();
    for (index, segment) in segments.iter().enumerate() {
        if index > 0 {
            uri.push('/');
        }
        for byte in segment.bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                    uri.push(byte as char);
                }
                _ => uri.push_str(&format!("%{byte:02X}")),
            }
        }
    }
    uri
}

/// The working directory, as given and lexically normalised.
struct Root<'a> {
    path: &'a Path,
    normalized: Normalized,
}

/// The artifact location of a source path. A relative path is first joined
/// onto `root`, then `.` and `..` are resolved lexically. A path under `root`
/// is relative to it with `uriBaseId` `%SRCROOT%`; any other path is an
/// absolute `file://` URI with no base. With no `root`, every path is written
/// as it is, so an absolute path is a `file://` URI and a relative path stays
/// relative with no base.
fn artifact_location(path: &str, root: Option<&Root>) -> ArtifactLocation {
    let path = Path::new(path);
    let Some(root) = root else {
        return ArtifactLocation {
            uri: Normalized::new(path).uri(),
            uri_base_id: None,
        };
    };
    // `Path::join` keeps an absolute `path` as it is.
    let full = Normalized::new(&root.path.join(path));
    match full.strip_prefix(&root.normalized) {
        Some(relative) => ArtifactLocation {
            uri: join_segments(relative),
            uri_base_id: Some(SRCROOT),
        },
        None => ArtifactLocation {
            uri: full.uri(),
            uri_base_id: None,
        },
    }
}

/// The `originalUriBaseIds` of a run whose root is `root`: `%SRCROOT%` as a
/// `file://` URI that ends with `/`.
fn original_uri_base_ids(root: &Normalized) -> BTreeMap<&'static str, ArtifactLocation> {
    let mut uri = root.uri();
    if !uri.ends_with('/') {
        uri.push('/');
    }
    BTreeMap::from([(
        SRCROOT,
        ArtifactLocation {
            uri,
            uri_base_id: None,
        },
    )])
}

/// The physical location of `span`, or `None` when its file is not in
/// `sources`.
fn physical_location(
    span: Span,
    sources: &SourceMap,
    root: Option<&Root>,
) -> Option<PhysicalLocation> {
    let path = sources.path(span.file)?;
    let text = sources.text(span.file)?;
    let start = line_col(text, span.range.start());
    let end = line_col(text, span.range.end());
    Some(PhysicalLocation {
        artifact_location: artifact_location(path, root),
        region: Region {
            start_line: start.line,
            start_column: start.column,
            end_line: end.line,
            end_column: end.column,
        },
    })
}

fn related_location(label: &Label, sources: &SourceMap, root: Option<&Root>) -> Location {
    Location {
        physical_location: physical_location(label.span, sources, root),
        message: Some(Message {
            text: label.message.clone(),
        }),
    }
}

/// Projects `diagnostics` onto one SARIF 2.1.0 log. `root` is the working
/// directory: every artifact under it is a URI relative to it with
/// `uriBaseId` `%SRCROOT%`, which `originalUriBaseIds` resolves; every other
/// artifact is an absolute `file://` URI. With no `root` (the working
/// directory could not be read) the run has no `originalUriBaseIds` and every
/// artifact is written as its source path is, after lexical normalisation: an
/// absolute path is an absolute `file://` URI, a relative path stays relative
/// with no `uriBaseId`. `tool_version` is the version `tool.driver` reports.
pub fn to_sarif(
    diagnostics: &[Diagnostic],
    sources: &SourceMap,
    root: Option<&Path>,
    tool_version: &str,
) -> SarifLog {
    let root = root.map(|path| Root {
        path,
        normalized: Normalized::new(path),
    });
    let root = root.as_ref();
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
            original_uri_base_ids: root.map(|root| original_uri_base_ids(&root.normalized)),
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
    /// wire shape; the assertions below check the SARIF properties that
    /// ADR-0024 decisions 5, 12 and 13 and docs/book/cli-reference.md describe.
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

        let log = to_sarif(&diagnostics, &sources, Some(Path::new("/ws")), "0.0.0-test");
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
        assert_eq!(run["originalUriBaseIds"]["%SRCROOT%"]["uri"], "file:///ws/");

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
        assert_eq!(location["artifactLocation"]["uriBaseId"], "%SRCROOT%");
        assert_eq!(location["region"]["startLine"], 3);
        assert_eq!(location["region"]["startColumn"], 10);
        assert_eq!(location["region"]["endLine"], 3);
        assert_eq!(location["region"]["endColumn"], 15);
        let related = ridl_100["relatedLocations"]
            .as_array()
            .expect("relatedLocations is an array");
        assert_eq!(related.len(), 1);
        assert_eq!(related[0]["message"]["text"], "in this interface");
        assert_eq!(
            related[0]["physicalLocation"]["artifactLocation"]["uriBaseId"],
            "%SRCROOT%"
        );

        let uncoded = &results[1];
        assert!(uncoded.get("ruleId").is_none(), "{uncoded}");
        assert!(uncoded.get("ruleIndex").is_none(), "{uncoded}");
        assert_eq!(uncoded["level"], "error");

        let detached = &results[2];
        assert_eq!(detached["ruleId"], "MANI-101");
        assert!(detached.get("locations").is_none(), "{detached}");

        assert!(results.iter().all(|result| result.get("fixes").is_none()));
    }

    /// The log of one RIDL-100 at `path`, projected against `root`.
    fn log_for(root: Option<&Path>, path: &str) -> serde_json::Value {
        let mut sources = SourceMap::new();
        let file = sources.file_id(path, "package p\n");
        let diagnostics = vec![Diagnostic {
            code: DiagCode::RIDL_100,
            severity: Severity::Warning,
            message: "m".to_string(),
            primary: span(file, 0, 7),
            labels: Vec::new(),
            fixits: Vec::new(),
        }];
        serde_json::to_value(to_sarif(&diagnostics, &sources, root, "0"))
            .expect("the log serializes")
    }

    /// The `(uri, uriBaseId)` of the one result's primary location when a
    /// source at `path` is projected against `root`.
    fn uri_of(root: Option<&Path>, path: &str) -> (String, Option<String>) {
        let log = log_for(root, path);
        let location =
            &log["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"];
        (
            location["uri"]
                .as_str()
                .expect("uri is a string")
                .to_string(),
            location["uriBaseId"].as_str().map(str::to_string),
        )
    }

    const SRCROOT: Option<&str> = Some("%SRCROOT%");

    fn srcroot() -> Option<String> {
        SRCROOT.map(str::to_string)
    }

    /// A source path outside `root` is an absolute `file://` URI with no base.
    #[test]
    fn a_path_outside_the_root_is_an_absolute_file_uri() {
        let root = Some(Path::new("/ws"));
        assert_eq!(
            uri_of(root, "/elsewhere/a.ridl"),
            ("file:///elsewhere/a.ridl".to_string(), None)
        );
        assert_eq!(
            uri_of(root, "/ws-other/a.ridl"),
            ("file:///ws-other/a.ridl".to_string(), None),
            "a sibling whose name starts with the root's name is outside it"
        );
    }

    /// `.` and `..` are resolved lexically before the path is compared with
    /// `root`: a `..` that leaves the root makes the file an outside file, a
    /// `..` that stays inside is removed.
    #[test]
    fn dot_and_parent_dir_components_are_normalised_lexically() {
        let root = Some(Path::new("/ws"));
        assert_eq!(
            uri_of(root, "/ws/../other/a.ridl"),
            ("file:///other/a.ridl".to_string(), None)
        );
        assert_eq!(
            uri_of(root, "/ws/sub/../a.ridl"),
            ("a.ridl".to_string(), srcroot())
        );
        assert_eq!(
            uri_of(root, "/ws/./sub/a.ridl"),
            ("sub/a.ridl".to_string(), srcroot())
        );
        assert_eq!(
            uri_of(Some(Path::new("/ws/sub/..")), "/ws/a.ridl"),
            ("a.ridl".to_string(), srcroot()),
            "the root is normalised too"
        );
    }

    /// A relative source path, as `ridl check` interns the paths given on the
    /// command line, is joined onto `root` first.
    #[test]
    fn a_relative_path_is_joined_onto_the_root() {
        let root = Some(Path::new("/ws"));
        assert_eq!(
            uri_of(root, "sensor/a.ridl"),
            ("sensor/a.ridl".to_string(), srcroot())
        );
        assert_eq!(uri_of(root, "./a.ridl"), ("a.ridl".to_string(), srcroot()));
        assert_eq!(
            uri_of(root, "../x/a.ridl"),
            ("file:///x/a.ridl".to_string(), None)
        );
    }

    /// Every segment is percent-encoded: the RFC 3986 unreserved characters
    /// pass, every other byte of the UTF-8 form is `%XX` in uppercase hex. The
    /// `%SRCROOT%` base is encoded the same way.
    #[test]
    fn segments_are_percent_encoded() {
        let root = Some(Path::new("/my ws"));
        assert_eq!(
            uri_of(root, "/my ws/b c.typl"),
            ("b%20c.typl".to_string(), srcroot())
        );
        assert_eq!(
            uri_of(root, "/my ws/sub/my file#1.ridl"),
            ("sub/my%20file%231.ridl".to_string(), srcroot())
        );
        assert_eq!(
            uri_of(root, "/my ws/caf\u{e9}.typl"),
            ("caf%C3%A9.typl".to_string(), srcroot())
        );
        assert_eq!(
            uri_of(root, "/else where/a#b.ridl"),
            ("file:///else%20where/a%23b.ridl".to_string(), None)
        );
        assert_eq!(
            uri_of(root, "/my ws/100%.ridl"),
            ("100%25.ridl".to_string(), srcroot()),
            "a literal `%` is encoded, so the URI decodes to the path"
        );
        assert_eq!(
            uri_of(root, "/my ws/a:b.ridl"),
            ("a%3Ab.ridl".to_string(), srcroot()),
            "`:` is encoded, so a relative URI is not read as a scheme"
        );
        assert_eq!(
            uri_of(root, "/my ws/~u.ridl"),
            ("~u.ridl".to_string(), srcroot()),
            "`~` is unreserved and passes"
        );
        let log = log_for(root, "/my ws/b c.typl");
        assert_eq!(
            log["runs"][0]["originalUriBaseIds"]["%SRCROOT%"]["uri"],
            "file:///my%20ws/"
        );
    }

    /// With no root (the working directory could not be read) an absolute
    /// file is an absolute `file://` URI and the run has no
    /// `originalUriBaseIds`.
    #[test]
    fn no_root_gives_absolute_uris_and_no_base_ids() {
        assert_eq!(
            uri_of(None, "/ws/sensor/a.ridl"),
            ("file:///ws/sensor/a.ridl".to_string(), None)
        );
        let log = log_for(None, "/ws/sensor/a.ridl");
        assert!(log["runs"][0].get("originalUriBaseIds").is_none(), "{log}");
    }

    /// With no root a relative file stays relative, with no `uriBaseId`. A
    /// leading `..` is kept, and a second one does not remove the first.
    #[test]
    fn no_root_keeps_a_relative_path_relative() {
        assert_eq!(
            uri_of(None, "sensor/a.ridl"),
            ("sensor/a.ridl".to_string(), None)
        );
        assert_eq!(
            uri_of(None, "../../a.ridl"),
            ("../../a.ridl".to_string(), None)
        );
        assert_eq!(
            uri_of(None, "../x/../a.ridl"),
            ("../a.ridl".to_string(), None)
        );
    }

    /// A `..` directly under the filesystem root is dropped, as the
    /// filesystem would: `/../x` is `/x`.
    #[test]
    fn a_parent_dir_under_the_filesystem_root_is_dropped() {
        assert_eq!(
            uri_of(None, "/../x.ridl"),
            ("file:///x.ridl".to_string(), None)
        );
        assert_eq!(
            uri_of(Some(Path::new("/ws")), "/../ws/a.ridl"),
            ("a.ridl".to_string(), srcroot())
        );
    }

    /// The filesystem root as the working directory gives the base
    /// `file:///`, not `file:////`.
    #[test]
    fn the_filesystem_root_as_root_gives_one_trailing_slash() {
        let root = Some(Path::new("/"));
        let log = log_for(root, "/a.ridl");
        assert_eq!(
            log["runs"][0]["originalUriBaseIds"]["%SRCROOT%"]["uri"],
            "file:///"
        );
        assert_eq!(uri_of(root, "/a.ridl"), ("a.ridl".to_string(), srcroot()));
    }

    /// A relative root does not claim an absolute source path whose first
    /// segments spell the root's name: the two are compared as rooted or not
    /// before their segments are.
    #[test]
    fn a_relative_root_does_not_claim_an_absolute_path() {
        assert_eq!(
            uri_of(Some(Path::new("ws")), "/ws/a.ridl"),
            ("file:///ws/a.ridl".to_string(), None)
        );
    }
}
