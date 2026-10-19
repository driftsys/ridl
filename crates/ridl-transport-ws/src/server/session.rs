//! The sans-IO session core: decoded frames and backend samples in, frames
//! to send out. It does no IO, so the session rules are tested without a
//! socket; `serve` is the IO around it.

use std::collections::BTreeMap;
use std::time::Instant;

use ridl_rt::contract::{CatalogRef, InterfaceNo, Kind, Ordinal};
use ridl_rt::encoding::Encoding;
use ridl_rt::error::Contract;
use ridl_rt::port::{
    Attached, Changed, Clock, Encoded, RawSample, ReadError, SignalReader, Watermark,
};
use ridl_rt::sample::{Envelope, Provenance, Timestamp};

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
        // The header of a `publish`: what the message limit leaves for a
        // payload is the limit minus this.
        let empty_publish = Frame::Publish {
            target: signal_target((InterfaceNo(1), Ordinal(1))),
            envelope: Envelope {
                stamp: Timestamp(0),
                seq: 0,
            },
            state: PublishState::Live(&[]),
        };
        Session {
            backend,
            catalog,
            encoding,
            payload_cap: config.max_message_bytes() - empty_publish.encoded_len(),
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
            response_seq: 0,
            buf: vec![0; FIRST_BUFFER],
        }
    }

    /// Handles one message from the client. Each entry pushed to `out` is one
    /// encoded frame. `now` is the wall time of the message; the session
    /// does not use it yet (calls, which it will time, are not served yet).
    /// After a [`Step::Close`], every call returns that close and pushes
    /// nothing.
    pub(crate) fn on_message(&mut self, msg: &[u8], _now: Instant, out: &mut Vec<Vec<u8>>) -> Step {
        if let Some(code) = self.closed {
            return Step::Close(code);
        }
        let step = self.handle(msg, out);
        self.keep_closed(step)
    }

    /// Polls the backend once and pushes what changed to `out`. `now` is the
    /// wall time of the tick; the session does not use it yet (see
    /// [`Session::on_message`], which also states the rule after a close).
    pub(crate) fn tick(&mut self, _now: Instant, out: &mut Vec<Vec<u8>>) -> Step {
        if let Some(code) = self.closed {
            return Step::Close(code);
        }
        let step = self.poll(out);
        self.keep_closed(step)
    }

    /// Records a close, so that later calls return it.
    fn keep_closed(&mut self, step: Step) -> Step {
        if let Step::Close(code) = step {
            self.closed = Some(code);
        }
        step
    }

    fn handle(&mut self, msg: &[u8], out: &mut Vec<Vec<u8>>) -> Step {
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
                target, envelope, ..
            } => self.request(target, envelope, out),
        }
    }

    /// One pass over the signals. In scanning mode each `Changed` entry of a
    /// subscribed signal is sent, because the backend states that the
    /// channel changed and the mark set at subscribe excludes earlier
    /// commits. In polling mode every subscribed signal is read and sent
    /// when it differs in any field from the sample last sent.
    fn poll(&mut self, out: &mut Vec<Vec<u8>>) -> Step {
        if !self.attached {
            return Step::Continue;
        }
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
        // Occurrences and call outcomes are not served yet: their draining
        // goes here, after the signals.
        Step::Continue
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
            // Events are not served yet: the event path takes `Kind::Event`
            // out of this arm. Until then a subscribe of an event is refused
            // as unknown, like a kind the form does not admit.
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
            // Events are not served yet: the event path forgets the
            // subscription here.
            Kind::Event => {}
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

    fn request(&mut self, target: Target, envelope: Envelope, out: &mut Vec<Vec<u8>>) -> Step {
        // Calls are not served yet: the call path takes `Kind::Command` and
        // `Kind::Query` out of this function, to send the request to the
        // backend. Until then every request is refused as unknown, like one
        // whose kind the form does not admit.
        self.respond(
            target,
            envelope.seq,
            ResponseOutcome::Contract(Contract::UnknownInteraction),
            out,
        );
        Step::Continue
    }

    /// Sends one `response`, under the next response `seq` of the session
    /// and the backend's clock as the stamp.
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
    use std::cell::Cell;
    use std::rc::Rc;
    use std::sync::{Arc, Mutex};
    use std::time::Instant;

    use ridl_loopback::Loopback;
    use ridl_rt::contract::{CatalogHash, CatalogRef, InterfaceNo, Kind, Ordinal};
    use ridl_rt::error::{CallError, Contract};
    use ridl_rt::port::{
        Attached, Caller, Clock, Correlation, Encoded, EventSource, RawOccurrence, RawSample,
        ReadError, SendError, SignalReader, SignalWriter, SubscribeError,
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
}
