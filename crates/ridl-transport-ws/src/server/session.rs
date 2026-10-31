//! The sans-IO session core: decoded frames and backend samples in, frames
//! to send out. It does no IO, so the session rules are tested without a
//! socket; `serve` is the IO around it.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use ridl_rt::contract::{default_ceiling, CatalogRef, InterfaceNo, Kind, Ordinal};
use ridl_rt::encoding::Encoding;
use ridl_rt::error::{CallError, Contract, Transport};
use ridl_rt::port::{
    Attached, Caller, Changed, Clock, Correlation, Encoded, EventSource, RawSample, ReadError,
    SendError, SignalReader, SubscribeError, Watermark,
};
use ridl_rt::sample::{Envelope, Provenance, Timestamp};

use super::calls::{Calls, InFlight, Seen};
use super::config::Record;
use super::{Backend, Config, Note};
use crate::frame::{
    self, AnswerOutcome, Attach, AttachOutcome, DecodeError, Frame, PublishState, Reason,
    ResponseOutcome, Target, FRAME_VERSION,
};
use crate::ws::CloseCode;

/// What the caller does after a message or a tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    /// Send what is in `out` and go on.
    Continue,
    /// Send what is in `out`, then close with this code.
    Close(u16),
}

/// A signal channel: `(interface, ordinal)`.
type Key = (InterfaceNo, Ordinal);

/// The session's state of one signal channel. It outlives the subscription:
/// the session `seq` of a channel never restarts (frame specification §7).
#[derive(Default)]
struct SignalChannel {
    /// Whether the client is subscribed.
    subscribed: bool,
    /// The last session `seq` assigned on this channel; 0 before the first
    /// `Live` or `InvalidDeclared` publication.
    counter: u64,
    /// The sample last sent on this channel, which a later sample is
    /// compared against. `None` before the first `publish`.
    last: Option<Sent>,
    /// The size last recorded as too large, so that a value that stays too
    /// large is recorded once per size, not once per tick.
    too_large: Option<usize>,
}

/// What a `publish` carried, as the backend gave it. A polling tick sends a
/// sample that differs from it in any field, and a `read` reuses the session
/// `seq` only for a sample equal to it in every field: a replaced writer may
/// repeat a backend `seq`, so the `seq` alone does not identify a sample.
struct Sent {
    seq: u64,
    provenance: Provenance,
    stamp: Timestamp,
    bytes: Vec<u8>,
}

impl SignalChannel {
    /// Whether `sample`, with `bytes` as its value, equals the sample last
    /// sent in every field.
    fn unchanged(&self, sample: &RawSample, bytes: &[u8]) -> bool {
        self.last.as_ref().is_some_and(|last| {
            last.seq == sample.envelope.seq
                && last.provenance == sample.provenance
                && last.stamp == sample.envelope.stamp
                && last.bytes == bytes
        })
    }
}

/// The size of the read buffer before any value needs a larger one.
const FIRST_BUFFER: usize = 1024;
/// The number of `Changed` entries a scan starts with.
const FIRST_SCAN: usize = 16;
/// The most occurrences one tick reads from the backend, the ones it drops
/// included, so that a provider that raises continuously cannot keep a tick
/// from returning. The rest wait for the next tick.
const OCCURRENCES_PER_TICK: usize = 64;

/// One session over one backend.
pub(crate) struct Session<B: Backend> {
    backend: B,
    /// The catalog the backend serves, read once.
    catalog: CatalogRef,
    /// The encoding tag of the backend's ports, or `None` when the table of
    /// `frame::encoding_tag` has no row for it.
    encoding: Option<u8>,
    /// The largest payload one `publish` may carry under the message limit.
    payload_cap: usize,
    /// The same for one `occurrence`.
    occurrence_cap: usize,
    /// The same for one `response` under `reply`.
    reply_cap: usize,
    /// The message limit, which also bounds the occurrence bytes one tick
    /// sends.
    message_limit: usize,
    /// The most bytes the kept responses may hold before the session closes
    /// with 1008.
    max_retained_bytes: usize,
    call_abandon_after: Duration,
    record: Option<Record>,
    /// Whether an `attach` has been accepted.
    attached: bool,
    /// The close code once a message or a tick returned [`Step::Close`]:
    /// from then on nothing is sent and every call returns that close.
    closed: Option<u16>,
    signals: BTreeMap<Key, SignalChannel>,
    /// One mark per interface with a subscribed signal, in scanning mode.
    marks: Vec<Watermark>,
    /// The scan buffer, in scanning mode.
    changes: Vec<Changed>,
    /// The last session `seq` assigned on each event channel; the entry
    /// outlives the subscription, as a signal channel's does.
    events: BTreeMap<Key, u64>,
    calls: Calls,
    /// The counter of the responses sent, from 1.
    response_seq: u64,
    /// The buffer every backend read copies into.
    buf: Vec<u8>,
}

impl<B: Backend> Session<B> {
    /// A session over `backend`, open to an `attach`.
    pub(crate) fn new(mut backend: B, config: &Config) -> Self {
        let catalog = *backend.ports().catalog();
        let encoding = frame::encoding_tag(<B::Ports as Encoded>::Encoding::NAME);
        // The header of each frame that carries a payload: what the message
        // limit leaves for the payload is the limit minus this.
        let target = signal_target((InterfaceNo(1), Ordinal(1)));
        let envelope = Envelope {
            stamp: Timestamp(0),
            seq: 0,
        };
        let empty_publish = Frame::Publish {
            target,
            envelope,
            state: PublishState::Live(&[]),
        };
        let empty_occurrence = Frame::Occurrence {
            target,
            envelope,
            payload: &[],
        };
        let empty_reply = Frame::Response {
            target,
            envelope,
            correlation: 0,
            outcome: ResponseOutcome::Reply(&[]),
        };
        let limit = config.max_message_bytes();
        Session {
            backend,
            catalog,
            encoding,
            payload_cap: limit - empty_publish.encoded_len(),
            occurrence_cap: limit - empty_occurrence.encoded_len(),
            reply_cap: limit - empty_reply.encoded_len(),
            message_limit: limit,
            max_retained_bytes: config.max_retained_bytes(),
            call_abandon_after: config.call_abandon_after,
            record: config.record(),
            attached: false,
            closed: None,
            signals: BTreeMap::new(),
            marks: Vec::new(),
            changes: vec![
                Changed {
                    iface: InterfaceNo(0),
                    ord: Ordinal(0),
                    seq: 0,
                };
                FIRST_SCAN
            ],
            events: BTreeMap::new(),
            calls: Calls::default(),
            response_seq: 0,
            buf: vec![0; FIRST_BUFFER],
        }
    }

    /// Handles one message from the client. Each entry pushed to `out` is one
    /// encoded frame. `now` is the wall time of the message, which the abandon
    /// bound of a call it carries is measured from. After a [`Step::Close`],
    /// every call returns that close and pushes nothing.
    pub(crate) fn on_message(&mut self, msg: &[u8], now: Instant, out: &mut Vec<Vec<u8>>) -> Step {
        if let Some(code) = self.closed {
            return Step::Close(code);
        }
        let step = self.handle(msg, now, out);
        self.keep_closed(step)
    }

    /// Polls the backend once and pushes what changed to `out`. `now` is the
    /// wall time of the tick, which a call's abandon bound is measured
    /// against. See [`Session::on_message`] for the rule after a close.
    pub(crate) fn tick(&mut self, now: Instant, out: &mut Vec<Vec<u8>>) -> Step {
        if let Some(code) = self.closed {
            return Step::Close(code);
        }
        let step = self.poll(now, out);
        self.keep_closed(step)
    }

    /// Ends the session: every backend call in flight is forgotten, then the
    /// [`Backend`] value is dropped with the session, and the backend's
    /// subscriptions end with it (the rule on [`Backend`]).
    pub(crate) fn close(mut self) {
        for c in self.calls.drain() {
            self.backend.ports().forget(c);
        }
    }

    /// Records a close, so that later calls return it.
    fn keep_closed(&mut self, step: Step) -> Step {
        if let Step::Close(code) = step {
            self.closed = Some(code);
        }
        step
    }

    fn handle(&mut self, msg: &[u8], now: Instant, out: &mut Vec<Vec<u8>>) -> Step {
        let frame = match frame::decode(msg) {
            Ok(frame) => frame,
            Err(DecodeError::Violation(v)) => {
                self.note(Note::Violation(v));
                return Step::Close(CloseCode::PROTOCOL.0);
            }
            // A frame the codec drops is still a message before the
            // `attach` (frame specification §11.1 item 4).
            Err(DecodeError::Discard(_)) if !self.attached => {
                self.note(Note::MessageBeforeAttach);
                return Step::Close(CloseCode::PROTOCOL.0);
            }
            // Only a `publish` or a `response` decodes to a discard, and
            // the server receives neither form: the client has the roles
            // wrong whether or not the frame has bytes after its header.
            Err(DecodeError::Discard(_)) => {
                self.note(Note::UnexpectedDirection);
                return Step::Close(CloseCode::PROTOCOL.0);
            }
        };
        if !self.attached {
            return match frame {
                Frame::Attach(attach) => self.attach(attach, out),
                _ => {
                    self.note(Note::MessageBeforeAttach);
                    Step::Close(CloseCode::PROTOCOL.0)
                }
            };
        }
        match frame {
            Frame::Attach(_) => {
                self.note(Note::SecondAttach);
                Step::Close(CloseCode::PROTOCOL.0)
            }
            Frame::Attached(_)
            | Frame::Answer { .. }
            | Frame::Publish { .. }
            | Frame::Occurrence { .. }
            | Frame::Response { .. }
            | Frame::Block { .. } => {
                self.note(Note::UnexpectedDirection);
                Step::Close(CloseCode::PROTOCOL.0)
            }
            Frame::Subscribe(target) => self.subscribe(target, out),
            Frame::Unsubscribe(target) => self.unsubscribe(target),
            Frame::Read(target) => self.read(target, out),
            Frame::Request {
                target,
                envelope,
                payload,
            } => self.request(target, envelope, payload, now, out),
        }
    }

    /// One pass over the backend: the signals, then the waiting occurrences,
    /// then the outcomes of the calls in flight.
    fn poll(&mut self, now: Instant, out: &mut Vec<Vec<u8>>) -> Step {
        if !self.attached {
            return Step::Continue;
        }
        let step = self.poll_signals(out);
        if step != Step::Continue {
            return step;
        }
        let step = self.drain_occurrences(out);
        if step != Step::Continue {
            return step;
        }
        self.poll_calls(now, out)
    }

    /// One pass over the signals. In scanning mode each `Changed` entry of a
    /// subscribed signal is sent, because the backend states that the
    /// channel changed and the mark set at subscribe excludes earlier
    /// commits. In polling mode every subscribed signal is read and sent
    /// when it differs in any field from the sample last sent.
    fn poll_signals(&mut self, out: &mut Vec<Vec<u8>>) -> Step {
        let scanning = self.backend.scannable().is_some();
        let changed = if scanning {
            self.scan_changes()
        } else {
            self.signals
                .iter()
                .filter(|(_, ch)| ch.subscribed)
                .map(|(key, _)| *key)
                .collect()
        };
        for key in changed {
            let target = signal_target(key);
            match self.fetch_signal(key) {
                Fetch::Sample(sample) => {
                    let ch = self.signals.entry(key).or_default();
                    ch.too_large = None;
                    if scanning || !ch.unchanged(&sample, &self.buf[..sample.len]) {
                        self.publish_signal(target, sample, scanning, out);
                    }
                }
                Fetch::TooLarge(needed) => {
                    let ch = self.signals.entry(key).or_default();
                    if ch.too_large != Some(needed) {
                        ch.too_large = Some(needed);
                        self.note(Note::ValueTooLarge { target, needed });
                    }
                }
                // The signal was readable when it was subscribed; a backend
                // that no longer serves it has nothing to publish.
                Fetch::Contract(_) => {}
                Fetch::Gone => return Step::Close(CloseCode::GOING_AWAY.0),
            }
        }
        Step::Continue
    }

    /// Sends the waiting occurrences, each with the backend's stamp and the
    /// next session `seq` of its channel, up to a budget: at most
    /// [`OCCURRENCES_PER_TICK`] of them, and no more once the bytes sent in
    /// this tick reach the message limit. What is left waits for the next
    /// tick, so a provider that raises continuously cannot keep the tick
    /// from returning. The trace context is dropped: the frame does not
    /// carry one. An occurrence that does not fit one message, or is longer
    /// than its member's limit, is read and dropped, and recorded, because
    /// `next` does not consume what it cannot copy: left in the queue, it
    /// would block every occurrence behind it.
    fn drain_occurrences(&mut self, out: &mut Vec<Vec<u8>>) -> Step {
        let mut taken = 0;
        let mut bytes = 0;
        while taken < OCCURRENCES_PER_TICK && bytes < self.message_limit {
            let cap = self.occurrence_cap;
            let read = self.backend.ports().next(room(&mut self.buf, cap));
            let (occurrence, fits) = match read {
                Ok(Some(occurrence)) => (occurrence, true),
                Ok(None) => return Step::Continue,
                Err(ReadError::Short { needed }) if needed <= cap => {
                    self.buf.resize(needed, 0);
                    continue;
                }
                // Too large to send: read into a buffer of its own, which is
                // dropped with it, so the session's buffer never grows past
                // the cap.
                Err(ReadError::Short { needed }) => {
                    let mut scratch = vec![0; needed];
                    match self.backend.ports().next(&mut scratch) {
                        Ok(Some(occurrence)) => (occurrence, false),
                        Ok(None) => return Step::Continue,
                        Err(_) => return Step::Close(CloseCode::GOING_AWAY.0),
                    }
                }
                Err(_) => return Step::Close(CloseCode::GOING_AWAY.0),
            };
            let target = Target {
                kind: Kind::Event,
                iface: occurrence.iface,
                ord: occurrence.ord,
            };
            // The occurrence's identity is known only once it is read, so
            // its member's limit is applied after the read.
            let fits = fits && occurrence.len <= self.limit(target);
            // The channel's `seq` advances for a dropped occurrence too, so
            // that the client sees the gap and records a loss (frame
            // specification §5.2) rather than nothing.
            let counter = self.events.entry((target.iface, target.ord)).or_default();
            *counter += 1;
            taken += 1;
            if !fits {
                self.note(Note::ValueTooLarge {
                    target,
                    needed: occurrence.len,
                });
                continue;
            }
            push(
                out,
                &Frame::Occurrence {
                    target,
                    envelope: Envelope {
                        stamp: occurrence.envelope.stamp,
                        seq: *counter,
                    },
                    payload: &self.buf[..occurrence.len],
                },
            );
            bytes += out.last().map_or(0, Vec::len);
        }
        Step::Continue
    }

    /// Reads the outcome of each call in flight. A known outcome is one
    /// `response`, or nothing for the three outcomes that never cross (frame
    /// specification §9.6); either way the call is then forgotten. A reply
    /// longer than its member's limit, or than one message carries, is
    /// answered `corrupt` and recorded: the backend holds a value the
    /// catalog does not allow, and the call must still end. A call
    /// still unknown `call_abandon_after` after its request arrived is
    /// forgotten and recorded, so that it does not hold a backend slot until
    /// the client disconnects. The kept responses whose bound has passed by
    /// the backend's clock are let go first.
    fn poll_calls(&mut self, now: Instant, out: &mut Vec<Vec<u8>>) -> Step {
        let clock = self.backend.ports().now();
        self.calls.expire(clock);
        for call in self.calls.in_flight() {
            let outcome = match call.target.kind {
                Kind::Command => self
                    .backend
                    .ports()
                    .ack(call.correlation)
                    .map(Outcome::from),
                _ => loop {
                    let cap = self.reply_cap.min(self.limit(call.target));
                    let ports = self.backend.ports();
                    match ports.reply(call.correlation, room(&mut self.buf, cap)) {
                        Ok(known) => break known.map(Outcome::from),
                        Err(ReadError::Short { needed }) if needed <= cap => {
                            self.buf.resize(needed, 0);
                        }
                        Err(ReadError::Short { needed }) => {
                            self.note(Note::ValueTooLarge {
                                target: call.target,
                                needed,
                            });
                            break Some(Outcome::Corrupt);
                        }
                        Err(_) => return Step::Close(CloseCode::GOING_AWAY.0),
                    }
                },
            };
            let Some(outcome) = outcome else {
                if now.duration_since(call.sent) >= self.call_abandon_after {
                    self.note(Note::CallAbandoned { seq: call.seq });
                    self.forget(call.correlation);
                }
                continue;
            };
            // The reply is copied out of the session's buffer, which the
            // response's stamp read borrows the backend for.
            let reply;
            let response = match outcome {
                Outcome::Absent => None,
                Outcome::Accepted => Some(ResponseOutcome::Accepted),
                Outcome::Reply(len) => {
                    reply = self.buf[..len].to_vec();
                    Some(ResponseOutcome::Reply(&reply))
                }
                Outcome::Contract(c) => Some(ResponseOutcome::Contract(c)),
                Outcome::Corrupt => Some(ResponseOutcome::Corrupt),
                Outcome::Busy => Some(ResponseOutcome::Busy),
            };
            let step = match response {
                Some(response) => self.respond(
                    call.target,
                    call.seq,
                    response,
                    call.stamp,
                    call.receipt,
                    out,
                ),
                None => Step::Continue,
            };
            self.forget(call.correlation);
            if step != Step::Continue {
                return step;
            }
        }
        Step::Continue
    }

    /// Forgets a call on the backend and in the session.
    fn forget(&mut self, correlation: Correlation) {
        self.backend.ports().forget(correlation);
        self.calls.remove(correlation);
    }

    fn note(&self, note: Note) {
        if let Some(record) = &self.record {
            record(&note);
        }
    }

    /// Checks an `attach` in the order of its refusal reasons, so that one
    /// reason is given: frame version, unit name, hash, encoding.
    fn attach(&mut self, attach: Attach<'_>, out: &mut Vec<Vec<u8>>) -> Step {
        let refusal = if attach.frame_version() != FRAME_VERSION {
            Some(Reason::FrameVersionUnsupported)
        } else if attach.name() != self.catalog.name {
            Some(Reason::UnknownCatalog)
        } else if !self
            .backend
            .members()
            .map_or(*attach.hash() == self.catalog.hash, |t| {
                t.accepts(attach.hash())
            })
        {
            Some(Reason::CatalogMismatch)
        } else if Some(attach.encoding()) != self.encoding {
            Some(Reason::EncodingUnsupported)
        } else {
            None
        };
        match refusal {
            Some(reason) => {
                push(out, &Frame::Attached(AttachOutcome::Refused(reason)));
                Step::Close(CloseCode::NORMAL.0)
            }
            None => {
                push(out, &Frame::Attached(AttachOutcome::Accepted));
                self.attached = true;
                Step::Continue
            }
        }
    }

    /// Sends one `answer`. A `contract(InvalidValue)` whose rule the frame
    /// cannot carry is sent as `contract(ContractBroken)` and recorded: the
    /// `answer` form has no `corrupt` outcome.
    fn answer(&self, target: Target, outcome: AnswerOutcome, out: &mut Vec<Vec<u8>>) {
        let outcome = match outcome {
            AnswerOutcome::Contract(c) if !carried(c) => {
                self.note(Note::RuleNotCarried { target });
                AnswerOutcome::Contract(Contract::ContractBroken)
            }
            outcome => outcome,
        };
        push(out, &Frame::Answer { target, outcome });
    }

    /// Whether the frame's identity is in the member table under its kind.
    /// With no table, every identity is known and only the form check applies.
    fn known(&self, target: Target) -> bool {
        self.backend.members().is_none_or(|table| {
            table
                .member(target.iface, target.ord)
                .is_some_and(|(kind, _)| kind == target.kind)
        })
    }

    fn subscribe(&mut self, target: Target, out: &mut Vec<Vec<u8>>) -> Step {
        if !self.known(target) {
            self.note(Note::UnknownInteraction { target });
            self.answer(
                target,
                AnswerOutcome::Contract(Contract::UnknownInteraction),
                out,
            );
            return Step::Continue;
        }
        match target.kind {
            Kind::Signal => self.subscribe_signal(target, out),
            Kind::Event => self.subscribe_event(target, out),
            _ => {
                self.answer(
                    target,
                    AnswerOutcome::Contract(Contract::UnknownInteraction),
                    out,
                );
                Step::Continue
            }
        }
    }

    /// Subscribes the event on the backend before the `answer` is sent;
    /// nothing is drained until the next tick, so no occurrence reaches the
    /// client before its `answer`.
    fn subscribe_event(&mut self, target: Target, out: &mut Vec<Vec<u8>>) -> Step {
        match self.backend.ports().subscribe(target.iface, &[target.ord]) {
            Ok(()) => self.answer(target, AnswerOutcome::Accepted, out),
            Err(SubscribeError::Contract(c)) => {
                self.answer(target, AnswerOutcome::Contract(c), out)
            }
            Err(_) => return Step::Close(CloseCode::GOING_AWAY.0),
        }
        Step::Continue
    }

