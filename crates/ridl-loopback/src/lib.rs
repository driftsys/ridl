//! `ridl-loopback` — the in-process reference runtime.
//!
//! This crate implements the fourteen port traits of
//! [`ridl_rt::port`](https://docs.rs/ridl-rt) over one in-memory store. It is
//! the first runtime in this workspace (ADR-0020 decision 6, which names the
//! crate and fixes `ridl-rt` as its only dependency), and it is what the
//! generated interaction face runs against in a test, an example or a
//! single-process application.
//!
//! What it is not:
//!
//! - **Not a transport.** It carries no frame, opens no socket and has no wire
//!   format. A value published here is read here, in the same process.
//! - **Not the engine.** The store below is a map and a queue, not the seqlock
//!   store, the sans-IO session, the platform traits and the scheduler, which
//!   are outside this repository.
//! - **Not a checker.** Payload bytes are opaque to it: it carries them and
//!   never verifies one. `Payload::verify` is the generated face's, on both
//!   sides.
//!
//! # The handles
//!
//! A runtime presents one handle type per port role rather than one type
//! implementing them all, and may also offer an aggregate handle covering the
//! port set one interface's face needs (ADR-0021 decision 12). This crate
//! offers both. Its six handles group eleven roles the way that decision
//! derives the threading split: a handle each for the five roles with a
//! `&mut self` method, and one handle for the six whose methods all take
//! `&self`, which are exactly the roles several threads may hold at once. The
//! twelfth port trait, `Wakeable`, is implemented on every handle, because
//! each handle wakes its own waiters (see "Waking" below). The two stream
//! extensions, [`StreamCaller`] and [`StreamHandler`], are on the caller and
//! the handler handle.
//! [`Loopback`] is the aggregate: it implements all fourteen port traits by
//! delegating to the six role handles it holds, and it is what a generated
//! `Client`, `Publisher` or `dispatch` is normally built over, and it states
//! the encoding the face encodes with. Every handle carries that encoding as
//! its type parameter `E` and implements [`Encoded`] with it; the default is
//! FlatBuffers, and [`Loopback::with_encoding`] names another.
//!
//! ```
//! use ridl_loopback::Loopback;
//! use ridl_rt::contract::{CatalogHash, CatalogRef, InterfaceNo, Ordinal};
//! use ridl_rt::port::{SignalReader, SignalWriter};
//!
//! let catalog = CatalogRef { name: "face.demo", hash: CatalogHash([0u8; 32]) };
//! let mut rt = Loopback::new(catalog);
//!
//! rt.set(InterfaceNo(1), Ordinal(1), &[42]).expect("staged");
//! rt.commit();
//!
//! let mut out = [0u8; 8];
//! let sample = rt.read(InterfaceNo(1), Ordinal(1), &mut out).expect("read");
//! assert_eq!(&out[..sample.len], &[42]);
//! ```
//!
//! [`Loopback::split`] hands out the six role handles for a program that wants
//! them apart — one thread reading while another publishes, or two callers on
//! one provider. [`ReaderHandle`] is `Send + Sync`; the other five are `Send`
//! and driven by one thread each. Nothing here declares either: both follow
//! from the fields, and the assertions at the bottom of this file pin them.
//! [`Loopback::attach`] makes a second aggregate on the same store, for a
//! program that holds several faces over one runtime, each owning its own.
//!
//! # What it reports, and what it cannot
//!
//! The loopback holds no catalog descriptor — driftsys/ridl#381
//! writes the descriptor file, and giving the loopback one is not yet
//! assigned — so it has no member table, and there is no ordinal
//! it can call unknown, no member it can call unowned, and no timing
//! annotation it can measure a value's freshness or a call's remaining time
//! against. What it therefore never returns:
//! `WriteError::NotOwner`, `RaiseError::NotOwner`, `ServeError::NotOwner`, any
//! port error's `Contract` variant except
//! [`FixedReader::read_fixed`]'s and the stream ports'
//! `Contract::UnknownInteraction` for a call or a claim it does not hold, and
//! `Freshness::Fresh` or `Freshness::Stale`. Nothing detaches, because every
//! handle holds the store alive, so `Detached` never appears either. Two
//! things are bounded. The call table's [`Loopback::SLOTS`]: a send with
//! every slot taken answers `SendError::Busy`. And the size of a payload: a
//! sending port refuses a payload longer than its member's limit with
//! `TooLarge { cap }` — the limit [`Loopback::set_limit`] gave the member,
//! else the default ceiling of the port's kind (ADR-0030 decision 16) — and
//! [`Loopback::provision_fixed`] refuses a `fixed` the same way. `TooLarge`
//! comes from that limit, and from [`Loopback::fail_next_settle`], which
//! reports it with `cap: 0`. `Transport::Busy` reaches a caller only when a
//! provider settles a call with it.
//!
//! # Waking
//!
//! Every handle implements [`Wakeable`], and a handle stores a waker only
//! under a kind of key one of its roles observes: a caller handle under
//! `Interest::Outcome` and `Interest::Stream`, kept with each call, and one
//! waker under `Interest::Slot`, a source handle one waker under
//! `Interest::Event`, and a handler handle one waker under `Interest::Claim`
//! and one per stream of a claim under `Interest::ClaimStream`, kept with the
//! claim's call.
//! For `Event` and `Claim` the rule is one waker per kind, and a change to
//! any key of the kind wakes the stored waker, whatever interface it was
//! registered under (ADR-0021 decision 13); an `Outcome` waker is per call,
//! and no change but that call's settlement, its `forget`, or the drop of the
//! caller handle that sent it wakes it (a displacement by another task does,
//! as for every kind). A settlement wakes the
//! call's waiter, a raise wakes each source it queues the occurrence for, and
//! a send wakes each handler that serves the member, and a reclaimed slot of
//! the call table wakes every caller's `Slot` waiter. A chunk, an end, an
//! abort or a pull that frees a stream's window wakes the other side's waiter
//! of that stream, and a settlement or a `forget` wakes the call's `Stream`
//! waiters, as a `forget` wakes its `ClaimStream` waiters. A registration
//! whose key already holds — the outcome is known, an occurrence or a call is
//! waiting, a slot is free — is woken at once, and so is one under a kind the
//! handle does not observe; a stream registration is stored, and is woken at
//! once only for a call or a claim whose streams can no longer change. A registration of the waker already stored refreshes it
//! without waking it. No waker is woken while the store is locked.
//!
//! [`Attached::catalog`] returns the
//! `CatalogRef` the runtime was built with, unexamined. ADR-0021 decision 3
//! places a check of it against the interface's own `CATALOG` in a generated
//! face's constructor, once, when the face is built; the constructor the Rust
//! backend emits makes that check and panics on a mismatch (ADR-0023 decision
//! 8). It is the face's check and not the runtime's: the loopback carries the
//! value and compares nothing.
//!
//! The crate's as-built design record, with the reasoning behind each of these
//! choices, is `docs/design/ridl-loopback.md` in this repository.

