//! Arbitrary bytes through the generated podbuf codec.
//!
//! podbuf requires the start of a buffer to be 8-aligned, so the input is
//! copied to an aligned position first. The copy is verified as each root type
//! of the fixture. A buffer that verifies must decode, and the decoded value
//! must encode again to bytes that verify. The only allowed failure is a
//! returned error.

#![no_main]
#![allow(dead_code, deprecated, unused_imports)]

use libfuzzer_sys::fuzz_target;
use ridl_rt::encoding::Podbuf;
use ridl_rt::payload::{Payload, Ref};

include!(concat!(env!("OUT_DIR"), "/podbuf.rs"));

fn probe<T: Payload<Podbuf>>(data: &[u8]) {
    if let Ok(proof) = Ref::<'_, T, Podbuf>::verify(data) {
        let value = proof.decode();
        let mut out = <T as Payload<Podbuf>>::BUFFER;
        let encoded = Ref::<T, Podbuf>::encode(&value, out.as_mut())
            .expect("an accepted value encodes again");
        assert!(
            Ref::<T, Podbuf>::verify(encoded.bytes()).is_ok(),
            "the bytes written from an accepted value verify"
        );
    }
}

fuzz_target!(|data: &[u8]| {
    // Over-allocate and slice from the first 8-aligned address.
    let mut backing = vec![0u8; data.len() + 8];
    let start = backing.as_ptr().align_offset(8);
    let buf = &mut backing[start..start + data.len()];
    buf.copy_from_slice(data);
    let buf: &[u8] = buf;
    probe::<DoorPayload>(buf);
    probe::<ClimateReport>(buf);
    probe::<DiagFilter>(buf);
    probe::<FaultEvent>(buf);
    probe::<FaultPage>(buf);
    probe::<Outcome>(buf);
    probe::<Labels>(buf);
    probe::<Nested>(buf);
    probe::<Digest>(buf);
    probe::<Blob>(buf);
    probe::<Sparse>(buf);
    probe::<Stricts>(buf);
});
