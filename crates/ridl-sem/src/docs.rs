//! Doc-comment scanning (typl language reference §14; ADR-0026).
//!
//! A carrier's doc comments are trivia tokens sitting before it (collected by
//! the `ridl_syntax::ast::HasDocComments` trait). This module strips the
//! comment markers, separates the prose body from the tag lines, reads the
//! body as CommonMark to find the link candidates, and keeps a map from every
//! byte of the body to its source offset, so a link or a tag problem carries
//! an exact source span.
//!
//! The four tags are `@see`, `@since`, `@deprecated` and `@labels` (typl
//! §14.2, ADR-0026). A tag is `@word` at the start of a line of the doc, after
//! the comment markers and leading whitespace; an `@` anywhere else is prose.
//! Any other `@word` at the start of a line is an unknown tag (TYPL-408), and
//! a `@see` or `@since` whose value is missing or malformed is a malformed tag
//! (TYPL-409); both are returned as [`DocInfo::problems`] for the doc lints. A
//! `@deprecated` with no reason is still TYPL-405, raised by the checker.
//!
//! The scanner does not resolve a link: the resolver reads
//! [`DocInfo::links`] and [`DocInfo::see`] and raises TYPL-401 for a target
//! that does not resolve. TYPL-402/403 (`@labels` vocabulary and combination
//! validation) stay deferred to assurance profiles (ADR-0007 decision 10);
//! `@labels` identifiers are carried through to the IR unchecked (§14.3).

use std::ops::Range;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use ridl_syntax::SyntaxToken;
use rowan::{TextRange, TextSize};

/// The parsed content of a carrier's doc comments (typl §14, ADR-0026).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocInfo {
    /// The prose body: every non-tag line with its comment markers stripped,
    /// joined by newlines, without leading or trailing blank lines. Empty when
    /// the comment is only tags.
    pub doc: String,
    /// The `@labels` classification identifiers, in source order (§14.3),
    /// passed through to the IR unchanged.
    pub labels: Vec<String>,
    /// The `@deprecated` reason, when the tag is present. `Some("")` records a
    /// `@deprecated` with no reason string — the checker raises TYPL-405 for it
    /// while still marking the declaration deprecated.
    pub deprecated: Option<String>,
    /// The link candidates found in `doc` (ADR-0026), in source order: a
    /// `[Name]`, a ``[`Name`]`` or a `[text][Name]` whose target is a
    /// qualified identifier, outside code spans and fenced blocks, and with
    /// no CommonMark reference definition of the same label in the doc.
    pub links: Vec<LinkCandidate>,
    /// The `@see` targets, in source order; their `doc_range` is empty.
    pub see: Vec<LinkCandidate>,
    /// The `@since` versions, in source order; every value is kept.
    pub since: Vec<String>,
    /// The unknown and malformed tags, in source order (TYPL-408, TYPL-409).
    pub problems: Vec<TagProblem>,
}

/// One doc link candidate: a bracketed name in the body, or an `@see` target.
/// The resolver decides whether it names a declaration (ADR-0026).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkCandidate {
    /// The target's dotted segments: `["pkg", "Name", "member"]`.
    pub segments: Vec<String>,
    /// The visible link text: the name itself, or the label of a
    /// `[text][Name]` link.
    pub text: String,
    /// The byte range of the whole link in [`DocInfo::doc`]; empty for an
    /// `@see` target.
    pub doc_range: Range<usize>,
    /// The span of the whole link in the source file — for an `@see` target,
    /// the span of the name.
    pub source: TextRange,
    /// The span of the target name alone in the source file: the `Name` of
    /// `[Name]`, the name inside the backticks of ``[`Name`]``, the name in
    /// the second bracket of `[text][Name]`, or the name after `@see`. A
    /// rename of the target replaces exactly this span.
    pub target: TextRange,
}

