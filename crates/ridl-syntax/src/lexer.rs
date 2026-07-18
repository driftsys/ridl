//! The family lexer's E0 subset (docs/ROADMAP.md epic E0.2).

use logos::Logos;

use crate::syntax_kind::SyntaxKind;

/// A lexed token: its [`SyntaxKind`] and the exact source slice it covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token<'a> {
    pub kind: SyntaxKind,
    pub text: &'a str,
}

/// The `logos`-derived token set, private to this module. [`lex`] maps each
/// variant into [`SyntaxKind`] — the family lexer's stable public interface —
/// so this set can grow with the rest of the family keyword registry in E1
/// without disturbing consumers.
#[derive(Logos, Debug, Clone, Copy, PartialEq, Eq)]
enum RawToken {
    #[token("type")]
    TypeKw,
    #[token("const")]
    ConstKw,
    #[token("step")]
    StepKw,

    // Identifiers: start with a letter, digits permitted after the first
    // character, underscore only inside SCREAMING_SNAKE names (typl
    // reference §2.3). One token kind covers CamelCase, camelCase, and
    // SCREAMING_SNAKE — the checker distinguishes the conventions later.
    // Keyword literals above take precedence over this regex on exact
    // matches (logos: equal-length match, literal is more specific).
    #[regex("[A-Za-z][A-Za-z0-9_]*")]
    Ident,

    // Floats must contain a decimal point (typl reference §2.5); no
    // scientific notation. Declared so the longer match (digits '.'
    // digits) wins over the bare-digits `IntNumber` pattern — `0.0` lexes
    // whole rather than as `IntNumber` `0` followed by a stray `.0`, and
    // `0.0..250.0` lexes as `FloatNumber DotDot FloatNumber` rather than
    // the float regex swallowing the range dots.
    #[regex(r"[0-9]+\.[0-9]+")]
    FloatNumber,

    // Integers: decimal digits only in E0. Signed literals (a leading
    // `-`) are deferred to E1's full grammar — a bare `-` before a number
    // would otherwise be ambiguous with a range's lower bound
    // (`[-40..125]` vs. `[0..10]`), and the E0 walking-skeleton fixture
    // has no negative literals to force that call now.
    #[regex("[0-9]+")]
    IntNumber,

    #[token(":")]
    Colon,
    #[token("=")]
    Eq,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("..")]
    DotDot,
    #[token(".")]
    Dot,
    #[token("/")]
    Slash,
    #[token(",")]
    Comma,

    #[regex(r"[ \t\r\n]+")]
    Whitespace,

    // `//` line comments run to end of line. Doc comments do not exist
    // yet in E0 (typl reference §14 lands with the full grammar) — `///`
    // just lexes as an ordinary line comment here.
    #[regex("//[^\n]*", allow_greedy = true)]
    LineComment,
}

impl From<RawToken> for SyntaxKind {
    fn from(token: RawToken) -> Self {
        match token {
            RawToken::TypeKw => SyntaxKind::TypeKw,
            RawToken::ConstKw => SyntaxKind::ConstKw,
            RawToken::StepKw => SyntaxKind::StepKw,
            RawToken::Ident => SyntaxKind::Ident,
            RawToken::IntNumber => SyntaxKind::IntNumber,
            RawToken::FloatNumber => SyntaxKind::FloatNumber,
            RawToken::Colon => SyntaxKind::Colon,
            RawToken::Eq => SyntaxKind::Eq,
            RawToken::LBracket => SyntaxKind::LBracket,
            RawToken::RBracket => SyntaxKind::RBracket,
            RawToken::DotDot => SyntaxKind::DotDot,
            RawToken::Dot => SyntaxKind::Dot,
            RawToken::Slash => SyntaxKind::Slash,
            RawToken::Comma => SyntaxKind::Comma,
            RawToken::Whitespace => SyntaxKind::Whitespace,
            RawToken::LineComment => SyntaxKind::LineComment,
        }
    }
}

/// Lexes `input` into a flat token stream.
///
/// Total: concatenating `token.text` for every returned token reproduces
/// `input` byte for byte. Input the family lexer does not recognise becomes
/// `SyntaxKind::Error` tokens rather than aborting the lex.
pub fn lex(input: &str) -> Vec<Token<'_>> {
    RawToken::lexer(input)
        .spanned()
        .map(|(result, span)| Token {
            kind: result.map_or(SyntaxKind::Error, SyntaxKind::from),
            text: &input[span],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn concat(tokens: &[Token<'_>]) -> String {
        tokens.iter().map(|t| t.text).collect()
    }

    #[test]
    fn fixture_round_trips_with_no_errors() {
        let input = include_str!("../fixtures/walking_skeleton.typl");
        let tokens = lex(input);
        assert_eq!(concat(&tokens), input, "token texts must reproduce input");
        assert!(
            tokens.iter().all(|t| t.kind != SyntaxKind::Error),
            "fixture must lex with zero Error tokens, got: {tokens:?}"
        );
    }

    #[test]
    fn range_dots_do_not_get_swallowed_by_float_regex() {
        let tokens = lex("0.0..250.0");
        let kinds: Vec<_> = tokens.iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            vec![
                SyntaxKind::FloatNumber,
                SyntaxKind::DotDot,
                SyntaxKind::FloatNumber,
            ]
        );
    }

    #[test]
    fn slash_separated_unit_lexes_as_ident_slash_ident() {
        let tokens = lex("km/h");
        let kinds: Vec<_> = tokens.iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            vec![SyntaxKind::Ident, SyntaxKind::Slash, SyntaxKind::Ident]
        );
    }

    #[test]
    fn line_comment_is_one_token() {
        let tokens = lex("// x");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, SyntaxKind::LineComment);
        assert_eq!(tokens[0].text, "// x");
    }

    #[test]
    fn unknown_byte_yields_error_and_still_round_trips() {
        let input = "$";
        let tokens = lex(input);
        assert_eq!(concat(&tokens), input);
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, SyntaxKind::Error);
    }

    #[test]
    fn keywords_lex_distinctly_from_ident() {
        let tokens = lex("type const step notAKeyword");
        let kinds: Vec<_> = tokens
            .iter()
            .filter(|t| t.kind != SyntaxKind::Whitespace)
            .map(|t| t.kind)
            .collect();
        assert_eq!(
            kinds,
            vec![
                SyntaxKind::TypeKw,
                SyntaxKind::ConstKw,
                SyntaxKind::StepKw,
                SyntaxKind::Ident,
            ]
        );
    }
}
