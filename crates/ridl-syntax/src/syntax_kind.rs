//! Token and node kinds shared by the lexer ([`crate::lexer`], epic E0.2)
//! and the grammar (epic E0.3).

/// The kinds of syntax tokens and nodes the family grammar produces.
///
/// The variants below are the E0 token subset the family lexer recognises
/// (docs/ROADMAP.md epic E0.2). Node variants are appended once the grammar
/// lands (epic E0.3).
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
    // Nodes are appended by Task 3.
}
