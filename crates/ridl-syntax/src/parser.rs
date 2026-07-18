//! The hand-written recursive-descent parser for the E0 grammar
//! (docs/ROADMAP.md epic E0.3, ADR-0004 §2).
//!
//! The parser consumes the flat token stream from [`crate::lex`] and builds a
//! lossless rowan tree: every token — including whitespace and comments —
//! lands in the tree in source order, so [`Parse::syntax`] round-trips back to
//! the original source for both valid and broken input. On an unexpected token
//! the parser records a [`SyntaxError`] and wraps the token in an
//! [`Error`](SyntaxKind::Error) node rather than dropping it, so recovery never
//! loses text and never panics.

use rowan::{GreenNode, GreenNodeBuilder};

use crate::lexer::{Token, lex};
use crate::syntax_kind::{SyntaxKind, SyntaxNode};

/// A parse diagnostic: a message plus the byte offset into the source where the
/// problem was seen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    pub message: String,
    pub offset: usize,
}

/// The result of [`parse`]: the lossless green tree plus any diagnostics.
///
/// [`Parse`] compares by the identity of its green tree, which is what the
/// salsa query graph (epic E0.4) needs to decide whether a reparse changed
/// anything downstream.
#[derive(Debug, Clone)]
pub struct Parse {
    green: GreenNode,
    errors: Vec<SyntaxError>,
}

impl Parse {
    /// A fresh red-tree root over the parsed green tree.
    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    /// The diagnostics gathered while parsing, in source order.
    pub fn errors(&self) -> &[SyntaxError] {
        &self.errors
    }
}

impl PartialEq for Parse {
    fn eq(&self, other: &Self) -> bool {
        self.green == other.green
    }
}

impl Eq for Parse {}

/// Parses `input` into a lossless [`Parse`].
pub fn parse(input: &str) -> Parse {
    let mut parser = Parser::new(lex(input));
    parser.source_file();
    parser.finish()
}

/// The recursive-descent state: the token stream, a cursor, the tree builder,
/// and the diagnostics.
struct Parser<'a> {
    tokens: Vec<Token<'a>>,
    /// Byte offset of the start of each token, plus a final entry for the end
    /// of input; `offsets[i]` is the offset of `tokens[i]`.
    offsets: Vec<usize>,
    pos: usize,
    builder: GreenNodeBuilder<'static>,
    errors: Vec<SyntaxError>,
}

impl<'a> Parser<'a> {
    fn new(tokens: Vec<Token<'a>>) -> Self {
        let mut offsets = Vec::with_capacity(tokens.len() + 1);
        let mut acc = 0;
        for token in &tokens {
            offsets.push(acc);
            acc += token.text.len();
        }
        offsets.push(acc);
        Self {
            tokens,
            offsets,
            pos: 0,
            builder: GreenNodeBuilder::new(),
            errors: Vec::new(),
        }
    }

    fn finish(self) -> Parse {
        Parse {
            green: self.builder.finish(),
            errors: self.errors,
        }
    }

    // --- cursor over significant (non-trivia) tokens ---------------------

    /// Index of the next non-trivia token at or after the cursor.
    fn significant_pos(&self) -> usize {
        let mut i = self.pos;
        while self.tokens.get(i).is_some_and(|t| t.kind.is_trivia()) {
            i += 1;
        }
        i
    }

    /// Kind of the next significant token, or `None` at end of input.
    fn current(&self) -> Option<SyntaxKind> {
        self.tokens.get(self.significant_pos()).map(|t| t.kind)
    }

    fn at(&self, kind: SyntaxKind) -> bool {
        self.current() == Some(kind)
    }

    // --- token consumption (feeds the tree, keeps losslessness) ----------

