//! The RIDL family compiler core: the salsa incremental database and the
//! queries built on the syntax layer (docs/ROADMAP.md epic E0, ADR-0004 §3).
//! The resolver and checker moved to `ridl-sem` (ADR-0007 decision 4).

pub mod db;

pub use db::{InputFile, RidlDatabase, parse_file};
