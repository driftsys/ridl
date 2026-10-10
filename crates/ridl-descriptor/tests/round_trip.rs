//! Spec §4 "Schema round trip": build a descriptor with the Rust builder,
//! finish it with the file identifier, read every field back.
//!
//! The buffers come from [`ridl_descriptor::finish`], which writes the
//! FlatBuffers header layout that planus 1.3.0 does not write when it is
//! given a file identifier.
//! [`planus_writes_the_identifier_before_the_root_offset`] fails when a planus
//! release changes that behaviour, so the workaround in `finish` is removed
//! then.

use planus::ReadAsRoot;
use ridl_descriptor::{
    Catalog, CatalogRef, EarlierCatalog, Encoding, FILE_IDENTIFIER, Interface, Kind, MaxSize,
    Member, Payload, RetiredInterface, SCHEMA_VERSION, SizeStateTag, Timing, TimingMode,
    UnboundedCause, finish,
};

fn sample() -> Catalog {
    Catalog {
        version: SCHEMA_VERSION,
        name: "veh.cluster".to_owned(),
        hash: vec![7u8; 32],
        toolchain: "0.0.0".to_owned(),
        interfaces: vec![Interface {
            name: "VehicleStatus".to_owned(),
            number: 1,
            provisional: true,
            members: vec![Member {
                name: "currentSpeed".to_owned(),
                ordinal: 1,
                kind: Kind::Signal,
                payloads: vec![Payload {
                    role: "value".to_owned(),
                    type_name: "Speed".to_owned(),
                    max_sizes: vec![
                        MaxSize {
                            encoding: Encoding::Proto3,
                            bytes: 0,
                            state: SizeStateTag::Unbounded,
                            cause: UnboundedCause::Member,
                        },
                        MaxSize {
                            encoding: Encoding::FlatBuffers,
                            bytes: 40,
                            state: SizeStateTag::Bounded,
                            cause: UnboundedCause::Unspecified,
                        },
                    ],
                }],
                timing: Some(Box::new(Timing {
                    mode: TimingMode::StrictPeriodic,
                    min_us: Some("100000".to_owned()),
                    max_us: None,
                })),
            }],
            reserved_ordinals: vec![5],
        }],
        retired: vec![RetiredInterface {
            name: "LaneAssist".to_owned(),
            number: 2,
        }],
        compatible: None,
    }
}

#[test]
fn every_field_reads_back() {
    let bytes = finish(&sample());

    assert_eq!(&bytes[4..8], &FILE_IDENTIFIER);
    let catalog = CatalogRef::read_as_root(&bytes).expect("a finished buffer reads");
    assert_eq!(catalog.version().unwrap(), SCHEMA_VERSION);
    assert_eq!(catalog.name().unwrap(), "veh.cluster");
    assert_eq!(catalog.hash().unwrap().len(), 32);
    assert_eq!(catalog.toolchain().unwrap(), "0.0.0");

    let interfaces = catalog.interfaces().unwrap();
    assert_eq!(interfaces.len(), 1);
    let interface = interfaces.get(0).unwrap().unwrap();
    assert_eq!(interface.name().unwrap(), "VehicleStatus");
    assert_eq!(interface.number().unwrap(), 1);
    assert!(interface.provisional().unwrap());
    assert_eq!(
        interface
            .reserved_ordinals()
            .unwrap()
            .iter()
            .collect::<Vec<u32>>(),
        vec![5]
    );

    let member = interface.members().unwrap().get(0).unwrap().unwrap();
    assert_eq!(member.name().unwrap(), "currentSpeed");
    assert_eq!(member.ordinal().unwrap(), 1);
    assert_eq!(member.kind().unwrap(), Kind::Signal);
    let timing = member.timing().unwrap().expect("timing is present");
    assert_eq!(timing.mode().unwrap(), TimingMode::StrictPeriodic);
    assert_eq!(timing.min_us().unwrap(), Some("100000"));
    assert_eq!(timing.max_us().unwrap(), None);

    let payload = member.payloads().unwrap().get(0).unwrap().unwrap();
    assert_eq!(payload.role().unwrap(), "value");
    assert_eq!(payload.type_name().unwrap(), "Speed");
    let sizes = payload.max_sizes().unwrap();
    assert_eq!(
        sizes.get(0).unwrap().unwrap().state().unwrap(),
        SizeStateTag::Unbounded
    );
    assert_eq!(
        sizes.get(0).unwrap().unwrap().cause().unwrap(),
        UnboundedCause::Member
    );
    assert_eq!(
        sizes.get(1).unwrap().unwrap().encoding().unwrap(),
        Encoding::FlatBuffers
    );
    assert_eq!(
        sizes.get(1).unwrap().unwrap().state().unwrap(),
        SizeStateTag::Bounded
    );
    assert_eq!(sizes.get(1).unwrap().unwrap().bytes().unwrap(), 40);

    let retired = catalog.retired().unwrap().get(0).unwrap().unwrap();
    assert_eq!(retired.name().unwrap(), "LaneAssist");
    assert_eq!(retired.number().unwrap(), 2);
}

