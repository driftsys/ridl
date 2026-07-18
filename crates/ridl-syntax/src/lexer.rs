//! The family lexer (docs/ROADMAP.md epic E1.1, typl reference §1.4 and §2).
//!
//! One lexer serves the whole RIDL family. It recognises the shared token set —
//! the reserved-word registry (§1.4), identifiers, numeric, string, regex and
//! duration literals, punctuation, and the comment forms — and it is total:
//! concatenating the text of every returned token reproduces the input byte for
//! byte, so a rowan tree built from these tokens round-trips (ADR-0004 §2).
//!
//! `logos` lexes the raw stream; two source-driven refinements sit on top,
//! because they depend on more than a single token's bytes:
//!
//! - **Regex literals** (§2.7). A `/` whose previous significant token is `=`
//!   begins a regex literal. It is scanned from the source directly and the raw
//!   stream inside it is discarded by re-lexing from the literal's end, so the
//!   way `logos` happened to tokenise the pattern bytes does not matter.
//! - **Duration literals** (§2.8). A number immediately followed by a UCUM time
//!   atom (`us`, `ms`, `s`, `min`, `h`) is merged into one `Duration` token. The
//!   typl profile rejects durations later (TYPL-302); the family lexer still
//!   recognises them, because timing belongs to the other profiles.
//!
//! Identifiers are mapped against the family registry ([`crate::keywords`])
//! after lexing: a used keyword becomes its keyword kind, any other registry
//! word becomes [`SyntaxKind::ReservedWord`], everything else stays
//! [`SyntaxKind::Ident`].

use logos::Logos;

use crate::keywords;
use crate::syntax_kind::SyntaxKind;

/// A lexed token: its [`SyntaxKind`] and the exact source slice it covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token<'a> {
    pub kind: SyntaxKind,
    pub text: &'a str,
}

/// The `logos`-derived token set, private to this module. [`lex`] maps each
/// variant into [`SyntaxKind`] — the family lexer's stable public interface —
/// and applies the regex, duration, and keyword refinements on top.
#[derive(Logos, Debug, Clone, Copy, PartialEq, Eq)]
enum RawToken {
    // Identifiers: start with a letter, digits permitted after the first
    // character, underscore only inside SCREAMING_SNAKE names (typl reference
    // §2.3). One token kind covers CamelCase, camelCase, and SCREAMING_SNAKE;
    // keyword mapping (typl reference §1.4) happens after lexing.
    #[regex("[A-Za-z][A-Za-z0-9_]*")]
    Ident,

    // Floats must contain a decimal point (typl reference §2.5); no scientific
    // notation. The longer match (digits '.' digits) wins over the bare-digits
    // `IntNumber` pattern, and `0.0..250.0` lexes as float–dotdot–float because
    // the pattern needs a digit after the dot.
    #[regex(r"[0-9]+\.[0-9]+")]
    FloatNumber,

    // Integers: decimal digits (typl reference §2.4). Leading zeros lex as one
    // token here; the parser flags them (FORM-005). A negative literal is a
    // separate `Minus` token followed by digits — the parser combines them.
    #[regex("[0-9]+")]
    IntNumber,

    // String literals (typl reference §2.6): RFC 8259 escapes, no raw newline.
    // The callback scans to the closing quote; an unterminated string is an
    // error token to end of line.
    #[token("\"", lex_string)]
    Str,

    // Doc comments (typl reference §14): `///` line form and `/** ... */` block
    // form. The line form outranks the ordinary `//` line comment.
    #[regex(r"///[^\n]*", priority = 5, allow_greedy = true)]
    DocCommentLine,

    // Line comment `//` to end of line (typl reference §13).
    #[regex(r"//[^\n]*", priority = 4, allow_greedy = true)]
    LineComment,

    // Block comment `/* ... */`, non-nesting (typl reference §13). The callback
    // reports whether it is a doc comment (`/**`) and stops at the first `*/`.
    #[token("/*", lex_block_comment)]
    BlockComment(bool),

    #[token(":")]
    Colon,
    #[token("=")]
    Eq,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("..")]
    DotDot,
    #[token(".")]
    Dot,
    #[token("/")]
    Slash,
    #[token(",")]
    Comma,
    #[token("?")]
    Question,
    #[token(";")]
    Semicolon,
    #[token("@")]
    At,
    #[token("|")]
    Pipe,
    #[token("%")]
    Percent,
    #[token("-")]
    Minus,

    #[regex(r"[ \t\r\n]+")]
    Whitespace,
}

