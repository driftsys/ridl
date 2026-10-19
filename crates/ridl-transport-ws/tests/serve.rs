//! `serve` over a TCP connection, with `tungstenite` as the client and
//! `ridl-loopback` as the backend.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver};
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
use ridl_transport_ws::{serve, Backend, Config, Ended, Note, Polling, Scanning};
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
fn spawn<B: Backend + Send + 'static>(conn: TcpStream, backend: B, config: Config) -> Server {
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
    client
        .write_all(
            b"GET / HTTP/1.1\r\n\
              Host: localhost\r\n\
              Upgrade: websocket\r\n\
              Connection: Upgrade\r\n\
              Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
              Sec-WebSocket-Version: 13\r\n\
              Sec-WebSocket-Protocol: ridl-ws-v1\r\n\
              \r\n",
        )
        .unwrap();
    let handle = spawn(server, Scanning(Loopback::new(CATALOG)), config);
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        client.read_exact(&mut byte).unwrap();
        head.push(byte[0]);
    }
    assert!(head.starts_with(b"HTTP/1.1 101 "));

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
    let (backend, _) = watched(true);
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
    let (backend, log) = watched(false);
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

/// Ports that delegate to a loopback and log each `forget` and their drop.
/// With `detach_next`, `EventSource::next` answers `Detached`.
struct Watched {
    inner: Loopback,
    detach_next: bool,
    log: Log,
}

fn watched(detach_next: bool) -> (Polling<Watched>, Log) {
    let log = Log::default();
    let ports = Watched {
        inner: Loopback::new(CATALOG),
        detach_next,
        log: Arc::clone(&log),
    };
    (Polling(ports), log)
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
