//! The RIDL language server (docs/ROADMAP.md epic E1.15a, ADR-0004 §6,
//! concept note §8.1).
//!
//! The server is built on `lsp-server` — rust-analyzer's minimal synchronous
//! transport crate (ADR-0004 §6). There is no async runtime: [`server::run`]
//! is a plain loop that receives one message at a time, dispatches it, and
//! calls straight into the memoized salsa queries (`parse_file`,
//! `resolve_package`, `check_package`).
//!
//! # The incremental overlay design
//!
//! `ridlc::compile_workspace` is a cold, from-disk, whole-workspace compile:
//! it mints fresh `InputFile` and `Workspace` salsa inputs and re-reads every
//! file on each call. Driving it on every keystroke would miss unsaved editor
//! buffers and defeat salsa incrementality (new inputs mean a full
//! recompute). The server therefore loads the workspace **once** at
//! `initialize` (`load_workspace`), holds the `Workspace` handle plus a map
//! of file path → `InputFile`, and on `didOpen`/`didChange` drives `set_text`
//! on the existing input — the editor buffer overlays the disk state. A file
//! opened from outside the loaded workspace becomes its own overlay input
//! wrapped in a synthetic single-file package. Every recompute then goes
//! through the memoized queries, so editing one file re-checks only the
//! package that file belongs to.

pub mod complete;
pub mod convert;
pub mod hover;
pub mod nav;
pub mod rename;
pub mod server;
