//! The `ridl.toml` manifest parser (docs/ROADMAP.md epic E1.5, ADR-0002 §4).
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
//! `MANI-007` invalid import URL. `MANI-004` (nested workspace) is defined in
//! the catalogue but emitted by the package loader (E1.3, task 8), not here: a
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
//! The text is deserialized twice: once into a typed shape with [`toml::Spanned`]
//! leaves (for the values and their spans), and once into a key-to-spanned-value
//! map (to enumerate the keys the typed shape silently drops, so unknown keys
//! can warn). Both parses read the same valid TOML; the second cannot fail once
//! the first has.

use std::collections::BTreeMap;
use std::ops::Range;

use rowan::{TextRange, TextSize};
use serde::Deserialize;
use toml::Spanned;

use crate::diag::{DiagCode, Diagnostic, FileId, Severity, Span};

/// A parsed `ridl.toml` manifest: its mode-specific [`ManifestKind`], its
/// `[imports]` table (logical package name to URL), and the optional
/// `[defaults].timing` string, all shared by both modes.
///
/// `default_timing` is the raw `[defaults].timing` text (e.g.
/// `"[100ms..1000ms]"`), stored **unparsed**: `ridl-core` cannot depend on
/// `ridl-sem`, so the checker parses and validates it (MANI-009) — the
/// manifest layer only records the string (ridl §9.1, E2 task 9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub kind: ManifestKind,
    pub imports: BTreeMap<String, String>,
    pub default_timing: Option<String>,
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
    // The raw `[defaults].timing` string, recorded verbatim; the checker
    // parses it and reports MANI-009 (ridl §9.1, E2 task 9).
    let default_timing = raw
        .defaults
        .and_then(|defaults| defaults.into_inner().timing);

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
            default_timing,
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
    for (key, value) in &doc {
        match key.as_str() {
            "package" => check_section_keys(file_id, "package", value, &["name", "version"], diags),
            "workspace" => check_section_keys(file_id, "workspace", value, &["members"], diags),
            "defaults" => check_section_keys(file_id, "defaults", value, &["timing"], diags),
            "imports" => {}
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
/// layer's job (E1.6); this only rejects values that are plainly not URLs.
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

/// Builds a warning [`Diagnostic`] (used only for MANI-005).
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
        // Nested-workspace detection is the loader's job (E1.3, task 8). Parsed
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
        assert_eq!(manifest.default_timing.as_deref(), Some("[50ms..2s]"));
    }

    #[test]
    fn no_defaults_section_leaves_default_timing_absent() {
        let (manifest, diags) = parse(STANDALONE);
        assert!(diags.is_empty());
        assert_eq!(
            manifest.expect("parses").default_timing,
            None,
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
}
