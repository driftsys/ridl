//! Terminal rendering of [`Diagnostic`]s via `codespan-reporting` (ADR-0004 §5).
//!
//! The renderer is a pure function over the homegrown [`Diagnostic`] model and a
//! [`SourceMap`] — the model stays the single source of truth, and this layer
//! only draws it. [`render`] returns a `String` (colour off) so the CLI can
//! print it and tests can snapshot it byte for byte.
//!
//! Each diagnostic is emitted as its own block. Two diagnostics that point at
//! the same offset — for example a positional `FORM-101` and the profile-boundary
//! `TYPL-302` a duration literal raises at the same token — render as two clean
//! blocks with their own carets; there is no shared-label overlap to trip on. A
//! span that runs across a line break (an honest error range that reaches the
//! next declaration's keyword) renders as a multi-line underline rather than
//! panicking.

use std::ops::Range;

use codespan_reporting::diagnostic as cs;
use codespan_reporting::files::{Error as FilesError, Files};
use codespan_reporting::term::{self, Config};

use super::{Diagnostic, Severity, SourceMap, Span, ends_line};
use crate::lint;

/// Renders `diags` against `sources` to a plain (uncoloured) terminal string.
///
/// Every diagnostic's [`Span`] carries a [`FileId`](super::FileId) issued by
/// `sources`, so replaying the source map's files into the `codespan-reporting`
/// file table in id order keeps each `FileId` aligned with its codespan id.
/// Rendering into a `String` produces plain text with no ANSI colour codes, so
/// the output is stable for snapshots and clean when piped.
pub fn render(diags: &[Diagnostic], sources: &SourceMap) -> String {
    let files = LineFiles(
        sources
            .iter_files()
            .map(|(path, text)| LineFile::new(path, text))
            .collect(),
    );
    let file_count = files.0.len();

    let config = Config::default();
    let mut out = String::new();
    for diag in diags {
        let rendered = to_codespan(diag, file_count);
        term::emit_to_string(&mut out, &config, &files, &rendered)
            .expect("rendering to an in-memory string cannot fail");
    }
    out
}

/// One file of the `codespan-reporting` file table: its path, its text, and the
/// byte offset at which each of its lines starts.
struct LineFile<'a> {
    path: &'a str,
    text: &'a str,
    line_starts: Vec<usize>,
}

impl<'a> LineFile<'a> {
    /// Indexes `text`. A line ends at an LF, a CRLF pair or a lone CR, the same
    /// rule [`line_col`](super::line_col) follows, so the rendered locations
    /// and the JSON positions agree. `codespan-reporting`'s own `SimpleFiles`
    /// ends a line at an LF only.
    fn new(path: &'a str, text: &'a str) -> Self {
        let bytes = text.as_bytes();
        let line_starts = std::iter::once(0)
            .chain(
                (0..bytes.len())
                    .filter(|&at| ends_line(bytes, at))
                    .map(|at| at + 1),
            )
            .collect();
        LineFile {
            path,
            text,
            line_starts,
        }
    }
}

/// The `codespan-reporting` file table, indexed by [`FileId`](super::FileId).
struct LineFiles<'a>(Vec<LineFile<'a>>);

impl LineFiles<'_> {
    fn file(&self, id: usize) -> Result<&LineFile<'_>, FilesError> {
        self.0.get(id).ok_or(FilesError::FileMissing)
    }
}

