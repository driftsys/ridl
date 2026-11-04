//! Listening gateway tests against the committed identity deployment.

#[path = "../examples/support/debug_gateway.rs"]
mod gateway;

use std::io;
use std::net::{SocketAddr, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use gateway::{Gateway, Options};
use ridl_loopback::Attachment;
use ridl_rt::contract::{CatalogRef, InstanceNo, InterfaceNo, Kind, Ordinal};
use ridl_rt::port::{EventSink, Handler, SignalWriter};
use ridl_rt::sample::{Envelope, Timestamp};
use ridl_transport_ws::frame::{
    self, AnswerOutcome, Attach, AttachOutcome, Frame, PublishState, ResponseOutcome, Target,
};
use ridl_transport_ws::{Config, Ended, Note};
use tungstenite::client::IntoClientRequest;
use tungstenite::{Message, WebSocket};

const DESCRIPTOR: &[u8] =
    include_bytes!("../../ridl-rt-conformance/fixtures/identity/identity.deployment.binfb");
const LIMIT: Duration = Duration::from_secs(3);
const SIGNAL: Target = Target {
    kind: Kind::Signal,
    iface: InterfaceNo(1),
    ord: Ordinal(1),
};
const EVENT: Target = Target {
    kind: Kind::Event,
    iface: InterfaceNo(1),
    ord: Ordinal(2),
};
const COMMAND: Target = Target {
    kind: Kind::Command,
    iface: InterfaceNo(1),
    ord: Ordinal(3),
};

fn gateway() -> Arc<Gateway> {
    Arc::new(Gateway::from_descriptor(DESCRIPTOR, "Local").unwrap())
}
fn catalog(g: &Gateway, name: &str) -> CatalogRef {
    *g.node().catalogs().find(|c| c.name == name).unwrap()
}
fn provider(g: &Gateway, number: i32, catalog: CatalogRef) -> Attachment {
    g.node()
        .attach(InstanceNo::new(number).unwrap(), &catalog)
        .unwrap()
}
fn options() -> Options {
    Options::parse(["descriptor", "Local", "0"].map(str::to_owned)).unwrap()
}
fn server(
    g: Arc<Gateway>,
    config: Config,
    count: usize,
) -> (SocketAddr, thread::JoinHandle<Vec<Ended>>) {
    let listener = options().bind().unwrap();
    let addr = listener.local_addr().unwrap();
    // Accept is bounded even when a client fails before connecting.
    listener.set_nonblocking(true).unwrap();
    let handle = thread::spawn(move || {
        let mut results = Vec::new();
        for _ in 0..count {
            let deadline = Instant::now() + LIMIT;
            let conn = loop {
                match listener.accept() {
                    Ok((conn, _)) => {
                        // macOS and the BSDs give the accepted socket the
                        // listener's non-blocking mode; the server reads it
                        // blocking.
                        conn.set_nonblocking(false).unwrap();
                        break conn;
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "accept deadline");
                        thread::yield_now();
                    }
                    Err(e) => panic!("accept: {e}"),
                }
            };
            results.push(g.serve(conn, &config));
        }
        results
    });
    (addr, handle)
}
fn send(ws: &mut WebSocket<TcpStream>, frame: Frame<'_>) {
    let mut bytes = vec![0; frame.encoded_len()];
    frame.encode(&mut bytes).unwrap();
    ws.send(Message::Binary(bytes.into())).unwrap();
}
fn read(ws: &mut WebSocket<TcpStream>) -> Vec<u8> {
    loop {
        match ws.read().unwrap() {
            Message::Binary(bytes) => return bytes.to_vec(),
            Message::Ping(_) => ws.flush().unwrap(),
            other => panic!("unexpected message: {other:?}"),
        }
    }
}
fn connect(addr: SocketAddr, catalog: CatalogRef) -> WebSocket<TcpStream> {
    let stream = TcpStream::connect_timeout(&addr, LIMIT).unwrap();
    stream.set_read_timeout(Some(LIMIT)).unwrap();
    stream.set_write_timeout(Some(LIMIT)).unwrap();
    let mut request = format!("ws://{addr}/").into_client_request().unwrap();
    request
        .headers_mut()
        .insert("Sec-WebSocket-Protocol", "ridl-ws-v2".parse().unwrap());
    let (mut ws, _) = tungstenite::client(request, stream).unwrap();
    // Node stores use FlatBuffers; encoding tag 1 is explicit.
    send(
        &mut ws,
        Frame::Attach(Attach::new(2, 1, catalog.hash, catalog.name).unwrap()),
    );
    assert_eq!(
        frame::decode(&read(&mut ws)).unwrap(),
        Frame::Attached(AttachOutcome::Accepted)
    );
    ws
}
fn subscribe(ws: &mut WebSocket<TcpStream>, target: Target) {
    send(ws, Frame::Subscribe(target));
    assert_eq!(
        frame::decode(&read(ws)).unwrap(),
        Frame::Answer {
            target,
            outcome: AnswerOutcome::Accepted
        }
    );
}
fn sample(ws: &mut WebSocket<TcpStream>, byte: u8) -> u64 {
    match frame::decode(&read(ws)).unwrap() {
        Frame::Publish {
            target: SIGNAL,
            state: PublishState::Live(bytes),
            envelope,
        } => {
            assert_eq!(bytes, [byte]);
            envelope.seq
        }
        other => panic!("expected signal: {other:?}"),
    }
}

