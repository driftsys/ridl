//! The one store every handle shares, and the operations the handles call on
//! it.
//!
//! Every handle of one runtime holds an `Arc<Mutex<Store>>` pointing here, so
//! a value published through a writer handle is visible to a reader handle,
//! and a call sent through a caller handle is presented to a handler handle.
//! The critical sections below are the whole of the runtime's locking: each
//! one reads or writes the maps and returns, and no port method waits for
//! data while holding the lock, which is what `ridl_rt::port` requires of
//! every port method.
//!
//! What lives here and what lives on a handle is a division with one rule:
//! state two handles must agree on lives here, and state that belongs to one
//! handle alone lives on that handle. So the signal map, the event queues, the
//! call table, the handlers' served sets, the stored wakers and the clock are
//! here; a writer's staged changes and its per channel sequence counters, a
//! caller's sequence counter, and a source's, a caller's and a handler's
//! identity, are on the handle. A served set is here because a caller's send
//! must know which handler to wake, and a waker is here because another
//! handle's operation wakes it.
//!
//! The caller side of the call table is `ridl_rt::correlate::Table`, with
//! [`Loopback::SLOTS`](crate::Loopback::SLOTS) slots and no byte budget. It
//! holds each call's outcome status and its `Outcome` waker; the call's
//! arguments, its place in send order and its reply bytes are kept here by
//! slot, beside it. Each source, caller and handler keeps its `Event`, `Slot`
//! or `Claim` waker in a `ridl_rt::correlate::Waiters`.
//!
//! An operation that changes what a waiter waits for takes a `wake` list and
//! pushes the wakers to wake onto it. It never wakes one itself: the handle
//! wakes them after it has released the lock ([`crate::handle::locked`]), so
//! no waker is woken while the store is locked. A waker is cloned under the
//! lock, and a refreshed one is dropped under it; neither wakes a task. The
//! wakers of a closing handle are dropped after the lock is released, because
//! the last reference to one may own another handle whose `Drop` takes the
//! lock.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::task::Waker;

use ridl_rt::contract::{InterfaceNo, Ordinal};
use ridl_rt::correlate::{Forgotten, Settled, Table, Waiters};
use ridl_rt::error::CallError;
use ridl_rt::port::{
    Changed, Claim, ClaimId, Correlation, Interest, RawOccurrence, RawSample, ReadError, SendError,
    SettleError, Watermark,
};
use ridl_rt::sample::{Cause, Envelope, Freshness, Provenance, Timestamp};

/// One interaction, addressed the way every port method addresses one.
pub(crate) type Key = (InterfaceNo, Ordinal);

/// The caller-side call table: sixteen slots and no byte budget.
type Calls = Table<{ crate::Loopback::SLOTS }>;

/// A signal channel's published state.
struct SignalEntry {
    bytes: Vec<u8>,
    envelope: Envelope,
    /// `true` after `invalidate`, cleared by the next `set`.
    invalid: bool,
    /// The interface generation this entry last changed at, which is what
    /// [`Store::scan`] selects on.
    changed_at: u64,
}

/// A staged change, held on the writer handle until its `commit`.
pub(crate) enum Staged {
    Set(Vec<u8>),
    Invalidate,
    Touch,
}

/// One occurrence in one source handle's queue.
struct QueuedEvent {
    iface: InterfaceNo,
    ord: Ordinal,
    bytes: Vec<u8>,
    envelope: Envelope,
}

/// One source handle's subscription set, its own queue, and its waiter. `raise`
/// copies an occurrence into the queue of every source subscribed to it at
/// that moment, so each source reads its own occurrences in order and no
/// source consumes another's.
#[derive(Default)]
struct SourceState {
    subscribed: BTreeSet<Key>,
    queue: VecDeque<QueuedEvent>,
    /// The one `Interest::Event` waker this handle holds, whatever interface
    /// it was registered under: any occurrence queued for this source wakes
    /// it.
    waiters: Waiters,
}