use std::sync::{Arc, Mutex};
use std::task::Waker;

use ridl_rt::contract::{CatalogRef, InterfaceNo, Ordinal, StreamNo};
use ridl_rt::encoding::{Encoding, FlatBuffers};
use ridl_rt::error::CallError;
use ridl_rt::port::{
    Attached, Caller, Changed, Claim, ClaimId, Clock, CoherentSignals, Correlation, Encoded,
    EventSink, EventSource, FixedReader, Handler, Interest, Pull, PushError, RaiseError,
    RawOccurrence, RawSample, ReadError, ScannableSignals, SendError, ServeError, SettleError,
    SignalReader, SignalWriter, StreamCaller, StreamHandler, SubscribeError, Wakeable, Watermark,
    WriteError,
};
use ridl_rt::sample::{Duration, Timestamp};
use ridl_rt::trace::TraceContext;

mod handle;
mod store;

pub use handle::{
    CallerHandle, HandlerHandle, ReaderHandle, SinkHandle, SourceHandle, WriterHandle,
};

use handle::{lock, Shared};
use store::Store;

/// The six role handles of one runtime, as [`Loopback::split`] hands them out.
/// Each carries the runtime's encoding `E`.
pub struct Handles<E: Encoding = FlatBuffers> {
    /// `Attached`, `Clock`, `SignalReader`, `FixedReader`, `ScannableSignals`
    /// and `CoherentSignals`.
    pub reader: ReaderHandle<E>,
    /// `SignalWriter`.
    pub writer: WriterHandle<E>,
    /// `EventSource`.
    pub source: SourceHandle<E>,
    /// `EventSink`.
    pub sink: SinkHandle<E>,
    /// `Caller`.
    pub caller: CallerHandle<E>,
    /// `Handler`.
    pub handler: HandlerHandle<E>,
}

