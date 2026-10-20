//! The catalog hash (rsdl note D-8), computed by [`ridl_ir::catalog_hash`].
//! The hash is computed in `ridl-ir` (ADR-0014 decision 15) because the
//! codegen model carries it too, in its `Catalog.hash`, and the model is
//! lowered there. This module is the name the descriptor's callers use. Each
//! function here is a wrapper with the signature of the `ridl-ir` function it
//! calls, so that it is an item of this crate and the freeze check compares it
//! with the baseline.

use std::collections::BTreeMap;

use ridl_ir::v2::{Decl, Package};

/// Every declaration an interface shape of `unit` reaches, in any package
/// of `packages`, keyed by canonical name (`pkg.Name`).
///
/// See [`ridl_ir::catalog_hash::reachable_decls`].
pub fn reachable_decls<'a>(unit: &str, packages: &[&'a Package]) -> BTreeMap<String, &'a Decl> {
    ridl_ir::catalog_hash::reachable_decls(unit, packages)
}

/// The exact input of the hash, one IR package named after the unit: every
/// interface shape of every package of the unit under its catalog name
/// (`Package::catalog_name`: the declared name relative to the unit, or the
/// owning service's dotted global name for an inline shape), with the
/// owning service's visibility for an inline shape, the IR's `number` and
/// `provisional`, and its interactions, in (number, name) order because the
/// lock makes the number the identity; the reached declarations under their
/// full canonical names (`pkg.Name`, for the unit's own packages too, since
/// two packages of one unit can declare the same short name), in
/// canonical-name order, every type reference inside them rewritten to the
/// canonical name of the declaration it resolves to, and every expression
/// string left as written; doc strings, doc links and doc tags (`labels`,
/// `deprecated`, `see`, `since`) blanked, a parameter's included; no
/// services and no retired entries.
///
/// `packages` is every package of the build; a package named twice is read
/// once.
pub fn reduced_unit(unit: &str, packages: &[&Package]) -> Package {
    ridl_ir::catalog_hash::reduced_unit(unit, packages)
}

/// SHA-256 over the protobuf binary of [`reduced_unit`]. The numbers are
/// inside: each reduced interface carries the IR's `number` and
/// `provisional`. A package named twice in `packages` is read once, so the
/// hash is the same whether or not the caller lists a package of the unit
/// twice.
pub fn catalog_hash(unit: &str, packages: &[&Package]) -> [u8; 32] {
    ridl_ir::catalog_hash::catalog_hash(unit, packages)
}
