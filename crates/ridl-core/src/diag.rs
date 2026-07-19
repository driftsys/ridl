//! The coded diagnostic model every compiler pass emits (docs/ROADMAP.md epic
//! E1.10, ADR-0004 §5, ADR-0007 decision 2).
//!
//! A [`Diagnostic`] is a first-class homegrown value — a stable [`DiagCode`], a
//! [`Severity`], a message, a primary source [`Span`], secondary [`Label`]s, and
//! optional [`FixIt`]s — held as the single source of truth and accumulated in a
//! `Vec`, never modeled as an error return (ADR-0004 §5). The struct maps two
//! ways: to a terminal renderer for the CLI ([`render`](render::render), over
//! `codespan-reporting`) and, in a later epic, to LSP `Diagnostic` for editors.
//!
//! # Namespaces (ADR-0007 decision 2)
//!
//! Codes are grouped by hundreds and never renumbered or reused. Three
//! namespaces are in play across the family:
//!
//! - `TYPL-…` — typl semantic rules, defined by the typl reference §16.
//! - `FORM-…` — the shared family grammar: lexical `0xx`, parse `1xx`. Named
//!   after the general form's own "shared form namespace". This module is the
//!   SSOT the error index (E4.2) will read; the full FORM catalogue is defined
//!   here as [`FORM_CATALOG`] even for codes no pass emits yet.
//! - `MANI-…` — manifest, lockfile, cache, and fetch. The manifest `0xx` codes
//!   are defined here as [`MANI_CATALOG`] (E1.5); the distribution `1xx` codes
//!   (lockfile, cache, fetch) arrive with E1.6.
//!
//! # The [`FileId`] bridge
//!
//! A [`Span`] locates its range inside a file identified by an interned
//! [`FileId`]. [`SourceMap`] issues those ids: a pass that holds a file's path
//! and text calls [`SourceMap::file_id`] to obtain the id it stamps into its
//! spans. The [`SourceMap`] is the only bridge between a diagnostic and the
//! source text the renderer needs.

use rowan::TextRange;
use serde::{Serialize, Serializer};

pub mod render;

pub use render::render;

/// A stable diagnostic code, e.g. `"FORM-101"` or `"TYPL-108"` (ADR-0007
/// decision 2). The empty string means "no code yet" — a diagnostic that has
/// not been assigned a catalogue code renders as a plain message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct DiagCode(pub &'static str);

