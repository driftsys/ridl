//! The catalog descriptor: the FlatBuffers file per package that an engine
//! reads without decoding (`docs/wip/2026-09-13-runtime-descriptors-design.md`).
//!
//! `schema/catalog.fbs` is the schema; `generated.rs` holds the accessors
//! planus generates from it (`cargo xtask descriptor-codegen`). [`finish`]
//! turns an owned [`Catalog`] into a descriptor buffer, and [`verify`] checks
//! a buffer before the first read of it. This crate does no I/O: a caller
//! writes the bytes [`finish`] returns and passes the bytes it read to
//! [`verify`].

pub mod generated;

use planus::ReadAsRoot;

pub use generated::ridl::descriptor::{
    Catalog, CatalogRef, Encoding, Interface, InterfaceRef, Kind, MaxSize, MaxSizeRef, Member,
    MemberRef, Payload, PayloadRef, RetiredInterface, RetiredInterfaceRef,
    SizeState as SizeStateTag, Timing, TimingMode, TimingRef, UnboundedCause,
};

/// The schema version this toolchain writes and accepts.
pub const SCHEMA_VERSION: u32 = 1;

/// The FlatBuffers file identifier, at bytes 4..8 of every catalog descriptor.
pub const FILE_IDENTIFIER: [u8; 4] = *b"RDLC";

/// The artifact suffix: `<base>.catalog.binfb` (ADR-0014 decision 4's
/// convention — a plain-English flag value, an encoding-bearing extension).
pub const FILE_SUFFIX: &str = ".catalog.binfb";

/// Builds the descriptor buffer for `catalog`: the root offset at bytes
/// 0..4, [`FILE_IDENTIFIER`] at bytes 4..8, the layout the FlatBuffers
/// specification defines. This is the one way this crate turns an owned
/// [`Catalog`] into bytes.
///
/// planus 1.3.0 does not write that layout when it is given a file
/// identifier: `Builder::finish(root, Some(id))` writes the identifier at
/// bytes 0..4 and the root offset at bytes 4..8, and planus's own
/// `read_as_root` then rejects the buffer. This function calls planus and
/// then swaps the two header fields. planus reserves the 8 header bytes with
/// the root's alignment, and the root offset it writes is relative to byte 4,
/// where it put it, so moving it to byte 0 adds 4 to it and nothing after
/// byte 8 moves. The test `planus_writes_the_identifier_before_the_root_offset`
/// in `tests/round_trip.rs` fails when a planus release changes that
/// behaviour, which is when this function can call planus directly.
pub fn finish(catalog: &Catalog) -> Vec<u8> {
    let mut builder = planus::Builder::new();
    let mut bytes = builder.finish(catalog, Some(FILE_IDENTIFIER)).to_vec();
    let from_byte_4 = u32::from_le_bytes(bytes[4..8].try_into().expect("four bytes"));
    bytes[0..4].copy_from_slice(&(from_byte_4 + 4).to_le_bytes());
    bytes[4..8].copy_from_slice(&FILE_IDENTIFIER);
    bytes
}

/// Why a buffer is not a catalog descriptor this toolchain reads.
#[derive(Debug)]
pub enum VerifyError {
    /// Fewer bytes than the root offset and the identifier need.
    TooShort(usize),
    /// Bytes 4..8 are not `FILE_IDENTIFIER`: another kind of file.
    WrongIdentifier([u8; 4]),
    /// A schema version this toolchain does not know.
    WrongVersion(u32),
    /// The FlatBuffers structure is not sound: an offset or a length points
    /// outside the buffer, a required field is missing, an enum tag is unknown.
    Invalid(planus::Error),
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort(len) => {
                write!(f, "{len} bytes is shorter than a catalog descriptor header")
            }
            Self::WrongIdentifier(id) => write!(
                f,
                "not a catalog descriptor: file identifier {:?}, expected {:?}",
                String::from_utf8_lossy(id),
                String::from_utf8_lossy(&FILE_IDENTIFIER),
            ),
            Self::WrongVersion(v) => write!(
                f,
                "catalog descriptor version {v}; this toolchain reads version {SCHEMA_VERSION}"
            ),
            Self::Invalid(err) => write!(f, "catalog descriptor is malformed: {err}"),
        }
    }
}

impl std::error::Error for VerifyError {}

/// Checks the identifier and the version, then walks every table, vector
/// and string once through the checked accessors so that a later read of
/// the returned view cannot fail on a malformed offset.
pub fn verify(bytes: &[u8]) -> Result<CatalogRef<'_>, VerifyError> {
    if bytes.len() < 8 {
        return Err(VerifyError::TooShort(bytes.len()));
    }
    let id: [u8; 4] = bytes[4..8].try_into().expect("four bytes");
    if id != FILE_IDENTIFIER {
        return Err(VerifyError::WrongIdentifier(id));
    }
    let catalog = CatalogRef::read_as_root(bytes).map_err(VerifyError::Invalid)?;
    let version = catalog.version().map_err(VerifyError::Invalid)?;
    if version != SCHEMA_VERSION {
        return Err(VerifyError::WrongVersion(version));
    }
    walk(catalog).map_err(VerifyError::Invalid)?;
    Ok(catalog)
}

/// Touches every field once. planus checks each access; a walk over all
/// of them is the whole-buffer verification of D-8.
fn walk(catalog: CatalogRef<'_>) -> planus::Result<()> {
    catalog.name()?;
    catalog.hash()?;
    catalog.toolchain()?;
    for interface in catalog.interfaces()? {
        let interface = interface?;
        interface.name()?;
        interface.number()?;
        interface.provisional()?;
        // A `u32` element reads without a check once the vector's bounds
        // are checked, which `reserved_ordinals()` does.
        interface.reserved_ordinals()?;
        for member in interface.members()? {
            let member = member?;
            member.name()?;
            member.ordinal()?;
            member.kind()?;
            if let Some(timing) = member.timing()? {
                timing.mode()?;
                timing.min_us()?;
                timing.max_us()?;
            }
            for payload in member.payloads()? {
                let payload = payload?;
                payload.role()?;
                payload.type_name()?;
                for size in payload.max_sizes()? {
                    let size = size?;
                    size.encoding()?;
                    size.bytes()?;
                    size.state()?;
                    size.cause()?;
                }
            }
        }
    }
    for retired in catalog.retired()? {
        let retired = retired?;
        retired.name()?;
        retired.number()?;
    }
    Ok(())
}
