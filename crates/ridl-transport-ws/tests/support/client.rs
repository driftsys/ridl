//! A test-only consuming runtime over WebSocket: `tungstenite` for RFC 6455
//! and this crate's codec for the frame. It is written once for the scenario
//! and is not published.
//!
//! It applies the consumer rules of the frame specification §5 and §6: one
//! held sample per subscribed signal, replaced only by a `publish` with a
//! greater `seq` (§5.1, §7); one queue of occurrences in the order received
//! (§5.2); and a map of calls in flight keyed by the request's own `seq`,
//! which each `response` names as its correlation (§5.3, §5.4). A `fixed` is
//! read once and cached for the session (§5.5).
//!
//! It is not a full runtime: it computes no freshness (every sample is
//! `Unbounded`, as on `ridl-loopback`), applies no time to live, records no
//! loss, and detects no `Undelivered` or `Timeout`. A wait it makes for the
//! peer, and a connection that fails, fail the test.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::ErrorKind;
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use ridl_rt::contract::{CatalogRef, InterfaceNo, Kind, Ordinal};
use ridl_rt::error::{CallError, Contract, Transport};
use ridl_rt::port::{
    Attached, Caller, Correlation, EventSource, FixedReader, RawOccurrence, RawSample, ReadError,
    SendError, SignalReader, SubscribeError,
};
use ridl_rt::sample::{Cause, Envelope, Freshness, Provenance, Timestamp};
use ridl_rt::trace::TraceContext;
use ridl_transport_ws::frame::{
    decode, AnswerOutcome, Attach, AttachOutcome, Frame, PublishState, ResponseOutcome, Target,
    FRAME_VERSION, SUBPROTOCOL,
};
use tungstenite::client::IntoClientRequest;
use tungstenite::{Message, WebSocket};

use super::provider::{IFACE, SIGNAL};
use super::scenario::LIMIT;

/// How long one `pump` waits for a message when none is waiting.
const PUMP_WAIT: Duration = Duration::from_millis(1);

/// The consumer's ports over one WebSocket session.
pub struct WsClient {
    catalog: CatalogRef,
    // `read_fixed` takes `&self` and sends a `read`, so the session state is
    // behind a `RefCell`.
    inner: RefCell<Inner>,
}

/// The held sample of one subscribed signal (§5.1).
struct Held {
    envelope: Envelope,
    provenance: Provenance,
    /// The last good value: the init value (no bytes, here) until a `Live`.
    value: Vec<u8>,
}

/// An occurrence in the queue (§5.2).
struct Queued {
    iface: InterfaceNo,
    ord: Ordinal,
    envelope: Envelope,
    payload: Vec<u8>,
}

/// The outcome of a call in flight, once its `response` arrived.
enum Outcome {
    Accepted,
    Reply(Vec<u8>),
    Contract(Contract),
    Corrupt,
    Busy,
}

/// A call in flight (§5.3, §5.4).
struct InFlight {
    kind: Kind,
    outcome: Option<Outcome>,
}

struct Inner {
    ws: WebSocket<TcpStream>,
    start: Instant,
    /// The `seq` of the next request: one counter over every call (§7).
    next_seq: u64,
    signals: HashMap<(InterfaceNo, Ordinal), Option<Held>>,
    events: HashSet<(InterfaceNo, Ordinal)>,
    queue: VecDeque<Queued>,
    calls: HashMap<u64, InFlight>,
    fixed: HashMap<(InterfaceNo, Ordinal), Vec<u8>>,
    /// The `answer` frames not yet taken by the `subscribe` or the `read`
    /// that waits for them.
    answers: VecDeque<(Target, AnswerOutcome)>,
}

