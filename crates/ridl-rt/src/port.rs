//! The ports: the traits a runtime implements and generated code calls.
//!
//! Every port method returns without waiting. Waiting — blocking, `async`, or
//! a loop that drives the runtime — belongs to a face over a port, and this
//! crate defines no face. A port carries interface numbers, ordinals and
//! bytes, never a payload type: the generated binding decodes. No port method
//! takes the current time; a runtime reads its own [`Clock`].
//!
//! A port is attached to one catalog ([`Attached`]), and the interface numbers
//! and ordinals its methods take are scoped by that catalog.
//!
//! [`ScannableSignals`] and [`CoherentSignals`] are extensions. They describe
//! mechanisms some runtimes have, not interaction semantics every runtime must
//! present, so a runtime may omit them.
//!
//! Each trait below names the generated method it backs, so a reader who
//! arrived from generated code can find the port under it. The crate-level
//! documentation has the whole table, and
//! `docs/technotes/ridl-rt-by-example.md` in this repository walks it against
//! concrete generated code.

use crate::contract::{CatalogRef, InterfaceNo, Ordinal};
use crate::error::{CallError, Contract};
use crate::sample::{Duration, Envelope, Freshness, Provenance, Timestamp};

/// A port attached to one catalog.
pub trait Attached {
    /// The catalog this port serves.
    fn catalog(&self) -> &CatalogRef;
}

/// The clock envelopes are stamped from. A runtime has one.
pub trait Clock {
    /// The current time in the platform time base.
    fn now(&self) -> Timestamp;
}

/// Signals, consumer side.
///
/// A generated `Client` has one method per signal over this port. It reads
/// into a stack buffer sized from the payload's
/// [`MAX_SIZE`](crate::payload::Payload::MAX_SIZE), checks the bytes, and
/// returns a [`Sample`](crate::sample::Sample). Bytes that fail their check
/// are not an error there: the sample carries the channel's init value and a
/// provenance of [`Invalid`](crate::sample::Provenance::Invalid), so the
/// [`ReadError`] here reports the read itself.
pub trait SignalReader: Attached {
    /// Copies the signal's current value into the front of `out` and returns
    /// its provenance, its freshness and its envelope. The runtime resolves
    /// both the provenance and the freshness.
    ///
    /// Returns `ReadError::Short` when `out` is shorter than the value.
    fn read(
        &self,
        iface: InterfaceNo,
        ord: Ordinal,
        out: &mut [u8],
    ) -> Result<RawSample, ReadError>;
}

/// What [`SignalReader::read`] returns beside the copied bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawSample {
    /// Where the value comes from.
    pub provenance: Provenance,
    /// How old the value is.
    pub freshness: Freshness,
    /// The sender's timestamp and sequence number.
    pub envelope: Envelope,
    /// The number of bytes copied into `out`.
    pub len: usize,
}

/// Signals, provider side.
///
/// `set`, `invalidate` and `touch` stage a change. Each fails with
/// [`WriteError::Contract`] when the ordinal names no member, and with
/// [`WriteError::NotOwner`] when it names a member this provider does not
/// own. `commit` publishes every staged change, with one generation
/// increment and one timestamp per interface, taken from the runtime's
/// [`Clock`].
///
/// A generated `Publisher` has one method per signal over `set`, plus an
/// `invalidate_<name>` and a `commit`. The staging split is why a provider
/// that updates several signals of one interface and then commits produces one
/// coherent publication rather than several.
pub trait SignalWriter: Attached {
    /// Stages a new value.
    fn set(&mut self, iface: InterfaceNo, ord: Ordinal, bytes: &[u8]) -> Result<(), WriteError>;
    /// Stages the invalid state (ridl §4.5), with
    /// [`Cause::Declared`](crate::sample::Cause::Declared).
    fn invalidate(&mut self, iface: InterfaceNo, ord: Ordinal) -> Result<(), WriteError>;
    /// Stages a re-affirmation of the current value, without a new value.
    fn touch(&mut self, iface: InterfaceNo, ord: Ordinal) -> Result<(), WriteError>;
    /// Publishes everything staged. It cannot fail, so it reports nothing —
    /// including that the runtime behind the port is gone. A caller learns
    /// that from [`WriteError::Detached`] on a later `set`, `invalidate` or
    /// `touch`, never from that `commit`. What becomes of the changes staged
    /// before a commit whose runtime has already detached is not fixed here.
    fn commit(&mut self);
}

