//! The RIDL family compiler core: the salsa incremental database, the name
//! resolver, the checker, and the queries built on the syntax layer
//! (docs/ROADMAP.md epic E0, ADR-0004 §3).

pub mod check;
pub mod db;
pub mod resolve;

pub use check::{CheckError, check};
pub use db::{RidlDatabase, SourceFile, parse_file};
pub use resolve::{Resolution, ResolveError, SymbolKind, resolve};
