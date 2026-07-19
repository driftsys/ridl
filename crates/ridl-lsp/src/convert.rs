//! Coordinate and diagnostic conversion between the compiler's world
//! (byte-offset `rowan::TextRange`, coded [`Diagnostic`]) and the LSP world
//! (UTF-16 line/character [`lsp_types::Position`], `lsp_types::Diagnostic`).
//!
//! LSP positions count UTF-16 code units (the protocol default encoding; the
//! server does not negotiate another one), while every compiler span is a byte
//! offset into UTF-8 text. [`LineIndex`] is the bridge: build one per file
//! text with [`line_index`], then convert offsets and positions both ways.

use std::collections::HashMap;
use std::str::FromStr;

use lsp_types as lt;
use ridl_core::diag::{Diagnostic, FileId, FixIt, Severity};
use rowan::{TextRange, TextSize};

/// A line table over one file's text: byte offset ↔ UTF-16 line/character.
pub struct LineIndex {
    text: String,
    /// Byte offset of the first character of each line, ascending; index 0 is
    /// always 0.
    line_starts: Vec<TextSize>,
}

/// Builds the [`LineIndex`] for `text`. Lines are separated by `\n` (a `\r\n`
/// separator leaves the `\r` at the end of the line, which never affects
/// column arithmetic before it).
pub fn line_index(text: &str) -> LineIndex {
    let mut line_starts = vec![TextSize::from(0)];
    for (offset, byte) in text.bytes().enumerate() {
        if byte == b'\n' {
            line_starts.push(TextSize::from(offset as u32 + 1));
        }
    }
    LineIndex {
        text: text.to_string(),
        line_starts,
    }
}

impl LineIndex {
    /// The LSP position of byte `offset`, in UTF-16 code units. An offset past
    /// the end of the text clamps to the end.
    pub fn position(&self, offset: TextSize) -> lt::Position {
        let offset = offset.min(TextSize::of(self.text.as_str()));
        let line = self.line_starts.partition_point(|start| *start <= offset) - 1;
        let start = self.line_starts[line];
        let character: usize = self.text[usize::from(start)..usize::from(offset)]
            .chars()
            .map(char::len_utf16)
            .sum();
        lt::Position {
            line: line as u32,
            character: character as u32,
        }
    }

    /// The byte offset of `position`. Per the LSP specification, a line past
    /// the last line clamps to the end of the text and a character past the
    /// end of its line clamps to the line end. A character landing inside a
    /// surrogate pair consumes the whole pair.
    pub fn offset(&self, position: lt::Position) -> TextSize {
        let Some(start) = self.line_starts.get(position.line as usize).copied() else {
            return TextSize::of(self.text.as_str());
        };
        let line_end = match self.line_starts.get(position.line as usize + 1) {
            // Exclude the `\n`, so a clamped character stays on its line.
            Some(next_start) => usize::from(*next_start) - 1,
            None => self.text.len(),
        };
        let mut units = 0u32;
        let mut offset = usize::from(start);
        for character in self.text[usize::from(start)..line_end].chars() {
            if units >= position.character {
                break;
            }
            units += character.len_utf16() as u32;
            offset += character.len_utf8();
        }
        TextSize::from(offset as u32)
    }

    /// The LSP range of a byte `range`.
    pub fn range(&self, range: TextRange) -> lt::Range {
        lt::Range {
            start: self.position(range.start()),
            end: self.position(range.end()),
        }
    }

    /// The byte range of an LSP `range`.
    pub fn text_range(&self, range: lt::Range) -> TextRange {
        TextRange::new(self.offset(range.start), self.offset(range.end))
    }
}

/// Maps a compiler [`Severity`] to the LSP severity.
pub fn severity(severity: Severity) -> lt::DiagnosticSeverity {
    match severity {
        Severity::Error => lt::DiagnosticSeverity::ERROR,
        Severity::Warning => lt::DiagnosticSeverity::WARNING,
        Severity::Info => lt::DiagnosticSeverity::INFORMATION,
    }
}

/// What a [`FileId`] resolves to: the file's URI plus its line table.
pub type FileInfo<'a> = (&'a lt::Uri, &'a LineIndex);