impl DiagCode {
    /// The code string.
    pub fn as_str(&self) -> &'static str {
        self.0
    }

    /// Whether this is the sentinel "no code yet" value.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The sentinel for a diagnostic that carries no catalogue code yet. The
    /// unknown-type-name check uses it until a later task rehomes it (typl §16
    /// defines no code for it); the Rust backend's codegen error uses it too.
    pub const NONE: DiagCode = DiagCode("");

    // --- FORM lexical (0xx) ---
    /// Invalid character.
    pub const FORM_001: DiagCode = DiagCode("FORM-001");
    /// Unterminated string literal.
    pub const FORM_002: DiagCode = DiagCode("FORM-002");
    /// Unterminated regex literal.
    pub const FORM_003: DiagCode = DiagCode("FORM-003");
    /// Unterminated block comment.
    pub const FORM_004: DiagCode = DiagCode("FORM-004");
    /// Leading zeros in an integer literal.
    pub const FORM_005: DiagCode = DiagCode("FORM-005");

    // --- FORM parse (1xx) ---
    /// Expected a specific token.
    pub const FORM_101: DiagCode = DiagCode("FORM-101");
    /// Unexpected token.
    pub const FORM_102: DiagCode = DiagCode("FORM-102");
    /// Unclosed delimiter.
    pub const FORM_103: DiagCode = DiagCode("FORM-103");
    /// Missing `package` declaration.
    pub const FORM_104: DiagCode = DiagCode("FORM-104");
    /// Reserved word used as an identifier.
    pub const FORM_105: DiagCode = DiagCode("FORM-105");

    // --- typl codes emitted in E1 so far (typl reference §16) ---
    /// More than one `package` declaration in a single file (typl §16.1).
    /// Emitted by the package loader (E1.3).
    pub const TYPL_001: DiagCode = DiagCode("TYPL-001");
    /// Package name does not mirror the directory path relative to the
    /// manifest root (typl §16.1, ADR-0002 §1). Emitted by the package loader
    /// (E1.3); single-file mode is exempt.
    pub const TYPL_002: DiagCode = DiagCode("TYPL-002");
    /// Duplicate definition of the same name in a package (typl §16.1).
    pub const TYPL_009: DiagCode = DiagCode("TYPL-009");
    /// `const` value violates its declared type constraints (typl §16.2).
    pub const TYPL_108: DiagCode = DiagCode("TYPL-108");
    /// Timing annotation or duration literal in a typl context (typl §16.4).
    pub const TYPL_302: DiagCode = DiagCode("TYPL-302");

    // --- MANI manifest (0xx) — ADR-0007 decision 2, ADR-0002 §4 ---
    /// The `ridl.toml` text is not valid TOML.
    pub const MANI_001: DiagCode = DiagCode("MANI-001");
    /// The manifest declares both `[package]` and `[workspace]`; the two modes
    /// are mutually exclusive (ADR-0002 §4).
    pub const MANI_002: DiagCode = DiagCode("MANI-002");
    /// The manifest declares neither `[package]` nor `[workspace]` (ADR-0002 §4).
    pub const MANI_003: DiagCode = DiagCode("MANI-003");
    /// A workspace member's own manifest declares `[workspace]`; nested
    /// workspaces are forbidden (ADR-0002 §4). Defined here, but emitted by the
    /// package loader (E1.3, task 8) when a member manifest is read — a single
    /// manifest parsed in isolation cannot know it is a member.
    pub const MANI_004: DiagCode = DiagCode("MANI-004");
    /// An unrecognized key in the manifest or one of its sections (warning).
    pub const MANI_005: DiagCode = DiagCode("MANI-005");
    /// The package name is not lowercase dot-separated segments (ADR-0002 §1).
    pub const MANI_006: DiagCode = DiagCode("MANI-006");
    /// An `[imports]` value is not a valid import URL.
    pub const MANI_007: DiagCode = DiagCode("MANI-007");
}

/// A diagnostic's severity. Warnings and info diagnostics arrive with later
/// passes; every code the E1.10 pipeline emits is an [`Error`](Severity::Error).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// An interned file id, issued by [`SourceMap::file_id`]. It indexes the file's
/// path and text inside the [`SourceMap`], which the renderer reads to draw the
/// source snippet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct FileId(u32);

/// A source location: a byte range inside a specific file. The range is a
/// `rowan::TextRange` — the same coordinate space parse and semantic passes work
/// in — so an offset never has to be translated on the way into a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Span {
    pub file: FileId,
    #[serde(serialize_with = "serialize_text_range")]
    pub range: TextRange,
}

/// Serializes a `rowan::TextRange` as `{ "start": u32, "end": u32 }`, keeping
/// the byte offsets readable and exact in JSON snapshots.
fn serialize_text_range<S: Serializer>(
    range: &TextRange,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeStruct;
    let mut state = serializer.serialize_struct("TextRange", 2)?;
    state.serialize_field("start", &u32::from(range.start()))?;
    state.serialize_field("end", &u32::from(range.end()))?;
    state.end()
}

/// A secondary annotation: a span with a message, drawn under the source
/// alongside the primary span.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Label {
    pub span: Span,
    pub message: String,
}

/// A suggested edit: replace the text at `span` with `replacement`. `label`
/// describes the fix for a human. Rendered as a note under the diagnostic; a
/// later LSP task maps it to a code action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FixIt {
    pub span: Span,
    pub replacement: String,
    pub label: String,
}

/// One coded diagnostic. `primary` is the main span the message points at;
/// `labels` are secondary annotations; `fixits` are suggested edits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    pub code: DiagCode,
    pub severity: Severity,
    pub message: String,
    pub primary: Span,
    pub labels: Vec<Label>,
    pub fixits: Vec<FixIt>,
}

/// The path and text of one interned file.
#[derive(Debug)]
struct SourceEntry {
    path: String,
    text: String,
}

/// The file table the renderer reads: it maps every [`FileId`] to the path and
/// text a diagnostic's spans point into. Ids are interned by path, so asking for
/// the same path twice returns the same id.
#[derive(Debug, Default)]
pub struct SourceMap {
    files: Vec<SourceEntry>,
}

impl SourceMap {
    /// An empty source map.
    pub fn new() -> Self {
        Self::default()
    }