/// One handler handle's served set and its waiter.
#[derive(Default)]
struct HandlerState {
    /// The members `serve` recorded. Empty is no filter: see
    /// [`HandlerHandle`](crate::HandlerHandle).
    served: Vec<Key>,
    /// The one `Interest::Claim` waker this handle holds, whatever interface
    /// it was registered under: any call this handler serves wakes it.
    waiters: Waiters,
}

impl HandlerState {
    fn serves(&self, key: Key) -> bool {
        self.served.is_empty() || self.served.contains(&key)
    }
}

/// A presented claim: the call it presented, and the handler holding it.
struct ClaimOwner {
    call: Correlation,
    handler: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum CallKind {
    Command,
    Query,
}

/// What the loopback keeps for one sent call, by its slot in the table, from
/// the caller's send until the table reclaims the slot. The outcome status,
/// whether the call was forgotten, and its `Interest::Outcome` waker are the
/// table's.
///
/// A forgotten call keeps its entry until its slot is reclaimed, because the
/// provider's side of the call is not the caller's to revoke: a claim already
/// presented is still settled, and a call still waiting is still presented.
struct CallEntry {
    /// The call's correlation, which a dropped caller's calls are forgotten
    /// by.
    correlation: Correlation,
    /// The caller handle that sent the call.
    caller: usize,
    kind: CallKind,
    iface: InterfaceNo,
    ord: Ordinal,
    args: Vec<u8>,
    envelope: Envelope,
    /// The call's place in send order, which a returned claim goes back in
    /// by. A correlation does not give that order once a slot is reused.
    sent: u64,
    /// The bytes of a successful settlement. Empty until then.
    reply: Vec<u8>,
}

/// Everything two handles must agree on.
///
/// The maps are `BTreeMap`s rather than hash maps so that a commit applies its
/// staged changes, and a scan reports its changes, in one order on every run:
/// a reference runtime a test compares output against has no reason to be
/// nondeterministic.
pub(crate) struct Store {
    now: Timestamp,
    signals: BTreeMap<Key, SignalEntry>,
    generations: BTreeMap<InterfaceNo, u64>,
    fixed: BTreeMap<Key, Vec<u8>>,
    sources: BTreeMap<usize, SourceState>,
    next_source_id: usize,
    table: Calls,
    /// The calls the table holds, by slot.
    calls: BTreeMap<usize, CallEntry>,
    /// The calls sent and not yet presented, in send order.
    pending: VecDeque<Correlation>,
    next_sent: u64,
    /// Each open caller's `Interest::Slot` waker. A caller has an entry from
    /// `open_caller` to `close_caller`.
    callers: BTreeMap<usize, Waiters>,
    next_caller_id: usize,
    /// The calls presented and not yet settled: a claim identity of its own,
    /// minted by `next_claim`, to the call it presented and the handler it was
    /// presented to. A `ClaimId` is therefore never a correlation that was
    /// never presented, never one already settled, and never one another
    /// handler holds.
    claims: BTreeMap<u64, ClaimOwner>,
    next_claim_id: u64,
    handlers: BTreeMap<usize, HandlerState>,
    next_handler_id: usize,
    fail_next_settle: bool,
}

impl Store {
    pub(crate) fn new() -> Self {
        Store {
            now: Timestamp(0),
            signals: BTreeMap::new(),
            generations: BTreeMap::new(),
            fixed: BTreeMap::new(),
            sources: BTreeMap::new(),
            next_source_id: 0,
            table: Calls::new(None),
            calls: BTreeMap::new(),
            pending: VecDeque::new(),
            next_sent: 0,
            callers: BTreeMap::new(),
            next_caller_id: 0,
            claims: BTreeMap::new(),
            next_claim_id: 0,
            handlers: BTreeMap::new(),
            next_handler_id: 0,
            fail_next_settle: false,
        }
    }

    // -- the clock ---------------------------------------------------------

    pub(crate) fn now(&self) -> Timestamp {
        self.now
    }

    pub(crate) fn advance(&mut self, by: ridl_rt::sample::Duration) {
        assert!(by.0 >= 0, "the clock advances forward: `by` is {}", by.0);
        self.now = Timestamp(self.now.0.saturating_add(by.0));
    }

