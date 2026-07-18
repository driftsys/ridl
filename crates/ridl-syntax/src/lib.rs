//! The family lexer and grammar (epic E0, docs/ROADMAP.md).

mod lexer;
mod syntax_kind;

pub use lexer::{Token, lex};
pub use syntax_kind::SyntaxKind;
