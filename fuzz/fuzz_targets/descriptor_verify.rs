//! Arbitrary bytes through the catalog descriptor verifier.
//!
//! A buffer that verifies must convert to the owned catalog, and that catalog
//! must be written again to bytes that verify. The only allowed failure is a
//! returned error.

#![no_main]

use libfuzzer_sys::fuzz_target;
use ridl_descriptor::{finish, verify, Catalog};

fuzz_target!(|data: &[u8]| {
    if let Ok(view) = verify(data) {
        let catalog = Catalog::try_from(view).expect("a verified catalog converts");
        let encoded = finish(&catalog);
        assert!(
            verify(&encoded).is_ok(),
            "the bytes written from a verified catalog verify"
        );
    }
});
