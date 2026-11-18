//! Arbitrary bytes through the generated FlatBuffers codec.
//!
//! The input is verified as each root type of the fixture. A buffer that
//! verifies must decode, and the decoded value must encode again to bytes that
//! verify. The only allowed failure is a returned error.

#![no_main]
#![allow(dead_code, deprecated, unused_imports)]

use libfuzzer_sys::fuzz_target;
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::payload::{Payload, Ref};

include!(concat!(env!("OUT_DIR"), "/flatbuffers.rs"));

fn probe<T: Payload<FlatBuffers>>(data: &[u8]) {
    if let Ok(proof) = Ref::<'_, T, FlatBuffers>::verify(data) {
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
    probe::<Report>(data);
    probe::<Inner>(data);
    probe::<Outcome>(data);
    probe::<Count>(data);
    probe::<Label>(data);
    probe::<Health>(data);
});
