//! The six handle types, one per port role, and their port implementations.
//!
//! A **port role** is one port trait (ADR-0021 decision 12). Every handle here
//! holds the same `Arc<Mutex<Store>>` and its own copy of the
//! [`CatalogRef`](ridl_rt::contract::CatalogRef) the runtime was built with,
//! so `Attached::catalog` can return a reference without reaching through the
//! lock.
//!
//! Every handle implements `Attached`, which every port trait but `Clock` has
//! as a supertrait. The table lists what each handle adds to it, and the split
//! follows the receiver of those methods, as ADR-0021 decision 12 derives it:
//!
//! | Handle            | Port roles beside `Attached`                                                  | Threading     |
//! | ----------------- | ----------------------------------------------------------------------------- | ------------- |
//! | [`ReaderHandle`]  | `Clock`, `SignalReader`, `FixedReader`, `ScannableSignals`, `CoherentSignals` | `Send + Sync` |
//! | [`WriterHandle`]  | `SignalWriter`                                                                | `Send`        |
//! | [`SourceHandle`]  | `EventSource`                                                                 | `Send`        |
//! | [`SinkHandle`]    | `EventSink`                                                                   | `Send`        |
//! | [`CallerHandle`]  | `Caller`                                                                      | `Send`        |
//! | [`HandlerHandle`] | `Handler`                                                                     | `Send`        |
//!
//! Every method on the reader handle takes `&self`, so several threads may
//! read one store at once; every other handle carries a trait with a
//! `&mut self` method and is driven by one thread at a time. Neither property
//! is declared: both are derived by the compiler from the fields, and
//! `crates/ridl-loopback/src/lib.rs` asserts them at compile time.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use ridl_rt::contract::{CatalogRef, InterfaceNo, Ordinal};
use ridl_rt::error::CallError;
use ridl_rt::port::{
    Attached, Caller, Changed, Claim, ClaimId, Clock, CoherentSignals, Correlation, EventSink,
    EventSource, FixedReader, Handler, RaiseError, RawOccurrence, RawSample, ReadError,
    ScannableSignals, SendError, ServeError, SettleError, SignalReader, SignalWriter,
    SubscribeError, Watermark, WriteError,
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
        self.staged.insert((iface, ord), Staged::Touch);
        Ok(())
    }

    fn commit(&mut self) {
        lock(&self.shared).commit(&mut self.staged, &mut self.seqs);
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

impl Drop for SourceHandle {
    fn drop(&mut self) {
        lock(&self.shared).close_source(self.id);
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
        lock(&self.shared).raise(iface, ord, bytes, seq);
        Ok(())
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
pub struct CallerHandle {
    shared: Shared,
    catalog: CatalogRef,
    next_seq: u64,
}

impl CallerHandle {
    pub(crate) fn new(shared: Shared, catalog: CatalogRef) -> Self {
        CallerHandle {
            shared,
            catalog,
            next_seq: 0,
        }
    }

    fn take_seq(&mut self) -> u64 {
        self.next_seq += 1;
        self.next_seq
    }
}

impl Attached for CallerHandle {
    fn catalog(&self) -> &CatalogRef {
        &self.catalog
    }
}

impl Caller for CallerHandle {
    fn command(
        &mut self,
        iface: InterfaceNo,
        ord: Ordinal,
        args: &[u8],
    ) -> Result<Correlation, SendError> {
        let seq = self.take_seq();
        Ok(lock(&self.shared).send(CallKind::Command, iface, ord, args, seq))
    }

    fn query(
        &mut self,
        iface: InterfaceNo,
        ord: Ordinal,
        args: &[u8],
    ) -> Result<Correlation, SendError> {
        let seq = self.take_seq();
        Ok(lock(&self.shared).send(CallKind::Query, iface, ord, args, seq))
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
        lock(&self.shared).forget(c);
    }
}

/// The `Handler` port role.
///
/// A handler that has served nothing is presented every call waiting in the
/// store; once it has served anything, it is presented only the members it
/// served, and another handler's calls stay waiting for that handler. The
/// empty set means no filter rather than no members, because the generated
/// `dispatch` never calls `serve` (`crates/ridl-backend-rust/src/face.rs`) and
/// a handler that filtered on an empty set would be presented nothing at all.
/// [`served`](HandlerHandle::served) reads the set back.
pub struct HandlerHandle {
    shared: Shared,
    catalog: CatalogRef,
    served: Vec<Key>,
}

impl HandlerHandle {
    pub(crate) fn new(shared: Shared, catalog: CatalogRef) -> Self {
        HandlerHandle {
            shared,
            catalog,
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
        Ok(())
    }

    fn next_claim(&mut self, out: &mut [u8]) -> Result<Option<Claim>, ReadError> {
        let served = if self.served.is_empty() {
            None
        } else {
            Some(self.served.as_slice())
        };
        lock(&self.shared).next_claim(served, out)
    }

    fn settle(
        &mut self,
        claim: ClaimId,
        outcome: Result<&[u8], CallError>,
    ) -> Result<(), SettleError> {
        lock(&self.shared).settle(claim, outcome)
    }
}