/// A doc tag the scanner could not read (ADR-0026).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagProblem {
    pub kind: TagProblemKind,
    /// The span of the tag line's text, from the `@` to its last character.
    pub source: TextRange,
}

/// What is wrong with a doc tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagProblemKind {
    /// A `@word` at the start of a line that is none of the four tags
    /// (TYPL-408); holds the word without its `@`.
    Unknown(String),
    /// A known tag with a missing or malformed value (TYPL-409); holds the tag
    /// name without its `@`: `"see"` or `"since"`.
    Malformed(&'static str),
}

impl DocInfo {
    /// Whether a `@deprecated` tag was present but carried no reason string
    /// (TYPL-405).
    pub fn deprecated_missing_reason(&self) -> bool {
        matches!(&self.deprecated, Some(reason) if reason.is_empty())
    }
}

/// Scans the doc-comment tokens preceding a carrier into a [`DocInfo`]
/// (typl §14, ADR-0026). The tokens are in source order; both `///` line
/// comments and `/** ... */` block comments are accepted, and the tokens may
/// be any run of `DocComment` tokens of a tree — the language server scans one
/// comment run the same way.
pub fn scan(tokens: &[SyntaxToken]) -> DocInfo {
    let mut info = DocInfo::default();
    let mut body: Vec<Line<'_>> = Vec::new();
    for line in tokens.iter().flat_map(comment_lines) {
        if !read_tag(&line, &mut info) {
            body.push(line);
        }
    }
    let (doc, lines) = join_body(&body);
    info.doc = doc;
    info.links = link_candidates(&info.doc, &lines);
    info
}

/// One line of a doc comment with its markers stripped: the text and the
/// source offset of its first byte.
#[derive(Debug, Clone, Copy)]
struct Line<'a> {
    text: &'a str,
    source: TextSize,
}

impl Line<'_> {
    /// The source span of `range`, a byte range of `text`.
    fn span(&self, range: Range<usize>) -> TextRange {
        TextRange::new(
            self.source + TextSize::from(range.start as u32),
            self.source + TextSize::from(range.end as u32),
        )
    }
}

/// One line of the joined body: its byte range in `doc` and its source
/// offset.
#[derive(Debug, Clone)]
struct DocLine {
    doc: Range<usize>,
    source: TextSize,
}

/// Reads `line` as a tag line, recording it in `info`. Returns `false` when
/// the line is prose (it does not start with `@word`).
fn read_tag(line: &Line<'_>, info: &mut DocInfo) -> bool {
    let text = line.text;
    let at = text.len() - text.trim_start().len();
    let after_at = &text[at..];
    let Some(rest) = after_at.strip_prefix('@') else {
        return false;
    };
    let word_len = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(rest.len());
    let word = &rest[..word_len];
    if !word.starts_with(|c: char| c.is_ascii_alphabetic()) {
        // `@` alone or `@[..10ms]` is prose, not a tag.
        return false;
    }
    let value_start = at + 1 + word_len;
    let value_start =
        value_start + text[value_start..].len() - text[value_start..].trim_start().len();
    let value = &text[value_start..];
    let tag_span = line.span(at..text.len());
    match word {
        "see" => {
            if is_qualified_name(value) {
                let span = line.span(value_start..text.len());
                info.see.push(LinkCandidate {
                    segments: value.split('.').map(str::to_string).collect(),
                    text: value.to_string(),
                    doc_range: 0..0,
                    source: span,
                    target: span,
                });
            } else {
                info.problems.push(TagProblem {
                    kind: TagProblemKind::Malformed("see"),
                    source: tag_span,
                });
            }
        }
        "since" => {
            if is_version(value) {
                info.since.push(value.to_string());
            } else {
                info.problems.push(TagProblem {
                    kind: TagProblemKind::Malformed("since"),
                    source: tag_span,
                });
            }
        }
        "labels" => {
            for label in value.split(',') {
                let label = label.trim();
                if !label.is_empty() {
                    info.labels.push(label.to_string());
                }
            }
        }
        "deprecated" => info.deprecated = Some(deprecated_reason(value)),
        other => info.problems.push(TagProblem {
            kind: TagProblemKind::Unknown(other.to_string()),
            source: tag_span,
        }),
    }
    true
}

