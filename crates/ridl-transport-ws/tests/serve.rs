//! `serve` over a TCP connection, with `tungstenite` as the client and
//! `ridl-loopback` as the backend.

// The provider module is shared with the other tests, and this file uses
// only part of it.
#[allow(dead_code)]
#[path = "support/provider.rs"]
mod provider;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use ridl_loopback::Loopback;
use ridl_rt::contract::{CatalogHash, CatalogRef, InterfaceNo, Kind, Ordinal, CALL_CEILING};
use ridl_rt::error::{CallError, Contract, Transport};
use ridl_rt::port::{
    Attached, Caller, Clock, Correlation, Encoded, EventSource, Handler, RawOccurrence, RawSample,
    ReadError, SendError, SignalReader, SignalWriter, SubscribeError,
};
use ridl_rt::sample::{Envelope, Timestamp};
use ridl_rt::trace::TraceContext;
use ridl_transport_ws::frame::{
    decode, AnswerOutcome, Attach, AttachOutcome, Frame, ProtocolViolation, PublishState, Reason,
    ResponseOutcome, StreamOutcome, Target, BLOCK_BYTES, FRAME_VERSION, FRAME_VERSION_3,
    SUBPROTOCOL, SUBPROTOCOL_V3,
};
use ridl_transport_ws::{
    serve, Backend, Config, Connection, Described, Ended, MemberTable, Note, Polling, Scanning,
    Streaming,
};
use tungstenite::client::IntoClientRequest;
use tungstenite::protocol::frame::coding::CloseCode;
use tungstenite::protocol::frame::CloseFrame;
use tungstenite::{Message, WebSocket};

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
const FIXED: Target = Target {
    kind: Kind::Fixed,
    iface: IFACE,
    ord: Ordinal(5),
};

/// The member table of the test catalog: signal 1, event 2, command 3, query
/// 4 and fixed 5, each with the payload limit `limit` gives it, or none.
fn table(limit: impl Fn(Target) -> Option<u32>) -> Arc<MemberTable> {
    let mut t = MemberTable::new(CATALOG.name, CATALOG.hash);
    let kinds = [
        Kind::Signal,
        Kind::Event,
        Kind::Command,
        Kind::Query,
        Kind::Fixed,
    ];
    for (ord, kind) in (1..).zip(kinds) {
        let target = Target {
            kind,
            iface: IFACE,
            ord: Ordinal(ord),
        };
        t.insert(IFACE, target.ord, kind, None);
        if let Some(limit) = limit(target) {
            assert!(t.set_limit(IFACE, target.ord, limit));
        }
    }
    Arc::new(t)
}

/// An upgrade request this server accepts, for a client without
/// `tungstenite`.
const UPGRADE_REQUEST: &[u8] = b"GET / HTTP/1.1\r\n\
    Host: localhost\r\n\
    Upgrade: websocket\r\n\
    Connection: Upgrade\r\n\
    Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
    Sec-WebSocket-Version: 13\r\n\
    Sec-WebSocket-Protocol: ridl-ws-v2\r\n\
    \r\n";

/// How long a test waits on its peer before it fails instead of hanging.
const TEST_LIMIT: Duration = Duration::from_secs(10);

/// A connected pair: the server's end, accepted, and the client's end.
fn pair() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (server, _) = listener.accept().unwrap();
    client.set_read_timeout(Some(TEST_LIMIT)).unwrap();
    (server, client)
}

/// A configuration with a short poll interval and a liveness timeout no test
/// reaches unless it sets a shorter one.
fn config() -> Config {
    let mut config = Config::default();
    config.poll_interval = Duration::from_millis(10);
    config.liveness_timeout = TEST_LIMIT;
    config
}

/// The same, recording each note into the returned list.
fn recording() -> (Config, Arc<Mutex<Vec<Note>>>) {
    let notes = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&notes);
    let config = config().with_record(move |n| sink.lock().unwrap().push(*n));
    (config, notes)
}

/// `serve` running on its own thread.
struct Server(Receiver<(Ended, Duration)>);

impl Server {
    /// Why `serve` returned and how long it ran. A `serve` that does not
    /// return within [`TEST_LIMIT`] fails the test instead of hanging it.
    fn join(self) -> (Ended, Duration) {
        self.0
            .recv_timeout(TEST_LIMIT)
            .expect("serve returns within the test limit")
    }
}

/// Runs `serve` on its own thread.
fn spawn<C: Connection + Send + 'static, B: Backend + Send + 'static>(
    conn: C,
    backend: B,
    config: Config,
) -> Server {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let start = Instant::now();
        let ended = serve(conn, backend, &config);
        let _ = tx.send((ended, start.elapsed()));
    });
    Server(rx)
}

/// Runs the `tungstenite` client handshake, offering `protocol` when it is
/// not empty.
fn connect(stream: TcpStream, protocol: &str) -> Result<WebSocket<TcpStream>, tungstenite::Error> {
    let mut request = "ws://localhost/".into_client_request().unwrap();
    if !protocol.is_empty() {
        request
            .headers_mut()
            .insert("Sec-WebSocket-Protocol", protocol.parse().unwrap());
    }
    tungstenite::client(request, stream)
        .map(|(ws, _)| ws)
        .map_err(|e| match e {
            tungstenite::HandshakeError::Failure(e) => e,
            tungstenite::HandshakeError::Interrupted(_) => panic!("a blocking stream"),
        })
}

/// The same, with the subprotocol the server selected.
fn connect_selected(stream: TcpStream, protocol: &str) -> (WebSocket<TcpStream>, Option<String>) {
    let mut request = "ws://localhost/".into_client_request().unwrap();
    request
        .headers_mut()
        .insert("Sec-WebSocket-Protocol", protocol.parse().unwrap());
    match tungstenite::client(request, stream) {
        Ok((ws, response)) => {
            let selected = response
                .headers()
                .get("Sec-WebSocket-Protocol")
                .map(|v| v.to_str().unwrap().to_owned());
            (ws, selected)
        }
        Err(_) => panic!("the handshake failed"),
    }
}

fn encode(frame: Frame<'_>) -> Vec<u8> {
    let mut v = vec![0; frame.encoded_len()];
    frame.encode(&mut v).unwrap();
    v
}

fn send(ws: &mut WebSocket<TcpStream>, frame: Frame<'_>) {
    ws.send(Message::binary(encode(frame))).unwrap();
}

/// Reads the next binary message; pings are answered and skipped. A server
/// that sends only pings fails the test after [`TEST_LIMIT`].
fn recv(ws: &mut WebSocket<TcpStream>) -> Vec<u8> {
    let start = Instant::now();
    while start.elapsed() < TEST_LIMIT {
        match ws.read().unwrap() {
            Message::Binary(b) => return b.to_vec(),
            Message::Ping(_) | Message::Pong(_) => {}
            other => panic!("expected a binary message, got {other:?}"),
        }
    }
    panic!("no binary message within {TEST_LIMIT:?}");
}

/// Reads until the server's close frame and returns its code, or fails the
/// test after [`TEST_LIMIT`].
fn recv_close(ws: &mut WebSocket<TcpStream>) -> Option<CloseCode> {
    let start = Instant::now();
    while start.elapsed() < TEST_LIMIT {
        match ws.read() {
            Ok(Message::Close(frame)) => return frame.map(|f| f.code),
            Ok(Message::Binary(_) | Message::Ping(_) | Message::Pong(_)) => {}
            other => panic!("expected a close frame, got {other:?}"),
        }
    }
    panic!("no close frame within {TEST_LIMIT:?}");
}

/// Sends the matching `attach` and checks that it is accepted.
fn attach(ws: &mut WebSocket<TcpStream>) {
    let attach = Attach::new(FRAME_VERSION, 1, CATALOG.hash, CATALOG.name).unwrap();
    send(ws, Frame::Attach(attach));
    assert_eq!(
        decode(&recv(ws)).unwrap(),
        Frame::Attached(AttachOutcome::Accepted)
    );
}

fn publish(stamp: i64, seq: u64, state: PublishState<'_>) -> Frame<'_> {
    Frame::Publish {
        target: SIGNAL,
        envelope: Envelope {
            stamp: Timestamp(stamp),
            seq,
        },
        state,
    }
}

#[test]
fn serve_answers_attach_and_streams_a_signal_change() {
    let (server, client) = pair();
    let rt = Loopback::new(CATALOG);
    let mut provider = rt.attach();
    let handle = spawn(server, Scanning(rt), config());

    let mut ws = connect(client, SUBPROTOCOL).unwrap();
    attach(&mut ws);
    send(&mut ws, Frame::Subscribe(SIGNAL));
    assert_eq!(
        decode(&recv(&mut ws)).unwrap(),
        Frame::Answer {
            target: SIGNAL,
            outcome: AnswerOutcome::Accepted,
        }
    );
    assert_eq!(
        decode(&recv(&mut ws)).unwrap(),
        publish(0, 0, PublishState::Init)
    );

    provider.set(IFACE, Ordinal(1), &[1, 2, 3]).unwrap();
    provider.commit();
    assert_eq!(
        decode(&recv(&mut ws)).unwrap(),
        publish(0, 1, PublishState::Live(&[1, 2, 3]))
    );

    ws.close(None).unwrap();
    assert_eq!(recv_close(&mut ws), Some(CloseCode::Normal));
    assert_eq!(handle.join().0, Ended::ClientClosed(None));
}

#[test]
fn a_rejected_handshake_returns_rejected() {
    let (server, client) = pair();
    let handle = spawn(server, Scanning(Loopback::new(CATALOG)), config());
    // Without `ridl-ws-v2` or `ridl-ws-v1` offered, the server answers 400,
    // and the client reads that answer rather than a reset connection.
    match connect(client, "") {
        Err(tungstenite::Error::Http(response)) => assert_eq!(response.status(), 400),
        other => panic!("expected a 400 answer, got {:?}", other.map(|_| ())),
    }
    assert_eq!(handle.join().0, Ended::Rejected);
}

#[test]
fn a_client_offering_both_versions_is_upgraded_with_v2_and_attached() {
    let (server, client) = pair();
    let handle = spawn(server, Scanning(Loopback::new(CATALOG)), config());
    let mut ws = connect(client, "ridl-ws-v2, ridl-ws-v1").unwrap();
    let attach = Attach::new(2, 1, CATALOG.hash, CATALOG.name).unwrap();
    send(&mut ws, Frame::Attach(attach));
    assert_eq!(
        decode(&recv(&mut ws)).unwrap(),
        Frame::Attached(AttachOutcome::Accepted)
    );
    ws.close(None).unwrap();
    assert_eq!(recv_close(&mut ws), Some(CloseCode::Normal));
    assert_eq!(handle.join().0, Ended::ClientClosed(None));
}

#[test]
fn a_version_1_client_is_upgraded_with_ridl_ws_v1_and_its_attach_refused() {
    let (server, client) = pair();
    let handle = spawn(server, Scanning(Loopback::new(CATALOG)), config());
    let mut ws = connect(client, "ridl-ws-v1").unwrap();
    let attach = Attach::new(1, 1, CATALOG.hash, CATALOG.name).unwrap();
    send(&mut ws, Frame::Attach(attach));
    assert_eq!(
        decode(&recv(&mut ws)).unwrap(),
        Frame::Attached(AttachOutcome::Refused(Reason::FrameVersionUnsupported))
    );
    assert_eq!(recv_close(&mut ws), Some(CloseCode::Normal));
    assert_eq!(handle.join().0, Ended::ServerClosed(1000));
}

/// The session tests set the connection's frame version directly; this one
/// checks that `serve` passes the version of the subprotocol it selected.
#[test]
fn an_attach_of_frame_version_2_on_a_ridl_ws_v1_connection_is_refused() {
    let (server, client) = pair();
    let handle = spawn(server, Scanning(Loopback::new(CATALOG)), config());
    let mut ws = connect(client, "ridl-ws-v1").unwrap();
    let attach = Attach::new(2, 1, CATALOG.hash, CATALOG.name).unwrap();
    send(&mut ws, Frame::Attach(attach));
    assert_eq!(
        decode(&recv(&mut ws)).unwrap(),
        Frame::Attached(AttachOutcome::Refused(Reason::FrameVersionUnsupported))
    );
    assert_eq!(recv_close(&mut ws), Some(CloseCode::Normal));
    assert_eq!(handle.join().0, Ended::ServerClosed(1000));
}

#[test]
fn an_oversized_head_is_answered_400_and_not_reset() {
    let (server, mut client) = pair();
    let handle = spawn(server, Scanning(Loopback::new(CATALOG)), config());
    // The head passes 8 KiB, so the server stops reading it with bytes
    // still unread; it must read them before it closes, or the close resets
    // the connection and the client can lose the answer.
    let mut head = b"GET / HTTP/1.1\r\nX-Pad: ".to_vec();
    head.resize(32 * 1024, b'a');
    head.extend_from_slice(b"\r\n\r\n");
    client.write_all(&head).unwrap();
    let mut answer = Vec::new();
    client.read_to_end(&mut answer).unwrap();
    assert!(answer.starts_with(b"HTTP/1.1 400 "), "{answer:?}");
    assert_eq!(handle.join().0, Ended::Rejected);
}

/// [`UPGRADE_REQUEST`] with an `Origin` header.
fn upgrade_request_from(origin: &str) -> Vec<u8> {
    let request = std::str::from_utf8(UPGRADE_REQUEST).unwrap();
    request
        .replacen("Host:", &format!("Origin: {origin}\r\nHost:"), 1)
        .into_bytes()
}

/// A configuration whose origin check accepts `http://good.example` only.
fn origin_checked() -> Config {
    config().with_origin_check(|origin| origin == Some("http://good.example"))
}

#[test]
fn a_refused_origin_is_answered_403_and_returns_rejected() {
    let (server, mut client) = pair();
    let handle = spawn(server, Scanning(Loopback::new(CATALOG)), origin_checked());
    client
        .write_all(&upgrade_request_from("http://evil.example"))
        .unwrap();
    let mut answer = Vec::new();
    client.read_to_end(&mut answer).unwrap();
    assert!(
        answer.starts_with(b"HTTP/1.1 403 Forbidden\r\n"),
        "{answer:?}"
    );
    assert_eq!(handle.join().0, Ended::Rejected);
}

#[test]
fn an_accepted_origin_is_upgraded() {
    let (server, mut client) = pair();
    let handle = spawn(server, Scanning(Loopback::new(CATALOG)), origin_checked());
    client
        .write_all(&upgrade_request_from("http://good.example"))
        .unwrap();
    read_upgrade_answer(&mut client);
    drop(client);
    assert_eq!(
        handle.join().0,
        Ended::Io(std::io::ErrorKind::UnexpectedEof)
    );
}

#[test]
fn a_request_without_origin_passes_none_to_the_check() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    let config = config().with_origin_check(move |origin| {
        sink.lock().unwrap().push(origin.map(str::to_owned));
        true
    });
    let (server, mut client) = pair();
    let handle = spawn(server, Scanning(Loopback::new(CATALOG)), config);
    client.write_all(UPGRADE_REQUEST).unwrap();
    read_upgrade_answer(&mut client);
    drop(client);
    handle.join();
    assert_eq!(*seen.lock().unwrap(), vec![None]);
}

