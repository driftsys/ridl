//! The six handle types and their port implementations.
//!
//! A **port role** is one port trait, and a runtime presents one handle type
//! per role rather than one type implementing them all (ADR-0021 decision 12).
//! The six here group eleven roles the way that decision derives the
//! threading split: the five roles with a `&mut self` method take a handle
//! each, and the six whose methods all take `&self` share one, because they
//! are exactly the roles several threads may hold at once. The twelfth port
//! trait, `Wakeable`, is on every handle, because each handle wakes its own
//! waiters. Every handle here
//! holds the same `Arc<Mutex<Store>>` and its own copy of the
//! [`CatalogRef`](ridl_rt::contract::CatalogRef) the runtime was built with,
//! so `Attached::catalog` can return a reference without reaching through the
//! lock.
//!
//! Every handle implements `Attached`, which every port trait but `Clock` and
//! `Wakeable` has as a supertrait, and `Wakeable`. The table lists what each
//! handle adds to them, and the split follows the receiver of those methods,
//! as ADR-0021 decision 12 derives it:
//!
//! | Handle            | Port roles beside `Attached` and `Wakeable`                                   | Kinds of key it stores         | Threading     |
//! | ----------------- | ----------------------------------------------------------------------------- | ------------------------------ | ------------- |
//! | [`ReaderHandle`]  | `Clock`, `SignalReader`, `FixedReader`, `ScannableSignals`, `CoherentSignals` | none                           | `Send + Sync` |
//! | [`WriterHandle`]  | `SignalWriter`                                                                | none                           | `Send`        |
//! | [`SourceHandle`]  | `EventSource`                                                                 | `Event`, one waker             | `Send`        |
//! | [`SinkHandle`]    | `EventSink`                                                                   | none                           | `Send`        |
//! | [`CallerHandle`]  | `Clock`, `Caller`                                                             | `Outcome`, kept with each call; `Slot`, one waker | `Send`        |
//! | [`HandlerHandle`] | `Handler`                                                                     | `Claim`, one waker             | `Send`        |
//!
//! A handle stores a waker only under a kind of key one of its roles
//! observes. For `Event` and `Claim` that is one waker per kind, and a
//! change to any key of that kind wakes it (ADR-0021 decision 13). An
//! `Outcome` waker is per call: it is kept with its call, and no change but
//! that call's settlement or `forget` wakes it; a displacement by another
//! task does, as for every kind. A caller stores one `Slot` waker, woken at
//! once while a slot of the call table is free, and otherwise by the next
//! reclaim of a slot, which wakes every caller's `Slot` waker. A registration
//! under any other kind is woken at once, because nothing that handle could
//! read changes under it, and a stored waker would never be woken.
//!
//! Every method on the reader handle takes `&self`, so several threads may
//! read one store at once; every other handle carries a trait with a
//! `&mut self` method and is driven by one thread at a time. Neither property
//! is declared: both are derived by the compiler from the fields, and
//! `crates/ridl-loopback/src/lib.rs` asserts them at compile time.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::Waker;

use ridl_rt::contract::{CatalogRef, InterfaceNo, Ordinal};
use ridl_rt::error::CallError;
use ridl_rt::port::{
    Attached, Caller, Changed, Claim, ClaimId, Clock, CoherentSignals, Correlation, EventSink,
    EventSource, FixedReader, Handler, Interest, RaiseError, RawOccurrence, RawSample, ReadError,
    ScannableSignals, SendError, ServeError, SettleError, SignalReader, SignalWriter,
    SubscribeError, Wakeable, Watermark, WriteError,
};
use ridl_rt::sample::Timestamp;

use crate::store::{CallKind, Key, Staged, Store};

/// The shared store, as every handle holds it.
pub(crate) type Shared = Arc<Mutex<Store>>;

