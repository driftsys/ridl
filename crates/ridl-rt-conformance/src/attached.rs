//! `Attached`: the catalog a port serves.

use ridl_rt::port::Attached;

use crate::{Factory, catalog, runtime};

/// Every port answers with the catalog its runtime was built with, carried
/// unexamined — the runtime and each handle made from it.
pub fn the_catalog_is_the_one_the_runtime_was_built_with<F: Factory>() {
    let rt = runtime::<F>();
    assert_eq!(*rt.catalog(), catalog());
    assert_eq!(*F::source(&rt).catalog(), catalog());
    assert_eq!(*F::caller(&rt).catalog(), catalog());
    assert_eq!(*F::handler(&rt).catalog(), catalog());
}