#[test]
fn a_handshake_that_does_not_finish_ends_at_the_liveness_timeout() {
    let mut config = config();
    config.liveness_timeout = Duration::from_secs(1);
    let bound = config.liveness_timeout + Duration::from_millis(200);
    let (server, mut client) = pair();
    let handle = spawn(server, Scanning(Loopback::new(CATALOG)), config);
    // One byte of the head every 50 ms: each read is shorter than the
    // liveness timeout, and the head never ends.
    let trickle = thread::spawn(move || {
        client.write_all(b"GET / HTTP/1.1\r\nX-Pad: ").unwrap();
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(2) && client.write_all(b"a").is_ok() {
            thread::sleep(Duration::from_millis(50));
        }
    });
    let (ended, elapsed) = handle.join();
    assert_eq!(ended, Ended::LivenessTimeout);
    assert!(elapsed < bound, "ended after {elapsed:?}, over {bound:?}");
    trickle.join().unwrap();
}

#[test]
fn a_client_close_returns_client_closed_with_its_code() {
    let (server, client) = pair();
    let handle = spawn(server, Scanning(Loopback::new(CATALOG)), config());
    let mut ws = connect(client, SUBPROTOCOL).unwrap();
    attach(&mut ws);
    ws.close(Some(CloseFrame {
        code: CloseCode::Library(4000),
        reason: "".into(),
    }))
    .unwrap();
    // The server answers with a close frame that echoes the code.
    assert_eq!(recv_close(&mut ws), Some(CloseCode::Library(4000)));
    assert_eq!(handle.join().0, Ended::ClientClosed(Some(4000)));
}

/// 1008 is the code the server itself sends when the retained bytes pass the
/// cap; a client that closes with it first is answered as any other close.
#[test]
fn a_client_close_with_1008_is_answered_with_1008() {
    let (server, client) = pair();
    let handle = spawn(server, Scanning(Loopback::new(CATALOG)), config());
    let mut ws = connect(client, SUBPROTOCOL).unwrap();
    attach(&mut ws);
    ws.close(Some(CloseFrame {
        code: CloseCode::Policy,
        reason: "".into(),
    }))
    .unwrap();
    assert_eq!(recv_close(&mut ws), Some(CloseCode::Policy));
    assert_eq!(handle.join().0, Ended::ClientClosed(Some(1008)));
}

#[test]
fn the_server_pings_every_ping_interval() {
    let interval = Duration::from_millis(50);
    let mut config = config();
    config.ping_interval = interval;
    let (server, client) = pair();
    let handle = spawn(server, Scanning(Loopback::new(CATALOG)), config);
    let mut ws = connect(client, SUBPROTOCOL).unwrap();
    let start = Instant::now();
    let mut pings = 0;
    while pings < 3 {
        match ws.read().unwrap() {
            Message::Ping(_) => pings += 1,
            other => panic!("expected a ping, got {other:?}"),
        }
    }
    let elapsed = start.elapsed();
    // The third ping is due three intervals after the session opened; one
    // interval of the lower bound and a second of the upper bound are margin.
    assert!(elapsed >= 2 * interval, "three pings in {elapsed:?}");
    assert!(
        elapsed < 3 * interval + Duration::from_secs(1),
        "three pings in {elapsed:?}"
    );
    ws.close(None).unwrap();
    recv_close(&mut ws);
    assert_eq!(handle.join().0, Ended::ClientClosed(None));
}

#[test]
fn a_client_ping_is_answered_with_a_pong_carrying_its_payload() {
    let (server, client) = pair();
    let handle = spawn(server, Scanning(Loopback::new(CATALOG)), config());
    let mut ws = connect(client, SUBPROTOCOL).unwrap();
    ws.send(Message::Ping(vec![1, 2, 3].into())).unwrap();
    let start = Instant::now();
    let pong = loop {
        assert!(
            start.elapsed() < TEST_LIMIT,
            "no pong within {TEST_LIMIT:?}"
        );
        match ws.read().unwrap() {
            Message::Pong(payload) => break payload,
            Message::Ping(_) => {}
            other => panic!("expected a pong, got {other:?}"),
        }
    };
    assert_eq!(pong.to_vec(), vec![1, 2, 3]);
    ws.close(None).unwrap();
    recv_close(&mut ws);
    assert_eq!(handle.join().0, Ended::ClientClosed(None));
}

#[test]
fn a_silent_client_ends_the_session_at_the_liveness_timeout() {
    let mut config = config();
    config.poll_interval = Duration::from_millis(20);
    config.liveness_timeout = Duration::from_secs(1);
    let bound = config.liveness_timeout + config.poll_interval + Duration::from_millis(100);
    let liveness = config.liveness_timeout;

    let (server, mut client) = pair();
    // The request is written before `serve` starts, so that the time `serve`
    // runs is the handshake, which needs no wait, and then the silence.
    client.write_all(UPGRADE_REQUEST).unwrap();
    let handle = spawn(server, Scanning(Loopback::new(CATALOG)), config);
    read_upgrade_answer(&mut client);

    // The client now reads nothing and sends nothing.
    let (ended, elapsed) = handle.join();
    assert_eq!(ended, Ended::LivenessTimeout);
    assert!(elapsed >= liveness, "ended after {elapsed:?}");
    assert!(elapsed < bound, "ended after {elapsed:?}, over {bound:?}");
    drop(client);
}

#[test]
fn a_protocol_violation_closes_1002_and_records_a_note() {
    let (config, notes) = recording();
    let (server, client) = pair();
    let handle = spawn(server, Scanning(Loopback::new(CATALOG)), config);
    let mut ws = connect(client, SUBPROTOCOL).unwrap();
    attach(&mut ws);
    // An empty message is shorter than any header.
    ws.send(Message::binary(Vec::new())).unwrap();
    assert_eq!(recv_close(&mut ws), Some(CloseCode::Protocol));
    assert_eq!(handle.join().0, Ended::ServerClosed(1002));
    assert_eq!(
        *notes.lock().unwrap(),
        vec![Note::Violation(ProtocolViolation::TooShort)]
    );
}

#[test]
fn a_backend_that_detaches_closes_1001() {
    let (server, client) = pair();
    let (backend, _, _) = watched(true);
    let handle = spawn(server, backend, config());
    let mut ws = connect(client, SUBPROTOCOL).unwrap();
    attach(&mut ws);
    // The first tick after the `attach` reads the event queue, which fails.
    assert_eq!(recv_close(&mut ws), Some(CloseCode::Away));
    assert_eq!(handle.join().0, Ended::ServerClosed(1001));
}

#[test]
fn the_end_of_a_session_forgets_each_call_in_flight_then_drops_the_backend() {
    let (server, client) = pair();
    let (backend, log, _) = watched(false);
    let handle = spawn(server, backend, config());
    let mut ws = connect(client, SUBPROTOCOL).unwrap();
    attach(&mut ws);
    // No handler serves the command, so its outcome stays unknown.
    send(
        &mut ws,
        Frame::Request {
            target: COMMAND,
            envelope: Envelope {
                stamp: Timestamp(0),
                seq: 1,
            },
            payload: &[],
        },
    );
    ws.close(None).unwrap();
    recv_close(&mut ws);
    assert_eq!(handle.join().0, Ended::ClientClosed(None));
    assert_eq!(*log.lock().unwrap(), vec!["forget", "drop"]);
}

/// What happened to a [`Watched`] backend, in order: `"forget"` for each
/// call forgotten, `"drop"` when it was dropped.
type Log = Arc<Mutex<Vec<&'static str>>>;

/// The number of times a [`Watched`] backend's `next` was called, which is
/// once per tick.
type Polls = Arc<AtomicUsize>;

/// Ports that delegate to a loopback, log each `forget` and their drop, and
/// count the calls of `next`. With `detach_next`, `EventSource::next` answers
/// `Detached`.
struct Watched {
    inner: Loopback,
    detach_next: bool,
    log: Log,
    polls: Polls,
}

fn watched(detach_next: bool) -> (Polling<Watched>, Log, Polls) {
    let log = Log::default();
    let polls = Polls::default();
    let ports = Watched {
        inner: Loopback::new(CATALOG),
        detach_next,
        log: Arc::clone(&log),
        polls: Arc::clone(&polls),
    };
    (Polling(ports), log, polls)
}

impl Drop for Watched {
    fn drop(&mut self) {
        self.log.lock().unwrap().push("drop");
    }
}

impl Attached for Watched {
    fn catalog(&self) -> &CatalogRef {
        self.inner.catalog()
    }
}

impl Encoded for Watched {
    type Encoding = <Loopback as Encoded>::Encoding;
}

impl Clock for Watched {
    fn now(&self) -> Timestamp {
        self.inner.now()
    }
}

impl SignalReader for Watched {
    fn check_with(
        &self,
        _: ridl_rt::contract::InterfaceNo,
        _: ridl_rt::contract::Ordinal,
        _: ridl_rt::port::Check,
    ) {
        // This double receives no payload from a peer.
    }
    fn read(
        &self,
        iface: InterfaceNo,
        ord: Ordinal,
        out: &mut [u8],
    ) -> Result<RawSample, ReadError> {
        self.inner.read(iface, ord, out)
    }
}

impl EventSource for Watched {
    fn subscribe(&mut self, iface: InterfaceNo, ords: &[Ordinal]) -> Result<(), SubscribeError> {
        self.inner.subscribe(iface, ords)
    }
    fn unsubscribe(&mut self, iface: InterfaceNo, ords: &[Ordinal]) {
        self.inner.unsubscribe(iface, ords);
    }
    fn next(&mut self, out: &mut [u8]) -> Result<Option<RawOccurrence>, ReadError> {
        self.polls.fetch_add(1, Ordering::SeqCst);
        if self.detach_next {
            return Err(ReadError::Detached);
        }
        self.inner.next(out)
    }
}

impl Caller for Watched {
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
        self.log.lock().unwrap().push("forget");
        self.inner.forget(c);
    }
}

#[test]
fn a_client_that_stops_reading_ends_the_session_at_the_liveness_timeout() {
    let mut config = config();
    config.liveness_timeout = Duration::from_secs(1);
    // A full send buffer blocks a write for at most the liveness timeout, and
    // the last message arrived at most one liveness timeout earlier. The
    // margin is one second, so a loaded runner has room, and the bound stays
    // below three liveness timeouts, so a write timeout that is a multiple of
    // the liveness timeout fails the test.
    let bound = 2 * config.liveness_timeout + Duration::from_secs(1);
    let (server, client) = pair();
    let rt = Loopback::new(CATALOG);
    let mut provider = rt.attach();
    // The signal's limit is raised to 16,128 bytes, above its default
    // ceiling, in the member table, which the session applies, and on the loopback,
    // which refuses a value above its member's limit on send.
    let limit = |t: Target| (t == SIGNAL).then_some(CALL_CEILING as u32);
    let backend = Described::new(Scanning(rt), table(limit)).unwrap();
    provider.set_limit(IFACE, Ordinal(1), CALL_CEILING);
    let handle = spawn(server, backend, config);
    let mut ws = connect(client, SUBPROTOCOL).unwrap();
    attach(&mut ws);
    send(&mut ws, Frame::Subscribe(SIGNAL));

    // The client now reads nothing, while the provider commits large values
    // until the session ends.
    let (done, ended) = mpsc::channel();
    let committer = thread::spawn(move || {
        let mut value = vec![0u8; CALL_CEILING];
        while let Err(TryRecvError::Empty) = ended.try_recv() {
            value[0] = value[0].wrapping_add(1);
            provider.set(IFACE, Ordinal(1), &value).unwrap();
            provider.commit();
            thread::sleep(Duration::from_millis(5));
        }
    });
    let (ended, elapsed) = handle.join();
    done.send(()).unwrap();
    committer.join().unwrap();
    assert_eq!(ended, Ended::LivenessTimeout);
    assert!(elapsed < bound, "ended after {elapsed:?}, over {bound:?}");
    drop(ws);
}

/// A client that sends a message over the limit with more of it still
/// unsent: the server must read the rest before it closes, or the close
/// resets the connection and the client loses the close frame.
#[test]
fn an_oversized_message_closes_1009_and_the_client_sees_it() {
    let config = config().with_max_message_bytes(1024).unwrap();
    let (server, client) = pair();
    let handle = spawn(server, Scanning(Loopback::new(CATALOG)), config);
    let mut ws = connect(client, SUBPROTOCOL).unwrap();
    attach(&mut ws);
    ws.send(Message::binary(vec![0u8; 256 * 1024])).unwrap();
    // A reset, when there is one, arrives within this wait, before the
    // client reads.
    thread::sleep(Duration::from_millis(200));
    assert_eq!(recv_close(&mut ws), Some(CloseCode::Size));
    assert_eq!(handle.join().0, Ended::ServerClosed(1009));
}

