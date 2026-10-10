//! Identity, and the interaction descriptors generated code writes.
//!
//! A descriptor is a zero-sized marker type with constants. The descriptors
//! are the per-member form of the ordinal table of ADR-0013 decision 3,
//! extended with the interface number and the catalog hash.
//!
//! One computation reads the descriptors: a member's call deadline
//! ([`Member::call_deadline`]).
//!
//! The size table of an interface in one encoding is [`Sizes`], which
//! generated code implements once per interface per codec; [`reservation`]
//! and [`table_budget`] read it.

use crate::encoding::Encoding;
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

/// A catalog: the interfaces of one unit — one package manifest and the
/// source packages in its directory tree.
///
/// Two `CatalogRef`s are equal only when both the names and the hashes are
/// equal, because two units with the same contents can have the same hash.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CatalogRef {
    /// The unit name.
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
    /// The index of `MEMBER` in `Iface::MEMBERS`, which is also the
    /// interaction's row in every size table of the interface
    /// ([`Sizes::RESERVATIONS`]), so a row is reached without a scan.
    const ROW: usize;
}

/// The size table of an interface in encoding `E`: what a face and a runtime
/// need to hold one table of calls in flight, or one payload whose type is
/// not known before it is read, in that encoding.
///
/// Generated code implements it once per interface per codec the package
/// carries. Every number derives from
/// [`Payload::MAX_SIZE`](crate::payload::Payload::MAX_SIZE), which is the one
/// source of a payload's bound, so a table cannot drift from its codec. An
/// encoding the package does not carry has no table, so naming it is a
/// compile error at the call site (`Cabin: Sizes<Proto3>` is not satisfied),
/// not a run-time error.
///
/// Two buffers are interface-wide because the face fills them before it
/// knows the payload type: a server reads a claim into one buffer and then
/// matches the ordinal, and
/// [`EventSource::next`](crate::port::EventSource::next) writes one
/// occurrence of any subscribed event into a buffer and reports its ordinal
/// afterwards. Each has an associated type here, because a generic array
/// length is not expressible on stable Rust and generic code must be able to
/// name the buffer.
pub trait Sizes<E: Encoding>: Interface {
    /// Row `i` is the bytes one in-flight instance of `MEMBERS[i]` reserves:
    /// the sum of `MAX_SIZE` over the member's payloads — one payload for most
    /// kinds, the request and then the reply for a `query`, none for a
    /// `fixed`. The rows are parallel to [`Interface::MEMBERS`].
    const RESERVATIONS: &'static [u64];
    /// The largest `MAX_SIZE` over the argument and reply payloads of the
    /// interface's commands and queries; `0` when it has none.
    const MAX_BUFFER_SIZE: usize;
    /// The largest `MAX_SIZE` over the interface's event payloads; `0` when it
    /// has none.
    const EVENT_SOURCE_BUFFER_SIZE: usize;
    /// `[u8; Self::MAX_BUFFER_SIZE]`: the server's claim buffer, which
    /// receives a call before its member is known.
    type ClaimBuffer: AsRef<[u8]> + AsMut<[u8]> + Copy;
    /// `[u8; Self::EVENT_SOURCE_BUFFER_SIZE]`: the client's next-event
    /// buffer, which `EventSource::next` fills before the event is known.
    type EventBuffer: AsRef<[u8]> + AsMut<[u8]> + Copy;
    /// A zeroed `ClaimBuffer`.
    const CLAIM_BUFFER: Self::ClaimBuffer;
    /// A zeroed `EventBuffer`.
    const EVENT_BUFFER: Self::EventBuffer;
}

/// The bytes one in-flight instance of interaction `X` reserves in encoding
/// `E`: its row ([`Interaction::ROW`]) of the interface's size table
/// ([`Sizes::RESERVATIONS`]).
///
/// Infallible: a row exists for every member, because a codec is emitted for
/// every payload type of the package or the build fails. A `const fn`, so a
/// runtime may size a static table from it.
pub const fn reservation<X, E>() -> u64
where
    X: Interaction,
    X::Iface: Sizes<E>,
    E: Encoding,
{
    <X::Iface as Sizes<E>>::RESERVATIONS[X::ROW]
}

/// The in-flight byte budget of interface `I`'s table in encoding `E`: the
/// sum of its rows ([`Sizes::RESERVATIONS`]), saturating. A table serving
/// several interfaces adds the budget of each.
///
/// Infallible, for the reason [`reservation`] gives. A `const fn`, so a
/// runtime whose set of interfaces is known statically sizes a `static` table
/// from it at compile time, with no allocator.
pub const fn table_budget<I: Sizes<E>, E: Encoding>() -> u64 {
    let rows = I::RESERVATIONS;
    let mut total: u64 = 0;
    let mut i = 0;
    while i < rows.len() {
        total = total.saturating_add(rows[i]);
        i += 1;
    }
    total
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
    /// no timing: a `fixed`, or a `command` or `query` in a catalog built
    /// before commands and queries took a default response bound.
    pub timing: Option<Timing>,
    /// One entry per payload: two for a `query` (the request, then the
    /// reply), one for every other kind.
    pub payloads: &'static [PayloadInfo],
}

impl Member {
    /// The call deadline: the `max` of the member's `timing`, which on a
    /// `command` or a `query` is the response bound (ridl §9.3).
    ///
    /// A `command` or a `query` in a catalog built by this version always has
    /// a `max`: an untimed member takes the package's default response bound
    /// (ridl §9.1). `None` remains for a catalog built before that default
    /// existed, where such a member has no timing or a timing without `max`.
    /// This function takes no position on what a caller does with `None`.
    ///
    /// The function reads `max` whatever the member's `kind`. On a `signal`
    /// `max` is the staleness bound and on an `event` the time to live
    /// (ridl §9), neither of which is a call deadline, so call it on a
    /// `command` or a `query`.
    pub fn call_deadline(&self) -> Option<Duration> {
        self.timing.and_then(|timing| timing.max)
    }
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
///
/// A payload's size in an encoding is not here: it is the member's row of the
/// interface's size table ([`Sizes::RESERVATIONS`]), which derives from the
/// payload type's own `MAX_SIZE` for that encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PayloadInfo {
    /// The payload type's name.
    pub type_name: &'static str,
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