impl WsClient {
    /// Connects to `addr`, attaches to `catalog` under `encoding`, and
    /// subscribes [`SIGNAL`] (§6.2 permits a runtime to subscribe its signals
    /// at attach), so that a read of it is local. Returns once the signal's
    /// first `publish` is held. Fails the test when the peer refuses, or when
    /// a step does not finish within the limit.
    pub fn connect(addr: SocketAddr, catalog: CatalogRef, encoding: u8) -> Self {
        let stream = TcpStream::connect_timeout(&addr, LIMIT).expect("connected");
        stream
            .set_read_timeout(Some(LIMIT))
            .expect("read timeout set");
        stream
            .set_write_timeout(Some(LIMIT))
            .expect("write timeout set");
        let mut request = format!("ws://{addr}/")
            .into_client_request()
            .expect("a request");
        request.headers_mut().insert(
            "Sec-WebSocket-Protocol",
            SUBPROTOCOL.parse().expect("a header value"),
        );
        let (ws, _) = tungstenite::client(request, stream).expect("the handshake succeeds");

        let mut inner = Inner {
            ws,
            start: Instant::now(),
            next_seq: 1,
            signals: HashMap::new(),
            events: HashSet::new(),
            queue: VecDeque::new(),
            calls: HashMap::new(),
            fixed: HashMap::new(),
            answers: VecDeque::new(),
        };
        let attach = Attach::new(FRAME_VERSION, encoding, catalog.hash, catalog.name)
            .expect("a valid attach");
        inner.send(Frame::Attach(attach));
        let msg = inner.recv_blocking();
        assert_eq!(
            decode(&msg).expect("a frame"),
            Frame::Attached(AttachOutcome::Accepted),
            "the provider accepts the attach"
        );
        inner
            .ws
            .get_ref()
            .set_read_timeout(Some(PUMP_WAIT))
            .expect("read timeout set");

        let target = Target {
            kind: Kind::Signal,
            iface: IFACE,
            ord: SIGNAL,
        };
        inner.signals.insert((IFACE, SIGNAL), None);
        inner.send(Frame::Subscribe(target));
        let outcome = inner.wait_answer(target);
        assert_eq!(outcome, AnswerOutcome::Accepted, "the signal is subscribed");
        inner.wait("the signal's first publish", |i| {
            i.signals[&(IFACE, SIGNAL)].is_some()
        });

        WsClient {
            catalog,
            inner: RefCell::new(inner),
        }
    }

    /// Reads every message waiting on the connection and applies it.
    pub fn pump(&mut self) {
        self.inner.get_mut().pump();
    }
}

impl Inner {
    fn now(&self) -> Timestamp {
        Timestamp(i64::try_from(self.start.elapsed().as_micros()).unwrap_or(i64::MAX))
    }

    fn send(&mut self, frame: Frame<'_>) {
        let mut buf = vec![0; frame.encoded_len()];
        frame.encode(&mut buf).expect("the buffer fits the frame");
        self.ws
            .send(Message::binary(buf))
            .expect("the message is sent");
    }

    /// Reads the next binary message, under the connection's read timeout.
    /// `None` when none arrived in time. Control messages are skipped;
    /// `tungstenite` answers a ping on its own.
    fn recv(&mut self) -> Option<Vec<u8>> {
        loop {
            match self.ws.read() {
                Ok(Message::Binary(b)) => return Some(b.to_vec()),
                Ok(Message::Ping(_) | Message::Pong(_)) => {}
                Ok(other) => panic!("the provider sent {other:?}"),
                Err(tungstenite::Error::Io(e))
                    if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
                {
                    return None;
                }
                Err(e) => panic!("the connection failed: {e}"),
            }
        }
    }

    /// Reads the next binary message, failing the test when none arrives
    /// within the read timeout.
    fn recv_blocking(&mut self) -> Vec<u8> {
        self.recv().expect("a message within the limit")
    }

    fn pump(&mut self) {
        while let Some(msg) = self.recv() {
            self.apply(&msg);
        }
    }

    /// Pumps until `done` holds, failing the test after the limit.
    fn wait(&mut self, what: &str, mut done: impl FnMut(&mut Self) -> bool) {
        let start = Instant::now();
        while !done(self) {
            assert!(start.elapsed() < LIMIT, "{what} within {LIMIT:?}");
            self.pump();
        }
    }

    /// Pumps until an `answer` for `target` arrives, and returns its outcome.
    fn wait_answer(&mut self, target: Target) -> AnswerOutcome {
        let mut found = None;
        self.wait("the answer", |i| {
            if let Some(at) = i.answers.iter().position(|(t, _)| *t == target) {
                found = i.answers.remove(at).map(|(_, o)| o);
            }
            found.is_some()
        });
        found.expect("an answer")
    }

