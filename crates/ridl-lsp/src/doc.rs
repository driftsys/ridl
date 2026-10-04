//! The cursor inside a doc comment (typl §14, ADR-0026).
//!
//! A doc comment is a trivia token, so the parse tree gives the editor
//! features nothing to stand on inside one. This module reads the comment
//! run under the cursor the way the checker reads a carrier's docs
//! ([`scan`]), and answers two questions: which doc link or `@see` target
//! the cursor sits on ([`doc_link_at`]), and what the cursor is typing
//! inside the comment ([`in_doc_comment`]) — the start of a link after `[`,
//! or a tag after `@` at the start of a line.

use ridl_sem::docs::{LinkCandidate, scan};
use ridl_syntax::ast::{AstNode, SourceFile};
use ridl_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};
use rowan::{TextSize, TokenAtOffset};

/// What the cursor is typing inside a doc comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DocCursor {
    /// The text between the last unclosed `[` of the cursor's line and the
    /// cursor, with a leading backtick dropped — the partial link target
    /// (`""`, `"Ge"`, `"veh.common."`). `None` when no link is open.
    pub after_open_bracket: Option<String>,
    /// Whether the cursor's line, after its comment marker, holds exactly an
    /// `@` and a partial tag word up to the cursor (`@`, `@se`).
    pub at_line_start_at: bool,
}

/// The doc link or `@see` target whose span covers `offset`, read from the
/// doc-comment run the offset falls in. `None` outside a doc comment or on
/// prose.
pub(crate) fn doc_link_at(file: &SourceFile, offset: TextSize) -> Option<LinkCandidate> {
    let token = doc_token_at(file.syntax(), offset)?;
    let info = scan(&run_of(&token));
    info.links
        .into_iter()
        .chain(info.see)
        .find(|candidate| candidate.source.contains_inclusive(offset))
}

