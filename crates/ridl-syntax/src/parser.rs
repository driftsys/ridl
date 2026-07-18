//! The hand-written recursive-descent parser for the typl grammar
//! (docs/ROADMAP.md epic E1.2b, ADR-0004 §2, typl reference Appendix E).
//!
//! The parser consumes the flat token stream from [`crate::lex`] and builds a
//! lossless rowan tree whose nodes match `typl.ungram` exactly: every token —
//! including whitespace and comments — lands in the tree in source order, so
//! [`Parse::syntax`] round-trips back to the original source for both valid
//! and broken input. On an unexpected token the parser records a
//! [`SyntaxError`] and wraps the token in an [`ErrorNode`](SyntaxKind::ErrorNode)
//! rather than dropping it, so recovery never loses text and never panics.
//! (Resynchronizing recovery is task E1.2c; this parser advances one token at
//! a time.)
//!
//! Trivia placement: leading trivia — including doc comments — is flushed at
//! the parent level before a child node starts, so a doc comment sits as a
//! sibling immediately before the definition it documents
//! (`ast::HasDocComments` reads it from there).
//!
//! # The `[a..b]` bound policy
//!
//! `[3..6]` has two legal tree shapes: a pair of scalar `Literal` children
//! (a value range) or a `Bound` child (a length bound). The parser emits a
//! `Bound` iff the constraint attaches to a `string`/`bytes` backing and both
//! endpoints are integer-shaped (no float token); it emits scalar `Literal`s
//! otherwise — including when a `step` clause follows, which only the scalar
//! shape can carry. A single literal with no `..` (`[17]`) is always a
//! `Bound`; a float endpoint is always scalar.
//!
//! # Grammar over-approximations
//!
//! The grammar deliberately accepts more than the typl reference allows —
//! constraints on any primitive in field position, arbitrary map key types,
//! identifier literals in value positions, `step` on open ranges, non-integer
//! literals in bounds and reserved entries, repeated separator commas with no
//! member between them (`,,,`), and the `internal`/`error` modifiers on every
//! definition kind. The checker narrows these later (TYPL-2xx); the parser
//! must not reject them.
//!
//! # Profile boundary
//!
//! A `Duration` token or a stray `@` anywhere in a `.typl` parse emits
//! **TYPL-302** (typl reference §2.8) and parsing continues. Leading zeros in
//! an integer literal emit **FORM-005**. Every [`SyntaxError`] carries its
//! diagnostic code; the coded `Diagnostic` model consumes it in task E1.10.

use rowan::{GreenNode, GreenNodeBuilder, TextRange, TextSize};

use crate::lexer::{Token, lex};
use crate::syntax_kind::{SyntaxKind, SyntaxNode};

/// A parse diagnostic: a message, the stable diagnostic code it will carry in
/// the coded model (`FORM-…` per ADR-0007 decision 2, `TYPL-302` per typl
/// reference §16.4), and the source range of the offending token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    pub message: String,
    pub code: &'static str,
    pub range: TextRange,
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

/// Whether `kind` can be the value token of a `Literal` (`typl.ungram` rule
/// `Literal`, minus the optional leading `-`).
fn is_value_token(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::IntNumber
            | SyntaxKind::FloatNumber
            | SyntaxKind::String
            | SyntaxKind::TrueKw
            | SyntaxKind::FalseKw
            | SyntaxKind::Regex
            | SyntaxKind::Ident
    )
}

/// The deepest field-type nesting the parser follows before it stops
/// recursing — far beyond any real schema, small enough that a pathological
/// input cannot overflow the stack.
const MAX_TYPE_DEPTH: usize = 128;

