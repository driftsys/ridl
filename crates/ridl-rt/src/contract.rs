//! Identity, and the interaction descriptors generated code writes.
//!
//! A descriptor is a zero-sized marker type with constants. The descriptors
//! are the per-member form of the ordinal table of ADR-0013 decision 3,
//! extended with the interface number and the catalog hash.

use crate::sample::Duration;

/// A member's ordinal: its position in the interface body, counted from 1
/// (ridl §11). An ordinal is never 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ordinal(pub u32);

/// An interface's number in its catalog: the number the lock file froze, or a
/// provisional number. An interface number is never 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InterfaceNo(pub u32);

/// The catalog hash: SHA-256 over the catalog's interfaces, their numbers, and
/// the types they reach.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CatalogHash(pub [u8; 32]);

/// A catalog: one package's interfaces.
///
/// Two `CatalogRef`s are equal only when both the names and the hashes are
/// equal, because two packages with the same contents can have the same hash.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CatalogRef {
    /// The package name.
    pub name: &'static str,
    /// The catalog hash.
    pub hash: CatalogHash,
}

/// The five interaction kinds of the language.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    /// A `signal`.
    Signal = 1,
    /// An `event`.
    Event = 2,
    /// A `command`.
    Command = 3,
    /// A `query`.
    Query = 4,
    /// A `fixed`.
    Fixed = 5,
}

/// An interface's descriptor.
pub trait Interface {
    /// The catalog the interface belongs to.
    const CATALOG: &'static CatalogRef;
    /// The interface number in that catalog.
    const NUMBER: InterfaceNo;
    /// `true` when the number is provisional, not yet frozen in the lock file.
    const PROVISIONAL: bool;
    /// The interface name.
    const NAME: &'static str;
    /// The members, in ordinal order. A reserved ordinal has no row, so the
    /// index of a row is not its ordinal minus one.
    const MEMBERS: &'static [Member];
}

/// An interaction's descriptor: the part every kind has.
pub trait Interaction {
    /// The interface that declares the interaction.
    type Iface: Interface;
    /// The interaction's row in `Iface::MEMBERS`.
    const MEMBER: &'static Member;
}

/// A `signal` (ridl §4).
pub trait Signal: Interaction {
    /// The value type.
    type Payload;
    /// The channel's init value (ridl §4.4).
    fn init() -> Self::Payload;
}

/// An `event` (ridl §5).
pub trait Event: Interaction {
    /// The occurrence type.
    type Payload;
}

/// A `fixed` (ridl §8).
pub trait Fixed: Interaction {
    /// The provisioned value type.
    type Payload;
}

/// A `command` (ridl §6).
pub trait Command: Interaction {
    /// The argument type.
    type Args;
    /// Evaluates the command's `require` clauses. `Ok` when every clause is
    /// true or the command declares none. `Err` when a clause is false, which
    /// the provider reports as
    /// [`Contract::PreconditionFailed`](crate::error::Contract::PreconditionFailed).
    #[allow(
        clippy::result_unit_err,
        reason = "the failing method decides the contract error, so the error carries no value"
    )]
    fn require(args: &Self::Args) -> Result<(), ()>;
}

/// A `query` (ridl §7).
pub trait Query: Interaction {
    /// The argument type.
    type Args;
    /// The reply type.
    type Reply;
    /// Evaluates the query's `require` clauses. `Ok` when every clause is
    /// true or the query declares none. `Err` when a clause is false, which
    /// the provider reports as
    /// [`Contract::PreconditionFailed`](crate::error::Contract::PreconditionFailed).
    #[allow(
        clippy::result_unit_err,
        reason = "the failing method decides the contract error, so the error carries no value"
    )]
    fn require(args: &Self::Args) -> Result<(), ()>;
    /// Evaluates the query's `ensure` clauses. `Ok` when every clause is true
    /// or the query declares none. `Err` when a clause is false, which the
    /// provider reports as
    /// [`Contract::ContractBroken`](crate::error::Contract::ContractBroken).
    #[allow(
        clippy::result_unit_err,
        reason = "the failing method decides the contract error, so the error carries no value"
    )]
    fn ensure(args: &Self::Args, reply: &Self::Reply) -> Result<(), ()>;
}

/// One member of an interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Member {
    /// The member's ordinal.
    pub ordinal: Ordinal,
    /// The member's kind.
    pub kind: Kind,
    /// The member's name.
    pub name: &'static str,
    /// The member's timing, as the IR resolved it. `None` when the IR carries
    /// no timing: a `command` or a `query` with no timing annotation, or a
    /// `fixed`.
    pub timing: Option<Timing>,
    /// One entry per payload: two for a `query` (the request, then the
    /// reply), one for every other kind.
    pub payloads: &'static [PayloadInfo],
}

/// The form of a timing annotation (ridl §9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimingMode {
    /// `@Xms`: a strict period, on a signal only (ridl §9.2).
    StrictPeriodic,
    /// `@[min..max]`, where either side may be absent.
    Range,
}

/// A member's timing (ridl §9).
///
/// `max` is the staleness bound of a signal, the time to live of an event, and
/// the response bound of a call.
///
/// Under `StrictPeriodic`, `min` and `max` both hold the period.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timing {
    /// The form of the annotation.
    pub mode: TimingMode,
    /// The lower bound. `None` when the IR leaves it unset.
    pub min: Option<Duration>,
    /// The upper bound. `None` when the IR leaves it unset.
    pub max: Option<Duration>,
}

/// One payload of a member.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PayloadInfo {
    /// The payload type's name.
    pub type_name: &'static str,
    /// The payload's largest encoded size in each core encoding.
    pub max_size: EncodedSizes,
}

/// A payload's largest encoded size in bytes, one field per core encoding.
///
/// A field is `None` when the toolchain cannot size the payload for that
/// encoding. That covers two cases the reader does not have to tell apart: the
/// encoding cannot carry the payload at all, and the encoding can carry it but
/// the size is not derivable yet, because the story that defines the layout has
/// not landed. The `repr(C)` column is `None` for every payload until E11.12
/// defines the C-representable layout, and a backend that emits descriptors
/// before its codec exists writes `None` for that codec's column too.
///
/// So a consumer reads `None` as "no size is available here", never as "this
/// payload cannot be encoded this way", and asks the catalog descriptor rather
/// than this field when it needs to know which encodings a payload has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EncodedSizes {
    /// The proto3 size.
    pub proto3: Option<u32>,
    /// The FlatBuffers size.
    pub flatbuffers: Option<u32>,
    /// The `repr(C)` size.
    pub repr_c: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::{CatalogHash, CatalogRef, Kind};

    #[test]
    fn catalog_refs_are_equal_only_when_name_and_hash_are_equal() {
        let base = CatalogRef {
            name: "vehicle",
            hash: CatalogHash([1; 32]),
        };
        let same = CatalogRef {
            name: "vehicle",
            hash: CatalogHash([1; 32]),
        };
        let other_hash = CatalogRef {
            name: "vehicle",
            hash: CatalogHash([2; 32]),
        };
        let other_name = CatalogRef {
            name: "cabin",
            hash: CatalogHash([1; 32]),
        };
        assert_eq!(base, same);
        assert_ne!(base, other_hash);
        assert_ne!(base, other_name);
    }

    #[test]
    fn kind_values_are_the_language_order_from_one() {
        assert_eq!(Kind::Signal as u8, 1);
        assert_eq!(Kind::Event as u8, 2);
        assert_eq!(Kind::Command as u8, 3);
        assert_eq!(Kind::Query as u8, 4);
        assert_eq!(Kind::Fixed as u8, 5);
    }
}
