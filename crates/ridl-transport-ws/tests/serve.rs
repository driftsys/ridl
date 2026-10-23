//! `serve` over a TCP connection, with `tungstenite` as the client and
//! `ridl-loopback` as the backend.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use ridl_loopback::Loopback;
use ridl_rt::contract::{CatalogHash, CatalogRef, InterfaceNo, Kind, Ordinal};
use ridl_rt::error::CallError;
use ridl_rt::port::{
    Attached, Caller, Clock, Correlation, Encoded, EventSource, RawOccurrence, RawSample,
    ReadError, SendError, SignalReader, SignalWriter, SubscribeError,
};
use ridl_rt::sample::{Envelope, Timestamp};
use ridl_rt::trace::TraceContext;
use ridl_transport_ws::frame::{
    decode, AnswerOutcome, Attach, AttachOutcome, Frame, ProtocolViolation, PublishState, Target,
};
use ridl_transport_ws::{serve, Backend, Config, Connection, Ended, Note, Polling, Scanning};
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

/// An upgrade request this server accepts, for a client without
/// `tungstenite`.
const UPGRADE_REQUEST: &[u8] = b"GET / HTTP/1.1\r\n\
    Host: localhost\r\n\
    Upgrade: websocket\r\n\
    Connection: Upgrade\r\n\
    Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
    Sec-WebSocket-Version: 13\r\n\
    Sec-WebSocket-Protocol: ridl-ws-v1\r\n\
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
    let attach = Attach::new(1, 1, CATALOG.hash, CATALOG.name).unwrap();
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

    let mut ws = connect(client, "ridl-ws-v1").unwrap();
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
    // Without `ridl-ws-v1` offered, the server answers 400, and the client
    // reads that answer rather than a reset connection.
    match connect(client, "") {
        Err(tungstenite::Error::Http(response)) => assert_eq!(response.status(), 400),
        other => panic!("expected a 400 answer, got {:?}", other.map(|_| ())),
    }
    assert_eq!(handle.join().0, Ended::Rejected);
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
    config.liveness_timeout = Duration::from_millis(300);
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
    let mut ws = connect(client, "ridl-ws-v1").unwrap();
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

#[test]
fn the_server_pings_every_ping_interval() {
    let interval = Duration::from_millis(50);
    let mut config = config();
    config.ping_interval = interval;
    let (server, client) = pair();
    let handle = spawn(server, Scanning(Loopback::new(CATALOG)), config);
    let mut ws = connect(client, "ridl-ws-v1").unwrap();
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
    let mut ws = connect(client, "ridl-ws-v1").unwrap();
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
    config.liveness_timeout = Duration::from_millis(300);
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
    let mut ws = connect(client, "ridl-ws-v1").unwrap();
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
    let mut ws = connect(client, "ridl-ws-v1").unwrap();
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
    let mut ws = connect(client, "ridl-ws-v1").unwrap();
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
    config.liveness_timeout = Duration::from_millis(300);
    // A full send buffer blocks a write for at most the liveness timeout,
    // and the last message arrived at most one liveness timeout earlier.
    let bound = 2 * config.liveness_timeout + Duration::from_millis(500);
    let (server, client) = pair();
    let rt = Loopback::new(CATALOG);
    let mut provider = rt.attach();
    let handle = spawn(server, Scanning(rt), config);
    let mut ws = connect(client, "ridl-ws-v1").unwrap();
    attach(&mut ws);
    send(&mut ws, Frame::Subscribe(SIGNAL));

    // The client now reads nothing, while the provider commits large values
    // until the session ends.
    let (done, ended) = mpsc::channel();
    let committer = thread::spawn(move || {
        let mut value = vec![0u8; 60 * 1024];
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
    let mut ws = connect(client, "ridl-ws-v1").unwrap();
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
    let mut ws = connect(client, "ridl-ws-v1").unwrap();
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
    config.liveness_timeout = Duration::from_millis(300);
    // No ping, so that the client sends no pong: only data messages arrive.
    config.ping_interval = TEST_LIMIT;
    let (server, client) = pair();
    let handle = spawn(server, Scanning(Loopback::new(CATALOG)), config);
    let mut ws = connect(client, "ridl-ws-v1").unwrap();
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
    let mut ws = connect(client, "ridl-ws-v1").unwrap();
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
    let mut ws = connect(client, "ridl-ws-v1").unwrap();
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
    let attach = Attach::new(1, 1, CATALOG.hash, CATALOG.name).unwrap();
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
    config.liveness_timeout = Duration::from_millis(300);
    config.ping_interval = TEST_LIMIT;
    let (server, client) = pair();
    let handle = spawn(server, Scanning(Loopback::new(CATALOG)), config);
    let mut ws = connect(client, "ridl-ws-v1").unwrap();
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
    let mut ws = connect(client, "ridl-ws-v1").unwrap();
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
    let attach = Attach::new(1, 1, CATALOG.hash, CATALOG.name).unwrap();
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

/// Mutation: count frames instead of bytes (1044 frames never arrive within
/// the test limit).
#[test]
fn responses_past_the_retained_bytes_cap_close_1008_and_the_client_sees_it() {
    let (server, client) = pair();
    let config = config()
        .with_max_message_bytes(1024)
        .unwrap()
        .with_max_retained_bytes(29 * 36)
        .unwrap();
    let handle = spawn(server, Scanning(Loopback::new(CATALOG)), config);
    let mut ws = connect(client, "ridl-ws-v1").unwrap();
    attach(&mut ws);
    // Each `request` of a signal kind is answered at once with a 36-byte
    // `response`; 29 of them are 1044 bytes, the cap, and the 30th passes it.
    for seq in 1..=30u64 {
        send(
            &mut ws,
            Frame::Request {
                target: SIGNAL,
                envelope: Envelope {
                    stamp: Timestamp(0),
                    seq,
                },
                payload: &[],
            },
        );
        assert_eq!(recv(&mut ws).len(), 36, "response {seq}");
    }
    assert_eq!(recv_close(&mut ws), Some(CloseCode::Policy));
    assert_eq!(handle.join().0, Ended::ServerClosed(1008));
}
