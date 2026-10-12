//! `port::Encoded`: a port type states the encoding of the bytes it carries,
//! and a shared or mutable borrow of a port carries the same encoding.

#![forbid(unsafe_code)]

use ridl_rt::encoding::{Encoding, FlatBuffers};
use ridl_rt::port::Encoded;

/// A port that carries FlatBuffers bytes. It implements no other port trait:
/// `Encoded` is not a supertrait of `Attached`.
struct P;

impl Encoded for P {
    type Encoding = FlatBuffers;
}

/// Reads the encoding through the trait alone, as a generated face does from
/// its port type parameter.
fn name<Q: Encoded>(_: &Q) -> &'static str {
    Q::Encoding::NAME
}

#[test]
fn a_mut_borrow_carries_the_ports_encoding() {
    let mut port = P;
    assert_eq!(name(&port), "flatbuffers");
    assert_eq!(name(&&mut port), "flatbuffers");
}

#[test]
fn a_shared_borrow_carries_the_ports_encoding() {
    let port = P;
    assert_eq!(name(&&port), "flatbuffers");
}

/// Compiles only when `Q::Encoding` is the same type as `E`.
fn same_encoding<Q: Encoded<Encoding = E>, E: Encoding>() {}

#[test]
fn borrows_have_the_same_encoding_type_as_the_port() {
    same_encoding::<&P, FlatBuffers>();
    same_encoding::<&mut P, FlatBuffers>();
}
