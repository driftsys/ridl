//! The family lexer and grammar (epic E0, docs/ROADMAP.md).

pub mod ast;
pub mod keywords;
mod lexer;
mod parser;
mod syntax_kind;

pub use keywords::Profile;
pub use lexer::{Token, lex};
pub use parser::{Parse, SyntaxError, parse};
pub use syntax_kind::{RidlLanguage, SyntaxKind, SyntaxNode, SyntaxToken};
