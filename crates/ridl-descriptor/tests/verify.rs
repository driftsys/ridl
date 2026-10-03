//! Spec D-8: every reader checks the identifier and the version first, then
//! walks the whole buffer through checked accessors; a buffer that fails is
//! rejected as a whole.

use planus::ReadAsRoot;
use ridl_descriptor::{
    Catalog, CatalogRef, Encoding, FILE_IDENTIFIER, Interface, Kind, MaxSize, Member, Payload,
    RetiredInterface, SCHEMA_VERSION, SizeStateTag, Timing, TimingMode, UnboundedCause,
    VerifyError, finish, verify,
};

fn minimal(version: u32) -> Vec<u8> {
    let catalog = Catalog {
        version,
        name: "p".to_owned(),
        hash: vec![0u8; 32],
        toolchain: "0.0.0".to_owned(),
        interfaces: vec![],
        retired: vec![],
    };
    finish(&catalog)
}

#[test]
fn a_finished_buffer_verifies() {
    let bytes = minimal(SCHEMA_VERSION);
    let catalog = verify(&bytes).expect("verifies");
    assert_eq!(catalog.name().unwrap(), "p");
}

#[test]
fn finish_writes_the_root_offset_then_the_identifier() {
    let bytes = minimal(SCHEMA_VERSION);
    assert_eq!(&bytes[4..8], &FILE_IDENTIFIER);
    // The root offset at 0..4 is relative to byte 0 and points at the root
    // table, whose first 4 bytes are a signed offset back to its vtable.
    let root = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
    assert!(root >= 8 && root + 4 <= bytes.len(), "root offset {root}");
    let to_vtable = i32::from_le_bytes(bytes[root..root + 4].try_into().unwrap());
    let vtable = (root as i64 - i64::from(to_vtable)) as usize;
    assert!(vtable + 4 <= bytes.len(), "vtable at {vtable}");
    let table_size = u16::from_le_bytes(bytes[vtable + 2..vtable + 4].try_into().unwrap());
    assert!(table_size >= 4 && root + usize::from(table_size) <= bytes.len());
    assert!(verify(&bytes).is_ok());
}

#[test]
fn a_buffer_shorter_than_the_header_is_too_short() {
    assert!(matches!(verify(&[0, 0, 0]), Err(VerifyError::TooShort(3))));
    // 4 to 7 bytes hold a root offset but not the whole identifier.
    for len in 4..8 {
        let bytes = vec![0u8; len];
        assert!(
            matches!(verify(&bytes), Err(VerifyError::TooShort(n)) if n == len),
            "{len} bytes"
        );
    }
}

#[test]
fn a_foreign_identifier_is_rejected_before_any_read() {
    let mut bytes = minimal(SCHEMA_VERSION);
    bytes[4..8].copy_from_slice(b"RDLS");
    assert!(matches!(verify(&bytes), Err(VerifyError::WrongIdentifier(id)) if &id == b"RDLS"));
}

#[test]
fn version_zero_is_rejected() {
    // 0 is what a missing `version` field reads as.
    let bytes = minimal(0);
    assert!(matches!(verify(&bytes), Err(VerifyError::WrongVersion(0))));
}

#[test]
fn the_identifier_is_checked_before_the_body() {
    let mut bytes = minimal(SCHEMA_VERSION);
    bytes[4..8].copy_from_slice(b"RDLS");
    let past_the_end = bytes.len() as u32 + 64;
    bytes[0..4].copy_from_slice(&past_the_end.to_le_bytes());
    assert!(matches!(verify(&bytes), Err(VerifyError::WrongIdentifier(id)) if &id == b"RDLS"));
}

#[test]
fn the_version_is_checked_before_the_walk() {
    let mut bytes = minimal(SCHEMA_VERSION + 1);
    let name = field(&bytes, root(&bytes), CATALOG_NAME);
    point_past_the_end(&mut bytes, name);
    assert!(matches!(verify(&bytes), Err(VerifyError::WrongVersion(v)) if v == SCHEMA_VERSION + 1));
}

