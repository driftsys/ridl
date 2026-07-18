//! The family lexer and grammar (epic E0, docs/ROADMAP.md).

mod ast;
mod lexer;
mod parser;
mod syntax_kind;

pub use ast::{ConstDecl, RangeSpec, SourceFile, TypeDecl};
pub use lexer::{Token, lex};
pub use parser::{Parse, SyntaxError, parse};
pub use syntax_kind::{RidlLanguage, SyntaxKind, SyntaxNode, SyntaxToken};