    // -- signals -----------------------------------------------------------

    pub(crate) fn read(
        &self,
        iface: InterfaceNo,
        ord: Ordinal,
        out: &mut [u8],
    ) -> Result<RawSample, ReadError> {
        let Some(entry) = self.signals.get(&(iface, ord)) else {
            return Ok(unpublished());
        };
        if out.len() < entry.bytes.len() {
            return Err(ReadError::Short {
                needed: entry.bytes.len(),
            });
        }
        out[..entry.bytes.len()].copy_from_slice(&entry.bytes);
        Ok(sample_of(entry))
    }

    /// Applies one writer handle's staged changes: one generation increment
    /// and one timestamp per interface touched, and one sequence number per
    /// channel published, taken from the writer's own counters.
    pub(crate) fn commit(
        &mut self,
        staged: &mut BTreeMap<Key, Staged>,
        seqs: &mut BTreeMap<Key, u64>,
    ) {
        let stamp = self.now;
        // A `touch` of a channel with no publication re-affirms nothing, so it
        // is dropped here rather than publishing a zero-length value as
        // `Live`. It is dropped before the generation is advanced, so a commit
        // whose every staged change is such a touch changes nothing at all.
        staged.retain(|key, op| !matches!(op, Staged::Touch) || self.signals.contains_key(key));
        let mut bumped = BTreeSet::new();
        for (iface, _) in staged.keys() {
            if bumped.insert(*iface) {
                *self.generations.entry(*iface).or_insert(0) += 1;
            }
        }
        for (key, op) in std::mem::take(staged) {
            let seq = {
                let counter = seqs.entry(key).or_insert(0);
                *counter += 1;
                *counter
            };
            let changed_at = self.generations[&key.0];
            let envelope = Envelope { stamp, seq };
            let previous = self.signals.get(&key);
            let (bytes, invalid) = match op {
                Staged::Set(bytes) => (bytes, false),
                Staged::Invalidate => (previous.map(|e| e.bytes.clone()).unwrap_or_default(), true),
                Staged::Touch => {
                    let entry = previous.expect("an unpublished touch was dropped above");
                    (entry.bytes.clone(), entry.invalid)
                }
            };
            self.signals.insert(
                key,
                SignalEntry {
                    bytes,
                    envelope,
                    invalid,
                    changed_at,
                },
            );
        }
    }

    pub(crate) fn generation(&self, iface: InterfaceNo) -> u64 {
        self.generations.get(&iface).copied().unwrap_or(0)
    }

    /// Writes each interface's changes since its mark, in the order of
    /// `marks`, and updates the marks it wrote.
    ///
    /// An interface's changes go into `out` all together or not at all. On the
    /// first interface whose changes do not fit in what is left of `out`, the
    /// scan stops with that interface's mark untouched and returns what it
    /// wrote so far, so the order of `marks` is the order the caller is served
    /// in and a caller that grows `out` makes progress.
    pub(crate) fn scan(&self, marks: &mut [Watermark], out: &mut [Changed]) -> usize {
        let mut written = 0usize;
        for mark in marks.iter_mut() {
            let current = self.generation(mark.iface);
            if mark.generation >= current {
                continue;
            }
            let changes = self
                .signals
                .iter()
                .filter(|((iface, _), entry)| {
                    *iface == mark.iface && entry.changed_at > mark.generation
                })
                .map(|((iface, ord), entry)| Changed {
                    iface: *iface,
                    ord: *ord,
                    seq: entry.envelope.seq,
                });
            let staging: Vec<Changed> = changes.collect();
            if staging.len() > out.len() - written {
                return written;
            }
            for change in &staging {
                out[written] = *change;
                written += 1;
            }
            mark.generation = current;
            if let Some(highest) = staging.iter().map(|change| change.seq).max() {
                mark.seq = highest;
            }
        }
        written
    }