#[test]
fn a_text_message_closes_1003_and_the_client_sees_it() {
    let (server, client) = pair();
    let handle = spawn(server, Scanning(Loopback::new(CATALOG)), config());
    let mut ws = connect(client, SUBPROTOCOL).unwrap();
    attach(&mut ws);
    // A large message after the text one is still unread when the server
    // closes.
    ws.send(Message::text("not binary")).unwrap();
    ws.send(Message::binary(vec![0u8; 256 * 1024])).unwrap();
    thread::sleep(Duration::from_millis(200));
    assert_eq!(recv_close(&mut ws), Some(CloseCode::Unsupported));
    assert_eq!(handle.join().0, Ended::ServerClosed(1003));
}

#[test]
fn a_data_message_resets_the_liveness_clock() {
    let mut config = config();
    config.liveness_timeout = Duration::from_secs(1);
    // No ping, so that the client sends no pong: only data messages arrive.
    config.ping_interval = TEST_LIMIT;
    let (server, client) = pair();
    let handle = spawn(server, Scanning(Loopback::new(CATALOG)), config);
    let mut ws = connect(client, SUBPROTOCOL).unwrap();
    attach(&mut ws);
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(900) {
        send(&mut ws, Frame::Subscribe(SIGNAL));
        thread::sleep(Duration::from_millis(50));
    }
    ws.close(None).unwrap();
    assert_eq!(recv_close(&mut ws), Some(CloseCode::Normal));
    assert_eq!(handle.join().0, Ended::ClientClosed(None));
}

#[test]
fn the_server_pings_while_messages_keep_arriving() {
    let interval = Duration::from_millis(50);
    let mut config = config();
    config.ping_interval = interval;
    let (server, client) = pair();
    let handle = spawn(server, Scanning(Loopback::new(CATALOG)), config);
    let mut ws = connect(client, SUBPROTOCOL).unwrap();
    attach(&mut ws);
    // A message every 10 ms for 8 ping intervals.
    let start = Instant::now();
    while start.elapsed() < 8 * interval {
        send(&mut ws, Frame::Subscribe(SIGNAL));
        thread::sleep(Duration::from_millis(10));
    }
    ws.close(None).unwrap();
    let mut pings = 0;
    let start = Instant::now();
    loop {
        assert!(
            start.elapsed() < TEST_LIMIT,
            "no close within {TEST_LIMIT:?}"
        );
        match ws.read().unwrap() {
            Message::Ping(_) => pings += 1,
            Message::Binary(_) | Message::Pong(_) => {}
            Message::Close(_) => break,
            other => panic!("unexpected {other:?}"),
        }
    }
    // About 8 are due; 3 leaves a wide margin for a slow machine.
    assert!(pings >= 3, "{pings} pings in 8 intervals");
    assert_eq!(handle.join().0, Ended::ClientClosed(None));
}

/// A client that sends a message every millisecond does not make the server
/// poll the backend more often than once per poll interval.
#[test]
fn the_backend_is_polled_at_most_once_per_poll_interval_under_a_fast_sender() {
    let mut config = config();
    config.poll_interval = Duration::from_millis(20);
    let (server, client) = pair();
    let (backend, _, polls) = watched(false);
    let handle = spawn(server, backend, config);
    let mut ws = connect(client, SUBPROTOCOL).unwrap();
    attach(&mut ws);
    let messages = 300;
    for _ in 0..messages {
        send(&mut ws, Frame::Subscribe(SIGNAL));
        thread::sleep(Duration::from_millis(1));
    }
    ws.close(None).unwrap();
    recv_close(&mut ws);
    assert_eq!(handle.join().0, Ended::ClientClosed(None));
    // About one poll per 20 ms over at least 300 ms: 15 or so, and far below
    // the one per message that polling on every read would give.
    let polls = polls.load(Ordering::SeqCst);
    assert!(
        polls < messages / 4,
        "{polls} polls for {messages} messages"
    );
}

/// A masked binary frame that carries `payload`, under a zero mask, for a
/// client without `tungstenite`. Payloads below 126 bytes only.
fn masked(payload: &[u8]) -> Vec<u8> {
    assert!(payload.len() < 126);
    let mut frame = vec![0x82, 0x80 | payload.len() as u8, 0, 0, 0, 0];
    frame.extend_from_slice(payload);
    frame
}

/// Takes every whole unmasked server frame at the front of `buf` and returns
/// the payloads of the binary ones. Payloads below 126 bytes only.
fn take_binary_frames(buf: &mut Vec<u8>) -> Vec<Vec<u8>> {
    let mut frames = Vec::new();
    while buf.len() >= 2 {
        let len = usize::from(buf[1]);
        assert!(len < 126, "a frame of {len} bytes");
        if buf.len() < 2 + len {
            break;
        }
        let frame: Vec<u8> = buf.drain(..2 + len).collect();
        if frame[0] == 0x82 {
            frames.push(frame[2..].to_vec());
        }
    }
    frames
}

/// The other direction of the poll gate: a client that sends a message every
/// millisecond does not delay the polls either. A change committed while the
/// messages flow is sent within about two poll intervals.
#[test]
fn a_change_reaches_a_fast_sender_within_two_poll_intervals() {
    let mut config = config();
    config.poll_interval = Duration::from_millis(50);
    config.ping_interval = TEST_LIMIT;
    let bound = 2 * config.poll_interval + Duration::from_millis(50);
    let (server, mut client) = pair();
    let rt = Loopback::new(CATALOG);
    let mut provider = rt.attach();
    client.write_all(UPGRADE_REQUEST).unwrap();
    let handle = spawn(server, Scanning(rt), config);
    read_upgrade_answer(&mut client);

    // The attach, then the subscribe: three frames come back.
    let attach = Attach::new(FRAME_VERSION, 1, CATALOG.hash, CATALOG.name).unwrap();
    client
        .write_all(&masked(&encode(Frame::Attach(attach))))
        .unwrap();
    let subscribe = masked(&encode(Frame::Subscribe(SIGNAL)));
    client.write_all(&subscribe).unwrap();
    let mut buf = Vec::new();
    let mut got = Vec::new();
    let mut chunk = [0u8; 4096];
    while got.len() < 3 {
        let n = client.read(&mut chunk).unwrap();
        assert!(n > 0, "the server closed");
        buf.extend_from_slice(&chunk[..n]);
        got.extend(take_binary_frames(&mut buf));
    }
    assert_eq!(decode(&got[2]).unwrap(), publish(0, 0, PublishState::Init));

    // A subscribe every millisecond, each answered; the change is committed
    // after 100 ms of that, and the answers are read until its publish
    // arrives.
    client
        .set_read_timeout(Some(Duration::from_millis(1)))
        .unwrap();
    let start = Instant::now();
    let mut committed = None;
    let latency = loop {
        assert!(start.elapsed() < TEST_LIMIT, "no publish within the limit");
        client.write_all(&subscribe).unwrap();
        if committed.is_none() && start.elapsed() >= Duration::from_millis(100) {
            provider.set(IFACE, Ordinal(1), &[9]).unwrap();
            provider.commit();
            committed = Some(Instant::now());
        }
        match client.read(&mut chunk) {
            Ok(0) => panic!("the server closed"),
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(e) => panic!("{e}"),
        }
        let frames = take_binary_frames(&mut buf);
        if let Some(at) = committed {
            if frames
                .iter()
                .any(|f| decode(f).unwrap() == publish(0, 1, PublishState::Live(&[9])))
            {
                break at.elapsed();
            }
        }
        thread::sleep(Duration::from_millis(1));
    };
    assert!(
        latency < bound,
        "the change arrived after {latency:?}, over {bound:?}"
    );
    // Answers to the subscribes may still be unread. Closing a TCP socket that
    // holds unread data sends a reset instead of an orderly close, and the
    // server would then end with a reset or a broken pipe. So the client shuts
    // down its write side, reads until the server closes, and only then drops.
    client.shutdown(std::net::Shutdown::Write).unwrap();
    client.set_read_timeout(Some(TEST_LIMIT)).unwrap();
    while client.read(&mut chunk).unwrap() > 0 {}
    drop(client);
    assert_eq!(
        handle.join().0,
        Ended::Io(std::io::ErrorKind::UnexpectedEof)
    );
}

/// Pings from the client, with no data message, keep the session alive past
/// the liveness timeout: a control frame counts as a message.
#[test]
fn a_control_frame_resets_the_liveness_clock() {
    let mut config = config();
    config.liveness_timeout = Duration::from_secs(1);
    config.ping_interval = TEST_LIMIT;
    let (server, client) = pair();
    let handle = spawn(server, Scanning(Loopback::new(CATALOG)), config);
    let mut ws = connect(client, SUBPROTOCOL).unwrap();
    attach(&mut ws);
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(900) {
        ws.send(Message::Ping(Vec::new().into())).unwrap();
        thread::sleep(Duration::from_millis(50));
    }
    ws.close(None).unwrap();
    assert_eq!(recv_close(&mut ws), Some(CloseCode::Normal));
    assert_eq!(handle.join().0, Ended::ClientClosed(None));
}

#[test]
fn a_client_that_closes_the_connection_without_a_close_frame_ends_with_eof() {
    let (server, client) = pair();
    let handle = spawn(server, Scanning(Loopback::new(CATALOG)), config());
    let mut ws = connect(client, SUBPROTOCOL).unwrap();
    attach(&mut ws);
    drop(ws);
    assert_eq!(
        handle.join().0,
        Ended::Io(std::io::ErrorKind::UnexpectedEof)
    );
}

/// A connection that gives a complete upgrade request, then fails every
/// read with `kind`: `TimedOut`, the kind a read timeout gives on Windows,
/// or `Interrupted`, a read a signal cut short.
struct Failing {
    request: Vec<u8>,
    pos: usize,
    kind: std::io::ErrorKind,
}

impl Failing {
    fn after_the_handshake(kind: std::io::ErrorKind) -> Self {
        Failing {
            request: UPGRADE_REQUEST.to_vec(),
            pos: 0,
            kind,
        }
    }
}

impl Read for Failing {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.pos < self.request.len() {
            let n = buf.len().min(self.request.len() - self.pos);
            buf[..n].copy_from_slice(&self.request[self.pos..self.pos + n]);
            self.pos += n;
            return Ok(n);
        }
        thread::sleep(Duration::from_millis(1));
        Err(self.kind.into())
    }
}

impl Write for Failing {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Connection for Failing {
    fn set_read_timeout(&self, _: Option<Duration>) -> std::io::Result<()> {
        Ok(())
    }
    fn set_write_timeout(&self, _: Option<Duration>) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn a_read_that_fails_with_timed_out_is_a_tick() {
    let mut config = config();
    config.liveness_timeout = Duration::from_millis(100);
    let conn = Failing::after_the_handshake(std::io::ErrorKind::TimedOut);
    assert_eq!(
        serve(conn, Scanning(Loopback::new(CATALOG)), &config),
        Ended::LivenessTimeout
    );
}

#[test]
fn a_read_that_fails_with_interrupted_is_a_tick_and_not_an_end() {
    let mut config = config();
    config.liveness_timeout = Duration::from_millis(100);
    let conn = Failing::after_the_handshake(std::io::ErrorKind::Interrupted);
    assert_eq!(
        serve(conn, Scanning(Loopback::new(CATALOG)), &config),
        Ended::LivenessTimeout
    );
}

/// A connection that gives the upgrade request, then one message half a
/// poll interval later, then fails every read with `TimedOut`, and records
/// each read timeout `serve` sets on it.
struct Scripted {
    request: Vec<u8>,
    message: Vec<u8>,
    pos: usize,
    delivered: bool,
    timeouts: Arc<Mutex<Vec<Duration>>>,
}

impl Read for Scripted {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.pos < self.request.len() {
            let n = buf.len().min(self.request.len() - self.pos);
            buf[..n].copy_from_slice(&self.request[self.pos..self.pos + n]);
            self.pos += n;
            return Ok(n);
        }
        if !self.delivered {
            self.delivered = true;
            thread::sleep(Duration::from_millis(25));
            buf[..self.message.len()].copy_from_slice(&self.message);
            return Ok(self.message.len());
        }
        thread::sleep(Duration::from_millis(1));
        Err(std::io::ErrorKind::TimedOut.into())
    }
}

impl Write for Scripted {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Connection for Scripted {
    fn set_read_timeout(&self, t: Option<Duration>) -> std::io::Result<()> {
        self.timeouts.lock().unwrap().push(t.unwrap());
        Ok(())
    }
    fn set_write_timeout(&self, _: Option<Duration>) -> std::io::Result<()> {
        Ok(())
    }
}

/// After a message read half a poll interval into the interval, the next
/// read waits only for the rest of the interval, not a whole new one, so
/// that the message does not push the next poll back.
#[test]
fn a_read_after_a_message_waits_only_until_the_next_poll_is_due() {
    let mut config = config();
    config.poll_interval = Duration::from_millis(50);
    config.liveness_timeout = Duration::from_millis(150);
    let timeouts = Arc::new(Mutex::new(Vec::new()));
    let attach = Attach::new(FRAME_VERSION, 1, CATALOG.hash, CATALOG.name).unwrap();
    let conn = Scripted {
        request: UPGRADE_REQUEST.to_vec(),
        message: masked(&encode(Frame::Attach(attach))),
        pos: 0,
        delivered: false,
        timeouts: Arc::clone(&timeouts),
    };
    assert_eq!(
        serve(conn, Scanning(Loopback::new(CATALOG)), &config),
        Ended::LivenessTimeout
    );
    let timeouts = timeouts.lock().unwrap();
    // The handshake's own timeouts, then the first poll interval, then the
    // timeout set after the message, about half an interval in.
    let after_message = timeouts
        .iter()
        .skip_while(|t| **t != config.poll_interval)
        .nth(1)
        .copied()
        .expect("a timeout set after the message");
    assert!(
        after_message <= Duration::from_millis(30),
        "the read after the message waits {after_message:?}"
    );
    assert!(after_message >= Duration::from_millis(1));
}

/// A connection whose drop waits for the test's release, so that an end of
/// stream the client reads before the release can only come from
/// `shutdown_write`, not from the connection being dropped.
struct Held {
    stream: TcpStream,
    release: Receiver<()>,
}

fn held(stream: TcpStream) -> (Held, mpsc::Sender<()>) {
    let (release, wait) = mpsc::channel();
    (
        Held {
            stream,
            release: wait,
        },
        release,
    )
}

impl Drop for Held {
    fn drop(&mut self) {
        let _ = self.release.recv_timeout(TEST_LIMIT);
    }
}

impl Read for Held {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.stream.read(buf)
    }
}

impl Write for Held {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.stream.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.stream.flush()
    }
}