/// The aggregate handle: one value implementing all fourteen port traits by
/// delegating to the six role handles it holds.
///
/// A face is built over one value implementing at least the port traits its
/// interface needs, and a generated `Client` is commonly bound over
/// `SignalReader + EventSource + Caller` at once, which no single role handle
/// satisfies. That is what an aggregate is for (ADR-0021 decision 12). Pass it
/// by value, or as `&mut` under the forwarding impls of ADR-0021 decision 11.
///
/// The type parameter `E` is the encoding its ports carry, stated for a face
/// over it to read through [`Encoded`]. The default is [`FlatBuffers`], because
/// both ends of a loopback are one process and FlatBuffers is the codec that
/// reads a payload in place; [`new`](Loopback::new) builds that default, and
/// [`with_encoding`](Loopback::with_encoding) names another. The encoding is
/// carried, not applied: the loopback moves bytes and decodes none of them.
pub struct Loopback<E: Encoding = FlatBuffers> {
    shared: Shared,
    catalog: CatalogRef,
    handles: Handles<E>,
}

impl Loopback<FlatBuffers> {
    /// The number of calls the runtime holds at once. A call holds its slot
    /// from its send until one of these reclaims it:
    ///
    /// - [`Caller::forget`], or the drop of the caller handle that sent it,
    ///   while it is settled or still waiting for a handler and offered to
    ///   none;
    /// - its settlement, when it was forgotten while a handler held it —
    ///   taken, or offered through `ReadError::ShortClaim` and still waiting
    ///   (driftsys/ridl#569);
    /// - the drop of the handler that held it unsettled, when it was
    ///   forgotten;
    /// - for a call forgotten while its provider still had a stream
    ///   parameter to read, the provider's pull of the last stream's end, a
    ///   settlement after it, or the drop of the handler.
    ///
    /// So a settled call keeps its slot until it is forgotten, and a claimed
    /// call keeps its slot after its `forget` until it is settled or its
    /// handler is dropped. With every slot taken, [`Caller::command`] and
    /// [`Caller::query`] answer [`SendError::Busy`], on every caller handle,
    /// because the table is the runtime's.
    ///
    /// Sixteen is a small bound, chosen so that a test reaches it in a few
    /// sends and a program that never forgets a call finds out at once rather
    /// than after its memory grows. The loopback has no catalog descriptor to
    /// size a byte budget from: the descriptor file is written by
    /// `--emit catalog` (driftsys/ridl#381), and nothing yet wires
    /// the loopback to read one. So the slot count is its only bound (note
    /// F-9 of the async face design).
    ///
    /// The generated async client's future forgets its call
    /// when it leaves the waiting phase: in the poll that takes the outcome,
    /// at the call's deadline, and on drop while the call is still waiting.
    /// So a program that calls through the generated face holds one slot per
    /// call in flight. A program that sends through the `Caller` port
    /// directly must forget each correlation itself once it has read the
    /// outcome, or drop the caller handle, which forgets that handle's calls;
    /// otherwise it gets `SendError::Busy` from its seventeenth call on.
    ///
    /// The count is the store's and the same under every encoding, so it is
    /// defined once, here, where `Loopback::SLOTS` resolves with no type
    /// argument.
    pub const SLOTS: usize = store::SLOTS;