    /// Answers every ordinal from the one publication state the lock is held
    /// over, which is what makes the read coherent.
    pub(crate) fn read_coherent(
        &self,
        iface: InterfaceNo,
        ords: &[Ordinal],
        out: &mut [u8],
        samples: &mut [RawSample],
    ) -> Result<usize, ReadError> {
        if samples.len() < ords.len() {
            return Err(ReadError::TooFewSamples { needed: ords.len() });
        }
        let needed: usize = ords
            .iter()
            .map(|ord| {
                self.signals
                    .get(&(iface, *ord))
                    .map_or(0, |entry| entry.bytes.len())
            })
            .sum();
        if out.len() < needed {
            return Err(ReadError::Short { needed });
        }
        let mut written = 0usize;
        for (index, ord) in ords.iter().enumerate() {
            match self.signals.get(&(iface, *ord)) {
                None => samples[index] = unpublished(),
                Some(entry) => {
                    out[written..written + entry.bytes.len()].copy_from_slice(&entry.bytes);
                    written += entry.bytes.len();
                    samples[index] = sample_of(entry);
                }
            }
        }
        Ok(written)
    }

    // -- `fixed` -----------------------------------------------------------

    pub(crate) fn provision_fixed(&mut self, iface: InterfaceNo, ord: Ordinal, bytes: &[u8]) {
        self.fixed.insert((iface, ord), bytes.to_vec());
    }

    pub(crate) fn read_fixed(
        &self,
        iface: InterfaceNo,
        ord: Ordinal,
        out: &mut [u8],
    ) -> Result<usize, ReadError> {
        let Some(bytes) = self.fixed.get(&(iface, ord)) else {
            return Err(ReadError::Contract(
                ridl_rt::error::Contract::UnknownInteraction,
            ));
        };
        if out.len() < bytes.len() {
            return Err(ReadError::Short {
                needed: bytes.len(),
            });
        }
        out[..bytes.len()].copy_from_slice(bytes);
        Ok(bytes.len())
    }

    // -- events ------------------------------------------------------------

    pub(crate) fn open_source(&mut self) -> usize {
        let id = self.next_source_id;
        self.next_source_id += 1;
        self.sources.insert(id, SourceState::default());
        id
    }

    /// Removes a dropped source and returns its waiters, for the handle to
    /// drop after the lock is released: a stored waker may be the last
    /// reference to a task that owns another handle of this runtime, and that
    /// handle's own `Drop` takes the lock.
    pub(crate) fn close_source(&mut self, id: usize) -> Option<Waiters> {
        self.sources.remove(&id).map(|state| state.waiters)
    }

    pub(crate) fn subscribe(&mut self, id: usize, iface: InterfaceNo, ords: &[Ordinal]) {
        let Some(state) = self.sources.get_mut(&id) else {
            return;
        };
        for ord in ords {
            state.subscribed.insert((iface, *ord));
        }
    }

    /// Stops delivery: the ordinals leave the subscription set, and the
    /// occurrences already queued for them are dropped rather than delivered
    /// after the unsubscribe.
    pub(crate) fn unsubscribe(&mut self, id: usize, iface: InterfaceNo, ords: &[Ordinal]) {
        let Some(state) = self.sources.get_mut(&id) else {
            return;
        };
        let SourceState {
            subscribed, queue, ..
        } = state;
        for ord in ords {
            subscribed.remove(&(iface, *ord));
        }
        queue.retain(|event| subscribed.contains(&(event.iface, event.ord)));
    }

    /// Copies the occurrence into the queue of every source subscribed to it
    /// now. A source that subscribes later receives nothing retroactive, which
    /// is ridl's own rule for a late joiner on an event.
    ///
    /// Each source it queues the occurrence for has its `Event` waiter woken,
    /// whatever interface that waiter was registered under.
    pub(crate) fn raise(
        &mut self,
        iface: InterfaceNo,
        ord: Ordinal,
        bytes: &[u8],
        seq: u64,
        wake: &mut Vec<Waker>,
    ) {
        let envelope = Envelope {
            stamp: self.now,
            seq,
        };
        for state in self.sources.values_mut() {
            if state.subscribed.contains(&(iface, ord)) {
                state.queue.push_back(QueuedEvent {
                    iface,
                    ord,
                    bytes: bytes.to_vec(),
                    envelope,
                });
                wake.extend(state.waiters.take(Interest::Event(iface)));
            }
        }
    }

