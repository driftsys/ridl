//! A disposable, in-process implementation of the `ridl-rt` port traits, for
//! this crate's tests only (Lane M stage M3, Task 4,
//! `docs/design/interaction-face.md`; the approved design is
//! `docs/archive/2026-09-16-interaction-face-v0-design.md` §5).
//!
//! This is not a runtime and carries no runtime or transport claim. It holds
//! every value in memory, in one process, on one thread; its clock is a
//! counter the test advances by hand, never wall-clock time; and it has no
//! IO. Story E11.9 replaces it with the real in-process runtime
//! (`ridl-loopback`, ADR-0020 decision 6), and the person landing E11.9
//! should read this module out rather than build on it.
//!
//! It implements exactly the ports the generated face needs to run:
//! [`Attached`], [`Clock`], [`SignalReader`], [`SignalWriter`],
//! [`EventSource`], [`EventSink`], [`Caller`], [`Handler`], and
//! [`FixedReader`]. It implements neither `ScannableSignals` nor
//! `CoherentSignals`: omitting them is allowed
//! (`ridl_rt::port`'s own module documentation), and adding either would move
//! this module toward being a reference runtime, which is not what a
//! disposable test double is for.
//!
//! `Caller` and `Handler` are two ends of one call table: a command or a
//! query sent through [`Caller::command`]/[`Caller::query`] is presented at
//! [`Handler::next_claim`] under the same id, and the settlement
//! [`Handler::settle`] records is what [`Caller::ack`] and [`Caller::reply`]
//! read back.

use std::collections::{HashMap, HashSet, VecDeque};

use ridl_rt::contract::{CatalogHash, CatalogRef, InterfaceNo, Ordinal};
use ridl_rt::error::CallError;
use ridl_rt::port::{
    Attached, Caller, Claim, ClaimId, Clock, Correlation, EventSink, EventSource, FixedReader,
    Handler, RaiseError, RawOccurrence, RawSample, ReadError, SendError, ServeError, SettleError,
    SignalReader, SignalWriter, SubscribeError, WriteError,
};
use ridl_rt::sample::{Cause, Duration, Envelope, Freshness, Provenance, Timestamp};

/// One in-memory port implementing every trait the generated face needs.
pub struct Loopback {
    catalog: CatalogRef,
    now: Timestamp,
    next_seq: u64,

    signals: HashMap<(InterfaceNo, Ordinal), SignalEntry>,
    staged: HashMap<(InterfaceNo, Ordinal), Staged>,

    subscribed: HashSet<(InterfaceNo, Ordinal)>,
    events: VecDeque<QueuedEvent>,

    fixed: HashMap<(InterfaceNo, Ordinal), Vec<u8>>,

    next_call_id: u64,
    pending_calls: VecDeque<u64>,
    calls: HashMap<u64, CallEntry>,
    fail_next_settle: bool,
}

struct SignalEntry {
    bytes: Vec<u8>,
    envelope: Envelope,
    /// `true` after `invalidate`, cleared by the next `set`.
    invalid: bool,
}

enum Staged {
    Set(Vec<u8>),
    Invalidate,
    Touch,
}

struct QueuedEvent {
    iface: InterfaceNo,
    ord: Ordinal,
    bytes: Vec<u8>,
    envelope: Envelope,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CallKind {
    Command,
    Query,
}

struct CallEntry {
    kind: CallKind,
    iface: InterfaceNo,
    ord: Ordinal,
    args: Vec<u8>,
    envelope: Envelope,
    /// `None` while unsettled, matching `Caller::ack`/`Caller::reply`.
    outcome: Option<Result<Vec<u8>, CallError>>,
}

impl Loopback {
    /// A port attached to a catalog named `package_name`, with an all-zero
    /// placeholder hash — the same placeholder the descriptor emitter writes
    /// (design §4.1).
    pub fn new(package_name: &'static str) -> Self {
        Loopback {
            catalog: CatalogRef {
                name: package_name,
                hash: CatalogHash([0u8; 32]),
            },
            now: Timestamp(0),
            next_seq: 0,
            signals: HashMap::new(),
            staged: HashMap::new(),
            subscribed: HashSet::new(),
            events: VecDeque::new(),
            fixed: HashMap::new(),
            next_call_id: 0,
            pending_calls: VecDeque::new(),
            calls: HashMap::new(),
            fail_next_settle: false,
        }
    }

