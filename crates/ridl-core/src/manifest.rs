//! The `ridl.toml` manifest parser (ADR-0002 §4).
//!
//! A manifest has one file shape and two mutually exclusive modes: a standalone
//! [`ManifestKind::Package`] or a [`ManifestKind::Workspace`] (ADR-0002 §4).
//! Both modes may carry an `[imports]` table that aliases logical package names
//! to URLs. [`parse_manifest`] reads one manifest's text and returns the parsed
//! [`Manifest`] together with any [`Diagnostic`]s — content problems are
//! accumulated diagnostics, never an error return (ADR-0004 §5).
//!
//! # Diagnostics (the `MANI-…` namespace, ADR-0007 decision 2)
//!
//! `MANI-001` invalid TOML, `MANI-002` both sections, `MANI-003` neither
//! section, `MANI-005` unknown key (warning), `MANI-006` invalid package name,
//! `MANI-007` invalid import URL, `MANI-010` a `[lints]` entry that names no
//! lint or whose value is not a level (warning, ADR-0024 decision 11).
//! `MANI-004` (nested workspace) is defined in
//! the catalogue but emitted by the package loader, not here: a
//! manifest read in isolation cannot know it is a workspace member, so a valid
//! `[workspace]` manifest parses clean.
//!
//! # Spans and the [`FileId`]
//!
//! [`parse_manifest`] takes the [`FileId`] the caller interned for the manifest
//! path (via [`SourceMap::file_id`](crate::diag::SourceMap::file_id)) and stamps
//! it into every diagnostic [`Span`]. Byte ranges come from the `toml` crate:
//! [`toml::Spanned`] for the offending value (name, import URL) and the parse
//! error's own span for `MANI-001`; structural problems with no single
//! offending value (`MANI-002`, `MANI-003`) point at the relevant section or the
//! whole document.
//!
//! # Parsing strategy
//!
//! The text is deserialized three times: once into a typed shape with
//! [`toml::Spanned`] leaves (for the values and their spans), once into a
//! key-to-spanned-value map (to enumerate the keys the typed shape silently
//! drops, so unknown keys can warn), and once into the `[lints]` table with
//! spanned keys (so each MANI-010 can point at its key). All three read the
//! same valid TOML; the second cannot fail once the first has, and the third
//! fails only when `lints` is not a table, which is MANI-010.

use std::collections::BTreeMap;
use std::ops::Range;

use rowan::{TextRange, TextSize};
use serde::Deserialize;
use toml::Spanned;

use crate::diag::{DiagCode, Diagnostic, FileId, Severity, Span};
use crate::lint::{LintLevel, LintTable, lint_by_name};

/// A parsed `ridl.toml` manifest: its mode-specific [`ManifestKind`], its
/// `[imports]` table (logical package name to URL), the optional
/// `[defaults].timing` string, and its `[lints]` table, all shared by both
/// modes.
///
/// `defaults` holds the raw `[defaults]` timing strings (e.g.
/// `"[100ms..1000ms]"`), stored **unparsed**: `ridl-core` cannot depend on
/// `ridl-sem`, so the checker parses and validates them (MANI-009) — the
/// manifest layer only records the strings (ridl §9.1).
///
/// `lints` holds only the valid `[lints]` entries: a registered lint name
/// mapped to a level. Every other entry is MANI-010 and is dropped
/// (ADR-0002 §4, ADR-0024 decision 11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub kind: ManifestKind,
    pub imports: BTreeMap<String, String>,
    pub defaults: TimingDefaults,
    pub lints: LintTable,
}

/// The raw `[defaults]` timing strings of one manifest, or the merge of
/// several. Each key is stored unparsed and is `None` when no manifest sets it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct TimingDefaults {
    /// `[defaults].timing`, the default for signals and events.
    pub timing: Option<String>,
    /// `[defaults].command_timing`, the default for commands.
    pub command_timing: Option<String>,
    /// `[defaults].query_timing`, the default for queries.
    pub query_timing: Option<String>,
}