#[test]
fn an_unknown_version_is_rejected() {
    let bytes = minimal(SCHEMA_VERSION + 1);
    assert!(matches!(verify(&bytes), Err(VerifyError::WrongVersion(v)) if v == SCHEMA_VERSION + 1));
}

#[test]
fn a_truncated_buffer_is_rejected() {
    let bytes = minimal(SCHEMA_VERSION);
    let cut = &bytes[..bytes.len() - 6];
    assert!(matches!(verify(cut), Err(VerifyError::Invalid(_))));
}

/// A catalog with one table at every nesting level, whose deepest string,
/// the payload's type name, is the marker [`DEEP`].
fn nested() -> Vec<u8> {
    finish(&Catalog {
        version: SCHEMA_VERSION,
        name: "p".to_owned(),
        hash: vec![0u8; 32],
        toolchain: "0.0.0".to_owned(),
        interfaces: vec![Interface {
            name: "I".to_owned(),
            number: 1,
            provisional: false,
            members: vec![Member {
                name: "m".to_owned(),
                ordinal: 1,
                kind: Kind::Signal,
                payloads: vec![Payload {
                    role: "value".to_owned(),
                    type_name: DEEP.to_owned(),
                    max_sizes: vec![MaxSize {
                        encoding: Encoding::FlatBuffers,
                        bytes: 4,
                        state: SizeStateTag::Bounded,
                        cause: UnboundedCause::Unspecified,
                    }],
                }],
                timing: None,
            }],
            reserved_ordinals: vec![],
        }],
        retired: vec![],
    })
}

const DEEP: &str = "DeepTypeNameMarker";

#[test]
fn a_damaged_nested_string_is_rejected_although_the_root_reads() {
    let mut bytes = nested();
    assert!(verify(&bytes).is_ok());
    // A FlatBuffers string is a u32 length followed by the bytes; give the
    // payload's type name a length that runs past the end of the buffer.
    let at = bytes
        .windows(DEEP.len())
        .position(|w| w == DEEP.as_bytes())
        .expect("the marker is in the buffer");
    let past_the_end = bytes.len() as u32;
    bytes[at - 4..at].copy_from_slice(&past_the_end.to_le_bytes());
    // The root table and every root-level field still read: only the walk
    // over the nested tables finds the damage.
    let root = CatalogRef::read_as_root(&bytes).expect("the root still reads");
    assert_eq!(root.version().unwrap(), SCHEMA_VERSION);
    assert_eq!(root.name().unwrap(), "p");
    assert_eq!(root.toolchain().unwrap(), "0.0.0");
    assert!(matches!(verify(&bytes), Err(VerifyError::Invalid(_))));
}

#[test]
fn a_flipped_offset_is_rejected_as_a_whole() {
    let mut bytes = minimal(SCHEMA_VERSION);
    // The root uoffset at 0..4 points at the table; send it past the end.
    let past_the_end = bytes.len() as u32 + 64;
    bytes[0..4].copy_from_slice(&past_the_end.to_le_bytes());
    assert!(matches!(verify(&bytes), Err(VerifyError::Invalid(_))));
}

#[test]
fn the_error_names_its_cause() {
    assert_eq!(
        VerifyError::WrongIdentifier(*b"RDLS").to_string(),
        r#"not a catalog descriptor: file identifier "RDLS", expected "RDLC""#
    );
    assert_eq!(
        VerifyError::WrongVersion(9).to_string(),
        format!("catalog descriptor version 9; this toolchain reads version {SCHEMA_VERSION}")
    );
    assert_eq!(
        VerifyError::TooShort(3).to_string(),
        "3 bytes is shorter than a catalog descriptor header"
    );
}

// Field-by-field damage. Each case below damages exactly one field that
// `walk` reads, in a buffer whose root table and version still read, so
// only the walk's read of that one field can reject it.

/// A catalog in which every vector of tables holds two elements, and the
/// last element of each has every field `walk` reads present: every enum
/// there holds a value other than its default, because planus leaves a field
/// that holds its default out of the table. [`full_with`] varies two fields;
/// see [`Fixture`].
fn full() -> Vec<u8> {
    full_with(true, true)
}