    /// Advances the hand-driven clock by `by`. The clock never reads
    /// wall-clock time; a test that wants `Clock::now` to change calls this.
    pub fn advance(&mut self, by: Duration) {
        self.now = Timestamp(self.now.0 + by.0);
    }

    /// Makes the next `Handler::settle` call fail with
    /// `SettleError::TooLarge` and record no outcome, so a test can exercise
    /// one settlement failure followed by a successful one.
    pub fn fail_next_settle(&mut self) {
        self.fail_next_settle = true;
    }

    fn next_seq(&mut self) -> u64 {
        let seq = self.next_seq;
        self.next_seq += 1;
        seq
    }

    fn enqueue_call(
        &mut self,
        kind: CallKind,
        iface: InterfaceNo,
        ord: Ordinal,
        args: &[u8],
    ) -> Correlation {
        let id = self.next_call_id;
        self.next_call_id += 1;
        let envelope = Envelope {
            stamp: self.now,
            seq: self.next_seq(),
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
            },
        );
        self.pending_calls.push_back(id);
        Correlation(id)
    }
}

impl Attached for Loopback {
    fn catalog(&self) -> &CatalogRef {
        &self.catalog
    }
}

impl Clock for Loopback {
    fn now(&self) -> Timestamp {
        self.now
    }
}

impl SignalReader for Loopback {
    fn read(
        &self,
        iface: InterfaceNo,
        ord: Ordinal,
        out: &mut [u8],
    ) -> Result<RawSample, ReadError> {
        match self.signals.get(&(iface, ord)) {
            None => Ok(RawSample {
                provenance: Provenance::Init,
                freshness: Freshness::Unbounded,
                envelope: Envelope {
                    stamp: Timestamp(0),
                    seq: 0,
                },
                len: 0,
            }),
            Some(entry) => {
                if out.len() < entry.bytes.len() {
                    return Err(ReadError::Short {
                        needed: entry.bytes.len(),
                    });
                }
                out[..entry.bytes.len()].copy_from_slice(&entry.bytes);
                let provenance = if entry.invalid {
                    Provenance::Invalid(Cause::Declared)
                } else {
                    Provenance::Live
                };
                Ok(RawSample {
                    provenance,
                    freshness: Freshness::Unbounded,
                    envelope: entry.envelope,
                    len: entry.bytes.len(),
                })
            }
        }
    }
}

impl SignalWriter for Loopback {
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
        let now = self.now;
        let staged = std::mem::take(&mut self.staged);
        for ((iface, ord), op) in staged {
            let seq = self
                .signals
                .get(&(iface, ord))
                .map_or(0, |entry| entry.envelope.seq + 1);
            let envelope = Envelope { stamp: now, seq };
            match op {
                Staged::Set(bytes) => {
                    self.signals.insert(
                        (iface, ord),
                        SignalEntry {
                            bytes,
                            envelope,
                            invalid: false,
                        },
                    );
                }
                Staged::Invalidate => {
                    let bytes = self
                        .signals
                        .get(&(iface, ord))
                        .map(|entry| entry.bytes.clone())
                        .unwrap_or_default();
                    self.signals.insert(
                        (iface, ord),
                        SignalEntry {
                            bytes,
                            envelope,
                            invalid: true,
                        },
                    );
                }
                Staged::Touch => {
                    let (bytes, invalid) = self
                        .signals
                        .get(&(iface, ord))
                        .map(|entry| (entry.bytes.clone(), entry.invalid))
                        .unwrap_or_default();
                    self.signals.insert(
                        (iface, ord),
                        SignalEntry {
                            bytes,
                            envelope,
                            invalid,
                        },
                    );
                }
            }
        }
    }
}

impl EventSource for Loopback {
    fn subscribe(&mut self, iface: InterfaceNo, ords: &[Ordinal]) -> Result<(), SubscribeError> {
        for &ord in ords {
            self.subscribed.insert((iface, ord));
        }
        Ok(())
    }

    fn unsubscribe(&mut self, iface: InterfaceNo, ords: &[Ordinal]) {
        for &ord in ords {
            self.subscribed.remove(&(iface, ord));
        }
    }

