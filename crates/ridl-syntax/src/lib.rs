//! The family lexer and grammar (epic E0, docs/ROADMAP.md).

pub mod ast;
mod ast_e0;
pub mod keywords;
mod lexer;
mod parser;
mod syntax_kind;

pub use ast_e0::{ConstDecl, RangeSpec, SourceFile, TypeDecl};
pub use lexer::{Token, lex};
pub use parser::{Parse, SyntaxError, parse};
pub use syntax_kind::{RidlLanguage, SyntaxKind, SyntaxNode, SyntaxToken};
