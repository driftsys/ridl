//! The catalog hash (rsdl note D-8), computed by [`ridl_ir::catalog_hash`].
//! The hash is computed in `ridl-ir` (ADR-0014 decision 15) because the
//! codegen model carries it too, in its `Catalog.hash`, and the model is
//! lowered there. This module is the name the descriptor's callers use. Each
//! function here is a wrapper with the signature of the `ridl-ir` function it
//! calls, so that it is an item of this crate and the freeze check compares it
//! with the baseline.

use std::collections::BTreeMap;

use ridl_ir::v2::{Decl, Package};

/// Wraps [`ridl_ir::catalog_hash::reachable_decls`].
pub fn reachable_decls<'a>(unit: &str, packages: &[&'a Package]) -> BTreeMap<String, &'a Decl> {
    ridl_ir::catalog_hash::reachable_decls(unit, packages)
}

/// Wraps [`ridl_ir::catalog_hash::reduced_unit`].
pub fn reduced_unit(unit: &str, packages: &[&Package]) -> Package {
    ridl_ir::catalog_hash::reduced_unit(unit, packages)
}

/// Wraps [`ridl_ir::catalog_hash::catalog_hash`].
pub fn catalog_hash(unit: &str, packages: &[&Package]) -> [u8; 32] {
    ridl_ir::catalog_hash::catalog_hash(unit, packages)
}