impl From<RawToken> for SyntaxKind {
    fn from(token: RawToken) -> Self {
        match token {
            RawToken::Ident => SyntaxKind::Ident,
            RawToken::FloatNumber => SyntaxKind::FloatNumber,
            RawToken::IntNumber => SyntaxKind::IntNumber,
            RawToken::Str => SyntaxKind::String,
            RawToken::DocCommentLine => SyntaxKind::DocComment,
            RawToken::LineComment => SyntaxKind::LineComment,
            RawToken::BlockComment(is_doc) => {
                if is_doc {
                    SyntaxKind::DocComment
                } else {
                    SyntaxKind::BlockComment
                }
            }
            RawToken::Colon => SyntaxKind::Colon,
            RawToken::Eq => SyntaxKind::Eq,
            RawToken::LBracket => SyntaxKind::LBracket,
            RawToken::RBracket => SyntaxKind::RBracket,
            RawToken::LBrace => SyntaxKind::LBrace,
            RawToken::RBrace => SyntaxKind::RBrace,
            RawToken::LParen => SyntaxKind::LParen,
            RawToken::RParen => SyntaxKind::RParen,
            RawToken::DotDot => SyntaxKind::DotDot,
            RawToken::Dot => SyntaxKind::Dot,
            RawToken::Slash => SyntaxKind::Slash,
            RawToken::Comma => SyntaxKind::Comma,
            RawToken::Question => SyntaxKind::Question,
            RawToken::Semicolon => SyntaxKind::Semicolon,
            RawToken::At => SyntaxKind::At,
            RawToken::Pipe => SyntaxKind::Pipe,
            RawToken::Percent => SyntaxKind::Percent,
            RawToken::Minus => SyntaxKind::Minus,
            RawToken::Whitespace => SyntaxKind::Whitespace,
        }
    }
}

/// A raw token with its byte span, before duration and keyword refinement.
struct Raw {
    kind: SyntaxKind,
    start: usize,
    end: usize,
}

/// Lexes `input` into a flat token stream.
///
/// Total: concatenating `token.text` for every returned token reproduces
/// `input` byte for byte. Input the family lexer does not recognise becomes
/// [`SyntaxKind::Error`] tokens rather than aborting the lex.
pub fn lex(input: &str) -> Vec<Token<'_>> {
    let raws = raw_tokens(input);
    refine(input, &raws)
}

/// Runs `logos` and lifts regex literals out of the raw stream (typl reference
/// §2.7). On a regex literal the scan is driven from the source and lexing
/// resumes from the literal's end, so the pattern bytes are never mis-tokenised
/// as slashes, identifiers, or a stray line comment.
fn raw_tokens(input: &str) -> Vec<Raw> {
    let mut out: Vec<Raw> = Vec::new();
    let mut prev_significant: Option<SyntaxKind> = None;
    let mut base = 0usize;
    'relex: loop {
        let mut lexer = RawToken::lexer(&input[base..]);
        while let Some(result) = lexer.next() {
            let span = lexer.span();
            let start = base + span.start;
            let end = base + span.end;
            let kind = result.map_or(SyntaxKind::Error, SyntaxKind::from);

            if kind == SyntaxKind::Slash && prev_significant == Some(SyntaxKind::Eq) {
                let (regex_kind, regex_end) = scan_regex(input, start);
                out.push(Raw {
                    kind: regex_kind,
                    start,
                    end: regex_end,
                });
                prev_significant = Some(regex_kind);
                base = regex_end;
                continue 'relex;
            }

            out.push(Raw { kind, start, end });
            if !kind.is_trivia() {
                prev_significant = Some(kind);
            }
        }
        break;
    }
    out
}

/// Scans a regex literal that begins at the `/` at byte `start`. Returns
/// [`SyntaxKind::Regex`] and the byte just past the closing `/`, or
/// [`SyntaxKind::Error`] and the end of the line when the literal does not
/// terminate on its own line (typl reference §2.7 — a regex cannot contain a
/// raw newline).
fn scan_regex(input: &str, start: usize) -> (SyntaxKind, usize) {
    let bytes = input.as_bytes();
    let mut i = start + 1; // past the opening '/'
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => {
                if i + 1 >= bytes.len() {
                    return (SyntaxKind::Error, bytes.len());
                }
                if bytes[i + 1] == b'\n' {
                    return (SyntaxKind::Error, i + 1);
                }
                i += 2;
            }
            b'/' => return (SyntaxKind::Regex, i + 1),
            b'\n' => return (SyntaxKind::Error, i),
            _ => i += 1,
        }
    }
    (SyntaxKind::Error, bytes.len())
}

