//! The hand-written recursive-descent parser for the typl grammar
//! (docs/ROADMAP.md epic E1.2b, ADR-0004 §2, typl reference Appendix E).
//!
//! The parser consumes the flat token stream from [`crate::lex`] and builds a
//! lossless rowan tree whose nodes match `family.ungram` exactly: every token —
//! including whitespace and comments — lands in the tree in source order, so
//! [`Parse::syntax`] round-trips back to the original source for both valid
//! and broken input. On an unexpected token the parser records a
//! [`SyntaxError`] and wraps the skipped tokens in an
//! [`ErrorNode`](SyntaxKind::ErrorNode) rather than dropping them, so recovery
//! never loses text and never panics.
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
//! The parser runs under a [`Profile`] (E2 task 2, ADR-0007 decision 10). In
//! a `.typl` parse — byte-identical to the E1 parser — a `Duration` token or
//! a stray `@` anywhere emits **TYPL-302** (typl reference §2.8) and parsing
//! continues, and an interaction keyword at declaration-start position emits
//! **TYPL-304**. In a `.ridl` parse durations and `@` are ordinary tokens,
//! and a `ReservedWord` at declaration-start position — a word of the
//! uxdl/rmdl/rsdl profiles — emits **RIDL-403** (ridl reference §16.4). Both
//! declaration-start boundaries recover exactly as FORM-105 does. Leading
//! zeros in an integer literal emit **FORM-005**. Every [`SyntaxError`]
//! carries its diagnostic code; the coded `Diagnostic` model consumes it in
//! task E1.10.
//!
//! # Error recovery
//!
//! A broken declaration does not derail the rest of the file. When a loop
//! meets a token it cannot place, it reports one diagnostic and skips forward,
//! wrapping the skipped run in a single [`ErrorNode`](SyntaxKind::ErrorNode),
//! until it reaches a resynchronization point: a top-level keyword (`type`,
//! `const`, `struct`, and the other definition starters) or a closing `}` (a
//! tuple loop also stops at its `)`). A block body that runs into a top-level
//! keyword treats its `{` as unclosed (FORM-103) and hands the keyword back to
//! the top level rather than swallowing the next declaration. A family reserved
//! word where a declared name is expected is held in an `ErrorNode` and flagged
//! FORM-105, so the rest of the declaration still parses. Every [`SyntaxError`]
//! carries the narrowest honest range — the offending token — while the
//! `ErrorNode` carries the wider skipped span.
//!
//! # One diagnostic per source position
//!
//! [`Parser::expect`] never consumes on a mismatch, so a stuck cursor could
//! otherwise re-report at the same offset on every unwind step — a closed
//! 200-level type nesting once produced over a thousand diagnostics piled on a
//! single token. The parser records at most one [`SyntaxError`] per token start
//! offset. Because the cursor only ever moves forward,
//! [`Parser::error_at_current`] suppressing an error at the offset the previous
//! error already used is exactly "one diagnostic per source position". A run of
//! garbage therefore collapses to one diagnostic plus one `ErrorNode`, and the
//! pathological nesting inputs report a small, constant number of diagnostics
//! instead of one per unwound level.
//!
//! The profile-boundary **TYPL-302** is the one exception: a positional
//! FORM-101 can land on a duration or `@` token first (`[0..10ms]` fails the
//! `]` check at `10ms`), and the profile-boundary fact is the higher-value
//! diagnostic, so both are recorded. A TYPL-302 token is always consumed where
//! it is raised, so exempting it can never re-introduce a flood.

use rowan::{GreenNode, GreenNodeBuilder, TextRange, TextSize};

use crate::keywords::{self, Profile};
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

/// Parses `input` into a lossless [`Parse`] under `profile`.
pub fn parse(input: &str, profile: Profile) -> Parse {
    let mut parser = Parser::new(lex(input, profile), profile);
    parser.source_file();
    parser.finish()
}

/// Whether `kind` can be the value token of a `Literal` (`family.ungram` rule
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