    /// The [`FileId`] for `path`, interning `path` and `text` on first sight.
    /// A pass holding a file's path and text calls this to stamp its spans.
    pub fn file_id(&mut self, path: &str, text: &str) -> FileId {
        if let Some(index) = self.files.iter().position(|entry| entry.path == path) {
            return FileId(index as u32);
        }
        let id = FileId(self.files.len() as u32);
        self.files.push(SourceEntry {
            path: path.to_string(),
            text: text.to_string(),
        });
        id
    }
}

/// One row of a diagnostic catalogue: a code, its default severity, and a short
/// human summary. The catalogue is the static SSOT the error index (E4.2) reads;
/// the per-diagnostic [`Severity`] a pass emits is set independently.
#[derive(Debug, Clone, Copy)]
pub struct CatalogEntry {
    pub code: DiagCode,
    pub severity: Severity,
    pub summary: &'static str,
}

/// The full FORM catalogue (ADR-0007 decision 2): lexical `0xx` and parse `1xx`.
/// Every FORM code is listed even when no pass emits it yet, so the error index
/// has one authoritative source. FORM diagnostics are all errors.
pub const FORM_CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        code: DiagCode::FORM_001,
        severity: Severity::Error,
        summary: "invalid character",
    },
    CatalogEntry {
        code: DiagCode::FORM_002,
        severity: Severity::Error,
        summary: "unterminated string literal",
    },
    CatalogEntry {
        code: DiagCode::FORM_003,
        severity: Severity::Error,
        summary: "unterminated regex literal",
    },
    CatalogEntry {
        code: DiagCode::FORM_004,
        severity: Severity::Error,
        summary: "unterminated block comment",
    },
    CatalogEntry {
        code: DiagCode::FORM_005,
        severity: Severity::Error,
        summary: "leading zeros in integer literal",
    },
    CatalogEntry {
        code: DiagCode::FORM_101,
        severity: Severity::Error,
        summary: "expected a specific token",
    },
    CatalogEntry {
        code: DiagCode::FORM_102,
        severity: Severity::Error,
        summary: "unexpected token",
    },
    CatalogEntry {
        code: DiagCode::FORM_103,
        severity: Severity::Error,
        summary: "unclosed delimiter",
    },
    CatalogEntry {
        code: DiagCode::FORM_104,
        severity: Severity::Error,
        summary: "missing `package` declaration",
    },
    CatalogEntry {
        code: DiagCode::FORM_105,
        severity: Severity::Error,
        summary: "reserved word used as an identifier",
    },
];

/// The manifest MANI catalogue (ADR-0007 decision 2): the manifest `0xx` codes
/// the `ridl.toml` parser (E1.5) and the package loader (E1.3) emit. Listed here
/// even for `MANI-004`, whose emission site is the loader rather than the
/// standalone parser, so the error index (E4.2) has one authoritative source.
/// The distribution `1xx` codes (lockfile, cache, fetch) arrive with E1.6.
pub const MANI_CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        code: DiagCode::MANI_001,
        severity: Severity::Error,
        summary: "invalid manifest TOML",
    },
    CatalogEntry {
        code: DiagCode::MANI_002,
        severity: Severity::Error,
        summary: "manifest declares both `[package]` and `[workspace]`",
    },
    CatalogEntry {
        code: DiagCode::MANI_003,
        severity: Severity::Error,
        summary: "manifest declares neither `[package]` nor `[workspace]`",
    },
    CatalogEntry {
        code: DiagCode::MANI_004,
        severity: Severity::Error,
        summary: "nested workspace: a member manifest declares `[workspace]`",
    },
    CatalogEntry {
        code: DiagCode::MANI_005,
        severity: Severity::Warning,
        summary: "unknown manifest key",
    },
    CatalogEntry {
        code: DiagCode::MANI_006,
        severity: Severity::Error,
        summary: "invalid package name",
    },
    CatalogEntry {
        code: DiagCode::MANI_007,
        severity: Severity::Error,
        summary: "invalid import URL",
    },
];