fn full_with(provisional: bool, timing: bool) -> Vec<u8> {
    finish(&Catalog {
        version: SCHEMA_VERSION,
        name: "p".to_owned(),
        hash: vec![0u8; 32],
        toolchain: "0.0.0".to_owned(),
        interfaces: vec![
            Interface {
                name: "I0".to_owned(),
                number: 3,
                provisional: false,
                members: vec![],
                reserved_ordinals: vec![],
            },
            Interface {
                name: "I".to_owned(),
                number: 1,
                provisional,
                members: vec![
                    Member {
                        name: "m0".to_owned(),
                        ordinal: 2,
                        kind: Kind::Signal,
                        payloads: vec![],
                        timing: None,
                    },
                    Member {
                        name: "m".to_owned(),
                        ordinal: 1,
                        kind: Kind::Event,
                        payloads: vec![
                            Payload {
                                role: "r0".to_owned(),
                                type_name: "T0".to_owned(),
                                max_sizes: vec![],
                            },
                            Payload {
                                role: "value".to_owned(),
                                type_name: "T".to_owned(),
                                max_sizes: vec![
                                    MaxSize {
                                        encoding: Encoding::Proto3,
                                        bytes: 0,
                                        state: SizeStateTag::Bounded,
                                        cause: UnboundedCause::Unspecified,
                                    },
                                    MaxSize {
                                        encoding: Encoding::FlatBuffers,
                                        bytes: 4,
                                        state: SizeStateTag::Unbounded,
                                        cause: UnboundedCause::Member,
                                    },
                                ],
                            },
                        ],
                        timing: timing.then(|| {
                            Box::new(Timing {
                                mode: TimingMode::StrictPeriodic,
                                min_us: Some("1".to_owned()),
                                max_us: Some("2".to_owned()),
                            })
                        }),
                    },
                ],
                reserved_ordinals: vec![5, 6],
            },
        ],
        retired: vec![
            RetiredInterface {
                name: "R0".to_owned(),
                number: 4,
            },
            RetiredInterface {
                name: "R".to_owned(),
                number: 2,
            },
        ],
    })
}

// Field slots, in schema order (`schema/catalog.fbs`).
const CATALOG_NAME: usize = 1;
const CATALOG_HASH: usize = 2;
const CATALOG_TOOLCHAIN: usize = 3;
const CATALOG_INTERFACES: usize = 4;
const CATALOG_RETIRED: usize = 5;
const INTERFACE_NAME: usize = 0;
const INTERFACE_NUMBER: usize = 1;
const INTERFACE_PROVISIONAL: usize = 2;
const INTERFACE_MEMBERS: usize = 3;
const INTERFACE_RESERVED: usize = 4;
const MEMBER_NAME: usize = 0;
const MEMBER_ORDINAL: usize = 1;
const MEMBER_KIND: usize = 2;
const MEMBER_PAYLOADS: usize = 3;
const MEMBER_TIMING: usize = 4;
const TIMING_MODE: usize = 0;
const TIMING_MIN_US: usize = 1;
const TIMING_MAX_US: usize = 2;
const PAYLOAD_ROLE: usize = 0;
const PAYLOAD_TYPE_NAME: usize = 1;
const PAYLOAD_MAX_SIZES: usize = 2;
const SIZE_ENCODING: usize = 0;
const SIZE_BYTES: usize = 1;
const SIZE_STATE: usize = 2;
const SIZE_CAUSE: usize = 3;
const RETIRED_NAME: usize = 0;
const RETIRED_NUMBER: usize = 1;

fn u32_at(bytes: &[u8], at: usize) -> usize {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize
}

/// The position of the root table.
fn root(bytes: &[u8]) -> usize {
    u32_at(bytes, 0)
}

/// The position of the vtable entry for `slot` of the table at `table`, or
/// `None` when the vtable ends before it.
fn try_vtable_entry(bytes: &[u8], table: usize, slot: usize) -> Option<usize> {
    let back = i32::from_le_bytes(bytes[table..table + 4].try_into().unwrap());
    let vtable = (table as i64 - i64::from(back)) as usize;
    let size = u16::from_le_bytes(bytes[vtable..vtable + 2].try_into().unwrap()) as usize;
    let entry = vtable + 4 + 2 * slot;
    (entry + 2 <= vtable + size).then_some(entry)
}

