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
/// `[imports]` table (logical package name to URL), its `[defaults]` as a
/// [`TimingDefaults`] — the three optional keys `timing`, `command_timing` and
/// `query_timing` — and its `[lints]` table, all shared by both modes.
///
/// `defaults` holds the raw `[defaults]` timing strings (e.g.
/// `"[100ms..1000ms]"` or `"[..1s]"`), stored **unparsed**: `ridl-core`
/// cannot depend on `ridl-sem`, so the checker parses and validates them
/// (MANI-009) — the manifest layer only records the strings (ridl §9.1,
/// §9.3).
///
/// `lints` holds only the valid `[lints]` entries: a registered lint name
/// mapped to a level. Every other entry is MANI-010 and is dropped
/// (ADR-0002 §4, ADR-0024 decision 11).
///
/// `codegen_header_file` is the raw `[codegen] header-file` value with the byte
/// range of that value in the manifest text. The loader resolves the path
/// against the manifest's directory and reads the file.
///
/// `backends` holds one [`BackendTable`] per `[backend.<name>]` table, keyed by
/// the backend's language name, with every `encodings` element that passed
/// validation; an invalid element is MANI-015 and is dropped. `backend_span` is
/// the byte range of the `backend` key's value in the manifest text, whatever
/// its shape, or `None` when the manifest has no `backend` key; the loader uses
/// it to raise MANI-016.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub kind: ManifestKind,
    pub imports: BTreeMap<String, String>,
    pub defaults: TimingDefaults,
    pub lints: LintTable,
    pub codegen_header_file: Option<(String, Range<usize>)>,
    pub backends: BTreeMap<String, BackendTable>,
    pub backend_span: Option<Range<usize>>,
}

/// The encoding names the toolchain knows, spelled as the encoding enum of
/// `ridl-rt` spells them. `[backend.<name>] encodings` accepts only these.
pub const ENCODING_NAMES: [&str; 3] = ["flatbuffers", "proto3", "repr-c"];

/// The key of a `[backend.<name>]` table that lists encodings, and the key of
/// the backend request option that carries the same list.
pub const ENCODINGS_KEY: &str = "encodings";

/// One `[backend.<name>]` table (ADR-0002 §4). The table is the same for every
/// backend name; whether a backend honours it is the backend's concern.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BackendTable {
    /// The validated `encodings` names in manifest order, each with the byte
    /// range of its string in the manifest text. Empty for an absent key and
    /// for `encodings = []`, both of which mean types only.
    pub encodings: Vec<(String, Range<usize>)>,
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
    Package {
        name: String,
        /// The byte range of the quoted `name` value, quotes included, or
        /// of the `[package]` table when `name` is absent.
        name_span: Range<usize>,
        version: String,
    },
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
    let (backends, backend_span) = collect_backends(file_id, text, &mut diags);
    let codegen_header_file = raw
        .codegen
        .and_then(|codegen| codegen.into_inner().header_file)
        .map(|value| (value.get_ref().clone(), value.span()));

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
                name_span.clone(),
                format!(
                    "invalid package name `{name}`; expected lowercase dot-separated segments (e.g. `veh.common`)"
                ),
            ));
        }
        ManifestKind::Package {
            name,
            name_span,
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
            codegen_header_file,
            backends,
            backend_span,
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
    codegen: Option<Spanned<RawCodegen>>,
}

#[derive(Deserialize)]
struct RawDefaults {
    timing: Option<String>,
    command_timing: Option<String>,
    query_timing: Option<String>,
}

