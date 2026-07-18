//! Token and node kinds shared by the lexer ([`crate::lexer`], epic E0.2)
//! and the grammar (epic E0.3).

/// The kinds of syntax tokens and nodes the family grammar produces.
///
/// The token variants are the E0 subset the family lexer recognises
/// (docs/ROADMAP.md epic E0.2). The node variants are the ones the E0 grammar
/// (epic E0.3) produces. New variants are only ever appended: [`RidlLanguage`]
/// maps between this enum and rowan's raw kind through the `#[repr(u16)]`
/// discriminants, so the order below is part of the interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum SyntaxKind {
    // Tokens — E0 subset of the family lexer.
    TypeKw,
    ConstKw,
    StepKw,
    Ident,
    IntNumber,
    FloatNumber,
    Colon,
    Eq,
    LBracket,
    RBracket,
    DotDot,
    Dot,
    Slash,
    Comma,
    Whitespace,
    LineComment,
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
    /// must stay in the tree for losslessness.
    pub fn is_trivia(self) -> bool {
        matches!(self, SyntaxKind::Whitespace | SyntaxKind::LineComment)
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