impl TimingDefaults {
    /// Per key: `self`'s value, else `fallback`'s.
    pub fn or(self, fallback: &TimingDefaults) -> TimingDefaults {
        TimingDefaults {
            timing: self.timing.or_else(|| fallback.timing.clone()),
            command_timing: self
                .command_timing
                .or_else(|| fallback.command_timing.clone()),
            query_timing: self.query_timing.or_else(|| fallback.query_timing.clone()),
        }
    }
}

/// The two mutually exclusive manifest modes (ADR-0002 §4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestKind {
    /// A standalone package distributed as a unit.
    Package { name: String, version: String },
    /// A coordinated set of packages developed together.
    Workspace { members: Vec<String> },
}

/// Parses and validates one `ridl.toml`. Content problems (both sections
/// present, neither present, unknown keys, an invalid package name, an invalid
/// import URL) are returned as accumulated [`Diagnostic`]s rather than an error;
/// only a structurally unusable manifest (invalid TOML, ambiguous or missing
/// mode) yields `None`. `file_id` is the id the caller interned for the manifest
/// path; every diagnostic span carries it.
pub fn parse_manifest(file_id: FileId, text: &str) -> (Option<Manifest>, Vec<Diagnostic>) {
    let mut diags = Vec::new();

    // A syntax (or schema) error means there is no usable manifest: MANI-001.
    let raw: RawManifest = match toml::from_str(text) {
        Ok(raw) => raw,
        Err(err) => {
            let range = err.span().unwrap_or(0..text.len());
            // The span already draws the caret at the location, so the message
            // carries the reason, not the location. A `toml` error renders as a
            // multi-line block whose first line is the location header
            // ("TOML parse error at line 2, column 5") and whose last line is
            // the description ("key with no value, expected `=`"); the last line
            // is the description-first text the house style wants (T6).
            let rendered = err.to_string();
            let message = rendered
                .lines()
                .last()
                .unwrap_or("invalid TOML")
                .trim()
                .to_string();
            diags.push(error(DiagCode::MANI_001, file_id, range, message));
            return (None, diags);
        }
    };

    // Exactly one mode section must be present (ADR-0002 §4).
    if raw.package.is_some() && raw.workspace.is_some() {
        let range = raw
            .workspace
            .as_ref()
            .map(Spanned::span)
            .unwrap_or(0..text.len());
        diags.push(error(
            DiagCode::MANI_002,
            file_id,
            range,
            "manifest declares both `[package]` and `[workspace]`; the two modes are mutually exclusive".to_string(),
        ));
        return (None, diags);
    }
    if raw.package.is_none() && raw.workspace.is_none() {
        diags.push(error(
            DiagCode::MANI_003,
            file_id,
            0..text.len(),
            "manifest declares neither `[package]` nor `[workspace]`".to_string(),
        ));
        return (None, diags);
    }

    check_unknown_keys(file_id, text, &mut diags);

    let imports = collect_imports(file_id, raw.imports, &mut diags);
    // The raw `[defaults]` strings, recorded verbatim; the checker parses
    // them and reports MANI-009 (ridl §9.1).
    let defaults = raw
        .defaults
        .map(|defaults| {
            let raw = defaults.into_inner();
            TimingDefaults {
                timing: raw.timing,
                command_timing: raw.command_timing,
                query_timing: raw.query_timing,
            }
        })
        .unwrap_or_default();
    let lints = collect_lints(file_id, text, &mut diags);

    let kind = if let Some(pkg) = raw.package {
        let section_span = pkg.span();
        let raw_pkg = pkg.into_inner();
        let (name, name_span) = match raw_pkg.name {
            Some(name) => (name.get_ref().clone(), name.span()),
            None => (String::new(), section_span),
        };
        if !is_valid_package_name(&name) {
            diags.push(error(
                DiagCode::MANI_006,
                file_id,
                name_span,
                format!(
                    "invalid package name `{name}`; expected lowercase dot-separated segments (e.g. `veh.common`)"
                ),
            ));
        }
        ManifestKind::Package {
            name,
            version: raw_pkg.version,
        }
    } else if let Some(ws) = raw.workspace {
        ManifestKind::Workspace {
            members: ws.into_inner().members,
        }
    } else {
        // Unreachable: exactly one section is present (checked above).
        return (None, diags);
    };

    (
        Some(Manifest {
            kind,
            imports,
            defaults,
            lints,
        }),
        diags,
    )
}