    /// Stores a source's `Interest::Event` waker, or wakes it at once when an
    /// occurrence is already in the source's queue. The interface of the key
    /// is not kept: the handle holds one `Event` waker, and any occurrence
    /// queued for it wakes that waker.
    pub(crate) fn wait_event(
        &mut self,
        id: usize,
        what: Interest,
        waker: &Waker,
        wake: &mut Vec<Waker>,
    ) {
        let Some(state) = self.sources.get_mut(&id) else {
            wake.push(waker.clone());
            return;
        };
        let ready = !state.queue.is_empty();
        register(&mut state.waiters, what, waker, ready, wake);
    }

    pub(crate) fn next_event(
        &mut self,
        id: usize,
        out: &mut [u8],
    ) -> Result<Option<RawOccurrence>, ReadError> {
        let Some(state) = self.sources.get_mut(&id) else {
            return Ok(None);
        };
        let Some(front) = state.queue.front() else {
            return Ok(None);
        };
        if out.len() < front.bytes.len() {
            return Err(ReadError::Short {
                needed: front.bytes.len(),
            });
        }
        let event = state.queue.pop_front().expect("the front was just read");
        out[..event.bytes.len()].copy_from_slice(&event.bytes);
        Ok(Some(RawOccurrence {
            iface: event.iface,
            ord: event.ord,
            envelope: event.envelope,
            len: event.bytes.len(),
        }))
    }

    // -- calls -------------------------------------------------------------

    /// Takes a slot in the call table and queues the call for presentation,
    /// or answers `SendError::Busy` when every slot is taken. Every handler
    /// that serves the member has its `Claim` waiter woken, whatever
    /// interface that waiter was registered under.
    pub(crate) fn send(
        &mut self,
        caller: usize,
        kind: CallKind,
        (iface, ord): Key,
        args: &[u8],
        seq: u64,
        wake: &mut Vec<Waker>,
    ) -> Result<Correlation, SendError> {
        // No budget, so the reservation is not read.
        let c = self.table.insert(0).ok_or(SendError::Busy)?;
        let sent = self.next_sent;
        self.next_sent += 1;
        let envelope = Envelope {
            stamp: self.now,
            seq,
        };
        self.calls.insert(
            Calls::slot(c),
            CallEntry {
                correlation: c,
                caller,
                kind,
                iface,
                ord,
                args: args.to_vec(),
                envelope,
                sent,
                reply: Vec::new(),
            },
        );
        self.pending.push_back(c);
        self.wake_handlers_serving((iface, ord), wake);
        Ok(c)
    }

    /// The entry of a call the table holds.
    fn entry(&self, c: Correlation) -> &CallEntry {
        &self.calls[&Calls::slot(c)]
    }

    /// Stores a call's `Interest::Outcome` waker, or wakes it at once when the
    /// outcome is already recorded or when no call in flight has that
    /// correlation — an unknown or forgotten one — because no outcome will
    /// ever be recorded for it. The table decides which.
    pub(crate) fn wait_outcome(&mut self, c: Correlation, waker: &Waker, wake: &mut Vec<Waker>) {
        wake.extend(self.table.wake_on(c, waker));
    }

    pub(crate) fn ack(&self, c: Correlation) -> Option<Result<(), CallError>> {
        // The table answers `None` for a call in flight, forgotten, or not
        // held; a correlation it answers for has its entry here.
        let outcome = self.table.outcome(c)?;
        if self.entry(c).kind != CallKind::Command {
            // A query's correlation always answers `None` here, which
            // `Caller::ack` states: a query's outcome comes from `reply`.
            return None;
        }
        Some(outcome)
    }

    pub(crate) fn reply(
        &self,
        c: Correlation,
        out: &mut [u8],
    ) -> Result<Option<Result<usize, CallError>>, ReadError> {
        match self.table.outcome(c) {
            None => Ok(None),
            Some(Err(error)) => Ok(Some(Err(error))),
            Some(Ok(())) => {
                let bytes = &self.entry(c).reply;
                if out.len() < bytes.len() {
                    return Err(ReadError::Short {
                        needed: bytes.len(),
                    });
                }
                out[..bytes.len()].copy_from_slice(bytes);
                Ok(Some(Ok(bytes.len())))
            }
        }
    }

