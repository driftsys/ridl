//! Shared test-support modules for this crate's integration test targets.
//!
//! Lives under `tests/support/` (a subdirectory), so Cargo's test-target
//! auto-discovery does not turn any file here into its own test binary
//! (`tests/support/ir.rs`'s own header records the same reasoning). Reached
//! with `mod support;` from an individual integration test target, such as
//! `tests/interaction_face.rs`.

pub mod loopback;
