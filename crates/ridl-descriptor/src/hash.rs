//! The catalog hash (rsdl note D-8), re-exported from
//! [`ridl_ir::catalog_hash`]. The hash is computed in `ridl-ir` (ADR-0014
//! decision 15) because the codegen model carries it too, in its
//! `Catalog.hash`, and the model is lowered there. This module is the name
//! the descriptor's callers use.

pub use ridl_ir::catalog_hash::{catalog_hash, reachable_decls, reduced_package};