    /// Attaches any pending trivia tokens to the current node, in source order.
    fn eat_trivia(&mut self) {
        while let Some(token) = self.tokens.get(self.pos) {
            if token.kind.is_trivia() {
                self.builder.token(token.kind.into(), token.text);
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    /// Consumes one significant token into the tree, flushing leading trivia
    /// first. Callers must have confirmed a token is present.
    fn bump(&mut self) {
        self.eat_trivia();
        if let Some(token) = self.tokens.get(self.pos) {
            self.builder.token(token.kind.into(), token.text);
            self.pos += 1;
        }
    }

    /// Consumes the current token if it is `kind`; reports otherwise.
    fn expect(&mut self, kind: SyntaxKind) {
        if self.at(kind) {
            self.bump();
        } else {
            self.error(format!("expected {kind:?}"));
        }
    }

    fn error(&mut self, message: String) {
        let offset = self.offsets[self.significant_pos()];
        self.errors.push(SyntaxError { message, offset });
    }

    /// Recovery: wrap the current unexpected token in an `Error` node and
    /// advance, so no text is dropped and the outer loop makes progress.
    fn err_and_bump(&mut self, message: &str) {
        self.error(message.to_string());
        self.builder.start_node(SyntaxKind::Error.into());
        self.bump();
        self.builder.finish_node();
    }

    // --- grammar productions --------------------------------------------

    /// `source_file = (type_decl | const_decl | trivia)*`
    fn source_file(&mut self) {
        self.builder.start_node(SyntaxKind::SourceFile.into());
        loop {
            self.eat_trivia();
            match self.current() {
                None => break,
                Some(SyntaxKind::TypeKw) => self.type_decl(),
                Some(SyntaxKind::ConstKw) => self.const_decl(),
                Some(_) => self.err_and_bump("unexpected token at top level"),
            }
        }
        self.builder.finish_node();
    }

    /// `type_decl = 'type' Name ':' unit_expr range?`
    fn type_decl(&mut self) {
        self.builder.start_node(SyntaxKind::TypeDecl.into());
        self.bump(); // 'type'
        self.name();
        self.expect(SyntaxKind::Colon);
        self.unit_expr();
        if self.at(SyntaxKind::LBracket) {
            self.range();
        }
        self.builder.finish_node();
    }

    /// `const_decl = 'const' Name ':' Ident '=' number`
    fn const_decl(&mut self) {
        self.builder.start_node(SyntaxKind::ConstDecl.into());
        self.bump(); // 'const'
        self.name();
        self.expect(SyntaxKind::Colon);
        self.expect(SyntaxKind::Ident); // the referenced type name
        self.expect(SyntaxKind::Eq);
        self.number();
        self.builder.finish_node();
    }

    /// A declared name — a single `Ident` wrapped so accessors can find it.
    fn name(&mut self) {
        self.builder.start_node(SyntaxKind::Name.into());
        self.expect(SyntaxKind::Ident);
        self.builder.finish_node();
    }

    /// `unit_expr = Ident (('/' | '.') Ident)*`
    fn unit_expr(&mut self) {
        self.builder.start_node(SyntaxKind::UnitExpr.into());
        self.expect(SyntaxKind::Ident);
        while self.at(SyntaxKind::Slash) || self.at(SyntaxKind::Dot) {
            self.bump(); // '/' or '.'
            self.expect(SyntaxKind::Ident);
        }
        self.builder.finish_node();
    }

    /// `range = '[' number '..' number ('step' number)? ']'`
    fn range(&mut self) {
        self.builder.start_node(SyntaxKind::Range.into());
        self.expect(SyntaxKind::LBracket);
        self.number();
        self.expect(SyntaxKind::DotDot);
        self.number();
        if self.at(SyntaxKind::StepKw) {
            self.bump(); // 'step'
            self.number();
        }
        self.expect(SyntaxKind::RBracket);
        self.builder.finish_node();
    }

    /// A numeric literal — an integer or float token wrapped in a `Literal`.
    fn number(&mut self) {
        self.builder.start_node(SyntaxKind::Literal.into());
        if self.at(SyntaxKind::IntNumber) || self.at(SyntaxKind::FloatNumber) {
            self.bump();
        } else {
            self.error("expected a number".to_string());
        }
        self.builder.finish_node();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../fixtures/walking_skeleton.typl");

    #[test]
    fn fixture_round_trips_losslessly() {
        let parse = parse(FIXTURE);
        assert_eq!(
            parse.syntax().text().to_string(),
            FIXTURE,
            "the tree text must reproduce the source byte for byte",
        );
        assert!(
            parse.errors().is_empty(),
            "the fixture must parse without errors, got: {:?}",
            parse.errors(),
        );
    }

    #[test]
    fn mangled_input_round_trips_and_reports_errors() {
        let input = "type 123 :: [";
        let parse = parse(input);
        assert_eq!(
            parse.syntax().text().to_string(),
            input,
            "recovery must not drop any text",
        );
        assert!(
            !parse.errors().is_empty(),
            "broken input must produce at least one error",
        );
    }

    #[test]
    fn parse_equality_is_green_node_identity() {
        assert_eq!(
            parse(FIXTURE),
            parse(FIXTURE),
            "parsing identical input twice must compare equal",
        );
        assert_ne!(
            parse(FIXTURE),
            parse("type X: m"),
            "parsing different input must not compare equal",
        );
    }
}
