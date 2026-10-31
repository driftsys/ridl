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
//! holds each call's outcome status, its `Outcome` waker and its `Stream`
//! wakers; the call's arguments, its place in send order, its reply bytes and
//! its streams, with each stream's `ClaimStream` waker, are kept here by slot,
//! beside it. Each source, caller and handler keeps its `Event`, `Slot`
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
//!
//! The member limits live here too, because every handle of the runtime
//! applies the same limit to one member. A limit given through
//! [`Loopback::set_limit`](crate::Loopback::set_limit) is kept by member;
//! a member with none has the default ceiling of its kind, the kind being
//! the port's (ADR-0030 decision 16). Each sending operation compares the
//! payload's length with the member's limit and refuses a longer one before
//! it changes anything.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::task::Waker;

use ridl_rt::contract::{default_ceiling, InterfaceNo, Kind, Ordinal, StreamNo};
use ridl_rt::correlate::{Forgotten, Settled, Table, Waiters};
use ridl_rt::error::{CallError, Contract, Transport};
use ridl_rt::port::{
    Changed, Claim, ClaimId, Correlation, Interest, Pull, PushError, RaiseError, RawOccurrence,
    RawSample, ReadError, SendError, SettleError, Watermark, WriteError, REPLY_STREAM,
};
use ridl_rt::sample::{Cause, Envelope, Freshness, Provenance, Timestamp};
use ridl_rt::trace::TraceContext;

/// One interaction, addressed the way every port method addresses one.
pub(crate) type Key = (InterfaceNo, Ordinal);

/// The number of slots of the call table. `Loopback::SLOTS` is this value,
/// and its documentation says why sixteen.
pub(crate) const SLOTS: usize = 16;

/// The caller-side call table: sixteen slots and no byte budget.
type Calls = Table<SLOTS>;

/// The number of non-empty chunks one stream holds before a push answers 0.
/// `Loopback::STREAM_WINDOW` is this value. An empty chunk is always taken,
/// whatever the window, so a producer that pushes only empty chunks grows the
/// queue without bound; that follows from the port contract's rule that an
/// empty chunk is taken whatever the window, and is accepted.
pub(crate) const STREAM_WINDOW: usize = 4;

/// The outcome a forget records on every stream of the call.
const INTERRUPTED: CallError = CallError::Transport(Transport::Interrupted);

/// One side of a call's streams: the caller, which holds the correlation, or
/// the provider, which holds the claim.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    Caller,
    Provider,
}

impl Side {
    /// The side that pushes to stream `s`: the provider for the reply stream,
    /// and the caller for a stream parameter.
    fn producer(s: StreamNo) -> Side {
        if s == REPLY_STREAM {
            Side::Provider
        } else {
            Side::Caller
        }
    }

    fn other(self) -> Side {
        match self {
            Side::Caller => Side::Provider,
            Side::Provider => Side::Caller,
        }
    }
}

/// One stream of one call: the chunks pushed and not yet pulled, and how the
/// stream stopped, if it did.
///
/// The caller's `Interest::Stream` waker of the stream is kept in the call
/// table, with the call; the provider's `Interest::ClaimStream` waker is kept
/// here.
#[derive(Default)]
struct StreamState {
    chunks: VecDeque<Vec<u8>>,
    /// `true` once the producer ended the stream.
    ended: bool,
    /// The outcome the stream stopped with and the side that stopped it: an
    /// abort, a settlement with an error of the reply stream, or a forget.
    stopped: Option<(CallError, Side)>,
    /// The provider's `Interest::ClaimStream` waker.
    claim_waker: Option<Waker>,
    /// `true` once the provider pulled `End` or `Failed` from the stream.
    drained: bool,
}

impl StreamState {
    /// Whether the window has no room for another non-empty chunk. The scan
    /// is over the whole queue, under the store's lock: the queue holds at
    /// most `STREAM_WINDOW` non-empty chunks and the empty chunks pushed
    /// between them.
    fn full(&self) -> bool {
        self.chunks.iter().filter(|chunk| !chunk.is_empty()).count() >= STREAM_WINDOW
    }

    /// What a push, an end or an abort of `side` answers when the stream no
    /// longer takes one: `Failed` with the outcome the other side stopped it
    /// with, and `Ended` after its end or this side's own stop.
    fn closed(&self, side: Side) -> Option<PushError> {
        match self.stopped {
            Some((error, by)) if by != side => Some(PushError::Failed(error)),
            Some(_) => Some(PushError::Ended),
            None => self.ended.then_some(PushError::Ended),
        }
    }

    /// Whether the stream ended or stopped.
    fn finished(&self) -> bool {
        self.ended || self.stopped.is_some()
    }

    /// Drops the unread chunks of a stream parameter the provider is done
    /// with, stops it with `Transport::Interrupted` by the provider's side
    /// unless it already finished, and counts it as read.
    fn release(&mut self) {
        self.chunks.clear();
        if !self.finished() {
            self.stopped = Some((INTERRUPTED, Side::Provider));
        }
        self.drained = true;
    }
}

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
    /// The trace context the event was raised with.
    trace: Option<TraceContext>,
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

