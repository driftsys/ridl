//! `ridl-loopback` — the in-process reference runtime.
//!
//! This crate implements the twelve port traits of
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
//! each handle wakes its own waiters (see "Waking" below).
//! [`Loopback`] is the aggregate: it implements all twelve port traits by
//! delegating to the six role handles it holds, and it is what a generated
//! `Client`, `Publisher` or `dispatch` is normally built over.
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
//!
//! # What it reports, and what it cannot
//!
//! The loopback holds no catalog descriptor — the descriptor and the catalog
//! hash arrive with story E16.2 (driftsys/ridl#378) — so it has no member
//! table, and there is no ordinal it can call unknown, no member it can call
//! unowned, and no timing annotation it can measure a value's freshness or a
//! call's remaining time against. What it therefore never returns:
//! `WriteError::NotOwner`, `RaiseError::NotOwner`, `ServeError::NotOwner`, any
//! port error's `Contract` variant except
//! [`FixedReader::read_fixed`](ridl_rt::port::FixedReader::read_fixed)'s, and
//! `Freshness::Fresh` or `Freshness::Stale`. Nothing detaches, because every
//! handle holds the store alive, so `Detached` never appears either. The one
//! bound is the call table's [`Loopback::SLOTS`]: a send with every slot
//! taken answers `SendError::Busy`. Nothing else is bounded, so `TooLarge`
//! appears only from [`Loopback::fail_next_settle`], and `Transport::Busy`
//! reaches a caller only when a provider settles a call with it.
//!
//! # Waking
//!
//! Every handle implements [`Wakeable`], and a handle stores a waker only
//! under a kind of key one of its roles observes: a caller handle under
//! `Interest::Outcome`, kept with each call, and one waker under
//! `Interest::Slot`, a source handle one waker under `Interest::Event`, and a
//! handler handle one waker under `Interest::Claim`.
//! For `Event` and `Claim` the rule is one waker per kind, and a change to
//! any key of the kind wakes the stored waker, whatever interface it was
//! registered under (ADR-0021 decision 13); an `Outcome` waker is per call,
//! and no change but that call's settlement, its `forget`, or the drop of the
//! caller handle that sent it wakes it (a displacement by another task does,
//! as for every kind). A settlement wakes the
//! call's waiter, a raise wakes each source it queues the occurrence for, and
//! a send wakes each handler that serves the member, and a reclaimed slot of
//! the call table wakes every caller's `Slot` waiter. A registration whose key
//! already holds — the outcome is known, an occurrence or a call is waiting, a
//! slot is free — is woken at once, and so is one under a kind the handle does
//! not observe. A registration of the waker already stored refreshes it
//! without waking it. No waker is woken while the store is locked.
//!
//! [`Attached::catalog`](ridl_rt::port::Attached::catalog) returns the
//! `CatalogRef` the runtime was built with, unexamined. ADR-0021 decision 3
//! places a check of it against the interface's own `CATALOG` in a generated
//! face's constructor, once, when the face is built; the constructor the Rust
//! backend emits today performs no such check (driftsys/ridl#448). Either way
//! it is the face's check and not the runtime's: the loopback carries the
//! value and compares nothing.
//!
//! The crate's as-built design record, with the reasoning behind each of these
//! choices, is `docs/design/ridl-loopback.md` in this repository.

use std::sync::{Arc, Mutex};
use std::task::Waker;

use ridl_rt::contract::{CatalogRef, InterfaceNo, Ordinal};
use ridl_rt::error::CallError;
use ridl_rt::port::{
    Attached, Caller, Changed, Claim, ClaimId, Clock, CoherentSignals, Correlation, EventSink,
    EventSource, FixedReader, Handler, Interest, RaiseError, RawOccurrence, RawSample, ReadError,
    ScannableSignals, SendError, ServeError, SettleError, SignalReader, SignalWriter,
    SubscribeError, Wakeable, Watermark, WriteError,
};
use ridl_rt::sample::{Duration, Timestamp};

mod handle;
mod store;

pub use handle::{
    CallerHandle, HandlerHandle, ReaderHandle, SinkHandle, SourceHandle, WriterHandle,
};

use handle::{Shared, lock};
use store::Store;

/// The six role handles of one runtime, as [`Loopback::split`] hands them out.
pub struct Handles {
    /// `Attached`, `Clock`, `SignalReader`, `FixedReader`, `ScannableSignals`
    /// and `CoherentSignals`.
    pub reader: ReaderHandle,
    /// `SignalWriter`.
    pub writer: WriterHandle,
    /// `EventSource`.
    pub source: SourceHandle,
    /// `EventSink`.
    pub sink: SinkHandle,
    /// `Caller`.
    pub caller: CallerHandle,
    /// `Handler`.
    pub handler: HandlerHandle,
}

/// The aggregate handle: one value implementing all twelve port traits by
/// delegating to the six role handles it holds.
///
/// A face is built over one value implementing at least the port traits its
/// interface needs, and a generated `Client` is commonly bound over
/// `SignalReader + EventSource + Caller` at once, which no single role handle
/// satisfies. That is what an aggregate is for (ADR-0021 decision 12). Pass it
/// by value, or as `&mut` under the forwarding impls of ADR-0021 decision 11.
pub struct Loopback {
    shared: Shared,
    catalog: CatalogRef,
    handles: Handles,
}

