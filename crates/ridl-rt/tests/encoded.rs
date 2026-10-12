//! `port::Encoded`: a port type states the encoding of the bytes it carries,
//! and a shared or mutable borrow of a port carries the same encoding.

#![forbid(unsafe_code)]

use ridl_rt::encoding::{Encoding, FlatBuffers, Proto3};
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

/// A port that carries Protobuf bytes, so an impl that fixes the encoding
/// to one type does not pass for both fixtures.
struct Pb;

impl Encoded for Pb {
    type Encoding = Proto3;
}

/// Compiles only when `Q::Encoding` is the same type as `E`.
fn same_encoding<Q: Encoded<Encoding = E>, E: Encoding>() {}

/// A shared and a mutable borrow of `T` carry `T`'s encoding. `T` may be
/// unsized, which pins `?Sized` on both forwarding impls.
fn forwards<T: Encoded + ?Sized>() {
    same_encoding::<&T, T::Encoding>();
    same_encoding::<&mut T, T::Encoding>();
}

#[test]
fn borrows_have_the_same_encoding_type_as_the_port() {
    forwards::<P>();
    forwards::<Pb>();
    forwards::<dyn Encoded<Encoding = Proto3>>();
}

#[test]
fn a_shared_borrow_reads_the_second_ports_encoding_at_runtime() {
    let port = Pb;
    assert_eq!(name(&&port), "proto3");
}