    fn subscribe_signal(&mut self, target: Target, out: &mut Vec<Vec<u8>>) -> Step {
        let key = (target.iface, target.ord);
        if self.signals.get(&key).is_some_and(|ch| ch.subscribed) {
            self.answer(target, AnswerOutcome::Accepted, out);
            return Step::Continue;
        }
        // The mark is set before the read, so a commit between the two is
        // scanned on the next tick rather than lost.
        let marked = self.mark(target.iface);
        match self.fetch_signal(key) {
            Fetch::Contract(c) => {
                if marked {
                    self.marks.retain(|m| m.iface != target.iface);
                }
                self.answer(target, AnswerOutcome::Contract(c), out);
            }
            Fetch::Gone => return Step::Close(CloseCode::GOING_AWAY.0),
            Fetch::Sample(sample) => {
                self.signals.entry(key).or_default().subscribed = true;
                self.answer(target, AnswerOutcome::Accepted, out);
                self.publish_signal(target, sample, false, out);
            }
            Fetch::TooLarge(needed) => {
                let ch = self.signals.entry(key).or_default();
                ch.subscribed = true;
                ch.too_large = Some(needed);
                self.answer(target, AnswerOutcome::Accepted, out);
                self.note(Note::ValueTooLarge { target, needed });
            }
        }
        Step::Continue
    }

    /// Adds the interface's watermark in scanning mode, at its current
    /// generation, when it has none. Returns whether one was added.
    fn mark(&mut self, iface: InterfaceNo) -> bool {
        let Some(scannable) = self.backend.scannable() else {
            return false;
        };
        if self.marks.iter().any(|m| m.iface == iface) {
            return false;
        }
        let generation = scannable.generation(iface);
        self.marks.push(Watermark { iface, generation });
        true
    }

    fn unsubscribe(&mut self, target: Target) -> Step {
        if !self.known(target) {
            self.note(Note::UnknownInteraction { target });
            return Step::Continue;
        }
        match target.kind {
            Kind::Signal => {
                let key = (target.iface, target.ord);
                if let Some(ch) = self.signals.get_mut(&key) {
                    ch.subscribed = false;
                }
                let iface_still_subscribed = self
                    .signals
                    .iter()
                    .any(|((iface, _), ch)| *iface == target.iface && ch.subscribed);
                if !iface_still_subscribed {
                    self.marks.retain(|m| m.iface != target.iface);
                }
            }
            Kind::Event => self
                .backend
                .ports()
                .unsubscribe(target.iface, &[target.ord]),
            _ => self.note(Note::KindNotAdmitted { target }),
        }
        Step::Continue
    }

    fn read(&mut self, target: Target, out: &mut Vec<Vec<u8>>) -> Step {
        if !self.known(target) {
            self.note(Note::UnknownInteraction { target });
            self.answer(
                target,
                AnswerOutcome::Contract(Contract::UnknownInteraction),
                out,
            );
            return Step::Continue;
        }
        match target.kind {
            Kind::Signal => match self.fetch_signal((target.iface, target.ord)) {
                Fetch::Sample(sample) => self.publish_signal(target, sample, false, out),
                // A value that is not sent is answered as a `fixed` that is
                // not sent is (`read_fixed`), so that the read ends instead
                // of waiting until the session ends.
                Fetch::TooLarge(needed) => {
                    self.note(Note::ValueTooLarge { target, needed });
                    self.answer(
                        target,
                        AnswerOutcome::Contract(Contract::UnknownInteraction),
                        out,
                    );
                }
                Fetch::Contract(c) => self.answer(target, AnswerOutcome::Contract(c), out),
                Fetch::Gone => return Step::Close(CloseCode::GOING_AWAY.0),
            },
            Kind::Fixed => return self.read_fixed(target, out),
            _ => self.answer(
                target,
                AnswerOutcome::Contract(Contract::UnknownInteraction),
                out,
            ),
        }
        Step::Continue
    }

    /// Answers a `read` of a `fixed` with one `publish`: `Live`, `seq` 0 and
    /// the backend's clock as the stamp. A value that one message cannot
    /// carry, or that is longer than the member's limit, is not sent: the
    /// read is answered `contract(UnknownInteraction)`, as a `fixed` with no
    /// value is, and recorded. Until the binding carries a `fixed` in blocks
    /// (driftsys/ridl#1129), that is every `fixed` above `max_message_bytes`
    /// minus the 27-byte `publish` header, 16,137 bytes at the default
    /// limit. A `read` of a signal above its limit is answered the same way.
    fn read_fixed(&mut self, target: Target, out: &mut Vec<Vec<u8>>) -> Step {
        let cap = self.payload_cap.min(self.limit(target));
        let len = loop {
            match self
                .backend
                .read_fixed(target.iface, target.ord, room(&mut self.buf, cap))
            {
                Ok(len) => break len,
                Err(ReadError::Short { needed }) => {
                    if needed > cap {
                        self.note(Note::ValueTooLarge { target, needed });
                        self.answer(
                            target,
                            AnswerOutcome::Contract(Contract::UnknownInteraction),
                            out,
                        );
                        return Step::Continue;
                    }
                    self.buf.resize(needed, 0);
                }
                Err(ReadError::Contract(c)) => {
                    self.answer(target, AnswerOutcome::Contract(c), out);
                    return Step::Continue;
                }
                Err(_) => return Step::Close(CloseCode::GOING_AWAY.0),
            }
        };
        let stamp = self.backend.ports().now();
        push(
            out,
            &Frame::Publish {
                target,
                envelope: Envelope { stamp, seq: 0 },
                state: PublishState::Live(&self.buf[..len]),
            },
        );
        Step::Continue
    }

    /// Handles one `request`. Its `seq` is placed first (frame specification
    /// §7): a repeat of a call in flight is dropped and recorded, a repeat of
    /// a settled call gets its response again, and a `seq` not above the
    /// highest seen that is neither is dropped and recorded. A new `seq` is
    /// sent to the backend, with no trace context; a send that fails is
    /// answered at once. A member that the member table does not hold under
    /// the request's kind is answered `contract(UnknownInteraction)`. Arguments
    /// longer than the member's limit (see [`Session::limit`]) are answered
    /// `corrupt` without a call to the backend, and no note is recorded; the
    /// response is kept like any other.
    fn request(
        &mut self,
        target: Target,
        envelope: Envelope,
        payload: &[u8],
        now: Instant,
        out: &mut Vec<Vec<u8>>,
    ) -> Step {
        let seq = envelope.seq;
        let stamp = envelope.stamp;
        // Responses whose bound has passed are let go before the `seq` is
        // placed, so that a repeat after the bound is not answered from them.
        let receipt = self.backend.ports().now();
        self.calls.expire(receipt);
        match self.calls.seen(seq) {
            Seen::New => self.calls.accept(seq),
            Seen::Settled(response) => {
                out.push(response);
                return Step::Continue;
            }
            Seen::InFlight | Seen::Older => {
                self.note(Note::DuplicateDropped { seq });
                return Step::Continue;
            }
        }
        if !self.known(target) {
            self.note(Note::UnknownInteraction { target });
            return self.respond(
                target,
                seq,
                ResponseOutcome::Contract(Contract::UnknownInteraction),
                stamp,
                receipt,
                out,
            );
        }
        // Only a command or a query is answered; any other kind falls to the
        // unknown-interaction answer below whatever the length.
        let over_limit = matches!(target.kind, Kind::Command | Kind::Query)
            && payload.len() > self.limit(target);
        if over_limit {
            return self.respond(target, seq, ResponseOutcome::Corrupt, stamp, receipt, out);
        }
        let ports = self.backend.ports();
        let sent = match target.kind {
            Kind::Command => ports.command(target.iface, target.ord, payload, None),
            Kind::Query => ports.query(target.iface, target.ord, payload, None),
            _ => {
                return self.respond(
                    target,
                    seq,
                    ResponseOutcome::Contract(Contract::UnknownInteraction),
                    stamp,
                    receipt,
                    out,
                );
            }
        };
        let outcome = match sent {
            Ok(correlation) => {
                self.calls.track(InFlight {
                    correlation,
                    target,
                    seq,
                    sent: now,
                    stamp,
                    receipt,
                });
                return Step::Continue;
            }
            Err(SendError::Busy) => ResponseOutcome::Busy,
            Err(SendError::Contract(c)) => ResponseOutcome::Contract(c),
            // Arguments larger than the member's capacity are not a
            // well-formed encoding of its type.
            Err(SendError::TooLarge { .. }) => ResponseOutcome::Corrupt,
            Err(_) => return Step::Close(CloseCode::GOING_AWAY.0),
        };
        self.respond(target, seq, outcome, stamp, receipt, out)
    }

    /// Sends one `response`, under the next response `seq` of the session
    /// and the backend's clock as the stamp, and keeps it for a repeat of
    /// the request: until the request's bound has passed, measured from the
    /// earlier of the request's `stamp` and `receipt`, when the member table
    /// gives one (frame specification §7), and as one of the last 64
    /// otherwise. A `contract(InvalidValue)` whose rule the frame cannot
    /// carry is sent as `corrupt` and recorded. Returns a close with 1008
    /// when the kept responses then pass [`Config::max_retained_bytes`]; the
    /// response is still sent.
    fn respond(
        &mut self,
        target: Target,
        correlation: u64,
        outcome: ResponseOutcome<'_>,
        stamp: Timestamp,
        receipt: Timestamp,
        out: &mut Vec<Vec<u8>>,
    ) -> Step {
        let outcome = match outcome {
            ResponseOutcome::Contract(c) if !carried(c) => {
                self.note(Note::RuleNotCarried { target });
                ResponseOutcome::Corrupt
            }
            outcome => outcome,
        };
        self.response_seq += 1;
        let now = self.backend.ports().now();
        push(
            out,
            &Frame::Response {
                target,
                envelope: Envelope {
                    stamp: now,
                    seq: self.response_seq,
                },
                correlation,
                outcome,
            },
        );
        let until = self.retention(target, stamp, receipt);
        let response = out.last().expect("the response was just pushed");
        self.calls.settle(correlation, response, until);
        if self.calls.retained_bytes() > self.max_retained_bytes {
            return Step::Close(CloseCode::POLICY.0);
        }
        Step::Continue
    }

    /// The last instant of the backend's clock at which the response to a
    /// request is kept: `min(stamp, receipt) + max`, with `max` the member's
    /// bound from the member table. `None` when the backend gives no table,
    /// the table gives the member no bound, or the table does not hold the
    /// identity as a command or query under the request's kind: a refused
    /// request takes no bound from a member of another kind (a signal's
    /// `max_us` is not a call bound).
    fn retention(&self, target: Target, stamp: Timestamp, receipt: Timestamp) -> Option<Timestamp> {
        let (kind, max) = self.backend.members()?.member(target.iface, target.ord)?;
        if kind != target.kind || !matches!(kind, Kind::Command | Kind::Query) {
            return None;
        }
        let max = max?;
        let max_micros = i64::try_from(max.as_micros()).unwrap_or(i64::MAX);
        Some(Timestamp(stamp.0.min(receipt.0).saturating_add(max_micros)))
    }

    /// The payload limit of a member, in bytes: the limit the member table
    /// gives it, or the default ceiling of its kind when the table gives none
    /// or the backend has no table (ADR-0030 decisions 13 and 19). A value
    /// the session sends, and the arguments of a `request`, are checked
    /// against it.
    fn limit(&self, target: Target) -> usize {
        self.backend
            .members()
            .and_then(|table| table.limit(target.iface, target.ord))
            .map_or(default_ceiling(target.kind), |limit| limit as usize)
    }

    /// Reads a signal into the session's buffer, growing it on `Short` up to
    /// the payload the message limit allows and the member's limit.
    fn fetch_signal(&mut self, key: Key) -> Fetch {
        let cap = self.payload_cap.min(self.limit(signal_target(key)));
        loop {
            let ports = self.backend.ports();
            match ports.read(key.0, key.1, room(&mut self.buf, cap)) {
                Ok(sample) => return Fetch::Sample(sample),
                Err(ReadError::Short { needed }) => {
                    if needed > cap {
                        return Fetch::TooLarge(needed);
                    }
                    self.buf.resize(needed, 0);
                }
                Err(ReadError::Contract(c)) => return Fetch::Contract(c),
                Err(_) => return Fetch::Gone,
            }
        }
    }

    /// Sends a signal's sample as one `publish`, with the backend's stamp
    /// and the session's `seq`: 0 under `Init`; otherwise the next one, or,
    /// unless `fresh`, the `seq` sent last when the sample equals the one
    /// sent last in every field.
    ///
    /// An `Init` sample on a channel the session holds no state for, which
    /// only a `read` sends, is published without creating that state: its
    /// `seq` is 0 whatever the channel holds, so a client cannot grow the
    /// session by reading signals that were never published.
    fn publish_signal(
        &mut self,
        target: Target,
        sample: RawSample,
        fresh: bool,
        out: &mut Vec<Vec<u8>>,
    ) {
        let key = (target.iface, target.ord);
        if sample.provenance == Provenance::Init && !self.signals.contains_key(&key) {
            push(
                out,
                &Frame::Publish {
                    target,
                    envelope: Envelope {
                        stamp: sample.envelope.stamp,
                        seq: 0,
                    },
                    state: PublishState::Init,
                },
            );
            return;
        }
        let ch = self.signals.entry(key).or_default();
        let bytes = &self.buf[..sample.len];
        let seq = match sample.provenance {
            Provenance::Init => 0,
            _ if !fresh && ch.unchanged(&sample, bytes) => ch.counter,
            _ => {
                ch.counter += 1;
                ch.counter
            }
        };
        ch.last = Some(Sent {
            seq: sample.envelope.seq,
            provenance: sample.provenance,
            stamp: sample.envelope.stamp,
            bytes: bytes.to_vec(),
        });
        let state = match sample.provenance {
            Provenance::Init => PublishState::Init,
            Provenance::Live => PublishState::Live(&self.buf[..sample.len]),
            // `Detected(_)` never crosses (frame specification §9.6): the
            // invalidity is sent and its cause is lost.
            Provenance::Invalid(_) => PublishState::InvalidDeclared,
        };
        push(
            out,
            &Frame::Publish {
                target,
                envelope: Envelope {
                    stamp: sample.envelope.stamp,
                    seq,
                },
                state,
            },
        );
    }

    /// Scans the marked interfaces once and returns the subscribed signals
    /// that changed, each key once. The scan is repeated only to grow the
    /// buffer, when it wrote nothing while a mark is still behind its
    /// generation, as the port's documentation describes; an interface whose
    /// changes did not fit behind the ones written waits for the next tick.
    /// One pass per tick keeps a provider that commits continuously from
    /// holding the tick, and a commit between two scans from being sent twice.
    fn scan_changes(&mut self) -> Vec<Key> {
        let Some(scannable) = self.backend.scannable() else {
            return Vec::new();
        };
        if self.marks.is_empty() {
            return Vec::new();
        }
        let n = loop {
            let n = scannable.scan(&mut self.marks, &mut self.changes);
            if n > 0 {
                break n;
            }
            let behind = self
                .marks
                .iter()
                .any(|m| m.generation < scannable.generation(m.iface));
            if !behind {
                return Vec::new();
            }
            let len = self.changes.len() * 2;
            self.changes.resize(
                len,
                Changed {
                    iface: InterfaceNo(0),
                    ord: Ordinal(0),
                    seq: 0,
                },
            );
        };
        let mut keys: Vec<Key> = self.changes[..n]
            .iter()
            .map(|c| (c.iface, c.ord))
            .filter(|key| self.signals.get(key).is_some_and(|ch| ch.subscribed))
            .collect();
        keys.sort_unstable();
        keys.dedup();
        keys
    }
}

/// A call's known outcome, as the `response` carries it.
enum Outcome {
    Accepted,
    /// A reply of this many bytes, in the session's buffer.
    Reply(usize),
    Contract(Contract),
    Corrupt,
    Busy,
    /// An outcome that never crosses: `Timeout`, `Undelivered` or `Down`
    /// (frame specification §9.6). The client's own bound reports it.
    Absent,
}

impl From<Result<(), CallError>> for Outcome {
    fn from(ack: Result<(), CallError>) -> Self {
        match ack {
            Ok(()) => Outcome::Accepted,
            Err(e) => Outcome::failed(e),
        }
    }
}

impl From<Result<usize, CallError>> for Outcome {
    fn from(reply: Result<usize, CallError>) -> Self {
        match reply {
            Ok(len) => Outcome::Reply(len),
            Err(e) => Outcome::failed(e),
        }
    }
}

impl Outcome {
    fn failed(e: CallError) -> Self {
        match e {
            CallError::Contract(c) => Outcome::Contract(c),
            CallError::Transport(Transport::Corrupt) => Outcome::Corrupt,
            CallError::Transport(Transport::Busy) => Outcome::Busy,
            // `Timeout`, `Undelivered` and `Down`; `Transport` is
            // non-exhaustive, and a variant this crate does not know has no
            // frame either.
            CallError::Transport(_) => Outcome::Absent,
        }
    }
}

/// What a backend read of a signal gave.
enum Fetch {
    Sample(RawSample),
    /// The value needs more bytes than one message may carry, or than the
    /// member's limit allows.
    TooLarge(usize),
    Contract(Contract),
    /// The port cannot be read: the runtime behind it is gone.
    Gone,
}

/// Whether the frame can carry `c`: every contract error but an
/// `InvalidValue` whose `Rule` frame version 1 has no code for (frame
/// specification §9.6), which a backend built against a newer `ridl-rt`
/// could report.
fn carried(c: Contract) -> bool {
    match c {
        Contract::InvalidValue(v) => frame::rule_tag(v.rule).is_some(),
        _ => true,
    }
}

fn signal_target(key: Key) -> Target {
    Target {
        kind: Kind::Signal,
        iface: key.0,
        ord: key.1,
    }
}

/// The front of `buf`, at most `cap` bytes: what a backend read may copy
/// into, so that a value over the cap is reported as `Short` and never read
/// whole into the session's buffer, whatever size the buffer has.
fn room(buf: &mut [u8], cap: usize) -> &mut [u8] {
    let n = buf.len().min(cap);
    &mut buf[..n]
}