/// Events, consumer side.
///
/// A generated `Client` has a `subscribe_<name>` per event, but a single
/// `next_event` for the whole interface, because [`next`](EventSource::next)
/// returns the next occurrence of anything subscribed and the payload type is
/// not known until its ordinal has been read. The generated method routes on
/// that ordinal into an enum with one variant per event.
pub trait EventSource: Attached {
    /// Starts delivery of the listed events.
    fn subscribe(&mut self, iface: InterfaceNo, ords: &[Ordinal]) -> Result<(), SubscribeError>;
    /// Stops delivery of the listed events.
    fn unsubscribe(&mut self, iface: InterfaceNo, ords: &[Ordinal]);
    /// Copies the next occurrence into the front of `out`. `Ok(None)` when no
    /// occurrence is waiting.
    ///
    /// Occurrences come in order, and a gap in `seq` is a loss (ridl §3.1). An
    /// occurrence older than its time to live is discarded here (ridl §5.2).
    /// `ReadError::Short` does not consume the occurrence: the next call
    /// returns the same one.
    fn next(&mut self, out: &mut [u8]) -> Result<Option<RawOccurrence>, ReadError>;
}

/// What [`EventSource::next`] returns beside the copied bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawOccurrence {
    /// The interface of the event.
    pub iface: InterfaceNo,
    /// The ordinal of the event.
    pub ord: Ordinal,
    /// The sender's timestamp and sequence number.
    pub envelope: Envelope,
    /// The number of bytes copied into `out`.
    pub len: usize,
}

/// Events, provider side.
///
/// A generated `Publisher` has one method per event over this port. Unlike a
/// signal, an occurrence is not staged and there is no `commit`: there is no
/// coherent set to assemble.
pub trait EventSink: Attached {
    /// Raises one occurrence.
    fn raise(&mut self, iface: InterfaceNo, ord: Ordinal, bytes: &[u8]) -> Result<(), RaiseError>;
}

/// Calls, consumer side.
///
/// A command and a query are separate methods, because their outcomes differ
/// (ridl §6, §7).
///
/// A generated `Client` has one method per command and query over this port,
/// each returning a [`Correlation`] rather than an outcome, because nothing
/// here waits. The outcome is retrieved separately: [`ack`](Caller::ack) for a
/// command, a generated `<name>_reply` over [`reply`](Caller::reply) for a
/// query. A `require` clause is evaluated before sending, so a failing
/// precondition costs no round trip and is reported as
/// [`SendError::Contract`].
pub trait Caller: Attached {
    /// Sends a command and returns the correlation of its outcome.
    fn command(
        &mut self,
        iface: InterfaceNo,
        ord: Ordinal,
        args: &[u8],
    ) -> Result<Correlation, SendError>;
    /// Sends a query and returns the correlation of its reply.
    fn query(
        &mut self,
        iface: InterfaceNo,
        ord: Ordinal,
        args: &[u8],
    ) -> Result<Correlation, SendError>;
    /// A command's delivery acknowledgment (ridl §6.1), once it is known:
    /// `Ok(())` when accepted, `Err(CallError::Contract(_))` when rejected,
    /// `Err(CallError::Transport(Transport::Corrupt))` when the provider could
    /// not read the command's argument bytes, and
    /// `Err(CallError::Transport(Transport::Undelivered))` when no
    /// acknowledgment came within the bound. `None` while unknown, and always
    /// `None` for a query's correlation.
    ///
    /// `None` has two causes this method does not separate: the acknowledgment
    /// is not known yet, and `c` is a query's correlation, for which `None` is
    /// the standing answer. A caller that polls `ack` for a query's
    /// correlation therefore never finishes. The return carries no error, so
    /// keep the correlations [`command`](Caller::command) returned and ask
    /// only about those. After [`forget`](Caller::forget) a correlation's
    /// outcome is no longer retrievable, so do not ask about it.
    fn ack(&mut self, c: Correlation) -> Option<Result<(), CallError>>;
    /// A query's reply, once it is known: the reply bytes copied into the front
    /// of `out` and their length, or the error. `Ok(None)` while unknown.
    /// `ReadError::Short` does not consume the reply.
    ///
    /// The outer [`ReadError`] reports the port call itself: `Short` when
    /// `out` is too short, and `Detached` when the local runtime is gone.
    /// `reply` never returns `ReadError::Contract`. The inner [`CallError`] is
    /// the outcome from the peer or the transport, such as a contract error
    /// the provider settled, or `Transport::Down` when the connection to the
    /// peer is lost.
    fn reply(
        &mut self,
        c: Correlation,
        out: &mut [u8],
    ) -> Result<Option<Result<usize, CallError>>, ReadError>;
    /// Releases a correlation whose outcome the caller no longer needs.
    fn forget(&mut self, c: Correlation);
}