/// Merges duration literals and maps identifiers against the family registry.
fn refine<'a>(input: &'a str, raws: &[Raw]) -> Vec<Token<'a>> {
    let mut out = Vec::with_capacity(raws.len());
    let mut i = 0;
    while i < raws.len() {
        let raw = &raws[i];

        // Duration: a number immediately followed by a UCUM time atom (typl
        // reference §2.8). "Immediately" means the very next token — whitespace
        // would appear as a separate trivia token in between.
        if matches!(raw.kind, SyntaxKind::IntNumber | SyntaxKind::FloatNumber)
            && let Some(next) = raws.get(i + 1)
            && next.kind == SyntaxKind::Ident
            && is_time_atom(&input[next.start..next.end])
        {
            out.push(Token {
                kind: SyntaxKind::Duration,
                text: &input[raw.start..next.end],
            });
            i += 2;
            continue;
        }

        let text = &input[raw.start..raw.end];
        let kind = if raw.kind == SyntaxKind::Ident {
            keywords::typl_keyword(text).unwrap_or(if keywords::is_reserved(text) {
                SyntaxKind::ReservedWord
            } else {
                SyntaxKind::Ident
            })
        } else {
            raw.kind
        };
        out.push(Token { kind, text });
        i += 1;
    }
    out
}

/// The UCUM time atoms a duration literal may use (typl reference §2.8).
fn is_time_atom(text: &str) -> bool {
    matches!(text, "us" | "ms" | "s" | "min" | "h")
}

/// Scans a string literal after the opening `"` (typl reference §2.6). Returns
/// `Ok` on the closing quote; an unterminated string (a raw newline or the end
/// of input before the close) consumes to end of line and becomes an error
/// token.
fn lex_string(lex: &mut logos::Lexer<RawToken>) -> Result<(), ()> {
    let bytes = lex.remainder().as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\n' => {
                lex.bump(i);
                return Err(());
            }
            b'\\' => {
                if i + 1 >= bytes.len() || bytes[i + 1] == b'\n' {
                    // Dangling backslash at end of line or input: unterminated.
                    lex.bump(i + 1);
                    return Err(());
                }
                i += 2;
            }
            b'"' => {
                lex.bump(i + 1);
                return Ok(());
            }
            _ => i += 1,
        }
    }
    lex.bump(bytes.len());
    Err(())
}