    /// Applies one message by the consumer rules of §5 and §6.
    fn apply(&mut self, msg: &[u8]) {
        match decode(msg).expect("the provider sends well-formed frames") {
            Frame::Answer { target, outcome } => self.answers.push_back((target, outcome)),
            Frame::Publish {
                target,
                envelope,
                state,
            } => match target.kind {
                Kind::Signal => self.apply_signal(target, envelope, state),
                Kind::Fixed => {
                    if let PublishState::Live(bytes) = state {
                        self.fixed
                            .insert((target.iface, target.ord), bytes.to_vec());
                    }
                }
                _ => {}
            },
            Frame::Block { .. } => panic!(
                "the test client has no support for `block` frames: it only reads a `fixed` \
                 that one `publish` carries, so provision a smaller value or add the support"
            ),
            Frame::Occurrence {
                target,
                envelope,
                payload,
            } => {
                // An occurrence of an event not subscribed is in flight from
                // before an `unsubscribe`, and is discarded (§6.2).
                if self.events.contains(&(target.iface, target.ord)) {
                    self.queue.push_back(Queued {
                        iface: target.iface,
                        ord: target.ord,
                        envelope,
                        payload: payload.to_vec(),
                    });
                }
            }
            Frame::Response {
                target,
                correlation,
                outcome,
                ..
            } => {
                // A response that names no call in flight, or a call of
                // another kind, is discarded (§5.3).
                let Some(call) = self.calls.get_mut(&correlation) else {
                    return;
                };
                if call.kind != target.kind || call.outcome.is_some() {
                    return;
                }
                call.outcome = match (call.kind, outcome) {
                    (Kind::Command, ResponseOutcome::Accepted) => Some(Outcome::Accepted),
                    (Kind::Query, ResponseOutcome::Reply(b)) => Some(Outcome::Reply(b.to_vec())),
                    (_, ResponseOutcome::Contract(c)) => Some(Outcome::Contract(c)),
                    (_, ResponseOutcome::Corrupt) => Some(Outcome::Corrupt),
                    (_, ResponseOutcome::Busy) => Some(Outcome::Busy),
                    _ => None,
                };
            }
            other => panic!("the provider sent a consumer's frame: {other:?}"),
        }
    }

    /// §5.1: a `publish` replaces the held sample only when its `seq` is
    /// greater than the held one's.
    fn apply_signal(&mut self, target: Target, envelope: Envelope, state: PublishState<'_>) {
        // A signal not subscribed is in flight from before an `unsubscribe`.
        let Some(slot) = self.signals.get_mut(&(target.iface, target.ord)) else {
            return;
        };
        if let Some(held) = slot {
            if envelope.seq <= held.envelope.seq {
                return;
            }
        }
        let last_good = slot.take().map(|h| h.value).unwrap_or_default();
        let (provenance, value) = match state {
            PublishState::Live(bytes) => (Provenance::Live, bytes.to_vec()),
            // The init value: the test catalog's is no bytes.
            PublishState::Init => (Provenance::Init, Vec::new()),
            PublishState::InvalidDeclared => (Provenance::Invalid(Cause::Declared), last_good),
        };
        *slot = Some(Held {
            envelope,
            provenance,
            value,
        });
    }

    fn call(&mut self, kind: Kind, iface: InterfaceNo, ord: Ordinal, args: &[u8]) -> Correlation {
        let seq = self.next_seq;
        self.next_seq += 1;
        let envelope = Envelope {
            stamp: self.now(),
            seq,
        };
        self.send(Frame::Request {
            target: Target { kind, iface, ord },
            envelope,
            payload: args,
        });
        self.calls.insert(
            seq,
            InFlight {
                kind,
                outcome: None,
            },
        );
        Correlation(seq)
    }
}

fn copy(bytes: &[u8], out: &mut [u8]) -> Result<usize, ReadError> {
    let dst = out.get_mut(..bytes.len()).ok_or(ReadError::Short {
        needed: bytes.len(),
    })?;
    dst.copy_from_slice(bytes);
    Ok(bytes.len())
}

impl Attached for WsClient {
    fn catalog(&self) -> &CatalogRef {
        &self.catalog
    }
}

impl SignalReader for WsClient {
    /// Local: answers from the held sample. A signal not subscribed has none
    /// and answers `UnknownInteraction`.
    fn read(
        &self,
        iface: InterfaceNo,
        ord: Ordinal,
        out: &mut [u8],
    ) -> Result<RawSample, ReadError> {
        let inner = self.inner.borrow();
        let Some(Some(held)) = inner.signals.get(&(iface, ord)) else {
            return Err(ReadError::Contract(Contract::UnknownInteraction));
        };
        let len = copy(&held.value, out)?;
        Ok(RawSample {
            provenance: held.provenance,
            freshness: Freshness::Unbounded,
            envelope: held.envelope,
            len,
        })
    }
}