#[derive(Deserialize)]
struct RawCodegen {
    #[serde(rename = "header-file")]
    header_file: Option<Spanned<String>>,
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

/// The `[backend]` table alone, read in a parse of its own so that each value
/// carries its span and a wrong shape is MANI-015 with the rest of the manifest
/// intact, not a failure of the typed manifest parse (as for [`RawLints`]).
#[derive(Deserialize)]
struct RawBackends {
    backend: Option<Spanned<Shape<RawBackendTables>>>,
}

/// The `[backend.<name>]` tables of a manifest, keyed by backend name.
type RawBackendTables = BTreeMap<String, Spanned<Shape<RawBackend>>>;

#[derive(Default)]
struct RawBackend {
    encodings: Option<Spanned<Shape<Vec<Spanned<toml::Value>>>>>,
}

/// A value that is either the table or array shape a reader expects (`Right`)
/// or any other shape (`Wrong`). Reading a wrong shape as `Wrong` keeps one
/// misplaced value from failing the whole typed parse.
enum Shape<T> {
    Right(T),
    Wrong,
}

/// Implements `Visitor` for [`Shape`] over one container type: `$visit` is
/// the visitor method that accepts the expected shape and `$body` builds the
/// value. Every other shape is drained and read as `Wrong`.
macro_rules! shape_visitor {
    ($ty:ty, $expecting:literal, |$access:ident| $body:expr, $visit:ident, $access_ty:ident, $other:ident, $other_ty:ident) => {
        impl<'de> Deserialize<'de> for Shape<$ty> {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                struct V;
                impl<'de> serde::de::Visitor<'de> for V {
                    type Value = Shape<$ty>;
                    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        f.write_str($expecting)
                    }
                    fn $visit<A: serde::de::$access_ty<'de>>(
                        self,
                        mut $access: A,
                    ) -> Result<Self::Value, A::Error> {
                        $body
                    }
                    fn $other<A: serde::de::$other_ty<'de>>(
                        self,
                        mut other: A,
                    ) -> Result<Self::Value, A::Error> {
                        drain_shape!($other, other);
                        Ok(Shape::Wrong)
                    }
                    fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E> {
                        Ok(Shape::Wrong)
                    }
                    fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E> {
                        Ok(Shape::Wrong)
                    }
                    fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E> {
                        Ok(Shape::Wrong)
                    }
                    fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E> {
                        Ok(Shape::Wrong)
                    }
                    fn visit_str<E>(self, _: &str) -> Result<Self::Value, E> {
                        Ok(Shape::Wrong)
                    }
                }
                deserializer.deserialize_any(V)
            }
        }
    };
}

/// The one key of the map the `toml` deserializer hands a visitor for a TOML
/// datetime. A datetime is not a table, so a table visitor that reads this key
/// reads a wrong shape. `toml_datetime` keeps the name private;
/// `backend_not_a_table_is_mani_015` fails if it changes.
const TOML_DATETIME_KEY: &str = "$__toml_private_datetime";

/// Drains the rest of a datetime's map and reads it as [`Shape::Wrong`].
fn datetime<'de, T, A: serde::de::MapAccess<'de>>(mut map: A) -> Result<Shape<T>, A::Error> {
    map.next_value::<serde::de::IgnoredAny>()?;
    while map
        .next_entry::<serde::de::IgnoredAny, serde::de::IgnoredAny>()?
        .is_some()
    {}
    Ok(Shape::Wrong)
}

macro_rules! drain_shape {
    (visit_seq, $access:ident) => {
        while $access.next_element::<serde::de::IgnoredAny>()?.is_some() {}
    };
    (visit_map, $access:ident) => {
        while $access
            .next_entry::<serde::de::IgnoredAny, serde::de::IgnoredAny>()?
            .is_some()
        {}
    };
}

shape_visitor!(
    BTreeMap<String, Spanned<Shape<RawBackend>>>,
    "a table",
    |map| {
        let mut tables = BTreeMap::new();
        while let Some(key) = map.next_key::<String>()? {
            if key == TOML_DATETIME_KEY {
                return datetime(map);
            }
            tables.insert(key, map.next_value::<Spanned<Shape<RawBackend>>>()?);
        }
        Ok(Shape::Right(tables))
    },
    visit_map,
    MapAccess,
    visit_seq,
    SeqAccess
);

shape_visitor!(
    RawBackend,
    "a table",
    |map| {
        let mut table = RawBackend::default();
        while let Some(key) = map.next_key::<String>()? {
            if key == TOML_DATETIME_KEY {
                return datetime(map);
            }
            if key == ENCODINGS_KEY {
                table.encodings = Some(map.next_value()?);
            } else {
                map.next_value::<serde::de::IgnoredAny>()?;
            }
        }
        Ok(Shape::Right(table))
    },
    visit_map,
    MapAccess,
    visit_seq,
    SeqAccess
);

