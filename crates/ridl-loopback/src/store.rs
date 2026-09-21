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
//! call table and the clock are here; a writer's staged changes and its per
//! channel sequence counters, a source's identity, and a handler's served set
//! are on the handle.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use ridl_rt::contract::{InterfaceNo, Ordinal};
use ridl_rt::error::CallError;
use ridl_rt::port::{
    Changed, Claim, ClaimId, Correlation, RawOccurrence, RawSample, ReadError, SettleError,
    Watermark,
};
use ridl_rt::sample::{Cause, Envelope, Freshness, Provenance, Timestamp};

/// One interaction, addressed the way every port method addresses one.
pub(crate) type Key = (InterfaceNo, Ordinal);

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

/// One source handle's subscription set and its own queue. `raise` copies an
/// occurrence into the queue of every source subscribed to it at that moment,
/// so each source reads its own occurrences in order and no source consumes
/// another's.
#[derive(Default)]
struct SourceState {
    subscribed: BTreeSet<Key>,
    queue: VecDeque<QueuedEvent>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum CallKind {
    Command,
    Query,
}

/// One sent call, from the caller's send to the provider's settlement.
struct CallEntry {
    kind: CallKind,
    iface: InterfaceNo,
    ord: Ordinal,
    args: Vec<u8>,
    envelope: Envelope,
    /// `None` while unsettled, which is what `Caller::ack` and `Caller::reply`
    /// report as `None`.
    outcome: Option<Result<Vec<u8>, CallError>>,
    /// `true` once the caller has released the correlation. The entry stays,
    /// because the provider's side of the call is not the caller's to revoke:
    /// a claim already presented is still settled, and a call still waiting is
    /// still presented. What changes is that `ack` and `reply` answer as they
    /// do for a call they never heard of.
    forgotten: bool,
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
    calls: BTreeMap<u64, CallEntry>,
    pending: VecDeque<u64>,
    next_call_id: u64,
    /// The calls presented and not yet settled: a claim identity of its own,
    /// minted by `next_claim`, to the call it presented. A `ClaimId` is
    /// therefore never a correlation that was never presented, and never one
    /// already settled.
    claims: BTreeMap<u64, u64>,
    next_claim_id: u64,
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
            calls: BTreeMap::new(),
            pending: VecDeque::new(),
            next_call_id: 0,
            claims: BTreeMap::new(),
            next_claim_id: 0,
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

    pub(crate) fn close_source(&mut self, id: usize) {
        self.sources.remove(&id);
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
        let SourceState { subscribed, queue } = state;
        for ord in ords {
            subscribed.remove(&(iface, *ord));
        }
        queue.retain(|event| subscribed.contains(&(event.iface, event.ord)));
    }

    /// Copies the occurrence into the queue of every source subscribed to it
    /// now. A source that subscribes later receives nothing retroactive, which
    /// is ridl's own rule for a late joiner on an event.
    pub(crate) fn raise(&mut self, iface: InterfaceNo, ord: Ordinal, bytes: &[u8], seq: u64) {
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
            }
        }
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

    pub(crate) fn send(
        &mut self,
        kind: CallKind,
        iface: InterfaceNo,
        ord: Ordinal,
        args: &[u8],
        seq: u64,
    ) -> Correlation {
        let id = self.next_call_id;
        self.next_call_id += 1;
        let envelope = Envelope {
            stamp: self.now,
            seq,
        };
        self.calls.insert(
            id,
            CallEntry {
                kind,
                iface,
                ord,
                args: args.to_vec(),
                envelope,
                outcome: None,
                forgotten: false,
            },
        );
        self.pending.push_back(id);
        Correlation(id)
    }

    pub(crate) fn ack(&self, c: Correlation) -> Option<Result<(), CallError>> {
        let entry = self.calls.get(&c.0)?;
        if entry.forgotten {
            return None;
        }
        if entry.kind != CallKind::Command {
            // A query's correlation always answers `None` here, which
            // `Caller::ack` states: a query's outcome comes from `reply`.
            return None;
        }
        match &entry.outcome {
            None => None,
            Some(Ok(_)) => Some(Ok(())),
            Some(Err(error)) => Some(Err(*error)),
        }
    }

    pub(crate) fn reply(
        &self,
        c: Correlation,
        out: &mut [u8],
    ) -> Result<Option<Result<usize, CallError>>, ReadError> {
        let Some(entry) = self.calls.get(&c.0) else {
            return Ok(None);
        };
        if entry.forgotten {
            return Ok(None);
        }
        match &entry.outcome {
            None => Ok(None),
            Some(Err(error)) => Ok(Some(Err(*error))),
            Some(Ok(bytes)) => {
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

    /// Releases the caller's interest in a correlation.
    ///
    /// A call whose outcome is already recorded has nothing left to happen to
    /// it, so its entry goes. A call still in flight keeps its entry and is
    /// marked forgotten: the provider still sees it at `next_claim` and still
    /// settles it — `Handler`'s contract is that every claim is settled, and a
    /// caller losing interest is not the provider's business — and the entry
    /// goes when that settlement lands. Either way the correlation answers
    /// `None` from `ack` and `reply` afterwards.
    pub(crate) fn forget(&mut self, c: Correlation) {
        let Some(entry) = self.calls.get_mut(&c.0) else {
            return;
        };
        if entry.outcome.is_some() {
            self.calls.remove(&c.0);
        } else {
            entry.forgotten = true;
        }
    }

    /// Presents the next waiting call this handler serves.
    ///
    /// `served` is the handler's served set, or `None` when it has served
    /// nothing, in which case it is presented every waiting call. The claim
    /// carries an identity of its own, minted here, so a `ClaimId` names a
    /// call that was actually presented.
    pub(crate) fn next_claim(
        &mut self,
        served: Option<&[Key]>,
        out: &mut [u8],
    ) -> Result<Option<Claim>, ReadError> {
        let position = self.pending.iter().position(|id| {
            let entry = &self.calls[id];
            served.is_none_or(|set| set.contains(&(entry.iface, entry.ord)))
        });
        let Some(position) = position else {
            return Ok(None);
        };
        let id = self.pending[position];
        let entry = &self.calls[&id];
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
        self.claims.insert(claim_id, id);
        Ok(Some(claim))
    }

    /// Records a claim's outcome.
    ///
    /// The claim is looked up before the injected failure is consumed, so
    /// `fail_next_settle` fails the next settlement of a claim that exists
    /// rather than being spent on one that does not. An injected failure
    /// leaves the claim settleable, which is what makes one failed settlement
    /// followed by a successful one expressible.
    pub(crate) fn settle(
        &mut self,
        claim: ClaimId,
        outcome: Result<&[u8], CallError>,
    ) -> Result<(), SettleError> {
        let Some(&id) = self.claims.get(&claim.0) else {
            return Err(SettleError::UnknownClaim);
        };
        if self.fail_next_settle {
            self.fail_next_settle = false;
            return Err(SettleError::TooLarge { cap: 0 });
        }
        self.claims.remove(&claim.0);
        let entry = self
            .calls
            .get_mut(&id)
            .expect("a claim names a call that is in the call table");
        if entry.forgotten {
            // The caller released it while it was in flight. It was still
            // presented and is still settled; nothing can read the outcome, so
            // the entry goes with the settlement.
            self.calls.remove(&id);
            return Ok(());
        }
        entry.outcome = Some(outcome.map(<[u8]>::to_vec));
        Ok(())
    }

    pub(crate) fn fail_next_settle(&mut self) {
        self.fail_next_settle = true;
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