impl Connection for Held {
    fn set_read_timeout(&self, t: Option<Duration>) -> std::io::Result<()> {
        self.stream.set_read_timeout(t)
    }
    fn set_write_timeout(&self, t: Option<Duration>) -> std::io::Result<()> {
        self.stream.set_write_timeout(t)
    }
    fn shutdown_write(&self) -> std::io::Result<()> {
        self.stream.shutdown(std::net::Shutdown::Write)
    }
}

/// A connection whose writes progress in small chunks with a delay, as
/// they do for a slowly reading client, and can stall after a set count.
struct Throttled {
    stream: TcpStream,
    chunk: usize,
    delay: Duration,
    stall_after: Option<usize>,
    calls: usize,
    write_timeout: std::cell::Cell<Option<Duration>>,
}

impl Read for Throttled {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.stream.read(buf)
    }
}

impl Write for Throttled {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.stall_after.is_some_and(|n| self.calls >= n) {
            thread::sleep(self.write_timeout.get().unwrap_or(2 * TEST_LIMIT));
            return Err(std::io::ErrorKind::TimedOut.into());
        }
        let n = self.stream.write(&buf[..buf.len().min(self.chunk)])?;
        self.calls += 1;
        thread::sleep(self.delay);
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.stream.flush()
    }
}

impl Connection for Throttled {
    fn set_read_timeout(&self, t: Option<Duration>) -> std::io::Result<()> {
        self.stream.set_read_timeout(t)
    }
    fn set_write_timeout(&self, t: Option<Duration>) -> std::io::Result<()> {
        self.write_timeout.set(t);
        self.stream.set_write_timeout(t)
    }
    fn shutdown_write(&self) -> std::io::Result<()> {
        self.stream.shutdown(std::net::Shutdown::Write)
    }
}

/// The bytes of a raw client's upgrade answer, read up to the empty line.
fn read_upgrade_answer(client: &mut TcpStream) {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        client.read_exact(&mut byte).unwrap();
        head.push(byte[0]);
    }
    assert!(head.starts_with(b"HTTP/1.1 101 "));
}

/// Reads the server's unmasked close frame with `code`, then the end of the
/// stream, while the server's connection is held open: the end of the
/// stream is `shutdown_write`, and not the drop of the connection.
fn read_close_then_end_of_stream(client: &mut TcpStream, code: u16) {
    let mut close = [0u8; 4];
    client.read_exact(&mut close).unwrap();
    let [hi, lo] = code.to_be_bytes();
    assert_eq!(close, [0x88, 0x02, hi, lo]);
    let mut rest = [0u8; 16];
    assert_eq!(client.read(&mut rest).unwrap(), 0);
}

#[test]
fn a_server_close_is_followed_by_the_end_of_the_stream_and_not_a_reset() {
    let config = config().with_max_message_bytes(1024).unwrap();
    let (server, mut client) = pair();
    let (server, release) = held(server);
    client.write_all(UPGRADE_REQUEST).unwrap();
    let handle = spawn(server, Scanning(Loopback::new(CATALOG)), config);
    read_upgrade_answer(&mut client);
    // A masked binary frame that announces 1 MiB, with 64 KiB of its payload:
    // the server closes 1009 once it reads the length, with the payload
    // still unread. The server read what the client sent before it closed
    // the connection, so the connection was not reset.
    let mut frame = vec![0x82, 0x80 | 127];
    frame.extend_from_slice(&(1024u64 * 1024).to_be_bytes());
    frame.extend_from_slice(&[1, 2, 3, 4]);
    frame.resize(frame.len() + 64 * 1024, 0);
    client.write_all(&frame).unwrap();
    read_close_then_end_of_stream(&mut client, 1009);
    release.send(()).unwrap();
    assert_eq!(handle.join().0, Ended::ServerClosed(1009));
}

/// The answer to a client's close frame is followed by the end of the
/// stream too, before the server's connection is dropped.
#[test]
fn the_answer_to_a_client_close_is_followed_by_the_end_of_the_stream() {
    let (server, mut client) = pair();
    let (server, release) = held(server);
    client.write_all(UPGRADE_REQUEST).unwrap();
    let handle = spawn(server, Scanning(Loopback::new(CATALOG)), config());
    read_upgrade_answer(&mut client);
    // A masked close frame with code 1000, under a zero mask.
    client
        .write_all(&[0x88, 0x82, 0, 0, 0, 0, 0x03, 0xE8])
        .unwrap();
    read_close_then_end_of_stream(&mut client, 1000);
    release.send(()).unwrap();
    assert_eq!(handle.join().0, Ended::ClientClosed(Some(1000)));
}

/// Mutation: count frames instead of bytes (the session would then never
/// close, because with no member table the ring keeps at most 64 responses).
#[test]
fn responses_past_the_retained_bytes_cap_close_1008_and_the_client_sees_it() {
    let (server, client) = pair();
    let config = config().with_max_retained_bytes(16_164).unwrap();
    let rt = Loopback::new(CATALOG);
    let mut provider = rt.attach();
    provider.serve(IFACE, &[QUERY.ord]).unwrap();
    let handle = spawn(server, Scanning(rt), config);
    let mut ws = connect(client, SUBPROTOCOL).unwrap();
    attach(&mut ws);
    // Each `request` of the query is settled with 4,005 bytes, and its
    // `response` is 4,041 bytes; four of them are 16,164 bytes, the cap, and
    // the fifth passes it.
    let reply = vec![1u8; 4005];
    for seq in 1..=5u64 {
        send(
            &mut ws,
            Frame::Request {
                target: QUERY,
                envelope: Envelope {
                    stamp: Timestamp(0),
                    seq,
                },
                payload: &[],
            },
        );
        let start = Instant::now();
        let claim = loop {
            assert!(start.elapsed() < TEST_LIMIT, "no claim within the limit");
            if let Some(claim) = provider.next_claim(&mut [0; 64]).unwrap() {
                break claim;
            }
            thread::sleep(Duration::from_millis(1));
        };
        provider.settle(claim.id, Ok(&reply)).unwrap();
        assert_eq!(recv(&mut ws).len(), 4041, "response {seq}");
    }
    assert_eq!(recv_close(&mut ws), Some(CloseCode::Policy));
    assert_eq!(handle.join().0, Ended::ServerClosed(1008));
}

// -- The `Config` check against the member table (ADR-0030 decision 10) -----

/// A message limit below what one message of a member needs is a
/// configuration error: `serve` returns before the handshake, records the
/// member and the limit it needs, and the client sees no upgrade answer.
///
/// Mutation: skip the check when the table gives the member no limit.
#[test]
fn a_config_that_cannot_carry_a_member_is_refused_before_the_handshake() {
    let (config, notes) = recording();
    let config = config.with_max_message_bytes(1000).unwrap();
    let (server, client) = pair();
    // Every member but the command is lowered to 900 bytes; the command
    // keeps the default ceiling of its kind, 16,128 bytes.
    let limit = |t: Target| (t != COMMAND).then_some(900);
    let backend = Described::new(Scanning(Loopback::new(CATALOG)), table(limit)).unwrap();
    let handle = spawn(server, backend, config);
    assert_eq!(handle.join().0, Ended::ConfigRefused);
    assert_eq!(
        *notes.lock().unwrap(),
        vec![Note::MessageLimitTooSmall {
            target: COMMAND,
            needed: 36 + CALL_CEILING
        }]
    );
    assert!(connect(client, SUBPROTOCOL).is_err());
}

#[test]
fn a_config_that_carries_every_lowered_member_is_accepted() {
    let (config, notes) = recording();
    let config = config.with_max_message_bytes(1000).unwrap();
    let (server, client) = pair();
    let backend = Described::new(Scanning(Loopback::new(CATALOG)), table(|_| Some(900))).unwrap();
    let handle = spawn(server, backend, config);
    let mut ws = connect(client, SUBPROTOCOL).unwrap();
    attach(&mut ws);
    ws.close(None).unwrap();
    assert_eq!(handle.join().0, Ended::ClientClosed(None));
    assert!(notes.lock().unwrap().is_empty());
}

/// When two members need the same number of bytes, the first in key order
/// is the one recorded.
///
/// Mutation: let a later member of equal need replace the first.
#[test]
fn the_member_recorded_on_a_tie_is_the_first_in_key_order() {
    let (config, notes) = recording();
    let config = config.with_max_message_bytes(1000).unwrap();
    let (server, client) = pair();
    // The signal (ordinal 1) and the query (ordinal 4) need the same bytes.
    let limit = |t: Target| Some(if t == SIGNAL || t == QUERY { 2000 } else { 900 });
    let backend = Described::new(Scanning(Loopback::new(CATALOG)), table(limit)).unwrap();
    let handle = spawn(server, backend, config);
    assert_eq!(handle.join().0, Ended::ConfigRefused);
    assert_eq!(
        *notes.lock().unwrap(),
        vec![Note::MessageLimitTooSmall {
            target: SIGNAL,
            needed: 36 + 2000
        }]
    );
    assert!(connect(client, SUBPROTOCOL).is_err());
}

/// A `fixed` counts at most one block message, 4,130 bytes: a larger value
/// crosses in blocks, so a catalog with a `fixed` of the default limit,
/// 65,280 bytes, is served at the default `Config`.
///
/// Mutation: count the fixed's whole limit.
#[test]
fn the_default_config_is_accepted_for_a_catalog_with_a_fixed_of_default_limit() {
    let (config, notes) = recording();
    let (server, client) = pair();
    let backend = Described::new(Scanning(Loopback::new(CATALOG)), table(|_| None)).unwrap();
    let handle = spawn(server, backend, config);
    let mut ws = connect(client, SUBPROTOCOL).unwrap();
    attach(&mut ws);
    ws.close(None).unwrap();
    assert_eq!(handle.join().0, Ended::ClientClosed(None));
    assert!(notes.lock().unwrap().is_empty());
}

/// The table of the next tests: every member but the `fixed` lowered to 900
/// bytes, and the `fixed` given `fixed`, or the default ceiling of its kind.
fn fixed_table(fixed: Option<u32>) -> Arc<MemberTable> {
    table(move |t| {
        if t.kind == Kind::Fixed {
            fixed
        } else {
            Some(900)
        }
    })
}

/// Serves `table` under the message limit `max` and returns how the session
/// ended and the notes: a session that passes the check is attached and
/// closed by the client.
fn served_under(max: usize, table: Arc<MemberTable>) -> (Ended, Vec<Note>) {
    let (config, notes) = recording();
    let config = config.with_max_message_bytes(max).unwrap();
    let (server, client) = pair();
    let backend = Described::new(Scanning(Loopback::new(CATALOG)), table).unwrap();
    let handle = spawn(server, backend, config);
    if let Ok(mut ws) = connect(client, SUBPROTOCOL) {
        attach(&mut ws);
        ws.close(None).unwrap();
    }
    let ended = handle.join().0;
    let notes = notes.lock().unwrap().clone();
    (ended, notes)
}

/// The table of the next tests: the signal has the largest limit that a
/// member may have, 65,280 bytes; every other member has the default ceiling
/// of its kind.
fn raised_signal_table() -> Arc<MemberTable> {
    table(|t| (t == SIGNAL).then_some(65_280))
}

/// A signal whose limit is above 16,128 bytes is carried whole, as one
/// `publish`, when the message limit is raised to carry it: 47,000 bytes and
/// the largest, 65,280 bytes.
///
/// Mutation: lower `MAX_MESSAGE_LIMIT` to 16,164.
#[test]
fn a_signal_above_16128_bytes_is_carried_when_the_message_limit_is_raised() {
    let config = config().with_max_message_bytes(65_316).unwrap();
    let (server, client) = pair();
    let rt = Loopback::new(CATALOG);
    let mut provider = rt.attach();
    provider.set_limit(IFACE, SIGNAL.ord, 65_280);
    let backend = Described::new(Scanning(rt), raised_signal_table()).unwrap();
    let handle = spawn(server, backend, config);
    let mut ws = connect(client, SUBPROTOCOL).unwrap();
    attach(&mut ws);
    send(&mut ws, Frame::Subscribe(SIGNAL));
    assert_eq!(
        decode(&recv(&mut ws)).unwrap(),
        Frame::Answer {
            target: SIGNAL,
            outcome: AnswerOutcome::Accepted,
        }
    );
    assert_eq!(
        decode(&recv(&mut ws)).unwrap(),
        publish(0, 0, PublishState::Init)
    );
    for (seq, len) in [(1, 47_000), (2, 65_280)] {
        let value = pattern(len);
        provider.set(IFACE, SIGNAL.ord, &value).unwrap();
        provider.commit();
        let message = recv(&mut ws);
        assert_eq!(message.len(), len + 27);
        assert_eq!(
            decode(&message).unwrap(),
            publish(0, seq, PublishState::Live(&value))
        );
    }
    ws.close(None).unwrap();
    assert_eq!(handle.join().0, Ended::ClientClosed(None));
    assert_eq!(recv_close(&mut ws), Some(CloseCode::Normal));
}