#[test]
fn gateway_observes_every_interface_across_catalogs() {
    let g = gateway();
    // Independent inventory from fixtures/identity/*/face.ridl and system.rsdl:
    // Local offers alpha.Face and beta.Face and maps mapped.Face. Each has
    // interface 1, signal value at 1 and event changed at 2. remote is off-machine.
    assert_eq!(
        g.node().catalogs().map(|c| c.name).collect::<Vec<_>>(),
        ["alpha", "beta", "mapped"]
    );
    for (name, number, value) in [("alpha", 1, 11), ("beta", 3, 22), ("mapped", 1, 33)] {
        let cat = catalog(&g, name);
        let mut p = provider(&g, number, cat);
        p.set(SIGNAL.iface, SIGNAL.ord, &[value]).unwrap();
        p.commit();
        let (addr, handle) = server(g.clone(), Config::default(), 1);
        let mut ws = connect(addr, cat);
        subscribe(&mut ws, SIGNAL);
        assert_eq!(sample(&mut ws, value), 1);
        subscribe(&mut ws, EVENT);
        // The accepted answer is a deterministic subscription barrier.
        p.set(SIGNAL.iface, SIGNAL.ord, &[value + 1]).unwrap();
        p.commit();
        p.raise(EVENT.iface, EVENT.ord, &[value + 2], None).unwrap();
        let mut saw_signal = false;
        let mut saw_event = false;
        while !(saw_signal && saw_event) {
            match frame::decode(&read(&mut ws)).unwrap() {
                Frame::Publish {
                    target: SIGNAL,
                    state: PublishState::Live(bytes),
                    envelope,
                } => {
                    assert_eq!(bytes, [value + 1]);
                    assert_eq!(envelope.seq, 2);
                    saw_signal = true;
                }
                Frame::Occurrence {
                    target: EVENT,
                    payload,
                    ..
                } => {
                    assert_eq!(payload, [value + 2]);
                    saw_event = true;
                }
                other => panic!("unexpected update: {other:?}"),
            }
        }
        drop(ws);
        assert!(matches!(handle.join().unwrap()[0], Ended::Io(_)));
    }
}

#[test]
fn gateway_listener_sends_a_command() {
    let g = gateway();
    let cat = catalog(&g, "alpha");
    let mut p = provider(&g, 1, cat);
    p.serve(COMMAND.iface, &[COMMAND.ord]).unwrap();
    let (addr, handle) = server(g, Config::default(), 1);
    let mut ws = connect(addr, cat);
    let worker = thread::spawn(move || {
        let deadline = Instant::now() + LIMIT;
        let mut bytes = [0; 8];
        let claim = loop {
            if let Some(claim) = p.next_claim(&mut bytes).unwrap() {
                break claim;
            }
            assert!(Instant::now() < deadline, "command did not reach provider");
            thread::yield_now();
        };
        assert_eq!((claim.iface, claim.ord), (COMMAND.iface, COMMAND.ord));
        assert_eq!(&bytes[..claim.len], [42]);
        p.settle(claim.id, Ok(&[])).unwrap();
    });
    send(
        &mut ws,
        Frame::Request {
            target: COMMAND,
            envelope: Envelope {
                stamp: Timestamp(0),
                seq: 7,
            },
            payload: &[42],
        },
    );
    match frame::decode(&read(&mut ws)).unwrap() {
        Frame::Response {
            target: COMMAND,
            correlation: 7,
            outcome: ResponseOutcome::Accepted,
            ..
        } => {}
        other => panic!("expected command answer: {other:?}"),
    }
    worker.join().unwrap();
    drop(ws);
    handle.join().unwrap();
}

#[test]
fn gateway_rejects_client_publication() {
    let g = gateway();
    let cat = catalog(&g, "alpha");
    let notes = Arc::new(Mutex::new(Vec::new()));
    let sink = notes.clone();
    let config = Config::default().with_record(move |note| sink.lock().unwrap().push(*note));
    let (addr, handle) = server(g, config, 1);
    let mut ws = connect(addr, cat);
    send(
        &mut ws,
        Frame::Publish {
            target: SIGNAL,
            envelope: Envelope {
                stamp: Timestamp(0),
                seq: 1,
            },
            state: PublishState::Live(&[99]),
        },
    );
    match ws.read().unwrap() {
        Message::Close(Some(close)) => assert_eq!(u16::from(close.code), 1002),
        other => panic!("expected close: {other:?}"),
    }
    assert_eq!(handle.join().unwrap(), [Ended::ServerClosed(1002)]);
    assert!(notes
        .lock()
        .unwrap()
        .iter()
        .any(|n| matches!(n, Note::UnexpectedDirection)));
}

#[test]
fn gateway_disconnect_drops_listener_attachment() {
    let g = gateway();
    let cat = catalog(&g, "alpha");
    let mut p = provider(&g, 1, cat);
    p.set(SIGNAL.iface, SIGNAL.ord, &[11]).unwrap();
    p.commit();
    // A selected backend owns one Arc of its catalog's table. Its reference count
    // observes lifetime directly: retaining the backend after EOF retains it.
    let table = g.table(&cat).unwrap();
    assert_eq!(Arc::strong_count(table), 1);
    for value in [11, 22] {
        let (addr, handle) = server(g.clone(), Config::default(), 1);
        let mut ws = connect(addr, cat);
        subscribe(&mut ws, SIGNAL);
        assert_eq!(sample(&mut ws, value), 1);
        subscribe(&mut ws, EVENT);
        assert_eq!(Arc::strong_count(table), 2);
        drop(ws);
        handle.join().unwrap();
        assert_eq!(Arc::strong_count(table), 1, "backend retained after EOF");
        p.set(SIGNAL.iface, SIGNAL.ord, &[22]).unwrap();
        p.commit();
    }
}

