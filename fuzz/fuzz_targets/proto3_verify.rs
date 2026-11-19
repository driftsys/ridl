//! Arbitrary bytes through the generated proto3 codec.
//!
//! The input is verified as each root type of the fixture. A buffer that
//! verifies must decode, and the decoded value must encode again to bytes that
//! verify. The only allowed failure is a returned error.

#![no_main]
#![allow(dead_code, deprecated, unused_imports)]

use libfuzzer_sys::fuzz_target;
use ridl_rt::encoding::Proto3;
use ridl_rt::payload::{Payload, Ref};

include!(concat!(env!("OUT_DIR"), "/proto3.rs"));

fn probe<T: Payload<Proto3>>(data: &[u8]) {
    if let Ok(proof) = Ref::<'_, T, Proto3>::verify(data) {
        let value = proof.decode();
        let mut out = vec![0u8; T::MAX_SIZE];
        let encoded = value
            .encode(&mut out)
            .expect("an accepted value encodes again");
        assert!(
            T::verify(encoded.bytes).is_ok(),
            "the bytes written from an accepted value verify"
        );
    }
}

fuzz_target!(|data: &[u8]| {
    probe::<Scalars>(data);
    probe::<Nested>(data);
    probe::<Optionals>(data);
    probe::<Lists>(data);
    probe::<Maps>(data);
    probe::<Tuples>(data);
    probe::<Outcome>(data);
    probe::<OptionalComposites>(data);
    probe::<Count>(data);
    probe::<Label>(data);
    probe::<Health>(data);
});
