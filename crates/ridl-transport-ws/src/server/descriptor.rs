//! The member table read from a catalog descriptor, under the `descriptor`
//! feature.

use std::fmt;
use std::time::Duration;

use ridl_descriptor::{verify, Kind as DescriptorKind, VerifyError};
use ridl_rt::contract::{CatalogHash, InterfaceNo, Kind, Ordinal};

use super::members::MemberTable;

/// Why [`MemberTable::from_descriptor`] refused a buffer.
#[derive(Debug)]
#[non_exhaustive]
pub enum DescriptorError {
    /// `ridl_descriptor::verify` refused the buffer.
    Verify(VerifyError),
    /// The catalog hash, or a compatible hash, is not 32 bytes.
    HashLength {
        /// The length of the hash, in bytes.
        len: usize,
    },
    /// An interface's `number` is 0.
    ZeroNumber {
        /// The name of the interface.
        interface: String,
    },
    /// A member's `ordinal` is 0.
    ZeroOrdinal {
        /// The name of the interface.
        interface: String,
        /// The name of the member.
        member: String,
    },
    /// A `max_us` that is not a decimal `u64`.
    MaxUs {
        /// The member's interface number.
        iface: InterfaceNo,
        /// The member's ordinal.
        ord: Ordinal,
        /// The text of `max_us`.
        text: String,
    },
}

impl fmt::Display for DescriptorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Verify(e) => write!(f, "{e}"),
            Self::HashLength { len } => {
                write!(
                    f,
                    "a catalog hash of {len} bytes; a catalog hash is 32 bytes"
                )
            }
            Self::ZeroNumber { interface } => {
                write!(f, "interface `{interface}` has the number 0")
            }
            Self::ZeroOrdinal { interface, member } => {
                write!(f, "member `{interface}.{member}` has the ordinal 0")
            }
            Self::MaxUs { iface, ord, text } => write!(
                f,
                "member {}.{} has the max_us {text:?}, which is not a decimal u64",
                iface.0, ord.0
            ),
        }
    }
}

impl std::error::Error for DescriptorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Verify(e) => Some(e),
            _ => None,
        }
    }
}

impl From<VerifyError> for DescriptorError {
    fn from(e: VerifyError) -> Self {
        Self::Verify(e)
    }
}

/// The hash in `bytes`, which must be 32 bytes long.
fn hash(bytes: &[u8]) -> Result<CatalogHash, DescriptorError> {
    <[u8; 32]>::try_from(bytes)
        .map(CatalogHash)
        .map_err(|_| DescriptorError::HashLength { len: bytes.len() })
}

/// The kind of the same name. The two enums number their variants
/// differently, so the mapping is by name, not by discriminant.
fn kind(kind: DescriptorKind) -> Kind {
    match kind {
        DescriptorKind::Signal => Kind::Signal,
        DescriptorKind::Event => Kind::Event,
        DescriptorKind::Command => Kind::Command,
        DescriptorKind::Query => Kind::Query,
        DescriptorKind::Fixed => Kind::Fixed,
    }
}

impl MemberTable {
    /// Builds the table from the bytes of a `<unit>.catalog.binfb` file,
    /// which `ridl build --emit catalog` writes. The crate does no I/O: the
    /// embedder reads the file, or embeds it with `include_bytes!`.
    ///
    /// The table holds one entry for each member of each interface, with
    /// the member's kind and its `max` bound (the `max_us` of its timing,
    /// `None` when the member has no timing or no `max_us`). A reserved
    /// ordinal and a retired interface give no entry. Each hash of the
    /// catalog's `compatible` list is added with
    /// [`add_compatible`](Self::add_compatible), so a repeated hash and the
    /// own hash are not listed.
    ///
    /// # Errors
    ///
    /// [`DescriptorError`]: the buffer fails
    /// `ridl_descriptor::verify`, a hash is not 32 bytes, an interface number
    /// or an ordinal is 0, or a `max_us` is not a decimal `u64`.
    pub fn from_descriptor(bytes: &[u8]) -> Result<MemberTable, DescriptorError> {
        // `verify` has read every field once, so a read below cannot fail on
        // the structure; an error is still returned, not unwrapped.
        let invalid = |e| DescriptorError::Verify(VerifyError::Invalid(e));
        let catalog = verify(bytes)?;
        let mut table = MemberTable::new(
            catalog.name().map_err(invalid)?,
            hash(catalog.hash().map_err(invalid)?)?,
        );
        for interface in catalog.interfaces().map_err(invalid)? {
            let interface = interface.map_err(invalid)?;
            let number = interface.number().map_err(invalid)?;
            if number == 0 {
                return Err(DescriptorError::ZeroNumber {
                    interface: interface.name().map_err(invalid)?.to_owned(),
                });
            }
            let iface = InterfaceNo(number);
            for member in interface.members().map_err(invalid)? {
                let member = member.map_err(invalid)?;
                let ordinal = member.ordinal().map_err(invalid)?;
                if ordinal == 0 {
                    return Err(DescriptorError::ZeroOrdinal {
                        interface: interface.name().map_err(invalid)?.to_owned(),
                        member: member.name().map_err(invalid)?.to_owned(),
                    });
                }
                let ord = Ordinal(ordinal);
                let max_us = match member.timing().map_err(invalid)? {
                    Some(timing) => timing.max_us().map_err(invalid)?,
                    None => None,
                };
                let max = match max_us {
                    Some(text) => Some(Duration::from_micros(text.parse::<u64>().map_err(
                        |_| DescriptorError::MaxUs {
                            iface,
                            ord,
                            text: text.to_owned(),
                        },
                    )?)),
                    None => None,
                };
                table.insert(iface, ord, kind(member.kind().map_err(invalid)?), max);
            }
        }
        // Absent in a file written before the field existed.
        if let Some(compatible) = catalog.compatible().map_err(invalid)? {
            for earlier in compatible {
                table.add_compatible(hash(earlier.map_err(invalid)?.hash().map_err(invalid)?)?);
            }
        }
        Ok(table)
    }
}
