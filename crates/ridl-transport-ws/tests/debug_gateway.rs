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
}
impl io::Read for Memory {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.input.position() == self.input.get_ref().len() as u64 {
            if let Some(step) = self.on_idle.take() {
                step();
                return Err(io::ErrorKind::TimedOut.into());
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
