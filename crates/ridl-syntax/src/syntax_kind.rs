//! Token and node kinds shared by the lexer ([`crate::lexer`]) and the grammar.

/// The kinds of syntax tokens and nodes the family grammar produces.
///
/// The token variants are the full family token set the lexer recognises
/// (docs/ROADMAP.md epic E1.1, typl reference §1.4 and §2). The node variants
/// are the ones the E0 grammar (epic E0.3) produces. [`RidlLanguage`] maps
/// between this enum and rowan's raw kind through the `#[repr(u16)]`
/// discriminants, so the node variants stay grouped at the end with
/// [`Literal`](SyntaxKind::Literal) last — that is the range the
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
    /// A family-registry word that the typl profile does not use (typl
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
    // Trivia and the catch-all for unrecognised input.
    Whitespace,
    LineComment,
    BlockComment,
    DocComment,
    Error,
    // Nodes — produced by the E0 grammar (epic E0.3).
    SourceFile,
    TypeDecl,
    ConstDecl,
    Name,
    /// The backing representation of a `type` declaration. Reserved for the
    /// full E1 grammar; the E0 subset spells the backing inline as
    /// [`UnitExpr`](SyntaxKind::UnitExpr) plus an optional
    /// [`Range`](SyntaxKind::Range), so no `Backing` node is emitted yet.
    Backing,
    UnitExpr,
    Range,
    Literal,
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
            raw.0 <= SyntaxKind::Literal as u16,
            "raw kind {} is out of range for SyntaxKind",
            raw.0,
        );
        // SAFETY: SyntaxKind is `#[repr(u16)]` with contiguous discriminants
        // from 0 up to and including `Literal`. The assertion above proves
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