    /// The number of non-empty chunks one stream of a call holds, pushed and
    /// not yet pulled. A push of a non-empty chunk into a stream that holds
    /// this many answers 0 and takes nothing, and the receiver's next pull
    /// frees room for one. An empty chunk is always taken, whatever the
    /// window holds.
    ///
    /// Each stream has one producer, so this is the window of one stream in
    /// its one direction: the reply stream's, which the provider pushes, and
    /// each stream parameter's, which the caller pushes. Four is small, so
    /// that a test reaches the bound in a few pushes, as with
    /// [`SLOTS`](Loopback::SLOTS).
    pub const STREAM_WINDOW: usize = store::STREAM_WINDOW;

    /// A runtime attached to `catalog`, with an empty store and its clock at
    /// [`Timestamp`] 0, whose ports carry FlatBuffers, the loopback's
    /// default encoding.
    ///
    /// The catalog is carried, not checked: see the crate documentation.
    #[must_use]
    pub fn new(catalog: CatalogRef) -> Self {
        Loopback::<FlatBuffers>::with_encoding(catalog)
    }
}

impl<E: Encoding> Loopback<E> {
    /// A runtime attached to `catalog`, with an empty store and its clock at
    /// [`Timestamp`] 0, whose ports carry `E`.
    ///
    /// `Loopback::<Proto3>::with_encoding(catalog)` is a runtime whose ports
    /// carry proto3, which a face over them reads through [`Encoded`]. The
    /// catalog is carried, not checked: see the crate documentation.
    #[must_use]
    pub fn with_encoding(catalog: CatalogRef) -> Self {
        Loopback::over(Arc::new(Mutex::new(Store::new())), catalog)
    }

    /// An aggregate of six new role handles on `shared`: what
    /// `with_encoding` and `attach` both build, over a new store and over
    /// this one.
    fn over(shared: Shared, catalog: CatalogRef) -> Self {
        let handles = Handles {
            reader: ReaderHandle::new(Arc::clone(&shared), catalog),
            writer: WriterHandle::new(Arc::clone(&shared), catalog),
            source: SourceHandle::new(Arc::clone(&shared), catalog),
            sink: SinkHandle::new(Arc::clone(&shared), catalog),
            caller: CallerHandle::new(Arc::clone(&shared), catalog),
            handler: HandlerHandle::new(Arc::clone(&shared), catalog),
        };
        Loopback {
            shared,
            catalog,
            handles,
        }
    }

    /// Hands out the six role handles this aggregate holds. Every one of them
    /// keeps the same store, so a value published through `writer` is read
    /// through `reader`.
    #[must_use]
    pub fn split(self) -> Handles<E> {
        self.handles
    }

    /// An additional aggregate on the same store: six new role handles, made
    /// the way [`reader`](Loopback::reader) to [`handler`](Loopback::handler)
    /// make one each. This is how an application holds several faces over one
    /// runtime — a `Client`, a `Publisher`, a second `Client` with a call of
    /// its own in flight — each owning its own aggregate (driftsys/ridl#488).
    ///
    /// What is in the store is shared: the published signals, the `fixed`
    /// values, the clock and the call table. What is on a handle is not
    /// carried over: the attached aggregate starts subscribed to nothing, with
    /// no value staged, no call sent and nothing served. Dropping it closes
    /// only its own handles, and the store lives until the last handle of
    /// every aggregate is dropped.
    ///
    /// Not `Clone`: `docs/design/ridl-loopback.md` records why.
    #[must_use]
    pub fn attach(&self) -> Loopback<E> {
        Loopback::over(Arc::clone(&self.shared), self.catalog)
    }

    /// An additional reader handle on the same store.
    #[must_use]
    pub fn reader(&self) -> ReaderHandle<E> {
        ReaderHandle::new(Arc::clone(&self.shared), self.catalog)
    }

    /// An additional writer handle on the same store, with its own staging
    /// area and its own per channel sequence counters.
    #[must_use]
    pub fn writer(&self) -> WriterHandle<E> {
        WriterHandle::new(Arc::clone(&self.shared), self.catalog)
    }

    /// An additional event source on the same store, with its own
    /// subscription set and its own queue.
    #[must_use]
    pub fn source(&self) -> SourceHandle<E> {
        SourceHandle::new(Arc::clone(&self.shared), self.catalog)
    }

    /// An additional event sink on the same store, with its own sequence
    /// counter.
    #[must_use]
    pub fn sink(&self) -> SinkHandle<E> {
        SinkHandle::new(Arc::clone(&self.shared), self.catalog)
    }