    /// Releases the caller's interest in a correlation, which is the one
    /// operation that frees a slot.
    ///
    /// A call whose outcome is already recorded has nothing left to happen to
    /// it, so its slot is reclaimed now. A call still in flight keeps its slot
    /// and is marked forgotten: the provider still sees it at `next_claim` and
    /// still settles it — `Handler`'s contract is that every claim is settled,
    /// and a caller losing interest is not the provider's business — and the
    /// slot is reclaimed when that settlement lands. Either way the
    /// correlation answers `None` from `ack` and `reply` afterwards.
    ///
    /// A waiter on the outcome of a call still in flight is woken: no outcome
    /// will be recorded for it, so it would otherwise never be.
    pub(crate) fn forget(&mut self, c: Correlation, wake: &mut Vec<Waker>) {
        match self.table.forget(c) {
            Forgotten::Reclaimed => self.reclaimed(c, wake),
            Forgotten::Marked(waker) => wake.extend(waker),
            Forgotten::Unknown => {}
        }
    }

    /// Drops what the loopback kept for a reclaimed slot, and takes every
    /// caller's `Slot` waker: the first to send again takes the slot, and the
    /// others find the table full and register again (note F-5, no queue).
    fn reclaimed(&mut self, c: Correlation, wake: &mut Vec<Waker>) {
        self.calls.remove(&Calls::slot(c));
        for waiters in self.callers.values_mut() {
            wake.extend(waiters.take(Interest::Slot));
        }
    }

    pub(crate) fn open_caller(&mut self) -> usize {
        let id = self.next_caller_id;
        self.next_caller_id += 1;
        self.callers.insert(id, Waiters::new());
        id
    }

    /// Removes a dropped caller, and forgets every call it sent and did not
    /// forget: a settled one's slot is reclaimed now, and one still in flight
    /// is marked, so its settlement reclaims the slot. No handle can read the
    /// outcome of a call whose caller is gone, and a slot kept for it would
    /// be lost to every other caller.
    ///
    /// Returns the caller's waiters, for the handle to drop after the lock is
    /// released, as `close_source` explains.
    pub(crate) fn close_caller(&mut self, id: usize, wake: &mut Vec<Waker>) -> Option<Waiters> {
        let waiters = self.callers.remove(&id);
        let sent: Vec<Correlation> = self
            .calls
            .values()
            .filter(|entry| entry.caller == id)
            .map(|entry| entry.correlation)
            .collect();
        for c in sent {
            self.forget(c, wake);
        }
        waiters
    }

    /// Stores a caller's `Interest::Slot` waker, or wakes it at once when a
    /// slot is free.
    pub(crate) fn wait_slot(&mut self, id: usize, waker: &Waker, wake: &mut Vec<Waker>) {
        let ready = self.calls.len() < crate::Loopback::SLOTS;
        let Some(waiters) = self.callers.get_mut(&id) else {
            wake.push(waker.clone());
            return;
        };
        register(waiters, Interest::Slot, waker, ready, wake);
    }

    pub(crate) fn open_handler(&mut self) -> usize {
        let id = self.next_handler_id;
        self.next_handler_id += 1;
        self.handlers.insert(id, HandlerState::default());
        id
    }

    /// Removes a dropped handler. Every claim it held and had not settled
    /// returns to the waiting calls, in its place by send order, so another
    /// handler that serves the member can take it, and every handler that
    /// serves the member has its `Claim` waiter woken. The loopback enforces
    /// no deadline on the returned call: what bounds the caller's wait is the
    /// generated async client's deadline (story E11.21, ADR-0023 decision 6).
    ///
    /// Returns the handler's waiters, for the handle to drop after the lock is
    /// released, as `close_source` explains.
    pub(crate) fn close_handler(&mut self, id: usize, wake: &mut Vec<Waker>) -> Option<Waiters> {
        let waiters = self.handlers.remove(&id).map(|state| state.waiters);
        let held: Vec<u64> = self
            .claims
            .iter()
            .filter(|(_, owner)| owner.handler == id)
            .map(|(claim, _)| *claim)
            .collect();
        for claim in held {
            let owner = self.claims.remove(&claim).expect("listed above");
            let entry = self.entry(owner.call);
            let (sent, key) = (entry.sent, (entry.iface, entry.ord));
            let at = self
                .pending
                .partition_point(|call| self.calls[&Calls::slot(*call)].sent < sent);
            self.pending.insert(at, owner.call);
            self.wake_handlers_serving(key, wake);
        }
        waiters
    }

