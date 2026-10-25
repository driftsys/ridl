//! The encoding a `Loopback` and its handles carry.
//!
//! `Loopback<E>` and every role handle implement
//! [`Encoded`](ridl_rt::port::Encoded), so a generated face reads the
//! encoding from its port type. The default is FlatBuffers, chosen by
//! `Loopback::new`; `Loopback::<E>::with_encoding` names another. The
//! encoding is a type and not a value: two runtimes over one catalog with
//! different encodings are two stores.

use ridl_loopback::Loopback;
use ridl_rt::contract::{CatalogHash, CatalogRef, InterfaceNo, Ordinal};
use ridl_rt::encoding::{Encoding, Podbuf, Proto3};
use ridl_rt::port::{Encoded, SignalReader, SignalWriter};
use ridl_rt::sample::Provenance;

fn catalog() -> CatalogRef {
    CatalogRef {
        name: "face.demo",
        hash: CatalogHash([0u8; 32]),
    }
}

/// The name of the encoding a port's type carries, read through `Encoded`
/// alone, so a port that does not implement it fails to compile here.
fn encoding_of<P: Encoded>(_: &P) -> &'static str {
    P::Encoding::NAME
}

#[test]
fn the_default_encoding_is_flatbuffers() {
    let rt = Loopback::new(catalog());
    assert_eq!(encoding_of(&rt), "flatbuffers");
    assert_eq!(encoding_of(&rt.attach()), "flatbuffers");
    assert_eq!(encoding_of(&rt.handler()), "flatbuffers");
    assert_eq!(encoding_of(&rt.split().handler), "flatbuffers");
}

#[test]
fn with_encoding_types_every_handle_that_meets_a_face() {
    let rt = Loopback::<Proto3>::with_encoding(catalog());
    assert_eq!(encoding_of(&rt), "proto3");
    assert_eq!(encoding_of(&rt.attach()), "proto3");

    // Each of the six role handles the aggregate hands out.
    assert_eq!(encoding_of(&rt.reader()), "proto3");
    assert_eq!(encoding_of(&rt.writer()), "proto3");
    assert_eq!(encoding_of(&rt.source()), "proto3");
    assert_eq!(encoding_of(&rt.sink()), "proto3");
    assert_eq!(encoding_of(&rt.caller()), "proto3");
    assert_eq!(encoding_of(&rt.handler()), "proto3");

    // And the six it holds, as `split` hands them out.
    let handles = rt.split();
    assert_eq!(encoding_of(&handles.reader), "proto3");
    assert_eq!(encoding_of(&handles.writer), "proto3");
    assert_eq!(encoding_of(&handles.source), "proto3");
    assert_eq!(encoding_of(&handles.sink), "proto3");
    assert_eq!(encoding_of(&handles.caller), "proto3");
    assert_eq!(encoding_of(&handles.handler), "proto3");
}

#[test]
fn a_mut_borrow_of_a_handle_is_encoded() {
    let mut rt = Loopback::<Proto3>::with_encoding(catalog());
    assert_eq!(encoding_of(&&mut rt), "proto3");
    assert_eq!(encoding_of(&&mut rt.handler()), "proto3");
}

#[test]
fn the_encoding_is_a_type_and_not_a_value() {
    let mut proto = Loopback::<Proto3>::with_encoding(catalog());
    let mut flat = Loopback::new(catalog());

    proto.set(InterfaceNo(1), Ordinal(1), &[1]).expect("staged");
    proto.commit();
    flat.set(InterfaceNo(1), Ordinal(2), &[2]).expect("staged");
    flat.commit();

    // Each runtime is its own store: what one published, the other has not.
    let mut out = [0u8; 8];
    let sample = proto
        .read(InterfaceNo(1), Ordinal(1), &mut out)
        .expect("read back");
    assert_eq!(&out[..sample.len], &[1]);
    let unpublished = proto
        .read(InterfaceNo(1), Ordinal(2), &mut out)
        .expect("read");
    assert_eq!(unpublished.provenance, Provenance::Init);
    let sample = flat
        .read(InterfaceNo(1), Ordinal(2), &mut out)
        .expect("read back");
    assert_eq!(&out[..sample.len], &[2]);
    let unpublished = flat
        .read(InterfaceNo(1), Ordinal(1), &mut out)
        .expect("read");
    assert_eq!(unpublished.provenance, Provenance::Init);

    let podbuf = Loopback::<Podbuf>::with_encoding(catalog());
    assert_eq!(encoding_of(&podbuf), "podbuf");
}