    /// An additional caller on the same store, with its own sequence counter.
    /// Two callers on one provider therefore never collide on a sequence
    /// number (driftsys/ridl#308).
    #[must_use]
    pub fn caller(&self) -> CallerHandle<E> {
        CallerHandle::new(Arc::clone(&self.shared), self.catalog)
    }

    /// An additional handler on the same store.
    ///
    /// Every handler draws from the one queue of waiting calls, filtered by
    /// what it has served: a handler that has served nothing is presented
    /// every waiting call, and one that has served members is presented only
    /// those. A claim belongs to the handler it was presented to.
    #[must_use]
    pub fn handler(&self) -> HandlerHandle<E> {
        HandlerHandle::new(Arc::clone(&self.shared), self.catalog)
    }

    /// Advances the clock by `by`.
    ///
    /// The clock is a counter this method moves and nothing else moves. It
    /// never reads wall-clock time, so a round trip over this runtime produces
    /// the same timestamps on every run and on every machine. It saturates
    /// rather than overflowing.
    ///
    /// # Panics
    ///
    /// When `by` is negative. A clock that ran backwards would stamp an
    /// envelope before one already stamped.
    pub fn advance(&mut self, by: Duration) {
        lock(&self.shared).advance(by);
    }

    /// Supplies the value of a `fixed` (ridl §8), which is provisioned into a
    /// runtime rather than published by application code.
    ///
    /// A value longer than the member's limit — the one [`set_limit`] gave
    /// it, else the default ceiling of a `fixed`, 65,280 bytes — is refused
    /// with `WriteError::TooLarge { cap }`, `cap` being that limit, and
    /// nothing is stored: a value provisioned before stays as it was.
    ///
    /// Until a `fixed` is provisioned, reading it answers
    /// `ReadError::Contract(Contract::UnknownInteraction)`: the loopback holds
    /// no value and, having no member table, cannot say anything narrower. A
    /// `fixed` whose only provision was refused reads the same way, so a
    /// consumer's read of it is answered rather than left waiting.
    ///
    /// [`set_limit`]: Loopback::set_limit
    pub fn provision_fixed(
        &mut self,
        iface: InterfaceNo,
        ord: Ordinal,
        bytes: &[u8],
    ) -> Result<(), WriteError> {
        lock(&self.shared).provision_fixed(iface, ord, bytes)
    }

    /// Gives member `ord` of interface `iface` the limit `limit`, in bytes
    /// of encoded payload, in place of the default ceiling of its kind.
    ///
    /// The loopback holds no catalog descriptor, so this is how a program
    /// tells it the limit that a `[limits]` entry of the manifest gives a
    /// member (ADR-0030 decision 12). The limit is the store's: every handle and
    /// every attached aggregate applies it, and a later call replaces it. A
    /// member with no limit given has the default ceiling of its kind,
    /// read from the port the payload is sent through
    /// ([`ridl_rt::contract::default_ceiling`]): 3,840 bytes for a signal
    /// and an event, 16,128 for a command and a query, 65,280 for a fixed.
    ///
    /// A sending port refuses a payload longer than the member's limit with
    /// its `TooLarge { cap }`, `cap` being that limit, and nothing is staged,
    /// raised, sent or recorded. The receiving side of this runtime reads
    /// only what its sending side accepted, so it adds no check.
    pub fn set_limit(&mut self, iface: InterfaceNo, ord: Ordinal, limit: usize) {
        lock(&self.shared).set_limit((iface, ord), limit);
    }

    /// Makes the next `Handler::settle` on this runtime fail with
    /// `SettleError::TooLarge` and record no outcome.
    ///
    /// This is the one fault this runtime injects, and the one place a
    /// `SettleError` other than `UnknownClaim` comes from. It exists because
    /// the generated `dispatch` counts a claim only once the handler has
    /// accepted its settlement, and nothing else in an in-process runtime can
    /// make that path fail.
    pub fn fail_next_settle(&mut self) {
        lock(&self.shared).fail_next_settle();
    }
}