/// The same catalog under the default configuration is refused before the
/// handshake, and the note names the signal and the 65,316 bytes it needs.
///
/// Mutation: skip the check for a member above the default ceiling of its
/// kind.
#[test]
fn a_catalog_with_a_member_above_16128_bytes_is_refused_under_the_default_config() {
    let (config, notes) = recording();
    assert_eq!(config.max_message_bytes(), 16_164);
    let (server, client) = pair();
    let backend = Described::new(Scanning(Loopback::new(CATALOG)), raised_signal_table()).unwrap();
    let handle = spawn(server, backend, config);
    assert_eq!(handle.join().0, Ended::ConfigRefused);
    drop(client);
    assert_eq!(
        *notes.lock().unwrap(),
        vec![Note::MessageLimitTooSmall {
            target: SIGNAL,
            needed: 65_316,
        }]
    );
}

/// 65,316 bytes carries the largest member limit, and a limit of one byte
/// more is not a configuration the crate accepts.
#[test]
fn a_message_limit_of_65316_is_accepted_and_65317_is_refused() {
    let (ended, notes) = served_under(65_316, raised_signal_table());
    assert_eq!(ended, Ended::ClientClosed(None));
    assert!(notes.is_empty());
    let (ended, notes) = served_under(65_315, raised_signal_table());
    assert_eq!(ended, Ended::ConfigRefused);
    assert_eq!(
        notes,
        vec![Note::MessageLimitTooSmall {
            target: SIGNAL,
            needed: 65_316,
        }]
    );
    assert!(Config::default().with_max_message_bytes(65_316).is_ok());
    assert!(Config::default().with_max_message_bytes(65_317).is_err());
    assert_eq!(Config::default().max_message_bytes(), 16_164);
}

/// A `fixed` crosses in blocks under the default configuration whatever limit
/// its member has, and whole in one `publish` when a raised message limit
/// carries it.
#[test]
fn a_fixed_of_65280_bytes_crosses_in_blocks_by_default_and_whole_under_a_raised_limit() {
    let read = |config: Config| {
        let (server, client) = pair();
        let mut rt = Loopback::new(CATALOG);
        let mut provider = rt.attach();
        provider
            .provision_fixed(IFACE, FIXED.ord, &pattern(65_280))
            .unwrap();
        rt.set_limit(IFACE, FIXED.ord, 65_280);
        let backend = Described::new(Scanning(rt), table(|_| None)).unwrap();
        let handle = spawn(server, backend, config);
        let mut ws = connect(client, SUBPROTOCOL).unwrap();
        attach(&mut ws);
        send(&mut ws, Frame::Read(FIXED));
        let first = recv(&mut ws);
        ws.close(None).unwrap();
        assert_eq!(handle.join().0, Ended::ClientClosed(None));
        first
    };
    let first = read(config());
    assert!(matches!(decode(&first).unwrap(), Frame::Block { .. }));
    assert_eq!(first.len(), 4_130);
    let first = read(config().with_max_message_bytes(65_316).unwrap());
    assert!(matches!(decode(&first).unwrap(), Frame::Publish { .. }));
    assert_eq!(first.len(), 65_307);
}

/// Serves a catalog of default ceilings under `config` and returns how the
/// session ended and the notes: a session that passes the checks is attached
/// and closed by the client.
fn served_with(config: Config) -> (Ended, Vec<Note>) {
    let notes = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&notes);
    let config = config.with_record(move |n| sink.lock().unwrap().push(*n));
    let (server, client) = pair();
    let backend = Described::new(Scanning(Loopback::new(CATALOG)), table(|_| None)).unwrap();
    let handle = spawn(server, backend, config);
    if let Ok(mut ws) = connect(client, SUBPROTOCOL) {
        attach(&mut ws);
        ws.close(None).unwrap();
    }
    let ended = handle.join().0;
    let notes = notes.lock().unwrap().clone();
    (ended, notes)
}

/// A retained-bytes cap of 16,164 bytes, accepted before, is still accepted
/// with the default message limit.
#[test]
fn a_retained_cap_of_16164_is_served_with_the_default_message_limit() {
    let config = config().with_max_retained_bytes(16_164).unwrap();
    let (ended, notes) = served_with(config);
    assert_eq!(ended, Ended::ClientClosed(None));
    assert!(notes.is_empty());
}

/// A retained cap below the message limit is refused where the configuration
/// is used, in either order of the builder calls.
///
/// Mutation: remove the check from `serve`.
#[test]
fn a_retained_cap_below_the_message_limit_is_refused_at_serve_in_either_order() {
    let expected = vec![Note::RetainedBelowMessageLimit {
        retained: 16_164,
        message: 65_316,
    }];
    let first = config()
        .with_max_retained_bytes(16_164)
        .unwrap()
        .with_max_message_bytes(65_316)
        .unwrap();
    let second = config()
        .with_max_message_bytes(65_316)
        .unwrap()
        .with_max_retained_bytes(16_164)
        .unwrap();
    for config in [first, second] {
        let (ended, notes) = served_with(config);
        assert_eq!(ended, Ended::ConfigRefused);
        assert_eq!(notes, expected);
    }
}

#[test]
fn a_retained_cap_equal_to_the_message_limit_is_served() {
    let config = config()
        .with_max_message_bytes(65_316)
        .unwrap()
        .with_max_retained_bytes(65_316)
        .unwrap();
    let (ended, notes) = served_with(config);
    assert_eq!(ended, Ended::ClientClosed(None));
    assert!(notes.is_empty());
}

/// A `fixed` of the default limit needs one block message, 4,130 bytes: a
/// message limit one byte below cannot carry a block.
///
/// Mutation: count a `fixed` as 36 bytes plus 4,093.
#[test]
fn a_config_below_one_block_message_is_refused_for_a_fixed_of_default_limit() {
    let (ended, notes) = served_under(4_129, fixed_table(None));
    assert_eq!(ended, Ended::ConfigRefused);
    assert_eq!(
        notes,
        vec![Note::MessageLimitTooSmall {
            target: FIXED,
            needed: 4_130
        }]
    );
}

#[test]
fn a_config_of_one_block_message_is_accepted_for_a_fixed_of_default_limit() {
    let (ended, notes) = served_under(4_130, fixed_table(None));
    assert_eq!(ended, Ended::ClientClosed(None));
    assert!(notes.is_empty());
}

/// A `fixed` below one block message counts 36 bytes plus its limit, the
/// largest header with its envelope, as every member does: 9 bytes above the
/// 27 that a `publish` needs.
///
/// Mutation: count a `fixed` as one block message always.
#[test]
fn a_fixed_below_one_block_message_is_counted_as_36_bytes_plus_its_limit() {
    let (ended, notes) = served_under(3_035, fixed_table(Some(3_000)));
    assert_eq!(ended, Ended::ConfigRefused);
    assert_eq!(
        notes,
        vec![Note::MessageLimitTooSmall {
            target: FIXED,
            needed: 3_036
        }]
    );
    let (ended, notes) = served_under(3_036, fixed_table(Some(3_000)));
    assert_eq!(ended, Ended::ClientClosed(None));
    assert!(notes.is_empty());
}

/// The largest limit a table can give a `fixed` still counts one block
/// message. On a 64-bit host this pins the result of the clamp, not the order
/// of the clamp and the addition: the order matters on a 32-bit `usize`,
/// where the gates do not run this test; they run it on 64-bit hosts.
///
/// Mutation: count a `fixed` at its limit, with no clamp.
#[test]
fn a_fixed_of_the_largest_table_limit_is_counted_as_one_block_message() {
    let (ended, notes) = served_under(4_130, fixed_table(Some(u32::MAX)));
    assert_eq!(ended, Ended::ClientClosed(None));
    assert!(notes.is_empty());
    let (ended, notes) = served_under(4_129, fixed_table(Some(u32::MAX)));
    assert_eq!(ended, Ended::ConfigRefused);
    assert_eq!(
        notes,
        vec![Note::MessageLimitTooSmall {
            target: FIXED,
            needed: 4_130
        }]
    );
}

/// The `n` bytes whose byte `i` is `i mod 251`.
fn pattern(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

/// The largest `fixed`, 65,280 bytes, crosses a real socket as 16 binary
/// messages, 15 of 4,130 bytes and one of 3,874, whose payloads joined are
/// the value.
#[test]
fn a_fixed_of_65_280_bytes_crosses_as_16_block_messages() {
    let (server, client) = pair();
    let rt = Loopback::new(CATALOG);
    let value = pattern(65_280);
    rt.attach()
        .provision_fixed(IFACE, FIXED.ord, &value)
        .unwrap();
    let handle = spawn(server, Scanning(rt), config());
    let mut ws = connect(client, SUBPROTOCOL).unwrap();
    attach(&mut ws);
    send(&mut ws, Frame::Read(FIXED));
    let mut joined = Vec::new();
    for k in 0..16u32 {
        let m = recv(&mut ws);
        assert_eq!(m.len(), if k < 15 { 4_130 } else { 3_874 }, "block {k}");
        let Frame::Block {
            target,
            envelope,
            total,
            index,
            payload,
        } = decode(&m).unwrap()
        else {
            panic!("message {k} is not a block");
        };
        assert_eq!(target, FIXED);
        assert_eq!(envelope.seq, 0);
        assert_eq!(total, 65_280);
        assert_eq!(index, k);
        assert!(payload.len() <= BLOCK_BYTES);
        joined.extend_from_slice(payload);
    }
    assert_eq!(joined, value);
    ws.close(None).unwrap();
    assert_eq!(handle.join().0, Ended::ClientClosed(None));
}

/// A client that reads one answer over several liveness timeouts stays
/// connected while each write makes progress.
///
/// Mutation: remove the clock shift after the message flush.
#[test]
fn a_client_that_reads_a_long_answer_slowly_is_not_ended() {
    let mut config = config();
    config.liveness_timeout = Duration::from_millis(300);
    config.ping_interval = TEST_LIMIT;
    let (server, client) = pair();
    let server = Throttled {
        stream: server,
        chunk: 8192,
        delay: Duration::from_millis(60),
        stall_after: None,
        calls: 0,
        write_timeout: std::cell::Cell::new(None),
    };
    let rt = Loopback::new(CATALOG);
    let value = pattern(65_280);
    rt.attach()
        .provision_fixed(IFACE, FIXED.ord, &value)
        .unwrap();
    let handle = spawn(server, Scanning(rt), config);
    let mut ws = connect(client, SUBPROTOCOL).unwrap();
    attach(&mut ws);
    send(&mut ws, Frame::Read(FIXED));
    let mut joined = Vec::new();
    for k in 0..16u32 {
        let m = recv(&mut ws);
        let Frame::Block {
            target,
            total,
            index,
            payload,
            ..
        } = decode(&m).unwrap()
        else {
            panic!("message {k} is not a block");
        };
        assert_eq!(target, FIXED);
        assert_eq!(total, 65_280);
        assert_eq!(index, k);
        joined.extend_from_slice(payload);
    }
    assert_eq!(joined, value);
    ws.close(None).unwrap();
    assert_eq!(handle.join().0, Ended::ClientClosed(None));
    assert_eq!(recv_close(&mut ws), Some(CloseCode::Normal));
}

/// A signal change written by a poll over several liveness timeouts does
/// not end a client whose reads keep making progress.
///
/// Mutation: remove the clock shift after the tick flush.
#[test]
fn a_client_that_reads_a_long_signal_change_slowly_is_not_ended() {
    let mut config = config();
    config.liveness_timeout = Duration::from_millis(500);
    config.ping_interval = TEST_LIMIT;
    let (server, client) = pair();
    let server = Throttled {
        stream: server,
        chunk: 1024,
        delay: Duration::from_millis(60),
        stall_after: None,
        calls: 0,
        write_timeout: std::cell::Cell::new(None),
    };
    let rt = Loopback::new(CATALOG);
    let mut provider = rt.attach();
    let limit = |t: Target| (t == SIGNAL).then_some(CALL_CEILING as u32);
    let backend = Described::new(Scanning(rt), table(limit)).unwrap();
    provider.set_limit(IFACE, SIGNAL.ord, CALL_CEILING);
    let handle = spawn(server, backend, config);
    let mut ws = connect(client, SUBPROTOCOL).unwrap();
    attach(&mut ws);
    send(&mut ws, Frame::Subscribe(SIGNAL));
    assert_eq!(
        decode(&recv(&mut ws)).unwrap(),
        Frame::Answer {
            target: SIGNAL,
            outcome: AnswerOutcome::Accepted,
        }
    );
    assert_eq!(
        decode(&recv(&mut ws)).unwrap(),
        publish(0, 0, PublishState::Init)
    );
    let value = pattern(16_128);
    provider.set(IFACE, SIGNAL.ord, &value).unwrap();
    provider.commit();
    assert_eq!(
        decode(&recv(&mut ws)).unwrap(),
        publish(0, 1, PublishState::Live(&value))
    );
    thread::sleep(Duration::from_millis(100));
    ws.close(None).unwrap();
    assert_eq!(handle.join().0, Ended::ClientClosed(None));
    assert_eq!(recv_close(&mut ws), Some(CloseCode::Normal));
}

/// A reader that stops making progress during a slow answer is ended by
/// the write timeout, even though time spent writing is not peer silence.
///
/// Mutation: `run` sets the write timeout to three liveness timeouts.
#[test]
fn a_client_that_stops_reading_during_a_slow_answer_ends_at_the_liveness_timeout() {
    let mut config = config();
    config.liveness_timeout = Duration::from_millis(500);
    let delay = Duration::from_millis(60);
    let bound = 6 * delay + 2 * config.liveness_timeout + Duration::from_millis(300);
    let (server, client) = pair();
    let server = Throttled {
        stream: server,
        chunk: 8192,
        delay,
        stall_after: Some(6),
        calls: 0,
        write_timeout: std::cell::Cell::new(None),
    };
    let rt = Loopback::new(CATALOG);
    rt.attach()
        .provision_fixed(IFACE, FIXED.ord, &pattern(65_280))
        .unwrap();
    let handle = spawn(server, Scanning(rt), config);
    let mut ws = connect(client, SUBPROTOCOL).unwrap();
    attach(&mut ws);
    send(&mut ws, Frame::Read(FIXED));
    let (ended, elapsed) = handle.join();
    assert_eq!(ended, Ended::LivenessTimeout);
    assert!(elapsed < bound, "ended after {elapsed:?}, over {bound:?}");
}

/// A client that sends reads of a large `fixed` and reads nothing more ends
/// the session at the liveness timeout: each `read` makes the server write
/// 65,824 bytes in blocks, the socket buffers fill, and a write waits its
/// whole write timeout. The first answer is read and checked to be a block,
/// so that the test fails when the value is not sent in blocks.
///
/// Mutation: `run` sets no write timeout.
///
/// Mutation: answer the read with a frame that is not a block.
#[test]
fn a_client_that_reads_a_large_fixed_and_stops_reading_ends_at_the_liveness_timeout() {
    let mut config = config();
    config.liveness_timeout = Duration::from_secs(1);
    // A full send buffer blocks a write for at most the liveness timeout, and
    // the last message arrived at most one liveness timeout earlier. The
    // margin is one second, so a loaded runner has room, and the bound stays
    // below three liveness timeouts, so a write timeout that is a multiple of
    // the liveness timeout fails the test.
    let bound = 2 * config.liveness_timeout + Duration::from_secs(1);
    let (server, client) = pair();
    let rt = Loopback::new(CATALOG);
    rt.attach()
        .provision_fixed(IFACE, FIXED.ord, &pattern(65_280))
        .unwrap();
    // The sender's last send, into a server that no longer reads, ends at
    // this timeout.
    client.set_write_timeout(Some(TEST_LIMIT)).unwrap();
    let handle = spawn(server, Scanning(rt), config);
    let mut ws = connect(client, SUBPROTOCOL).unwrap();
    attach(&mut ws);
    send(&mut ws, Frame::Read(FIXED));
    let first = recv(&mut ws);
    assert!(
        matches!(decode(&first).unwrap(), Frame::Block { .. }),
        "the first answer is a block"
    );

    // The reads are sent from another thread, which reads nothing, so that
    // the test thread waits only in `join`, which `TEST_LIMIT` bounds.
    let (done, ended) = mpsc::channel::<()>();
    let sender = thread::spawn(move || {
        while let Err(TryRecvError::Empty) = ended.try_recv() {
            if ws
                .send(Message::binary(encode(Frame::Read(FIXED))))
                .is_err()
            {
                break;
            }
        }
    });
    let (ended, elapsed) = handle.join();
    done.send(()).ok();
    sender.join().unwrap();
    assert_eq!(ended, Ended::LivenessTimeout);
    assert!(elapsed < bound, "ended after {elapsed:?}, over {bound:?}");
}

const ALL_VERSIONS: &str = "ridl-ws-v3, ridl-ws-v2, ridl-ws-v1";

/// A client that offers every version against `backend`: the subprotocol
/// the server selects, after the client attaches with the frame version of
/// that subprotocol.
fn selected_against<B: Backend + Send + 'static>(backend: B) -> Option<String> {
    let (server, client) = pair();
    let handle = spawn(server, backend, config());
    let (mut ws, selected) = connect_selected(client, ALL_VERSIONS);
    let version = match selected.as_deref() {
        Some("ridl-ws-v3") => FRAME_VERSION_3,
        Some("ridl-ws-v2") => FRAME_VERSION,
        other => panic!("unexpected subprotocol {other:?}"),
    };
    let attach = Attach::new(version, 1, CATALOG.hash, CATALOG.name).unwrap();
    send(&mut ws, Frame::Attach(attach));
    assert_eq!(
        decode(&recv(&mut ws)).unwrap(),
        Frame::Attached(AttachOutcome::Accepted)
    );
    ws.close(None).unwrap();
    assert_eq!(recv_close(&mut ws), Some(CloseCode::Normal));
    assert_eq!(handle.join().0, Ended::ClientClosed(None));
    selected
}