/// Whether `text` is a qualified identifier: `Name`, `pkg.Name` or
/// `pkg.Name.member` — segments of `[A-Za-z_][A-Za-z0-9_]*` joined by `.`.
fn is_qualified_name(text: &str) -> bool {
    !text.is_empty()
        && text.split('.').all(|segment| {
            let mut chars = segment.chars();
            chars
                .next()
                .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
}

/// Whether `text` is a `@since` version: `MAJOR.MINOR` or
/// `MAJOR.MINOR.PATCH`, each a run of ASCII digits.
fn is_version(text: &str) -> bool {
    let parts: Vec<&str> = text.split('.').collect();
    (parts.len() == 2 || parts.len() == 3)
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
}

/// The reason string of a `@deprecated` tag. A quoted `"reason"` yields its
/// inner text; a bare non-empty value is tolerated as the reason; an empty
/// value yields `""`, which the checker reports as TYPL-405.
fn deprecated_reason(rest: &str) -> String {
    let rest = rest.trim();
    if let Some(inner) = rest.strip_prefix('"').and_then(|r| r.strip_suffix('"')) {
        return inner.to_string();
    }
    rest.to_string()
}

/// Joins the prose lines into the doc body, without leading or trailing blank
/// lines, and returns the map from the body's lines to their source offsets.
fn join_body(body: &[Line<'_>]) -> (String, Vec<DocLine>) {
    let first = body.iter().position(|line| !line.text.is_empty());
    let last = body.iter().rposition(|line| !line.text.is_empty());
    let (Some(first), Some(last)) = (first, last) else {
        return (String::new(), Vec::new());
    };
    let mut doc = String::new();
    let mut lines = Vec::new();
    for (index, line) in body[first..=last].iter().enumerate() {
        if index > 0 {
            doc.push('\n');
        }
        let start = doc.len();
        doc.push_str(line.text);
        lines.push(DocLine {
            doc: start..doc.len(),
            source: line.source,
        });
    }
    (doc, lines)
}

/// What a byte of the doc body is, as CommonMark reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mask {
    /// Not text: structure, a fenced block, HTML, or a link the doc defines.
    Other,
    /// Prose text, outside code.
    Text,
    /// A code span, backticks included.
    Code,
}

/// The link candidates of `doc` (ADR-0026): every `[Name]`, ``[`Name`]`` and
/// `[text][Name]` whose target is a qualified identifier, found in the text
/// CommonMark reads as prose. A bracket in a code span or a fenced block is
/// not text, and a `[Name]` with a reference definition in the doc is a
/// CommonMark link, so neither is a candidate. A candidate that spans a line
/// break is not one either.
fn link_candidates(doc: &str, lines: &[DocLine]) -> Vec<LinkCandidate> {
    let mut mask = vec![Mask::Other; doc.len()];
    let mut code_blocks = 0usize;
    for (event, range) in Parser::new_ext(doc, Options::empty()).into_offset_iter() {
        match event {
            Event::Start(Tag::CodeBlock(_)) => code_blocks += 1,
            Event::End(TagEnd::CodeBlock) => code_blocks = code_blocks.saturating_sub(1),
            Event::Text(_) if code_blocks == 0 => mask[range].fill(Mask::Text),
            Event::Code(_) => mask[range].fill(Mask::Code),
            _ => {}
        }
    }

    let bytes = doc.as_bytes();
    let mut links = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let Some(candidate) = candidate_at(doc, &mask, index) else {
            index += 1;
            continue;
        };
        let end = candidate.doc_range.end;
        if let (Some(source), Some(target)) = (
            source_span(lines, &candidate.doc_range),
            source_span(lines, &candidate.target),
        ) {
            links.push(LinkCandidate {
                segments: candidate
                    .target_text()
                    .split('.')
                    .map(str::to_string)
                    .collect(),
                text: candidate.text,
                doc_range: candidate.doc_range,
                source,
                target,
            });
        }
        index = end;
    }
    links
}

/// A link candidate found in the doc body, as byte ranges of the body.
struct Candidate<'a> {
    doc: &'a str,
    /// The visible text.
    text: String,
    /// The whole link.
    doc_range: Range<usize>,
    /// The target name alone.
    target: Range<usize>,
}