#[test]
fn gateway_arguments_and_descriptor_validation() {
    for args in [
        vec![],
        vec!["file"],
        vec!["file", "Local"],
        vec!["file", "Local", "bad"],
        vec!["file", "Local", "65536"],
        vec!["file", "Local", "0", "extra"],
    ] {
        assert!(Options::parse(args.into_iter().map(str::to_owned)).is_err());
    }
    let mut options = options();
    assert_eq!(options.machine, "Local");
    assert_eq!(options.descriptor.to_str(), Some("descriptor"));
    options.descriptor = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ridl-rt-conformance/fixtures/identity/identity.deployment.binfb");
    let loaded = Gateway::load(&options).unwrap();
    assert_eq!(loaded.node().catalogs().count(), 3);
    assert!(Gateway::from_descriptor(DESCRIPTOR, "missing").is_err());
    assert!(Gateway::from_descriptor(&[], "Local").is_err());
}

#[test]
fn gateway_bind_policy() {
    let listener = options().bind().unwrap();
    let addr = listener.local_addr().unwrap();
    assert_eq!(addr.ip(), std::net::Ipv4Addr::LOCALHOST);
    assert_ne!(addr.port(), 0);
    let mut output = Vec::new();
    gateway::announce(&listener, &mut output).unwrap();
    assert_eq!(
        String::from_utf8(output).unwrap(),
        format!("port {}\n", addr.port())
    );
}