/// The raw manifest shape `toml` deserializes into. Every leaf a diagnostic may
/// point at is a [`toml::Spanned`] so its byte range is available; the mode
/// sections are spanned so a missing package name or a mode conflict can fall
/// back to the section's range.
#[derive(Deserialize)]
struct RawManifest {
    package: Option<Spanned<RawPackage>>,
    workspace: Option<Spanned<RawWorkspace>>,
    #[serde(default)]
    imports: BTreeMap<String, Spanned<String>>,
    defaults: Option<Spanned<RawDefaults>>,
}

#[derive(Deserialize)]
struct RawDefaults {
    timing: Option<String>,
    command_timing: Option<String>,
    query_timing: Option<String>,
}

#[derive(Deserialize)]
struct RawPackage {
    name: Option<Spanned<String>>,
    #[serde(default)]
    version: String,
}

#[derive(Deserialize)]
struct RawWorkspace {
    #[serde(default)]
    members: Vec<String>,
}

/// The `[lints]` table alone, read in a parse of its own so that each key
/// carries its span. `lints` is not a field of [`RawManifest`]: a typed field
/// there would make `lints = 1` fail the whole typed parse, which is MANI-001
/// with no manifest, where ADR-0024 decision 11 wants one MANI-010 and the rest
/// of the manifest.
#[derive(Deserialize)]
struct RawLints {
    #[serde(default)]
    lints: Option<BTreeMap<Spanned<String>, Spanned<toml::Value>>>,
}

/// Collects the `[lints]` table into a [`LintTable`] of registered lint names
/// and levels (ADR-0002 §4). Every entry whose key is not a lint name, or whose
/// value is not one of the four level strings, is MANI-010 on the key and is
/// dropped (ADR-0024 decision 11). A `lints` key that is not a table is one
/// MANI-010 on the value, and the table is empty.
fn collect_lints(file_id: FileId, text: &str, diags: &mut Vec<Diagnostic>) -> LintTable {
    let mut lints = LintTable::new();
    let raw: RawLints = match toml::from_str(text) {
        Ok(raw) => raw,
        Err(_) => {
            // The typed manifest parse accepted this text, and every key and
            // value of a table fits `RawLints`, so the only failure left is a
            // `lints` value that is not a table. Its span comes from the
            // untyped map (the one `check_unknown_keys` reads).
            let doc: BTreeMap<String, Spanned<toml::Value>> =
                toml::from_str(text).unwrap_or_default();
            let range = doc.get("lints").map(Spanned::span).unwrap_or(0..text.len());
            diags.push(warning(
                DiagCode::MANI_010,
                file_id,
                range,
                "`[lints]` must be a table".to_string(),
            ));
            return lints;
        }
    };
    for (key, value) in raw.lints.unwrap_or_default() {
        let name = key.get_ref();
        let Some(registered) = lint_by_name(name).and_then(|entry| entry.lint) else {
            diags.push(warning(
                DiagCode::MANI_010,
                file_id,
                key.span(),
                format!("unknown lint `{name}` in `[lints]`"),
            ));
            continue;
        };
        let Some(level) = value.get_ref().as_str().and_then(LintLevel::parse) else {
            diags.push(warning(
                DiagCode::MANI_010,
                file_id,
                key.span(),
                format!("`[lints].{name}` must be one of \"allow\", \"info\", \"warn\", \"deny\""),
            ));
            continue;
        };
        lints.insert(registered, level);
    }
    lints
}

/// Collects the `[imports]` table into the public `name -> URL` map, flagging
/// any value that is not a valid import URL (MANI-007). A flagged import is
/// still recorded verbatim so downstream resolution can report against it.
fn collect_imports(
    file_id: FileId,
    raw: BTreeMap<String, Spanned<String>>,
    diags: &mut Vec<Diagnostic>,
) -> BTreeMap<String, String> {
    let mut imports = BTreeMap::new();
    for (key, url) in raw {
        let value = url.get_ref().clone();
        if !is_valid_import_url(&value) {
            diags.push(error(
                DiagCode::MANI_007,
                file_id,
                url.span(),
                format!("invalid import URL `{value}` for `{key}`; expected an `http://` or `https://` URL with a host"),
            ));
        }
        imports.insert(key, value);
    }
    imports
}