    /// Takes the `Claim` waker of every handler that serves `key`.
    fn wake_handlers_serving(&mut self, key: Key, wake: &mut Vec<Waker>) {
        for state in self.handlers.values_mut() {
            if state.serves(key) {
                wake.extend(state.waiters.take(Interest::Claim(key.0)));
            }
        }
    }

    /// Adds members to a handler's served set. A call already waiting that
    /// the handler now serves wakes its `Claim` waiter.
    pub(crate) fn serve(
        &mut self,
        handler: usize,
        iface: InterfaceNo,
        ords: &[Ordinal],
        wake: &mut Vec<Waker>,
    ) {
        let Some(state) = self.handlers.get_mut(&handler) else {
            return;
        };
        for ord in ords {
            if !state.served.contains(&(iface, *ord)) {
                state.served.push((iface, *ord));
            }
        }
        // Only a registered waker is worth the scan of the waiting calls: take
        // it, and put it back when no call it serves is waiting.
        let Some(waker) = state.waiters.take(Interest::Claim(iface)) else {
            return;
        };
        if self.claim_waiting(handler) {
            wake.push(waker);
        } else {
            let state = self.handlers.get_mut(&handler).expect("looked up above");
            let displaced = state.waiters.register(Interest::Claim(iface), &waker);
            debug_assert!(displaced.is_none(), "the kind was empty");
        }
    }

    /// Stores a handler's `Interest::Claim` waker, or wakes it at once when a
    /// call it serves is already waiting. The interface of the key is not
    /// kept: the handle holds one `Claim` waker, and any call it serves wakes
    /// that waker.
    pub(crate) fn wait_claim(
        &mut self,
        handler: usize,
        what: Interest,
        waker: &Waker,
        wake: &mut Vec<Waker>,
    ) {
        let ready = self.claim_waiting(handler);
        let Some(state) = self.handlers.get_mut(&handler) else {
            wake.push(waker.clone());
            return;
        };
        register(&mut state.waiters, what, waker, ready, wake);
    }

    /// Whether a call is waiting that `next_claim` would present to this
    /// handler. The scan stops at the first such call, and it is the same scan
    /// `next_claim` makes.
    fn claim_waiting(&self, handler: usize) -> bool {
        let Some(state) = self.handlers.get(&handler) else {
            return false;
        };
        self.pending.iter().any(|c| {
            let entry = self.entry(*c);
            state.serves((entry.iface, entry.ord))
        })
    }

    /// Presents the next waiting call this handler serves: every waiting call
    /// when it has served nothing. The claim carries an identity of its own,
    /// minted here, so a `ClaimId` names a call that was actually presented.
    pub(crate) fn next_claim(
        &mut self,
        handler: usize,
        out: &mut [u8],
    ) -> Result<Option<Claim>, ReadError> {
        let Some(state) = self.handlers.get(&handler) else {
            return Ok(None);
        };
        let position = self.pending.iter().position(|c| {
            let entry = self.entry(*c);
            state.serves((entry.iface, entry.ord))
        });
        let Some(position) = position else {
            return Ok(None);
        };
        let c = self.pending[position];
        let entry = &self.calls[&Calls::slot(c)];
        if out.len() < entry.args.len() {
            return Err(ReadError::Short {
                needed: entry.args.len(),
            });
        }
        out[..entry.args.len()].copy_from_slice(&entry.args);
        let claim_id = self.next_claim_id;
        self.next_claim_id += 1;
        let claim = Claim {
            id: ClaimId(claim_id),
            iface: entry.iface,
            ord: entry.ord,
            envelope: entry.envelope,
            // No response bound: a bound is a member's timing annotation, and
            // the loopback has no member table to read one from.
            remaining: None,
            len: entry.args.len(),
        };
        self.pending.remove(position);
        self.claims
            .insert(claim_id, ClaimOwner { call: c, handler });
        Ok(Some(claim))
    }

