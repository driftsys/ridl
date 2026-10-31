//! `MemberTable::from_descriptor`: the member table read from the bytes of a
//! catalog descriptor, and the refusals of a descriptor it cannot use.

use std::time::Duration;

use ridl_descriptor::{
    finish, Catalog, EarlierCatalog, Interface, Member, RetiredInterface, Timing, TimingMode,
    VerifyError, SCHEMA_VERSION,
};
use ridl_rt::contract::{CatalogHash, InterfaceNo, Kind, Ordinal};
use ridl_transport_ws::{DescriptorError, MemberTable};

fn member(ordinal: u32, kind: ridl_descriptor::Kind, max_us: Option<&str>) -> Member {
    Member {
        name: format!("m{ordinal}"),
        ordinal,
        kind,
        payloads: vec![],
        timing: max_us.map(|max| {
            Box::new(Timing {
                mode: TimingMode::Unspecified,
                min_us: None,
                max_us: Some(max.to_owned()),
            })
        }),
        limit: None,
        streams: None,
    }
}

/// The scenario catalog: interface `I` number 1 with a signal, an event, a
/// command with a 1 s bound, a query with a 250 ms bound, a fixed member and
/// the reserved ordinal 6; a retired interface `R` number 9; one compatible
/// hash.
fn catalog() -> Catalog {
    Catalog {
        version: SCHEMA_VERSION,
        name: "ws.test".to_owned(),
        hash: vec![7; 32],
        toolchain: "0.0.0".to_owned(),
        interfaces: vec![Interface {
            name: "I".to_owned(),
            number: 1,
            provisional: false,
            members: vec![
                member(1, ridl_descriptor::Kind::Signal, None),
                member(2, ridl_descriptor::Kind::Event, None),
                member(3, ridl_descriptor::Kind::Command, Some("1000000")),
                member(4, ridl_descriptor::Kind::Query, Some("250000")),
                member(5, ridl_descriptor::Kind::Fixed, None),
            ],
            reserved_ordinals: vec![6],
        }],
        retired: vec![RetiredInterface {
            name: "R".to_owned(),
            number: 9,
        }],
        compatible: Some(vec![EarlierCatalog { hash: vec![6; 32] }]),
    }
}

fn descriptor() -> Vec<u8> {
    finish(&catalog())
}

fn descriptor_with(f: impl FnOnce(&mut Catalog)) -> Vec<u8> {
    let mut c = catalog();
    f(&mut c);
    finish(&c)
}

#[test]
fn a_descriptor_gives_the_members_their_kinds_and_bounds_and_no_entry_for_reserved_or_retired() {
    let t = MemberTable::from_descriptor(&descriptor()).unwrap();
    assert_eq!((t.name(), t.hash()), ("ws.test", &CatalogHash([7; 32])));
    let i = InterfaceNo(1);
    assert_eq!(t.member(i, Ordinal(1)), Some((Kind::Signal, None)));
    assert_eq!(t.member(i, Ordinal(2)), Some((Kind::Event, None)));
    assert_eq!(
        t.member(i, Ordinal(3)),
        Some((Kind::Command, Some(Duration::from_secs(1))))
    );
    assert_eq!(
        t.member(i, Ordinal(4)),
        Some((Kind::Query, Some(Duration::from_millis(250))))
    );
    assert_eq!(t.member(i, Ordinal(5)), Some((Kind::Fixed, None)));
    assert_eq!(t.member(i, Ordinal(6)), None);
    assert_eq!(t.member(InterfaceNo(9), Ordinal(1)), None);
    assert_eq!(t.len(), 5);
    assert_eq!(t.compatible(), &[CatalogHash([6; 32])]);
}

#[test]
fn each_members_limit_is_read_and_an_absent_one_is_none() {
    let bytes = descriptor_with(|c| c.interfaces[0].members[0].limit = Some(64));
    let t = MemberTable::from_descriptor(&bytes).unwrap();
    let i = InterfaceNo(1);
    assert_eq!(t.limit(i, Ordinal(1)), Some(64));
    assert_eq!(t.limit(i, Ordinal(2)), None);
    assert_eq!(t.limit(InterfaceNo(9), Ordinal(1)), None);
}

#[test]
fn a_malformed_buffer_is_a_verify_error() {
    let mut bytes = descriptor();
    bytes.truncate(12);
    assert!(matches!(
        MemberTable::from_descriptor(&bytes),
        Err(DescriptorError::Verify(_))
    ));
    assert!(matches!(
        MemberTable::from_descriptor(b"RDLC"),
        Err(DescriptorError::Verify(VerifyError::TooShort(4)))
    ));
}