/// Converts one coded [`Diagnostic`] to an LSP diagnostic.
///
/// `resolve` maps a [`FileId`] to the file's URI and line table. Returns
/// `None` when the primary span's file does not resolve (a detached
/// diagnostic has no document to attach to). Labels whose file does not
/// resolve are dropped from the related information.
pub fn diagnostic<'a>(
    diag: &Diagnostic,
    resolve: impl Fn(FileId) -> Option<FileInfo<'a>>,
) -> Option<lt::Diagnostic> {
    let (_, lines) = resolve(diag.primary.file)?;
    let related: Vec<lt::DiagnosticRelatedInformation> = diag
        .labels
        .iter()
        .filter_map(|label| {
            let (uri, lines) = resolve(label.span.file)?;
            Some(lt::DiagnosticRelatedInformation {
                location: lt::Location {
                    uri: uri.clone(),
                    range: lines.range(label.span.range),
                },
                message: label.message.clone(),
            })
        })
        .collect();
    Some(lt::Diagnostic {
        range: lines.range(diag.primary.range),
        severity: Some(severity(diag.severity)),
        code: (!diag.code.is_empty())
            .then(|| lt::NumberOrString::String(diag.code.as_str().to_string())),
        code_description: None,
        source: Some("ridl".to_string()),
        message: diag.message.clone(),
        related_information: (!related.is_empty()).then_some(related),
        tags: None,
        data: None,
    })
}

/// Converts a diagnostic's fix-its to quick-fix code actions — one action per
/// [`FixIt`], titled by its label, editing the fix-it's own file. A fix-it
/// whose file does not resolve is dropped.
pub fn quick_fixes<'a>(
    diag: &lt::Diagnostic,
    fixits: &[FixIt],
    resolve: impl Fn(FileId) -> Option<FileInfo<'a>>,
) -> Vec<lt::CodeAction> {
    fixits
        .iter()
        .filter_map(|fixit| {
            let (uri, lines) = resolve(fixit.span.file)?;
            let edit = lt::TextEdit {
                range: lines.range(fixit.span.range),
                new_text: fixit.replacement.clone(),
            };
            // `WorkspaceEdit.changes` is keyed by `lsp_types::Uri`, whose
            // inner cache cell trips `mutable_key_type`; the key's identity
            // (the URI string) never mutates.
            #[allow(clippy::mutable_key_type)]
            let changes = HashMap::from([(uri.clone(), vec![edit])]);
            Some(lt::CodeAction {
                title: fixit.label.clone(),
                kind: Some(lt::CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![diag.clone()]),
                edit: Some(lt::WorkspaceEdit {
                    changes: Some(changes),
                    document_changes: None,
                    change_annotations: None,
                }),
                command: None,
                is_preferred: None,
                disabled: None,
                data: None,
            })
        })
        .collect()
}

/// The `file://` URI for an absolute filesystem path, percent-encoding every
/// byte outside the unreserved set and `/`. Returns `None` for a path that is
/// not absolute (a URI needs a rooted path).
pub fn path_to_uri(path: &str) -> Option<lt::Uri> {
    if !path.starts_with('/') {
        return None;
    }
    let mut encoded = String::with_capacity(path.len() + "file://".len());
    encoded.push_str("file://");
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    lt::Uri::from_str(&encoded).ok()
}