/// A backend that serves streams and has a member table.
fn streams_backend() -> Described<Streaming<Scanning<Loopback>>> {
    Described::new(Streaming(Scanning(Loopback::new(CATALOG))), table(|_| None)).unwrap()
}

#[test]
fn a_v2_client_against_a_streams_server_is_served_v2() {
    let (server, client) = pair();
    let handle = spawn(server, streams_backend(), config());
    let (mut ws, selected) = connect_selected(client, "ridl-ws-v2, ridl-ws-v1");
    assert_eq!(selected.as_deref(), Some("ridl-ws-v2"));
    attach(&mut ws);
    ws.close(None).unwrap();
    assert_eq!(recv_close(&mut ws), Some(CloseCode::Normal));
    assert_eq!(handle.join().0, Ended::ClientClosed(None));
}

#[test]
fn a_v3_client_against_a_backend_without_streams_is_selected_v2() {
    let backend = Described::new(Scanning(Loopback::new(CATALOG)), table(|_| None)).unwrap();
    assert_eq!(selected_against(backend).as_deref(), Some("ridl-ws-v2"));
}

#[test]
fn a_v3_client_against_a_backend_without_a_table_is_selected_v2() {
    let backend = Streaming(Scanning(Loopback::new(CATALOG)));
    assert_eq!(selected_against(backend).as_deref(), Some("ridl-ws-v2"));
}

#[test]
fn a_v3_client_against_a_streams_server_is_selected_v3() {
    assert_eq!(
        selected_against(streams_backend()).as_deref(),
        Some("ridl-ws-v3")
    );
}

#[test]
fn a_stream_form_on_a_v2_session_closes_1002() {
    let (server, client) = pair();
    let (config, notes) = recording();
    let handle = spawn(server, streams_backend(), config);
    let (mut ws, selected) =
        connect_selected(client, ALL_VERSIONS.replace("ridl-ws-v3, ", "").as_str());
    assert_eq!(selected.as_deref(), Some("ridl-ws-v2"));
    attach(&mut ws);
    send(
        &mut ws,
        Frame::StreamEnd {
            target: QUERY,
            envelope: Envelope {
                stamp: Timestamp(0),
                seq: 0,
            },
            correlation: 1,
            stream: 0,
            outcome: StreamOutcome::End,
        },
    );
    assert_eq!(recv_close(&mut ws), Some(CloseCode::Protocol));
    assert_eq!(handle.join().0, Ended::ServerClosed(1002));
    assert_eq!(
        *notes.lock().unwrap(),
        vec![Note::Violation(ProtocolViolation::UnknownCode)]
    );
}

// -- A streamed reply over frame version 3 ---------------------------------

/// The query of the test provider whose reply is a stream.
const STREAM_QUERY: Target = Target {
    kind: Kind::Query,
    iface: provider::IFACE,
    ord: provider::STREAM_QUERY,
};
/// The plain query of the test provider.
const PLAIN_QUERY: Target = Target {
    kind: Kind::Query,
    iface: provider::IFACE,
    ord: provider::QUERY,
};
/// How long a test waits to conclude that no frame is coming.
const QUIET: Duration = Duration::from_millis(200);

/// The test provider, shared with the server's backend, stepped every
/// millisecond on its own thread when it was made by [`Stepped::start`].
/// Dropping the value stops the thread.
struct Stepped {
    shared: Arc<Mutex<(Loopback, provider::Streams)>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Stepped {
    /// A provider stepped on its own thread.
    fn start(rt: Loopback) -> Self {
        let mut stepped = Self::held(rt);
        let shared = Arc::clone(&stepped.shared);
        let stop = Arc::clone(&stepped.stop);
        stepped.thread = Some(thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                {
                    let (rt, streams) = &mut *shared.lock().unwrap();
                    provider::provider_step(rt, streams);
                }
                thread::sleep(Duration::from_millis(1));
            }
        }));
        stepped
    }

    /// A provider that is never stepped, so that no call is claimed.
    fn held(rt: Loopback) -> Self {
        Stepped {
            shared: Arc::new(Mutex::new((rt, provider::Streams::default()))),
            stop: Arc::new(AtomicBool::new(false)),
            thread: None,
        }
    }

    /// The failures the provider's pushes read so far.
    fn failures(&self) -> Vec<CallError> {
        self.shared.lock().unwrap().1.failures.clone()
    }

    /// Makes one pass of the provider, for a provider made by
    /// [`Stepped::held`].
    fn step(&self) {
        let (rt, streams) = &mut *self.shared.lock().unwrap();
        provider::provider_step(rt, streams);
    }

    /// Sets the signal of the test provider and commits it.
    fn set_signal(&self, bytes: &[u8]) {
        let (rt, _) = &mut *self.shared.lock().unwrap();
        rt.set(provider::IFACE, provider::SIGNAL, bytes).unwrap();
        rt.commit();
    }
}

impl Drop for Stepped {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

/// A session with the test provider, attached under the frame version of
/// `protocol`, over a backend that carries streams and has the provider's
/// member table. With `step`, the provider is stepped on its own thread.
fn stream_session(
    config: Config,
    protocol: &str,
    step: bool,
) -> (WebSocket<TcpStream>, Server, Stepped) {
    let (server, client) = pair();
    let rt = provider::provider();
    let backend = Described::new(
        Streaming(Scanning(rt.attach())),
        Arc::new(provider::member_table()),
    )
    .unwrap();
    let stepped = if step {
        Stepped::start(rt)
    } else {
        Stepped::held(rt)
    };
    let handle = spawn(server, backend, config);
    let (mut ws, selected) = connect_selected(client, protocol);
    assert_eq!(selected.as_deref(), Some(protocol));
    let version = if protocol == SUBPROTOCOL_V3 {
        FRAME_VERSION_3
    } else {
        FRAME_VERSION
    };
    let catalog = provider::CATALOG;
    let attach = Attach::new(version, 1, catalog.hash, catalog.name).unwrap();
    send(&mut ws, Frame::Attach(attach));
    assert_eq!(
        decode(&recv(&mut ws)).unwrap(),
        Frame::Attached(AttachOutcome::Accepted)
    );
    (ws, handle, stepped)
}

/// A version 3 session with the stepped test provider.
fn v3_session(config: Config) -> (WebSocket<TcpStream>, Server, Stepped) {
    stream_session(config, SUBPROTOCOL_V3, true)
}

/// Closes the session from the client and checks that `serve` returns.
fn close(mut ws: WebSocket<TcpStream>, handle: Server) {
    ws.close(None).unwrap();
    assert_eq!(recv_close(&mut ws), Some(CloseCode::Normal));
    assert_eq!(handle.join().0, Ended::ClientClosed(None));
}

/// Sends a `request` of `target` under `seq`.
fn request(ws: &mut WebSocket<TcpStream>, target: Target, seq: u64, payload: &[u8]) {
    send(
        ws,
        Frame::Request {
            target,
            envelope: Envelope {
                stamp: Timestamp(0),
                seq,
            },
            payload,
        },
    );
}

/// Grants `credit` more chunks of the reply stream of the call `correlation`.
fn grant(ws: &mut WebSocket<TcpStream>, correlation: u64, credit: u32) {
    send(
        ws,
        Frame::StreamCredit {
            target: STREAM_QUERY,
            correlation,
            stream: 0,
            credit,
        },
    );
}

/// The `chunk` the server sends as element `element` of the reply stream of
/// the call `correlation`, under the stream `seq` `seq`.
fn element(correlation: u64, seq: u64, element: u8) -> Vec<u8> {
    encode(Frame::Chunk {
        target: STREAM_QUERY,
        envelope: Envelope {
            stamp: Timestamp(0),
            seq,
        },
        correlation,
        stream: 0,
        payload: &[element; provider::ELEMENT_LEN],
    })
}

/// The `stream_end` the server sends for the reply stream of the call
/// `correlation`, under the stream `seq` `seq`.
fn stream_end(correlation: u64, seq: u64, outcome: StreamOutcome) -> Frame<'static> {
    Frame::StreamEnd {
        target: STREAM_QUERY,
        envelope: Envelope {
            stamp: Timestamp(0),
            seq,
        },
        correlation,
        stream: 0,
        outcome,
    }
}

/// Fails the test when a binary message arrives within [`QUIET`].
fn assert_quiet(ws: &mut WebSocket<TcpStream>) {
    ws.get_mut().set_read_timeout(Some(QUIET)).unwrap();
    let start = Instant::now();
    while start.elapsed() < QUIET {
        match ws.read() {
            Ok(Message::Binary(b)) => panic!("unexpected frame {:?}", decode(&b)),
            Ok(Message::Ping(_) | Message::Pong(_)) => {}
            Ok(other) => panic!("unexpected message {other:?}"),
            Err(tungstenite::Error::Io(e))
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                break
            }
            Err(e) => panic!("read failed: {e}"),
        }
    }
    ws.get_mut().set_read_timeout(Some(TEST_LIMIT)).unwrap();
}

/// The next binary message, when one arrives within `wait`.
fn try_recv(ws: &mut WebSocket<TcpStream>, wait: Duration) -> Option<Vec<u8>> {
    ws.get_mut().set_read_timeout(Some(wait)).unwrap();
    let got = loop {
        match ws.read() {
            Ok(Message::Binary(b)) => break Some(b.to_vec()),
            Ok(Message::Ping(_) | Message::Pong(_)) => {}
            Ok(other) => panic!("unexpected message {other:?}"),
            Err(tungstenite::Error::Io(e))
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                break None
            }
            Err(e) => panic!("read failed: {e}"),
        }
    };
    ws.get_mut().set_read_timeout(Some(TEST_LIMIT)).unwrap();
    got
}

/// Sends a `request` of the plain query under `seq`, then reads up to its
/// `response`, and returns the frames read before it.
fn sync(ws: &mut WebSocket<TcpStream>, seq: u64) -> Vec<Vec<u8>> {
    request(ws, PLAIN_QUERY, seq, &[5]);
    let mut before = Vec::new();
    loop {
        let m = recv(ws);
        if matches!(decode(&m).unwrap(), Frame::Response { correlation, .. } if correlation == seq)
        {
            return before;
        }
        before.push(m);
    }
}

