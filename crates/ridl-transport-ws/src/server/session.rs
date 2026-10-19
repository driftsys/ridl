//! The sans-IO session core: decoded frames and backend samples in, frames
//! to send out. It does no IO, so the session rules are tested without a
//! socket; `serve` is the IO around it.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use ridl_rt::contract::{CatalogRef, InterfaceNo, Kind, Ordinal};
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

    /// Ends the session: every backend call in flight is forgotten, and the
    /// backend's subscriptions go with the backend value.
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
            Err(DecodeError::Discard(d)) => {
                self.note(Note::Discarded(d));
                return Step::Continue;
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
            | Frame::Response { .. } => {
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

    /// Sends every waiting occurrence, each with the backend's stamp and the
    /// next session `seq` of its channel. The trace context is dropped: the
    /// frame does not carry one. An occurrence that does not fit one message
    /// is read and dropped, and recorded, because `next` does not consume
    /// what it cannot copy: left in the queue, it would block every
    /// occurrence behind it.
    fn drain_occurrences(&mut self, out: &mut Vec<Vec<u8>>) -> Step {
        loop {
            let occurrence = match self.backend.ports().next(&mut self.buf) {
                Ok(Some(occurrence)) => occurrence,
                Ok(None) => return Step::Continue,
                Err(ReadError::Short { needed }) => {
                    self.buf.resize(needed, 0);
                    continue;
                }
                Err(_) => return Step::Close(CloseCode::GOING_AWAY.0),
            };
            let target = Target {
                kind: Kind::Event,
                iface: occurrence.iface,
                ord: occurrence.ord,
            };
            if occurrence.len > self.occurrence_cap {
                self.note(Note::ValueTooLarge {
                    target,
                    needed: occurrence.len,
                });
                continue;
            }
            let counter = self.events.entry((target.iface, target.ord)).or_default();
            *counter += 1;
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
        }
    }

    /// Reads the outcome of each call in flight. A known outcome is one
    /// `response`, or nothing for the three outcomes that never cross (frame
    /// specification §9.6); either way the call is then forgotten. A call
    /// still unknown `call_abandon_after` after its request arrived is
    /// forgotten and recorded, so that it does not hold a backend slot until
    /// the client disconnects.
    fn poll_calls(&mut self, now: Instant, out: &mut Vec<Vec<u8>>) -> Step {
        for call in self.calls.in_flight() {
            let outcome = match call.target.kind {
                Kind::Command => self
                    .backend
                    .ports()
                    .ack(call.correlation)
                    .map(Outcome::from),
                _ => match self.backend.ports().reply(call.correlation, &mut self.buf) {
                    Ok(known) => known.map(Outcome::from),
                    Err(ReadError::Short { needed }) => {
                        if needed > self.reply_cap {
                            self.note(Note::ValueTooLarge {
                                target: call.target,
                                needed,
                            });
                            self.forget(call.correlation);
                            continue;
                        }
                        self.buf.resize(needed, 0);
                        match self.backend.ports().reply(call.correlation, &mut self.buf) {
                            Ok(known) => known.map(Outcome::from),
                            Err(_) => return Step::Close(CloseCode::GOING_AWAY.0),
                        }
                    }
                    Err(_) => return Step::Close(CloseCode::GOING_AWAY.0),
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
            if let Some(response) = response {
                self.respond(call.target, call.seq, response, out);
            }
            self.forget(call.correlation);
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
        } else if *attach.hash() != self.catalog.hash {
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

    fn answer(&self, target: Target, outcome: AnswerOutcome, out: &mut Vec<Vec<u8>>) {
        push(out, &Frame::Answer { target, outcome });
    }

    fn subscribe(&mut self, target: Target, out: &mut Vec<Vec<u8>>) -> Step {
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
        match target.kind {
            Kind::Signal => match self.fetch_signal((target.iface, target.ord)) {
                Fetch::Sample(sample) => self.publish_signal(target, sample, false, out),
                Fetch::TooLarge(needed) => self.note(Note::ValueTooLarge { target, needed }),
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
    /// the backend's clock as the stamp.
    fn read_fixed(&mut self, target: Target, out: &mut Vec<Vec<u8>>) -> Step {
        let len = loop {
            match self
                .backend
                .read_fixed(target.iface, target.ord, &mut self.buf)
            {
                Ok(len) => break len,
                Err(ReadError::Short { needed }) => {
                    if needed > self.payload_cap {
                        self.note(Note::ValueTooLarge { target, needed });
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
    /// answered at once.
    fn request(
        &mut self,
        target: Target,
        envelope: Envelope,
        payload: &[u8],
        now: Instant,
        out: &mut Vec<Vec<u8>>,
    ) -> Step {
        let seq = envelope.seq;
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
        let ports = self.backend.ports();
        let sent = match target.kind {
            Kind::Command => ports.command(target.iface, target.ord, payload, None),
            Kind::Query => ports.query(target.iface, target.ord, payload, None),
            _ => {
                self.respond(
                    target,
                    seq,
                    ResponseOutcome::Contract(Contract::UnknownInteraction),
                    out,
                );
                return Step::Continue;
            }
        };
        let outcome = match sent {
            Ok(correlation) => {
                self.calls.track(InFlight {
                    correlation,
                    target,
                    seq,
                    sent: now,
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
        self.respond(target, seq, outcome, out);
        Step::Continue
    }

    /// Sends one `response`, under the next response `seq` of the session
    /// and the backend's clock as the stamp, and keeps it for a repeat of
    /// the request.
    fn respond(
        &mut self,
        target: Target,
        correlation: u64,
        outcome: ResponseOutcome<'_>,
        out: &mut Vec<Vec<u8>>,
    ) {
        self.response_seq += 1;
        let stamp = self.backend.ports().now();
        push(
            out,
            &Frame::Response {
                target,
                envelope: Envelope {
                    stamp,
                    seq: self.response_seq,
                },
                correlation,
                outcome,
            },
        );
        let response = out.last().expect("the response was just pushed");
        self.calls.settle(correlation, response);
    }

    /// Reads a signal into the session's buffer, growing it on `Short` up to
    /// the payload the message limit allows.
    fn fetch_signal(&mut self, key: Key) -> Fetch {
        loop {
            match self.backend.ports().read(key.0, key.1, &mut self.buf) {
                Ok(sample) => return Fetch::Sample(sample),
                Err(ReadError::Short { needed }) => {
                    if needed > self.payload_cap {
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
    fn publish_signal(
        &mut self,
        target: Target,
        sample: RawSample,
        fresh: bool,
        out: &mut Vec<Vec<u8>>,
    ) {
        let ch = self.signals.entry((target.iface, target.ord)).or_default();
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

    /// Scans every marked interface and returns the subscribed signals that
    /// changed. The buffer grows when a scan writes nothing while a mark is
    /// still behind its generation, as the port's documentation describes.
    fn scan_changes(&mut self) -> Vec<Key> {
        let mut keys = Vec::new();
        let Some(scannable) = self.backend.scannable() else {
            return keys;
        };
        if self.marks.is_empty() {
            return keys;
        }
        loop {
            let n = scannable.scan(&mut self.marks, &mut self.changes);
            if n == 0 {
                let behind = self
                    .marks
                    .iter()
                    .any(|m| m.generation < scannable.generation(m.iface));
                if !behind {
                    return keys;
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
                continue;
            }
            keys.extend(
                self.changes[..n]
                    .iter()
                    .map(|c| (c.iface, c.ord))
                    .filter(|key| self.signals.get(key).is_some_and(|ch| ch.subscribed)),
            );
        }
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
    /// The value needs more bytes than one message may carry.
    TooLarge(usize),
    Contract(Contract),
    /// The port cannot be read: the runtime behind it is gone.
    Gone,
}

fn signal_target(key: Key) -> Target {
    Target {
        kind: Kind::Signal,
        iface: key.0,
        ord: key.1,
    }
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
    use ridl_rt::contract::{CatalogHash, CatalogRef, InterfaceNo, Kind, Ordinal};
    use ridl_rt::error::{CallError, Contract, Transport};
    use ridl_rt::payload::{Rule, Violation};
    use ridl_rt::port::{
        Attached, Caller, Claim, Clock, Correlation, Encoded, EventSink, EventSource, Handler,
        RawOccurrence, RawSample, ReadError, SendError, SignalReader, SignalWriter, SubscribeError,
    };
    use ridl_rt::sample::{Cause, Duration, Envelope, Freshness, Provenance, Timestamp};
    use ridl_rt::trace::TraceContext;

    use super::{Session, Step};
    use crate::frame::{
        decode, AnswerOutcome, Attach, AttachOutcome, Frame, PublishState, Reason, ResponseOutcome,
        Target,
    };
    use crate::server::{Backend, Config, Note, Polling, Scanning};

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

    #[test]
    fn a_read_publishes_once_and_does_not_subscribe() {
        let (mut s, mut p, _n) = scanning();
        open(&mut s);
        commit(&mut p, &[1]);
        let (step, out) = send(&mut s, &msg(Frame::Read(SIGNAL)));
        assert_eq!(step, Step::Continue);
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            publish(SIGNAL, 0, 1, PublishState::Live(&[1]))
        );
        commit(&mut p, &[2]);
        let (_, out) = tick(&mut s);
        assert!(out.is_empty());
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
        let (mut s, mut p, _n) = scanning();
        open(&mut s);
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
        p.provision_fixed(IFACE, Ordinal(5), &[0xF1, 0xF2]);
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
        p.provision_fixed(IFACE, Ordinal(5), &[1]);
        let (_, out) = send(&mut s, &msg(Frame::Read(FIXED)));
        assert_eq!(out.len(), 1);
        assert_eq!(decode(&out[0]).unwrap(), unknown(FIXED));
    }

    #[test]
    fn a_subscribe_of_a_fixed_is_unknown_interaction() {
        let (mut s, mut p, _n) = scanning();
        open(&mut s);
        p.provision_fixed(IFACE, Ordinal(5), &[1]);
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
        let (mut s, mut p, _n) = scanning();
        open(&mut s);
        send(&mut s, &msg(Frame::Subscribe(EVENT)));
        let value = vec![0xCD; 4096];
        raise(&mut p, EVENT, &value);
        let (_, out) = tick(&mut s);
        assert_eq!(out.len(), 1);
        assert_eq!(decode(&out[0]).unwrap(), occurrence(EVENT, 0, 1, &value));
    }

    /// An occurrence that does not fit one message is recorded and dropped,
    /// and the ones behind it in the queue still reach the client.
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
        assert_eq!(decode(&out[0]).unwrap(), occurrence(EVENT, 0, 1, &[1]));
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::ValueTooLarge {
                target: EVENT,
                needed: 2000
            }]
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
        msg(Frame::Request {
            target,
            envelope: Envelope {
                stamp: Timestamp(0),
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
    fn settled(
        s: &mut Session<Scanning<Loopback>>,
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

    /// A reply that does not fit one message is recorded, and the call is
    /// forgotten: its slot takes a new call.
    #[test]
    fn a_reply_larger_than_the_message_limit_is_recorded_and_not_sent() {
        let notes = Notes::default();
        let rt = Loopback::new(CATALOG);
        let mut p = rt.attach();
        let config = config(&notes)
            .with_max_message_bytes(1024)
            .expect("above the minimum");
        let mut s = Session::new(Scanning(rt), &config);
        open(&mut s);
        let out = settled(&mut s, &mut p, QUERY, 1, Ok(&[1; 2000]));
        assert!(out.is_empty());
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::ValueTooLarge {
                target: QUERY,
                needed: 2000
            }]
        );
        assert!(send_commands(&mut s, 2, Loopback::SLOTS as u64).is_empty());
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
        /// `EventSource::next` answers `Detached`.
        Next,
        /// `Caller::command` answers `Detached`.
        Command,
        /// `Caller::query` answers `Detached`.
        Query,
        /// `Caller::reply` answers `Detached`.
        Reply,
        /// `Caller::command` answers `TooLarge`.
        CommandTooLarge,
    }

    /// Ports that delegate to a loopback, but for one call that fails, and
    /// that record what is forgotten.
    struct Faulty {
        inner: Loopback,
        fail: Fail,
        forgotten: Rc<RefCell<Vec<Correlation>>>,
    }

    fn faulty(fail: Fail) -> (Session<Polling<Faulty>>, Rc<RefCell<Vec<Correlation>>>) {
        let forgotten = Rc::new(RefCell::new(Vec::new()));
        let backend = Polling(Faulty {
            inner: Loopback::new(CATALOG),
            fail,
            forgotten: Rc::clone(&forgotten),
        });
        let mut s = Session::new(backend, &Config::default());
        open(&mut s);
        (s, forgotten)
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
            self.inner.read(iface, ord, out)
        }
    }

    impl EventSource for Faulty {
        fn subscribe(
            &mut self,
            iface: InterfaceNo,
            ords: &[Ordinal],
        ) -> Result<(), SubscribeError> {
            if self.fail == Fail::Subscribe {
                return Err(SubscribeError::Detached);
            }
            self.inner.subscribe(iface, ords)
        }
        fn unsubscribe(&mut self, iface: InterfaceNo, ords: &[Ordinal]) {
            self.inner.unsubscribe(iface, ords);
        }
        fn next(&mut self, out: &mut [u8]) -> Result<Option<RawOccurrence>, ReadError> {
            if self.fail == Fail::Next {
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
            match self.fail {
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
            if self.fail == Fail::Query {
                return Err(SendError::Detached);
            }
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
            if self.fail == Fail::Reply {
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
        let (mut s, forgotten) = faulty(Fail::None);
        send(&mut s, &request(COMMAND, 1, &[]));
        send(&mut s, &request(QUERY, 2, &[]));
        assert!(forgotten.borrow().is_empty());
        s.close();
        let mut forgotten = forgotten.borrow().clone();
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
        ];
        for (fail, m) in cases {
            let (mut s, _) = faulty(fail);
            let (step, out) = send(&mut s, &m);
            assert_eq!(step, Step::Close(1001), "{fail:?}");
            assert!(out.is_empty(), "{fail:?}");
        }
        let (mut s, _) = faulty(Fail::Next);
        let (step, out) = tick(&mut s);
        assert_eq!(step, Step::Close(1001));
        assert!(out.is_empty());
        let (mut s, _) = faulty(Fail::Reply);
        let (step, _) = send(&mut s, &request(QUERY, 1, &[]));
        assert_eq!(step, Step::Continue);
        let (step, out) = tick(&mut s);
        assert_eq!(step, Step::Close(1001));
        assert!(out.is_empty());
    }

    #[test]
    fn a_send_refused_as_too_large_is_answered_corrupt() {
        let (mut s, _) = faulty(Fail::CommandTooLarge);
        let (step, out) = send(&mut s, &request(COMMAND, 1, &[1; 9]));
        assert_eq!(step, Step::Continue);
        assert_eq!(out.len(), 1);
        assert_eq!(
            decode(&out[0]).unwrap(),
            response(COMMAND, 0, 1, 1, ResponseOutcome::Corrupt)
        );
    }
}