/// Warns (MANI-005) on any key the manifest schema does not define. The typed
/// [`RawManifest`] silently drops unknown keys, so the text is re-read as a map
/// of spanned values to enumerate every key. Keys under `[imports]` are the
/// user's logical package names and are never "unknown".
fn check_unknown_keys(file_id: FileId, text: &str, diags: &mut Vec<Diagnostic>) {
    let doc: BTreeMap<String, Spanned<toml::Value>> = match toml::from_str(text) {
        Ok(doc) => doc,
        // Unreachable: the typed parse already accepted this text.
        Err(_) => return,
    };
    // The allowed-key lists below must stay in sync with the fields of
    // `RawPackage`, `RawWorkspace`, and `RawDefaults`: a field added there
    // without a matching entry here would wrongly warn as an unknown key.
    // Keys under `[lints]` are lint names; `collect_lints` checks them against
    // the registry (MANI-010), so they are never "unknown" here.
    for (key, value) in &doc {
        match key.as_str() {
            "package" => check_section_keys(file_id, "package", value, &["name", "version"], diags),
            "workspace" => check_section_keys(file_id, "workspace", value, &["members"], diags),
            "defaults" => check_section_keys(
                file_id,
                "defaults",
                value,
                &["timing", "command_timing", "query_timing"],
                diags,
            ),
            "imports" => {}
            "lints" => {}
            _ => diags.push(warning(
                DiagCode::MANI_005,
                file_id,
                value.span(),
                format!("unknown manifest key `{key}`"),
            )),
        }
    }
}

/// Warns (MANI-005) on any key in a mode section that is not in `allowed`. The
/// section value carries the only span available, so an unknown nested key
/// points at its section.
fn check_section_keys(
    file_id: FileId,
    section: &str,
    value: &Spanned<toml::Value>,
    allowed: &[&str],
    diags: &mut Vec<Diagnostic>,
) {
    let Some(table) = value.get_ref().as_table() else {
        return;
    };
    for key in table.keys() {
        if !allowed.contains(&key.as_str()) {
            diags.push(warning(
                DiagCode::MANI_005,
                file_id,
                value.span(),
                format!("unknown key `{key}` in `[{section}]`"),
            ));
        }
    }
}

/// A package name is one or more lowercase dot-separated segments, each an ASCII
/// lowercase letter followed by ASCII lowercase letters or digits (ADR-0002 §1).
/// The empty string (a missing name) is invalid.
fn is_valid_package_name(name: &str) -> bool {
    !name.is_empty() && name.split('.').all(is_valid_name_segment)
}