/// The position of the vtable entry for `slot` of the table at `table`.
fn vtable_entry(bytes: &[u8], table: usize, slot: usize) -> usize {
    try_vtable_entry(bytes, table, slot).unwrap_or_else(|| panic!("slot {slot} is past the vtable"))
}

/// Whether field `slot` of the table at `table` is present.
fn has_field(bytes: &[u8], table: usize, slot: usize) -> bool {
    try_vtable_entry(bytes, table, slot)
        .is_some_and(|entry| u16::from_le_bytes(bytes[entry..entry + 2].try_into().unwrap()) != 0)
}

/// The position of field `slot` of the table at `table`.
fn field(bytes: &[u8], table: usize, slot: usize) -> usize {
    let entry = vtable_entry(bytes, table, slot);
    let offset = u16::from_le_bytes(bytes[entry..entry + 2].try_into().unwrap()) as usize;
    assert_ne!(offset, 0, "slot {slot} is absent from the table");
    table + offset
}

/// The position an offset field at `at` points to.
fn follow(bytes: &[u8], at: usize) -> usize {
    at + u32_at(bytes, at)
}

/// The position of the last table in the vector field `slot`. The cases
/// damage the last of two or more elements, so a walk that stops after the
/// first element of a vector misses the damage.
fn last_in(bytes: &[u8], table: usize, slot: usize) -> usize {
    let vector = follow(bytes, field(bytes, table, slot));
    let len = u32_at(bytes, vector);
    assert!(len >= 2, "slot {slot}: the fixture has {len} elements");
    follow(bytes, vector + 4 * len)
}

/// Makes the offset field at `at` point past the end of the buffer.
fn point_past_the_end(bytes: &mut [u8], at: usize) {
    let past_the_end = bytes.len() as u32;
    bytes[at..at + 4].copy_from_slice(&past_the_end.to_le_bytes());
}

/// One way to damage one field.
enum Damage {
    /// An offset field (a string, a vector or a table) points past the end.
    Offset,
    /// An enum field holds a tag its enum does not define.
    Tag,
    /// The vtable places a scalar field past the end of the buffer.
    Scalar,
}

fn damage(bytes: &mut [u8], table: usize, slot: usize, how: Damage) {
    match how {
        Damage::Offset => {
            let at = field(bytes, table, slot);
            point_past_the_end(bytes, at);
        }
        Damage::Tag => {
            let at = field(bytes, table, slot);
            bytes[at] = 200;
        }
        Damage::Scalar => {
            field(bytes, table, slot);
            let entry = vtable_entry(bytes, table, slot);
            bytes[entry..entry + 2].copy_from_slice(&u16::MAX.to_le_bytes());
        }
    }
}

/// Which buffer a case damages.
///
/// planus writes one vtable for every table whose vtable bytes are equal.
/// With every field present, an `Interface` and a `Member` have the same
/// vtable, so damaging a scalar's vtable entry in one damages the other too,
/// and the case would not show which read rejected the buffer. A scalar case
/// on one of the two uses a buffer in which the other leaves a field out.
#[derive(Clone, Copy)]
enum Fixture {
    /// Every field present.
    Full,
    /// The member has no timing, so its vtable is shorter.
    NoTiming,
    /// The interface is not provisional, so its vtable has no entry there.
    NotProvisional,
}

/// The positions of the tables a case can damage.
struct Tables {
    catalog: usize,
    interface: usize,
    member: usize,
    timing: Option<usize>,
    payload: usize,
    size: usize,
    retired: usize,
}

fn locate(bytes: &[u8]) -> Tables {
    let catalog = root(bytes);
    let interface = last_in(bytes, catalog, CATALOG_INTERFACES);
    let member = last_in(bytes, interface, INTERFACE_MEMBERS);
    let timing = has_field(bytes, member, MEMBER_TIMING)
        .then(|| follow(bytes, field(bytes, member, MEMBER_TIMING)));
    let payload = last_in(bytes, member, MEMBER_PAYLOADS);
    let size = last_in(bytes, payload, PAYLOAD_MAX_SIZES);
    let retired = last_in(bytes, catalog, CATALOG_RETIRED);
    Tables {
        catalog,
        interface,
        member,
        timing,
        payload,
        size,
        retired,
    }
}