impl<'a> Files<'a> for LineFiles<'a> {
    type FileId = usize;
    type Name = &'a str;
    type Source = &'a str;

    fn name(&'a self, id: usize) -> Result<&'a str, FilesError> {
        Ok(self.file(id)?.path)
    }

    fn source(&'a self, id: usize) -> Result<&'a str, FilesError> {
        Ok(self.file(id)?.text)
    }

    fn line_index(&'a self, id: usize, byte_index: usize) -> Result<usize, FilesError> {
        Ok(self
            .file(id)?
            .line_starts
            .partition_point(|start| *start <= byte_index)
            - 1)
    }

    fn line_range(&'a self, id: usize, line_index: usize) -> Result<Range<usize>, FilesError> {
        let file = self.file(id)?;
        let start = *file
            .line_starts
            .get(line_index)
            .ok_or(FilesError::LineTooLarge {
                given: line_index,
                max: file.line_starts.len() - 1,
            })?;
        let end = file
            .line_starts
            .get(line_index + 1)
            .copied()
            .unwrap_or(file.text.len());
        Ok(start..end)
    }
}

/// Maps one homegrown [`Diagnostic`] to a `codespan-reporting` diagnostic.
///
/// A label whose [`FileId`](super::FileId) is not in the source map — most
/// often [`FileId::DETACHED`](super::FileId::DETACHED), which the lockfile and
/// fetch diagnostics carry — is dropped rather than looked up, so a detached
/// diagnostic renders as a bare coded message with no source snippet instead of
/// panicking on a missing file.
fn to_codespan(diag: &Diagnostic, file_count: usize) -> cs::Diagnostic<usize> {
    let severity = match diag.severity {
        Severity::Error => cs::Severity::Error,
        Severity::Warning => cs::Severity::Warning,
        Severity::Info => cs::Severity::Note,
    };

    let in_range = |span: &Span| (span.file.0 as usize) < file_count;

    let mut labels = Vec::new();
    if in_range(&diag.primary) {
        labels.push(label(&diag.primary, cs::LabelStyle::Primary, ""));
    }
    for secondary in &diag.labels {
        if in_range(&secondary.span) {
            labels.push(label(
                &secondary.span,
                cs::LabelStyle::Secondary,
                &secondary.message,
            ));
        }
    }
    for fixit in &diag.fixits {
        if in_range(&fixit.span) {
            labels.push(label(&fixit.span, cs::LabelStyle::Secondary, &fixit.label));
        }
    }

    // Fix-its render as notes: codespan-reporting has no first-class suggestion,
    // so the suggested replacement text is spelled out under the diagnostic.
    let mut notes: Vec<String> = diag
        .fixits
        .iter()
        .map(|fixit| {
            format!(
                "suggestion: replace with `{}` — {}",
                fixit.replacement, fixit.label
            )
        })
        .collect();
    // A lint diagnostic names its lint after the fix-it notes, so a reader
    // knows the key that sets its level
    // (docs/archive/2026-10-03-lint-foundation-design.md §7.2). The note
    // depends only on the code, not on the level that applied.
    if let Some(name) = lint::lint_of(diag.code).and_then(|entry| entry.lint) {
        notes.push(format!(
            "lint: `{name}` (set its level in `[lints]` in ridl.toml)"
        ));
    }

    let mut rendered = cs::Diagnostic::new(severity)
        .with_message(&diag.message)
        .with_labels(labels)
        .with_notes(notes);
    if !diag.code.is_empty() {
        rendered = rendered.with_code(diag.code.as_str());
    }
    rendered
}

/// Builds a codespan label from a [`Span`]. An empty message yields a bare caret
/// (the primary span carries the message in the diagnostic header).
fn label(span: &Span, style: cs::LabelStyle, message: &str) -> cs::Label<usize> {
    let start = u32::from(span.range.start()) as usize;
    let end = u32::from(span.range.end()) as usize;
    let built = cs::Label::new(style, span.file.0 as usize, start..end);
    if message.is_empty() {
        built
    } else {
        built.with_message(message)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{DiagCode, Diagnostic, FixIt, Severity, SourceMap, Span};
    use rowan::{TextRange, TextSize};

    fn span(map: &mut SourceMap, path: &str, text: &str, start: u32, end: u32) -> Span {
        let file = map.file_id(path, text);
        Span {
            file,
            range: TextRange::new(TextSize::new(start), TextSize::new(end)),
        }
    }

    /// Two diagnostics at the same offset — a positional FORM-101 and the
    /// profile-boundary TYPL-302 a duration literal raises — render as two clean
    /// blocks with their own carets, no overlap panic.
    #[test]
    fn two_diagnostics_at_one_offset_render_cleanly() {
        let text = "package p\ntype X: integer [0..10ms]\n";
        let mut map = SourceMap::new();
        // `10ms` sits at bytes 30..34.
        let at_duration = span(&mut map, "demo.typl", text, 30, 34);
        let diags = vec![
            Diagnostic {
                code: DiagCode::FORM_101,
                severity: Severity::Error,
                message: "expected `]`".to_string(),
                primary: at_duration,
                labels: Vec::new(),
                fixits: Vec::new(),
            },
            Diagnostic {
                code: DiagCode::TYPL_302,
                severity: Severity::Error,
                message: "duration literal in typl context".to_string(),
                primary: at_duration,
                labels: Vec::new(),
                fixits: Vec::new(),
            },
        ];
        let rendered = super::render(&diags, &map);
        insta::assert_snapshot!("two_diagnostics_same_offset", rendered);
    }

    /// A fix-it-carrying diagnostic spells its suggested replacement out under
    /// the diagnostic.
    /// The location header counts a lone CR as a line break, as `line_col` and
    /// the language server do, and a CRLF pair as one line break.
    #[test]
    fn location_counts_a_lone_cr_and_a_crlf_pair_as_one_line_break_each() {
        for (text, expected) in [
            ("package p\rtype Speed: km/h\r", "demo.typl:2:6"),
            ("package p\r\ntype Speed: km/h\r\n", "demo.typl:2:6"),
            ("package p\r\r\ntype Speed: km/h\r\n", "demo.typl:3:6"),
        ] {
            let mut map = SourceMap::new();
            let start = text.find("Speed").unwrap() as u32;
            let at_name = span(&mut map, "demo.typl", text, start, start + 5);
            let diags = vec![Diagnostic {
                code: DiagCode::NONE,
                severity: Severity::Warning,
                message: "type name should be capitalised".to_string(),
                primary: at_name,
                labels: Vec::new(),
                fixits: Vec::new(),
            }];
            let rendered = super::render(&diags, &map);
            assert!(
                rendered.contains(expected),
                "expected `{expected}` in:\n{rendered}",
            );
        }
    }

    #[test]
    fn fixit_renders_its_suggestion() {
        let text = "package p\ntype Speed: km/h\n";
        let mut map = SourceMap::new();
        let at_name = span(&mut map, "demo.typl", text, 15, 20); // `Speed`
        let diags = vec![Diagnostic {
            code: DiagCode::NONE,
            severity: Severity::Warning,
            message: "type name should be capitalised".to_string(),
            primary: at_name,
            labels: Vec::new(),
            fixits: vec![FixIt {
                span: at_name,
                replacement: "Velocity".to_string(),
                label: "rename to `Velocity`".to_string(),
            }],
        }];
        let rendered = super::render(&diags, &map);
        assert!(
            rendered.contains("suggestion: replace with `Velocity`"),
            "the rendered output must spell the suggested replacement, got:\n{rendered}",
        );
        assert!(rendered.contains("rename to `Velocity`"));
    }

    /// A lint diagnostic ends with the note that names its lint, after the
    /// fix-it notes (docs/book/lints.md). The file has fewer than ten
    /// lines, so the note line is two spaces of gutter followed by the text.
    #[test]
    fn lint_diagnostic_renders_its_lint_note() {
        let text = "package p\ninterface S {\n  signal speed: Speed\n}\n";
        let mut map = SourceMap::new();
        let at_signal = span(&mut map, "demo.ridl", text, 32, 37); // `speed`
        let diags = vec![Diagnostic {
            code: DiagCode::RIDL_100,
            severity: Severity::Warning,
            message: "signal without a timing annotation".to_string(),
            primary: at_signal,
            labels: Vec::new(),
            fixits: vec![FixIt {
                span: at_signal,
                replacement: "speed: Speed @10ms".to_string(),
                label: "write the timing".to_string(),
            }],
        }];
        let rendered = super::render(&diags, &map);
        assert!(
            rendered.lines().any(|line| line
                == "  = lint: `missing-timing` (set its level in `[lints]` in ridl.toml)"),
            "the rendered output must hold the lint note line, got:\n{rendered}",
        );
        insta::assert_snapshot!("lint_diagnostic_note", rendered);
    }

    /// A span that runs across a blank line into the next declaration's keyword
    /// renders as a multi-line underline without panicking.
    #[test]
    fn cross_line_span_renders_without_panic() {
        let text = "type A: m\n\ntype B: s\n";
        let mut map = SourceMap::new();
        // From the end of the first declaration across the blank line to `type`.
        let across = span(&mut map, "demo.typl", text, 9, 15);
        let diags = vec![Diagnostic {
            code: DiagCode::FORM_103,
            severity: Severity::Error,
            message: "unclosed `{`".to_string(),
            primary: across,
            labels: Vec::new(),
            fixits: Vec::new(),
        }];
        let rendered = super::render(&diags, &map);
        assert!(
            rendered.contains("FORM-103"),
            "the rendered output must carry the code, got:\n{rendered}",
        );
    }

    /// No diagnostics render to an empty string.
    #[test]
    fn no_diagnostics_render_to_empty_string() {
        let map = SourceMap::new();
        assert_eq!(super::render(&[], &map), "");
    }

    /// A detached diagnostic (a MANI-1xx fetch or lockfile diagnostic, whose
    /// primary span is [`FileId::DETACHED`]) renders as a bare coded message —
    /// its out-of-range file id is dropped, not looked up, so rendering never
    /// panics even against an empty source map.
    #[test]
    fn detached_diagnostic_renders_without_a_source_snippet() {
        use super::super::FileId;
        let diags = vec![Diagnostic {
            code: DiagCode::MANI_101,
            severity: Severity::Error,
            message: "failed to fetch `https://registry.example.com/foo@v1`".to_string(),
            primary: Span {
                file: FileId::DETACHED,
                range: TextRange::default(),
            },
            labels: Vec::new(),
            fixits: Vec::new(),
        }];
        let map = SourceMap::new();
        let rendered = super::render(&diags, &map);
        assert!(
            rendered.contains("MANI-101"),
            "the rendered output must carry the code, got:\n{rendered}",
        );
        assert!(
            rendered.contains("failed to fetch"),
            "the rendered output must carry the message, got:\n{rendered}",
        );
    }
}