impl Loopback {
    /// The number of calls the runtime holds at once: sent, and not yet
    /// released by [`Caller::forget`] or by the drop of the caller handle that
    /// sent them. A settled call keeps its slot until it is released. With every slot taken, [`Caller::command`] and
    /// [`Caller::query`] answer [`SendError::Busy`], on every caller handle,
    /// because the table is the runtime's.
    ///
    /// Sixteen is a small bound, chosen so that a test reaches it in a few
    /// sends and a program that never forgets a call finds out at once rather
    /// than after its memory grows. The loopback has no catalog descriptor to
    /// size a byte budget from (story E16.2), so the slot count is its only
    /// bound (note F-9 of the async face design).
    ///
    /// The generated face does not call `forget` yet, so a program that calls
    /// through it over one runtime gets `SendError::Busy` from its
    /// seventeenth call on, unless it drops the caller handle, which forgets
    /// that handle's calls. The async client of story E11.21 forgets each
    /// call once it has taken the outcome, which closes this limit; no
    /// release happens before it.
    pub const SLOTS: usize = 16;

    /// A runtime attached to `catalog`, with an empty store and its clock at
    /// [`Timestamp`] 0.
    ///
    /// The catalog is carried, not checked: see the crate documentation.
    #[must_use]
    pub fn new(catalog: CatalogRef) -> Self {
        let shared: Shared = Arc::new(Mutex::new(Store::new()));
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
    pub fn split(self) -> Handles {
        self.handles
    }

    /// An additional reader handle on the same store.
    #[must_use]
    pub fn reader(&self) -> ReaderHandle {
        ReaderHandle::new(Arc::clone(&self.shared), self.catalog)
    }

    /// An additional writer handle on the same store, with its own staging
    /// area and its own per channel sequence counters.
    #[must_use]
    pub fn writer(&self) -> WriterHandle {
        WriterHandle::new(Arc::clone(&self.shared), self.catalog)
    }

    /// An additional event source on the same store, with its own
    /// subscription set and its own queue.
    #[must_use]
    pub fn source(&self) -> SourceHandle {
        SourceHandle::new(Arc::clone(&self.shared), self.catalog)
    }

    /// An additional event sink on the same store, with its own sequence
    /// counter.
    #[must_use]
    pub fn sink(&self) -> SinkHandle {
        SinkHandle::new(Arc::clone(&self.shared), self.catalog)
    }

    /// An additional caller on the same store, with its own sequence counter.
    /// Two callers on one provider therefore never collide on a sequence
    /// number (driftsys/ridl#308).
    #[must_use]
    pub fn caller(&self) -> CallerHandle {
        CallerHandle::new(Arc::clone(&self.shared), self.catalog)
    }

    /// An additional handler on the same store.
    ///
    /// Every handler draws from the one queue of waiting calls, filtered by
    /// what it has served: a handler that has served nothing is presented
    /// every waiting call, and one that has served members is presented only
    /// those. A claim belongs to the handler it was presented to.
    #[must_use]
    pub fn handler(&self) -> HandlerHandle {
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
    /// Until a `fixed` is provisioned, reading it answers
    /// `ReadError::Contract(Contract::UnknownInteraction)`: the loopback holds
    /// no value and, having no member table, cannot say anything narrower.
    pub fn provision_fixed(&mut self, iface: InterfaceNo, ord: Ordinal, bytes: &[u8]) {
        lock(&self.shared).provision_fixed(iface, ord, bytes);
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
// The aggregate's twelve port implementations, each one a delegation.
// ---------------------------------------------------------------------------

impl Attached for Loopback {
    fn catalog(&self) -> &CatalogRef {
        self.handles.reader.catalog()
    }
}

impl Clock for Loopback {
    fn now(&self) -> Timestamp {
        self.handles.reader.now()
    }
}

impl SignalReader for Loopback {
    fn read(
        &self,
        iface: InterfaceNo,
        ord: Ordinal,
        out: &mut [u8],
    ) -> Result<RawSample, ReadError> {
        self.handles.reader.read(iface, ord, out)
    }
}

impl FixedReader for Loopback {
    fn read_fixed(
        &self,
        iface: InterfaceNo,
        ord: Ordinal,
        out: &mut [u8],
    ) -> Result<usize, ReadError> {
        self.handles.reader.read_fixed(iface, ord, out)
    }
}

impl ScannableSignals for Loopback {
    fn generation(&self, iface: InterfaceNo) -> u64 {
        self.handles.reader.generation(iface)
    }

    fn scan(&self, marks: &mut [Watermark], out: &mut [Changed]) -> usize {
        self.handles.reader.scan(marks, out)
    }
}

impl CoherentSignals for Loopback {
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

impl SignalWriter for Loopback {
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

impl EventSource for Loopback {
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

impl EventSink for Loopback {
    fn raise(&mut self, iface: InterfaceNo, ord: Ordinal, bytes: &[u8]) -> Result<(), RaiseError> {
        self.handles.sink.raise(iface, ord, bytes)
    }
}

impl Caller for Loopback {
    fn command(
        &mut self,
        iface: InterfaceNo,
        ord: Ordinal,
        args: &[u8],
    ) -> Result<Correlation, SendError> {
        self.handles.caller.command(iface, ord, args)
    }

    fn query(
        &mut self,
        iface: InterfaceNo,
        ord: Ordinal,
        args: &[u8],
    ) -> Result<Correlation, SendError> {
        self.handles.caller.query(iface, ord, args)
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

impl Handler for Loopback {
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

/// Each key goes to the handle that observes it: `Outcome` and `Slot` to the
/// caller, `Event` to the source, and `Claim` to the handler.
impl Wakeable for Loopback {
    fn wake_on(&self, what: Interest, waker: &Waker) {
        match what {
            Interest::Outcome(_) | Interest::Slot => self.handles.caller.wake_on(what, waker),
            Interest::Event(_) => self.handles.source.wake_on(what, waker),
            Interest::Claim(_) => self.handles.handler.wake_on(what, waker),
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

    // The aggregate is as `Send` as the handles it holds.
    assert_send::<Loopback>();
};