/// The recursive-descent state: the token stream, a cursor, the tree builder,
/// and the diagnostics.
struct Parser<'a> {
    tokens: Vec<Token<'a>>,
    /// Byte offset of the start of each token, plus a final entry for the end
    /// of input; `offsets[i]` is the offset of `tokens[i]`.
    offsets: Vec<usize>,
    pos: usize,
    /// Current field-type nesting depth, bounded by [`MAX_TYPE_DEPTH`].
    depth: usize,
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
            depth: 0,
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

    /// Kind of the `n`-th significant token ahead (`nth(0)` is [`current`]).
    fn nth(&self, n: usize) -> Option<SyntaxKind> {
        let mut i = self.pos;
        let mut seen = 0;
        loop {
            let token = self.tokens.get(i)?;
            if !token.kind.is_trivia() {
                if seen == n {
                    return Some(token.kind);
                }
                seen += 1;
            }
            i += 1;
        }
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

    /// Flushes leading trivia to the currently open node, then starts a child
    /// node — so a definition's doc comment stays a sibling of the definition
    /// rather than its first child.
    fn start(&mut self, kind: SyntaxKind) {
        self.eat_trivia();
        self.builder.start_node(kind.into());
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

    /// Consumes the current token if it is `kind`; reports FORM-101 otherwise.
    fn expect(&mut self, kind: SyntaxKind) {
        if self.at(kind) {
            self.bump();
        } else {
            self.error_at_current("FORM-101", format!("expected {kind:?}"));
        }
    }

    /// Records an error spanning the current significant token (or the end of
    /// input).
    fn error_at_current(&mut self, code: &'static str, message: String) {
        let i = self.significant_pos();
        let start = self.offsets[i];
        let end = self.tokens.get(i).map_or(start, |t| start + t.text.len());
        self.errors.push(SyntaxError {
            message,
            code,
            range: TextRange::new(TextSize::new(start as u32), TextSize::new(end as u32)),
        });
    }

    /// Recovery: report the current unexpected token — TYPL-302 for the
    /// profile-boundary tokens (duration literals and the `@` timing sigil),
    /// FORM-102 otherwise — wrap it in an [`SyntaxKind::ErrorNode`], and
    /// advance, so no text is dropped and the enclosing loop makes progress.
    fn err_and_bump(&mut self, context: &str) {
        let (code, message) = match self.current() {
            Some(SyntaxKind::Duration) => {
                ("TYPL-302", "duration literal in typl context".to_string())
            }
            Some(SyntaxKind::At) => ("TYPL-302", "timing annotation in typl context".to_string()),
            _ => ("FORM-102", format!("unexpected token {context}")),
        };
        self.error_at_current(code, message);
        self.start(SyntaxKind::ErrorNode);
        self.bump();
        self.builder.finish_node();
    }

    // --- grammar productions --------------------------------------------

    /// `SourceFile = PackageDecl? Import* Definition*`
    ///
    /// A missing `package` emits FORM-104 and parsing continues; extra
    /// `package` declarations parse as further `PackageDecl` nodes (TYPL-001
    /// is checker scope). Imports and definitions are accepted in any order —
    /// ordering is surface convention, not tree shape.
    fn source_file(&mut self) {
        self.builder.start_node(SyntaxKind::SourceFile.into());
        self.eat_trivia();
        if self.at(SyntaxKind::PackageKw) {
            self.package_decl();
        } else {
            self.error_at_current("FORM-104", "missing `package` declaration".to_string());
        }
        loop {
            self.eat_trivia();
            match self.current() {
                None => break,
                Some(SyntaxKind::PackageKw) => self.package_decl(),
                Some(SyntaxKind::ImportKw) => self.import(),
                Some(
                    SyntaxKind::InternalKw
                    | SyntaxKind::ErrorKw
                    | SyntaxKind::TypeKw
                    | SyntaxKind::ConstKw
                    | SyntaxKind::StructKw
                    | SyntaxKind::EnumKw
                    | SyntaxKind::EnumsetKw
                    | SyntaxKind::UnionKw,
                ) => self.definition(),
                Some(_) => self.err_and_bump("at top level"),
            }
        }
        self.builder.finish_node();
    }

    /// `PackageDecl = 'package' QualifiedName`
    fn package_decl(&mut self) {
        self.start(SyntaxKind::PackageDecl);
        self.bump(); // 'package'
        self.qualified_name();
        self.builder.finish_node();
    }

    /// `Import = 'import' QualifiedName ('as' alias:Name)?`
    fn import(&mut self) {
        self.start(SyntaxKind::Import);
        self.bump(); // 'import'
        self.qualified_name();
        if self.at(SyntaxKind::AsKw) {
            self.bump();
            self.name();
        }
        self.builder.finish_node();
    }

    /// `Definition = TypeDef | ConstDef | StructDef | EnumDef | EnumSetDef |
    /// UnionDef`, each with optional leading `internal`/`error` modifiers.
    /// The definition keyword is found by looking past the modifiers; a
    /// modifier with no definition keyword after it is an unexpected token.
    fn definition(&mut self) {
        let mut n = 0;
        while matches!(
            self.nth(n),
            Some(SyntaxKind::InternalKw | SyntaxKind::ErrorKw)
        ) {
            n += 1;
        }
        match self.nth(n) {
            Some(SyntaxKind::TypeKw) => self.type_def(),
            Some(SyntaxKind::ConstKw) => self.const_def(),
            Some(SyntaxKind::StructKw) => self.struct_def(),
            Some(SyntaxKind::EnumKw) => self.enum_def(),
            Some(SyntaxKind::EnumsetKw) => self.enum_set_def(),
            Some(SyntaxKind::UnionKw) => self.union_def(),
            _ => self.err_and_bump("in a definition"),
        }
    }

    /// Consumes the `internal`/`error` modifier run into the open node.
    fn modifiers(&mut self) {
        while matches!(
            self.current(),
            Some(SyntaxKind::InternalKw | SyntaxKind::ErrorKw)
        ) {
            self.bump();
        }
    }

    /// `TypeDef = 'internal'? 'error'? 'type' Name ':' Backing Constraint?
    /// InitValue?` — the constraint is a direct child of `TypeDef` here,
    /// never nested inside `PrimitiveType` (that position is for field
    /// types).
    fn type_def(&mut self) {
        self.start(SyntaxKind::TypeDef);
        self.modifiers();
        self.bump(); // 'type'
        self.name();
        self.expect(SyntaxKind::Colon);
        let stringish = self.backing();
        if self.at(SyntaxKind::LBracket) {
            self.constraint(stringish);
        }
        self.init_value_opt();
        self.builder.finish_node();
    }

    /// `Backing = PrimitiveType | UnitExpr` in `type`-definition position.
    /// Returns whether the backing is `string`/`bytes` — the bound policy
    /// input for the constraint that may follow.
    fn backing(&mut self) -> bool {
        match self.current() {
            Some(
                kw @ (SyntaxKind::BooleanKw
                | SyntaxKind::IntegerKw
                | SyntaxKind::FloatKw
                | SyntaxKind::StringKw
                | SyntaxKind::BytesKw),
            ) => {
                self.start(SyntaxKind::PrimitiveType);
                self.bump();
                self.builder.finish_node();
                matches!(kw, SyntaxKind::StringKw | SyntaxKind::BytesKw)
            }
            Some(SyntaxKind::Ident | SyntaxKind::Slash | SyntaxKind::Dot | SyntaxKind::Percent) => {
                // `UnitExpr = ('ident' | '/' | '.' | '%')*` — a UCUM
                // expression (`km/h`, `N.m`, `/min`, `%`). Semantic
                // validation is the ucum module's job (TYPL-110).
                self.start(SyntaxKind::UnitExpr);
                while matches!(
                    self.current(),
                    Some(
                        SyntaxKind::Ident
                            | SyntaxKind::Slash
                            | SyntaxKind::Dot
                            | SyntaxKind::Percent
                    )
                ) {
                    self.bump();
                }
                self.builder.finish_node();
                false
            }
            _ => {
                self.error_at_current("FORM-101", "expected a backing type".to_string());
                false
            }
        }
    }

    /// `ConstDef = 'internal'? 'error'? 'const' Name (':' type_ref:PathType)?
    /// '=' value:Literal`
    fn const_def(&mut self) {
        self.start(SyntaxKind::ConstDef);
        self.modifiers();
        self.bump(); // 'const'
        self.name();
        if self.at(SyntaxKind::Colon) {
            self.bump();
            self.path_type();
        }
        self.expect(SyntaxKind::Eq);
        self.literal();
        self.builder.finish_node();
    }

    /// `StructDef = 'internal'? 'error'? 'struct' Name '{' (members ','?)*
    /// '}'` — a member is a `FieldDef` or a `ReservedEntry`.
    fn struct_def(&mut self) {
        self.start(SyntaxKind::StructDef);
        self.modifiers();
        self.bump(); // 'struct'
        self.name();
        self.block_body("in a struct body", true, Self::field_def);
        self.builder.finish_node();
    }

    /// `EnumDef = 'internal'? 'error'? 'enum' Name '{' ((values | reserved)
    /// ','?)* '}'`
    fn enum_def(&mut self) {
        self.start(SyntaxKind::EnumDef);
        self.modifiers();
        self.bump(); // 'enum'
        self.name();
        self.block_body("in an enum body", true, Self::enum_value);
        self.builder.finish_node();
    }

    /// `EnumSetDef` — the standalone form (`enumset Name '{' bits '}'`) or
    /// the derived form (`enumset Name ':' backing_ref:PathType`), §9.1–§9.2.
    fn enum_set_def(&mut self) {
        self.start(SyntaxKind::EnumSetDef);
        self.modifiers();
        self.bump(); // 'enumset'
        self.name();
        if self.at(SyntaxKind::Colon) {
            self.bump();
            self.path_type();
        } else {
            self.block_body("in an enumset body", false, Self::enum_set_bit);
        }
        self.builder.finish_node();
    }

    /// `UnionDef = 'internal'? 'error'? 'union' Name '{' ((arms | reserved)
    /// ','?)* '}'`
    fn union_def(&mut self) {
        self.start(SyntaxKind::UnionDef);
        self.modifiers();
        self.bump(); // 'union'
        self.name();
        self.block_body("in a union body", true, Self::union_arm);
        self.builder.finish_node();
    }

    /// `UnionArm = Name ':' type_ref:PathType` — arms reference named types
    /// only; a primitive keyword still parses as a path segment and the
    /// checker rejects it (TYPL-204).
    fn union_arm(&mut self) {
        self.start(SyntaxKind::UnionArm);
        self.name();
        self.expect(SyntaxKind::Colon);
        self.path_type();
        self.builder.finish_node();
    }

    /// `EnumValue = Name '=' value:Literal`
    fn enum_value(&mut self) {
        self.value_assignment(SyntaxKind::EnumValue);
    }

    /// `EnumSetBit = Name '=' value:Literal`
    fn enum_set_bit(&mut self) {
        self.value_assignment(SyntaxKind::EnumSetBit);
    }

    /// The shared `Name '=' Literal` shape of enum values and enumset bits.
    fn value_assignment(&mut self, kind: SyntaxKind) {
        self.start(kind);
        self.name();
        self.expect(SyntaxKind::Eq);
        self.literal();
        self.builder.finish_node();
    }

    /// The shared block-body loop (§15.2): members separated by newlines or
    /// commas, trailing comma permitted, separator commas placed as direct
    /// children of the block node between members. `member` parses the
    /// member a leading `Ident` announces; `allow_reserved` admits
    /// `ReservedEntry` members (structs, enums, unions — not enumsets).
    fn block_body(&mut self, context: &str, allow_reserved: bool, member: fn(&mut Self)) {
        if !self.block_open() {
            return;
        }
        loop {
            self.eat_trivia();
            match self.current() {
                None => {
                    self.error_at_current("FORM-103", "unclosed `{`".to_string());
                    break;
                }
                Some(SyntaxKind::RBrace) => {
                    self.bump();
                    break;
                }
                Some(SyntaxKind::Comma) => self.bump(),
                Some(SyntaxKind::ReservedKw) if allow_reserved => self.reserved_entry(),
                Some(SyntaxKind::Ident) => member(self),
                Some(_) => self.err_and_bump(context),
            }
        }
    }

    /// Consumes the `{` opening a block body; reports FORM-101 and returns
    /// `false` when it is missing, so the caller skips the body loop instead
    /// of swallowing the rest of the file.
    fn block_open(&mut self) -> bool {
        if self.at(SyntaxKind::LBrace) {
            self.bump();
            true
        } else {
            self.error_at_current("FORM-101", "expected `{`".to_string());
            false
        }
    }

    /// `FieldDef = Name ':' FieldType InitValue?`
    fn field_def(&mut self) {
        self.start(SyntaxKind::FieldDef);
        self.name();
        self.expect(SyntaxKind::Colon);
        self.field_type();
        self.init_value_opt();
        self.builder.finish_node();
    }

    /// `ReservedEntry = 'reserved' (Name | Literal)` — the tombstone (§7.4):
    /// name form for structs and unions, integer form for enums. The parser
    /// accepts both forms everywhere; the checker narrows.
    fn reserved_entry(&mut self) {
        self.start(SyntaxKind::ReservedEntry);
        self.bump(); // 'reserved'
        if self.at(SyntaxKind::Ident) {
            self.name();
        } else if self.literal_len_at(0) > 0 {
            self.literal();
        } else {
            self.error_at_current("FORM-101", "expected a name or value".to_string());
        }
        self.builder.finish_node();
    }

    /// `FieldType = PathType | PrimitiveType | TupleType | ArrayType |
    /// MapType | OptionalType` — the type of a field, tuple field, or
    /// collection element. Each `?` suffix wraps the type parsed so far in an
    /// `OptionalType` node. The depth guard keeps a pathological nesting of
    /// collection and tuple types from overflowing the stack — the parser
    /// must never panic.
    fn field_type(&mut self) {
        if self.depth >= MAX_TYPE_DEPTH {
            self.error_at_current(
                "FORM-102",
                format!("type nesting deeper than {MAX_TYPE_DEPTH} levels"),
            );
            self.start(SyntaxKind::ErrorNode);
            self.bump();
            self.builder.finish_node();
            return;
        }
        self.depth += 1;
        self.field_type_inner();
        self.depth -= 1;
    }

    fn field_type_inner(&mut self) {
        self.eat_trivia();
        let checkpoint = self.builder.checkpoint();
        match self.current() {
            Some(
                kw @ (SyntaxKind::BooleanKw
                | SyntaxKind::IntegerKw
                | SyntaxKind::FloatKw
                | SyntaxKind::StringKw
                | SyntaxKind::BytesKw),
            ) => {
                // The inline constrained primitive (`integer [0..6]`,
                // §15.3): in field position the constraint nests inside
                // PrimitiveType, unlike the `type`-definition position.
                self.start(SyntaxKind::PrimitiveType);
                self.bump();
                if self.at(SyntaxKind::LBracket) {
                    let stringish = matches!(kw, SyntaxKind::StringKw | SyntaxKind::BytesKw);
                    self.constraint(stringish);
                }
                self.builder.finish_node();
            }
            Some(SyntaxKind::Ident) => self.path_type(),
            Some(SyntaxKind::LParen) => self.tuple_type(),
            Some(SyntaxKind::LBracket) => self.array_or_map(checkpoint),
            _ => {
                self.error_at_current("FORM-101", "expected a type".to_string());
                return;
            }
        }
        // Each `?` wraps one OptionalType node, so an unbounded run would
        // build a tree deep enough to overflow the stack when it is later
        // traversed or dropped. The wrap count shares the MAX_TYPE_DEPTH
        // bound: past it, the remaining `?` tokens land flat in a single
        // ErrorNode with one FORM-102.
        let mut wraps = 0;
        while self.at(SyntaxKind::Question) {
            if wraps >= MAX_TYPE_DEPTH {
                self.error_at_current(
                    "FORM-102",
                    format!("type nesting deeper than {MAX_TYPE_DEPTH} levels"),
                );
                self.start(SyntaxKind::ErrorNode);
                while self.at(SyntaxKind::Question) {
                    self.bump();
                }
                self.builder.finish_node();
                break;
            }
            self.builder
                .start_node_at(checkpoint, SyntaxKind::OptionalType.into());
            self.bump();
            self.builder.finish_node();
            wraps += 1;
        }
    }

    /// `TupleType = '(' fields:TupleField (',' fields:TupleField)* ')'` —
    /// separator commas are direct children of the tuple node.
    fn tuple_type(&mut self) {
        self.start(SyntaxKind::TupleType);
        self.bump(); // '('
        loop {
            self.eat_trivia();
            match self.current() {
                None => {
                    self.error_at_current("FORM-103", "unclosed `(`".to_string());
                    break;
                }
                Some(SyntaxKind::RParen) => {
                    self.bump();
                    break;
                }
                Some(SyntaxKind::Comma) => self.bump(),
                Some(SyntaxKind::Ident) => self.tuple_field(),
                Some(_) => self.err_and_bump("in a tuple type"),
            }
        }
        self.builder.finish_node();
    }

    /// `TupleField = Name ':' FieldType`
    fn tuple_field(&mut self) {
        self.start(SyntaxKind::TupleField);
        self.name();
        self.expect(SyntaxKind::Colon);
        self.field_type();
        self.builder.finish_node();
    }

    /// `ArrayType = '[' element:FieldType ';' Bound ']'` or
    /// `MapType = '[' key:FieldType ':' value:FieldType ';' Bound ']'` — the
    /// node kind is decided by the token after the first type, then wrapped
    /// around everything from the checkpoint (the `[`).
    fn array_or_map(&mut self, checkpoint: rowan::Checkpoint) {
        self.bump(); // '['
        self.field_type(); // the element or key type
        if self.at(SyntaxKind::Colon) {
            self.builder
                .start_node_at(checkpoint, SyntaxKind::MapType.into());
            self.bump(); // ':'
            self.field_type(); // the value type
        } else {
            self.builder
                .start_node_at(checkpoint, SyntaxKind::ArrayType.into());
        }
        self.expect(SyntaxKind::Semicolon);
        self.bound();
        self.expect(SyntaxKind::RBracket);
        self.builder.finish_node();
    }

    /// `Constraint = '[' … ']'` — see the module doc for the bound policy.
    fn constraint(&mut self, stringish: bool) {
        self.start(SyntaxKind::Constraint);
        self.bump(); // '['
        self.constraint_body(stringish);
        self.expect(SyntaxKind::RBracket);
        self.builder.finish_node();
    }

    /// The three constraint shapes behind the brackets: a scalar range with
    /// optional step, a length bound with optional match, and a bare match.
    fn constraint_body(&mut self, stringish: bool) {
        match self.current() {
            Some(SyntaxKind::MatchKw) => {
                self.bump();
                self.literal();
            }
            Some(SyntaxKind::DotDot) => {
                // `[..max]` — an open lower bound is always the scalar shape.
                self.bump();
                self.literal();
                self.step_opt();
            }
            _ if self.literal_len_at(0) > 0 => self.constraint_led_by_literal(stringish),
            _ => self.error_at_current("FORM-101", "expected a constraint".to_string()),
        }
    }

    /// A constraint whose body starts with a literal: decide between the
    /// length-`Bound` shape and the scalar-range shape (module doc policy).
    fn constraint_led_by_literal(&mut self, stringish: bool) {
        let first_len = self.literal_len_at(0);
        match self.nth(first_len) {
            // A single literal with no `..` is always a length bound:
            // `[17]`, `[8]`, `[17 match P]`.
            Some(SyntaxKind::RBracket | SyntaxKind::MatchKw) | None => {
                self.bound();
                self.match_opt();
            }
            Some(SyntaxKind::DotDot) => {
                let second_len = self.literal_len_at(first_len + 1);
                let after_second = self.nth(first_len + 1 + second_len);
                let bound_shape = stringish
                    && second_len > 0
                    && !self.literal_is_float_at(0)
                    && !self.literal_is_float_at(first_len + 1)
                    && matches!(
                        after_second,
                        Some(SyntaxKind::RBracket | SyntaxKind::MatchKw) | None
                    );
                if bound_shape {
                    self.bound();
                    self.match_opt();
                } else {
                    self.literal(); // min
                    self.bump(); // '..'
                    if self.literal_len_at(0) > 0 {
                        self.literal(); // max — absent on `[min..]`
                    }
                    self.step_opt();
                }
            }
            _ => {
                // Not a legal shape after the literal; parse the literal as a
                // length bound and let the closing-bracket check report.
                self.bound();
                self.match_opt();
            }
        }
    }

    /// `Bound = min:Literal ('..' max:Literal)?`
    fn bound(&mut self) {
        if self.literal_len_at(0) == 0 {
            self.error_at_current("FORM-101", "expected a bound".to_string());
            return;
        }
        self.start(SyntaxKind::Bound);
        self.literal();
        if self.at(SyntaxKind::DotDot) {
            self.bump();
            self.literal();
        }
        self.builder.finish_node();
    }

    /// `('step' step:Literal)?`
    fn step_opt(&mut self) {
        if self.at(SyntaxKind::StepKw) {
            self.bump();
            self.literal();
        }
    }

    /// `('match' match_pattern:Literal)?`
    fn match_opt(&mut self) {
        if self.at(SyntaxKind::MatchKw) {
            self.bump();
            self.literal();
        }
    }

    /// `InitValue = '=' Literal`, when present.
    fn init_value_opt(&mut self) {
        if self.at(SyntaxKind::Eq) {
            self.start(SyntaxKind::InitValue);
            self.bump();
            self.literal();
            self.builder.finish_node();
        }
    }

    /// How many significant tokens the literal starting at the `n`-th
    /// significant position spans: 2 for `-` + value, 1 for a bare value, 0
    /// when no literal starts there.
    fn literal_len_at(&self, n: usize) -> usize {
        match self.nth(n) {
            Some(SyntaxKind::Minus) => match self.nth(n + 1) {
                Some(kind) if is_value_token(kind) => 2,
                _ => 0,
            },
            Some(kind) if is_value_token(kind) => 1,
            _ => 0,
        }
    }

    /// Whether the literal starting at the `n`-th significant position has a
    /// float value token.
    fn literal_is_float_at(&self, n: usize) -> bool {
        match self.nth(n) {
            Some(SyntaxKind::FloatNumber) => true,
            Some(SyntaxKind::Minus) => matches!(self.nth(n + 1), Some(SyntaxKind::FloatNumber)),
            _ => false,
        }
    }

    /// `Literal = '-'? ('int_number' | 'float_number' | 'string_lit' | 'true'
    /// | 'false' | 'regex' | 'ident')` — an `ident` is a constant reference
    /// in a value position. A duration literal here is the profile boundary
    /// (TYPL-302); an integer with leading zeros is FORM-005.
    fn literal(&mut self) {
        if self.at(SyntaxKind::Duration) {
            self.error_at_current("TYPL-302", "duration literal in typl context".to_string());
            self.start(SyntaxKind::ErrorNode);
            self.bump();
            self.builder.finish_node();
            return;
        }
        if self.literal_len_at(0) == 0 {
            self.error_at_current("FORM-101", "expected a value".to_string());
            return;
        }
        self.start(SyntaxKind::Literal);
        if self.at(SyntaxKind::Minus) {
            self.bump();
        }
        if self.at(SyntaxKind::IntNumber) {
            self.flag_leading_zeros();
        }
        self.bump();
        self.builder.finish_node();
    }

    /// FORM-005: an integer literal with leading zeros (typl reference §2.4)
    /// lexes as one token and is flagged here.
    fn flag_leading_zeros(&mut self) {
        let i = self.significant_pos();
        let text = self.tokens[i].text;
        if text.len() > 1 && text.starts_with('0') {
            self.error_at_current("FORM-005", "integer literal has leading zeros".to_string());
        }
    }

    /// A declared name — a single `Ident` wrapped in a `Name` node. No node
    /// is built when the name is missing, so accessors see `None` rather
    /// than an empty `Name`.
    fn name(&mut self) {
        if self.at(SyntaxKind::Ident) {
            self.start(SyntaxKind::Name);
            self.bump();
            self.builder.finish_node();
        } else {
            self.error_at_current("FORM-101", "expected a name".to_string());
        }
    }

    /// Whether the current token can be a path segment. Primitive keywords
    /// are admitted as segments so `const MAX_GEAR : integer = 6` (typl
    /// reference Appendix B) parses; the checker narrows where a primitive
    /// is not allowed (TYPL-204 and the related composite checks).
    fn at_path_segment(&self) -> bool {
        matches!(
            self.current(),
            Some(
                SyntaxKind::Ident
                    | SyntaxKind::BooleanKw
                    | SyntaxKind::IntegerKw
                    | SyntaxKind::FloatKw
                    | SyntaxKind::StringKw
                    | SyntaxKind::BytesKw
            )
        )
    }

    /// `PathType = QualifiedName` — a type reference in type position. No
    /// node is built when the reference is missing.
    fn path_type(&mut self) {
        if !self.at_path_segment() {
            self.error_at_current("FORM-101", "expected a type name".to_string());
            return;
        }
        self.start(SyntaxKind::PathType);
        self.qualified_name();
        self.builder.finish_node();
    }

    /// `QualifiedName = 'ident' ('.' 'ident')*`
    fn qualified_name(&mut self) {
        if !self.at_path_segment() {
            self.error_at_current("FORM-101", "expected a name".to_string());
            return;
        }
        self.start(SyntaxKind::QualifiedName);
        self.bump();
        while self.at(SyntaxKind::Dot) {
            self.bump();
            if self.at_path_segment() {
                self.bump();
            } else {
                self.error_at_current("FORM-101", "expected a name after `.`".to_string());
                break;
            }
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
            parse("package fixtures\ntype X: m"),
            "parsing different input must not compare equal",
        );
    }

    /// The codes of the recorded errors, in source order.
    fn error_codes(input: &str) -> Vec<&'static str> {
        parse(input).errors().iter().map(|e| e.code).collect()
    }

    #[test]
    fn missing_package_flags_form_104() {
        let codes = error_codes("type Speed: km/h\n");
        assert_eq!(codes, vec!["FORM-104"]);
    }

    #[test]
    fn leading_zero_integer_flags_form_005() {
        let codes = error_codes("package p\nconst BAD = 042\n");
        assert_eq!(codes, vec!["FORM-005"]);
    }

    #[test]
    fn duration_literal_flags_typl_302() {
        let parse = parse("package p\nconst BAD = 10ms\n");
        assert_eq!(
            parse.errors().iter().map(|e| e.code).collect::<Vec<_>>(),
            vec!["TYPL-302"],
        );
        assert_eq!(
            parse.syntax().text().to_string(),
            "package p\nconst BAD = 10ms\n",
            "the rejected duration token must stay in the tree",
        );
    }

    #[test]
    fn stray_at_sigil_flags_typl_302() {
        let codes = error_codes("package p\n@\n");
        assert_eq!(codes, vec!["TYPL-302"]);
    }

    #[test]
    fn pathological_type_nesting_does_not_panic_or_drop_text() {
        let input = format!("package p\nstruct S {{ f : {} }}\n", "[".repeat(300));
        let parse = parse(&input);
        assert_eq!(
            parse.syntax().text().to_string(),
            input,
            "deep nesting must stay lossless",
        );
        assert!(!parse.errors().is_empty());
    }

    #[test]
    fn pathological_question_repetition_does_not_panic_or_drop_text() {
        // Each `?` wraps one OptionalType; an unbounded run used to build a
        // tree deep enough to overflow the stack on traversal or drop.
        let input = format!("package p\nstruct S {{ f : T{} }}\n", "?".repeat(30_000));
        let parse = parse(&input);
        assert_eq!(
            parse.syntax().text().to_string(),
            input,
            "the capped optional run must stay lossless",
        );
        assert_eq!(
            parse.errors().iter().map(|e| e.code).collect::<Vec<_>>(),
            vec!["FORM-102"],
            "the cap must report exactly one error",
        );
    }

    #[test]
    fn misplaced_error_modifier_parses_clean() {
        // TYPL-212 is checker scope: the grammar admits `error` on every
        // definition kind.
        let codes = error_codes("package p\nerror type Legacy : integer [0..1]\n");
        assert_eq!(codes, Vec::<&str>::new());
    }

    #[test]
    fn error_ranges_cover_the_offending_token() {
        // `042` sits at bytes 22..25 of the input.
        let parse = parse("package p\nconst BAD = 042\n");
        let error = &parse.errors()[0];
        assert_eq!(error.code, "FORM-005");
        assert_eq!(
            error.range,
            TextRange::new(TextSize::new(22), TextSize::new(25)),
        );
    }
}
