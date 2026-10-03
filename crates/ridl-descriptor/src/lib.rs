//! The catalog descriptor: the FlatBuffers file per package that an engine
//! reads without decoding (`docs/wip/2026-09-13-runtime-descriptors-design.md`).
//!
//! `schema/catalog.fbs` is the schema; `generated.rs` holds the accessors
//! planus generates from it (`cargo xtask descriptor-codegen`). This crate
//! does no I/O: `ridlc` writes the bytes `lower` returns, and `ridl describe`
//! hands the bytes it read to `verify`.

pub mod generated;

pub use generated::ridl::descriptor::{
    Catalog, CatalogRef, Encoding, Interface, InterfaceRef, Kind, MaxSize, MaxSizeRef, Member,
    MemberRef, Payload, PayloadRef, RetiredInterface, RetiredInterfaceRef,
    SizeState as SizeStateTag, Timing, TimingMode, TimingRef, UnboundedCause,
};

/// The schema version this toolchain writes and accepts.
pub const SCHEMA_VERSION: u32 = 1;

/// The FlatBuffers file identifier, at bytes 4..8 of every catalog descriptor.
pub const FILE_IDENTIFIER: [u8; 4] = *b"RDLC";

/// The artifact suffix: `<base>.catalog.binfb` (ADR-0014 decision 4's
/// convention — a plain-English flag value, an encoding-bearing extension).
pub const FILE_SUFFIX: &str = ".catalog.binfb";
