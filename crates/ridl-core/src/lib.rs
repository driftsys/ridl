//! The RIDL family compiler core: the salsa incremental database, the queries
//! built on the syntax layer (docs/ROADMAP.md epic E0, ADR-0004 §3), the
//! coded [`Diagnostic`] model every pass emits (E1.10,
//! ADR-0004 §5), and the ns core — manifest, package model, and filesystem
//! discovery (E1.3, E1.5, ADR-0002). The resolver and checker moved to
//! `ridl-sem` (ADR-0007 decision 4).
//!
//! Filesystem discovery ([`workspace`]) and the lockfile ([`lock`]) sit behind
//! the default-on `fs` feature; remote fetch and the cache ([`fetch`](mod@fetch),
//! [`cache`]) sit behind the default-on `fetch` feature, which implies `fs`
//! (ADR-0007 decision 5). With `--no-default-features` the crate builds for
//! `wasm32-unknown-unknown`.

#[cfg(feature = "fetch")]
pub mod cache;
pub mod db;
pub mod diag;
#[cfg(feature = "fetch")]
pub mod fetch;
#[cfg(feature = "fs")]
pub mod lock;
pub mod manifest;
pub mod package;
pub mod std_lib;
#[cfg(feature = "fs")]
pub mod workspace;

#[cfg(feature = "fetch")]
pub use cache::Cache;
pub use db::{InputFile, RidlDatabase, parse_file, profile_of_path};
pub use diag::{
    DiagCode, Diagnostic, FileId, FixIt, Label, Severity, SourceMap, Span, house_style_message,
    render,
};
#[cfg(feature = "fetch")]
pub use fetch::{FetchError, Frozen, fetch, materialize_imports};
#[cfg(feature = "fs")]
pub use lock::{LockEntry, Lockfile, read_lockfile, write_lockfile};
pub use manifest::{Manifest, ManifestKind, parse_manifest};
pub use package::{Package, PackageOrigin, Workspace, package_of};
pub use std_lib::std_package;
#[cfg(feature = "fs")]
pub use workspace::{LoadedWorkspace, load_workspace};
