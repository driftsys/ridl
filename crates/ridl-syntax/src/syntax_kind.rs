//! Token and node kinds shared by the lexer ([`crate::lexer`]) and the grammar.

/// The kinds of syntax tokens and nodes the family grammar produces.
///
/// The token variants are the full family token set the lexer recognises
/// (docs/ROADMAP.md epic E1.1, typl reference §1.4 and §2). The node variants
/// are the typl grammar's node inventory (epic E1.2a) — one variant per rule
/// in `family.ungram`, plus [`ErrorNode`](SyntaxKind::ErrorNode) for recovery.
/// [`RidlLanguage`] maps between this enum and rowan's raw kind through the
/// `#[repr(u16)]` discriminants, so the node variants stay grouped at the end
/// with [`ErrorNode`](SyntaxKind::ErrorNode) last — that is the range the
/// `kind_from_raw` assertion below guards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum SyntaxKind {
    // Tokens — the full family lexer (typl reference §1.4, §2).
    //
    // Keywords the typl profile uses (typl reference §1.4). Each used keyword
    // maps to a distinct variant; every family-registry word the typl profile
    // does not use maps to `ReservedWord` instead.
    PackageKw,
    ImportKw,
    AsKw,
    InternalKw,
    TypeKw,
    ConstKw,
    StructKw,
    EnumKw,
    EnumsetKw,
    UnionKw,
    BooleanKw,
    IntegerKw,
    FloatKw,
    StringKw,
    BytesKw,
    TrueKw,
    FalseKw,
    StepKw,
    MatchKw,
    ReservedKw,
    ErrorKw,
    // Keywords the ridl profile activates beyond typl's set (ridl reference
    // §2.3): the interaction and container words plus the expr-core pair.
    // Under `Profile::Typl` these words still lex to `ReservedWord`.
    InterfaceKw,
    ServiceKw,
    SignalKw,
    EventKw,
    CommandKw,
    QueryKw,
    FinalKw,
    RequireKw,
    EnsureKw,
    /// A family-registry word that the active profile does not use (typl
    /// reference §1.4). Reserved in every profile, never a valid identifier.
    ReservedWord,
    // Names and literals.
    Ident,
    IntNumber,
    FloatNumber,
    String,
    Regex,
    Duration,
    // Punctuation.
    Colon,
    Eq,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    LParen,
    RParen,
    DotDot,
    Dot,
    Slash,
    Comma,
    Question,
    Semicolon,
    At,
    Pipe,
    Percent,
    Minus,
    // Expression operators (ridl reference §13, general form expr core).
    // Family tokens like `Duration`: the lexer recognises them under every
    // profile; the typl grammar simply never accepts them.
    Lt,
    Le,
    Gt,
    Ge,
    EqEq,
    Neq,
    AmpAmp,
    PipePipe,
    Bang,
    Plus,
    Star,
    // Trivia and the catch-all for unrecognised input.
    Whitespace,
    LineComment,
    BlockComment,
    DocComment,
    Error,
    // Nodes — the typl grammar's node inventory (`family.ungram`, epic E1.2a).
    // The full parser (task E1.2b) produces them; until it lands, the
    // generated typed AST casts over trees built directly in tests.
    SourceFile,
    PackageDecl,
    Import,
    TypeDef,
    ConstDef,
    StructDef,
    FieldDef,
    ReservedEntry,
    EnumDef,
    EnumValue,
    EnumSetDef,
    EnumSetBit,
    UnionDef,
    UnionArm,
    TupleType,
    TupleField,
    ArrayType,
    MapType,
    OptionalType,
    PathType,
    PrimitiveType,
    UnitExpr,
    Constraint,
    Bound,
    Name,
    QualifiedName,
    Literal,
    InitValue,
    // Nodes of the ridl interaction grammar (`family.ungram`, epic E2.1a —
    // ridl reference Appendix C, ADR-0008 decisions 1 and 2).
    InterfaceDef,
    SignalDef,
    EventDef,
    CommandDef,
    QueryDef,
    FinalDef,
    Param,
    ParamList,
    ReturnType,
    StreamType,
    FallibleType,
    Timing,
    TimingRange,
    AttrBlock,
    /// A recovery node: error recovery wraps the tokens it skips in one of
    /// these, so broken input still produces a lossless tree. It is the one
    /// node kind with no rule in `family.ungram`, and the last variant — the
    /// `kind_from_raw` assertion below guards the range up to it.
    ErrorNode,
}

impl SyntaxKind {
    /// Whitespace and comments — tokens that carry no grammatical meaning but
    /// must stay in the tree for losslessness. Doc comments are trivia too: the
    /// grammar reads them from the trivia preceding a definition.
    pub fn is_trivia(self) -> bool {
        matches!(
            self,
            SyntaxKind::Whitespace
                | SyntaxKind::LineComment
                | SyntaxKind::BlockComment
                | SyntaxKind::DocComment
        )
    }
}

impl From<SyntaxKind> for rowan::SyntaxKind {
    fn from(kind: SyntaxKind) -> Self {
        Self(kind as u16)
    }
}

/// The rowan [`Language`](rowan::Language) instance for the RIDL family. It
/// binds rowan's raw `u16` kinds to [`SyntaxKind`] through the enum's
/// `#[repr(u16)]` discriminants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RidlLanguage {}

impl rowan::Language for RidlLanguage {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> SyntaxKind {
        assert!(
            raw.0 <= SyntaxKind::ErrorNode as u16,
            "raw kind {} is out of range for SyntaxKind",
            raw.0,
        );
        // SAFETY: SyntaxKind is `#[repr(u16)]` with contiguous discriminants
        // from 0 up to and including `ErrorNode`. The assertion above proves
        // `raw.0` names one of them, so the transmute produces a valid variant.
        unsafe { std::mem::transmute::<u16, SyntaxKind>(raw.0) }
    }

    fn kind_to_raw(kind: SyntaxKind) -> rowan::SyntaxKind {
        kind.into()
    }
}

/// A node in the red (cursor) tree over [`RidlLanguage`].
pub type SyntaxNode = rowan::SyntaxNode<RidlLanguage>;
/// A token in the red (cursor) tree over [`RidlLanguage`].
pub type SyntaxToken = rowan::SyntaxToken<RidlLanguage>;