/// Scans a block comment after the opening `/*` (typl reference §13). Non-
/// nesting: stops at the first `*/`. Returns whether it is a doc comment
/// (`/** ... */`, typl reference §14); an unterminated block comment consumes
/// the rest of the input and becomes an error token.
fn lex_block_comment(lex: &mut logos::Lexer<RawToken>) -> Result<bool, ()> {
    let rest = lex.remainder();
    // A doc comment starts `/**` but the empty comment `/**/` does not.
    let is_doc = rest.starts_with('*') && !rest.starts_with("*/");
    match rest.find("*/") {
        Some(idx) => {
            lex.bump(idx + 2);
            Ok(is_doc)
        }
        None => {
            lex.bump(rest.len());
            Err(())
        }
    }
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

    /// The kinds of the significant (non-trivia) tokens, in order.
    fn significant_kinds(input: &str) -> Vec<SyntaxKind> {
        lex(input)
            .iter()
            .filter(|t| !t.kind.is_trivia())
            .map(|t| t.kind)
            .collect()
    }

    // Step (a): every used keyword lexes to its variant; every reserved word to
    // ReservedWord.
    #[test]
    fn every_used_keyword_lexes_to_its_variant() {
        for word in keywords::FAMILY_RESERVED {
            let tokens = lex(word);
            assert_eq!(tokens.len(), 1, "`{word}` must lex to one token");
            let expected = keywords::typl_keyword(word).unwrap_or(SyntaxKind::ReservedWord);
            assert_eq!(tokens[0].kind, expected, "`{word}` lexed to the wrong kind");
        }
    }

    #[test]
    fn reserved_word_is_not_an_identifier() {
        assert_eq!(significant_kinds("signal"), vec![SyntaxKind::ReservedWord]);
        assert_eq!(significant_kinds("model"), vec![SyntaxKind::ReservedWord]);
        assert_eq!(significant_kinds("Speed"), vec![SyntaxKind::Ident]);
    }

    // Step (c): `0.0..250.0` lexes float–dotdot–float (kept from E0, extended).
    #[test]
    fn float_range_lexes_as_float_dotdot_float() {
        assert_eq!(
            significant_kinds("0.0..250.0"),
            vec![
                SyntaxKind::FloatNumber,
                SyntaxKind::DotDot,
                SyntaxKind::FloatNumber,
            ]
        );
    }

    // Step (d): `km/h` is ident–slash–ident; a regex literal after `=` is one
    // Regex token.
    #[test]
    fn slash_in_unit_is_not_a_regex() {
        assert_eq!(
            significant_kinds("km/h"),
            vec![SyntaxKind::Ident, SyntaxKind::Slash, SyntaxKind::Ident]
        );
    }

    #[test]
    fn regex_literal_after_eq_is_one_token() {
        let tokens = lex(r"const V = /a\/b/");
        let regex: Vec<_> = tokens
            .iter()
            .filter(|t| t.kind == SyntaxKind::Regex)
            .collect();
        assert_eq!(regex.len(), 1, "one regex token expected: {tokens:?}");
        assert_eq!(regex[0].text, r"/a\/b/");
    }

    #[test]
    fn unterminated_regex_is_error_to_end_of_line() {
        let tokens = lex("const V = /abc\n");
        let last_significant = tokens
            .iter()
            .rev()
            .find(|t| !t.kind.is_trivia())
            .expect("a significant token");
        assert_eq!(last_significant.kind, SyntaxKind::Error);
        assert_eq!(last_significant.text, "/abc");
    }

    // Step (e): durations lex as one Duration; a bare atom is an Ident.
    #[test]
    fn durations_lex_as_one_token() {
        assert_eq!(significant_kinds("10ms"), vec![SyntaxKind::Duration]);
        assert_eq!(significant_kinds("500us"), vec![SyntaxKind::Duration]);
        assert_eq!(significant_kinds("1s"), vec![SyntaxKind::Duration]);
        assert_eq!(significant_kinds("1min"), vec![SyntaxKind::Duration]);
        assert_eq!(significant_kinds("1.5h"), vec![SyntaxKind::Duration]);
    }

    #[test]
    fn bare_time_atom_is_an_identifier() {
        assert_eq!(significant_kinds("min"), vec![SyntaxKind::Ident]);
        // A space breaks the adjacency a duration needs.
        assert_eq!(
            significant_kinds("10 ms"),
            vec![SyntaxKind::IntNumber, SyntaxKind::Ident]
        );
    }

    // Step (f): strings with escapes, and unterminated strings.
    #[test]
    fn string_with_escape_is_one_token() {
        let tokens = lex(r#""a\nb""#);
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, SyntaxKind::String);
        assert_eq!(tokens[0].text, r#""a\nb""#);
    }

    #[test]
    fn unterminated_string_is_error_to_end_of_line() {
        let tokens = lex("\"abc\n");
        assert_eq!(tokens[0].kind, SyntaxKind::Error);
        assert_eq!(tokens[0].text, "\"abc");
        // The newline survives as its own trivia token (losslessness).
        assert_eq!(tokens[1].kind, SyntaxKind::Whitespace);
    }

    // Step (g): block comments do not nest.
    #[test]
    fn block_comment_is_one_token() {
        let tokens = lex("/* block */");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, SyntaxKind::BlockComment);
    }

    #[test]
    fn block_comment_ends_at_first_close() {
        let tokens = lex("/* /* */");
        assert_eq!(tokens.len(), 1, "no nesting: {tokens:?}");
        assert_eq!(tokens[0].kind, SyntaxKind::BlockComment);
        assert_eq!(tokens[0].text, "/* /* */");
    }

    #[test]
    fn doc_comments_are_distinct_from_plain_comments() {
        assert_eq!(significant_kinds("/// doc"), Vec::<SyntaxKind>::new());
        let doc_line = lex("/// doc");
        assert_eq!(doc_line[0].kind, SyntaxKind::DocComment);
        let doc_block = lex("/** doc */");
        assert_eq!(doc_block[0].kind, SyntaxKind::DocComment);
        let plain = lex("// doc");
        assert_eq!(plain[0].kind, SyntaxKind::LineComment);
    }

    // Step (h): leading zeros lex as one IntNumber; flagging is the parser's job.
    #[test]
    fn leading_zeros_lex_as_one_integer() {
        let tokens = lex("042");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, SyntaxKind::IntNumber);
        assert_eq!(tokens[0].text, "042");
    }

    #[test]
    fn minus_is_its_own_token() {
        assert_eq!(
            significant_kinds("-40"),
            vec![SyntaxKind::Minus, SyntaxKind::IntNumber]
        );
    }

    #[test]
    fn all_new_punctuation_lexes() {
        assert_eq!(
            significant_kinds("{}()?;@|%"),
            vec![
                SyntaxKind::LBrace,
                SyntaxKind::RBrace,
                SyntaxKind::LParen,
                SyntaxKind::RParen,
                SyntaxKind::Question,
                SyntaxKind::Semicolon,
                SyntaxKind::At,
                SyntaxKind::Pipe,
                SyntaxKind::Percent,
            ]
        );
    }
}