shape_visitor!(
    Vec<Spanned<toml::Value>>,
    "an array",
    |seq| {
        let mut elements = Vec::new();
        while let Some(element) = seq.next_element()? {
            elements.push(element);
        }
        Ok(Shape::Right(elements))
    },
    visit_seq,
    SeqAccess,
    visit_map,
    MapAccess
);

/// Collects the `[backend.<name>]` tables (ADR-0002 §4). Returns the tables and
/// the span of the `backend` key's value, or `None` when the key is absent. A `backend` that is not a table, a `[backend.<name>]` that is not a
/// table, an `encodings` that is not an array, and an element that is not one
/// of [`ENCODING_NAMES`] or repeats an earlier one are each MANI-015 on the
/// offending value (the table's, for a wrong shape); an invalid element is
/// dropped and the rest are kept.
fn collect_backends(
    file_id: FileId,
    text: &str,
    diags: &mut Vec<Diagnostic>,
) -> (BTreeMap<String, BackendTable>, Option<Range<usize>>) {
    let mut backends = BTreeMap::new();
    // The typed manifest parse accepted this text and `Shape` accepts every
    // value, so this parse cannot fail; the fallback keeps the function total.
    let Ok(RawBackends {
        backend: Some(backend),
    }) = toml::from_str::<RawBackends>(text)
    else {
        return (backends, None);
    };
    let span = backend.span();
    let known = known_encodings();
    let Shape::Right(tables) = backend.into_inner() else {
        diags.push(error(
            DiagCode::MANI_015,
            file_id,
            span.clone(),
            format!("`backend` must be a table of `[backend.<name>]` tables; {known}"),
        ));
        return (backends, Some(span));
    };
    for (name, table) in tables {
        let table_span = table.span();
        let Shape::Right(table) = table.into_inner() else {
            diags.push(error(
                DiagCode::MANI_015,
                file_id,
                table_span,
                format!("`[backend.{name}]` must be a table; {known}"),
            ));
            continue;
        };
        let mut entry = BackendTable::default();
        match table.encodings {
            None => {}
            Some(encodings) => {
                let encodings_span = encodings.span();
                match encodings.into_inner() {
                    Shape::Wrong => diags.push(error(
                        DiagCode::MANI_015,
                        file_id,
                        encodings_span,
                        format!(
                            "`[backend.{name}] encodings` must be an array of encoding names; {known}"
                        ),
                    )),
                    Shape::Right(elements) => {
                        for element in elements {
                            let range = element.span();
                            let message = match element.get_ref().as_str() {
                                Some(value) if !ENCODING_NAMES.contains(&value) => Some(format!(
                                    "unknown encoding `{value}` in `[backend.{name}] encodings`; {known}"
                                )),
                                Some(value) if entry.encodings.iter().any(|(seen, _)| seen == value) => {
                                    Some(format!(
                                        "encoding `{value}` is named twice in `[backend.{name}] encodings`; {known}"
                                    ))
                                }
                                Some(value) => {
                                    entry.encodings.push((value.to_string(), range.clone()));
                                    None
                                }
                                None => Some(format!(
                                    "`[backend.{name}] encodings` holds `{}`, which is not a string; {known}",
                                    element.get_ref()
                                )),
                            };
                            if let Some(message) = message {
                                diags.push(error(DiagCode::MANI_015, file_id, range, message));
                            }
                        }
                    }
                }
            }
        }
        backends.insert(name, entry);
    }
    (backends, Some(span))
}