/// Subscribes the provider's signal and reads the answer and the first
/// `publish`. The server reads messages in order, so every message sent
/// before has then been handled: a `request` has reached the backend, and a
/// `stream_credit` has been added.
fn barrier(ws: &mut WebSocket<TcpStream>) {
    let signal = Target {
        kind: Kind::Signal,
        iface: provider::IFACE,
        ord: provider::SIGNAL,
    };
    send(ws, Frame::Subscribe(signal));
    assert!(matches!(decode(&recv(ws)).unwrap(), Frame::Answer { .. }));
    assert!(matches!(decode(&recv(ws)).unwrap(), Frame::Publish { .. }));
}

/// Waits until `cond` holds, or fails the test after [`TEST_LIMIT`].
fn wait_until(what: &str, cond: impl Fn() -> bool) {
    let start = Instant::now();
    while !cond() {
        assert!(start.elapsed() < TEST_LIMIT, "{what} within the limit");
        thread::sleep(Duration::from_millis(1));
    }
}

/// Mutations: answer the query with a `response`; start the stream `seq` at
/// 0; send the chunks past the credit; leave out the `stream_end`.
#[test]
fn streams_64_elements_then_ends_with_the_next_seq() {
    let (mut ws, handle, _p) = v3_session(config());
    request(&mut ws, STREAM_QUERY, 1, &[64]);
    for i in 0..64u8 {
        assert_eq!(recv(&mut ws), element(1, u64::from(i) + 1, i), "chunk {i}");
        grant(&mut ws, 1, 1);
    }
    assert_eq!(
        decode(&recv(&mut ws)).unwrap(),
        stream_end(1, 65, StreamOutcome::End)
    );
    assert_quiet(&mut ws);
    close(ws, handle);
}

#[test]
fn an_empty_stream_ends_with_seq_1() {
    let (mut ws, handle, _p) = v3_session(config());
    request(&mut ws, STREAM_QUERY, 1, &[0]);
    assert_eq!(
        decode(&recv(&mut ws)).unwrap(),
        stream_end(1, 1, StreamOutcome::End)
    );
    assert_quiet(&mut ws);
    close(ws, handle);
}

/// The credit is granted here only once the first 4 are spent, so adding it
/// and replacing it give the same result:
/// `a_credit_adds_to_the_credit_left` pins the add.
///
/// Mutations: an initial credit other than 4; ignore the `stream_credit`.
#[test]
fn no_credit_means_four_chunks_and_no_more() {
    let (mut ws, handle, _p) = v3_session(config());
    request(&mut ws, STREAM_QUERY, 1, &[64]);
    for i in 0..4u8 {
        assert_eq!(recv(&mut ws), element(1, u64::from(i) + 1, i), "chunk {i}");
    }
    assert_quiet(&mut ws);
    grant(&mut ws, 1, 2);
    for i in 4..6u8 {
        assert_eq!(recv(&mut ws), element(1, u64::from(i) + 1, i), "chunk {i}");
    }
    assert_quiet(&mut ws);
    close(ws, handle);
}

/// Mutation: wait for credit inside the tick.
#[test]
fn credit_stall_does_not_block_a_signal() {
    let (mut ws, handle, provider) = v3_session(config());
    let signal = Target {
        kind: Kind::Signal,
        iface: provider::IFACE,
        ord: provider::SIGNAL,
    };
    send(&mut ws, Frame::Subscribe(signal));
    assert_eq!(
        decode(&recv(&mut ws)).unwrap(),
        Frame::Answer {
            target: signal,
            outcome: AnswerOutcome::Accepted,
        }
    );
    assert!(matches!(
        decode(&recv(&mut ws)).unwrap(),
        Frame::Publish {
            state: PublishState::Init,
            ..
        }
    ));
    request(&mut ws, STREAM_QUERY, 1, &[64]);
    for i in 0..4u8 {
        assert_eq!(recv(&mut ws), element(1, u64::from(i) + 1, i), "chunk {i}");
    }
    assert_quiet(&mut ws);
    provider.set_signal(&[9]);
    assert_eq!(
        decode(&recv(&mut ws)).unwrap(),
        Frame::Publish {
            target: signal,
            envelope: Envelope {
                stamp: Timestamp(0),
                seq: 1,
            },
            state: PublishState::Live(&[9]),
        }
    );
    close(ws, handle);
}

/// The client's `stream_end` reaches the provider as
/// `Transport::Interrupted`: the server aborts the stream and then forgets
/// the call, and after a forget a push answers `Interrupted` whatever the
/// abort carried, so this test cannot tell the abort from the forget. It pins
/// that the stream stops on the backend, that a later credit sends nothing,
/// and the note.
///
/// Mutations: leave out both the abort and the forget; leave out the note.
#[test]
fn client_interrupt_forgets_the_call_on_the_backend() {
    let (config, notes) = recording();
    let (mut ws, handle, provider) = v3_session(config);
    request(&mut ws, STREAM_QUERY, 1, &[64]);
    for i in 0..2u8 {
        assert_eq!(recv(&mut ws), element(1, u64::from(i) + 1, i), "chunk {i}");
    }
    send(
        &mut ws,
        Frame::StreamEnd {
            target: STREAM_QUERY,
            envelope: Envelope {
                stamp: Timestamp(0),
                seq: 1,
            },
            correlation: 1,
            stream: 0,
            outcome: StreamOutcome::Interrupted,
        },
    );
    // A credit for a stream the server no longer sends is ignored.
    grant(&mut ws, 1, 10);
    // The plain query's response comes after every frame the server sent
    // before it read the `stream_end`: at most the 2 chunks left of the
    // initial credit.
    request(&mut ws, PLAIN_QUERY, 2, &[5]);
    let mut late = 0;
    loop {
        let m = recv(&mut ws);
        match decode(&m).unwrap() {
            Frame::Response { correlation: 2, .. } => break,
            Frame::Chunk { correlation: 1, .. } => late += 1,
            other => panic!("unexpected frame {other:?}"),
        }
    }
    assert!(late <= 2, "{late} chunks after the cancel");
    assert_quiet(&mut ws);
    wait_until("a failed push", || !provider.failures().is_empty());
    assert_eq!(
        provider.failures(),
        vec![CallError::Transport(Transport::Interrupted)]
    );
    assert!(notes
        .lock()
        .unwrap()
        .contains(&Note::StreamStopped { seq: 1 }));
    close(ws, handle);
}

/// Mutation: answer a `Pull::Failed` with `stream_end end`.
#[test]
fn provider_interrupt_reaches_the_client() {
    let (mut ws, handle, _p) = v3_session(config());
    request(&mut ws, STREAM_QUERY, 1, &[provider::STREAM_INTERRUPT]);
    for i in 0..3u8 {
        assert_eq!(recv(&mut ws), element(1, u64::from(i) + 1, i), "chunk {i}");
    }
    assert_eq!(
        decode(&recv(&mut ws)).unwrap(),
        stream_end(1, 4, StreamOutcome::Interrupted)
    );
    assert_quiet(&mut ws);
    close(ws, handle);
}

/// Mutations: keep the `stream_end end` that was sent instead of the
/// `interrupted` one; keep nothing, so the repeat is dropped.
#[test]
fn retransmit_after_the_end_is_answered_stream_end_interrupted() {
    let (config, notes) = recording();
    let (mut ws, handle, _p) = v3_session(config);
    request(&mut ws, STREAM_QUERY, 1, &[1]);
    assert_eq!(recv(&mut ws), element(1, 1, 0));
    assert_eq!(
        decode(&recv(&mut ws)).unwrap(),
        stream_end(1, 2, StreamOutcome::End)
    );
    request(&mut ws, STREAM_QUERY, 1, &[1]);
    assert_eq!(
        decode(&recv(&mut ws)).unwrap(),
        stream_end(1, 1, StreamOutcome::Interrupted)
    );
    assert_quiet(&mut ws);
    assert!(notes.lock().unwrap().is_empty());
    close(ws, handle);
}

/// Mutation: answer a refused dispatch with a `response`.
#[test]
fn a_busy_admission_is_a_stream_end_busy_with_seq_1() {
    let (mut ws, handle, _p) = stream_session(config(), SUBPROTOCOL_V3, false);
    // The provider is not stepped, so every call keeps its slot of the
    // loopback's call table, and the call after the last slot is refused.
    let slots = Loopback::SLOTS as u64;
    for seq in 1..=slots {
        request(&mut ws, STREAM_QUERY, seq, &[1]);
    }
    request(&mut ws, STREAM_QUERY, slots + 1, &[1]);
    assert_eq!(
        decode(&recv(&mut ws)).unwrap(),
        stream_end(slots + 1, 1, StreamOutcome::Busy)
    );
    assert_quiet(&mut ws);
    close(ws, handle);
}

/// A client stream frame that a session with no parameter stream cannot
/// accept closes the session with 1002 and records `note`.
fn refused_from_the_client(frame: Frame<'_>, note: Note) {
    let (config, notes) = recording();
    let (mut ws, handle, _p) = v3_session(config);
    request(&mut ws, STREAM_QUERY, 1, &[64]);
    send(&mut ws, frame);
    assert_eq!(recv_close(&mut ws), Some(CloseCode::Protocol));
    assert_eq!(handle.join().0, Ended::ServerClosed(1002));
    assert_eq!(*notes.lock().unwrap(), vec![note]);
}

fn client_chunk(stream: u8) -> Frame<'static> {
    Frame::Chunk {
        target: STREAM_QUERY,
        envelope: Envelope {
            stamp: Timestamp(0),
            seq: 1,
        },
        correlation: 1,
        stream,
        payload: &[1],
    }
}

#[test]
fn a_client_chunk_on_stream_0_closes_1002() {
    refused_from_the_client(client_chunk(0), Note::UnexpectedDirection);
}

#[test]
fn a_client_chunk_on_stream_1_closes_1002() {
    refused_from_the_client(client_chunk(1), Note::StreamCreditExceeded { seq: 1 });
}

#[test]
fn a_client_stream_end_end_closes_1002() {
    refused_from_the_client(
        Frame::StreamEnd {
            target: STREAM_QUERY,
            envelope: Envelope {
                stamp: Timestamp(0),
                seq: 1,
            },
            correlation: 1,
            stream: 0,
            outcome: StreamOutcome::End,
        },
        Note::Violation(ProtocolViolation::OutcomeNotAllowed),
    );
}

#[test]
fn a_plain_query_on_a_v3_session_is_answered_by_a_response() {
    let (mut ws, handle, _p) = v3_session(config());
    request(&mut ws, PLAIN_QUERY, 1, &[1, 2]);
    assert_eq!(
        decode(&recv(&mut ws)).unwrap(),
        Frame::Response {
            target: PLAIN_QUERY,
            envelope: Envelope {
                stamp: Timestamp(0),
                seq: 1,
            },
            correlation: 1,
            outcome: ResponseOutcome::Reply(&[2, 3]),
        }
    );
    close(ws, handle);
}

/// Mutation: serve the stream whatever the frame version.
#[test]
fn a_streamed_member_on_a_v2_session_is_unknown_interaction() {
    let (config, notes) = recording();
    let (mut ws, handle, _p) = stream_session(config, SUBPROTOCOL, true);
    request(&mut ws, STREAM_QUERY, 1, &[1]);
    assert_eq!(
        decode(&recv(&mut ws)).unwrap(),
        Frame::Response {
            target: STREAM_QUERY,
            envelope: Envelope {
                stamp: Timestamp(0),
                seq: 1,
            },
            correlation: 1,
            outcome: ResponseOutcome::Contract(Contract::UnknownInteraction),
        }
    );
    assert_quiet(&mut ws);
    assert_eq!(
        *notes.lock().unwrap(),
        vec![Note::UnknownInteraction {
            target: STREAM_QUERY
        }]
    );
    close(ws, handle);
}

/// Mutations: never abandon a streamed call; abandon it with nothing sent.
#[test]
fn an_abandoned_stream_ends_interrupted() {
    let (mut config, notes) = recording();
    config.call_abandon_after = Duration::from_millis(100);
    let (mut ws, handle, _p) = v3_session(config);
    request(&mut ws, STREAM_QUERY, 1, &[64]);
    for i in 0..4u8 {
        assert_eq!(recv(&mut ws), element(1, u64::from(i) + 1, i), "chunk {i}");
    }
    assert_eq!(
        decode(&recv(&mut ws)).unwrap(),
        stream_end(1, 5, StreamOutcome::Interrupted)
    );
    assert_eq!(*notes.lock().unwrap(), vec![Note::CallAbandoned { seq: 1 }]);
    close(ws, handle);
}

/// A stream that ends right after the last chunk the credit allows sends its
/// `stream_end` with no more credit: the server holds one more pull.
///
/// Mutation: stop pulling while the credit is 0.
#[test]
fn a_stream_of_exactly_the_initial_credit_ends_with_no_more_credit() {
    let (mut ws, handle, _p) = v3_session(config());
    request(&mut ws, STREAM_QUERY, 1, &[4]);
    for i in 0..4u8 {
        assert_eq!(recv(&mut ws), element(1, u64::from(i) + 1, i), "chunk {i}");
    }
    assert_eq!(
        decode(&recv(&mut ws)).unwrap(),
        stream_end(1, 5, StreamOutcome::End)
    );
    assert_quiet(&mut ws);
    close(ws, handle);
}

/// A chunk pulled with no credit left is held and sent first, in order, when
/// credit arrives.
///
/// Mutation: drop the held chunk.
#[test]
fn a_held_chunk_is_sent_first_when_credit_arrives() {
    let (mut ws, handle, _p) = v3_session(config());
    request(&mut ws, STREAM_QUERY, 1, &[6]);
    for i in 0..4u8 {
        assert_eq!(recv(&mut ws), element(1, u64::from(i) + 1, i), "chunk {i}");
    }
    assert_quiet(&mut ws);
    grant(&mut ws, 1, 2);
    for i in 4..6u8 {
        assert_eq!(recv(&mut ws), element(1, u64::from(i) + 1, i), "chunk {i}");
    }
    assert_eq!(
        decode(&recv(&mut ws)).unwrap(),
        stream_end(1, 7, StreamOutcome::End)
    );
    close(ws, handle);
}