impl Candidate<'_> {
    fn target_text(&self) -> &str {
        &self.doc[self.target.clone()]
    }
}

/// A link candidate opening at `index` with a `[` of prose text. `None` when
/// no candidate opens there.
fn candidate_at<'a>(doc: &'a str, mask: &[Mask], index: usize) -> Option<Candidate<'a>> {
    let bytes = doc.as_bytes();
    if bytes[index] != b'[' || mask[index] != Mask::Text {
        return None;
    }
    if index > 0 && bytes[index - 1] == b'\\' {
        return None;
    }
    let close = closing_bracket(bytes, index)?;
    let content = &doc[index + 1..close];
    let text_only = |range: Range<usize>| mask[range].iter().all(|m| *m == Mask::Text);

    // `[text][Name]`: the explicit label wins when a second bracket follows,
    // as it does in CommonMark.
    if bytes.get(close + 1) == Some(&b'[')
        && let Some(second_close) = closing_bracket(bytes, close + 1)
    {
        let target = close + 2..second_close;
        if text_only(target.clone()) && is_qualified_name(&doc[target.clone()]) {
            return Some(Candidate {
                doc,
                text: content.to_string(),
                doc_range: index..second_close + 1,
                target,
            });
        }
    }
    // `[Name]`.
    if text_only(index + 1..close) && is_qualified_name(content) {
        return Some(Candidate {
            doc,
            text: content.to_string(),
            doc_range: index..close + 1,
            target: index + 1..close,
        });
    }
    // ``[`Name`]``: the content is exactly one code span.
    let inner = content.strip_prefix('`')?.strip_suffix('`')?;
    if mask[index + 1..close].iter().all(|m| *m == Mask::Code) && is_qualified_name(inner) {
        return Some(Candidate {
            doc,
            text: inner.to_string(),
            doc_range: index..close + 1,
            target: index + 2..close - 1,
        });
    }
    None
}

/// The `]` that closes the `[` at `open`, on the same line and with no other
/// `[` between them.
fn closing_bracket(bytes: &[u8], open: usize) -> Option<usize> {
    bytes[open + 1..]
        .iter()
        .position(|b| matches!(b, b']' | b'[' | b'\n'))
        .map(|offset| open + 1 + offset)
        .filter(|&close| bytes[close] == b']')
}

/// The source span of `range`, a byte range of the doc body, when the range
/// sits inside one line of the body.
fn source_span(lines: &[DocLine], range: &Range<usize>) -> Option<TextRange> {
    let line = lines
        .iter()
        .find(|line| line.doc.start <= range.start && range.end <= line.doc.end)?;
    let start = line.source + TextSize::from((range.start - line.doc.start) as u32);
    let end = line.source + TextSize::from((range.end - line.doc.start) as u32);
    Some(TextRange::new(start, end))
}

