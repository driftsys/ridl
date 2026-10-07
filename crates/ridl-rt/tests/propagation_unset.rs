//! With no call to `set_propagation`, there is no hook. The hook is global,
//! so this file is its own test binary.
#![cfg(feature = "std")]

use ridl_rt::trace::propagation;

#[test]
fn no_hook_by_default() {
    assert!(propagation().is_none());
}