#[test]
fn the_owned_form_round_trips_through_the_view() {
    let bytes = finish(&sample());
    let view = CatalogRef::read_as_root(&bytes).unwrap();
    let owned: Catalog = view.try_into().expect("a valid view converts");
    assert_eq!(owned, sample());
}

/// Pins the planus behaviour [`ridl_descriptor::finish`] works around, and so
/// proves that function is still needed. When this fails, a planus release
/// writes the standard layout itself: make `finish` call
/// `planus::Builder::finish` directly.
#[test]
fn planus_writes_the_identifier_before_the_root_offset() {
    let mut builder = planus::Builder::new();
    let bytes = builder.finish(sample(), Some(FILE_IDENTIFIER)).to_vec();
    assert_eq!(&bytes[0..4], &FILE_IDENTIFIER);
    assert!(CatalogRef::read_as_root(&bytes).is_err());
}

/// The builder's string cache (planus's `string-cache` feature, which this
/// crate's `dedup` feature turns on) writes a repeated string once. Fails when
/// that feature is dropped from `dedup`, which would change the bytes of every
/// descriptor. With `dedup` off the cache is absent and the test does not run.
#[test]
#[cfg(feature = "dedup")]
fn a_repeated_string_is_written_once() {
    let mut catalog = sample();
    let mut second = catalog.interfaces[0].clone();
    second.name = "Doors".to_owned();
    second.number = 3;
    catalog.interfaces.push(second);
    let bytes = finish(&catalog);
    // A FlatBuffers string: its length as a little-endian u32, the bytes, a NUL.
    let needle = [&5u32.to_le_bytes()[..], b"value", &[0]].concat();
    let count = bytes
        .windows(needle.len())
        .filter(|w| *w == needle.as_slice())
        .count();
    assert_eq!(count, 1, "the role `value` is written once");
    // The vtable cache shows in the length: the two interfaces, and their
    // members and payloads, have the same shape and share one vtable each.
    // Without planus's `vtable-cache` (part of `dedup`) the same catalog is
    // 652 bytes. The byte-vector cache has nothing to share in this catalog;
    // `a_repeated_byte_vector_is_written_once` pins it.
    assert_eq!(
        bytes.len(),
        576,
        "the buffer length with the vtable and string caches on"
    );
}

/// The builder's byte-vector cache (planus's `bytes-cache` feature, which this
/// crate's `dedup` feature turns on) writes a repeated byte vector once. Fails
/// when that feature is dropped from `dedup`. With `dedup` off the cache is
/// absent and the test does not run.
#[test]
#[cfg(feature = "dedup")]
fn a_repeated_byte_vector_is_written_once() {
    let mut catalog = sample();
    // An earlier catalog whose hash is the same 32 bytes as this catalog's.
    catalog.compatible = Some(vec![EarlierCatalog {
        hash: catalog.hash.clone(),
    }]);
    let bytes = finish(&catalog);
    // Both copies read back, so a count of one below can only come from the
    // cache, not from a `compatible` list that was never written.
    let view = CatalogRef::read_as_root(&bytes).expect("a finished buffer reads");
    let compatible = view.compatible().unwrap().expect("the compatible list");
    assert_eq!(compatible.len(), 1);
    let earlier = compatible.get(0).unwrap().unwrap();
    assert_eq!(earlier.hash().unwrap(), catalog.hash.as_slice());
    // A FlatBuffers vector: its length as a little-endian u32, then the bytes.
    let len = u32::try_from(catalog.hash.len()).unwrap();
    let needle = [&len.to_le_bytes()[..], catalog.hash.as_slice()].concat();
    let count = bytes
        .windows(needle.len())
        .filter(|w| *w == needle.as_slice())
        .count();
    assert_eq!(count, 1, "the 32-byte hash is written once");
}