/// The filesystem path of a `file://` URI, percent-decoded. Returns `None`
/// for a non-`file` scheme or a path that does not decode to UTF-8.
pub fn uri_to_path(uri: &lt::Uri) -> Option<String> {
    if uri.scheme()?.as_str() != "file" {
        return None;
    }
    let encoded = uri.path().as_str();
    let mut bytes = Vec::with_capacity(encoded.len());
    let mut rest = encoded.bytes();
    while let Some(byte) = rest.next() {
        if byte == b'%' {
            let high = rest.next()?;
            let low = rest.next()?;
            let hex = [high, low];
            let hex = std::str::from_utf8(&hex).ok()?;
            bytes.push(u8::from_str_radix(hex, 16).ok()?);
        } else {
            bytes.push(byte);
        }
    }
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use ridl_core::diag::{DiagCode, Label, Span};

    use super::*;

    /// `ré🚀x\nab` — a multibyte first line: `r` (1 byte, 1 unit), `é`
    /// (2 bytes, 1 unit), `🚀` (4 bytes, 2 units), `x` (1 byte, 1 unit).
    const MULTIBYTE: &str = "r\u{e9}\u{1F680}x\nab";

    fn size(offset: u32) -> TextSize {
        TextSize::from(offset)
    }

    fn pos(line: u32, character: u32) -> lt::Position {
        lt::Position { line, character }
    }

    #[test]
    fn position_counts_utf16_units_on_a_multibyte_line() {
        let index = line_index(MULTIBYTE);
        // Byte offsets of the char boundaries: r=0, é=1, 🚀=3, x=7, \n=8.
        assert_eq!(index.position(size(0)), pos(0, 0));
        assert_eq!(index.position(size(1)), pos(0, 1), "after `r`");
        assert_eq!(index.position(size(3)), pos(0, 2), "after `é`: 1 unit");
        assert_eq!(index.position(size(7)), pos(0, 4), "after `🚀`: 2 units");
        assert_eq!(index.position(size(8)), pos(0, 5), "after `x`");
        assert_eq!(index.position(size(9)), pos(1, 0), "start of line 1");
        assert_eq!(index.position(size(11)), pos(1, 2), "end of text");
    }

    #[test]
    fn offset_walks_utf16_units_back_to_bytes() {
        let index = line_index(MULTIBYTE);
        assert_eq!(index.offset(pos(0, 0)), size(0));
        assert_eq!(index.offset(pos(0, 2)), size(3), "before `🚀`");
        assert_eq!(index.offset(pos(0, 4)), size(7), "after `🚀`");
        assert_eq!(index.offset(pos(1, 1)), size(10));
    }

    #[test]
    fn offset_and_position_round_trip_on_every_char_boundary() {
        let index = line_index(MULTIBYTE);
        for (offset, _) in MULTIBYTE.char_indices() {
            let offset = size(offset as u32);
            assert_eq!(
                index.offset(index.position(offset)),
                offset,
                "round trip at byte {offset:?}",
            );
        }
    }

    #[test]
    fn offset_clamps_per_the_lsp_specification() {
        let index = line_index(MULTIBYTE);
        // A character inside the surrogate pair consumes the whole pair.
        assert_eq!(index.offset(pos(0, 3)), size(7), "mid-pair rounds up");
        // A character past the line end clamps to the line end (before `\n`).
        assert_eq!(index.offset(pos(0, 99)), size(8));
        // A line past the last line clamps to the end of the text.
        assert_eq!(index.offset(pos(99, 0)), size(11));
        // An offset past the end of the text clamps to the end.
        assert_eq!(index.position(size(99)), pos(1, 2));
    }

    #[test]
    fn range_round_trips_across_a_multibyte_line() {
        let index = line_index(MULTIBYTE);
        let byte_range = TextRange::new(size(1), size(7)); // `é🚀`
        let lsp_range = index.range(byte_range);
        assert_eq!(lsp_range.start, pos(0, 1));
        assert_eq!(lsp_range.end, pos(0, 4));
        assert_eq!(index.text_range(lsp_range), byte_range);
    }

    #[test]
    fn severity_maps_error_warning_info() {
        assert_eq!(severity(Severity::Error), lt::DiagnosticSeverity::ERROR);
        assert_eq!(severity(Severity::Warning), lt::DiagnosticSeverity::WARNING);
        assert_eq!(
            severity(Severity::Info),
            lt::DiagnosticSeverity::INFORMATION
        );
    }

    #[test]
    fn path_and_uri_round_trip_with_percent_encoding() {
        let path = "/tmp/ridl space/caf\u{e9}.typl";
        let uri = path_to_uri(path).expect("an absolute path converts");
        assert_eq!(uri.as_str(), "file:///tmp/ridl%20space/caf%C3%A9.typl");
        assert_eq!(uri_to_path(&uri).as_deref(), Some(path));
        assert_eq!(path_to_uri("relative.typl"), None);
        let http = lt::Uri::from_str("http://example.com/a.typl").expect("a valid URI");
        assert_eq!(uri_to_path(&http), None, "non-file schemes do not convert");
    }

    /// A two-file conversion fixture: the diagnostic's primary span is in
    /// `a.typl`, its label points into `b.typl`.
    fn fixture() -> (Vec<(lt::Uri, LineIndex)>, Diagnostic) {
        let a_uri = path_to_uri("/ws/a.typl").expect("absolute");
        let b_uri = path_to_uri("/ws/b.typl").expect("absolute");
        let files = vec![
            (a_uri, line_index("type A: m\n")),
            (b_uri, line_index("type B: s\n")),
        ];
        let diag = Diagnostic {
            code: DiagCode::TYPL_009,
            severity: Severity::Error,
            message: "duplicate definition of `A`".to_string(),
            primary: Span {
                file: FileId::DETACHED, // patched by each test below
                range: TextRange::new(size(5), size(6)),
            },
            labels: vec![Label {
                span: Span {
                    file: FileId::DETACHED,
                    range: TextRange::new(size(5), size(6)),
                },
                message: "first defined here".to_string(),
            }],
            fixits: Vec::new(),
        };
        (files, diag)
    }

    /// A resolver over the fixture table: index 0 is `a.typl`, 1 is `b.typl`.
    /// The tests reuse a fresh `SourceMap` to mint real `FileId`s.
    fn ids() -> (FileId, FileId) {
        let mut sources = ridl_core::diag::SourceMap::new();
        let a = sources.file_id("/ws/a.typl", "type A: m\n");
        let b = sources.file_id("/ws/b.typl", "type B: s\n");
        (a, b)
    }

    #[test]
    fn diagnostic_carries_code_severity_range_and_related_info() {
        let (files, mut diag) = fixture();
        let (a, b) = ids();
        diag.primary.file = a;
        diag.labels[0].span.file = b;

        let resolve = |file: FileId| {
            let index = if file == a {
                0
            } else if file == b {
                1
            } else {
                return None;
            };
            Some((&files[index].0, &files[index].1))
        };
        let lsp = diagnostic(&diag, resolve).expect("the primary file resolves");
        assert_eq!(
            lsp.code,
            Some(lt::NumberOrString::String("TYPL-009".to_string()))
        );
        assert_eq!(lsp.severity, Some(lt::DiagnosticSeverity::ERROR));
        assert_eq!(lsp.message, "duplicate definition of `A`");
        assert_eq!(lsp.range.start, pos(0, 5));
        assert_eq!(lsp.range.end, pos(0, 6));
        assert_eq!(lsp.source.as_deref(), Some("ridl"));
        let related = lsp.related_information.expect("the label converts");
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].message, "first defined here");
        assert_eq!(related[0].location.uri.as_str(), files[1].0.as_str());

        // A detached primary span has no document to attach to.
        diag.primary.file = FileId::DETACHED;
        assert_eq!(diagnostic(&diag, resolve), None);
    }

    #[test]
    fn a_synthetic_fixit_becomes_a_quick_fix_code_action() {
        let (files, mut diag) = fixture();
        let (a, b) = ids();
        diag.primary.file = a;
        diag.labels.clear();
        diag.fixits = vec![FixIt {
            span: Span {
                file: a,
                range: TextRange::new(size(5), size(6)),
            },
            replacement: "A2".to_string(),
            label: "rename the duplicate to `A2`".to_string(),
        }];

        let resolve = |file: FileId| {
            let index = if file == a {
                0
            } else if file == b {
                1
            } else {
                return None;
            };
            Some((&files[index].0, &files[index].1))
        };
        let lsp = diagnostic(&diag, resolve).expect("the primary file resolves");
        let actions = quick_fixes(&lsp, &diag.fixits, resolve);
        assert_eq!(actions.len(), 1);
        let action = &actions[0];
        assert_eq!(action.title, "rename the duplicate to `A2`");
        assert_eq!(action.kind, Some(lt::CodeActionKind::QUICKFIX));
        assert_eq!(
            action.diagnostics.as_deref(),
            Some(std::slice::from_ref(&lsp)),
            "the action cites the diagnostic it fixes",
        );
        let edit = action.edit.as_ref().expect("the action carries the edit");
        // Same `mutable_key_type` false positive as in `quick_fixes`: the
        // `Uri` key's identity never mutates.
        #[allow(clippy::mutable_key_type)]
        let changes = edit.changes.as_ref().expect("changes keyed by URI");
        let edits = &changes[&files[0].0];
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].new_text, "A2");
        assert_eq!(edits[0].range.start, pos(0, 5));
        assert_eq!(edits[0].range.end, pos(0, 6));
    }
}