/// Polishes a raw parser message into the house diagnostic style —
/// description-first, with backticked names (fixes issue #102). Most parser
/// messages are already house-style and pass through unchanged; the parser's
/// `expect` path emits a `Debug`-shaped token name (`expected RBracket`), which
/// this maps to a backticked glyph (`` expected `]` ``).
///
/// Keeping the raw parser message as the input leaves `ridl-syntax` and its
/// tests untouched — they assert on codes and ranges, not message text — while
/// the rendered diagnostics still read in one consistent style.
pub fn house_style_message(raw: &str) -> String {
    if let Some(token) = raw.strip_prefix("expected ")
        && let Some(glyph) = punctuation_glyph(token)
    {
        return format!("expected {glyph}");
    }
    raw.to_string()
}

/// The backticked glyph for a punctuation `SyntaxKind` `Debug` name, or `None`
/// when the name is not a known punctuation token. Covers every punctuation kind
/// the parser can name in a FORM-101 `expected` message, so the mapping stays
/// correct if new `expect` call sites are added.
fn punctuation_glyph(debug_name: &str) -> Option<&'static str> {
    Some(match debug_name {
        "Colon" => "`:`",
        "Eq" => "`=`",
        "Semicolon" => "`;`",
        "Comma" => "`,`",
        "LBracket" => "`[`",
        "RBracket" => "`]`",
        "LBrace" => "`{`",
        "RBrace" => "`}`",
        "LParen" => "`(`",
        "RParen" => "`)`",
        "DotDot" => "`..`",
        "Dot" => "`.`",
        "Question" => "`?`",
        "At" => "`@`",
        "Pipe" => "`|`",
        _ => return None,
    })
}

impl SourceMap {
    /// The interned files, in id order — the renderer replays them into a
    /// `codespan_reporting` file table so `FileId(i)` lines up with codespan id
    /// `i`.
    pub(crate) fn iter_files(&self) -> impl Iterator<Item = (&str, &str)> {
        self.files
            .iter()
            .map(|entry| (entry.path.as_str(), entry.text.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_map_interns_by_path() {
        let mut map = SourceMap::new();
        let a = map.file_id("a.typl", "type A: m");
        let b = map.file_id("b.typl", "type B: s");
        let a_again = map.file_id("a.typl", "type A: m");
        assert_ne!(a, b, "distinct paths get distinct ids");
        assert_eq!(a, a_again, "the same path interns to the same id");
    }

    #[test]
    fn house_style_rewrites_debug_token_names() {
        assert_eq!(house_style_message("expected RBracket"), "expected `]`");
        assert_eq!(house_style_message("expected Colon"), "expected `:`");
        assert_eq!(house_style_message("expected Eq"), "expected `=`");
        assert_eq!(house_style_message("expected Semicolon"), "expected `;`");
    }

    #[test]
    fn house_style_passes_through_already_styled_messages() {
        // Already description-first with backticks — unchanged.
        assert_eq!(house_style_message("expected a name"), "expected a name");
        assert_eq!(house_style_message("unclosed `{`"), "unclosed `{`");
        assert_eq!(
            house_style_message("missing `package` declaration"),
            "missing `package` declaration",
        );
    }

    #[test]
    fn form_catalog_is_complete_and_ordered() {
        let codes: Vec<&str> = FORM_CATALOG
            .iter()
            .map(|entry| entry.code.as_str())
            .collect();
        assert_eq!(
            codes,
            vec![
                "FORM-001", "FORM-002", "FORM-003", "FORM-004", "FORM-005", "FORM-101", "FORM-102",
                "FORM-103", "FORM-104", "FORM-105",
            ],
        );
        assert!(
            FORM_CATALOG
                .iter()
                .all(|entry| entry.severity == Severity::Error),
            "every FORM code is an error",
        );
    }

    #[test]
    fn mani_catalog_is_complete_and_ordered() {
        let codes: Vec<&str> = MANI_CATALOG
            .iter()
            .map(|entry| entry.code.as_str())
            .collect();
        assert_eq!(
            codes,
            vec![
                "MANI-001", "MANI-002", "MANI-003", "MANI-004", "MANI-005", "MANI-006", "MANI-007",
            ],
        );
        // Every MANI code is an error except the unknown-key warning (MANI-005).
        for entry in MANI_CATALOG {
            let expected = if entry.code == DiagCode::MANI_005 {
                Severity::Warning
            } else {
                Severity::Error
            };
            assert_eq!(
                entry.severity,
                expected,
                "unexpected severity for {}",
                entry.code.as_str(),
            );
        }
    }
}