    fn next(&mut self, out: &mut [u8]) -> Result<Option<RawOccurrence>, ReadError> {
        loop {
            let (iface, ord, needed) = match self.events.front() {
                None => return Ok(None),
                Some(event) => (event.iface, event.ord, event.bytes.len()),
            };
            if !self.subscribed.contains(&(iface, ord)) {
                // Not subscribed: drop it and look at the next one.
                self.events.pop_front();
                continue;
            }
            if out.len() < needed {
                return Err(ReadError::Short { needed });
            }
            break;
        }
        let event = self.events.pop_front().expect("checked by the loop above");
        out[..event.bytes.len()].copy_from_slice(&event.bytes);
        Ok(Some(RawOccurrence {
            iface: event.iface,
            ord: event.ord,
            envelope: event.envelope,
            len: event.bytes.len(),
        }))
    }
}

impl EventSink for Loopback {
    fn raise(&mut self, iface: InterfaceNo, ord: Ordinal, bytes: &[u8]) -> Result<(), RaiseError> {
        let envelope = Envelope {
            stamp: self.now,
            seq: self.next_seq(),
        };
        self.events.push_back(QueuedEvent {
            iface,
            ord,
            bytes: bytes.to_vec(),
            envelope,
        });
        Ok(())
    }
}

impl Caller for Loopback {
    fn command(
        &mut self,
        iface: InterfaceNo,
        ord: Ordinal,
        args: &[u8],
    ) -> Result<Correlation, SendError> {
        Ok(self.enqueue_call(CallKind::Command, iface, ord, args))
    }

    fn query(
        &mut self,
        iface: InterfaceNo,
        ord: Ordinal,
        args: &[u8],
    ) -> Result<Correlation, SendError> {
        Ok(self.enqueue_call(CallKind::Query, iface, ord, args))
    }

    fn ack(&mut self, c: Correlation) -> Option<Result<(), CallError>> {
        let entry = self.calls.get(&c.0)?;
        if entry.kind != CallKind::Command {
            // A query's correlation always answers `None` here
            // (`Caller::ack`'s own documentation).
            return None;
        }
        match &entry.outcome {
            None => None,
            Some(Ok(_)) => Some(Ok(())),
            Some(Err(error)) => Some(Err(*error)),
        }
    }

    fn reply(
        &mut self,
        c: Correlation,
        out: &mut [u8],
    ) -> Result<Option<Result<usize, CallError>>, ReadError> {
        let Some(entry) = self.calls.get(&c.0) else {
            return Ok(None);
        };
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

    fn forget(&mut self, c: Correlation) {
        self.calls.remove(&c.0);
    }
}

impl Handler for Loopback {
    fn serve(&mut self, _iface: InterfaceNo, _ords: &[Ordinal]) -> Result<(), ServeError> {
        // The loopback presents every call it holds regardless of `serve`;
        // tracking a served set adds nothing this harness's tests need.
        Ok(())
    }

    fn next_claim(&mut self, out: &mut [u8]) -> Result<Option<Claim>, ReadError> {
        let Some(&id) = self.pending_calls.front() else {
            return Ok(None);
        };
        let entry = self
            .calls
            .get(&id)
            .expect("a pending call id is always in the call table");
        if out.len() < entry.args.len() {
            return Err(ReadError::Short {
                needed: entry.args.len(),
            });
        }
        out[..entry.args.len()].copy_from_slice(&entry.args);
        let claim = Claim {
            id: ClaimId(id),
            iface: entry.iface,
            ord: entry.ord,
            envelope: entry.envelope,
            remaining: None,
            len: entry.args.len(),
        };
        self.pending_calls.pop_front();
        Ok(Some(claim))
    }

    fn settle(
        &mut self,
        claim: ClaimId,
        outcome: Result<&[u8], CallError>,
    ) -> Result<(), SettleError> {
        if self.fail_next_settle {
            self.fail_next_settle = false;
            return Err(SettleError::TooLarge { cap: 0 });
        }
        let Some(entry) = self.calls.get_mut(&claim.0) else {
            return Err(SettleError::UnknownClaim);
        };
        entry.outcome = Some(outcome.map(|bytes| bytes.to_vec()));
        Ok(())
    }
}

impl FixedReader for Loopback {
    fn read_fixed(
        &self,
        iface: InterfaceNo,
        ord: Ordinal,
        out: &mut [u8],
    ) -> Result<usize, ReadError> {
        let bytes = self
            .fixed
            .get(&(iface, ord))
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        if out.len() < bytes.len() {
            return Err(ReadError::Short {
                needed: bytes.len(),
            });
        }
        out[..bytes.len()].copy_from_slice(bytes);
        Ok(bytes.len())
    }
}