/// Takes the one lock.
///
/// A panic while the lock is held poisons it. This runtime recovers the guard
/// rather than propagating the poison, because every critical section here is
/// a read or a write of the maps that leaves them well formed, and a poisoned
/// lock would otherwise turn one panic in one test into a panic in every later
/// port call over the same runtime.
pub(crate) fn lock(shared: &Shared) -> MutexGuard<'_, Store> {
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Takes the one lock, runs `f` with a list of wakers to wake, releases the
/// lock, and then wakes each waker `f` put on the list.
///
/// A waker runs code the runtime does not control — an executor's scheduling,
/// or a test's own — and that code may call a port method on this runtime. So
/// no waker is woken while the lock is held.
pub(crate) fn locked<R>(shared: &Shared, f: impl FnOnce(&mut Store, &mut Vec<Waker>) -> R) -> R {
    let mut wake = Vec::new();
    let result = {
        let mut store = lock(shared);
        f(&mut store, &mut wake)
    };
    for waker in wake {
        waker.wake();
    }
    result
}

/// `Wakeable` on a handle none of whose roles observes any key: every
/// registration is woken at once.
fn wake_at_once(waker: &Waker) {
    waker.wake_by_ref();
}

// ---------------------------------------------------------------------------
// The reader handle
// ---------------------------------------------------------------------------

/// The `&self` port roles: `Attached`, `Clock`, `SignalReader`,
/// `FixedReader`, and the two signal extensions.
///
/// It is `Send + Sync`, so several threads may hold one and read at once, and
/// a face that needs only `SignalReader` can be built over this handle
/// directly rather than over the aggregate.
pub struct ReaderHandle {
    shared: Shared,
    catalog: CatalogRef,
}

impl ReaderHandle {
    pub(crate) fn new(shared: Shared, catalog: CatalogRef) -> Self {
        ReaderHandle { shared, catalog }
    }
}

impl Attached for ReaderHandle {
    fn catalog(&self) -> &CatalogRef {
        &self.catalog
    }
}

impl Clock for ReaderHandle {
    fn now(&self) -> Timestamp {
        lock(&self.shared).now()
    }
}

impl Wakeable for ReaderHandle {
    fn wake_on(&self, _: Interest, waker: &Waker) {
        wake_at_once(waker);
    }
}

impl SignalReader for ReaderHandle {
    fn read(
        &self,
        iface: InterfaceNo,
        ord: Ordinal,
        out: &mut [u8],
    ) -> Result<RawSample, ReadError> {
        lock(&self.shared).read(iface, ord, out)
    }
}

impl FixedReader for ReaderHandle {
    fn read_fixed(
        &self,
        iface: InterfaceNo,
        ord: Ordinal,
        out: &mut [u8],
    ) -> Result<usize, ReadError> {
        lock(&self.shared).read_fixed(iface, ord, out)
    }
}

impl ScannableSignals for ReaderHandle {
    fn generation(&self, iface: InterfaceNo) -> u64 {
        lock(&self.shared).generation(iface)
    }

    fn scan(&self, marks: &mut [Watermark], out: &mut [Changed]) -> usize {
        lock(&self.shared).scan(marks, out)
    }
}

impl CoherentSignals for ReaderHandle {
    fn read_coherent(
        &self,
        iface: InterfaceNo,
        ords: &[Ordinal],
        out: &mut [u8],
        samples: &mut [RawSample],
    ) -> Result<usize, ReadError> {
        lock(&self.shared).read_coherent(iface, ords, out, samples)
    }
}

// ---------------------------------------------------------------------------
// The writer handle
// ---------------------------------------------------------------------------

/// The `SignalWriter` port role.
///
/// The staged changes and the per channel sequence counters live on the
/// handle, not in the store: staging is one provider's private state until its
/// `commit`, and a sequence number is assigned by the sender (ridl §3.1).
pub struct WriterHandle {
    shared: Shared,
    catalog: CatalogRef,
    staged: BTreeMap<Key, Staged>,
    seqs: BTreeMap<Key, u64>,
}

impl WriterHandle {
    pub(crate) fn new(shared: Shared, catalog: CatalogRef) -> Self {
        WriterHandle {
            shared,
            catalog,
            staged: BTreeMap::new(),
            seqs: BTreeMap::new(),
        }
    }
}

impl Attached for WriterHandle {
    fn catalog(&self) -> &CatalogRef {
        &self.catalog
    }
}

