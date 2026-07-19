//! The RIDL family compiler core: the salsa incremental database, the queries
//! built on the syntax layer (docs/ROADMAP.md epic E0, ADR-0004 §3), and the
//! coded [`Diagnostic`](diag::Diagnostic) model every pass emits (E1.10,
//! ADR-0004 §5). The resolver and checker moved to `ridl-sem` (ADR-0007
//! decision 4).

pub mod db;
pub mod diag;
pub mod manifest;

pub use db::{InputFile, RidlDatabase, parse_file};
pub use diag::{
    DiagCode, Diagnostic, FileId, FixIt, Label, Severity, SourceMap, Span, house_style_message,
    render,
};
pub use manifest::{Manifest, ManifestKind, parse_manifest};