#[test]
fn a_max_us_that_is_not_a_decimal_u64_is_refused() {
    let bytes = descriptor_with(|c| {
        c.interfaces[0].members[2].timing.as_mut().unwrap().max_us = Some("1s".to_owned());
    });
    assert!(matches!(
        MemberTable::from_descriptor(&bytes),
        Err(DescriptorError::MaxUs { iface: InterfaceNo(1), ord: Ordinal(3), ref text }) if text == "1s"
    ));
}

#[test]
fn a_zero_interface_number_and_a_zero_ordinal_are_refused() {
    let bytes = descriptor_with(|c| c.interfaces[0].number = 0);
    assert!(matches!(
        MemberTable::from_descriptor(&bytes),
        Err(DescriptorError::ZeroNumber { ref interface }) if interface == "I"
    ));
    let bytes = descriptor_with(|c| c.interfaces[0].members[0].ordinal = 0);
    assert!(matches!(
        MemberTable::from_descriptor(&bytes),
        Err(DescriptorError::ZeroOrdinal { ref interface, ref member })
            if interface == "I" && member == "m1"
    ));
}

#[test]
fn a_hash_that_is_not_32_bytes_is_refused() {
    let bytes = descriptor_with(|c| c.hash = vec![7; 31]);
    assert!(matches!(
        MemberTable::from_descriptor(&bytes),
        Err(DescriptorError::HashLength { len: 31 })
    ));
    let bytes =
        descriptor_with(|c| c.compatible = Some(vec![EarlierCatalog { hash: vec![6; 33] }]));
    assert!(matches!(
        MemberTable::from_descriptor(&bytes),
        Err(DescriptorError::HashLength { len: 33 })
    ));
}

/// A `compatible` list that repeats a hash, or names the catalog's own hash,
/// gives each compatible hash once and never the own hash.
#[test]
fn a_repeated_or_own_hash_in_compatible_is_kept_once() {
    let bytes = descriptor_with(|c| {
        c.compatible = Some(vec![
            EarlierCatalog { hash: vec![6; 32] },
            EarlierCatalog { hash: vec![6; 32] },
            EarlierCatalog { hash: vec![7; 32] },
        ]);
    });
    let t = MemberTable::from_descriptor(&bytes).unwrap();
    assert_eq!(t.compatible(), &[CatalogHash([6; 32])]);
    assert!(t.accepts(&CatalogHash([7; 32])) && t.accepts(&CatalogHash([6; 32])));
}

/// A descriptor written before the `compatible` field existed lists no
/// compatible hash.
#[test]
fn an_absent_compatible_field_lists_no_compatible_hash() {
    let bytes = descriptor_with(|c| c.compatible = None);
    let t = MemberTable::from_descriptor(&bytes).unwrap();
    assert!(t.compatible().is_empty());
}

/// Each kind maps by name, not by discriminant (the two enums number them
/// differently).
#[test]
fn each_kind_maps_to_the_same_named_kind() {
    for (d, r) in [
        (ridl_descriptor::Kind::Signal, Kind::Signal),
        (ridl_descriptor::Kind::Event, Kind::Event),
        (ridl_descriptor::Kind::Command, Kind::Command),
        (ridl_descriptor::Kind::Query, Kind::Query),
        (ridl_descriptor::Kind::Fixed, Kind::Fixed),
    ] {
        let bytes = descriptor_with(|c| c.interfaces[0].members[0].kind = d);
        let t = MemberTable::from_descriptor(&bytes).unwrap();
        assert_eq!(t.member(InterfaceNo(1), Ordinal(1)).unwrap().0, r);
    }
}

/// The error names what is wrong in plain text.
#[test]
fn each_error_displays_its_cause() {
    let display = |f: fn(&mut Catalog)| {
        MemberTable::from_descriptor(&descriptor_with(f))
            .unwrap_err()
            .to_string()
    };
    assert_eq!(
        display(|c| c.hash = vec![7; 31]),
        "a catalog hash of 31 bytes; a catalog hash is 32 bytes"
    );
    assert_eq!(
        display(|c| c.interfaces[0].number = 0),
        "interface `I` has the number 0"
    );
    assert_eq!(
        display(|c| c.interfaces[0].members[0].ordinal = 0),
        "member `I.m1` has the ordinal 0"
    );
    assert_eq!(
        display(|c| {
            c.interfaces[0].members[2].timing.as_mut().unwrap().max_us = Some("1s".to_owned());
        }),
        "member 1.3 has the max_us \"1s\", which is not a decimal u64"
    );
}