/// A finite connection for deterministic attach, routing and lifetime controls.
struct Memory {
    input: io::Cursor<Vec<u8>>,
    output: std::rc::Rc<std::cell::RefCell<Vec<u8>>>,
    on_idle: Option<Box<dyn FnOnce()>>,
    after_idle: Vec<u8>,
}
impl io::Read for Memory {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.input.position() == self.input.get_ref().len() as u64 {
            if let Some(step) = self.on_idle.take() {
                step();
                return Err(io::ErrorKind::TimedOut.into());
            }
            if !self.after_idle.is_empty() {
                self.input = io::Cursor::new(std::mem::take(&mut self.after_idle));
            }
        }
        io::Read::read(&mut self.input, out)
    }
}
impl io::Write for Memory {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.output.borrow_mut().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl ridl_transport_ws::Connection for Memory {
    fn set_read_timeout(&self, _: Option<Duration>) -> io::Result<()> {
        Ok(())
    }
    fn set_write_timeout(&self, _: Option<Duration>) -> io::Result<()> {
        Ok(())
    }
}
fn memory(cat: CatalogRef, messages: &[Frame<'_>]) -> Memory {
    let mut input = b"GET / HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Protocol: ridl-ws-v2\r\n\r\n".to_vec();
    for frame in std::iter::once(Frame::Attach(
        Attach::new(2, 1, cat.hash, cat.name).unwrap(),
    ))
    .chain(messages.iter().copied())
    {
        let mut bytes = vec![0; frame.encoded_len()];
        frame.encode(&mut bytes).unwrap();
        assert!(bytes.len() < 126);
        input.extend([0x82, 0x80 | bytes.len() as u8, 0, 0, 0, 0]);
        input.extend(bytes);
    }
    Memory {
        input: io::Cursor::new(input),
        output: Default::default(),
        on_idle: None,
        after_idle: Vec::new(),
    }
}
fn binary_frames(bytes: &[u8]) -> Vec<Frame<'_>> {
    assert!(bytes.starts_with(b"HTTP/1.1 101"));
    let head = bytes.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
    let mut rest = &bytes[head..];
    let mut frames = Vec::new();
    while !rest.is_empty() {
        let len = rest[1] as usize;
        assert!(len < 126);
        if rest[0] & 15 == 2 {
            frames.push(frame::decode(&rest[2..2 + len]).unwrap());
        }
        rest = &rest[2 + len..];
    }
    frames
}

#[test]
fn gateway_memory_routes_catalogs_and_releases_on_eof() {
    let g = gateway();
    for (name, number, byte) in [("alpha", 1, 11), ("beta", 3, 22), ("mapped", 1, 33)] {
        let cat = catalog(&g, name);
        let mut p = provider(&g, number, cat);
        p.set(SIGNAL.iface, SIGNAL.ord, &[byte]).unwrap();
        p.commit();
        let table = g.table(&cat).unwrap();
        for _ in 0..2 {
            let conn = memory(cat, &[Frame::Subscribe(SIGNAL), Frame::Subscribe(EVENT)]);
            let output = conn.output.clone();
            assert_eq!(
                g.serve(conn, &Config::default()),
                Ended::Io(io::ErrorKind::UnexpectedEof)
            );
            assert_eq!(Arc::strong_count(table), 1, "backend retained after EOF");
            assert_eq!(
                binary_frames(&output.borrow()),
                [
                    Frame::Attached(AttachOutcome::Accepted),
                    Frame::Answer {
                        target: SIGNAL,
                        outcome: AnswerOutcome::Accepted
                    },
                    Frame::Publish {
                        target: SIGNAL,
                        envelope: Envelope {
                            stamp: Timestamp(0),
                            seq: 1
                        },
                        state: PublishState::Live(&[byte])
                    },
                    Frame::Answer {
                        target: EVENT,
                        outcome: AnswerOutcome::Accepted
                    },
                ]
            );
        }
    }
}

#[test]
fn gateway_memory_refuses_publication_with_direction_note() {
    let g = gateway();
    let cat = catalog(&g, "alpha");
    let notes = Arc::new(Mutex::new(Vec::new()));
    let sink = notes.clone();
    let config = Config::default().with_record(move |note| sink.lock().unwrap().push(*note));
    let conn = memory(
        cat,
        &[Frame::Publish {
            target: SIGNAL,
            envelope: Envelope {
                stamp: Timestamp(0),
                seq: 1,
            },
            state: PublishState::Live(&[9]),
        }],
    );
    assert_eq!(g.serve(conn, &config), Ended::ServerClosed(1002));
    assert_eq!(*notes.lock().unwrap(), [Note::UnexpectedDirection]);
}

#[test]
fn gateway_memory_forwards_updates_events_and_command_answers() {
    let g = gateway();
    let mut config = Config::default();
    config.poll_interval = Duration::ZERO;
    for (name, number, byte) in [("alpha", 1, 11), ("beta", 3, 22), ("mapped", 1, 33)] {
        let cat = catalog(&g, name);
        let mut p = provider(&g, number, cat);
        p.set(SIGNAL.iface, SIGNAL.ord, &[byte]).unwrap();
        p.commit();
        p.serve(COMMAND.iface, &[COMMAND.ord]).unwrap();
        let mut conn = memory(
            cat,
            &[
                Frame::Subscribe(SIGNAL),
                Frame::Subscribe(EVENT),
                Frame::Request {
                    target: COMMAND,
                    envelope: Envelope {
                        stamp: Timestamp(0),
                        seq: 7,
                    },
                    payload: &[42],
                },
            ],
        );
        let output = conn.output.clone();
        // The next read is reached only after dispatch of every input frame.
        // One timeout gives the zero-interval poll a deterministic step.
        conn.on_idle = Some(Box::new(move || {
            let mut bytes = [0; 8];
            let claim = p
                .next_claim(&mut bytes)
                .unwrap()
                .expect("command reached provider");
            assert_eq!((claim.iface, claim.ord), (COMMAND.iface, COMMAND.ord));
            assert_eq!(&bytes[..claim.len], [42]);
            p.settle(claim.id, Ok(&[])).unwrap();
            p.set(SIGNAL.iface, SIGNAL.ord, &[byte + 1]).unwrap();
            p.commit();
            p.raise(EVENT.iface, EVENT.ord, &[byte + 2], None).unwrap();
        }));
        assert_eq!(
            g.serve(conn, &config),
            Ended::Io(io::ErrorKind::UnexpectedEof)
        );
        let output = output.borrow();
        let frames = binary_frames(&output);
        assert!(frames.iter().any(|f| matches!(f, Frame::Publish { target: SIGNAL, state: PublishState::Live(bytes), envelope } if *bytes == [byte] && envelope.seq == 1)));
        assert!(frames.iter().any(|f| matches!(f, Frame::Publish { target: SIGNAL, state: PublishState::Live(bytes), envelope } if *bytes == [byte + 1] && envelope.seq == 2)));
        assert!(frames.iter().any(|f| matches!(f, Frame::Occurrence { target: EVENT, payload, .. } if *payload == [byte + 2])));
        assert!(frames.iter().any(|f| matches!(
            f,
            Frame::Response {
                target: COMMAND,
                correlation: 7,
                outcome: ResponseOutcome::Accepted,
                ..
            }
        )));
    }
}

#[test]
fn gateway_memory_catalog_refusal_reasons() {
    use frame::Reason;
    use ridl_rt::contract::CatalogHash;
    let g = gateway();
    let alpha = catalog(&g, "alpha");
    for (cat, reason) in [
        (
            CatalogRef {
                name: "unknown",
                ..alpha
            },
            Reason::UnknownCatalog,
        ),
        (
            CatalogRef {
                name: "remote",
                ..alpha
            },
            Reason::UnknownCatalog,
        ),
        (
            CatalogRef {
                hash: CatalogHash([0; 32]),
                ..alpha
            },
            Reason::CatalogMismatch,
        ),
    ] {
        let conn = memory(cat, &[]);
        let output = conn.output.clone();
        assert_eq!(g.serve(conn, &Config::default()), Ended::ServerClosed(1000));
        assert_eq!(
            binary_frames(&output.borrow()),
            [Frame::Attached(AttachOutcome::Refused(reason))]
        );
    }
}

#[test]
fn gateway_memory_max_version_is_two() {
    let g = gateway();
    for (offer, accepted) in [("ridl-ws-v3, ridl-ws-v2", true), ("ridl-ws-v3", false)] {
        let mut conn = memory(catalog(&g, "alpha"), &[]);
        let bytes = conn.input.into_inner();
        let head = bytes.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
        let mut input = String::from_utf8(bytes[..head].to_vec())
            .unwrap()
            .replace(
                "Sec-WebSocket-Protocol: ridl-ws-v2",
                &format!("Sec-WebSocket-Protocol: {offer}"),
            )
            .into_bytes();
        input.extend_from_slice(&bytes[head..]);
        conn.input = io::Cursor::new(input);
        let output = conn.output.clone();
        let ended = g.serve(conn, &Config::default());
        let bytes = output.borrow();
        if accepted {
            assert_eq!(ended, Ended::Io(io::ErrorKind::UnexpectedEof));
            assert!(
                String::from_utf8_lossy(&bytes).contains("Sec-WebSocket-Protocol: ridl-ws-v2\r\n")
            );
        } else {
            assert_eq!(ended, Ended::Rejected);
            assert!(bytes.starts_with(b"HTTP/1.1 400"));
        }
    }
}

#[test]
fn gateway_memory_listener_does_not_reserve_instance_one() {
    let g = gateway();
    let cat = catalog(&g, "alpha");
    let mut conn = memory(cat, &[]);
    let node = g.clone();
    conn.on_idle = Some(Box::new(move || {
        let twin = node
            .node()
            .attach(InstanceNo::new(1).unwrap().twin().unwrap(), &cat);
        assert!(
            twin.is_ok(),
            "listener must not reserve instance 1 against its twin"
        );
    }));
    assert_eq!(
        g.serve(conn, &Config::default()),
        Ended::Io(io::ErrorKind::UnexpectedEof)
    );
}

#[test]
fn gateway_accept_errors_do_not_end_iteration() {
    // ECONNABORTED and EMFILE on macOS; both errors must leave later accepts reachable.
    let connections = [
        Err(io::Error::from_raw_os_error(53)),
        Ok(1),
        Err(io::Error::from_raw_os_error(24)),
        Ok(2),
    ];
    assert_eq!(
        gateway::accepted(connections.into_iter()).collect::<Vec<_>>(),
        [1, 2]
    );
}

#[test]
fn gateway_memory_selects_matching_hash_among_duplicate_names() {
    use ridl_descriptor::deployment::{self, Deployment};
    use ridl_rt::contract::CatalogHash;
    let mut descriptor = Deployment::try_from(deployment::verify(DESCRIPTOR).unwrap()).unwrap();
    let mut duplicate = descriptor
        .catalogs
        .iter()
        .find(|c| c.name == "alpha")
        .unwrap()
        .clone();
    let mut embedded =
        ridl_descriptor::Catalog::try_from(ridl_descriptor::verify(&duplicate.bytes).unwrap())
            .unwrap();
    embedded.hash = vec![255; 32];
    duplicate.hash = embedded.hash.clone();
    duplicate.bytes = ridl_descriptor::finish(&embedded);
    descriptor.catalogs.push(duplicate);
    let bytes = Box::leak(deployment::finish(&descriptor).into_boxed_slice());
    // Verification admits two hashes under one name; the node keeps both stores.
    deployment::verify(bytes).unwrap();
    let g = Gateway::from_descriptor(bytes, "Local").unwrap();
    let catalogs: Vec<_> = g
        .node()
        .catalogs()
        .filter(|c| c.name == "alpha")
        .copied()
        .collect();
    assert_eq!(catalogs.len(), 2);
    for (cat, value) in catalogs.into_iter().zip([11, 22]) {
        let mut p = provider(&g, 1, cat);
        p.set(SIGNAL.iface, SIGNAL.ord, &[value]).unwrap();
        p.commit();
        let conn = memory(cat, &[Frame::Subscribe(SIGNAL)]);
        let output = conn.output.clone();
        assert_eq!(
            g.serve(conn, &Config::default()),
            Ended::Io(io::ErrorKind::UnexpectedEof)
        );
        let bytes = output.borrow();
        let frames = binary_frames(&bytes);
        assert_eq!(frames[0], Frame::Attached(AttachOutcome::Accepted));
        assert!(frames.iter().any(|f| matches!(f, Frame::Publish { state: PublishState::Live(bytes), .. } if *bytes == [value])));
    }
    let conn = memory(
        CatalogRef {
            name: "alpha",
            hash: CatalogHash([0; 32]),
        },
        &[],
    );
    let output = conn.output.clone();
    assert_eq!(g.serve(conn, &Config::default()), Ended::ServerClosed(1000));
    assert_eq!(
        binary_frames(&output.borrow()),
        [Frame::Attached(AttachOutcome::Refused(
            frame::Reason::CatalogMismatch
        ))]
    );
}
fn injection(path: &str, messages: &[Frame<'_>]) -> Memory {
    let mut conn = memory(
        CatalogRef {
            name: "unused",
            hash: ridl_rt::contract::CatalogHash([0; 32]),
        },
        &[],
    );
    let mut input = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Protocol: ridl-ws-v2-provider\r\n\r\n").into_bytes();
    for frame in messages {
        let mut bytes = vec![0; frame.encoded_len()];
        frame.encode(&mut bytes).unwrap();
        assert!(bytes.len() < 126);
        input.extend([0x82, 0x80 | bytes.len() as u8, 0, 0, 0, 0]);
        input.extend(bytes);
    }
    conn.input = io::Cursor::new(input);
    conn
}
#[test]
fn gateway_injection_path_statuses() {
    let g = gateway();
    for path in [
        "/missing/Unit/alpha",
        "/alpha.First/missing/alpha",
        "/alpha.First/Unit/missing",
        "/alpha.Foreign/Unit/mapped",
        "/alpha.First/Unit/alpha/",
        "/alpha.First/Unit/alpha?x",
        "/alpha.First/Unit/%61lpha",
        "/alpha.First//alpha",
        "/alpha.First/Unit/alpha/extra",
    ] {
        let conn = injection(path, &[]);
        let output = conn.output.clone();
        assert_eq!(g.serve_injection(conn, &Config::default()), Ended::Rejected);
        assert!(output.borrow().starts_with(b"HTTP/1.1 404"), "{path}");
    }
    let _positive = provider(&g, 1, catalog(&g, "alpha"));
    let conn = injection("/alpha.First/Unit/alpha", &[]);
    let output = conn.output.clone();
    assert_eq!(g.serve_injection(conn, &Config::default()), Ended::Rejected);
    assert!(output.borrow().starts_with(b"HTTP/1.1 409"));
}
#[test]
fn gateway_path_selects_only_offered_members() {
    let g = gateway();
    for path in ["/alpha.First/Unit/mapped", "/alpha.Second/Unit/alpha"] {
        let conn = injection(path, &[]);
        let output = conn.output.clone();
        assert_eq!(g.serve_injection(conn, &Config::default()), Ended::Rejected);
        assert!(output.borrow().starts_with(b"HTTP/1.1 404"));
    }
    // Add a second, non-offered interface to the real alpha catalog. Its
    // member kinds and ordinals remain those of alpha.Face.
    let mut descriptor = ridl_descriptor::deployment::Deployment::try_from(
        ridl_descriptor::deployment::verify(DESCRIPTOR).unwrap(),
    )
    .unwrap();
    let embedded = descriptor
        .catalogs
        .iter_mut()
        .find(|c| c.name == "alpha")
        .unwrap();
    let mut cat =
        ridl_descriptor::Catalog::try_from(ridl_descriptor::verify(&embedded.bytes).unwrap())
            .unwrap();
    let mut extra = cat.interfaces[0].clone();
    extra.number = 2;
    extra.name = "alpha.NotOffered".into();
    cat.interfaces.push(extra);
    embedded.bytes = ridl_descriptor::finish(&cat);
    let bytes = Box::leak(ridl_descriptor::deployment::finish(&descriptor).into_boxed_slice());
    let g = Arc::new(Gateway::from_descriptor(bytes, "Local").unwrap());
    let mut conn = injection(
        "/alpha.First/Unit/alpha",
        &[
            Frame::Attached(AttachOutcome::Accepted),
            Frame::Answer {
                target: Target {
                    iface: InterfaceNo(2),
                    ..SIGNAL
                },
                outcome: AnswerOutcome::Accepted,
            },
            Frame::Publish {
                target: Target {
                    iface: InterfaceNo(2),
                    ..SIGNAL
                },
                envelope: Envelope {
                    stamp: Timestamp(1),
                    seq: 1,
                },
                state: PublishState::Live(&[99]),
            },
        ],
    );
    let output = conn.output.clone();
    let check = g.clone();
    conn.on_idle = Some(Box::new(move || {
        use ridl_rt::port::SignalReader;
        let listener = check
            .node()
            .attach(InstanceNo::LISTENER, &catalog(&check, "alpha"))
            .unwrap();
        let mut out = [0; 8];
        assert_eq!(
            listener
                .read(InterfaceNo(2), SIGNAL.ord, &mut out)
                .unwrap()
                .provenance,
            ridl_rt::sample::Provenance::Init
        );
    }));
    g.serve_injection(conn, &Config::default());
    let output = output.borrow();
    let frames = binary_frames(&output);
    assert_eq!(
        frames
            .iter()
            .filter(|f| matches!(f, Frame::Subscribe(_)))
            .count(),
        2
    );
    assert!(frames
        .iter()
        .all(|f| !matches!(f, Frame::Subscribe(t) if t.iface == InterfaceNo(2))));
}
#[test]
fn gateway_refused_handshake_releases_twin() {
    let g = gateway();
    let cat = catalog(&g, "alpha");
    for mode in ["origin", "protocol", "refused", "eof", "accepted"] {
        let messages = match mode {
            "refused" => vec![Frame::Attached(AttachOutcome::Refused(
                frame::Reason::UnknownCatalog,
            ))],
            "accepted" => vec![Frame::Attached(AttachOutcome::Accepted)],
            _ => vec![],
        };
        let mut conn = injection("/alpha.First/Unit/alpha", &messages);
        if mode == "protocol" {
            conn.input = io::Cursor::new(
                String::from_utf8(conn.input.into_inner())
                    .unwrap()
                    .replace("ridl-ws-v2-provider", "ridl-ws-v2")
                    .into_bytes(),
            );
        }
        if mode == "origin" {
            conn.input = io::Cursor::new(
                String::from_utf8(conn.input.into_inner())
                    .unwrap()
                    .replace(
                        "Host: localhost",
                        "Origin: https://untrusted.invalid\r\nHost: localhost",
                    )
                    .into_bytes(),
            );
        }
        if mode == "accepted" {
            let check = g.clone();
            conn.on_idle = Some(Box::new(move || {
                assert!(check
                    .node()
                    .attach(InstanceNo::new(1).unwrap(), &cat)
                    .is_err());
                assert!(check
                    .node()
                    .attach(InstanceNo::new(1).unwrap(), &catalog(&check, "mapped"))
                    .is_err());
            }));
        }
        let output = conn.output.clone();
        let ended = g.serve_injection(
            conn,
            &Config::default().with_origin_check(|origin| origin.is_none()),
        );
        match mode {
            "origin" => {
                assert_eq!(ended, Ended::Rejected);
                assert!(output.borrow().starts_with(b"HTTP/1.1 403"));
            }
            "protocol" => {
                assert_eq!(ended, Ended::Rejected);
                assert!(output.borrow().starts_with(b"HTTP/1.1 400"));
            }
            "refused" => assert_eq!(ended, Ended::ServerClosed(1000)),
            _ => assert_eq!(ended, Ended::Io(io::ErrorKind::UnexpectedEof)),
        }
        drop(provider(&g, 1, cat));
    }
}
#[test]
fn gateway_listening_port_refuses_provider_protocol() {
    use ridl_rt::port::SignalReader;
    let g = gateway();
    let conn = injection(
        "/alpha.First/Unit/alpha",
        &[
            Frame::Attached(AttachOutcome::Accepted),
            Frame::Publish {
                target: SIGNAL,
                envelope: Envelope {
                    stamp: Timestamp(1),
                    seq: 1,
                },
                state: PublishState::Live(&[99]),
            },
        ],
    );
    let output = conn.output.clone();
    assert_eq!(g.serve(conn, &Config::default()), Ended::Rejected);
    assert!(!output.borrow().starts_with(b"HTTP/1.1 101"));
    let p = provider(&g, 1, catalog(&g, "alpha"));
    assert_eq!(
        p.read(SIGNAL.iface, SIGNAL.ord, &mut [0; 8])
            .unwrap()
            .provenance,
        ridl_rt::sample::Provenance::Init
    );
}
#[test]
fn gateway_injection_is_absent_by_default() {
    let options = options();
    assert!(options.bind_injection().unwrap().is_none());
}
#[test]
fn gateway_injection_binds_loopback() {
    let options = Options::parse(["file", "Local", "0", "0"].map(str::to_owned)).unwrap();
    let listener = options.bind_injection().unwrap().unwrap();
    assert_eq!(
        listener.local_addr().unwrap().ip(),
        std::net::Ipv4Addr::LOCALHOST
    );
}

#[test]
fn gateway_memory_injection_reaches_local_ports() {
    use ridl_rt::port::{Caller, EventSource, SignalReader};
    use ridl_rt::sample::{Cause, Provenance};
    let g = gateway();
    let cat = catalog(&g, "alpha");
    let mut local = g.node().attach(InstanceNo::LISTENER, &cat).unwrap();
    local.subscribe(EVENT.iface, &[EVENT.ord]).unwrap();
    let command = local
        .command(COMMAND.iface, COMMAND.ord, &[11], None)
        .unwrap();
    let query = local.query(COMMAND.iface, Ordinal(4), &[13], None).unwrap();
    let local = Arc::new(Mutex::new(local));
    let mut conn = injection(
        "/alpha.First/Unit/alpha",
        &[
            Frame::Attached(AttachOutcome::Accepted),
            Frame::Answer {
                target: SIGNAL,
                outcome: AnswerOutcome::Accepted,
            },
            Frame::Answer {
                target: EVENT,
                outcome: AnswerOutcome::Accepted,
            },
            Frame::Publish {
                target: SIGNAL,
                envelope: Envelope {
                    stamp: Timestamp(1),
                    seq: 1,
                },
                state: PublishState::Live(&[42]),
            },
            Frame::Occurrence {
                target: EVENT,
                envelope: Envelope {
                    stamp: Timestamp(1),
                    seq: 1,
                },
                payload: &[7],
            },
        ],
    );
    // These frames arrive only after a tick has sent the two requests.
    let responses = injection(
        "/unused",
        &[
            Frame::Response {
                target: COMMAND,
                envelope: Envelope {
                    stamp: Timestamp(2),
                    seq: 1,
                },
                correlation: 1,
                outcome: ResponseOutcome::Accepted,
            },
            Frame::Response {
                target: Target {
                    kind: Kind::Query,
                    ord: Ordinal(4),
                    ..COMMAND
                },
                envelope: Envelope {
                    stamp: Timestamp(2),
                    seq: 2,
                },
                correlation: 2,
                outcome: ResponseOutcome::Reply(&[91]),
            },
            Frame::Publish {
                target: SIGNAL,
                envelope: Envelope {
                    stamp: Timestamp(2),
                    seq: 2,
                },
                state: PublishState::InvalidDeclared,
            },
        ],
    )
    .input
    .into_inner();
    let body = responses.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
    conn.after_idle = responses[body..].to_vec();
    let output = conn.output.clone();
    let observed = local.clone();
    conn.on_idle = Some(Box::new(move || {
        let mut local = observed.lock().unwrap();
        let mut bytes = [0; 8];
        let sample = local.read(SIGNAL.iface, SIGNAL.ord, &mut bytes).unwrap();
        assert_eq!(
            (sample.provenance, sample.len, bytes[0]),
            (Provenance::Live, 1, 42)
        );
        let event = local.next(&mut bytes).unwrap().unwrap();
        assert_eq!(
            (event.iface, event.ord, event.len, bytes[0]),
            (EVENT.iface, EVENT.ord, 1, 7)
        );
        let output = output.borrow();
        let frames = binary_frames(&output);
        for (kind, ord, seq, args) in [(Kind::Command, 3, 1, 11), (Kind::Query, 4, 2, 13)] {
            assert!(frames.iter().any(|f| matches!(f, Frame::Request { target, envelope, payload } if target.kind == kind && target.iface == InterfaceNo(1) && target.ord == Ordinal(ord) && envelope.seq == seq && *payload == [args])));
        }
    }));
    let mut config = Config::default();
    config.poll_interval = Duration::ZERO;
    assert_eq!(
        g.serve_injection(conn, &config),
        Ended::Io(io::ErrorKind::UnexpectedEof)
    );
    let mut local = local.lock().unwrap();
    let mut bytes = [0; 8];
    let sample = local.read(SIGNAL.iface, SIGNAL.ord, &mut bytes).unwrap();
    assert_eq!(
        (sample.provenance, sample.len, bytes[0]),
        (Provenance::Invalid(Cause::Declared), 1, 42)
    );
    assert_eq!(local.ack(command), Some(Ok(())));
    assert_eq!(local.reply(query, &mut bytes), Ok(Some(Ok(1))));
    assert_eq!(bytes[0], 91);
    drop(provider(&g, 1, cat));
}

#[test]
fn gateway_injection_refuses_streamed_query_locally() {
    use ridl_rt::error::{CallError, Contract};
    use ridl_rt::port::Caller;
    let mut descriptor = ridl_descriptor::deployment::Deployment::try_from(
        ridl_descriptor::deployment::verify(DESCRIPTOR).unwrap(),
    )
    .unwrap();
    let embedded = descriptor
        .catalogs
        .iter_mut()
        .find(|c| c.name == "alpha")
        .unwrap();
    let mut cat =
        ridl_descriptor::Catalog::try_from(ridl_descriptor::verify(&embedded.bytes).unwrap())
            .unwrap();
    let query = cat.interfaces[0]
        .members
        .iter_mut()
        .find(|m| m.ordinal == 4)
        .unwrap();
    query.streams = Some(vec![ridl_descriptor::StreamPayload {
        no: 0,
        element: Some("u8".into()),
        ..Default::default()
    }]);
    embedded.bytes = ridl_descriptor::finish(&cat);
    let bytes = Box::leak(ridl_descriptor::deployment::finish(&descriptor).into_boxed_slice());
    let g = Gateway::from_descriptor(bytes, "Local").unwrap();
    let cat = catalog(&g, "alpha");
    let mut local = g.node().attach(InstanceNo::LISTENER, &cat).unwrap();
    let call = local
        .query(InterfaceNo(1), Ordinal(4), &[13], None)
        .unwrap();
    let mut conn = injection(
        "/alpha.First/Unit/alpha",
        &[Frame::Attached(AttachOutcome::Accepted)],
    );
    let output = conn.output.clone();
    // Observe settlement during the session, before EOF could interrupt it.
    conn.on_idle = Some(Box::new(move || {
        assert_eq!(
            local.reply(call, &mut [0; 8]),
            Ok(Some(Err(CallError::Contract(Contract::UnknownInteraction))))
        );
    }));
    let mut config = Config::default();
    config.poll_interval = Duration::ZERO;
    assert_eq!(
        g.serve_injection(conn, &config),
        Ended::Io(io::ErrorKind::UnexpectedEof)
    );
    assert!(binary_frames(&output.borrow())
        .iter()
        .all(|f| !matches!(f, Frame::Request { .. })));
    drop(provider(&g, 1, cat));
}

/// Captures complete startup announcements without sharing stdout with tests.
struct Announcements {
    bytes: Vec<u8>,
    send: std::sync::mpsc::Sender<String>,
}
impl io::Write for Announcements {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.send
            .send(String::from_utf8(std::mem::take(&mut self.bytes)).unwrap())
            .map_err(|_| io::ErrorKind::BrokenPipe.into())
    }
}
fn executable_options(injection: bool) -> Options {
    let mut args = vec![
        format!(
            "{}/../ridl-rt-conformance/fixtures/identity/identity.deployment.binfb",
            env!("CARGO_MANIFEST_DIR")
        ),
        "Local".into(),
        "0".into(),
    ];
    if injection {
        args.push("0".into());
    }
    Options::parse(args).unwrap()
}

#[test]
fn gateway_executable_wiring_negotiates_both_roles() {
    let (announcements, receive) = std::sync::mpsc::channel();
    thread::spawn(move || {
        gateway::run(
            executable_options(true),
            &mut Announcements {
                bytes: vec![],
                send: announcements,
            },
        )
        .unwrap();
    });
    let listening = receive.recv_timeout(LIMIT).unwrap();
    let injection = receive.recv_timeout(LIMIT).unwrap();
    let port = |line: &str, prefix: &str| -> SocketAddr {
        format!("127.0.0.1:{}", line.trim().strip_prefix(prefix).unwrap())
            .parse()
            .unwrap()
    };
    let g = gateway();
    let mut listen = connect(port(&listening, "port "), catalog(&g, "alpha"));
    subscribe(&mut listen, SIGNAL);
    let addr = port(&injection, "inject-port ");
    let stream = TcpStream::connect_timeout(&addr, LIMIT).unwrap();
    stream.set_read_timeout(Some(LIMIT)).unwrap();
    let mut request = format!("ws://{addr}/alpha.First/Unit/alpha")
        .into_client_request()
        .unwrap();
    request.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        "ridl-ws-v2-provider".parse().unwrap(),
    );
    let (mut provider, _) = tungstenite::client(request, stream).unwrap();
    assert!(matches!(
        frame::decode(&read(&mut provider)).unwrap(),
        Frame::Attach(_)
    ));
    send(&mut provider, Frame::Attached(AttachOutcome::Accepted));
    assert!(matches!(
        frame::decode(&read(&mut provider)).unwrap(),
        Frame::Subscribe(_)
    ));
}

#[test]
fn gateway_executable_without_injection_announces_only_listening() {
    let (announcements, receive) = std::sync::mpsc::channel();
    thread::spawn(move || {
        gateway::run(
            executable_options(false),
            &mut Announcements {
                bytes: vec![],
                send: announcements,
            },
        )
        .unwrap();
    });
    let listening = receive.recv_timeout(LIMIT).unwrap();
    assert!(listening.starts_with("port "));
    assert!(receive.recv_timeout(Duration::from_millis(50)).is_err());
}

#[test]
fn gateway_failed_injection_bind_announces_nothing() {
    let busy = options().bind().unwrap();
    let options = Options::parse([
        format!(
            "{}/../ridl-rt-conformance/fixtures/identity/identity.deployment.binfb",
            env!("CARGO_MANIFEST_DIR")
        ),
        "Local".into(),
        "0".into(),
        busy.local_addr().unwrap().port().to_string(),
    ])
    .unwrap();
    let mut output = vec![];
    assert!(gateway::run(options, &mut output).is_err());
    assert!(output.is_empty());
}