/// The marker-stripped lines of one doc-comment token, each with the source
/// offset of its text.
///
/// - A `///` line yields one line: the text after `///` and one separating
///   space, with trailing whitespace removed. The rest of the indentation
///   stays, so an indented Markdown construct survives.
/// - A `/** ... */` block yields one line per source line. The first line is
///   the text after `/**`, read as a `///` line. The continuation lines are
///   decorated when every one that is not blank starts, after its leading
///   whitespace, with a `*` followed by a space or the end of the line; on a
///   decorated line the whitespace, the `*` and one space are removed. The
///   continuation lines of an undecorated block lose the leading whitespace
///   they all share. These are the rules of the TYPL-410 fix-it, so the
///   rewritten comment scans to the same doc.
fn comment_lines(token: &SyntaxToken) -> Vec<Line<'_>> {
    let text = token.text();
    let base = token.text_range().start();
    if let Some(rest) = text.strip_prefix("///") {
        return vec![after_marker(rest, base + TextSize::from(3))];
    }
    let Some(inner) = text
        .strip_prefix("/**")
        .and_then(|rest| rest.strip_suffix("*/"))
    else {
        // Not a recognized doc-comment shape; stay total.
        return vec![trimmed(text, base)];
    };
    let mut lines = Vec::new();
    let mut offset = 3usize;
    let mut raw = inner.split('\n');
    let first = raw.next().unwrap_or_default();
    lines.push(after_marker(first, base + TextSize::from(offset as u32)));
    offset += first.len() + 1;
    // The trailing whitespace — a `\r` under CRLF among it — is removed
    // first, so a bare ` *` line reads as decoration.
    let rest: Vec<(&str, usize)> = raw
        .map(|line| {
            let at = offset;
            offset += line.len() + 1;
            (line.trim_end(), at)
        })
        .collect();

    let is_decoration = |line: &str| {
        let line = line.trim_start();
        line == "*" || line.starts_with("* ")
    };
    let not_blank = |line: &str| !line.trim().is_empty();
    let decorated = rest
        .iter()
        .filter(|(line, _)| not_blank(line))
        .all(|(line, _)| is_decoration(line));
    let shared_indent = rest
        .iter()
        .filter(|(line, _)| not_blank(line))
        .map(|(line, _)| line.len() - line.trim_start_matches([' ', '\t']).len())
        .min()
        .unwrap_or(0);
    for (line, at) in rest {
        let source = base + TextSize::from(at as u32);
        if !not_blank(line) {
            lines.push(Line { text: "", source });
        } else if decorated {
            let indent = line.len() - line.trim_start().len();
            // The `*` after the indentation, then one separating space.
            lines.push(after_marker(
                &line[indent + 1..],
                source + TextSize::from(indent as u32 + 1),
            ));
        } else {
            // Only the shared indent is removed; `line` already lost its
            // trailing whitespace, and the indentation beyond the shared
            // part is content, as on a `///` line.
            lines.push(Line {
                text: &line[shared_indent..],
                source: source + TextSize::from(shared_indent as u32),
            });
        }
    }
    lines
}

/// The line text after a `///` or `*` marker at `source`: one separating
/// space is removed, trailing whitespace is removed, and the rest stays.
fn after_marker(rest: &str, source: TextSize) -> Line<'_> {
    match rest.strip_prefix(' ') {
        Some(after) => Line {
            text: after.trim_end(),
            source: source + TextSize::from(1),
        },
        None => Line {
            text: rest.trim_end(),
            source,
        },
    }
}