/// Encodes one frame as one entry of `out`.
fn push(out: &mut Vec<Vec<u8>>, frame: &Frame<'_>) {
    let mut v = vec![0; frame.encoded_len()];
    let n = frame
        .encode(&mut v)
        .expect("the buffer has the frame's length");
    debug_assert_eq!(n, v.len());
    out.push(v);
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::sync::{Arc, Mutex};
    use std::time::Instant;

    use ridl_loopback::Loopback;
    use ridl_rt::contract::{
        CatalogHash, CatalogRef, InterfaceNo, Kind, Ordinal, CALL_CEILING, FIXED_CEILING,
        SIGNAL_EVENT_CEILING,
    };
    use ridl_rt::error::{CallError, Contract, Transport};
    use ridl_rt::payload::{Rule, Violation};
    use ridl_rt::port::{
        Attached, Caller, Changed, Claim, Clock, Correlation, Encoded, EventSink, EventSource,
        FixedReader, Handler, RawOccurrence, RawSample, ReadError, ScannableSignals, SendError,
        SignalReader, SignalWriter, SubscribeError, Watermark,
    };
    use ridl_rt::sample::{Cause, Detection, Duration, Envelope, Freshness, Provenance, Timestamp};
    use ridl_rt::trace::TraceContext;

    use super::{Session, Step};
    use crate::frame::{
        decode, AnswerOutcome, Attach, AttachOutcome, Frame, PublishState, Reason, ResponseOutcome,
        Target,
    };
    use crate::server::{
        Backend, Config, Described, MemberTable, Note, Polling, Scanning, TableMismatch,
    };

    const CATALOG: CatalogRef = CatalogRef {
        name: "ws.test",
        hash: CatalogHash([7u8; 32]),
    };
    const IFACE: InterfaceNo = InterfaceNo(1);
    const SIGNAL: Target = Target {
        kind: Kind::Signal,
        iface: IFACE,
        ord: Ordinal(1),
    };
    const FIXED: Target = Target {
        kind: Kind::Fixed,
        iface: IFACE,
        ord: Ordinal(5),
    };
    /// The size of a `publish` that carries no payload.
    const PUBLISH_HEADER: usize = 27;

    fn target(kind: Kind, ord: u32) -> Target {
        Target {
            kind,
            iface: IFACE,
            ord: Ordinal(ord),
        }
    }

    fn msg(frame: Frame<'_>) -> Vec<u8> {
        let mut v = vec![0; frame.encoded_len()];
        frame
            .encode(&mut v)
            .expect("the buffer has the frame's length");
        v
    }

    fn attach_msg(version: u32, encoding: u8, hash: [u8; 32], name: &str) -> Vec<u8> {
        msg(Frame::Attach(
            Attach::new(version, encoding, CatalogHash(hash), name).expect("a valid attach"),
        ))
    }

    fn now() -> Instant {
        Instant::now()
    }

    /// The recorded notes of one session.
    type Notes = Arc<Mutex<Vec<Note>>>;

    fn config(notes: &Notes) -> Config {
        let notes = Arc::clone(notes);
        Config::default().with_record(move |n| notes.lock().unwrap().push(*n))
    }

    /// A session over a scanning loopback, the provider's aggregate on the
    /// same store, and the notes.
    fn scanning() -> (Session<Scanning<Loopback>>, Loopback, Notes) {
        let notes = Notes::default();
        let rt = Loopback::new(CATALOG);
        let provider = rt.attach();
        (Session::new(Scanning(rt), &config(&notes)), provider, notes)
    }

    /// A session over a scanning loopback described by `t`, the provider's
    /// aggregate on the same store, and the notes.
    fn described_with(
        t: MemberTable,
        config: impl FnOnce(Config) -> Config,
    ) -> (Session<Described<Scanning<Loopback>>>, Loopback, Notes) {
        let notes = Notes::default();
        let rt = Loopback::new(CATALOG);
        let provider = rt.attach();
        let backend = Described::new(Scanning(rt), Arc::new(t)).unwrap();
        let config = config(Config::default());
        let n = Arc::clone(&notes);
        let config = config.with_record(move |x| n.lock().unwrap().push(*x));
        (Session::new(backend, &config), provider, notes)
    }

    /// The same over a polling loopback.
    fn polling() -> (Session<Polling<Loopback>>, Loopback, Notes) {
        let notes = Notes::default();
        let rt = Loopback::new(CATALOG);
        let provider = rt.attach();
        (Session::new(Polling(rt), &config(&notes)), provider, notes)
    }

    /// Sends one message and returns the frames the session answered with.
    fn send<B: Backend>(s: &mut Session<B>, m: &[u8]) -> (Step, Vec<Vec<u8>>) {
        let mut out = Vec::new();
        let step = s.on_message(m, now(), &mut out);
        (step, out)
    }

    fn tick<B: Backend>(s: &mut Session<B>) -> (Step, Vec<Vec<u8>>) {
        let mut out = Vec::new();
        let step = s.tick(now(), &mut out);
        (step, out)
    }

    /// Attaches with the matching `attach` and checks it is accepted.
    fn open<B: Backend>(s: &mut Session<B>) {
        let (step, out) = send(s, &attach_msg(1, 1, [7u8; 32], "ws.test"));
        assert_eq!(step, Step::Continue);
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            Frame::Attached(AttachOutcome::Accepted)
        );
    }

    fn accepted(target: Target) -> Frame<'static> {
        Frame::Answer {
            target,
            outcome: AnswerOutcome::Accepted,
        }
    }

    fn unknown(target: Target) -> Frame<'static> {
        Frame::Answer {
            target,
            outcome: AnswerOutcome::Contract(Contract::UnknownInteraction),
        }
    }

    fn publish(target: Target, stamp: i64, seq: u64, state: PublishState<'_>) -> Frame<'_> {
        Frame::Publish {
            target,
            envelope: Envelope {
                stamp: Timestamp(stamp),
                seq,
            },
            state,
        }
    }

    /// The shared `block-last` vector: block 1 of a payload of 4,098 bytes.
    fn block_last() -> Vec<u8> {
        msg(Frame::Block {
            target: FIXED,
            envelope: Envelope {
                stamp: Timestamp(0x0123_4567_89AB),
                seq: 0,
            },
            total: 4_098,
            index: 1,
            payload: &[0xab, 0xcd],
        })
    }

    fn commit(provider: &mut Loopback, bytes: &[u8]) {
        provider.set(IFACE, Ordinal(1), bytes).unwrap();
        provider.commit();
    }

    // -- D-11: attach --------------------------------------------------------

    #[test]
    fn the_first_message_must_be_attach() {
        let (mut s, _p, notes) = scanning();
        let (step, out) = send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
        assert_eq!(step, Step::Close(1002));
        assert!(out.is_empty());
        assert_eq!(*notes.lock().unwrap(), vec![Note::MessageBeforeAttach]);
    }

    /// A frame the codec would drop on an open session is still a message
    /// before the `attach`, so it closes the session with 1002.
    #[test]
    fn a_discardable_frame_before_attach_closes_1002() {
        let (mut s, _p, notes) = scanning();
        let mut m = msg(publish(SIGNAL, 0, 0, PublishState::Init));
        m.push(0);
        let (step, out) = send(&mut s, &m);
        assert_eq!(step, Step::Close(1002));
        assert!(out.is_empty());
        assert_eq!(*notes.lock().unwrap(), vec![Note::MessageBeforeAttach]);
    }

    #[test]
    fn a_second_attach_is_a_violation() {
        let (mut s, _p, notes) = scanning();
        open(&mut s);
        let (step, out) = send(&mut s, &attach_msg(1, 1, [7u8; 32], "ws.test"));
        assert_eq!(step, Step::Close(1002));
        assert!(out.is_empty());
        assert_eq!(*notes.lock().unwrap(), vec![Note::SecondAttach]);
    }

    fn refused<B: Backend>(s: &mut Session<B>, m: &[u8], reason: Reason) {
        let (step, out) = send(s, m);
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            Frame::Attached(AttachOutcome::Refused(reason))
        );
        assert_eq!(step, Step::Close(1000));
    }

    #[test]
    fn frame_version_2_is_refused() {
        let (mut s, _p, _n) = scanning();
        refused(
            &mut s,
            &attach_msg(2, 1, [7u8; 32], "ws.test"),
            Reason::FrameVersionUnsupported,
        );
    }

    #[test]
    fn another_unit_name_is_refused_unknown_catalog() {
        let (mut s, _p, _n) = scanning();
        refused(
            &mut s,
            &attach_msg(1, 1, [7u8; 32], "ws.other"),
            Reason::UnknownCatalog,
        );
    }

    #[test]
    fn the_same_name_with_another_hash_is_refused_catalog_mismatch() {
        let (mut s, _p, _n) = scanning();
        refused(
            &mut s,
            &attach_msg(1, 1, [8u8; 32], "ws.test"),
            Reason::CatalogMismatch,
        );
    }

    #[test]
    fn another_encoding_tag_is_refused_encoding_unsupported() {
        let (mut s, _p, _n) = scanning();
        refused(
            &mut s,
            &attach_msg(1, 2, [7u8; 32], "ws.test"),
            Reason::EncodingUnsupported,
        );
    }

    #[test]
    fn a_refusal_is_followed_by_close_1000() {
        let (mut s, _p, _n) = scanning();
        let (step, out) = send(&mut s, &attach_msg(1, 1, [7u8; 32], "ws.other"));
        assert_eq!(out.len(), 1);
        assert_eq!(step, Step::Close(1000));
    }

    #[test]
    fn a_matching_attach_is_accepted() {
        let (mut s, _p, _n) = scanning();
        open(&mut s);
        // The session is open: a subscribe is answered, not refused.
        let (step, out) = send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
        assert_eq!(step, Step::Continue);
        assert_eq!(decode(&out[0]).unwrap(), accepted(SIGNAL));
    }

    // -- D-12 and D-25: routing on the kind ---------------------------------

    #[test]
    fn a_subscribe_of_a_command_is_answered_unknown_interaction_with_its_own_identity() {
        let (mut s, _p, _n) = scanning();
        open(&mut s);
        let t = target(Kind::Command, 3);
        let (step, out) = send(&mut s, &msg(Frame::Subscribe(t)));
        assert_eq!(step, Step::Continue);
        assert_eq!(out.len(), 1);
        assert_eq!(decode(&out[0]).unwrap(), unknown(t));
    }

    #[test]
    fn a_read_of_an_event_is_answered_unknown_interaction() {
        let (mut s, _p, _n) = scanning();
        open(&mut s);
        let t = target(Kind::Event, 2);
        let (step, out) = send(&mut s, &msg(Frame::Read(t)));
        assert_eq!(step, Step::Continue);
        assert_eq!(out.len(), 1);
        assert_eq!(decode(&out[0]).unwrap(), unknown(t));
    }

    #[test]
    fn a_request_of_a_signal_kind_is_answered_unknown_interaction_with_kind_signal() {
        let (mut s, mut p, _n) = scanning();
        open(&mut s);
        p.advance(Duration(40));
        let request = Frame::Request {
            target: SIGNAL,
            envelope: Envelope {
                stamp: Timestamp(9),
                seq: 17,
            },
            payload: &[1, 2, 3],
        };
        let (step, out) = send(&mut s, &msg(request));
        assert_eq!(step, Step::Continue);
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            Frame::Response {
                target: SIGNAL,
                envelope: Envelope {
                    stamp: Timestamp(40),
                    seq: 1,
                },
                correlation: 17,
                outcome: ResponseOutcome::Contract(Contract::UnknownInteraction),
            }
        );
    }

    #[test]
    fn a_publish_from_the_client_closes_with_1002() {
        let (mut s, _p, notes) = scanning();
        open(&mut s);
        let (step, out) = send(
            &mut s,
            &msg(publish(SIGNAL, 0, 1, PublishState::Live(&[1]))),
        );
        assert_eq!(step, Step::Close(1002));
        assert!(out.is_empty());
        assert_eq!(*notes.lock().unwrap(), vec![Note::UnexpectedDirection]);
    }

    /// Every provider-to-consumer form, not only `publish`.
    #[test]
    fn every_provider_to_consumer_form_from_the_client_closes_with_1002() {
        let envelope = Envelope {
            stamp: Timestamp(0),
            seq: 1,
        };
        let frames = [
            Frame::Attached(AttachOutcome::Accepted),
            accepted(SIGNAL),
            Frame::Occurrence {
                target: target(Kind::Event, 2),
                envelope,
                payload: &[1],
            },
            Frame::Response {
                target: target(Kind::Command, 3),
                envelope,
                correlation: 1,
                outcome: ResponseOutcome::Accepted,
            },
        ];
        for frame in frames {
            let (mut s, _p, notes) = scanning();
            open(&mut s);
            let (step, out) = send(&mut s, &msg(frame));
            assert_eq!(step, Step::Close(1002), "{frame:?}");
            assert!(out.is_empty(), "{frame:?}");
            assert_eq!(
                *notes.lock().unwrap(),
                vec![Note::UnexpectedDirection],
                "{frame:?}"
            );
        }
    }

    /// A `block` is a provider-to-consumer form (frame version 2): the
    /// provider never reassembles, so a block from the client is a violation.
    #[test]
    fn a_block_from_the_client_closes_with_1002_and_records_unexpected_direction() {
        let (mut s, _p, notes) = scanning();
        open(&mut s);
        let (step, out) = send(&mut s, &block_last());
        assert_eq!(step, Step::Close(1002));
        assert!(out.is_empty());
        assert_eq!(*notes.lock().unwrap(), vec![Note::UnexpectedDirection]);
    }

    #[test]
    fn a_block_before_attach_closes_with_1002_as_a_message_before_attach() {
        let (mut s, _p, notes) = scanning();
        let (step, out) = send(&mut s, &block_last());
        assert_eq!(step, Step::Close(1002));
        assert!(out.is_empty());
        assert_eq!(*notes.lock().unwrap(), vec![Note::MessageBeforeAttach]);
    }

    /// A `publish` or a `response` with bytes after a header that carries
    /// no payload, which a consumer would discard, is still a
    /// provider-to-consumer form: the server closes with 1002, as for one
    /// without them.
    #[test]
    fn a_discardable_frame_after_attach_closes_with_1002() {
        let mut publish_init = msg(publish(SIGNAL, 0, 0, PublishState::Init));
        publish_init.push(0);
        let mut response_accepted = msg(Frame::Response {
            target: target(Kind::Command, 3),
            envelope: Envelope {
                stamp: Timestamp(0),
                seq: 1,
            },
            correlation: 1,
            outcome: ResponseOutcome::Accepted,
        });
        response_accepted.push(0);
        for m in [publish_init, response_accepted] {
            let (mut s, _p, notes) = scanning();
            open(&mut s);
            let (step, out) = send(&mut s, &m);
            assert_eq!(step, Step::Close(1002), "{m:?}");
            assert!(out.is_empty(), "{m:?}");
            assert_eq!(
                *notes.lock().unwrap(),
                vec![Note::UnexpectedDirection],
                "{m:?}"
            );
        }
    }

    #[test]
    fn an_unsubscribe_of_a_query_is_dropped_and_recorded() {
        let (mut s, _p, notes) = scanning();
        open(&mut s);
        let t = target(Kind::Query, 4);
        let (step, out) = send(&mut s, &msg(Frame::Unsubscribe(t)));
        assert_eq!(step, Step::Continue);
        assert!(out.is_empty());
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::KindNotAdmitted { target: t }]
        );
    }

    // -- D-13: signals -------------------------------------------------------

    #[test]
    fn subscribing_a_signal_with_no_publication_answers_then_publishes_init_seq_0() {
        let (mut s, _p, _n) = scanning();
        open(&mut s);
        let (step, out) = send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
        assert_eq!(step, Step::Continue);
        assert_eq!(out.len(), 2);
        assert_eq!(decode(&out[0]).unwrap(), accepted(SIGNAL));
        assert_eq!(
            decode(&out[1]).unwrap(),
            publish(SIGNAL, 0, 0, PublishState::Init)
        );
    }

    #[test]
    fn subscribing_a_published_signal_publishes_live_with_its_bytes_and_stamp() {
        let (mut s, mut p, _n) = scanning();
        open(&mut s);
        p.advance(Duration(250));
        commit(&mut p, &[4, 5, 6]);
        let (_, out) = send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
        assert_eq!(out.len(), 2);
        assert_eq!(decode(&out[0]).unwrap(), accepted(SIGNAL));
        assert_eq!(
            decode(&out[1]).unwrap(),
            publish(SIGNAL, 250, 1, PublishState::Live(&[4, 5, 6]))
        );
    }

    #[test]
    fn a_second_subscribe_is_accepted_and_publishes_nothing_more() {
        let (mut s, mut p, _n) = scanning();
        open(&mut s);
        commit(&mut p, &[1]);
        send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
        let (step, out) = send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
        assert_eq!(step, Step::Continue);
        assert_eq!(out.len(), 1);
        assert_eq!(decode(&out[0]).unwrap(), accepted(SIGNAL));
    }

    /// A `read` of a signal that was never published is answered `Init`
    /// with `seq` 0 and leaves no channel state, so reads of many distinct
    /// keys do not grow the session. A read after a publication then counts
    /// from 1 on that channel.
    #[test]
    fn a_read_answered_init_keeps_no_channel_state() {
        let (mut s, mut p, _n) = scanning();
        open(&mut s);
        for ord in 100..200 {
            let t = target(Kind::Signal, ord);
            let (step, out) = send(&mut s, &msg(Frame::Read(t)));
            assert_eq!(step, Step::Continue);
            assert_eq!(out.len(), 1);
            assert_eq!(
                decode(&out[0]).unwrap(),
                publish(t, 0, 0, PublishState::Init)
            );
        }
        let (_, out) = send(&mut s, &msg(Frame::Read(SIGNAL)));
        assert_eq!(
            decode(&out[0]).unwrap(),
            publish(SIGNAL, 0, 0, PublishState::Init)
        );
        assert_eq!(s.signals.len(), 0);
        commit(&mut p, &[1]);
        let (_, out) = send(&mut s, &msg(Frame::Read(SIGNAL)));
        assert_eq!(
            decode(&out[0]).unwrap(),
            publish(SIGNAL, 0, 1, PublishState::Live(&[1]))
        );
        commit(&mut p, &[2]);
        let (_, out) = send(&mut s, &msg(Frame::Read(SIGNAL)));
        assert_eq!(
            decode(&out[0]).unwrap(),
            publish(SIGNAL, 0, 2, PublishState::Live(&[2]))
        );
        assert_eq!(s.signals.len(), 1);
    }

    #[test]
    fn an_invalidation_is_published_as_invalid_declared_with_no_payload() {
        let (mut s, mut p, _n) = scanning();
        open(&mut s);
        commit(&mut p, &[1]);
        send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
        p.advance(Duration(3));
        p.invalidate(IFACE, Ordinal(1)).unwrap();
        p.commit();
        let (_, out) = tick(&mut s);
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            publish(SIGNAL, 3, 2, PublishState::InvalidDeclared)
        );
    }

    #[test]
    fn a_signal_larger_than_the_first_buffer_is_read_again() {
        // The signal's limit is raised to 4,096 bytes in the member table,
        // which the session applies, and on the loopback, which refuses a
        // value above its member's limit on send.
        let mut t = (*table()).clone();
        assert!(t.set_limit(IFACE, SIGNAL.ord, 4096));
        let (mut s, mut p, _n) = described_with(t, |c| c);
        open(&mut s);
        p.set_limit(SIGNAL.iface, SIGNAL.ord, 4096);
        let value = vec![0xAB; 4096];
        commit(&mut p, &value);
        let (_, out) = send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
        assert_eq!(out.len(), 2);
        assert_eq!(
            decode(&out[1]).unwrap(),
            publish(SIGNAL, 0, 1, PublishState::Live(&value))
        );
    }

    #[test]
    fn a_value_larger_than_the_message_limit_is_recorded_and_not_sent() {
        let notes = Notes::default();
        let rt = Loopback::new(CATALOG);
        let mut p = rt.attach();
        let config = config(&notes)
            .with_max_message_bytes(1024)
            .expect("above the minimum");
        let mut s = Session::new(Scanning(rt), &config);
        open(&mut s);
        let value = vec![1; 2000];
        commit(&mut p, &value);
        let (step, out) = send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
        assert_eq!(step, Step::Continue);
        assert_eq!(out.len(), 1);
        assert_eq!(decode(&out[0]).unwrap(), accepted(SIGNAL));
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::ValueTooLarge {
                target: SIGNAL,
                needed: 2000
            }]
        );
        // The signal stays subscribed: a value that fits reaches the client.
        commit(&mut p, &[9]);
        let (_, out) = tick(&mut s);
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            publish(SIGNAL, 0, 1, PublishState::Live(&[9]))
        );
    }

    // -- D-14 and D-24, in both modes ---------------------------------------

    /// Defines one test body as a function generic over the backend and one
    /// `#[test]` per mode calling it.
    macro_rules! both_modes {
        ($name:ident, $scanning:ident, $polling:ident, |$s:ident, $p:ident| $body:block) => {
            fn $name<B: Backend>(mut $s: Session<B>, mut $p: Loopback) {
                let _ = &mut $p;
                $body
            }
            #[test]
            fn $scanning() {
                let (s, p, _n) = scanning();
                $name(s, p);
            }
            #[test]
            fn $polling() {
                let (s, p, _n) = polling();
                $name(s, p);
            }
        };
    }

    both_modes!(
        commit_after_subscribe,
        a_commit_after_subscribe_is_published_on_the_next_tick_scanning,
        a_commit_after_subscribe_is_published_on_the_next_tick_polling,
        |s, p| {
            open(&mut s);
            send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
            let (_, out) = tick(&mut s);
            assert!(out.is_empty());
            p.advance(Duration(7));
            commit(&mut p, &[3]);
            let (step, out) = tick(&mut s);
            assert_eq!(step, Step::Continue);
            assert_eq!(out.len(), 1);
            assert_eq!(
                decode(&out[0]).unwrap(),
                publish(SIGNAL, 7, 1, PublishState::Live(&[3]))
            );
            let (_, out) = tick(&mut s);
            assert!(out.is_empty());
        }
    );

    both_modes!(
        read_does_not_subscribe,
        a_read_publishes_once_and_does_not_subscribe_scanning,
        a_read_publishes_once_and_does_not_subscribe_polling,
        |s, p| {
            open(&mut s);
            commit(&mut p, &[1]);
            let (step, out) = send(&mut s, &msg(Frame::Read(SIGNAL)));
            assert_eq!(step, Step::Continue);
            assert_eq!(out.len(), 1);
            assert_eq!(
                decode(&out[0]).unwrap(),
                publish(SIGNAL, 0, 1, PublishState::Live(&[1]))
            );
            let subscribed = s
                .signals
                .get(&(IFACE, Ordinal(1)))
                .is_some_and(|ch| ch.subscribed);
            assert!(!subscribed);
            commit(&mut p, &[2]);
            let (_, out) = tick(&mut s);
            assert!(out.is_empty());
        }
    );

    both_modes!(
        signal_seq_outlives_subscription,
        a_signal_channel_seq_outlives_its_subscription_scanning,
        a_signal_channel_seq_outlives_its_subscription_polling,
        |s, p| {
            open(&mut s);
            send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
            commit(&mut p, &[1]);
            let (_, out) = tick(&mut s);
            assert_eq!(
                decode(&out[0]).unwrap(),
                publish(SIGNAL, 0, 1, PublishState::Live(&[1]))
            );
            send(&mut s, &msg(Frame::Unsubscribe(SIGNAL)));
            commit(&mut p, &[2]);
            let (_, out) = tick(&mut s);
            assert!(out.is_empty());
            // The counter goes on from 1: the channel's state lasts as long
            // as the session (frame specification §7).
            let (_, out) = send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
            assert_eq!(out.len(), 2);
            assert_eq!(
                decode(&out[1]).unwrap(),
                publish(SIGNAL, 0, 2, PublishState::Live(&[2]))
            );
        }
    );

    /// A `Changed` entry of a signal of a marked interface that the client
    /// did not subscribe is ignored.
    #[test]
    fn a_scan_entry_of_a_signal_that_is_not_subscribed_is_not_published() {
        let (mut s, mut p, _n) = scanning();
        open(&mut s);
        send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
        p.set(IFACE, Ordinal(2), &[2]).unwrap();
        p.commit();
        let (_, out) = tick(&mut s);
        assert!(out.is_empty());
    }

    /// Unsubscribing one signal keeps the interface's mark while a sibling
    /// is still subscribed, so the sibling's changes are still scanned.
    #[test]
    fn the_interface_mark_stays_while_a_sibling_signal_is_subscribed() {
        let (mut s, mut p, _n) = scanning();
        open(&mut s);
        let sibling = target(Kind::Signal, 2);
        send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
        send(&mut s, &msg(Frame::Subscribe(sibling)));
        send(&mut s, &msg(Frame::Unsubscribe(sibling)));
        commit(&mut p, &[5]);
        let (_, out) = tick(&mut s);
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            publish(SIGNAL, 0, 1, PublishState::Live(&[5]))
        );
    }

    both_modes!(
        two_commits_coalesced,
        two_commits_between_ticks_are_coalesced_into_the_newest_scanning,
        two_commits_between_ticks_are_coalesced_into_the_newest_polling,
        |s, p| {
            open(&mut s);
            send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
            commit(&mut p, &[1]);
            commit(&mut p, &[2]);
            let (_, out) = tick(&mut s);
            assert_eq!(out.len(), 1);
            assert_eq!(
                decode(&out[0]).unwrap(),
                publish(SIGNAL, 0, 1, PublishState::Live(&[2]))
            );
        }
    );

    both_modes!(
        unsubscribed_not_published,
        an_unsubscribed_signal_is_not_published_scanning,
        an_unsubscribed_signal_is_not_published_polling,
        |s, p| {
            open(&mut s);
            send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
            let (step, out) = send(&mut s, &msg(Frame::Unsubscribe(SIGNAL)));
            assert_eq!(step, Step::Continue);
            assert!(out.is_empty());
            commit(&mut p, &[1]);
            // Another signal of the interface, never subscribed.
            p.set(IFACE, Ordinal(2), &[2]).unwrap();
            p.commit();
            let (_, out) = tick(&mut s);
            assert!(out.is_empty());
        }
    );

    both_modes!(
        session_seq_from_1,
        session_seq_counts_from_1_per_channel_scanning,
        session_seq_counts_from_1_per_channel_polling,
        |s, p| {
            open(&mut s);
            for i in 1..=5u8 {
                commit(&mut p, &[i]);
            }
            let (_, out) = send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
            assert_eq!(
                decode(&out[1]).unwrap(),
                publish(SIGNAL, 0, 1, PublishState::Live(&[5]))
            );
            commit(&mut p, &[6]);
            let (_, out) = tick(&mut s);
            assert_eq!(
                decode(&out[0]).unwrap(),
                publish(SIGNAL, 0, 2, PublishState::Live(&[6]))
            );
        }
    );

    both_modes!(
        backend_seq_restart,
        a_backend_seq_restart_still_reaches_the_client_scanning,
        a_backend_seq_restart_still_reaches_the_client_polling,
        |s, p| {
            open(&mut s);
            send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
            commit(&mut p, &[1]);
            commit(&mut p, &[2]);
            let (_, out) = tick(&mut s);
            assert_eq!(
                decode(&out[0]).unwrap(),
                publish(SIGNAL, 0, 1, PublishState::Live(&[2]))
            );
            // A new writer on the same store starts its counter again at 1.
            let mut fresh = p.attach();
            drop(p);
            commit(&mut fresh, &[3]);
            let (_, out) = tick(&mut s);
            assert_eq!(out.len(), 1);
            assert_eq!(
                decode(&out[0]).unwrap(),
                publish(SIGNAL, 0, 2, PublishState::Live(&[3]))
            );
        }
    );

    both_modes!(
        read_reuses_seq,
        a_read_of_an_unchanged_signal_reuses_its_session_seq_scanning,
        a_read_of_an_unchanged_signal_reuses_its_session_seq_polling,
        |s, p| {
            open(&mut s);
            commit(&mut p, &[1]);
            let (_, out) = send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
            assert_eq!(
                decode(&out[1]).unwrap(),
                publish(SIGNAL, 0, 1, PublishState::Live(&[1]))
            );
            let (_, out) = send(&mut s, &msg(Frame::Read(SIGNAL)));
            assert_eq!(
                decode(&out[0]).unwrap(),
                publish(SIGNAL, 0, 1, PublishState::Live(&[1]))
            );
            commit(&mut p, &[2]);
            let (_, out) = send(&mut s, &msg(Frame::Read(SIGNAL)));
            assert_eq!(
                decode(&out[0]).unwrap(),
                publish(SIGNAL, 0, 2, PublishState::Live(&[2]))
            );
        }
    );

    /// After a `read` sent a change, the scan still reports it, and the
    /// scanning tick sends it again under the next `seq`.
    #[test]
    fn a_change_a_read_sent_is_sent_again_by_the_scan() {
        let (mut s, mut p, _n) = scanning();
        open(&mut s);
        send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
        commit(&mut p, &[2]);
        send(&mut s, &msg(Frame::Read(SIGNAL)));
        let (_, out) = tick(&mut s);
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            publish(SIGNAL, 0, 2, PublishState::Live(&[2]))
        );
    }

    /// After a `read` sent a change, the polling tick finds the sample equal
    /// to the one sent and sends nothing.
    #[test]
    fn a_change_a_read_sent_is_not_sent_again_by_polling() {
        let (mut s, mut p, _n) = polling();
        open(&mut s);
        send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
        commit(&mut p, &[2]);
        send(&mut s, &msg(Frame::Read(SIGNAL)));
        let (_, out) = tick(&mut s);
        assert!(out.is_empty());
    }

    both_modes!(
        new_writer_reuses_seq,
        a_new_writer_whose_first_commit_reuses_the_last_seq_is_still_published_scanning,
        a_new_writer_whose_first_commit_reuses_the_last_seq_is_still_published_polling,
        |s, p| {
            open(&mut s);
            send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
            commit(&mut p, &[1]);
            let (_, out) = tick(&mut s);
            assert_eq!(
                decode(&out[0]).unwrap(),
                publish(SIGNAL, 0, 1, PublishState::Live(&[1]))
            );
            // The new writer's first commit is backend seq 1 again, under
            // the same stamp: only the bytes differ.
            let mut fresh = p.attach();
            drop(p);
            commit(&mut fresh, &[3]);
            let (_, out) = tick(&mut s);
            assert_eq!(out.len(), 1);
            assert_eq!(
                decode(&out[0]).unwrap(),
                publish(SIGNAL, 0, 2, PublishState::Live(&[3]))
            );
        }
    );

    /// The stamp half of the polling comparison: the same backend `seq`,
    /// provenance and bytes under a later stamp is a change.
    #[test]
    fn polling_publishes_a_new_stamp_under_the_same_seq_and_bytes() {
        let (mut s, mut p, _n) = polling();
        open(&mut s);
        send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
        commit(&mut p, &[1]);
        tick(&mut s);
        let mut fresh = p.attach();
        drop(p);
        fresh.advance(Duration(5));
        commit(&mut fresh, &[1]);
        let (_, out) = tick(&mut s);
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            publish(SIGNAL, 5, 2, PublishState::Live(&[1]))
        );
    }

    /// The backend-`seq` half of the polling comparison: a `touch` committed
    /// with the clock held has the same stamp, provenance and bytes, and a
    /// new backend `seq`, and is a change.
    #[test]
    fn polling_publishes_a_touch_under_the_same_stamp_and_bytes() {
        let (mut s, mut p, _n) = polling();
        open(&mut s);
        send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
        commit(&mut p, &[1]);
        let (_, out) = tick(&mut s);
        assert_eq!(
            decode(&out[0]).unwrap(),
            publish(SIGNAL, 0, 1, PublishState::Live(&[1]))
        );
        p.touch(IFACE, Ordinal(1)).unwrap();
        p.commit();
        let (_, out) = tick(&mut s);
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            publish(SIGNAL, 0, 2, PublishState::Live(&[1]))
        );
    }

    /// A reader whose sample is fixed but for its provenance, which the test
    /// changes from outside. Every other port delegates to a loopback.
    struct Fixed {
        inner: Loopback,
        provenance: Rc<Cell<Provenance>>,
    }

    impl Attached for Fixed {
        fn catalog(&self) -> &CatalogRef {
            self.inner.catalog()
        }
    }

    impl Encoded for Fixed {
        type Encoding = <Loopback as Encoded>::Encoding;
    }

    impl Clock for Fixed {
        fn now(&self) -> Timestamp {
            self.inner.now()
        }
    }

    impl SignalReader for Fixed {
        fn read(&self, _: InterfaceNo, _: Ordinal, out: &mut [u8]) -> Result<RawSample, ReadError> {
            out[0] = 1;
            Ok(RawSample {
                provenance: self.provenance.get(),
                freshness: Freshness::Unbounded,
                envelope: Envelope {
                    stamp: Timestamp(0),
                    seq: 1,
                },
                len: 1,
            })
        }
    }

    impl EventSource for Fixed {
        fn subscribe(
            &mut self,
            iface: InterfaceNo,
            ords: &[Ordinal],
        ) -> Result<(), SubscribeError> {
            self.inner.subscribe(iface, ords)
        }
        fn unsubscribe(&mut self, iface: InterfaceNo, ords: &[Ordinal]) {
            self.inner.unsubscribe(iface, ords);
        }
        fn next(&mut self, out: &mut [u8]) -> Result<Option<RawOccurrence>, ReadError> {
            self.inner.next(out)
        }
    }

    impl Caller for Fixed {
        fn command(
            &mut self,
            iface: InterfaceNo,
            ord: Ordinal,
            args: &[u8],
            trace: Option<TraceContext>,
        ) -> Result<Correlation, SendError> {
            self.inner.command(iface, ord, args, trace)
        }
        fn query(
            &mut self,
            iface: InterfaceNo,
            ord: Ordinal,
            args: &[u8],
            trace: Option<TraceContext>,
        ) -> Result<Correlation, SendError> {
            self.inner.query(iface, ord, args, trace)
        }
        fn ack(&mut self, c: Correlation) -> Option<Result<(), CallError>> {
            self.inner.ack(c)
        }
        fn reply(
            &mut self,
            c: Correlation,
            out: &mut [u8],
        ) -> Result<Option<Result<usize, CallError>>, ReadError> {
            self.inner.reply(c, out)
        }
        fn forget(&mut self, c: Correlation) {
            self.inner.forget(c);
        }
    }

    /// The provenance half of the polling comparison: the same backend
    /// `seq`, stamp and bytes under another provenance is a change.
    #[test]
    fn polling_publishes_a_provenance_change_under_the_same_seq_stamp_and_bytes() {
        let provenance = Rc::new(Cell::new(Provenance::Live));
        let backend = Polling(Fixed {
            inner: Loopback::new(CATALOG),
            provenance: Rc::clone(&provenance),
        });
        let mut s = Session::new(backend, &Config::default());
        open(&mut s);
        let (_, out) = send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
        assert_eq!(
            decode(&out[1]).unwrap(),
            publish(SIGNAL, 0, 1, PublishState::Live(&[1]))
        );
        let (_, out) = tick(&mut s);
        assert!(out.is_empty());
        provenance.set(Provenance::Invalid(Cause::Declared));
        let (_, out) = tick(&mut s);
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            publish(SIGNAL, 0, 2, PublishState::InvalidDeclared)
        );
    }

    /// `Invalid(Detected(_))` never crosses (frame specification §9.6): it
    /// is sent as `InvalidDeclared`, with no payload.
    #[test]
    fn a_detected_invalidity_is_published_as_invalid_declared_with_no_payload() {
        let provenance = Rc::new(Cell::new(Provenance::Invalid(Cause::Detected(
            Detection::Corrupt,
        ))));
        let backend = Polling(Fixed {
            inner: Loopback::new(CATALOG),
            provenance: Rc::clone(&provenance),
        });
        let mut s = Session::new(backend, &Config::default());
        open(&mut s);
        let (_, out) = send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
        assert_eq!(out.len(), 2);
        assert_eq!(
            decode(&out[1]).unwrap(),
            publish(SIGNAL, 0, 1, PublishState::InvalidDeclared)
        );
        assert_eq!(out[1].len(), PUBLISH_HEADER);
    }

    /// One commit of more signals than the scan buffer holds: the buffer
    /// grows and every subscribed one is published.
    #[test]
    fn more_changed_signals_than_the_first_scan_buffer_are_all_published() {
        let (mut s, mut p, _n) = scanning();
        open(&mut s);
        let count = 20u32;
        for ord in 1..=count {
            send(&mut s, &msg(Frame::Subscribe(target(Kind::Signal, ord))));
        }
        for ord in 1..=count {
            p.set(IFACE, Ordinal(ord), &[ord as u8]).unwrap();
        }
        p.commit();
        let (_, out) = tick(&mut s);
        assert_eq!(out.len(), count as usize);
        let mut seen: Vec<u32> = out
            .iter()
            .map(|m| match decode(m).unwrap() {
                Frame::Publish {
                    target,
                    envelope,
                    state: PublishState::Live(bytes),
                } => {
                    assert_eq!(envelope.seq, 1);
                    assert_eq!(bytes, &[target.ord.0 as u8]);
                    target.ord.0
                }
                other => panic!("not a live publish: {other:?}"),
            })
            .collect();
        seen.sort_unstable();
        assert_eq!(seen, (1..=count).collect::<Vec<_>>());
        let (_, out) = tick(&mut s);
        assert!(out.is_empty());
    }

    /// Each attach has every field wrong from one check on, so that the
    /// reason given is the first check's.
    #[test]
    fn the_attach_checks_run_in_the_design_order() {
        let cases = [
            (2, 2, [8u8; 32], "ws.other", Reason::FrameVersionUnsupported),
            (1, 2, [8u8; 32], "ws.other", Reason::UnknownCatalog),
            (1, 2, [8u8; 32], "ws.test", Reason::CatalogMismatch),
            (1, 2, [7u8; 32], "ws.test", Reason::EncodingUnsupported),
        ];
        for (version, encoding, hash, name, reason) in cases {
            let (mut s, _p, _n) = scanning();
            refused(&mut s, &attach_msg(version, encoding, hash, name), reason);
        }
    }

    /// A value too large at subscribe is recorded once, not again by the
    /// polling ticks that read it again.
    #[test]
    fn a_too_large_value_is_recorded_once_across_subscribe_and_polling_ticks() {
        let notes = Notes::default();
        let rt = Loopback::new(CATALOG);
        let mut p = rt.attach();
        let config = config(&notes)
            .with_max_message_bytes(1024)
            .expect("above the minimum");
        let mut s = Session::new(Polling(rt), &config);
        open(&mut s);
        commit(&mut p, &[1; 2000]);
        send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
        tick(&mut s);
        tick(&mut s);
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::ValueTooLarge {
                target: SIGNAL,
                needed: 2000
            }]
        );
    }

    /// A value that fits resets the record, so a value too large again
    /// afterwards, of the same size, is recorded again.
    #[test]
    fn a_value_too_large_again_after_one_that_fit_is_recorded_again() {
        let (mut s, mut p, notes) = limited();
        open(&mut s);
        commit(&mut p, &[1; 2000]);
        send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
        commit(&mut p, &[9]);
        let (_, out) = tick(&mut s);
        assert_eq!(out.len(), 1);
        commit(&mut p, &[2; 2000]);
        let (_, out) = tick(&mut s);
        assert!(out.is_empty());
        let too_large = Note::ValueTooLarge {
            target: SIGNAL,
            needed: 2000,
        };
        assert_eq!(*notes.lock().unwrap(), vec![too_large, too_large]);
    }

    #[test]
    fn after_a_close_nothing_more_is_sent_and_the_close_is_repeated() {
        let (mut s, mut p, _n) = scanning();
        open(&mut s);
        send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
        let (step, _) = send(
            &mut s,
            &msg(publish(SIGNAL, 0, 1, PublishState::Live(&[1]))),
        );
        assert_eq!(step, Step::Close(1002));
        let (step, out) = send(&mut s, &msg(Frame::Read(SIGNAL)));
        assert_eq!(step, Step::Close(1002));
        assert!(out.is_empty());
        commit(&mut p, &[2]);
        let (step, out) = tick(&mut s);
        assert_eq!(step, Step::Close(1002));
        assert!(out.is_empty());
    }

    #[test]
    fn after_a_refusal_the_close_is_repeated() {
        let (mut s, _p, _n) = scanning();
        send(&mut s, &attach_msg(1, 1, [7u8; 32], "ws.other"));
        let (step, out) = send(&mut s, &attach_msg(1, 1, [7u8; 32], "ws.test"));
        assert_eq!(step, Step::Close(1000));
        assert!(out.is_empty());
    }

    // -- D-17: fixed ---------------------------------------------------------

    #[test]
    fn a_read_of_a_fixed_publishes_kind_fixed_seq_0_live() {
        let (mut s, mut p, _n) = scanning();
        open(&mut s);
        p.provision_fixed(IFACE, Ordinal(5), &[0xF1, 0xF2])
            .expect("the loopback stores the fixed");
        p.advance(Duration(11));
        let (step, out) = send(&mut s, &msg(Frame::Read(FIXED)));
        assert_eq!(step, Step::Continue);
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            publish(FIXED, 11, 0, PublishState::Live(&[0xF1, 0xF2]))
        );
    }

    #[test]
    fn a_read_of_an_unprovisioned_fixed_is_unknown_interaction() {
        let (mut s, _p, _n) = scanning();
        open(&mut s);
        let (_, out) = send(&mut s, &msg(Frame::Read(FIXED)));
        assert_eq!(out.len(), 1);
        assert_eq!(decode(&out[0]).unwrap(), unknown(FIXED));
    }

    #[test]
    fn a_read_of_a_fixed_through_polling_is_unknown_interaction() {
        let (mut s, mut p, _n) = polling();
        open(&mut s);
        p.provision_fixed(IFACE, Ordinal(5), &[1])
            .expect("the loopback stores the fixed");
        let (_, out) = send(&mut s, &msg(Frame::Read(FIXED)));
        assert_eq!(out.len(), 1);
        assert_eq!(decode(&out[0]).unwrap(), unknown(FIXED));
    }

    #[test]
    fn a_subscribe_of_a_fixed_is_unknown_interaction() {
        let (mut s, mut p, _n) = scanning();
        open(&mut s);
        p.provision_fixed(IFACE, Ordinal(5), &[1])
            .expect("the loopback stores the fixed");
        let (_, out) = send(&mut s, &msg(Frame::Subscribe(FIXED)));
        assert_eq!(out.len(), 1);
        assert_eq!(decode(&out[0]).unwrap(), unknown(FIXED));
    }

    // -- D-15: events --------------------------------------------------------

    const EVENT: Target = Target {
        kind: Kind::Event,
        iface: IFACE,
        ord: Ordinal(2),
    };
    const EVENT_B: Target = Target {
        kind: Kind::Event,
        iface: IFACE,
        ord: Ordinal(3),
    };

    fn raise(p: &mut Loopback, target: Target, bytes: &[u8]) {
        p.raise(target.iface, target.ord, bytes, None).unwrap();
    }

    fn occurrence(target: Target, stamp: i64, seq: u64, payload: &[u8]) -> Frame<'_> {
        Frame::Occurrence {
            target,
            envelope: Envelope {
                stamp: Timestamp(stamp),
                seq,
            },
            payload,
        }
    }

    #[test]
    fn an_event_subscribe_is_answered_accepted() {
        let (mut s, _p, _n) = scanning();
        open(&mut s);
        let (step, out) = send(&mut s, &msg(Frame::Subscribe(EVENT)));
        assert_eq!(step, Step::Continue);
        assert_eq!(out.len(), 1);
        assert_eq!(decode(&out[0]).unwrap(), accepted(EVENT));
    }

    #[test]
    fn an_occurrence_raised_after_subscribe_is_sent_on_the_next_tick() {
        let (mut s, mut p, _n) = scanning();
        open(&mut s);
        send(&mut s, &msg(Frame::Subscribe(EVENT)));
        p.advance(Duration(9));
        raise(&mut p, EVENT, &[1, 2]);
        let (step, out) = tick(&mut s);
        assert_eq!(step, Step::Continue);
        assert_eq!(out.len(), 1);
        assert_eq!(decode(&out[0]).unwrap(), occurrence(EVENT, 9, 1, &[1, 2]));
        let (_, out) = tick(&mut s);
        assert!(out.is_empty());
    }

    #[test]
    fn an_occurrence_raised_before_subscribe_is_never_sent() {
        let (mut s, mut p, _n) = scanning();
        open(&mut s);
        raise(&mut p, EVENT, &[1]);
        send(&mut s, &msg(Frame::Subscribe(EVENT)));
        let (_, out) = tick(&mut s);
        assert!(out.is_empty());
        // The backend's counter is at 2; the session's starts at 1.
        raise(&mut p, EVENT, &[2]);
        let (_, out) = tick(&mut s);
        assert_eq!(out.len(), 1);
        assert_eq!(decode(&out[0]).unwrap(), occurrence(EVENT, 0, 1, &[2]));
    }

    #[test]
    fn occurrences_keep_their_order_and_count_session_seq_from_1() {
        let (mut s, mut p, _n) = scanning();
        open(&mut s);
        send(&mut s, &msg(Frame::Subscribe(EVENT)));
        send(&mut s, &msg(Frame::Subscribe(EVENT_B)));
        raise(&mut p, EVENT, &[1]);
        raise(&mut p, EVENT_B, &[10]);
        raise(&mut p, EVENT, &[2]);
        raise(&mut p, EVENT, &[3]);
        let (_, out) = tick(&mut s);
        let frames: Vec<Frame<'_>> = out.iter().map(|m| decode(m).unwrap()).collect();
        assert_eq!(
            frames,
            vec![
                occurrence(EVENT, 0, 1, &[1]),
                occurrence(EVENT_B, 0, 1, &[10]),
                occurrence(EVENT, 0, 2, &[2]),
                occurrence(EVENT, 0, 3, &[3]),
            ]
        );
    }

    #[test]
    fn unsubscribe_stops_delivery() {
        let (mut s, mut p, _n) = scanning();
        open(&mut s);
        send(&mut s, &msg(Frame::Subscribe(EVENT)));
        raise(&mut p, EVENT, &[1]);
        let (_, out) = tick(&mut s);
        assert_eq!(out.len(), 1);
        let (step, out) = send(&mut s, &msg(Frame::Unsubscribe(EVENT)));
        assert_eq!(step, Step::Continue);
        assert!(out.is_empty());
        raise(&mut p, EVENT, &[2]);
        let (_, out) = tick(&mut s);
        assert!(out.is_empty());
        // The channel's session counter outlives the subscription.
        send(&mut s, &msg(Frame::Subscribe(EVENT)));
        raise(&mut p, EVENT, &[3]);
        let (_, out) = tick(&mut s);
        assert_eq!(out.len(), 1);
        assert_eq!(decode(&out[0]).unwrap(), occurrence(EVENT, 0, 2, &[3]));
    }

    #[test]
    fn a_large_occurrence_is_read_again() {
        // As in `a_signal_larger_than_the_first_buffer_is_read_again`: the
        // event's limit is raised in the table and on the loopback.
        let mut t = (*table()).clone();
        assert!(t.set_limit(IFACE, EVENT.ord, 4096));
        let (mut s, mut p, _n) = described_with(t, |c| c);
        open(&mut s);
        send(&mut s, &msg(Frame::Subscribe(EVENT)));
        p.set_limit(EVENT.iface, EVENT.ord, 4096);
        let value = vec![0xCD; 4096];
        raise(&mut p, EVENT, &value);
        let (_, out) = tick(&mut s);
        assert_eq!(out.len(), 1);
        assert_eq!(decode(&out[0]).unwrap(), occurrence(EVENT, 0, 1, &value));
    }

    /// An occurrence that does not fit one message is recorded and dropped,
    /// and the ones behind it in the queue still reach the client. The
    /// channel's `seq` advances for the dropped one, so the client sees the
    /// gap.
    #[test]
    fn an_occurrence_larger_than_the_message_limit_is_recorded_and_not_sent() {
        let notes = Notes::default();
        let rt = Loopback::new(CATALOG);
        let mut p = rt.attach();
        let config = config(&notes)
            .with_max_message_bytes(1024)
            .expect("above the minimum");
        let mut s = Session::new(Scanning(rt), &config);
        open(&mut s);
        send(&mut s, &msg(Frame::Subscribe(EVENT)));
        raise(&mut p, EVENT, &[1; 2000]);
        raise(&mut p, EVENT, &[1]);
        let (step, out) = tick(&mut s);
        assert_eq!(step, Step::Continue);
        assert_eq!(out.len(), 1);
        assert_eq!(decode(&out[0]).unwrap(), occurrence(EVENT, 0, 2, &[1]));
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::ValueTooLarge {
                target: EVENT,
                needed: 2000
            }]
        );
    }

    // -- Bounded work per tick ------------------------------------------------

    /// The sequence numbers of the occurrences in `out`, in order.
    fn occurrence_seqs(out: &[Vec<u8>]) -> Vec<u64> {
        out.iter()
            .map(|m| match decode(m).unwrap() {
                Frame::Occurrence { envelope, .. } => envelope.seq,
                other => panic!("not an occurrence: {other:?}"),
            })
            .collect()
    }

    #[test]
    fn a_tick_sends_at_most_64_occurrences_and_the_rest_on_the_next_tick() {
        let (mut s, mut p, _n) = scanning();
        open(&mut s);
        send(&mut s, &msg(Frame::Subscribe(EVENT)));
        for i in 0..100u8 {
            raise(&mut p, EVENT, &[i]);
        }
        let (step, out) = tick(&mut s);
        assert_eq!(step, Step::Continue);
        assert_eq!(occurrence_seqs(&out), (1..=64).collect::<Vec<_>>());
        let (_, out) = tick(&mut s);
        assert_eq!(occurrence_seqs(&out), (65..=100).collect::<Vec<_>>());
        let (_, out) = tick(&mut s);
        assert!(out.is_empty());
    }

    /// Two occurrences whose frames total exactly the message limit stop the
    /// tick: the limit is reached, so the third waits.
    #[test]
    fn a_tick_stops_draining_once_it_has_sent_a_message_limit_of_occurrences() {
        let (mut s, mut p, _n) = limited();
        open(&mut s);
        send(&mut s, &msg(Frame::Subscribe(EVENT)));
        let header = occurrence(EVENT, 0, 0, &[]).encoded_len();
        let first = occurrence_cap() / 2;
        let second = LIMIT - (header + first) - header;
        raise(&mut p, EVENT, &vec![1; first]);
        raise(&mut p, EVENT, &vec![2; second]);
        raise(&mut p, EVENT, &[3]);
        let (_, out) = tick(&mut s);
        assert_eq!(occurrence_seqs(&out), vec![1, 2]);
        assert_eq!(out[0].len() + out[1].len(), LIMIT);
        let (_, out) = tick(&mut s);
        assert_eq!(occurrence_seqs(&out), vec![3]);
        let (_, out) = tick(&mut s);
        assert!(out.is_empty());
    }

    /// A dropped occurrence counts against the budget of 64 as a read one
    /// does, so a provider that raises oversized occurrences cannot hold the
    /// tick either.
    #[test]
    fn a_dropped_occurrence_counts_against_the_budget_of_a_tick() {
        let (mut s, mut p, notes) = limited();
        open(&mut s);
        send(&mut s, &msg(Frame::Subscribe(EVENT)));
        for _ in 0..100 {
            raise(&mut p, EVENT, &[1; 2000]);
        }
        raise(&mut p, EVENT, &[9]);
        let (_, out) = tick(&mut s);
        assert!(out.is_empty());
        assert_eq!(notes.lock().unwrap().len(), 64);
        let (_, out) = tick(&mut s);
        assert_eq!(occurrence_seqs(&out), vec![101]);
        assert_eq!(notes.lock().unwrap().len(), 100);
    }

    /// One tick with a changed signal, a waiting occurrence and a settled
    /// call sends them in that order.
    #[test]
    fn a_tick_sends_signals_then_occurrences_then_responses() {
        let (mut s, mut p, _n) = scanning();
        open(&mut s);
        send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
        send(&mut s, &msg(Frame::Subscribe(EVENT)));
        let (_, out) = send(&mut s, &request(COMMAND, 1, &[]));
        assert!(out.is_empty());
        let c = claim(&mut p, COMMAND);
        p.settle(c.id, Ok(&[])).unwrap();
        raise(&mut p, EVENT, &[2]);
        commit(&mut p, &[1]);
        let (_, out) = tick(&mut s);
        let frames: Vec<Frame<'_>> = out.iter().map(|m| decode(m).unwrap()).collect();
        assert_eq!(
            frames,
            vec![
                publish(SIGNAL, 0, 1, PublishState::Live(&[1])),
                occurrence(EVENT, 0, 1, &[2]),
                response(COMMAND, 0, 1, 1, ResponseOutcome::Accepted),
            ]
        );
    }

    /// A scanning backend that counts its `scan` calls and, when asked,
    /// reports the first changed entry twice.
    struct Counted {
        inner: Loopback,
        scans: Cell<usize>,
        duplicate: bool,
    }

    impl Backend for Counted {
        type Ports = Loopback;

        fn ports(&mut self) -> &mut Loopback {
            &mut self.inner
        }

        fn scannable(&self) -> Option<&dyn ScannableSignals> {
            Some(self)
        }
    }

    impl Attached for Counted {
        fn catalog(&self) -> &CatalogRef {
            self.inner.catalog()
        }
    }

    impl SignalReader for Counted {
        fn read(
            &self,
            iface: InterfaceNo,
            ord: Ordinal,
            out: &mut [u8],
        ) -> Result<RawSample, ReadError> {
            self.inner.read(iface, ord, out)
        }
    }

    impl ScannableSignals for Counted {
        fn generation(&self, iface: InterfaceNo) -> u64 {
            self.inner.generation(iface)
        }

        fn scan(&self, marks: &mut [Watermark], out: &mut [Changed]) -> usize {
            self.scans.set(self.scans.get() + 1);
            let n = self.inner.scan(marks, out);
            if self.duplicate && n > 0 && n < out.len() {
                out[n] = out[0];
                return n + 1;
            }
            n
        }
    }

    fn counted(duplicate: bool) -> (Session<Counted>, Loopback) {
        let rt = Loopback::new(CATALOG);
        let provider = rt.attach();
        let backend = Counted {
            inner: rt,
            scans: Cell::new(0),
            duplicate,
        };
        (Session::new(backend, &Config::default()), provider)
    }

    #[test]
    fn a_tick_scans_once_when_the_scan_buffer_holds_the_changes() {
        let (mut s, mut p) = counted(false);
        open(&mut s);
        send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
        commit(&mut p, &[1]);
        s.backend.scans.set(0);
        let (_, out) = tick(&mut s);
        assert_eq!(out.len(), 1);
        assert_eq!(s.backend.scans.get(), 1);
        let (_, out) = tick(&mut s);
        assert!(out.is_empty());
        assert_eq!(s.backend.scans.get(), 2);
    }

    #[test]
    fn a_key_a_scan_reports_twice_is_published_once_per_tick() {
        let (mut s, mut p) = counted(true);
        open(&mut s);
        send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
        commit(&mut p, &[1]);
        let (_, out) = tick(&mut s);
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            publish(SIGNAL, 0, 1, PublishState::Live(&[1]))
        );
    }

    // -- D-16: calls ---------------------------------------------------------

    const COMMAND: Target = Target {
        kind: Kind::Command,
        iface: IFACE,
        ord: Ordinal(3),
    };
    const QUERY: Target = Target {
        kind: Kind::Query,
        iface: IFACE,
        ord: Ordinal(4),
    };

    fn request(target: Target, seq: u64, payload: &[u8]) -> Vec<u8> {
        request_at(target, seq, 0, payload)
    }

    /// The same, with the request stamped at `stamp`.
    fn request_at(target: Target, seq: u64, stamp: i64, payload: &[u8]) -> Vec<u8> {
        msg(Frame::Request {
            target,
            envelope: Envelope {
                stamp: Timestamp(stamp),
                seq,
            },
            payload,
        })
    }

    fn response(
        target: Target,
        stamp: i64,
        seq: u64,
        correlation: u64,
        outcome: ResponseOutcome<'_>,
    ) -> Frame<'_> {
        Frame::Response {
            target,
            envelope: Envelope {
                stamp: Timestamp(stamp),
                seq,
            },
            correlation,
            outcome,
        }
    }

    /// Makes the provider serve the member and takes the next claim on it.
    fn claim(p: &mut Loopback, target: Target) -> Claim {
        p.serve(target.iface, &[target.ord]).unwrap();
        let mut buf = [0; 64];
        p.next_claim(&mut buf)
            .expect("a claim is waiting")
            .expect("a claim is waiting")
    }

    /// Sends a request, settles it on the provider, and returns the frames
    /// of the next tick.
    fn settled<B: Backend>(
        s: &mut Session<B>,
        p: &mut Loopback,
        target: Target,
        seq: u64,
        outcome: Result<&[u8], CallError>,
    ) -> Vec<Vec<u8>> {
        let (_, out) = send(s, &request(target, seq, &[7]));
        assert!(out.is_empty());
        let c = claim(p, target);
        p.settle(c.id, outcome).unwrap();
        tick(s).1
    }

    #[test]
    fn a_command_request_is_answered_accepted_with_the_client_seq_as_correlation() {
        let (mut s, mut p, _n) = scanning();
        open(&mut s);
        let (step, out) = send(&mut s, &request(COMMAND, 5, &[1, 2]));
        assert_eq!(step, Step::Continue);
        assert!(out.is_empty());
        // Nothing is settled yet: the tick sends nothing.
        let (_, out) = tick(&mut s);
        assert!(out.is_empty());
        let c = claim(&mut p, COMMAND);
        assert_eq!((c.iface, c.ord, c.len), (IFACE, Ordinal(3), 2));
        assert_eq!(c.trace, None);
        p.settle(c.id, Ok(&[])).unwrap();
        p.advance(Duration(40));
        let (step, out) = tick(&mut s);
        assert_eq!(step, Step::Continue);
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            response(COMMAND, 40, 1, 5, ResponseOutcome::Accepted)
        );
        let (_, out) = tick(&mut s);
        assert!(out.is_empty());
    }

    #[test]
    fn a_query_request_is_answered_reply_with_the_reply_bytes() {
        let (mut s, mut p, _n) = scanning();
        open(&mut s);
        let out = settled(&mut s, &mut p, QUERY, 1, Ok(&[8, 9]));
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            response(QUERY, 0, 1, 1, ResponseOutcome::Reply(&[8, 9]))
        );
    }

    #[test]
    fn a_contract_settlement_crosses_as_contract_with_its_rule() {
        let (mut s, mut p, _n) = scanning();
        open(&mut s);
        let violation = Contract::InvalidValue(Violation {
            type_name: Some("Speed"),
            rule: Rule::Range,
        });
        let out = settled(
            &mut s,
            &mut p,
            QUERY,
            1,
            Err(CallError::Contract(violation)),
        );
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            response(
                QUERY,
                0,
                1,
                1,
                ResponseOutcome::Contract(Contract::InvalidValue(Violation {
                    type_name: None,
                    rule: Rule::Range,
                }))
            )
        );
    }

    #[test]
    fn a_corrupt_settlement_crosses_as_corrupt() {
        let (mut s, mut p, _n) = scanning();
        open(&mut s);
        let out = settled(
            &mut s,
            &mut p,
            COMMAND,
            1,
            Err(CallError::Transport(Transport::Corrupt)),
        );
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            response(COMMAND, 0, 1, 1, ResponseOutcome::Corrupt)
        );
    }

    #[test]
    fn a_busy_settlement_crosses_as_busy() {
        let (mut s, mut p, _n) = scanning();
        open(&mut s);
        let out = settled(
            &mut s,
            &mut p,
            QUERY,
            1,
            Err(CallError::Transport(Transport::Busy)),
        );
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            response(QUERY, 0, 1, 1, ResponseOutcome::Busy)
        );
    }

    /// Sends `count` command requests from `first_seq` and returns the frames
    /// they drew.
    fn send_commands<B: Backend>(s: &mut Session<B>, first_seq: u64, count: u64) -> Vec<Vec<u8>> {
        let mut all = Vec::new();
        for seq in first_seq..first_seq + count {
            let (step, out) = send(s, &request(COMMAND, seq, &[]));
            assert_eq!(step, Step::Continue);
            all.extend(out);
        }
        all
    }

    /// A timeout settlement sends nothing, and the call is forgotten on the
    /// backend: its slot takes a new call.
    #[test]
    fn a_timeout_settlement_sends_nothing_and_frees_the_slot() {
        let (mut s, mut p, _n) = scanning();
        open(&mut s);
        let out = settled(
            &mut s,
            &mut p,
            COMMAND,
            1,
            Err(CallError::Transport(Transport::Timeout)),
        );
        assert!(out.is_empty());
        assert!(send_commands(&mut s, 2, Loopback::SLOTS as u64).is_empty());
    }

    #[test]
    fn a_full_call_table_answers_busy_at_once() {
        let (mut s, _p, _n) = scanning();
        open(&mut s);
        assert!(send_commands(&mut s, 1, Loopback::SLOTS as u64).is_empty());
        let (step, out) = send(&mut s, &request(COMMAND, 17, &[]));
        assert_eq!(step, Step::Continue);
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            response(COMMAND, 0, 1, 17, ResponseOutcome::Busy)
        );
    }

    #[test]
    fn response_seq_counts_from_1_per_session() {
        let (mut s, mut p, _n) = scanning();
        open(&mut s);
        let out = settled(&mut s, &mut p, COMMAND, 1, Ok(&[]));
        assert_eq!(
            decode(&out[0]).unwrap(),
            response(COMMAND, 0, 1, 1, ResponseOutcome::Accepted)
        );
        let out = settled(&mut s, &mut p, QUERY, 2, Ok(&[]));
        assert_eq!(
            decode(&out[0]).unwrap(),
            response(QUERY, 0, 2, 2, ResponseOutcome::Reply(&[]))
        );
    }

    #[test]
    fn a_duplicate_seq_in_flight_is_not_sent_again() {
        let (mut s, mut p, notes) = scanning();
        open(&mut s);
        send(&mut s, &request(COMMAND, 1, &[]));
        let (step, out) = send(&mut s, &request(COMMAND, 1, &[]));
        assert_eq!(step, Step::Continue);
        assert!(out.is_empty());
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::DuplicateDropped { seq: 1 }]
        );
        claim(&mut p, COMMAND);
        let mut buf = [0; 64];
        assert_eq!(p.next_claim(&mut buf).unwrap(), None);
    }

    #[test]
    fn a_duplicate_seq_already_settled_gets_the_same_response_again() {
        let (mut s, mut p, notes) = scanning();
        open(&mut s);
        let out = settled(&mut s, &mut p, QUERY, 1, Ok(&[3]));
        assert_eq!(out.len(), 1);
        p.advance(Duration(50));
        let (step, again) = send(&mut s, &request(QUERY, 1, &[7]));
        assert_eq!(step, Step::Continue);
        // The same bytes: the same response seq and stamp as the first time.
        assert_eq!(again, out);
        assert!(notes.lock().unwrap().is_empty());
        // The repeat did not advance the response counter.
        let out = settled(&mut s, &mut p, QUERY, 2, Ok(&[]));
        assert_eq!(
            decode(&out[0]).unwrap(),
            response(QUERY, 50, 2, 2, ResponseOutcome::Reply(&[]))
        );
    }

    #[test]
    fn an_older_unknown_seq_is_recorded_and_dropped() {
        let (mut s, mut p, notes) = scanning();
        open(&mut s);
        send(&mut s, &request(COMMAND, 5, &[5]));
        let (step, out) = send(&mut s, &request(COMMAND, 3, &[3]));
        assert_eq!(step, Step::Continue);
        assert!(out.is_empty());
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::DuplicateDropped { seq: 3 }]
        );
        // The one call the provider sees is the first: the loopback stamps
        // its own caller `seq` on a claim, so the arguments identify it.
        p.serve(IFACE, &[Ordinal(3)]).unwrap();
        let mut buf = [0; 64];
        let c = p.next_claim(&mut buf).unwrap().expect("the first call");
        assert_eq!(&buf[..c.len], &[5]);
        assert_eq!(p.next_claim(&mut buf).unwrap(), None);
    }

    /// Sixty-five responses settled at once, through requests of a kind the
    /// form does not admit: the first leaves the retention, the second stays.
    #[test]
    fn the_retention_of_settled_responses_holds_the_last_64() {
        let (mut s, _p, notes) = scanning();
        open(&mut s);
        let mut responses = Vec::new();
        for seq in 1..=65 {
            let (_, out) = send(&mut s, &request(SIGNAL, seq, &[]));
            assert_eq!(out.len(), 1);
            responses.push(out.into_iter().next().unwrap());
        }
        let (_, out) = send(&mut s, &request(SIGNAL, 1, &[]));
        assert!(out.is_empty());
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::DuplicateDropped { seq: 1 }]
        );
        let (_, out) = send(&mut s, &request(SIGNAL, 2, &[]));
        assert_eq!(out, vec![responses[1].clone()]);
    }

    #[test]
    fn a_call_unsettled_after_call_abandon_after_is_forgotten_and_answered_nothing() {
        let (mut s, _p, notes) = scanning();
        open(&mut s);
        let t0 = now();
        let mut out = Vec::new();
        assert_eq!(
            s.on_message(&request(COMMAND, 1, &[]), t0, &mut out),
            Step::Continue
        );
        let after = Config::default().call_abandon_after;
        assert_eq!(
            s.tick(t0 + after - std::time::Duration::from_secs(1), &mut out),
            Step::Continue
        );
        assert!(out.is_empty());
        assert!(notes.lock().unwrap().is_empty());
        assert_eq!(s.tick(t0 + after, &mut out), Step::Continue);
        assert!(out.is_empty());
        assert_eq!(*notes.lock().unwrap(), vec![Note::CallAbandoned { seq: 1 }]);
        // The loopback slot is free again: a full table's worth of new calls
        // is accepted.
        assert!(send_commands(&mut s, 2, Loopback::SLOTS as u64).is_empty());
    }

    /// A reply that does not fit one message is answered `corrupt` and
    /// recorded, and the call is forgotten: its slot takes a new call.
    #[test]
    fn a_reply_larger_than_the_message_limit_is_answered_corrupt_and_recorded() {
        let notes = Notes::default();
        let rt = Loopback::new(CATALOG);
        let mut p = rt.attach();
        let config = config(&notes)
            .with_max_message_bytes(1024)
            .expect("above the minimum");
        let mut s = Session::new(Scanning(rt), &config);
        open(&mut s);
        let out = settled(&mut s, &mut p, QUERY, 1, Ok(&[1; 2000]));
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            response(QUERY, 0, 1, 1, ResponseOutcome::Corrupt)
        );
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::ValueTooLarge {
                target: QUERY,
                needed: 2000
            }]
        );
        assert!(send_commands(&mut s, 2, Loopback::SLOTS as u64).is_empty());
    }

    // -- The message limit holds whatever the buffer has grown to -----------

    const LIMIT: usize = 1024;

    /// A session over a scanning loopback under [`LIMIT`].
    fn limited() -> (Session<Scanning<Loopback>>, Loopback, Notes) {
        let notes = Notes::default();
        let rt = Loopback::new(CATALOG);
        let p = rt.attach();
        let config = config(&notes)
            .with_max_message_bytes(LIMIT)
            .expect("above the minimum");
        (Session::new(Scanning(rt), &config), p, notes)
    }

    /// The largest payload `frame` may carry under [`LIMIT`], with `frame`
    /// built with an empty payload.
    fn cap(frame: Frame<'_>) -> usize {
        LIMIT - frame.encoded_len()
    }

    fn signal_cap() -> usize {
        cap(publish(SIGNAL, 0, 0, PublishState::Live(&[])))
    }

    fn occurrence_cap() -> usize {
        cap(occurrence(EVENT, 0, 0, &[]))
    }

    fn reply_cap() -> usize {
        cap(response(QUERY, 0, 0, 0, ResponseOutcome::Reply(&[])))
    }

    fn assert_within_limit(out: &[Vec<u8>]) {
        for m in out {
            assert!(m.len() <= LIMIT, "a frame of {} bytes", m.len());
        }
    }

    /// Drains one oversized occurrence, which grows nothing the later reads
    /// can use, and then an occurrence of the cap, which fills the buffer to
    /// one byte above the signal cap.
    fn grow_by_occurrences(s: &mut Session<Scanning<Loopback>>, p: &mut Loopback) {
        send(s, &msg(Frame::Subscribe(EVENT)));
        raise(p, EVENT, &[1; 2000]);
        raise(p, EVENT, &vec![2; occurrence_cap()]);
        let (_, out) = tick(s);
        assert_eq!(out.len(), 1);
        assert_within_limit(&out);
    }

    #[test]
    fn a_signal_is_capped_after_the_buffer_grew_on_occurrences() {
        let (mut s, mut p, notes) = limited();
        open(&mut s);
        grow_by_occurrences(&mut s, &mut p);
        // One byte over the signal cap, and inside the buffer's size.
        commit(&mut p, &vec![3; signal_cap() + 1]);
        let (_, out) = send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
        assert_eq!(out.len(), 1);
        assert_eq!(decode(&out[0]).unwrap(), accepted(SIGNAL));
        assert!(notes.lock().unwrap().contains(&Note::ValueTooLarge {
            target: SIGNAL,
            needed: signal_cap() + 1
        }));
        commit(&mut p, &vec![4; signal_cap()]);
        let (_, out) = tick(&mut s);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].len(), LIMIT);
        assert_within_limit(&out);
    }

    #[test]
    fn an_occurrence_is_capped_after_the_buffer_grew_on_an_occurrence() {
        let (mut s, mut p, notes) = limited();
        open(&mut s);
        grow_by_occurrences(&mut s, &mut p);
        raise(&mut p, EVENT, &vec![3; occurrence_cap() + 1]);
        raise(&mut p, EVENT, &vec![4; occurrence_cap()]);
        let (_, out) = tick(&mut s);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].len(), LIMIT);
        assert_eq!(
            decode(&out[0]).unwrap(),
            occurrence(EVENT, 0, 4, &vec![4; occurrence_cap()])
        );
        assert!(notes.lock().unwrap().contains(&Note::ValueTooLarge {
            target: EVENT,
            needed: occurrence_cap() + 1
        }));
    }

    #[test]
    fn a_fixed_is_capped_after_the_buffer_grew_on_occurrences() {
        let (mut s, mut p, notes) = limited();
        open(&mut s);
        grow_by_occurrences(&mut s, &mut p);
        p.provision_fixed(IFACE, Ordinal(5), &vec![3; signal_cap() + 1])
            .expect("the loopback stores the fixed");
        let (_, out) = send(&mut s, &msg(Frame::Read(FIXED)));
        assert_eq!(out.len(), 1);
        assert_eq!(decode(&out[0]).unwrap(), unknown(FIXED));
        assert!(notes.lock().unwrap().contains(&Note::ValueTooLarge {
            target: FIXED,
            needed: signal_cap() + 1
        }));
        p.provision_fixed(IFACE, Ordinal(5), &vec![4; signal_cap()])
            .expect("the loopback stores the fixed");
        let (_, out) = send(&mut s, &msg(Frame::Read(FIXED)));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].len(), LIMIT);
    }

    /// The review's probe: a 2000-byte occurrence, then a 1500-byte reply.
    #[test]
    fn a_reply_is_capped_after_the_buffer_grew_on_occurrences() {
        let (mut s, mut p, notes) = limited();
        open(&mut s);
        grow_by_occurrences(&mut s, &mut p);
        let out = settled(&mut s, &mut p, QUERY, 1, Ok(&[1; 1500]));
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            response(QUERY, 0, 1, 1, ResponseOutcome::Corrupt)
        );
        assert!(notes.lock().unwrap().contains(&Note::ValueTooLarge {
            target: QUERY,
            needed: 1500
        }));
    }

    /// The reply cap is below the signal cap: after a signal of the signal
    /// cap, a reply one byte over its own cap fits the buffer and is still
    /// not sent; the call is answered `corrupt`.
    #[test]
    fn a_reply_one_byte_over_its_cap_is_not_sent_after_a_full_size_signal() {
        let (mut s, mut p, notes) = limited();
        open(&mut s);
        commit(&mut p, &vec![1; signal_cap()]);
        let (_, out) = send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
        assert_eq!(out.len(), 2);
        assert_within_limit(&out);
        let out = settled(&mut s, &mut p, QUERY, 1, Ok(&vec![2; reply_cap() + 1]));
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            response(QUERY, 0, 1, 1, ResponseOutcome::Corrupt)
        );
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::ValueTooLarge {
                target: QUERY,
                needed: reply_cap() + 1
            }]
        );
        let out = settled(&mut s, &mut p, QUERY, 2, Ok(&vec![3; reply_cap()]));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].len(), LIMIT);
    }

    // -- The cap boundaries, above the first buffer ---------------------------

    /// A limit above the first buffer, so that a value at the cap is first
    /// reported as `Short` and the comparison with the cap decides.
    const ROOMY_LIMIT: usize = 4096;

    fn roomy() -> (Session<Described<Scanning<Loopback>>>, Loopback, Notes) {
        // The default ceiling of a signal and an event is below
        // `ROOMY_LIMIT`, so the table raises both to it, and the loopback,
        // which refuses a value above its member's limit on send, is given
        // the same raise: the session's message cap is what these tests
        // observe.
        let mut t = (*table()).clone();
        assert!(t.set_limit(IFACE, SIGNAL.ord, ROOMY_LIMIT as u32));
        assert!(t.set_limit(IFACE, EVENT.ord, ROOMY_LIMIT as u32));
        let (s, mut p, notes) = described_with(t, |c| {
            c.with_max_message_bytes(ROOMY_LIMIT)
                .expect("above the minimum")
        });
        p.set_limit(SIGNAL.iface, SIGNAL.ord, ROOMY_LIMIT);
        p.set_limit(EVENT.iface, EVENT.ord, ROOMY_LIMIT);
        (s, p, notes)
    }

    fn roomy_cap(frame: Frame<'_>) -> usize {
        ROOMY_LIMIT - frame.encoded_len()
    }

    #[test]
    fn a_signal_of_exactly_the_cap_is_sent_and_one_byte_more_is_not() {
        let (mut s, mut p, notes) = roomy();
        open(&mut s);
        send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
        let cap = roomy_cap(publish(SIGNAL, 0, 0, PublishState::Live(&[])));
        commit(&mut p, &vec![1; cap]);
        let (_, out) = tick(&mut s);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].len(), ROOMY_LIMIT);
        commit(&mut p, &vec![2; cap + 1]);
        let (_, out) = tick(&mut s);
        assert!(out.is_empty());
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::ValueTooLarge {
                target: SIGNAL,
                needed: cap + 1
            }]
        );
    }

    #[test]
    fn a_fixed_of_exactly_the_cap_is_sent_and_one_byte_more_is_not() {
        let (mut s, mut p, notes) = roomy();
        open(&mut s);
        let cap = roomy_cap(publish(FIXED, 0, 0, PublishState::Live(&[])));
        p.provision_fixed(IFACE, Ordinal(5), &vec![1; cap])
            .expect("the loopback stores the fixed");
        let (_, out) = send(&mut s, &msg(Frame::Read(FIXED)));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].len(), ROOMY_LIMIT);
        p.provision_fixed(IFACE, Ordinal(5), &vec![2; cap + 1])
            .expect("the loopback stores the fixed");
        let (_, out) = send(&mut s, &msg(Frame::Read(FIXED)));
        assert_eq!(out.len(), 1);
        assert_eq!(decode(&out[0]).unwrap(), unknown(FIXED));
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::ValueTooLarge {
                target: FIXED,
                needed: cap + 1
            }]
        );
    }

    #[test]
    fn an_occurrence_of_exactly_the_cap_is_sent_and_one_byte_more_is_not() {
        let (mut s, mut p, notes) = roomy();
        open(&mut s);
        send(&mut s, &msg(Frame::Subscribe(EVENT)));
        let cap = roomy_cap(occurrence(EVENT, 0, 0, &[]));
        raise(&mut p, EVENT, &vec![1; cap]);
        let (_, out) = tick(&mut s);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].len(), ROOMY_LIMIT);
        raise(&mut p, EVENT, &vec![2; cap + 1]);
        let (_, out) = tick(&mut s);
        assert!(out.is_empty());
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::ValueTooLarge {
                target: EVENT,
                needed: cap + 1
            }]
        );
    }

    #[test]
    fn a_reply_of_exactly_the_cap_is_sent_and_one_byte_more_is_not() {
        let (mut s, mut p, notes) = roomy();
        open(&mut s);
        let cap = roomy_cap(response(QUERY, 0, 0, 0, ResponseOutcome::Reply(&[])));
        let out = settled(&mut s, &mut p, QUERY, 1, Ok(&vec![1; cap]));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].len(), ROOMY_LIMIT);
        let out = settled(&mut s, &mut p, QUERY, 2, Ok(&vec![2; cap + 1]));
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            response(QUERY, 0, 2, 2, ResponseOutcome::Corrupt)
        );
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::ValueTooLarge {
                target: QUERY,
                needed: cap + 1
            }]
        );
    }

    // -- D-18: the end of a session -----------------------------------------

    #[test]
    fn the_end_of_a_session_forgets_its_calls_in_flight() {
        let (mut s, mut p, _n) = scanning();
        open(&mut s);
        assert!(send_commands(&mut s, 1, Loopback::SLOTS as u64).is_empty());
        s.close();
        for _ in 0..Loopback::SLOTS {
            p.command(IFACE, Ordinal(3), &[], None).unwrap();
        }
    }

    /// Which port call of [`Faulty`] fails.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Fail {
        None,
        /// `EventSource::subscribe` answers `Detached`.
        Subscribe,
        /// `EventSource::subscribe` answers `Contract(PreconditionFailed)`.
        SubscribeContract,
        /// `EventSource::next` answers `Detached`.
        Next,
        /// `Caller::command` answers `Detached`.
        Command,
        /// `Caller::query` answers `Detached`.
        Query,
        /// `Caller::query` answers `Contract(PreconditionFailed)`.
        QueryContract,
        /// `Caller::reply` answers `Detached`.
        Reply,
        /// `Caller::command` answers `TooLarge`.
        CommandTooLarge,
        /// `SignalReader::read` answers `Detached`.
        ReadDetached,
        /// `SignalReader::read` of ordinal 1 answers
        /// `Contract(PreconditionFailed)`.
        ReadContract,
        /// `FixedReader::read_fixed` answers `Detached`.
        FixedDetached,
    }

    /// Ports that delegate to a loopback, but for one call that fails, and
    /// that record what is forgotten. The failing call can be changed while
    /// the session runs.
    struct Faulty {
        inner: Loopback,
        fail: Rc<Cell<Fail>>,
        forgotten: Rc<RefCell<Vec<Correlation>>>,
    }

    /// A scanning backend over [`Faulty`], with the loopback's `fixed`.
    struct FaultyBackend(Faulty);

    impl Backend for FaultyBackend {
        type Ports = Faulty;

        fn ports(&mut self) -> &mut Faulty {
            &mut self.0
        }

        fn scannable(&self) -> Option<&dyn ScannableSignals> {
            Some(&self.0.inner)
        }

        fn read_fixed(
            &self,
            iface: InterfaceNo,
            ord: Ordinal,
            out: &mut [u8],
        ) -> Result<usize, ReadError> {
            if self.0.fail.get() == Fail::FixedDetached {
                return Err(ReadError::Detached);
            }
            FixedReader::read_fixed(&self.0.inner, iface, ord, out)
        }
    }

    /// An open session over a [`FaultyBackend`], the provider's handle on
    /// the same store, the switch of the failing call, and the correlations
    /// forgotten.
    struct FaultyRig {
        s: Session<FaultyBackend>,
        p: Loopback,
        fail: Rc<Cell<Fail>>,
        forgotten: Rc<RefCell<Vec<Correlation>>>,
    }

    fn faulty(fail: Fail) -> FaultyRig {
        let forgotten = Rc::new(RefCell::new(Vec::new()));
        let fail = Rc::new(Cell::new(fail));
        let rt = Loopback::new(CATALOG);
        let p = rt.attach();
        let backend = FaultyBackend(Faulty {
            inner: rt,
            fail: Rc::clone(&fail),
            forgotten: Rc::clone(&forgotten),
        });
        let mut s = Session::new(backend, &Config::default());
        open(&mut s);
        FaultyRig {
            s,
            p,
            fail,
            forgotten,
        }
    }

    impl Attached for Faulty {
        fn catalog(&self) -> &CatalogRef {
            self.inner.catalog()
        }
    }

    impl Encoded for Faulty {
        type Encoding = <Loopback as Encoded>::Encoding;
    }

    impl Clock for Faulty {
        fn now(&self) -> Timestamp {
            self.inner.now()
        }
    }

    impl SignalReader for Faulty {
        fn read(
            &self,
            iface: InterfaceNo,
            ord: Ordinal,
            out: &mut [u8],
        ) -> Result<RawSample, ReadError> {
            match self.fail.get() {
                Fail::ReadDetached => Err(ReadError::Detached),
                Fail::ReadContract if ord == Ordinal(1) => {
                    Err(ReadError::Contract(Contract::PreconditionFailed))
                }
                _ => self.inner.read(iface, ord, out),
            }
        }
    }

    impl EventSource for Faulty {
        fn subscribe(
            &mut self,
            iface: InterfaceNo,
            ords: &[Ordinal],
        ) -> Result<(), SubscribeError> {
            match self.fail.get() {
                Fail::Subscribe => Err(SubscribeError::Detached),
                Fail::SubscribeContract => {
                    Err(SubscribeError::Contract(Contract::PreconditionFailed))
                }
                _ => self.inner.subscribe(iface, ords),
            }
        }
        fn unsubscribe(&mut self, iface: InterfaceNo, ords: &[Ordinal]) {
            self.inner.unsubscribe(iface, ords);
        }
        fn next(&mut self, out: &mut [u8]) -> Result<Option<RawOccurrence>, ReadError> {
            if self.fail.get() == Fail::Next {
                return Err(ReadError::Detached);
            }
            self.inner.next(out)
        }
    }

    impl Caller for Faulty {
        fn command(
            &mut self,
            iface: InterfaceNo,
            ord: Ordinal,
            args: &[u8],
            trace: Option<TraceContext>,
        ) -> Result<Correlation, SendError> {
            match self.fail.get() {
                Fail::Command => Err(SendError::Detached),
                Fail::CommandTooLarge => Err(SendError::TooLarge { cap: 8 }),
                _ => self.inner.command(iface, ord, args, trace),
            }
        }
        fn query(
            &mut self,
            iface: InterfaceNo,
            ord: Ordinal,
            args: &[u8],
            trace: Option<TraceContext>,
        ) -> Result<Correlation, SendError> {
            match self.fail.get() {
                Fail::Query => Err(SendError::Detached),
                Fail::QueryContract => Err(SendError::Contract(Contract::PreconditionFailed)),
                _ => self.inner.query(iface, ord, args, trace),
            }
        }
        fn ack(&mut self, c: Correlation) -> Option<Result<(), CallError>> {
            self.inner.ack(c)
        }
        fn reply(
            &mut self,
            c: Correlation,
            out: &mut [u8],
        ) -> Result<Option<Result<usize, CallError>>, ReadError> {
            if self.fail.get() == Fail::Reply {
                return Err(ReadError::Detached);
            }
            self.inner.reply(c, out)
        }
        fn forget(&mut self, c: Correlation) {
            self.forgotten.borrow_mut().push(c);
            self.inner.forget(c);
        }
    }

    #[test]
    fn close_forgets_each_call_on_the_backend() {
        let mut r = faulty(Fail::None);
        send(&mut r.s, &request(COMMAND, 1, &[]));
        send(&mut r.s, &request(QUERY, 2, &[]));
        assert!(r.forgotten.borrow().is_empty());
        r.s.close();
        let mut forgotten = r.forgotten.borrow().clone();
        forgotten.sort_unstable();
        assert_eq!(forgotten.len(), 2);
        assert_ne!(forgotten[0], forgotten[1]);
    }

    #[test]
    fn a_backend_that_detaches_closes_1001() {
        let cases = [
            (Fail::Subscribe, msg(Frame::Subscribe(EVENT))),
            (Fail::Command, request(COMMAND, 1, &[])),
            (Fail::Query, request(QUERY, 1, &[])),
            (Fail::ReadDetached, msg(Frame::Subscribe(SIGNAL))),
            (Fail::ReadDetached, msg(Frame::Read(SIGNAL))),
            (Fail::FixedDetached, msg(Frame::Read(FIXED))),
        ];
        for (fail, m) in cases {
            let mut r = faulty(fail);
            let (step, out) = send(&mut r.s, &m);
            assert_eq!(step, Step::Close(1001), "{fail:?}");
            assert!(out.is_empty(), "{fail:?}");
        }
        let mut r = faulty(Fail::Next);
        let (step, out) = tick(&mut r.s);
        assert_eq!(step, Step::Close(1001));
        assert!(out.is_empty());
        let mut r = faulty(Fail::Reply);
        let (step, _) = send(&mut r.s, &request(QUERY, 1, &[]));
        assert_eq!(step, Step::Continue);
        let (step, out) = tick(&mut r.s);
        assert_eq!(step, Step::Close(1001));
        assert!(out.is_empty());
    }

    /// A signal subscribed while the backend was healthy, read on a tick
    /// after the backend detached.
    #[test]
    fn a_signal_read_that_detaches_on_a_tick_closes_1001() {
        let mut r = faulty(Fail::None);
        send(&mut r.s, &msg(Frame::Subscribe(SIGNAL)));
        commit(&mut r.p, &[1]);
        r.fail.set(Fail::ReadDetached);
        let (step, out) = tick(&mut r.s);
        assert_eq!(step, Step::Close(1001));
        assert!(out.is_empty());
    }

    /// A signal `subscribe` the backend refuses with a contract error is
    /// answered `contract` and subscribes nothing: no mark is left on the
    /// interface, so a sibling subscribed later, after a commit, does not
    /// get that commit sent again by the next tick.
    #[test]
    fn a_signal_subscribe_the_backend_refuses_is_answered_contract_and_leaves_no_mark() {
        let mut r = faulty(Fail::ReadContract);
        let (step, out) = send(&mut r.s, &msg(Frame::Subscribe(SIGNAL)));
        assert_eq!(step, Step::Continue);
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            Frame::Answer {
                target: SIGNAL,
                outcome: AnswerOutcome::Contract(Contract::PreconditionFailed),
            }
        );
        assert!(r.s.marks.is_empty());
        assert!(!r.s.signals.contains_key(&(IFACE, Ordinal(1))));
        let sibling = target(Kind::Signal, 2);
        r.p.set(IFACE, Ordinal(2), &[2]).unwrap();
        r.p.commit();
        let (_, out) = send(&mut r.s, &msg(Frame::Subscribe(sibling)));
        assert_eq!(out.len(), 2);
        assert_eq!(
            decode(&out[1]).unwrap(),
            publish(sibling, 0, 1, PublishState::Live(&[2]))
        );
        let (_, out) = tick(&mut r.s);
        assert!(out.is_empty());
    }

    #[test]
    fn a_signal_read_the_backend_refuses_is_answered_contract() {
        let mut r = faulty(Fail::ReadContract);
        let (step, out) = send(&mut r.s, &msg(Frame::Read(SIGNAL)));
        assert_eq!(step, Step::Continue);
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            Frame::Answer {
                target: SIGNAL,
                outcome: AnswerOutcome::Contract(Contract::PreconditionFailed),
            }
        );
    }

    #[test]
    fn an_event_subscribe_the_backend_refuses_is_answered_contract() {
        let mut r = faulty(Fail::SubscribeContract);
        let (step, out) = send(&mut r.s, &msg(Frame::Subscribe(EVENT)));
        assert_eq!(step, Step::Continue);
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            Frame::Answer {
                target: EVENT,
                outcome: AnswerOutcome::Contract(Contract::PreconditionFailed),
            }
        );
    }

    #[test]
    fn a_query_the_backend_refuses_at_send_is_answered_contract() {
        let mut r = faulty(Fail::QueryContract);
        let (step, out) = send(&mut r.s, &request(QUERY, 1, &[]));
        assert_eq!(step, Step::Continue);
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            response(
                QUERY,
                0,
                1,
                1,
                ResponseOutcome::Contract(Contract::PreconditionFailed)
            )
        );
    }

    #[test]
    fn a_send_refused_as_too_large_is_answered_corrupt() {
        let mut r = faulty(Fail::CommandTooLarge);
        let (step, out) = send(&mut r.s, &request(COMMAND, 1, &[1; 9]));
        assert_eq!(step, Step::Continue);
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            response(COMMAND, 0, 1, 1, ResponseOutcome::Corrupt)
        );
    }

    // -- The member table (frame specification §6.4) -------------------------

    /// The member table of the test catalog: signal 1, event 2, command 3,
    /// query 4, fixed 5, and command 7 with no bound; ordinal 6 is reserved
    /// and has no entry.
    fn table() -> Arc<MemberTable> {
        let mut t = MemberTable::new("ws.test", CatalogHash([7; 32]));
        let bound = Some(std::time::Duration::from_secs(1));
        t.insert(IFACE, Ordinal(1), Kind::Signal, None);
        t.insert(IFACE, Ordinal(2), Kind::Event, None);
        t.insert(IFACE, Ordinal(3), Kind::Command, bound);
        t.insert(IFACE, Ordinal(4), Kind::Query, bound);
        t.insert(IFACE, Ordinal(5), Kind::Fixed, None);
        t.insert(IFACE, COMMAND_UNBOUNDED.ord, Kind::Command, None);
        Arc::new(t)
    }

    /// A command the member table gives no bound.
    const COMMAND_UNBOUNDED: Target = Target {
        kind: Kind::Command,
        iface: IFACE,
        ord: Ordinal(7),
    };

    fn described_scanning() -> (Session<Described<Scanning<Loopback>>>, Loopback, Notes) {
        let notes = Notes::default();
        let rt = Loopback::new(CATALOG);
        let provider = rt.attach();
        let backend = Described::new(Scanning(rt), table()).unwrap();
        (Session::new(backend, &config(&notes)), provider, notes)
    }
    // -- The payload limit of a member (ADR-0030 decision 15) ----------------

    /// The test table with a payload limit of `limit` on `COMMAND`, and on
    /// `QUERY` when `query` is set.
    fn limited_table(limit: u32, query: bool) -> MemberTable {
        let mut t = (*table()).clone();
        assert!(t.set_limit(IFACE, COMMAND.ord, limit));
        if query {
            assert!(t.set_limit(IFACE, QUERY.ord, limit));
        }
        t
    }

    /// Sends a request of `target` with `payload` and checks that the one
    /// frame sent back is `corrupt` and that the backend saw no call.
    fn assert_answered_corrupt(
        s: &mut Session<Described<Scanning<Loopback>>>,
        p: &mut Loopback,
        target: Target,
        seq: u64,
        payload: &[u8],
    ) -> Vec<u8> {
        let (step, out) = send(s, &request(target, seq, payload));
        assert_eq!(step, Step::Continue);
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            response(target, 0, 1, seq, ResponseOutcome::Corrupt)
        );
        p.serve(target.iface, &[target.ord]).unwrap();
        let mut buf = [0; 64];
        assert_eq!(p.next_claim(&mut buf).unwrap(), None);
        out.into_iter().next().unwrap()
    }

    #[test]
    fn a_request_above_its_members_limit_is_answered_corrupt() {
        let (mut s, mut p, notes) = described_with(limited_table(4, false), |c| c);
        open(&mut s);
        assert_answered_corrupt(&mut s, &mut p, COMMAND, 1, &[1; 5]);
        assert!(notes.lock().unwrap().is_empty());
    }

    #[test]
    fn a_request_at_its_members_limit_reaches_the_backend() {
        let (mut s, mut p, _n) = described_with(limited_table(4, false), |c| c);
        open(&mut s);
        let (step, out) = send(&mut s, &request(COMMAND, 1, &[1; 4]));
        assert_eq!(step, Step::Continue);
        assert!(out.is_empty());
        assert_eq!(claim(&mut p, COMMAND).len, 4);
    }

    #[test]
    fn a_query_above_its_members_limit_is_answered_corrupt() {
        let (mut s, mut p, _n) = described_with(limited_table(4, true), |c| c);
        open(&mut s);
        assert_answered_corrupt(&mut s, &mut p, QUERY, 1, &[1; 5]);
    }

    #[test]
    fn a_limit_of_zero_admits_only_empty_arguments() {
        let (mut s, mut p, _n) = described_with(limited_table(0, false), |c| c);
        open(&mut s);
        let (_, out) = send(&mut s, &request(COMMAND, 1, &[]));
        assert!(out.is_empty());
        assert_eq!(claim(&mut p, COMMAND).len, 0);
        assert_answered_corrupt(&mut s, &mut p, COMMAND, 2, &[1]);
    }

    /// A loopback whose members admit any value a catalog could allow, as a
    /// backend that does not refuse a value on send: the session's own
    /// checks are then what a test observes.
    fn permissive(p: &mut Loopback) {
        for ord in [1, 2, 3, 4, 5, COMMAND_UNBOUNDED.ord.0] {
            p.set_limit(IFACE, Ordinal(ord), FIXED_CEILING);
        }
    }

    #[test]
    fn a_member_with_no_limit_is_checked_against_the_default_ceiling_of_its_kind() {
        let (mut s, mut p, _n) = described_with(limited_table(4, false), |c| c);
        permissive(&mut p);
        open(&mut s);
        let over = vec![1; CALL_CEILING + 1];
        assert_answered_corrupt(&mut s, &mut p, COMMAND_UNBOUNDED, 1, &over);
        let (_, out) = send(&mut s, &request(COMMAND_UNBOUNDED, 2, &over[1..]));
        assert!(out.is_empty());
        let mut buf = vec![0; CALL_CEILING];
        let claim = p.next_claim(&mut buf).unwrap().expect("a claim is waiting");
        assert_eq!(claim.len, CALL_CEILING);
    }

    #[test]
    fn a_request_above_the_default_ceiling_of_its_kind_is_answered_corrupt_without_a_table() {
        let (mut s, mut p, notes) = scanning();
        permissive(&mut p);
        open(&mut s);
        p.serve(IFACE, &[COMMAND.ord]).unwrap();
        let over = vec![1; CALL_CEILING + 1];
        let (step, out) = send(&mut s, &request(COMMAND, 1, &over));
        assert_eq!(step, Step::Continue);
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            response(COMMAND, 0, 1, 1, ResponseOutcome::Corrupt)
        );
        let mut buf = vec![0; CALL_CEILING];
        assert_eq!(p.next_claim(&mut buf).unwrap(), None);
        assert!(notes.lock().unwrap().is_empty());
        let (_, out) = send(&mut s, &request(COMMAND, 2, &over[1..]));
        assert!(out.is_empty());
        let claim = p.next_claim(&mut buf).unwrap().expect("a claim is waiting");
        assert_eq!(claim.len, CALL_CEILING);
    }

    // -- The limit of a member on the send side (ADR-0030 decision 19) -------

    #[test]
    fn a_signal_above_the_default_ceiling_of_its_kind_is_recorded_and_not_sent() {
        let (mut s, mut p, notes) = scanning();
        permissive(&mut p);
        open(&mut s);
        send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
        commit(&mut p, &vec![1; SIGNAL_EVENT_CEILING + 1]);
        let (_, out) = tick(&mut s);
        assert!(out.is_empty());
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::ValueTooLarge {
                target: SIGNAL,
                needed: SIGNAL_EVENT_CEILING + 1
            }]
        );
        let value = vec![2; SIGNAL_EVENT_CEILING];
        commit(&mut p, &value);
        let (_, out) = tick(&mut s);
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            publish(SIGNAL, 0, 1, PublishState::Live(&value))
        );
    }

    #[test]
    fn a_signal_is_checked_against_the_limit_its_table_entry_gives() {
        let mut t = (*table()).clone();
        assert!(t.set_limit(IFACE, SIGNAL.ord, 4096));
        let (mut s, mut p, notes) = described_with(t, |c| c);
        permissive(&mut p);
        open(&mut s);
        send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
        commit(&mut p, &vec![1; 4097]);
        let (_, out) = tick(&mut s);
        assert!(out.is_empty());
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::ValueTooLarge {
                target: SIGNAL,
                needed: 4097
            }]
        );
        commit(&mut p, &vec![2; 4096]);
        let (_, out) = tick(&mut s);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].len(), 4096 + PUBLISH_HEADER);
    }

    #[test]
    fn an_occurrence_above_the_default_ceiling_of_its_kind_is_recorded_and_dropped() {
        let (mut s, mut p, notes) = scanning();
        permissive(&mut p);
        open(&mut s);
        send(&mut s, &msg(Frame::Subscribe(EVENT)));
        raise(&mut p, EVENT, &vec![1; SIGNAL_EVENT_CEILING + 1]);
        let value = vec![2; SIGNAL_EVENT_CEILING];
        raise(&mut p, EVENT, &value);
        let (_, out) = tick(&mut s);
        assert_eq!(out.len(), 1);
        // The dropped occurrence advances the channel's `seq`.
        assert_eq!(decode(&out[0]).unwrap(), occurrence(EVENT, 0, 2, &value));
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::ValueTooLarge {
                target: EVENT,
                needed: SIGNAL_EVENT_CEILING + 1
            }]
        );
    }

    #[test]
    fn a_reply_above_the_default_ceiling_of_its_kind_is_answered_corrupt_and_recorded() {
        let (mut s, mut p, notes) = scanning();
        permissive(&mut p);
        open(&mut s);
        let out = settled(&mut s, &mut p, QUERY, 1, Ok(&vec![1; CALL_CEILING + 1]));
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            response(QUERY, 0, 1, 1, ResponseOutcome::Corrupt)
        );
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::ValueTooLarge {
                target: QUERY,
                needed: CALL_CEILING + 1
            }]
        );
        let out = settled(&mut s, &mut p, QUERY, 2, Ok(&vec![2; CALL_CEILING]));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].len(), crate::frame::MAX_MESSAGE_LIMIT);
    }

    /// The reply cap is the member's limit from the table when the table
    /// gives one, below the message cap: a reply one byte above it is
    /// answered `corrupt` and recorded, one of exactly the limit is sent.
    #[test]
    fn a_reply_above_the_limit_its_table_entry_gives_is_answered_corrupt_and_recorded() {
        let (mut s, mut p, notes) = described_with(limited_table(900, true), |c| c);
        permissive(&mut p);
        open(&mut s);
        let out = settled(&mut s, &mut p, QUERY, 1, Ok(&vec![1; 901]));
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            response(QUERY, 0, 1, 1, ResponseOutcome::Corrupt)
        );
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::ValueTooLarge {
                target: QUERY,
                needed: 901
            }]
        );
        let out = settled(&mut s, &mut p, QUERY, 2, Ok(&vec![2; 900]));
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            response(QUERY, 0, 2, 2, ResponseOutcome::Reply(&[2; 900]))
        );
        assert_eq!(notes.lock().unwrap().len(), 1);
    }

    /// Until the binding carries a `fixed` in blocks (driftsys/ridl#1129), a
    /// `fixed` above what one message carries is not served: the read is
    /// answered `contract(UnknownInteraction)`, the refusal is recorded, and
    /// the session goes on serving every other member.
    #[test]
    fn a_read_of_a_fixed_above_one_message_is_answered_unknown_interaction_and_recorded() {
        let (mut s, mut p, notes) = scanning();
        open(&mut s);
        p.provision_fixed(IFACE, Ordinal(5), &vec![1; 20_000])
            .expect("within the default ceiling of a fixed");
        let (step, out) = send(&mut s, &msg(Frame::Read(FIXED)));
        assert_eq!(step, Step::Continue);
        assert_eq!(out.len(), 1);
        assert_eq!(decode(&out[0]).unwrap(), unknown(FIXED));
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::ValueTooLarge {
                target: FIXED,
                needed: 20_000
            }]
        );
        commit(&mut p, &[9]);
        let (_, out) = send(&mut s, &msg(Frame::Subscribe(SIGNAL)));
        assert_eq!(out.len(), 2);
        assert_eq!(
            decode(&out[1]).unwrap(),
            publish(SIGNAL, 0, 1, PublishState::Live(&[9]))
        );
    }

    #[test]
    fn a_fixed_above_the_limit_its_table_entry_gives_is_answered_unknown_interaction() {
        let mut t = (*table()).clone();
        assert!(t.set_limit(IFACE, FIXED.ord, 8));
        let (mut s, mut p, notes) = described_with(t, |c| c);
        permissive(&mut p);
        open(&mut s);
        p.provision_fixed(IFACE, Ordinal(5), &[1; 9]).unwrap();
        let (_, out) = send(&mut s, &msg(Frame::Read(FIXED)));
        assert_eq!(out.len(), 1);
        assert_eq!(decode(&out[0]).unwrap(), unknown(FIXED));
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::ValueTooLarge {
                target: FIXED,
                needed: 9
            }]
        );
        p.provision_fixed(IFACE, Ordinal(5), &[2; 8]).unwrap();
        let (_, out) = send(&mut s, &msg(Frame::Read(FIXED)));
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            publish(FIXED, 0, 0, PublishState::Live(&[2; 8]))
        );
    }

    /// A `read` of a signal whose value is above the limit its table entry
    /// gives is answered `contract(UnknownInteraction)`, as a `read` of a
    /// `fixed` above its limit is, so that the client's read ends; the value
    /// is recorded and no `publish` is sent.
    ///
    /// Mutation: record the value and send no frame.
    #[test]
    fn a_signal_read_above_the_limit_its_table_entry_gives_is_answered_unknown_interaction() {
        let mut t = (*table()).clone();
        assert!(t.set_limit(IFACE, SIGNAL.ord, 4096));
        let (mut s, mut p, notes) = described_with(t, |c| c);
        permissive(&mut p);
        open(&mut s);
        commit(&mut p, &vec![1; 4097]);
        let (step, out) = send(&mut s, &msg(Frame::Read(SIGNAL)));
        assert_eq!(step, Step::Continue);
        assert_eq!(out.len(), 1);
        assert_eq!(decode(&out[0]).unwrap(), unknown(SIGNAL));
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::ValueTooLarge {
                target: SIGNAL,
                needed: 4097
            }]
        );
        // The read did not subscribe, and a value within the limit is read.
        commit(&mut p, &vec![2; 4096]);
        let (_, out) = tick(&mut s);
        assert!(out.is_empty());
        let (_, out) = send(&mut s, &msg(Frame::Read(SIGNAL)));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].len(), 4096 + PUBLISH_HEADER);
    }

    /// The same without a table: the signal is read against the default
    /// ceiling of its kind.
    #[test]
    fn a_signal_read_above_the_default_ceiling_of_its_kind_is_answered_unknown_interaction() {
        let (mut s, mut p, notes) = scanning();
        permissive(&mut p);
        open(&mut s);
        commit(&mut p, &vec![1; SIGNAL_EVENT_CEILING + 1]);
        let (_, out) = send(&mut s, &msg(Frame::Read(SIGNAL)));
        assert_eq!(out.len(), 1);
        assert_eq!(decode(&out[0]).unwrap(), unknown(SIGNAL));
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::ValueTooLarge {
                target: SIGNAL,
                needed: SIGNAL_EVENT_CEILING + 1
            }]
        );
    }

    #[test]
    fn a_repeat_of_a_request_answered_corrupt_for_its_length_gets_the_same_response() {
        let (mut s, mut p, notes) = described_with(limited_table(4, false), |c| c);
        open(&mut s);
        let first = assert_answered_corrupt(&mut s, &mut p, COMMAND, 1, &[1; 5]);
        let (_, again) = send(&mut s, &request(COMMAND, 1, &[1; 5]));
        assert_eq!(again, vec![first]);
        let mut buf = [0; 64];
        assert_eq!(p.next_claim(&mut buf).unwrap(), None);
        assert!(notes.lock().unwrap().is_empty());
    }

    #[test]
    fn an_unknown_member_is_answered_unknown_interaction_whatever_its_length() {
        let (mut s, _p, _n) = described_with(limited_table(4, false), |c| c);
        open(&mut s);
        // Ordinal 1 is a signal in the table, so it is unknown under `Command`
        // and holds no limit. `COMMAND`'s ordinal is a command with a limit of
        // 4, so it is unknown under `Query`: the length check must not run
        // before the member is found to be unknown.
        let targets = [
            Target {
                kind: Kind::Command,
                iface: IFACE,
                ord: Ordinal(1),
            },
            Target {
                kind: Kind::Query,
                iface: IFACE,
                ord: COMMAND.ord,
            },
        ];
        for (seq, target) in (1..).zip(targets) {
            let (_, out) = send(&mut s, &request(target, seq, &[1; 5]));
            assert_eq!(out.len(), 1, "{target:?}");
            assert_eq!(
                decode(&out[0]).unwrap(),
                response(
                    target,
                    0,
                    seq,
                    seq,
                    ResponseOutcome::Contract(Contract::UnknownInteraction)
                ),
                "{target:?}"
            );
        }
    }

    #[test]
    fn a_request_under_another_kind_than_command_or_query_is_unknown_whatever_its_length() {
        // Ordinals 1, 2 and 5 are a signal, an event and a fixed in the table;
        // a limit on them must not turn the unknown-interaction answer into
        // `corrupt`.
        let kinds = [
            (Kind::Signal, Ordinal(1)),
            (Kind::Event, Ordinal(2)),
            (Kind::Fixed, Ordinal(5)),
        ];
        let mut t = (*table()).clone();
        for (_, ord) in kinds {
            assert!(t.set_limit(IFACE, ord, 4));
        }
        let (mut s, _p, _n) = described_with(t, |c| c);
        open(&mut s);
        for (seq, (kind, ord)) in (1..).zip(kinds) {
            let target = Target {
                kind,
                iface: IFACE,
                ord,
            };
            let (_, out) = send(&mut s, &request(target, seq, &[1; 5]));
            assert_eq!(out.len(), 1, "{target:?}");
            assert_eq!(
                decode(&out[0]).unwrap(),
                response(
                    target,
                    0,
                    seq,
                    seq,
                    ResponseOutcome::Contract(Contract::UnknownInteraction)
                ),
                "{target:?}"
            );
        }
    }

    #[test]
    fn an_attach_naming_a_listed_earlier_hash_is_accepted_and_an_unlisted_one_is_catalog_mismatch()
    {
        let notes = Notes::default();
        let rt = Loopback::new(CATALOG);
        let mut t = MemberTable::new("ws.test", CatalogHash([7; 32]));
        t.add_compatible(CatalogHash([6; 32]));
        let backend = Described::new(Scanning(rt.attach()), Arc::new(t)).unwrap();
        let mut s = Session::new(backend, &config(&notes));
        let (step, out) = send(&mut s, &attach_msg(1, 1, [6u8; 32], "ws.test"));
        assert_eq!(step, Step::Continue);
        assert_eq!(
            decode(&out[0]).unwrap(),
            Frame::Attached(AttachOutcome::Accepted)
        );

        let backend = Described::new(Scanning(rt.attach()), table()).unwrap();
        let mut s = Session::new(backend, &config(&notes));
        refused(
            &mut s,
            &attach_msg(1, 1, [5u8; 32], "ws.test"),
            Reason::CatalogMismatch,
        );
    }

    /// A session attached under a compatible earlier hash (§6.4) is served
    /// from the current table: a request for an ordinal that table no longer
    /// holds is refused, and a member it holds is sent to the backend. The
    /// session keeps no record of the hash the `attach` named, so the test
    /// guards against a later lookup conditioned on that hash.
    #[test]
    fn a_request_for_a_retired_ordinal_under_a_compatible_earlier_hash_is_refused() {
        let notes = Notes::default();
        let rt = Loopback::new(CATALOG);
        let mut p = rt.attach();
        let mut t = (*table()).clone();
        t.add_compatible(CatalogHash([6; 32]));
        let backend = Described::new(Scanning(rt), Arc::new(t)).unwrap();
        let mut s = Session::new(backend, &config(&notes));
        let (_, out) = send(&mut s, &attach_msg(1, 1, [6u8; 32], "ws.test"));
        assert_eq!(
            decode(&out[0]).unwrap(),
            Frame::Attached(AttachOutcome::Accepted)
        );
        let retired = target(Kind::Command, 9);
        let (step, out) = send(&mut s, &request(retired, 1, &[]));
        assert_eq!((step, out.len()), (Step::Continue, 1));
        assert_eq!(
            decode(&out[0]).unwrap(),
            response(
                retired,
                0,
                1,
                1,
                ResponseOutcome::Contract(Contract::UnknownInteraction)
            )
        );
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::UnknownInteraction { target: retired }]
        );
        let (step, out) = send(&mut s, &request(COMMAND, 2, &[]));
        assert_eq!((step, out.is_empty()), (Step::Continue, true));
        assert_eq!(claim(&mut p, COMMAND).ord, COMMAND.ord);
    }

    /// With a table, a refused `request` is kept like any settled response
    /// (D-12), so a repeat of its `seq` gets the same bytes and is neither
    /// refused again nor dropped.
    ///
    /// Mutation: send a refused response without keeping it.
    #[test]
    fn a_refused_request_is_kept_and_its_repeat_gets_the_same_bytes() {
        let (mut s, _p, notes) = described_scanning();
        open(&mut s);
        let t = target(Kind::Command, 9);
        let (_, out) = send(&mut s, &request(t, 1, &[]));
        assert_eq!(
            decode(&out[0]).unwrap(),
            response(
                t,
                0,
                1,
                1,
                ResponseOutcome::Contract(Contract::UnknownInteraction)
            )
        );
        let (step, again) = send(&mut s, &request(t, 1, &[]));
        assert_eq!(step, Step::Continue);
        assert_eq!(again, out);
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::UnknownInteraction { target: t }]
        );
    }

    // -- Retention by bound, and the retained-bytes cap (frame specification §7)

    /// Mutations: never expire; expire at once.
    #[test]
    fn a_repeat_within_the_bound_gets_the_response_and_one_after_it_is_dropped() {
        let (mut s, mut p, notes) = described_scanning();
        open(&mut s);
        // COMMAND's max is 1 s = 1_000_000 us; the request is stamped at the clock, 0.
        let out = settled(&mut s, &mut p, COMMAND, 1, Ok(&[]));
        assert_eq!(out.len(), 1);
        p.advance(Duration(500_000));
        tick(&mut s);
        let (_, again) = send(&mut s, &request_at(COMMAND, 1, 0, &[7]));
        assert_eq!(again, out);
        p.advance(Duration(500_001)); // the clock is at max + 1
        tick(&mut s);
        let (_, again) = send(&mut s, &request_at(COMMAND, 1, 0, &[7]));
        assert!(again.is_empty());
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::DuplicateDropped { seq: 1 }]
        );
    }

    /// Mutation: keep `RETAINED` for a bounded member.
    #[test]
    fn sixty_five_bounded_responses_are_all_kept_within_their_bound() {
        let (mut s, mut p, _n) = described_scanning();
        open(&mut s);
        let first = settled(&mut s, &mut p, COMMAND, 1, Ok(&[]));
        for seq in 2..=65 {
            settled(&mut s, &mut p, COMMAND, seq, Ok(&[]));
        }
        let (_, again) = send(&mut s, &request_at(COMMAND, 1, 0, &[]));
        assert_eq!(again, first);
    }

    /// Mutation: measure from the stamp only.
    #[test]
    fn a_request_stamped_an_hour_ahead_expires_at_receipt_plus_max() {
        let (mut s, mut p, notes) = described_scanning();
        open(&mut s);
        let ahead = 3_600_000_000; // one hour, in microseconds, while the clock is at 0
        let (_, out) = send(&mut s, &request_at(COMMAND, 1, ahead, &[]));
        assert!(out.is_empty());
        let c = claim(&mut p, COMMAND);
        p.settle(c.id, Ok(&[])).unwrap();
        let (_, out) = tick(&mut s);
        assert_eq!(out.len(), 1);
        p.advance(Duration(1_000_001)); // receipt (0) + max + 1
        tick(&mut s);
        let (_, again) = send(&mut s, &request_at(COMMAND, 1, ahead, &[]));
        assert!(again.is_empty());
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::DuplicateDropped { seq: 1 }]
        );
    }

    /// Mutation: measure from the receipt only.
    #[test]
    fn a_request_stamped_far_behind_the_clock_keeps_its_response_until_stamp_plus_max() {
        let (mut s, mut p, notes) = described_scanning();
        open(&mut s);
        p.advance(Duration(3_600_000_000)); // the clock is at one hour
        let out = settled(&mut s, &mut p, COMMAND, 1, Ok(&[])); // stamped 0 by `request`
        assert_eq!(out.len(), 1);
        tick(&mut s); // 0 + max is long past: the next tick expires it
        let (_, again) = send(&mut s, &request_at(COMMAND, 1, 0, &[]));
        assert!(again.is_empty());
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::DuplicateDropped { seq: 1 }]
        );
    }

    /// Mutation: drop a response with no bound at once.
    #[test]
    fn a_member_with_no_bound_falls_back_to_the_last_64() {
        let (mut s, mut p, notes) = described_scanning();
        open(&mut s);
        let mut responses = Vec::new();
        for seq in 1..=65 {
            responses.push(settled(&mut s, &mut p, COMMAND_UNBOUNDED, seq, Ok(&[])));
        }
        p.advance(Duration(3_600_000_000));
        tick(&mut s);
        let (_, again) = send(&mut s, &request_at(COMMAND_UNBOUNDED, 1, 0, &[]));
        assert!(again.is_empty());
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::DuplicateDropped { seq: 1 }]
        );
        let (_, again) = send(&mut s, &request_at(COMMAND_UNBOUNDED, 2, 0, &[]));
        assert_eq!(again, responses[1]);
    }

    /// Sends 65 requests of `target`, each refused `UnknownInteraction`,
    /// over a table that holds ordinal 8 under `eighth` with a bound of one
    /// hour, and checks that the first response is out of the ring after the
    /// 65th: the refused response took no bound from the member.
    fn assert_refused_requests_fall_into_the_last_64(eighth: Kind, target: Target) {
        let mut t = (*table()).clone();
        t.insert(
            IFACE,
            Ordinal(8),
            eighth,
            Some(std::time::Duration::from_secs(3600)),
        );
        let (mut s, _p, notes) = described_with(t, |c| c);
        open(&mut s);
        let mut responses = Vec::new();
        for seq in 1..=65 {
            let (step, out) = send(&mut s, &request(target, seq, &[]));
            assert_eq!(
                (step, out.len()),
                (Step::Continue, 1),
                "{target:?} request {seq}"
            );
            assert_eq!(
                decode(&out[0]).unwrap(),
                response(
                    target,
                    0,
                    seq,
                    seq,
                    ResponseOutcome::Contract(Contract::UnknownInteraction)
                )
            );
            responses.push(out);
        }
        notes.lock().unwrap().clear();
        // The 65th refused response pushed the first out of the ring, within
        // the bound the member would have given.
        let (_, again) = send(&mut s, &request(target, 1, &[]));
        assert!(again.is_empty(), "{target:?}");
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::DuplicateDropped { seq: 1 }]
        );
        let (_, again) = send(&mut s, &request(target, 2, &[]));
        assert_eq!(again, responses[1]);
    }

    /// A `request` naming an ordinal the table holds under another kind is
    /// refused, and its response takes no bound from that member: a signal's
    /// `max_us` is not a call bound. The refused response is one of the last
    /// 64.
    ///
    /// Mutation: look the bound up by `(interface, ordinal)` without the kind
    /// (the second case, whose member is a bounded query, catches it alone).
    #[test]
    fn a_refused_request_to_a_bounded_ordinal_of_another_kind_falls_into_the_last_64() {
        assert_refused_requests_fall_into_the_last_64(Kind::Signal, target(Kind::Command, 8));
        assert_refused_requests_fall_into_the_last_64(
            Kind::Signal,
            target(Kind::Command, QUERY.ord.0),
        );
        assert_refused_requests_fall_into_the_last_64(
            Kind::Signal,
            target(Kind::Query, COMMAND.ord.0),
        );
    }

    /// The frame admits a `request` under a kind that is not a call; the
    /// table holds the identity under that kind, so the form check refuses
    /// it, and the member's bound is still not a call bound. `Kind` has three
    /// such kinds, and each is walked.
    ///
    /// Mutations: drop the Command-or-Query condition and keep the kind
    /// comparison; refuse the bound for `Signal` only, or for `Signal` and
    /// `Event` only.
    #[test]
    fn a_refused_request_to_a_bounded_non_call_member_under_its_own_kind_falls_into_the_last_64() {
        for kind in [Kind::Signal, Kind::Event, Kind::Fixed] {
            assert_refused_requests_fall_into_the_last_64(kind, target(kind, 8));
        }
    }

    /// Mutation: keep the bound for `Kind::Command` only.
    #[test]
    fn sixty_five_bounded_query_responses_are_all_kept_within_their_bound() {
        let (mut s, mut p, _n) = described_scanning();
        open(&mut s);
        let first = settled(&mut s, &mut p, QUERY, 1, Ok(&[1]));
        for seq in 2..=65 {
            settled(&mut s, &mut p, QUERY, seq, Ok(&[1]));
        }
        let (_, again) = send(&mut s, &request_at(QUERY, 1, 0, &[]));
        assert_eq!(again, first);
    }

    /// The cap applies to the last-64 ring as well: a server with no table
    /// whose replies are large closes with 1008 before the ring holds 64.
    ///
    /// Mutation: leave a response with no bound out of the byte count.
    #[test]
    fn a_server_with_no_table_closes_1008_when_large_replies_pass_the_cap() {
        let notes = Notes::default();
        let rt = Loopback::new(CATALOG);
        let mut p = rt.attach();
        let config = config(&notes)
            .with_max_retained_bytes(crate::frame::MAX_MESSAGE_LIMIT)
            .unwrap();
        let mut s = Session::new(Scanning(rt), &config);
        open(&mut s);
        let big = vec![1u8; 4005];
        // Four replies of 4,005 bytes are 4,041 bytes each with their
        // headers, 16,164 bytes in all, the cap; the fifth passes it, long
        // before the ring holds 64.
        for seq in 1..=5 {
            let (_, out) = send(&mut s, &request(QUERY, seq, &[]));
            assert!(out.is_empty());
            let c = claim(&mut p, QUERY);
            p.settle(c.id, Ok(&big)).unwrap();
            let (step, out) = tick(&mut s);
            assert_eq!(out.len(), 1, "reply {seq}");
            assert_eq!(out[0].len(), 4041, "reply {seq}");
            let expected = if seq < 5 {
                Step::Continue
            } else {
                Step::Close(1008)
            };
            assert_eq!(step, expected, "reply {seq}");
        }
    }

    /// Mutation: close at `>=`.
    #[test]
    fn retained_bytes_equal_to_the_cap_do_not_close_and_one_more_byte_closes_1008() {
        let notes = Notes::default();
        let rt = Loopback::new(CATALOG);
        let mut p = rt.attach();
        // The cap cannot go below 16,164 bytes. Four `response`s of 4,041
        // bytes reach it exactly, and the next one passes it.
        let config = config(&notes)
            .with_max_retained_bytes(crate::frame::MAX_MESSAGE_LIMIT)
            .unwrap();
        let mut s = Session::new(Scanning(rt), &config);
        open(&mut s);
        let reply = vec![1u8; 4005];
        for seq in 1..=4 {
            let (_, out) = send(&mut s, &request(QUERY, seq, &[]));
            assert!(out.is_empty());
            let c = claim(&mut p, QUERY);
            p.settle(c.id, Ok(&reply)).unwrap();
            let (step, out) = tick(&mut s);
            assert_eq!(
                (step, out.len(), out[0].len()),
                (Step::Continue, 1, 4041),
                "response {seq}"
            );
        }
        let (step, out) = send(&mut s, &request(SIGNAL, 5, &[]));
        // The fifth response is still sent, then the session closes.
        assert_eq!((step, out.len()), (Step::Close(1008), 1));
    }

    /// Mutation: remove the expiry at the top of `request`.
    #[test]
    fn a_repeat_after_the_bound_with_no_tick_between_is_dropped() {
        let (mut s, mut p, notes) = described_scanning();
        open(&mut s);
        let out = settled(&mut s, &mut p, COMMAND, 1, Ok(&[]));
        assert_eq!(out.len(), 1);
        p.advance(Duration(1_000_001)); // max + 1, and no tick
        let (_, again) = send(&mut s, &request_at(COMMAND, 1, 0, &[]));
        assert!(again.is_empty());
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::DuplicateDropped { seq: 1 }]
        );
    }

    /// Mutation: keep while `now < until` (a zero bound then expires at once).
    #[test]
    fn a_bound_of_zero_keeps_the_response_until_the_clock_passes_the_receipt() {
        let mut t = (*table()).clone();
        t.insert(
            IFACE,
            COMMAND.ord,
            Kind::Command,
            Some(std::time::Duration::ZERO),
        );
        let (mut s, mut p, notes) = described_with(t, |c| c);
        open(&mut s);
        let out = settled(&mut s, &mut p, COMMAND, 1, Ok(&[]));
        assert_eq!(out.len(), 1);
        let (_, again) = send(&mut s, &request_at(COMMAND, 1, 0, &[]));
        assert_eq!(again, out);
        p.advance(Duration(1));
        tick(&mut s);
        let (_, again) = send(&mut s, &request_at(COMMAND, 1, 0, &[]));
        assert!(again.is_empty());
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::DuplicateDropped { seq: 1 }]
        );
    }

    /// Mutation: a plain `+` instead of `saturating_add` (it overflows).
    #[test]
    fn a_stamp_and_a_clock_near_the_end_of_time_saturate_the_bound() {
        let (mut s, mut p, _n) = described_scanning();
        open(&mut s);
        let late = i64::MAX - 10;
        p.advance(Duration(late));
        let (_, out) = send(&mut s, &request_at(COMMAND, 1, late, &[]));
        assert!(out.is_empty());
        let c = claim(&mut p, COMMAND);
        p.settle(c.id, Ok(&[])).unwrap();
        let (_, out) = tick(&mut s);
        assert_eq!(out.len(), 1);
        p.advance(Duration(10)); // the clock is at i64::MAX, the saturated bound
        tick(&mut s);
        let (_, again) = send(&mut s, &request_at(COMMAND, 1, late, &[]));
        assert_eq!(again, out);
    }

    #[test]
    fn the_default_retained_cap_is_sixteen_mebibytes() {
        assert_eq!(Config::default().max_retained_bytes(), 16 * 1024 * 1024);
    }

    #[test]
    fn described_new_refuses_a_table_whose_name_or_hash_differs_from_the_ports() {
        let rt = Loopback::new(CATALOG);
        let other_name = Arc::new(MemberTable::new("other", CATALOG.hash));
        let err = Described::new(Scanning(rt.attach()), other_name)
            .err()
            .expect("refused");
        assert_eq!(
            err,
            TableMismatch {
                ports: CATALOG,
                name: "other".to_owned(),
                hash: CATALOG.hash
            }
        );
        let other_hash = Arc::new(MemberTable::new("ws.test", CatalogHash([8; 32])));
        let err = Described::new(Scanning(rt.attach()), other_hash)
            .err()
            .expect("refused");
        assert_eq!(err.hash, CatalogHash([8; 32]));
        assert!(Described::new(Scanning(rt.attach()), table()).is_ok());
    }

    /// Mutation: skip the lookup (the loopback answers `Init` for any key).
    #[test]
    fn a_subscribe_of_an_ordinal_the_table_lacks_is_unknown_and_a_later_commit_sends_nothing() {
        let (mut s, mut p, notes) = described_scanning();
        open(&mut s);
        let t = target(Kind::Signal, 9);
        let (step, out) = send(&mut s, &msg(Frame::Subscribe(t)));
        assert_eq!(step, Step::Continue);
        assert_eq!(out.len(), 1);
        assert_eq!(decode(&out[0]).unwrap(), unknown(t));
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::UnknownInteraction { target: t }]
        );
        p.set(IFACE, Ordinal(9), &[1]).unwrap();
        p.commit();
        let (_, out) = tick(&mut s);
        assert!(out.is_empty());
    }

    /// Mutation: look the ordinal up without the kind.
    #[test]
    fn a_subscribe_of_the_event_ordinal_under_kind_signal_is_unknown() {
        let (mut s, _p, _n) = described_scanning();
        open(&mut s);
        let t = target(Kind::Signal, 2);
        let (_, out) = send(&mut s, &msg(Frame::Subscribe(t)));
        assert_eq!(decode(&out[0]).unwrap(), unknown(t));
        // The same ordinal under its own kind is accepted.
        let (_, out) = send(&mut s, &msg(Frame::Subscribe(target(Kind::Event, 2))));
        assert_eq!(decode(&out[0]).unwrap(), accepted(target(Kind::Event, 2)));
    }

    /// Mutation: route by form first (the reserved ordinal would reach the
    /// backend as a signal read; the signal under `Command` would be refused
    /// by the form check with the same frame, so the second half checks the
    /// note too).
    #[test]
    fn a_read_of_the_reserved_ordinal_and_a_request_of_the_signal_as_a_command_are_refused_with_the_identity_echoed(
    ) {
        let (mut s, _p, notes) = described_scanning();
        open(&mut s);
        let reserved = target(Kind::Signal, 6);
        let (_, out) = send(&mut s, &msg(Frame::Read(reserved)));
        assert_eq!(decode(&out[0]).unwrap(), unknown(reserved));
        let as_command = target(Kind::Command, 1);
        let (_, out) = send(&mut s, &request(as_command, 1, &[]));
        assert_eq!(
            decode(&out[0]).unwrap(),
            response(
                as_command,
                0,
                1,
                1,
                ResponseOutcome::Contract(Contract::UnknownInteraction)
            )
        );
        assert_eq!(
            *notes.lock().unwrap(),
            vec![
                Note::UnknownInteraction { target: reserved },
                Note::UnknownInteraction { target: as_command }
            ]
        );
    }

    /// Mutation: drop without recording.
    #[test]
    fn an_unsubscribe_of_an_unknown_key_sends_nothing_and_is_recorded() {
        let (mut s, _p, notes) = described_scanning();
        open(&mut s);
        let t = target(Kind::Event, 9);
        let (step, out) = send(&mut s, &msg(Frame::Unsubscribe(t)));
        assert_eq!((step, out.is_empty()), (Step::Continue, true));
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::UnknownInteraction { target: t }]
        );
    }

    /// Mutation: `known` ignores the kind.
    #[test]
    fn a_read_of_the_fixed_ordinal_under_kind_signal_is_unknown() {
        let (mut s, _p, notes) = described_scanning();
        open(&mut s);
        let t = target(Kind::Signal, 5);
        let (_, out) = send(&mut s, &msg(Frame::Read(t)));
        assert_eq!(out.len(), 1);
        assert_eq!(decode(&out[0]).unwrap(), unknown(t));
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::UnknownInteraction { target: t }]
        );
    }

    /// Mutation: `known` ignores the kind (the form admits `Signal`, so the
    /// frame would be dropped silently with no note).
    #[test]
    fn an_unsubscribe_of_a_known_ordinal_under_the_wrong_kind_is_dropped_and_recorded() {
        let (mut s, _p, notes) = described_scanning();
        open(&mut s);
        let t = target(Kind::Signal, 2);
        let (step, out) = send(&mut s, &msg(Frame::Unsubscribe(t)));
        assert_eq!((step, out.is_empty()), (Step::Continue, true));
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::UnknownInteraction { target: t }]
        );
    }

    /// Review Focus item 1: the table lists the command, no handler serves it.
    #[test]
    fn a_listed_command_no_handler_serves_stays_in_flight() {
        let (mut s, _p, notes) = described_scanning();
        open(&mut s);
        let (_, out) = send(&mut s, &request(COMMAND, 1, &[]));
        assert!(out.is_empty());
        let (_, out) = tick(&mut s);
        assert!(out.is_empty());
        assert!(notes.lock().unwrap().is_empty());
    }

    /// A loopback whose signal reads are counted. It presents every port the
    /// two adapters need.
    struct CountingPorts {
        inner: Loopback,
        reads: Rc<Cell<usize>>,
    }

    impl Attached for CountingPorts {
        fn catalog(&self) -> &CatalogRef {
            self.inner.catalog()
        }
    }

    impl Encoded for CountingPorts {
        type Encoding = <Loopback as Encoded>::Encoding;
    }

    impl Clock for CountingPorts {
        fn now(&self) -> Timestamp {
            self.inner.now()
        }
    }

    impl SignalReader for CountingPorts {
        fn read(
            &self,
            iface: InterfaceNo,
            ord: Ordinal,
            out: &mut [u8],
        ) -> Result<RawSample, ReadError> {
            self.reads.set(self.reads.get() + 1);
            self.inner.read(iface, ord, out)
        }
    }

    impl ScannableSignals for CountingPorts {
        fn generation(&self, iface: InterfaceNo) -> u64 {
            self.inner.generation(iface)
        }

        fn scan(&self, marks: &mut [Watermark], out: &mut [Changed]) -> usize {
            self.inner.scan(marks, out)
        }
    }

    impl FixedReader for CountingPorts {
        fn read_fixed(
            &self,
            iface: InterfaceNo,
            ord: Ordinal,
            out: &mut [u8],
        ) -> Result<usize, ReadError> {
            self.inner.read_fixed(iface, ord, out)
        }
    }

    impl EventSource for CountingPorts {
        fn subscribe(
            &mut self,
            iface: InterfaceNo,
            ords: &[Ordinal],
        ) -> Result<(), SubscribeError> {
            self.inner.subscribe(iface, ords)
        }
        fn unsubscribe(&mut self, iface: InterfaceNo, ords: &[Ordinal]) {
            self.inner.unsubscribe(iface, ords);
        }
        fn next(&mut self, out: &mut [u8]) -> Result<Option<RawOccurrence>, ReadError> {
            self.inner.next(out)
        }
    }

    impl Caller for CountingPorts {
        fn command(
            &mut self,
            iface: InterfaceNo,
            ord: Ordinal,
            args: &[u8],
            trace: Option<TraceContext>,
        ) -> Result<Correlation, SendError> {
            self.inner.command(iface, ord, args, trace)
        }
        fn query(
            &mut self,
            iface: InterfaceNo,
            ord: Ordinal,
            args: &[u8],
            trace: Option<TraceContext>,
        ) -> Result<Correlation, SendError> {
            self.inner.query(iface, ord, args, trace)
        }
        fn ack(&mut self, c: Correlation) -> Option<Result<(), CallError>> {
            self.inner.ack(c)
        }
        fn reply(
            &mut self,
            c: Correlation,
            out: &mut [u8],
        ) -> Result<Option<Result<usize, CallError>>, ReadError> {
            self.inner.reply(c, out)
        }
        fn forget(&mut self, c: Correlation) {
            self.inner.forget(c);
        }
    }

    /// Subscribes 1000 keys the table lacks, commits a value under each, and
    /// checks that a tick publishes nothing and reads nothing.
    fn assert_unknown_subscribes_leave_no_state<B: Backend>(
        mut s: Session<B>,
        mut p: Loopback,
        reads: &Cell<usize>,
        mode: &str,
    ) {
        open(&mut s);
        for ord in 100..1100u32 {
            let (_, out) = send(&mut s, &msg(Frame::Subscribe(target(Kind::Signal, ord))));
            assert_eq!(decode(&out[0]).unwrap(), unknown(target(Kind::Signal, ord)));
            p.set(IFACE, Ordinal(ord), &[1]).unwrap();
        }
        p.commit();
        reads.set(0);
        let (_, out) = tick(&mut s);
        assert!(out.is_empty(), "{mode}");
        assert_eq!(reads.get(), 0, "{mode}: a tick read a signal");
    }

    /// Mutation: insert the signal state before the lookup.
    #[test]
    fn a_thousand_unknown_subscribes_leave_no_signal_state_in_either_mode() {
        for scanning in [false, true] {
            let rt = Loopback::new(CATALOG);
            let provider = rt.attach();
            let reads = Rc::new(Cell::new(0));
            let ports = CountingPorts {
                inner: rt,
                reads: Rc::clone(&reads),
            };
            if scanning {
                let b = Described::new(Scanning(ports), table()).unwrap();
                let s = Session::new(b, &Config::default());
                assert_unknown_subscribes_leave_no_state(s, provider, &reads, "scanning");
            } else {
                let b = Described::new(Polling(ports), table()).unwrap();
                let s = Session::new(b, &Config::default());
                assert_unknown_subscribes_leave_no_state(s, provider, &reads, "polling");
            }
        }
    }
}