    /// Records a claim's outcome.
    ///
    /// The claim is looked up before the injected failure is consumed, so
    /// `fail_next_settle` fails the next settlement of a claim that exists
    /// rather than being spent on one that does not. An injected failure
    /// leaves the claim settleable, which is what makes one failed settlement
    /// followed by a successful one expressible.
    ///
    /// A recorded outcome wakes the call's `Outcome` waiter. The settlement of
    /// a call the caller forgot records nothing and reclaims its slot.
    pub(crate) fn settle(
        &mut self,
        handler: usize,
        claim: ClaimId,
        outcome: Result<&[u8], CallError>,
        wake: &mut Vec<Waker>,
    ) -> Result<(), SettleError> {
        let c = match self.claims.get(&claim.0) {
            // A claim another handler holds is unknown to this one: two
            // providers in one process settle their own calls and not each
            // other's.
            Some(owner) if owner.handler == handler => owner.call,
            _ => return Err(SettleError::UnknownClaim),
        };
        if self.fail_next_settle {
            self.fail_next_settle = false;
            return Err(SettleError::TooLarge { cap: 0 });
        }
        self.claims.remove(&claim.0);
        match self.table.settle(c, outcome.map(|_| ())) {
            Settled::Recorded(waker) => {
                if let Ok(bytes) = outcome {
                    self.calls
                        .get_mut(&Calls::slot(c))
                        .expect("a claim names a call the table holds")
                        .reply = bytes.to_vec();
                }
                wake.extend(waker);
            }
            // The caller released it while it was in flight. It was still
            // presented and is still settled; nothing can read the outcome, so
            // the slot goes with the settlement.
            Settled::Reclaimed => self.reclaimed(c, wake),
            Settled::Unknown => unreachable!("a claim names a call in flight"),
        }
        Ok(())
    }

    pub(crate) fn fail_next_settle(&mut self) {
        self.fail_next_settle = true;
    }
}

/// Stores `waker` as the handle's one waker of `what`'s kind, or, when
/// `ready`, puts it on `wake` at once and leaves that kind empty.
///
/// `Waiters::register` applies the rules of ADR-0021 decision 13: a stored
/// waker of another task is displaced and put on `wake`, so that task does
/// not wait on a registration that can no longer fire, and a stored waker
/// that wakes the same task as `waker` ([`Waker::will_wake`]) is refreshed
/// and dropped without being woken, because a task registers on every poll
/// and waking it for its own registration would schedule the next poll from
/// every poll.
fn register(
    waiters: &mut Waiters,
    what: Interest,
    waker: &Waker,
    ready: bool,
    wake: &mut Vec<Waker>,
) {
    wake.extend(waiters.register(what, waker));
    if ready {
        wake.extend(waiters.take(what));
    }
}

/// The sample a channel with no publication answers with: the consumer's
/// binding supplies the init value, so the port reports `Init` and copies
/// nothing (ridl §4.4).
fn unpublished() -> RawSample {
    RawSample {
        provenance: Provenance::Init,
        freshness: Freshness::Unbounded,
        envelope: Envelope {
            stamp: Timestamp(0),
            seq: 0,
        },
        len: 0,
    }
}

fn sample_of(entry: &SignalEntry) -> RawSample {
    RawSample {
        provenance: if entry.invalid {
            Provenance::Invalid(Cause::Declared)
        } else {
            Provenance::Live
        },
        // Every value is `Unbounded`: freshness is measured against a member's
        // staleness bound, and the loopback has no member table to read one
        // from.
        freshness: Freshness::Unbounded,
        envelope: entry.envelope,
        len: entry.bytes.len(),
    }
}