/// Identifies one sent call to its caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Correlation(pub u64);

/// Calls, provider side.
///
/// `next_claim` presents each delivered call once. A retransmission of a call
/// already presented is not presented again and receives the cached
/// acknowledgment. Calls from two callers are never merged, even when they
/// carry the same `seq`. A call lost in transport is never presented. The
/// caller of a lost command sees `Transport::Undelivered` from `Caller::ack`;
/// the caller of a lost query sees `Transport::Timeout` from `Caller::reply`
/// once the response bound passes.
///
/// Every claim is settled. For a command, the generated dispatch settles
/// `Ok(&[])` after the arguments and `require` pass, before application code
/// runs. For a query, it settles with the reply bytes or the outcome the
/// caller sees.
///
/// An application implements neither this trait nor a claim loop. It
/// implements the generated `Provider` trait and calls the generated
/// `dispatch`, which makes one pass over the claims already waiting, routes
/// each by ordinal, decodes, evaluates `require`, calls the provider,
/// evaluates a query's `ensure`, settles, and returns how many settlements
/// this port accepted.
pub trait Handler: Attached {
    /// Starts presenting calls to the listed members.
    fn serve(&mut self, iface: InterfaceNo, ords: &[Ordinal]) -> Result<(), ServeError>;
    /// Copies the next call's arguments into the front of `out`. `Ok(None)`
    /// when no call is waiting. `ReadError::Short` does not consume the call.
    fn next_claim(&mut self, out: &mut [u8]) -> Result<Option<Claim>, ReadError>;
    /// Settles a claim with the reply bytes (empty for a command) or the
    /// outcome the caller sees. A provider settles
    /// [`CallError::Contract`] when the arguments break their typl
    /// constraints, a `require` clause fails, or an `ensure` clause fails,
    /// and `CallError::Transport(Transport::Corrupt)` when the argument
    /// bytes fail the structure check.
    fn settle(
        &mut self,
        claim: ClaimId,
        outcome: Result<&[u8], CallError>,
    ) -> Result<(), SettleError>;
}

/// One call presented to a provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Claim {
    /// Unique in its channel.
    pub id: ClaimId,
    /// The interface of the call.
    pub iface: InterfaceNo,
    /// The ordinal of the call. The descriptor at this ordinal gives the kind.
    pub ord: Ordinal,
    /// The caller's timestamp and sequence number. `seq` is unique for each
    /// caller, not for each channel.
    pub envelope: Envelope,
    /// The time left before the response bound passes. `None` when the call
    /// has no response bound (ridl §9.3).
    pub remaining: Option<Duration>,
    /// The number of argument bytes copied into `out`.
    pub len: usize,
}

/// Identifies one claim to its provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClaimId(pub u64);

/// `fixed`, consumer side (ridl §8).
///
/// The generated face carries no method over this port in this version: a
/// `fixed` is provisioned rather than interacted with, so the emitter writes
/// its descriptor and nothing else, and an application that needs a
/// provisioned value calls [`read_fixed`](FixedReader::read_fixed) itself.
/// There is no provider side either — a provisioned constant is supplied to
/// the runtime, not published by application code.
pub trait FixedReader: Attached {
    /// Copies the provisioned value into the front of `out` and returns its
    /// length.
    fn read_fixed(
        &self,
        iface: InterfaceNo,
        ord: Ordinal,
        out: &mut [u8],
    ) -> Result<usize, ReadError>;
}