/// The suffix of every MANI-015 message about `[backend.<name>]`.
fn known_encodings() -> String {
    format!("known encodings: {}", ENCODING_NAMES.join(", "))
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
    // `KNOWN_KEYS` must stay in sync with the fields of `RawPackage`,
    // `RawWorkspace`, `RawDefaults`, `RawCodegen`, and `RawBackend`: a field
    // added there without a matching entry there would wrongly warn as an
    // unknown key. Keys under `[lints]` are lint names; `collect_lints` checks
    // them against the registry (MANI-010), so they are never "unknown" here.
    // Keys under `[imports]` are never "unknown" either.
    for (key, value) in &doc {
        if !known_keys("").contains(&key.as_str()) {
            diags.push(warning(
                DiagCode::MANI_005,
                file_id,
                value.span(),
                format!("unknown manifest key `{key}`"),
            ));
        } else if key == "backend" {
            // One sub-table per backend name; the name is any string, so
            // only the keys inside each sub-table are checked, each at
            // its own span.
            for (name, key, range) in backend_keys(text) {
                if !known_keys(BACKEND_SECTION).contains(&key.as_str()) {
                    diags.push(warning(
                        DiagCode::MANI_005,
                        file_id,
                        range,
                        format!("unknown key `{key}` in `[backend.{name}]`"),
                    ));
                }
            }
        } else if KNOWN_KEYS.iter().any(|(section, _)| section == key) {
            check_section_keys(file_id, key, value, &known_keys(key), diags);
        }
    }
}

/// The section name `KNOWN_KEYS` uses for every `[backend.<name>]` table: the
/// loader accepts any backend name, and the keys are the same for each.
pub const BACKEND_SECTION: &str = "backend.*";

/// The keys the manifest schema defines, as `(section, [(key, value type)])`.
/// The top level has the section name `""`. `[imports]` and `[lints]` hold
/// user-chosen names and are not listed as sections. MANI-005 warns on any key
/// outside these lists.
///
/// The value type is the TOML type the reader accepts for the key: `string`,
/// `table`, `array<string>` (an array of strings), `table<string>` (a table of
/// strings) or `table<table>` (a table of tables).
pub const KNOWN_KEYS: &[(&str, &[(&str, &str)])] = &[
    (
        "",
        &[
            ("package", "table"),
            ("workspace", "table"),
            ("defaults", "table"),
            ("codegen", "table"),
            ("backend", "table<table>"),
            ("imports", "table<string>"),
            ("lints", "table<string>"),
        ],
    ),
    ("package", &[("name", "string"), ("version", "string")]),
    ("workspace", &[("members", "array<string>")]),
    (
        "defaults",
        &[
            ("timing", "string"),
            ("command_timing", "string"),
            ("query_timing", "string"),
        ],
    ),
    ("codegen", &[("header-file", "string")]),
    (BACKEND_SECTION, &[(ENCODINGS_KEY, "array<string>")]),
];

/// The allowed keys of `section` in [`KNOWN_KEYS`]; empty for an unlisted one.
fn known_keys(section: &str) -> Vec<&'static str> {
    KNOWN_KEYS
        .iter()
        .find(|(name, _)| *name == section)
        .map_or(Vec::new(), |(_, keys)| {
            keys.iter().map(|(key, _)| *key).collect()
        })
}

/// The keys of every `[backend.<name>]` table, as `(name, key, span of the
/// key)`. Read from a span-carrying parse of its own, because the untyped map
/// [`check_unknown_keys`] reads carries the span of a value but not the span of
/// a key inside it. A `backend` or a `[backend.<name>]` that is not a table has
/// no keys here; [`collect_backends`] reports its shape.
fn backend_keys(text: &str) -> Vec<(String, String, Range<usize>)> {
    let Ok(doc) = toml::de::DeTable::parse(text) else {
        return Vec::new();
    };
    let mut keys = Vec::new();
    for (key, value) in doc.get_ref().iter() {
        let (true, toml::de::DeValue::Table(tables)) =
            (key.get_ref() == "backend", value.get_ref())
        else {
            continue;
        };
        for (name, table) in tables.iter() {
            let toml::de::DeValue::Table(table) = table.get_ref() else {
                continue;
            };
            for key in table.keys() {
                keys.push((
                    name.get_ref().to_string(),
                    key.get_ref().to_string(),
                    key.span(),
                ));
            }
        }
    }
    keys
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
    check_table_keys(
        file_id,
        section,
        value.get_ref(),
        value.span(),
        allowed,
        diags,
    );
}