/// Whether `kind` starts a top-level construct. These are the
/// resynchronization points recovery falls back to, both at the file level
/// and when a block body runs past an unclosed `}` into the next declaration.
/// `interface` joins the set with E2.1a; under [`Profile::Typl`] it never
/// occurs (the word lexes to `ReservedWord` there).
fn is_top_level_start(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::PackageKw
            | SyntaxKind::ImportKw
            | SyntaxKind::InternalKw
            | SyntaxKind::ErrorKw
            | SyntaxKind::TypeKw
            | SyntaxKind::ConstKw
            | SyntaxKind::StructKw
            | SyntaxKind::EnumKw
            | SyntaxKind::EnumsetKw
            | SyntaxKind::UnionKw
            | SyntaxKind::InterfaceKw
    )
}

/// Whether `kind` starts an interaction inside an interface body — the five
/// interaction keywords (E2.1a). They join the recovery sync set inside
/// interface bodies, so garbage resynchronizes at the next interaction.
fn is_interaction_start(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::SignalKw
            | SyntaxKind::EventKw
            | SyntaxKind::CommandKw
            | SyntaxKind::QueryKw
            | SyntaxKind::FinalKw
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
    /// The profile the file parses under — where the profile-boundary
    /// diagnostics (TYPL-302, TYPL-304, RIDL-403) are drawn.
    profile: Profile,
    builder: GreenNodeBuilder<'static>,
    errors: Vec<SyntaxError>,
}