/// Extension: signals in a store a consumer can walk. A runtime may omit it.
pub trait ScannableSignals: SignalReader {
    /// The interface's generation: a counter that each commit to the interface
    /// increments.
    ///
    /// The return carries no error, so an interface the port's catalog does
    /// not hold has no reserved answer: it gives a `u64` the caller cannot
    /// tell from a real generation. Ask only about an interface of
    /// [`catalog`](Attached::catalog).
    fn generation(&self, iface: InterfaceNo) -> u64;
    /// Writes the changes into `out`, interface by interface, in the order of
    /// `marks`, and updates `marks`. An interface's changes are written all
    /// together or not at all: when they do not fit in the rest of `out`,
    /// none of them is written, that interface's mark is not updated, and
    /// `scan` returns the number of entries written so far.
    ///
    /// The count alone does not say whether changes are still waiting, because
    /// a return of 0 has two causes. To tell them apart, compare each mark
    /// with its interface's [`generation`](ScannableSignals::generation) after
    /// the call:
    ///
    /// - every mark's `generation` equals `generation(iface)` — nothing had
    ///   changed, and the scan is complete;
    /// - some mark's `generation` is behind `generation(iface)` — that
    ///   interface's changes did not fit in `out`. When `scan` also returned
    ///   0, `out` is shorter than the changes of the first such interface in
    ///   the order of `marks`, and no call can make progress until `out` is
    ///   longer.
    ///
    /// A caller that scans in a loop therefore grows `out` when `scan` returns
    /// 0 and a mark is still behind its interface's generation.
    fn scan(&self, marks: &mut [Watermark], out: &mut [Changed]) -> usize;
}

/// How far a consumer has scanned one interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Watermark {
    /// The interface.
    pub iface: InterfaceNo,
    /// The generation last scanned. Named `generation`, not `gen`, because
    /// `gen` is a reserved keyword in the 2024 edition.
    pub generation: u64,
    /// The sequence number last scanned.
    pub seq: u64,
}

/// A signal that changed since a [`Watermark`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Changed {
    /// The interface of the signal.
    pub iface: InterfaceNo,
    /// The ordinal of the signal.
    pub ord: Ordinal,
    /// The signal's sequence number.
    pub seq: u64,
}

/// Extension: reads of several signals of one interface from one publication.
/// A runtime whose binding delivers each field separately cannot present this
/// and omits it.
pub trait CoherentSignals: SignalReader {
    /// Answers every ordinal in `ords` from one publication. Copies the values
    /// into `out` one after another, writes one `RawSample` for each ordinal
    /// into `samples` in the order of `ords`, and returns the number of bytes
    /// written to `out`.
    ///
    /// Returns `ReadError::Short` with the size the whole set needs when `out`
    /// is too short, `ReadError::TooFewSamples` with the number of entries
    /// `samples` needs when `samples` is shorter than `ords`, and
    /// `ReadError::Contract(Contract::UnknownInteraction)` when an ordinal
    /// names no member.
    fn read_coherent(
        &self,
        iface: InterfaceNo,
        ords: &[Ordinal],
        out: &mut [u8],
        samples: &mut [RawSample],
    ) -> Result<usize, ReadError>;
}

/// A read that failed.
///
/// One enum serves every read on every port, so a variant can be unreachable
/// for the method that returns it: [`TooFewSamples`](ReadError::TooFewSamples)
/// belongs to [`CoherentSignals::read_coherent`] alone, and
/// [`Caller::reply`] never returns [`Contract`](ReadError::Contract). Each
/// method documents what it can return.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadError {
    /// The output buffer is too short. Nothing was consumed.
    Short {
        /// The bytes the read needs.
        needed: usize,
    },
    /// `samples` has fewer entries than `ords`. Nothing was consumed.
    TooFewSamples {
        /// The entries `samples` needs.
        needed: usize,
    },
    /// A contract error, such as an unknown interaction.
    Contract(Contract),
    /// The runtime behind the port is gone.
    Detached,
}

/// A [`SignalWriter::set`], [`SignalWriter::invalidate`] or
/// [`SignalWriter::touch`] that failed.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteError {
    /// The value is larger than the signal's capacity.
    TooLarge {
        /// The capacity in bytes.
        cap: usize,
    },
    /// This provider does not own the signal.
    NotOwner,
    /// A contract error, such as an unknown interaction.
    Contract(Contract),
    /// The runtime behind the port is gone.
    Detached,
}