/// Each ended stream frees its call on the backend: more streams than the
/// loopback has call slots run one after another on one session.
///
/// Mutation: remove the call from the session only, not from the backend.
#[test]
fn more_streams_than_call_slots_run_one_after_another() {
    let (mut ws, handle, _p) = v3_session(config());
    for seq in 1..=(Loopback::SLOTS as u64 + 4) {
        request(&mut ws, STREAM_QUERY, seq, &[0]);
        assert_eq!(
            decode(&recv(&mut ws)).unwrap(),
            stream_end(seq, 1, StreamOutcome::End),
            "stream {seq}"
        );
    }
    close(ws, handle);
}

/// Mutation: keep nothing after a client stop, so the repeat is dropped.
#[test]
fn a_retransmit_after_a_client_stop_is_answered_stream_end_interrupted() {
    let (mut ws, handle, _p) = v3_session(config());
    request(&mut ws, STREAM_QUERY, 1, &[64]);
    assert_eq!(recv(&mut ws), element(1, 1, 0));
    send(
        &mut ws,
        Frame::StreamEnd {
            target: STREAM_QUERY,
            envelope: Envelope {
                stamp: Timestamp(0),
                seq: 1,
            },
            correlation: 1,
            stream: 0,
            outcome: StreamOutcome::Interrupted,
        },
    );
    sync(&mut ws, 2);
    request(&mut ws, STREAM_QUERY, 1, &[64]);
    assert_eq!(
        decode(&recv(&mut ws)).unwrap(),
        stream_end(1, 1, StreamOutcome::Interrupted)
    );
    assert_quiet(&mut ws);
    close(ws, handle);
}

/// A credit granted before the initial 4 are spent adds to them: the
/// provider is stepped only once the credit has arrived, and 6 chunks cross.
///
/// Mutation: replace the credit instead of adding to it.
#[test]
fn a_credit_adds_to_the_credit_left() {
    let (mut ws, handle, provider) = stream_session(config(), SUBPROTOCOL_V3, false);
    request(&mut ws, STREAM_QUERY, 1, &[64]);
    grant(&mut ws, 1, 2);
    barrier(&mut ws);
    for _ in 0..10 {
        provider.step();
        thread::sleep(Duration::from_millis(20));
    }
    let mut chunks = 0u8;
    while let Some(m) = try_recv(&mut ws, QUIET) {
        assert_eq!(m, element(1, u64::from(chunks) + 1, chunks));
        chunks += 1;
    }
    assert_eq!(chunks, 6);
    close(ws, handle);
}

/// A `stream_credit` restarts the abandon bound: credits every 100 ms keep a
/// stream whose provider sends nothing open for twice the bound, and the
/// stream is abandoned once they stop.
///
/// Mutation: do not restart the bound on a credit.
#[test]
fn a_credit_restarts_the_abandon_bound() {
    let (mut config, notes) = recording();
    config.call_abandon_after = Duration::from_millis(300);
    let (mut ws, handle, _p) = stream_session(config, SUBPROTOCOL_V3, false);
    request(&mut ws, STREAM_QUERY, 1, &[64]);
    for _ in 0..6 {
        thread::sleep(Duration::from_millis(100));
        grant(&mut ws, 1, 1);
        if let Some(m) = try_recv(&mut ws, Duration::from_millis(1)) {
            panic!("a frame while the credits flow: {:?}", decode(&m));
        }
    }
    assert_eq!(
        decode(&recv(&mut ws)).unwrap(),
        stream_end(1, 1, StreamOutcome::Interrupted)
    );
    assert_eq!(*notes.lock().unwrap(), vec![Note::CallAbandoned { seq: 1 }]);
    close(ws, handle);
}

/// A `chunk` sent restarts the abandon bound: with one credit granted at the
/// start, a provider that pushes every 100 ms keeps the stream open for
/// twice the bound, and the stream is abandoned once it stops.
///
/// Mutation: do not restart the bound on a chunk.
#[test]
fn a_chunk_restarts_the_abandon_bound() {
    let (mut config, notes) = recording();
    config.call_abandon_after = Duration::from_millis(300);
    let (mut ws, handle, provider) = stream_session(config, SUBPROTOCOL_V3, false);
    request(&mut ws, STREAM_QUERY, 1, &[64]);
    grant(&mut ws, 1, 100);
    barrier(&mut ws);
    let mut seq = 0u64;
    for _ in 0..6 {
        provider.step();
        // One step fills the loopback's window of 4 chunks.
        for _ in 0..4 {
            let i = seq as u8;
            seq += 1;
            assert_eq!(recv(&mut ws), element(1, seq, i), "chunk {seq}");
        }
        thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(
        decode(&recv(&mut ws)).unwrap(),
        stream_end(1, seq + 1, StreamOutcome::Interrupted)
    );
    assert_eq!(*notes.lock().unwrap(), vec![Note::CallAbandoned { seq: 1 }]);
    close(ws, handle);
}

const UPLOAD: Target = Target {
    kind: Kind::Command,
    iface: provider::IFACE,
    ord: provider::UPLOAD,
};
const UPLOAD_QUERY: Target = Target {
    kind: Kind::Query,
    iface: provider::IFACE,
    ord: provider::UPLOAD_QUERY,
};
const FETCH: Target = Target {
    kind: Kind::Query,
    iface: provider::IFACE,
    ord: provider::FETCH,
};

fn upload_chunk(ws: &mut WebSocket<TcpStream>, target: Target, seq: u64, payload: &[u8]) {
    send(
        ws,
        Frame::Chunk {
            target,
            envelope: Envelope {
                stamp: Timestamp(0),
                seq,
            },
            correlation: 1,
            stream: 1,
            payload,
        },
    );
}
fn upload_end(ws: &mut WebSocket<TcpStream>, target: Target, seq: u64) {
    send(
        ws,
        Frame::StreamEnd {
            target,
            envelope: Envelope {
                stamp: Timestamp(0),
                seq,
            },
            correlation: 1,
            stream: 1,
            outcome: StreamOutcome::End,
        },
    );
}

/// One chunk at a time, waiting for its returned credit before the next one.
fn write_upload(ws: &mut WebSocket<TcpStream>, target: Target, bytes: &[u8]) {
    let mut seq = 1;
    for chunk in bytes.chunks(provider::FETCH_CHUNK) {
        upload_chunk(ws, target, seq, chunk);
        assert_eq!(
            decode(&recv(ws)).unwrap(),
            Frame::StreamCredit {
                target,
                correlation: 1,
                stream: 1,
                credit: 1,
            }
        );
        seq += 1;
    }
    upload_end(ws, target, seq);
}

fn assert_accepted(ws: &mut WebSocket<TcpStream>) {
    assert!(matches!(
        decode(&recv(ws)).unwrap(),
        Frame::Response {
            target: UPLOAD,
            correlation: 1,
            outcome: ResponseOutcome::Accepted,
            ..
        }
    ));
}

// Mutation: drop a stored chunk, or fetch before draining the preceding upload.
#[test]
fn an_upload_of_200_000_bytes_is_stored_and_fetched_back() {
    let (mut ws, handle, _p) = v3_session(config());
    request(&mut ws, UPLOAD, 1, &[1, 0]);
    assert_accepted(&mut ws);
    let bytes = provider::pattern(200_000);
    write_upload(&mut ws, UPLOAD, &bytes);
    request(&mut ws, FETCH, 2, &[1]);
    let mut received = Vec::new();
    let mut seq = 1;
    loop {
        let raw = recv(&mut ws);
        match decode(&raw).unwrap() {
            Frame::Chunk {
                target,
                correlation,
                stream,
                envelope,
                payload,
            } => {
                assert_eq!(
                    (target, correlation, stream, envelope.seq),
                    (FETCH, 2, 0, seq)
                );
                assert!(payload.len() <= 16_128);
                received.extend_from_slice(payload);
                send(
                    &mut ws,
                    Frame::StreamCredit {
                        target: FETCH,
                        correlation: 2,
                        stream: 0,
                        credit: 1,
                    },
                );
            }
            Frame::StreamEnd {
                target,
                correlation,
                stream,
                envelope,
                outcome,
            } => {
                assert_eq!(
                    (target, correlation, stream, envelope.seq, outcome),
                    (FETCH, 2, 0, seq, StreamOutcome::End)
                );
                break;
            }
            other => panic!("unexpected fetch frame: {other:?}"),
        }
        seq += 1;
    }
    assert_eq!(received, bytes);
    close(ws, handle);
}

// Mutation: do not refuse the third block, or close the session on refusal.
#[test]
fn a_refused_third_block_sends_stream_end_contract_on_stream_1() {
    let (mut ws, handle, p) = stream_session(config(), SUBPROTOCOL_V3, false);
    request(&mut ws, UPLOAD, 1, &[2, provider::MODE_REFUSE_RANGE]);
    // A fixed read is a barrier proving that the request reached the backend.
    send(&mut ws, Frame::Read(FIXED));
    assert!(matches!(
        decode(&recv(&mut ws)).unwrap(),
        Frame::Publish { target: FIXED, .. }
    ));
    p.step();
    assert_accepted(&mut ws);
    for seq in 1..=3 {
        upload_chunk(&mut ws, UPLOAD, seq, &[1]);
        assert!(matches!(
            decode(&recv(&mut ws)).unwrap(),
            Frame::StreamCredit { credit: 1, .. }
        ));
        p.step();
    }
    upload_chunk(&mut ws, UPLOAD, 4, &[1]);
    let raw = recv(&mut ws);
    assert!(matches!(
        decode(&raw).unwrap(),
        Frame::StreamEnd {
            target: UPLOAD,
            correlation: 1,
            stream: 1,
            outcome: StreamOutcome::Contract(Contract::InvalidValue(ridl_rt::payload::Violation {
                type_name: None,
                rule: ridl_rt::payload::Rule::Range,
            })),
            ..
        }
    ));
    upload_chunk(&mut ws, UPLOAD, 5, &[1]);
    request(&mut ws, PLAIN_QUERY, 2, &[9]);
    // A fixed read gives a barrier before explicitly stepping the provider.
    send(&mut ws, Frame::Read(FIXED));
    assert!(matches!(
        decode(&recv(&mut ws)).unwrap(),
        Frame::Publish { target: FIXED, .. }
    ));
    p.step();
    assert!(matches!(
        decode(&recv(&mut ws)).unwrap(),
        Frame::Response {
            correlation: 2,
            outcome: ResponseOutcome::Reply([10]),
            ..
        }
    ));
    close(ws, handle);
}

// Mutation: accept a chunk one byte above the member limit.
#[test]
fn an_oversized_chunk_is_answered_stream_end_corrupt() {
    let (config, notes) = recording();
    let (mut ws, handle, _p) = v3_session(config);
    request(&mut ws, UPLOAD, 1, &[3, 0]);
    assert_accepted(&mut ws);
    upload_chunk(&mut ws, UPLOAD, 1, &vec![0; 16_129]);
    assert!(matches!(
        decode(&recv(&mut ws)).unwrap(),
        Frame::StreamEnd {
            target: UPLOAD,
            correlation: 1,
            stream: 1,
            outcome: StreamOutcome::Corrupt,
            ..
        }
    ));
    assert_eq!(
        *notes.lock().unwrap(),
        vec![Note::ChunkTooLarge {
            seq: 1,
            stream: 1,
            len: 16_129
        }]
    );
    close(ws, handle);
}

// Mutation: settle the upload query before consuming every chunk.
#[test]
fn a_query_upload_replies_the_count_and_the_sum() {
    let (mut ws, handle, _p) = v3_session(config());
    request(&mut ws, UPLOAD_QUERY, 1, &[0]);
    let bytes = provider::pattern(200_000);
    write_upload(&mut ws, UPLOAD_QUERY, &bytes);
    let mut expected = 200_000u64.to_le_bytes().to_vec();
    let sum = bytes
        .iter()
        .fold(0u32, |sum, b| sum.wrapping_add(u32::from(*b)));
    expected.extend_from_slice(&sum.to_le_bytes());
    assert!(matches!(decode(&recv(&mut ws)).unwrap(), Frame::Response {
        target: UPLOAD_QUERY, correlation: 1, outcome: ResponseOutcome::Reply(payload), ..
    } if payload == expected));
    close(ws, handle);
}

// Mutation: block session dispatch while an upload queue cannot drain.
#[test]
fn a_stalled_upload_does_not_block_a_signal() {
    let (mut ws, handle, p) = stream_session(config(), SUBPROTOCOL_V3, false);
    request(&mut ws, UPLOAD, 1, &[4, 0]);
    for seq in 1..=4 {
        upload_chunk(&mut ws, UPLOAD, seq, &[1]);
        assert_eq!(
            decode(&recv(&mut ws)).unwrap(),
            Frame::StreamCredit {
                target: UPLOAD,
                correlation: 1,
                stream: 1,
                credit: 1,
            }
        );
    }
    for seq in 5..=8 {
        upload_chunk(&mut ws, UPLOAD, seq, &[1]);
    }
    send(&mut ws, Frame::Subscribe(SIGNAL));
    assert!(matches!(
        decode(&recv(&mut ws)).unwrap(),
        Frame::Answer {
            target: SIGNAL,
            outcome: AnswerOutcome::Accepted,
        }
    ));
    assert!(matches!(
        decode(&recv(&mut ws)).unwrap(),
        Frame::Publish {
            target: SIGNAL,
            state: PublishState::Init,
            ..
        }
    ));
    p.set_signal(&[42]);
    assert!(matches!(
        decode(&recv(&mut ws)).unwrap(),
        Frame::Publish {
            target: SIGNAL,
            state: PublishState::Live([42]),
            ..
        }
    ));
    close(ws, handle);
}
