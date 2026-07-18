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

use codespan_reporting::diagnostic as cs;
use codespan_reporting::files::SimpleFiles;
use codespan_reporting::term::{self, Config};

use super::{Diagnostic, Severity, SourceMap, Span};

/// Renders `diags` against `sources` to a plain (uncoloured) terminal string.
///
/// Every diagnostic's [`Span`] carries a [`FileId`](super::FileId) issued by
/// `sources`, so replaying the source map's files into the `codespan-reporting`
/// file table in id order keeps each `FileId` aligned with its codespan id.
/// Rendering into a `String` produces plain text with no ANSI colour codes, so
/// the output is stable for snapshots and clean when piped.
pub fn render(diags: &[Diagnostic], sources: &SourceMap) -> String {
    let mut files = SimpleFiles::new();
    for (path, text) in sources.iter_files() {
        files.add(path, text);
    }

    let config = Config::default();
    let mut out = String::new();
    for diag in diags {
        let rendered = to_codespan(diag);
        term::emit_to_string(&mut out, &config, &files, &rendered)
            .expect("rendering to an in-memory string cannot fail");
    }
    out
}

/// Maps one homegrown [`Diagnostic`] to a `codespan-reporting` diagnostic.
fn to_codespan(diag: &Diagnostic) -> cs::Diagnostic<usize> {
    let severity = match diag.severity {
        Severity::Error => cs::Severity::Error,
        Severity::Warning => cs::Severity::Warning,
        Severity::Info => cs::Severity::Note,
    };

    let mut labels = vec![label(&diag.primary, cs::LabelStyle::Primary, "")];
    for secondary in &diag.labels {
        labels.push(label(
            &secondary.span,
            cs::LabelStyle::Secondary,
            &secondary.message,
        ));
    }
    for fixit in &diag.fixits {
        labels.push(label(&fixit.span, cs::LabelStyle::Secondary, &fixit.label));
    }

    // Fix-its render as notes: codespan-reporting has no first-class suggestion,
    // so the suggested replacement text is spelled out under the diagnostic.
    let notes: Vec<String> = diag
        .fixits
        .iter()
        .map(|fixit| {
            format!(
                "suggestion: replace with `{}` — {}",
                fixit.replacement, fixit.label
            )
        })
        .collect();

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
}