/// An [`EventSink::raise`] that failed.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RaiseError {
    /// The runtime cannot accept an occurrence now. Retryable.
    Busy,
    /// The occurrence is larger than the event's capacity.
    TooLarge {
        /// The capacity in bytes.
        cap: usize,
    },
    /// This provider does not own the event.
    NotOwner,
    /// A contract error, such as an unknown interaction.
    Contract(Contract),
    /// The runtime behind the port is gone.
    Detached,
}

/// A [`Caller::command`] or [`Caller::query`] that failed before sending.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendError {
    /// The runtime cannot accept a call now. Retryable.
    Busy,
    /// The arguments are larger than the call's capacity.
    TooLarge {
        /// The capacity in bytes.
        cap: usize,
    },
    /// A contract error, such as an unknown interaction.
    Contract(Contract),
    /// The runtime behind the port is gone.
    Detached,
}

/// An [`EventSource::subscribe`] that failed.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubscribeError {
    /// A contract error: an unknown interaction fails when subscribing (ridl
    /// §10.2).
    Contract(Contract),
    /// The runtime behind the port is gone.
    Detached,
}

/// A [`Handler::serve`] that failed.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServeError {
    /// A contract error, such as an unknown interaction.
    Contract(Contract),
    /// This provider does not own the call.
    NotOwner,
    /// The runtime behind the port is gone.
    Detached,
}

/// A [`Handler::settle`] that failed.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettleError {
    /// The claim was already settled, or was never issued.
    UnknownClaim,
    /// The reply is larger than the call's capacity.
    TooLarge {
        /// The capacity in bytes.
        cap: usize,
    },
    /// The runtime behind the port is gone.
    Detached,
}

// Forwarding impls (ADR-0021 decision 11).
//
// Every port trait above is implemented for `&mut P`, and the six whose
// methods all take `&self` — `Attached`, `Clock`, `SignalReader`,
// `FixedReader`, `ScannableSignals` and `CoherentSignals` — also for `&P`.
// What they buy is one thing: a value generic over a port trait, such as a
// generated face, can be built over a reference to a port rather than over the
// port itself. A wrapper that adds tracing and a test double are accepted by
// such a bound with or without them, because each implements the port traits
// itself.
//
// `impl<P: T + ?Sized> T for Box<P>` is deferred: it needs `alloc`, which no
// feature combination of this crate brings in.

impl<P: Attached + ?Sized> Attached for &P {
    fn catalog(&self) -> &CatalogRef {
        (**self).catalog()
    }
}

impl<P: Attached + ?Sized> Attached for &mut P {
    fn catalog(&self) -> &CatalogRef {
        (**self).catalog()
    }
}

impl<P: Clock + ?Sized> Clock for &P {
    fn now(&self) -> Timestamp {
        (**self).now()
    }
}

impl<P: Clock + ?Sized> Clock for &mut P {
    fn now(&self) -> Timestamp {
        (**self).now()
    }
}

impl<P: SignalReader + ?Sized> SignalReader for &P {
    fn read(
        &self,
        iface: InterfaceNo,
        ord: Ordinal,
        out: &mut [u8],
    ) -> Result<RawSample, ReadError> {
        (**self).read(iface, ord, out)
    }
}

impl<P: SignalReader + ?Sized> SignalReader for &mut P {
    fn read(
        &self,
        iface: InterfaceNo,
        ord: Ordinal,
        out: &mut [u8],
    ) -> Result<RawSample, ReadError> {
        (**self).read(iface, ord, out)
    }
}

impl<P: SignalWriter + ?Sized> SignalWriter for &mut P {
    fn set(&mut self, iface: InterfaceNo, ord: Ordinal, bytes: &[u8]) -> Result<(), WriteError> {
        (**self).set(iface, ord, bytes)
    }
    fn invalidate(&mut self, iface: InterfaceNo, ord: Ordinal) -> Result<(), WriteError> {
        (**self).invalidate(iface, ord)
    }
    fn touch(&mut self, iface: InterfaceNo, ord: Ordinal) -> Result<(), WriteError> {
        (**self).touch(iface, ord)
    }
    fn commit(&mut self) {
        (**self).commit();
    }
}