/// A presented claim: the call it presented, the handler holding it, and
/// whether the handler took it. A taken call has left the waiting calls; an
/// offered one is still among them, because `next_claim` offered it through
/// `ReadError::ShortClaim` and has not yet copied its arguments
/// (driftsys/ridl#569).
struct ClaimOwner {
    call: Correlation,
    handler: usize,
    taken: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum CallKind {
    Command,
    Query,
}

impl CallKind {
    /// The interaction kind of the call, which names its default ceiling.
    fn kind(self) -> Kind {
        match self {
            CallKind::Command => Kind::Command,
            CallKind::Query => Kind::Query,
        }
    }
}

/// What the loopback keeps for one sent call, by its slot in the table, from
/// the caller's send until the table reclaims the slot. The outcome status,
/// whether the call was forgotten, and its `Interest::Outcome` waker are the
/// table's.
///
/// A forgotten call that a handler holds keeps its entry until its
/// settlement reclaims the slot, because a claim already presented is the
/// provider's and is still settled. A forgotten call that no handler holds is
/// withdrawn: its entry goes with its slot, at the `forget` for a waiting
/// call and at the handler's drop for a claimed one.
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
    /// The trace context the call was sent with.
    trace: Option<TraceContext>,
    /// The call's place in send order, which a returned claim goes back in
    /// by. A correlation does not give that order once a slot is reused.
    sent: u64,
    /// The bytes of a successful settlement. Empty until then.
    reply: Vec<u8>,
    /// `true` once the caller forgot the call while a handler held it. The
    /// table keeps the same mark, but has no query for it; a dropped
    /// handler's claim is withdrawn rather than returned by this one.
    forgotten: bool,
    /// The claim id minted when the call was first presented, through
    /// `ReadError::ShortClaim` or through `Ok(Some)`, and reused by every
    /// later presentation of the same call, so a provider that received
    /// `ShortClaim` reads or settles the call under the id it was given.
    /// `None` until then.
    claim: Option<u64>,
    /// The handler that settled the claim. A settled claim's streams go on
    /// until the call is forgotten — a command is settled at its
    /// acknowledgment — so this handler still addresses them by the claim id.
    settled_by: Option<usize>,
    /// The call's streams, by stream number. A stream has an entry from the
    /// first operation that names it.
    streams: BTreeMap<u8, StreamState>,
    /// `true` while the caller's `forget` waits for the provider: the call
    /// is forgotten, but the table is not told until the provider has pulled
    /// every stream parameter to its end or its handler is dropped, so the
    /// slot is not reclaimed before then. `forgotten` is set as well.
    pending_forget: bool,
    /// `true` once a query was settled, or a command settled with an error:
    /// its stream parameters are released.
    parameters_released: bool,
    /// `true` once the provider moved one of the call's streams: it pulled a
    /// chunk, an end or a failure, pushed or ended a stream, or aborted one.
    /// A dropped handler's call with this mark is interrupted rather than
    /// presented again, because another handler would read partial streams.
    /// A pull that answers `Pending` and a registration move nothing.
    provider_used_streams: bool,
}

impl CallEntry {
    /// Releases the stream parameters of a query at its settlement, and of a
    /// command at a settlement with an error, which refuses the whole call:
    /// the provider is done with them, so their unread chunks are dropped and
    /// every unfinished one stops with `Transport::Interrupted`, by the
    /// provider's side. A stream parameter first named afterwards starts in
    /// the same state.
    ///
    /// The provider's `ClaimStream` waker of each released stream is put on
    /// `wake`, because the stream will not change again.
    fn release_parameters(&mut self, wake: &mut Vec<Waker>) {
        self.parameters_released = true;
        for (no, stream) in self.streams.iter_mut() {
            if *no != REPLY_STREAM.0 {
                stream.release();
                wake.extend(stream.claim_waker.take());
            }
        }
    }

    /// Whether the provider still has a stream parameter to pull to its end.
    fn undrained(&self) -> bool {
        self.streams
            .iter()
            .any(|(no, stream)| *no != REPLY_STREAM.0 && !stream.drained)
    }

    /// Stream `s` of the call, made empty when nothing has named it yet. A
    /// stream first named after the caller forgot the call starts stopped
    /// with `Transport::Interrupted`, as the forget stopped every stream the
    /// call had.
    fn stream(&mut self, s: StreamNo) -> &mut StreamState {
        let forgotten = self.forgotten;
        let released = self.parameters_released && s != REPLY_STREAM;
        self.streams.entry(s.0).or_insert_with(|| {
            let mut stream = StreamState {
                stopped: forgotten.then_some((INTERRUPTED, Side::Caller)),
                ..StreamState::default()
            };
            if released {
                stream.release();
            }
            stream
        })
    }
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
    /// The limit of each member `set_limit` was given, in bytes. A member
    /// with no entry has the default ceiling of its kind.
    limits: BTreeMap<Key, usize>,
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
    /// handler holds. A call offered through `ReadError::ShortClaim` is here
    /// and still among the waiting calls; a taken call is here alone.
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
            limits: BTreeMap::new(),
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

    // -- the member limits -------------------------------------------------

    pub(crate) fn set_limit(&mut self, key: Key, limit: usize) {
        self.limits.insert(key, limit);
    }

    /// The limit of member `key` whose port is of `kind`: the one `set_limit`
    /// gave it, else the default ceiling of `kind`.
    pub(crate) fn limit(&self, key: Key, kind: Kind) -> usize {
        self.limits
            .get(&key)
            .copied()
            .unwrap_or(default_ceiling(kind))
    }