impl SignalWriter for WriterHandle {
    fn set(&mut self, iface: InterfaceNo, ord: Ordinal, bytes: &[u8]) -> Result<(), WriteError> {
        self.staged
            .insert((iface, ord), Staged::Set(bytes.to_vec()));
        Ok(())
    }

    fn invalidate(&mut self, iface: InterfaceNo, ord: Ordinal) -> Result<(), WriteError> {
        self.staged.insert((iface, ord), Staged::Invalidate);
        Ok(())
    }

    fn touch(&mut self, iface: InterfaceNo, ord: Ordinal) -> Result<(), WriteError> {
        // A touch re-affirms the current value. It stages one only when
        // nothing else is staged for the channel: a `set` or an `invalidate`
        // already staged is itself a publication, and a re-affirmation adds
        // nothing to it. Replacing one here would discard a value this writer
        // staged, which is not what `touch` means. A later `set` or
        // `invalidate` does replace a staged touch, because each is a newer
        // decision about the same channel.
        self.staged.entry((iface, ord)).or_insert(Staged::Touch);
        Ok(())
    }

    fn commit(&mut self) {
        lock(&self.shared).commit(&mut self.staged, &mut self.seqs);
    }
}

impl Wakeable for WriterHandle {
    fn wake_on(&self, _: Interest, waker: &Waker) {
        wake_at_once(waker);
    }
}

// ---------------------------------------------------------------------------
// The event handles
// ---------------------------------------------------------------------------

/// The `EventSource` port role: one subscription set and one queue, both in
/// the store under this handle's identity.
///
/// Each source handle receives its own copy of every occurrence raised while
/// it is subscribed, so two consumers of one event never consume each other's
/// occurrences.
pub struct SourceHandle {
    shared: Shared,
    catalog: CatalogRef,
    id: usize,
}

impl SourceHandle {
    pub(crate) fn new(shared: Shared, catalog: CatalogRef) -> Self {
        let id = lock(&shared).open_source();
        SourceHandle {
            shared,
            catalog,
            id,
        }
    }
}

/// The source's waker is dropped after the lock is released: it may be the
/// last reference to a task that owns another handle of this runtime, and
/// that handle's own `Drop` takes the lock.
impl Drop for SourceHandle {
    fn drop(&mut self) {
        let waiters = lock(&self.shared).close_source(self.id);
        drop(waiters);
    }
}

impl Attached for SourceHandle {
    fn catalog(&self) -> &CatalogRef {
        &self.catalog
    }
}

impl EventSource for SourceHandle {
    fn subscribe(&mut self, iface: InterfaceNo, ords: &[Ordinal]) -> Result<(), SubscribeError> {
        lock(&self.shared).subscribe(self.id, iface, ords);
        Ok(())
    }

    fn unsubscribe(&mut self, iface: InterfaceNo, ords: &[Ordinal]) {
        lock(&self.shared).unsubscribe(self.id, iface, ords);
    }

    fn next(&mut self, out: &mut [u8]) -> Result<Option<RawOccurrence>, ReadError> {
        lock(&self.shared).next_event(self.id, out)
    }
}

/// Stores one `Event` waker, whatever interface it was registered under, woken
/// by any raise that queues an occurrence for this source.
impl Wakeable for SourceHandle {
    fn wake_on(&self, what: Interest, waker: &Waker) {
        match what {
            Interest::Event(_) => locked(&self.shared, |store, wake| {
                store.wait_event(self.id, what, waker, wake);
            }),
            Interest::Outcome(_) | Interest::Slot | Interest::Claim(_) => wake_at_once(waker),
        }
    }
}

/// The `EventSink` port role.
///
/// The sequence counters are the handle's own, one per event channel, for the
/// same reason the writer handle's are: ridl §3.1 scopes the number to the
/// channel and has the sender assign it. One counter for the whole handle
/// would make a consumer subscribed to some of this sink's events see a gap in
/// `seq` where nothing was lost, and `EventSource::next` states that a gap is
/// a loss.
pub struct SinkHandle {
    shared: Shared,
    catalog: CatalogRef,
    seqs: BTreeMap<Key, u64>,
}

impl SinkHandle {
    pub(crate) fn new(shared: Shared, catalog: CatalogRef) -> Self {
        SinkHandle {
            shared,
            catalog,
            seqs: BTreeMap::new(),
        }
    }