impl<P: EventSource + ?Sized> EventSource for &mut P {
    fn subscribe(&mut self, iface: InterfaceNo, ords: &[Ordinal]) -> Result<(), SubscribeError> {
        (**self).subscribe(iface, ords)
    }
    fn unsubscribe(&mut self, iface: InterfaceNo, ords: &[Ordinal]) {
        (**self).unsubscribe(iface, ords);
    }
    fn next(&mut self, out: &mut [u8]) -> Result<Option<RawOccurrence>, ReadError> {
        (**self).next(out)
    }
}

impl<P: EventSink + ?Sized> EventSink for &mut P {
    fn raise(&mut self, iface: InterfaceNo, ord: Ordinal, bytes: &[u8]) -> Result<(), RaiseError> {
        (**self).raise(iface, ord, bytes)
    }
}

impl<P: Caller + ?Sized> Caller for &mut P {
    fn command(
        &mut self,
        iface: InterfaceNo,
        ord: Ordinal,
        args: &[u8],
    ) -> Result<Correlation, SendError> {
        (**self).command(iface, ord, args)
    }
    fn query(
        &mut self,
        iface: InterfaceNo,
        ord: Ordinal,
        args: &[u8],
    ) -> Result<Correlation, SendError> {
        (**self).query(iface, ord, args)
    }
    fn ack(&mut self, c: Correlation) -> Option<Result<(), CallError>> {
        (**self).ack(c)
    }
    fn reply(
        &mut self,
        c: Correlation,
        out: &mut [u8],
    ) -> Result<Option<Result<usize, CallError>>, ReadError> {
        (**self).reply(c, out)
    }
    fn forget(&mut self, c: Correlation) {
        (**self).forget(c);
    }
}

impl<P: Handler + ?Sized> Handler for &mut P {
    fn serve(&mut self, iface: InterfaceNo, ords: &[Ordinal]) -> Result<(), ServeError> {
        (**self).serve(iface, ords)
    }
    fn next_claim(&mut self, out: &mut [u8]) -> Result<Option<Claim>, ReadError> {
        (**self).next_claim(out)
    }
    fn settle(
        &mut self,
        claim: ClaimId,
        outcome: Result<&[u8], CallError>,
    ) -> Result<(), SettleError> {
        (**self).settle(claim, outcome)
    }
}

impl<P: FixedReader + ?Sized> FixedReader for &P {
    fn read_fixed(
        &self,
        iface: InterfaceNo,
        ord: Ordinal,
        out: &mut [u8],
    ) -> Result<usize, ReadError> {
        (**self).read_fixed(iface, ord, out)
    }
}

impl<P: FixedReader + ?Sized> FixedReader for &mut P {
    fn read_fixed(
        &self,
        iface: InterfaceNo,
        ord: Ordinal,
        out: &mut [u8],
    ) -> Result<usize, ReadError> {
        (**self).read_fixed(iface, ord, out)
    }
}

impl<P: ScannableSignals + ?Sized> ScannableSignals for &P {
    fn generation(&self, iface: InterfaceNo) -> u64 {
        (**self).generation(iface)
    }
    fn scan(&self, marks: &mut [Watermark], out: &mut [Changed]) -> usize {
        (**self).scan(marks, out)
    }
}

impl<P: ScannableSignals + ?Sized> ScannableSignals for &mut P {
    fn generation(&self, iface: InterfaceNo) -> u64 {
        (**self).generation(iface)
    }
    fn scan(&self, marks: &mut [Watermark], out: &mut [Changed]) -> usize {
        (**self).scan(marks, out)
    }
}

impl<P: CoherentSignals + ?Sized> CoherentSignals for &P {
    fn read_coherent(
        &self,
        iface: InterfaceNo,
        ords: &[Ordinal],
        out: &mut [u8],
        samples: &mut [RawSample],
    ) -> Result<usize, ReadError> {
        (**self).read_coherent(iface, ords, out, samples)
    }
}

impl<P: CoherentSignals + ?Sized> CoherentSignals for &mut P {
    fn read_coherent(
        &self,
        iface: InterfaceNo,
        ords: &[Ordinal],
        out: &mut [u8],
        samples: &mut [RawSample],
    ) -> Result<usize, ReadError> {
        (**self).read_coherent(iface, ords, out, samples)
    }
}
