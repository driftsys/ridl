//! Arbitrary bytes through `ridl_transport_ws::frame::decode`.
//!
//! A message that decodes must encode again to the same bytes. The only
//! allowed failure is a returned error.

#![no_main]

use libfuzzer_sys::fuzz_target;
use ridl_transport_ws::frame::decode;

fuzz_target!(|data: &[u8]| {
    if let Ok(frame) = decode(data) {
        let mut out = vec![0u8; frame.encoded_len()];
        let len = frame
            .encode(&mut out)
            .expect("an accepted frame encodes again");
        assert_eq!(&out[..len], data, "an accepted message re-encodes exactly");
        assert_eq!(decode(&out[..len]), Ok(frame));
    }
});