/// Every doc-comment run of `file`, in source order: the `DocComment` tokens
/// that only trivia separates, grouped as the carrier walk groups them.
pub(crate) fn doc_runs(file: &SourceFile) -> Vec<Vec<SyntaxToken>> {
    let mut runs = Vec::new();
    let mut current: Vec<SyntaxToken> = Vec::new();
    for token in file
        .syntax()
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
    {
        if token.kind() == SyntaxKind::DocComment {
            current.push(token);
        } else if !token.kind().is_trivia() && !current.is_empty() {
            runs.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        runs.push(current);
    }
    runs
}

/// What the cursor at `offset` is typing, when it sits inside a doc comment
/// (after the comment's first byte).
pub(crate) fn in_doc_comment(file: &SourceFile, offset: TextSize) -> Option<DocCursor> {
    let token = doc_token_at(file.syntax(), offset)?;
    if offset <= token.text_range().start() {
        return None;
    }
    let typed = &token.text()[..usize::from(offset - token.text_range().start())];
    let line = typed.rsplit('\n').next().unwrap_or(typed);
    let content = strip_marker(line);
    let trimmed = content.trim_start();
    let at_line_start_at = trimmed
        .strip_prefix('@')
        .is_some_and(|word| word.chars().all(|c| c.is_ascii_alphabetic()));
    let after_open_bracket = content
        .rfind('[')
        .filter(|&open| !content[open + 1..].contains(']'))
        .map(|open| content[open + 1..].trim_start_matches('`').to_string());
    Some(DocCursor {
        after_open_bracket,
        at_line_start_at,
    })
}

/// The line's text after its comment marker: `///` or `/**` at the start of
/// the comment, or the `*` decoration of a continued block line.
fn strip_marker(line: &str) -> &str {
    if let Some(rest) = line
        .strip_prefix("///")
        .or_else(|| line.strip_prefix("/**"))
    {
        return rest;
    }
    let rest = line.trim_start();
    rest.strip_prefix('*').unwrap_or(rest)
}

/// The `DocComment` token at `offset`: the one containing it, or at a token
/// boundary the one ending there, else the one starting there.
fn doc_token_at(root: &SyntaxNode, offset: TextSize) -> Option<SyntaxToken> {
    let is_doc = |token: &SyntaxToken| token.kind() == SyntaxKind::DocComment;
    match root.token_at_offset(offset) {
        TokenAtOffset::None => None,
        TokenAtOffset::Single(token) => is_doc(&token).then_some(token),
        TokenAtOffset::Between(left, right) => {
            if is_doc(&left) {
                Some(left)
            } else if is_doc(&right) {
                Some(right)
            } else {
                None
            }
        }
    }
}

/// The doc-comment run `token` belongs to: the `DocComment` tokens around
/// it that only trivia separates, in source order — the run the carrier
/// walk (`doc_comments_before`) attaches to the next declaration.
fn run_of(token: &SyntaxToken) -> Vec<SyntaxToken> {
    let mut run = Vec::new();
    let mut cursor = token.prev_sibling_or_token();
    while let Some(rowan::NodeOrToken::Token(previous)) = cursor {
        if !previous.kind().is_trivia() {
            break;
        }
        if previous.kind() == SyntaxKind::DocComment {
            run.push(previous.clone());
        }
        cursor = previous.prev_sibling_or_token();
    }
    run.reverse();
    run.push(token.clone());
    let mut cursor = token.next_sibling_or_token();
    while let Some(rowan::NodeOrToken::Token(next)) = cursor {
        if !next.kind().is_trivia() {
            break;
        }
        if next.kind() == SyntaxKind::DocComment {
            run.push(next.clone());
        }
        cursor = next.next_sibling_or_token();
    }
    run
}

#[cfg(test)]
mod tests {
    use super::*;
    use ridl_syntax::{Profile, parse};

    fn file(text: &str) -> SourceFile {
        SourceFile::cast(parse(text, Profile::Ridl).syntax()).expect("a source file")
    }

    fn cursor(text: &str, after: &str) -> Option<DocCursor> {
        let offset = text.find(after).expect("the needle is in the text") + after.len();
        in_doc_comment(&file(text), TextSize::from(offset as u32))
    }

    #[test]
    fn an_open_bracket_gives_the_partial_target() {
        let text = "package demo\n/// See [veh.co\ntype T : integer\n";
        assert_eq!(
            cursor(text, "[veh.co"),
            Some(DocCursor {
                after_open_bracket: Some("veh.co".to_string()),
                at_line_start_at: false,
            })
        );
        let text = "package demo\n/// See [`Sp\ntype T : integer\n";
        assert_eq!(
            cursor(text, "[`Sp").and_then(|c| c.after_open_bracket),
            Some("Sp".to_string()),
            "a leading backtick is dropped"
        );
    }

    #[test]
    fn a_closed_bracket_opens_no_link() {
        let text = "package demo\n/// See [A] and\ntype T : integer\n";
        assert_eq!(
            cursor(text, "[A] and").and_then(|c| c.after_open_bracket),
            None
        );
    }

    #[test]
    fn an_at_at_the_line_start_is_a_tag_position() {
        let text = "package demo\n/// @se\ntype T : integer\n";
        assert!(cursor(text, "@se").is_some_and(|c| c.at_line_start_at));
        let text = "package demo\n/** Doc.\n * @\n */\ntype T : integer\n";
        assert!(
            cursor(text, " * @").is_some_and(|c| c.at_line_start_at),
            "a block line's `*` decoration is stripped"
        );
        let text = "package demo\n/// mail @me\ntype T : integer\n";
        assert!(!cursor(text, "@me").is_some_and(|c| c.at_line_start_at));
    }

    #[test]
    fn outside_a_doc_comment_there_is_no_cursor() {
        let text = "package demo\n// See [A\ntype T : integer\n";
        assert_eq!(cursor(text, "[A"), None, "a plain comment");
        let text = "package demo\n/// Doc.\ntype T : integer\n";
        assert_eq!(cursor(text, "integer"), None, "code");
        assert_eq!(
            cursor(text, "package demo\n"),
            None,
            "the comment's first byte"
        );
    }

    #[test]
    fn the_link_under_the_cursor_is_found_in_its_run() {
        let text = "package demo\n/// One [A].\n/// @see B.c\ntype T : integer\n";
        let source = file(text);
        let at = |needle: &str| {
            doc_link_at(
                &source,
                TextSize::from(text.find(needle).expect("needle") as u32),
            )
        };
        assert_eq!(at("A].").map(|c| c.segments), Some(vec!["A".to_string()]));
        assert_eq!(
            at("B.c").map(|c| c.segments),
            Some(vec!["B".to_string(), "c".to_string()])
        );
        assert_eq!(at("One").map(|c| c.segments), None, "prose");
        assert_eq!(doc_runs(&source).len(), 1, "one run of two lines");
    }
}