/// `text` with its surrounding whitespace removed, at `source`.
fn trimmed(text: &str, source: TextSize) -> Line<'_> {
    let indent = text.len() - text.trim_start().len();
    Line {
        text: text.trim(),
        source: source + TextSize::from(indent as u32),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ridl_syntax::{SyntaxKind, SyntaxNode};
    use rowan::GreenNodeBuilder;

    /// Builds standalone `DocComment` tokens from their source text, wrapped in
    /// a throwaway root node so the tokens are real `SyntaxToken`s.
    fn doc_tokens(texts: &[&str]) -> Vec<SyntaxToken> {
        let tokens: Vec<(SyntaxKind, &str)> = texts
            .iter()
            .map(|text| (SyntaxKind::DocComment, *text))
            .collect();
        tokens_of(&tokens)
    }

    /// Builds a run of tokens from their kinds and source text, wrapped in a
    /// throwaway root node, and returns its `DocComment` tokens. The source
    /// offsets of the tokens are those of the concatenated text.
    fn tokens_of(tokens: &[(SyntaxKind, &str)]) -> Vec<SyntaxToken> {
        let mut builder = GreenNodeBuilder::new();
        builder.start_node(SyntaxKind::SourceFile.into());
        for (kind, text) in tokens {
            builder.token((*kind).into(), text);
        }
        builder.finish_node();
        let root = SyntaxNode::new_root(builder.finish());
        root.children_with_tokens()
            .filter_map(|element| element.into_token())
            .filter(|token| token.kind() == SyntaxKind::DocComment)
            .collect()
    }

    /// The segments of every link candidate, in order.
    fn segments(links: &[LinkCandidate]) -> Vec<Vec<&str>> {
        links
            .iter()
            .map(|link| link.segments.iter().map(String::as_str).collect())
            .collect()
    }

    #[test]
    fn scan_finds_three_link_forms() {
        let info = scan(&doc_tokens(&[
            "/// The [Speed] of [`veh.Gear`] and [the gear][Gear.PARK].",
        ]));
        assert_eq!(
            segments(&info.links),
            [vec!["Speed"], vec!["veh", "Gear"], vec!["Gear", "PARK"]]
        );
        assert_eq!(info.links[0].text, "Speed");
        assert_eq!(info.links[1].text, "veh.Gear");
        assert_eq!(info.links[2].text, "the gear");
        assert_eq!(&info.doc[info.links[0].doc_range.clone()], "[Speed]");
        assert_eq!(&info.doc[info.links[1].doc_range.clone()], "[`veh.Gear`]");
        assert_eq!(
            &info.doc[info.links[2].doc_range.clone()],
            "[the gear][Gear.PARK]"
        );
        assert!(info.problems.is_empty());
    }

    #[test]
    fn scan_ignores_non_identifier_brackets() {
        let info = scan(&doc_tokens(&[
            "/// In [0..250], [see below] and [Speed.] or [a..b].",
        ]));
        assert_eq!(info.links, Vec::new());
    }

    #[test]
    fn scan_ignores_code() {
        let info = scan(&doc_tokens(&[
            "/// Not `[Speed]` here.",
            "///",
            "/// ```",
            "/// [Speed]",
            "/// ```",
        ]));
        assert_eq!(info.links, Vec::new());
    }

    #[test]
    fn scan_ignores_reference_definitions() {
        let info = scan(&doc_tokens(&[
            "/// See [Speed].",
            "///",
            "/// [Speed]: https://example.invalid/speed",
        ]));
        assert_eq!(info.links, Vec::new());
    }

    #[test]
    fn scan_reads_tags() {
        let info = scan(&doc_tokens(&[
            "/// A gear.",
            "/// @see veh.Gear",
            "/// @since 1.2",
            "/// @since 1.2.3",
            "/// @sinc 1.0",
            "/// @since soon",
            "/// @see",
            "/// mail me @home",
        ]));
        assert_eq!(info.doc, "A gear.\nmail me @home");
        assert_eq!(segments(&info.see), [vec!["veh", "Gear"]]);
        assert_eq!(info.see[0].text, "veh.Gear");
        assert_eq!(info.see[0].doc_range, 0..0);
        assert_eq!(info.since, ["1.2", "1.2.3"]);
        assert_eq!(
            info.problems
                .iter()
                .map(|problem| problem.kind.clone())
                .collect::<Vec<_>>(),
            [
                TagProblemKind::Unknown("sinc".to_string()),
                TagProblemKind::Malformed("since"),
                TagProblemKind::Malformed("see"),
            ]
        );
        assert!(info.links.is_empty());
    }

    #[test]
    fn scan_tag_problem_spans_cover_the_tag_text() {
        let text = "/// @sinc 1.0";
        let info = scan(&doc_tokens(&[text]));
        let range = info.problems[0].source;
        assert_eq!(
            &text[usize::from(range.start())..usize::from(range.end())],
            "@sinc 1.0"
        );
    }

    /// Review Focus 1: the `*` decoration of a block doc is stripped before
    /// the links and tags are read, and the source spans point at the
    /// decorated text's columns.
    #[test]
    fn scan_block_doc_decoration() {
        let text = "/**\n * [Speed] here\n * @since 1.0\n */";
        let info = scan(&doc_tokens(&[text]));
        assert_eq!(info.doc, "[Speed] here");
        assert_eq!(segments(&info.links), [vec!["Speed"]]);
        let range = info.links[0].source;
        assert_eq!(
            &text[usize::from(range.start())..usize::from(range.end())],
            "[Speed]"
        );
        assert_eq!(info.since, ["1.0"]);
        assert!(info.problems.is_empty());
    }

    /// Review Focus 2: `\r\n` line endings leave no `\r` in the doc, and the
    /// link's byte offsets in `doc` and its source span are both exact.
    #[test]
    fn scan_crlf() {
        let text = "/// [Speed] here\r\n/// @since 1.0\r\n";
        let info = scan(&tokens_of(&[
            (SyntaxKind::DocComment, "/// [Speed] here"),
            (SyntaxKind::Whitespace, "\r\n"),
            (SyntaxKind::DocComment, "/// @since 1.0"),
            (SyntaxKind::Whitespace, "\r\n"),
        ]));
        assert_eq!(info.doc, "[Speed] here");
        assert_eq!(segments(&info.links), [vec!["Speed"]]);
        assert_eq!(&info.doc[info.links[0].doc_range.clone()], "[Speed]");
        let range = info.links[0].source;
        assert_eq!(
            &text[usize::from(range.start())..usize::from(range.end())],
            "[Speed]"
        );
        assert_eq!(info.since, ["1.0"]);
    }

    /// A block doc with CRLF line endings and a bare ` *` line is still
    /// decorated: the `\r` is not part of the line's text.
    #[test]
    fn scan_block_doc_decoration_with_crlf() {
        let text = "/**\r\n * First.\r\n *\r\n * [Speed] here\r\n */";
        let info = scan(&doc_tokens(&[text]));
        assert_eq!(info.doc, "First.\n\n[Speed] here");
        let range = info.links[0].source;
        assert_eq!(
            &text[usize::from(range.start())..usize::from(range.end())],
            "[Speed]"
        );
    }

    /// The target span of every link form and of `@see` covers the name
    /// alone, after the `*` decoration of a block doc and after a multibyte
    /// character earlier on the line.
    #[test]
    fn scan_target_spans_cover_the_name_alone() {
        let text =
            "/**\n * é [Speed], é [`veh.Gear`], é [the gear][Gear.PARK].\n * @see veh.Gear\n */";
        let info = scan(&doc_tokens(&[text]));
        let slice = |range: TextRange| &text[usize::from(range.start())..usize::from(range.end())];
        let targets: Vec<&str> = info.links.iter().map(|link| slice(link.target)).collect();
        assert_eq!(targets, ["Speed", "veh.Gear", "Gear.PARK"]);
        let sources: Vec<&str> = info.links.iter().map(|link| slice(link.source)).collect();
        assert_eq!(
            sources,
            ["[Speed]", "[`veh.Gear`]", "[the gear][Gear.PARK]"]
        );
        assert_eq!(slice(info.see[0].target), "veh.Gear");
        assert_eq!(info.see[0].target, info.see[0].source);
    }

    /// A line comment keeps the indentation after its one separating space,
    /// so an indented Markdown construct survives.
    #[test]
    fn scan_keeps_indentation_after_the_marker() {
        let info = scan(&doc_tokens(&["/// List:", "///   * item"]));
        assert_eq!(info.doc, "List:\n  * item");
    }

    #[test]
    fn plain_line_comment_is_the_body() {
        let info = scan(&doc_tokens(&["/// Vehicle speed over ground"]));
        assert_eq!(info.doc, "Vehicle speed over ground");
        assert!(info.labels.is_empty());
        assert_eq!(info.deprecated, None);
    }

    #[test]
    fn multiple_line_comments_join_with_newlines() {
        let info = scan(&doc_tokens(&["/// first line", "/// second line"]));
        assert_eq!(info.doc, "first line\nsecond line");
    }

    #[test]
    fn labels_tag_splits_on_commas() {
        let info = scan(&doc_tokens(&[
            "/// A tag",
            "/// @labels SAFETY(D), CALIBRATION",
        ]));
        assert_eq!(info.doc, "A tag");
        assert_eq!(info.labels, vec!["SAFETY(D)", "CALIBRATION"]);
    }

    #[test]
    fn deprecated_with_quoted_reason() {
        let info = scan(&doc_tokens(&[r#"/// @deprecated "use Speed instead""#]));
        assert_eq!(info.deprecated.as_deref(), Some("use Speed instead"));
        assert!(!info.deprecated_missing_reason());
    }

    #[test]
    fn deprecated_without_reason_is_flagged() {
        let info = scan(&doc_tokens(&["/// @deprecated"]));
        assert_eq!(info.deprecated.as_deref(), Some(""));
        assert!(
            info.deprecated_missing_reason(),
            "a bare @deprecated has no reason string (TYPL-405)"
        );
    }

    #[test]
    fn see_tag_is_passed_through_without_body_or_error() {
        let info = scan(&doc_tokens(&["/// @see veh.common.Torque"]));
        assert_eq!(info.doc, "");
        assert!(info.labels.is_empty());
        assert_eq!(info.deprecated, None);
    }

    #[test]
    fn block_comment_strips_the_star_gutter() {
        let info = scan(&doc_tokens(&["/**\n * Line one\n * Line two\n */"]));
        assert_eq!(info.doc, "Line one\nLine two");
    }

    /// An undecorated block loses only the indentation its continuation
    /// lines share: the rest is content, so an indented Markdown construct
    /// survives, as it does on a `///` line.
    #[test]
    fn undecorated_block_keeps_the_indentation_beyond_the_shared_indent() {
        let text = "/** First.\n    Second.\n      indented\n    [Speed] */";
        let info = scan(&doc_tokens(&[text]));
        assert_eq!(info.doc, "First.\nSecond.\n  indented\n[Speed]");
        let range = info.links[0].source;
        assert_eq!(
            &text[usize::from(range.start())..usize::from(range.end())],
            "[Speed]"
        );
    }

    #[test]
    fn tag_lookalike_is_an_unknown_tag() {
        // `@seealso` is not the `@see` tag — it is an unknown tag (TYPL-408),
        // removed from the prose body.
        let info = scan(&doc_tokens(&["/// @seealso not a tag"]));
        assert_eq!(info.doc, "");
        assert!(info.see.is_empty());
        assert_eq!(
            info.problems
                .iter()
                .map(|problem| problem.kind.clone())
                .collect::<Vec<_>>(),
            [TagProblemKind::Unknown("seealso".to_string())]
        );
    }

    #[test]
    fn an_at_sign_that_does_not_start_a_word_is_prose() {
        let info = scan(&doc_tokens(&["/// @[..10ms] is timing", "/// @ alone"]));
        assert_eq!(info.doc, "@[..10ms] is timing\n@ alone");
        assert!(info.problems.is_empty());
    }

    #[test]
    fn deprecated_and_labels_are_removed_from_the_body_without_problems() {
        let info = scan(&doc_tokens(&[
            "/// A speed.",
            "/// @labels SAFETY(D)",
            "/// @deprecated \"use Velocity\"",
        ]));
        assert_eq!(info.doc, "A speed.");
        assert_eq!(info.labels, ["SAFETY(D)"]);
        assert_eq!(info.deprecated.as_deref(), Some("use Velocity"));
        assert!(info.problems.is_empty());
    }
}