impl<'a> Parser<'a> {
    fn new(tokens: Vec<Token<'a>>, profile: Profile) -> Self {
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
            profile,
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
    /// input), unless a diagnostic already sits at that start offset.
    ///
    /// The cursor only ever moves forward, so the start offset here is
    /// non-decreasing across calls; comparing against the last recorded error
    /// therefore enforces one diagnostic per source position (see the module
    /// doc). This is the flood control: on a stuck cursor the repeated,
    /// no-progress `expect`/`bound` errors during an unwind collapse to the
    /// first one.
    ///
    /// The profile-boundary **TYPL-302** is exempt: it is the higher-value
    /// diagnostic and must survive a positional FORM-101 that lands on the
    /// same boundary token first (`[0..10ms]` reports both). A TYPL-302 token
    /// is always consumed at the site that raises it, so it can never be the
    /// stuck-cursor flood source the suppression guards against.
    fn error_at_current(&mut self, code: &'static str, message: String) {
        let i = self.significant_pos();
        let start = TextSize::new(self.offsets[i] as u32);
        if code != "TYPL-302"
            && self
                .errors
                .last()
                .is_some_and(|last| last.range.start() == start)
        {
            return;
        }
        let end = self
            .tokens
            .get(i)
            .map_or(start, |t| start + TextSize::of(t.text));
        self.errors.push(SyntaxError {
            message,
            code,
            range: TextRange::new(start, end),
        });
    }

    /// The diagnostic for the current unexpected token: in a `.typl` parse,
    /// TYPL-302 for the profile-boundary tokens (duration literals and the `@`
    /// timing sigil, typl reference §2.8); FORM-102 otherwise. Under
    /// [`Profile::Ridl`] durations and `@` are ordinary tokens, so they draw
    /// the generic FORM-102 like any other token the grammar cannot place.
    fn unexpected_token_diag(&self, context: &str) -> (&'static str, String) {
        if self.profile == Profile::Typl {
            match self.current() {
                Some(SyntaxKind::Duration) => {
                    return ("TYPL-302", "duration literal in typl context".to_string());
                }
                Some(SyntaxKind::At) => {
                    return ("TYPL-302", "timing annotation in typl context".to_string());
                }
                _ => {}
            }
        }
        ("FORM-102", format!("unexpected token {context}"))
    }

    /// Recovery: report the current unexpected token, then skip forward —
    /// wrapping the skipped run in one [`SyntaxKind::ErrorNode`] — until a
    /// token `is_sync` accepts (a resynchronization point) or the input ends.
    /// The first token is always consumed, so the enclosing loop makes
    /// progress and no text is dropped.
    fn err_and_recover(&mut self, context: &str, is_sync: impl Fn(SyntaxKind) -> bool) {
        let (code, message) = self.unexpected_token_diag(context);
        self.error_at_current(code, message);
        self.recover(is_sync);
    }

    /// The recovery half of [`err_and_recover`]: consumes the current token
    /// and everything up to the next token `is_sync` accepts into one
    /// [`SyntaxKind::ErrorNode`]. The profile-boundary arms (TYPL-304,
    /// RIDL-403) share it, so they recover exactly as FORM-102/FORM-105 do.
    fn recover(&mut self, is_sync: impl Fn(SyntaxKind) -> bool) {
        self.start(SyntaxKind::ErrorNode);
        self.bump();
        while let Some(kind) = self.current() {
            if is_sync(kind) {
                break;
            }
            self.bump();
        }
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
                    | SyntaxKind::UnionKw
                    | SyntaxKind::InterfaceKw,
                ) => self.definition(),
                Some(SyntaxKind::ReservedWord) => {
                    if !self.profile_boundary_at_top_level() {
                        self.err_and_recover("at top level", is_top_level_start);
                    }
                }
                Some(_) => self.err_and_recover("at top level", is_top_level_start),
            }
        }
        self.builder.finish_node();
    }

    /// The profile boundary at declaration-start position (ADR-0007 decision
    /// 10, ridl reference §16.4). Called on a `ReservedWord` at the top level:
    ///
    /// - in a `.typl` parse, an interaction keyword (one of the nine ridl
    ///   words) draws **TYPL-304** instead of the generic FORM-102;
    /// - in a `.ridl` parse, any `ReservedWord` — a behaviour,
    ///   user-interaction, or architecture word of the uxdl/rmdl/rsdl
    ///   profiles — draws **RIDL-403**.
    ///
    /// Both recover exactly as FORM-105 does ([`Parser::recover`]: the run
    /// lands in one `ErrorNode`, resynchronizing at the next top-level
    /// keyword). Returns `false` — leaving the token untouched for the
    /// caller's generic recovery — when neither boundary applies (a
    /// uxdl/rmdl/rsdl word in a `.typl` parse stays a generic FORM-102).
    fn profile_boundary_at_top_level(&mut self) -> bool {
        let text = self.tokens[self.significant_pos()].text;
        let (code, message) = match self.profile {
            Profile::Typl => {
                if keywords::ridl_keyword(text).is_none() {
                    return false;
                }
                (
                    "TYPL-304",
                    format!("interaction declaration in typl context: `{text}`"),
                )
            }
            Profile::Ridl => (
                "RIDL-403",
                format!(
                    "behaviour, user-interaction, or architecture declaration in ridl context: `{text}`"
                ),
            ),
        };
        self.error_at_current(code, message);
        self.recover(is_top_level_start);
        true
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
            Some(SyntaxKind::InterfaceKw) => self.interface_def(),
            _ => self.err_and_recover("in a definition", is_top_level_start),
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

    // --- ridl interaction productions (E2.1a, ridl reference Appendix C) --

    /// `InterfaceDef = 'internal'? 'error'? 'interface' Name '{' (members
    /// ','?)* '}'` — an interface body holds interactions and `reserved`
    /// tombstones only (ridl reference §14.1); a type declaration inside it
    /// is RIDL-107, checker scope. The body loop is the interaction
    /// counterpart of [`Parser::block_body`]: members are announced by the
    /// five interaction keywords instead of an `Ident`, and those keywords
    /// are the recovery sync points, so garbage inside a body
    /// resynchronizes at the next interaction.
    fn interface_def(&mut self) {
        self.start(SyntaxKind::InterfaceDef);
        self.modifiers();
        self.bump(); // 'interface'
        self.name();
        if !self.block_open() {
            self.builder.finish_node();
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
                Some(SyntaxKind::ReservedKw) => self.reserved_entry(),
                Some(SyntaxKind::SignalKw) => self.value_interaction(SyntaxKind::SignalDef),
                Some(SyntaxKind::EventKw) => self.value_interaction(SyntaxKind::EventDef),
                Some(SyntaxKind::CommandKw) => self.callable_interaction(SyntaxKind::CommandDef),
                Some(SyntaxKind::QueryKw) => self.callable_interaction(SyntaxKind::QueryDef),
                // An unclosed `{`: the body ran into the next top-level
                // declaration. Report the missing brace and hand the
                // declaration back, exactly as `block_body` does.
                Some(kind) if is_top_level_start(kind) => {
                    self.error_at_current("FORM-103", "unclosed `{`".to_string());
                    break;
                }
                Some(_) => self.err_and_recover("in an interface body", |kind| {
                    matches!(
                        kind,
                        SyntaxKind::RBrace | SyntaxKind::Comma | SyntaxKind::ReservedKw
                    ) || is_interaction_start(kind)
                        || is_top_level_start(kind)
                }),
            }
        }
        self.builder.finish_node();
    }

    /// The shared `kw Name ':' payload InitValue? annotations` shape of the
    /// three value interactions — `SignalDef`, `EventDef`, and `FinalDef`.
    /// The bare `= value` init comes before the timing (ADR-0008 decision
    /// 2). The reference allows the init on signals only and timing on
    /// signals and events; here all three kinds accept an init, a timing,
    /// and an attr block, and the checker narrows (RIDL-106/-301, task 5).
    fn value_interaction(&mut self, kind: SyntaxKind) {
        self.start(kind);
        self.bump(); // 'signal' | 'event' | 'final'
        self.name();
        self.expect(SyntaxKind::Colon);
        self.field_type();
        self.init_value_opt();
        self.interaction_annotations();
        self.builder.finish_node();
    }

    /// The shared `kw Name '(' params ')' (':' ReturnType)? annotations`
    /// shape of the two callable interactions — `CommandDef` and
    /// `QueryDef`. A command's return type is lenient here (RIDL-104,
    /// checker scope); a query's is mandatory — a missing `:` draws
    /// FORM-101 (ridl reference §6.1, §7.1).
    fn callable_interaction(&mut self, kind: SyntaxKind) {
        self.start(kind);
        self.bump(); // 'command' | 'query'
        self.name();
        self.param_list();
        if self.at(SyntaxKind::Colon) {
            self.bump();
            self.return_type();
        } else if kind == SyntaxKind::QueryDef {
            self.error_at_current("FORM-101", "expected `:` and a return type".to_string());
        }
        self.interaction_annotations();
        self.builder.finish_node();
    }

    /// `ParamList = '(' (params:Param ','?)* ')'` — separator commas are
    /// direct children of the list node, mirroring the block-body
    /// discipline (typl reference §15.2). No node is built when the `(` is
    /// missing, so `CommandDef::params()` sees `None`.
    fn param_list(&mut self) {
        if !self.at(SyntaxKind::LParen) {
            self.error_at_current("FORM-101", "expected `(`".to_string());
            return;
        }
        self.start(SyntaxKind::ParamList);
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
                Some(SyntaxKind::Ident) => self.param(),
                // An unclosed `(`: the list ran into an interaction, body,
                // or top-level boundary. Report it and hand the token back.
                Some(kind)
                    if kind == SyntaxKind::RBrace
                        || is_interaction_start(kind)
                        || is_top_level_start(kind) =>
                {
                    self.error_at_current("FORM-103", "unclosed `(`".to_string());
                    break;
                }
                Some(_) => self.err_and_recover("in a parameter list", |kind| {
                    matches!(
                        kind,
                        SyntaxKind::RParen
                            | SyntaxKind::Comma
                            | SyntaxKind::Ident
                            | SyntaxKind::RBrace
                    ) || is_interaction_start(kind)
                        || is_top_level_start(kind)
                }),
            }
        }
        self.builder.finish_node();
    }

    /// `Param = Name ':' (FieldType | StreamType)` — a named typl type, a
    /// tuple (typl §11), or a stream `<T>` (ridl reference §12.1); the
    /// stream parses through the shared field-type dispatch.
    fn param(&mut self) {
        self.start(SyntaxKind::Param);
        self.name();
        self.expect(SyntaxKind::Colon);
        self.field_type();
        self.builder.finish_node();
    }

    /// `ReturnType = PathType | TupleType | StreamType | FallibleType` —
    /// the four return shapes (ridl reference §7.1). A named type followed
    /// by `|` extends into `FallibleType = ok '|' err` (general form §6.1,
    /// ADR-0008 decision 1). No node is built when no shape starts here, so
    /// `return_type()` accessors see `None`.
    fn return_type(&mut self) {
        self.eat_trivia();
        match self.current() {
            Some(SyntaxKind::LParen) => {
                self.start(SyntaxKind::ReturnType);
                self.tuple_type();
                self.builder.finish_node();
            }
            _ if self.at_path_segment() => {
                self.start(SyntaxKind::ReturnType);
                let checkpoint = self.builder.checkpoint();
                self.path_type();
                if self.at(SyntaxKind::Pipe) {
                    self.builder
                        .start_node_at(checkpoint, SyntaxKind::FallibleType.into());
                    self.bump(); // '|'
                    self.path_type();
                    self.builder.finish_node();
                }
                self.builder.finish_node();
            }
            _ => {
                self.error_at_current("FORM-101", "expected a return type".to_string());
            }
        }
    }

    /// The lenient trailing annotations of an interaction: at most one
    /// [`Timing`](SyntaxKind::Timing) and at most one
    /// [`AttrBlock`](SyntaxKind::AttrBlock), in either order. Which kinds
    /// may carry which annotation is checker scope (RIDL-104/-106/-301,
    /// task 5); the parser accepts both on every interaction.
    fn interaction_annotations(&mut self) {
        let mut seen_timing = false;
        let mut seen_attrs = false;
        loop {
            match self.current() {
                Some(SyntaxKind::At) if !seen_timing => {
                    seen_timing = true;
                    self.timing();
                }
                Some(SyntaxKind::LBracket) if !seen_attrs => {
                    seen_attrs = true;
                    self.attr_block();
                }
                _ => break,
            }
        }
    }

    /// `Timing = '@' ('duration' | '[' TimingRange ']')` — strict periodic
    /// or range (ridl reference §9). Callers have confirmed the `@`.
    fn timing(&mut self) {
        self.start(SyntaxKind::Timing);
        self.bump(); // '@'
        if self.at(SyntaxKind::Duration) {
            self.bump();
        } else if self.at(SyntaxKind::LBracket) {
            self.bump();
            self.timing_range();
            self.expect(SyntaxKind::RBracket);
        } else {
            self.error_at_current("FORM-101", "expected a duration or `[`".to_string());
        }
        self.builder.finish_node();
    }

    /// `TimingRange = 'duration'? '..' 'duration'?` — both bounds or the
    /// half-open forms `min..` and `..max` (ridl reference §9). A range
    /// with neither bound is a structural error (FORM-101).
    fn timing_range(&mut self) {
        self.start(SyntaxKind::TimingRange);
        let has_min = self.at(SyntaxKind::Duration);
        if has_min {
            self.bump();
        }
        self.expect(SyntaxKind::DotDot);
        let has_max = self.at(SyntaxKind::Duration);
        if has_max {
            self.bump();
        }
        if !has_min && !has_max {
            self.error_at_current("FORM-101", "expected a duration".to_string());
        }
        self.builder.finish_node();
    }

    /// A balanced `[ … ]` after an interaction signature, held as an
    /// [`AttrBlock`](SyntaxKind::AttrBlock) of raw tokens with no inner
    /// structure — lossless. The attribute grammar replaces this placeholder
    /// in task 4. An unclosed block stops at an interaction or body
    /// boundary (FORM-103) instead of swallowing the rest of the interface.
    fn attr_block(&mut self) {
        self.start(SyntaxKind::AttrBlock);
        self.bump(); // '['
        let mut depth = 1usize;
        loop {
            match self.current() {
                None => {
                    self.error_at_current("FORM-103", "unclosed `[`".to_string());
                    break;
                }
                Some(SyntaxKind::LBracket) => {
                    depth += 1;
                    self.bump();
                }
                Some(SyntaxKind::RBracket) => {
                    depth -= 1;
                    self.bump();
                    if depth == 0 {
                        break;
                    }
                }
                Some(kind)
                    if kind == SyntaxKind::RBrace
                        || is_interaction_start(kind)
                        || is_top_level_start(kind) =>
                {
                    self.error_at_current("FORM-103", "unclosed `[`".to_string());
                    break;
                }
                Some(_) => self.bump(),
            }
        }
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
                // An unclosed `{`: the body ran into the next top-level
                // declaration. Report the missing brace and let `source_file`
                // resynchronize instead of swallowing the declaration.
                Some(kind) if is_top_level_start(kind) => {
                    self.error_at_current("FORM-103", "unclosed `{`".to_string());
                    break;
                }
                Some(_) => self.err_and_recover(context, |kind| {
                    matches!(
                        kind,
                        SyntaxKind::RBrace | SyntaxKind::Comma | SyntaxKind::Ident
                    ) || (allow_reserved && kind == SyntaxKind::ReservedKw)
                        || is_top_level_start(kind)
                }),
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
                // An unclosed `(`: the tuple ran into a block or top-level
                // boundary. Report it and let the enclosing loop recover.
                Some(kind) if kind == SyntaxKind::RBrace || is_top_level_start(kind) => {
                    self.error_at_current("FORM-103", "unclosed `(`".to_string());
                    break;
                }
                Some(_) => self.err_and_recover("in a tuple type", |kind| {
                    matches!(
                        kind,
                        SyntaxKind::RParen
                            | SyntaxKind::Comma
                            | SyntaxKind::Ident
                            | SyntaxKind::RBrace
                    ) || is_top_level_start(kind)
                }),
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
    /// in a value position. A duration literal here in a `.typl` parse is the
    /// profile boundary (TYPL-302); an integer with leading zeros is FORM-005.
    /// Under [`Profile::Ridl`] a duration is an ordinary token that is simply
    /// not a literal, so it falls through to the generic missing-value path.
    fn literal(&mut self) {
        if self.profile == Profile::Typl && self.at(SyntaxKind::Duration) {
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

    /// A declared name — a single `Ident` wrapped in a `Name` node. A family
    /// reserved word is held in an `ErrorNode` and flagged FORM-105; otherwise
    /// no node is built when the name is missing, so accessors see `None`
    /// rather than an empty `Name`.
    fn name(&mut self) {
        if self.at(SyntaxKind::Ident) {
            self.start(SyntaxKind::Name);
            self.bump();
            self.builder.finish_node();
        } else if self.at(SyntaxKind::ReservedWord) {
            // A family reserved word (typl reference §1.4) used where a
            // declared name is expected: hold it in an `ErrorNode` so the
            // rest of the declaration still parses, and flag FORM-105.
            let message = format!(
                "reserved word `{}` used as identifier",
                self.tokens[self.significant_pos()].text,
            );
            self.error_at_current("FORM-105", message);
            self.start(SyntaxKind::ErrorNode);
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
        let parse = parse(FIXTURE, Profile::Typl);
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
        let parse = parse(input, Profile::Typl);
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
            parse(FIXTURE, Profile::Typl),
            parse(FIXTURE, Profile::Typl),
            "parsing identical input twice must compare equal",
        );
        assert_ne!(
            parse(FIXTURE, Profile::Typl),
            parse("package fixtures\ntype X: m", Profile::Typl),
            "parsing different input must not compare equal",
        );
    }

    /// The codes of the recorded errors, in source order.
    fn error_codes(input: &str) -> Vec<&'static str> {
        parse(input, Profile::Typl)
            .errors()
            .iter()
            .map(|e| e.code)
            .collect()
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
        let parse = parse("package p\nconst BAD = 10ms\n", Profile::Typl);
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

    // E2 task 2 step (b), parser half: under `Profile::Ridl` durations and `@`
    // are ordinary tokens — no TYPL-302 fires anywhere.
    #[test]
    fn duration_and_at_draw_no_typl_302_under_ridl() {
        let parsed = parse("package p\nconst BAD = 10ms\n", Profile::Ridl);
        assert!(
            parsed.errors().iter().all(|e| e.code != "TYPL-302"),
            "no TYPL-302 under Ridl, got: {:?}",
            parsed.errors(),
        );
        let parsed = parse("package p\n@\n", Profile::Ridl);
        assert_eq!(
            parsed.errors().iter().map(|e| e.code).collect::<Vec<_>>(),
            vec!["FORM-102"],
            "a stray `@` under Ridl is a generic unexpected token",
        );
    }

    // E2 task 2 step (e): an interaction keyword at declaration-start in a
    // `.typl` parse is the profile boundary — TYPL-304, recovering like
    // FORM-105 does (ErrorNode, resync at the next top-level keyword).
    #[test]
    fn interaction_keyword_in_typl_flags_typl_304_and_recovers() {
        let input = "package p\ninterface X {}\ntype Fine: m\n";
        let parsed = parse(input, Profile::Typl);
        assert_eq!(
            parsed.syntax().text().to_string(),
            input,
            "TYPL-304 recovery must stay lossless",
        );
        assert_eq!(
            parsed.errors().iter().map(|e| e.code).collect::<Vec<_>>(),
            vec!["TYPL-304"],
        );
        assert_eq!(def_names_in(Profile::Typl, input), vec!["Fine"]);

        // The same boundary fires for every interaction kind.
        let codes: Vec<_> = parse("package p\nsignal speed: Speed\n", Profile::Typl)
            .errors()
            .iter()
            .map(|e| e.code)
            .collect();
        assert_eq!(codes, vec!["TYPL-304"]);
    }

    // E2 task 2 step (f): a reserved word of another profile at
    // declaration-start in a `.ridl` parse is RIDL-403, with the same recovery.
    #[test]
    fn reserved_word_in_ridl_flags_ridl_403_and_recovers() {
        let input = "package p\nmodel X {}\ntype Fine: m\n";
        let parsed = parse(input, Profile::Ridl);
        assert_eq!(
            parsed.syntax().text().to_string(),
            input,
            "RIDL-403 recovery must stay lossless",
        );
        assert_eq!(
            parsed.errors().iter().map(|e| e.code).collect::<Vec<_>>(),
            vec!["RIDL-403"],
        );
        assert_eq!(def_names_in(Profile::Ridl, input), vec!["Fine"]);
    }

    #[test]
    fn pathological_type_nesting_does_not_panic_or_drop_text() {
        let input = format!("package p\nstruct S {{ f : {} }}\n", "[".repeat(300));
        let parse = parse(&input, Profile::Typl);
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
        let parse = parse(&input, Profile::Typl);
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
        let parse = parse("package p\nconst BAD = 042\n", Profile::Typl);
        let error = &parse.errors()[0];
        assert_eq!(error.code, "FORM-005");
        assert_eq!(
            error.range,
            TextRange::new(TextSize::new(22), TextSize::new(25)),
        );
    }

    /// The declared names of every top-level definition, in source order —
    /// the recovery proof that real declaration nodes survive garbage.
    fn def_names(input: &str) -> Vec<String> {
        def_names_in(Profile::Typl, input)
    }

    /// [`def_names`] under an explicit profile.
    fn def_names_in(profile: Profile, input: &str) -> Vec<String> {
        use crate::ast::{AstNode, HasName, SourceFile};
        let file = SourceFile::cast(parse(input, profile).syntax()).expect("root is a SourceFile");
        file.definitions()
            .filter_map(|def| Some(def.name()?.ident_token()?.text().to_string()))
            .collect()
    }

    #[test]
    fn top_level_garbage_resyncs_to_the_next_declaration() {
        // Valid, then garbage, then valid: the run of stray tokens is wrapped
        // in one ErrorNode and the declaration after it is a real node again.
        let input = "package p\ntype Good: km/h\n] } garbage 9\ntype AlsoGood: integer [0..1]\n";
        let parse = parse(input, Profile::Typl);
        assert_eq!(
            parse.syntax().text().to_string(),
            input,
            "resync must stay lossless",
        );
        assert_eq!(def_names(input), vec!["Good", "AlsoGood"]);
        assert_eq!(error_codes(input), vec!["FORM-102"]);
    }

    #[test]
    fn unclosed_brace_resyncs_to_the_next_declaration() {
        // A struct body that never closes hands the next `type` back to the
        // top level (FORM-103) instead of swallowing it.
        let input = "package p\nstruct Broken {\n  gear: integer\n\ntype After: km/h\n";
        let parse = parse(input, Profile::Typl);
        assert_eq!(
            parse.syntax().text().to_string(),
            input,
            "unclosed-brace recovery must stay lossless",
        );
        assert_eq!(def_names(input), vec!["Broken", "After"]);
        assert_eq!(error_codes(input), vec!["FORM-103"]);
    }

    #[test]
    fn reserved_word_as_name_flags_form_105_and_keeps_parsing() {
        // `signal` is a family reserved word (typl reference §1.4); used as a
        // type name it is held in an ErrorNode and the backing still parses.
        let input = "package p\ntype signal: integer [0..1]\ntype Fine: integer [0..1]\n";
        assert_eq!(error_codes(input), vec!["FORM-105"]);
        // Only `Fine` has a real Name; `signal` sits in an ErrorNode.
        assert_eq!(def_names(input), vec!["Fine"]);
    }

    #[test]
    fn unclosed_brackets_do_not_flood_diagnostics() {
        // 300 unclosed `[` in a field type: before the one-error-per-offset
        // rule the deep-recursion unwind piled hundreds of diagnostics on the
        // stuck tokens. Recovery now collapses the run to a handful.
        let input = format!("package p\nstruct S {{ f : {} }}\n", "[".repeat(300));
        let parse = parse(&input, Profile::Typl);
        assert_eq!(
            parse.syntax().text().to_string(),
            input,
            "the unclosed-bracket flood must stay lossless",
        );
        assert!(
            parse.errors().len() <= 4,
            "expected a bounded diagnostic count, got {}: {:?}",
            parse.errors().len(),
            parse.errors(),
        );
    }

    #[test]
    fn duration_in_constraint_reports_both_form_101_and_typl_302() {
        // A duration or `@` inside a constraint fails a positional check on the
        // same token first; the profile-boundary TYPL-302 must still fire — it
        // is exempt from the one-diagnostic-per-offset suppression, so both
        // codes are recorded (matching the honest init-position behavior).
        assert_eq!(
            error_codes("package p\ntype X: integer [0..10ms]\n"),
            vec!["FORM-101", "TYPL-302"],
        );
        assert_eq!(
            error_codes("package p\ntype X: integer [10ms]\n"),
            vec!["FORM-101", "TYPL-302"],
        );
        assert_eq!(
            error_codes("package p\ntype X: integer [0..5 @ 3]\n"),
            vec!["FORM-101", "TYPL-302"],
        );
    }

    #[test]
    fn closed_deep_nesting_does_not_flood_diagnostics() {
        // A balanced 200-level nested array, deeper than MAX_TYPE_DEPTH: the
        // closed shape once produced over a thousand diagnostics on unwind.
        let deep = format!("{}integer{}", "[".repeat(200), ";1]".repeat(200));
        let input = format!("package p\nstruct S {{ f : {deep} }}\n");
        let parse = parse(&input, Profile::Typl);
        assert_eq!(
            parse.syntax().text().to_string(),
            input,
            "the deep-nesting flood must stay lossless",
        );
        assert!(
            parse.errors().len() <= 4,
            "expected a bounded diagnostic count, got {}: {:?}",
            parse.errors().len(),
            parse.errors(),
        );
    }
}