    /// `Ok` when a payload of `len` bytes is within the limit of member
    /// `key`, else `Err` with that limit.
    pub(crate) fn admit(&self, key: Key, kind: Kind, len: usize) -> Result<(), usize> {
        let cap = self.limit(key, kind);
        if len > cap {
            Err(cap)
        } else {
            Ok(())
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
            let mut changes = self
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
            if changes.clone().count() > out.len() - written {
                return written;
            }
            for change in changes.by_ref() {
                out[written] = change;
                written += 1;
            }
            mark.generation = current;
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

    /// Stores the value of a `fixed`, or refuses one longer than the member's
    /// limit and stores nothing: a value stored before stays as it was.
    pub(crate) fn provision_fixed(
        &mut self,
        iface: InterfaceNo,
        ord: Ordinal,
        bytes: &[u8],
    ) -> Result<(), WriteError> {
        self.admit((iface, ord), Kind::Fixed, bytes.len())
            .map_err(|cap| WriteError::TooLarge { cap })?;
        self.fixed.insert((iface, ord), bytes.to_vec());
        Ok(())
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
    ///
    /// An occurrence longer than the event's limit is refused with that
    /// limit and queued for no source; `seq` is read only when the
    /// occurrence is admitted, so a refusal draws no sequence number.
    pub(crate) fn raise(
        &mut self,
        iface: InterfaceNo,
        ord: Ordinal,
        bytes: &[u8],
        seq: impl FnOnce() -> u64,
        trace: Option<TraceContext>,
        wake: &mut Vec<Waker>,
    ) -> Result<(), RaiseError> {
        self.admit((iface, ord), Kind::Event, bytes.len())
            .map_err(|cap| RaiseError::TooLarge { cap })?;
        let seq = seq();
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
                    trace,
                });
                wake.extend(state.waiters.take(Interest::Event(iface)));
            }
        }
        Ok(())
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
            trace: event.trace,
            len: event.bytes.len(),
        }))
    }

    // -- calls -------------------------------------------------------------

    /// Takes a slot in the call table and queues the call for presentation,
    /// or answers `SendError::Busy` when every slot is taken. Every handler
    /// that serves the member has its `Claim` waiter woken, whatever
    /// interface that waiter was registered under.
    ///
    /// Arguments longer than the call's limit are refused with that limit
    /// before a slot is taken, so nothing is sent and nothing is woken.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn send(
        &mut self,
        caller: usize,
        kind: CallKind,
        (iface, ord): Key,
        args: &[u8],
        seq: u64,
        trace: Option<TraceContext>,
        wake: &mut Vec<Waker>,
    ) -> Result<Correlation, SendError> {
        self.admit((iface, ord), kind.kind(), args.len())
            .map_err(|cap| SendError::TooLarge { cap })?;
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
                trace,
                sent,
                reply: Vec::new(),
                forgotten: false,
                claim: None,
                settled_by: None,
                streams: BTreeMap::new(),
                pending_forget: false,
                parameters_released: false,
                provider_used_streams: false,
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
        if self.forgotten_pending(c) {
            wake.push(waker.clone());
            return;
        }
        wake.extend(self.table.wake_on(c, waker));
    }

    /// Whether `c` is a call whose `forget` waits for the provider: the
    /// table still holds it, but the caller released it.
    fn forgotten_pending(&self, c: Correlation) -> bool {
        self.calls
            .get(&Calls::slot(c))
            .is_some_and(|entry| entry.correlation == c && entry.pending_forget)
    }

    pub(crate) fn ack(&self, c: Correlation) -> Option<Result<(), CallError>> {
        // The table answers `None` for a call in flight, forgotten, or not
        // held; a correlation it answers for has its entry here.
        let outcome = self.table.outcome(c)?;
        if self.forgotten_pending(c) {
            return None;
        }
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
        if self.forgotten_pending(c) {
            return Ok(None);
        }
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
    /// operation that frees a slot. What it does depends on where the call
    /// is:
    ///
    /// - **Settled**: nothing is left to happen to it, so its slot is
    ///   reclaimed now.
    /// - **Claimed by a handler and not settled**: it is marked forgotten and
    ///   keeps its slot. The provider still settles it — `Handler`'s contract
    ///   is that every claim is settled, and a caller losing interest is not
    ///   the provider's business — and the slot is reclaimed when that
    ///   settlement lands (ADR-0021 decision 15). If the handler is dropped
    ///   first, [`Store::close_handler`] withdraws the call instead of
    ///   returning it to the waiting calls.
    /// - **Waiting, claimed by no handler**: it is withdrawn. It leaves the
    ///   waiting calls, so no handler is ever presented it, and its slot is
    ///   reclaimed now. A call to a member no handler serves is never
    ///   settled, so without this its slot would be lost for the life of the
    ///   runtime. This is this runtime's behaviour, not a port contract: a
    ///   transport that has already sent a request cannot recall it (decision
    ///   1 of the pass-1 dispositions on driftsys/ridl#553).
    /// - **Waiting, offered to a handler through `ReadError::ShortClaim`**:
    ///   it is a handler's claim although it is still waiting, so it is
    ///   marked forgotten as a taken claim is, and it stays among the waiting
    ///   calls, so the provider's retry with a larger buffer still presents
    ///   it under the same id. Its settlement by the handler that holds it,
    ///   or that handler's drop, reclaims the slot (driftsys/ridl#569).
    ///
    /// `forget` and the handler side — `serve` and `next_claim` — run under
    /// the store's one lock, so a call is either claimed first, and held
    /// until settled, or withdrawn first, and never presented. Either way
    /// the correlation answers `None` from `ack` and `reply` afterwards. A
    /// waiter on the outcome of a call still in flight is woken: no outcome
    /// will be readable for it, so it would otherwise never be. A withdrawal
    /// wakes every caller's `Slot` waiter, as any reclaim does, and no
    /// handler's `Claim` waiter, because it adds no call to claim.
    pub(crate) fn forget(&mut self, c: Correlation, wake: &mut Vec<Waker>) {
        if self.forgotten_pending(c) {
            return;
        }
        self.interrupt_streams(c, wake);
        let waiting_at = if self.offered(c) {
            None
        } else {
            self.pending.iter().position(|waiting| *waiting == c)
        };
        if let Some(at) = waiting_at {
            self.pending.remove(at);
            self.withdraw(c, wake);
            return;
        }
        if self.defer_forget(c, wake) {
            return;
        }
        self.forget_in_table(c, wake);
    }

    /// Defers the `forget` of a call that a handler holds or settled while
    /// the provider has a stream parameter to pull to its end: the call is
    /// marked forgotten here and not in the table, so its slot stays taken.
    /// Its `Outcome` waiter and its `Stream` waiters are woken now, as a
    /// forget wakes them. Answers whether it deferred.
    fn defer_forget(&mut self, c: Correlation, wake: &mut Vec<Waker>) -> bool {
        let slot = Calls::slot(c);
        let Some(entry) = self.calls.get(&slot) else {
            return false;
        };
        let held = entry
            .claim
            .is_some_and(|claim| self.claims.contains_key(&claim))
            || entry.settled_by.is_some();
        if entry.correlation != c || entry.forgotten || !held || !entry.undrained() {
            return false;
        }
        let entry = self.calls.get_mut(&slot).expect("looked up above");
        entry.forgotten = true;
        entry.pending_forget = true;
        // The table hands back the stored `Outcome` waker when another is
        // registered in its place; the no-op waker left behind wakes nothing.
        wake.extend(self.table.wake_on(c, Waker::noop()));
        wake.extend(self.table.take_streams(slot));
        true
    }

    /// Completes a deferred `forget` once the provider has pulled every
    /// stream parameter to its end.
    fn release_if_drained(&mut self, slot: usize, wake: &mut Vec<Waker>) {
        let Some(entry) = self.calls.get_mut(&slot) else {
            return;
        };
        if !entry.pending_forget || entry.undrained() {
            return;
        }
        entry.pending_forget = false;
        let c = entry.correlation;
        self.forget_in_table(c, wake);
    }

    /// The table's part of a `forget`: a settled call's slot is reclaimed,
    /// and a call in flight is marked, so that its settlement reclaims it.
    fn forget_in_table(&mut self, c: Correlation, wake: &mut Vec<Waker>) {
        match self.table.forget(c) {
            Forgotten::Reclaimed => self.reclaimed(c, wake),
            Forgotten::Marked(waker) => {
                wake.extend(waker);
                wake.extend(self.table.take_streams(Calls::slot(c)));
                self.calls
                    .get_mut(&Calls::slot(c))
                    .expect("a call in flight has its entry")
                    .forgotten = true;
            }
            Forgotten::Unknown => {}
        }
    }

    /// Withdraws a call that no handler holds and that is not among the
    /// waiting calls: the table settles it and, when the caller has not
    /// forgotten it yet, forgets it, which reclaims its slot either way. The
    /// table calls are made under the one lock the caller of this function
    /// holds, so no reader sees the settlement in between.
    ///
    /// Two paths reach it: `forget` of a waiting call, which it has just
    /// taken out of the queue, and `close_handler` for a claim whose call
    /// the caller forgot while the dropped handler held it.
    fn withdraw(&mut self, c: Correlation, wake: &mut Vec<Waker>) {
        // The table discards this outcome with the slot, so no reader ever
        // sees it. `Undelivered` is chosen because it is what happened — no
        // provider settled the call — so the value does not read as an
        // acknowledgment.
        match self
            .table
            .settle(c, Err(CallError::Transport(Transport::Undelivered)))
        {
            // A waiting call, which the caller forgets now.
            Settled::Recorded(waker) => {
                wake.extend(waker);
                match self.table.forget(c) {
                    Forgotten::Reclaimed => self.reclaimed(c, wake),
                    Forgotten::Marked(_) | Forgotten::Unknown => {
                        unreachable!("the call was settled just above")
                    }
                }
            }
            // A call the caller forgot while a handler held it, withdrawn at
            // that handler's drop: the table marked it, so the settlement
            // reclaims the slot.
            Settled::Reclaimed => self.reclaimed(c, wake),
            Settled::Unknown => unreachable!("a withdrawn call is in flight"),
        }
    }

    /// Drops what the loopback kept for a reclaimed slot, and takes every
    /// caller's `Slot` waker: the first to send again takes the slot, and the
    /// others find the table full and register again (note F-5, no queue).
    ///
    /// Every stream waker of the call is taken as well: the caller's, from
    /// the table, and the provider's, from the entry, so that none of them is
    /// dropped under the lock and no task waits on a stream that is gone.
    fn reclaimed(&mut self, c: Correlation, wake: &mut Vec<Waker>) {
        let slot = Calls::slot(c);
        wake.extend(self.table.take_streams(slot));
        if let Some(entry) = self.calls.remove(&slot) {
            for stream in entry.streams.into_values() {
                wake.extend(stream.claim_waker);
            }
        }
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
    /// forget, under the rule of [`Store::forget`]: a settled one's slot is
    /// reclaimed now, one no handler has claimed is withdrawn from the
    /// waiting calls and its slot reclaimed now, and one a handler has
    /// claimed is marked, so its settlement reclaims the slot. No handle can
    /// read the outcome of a call whose caller is gone, and a slot kept for
    /// it would be lost to every other caller. One exception holds the slot
    /// longer: a call whose provider still has a stream parameter to read
    /// keeps its slot until the provider is done with it or its handler is
    /// dropped (`Store::forget`, the deferred case), so a settled command
    /// whose stream parameter no provider drains holds its slot for as long
    /// as the handler that acknowledged it lives.
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
        let ready = self.calls.len() < SLOTS;
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

    /// Removes a dropped handler. Every claim it had taken and not settled
    /// returns to the waiting calls, in its place by send order and under the
    /// id it was first presented with, so another handler that serves the
    /// member can take it, and every handler that serves the member has its
    /// `Claim` waiter woken, unless the handler already moved one of the
    /// call's streams: such a call is interrupted instead
    /// (`Store::interrupt_claim`). A claim it had only
    /// been offered, through
    /// `ReadError::ShortClaim`, never left the waiting calls, so it is neither
    /// re-inserted nor woken for; its id stays on the entry for the next
    /// presentation (driftsys/ridl#569). A claim whose call the caller forgot
    /// is withdrawn instead, taken or offered, as [`Store::forget`] withdraws
    /// a waiting call: its slot is reclaimed now, no handler is presented it
    /// again, and no `Claim` waiter is woken for it (decision 2 of the pass-1
    /// dispositions on driftsys/ridl#557). The loopback enforces
    /// no deadline on the returned call: what bounds the caller's wait is the
    /// generated async client's deadline (ADR-0023 decision 6).
    ///
    /// Returns the handler's waiters, for the handle to drop after the lock is
    /// released, as `close_source` explains.
    pub(crate) fn close_handler(&mut self, id: usize, wake: &mut Vec<Waker>) -> Option<Waiters> {
        let waiters = self.handlers.remove(&id).map(|state| state.waiters);
        let mut released = Vec::new();
        // The dropped handler's `ClaimStream` wakers, of the claims it holds
        // and of those it settled, are taken so that none is dropped under
        // the lock; a settled claim's streams are no longer addressable.
        for (slot, entry) in self.calls.iter_mut() {
            let holds = entry
                .claim
                .and_then(|claim| self.claims.get(&claim))
                .is_some_and(|owner| owner.handler == id);
            if holds || entry.settled_by == Some(id) {
                if entry.settled_by == Some(id) {
                    entry.settled_by = None;
                    // No provider is left to read the stream parameters of
                    // the settled claim: they are released, as a query's
                    // settlement releases them, so every unfinished one
                    // stops, one first named afterwards starts stopped, and
                    // the caller's waiters on them are woken. The reply
                    // stream of a settled claim already ended or stopped at
                    // the settlement.
                    entry.release_parameters(wake);
                    wake.extend(self.table.take_streams(*slot));
                    if entry.pending_forget {
                        entry.pending_forget = false;
                        released.push(entry.correlation);
                    }
                }
                for stream in entry.streams.values_mut() {
                    wake.extend(stream.claim_waker.take());
                }
            }
        }
        for c in released {
            self.forget_in_table(c, wake);
        }
        let held: Vec<u64> = self
            .claims
            .iter()
            .filter(|(_, owner)| owner.handler == id)
            .map(|(claim, _)| *claim)
            .collect();
        for claim in held {
            let owner = self.claims.remove(&claim).expect("listed above");
            let entry = self.entry(owner.call);
            if entry.forgotten {
                // A forgotten offered call is still among the waiting calls;
                // `withdraw` expects a call that is not.
                let waiting_at = if owner.taken {
                    None
                } else {
                    self.pending.iter().position(|c| *c == owner.call)
                };
                if let Some(at) = waiting_at {
                    self.pending.remove(at);
                }
                self.withdraw(owner.call, wake);
                continue;
            }
            if !owner.taken {
                continue;
            }
            if entry.provider_used_streams {
                // This handler already moved the call's streams, so another
                // handler cannot take the call over without reading partial
                // streams. Every unfinished stream
                // stops with `Transport::Interrupted`, and the call settles
                // with that error instead of returning to the waiting calls.
                self.interrupt_claim(owner.call, wake);
                continue;
            }
            let (sent, key) = (entry.sent, (entry.iface, entry.ord));
            let at = self
                .pending
                .partition_point(|call| self.calls[&Calls::slot(*call)].sent < sent);
            self.pending.insert(at, owner.call);
            self.wake_handlers_serving(key, wake);
        }
        waiters
    }

    /// Ends a taken call whose handler was dropped after its streams were
    /// used: every unfinished stream stops with `Transport::Interrupted`, by
    /// the provider's side, and the call settles `Err` with the same error,
    /// which wakes its `Outcome` waiter and its `Stream` waiters.
    fn interrupt_claim(&mut self, c: Correlation, wake: &mut Vec<Waker>) {
        let entry = self
            .calls
            .get_mut(&Calls::slot(c))
            .expect("a held claim names a call the table holds");
        // The settlement with an error releases the stream parameters, as
        // any settlement with an error does: every unfinished one stops, and
        // one first named afterwards starts stopped.
        entry.release_parameters(wake);
        // It stops the reply stream too, whether or not anything named it
        // yet.
        let reply = entry.stream(REPLY_STREAM);
        if !reply.finished() {
            reply.stopped = Some((INTERRUPTED, Side::Provider));
        }
        wake.extend(reply.claim_waker.take());
        match self.table.settle(c, Err(INTERRUPTED)) {
            Settled::Recorded(waker) => {
                wake.extend(waker);
                wake.extend(self.table.take_streams(Calls::slot(c)));
            }
            Settled::Reclaimed => self.reclaimed(c, wake),
            Settled::Unknown => unreachable!("a held claim names a call in flight"),
        }
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
    ///
    /// A call whose arguments do not fit `out` is offered rather than taken
    /// (driftsys/ridl#569): the id is minted, the claim is recorded as this
    /// handler's, the call stays among the waiting calls, and the answer is
    /// `ReadError::ShortClaim`. A later `next_claim`, by this handler or by
    /// another that serves the member, presents the same call under the same
    /// id, because the id lives on the call's entry; a take by another handler
    /// moves the claim to that handler. `settle` accepts the id whether or not
    /// the call was read.
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
        let claim_id = match self.calls[&Calls::slot(c)].claim {
            Some(id) => id,
            None => {
                let id = self.next_claim_id;
                self.next_claim_id += 1;
                self.calls
                    .get_mut(&Calls::slot(c))
                    .expect("a waiting call has its entry")
                    .claim = Some(id);
                id
            }
        };
        let entry = &self.calls[&Calls::slot(c)];
        if out.len() < entry.args.len() {
            let needed = entry.args.len();
            self.claims.insert(
                claim_id,
                ClaimOwner {
                    call: c,
                    handler,
                    taken: false,
                },
            );
            return Err(ReadError::ShortClaim {
                claim: ClaimId(claim_id),
                needed,
                trace: entry.trace,
            });
        }
        out[..entry.args.len()].copy_from_slice(&entry.args);
        let claim = Claim {
            id: ClaimId(claim_id),
            iface: entry.iface,
            ord: entry.ord,
            envelope: entry.envelope,
            trace: entry.trace,
            // No response bound: a bound is a member's timing annotation, and
            // the loopback has no member table to read one from.
            remaining: None,
            len: entry.args.len(),
        };
        self.pending.remove(position);
        self.claims.insert(
            claim_id,
            ClaimOwner {
                call: c,
                handler,
                taken: true,
            },
        );
        Ok(Some(claim))
    }

    /// Records a claim's outcome.
    ///
    /// A reply longer than the call's limit is refused with that limit before
    /// the injected failure is consumed: no outcome is recorded, the claim
    /// stays settleable, and the fault is not spent on it.
    ///
    /// The claim is looked up before the injected failure is consumed, so
    /// `fail_next_settle` fails the next settlement of a claim that exists
    /// rather than being spent on one that does not. An injected failure
    /// leaves the claim settleable, which is what makes one failed settlement
    /// followed by a successful one expressible.
    ///
    /// A recorded outcome wakes the call's `Outcome` waiter, ends the reply
    /// stream (`Ok`) or stops it with the error, after the chunks already
    /// pushed, and wakes every `Stream` waiter of the call. The settlement of
    /// a call the caller forgot records nothing and reclaims its slot.
    pub(crate) fn settle(
        &mut self,
        handler: usize,
        claim: ClaimId,
        outcome: Result<&[u8], CallError>,
        wake: &mut Vec<Waker>,
    ) -> Result<(), SettleError> {
        let (c, taken) = match self.claims.get(&claim.0) {
            // A claim another handler holds is unknown to this one: two
            // providers in one process settle their own calls and not each
            // other's.
            Some(owner) if owner.handler == handler => (owner.call, owner.taken),
            _ => return Err(SettleError::UnknownClaim),
        };
        if let Ok(reply) = outcome {
            let entry = self.entry(c);
            self.admit((entry.iface, entry.ord), entry.kind.kind(), reply.len())
                .map_err(|cap| SettleError::TooLarge { cap })?;
        }
        if self.fail_next_settle {
            self.fail_next_settle = false;
            return Err(SettleError::TooLarge { cap: 0 });
        }
        self.claims.remove(&claim.0);
        // An offered call is still among the waiting calls: its settlement
        // takes it out, so no handler is presented it afterwards.
        let waiting_at = if taken {
            None
        } else {
            self.pending.iter().position(|waiting| *waiting == c)
        };
        if let Some(at) = waiting_at {
            self.pending.remove(at);
        }
        match self.table.settle(c, outcome.map(|_| ())) {
            Settled::Recorded(waker) => {
                let entry = self
                    .calls
                    .get_mut(&Calls::slot(c))
                    .expect("a claim names a call the table holds");
                if let Ok(bytes) = outcome {
                    entry.reply = bytes.to_vec();
                }
                entry.settled_by = Some(handler);
                // A query's settlement is its end, and a settlement with an
                // error refuses the whole call: either way the provider is
                // done with the stream parameters. A command's settlement
                // `Ok` is its acknowledgment, and its stream parameters go on
                // after it.
                if entry.kind == CallKind::Query || outcome.is_err() {
                    entry.release_parameters(wake);
                }
                // The settlement ends the reply stream, or stops it with the
                // error, after the chunks already pushed.
                let reply = entry.stream(REPLY_STREAM);
                if !reply.finished() {
                    match outcome {
                        Ok(_) => reply.ended = true,
                        Err(error) => reply.stopped = Some((error, Side::Provider)),
                    }
                }
                wake.extend(waker);
                wake.extend(self.table.take_streams(Calls::slot(c)));
                self.release_if_drained(Calls::slot(c), wake);
            }
            // The caller released it while it was in flight. It was still
            // presented and is still settled; nothing can read the outcome, so
            // the slot goes with the settlement.
            Settled::Reclaimed => self.reclaimed(c, wake),
            Settled::Unknown => unreachable!("a claim names a call in flight"),
        }
        Ok(())
    }

    // -- streams -----------------------------------------------------------

    /// The slot of the call the caller holds under `c`: `None` for a
    /// correlation the table does not hold, of a generation the slot no
    /// longer has, or that the caller forgot.
    pub(crate) fn caller_slot(&self, c: Correlation) -> Option<usize> {
        let slot = Calls::slot(c);
        self.calls
            .get(&slot)
            .filter(|entry| entry.correlation == c && !entry.forgotten)
            .map(|_| slot)
    }

    /// The slot of the call `handler` holds under `claim`: a claim it took
    /// and has not settled, or one it settled whose call the caller has not
    /// forgotten yet. A claim only offered through `ReadError::ShortClaim`
    /// has no streams yet: the handler takes it first, so that a call whose
    /// offer moves to another handler, or returns to the waiting calls, has
    /// no stream that one holder has partly read.
    pub(crate) fn claim_slot(&self, handler: usize, claim: ClaimId) -> Option<usize> {
        let (slot, entry) = self
            .calls
            .iter()
            .find(|(_, entry)| entry.claim == Some(claim.0))?;
        let holder = match self.claims.get(&claim.0) {
            Some(owner) if owner.taken => Some(owner.handler),
            Some(_) => None,
            None => entry.settled_by,
        };
        (holder == Some(handler)).then_some(*slot)
    }

    /// Takes the waker of `side` on stream `s` of the call in `slot`: the
    /// caller's from the call table, the provider's from the stream.
    fn take_stream_waker(&mut self, slot: usize, s: StreamNo, side: Side) -> Option<Waker> {
        let entry = self.calls.get_mut(&slot)?;
        match side {
            Side::Caller => self.table.take_stream(entry.correlation, s),
            Side::Provider => entry.streams.get_mut(&s.0)?.claim_waker.take(),
        }
    }

    /// Copies the next chunk of stream `s` of the call in `slot` into `out`,
    /// for `side`, which must be the stream's receiver. A chunk taken wakes
    /// the producer, whose window it frees.
    pub(crate) fn pull(
        &mut self,
        slot: usize,
        side: Side,
        s: StreamNo,
        out: &mut [u8],
        wake: &mut Vec<Waker>,
    ) -> Result<Pull, ReadError> {
        if Side::producer(s) == side {
            return Err(ReadError::Contract(Contract::UnknownInteraction));
        }
        let entry = self
            .calls
            .get_mut(&slot)
            .expect("a held call has its entry");
        let stream = entry.stream(s);
        let Some(front) = stream.chunks.front() else {
            let pull = if stream.ended {
                Pull::End
            } else if let Some((error, _)) = stream.stopped {
                Pull::Failed(error)
            } else {
                return Ok(Pull::Pending);
            };
            if side == Side::Provider {
                stream.drained = true;
                entry.provider_used_streams = true;
                self.release_if_drained(slot, wake);
            }
            return Ok(pull);
        };
        if out.len() < front.len() {
            return Err(ReadError::Short {
                needed: front.len(),
            });
        }
        let chunk = stream.chunks.pop_front().expect("the front was just read");
        if side == Side::Provider {
            entry.provider_used_streams = true;
        }
        out[..chunk.len()].copy_from_slice(&chunk);
        wake.extend(self.take_stream_waker(slot, s, side.other()));
        Ok(Pull::Chunk(chunk.len()))
    }

    /// Offers one chunk to stream `s` of the call in `slot`, for `side`,
    /// which must be the stream's producer. A chunk taken wakes the receiver.
    pub(crate) fn push(
        &mut self,
        slot: usize,
        side: Side,
        s: StreamNo,
        chunk: &[u8],
        wake: &mut Vec<Waker>,
    ) -> Result<usize, PushError> {
        if Side::producer(s) != side {
            return Err(PushError::Contract(Contract::UnknownInteraction));
        }
        let entry = &self.calls[&slot];
        let (key, kind) = ((entry.iface, entry.ord), entry.kind.kind());
        let entry = self.calls.get_mut(&slot).expect("looked up above");
        if side == Side::Provider && entry.forgotten {
            return Err(PushError::Failed(INTERRUPTED));
        }
        if let Some(closed) = entry.stream(s).closed(side) {
            return Err(closed);
        }
        self.admit(key, kind, chunk.len())
            .map_err(|cap| PushError::TooLarge { cap })?;
        let stream = self
            .calls
            .get_mut(&slot)
            .expect("looked up above")
            .stream(s);
        // An empty chunk is always taken: the window bounds the non-empty
        // chunks only.
        if !chunk.is_empty() && stream.full() {
            return Ok(0);
        }
        stream.chunks.push_back(chunk.to_vec());
        if side == Side::Provider {
            self.calls
                .get_mut(&slot)
                .expect("looked up above")
                .provider_used_streams = true;
        }
        wake.extend(self.take_stream_waker(slot, s, side.other()));
        Ok(chunk.len())
    }

    /// Ends stream `s` of the call in `slot`, for `side`, which must be the
    /// stream's producer. The end wakes the receiver.
    pub(crate) fn end(
        &mut self,
        slot: usize,
        side: Side,
        s: StreamNo,
        wake: &mut Vec<Waker>,
    ) -> Result<(), PushError> {
        if Side::producer(s) != side {
            return Err(PushError::Contract(Contract::UnknownInteraction));
        }
        let entry = self
            .calls
            .get_mut(&slot)
            .expect("a held call has its entry");
        if side == Side::Provider && entry.forgotten {
            return Err(PushError::Failed(INTERRUPTED));
        }
        let stream = entry.stream(s);
        if let Some(closed) = stream.closed(side) {
            return Err(closed);
        }
        stream.ended = true;
        if side == Side::Provider {
            entry.provider_used_streams = true;
        }
        wake.extend(self.take_stream_waker(slot, s, side.other()));
        Ok(())
    }

    /// Stops stream `s` of the call in `slot` with `error`, from either side.
    /// The stop wakes the other side.
    pub(crate) fn abort(
        &mut self,
        slot: usize,
        side: Side,
        s: StreamNo,
        error: CallError,
        wake: &mut Vec<Waker>,
    ) -> Result<(), PushError> {
        let entry = self
            .calls
            .get_mut(&slot)
            .expect("a held call has its entry");
        if side == Side::Provider {
            entry.provider_used_streams = true;
        }
        let stream = entry.stream(s);
        let receiver = Side::producer(s) != side;
        if stream.finished() {
            // A receiver's abort of a stream that already ended or stopped
            // changes nothing for the producer, and answers `Ended`; the
            // receiver is done with the stream all the same, so its unread
            // chunks are dropped, and a provider that refuses a stream
            // parameter this way does not hold the call's slot.
            if receiver {
                stream.chunks.clear();
                if side == Side::Provider {
                    stream.drained = true;
                    self.release_if_drained(slot, wake);
                }
            }
            return Err(PushError::Ended);
        }
        stream.stopped = Some((error, side));
        // A receiver that refuses the stream reads none of what it refused:
        // its later pulls answer `Failed` with its own outcome.
        if receiver {
            stream.chunks.clear();
        }
        // A stream the provider stopped itself needs no more reading by it.
        if side == Side::Provider {
            stream.drained = true;
        }
        wake.extend(self.take_stream_waker(slot, s, side.other()));
        if side == Side::Provider {
            self.release_if_drained(slot, wake);
        }
        Ok(())
    }

    /// `end_claim`: on the reply stream of a claim still held, the claim's
    /// settlement `Ok` with no reply bytes, which ends the stream; on any
    /// other stream, or a claim already settled, an end.
    pub(crate) fn end_claim(
        &mut self,
        handler: usize,
        claim: ClaimId,
        s: StreamNo,
        wake: &mut Vec<Waker>,
    ) -> Result<(), PushError> {
        let slot = self
            .claim_slot(handler, claim)
            .ok_or(PushError::Contract(Contract::UnknownInteraction))?;
        if s == REPLY_STREAM && self.claims.contains_key(&claim.0) {
            // The claim is settled whatever the stream's verdict, so that a
            // provider whose reply stream was stopped does not hold the
            // call's slot; the verdict is the answer.
            let entry = self.calls.get_mut(&slot).expect("a held claim's call");
            let verdict = if entry.forgotten {
                Some(PushError::Failed(INTERRUPTED))
            } else {
                entry.stream(s).closed(Side::Provider)
            };
            // After the caller refused the reply stream, the claim settles
            // with that refusal as its outcome, not `Ok`.
            let outcome = match verdict {
                Some(PushError::Failed(error)) => Err(error),
                _ => Ok(&[][..]),
            };
            self.settle(handler, claim, outcome, wake)
                .map_err(push_error)?;
            return verdict.map_or(Ok(()), Err);
        }
        self.end(slot, Side::Provider, s, wake)
    }

    /// `abort_claim`: on the reply stream of a claim still held, the claim's
    /// settlement with `error`, which stops the stream; on any other stream,
    /// or a claim already settled, an abort.
    pub(crate) fn abort_claim(
        &mut self,
        handler: usize,
        claim: ClaimId,
        s: StreamNo,
        error: CallError,
        wake: &mut Vec<Waker>,
    ) -> Result<(), PushError> {
        let slot = self
            .claim_slot(handler, claim)
            .ok_or(PushError::Contract(Contract::UnknownInteraction))?;
        if s == REPLY_STREAM && self.claims.contains_key(&claim.0) {
            // The claim is settled whatever the stream's state, as in
            // `end_claim`.
            let entry = self.calls.get_mut(&slot).expect("a held claim's call");
            let finished = entry.stream(s).finished();
            self.settle(handler, claim, Err(error), wake)
                .map_err(push_error)?;
            return if finished {
                Err(PushError::Ended)
            } else {
                Ok(())
            };
        }
        self.abort(slot, Side::Provider, s, error, wake)
    }

    /// Stores the caller's `Interest::Stream(c, s)` waker with the call, in
    /// the table, which wakes it at once for a correlation it does not hold
    /// or that the caller forgot.
    pub(crate) fn wait_stream(
        &mut self,
        c: Correlation,
        s: StreamNo,
        waker: &Waker,
        wake: &mut Vec<Waker>,
    ) {
        if self.forgotten_pending(c) {
            wake.push(waker.clone());
            return;
        }
        wake.extend(self.table.wake_on_stream(c, s, waker));
    }

    /// Stores the provider's `Interest::ClaimStream(claim, s)` waker with the
    /// stream, or wakes it at once when `handler` holds no such claim or the
    /// caller forgot the call, since the stream will not change again.
    pub(crate) fn wait_claim_stream(
        &mut self,
        handler: usize,
        claim: ClaimId,
        s: StreamNo,
        waker: &Waker,
        wake: &mut Vec<Waker>,
    ) {
        let Some(slot) = self.claim_slot(handler, claim) else {
            wake.push(waker.clone());
            return;
        };
        let entry = self.calls.get_mut(&slot).expect("a held claim's call");
        if entry.forgotten {
            wake.push(waker.clone());
            return;
        }
        let stream = entry.stream(s);
        // A stream that ended, stopped or was released has nothing more to
        // wait for: the provider's next operation on it answers at once.
        if stream.finished() {
            wake.push(waker.clone());
            return;
        }
        let stored = &mut stream.claim_waker;
        wake.extend(
            stored
                .replace(waker.clone())
                .filter(|displaced| !displaced.will_wake(waker)),
        );
    }

    /// Stops every stream of the call the caller holds under `c` with
    /// `Transport::Interrupted`, except one already ended or stopped, and
    /// takes the provider's waker of each stream. A correlation the caller
    /// does not hold changes nothing, so a stale one does not touch the call
    /// now in its slot.
    fn interrupt_streams(&mut self, c: Correlation, wake: &mut Vec<Waker>) {
        let Some(slot) = self.caller_slot(c) else {
            return;
        };
        let entry = self.calls.get_mut(&slot).expect("looked up above");
        for stream in entry.streams.values_mut() {
            if !stream.finished() {
                stream.stopped = Some((INTERRUPTED, Side::Caller));
            }
            wake.extend(stream.claim_waker.take());
        }
    }

    pub(crate) fn fail_next_settle(&mut self) {
        self.fail_next_settle = true;
    }

    /// Whether a handler holds a claim on `c` that it has not yet taken: the
    /// call was offered through `ReadError::ShortClaim`.
    fn offered(&self, c: Correlation) -> bool {
        self.calls
            .get(&Calls::slot(c))
            .and_then(|entry| entry.claim)
            .and_then(|id| self.claims.get(&id))
            .is_some_and(|owner| !owner.taken)
    }
}

/// The push error a settlement through `end_claim` or `abort_claim` answers
/// with when the settlement is refused.
fn push_error(error: SettleError) -> PushError {
    match error {
        SettleError::UnknownClaim => PushError::Contract(Contract::UnknownInteraction),
        SettleError::TooLarge { cap } => PushError::TooLarge { cap },
        _ => PushError::Detached,
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
