//! Doc-comment tag scanning (typl language reference §14; docs/ROADMAP.md epic
//! E1.7b).
//!
//! A definition's doc comments are trivia tokens sitting before it (collected
//! by the `ridl_syntax::ast::HasDocComments` trait). This module strips the
//! comment markers, separates the prose body from the `@see` / `@labels` /
//! `@deprecated` tags (§14.2), and records whether `@deprecated` carried a
//! reason string. The checker turns the result into the IR `Decl` doc metadata
//! and raises TYPL-405 when a `@deprecated` reason is missing.
//!
//! Two documentation checks are deferred as debt (ADR-0007 decision 10):
//! TYPL-401 (`[TypeName]` reference resolution) needs a CommonMark reference
//! pass, and TYPL-402/403 (`@labels` vocabulary and combination validation)
//! need assurance profiles that do not exist in V1. `@see` is therefore an
//! unvalidated pass-through here, and `@labels` identifiers are carried through
//! to the IR unchecked (§14.3).

use ridl_syntax::SyntaxToken;

/// The parsed content of a definition's doc comments (typl §14).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct DocInfo {
    /// The prose body: every non-tag line with its comment markers stripped,
    /// joined by newlines and trimmed. Empty when the comment is only tags.
    pub doc: String,
    /// The `@labels` classification identifiers, in source order (§14.3),
    /// passed through to the IR unchanged.
    pub labels: Vec<String>,
    /// The `@deprecated` reason, when the tag is present. `Some("")` records a
    /// `@deprecated` with no reason string — the checker raises TYPL-405 for it
    /// while still marking the declaration deprecated.
    pub deprecated: Option<String>,
}

impl DocInfo {
    /// Whether a `@deprecated` tag was present but carried no reason string
    /// (TYPL-405).
    pub fn deprecated_missing_reason(&self) -> bool {
        matches!(&self.deprecated, Some(reason) if reason.is_empty())
    }
}

/// Scans the doc-comment tokens preceding a definition into a [`DocInfo`]
/// (typl §14). The tokens are in source order; both `///` line comments and
/// `/** ... */` block comments are accepted.
pub fn scan(tokens: &[SyntaxToken]) -> DocInfo {
    let mut info = DocInfo::default();
    let mut body: Vec<String> = Vec::new();
    for line in tokens.iter().flat_map(|token| comment_lines(token.text())) {
        let line = line.trim();
        if match_tag(line, "@see").is_some() {
            // §14.2: `@see` is an unvalidated pass-through; reference
            // resolution (TYPL-401) is deferred (ADR-0007 decision 10).
        } else if let Some(rest) = match_tag(line, "@labels") {
            for label in rest.split(',') {
                let label = label.trim();
                if !label.is_empty() {
                    info.labels.push(label.to_string());
                }
            }
        } else if let Some(rest) = match_tag(line, "@deprecated") {
            info.deprecated = Some(deprecated_reason(rest));
        } else {
            body.push(line.to_string());
        }
    }
    info.doc = body.join("\n").trim().to_string();
    info
}

/// Matches `tag` as a whole word at the start of `line`, returning the trimmed
/// remainder. `@see` matches `@see` and `@see Foo` but not `@seealso`.
fn match_tag<'a>(line: &'a str, tag: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(tag)?;
    if rest.is_empty() || rest.starts_with(char::is_whitespace) {
        Some(rest.trim_start())
    } else {
        None
    }
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

/// The marker-stripped lines of one doc-comment token. A `///` line comment
/// yields one line; a `/** ... */` block yields one line per source line, each
/// with a leading `*` gutter removed.
fn comment_lines(text: &str) -> Vec<String> {
    if let Some(rest) = text.strip_prefix("///") {
        return vec![rest.trim().to_string()];
    }
    if let Some(inner) = text
        .strip_prefix("/**")
        .and_then(|rest| rest.strip_suffix("*/"))
    {
        return inner
            .lines()
            .map(|line| line.trim().trim_start_matches('*').trim().to_string())
            .collect();
    }
    // Not a recognized doc-comment shape; stay total.
    vec![text.trim().to_string()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use ridl_syntax::{SyntaxKind, SyntaxNode};
    use rowan::GreenNodeBuilder;

    /// Builds standalone `DocComment` tokens from their source text, wrapped in
    /// a throwaway root node so the tokens are real `SyntaxToken`s.
    fn doc_tokens(texts: &[&str]) -> Vec<SyntaxToken> {
        let mut builder = GreenNodeBuilder::new();
        builder.start_node(SyntaxKind::SourceFile.into());
        for text in texts {
            builder.token(SyntaxKind::DocComment.into(), text);
        }
        builder.finish_node();
        let root = SyntaxNode::new_root(builder.finish());
        root.children_with_tokens()
            .filter_map(|element| element.into_token())
            .filter(|token| token.kind() == SyntaxKind::DocComment)
            .collect()
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

    #[test]
    fn tag_lookalike_is_kept_in_the_body() {
        // `@seealso` is not the `@see` tag — it stays in the prose body.
        let info = scan(&doc_tokens(&["/// @seealso not a tag"]));
        assert_eq!(info.doc, "@seealso not a tag");
    }
}