    fn take_seq(&mut self, key: Key) -> u64 {
        let counter = self.seqs.entry(key).or_insert(0);
        *counter += 1;
        *counter
    }
}

impl Attached for SinkHandle {
    fn catalog(&self) -> &CatalogRef {
        &self.catalog
    }
}

impl EventSink for SinkHandle {
    fn raise(&mut self, iface: InterfaceNo, ord: Ordinal, bytes: &[u8]) -> Result<(), RaiseError> {
        let seq = self.take_seq((iface, ord));
        locked(&self.shared, |store, wake| {
            store.raise(iface, ord, bytes, seq, wake);
        });
        Ok(())
    }
}

impl Wakeable for SinkHandle {
    fn wake_on(&self, _: Interest, waker: &Waker) {
        wake_at_once(waker);
    }
}

// ---------------------------------------------------------------------------
// The call handles
// ---------------------------------------------------------------------------

/// The `Caller` port role.
///
/// One counter for the whole handle, not one per channel: on a call the scope
/// is the caller instance (ADR-0021 decision 5, and driftsys/ridl#308's own
/// report), so every call this caller sends draws from one sequence. That is
/// what keeps two callers on one provider from colliding.
///
/// The call table is the runtime's, shared by every caller: with
/// [`Loopback::SLOTS`](crate::Loopback::SLOTS) calls sent and not forgotten,
/// a send answers [`SendError::Busy`] and draws no sequence number, because
/// nothing was sent.
pub struct CallerHandle {
    shared: Shared,
    catalog: CatalogRef,
    id: usize,
    next_seq: u64,
}

impl CallerHandle {
    pub(crate) fn new(shared: Shared, catalog: CatalogRef) -> Self {
        let id = lock(&shared).open_caller();
        CallerHandle {
            shared,
            catalog,
            id,
            next_seq: 0,
        }
    }

    /// Sends one call. The sequence number is drawn only when the table
    /// admits the call.
    fn send(
        &mut self,
        kind: CallKind,
        iface: InterfaceNo,
        ord: Ordinal,
        args: &[u8],
    ) -> Result<Correlation, SendError> {
        let seq = self.next_seq + 1;
        let c = locked(&self.shared, |store, wake| {
            store.send(kind, (iface, ord), args, seq, wake)
        })?;
        self.next_seq = seq;
        Ok(c)
    }
}

/// A dropped caller's `Slot` waker leaves the store. Its calls stay. The
/// waker is dropped after the lock is released, as the source's is.
impl Drop for CallerHandle {
    fn drop(&mut self) {
        let waiters = lock(&self.shared).close_caller(self.id);
        drop(waiters);
    }
}

impl Attached for CallerHandle {
    fn catalog(&self) -> &CatalogRef {
        &self.catalog
    }
}

/// The caller reads the runtime's one clock, so a generated async `Client`
/// can compute a call's deadline over this handle alone.
impl Clock for CallerHandle {
    fn now(&self) -> Timestamp {
        lock(&self.shared).now()
    }
}

/// Stores one `Outcome` waker per call, kept with the call and woken by its
/// settlement, its `forget`, or a displacement by another task. Stores one
/// `Slot` waker, woken at once while a slot is free and otherwise by the next
/// reclaim.
impl Wakeable for CallerHandle {
    fn wake_on(&self, what: Interest, waker: &Waker) {
        match what {
            Interest::Outcome(c) => locked(&self.shared, |store, wake| {
                store.wait_outcome(c, waker, wake);
            }),
            Interest::Slot => locked(&self.shared, |store, wake| {
                store.wait_slot(self.id, waker, wake);
            }),
            Interest::Event(_) | Interest::Claim(_) => wake_at_once(waker),
        }
    }
}

impl Caller for CallerHandle {
    fn command(
        &mut self,
        iface: InterfaceNo,
        ord: Ordinal,
        args: &[u8],
    ) -> Result<Correlation, SendError> {
        self.send(CallKind::Command, iface, ord, args)
    }

    fn query(
        &mut self,
        iface: InterfaceNo,
        ord: Ordinal,
        args: &[u8],
    ) -> Result<Correlation, SendError> {
        self.send(CallKind::Query, iface, ord, args)
    }