// ---------------------------------------------------------------------------
// The aggregate's fourteen port implementations, each one a delegation.
// ---------------------------------------------------------------------------

impl<E: Encoding> Attached for Loopback<E> {
    fn catalog(&self) -> &CatalogRef {
        self.handles.reader.catalog()
    }
}

/// One session has one encoding: every port of this runtime carries `E`, and
/// an aggregate from [`Loopback::attach`] carries the same `E`.
impl<E: Encoding> Encoded for Loopback<E> {
    type Encoding = E;
}

impl<E: Encoding> Clock for Loopback<E> {
    fn now(&self) -> Timestamp {
        self.handles.reader.now()
    }
}

impl<E: Encoding> SignalReader for Loopback<E> {
    fn read(
        &self,
        iface: InterfaceNo,
        ord: Ordinal,
        out: &mut [u8],
    ) -> Result<RawSample, ReadError> {
        self.handles.reader.read(iface, ord, out)
    }
}

impl<E: Encoding> FixedReader for Loopback<E> {
    fn read_fixed(
        &self,
        iface: InterfaceNo,
        ord: Ordinal,
        out: &mut [u8],
    ) -> Result<usize, ReadError> {
        self.handles.reader.read_fixed(iface, ord, out)
    }
}

impl<E: Encoding> ScannableSignals for Loopback<E> {
    fn generation(&self, iface: InterfaceNo) -> u64 {
        self.handles.reader.generation(iface)
    }

    fn scan(&self, marks: &mut [Watermark], out: &mut [Changed]) -> usize {
        self.handles.reader.scan(marks, out)
    }
}

impl<E: Encoding> CoherentSignals for Loopback<E> {
    fn read_coherent(
        &self,
        iface: InterfaceNo,
        ords: &[Ordinal],
        out: &mut [u8],
        samples: &mut [RawSample],
    ) -> Result<usize, ReadError> {
        self.handles.reader.read_coherent(iface, ords, out, samples)
    }
}

impl<E: Encoding> SignalWriter for Loopback<E> {
    fn set(&mut self, iface: InterfaceNo, ord: Ordinal, bytes: &[u8]) -> Result<(), WriteError> {
        self.handles.writer.set(iface, ord, bytes)
    }

    fn invalidate(&mut self, iface: InterfaceNo, ord: Ordinal) -> Result<(), WriteError> {
        self.handles.writer.invalidate(iface, ord)
    }

    fn touch(&mut self, iface: InterfaceNo, ord: Ordinal) -> Result<(), WriteError> {
        self.handles.writer.touch(iface, ord)
    }

    fn commit(&mut self) {
        self.handles.writer.commit();
    }
}

impl<E: Encoding> EventSource for Loopback<E> {
    fn subscribe(&mut self, iface: InterfaceNo, ords: &[Ordinal]) -> Result<(), SubscribeError> {
        self.handles.source.subscribe(iface, ords)
    }

    fn unsubscribe(&mut self, iface: InterfaceNo, ords: &[Ordinal]) {
        self.handles.source.unsubscribe(iface, ords);
    }

    fn next(&mut self, out: &mut [u8]) -> Result<Option<RawOccurrence>, ReadError> {
        self.handles.source.next(out)
    }
}

impl<E: Encoding> EventSink for Loopback<E> {
    fn raise(
        &mut self,
        iface: InterfaceNo,
        ord: Ordinal,
        bytes: &[u8],
        trace: Option<TraceContext>,
    ) -> Result<(), RaiseError> {
        self.handles.sink.raise(iface, ord, bytes, trace)
    }
}

impl<E: Encoding> Caller for Loopback<E> {
    fn command(
        &mut self,
        iface: InterfaceNo,
        ord: Ordinal,
        args: &[u8],
        trace: Option<TraceContext>,
    ) -> Result<Correlation, SendError> {
        self.handles.caller.command(iface, ord, args, trace)
    }

    fn query(
        &mut self,
        iface: InterfaceNo,
        ord: Ordinal,
        args: &[u8],
        trace: Option<TraceContext>,
    ) -> Result<Correlation, SendError> {
        self.handles.caller.query(iface, ord, args, trace)
    }

