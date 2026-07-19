//! The RIDL family per-profile semantic passes: the name resolver and the
//! AST-to-IR checker (docs/ROADMAP.md epic E1, ADR-0007 decision 4). Carved
//! verbatim out of `ridl-core`, which kept them only until this crate existed
//! (ADR-0006 decision 2).

pub mod check;
pub mod resolve;
pub mod scalar;
pub mod ucum;

pub use check::{CheckError, check};
pub use resolve::{
    FileResolution, Resolution, ResolveError, Symbol, SymbolKind, resolve, resolve_package,
};
