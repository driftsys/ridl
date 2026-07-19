//! The RIDL family per-profile semantic passes: the package resolver and the
//! package checker (docs/ROADMAP.md epic E1, ADR-0007 decision 4). Carved
//! verbatim out of `ridl-core`, which kept them only until this crate existed
//! (ADR-0006 decision 2).

pub mod check;
pub mod docs;
pub mod init;
pub mod resolve;
pub mod scalar;
// The proptest range generators (E1.18). Behind the default-on `testgen`
// feature so the wasm32-unknown-unknown check can drop proptest, whose
// `getrandom` dependency does not build for that target (ADR-0007 decision 5).
#[cfg(feature = "testgen")]
pub mod testgen;
pub mod ucum;

pub use check::{
    CheckedInterface, CheckedPackage, ConstValue, check_package, checked_interface, const_value,
};
pub use resolve::{Resolution, Symbol, SymbolKind, resolve_package};