fn is_valid_name_segment(segment: &str) -> bool {
    let mut chars = segment.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

/// A deliberately minimal import-URL check: the value must use the `http` or
/// `https` scheme and name a non-empty host (the run up to the first `/`, `?`,
/// or `#`). Full URL, version-suffix, and registry validation is the fetch
/// layer's job; this only rejects values that are plainly not URLs.
fn is_valid_import_url(url: &str) -> bool {
    let Some(rest) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
    else {
        return false;
    };
    let host_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    !rest[..host_end].is_empty()
}

/// Builds an error [`Diagnostic`] with the given code, file, byte range, and
/// message. Manifest diagnostics carry no secondary labels or fix-its.
fn error(code: DiagCode, file: FileId, range: Range<usize>, message: String) -> Diagnostic {
    diagnostic(code, Severity::Error, file, range, message)
}

/// Builds a warning [`Diagnostic`] (used only for MANI-005 and MANI-010).
fn warning(code: DiagCode, file: FileId, range: Range<usize>, message: String) -> Diagnostic {
    diagnostic(code, Severity::Warning, file, range, message)
}

fn diagnostic(
    code: DiagCode,
    severity: Severity,
    file: FileId,
    range: Range<usize>,
    message: String,
) -> Diagnostic {
    Diagnostic {
        code,
        severity,
        message,
        primary: Span {
            file,
            range: to_text_range(range),
        },
        labels: Vec::new(),
        fixits: Vec::new(),
    }
}

/// Converts a `toml` byte range into a `rowan::TextRange`, the coordinate space
/// the diagnostic model works in.
fn to_text_range(range: Range<usize>) -> TextRange {
    TextRange::new(
        TextSize::from(range.start as u32),
        TextSize::from(range.end as u32),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::{MANI_CATALOG, SourceMap};

    const STANDALONE: &str = "\
[package]
name = \"veh.common\"
version = \"1.2.0\"

[imports]
\"some.dep\" = \"https://ridl.example.com/some/dep@v1.0.0\"
";

    const WORKSPACE: &str = "\
[workspace]
members = [\"veh-common\", \"veh-cluster\", \"veh-adas\"]

[imports]
\"third-party.foo\" = \"https://ridl.example.com/third-party/foo@v1.0.0\"
";

    fn parse(text: &str) -> (Option<Manifest>, Vec<Diagnostic>) {
        let mut map = SourceMap::new();
        let file_id = map.file_id("ridl.toml", text);
        parse_manifest(file_id, text)
    }

    fn codes(diags: &[Diagnostic]) -> Vec<&str> {
        diags.iter().map(|d| d.code.as_str()).collect()
    }

    #[test]
    fn adr0002_standalone_example_parses() {
        let (manifest, diags) = parse(STANDALONE);
        assert!(diags.is_empty(), "clean standalone manifest, got {diags:?}");
        let manifest = manifest.expect("standalone manifest parses");
        assert_eq!(
            manifest.kind,
            ManifestKind::Package {
                name: "veh.common".to_string(),
                version: "1.2.0".to_string(),
            },
        );
        assert_eq!(manifest.imports.len(), 1);
        assert_eq!(
            manifest.imports.get("some.dep").map(String::as_str),
            Some("https://ridl.example.com/some/dep@v1.0.0"),
        );
    }

    #[test]
    fn adr0002_workspace_example_parses() {
        let (manifest, diags) = parse(WORKSPACE);
        assert!(diags.is_empty(), "clean workspace manifest, got {diags:?}");
        let manifest = manifest.expect("workspace manifest parses");
        assert_eq!(
            manifest.kind,
            ManifestKind::Workspace {
                members: vec![
                    "veh-common".to_string(),
                    "veh-cluster".to_string(),
                    "veh-adas".to_string(),
                ],
            },
        );
        assert_eq!(
            manifest.imports.get("third-party.foo").map(String::as_str),
            Some("https://ridl.example.com/third-party/foo@v1.0.0"),
        );
    }

    #[test]
    fn mani_001_invalid_toml() {
        // `name` with no `=` value is a TOML syntax error.
        let (manifest, diags) = parse("[package]\nname\n");
        assert!(manifest.is_none(), "invalid TOML yields no manifest");
        assert_eq!(codes(&diags), vec!["MANI-001"]);
        assert_eq!(diags[0].severity, Severity::Error);
        // The message carries the reason (description-first, T6 house style),
        // not the "TOML parse error at line …" location header.
        assert!(
            diags[0].message.contains("expected `=`"),
            "MANI-001 message must carry the reason, got {:?}",
            diags[0].message,
        );
        assert!(
            !diags[0].message.contains("TOML parse error at line"),
            "MANI-001 message must not be the location header, got {:?}",
            diags[0].message,
        );
    }

    #[test]
    fn mani_001_carries_type_error_reason() {
        // A wrong-typed field value is a schema error; its reason must survive.
        let (manifest, diags) = parse("[package]\nname = 123\nversion = \"1.0.0\"\n");
        assert!(manifest.is_none());
        assert_eq!(codes(&diags), vec!["MANI-001"]);
        assert!(
            diags[0].message.contains("invalid type"),
            "MANI-001 message must carry the type-mismatch reason, got {:?}",
            diags[0].message,
        );
        assert!(!diags[0].message.contains("TOML parse error at line"));
    }

    #[test]
    fn mani_002_both_sections() {
        let text = "\
[package]
name = \"veh.common\"
version = \"1.0.0\"

[workspace]
members = [\"a\"]
";
        let (manifest, diags) = parse(text);
        assert!(manifest.is_none(), "an ambiguous mode yields no manifest");
        assert_eq!(codes(&diags), vec!["MANI-002"]);
        assert_eq!(diags[0].severity, Severity::Error);
    }

    #[test]
    fn mani_003_neither_section() {
        // Only `[imports]`, no mode section.
        let text = "[imports]\n\"x.y\" = \"https://example.com/x\"\n";
        let (manifest, diags) = parse(text);
        assert!(manifest.is_none(), "a modeless manifest yields no manifest");
        assert_eq!(codes(&diags), vec!["MANI-003"]);

        // An empty manifest is the same failure.
        let (empty, empty_diags) = parse("");
        assert!(empty.is_none());
        assert_eq!(codes(&empty_diags), vec!["MANI-003"]);
    }

    #[test]
    fn mani_004_workspace_manifest_parses_clean_in_isolation() {
        // Nested-workspace detection is the loader's job. Parsed
        // in isolation a `[workspace]` manifest is a valid workspace, never
        // MANI-004.
        let (manifest, diags) = parse(WORKSPACE);
        assert!(diags.is_empty(), "no MANI-004 from the standalone parser");
        assert!(matches!(
            manifest.expect("workspace parses").kind,
            ManifestKind::Workspace { .. },
        ));
        // The code exists in the catalogue for the loader to emit.
        assert!(
            MANI_CATALOG
                .iter()
                .any(|entry| entry.code == DiagCode::MANI_004),
            "MANI-004 is defined for the loader",
        );
    }

    #[test]
    fn mani_005_unknown_key_warns_but_parses() {
        // Unknown key inside `[package]`.
        let text = "\
[package]
name = \"veh.common\"
version = \"1.0.0\"
description = \"not a known key\"
";
        let (manifest, diags) = parse(text);
        let manifest = manifest.expect("unknown key still parses");
        assert_eq!(
            manifest.kind,
            ManifestKind::Package {
                name: "veh.common".to_string(),
                version: "1.0.0".to_string(),
            },
        );
        assert_eq!(codes(&diags), vec!["MANI-005"]);
        assert_eq!(diags[0].severity, Severity::Warning);

        // Unknown top-level section is also warned.
        let top = "\
[package]
name = \"veh.common\"
version = \"1.0.0\"

[bogus]
x = 1
";
        let (top_manifest, top_diags) = parse(top);
        assert!(top_manifest.is_some(), "unknown top-level key still parses");
        assert_eq!(codes(&top_diags), vec!["MANI-005"]);
        assert_eq!(top_diags[0].severity, Severity::Warning);
    }

    #[test]
    fn mani_006_invalid_package_name() {
        // Uppercase is not a lowercase dot-segment name.
        let text = "[package]\nname = \"Veh.Common\"\nversion = \"1.0.0\"\n";
        let (manifest, diags) = parse(text);
        assert!(manifest.is_some(), "a bad name is a diagnostic, not None");
        assert_eq!(codes(&diags), vec!["MANI-006"]);
        assert_eq!(diags[0].severity, Severity::Error);

        // A missing name is an empty name, equally invalid.
        let no_name = "[package]\nversion = \"1.0.0\"\n";
        let (_, no_name_diags) = parse(no_name);
        assert_eq!(codes(&no_name_diags), vec!["MANI-006"]);
    }

    #[test]
    fn defaults_timing_is_recorded_unparsed() {
        // The `[defaults].timing` string rides through verbatim — no MANI-005,
        // and the checker (not the manifest) validates it (ridl §9.1).
        let text = "\
[package]
name = \"veh.common\"
version = \"1.0.0\"

[defaults]
timing = \"[50ms..2s]\"
";
        let (manifest, diags) = parse(text);
        assert!(
            diags.is_empty(),
            "a `[defaults]` section is known, got {diags:?}"
        );
        let manifest = manifest.expect("the manifest parses");
        assert_eq!(manifest.defaults.timing.as_deref(), Some("[50ms..2s]"));
    }

    #[test]
    fn defaults_command_and_query_timing_parse() {
        let text = "\
[package]
name = \"veh.common\"
version = \"1.0.0\"

[defaults]
timing = \"[100ms..1s]\"
command_timing = \"[..1s]\"
query_timing = \"[..3s]\"
";
        let (manifest, diags) = parse(text);
        assert!(diags.is_empty(), "known keys draw nothing, got {diags:?}");
        assert_eq!(
            manifest.expect("parses").defaults,
            TimingDefaults {
                timing: Some("[100ms..1s]".to_string()),
                command_timing: Some("[..1s]".to_string()),
                query_timing: Some("[..3s]".to_string()),
            }
        );
    }

    #[test]
    fn no_defaults_section_leaves_the_defaults_absent() {
        let (manifest, diags) = parse(STANDALONE);
        assert!(diags.is_empty());
        assert_eq!(
            manifest.expect("parses").defaults,
            TimingDefaults::default(),
            "no `[defaults]` means no configured default",
        );
    }

    #[test]
    fn unknown_key_inside_defaults_still_warns() {
        let text = "\
[package]
name = \"veh.common\"
version = \"1.0.0\"

[defaults]
timing = \"[100ms..1000ms]\"
command_timing = \"[..1s]\"
query_timing = \"[..3s]\"
bogus = 1
";
        let (manifest, diags) = parse(text);
        assert!(manifest.is_some(), "an unknown nested key still parses");
        assert_eq!(codes(&diags), vec!["MANI-005"]);
        assert_eq!(diags[0].severity, Severity::Warning);
    }

    #[test]
    fn mani_007_invalid_import_url() {
        let text = "\
[package]
name = \"veh.common\"
version = \"1.0.0\"

[imports]
\"some.dep\" = \"not-a-url\"
";
        let (manifest, diags) = parse(text);
        let manifest = manifest.expect("a bad URL is a diagnostic, not None");
        // The import is still recorded, verbatim.
        assert_eq!(
            manifest.imports.get("some.dep").map(String::as_str),
            Some("not-a-url"),
        );
        assert_eq!(codes(&diags), vec!["MANI-007"]);
        assert_eq!(diags[0].severity, Severity::Error);

        // A plain `http://` host is accepted (no MANI-007).
        let http = "\
[package]
name = \"veh.common\"
version = \"1.0.0\"

[imports]
\"some.dep\" = \"http://example.com/some/dep\"
";
        let (_, http_diags) = parse(http);
        assert!(
            http_diags.is_empty(),
            "http:// with a host is a valid URL, got {http_diags:?}",
        );
    }

    const PACKAGE_HEAD: &str = "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n\n";
    const WORKSPACE_HEAD: &str = "[workspace]\nmembers = [\"a\"]\n\n";

    /// The text of `text` under the primary span of `diag`.
    fn spanned_text<'a>(text: &'a str, diag: &Diagnostic) -> &'a str {
        let range = diag.primary.range;
        &text[usize::from(range.start())..usize::from(range.end())]
    }

    #[test]
    fn lints_table_is_read() {
        for head in [PACKAGE_HEAD, WORKSPACE_HEAD] {
            let text = format!("{head}[lints]\nmissing-timing = \"deny\"\n");
            let (manifest, diags) = parse(&text);
            assert!(diags.is_empty(), "a valid `[lints]` table, got {diags:?}");
            let manifest = manifest.expect("the manifest parses");
            assert_eq!(manifest.lints.get("missing-timing"), Some(&LintLevel::Deny));
            assert_eq!(manifest.lints.len(), 1);
        }
    }

    #[test]
    fn lints_key_is_known() {
        let text = format!("{PACKAGE_HEAD}[lints]\n");
        let (manifest, diags) = parse(&text);
        assert!(diags.is_empty(), "no MANI-005 for `lints`, got {diags:?}");
        assert!(manifest.expect("the manifest parses").lints.is_empty());
    }

    #[test]
    fn no_lints_table_leaves_lints_empty() {
        let (manifest, diags) = parse(STANDALONE);
        assert!(diags.is_empty());
        assert!(manifest.expect("parses").lints.is_empty());
    }

    #[test]
    fn unknown_lint_name() {
        let text = format!("{PACKAGE_HEAD}[lints]\nnope = \"deny\"\n");
        let (manifest, diags) = parse(&text);
        assert_eq!(codes(&diags), vec!["MANI-010"]);
        assert_eq!(diags[0].severity, Severity::Warning);
        assert_eq!(diags[0].message, "unknown lint `nope` in `[lints]`");
        assert!(
            manifest.expect("the manifest parses").lints.is_empty(),
            "an unknown lint is not recorded",
        );
    }

    #[test]
    fn code_as_key() {
        let text = format!("{PACKAGE_HEAD}[lints]\n\"RIDL-100\" = \"deny\"\n");
        let (manifest, diags) = parse(&text);
        assert_eq!(codes(&diags), vec!["MANI-010"]);
        assert_eq!(diags[0].message, "unknown lint `RIDL-100` in `[lints]`");
        assert!(manifest.expect("the manifest parses").lints.is_empty());
    }

    #[test]
    fn error_code_as_key() {
        // An Error code has no lint name, so neither its code nor any name is
        // accepted as a key.
        let text = format!("{PACKAGE_HEAD}[lints]\n\"RIDL-101\" = \"allow\"\n");
        let (manifest, diags) = parse(&text);
        assert_eq!(codes(&diags), vec!["MANI-010"]);
        assert!(manifest.expect("the manifest parses").lints.is_empty());
    }

    #[test]
    fn bad_level() {
        for value in ["\"Deny\"", "3"] {
            let text = format!("{PACKAGE_HEAD}[lints]\nmissing-timing = {value}\n");
            let (manifest, diags) = parse(&text);
            assert_eq!(codes(&diags), vec!["MANI-010"], "value {value}");
            assert_eq!(diags[0].severity, Severity::Warning);
            assert_eq!(
                diags[0].message,
                "`[lints].missing-timing` must be one of \"allow\", \"info\", \"warn\", \"deny\"",
            );
            assert_eq!(spanned_text(&text, &diags[0]), "missing-timing");
            assert!(
                manifest.expect("the manifest parses").lints.is_empty(),
                "an entry with a bad level is not recorded",
            );
        }
    }

    #[test]
    fn valid_entries_survive_a_bad_sibling() {
        let text = format!("{PACKAGE_HEAD}[lints]\nmissing-timing = \"deny\"\nnope = \"warn\"\n");
        let (manifest, diags) = parse(&text);
        assert_eq!(codes(&diags), vec!["MANI-010"]);
        let manifest = manifest.expect("the manifest parses");
        assert_eq!(manifest.lints.get("missing-timing"), Some(&LintLevel::Deny));
        assert_eq!(manifest.lints.len(), 1);
    }

    #[test]
    fn lints_not_a_table() {
        // A top-level key must come before the first section header.
        let text = format!("lints = 1\n\n{PACKAGE_HEAD}");
        let (manifest, diags) = parse(&text);
        assert_eq!(codes(&diags), vec!["MANI-010"], "no MANI-001, no MANI-005");
        assert_eq!(diags[0].message, "`[lints]` must be a table");
        assert_eq!(spanned_text(&text, &diags[0]), "1");
        let manifest = manifest.expect("the rest of the manifest is still read");
        assert_eq!(
            manifest.kind,
            ManifestKind::Package {
                name: "veh.common".to_string(),
                version: "1.0.0".to_string(),
            },
        );
        assert!(manifest.lints.is_empty());
    }

    #[test]
    fn mani_010_span_is_the_key() {
        let text = format!("{PACKAGE_HEAD}[lints]\nnope = \"deny\"\n");
        let (_, diags) = parse(&text);
        assert_eq!(codes(&diags), vec!["MANI-010"]);
        assert_eq!(spanned_text(&text, &diags[0]), "nope");
    }
}