impl FixedReader for WsClient {
    /// Sends a `read` and pumps until its `publish` arrives, or answers from
    /// the session's cache.
    fn read_fixed(
        &self,
        iface: InterfaceNo,
        ord: Ordinal,
        out: &mut [u8],
    ) -> Result<usize, ReadError> {
        let mut inner = self.inner.borrow_mut();
        if !inner.fixed.contains_key(&(iface, ord)) {
            let target = Target {
                kind: Kind::Fixed,
                iface,
                ord,
            };
            inner.send(Frame::Read(target));
            let mut refused = None;
            inner.wait("the fixed's publish", |i| {
                if let Some(at) = i.answers.iter().position(|(t, _)| *t == target) {
                    refused = i.answers.remove(at).map(|(_, o)| o);
                }
                refused.is_some() || i.fixed.contains_key(&(iface, ord))
            });
            match refused {
                Some(AnswerOutcome::Contract(c)) => return Err(ReadError::Contract(c)),
                Some(AnswerOutcome::Accepted) => panic!("a read is never answered accepted"),
                None => {}
            }
        }
        copy(&inner.fixed[&(iface, ord)], out)
    }
}

impl EventSource for WsClient {
    /// Sends one `subscribe` per event and waits for each `answer`.
    fn subscribe(&mut self, iface: InterfaceNo, ords: &[Ordinal]) -> Result<(), SubscribeError> {
        let inner = self.inner.get_mut();
        for &ord in ords {
            let target = Target {
                kind: Kind::Event,
                iface,
                ord,
            };
            inner.send(Frame::Subscribe(target));
            match inner.wait_answer(target) {
                AnswerOutcome::Accepted => {
                    inner.events.insert((iface, ord));
                }
                AnswerOutcome::Contract(c) => return Err(SubscribeError::Contract(c)),
            }
        }
        Ok(())
    }

    fn unsubscribe(&mut self, iface: InterfaceNo, ords: &[Ordinal]) {
        let inner = self.inner.get_mut();
        for &ord in ords {
            inner.events.remove(&(iface, ord));
            inner.send(Frame::Unsubscribe(Target {
                kind: Kind::Event,
                iface,
                ord,
            }));
        }
    }

    /// Local: takes the oldest queued occurrence.
    fn next(&mut self, out: &mut [u8]) -> Result<Option<RawOccurrence>, ReadError> {
        let inner = self.inner.get_mut();
        let Some(front) = inner.queue.front() else {
            return Ok(None);
        };
        let len = copy(&front.payload, out)?;
        let q = inner.queue.pop_front().expect("the front exists");
        Ok(Some(RawOccurrence {
            iface: q.iface,
            ord: q.ord,
            envelope: q.envelope,
            trace: None,
            len,
        }))
    }
}

/// The caller's view of an outcome that is not its call's success.
fn call_error(outcome: &Outcome) -> CallError {
    match outcome {
        Outcome::Contract(c) => CallError::Contract(*c),
        Outcome::Corrupt => CallError::Transport(Transport::Corrupt),
        Outcome::Busy => CallError::Transport(Transport::Busy),
        Outcome::Accepted | Outcome::Reply(_) => unreachable!("a success"),
    }
}

impl Caller for WsClient {
    /// Sends one `request`. The trace context does not cross: this binding
    /// carries none.
    fn command(
        &mut self,
        iface: InterfaceNo,
        ord: Ordinal,
        args: &[u8],
        _trace: Option<TraceContext>,
    ) -> Result<Correlation, SendError> {
        Ok(self.inner.get_mut().call(Kind::Command, iface, ord, args))
    }

    fn query(
        &mut self,
        iface: InterfaceNo,
        ord: Ordinal,
        args: &[u8],
        _trace: Option<TraceContext>,
    ) -> Result<Correlation, SendError> {
        Ok(self.inner.get_mut().call(Kind::Query, iface, ord, args))
    }

    fn ack(&mut self, c: Correlation) -> Option<Result<(), CallError>> {
        let call = self.inner.get_mut().calls.get(&c.0)?;
        if call.kind != Kind::Command {
            return None;
        }
        match call.outcome.as_ref()? {
            Outcome::Accepted => Some(Ok(())),
            other => Some(Err(call_error(other))),
        }
    }

    fn reply(
        &mut self,
        c: Correlation,
        out: &mut [u8],
    ) -> Result<Option<Result<usize, CallError>>, ReadError> {
        let Some(call) = self.inner.get_mut().calls.get(&c.0) else {
            return Ok(None);
        };
        if call.kind != Kind::Query {
            return Ok(None);
        }
        match call.outcome.as_ref() {
            None => Ok(None),
            Some(Outcome::Reply(bytes)) => copy(bytes, out).map(|n| Some(Ok(n))),
            Some(other) => Ok(Some(Err(call_error(other)))),
        }
    }

    fn forget(&mut self, c: Correlation) {
        self.inner.get_mut().calls.remove(&c.0);
    }
}