    fn ack(&mut self, c: Correlation) -> Option<Result<(), CallError>> {
        self.handles.caller.ack(c)
    }

    fn reply(
        &mut self,
        c: Correlation,
        out: &mut [u8],
    ) -> Result<Option<Result<usize, CallError>>, ReadError> {
        self.handles.caller.reply(c, out)
    }

    fn forget(&mut self, c: Correlation) {
        self.handles.caller.forget(c);
    }
}

impl<E: Encoding> Handler for Loopback<E> {
    fn serve(&mut self, iface: InterfaceNo, ords: &[Ordinal]) -> Result<(), ServeError> {
        self.handles.handler.serve(iface, ords)
    }

    fn next_claim(&mut self, out: &mut [u8]) -> Result<Option<Claim>, ReadError> {
        self.handles.handler.next_claim(out)
    }

    fn settle(
        &mut self,
        claim: ClaimId,
        outcome: Result<&[u8], CallError>,
    ) -> Result<(), SettleError> {
        self.handles.handler.settle(claim, outcome)
    }
}

impl<E: Encoding> StreamCaller for Loopback<E> {
    fn pull(&mut self, c: Correlation, s: StreamNo, out: &mut [u8]) -> Result<Pull, ReadError> {
        self.handles.caller.pull(c, s, out)
    }

    fn push(&mut self, c: Correlation, s: StreamNo, chunk: &[u8]) -> Result<usize, PushError> {
        self.handles.caller.push(c, s, chunk)
    }

    fn end(&mut self, c: Correlation, s: StreamNo) -> Result<(), PushError> {
        self.handles.caller.end(c, s)
    }

    fn abort(&mut self, c: Correlation, s: StreamNo, e: CallError) -> Result<(), PushError> {
        self.handles.caller.abort(c, s, e)
    }
}

impl<E: Encoding> StreamHandler for Loopback<E> {
    fn pull_claim(
        &mut self,
        claim: ClaimId,
        s: StreamNo,
        out: &mut [u8],
    ) -> Result<Pull, ReadError> {
        self.handles.handler.pull_claim(claim, s, out)
    }

    fn push_claim(
        &mut self,
        claim: ClaimId,
        s: StreamNo,
        chunk: &[u8],
    ) -> Result<usize, PushError> {
        self.handles.handler.push_claim(claim, s, chunk)
    }

    fn end_claim(&mut self, claim: ClaimId, s: StreamNo) -> Result<(), PushError> {
        self.handles.handler.end_claim(claim, s)
    }

    fn abort_claim(&mut self, claim: ClaimId, s: StreamNo, e: CallError) -> Result<(), PushError> {
        self.handles.handler.abort_claim(claim, s, e)
    }
}

/// Each key goes to the handle that observes it: `Outcome`, `Slot` and
/// `Stream` to the caller, `Event` to the source, and `Claim` and
/// `ClaimStream` to the handler.
impl<E: Encoding> Wakeable for Loopback<E> {
    fn wake_on(&self, what: Interest, waker: &Waker) {
        match what {
            Interest::Outcome(_) | Interest::Slot | Interest::Stream(..) => {
                self.handles.caller.wake_on(what, waker)
            }
            Interest::Event(_) => self.handles.source.wake_on(what, waker),
            Interest::Claim(_) | Interest::ClaimStream(..) => {
                self.handles.handler.wake_on(what, waker)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The threading model, asserted rather than documented (ADR-0021 decision 12).
// ---------------------------------------------------------------------------

const _: () = {
    const fn assert_sync<T: Sync>() {}
    const fn assert_send<T: Send>() {}

    // Several threads may read one store at once.
    assert_sync::<ReaderHandle>();
    assert_send::<ReaderHandle>();

    // One thread drives each of the rest.
    assert_send::<WriterHandle>();
    assert_send::<SourceHandle>();
    assert_send::<SinkHandle>();
    assert_send::<CallerHandle>();
    assert_send::<HandlerHandle>();

    // The aggregate is as `Send` as the handles it holds, whatever its
    // encoding: `PhantomData` of a unit marker type adds no restriction.
    assert_send::<Loopback>();
    assert_send::<Loopback<ridl_rt::encoding::Proto3>>();
};