#[test]
fn every_field_the_walk_reads_is_checked() {
    use Damage::{Offset, Scalar, Tag};
    use Fixture::{Full, NoTiming, NotProvisional};

    type Pick = fn(&Tables) -> usize;
    let catalog: Pick = |t| t.catalog;
    let interface: Pick = |t| t.interface;
    let member: Pick = |t| t.member;
    let timing: Pick = |t| t.timing.expect("this fixture has timing");
    let payload: Pick = |t| t.payload;
    let size: Pick = |t| t.size;
    let retired: Pick = |t| t.retired;

    let cases = [
        ("catalog.name", Full, catalog, CATALOG_NAME, Offset),
        ("catalog.hash", Full, catalog, CATALOG_HASH, Offset),
        (
            "catalog.toolchain",
            Full,
            catalog,
            CATALOG_TOOLCHAIN,
            Offset,
        ),
        (
            "catalog.interfaces",
            Full,
            catalog,
            CATALOG_INTERFACES,
            Offset,
        ),
        ("catalog.retired", Full, catalog, CATALOG_RETIRED, Offset),
        ("interface.name", Full, interface, INTERFACE_NAME, Offset),
        (
            "interface.number",
            NoTiming,
            interface,
            INTERFACE_NUMBER,
            Scalar,
        ),
        (
            "interface.provisional",
            NoTiming,
            interface,
            INTERFACE_PROVISIONAL,
            Scalar,
        ),
        (
            "interface.members",
            Full,
            interface,
            INTERFACE_MEMBERS,
            Offset,
        ),
        (
            "interface.reserved_ordinals",
            Full,
            interface,
            INTERFACE_RESERVED,
            Offset,
        ),
        ("member.name", Full, member, MEMBER_NAME, Offset),
        (
            "member.ordinal",
            NotProvisional,
            member,
            MEMBER_ORDINAL,
            Scalar,
        ),
        ("member.kind", Full, member, MEMBER_KIND, Tag),
        ("member.payloads", Full, member, MEMBER_PAYLOADS, Offset),
        ("member.timing", Full, member, MEMBER_TIMING, Offset),
        ("timing.mode", Full, timing, TIMING_MODE, Tag),
        ("timing.min_us", Full, timing, TIMING_MIN_US, Offset),
        ("timing.max_us", Full, timing, TIMING_MAX_US, Offset),
        ("payload.role", Full, payload, PAYLOAD_ROLE, Offset),
        (
            "payload.type_name",
            Full,
            payload,
            PAYLOAD_TYPE_NAME,
            Offset,
        ),
        (
            "payload.max_sizes",
            Full,
            payload,
            PAYLOAD_MAX_SIZES,
            Offset,
        ),
        ("size.encoding", Full, size, SIZE_ENCODING, Tag),
        ("size.bytes", Full, size, SIZE_BYTES, Scalar),
        ("size.state", Full, size, SIZE_STATE, Tag),
        ("size.cause", Full, size, SIZE_CAUSE, Tag),
        ("retired.name", Full, retired, RETIRED_NAME, Offset),
        ("retired.number", Full, retired, RETIRED_NUMBER, Scalar),
    ];
    let mut accepted = Vec::new();
    for (name, fixture, pick, slot, how) in cases {
        let mut bytes = match fixture {
            Full => full(),
            NoTiming => full_with(true, false),
            NotProvisional => full_with(false, true),
        };
        assert!(verify(&bytes).is_ok(), "{name}: the clean buffer verifies");
        let table = pick(&locate(&bytes));
        damage(&mut bytes, table, slot, how);
        // The root and the version still read: the damage is below them.
        let view = CatalogRef::read_as_root(&bytes).expect("the root still reads");
        assert_eq!(view.version().unwrap(), SCHEMA_VERSION, "{name}");
        if !matches!(verify(&bytes), Err(VerifyError::Invalid(_))) {
            accepted.push(name);
        }
    }
    assert!(
        accepted.is_empty(),
        "verify accepted a buffer with one damaged field: {accepted:?}"
    );
}
