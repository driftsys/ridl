//! Spec D-8: every reader checks the identifier and the version first, then
//! walks the whole buffer through checked accessors; a buffer that fails is
//! rejected as a whole.

use planus::ReadAsRoot;
use ridl_descriptor::{
    Catalog, CatalogRef, Encoding, FILE_IDENTIFIER, Interface, Kind, MaxSize, Member, Payload,
    SCHEMA_VERSION, SizeStateTag, UnboundedCause, VerifyError, finish, verify,
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
}

#[test]
fn a_foreign_identifier_is_rejected_before_any_read() {
    let mut bytes = minimal(SCHEMA_VERSION);
    bytes[4..8].copy_from_slice(b"RDLS");
    assert!(matches!(verify(&bytes), Err(VerifyError::WrongIdentifier(id)) if &id == b"RDLS"));
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
    let text = VerifyError::WrongIdentifier(*b"RDLS").to_string();
    assert!(text.contains("RDLS") && text.contains("RDLC"), "{text}");
    let text = VerifyError::WrongVersion(9).to_string();
    assert!(
        text.contains('9') && text.contains(&SCHEMA_VERSION.to_string()),
        "{text}"
    );
}