    fn ack(&mut self, c: Correlation) -> Option<Result<(), CallError>> {
        lock(&self.shared).ack(c)
    }

    fn reply(
        &mut self,
        c: Correlation,
        out: &mut [u8],
    ) -> Result<Option<Result<usize, CallError>>, ReadError> {
        lock(&self.shared).reply(c, out)
    }

    fn forget(&mut self, c: Correlation) {
        locked(&self.shared, |store, wake| store.forget(c, wake));
    }
}

/// The `Handler` port role.
///
/// A handler that has served nothing is presented every call waiting in the
/// store; once it has served anything, it is presented only the members it
/// served, and another handler's calls stay waiting for that handler. A claim
/// belongs to the handler it was presented to, so another handler's `settle`
/// of it answers [`SettleError::UnknownClaim`].
///
/// The empty set meaning no filter is a deliberate deviation from
/// [`Handler::serve`], which says delivery starts at the members listed. The
/// generated `dispatch` never calls `serve`
/// (`crates/ridl-backend-rust/src/face.rs`), so a handler that always filtered
/// would be presented nothing at all by it. `serve` with an empty slice
/// records nothing and so leaves the handler unfiltered, the same as never
/// having called it. [`served`](HandlerHandle::served) reads the set back.
///
/// The served set is kept twice: here, for `served` to return a slice, and in
/// the store, where `next_claim` filters by it and a caller's send reads it to
/// know which handler to wake. `serve` is the one method that changes it, and
/// it changes both.
pub struct HandlerHandle {
    shared: Shared,
    catalog: CatalogRef,
    id: usize,
    served: Vec<Key>,
}

impl HandlerHandle {
    pub(crate) fn new(shared: Shared, catalog: CatalogRef) -> Self {
        let id = lock(&shared).open_handler();
        HandlerHandle {
            shared,
            catalog,
            id,
            served: Vec::new(),
        }
    }

    /// The members `serve` was called with, in the order they were served,
    /// with no duplicate.
    #[must_use]
    pub fn served(&self) -> &[(InterfaceNo, Ordinal)] {
        &self.served
    }
}

/// A claim this handler holds and has not settled returns to the waiting
/// calls, and every handler that serves its member is woken. The handler's
/// waker is dropped after the lock is released, as the source's is.
impl Drop for HandlerHandle {
    fn drop(&mut self) {
        let waiters = locked(&self.shared, |store, wake| {
            store.close_handler(self.id, wake)
        });
        drop(waiters);
    }
}

impl Attached for HandlerHandle {
    fn catalog(&self) -> &CatalogRef {
        &self.catalog
    }
}

impl Handler for HandlerHandle {
    fn serve(&mut self, iface: InterfaceNo, ords: &[Ordinal]) -> Result<(), ServeError> {
        for ord in ords {
            if !self.served.contains(&(iface, *ord)) {
                self.served.push((iface, *ord));
            }
        }
        locked(&self.shared, |store, wake| {
            store.serve(self.id, iface, ords, wake);
        });
        Ok(())
    }

    fn next_claim(&mut self, out: &mut [u8]) -> Result<Option<Claim>, ReadError> {
        lock(&self.shared).next_claim(self.id, out)
    }

    fn settle(
        &mut self,
        claim: ClaimId,
        outcome: Result<&[u8], CallError>,
    ) -> Result<(), SettleError> {
        locked(&self.shared, |store, wake| {
            store.settle(self.id, claim, outcome, wake)
        })
    }
}

/// Stores one `Claim` waker, whatever interface it was registered under, woken
/// by a send of a member this handler serves, by a `serve` that admits a call
/// already waiting, or by another handler's drop that returns a claim this
/// handler serves.
impl Wakeable for HandlerHandle {
    fn wake_on(&self, what: Interest, waker: &Waker) {
        match what {
            Interest::Claim(_) => locked(&self.shared, |store, wake| {
                store.wait_claim(self.id, what, waker, wake);
            }),
            Interest::Outcome(_) | Interest::Slot | Interest::Event(_) => wake_at_once(waker),
        }
    }
}
