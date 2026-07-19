//! The RIDL family compiler core: the salsa incremental database, the queries
//! built on the syntax layer (docs/ROADMAP.md epic E0, ADR-0004 §3), the
//! coded [`Diagnostic`](diag::Diagnostic) model every pass emits (E1.10,
//! ADR-0004 §5), and the ns core — manifest, package model, and filesystem
//! discovery (E1.3, E1.5, ADR-0002). The resolver and checker moved to
//! `ridl-sem` (ADR-0007 decision 4).
//!
//! Filesystem discovery ([`workspace`]) sits behind the default-on `fs`
//! feature (ADR-0007 decision 5); with `--no-default-features` the crate
//! builds for `wasm32-unknown-unknown`.

pub mod db;
pub mod diag;
pub mod manifest;
pub mod package;
pub mod std_lib;
#[cfg(feature = "fs")]
pub mod workspace;

pub use db::{InputFile, RidlDatabase, parse_file};
pub use diag::{
    DiagCode, Diagnostic, FileId, FixIt, Label, Severity, SourceMap, Span, house_style_message,
    render,
};
pub use manifest::{Manifest, ManifestKind, parse_manifest};
pub use package::{Package, PackageOrigin, Workspace, package_of};
pub use std_lib::std_package;
#[cfg(feature = "fs")]
pub use workspace::{LoadedWorkspace, load_workspace};