/// [`check_section_keys`] over a plain value and an explicit span.
fn check_table_keys(
    file_id: FileId,
    section: &str,
    value: &toml::Value,
    span: Range<usize>,
    allowed: &[&str],
    diags: &mut Vec<Diagnostic>,
) {
    let Some(table) = value.as_table() else {
        return;
    };
    for key in table.keys() {
        if !allowed.contains(&key.as_str()) {
            diags.push(warning(
                DiagCode::MANI_005,
                file_id,
                span.clone(),
                format!("unknown key `{key}` in `[{section}]`"),
            ));
        }
    }
}

/// A package name is one or more lowercase dot-separated segments, each an ASCII
/// lowercase letter followed by ASCII lowercase letters or digits (ADR-0002 §1).
/// The empty string (a missing name) is invalid.
pub fn is_valid_package_name(name: &str) -> bool {
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
                name_span: range_of(STANDALONE, "\"veh.common\""),
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
                name_span: range_of(text, "\"veh.common\""),
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

    /// `or` resolves each key on its own, in both directions: a key `self`
    /// sets wins over the fallback's value, and a key `self` leaves unset takes
    /// the fallback's value.
    #[test]
    fn timing_defaults_or_resolves_each_key_on_its_own() {
        let all = |prefix: &str| TimingDefaults {
            timing: Some(format!("{prefix}-timing")),
            command_timing: Some(format!("{prefix}-command")),
            query_timing: Some(format!("{prefix}-query")),
        };
        // Every key set on both sides: `self` wins on every key.
        assert_eq!(all("own").or(&all("fallback")), all("own"));
        // No key set on `self`: every key comes from the fallback.
        assert_eq!(
            TimingDefaults::default().or(&all("fallback")),
            all("fallback")
        );
        // One key set on `self`: that key is its own, the other two are the
        // fallback's.
        let fallback = all("fallback");
        let own = || Some("own".to_string());
        let cases = [
            (
                TimingDefaults {
                    timing: own(),
                    ..TimingDefaults::default()
                },
                TimingDefaults {
                    timing: own(),
                    ..fallback.clone()
                },
            ),
            (
                TimingDefaults {
                    command_timing: own(),
                    ..TimingDefaults::default()
                },
                TimingDefaults {
                    command_timing: own(),
                    ..fallback.clone()
                },
            ),
            (
                TimingDefaults {
                    query_timing: own(),
                    ..TimingDefaults::default()
                },
                TimingDefaults {
                    query_timing: own(),
                    ..fallback.clone()
                },
            ),
        ];
        for (own, expected) in cases {
            assert_eq!(own.clone().or(&fallback), expected, "{own:?}");
        }
        // A key unset on both sides stays unset.
        assert_eq!(
            TimingDefaults::default().or(&TimingDefaults::default()),
            TimingDefaults::default()
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

    /// The byte range of the first occurrence of `needle` in `text`.
    fn range_of(text: &str, needle: &str) -> Range<usize> {
        let start = text.find(needle).expect("the needle occurs in the text");
        start..start + needle.len()
    }

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
                name_span: range_of(&text, "\"veh.common\""),
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

    #[test]
    fn codegen_header_file_is_parsed() {
        let text = format!("{PACKAGE_HEAD}[codegen]\nheader-file = \"H.txt\"\n");
        let (manifest, diags) = parse(&text);
        assert!(diags.is_empty(), "no diagnostic, got {diags:?}");
        let (path, range) = manifest
            .expect("the manifest parses")
            .codegen_header_file
            .expect("header-file is read");
        assert_eq!(path, "H.txt");
        assert_eq!(&text[range], "\"H.txt\"");
    }

    #[test]
    fn unknown_codegen_key_is_mani_005() {
        let text = format!("{PACKAGE_HEAD}[codegen]\nheader = \"x\"\n");
        let (_, diags) = parse(&text);
        assert_eq!(codes(&diags), vec!["MANI-005"]);
    }

    fn backend_names(manifest: &Manifest, backend: &str) -> Vec<String> {
        manifest.backends[backend]
            .encodings
            .iter()
            .map(|(name, _)| name.clone())
            .collect()
    }

    #[test]
    fn backend_encodings_parse_in_order() {
        let text =
            format!("{PACKAGE_HEAD}[backend.rust]\nencodings = [\"proto3\", \"flatbuffers\"]\n");
        let (manifest, diags) = parse(&text);
        assert!(diags.is_empty(), "{diags:?}");
        let manifest = manifest.expect("the manifest parses");
        assert_eq!(backend_names(&manifest, "rust"), ["proto3", "flatbuffers"]);
        let (name, span) = &manifest.backends["rust"].encodings[0];
        assert_eq!(&text[span.clone()], format!("\"{name}\""));
    }

    #[test]
    fn backend_encodings_empty_is_types_only() {
        let text = format!("{PACKAGE_HEAD}[backend.rust]\nencodings = []\n");
        let (manifest, diags) = parse(&text);
        assert!(diags.is_empty(), "{diags:?}");
        let manifest = manifest.expect("the manifest parses");
        assert!(manifest.backends["rust"].encodings.is_empty());
    }

    #[test]
    fn backend_table_for_any_name_is_accepted() {
        let text = format!("{PACKAGE_HEAD}[backend.kotlin]\n");
        let (manifest, diags) = parse(&text);
        assert!(diags.is_empty(), "{diags:?}");
        let manifest = manifest.expect("the manifest parses");
        assert!(manifest.backends["kotlin"].encodings.is_empty());
    }

    #[test]
    fn absent_backend_table_has_no_entry() {
        let (manifest, _) = parse(PACKAGE_HEAD);
        assert!(manifest.expect("the manifest parses").backends.is_empty());
    }

    #[test]
    fn unknown_encoding_is_mani_015() {
        let text = format!("{PACKAGE_HEAD}[backend.rust]\nencodings = [\"FlatBuffers\"]\n");
        let (manifest, diags) = parse(&text);
        assert_eq!(codes(&diags), vec!["MANI-015"]);
        let message = &diags[0].message;
        assert!(message.contains("FlatBuffers"), "{message}");
        assert!(message.contains("flatbuffers, proto3, repr-c"), "{message}");
        let start = usize::from(diags[0].primary.range.start());
        let end = usize::from(diags[0].primary.range.end());
        assert_eq!(&text[start..end], "\"FlatBuffers\"");
        assert!(
            manifest.expect("the manifest parses").backends["rust"]
                .encodings
                .is_empty()
        );
    }

    #[test]
    fn duplicate_encoding_is_mani_015() {
        let text = format!(
            "{PACKAGE_HEAD}[backend.rust]\nencodings = [\"flatbuffers\", \"flatbuffers\"]\n"
        );
        let (_, diags) = parse(&text);
        assert_eq!(codes(&diags), vec!["MANI-015"]);
        let start = usize::from(diags[0].primary.range.start());
        assert_eq!(start, text.rfind("\"flatbuffers\"").unwrap());
        let message = &diags[0].message;
        assert!(message.contains("is named twice"), "{message}");
        assert!(
            message.ends_with("; known encodings: flatbuffers, proto3, repr-c"),
            "{message}"
        );
    }

    /// An invalid element is dropped and the elements around it are kept, in
    /// their order.
    #[test]
    fn an_invalid_encoding_is_dropped_and_the_rest_are_kept() {
        let text = format!(
            "{PACKAGE_HEAD}[backend.rust]\nencodings = [\"proto3\", \"nope\", \"proto3\", 1, \"flatbuffers\"]\n"
        );
        let (manifest, diags) = parse(&text);
        assert_eq!(codes(&diags), vec!["MANI-015", "MANI-015", "MANI-015"]);
        let manifest = manifest.expect("the manifest parses");
        assert_eq!(backend_names(&manifest, "rust"), ["proto3", "flatbuffers"]);
    }

    #[test]
    fn empty_encoding_name_is_mani_015() {
        let text = format!("{PACKAGE_HEAD}[backend.rust]\nencodings = [\"\"]\n");
        let (_, diags) = parse(&text);
        assert_eq!(codes(&diags), vec!["MANI-015"]);
    }

    #[test]
    fn encoding_element_that_is_not_a_string_is_mani_015() {
        let text = format!("{PACKAGE_HEAD}[backend.rust]\nencodings = [1]\n");
        let (_, diags) = parse(&text);
        assert_eq!(codes(&diags), vec!["MANI-015"]);
        let message = &diags[0].message;
        assert!(
            message.starts_with("`[backend.rust] encodings` holds `1`, which is not a string;"),
            "{message}"
        );
    }

    #[test]
    fn encodings_not_an_array_is_mani_015() {
        let text = format!("{PACKAGE_HEAD}[backend.rust]\nencodings = \"flatbuffers\"\n");
        let (manifest, diags) = parse(&text);
        assert_eq!(codes(&diags), vec!["MANI-015"]);
        let message = &diags[0].message;
        assert!(
            message.starts_with("`[backend.rust] encodings` must be an array of encoding names;"),
            "{message}"
        );
        assert_eq!(span_text(&text, &diags[0]), "\"flatbuffers\"");
        assert!(manifest.is_some());
    }

    /// The source text a diagnostic's primary span covers.
    fn span_text<'t>(text: &'t str, diag: &Diagnostic) -> &'t str {
        let start = usize::from(diag.primary.range.start());
        let end = usize::from(diag.primary.range.end());
        &text[start..end]
    }

    #[test]
    fn backend_not_a_table_is_mani_015() {
        for (text, head, spanned) in [
            (
                format!("backend = 1\n\n{PACKAGE_HEAD}"),
                "`backend` must be a table of `[backend.<name>]` tables;",
                "1",
            ),
            (
                format!("{PACKAGE_HEAD}[backend]\nrust = 2\n"),
                "`[backend.rust]` must be a table;",
                "2",
            ),
            (
                format!("{PACKAGE_HEAD}[backend]\nrust = 1979-05-27T07:32:00Z\n"),
                "`[backend.rust]` must be a table;",
                "1979-05-27T07:32:00Z",
            ),
            (
                format!("backend = 1979-05-27T07:32:00Z\n\n{PACKAGE_HEAD}"),
                "`backend` must be a table of `[backend.<name>]` tables;",
                "1979-05-27T07:32:00Z",
            ),
        ] {
            let (manifest, diags) = parse(&text);
            assert_eq!(codes(&diags), vec!["MANI-015"], "{text}");
            let message = &diags[0].message;
            assert!(message.starts_with(head), "{text}: {message}");
            assert_eq!(span_text(&text, &diags[0]), spanned, "{text}");
            let manifest = manifest.expect("the manifest parses");
            assert!(manifest.backends.is_empty(), "{text}");
            assert!(manifest.backend_span.is_some(), "{text}");
        }
    }

    #[test]
    fn unknown_backend_key_is_mani_005() {
        let text = format!("{PACKAGE_HEAD}[backend.rust]\nfoo = 1\n\n[backend.kotlin]\nbar = 2\n");
        let (_, diags) = parse(&text);
        assert_eq!(codes(&diags), vec!["MANI-005", "MANI-005"]);
        // Each warning is on its own key, not on the whole `backend` table.
        let mut spanned: Vec<&str> = diags.iter().map(|diag| span_text(&text, diag)).collect();
        spanned.sort_unstable();
        assert_eq!(spanned, ["bar", "foo"]);
    }

    /// A manifest text that writes every key of `KNOWN_KEYS` once, with one
    /// table per section, so that the text is valid TOML.
    fn all_known_keys_text() -> String {
        let mut text = String::new();
        for key in known_keys("") {
            let section = if key == "backend" {
                BACKEND_SECTION
            } else {
                key
            };
            let header = if key == "backend" {
                "backend.rust"
            } else {
                key
            };
            text.push_str(&format!("[{header}]\n"));
            for key in known_keys(section) {
                text.push_str(&format!("{key} = 1\n"));
            }
        }
        text
    }

    /// The manifest of [`PACKAGE_HEAD`] with the key `key` of `section` set to
    /// the TOML value `value`. A key of `BACKEND_SECTION` is written in
    /// `[backend.rust]`.
    fn manifest_with(section: &str, key: &str, value: &str) -> String {
        let mut table: toml::Table = toml::from_str(PACKAGE_HEAD).unwrap();
        let value = toml::from_str::<toml::Table>(&format!("v = {value}")).unwrap()["v"].clone();
        let path: Vec<&str> = match section {
            "" => vec![],
            BACKEND_SECTION => vec!["backend", "rust"],
            other => vec![other],
        };
        let mut target = &mut table;
        for part in path {
            target = target
                .entry(part)
                .or_insert_with(|| toml::Value::Table(toml::Table::new()))
                .as_table_mut()
                .unwrap();
        }
        target.insert(key.to_string(), value);
        toml::to_string(&table).unwrap()
    }

    #[test]
    fn known_keys_drive_mani_005() {
        let mut map = SourceMap::new();
        let file_id = map.file_id("ridl.toml", "");
        let text = all_known_keys_text();
        // `check_unknown_keys` returns early on a text that is not TOML.
        assert!(toml::from_str::<toml::Table>(&text).is_ok(), "{text}");
        let mut diags = Vec::new();
        check_unknown_keys(file_id, &text, &mut diags);
        assert!(diags.is_empty(), "{diags:?}");

        // Each listed key is one the reader reads: an integer where a table,
        // a string or an array is expected draws a diagnostic other than
        // MANI-005. A key the reader ignores draws none.
        for (section, keys) in KNOWN_KEYS {
            for (key, _) in *keys {
                let text = manifest_with(section, key, "1");
                let (_, diags) = parse(&text);
                assert!(
                    diags.iter().any(|d| d.code.as_str() != "MANI-005"),
                    "`{section}` key `{key}` is in KNOWN_KEYS, but the reader ignores it:\n{text}"
                );
            }
        }

        let mut diags = Vec::new();
        check_unknown_keys(file_id, "[package]\nnmae = \"x\"\n", &mut diags);
        assert_eq!(codes(&diags), vec!["MANI-005"]);
    }

    /// Values of each type of `KNOWN_KEYS`, and values of other types. A plain
    /// `table` accepts a table that holds an integer, which each `table<T>` of
    /// the list rejects, so the two are told apart.
    fn type_samples(value_type: &str) -> (&'static [&'static str], &'static [&'static str]) {
        match value_type {
            "string" => (&["\"flatbuffers\""], &["1", "[]"]),
            "table" => (&["{}", "{ a = 1 }"], &["1", "\"x\""]),
            "array<string>" => (&["[\"flatbuffers\"]"], &["1", "[1]"]),
            "table<string>" => (
                &["{ missing-docs = \"warn\" }"],
                &["1", "{ missing-docs = 1 }"],
            ),
            "table<table>" => (&["{ rust = {} }"], &["1", "{ rust = 1 }"]),
            other => panic!("KNOWN_KEYS names the unknown value type `{other}`"),
        }
    }

    /// The codes a value of the wrong type draws: MANI-001 from the typed
    /// parse, MANI-010 from `[lints]` and MANI-015 from `[backend]`.
    const TYPE_CODES: [&str; 3] = ["MANI-001", "MANI-010", "MANI-015"];

    #[test]
    fn known_key_types_are_the_types_the_reader_accepts() {
        for (section, keys) in KNOWN_KEYS {
            for (key, value_type) in *keys {
                let (right, wrong) = type_samples(value_type);
                for value in right {
                    let text = manifest_with(section, key, value);
                    let (_, diags) = parse(&text);
                    assert!(
                        !codes(&diags).iter().any(|c| TYPE_CODES.contains(c)),
                        "`{section}` key `{key}` is `{value_type}`, but the reader rejects \
                         {value}: {diags:?}\n{text}"
                    );
                }
                for value in wrong {
                    let text = manifest_with(section, key, value);
                    let (_, diags) = parse(&text);
                    assert!(
                        codes(&diags).iter().any(|c| TYPE_CODES.contains(c)),
                        "`{section}` key `{key}` is `{value_type}`, but the reader accepts \
                         {value}\n{text}"
                    );
                }
            }
        }
    }
}
