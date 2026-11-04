//! Deterministic consuming-session selection over an in-memory connection.

use std::cell::Cell;
use std::io::{self, Cursor, Read, Write};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ridl_loopback::Loopback;
use ridl_rt::contract::{CatalogHash, CatalogRef, InterfaceNo, Kind, Ordinal};
use ridl_rt::port::SignalWriter;
use ridl_transport_ws::frame::{self, Attach, AttachOutcome, Frame, PublishState, Reason, Target};
use ridl_transport_ws::{
    serve, serve_selected, Backend, Config, Connection, Described, Ended, MemberTable, Note,
    Polling, Streaming,
};

const FIRST: CatalogRef = CatalogRef {
    name: "selected.first",
    hash: CatalogHash([1; 32]),
};
const SECOND: CatalogRef = CatalogRef {
    name: "selected.second",
    hash: CatalogHash([2; 32]),
};
const SIGNAL: Target = Target {
    kind: Kind::Signal,
    iface: InterfaceNo(1),
    ord: Ordinal(1),
};

struct Memory {
    input: Cursor<Vec<u8>>,
    output: Rc<std::cell::RefCell<Vec<u8>>>,
    reads: Rc<Cell<usize>>,
    boundary: Option<u64>,
}
impl Read for Memory {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.reads.set(self.reads.get() + 1);
        let len = self
            .boundary
            .filter(|end| *end > self.input.position())
            .map_or(buf.len(), |end| {
                buf.len().min((end - self.input.position()) as usize)
            });
        self.input.read(&mut buf[..len])
    }
}
impl Write for Memory {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.output.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Connection for Memory {
    fn set_read_timeout(&self, _: Option<Duration>) -> io::Result<()> {
        Ok(())
    }
    fn set_write_timeout(&self, _: Option<Duration>) -> io::Result<()> {
        Ok(())
    }
}
fn wire(frame: Frame<'_>) -> Vec<u8> {
    let mut msg = vec![0; frame.encoded_len()];
    frame.encode(&mut msg).unwrap();
    assert!(msg.len() < 126);
    let mut out = vec![0x82, 0x80 | msg.len() as u8, 0, 0, 0, 0];
    out.extend(msg);
    out
}
fn connection(protocol: &str, messages: Vec<Vec<u8>>) -> Memory {
    let mut input = format!("GET / HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Protocol: {protocol}\r\n\r\n").into_bytes();
    input.extend(messages.into_iter().flatten());
    Memory {
        input: Cursor::new(input),
        output: Rc::default(),
        reads: Rc::default(),
        boundary: None,
    }
}
fn attach(catalog: CatalogRef, version: u32, encoding: u8) -> Vec<u8> {
    wire(Frame::Attach(
        Attach::new(version, encoding, catalog.hash, catalog.name).unwrap(),
    ))
}
fn frames(output: &[u8]) -> Vec<(u8, Vec<u8>)> {
    let head = output.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
    assert!(output.starts_with(b"HTTP/1.1 101"));
    let mut rest = &output[head..];
    let mut frames = Vec::new();
    while !rest.is_empty() {
        let len = rest[1] as usize;
        assert!(len < 126);
        frames.push((rest[0] & 15, rest[2..2 + len].to_vec()));
        rest = &rest[2 + len..];
    }
    frames
}
fn refused(output: &[u8], reason: Reason) {
    let messages = frames(output);
    assert_eq!(messages.len(), 2);
    assert_eq!(
        frame::decode(&messages[0].1).unwrap(),
        Frame::Attached(AttachOutcome::Refused(reason))
    );
    assert_eq!(messages[1], (8, 1000u16.to_be_bytes().to_vec()));
}
fn store(catalog: CatalogRef, value: u8) -> Loopback {
    let mut rt = Loopback::new(catalog);
    rt.set(SIGNAL.iface, SIGNAL.ord, &[value]).unwrap();
    rt.commit();
    rt
}

#[test]
fn selected_routes_two_catalogs() {
    let first = store(FIRST, 11);
    let second = store(SECOND, 22);
    for (catalog, value) in [(FIRST, 11), (SECOND, 22)] {
        let attach = attach(catalog, 2, 1);
        let mut conn = connection(
            "ridl-ws-v2",
            vec![attach.clone(), wire(Frame::Subscribe(SIGNAL))],
        );
        let head = conn
            .input
            .get_ref()
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .unwrap()
            + 4;
        conn.boundary = Some((head + attach.len()) as u64);
        let output = conn.output.clone();
        let count = Cell::new(0);
        assert_eq!(
            serve_selected(
                conn,
                3,
                |request| {
                    count.set(count.get() + 1);
                    assert_eq!(request.hash, catalog.hash);
                    Some(Polling(if request.name == FIRST.name {
                        first.attach()
                    } else {
                        second.attach()
                    }))
                },
                &Config::default()
            ),
            Ended::Io(io::ErrorKind::UnexpectedEof)
        );
        assert_eq!(count.get(), 1);
        let messages = frames(&output.borrow());
        assert!(messages.iter().any(|(_, msg)| matches!(frame::decode(msg), Ok(Frame::Publish { target: SIGNAL, state: PublishState::Live(bytes), .. }) if bytes == [value])));
    }
}

#[test]
fn unknown_catalog_calls_selector_once_and_refuses() {
    let conn = connection("ridl-ws-v2", vec![attach(FIRST, 2, 1)]);
    let output = conn.output.clone();
    let count = Cell::new(0);
    let ended = serve_selected::<_, Polling<Loopback>, _>(
        conn,
        2,
        |request| {
            count.set(count.get() + 1);
            assert_eq!((request.name, request.hash), (FIRST.name, FIRST.hash));
            None
        },
        &Config::default(),
    );
    assert_eq!(ended, Ended::ServerClosed(1000));
    assert_eq!(count.get(), 1);
    refused(&output.borrow(), Reason::UnknownCatalog);
}

#[test]
fn unknown_catalog_on_version_one_refuses_version() {
    let conn = connection("ridl-ws-v1", vec![attach(FIRST, 1, 1)]);
    let output = conn.output.clone();
    assert_eq!(
        serve_selected::<_, Polling<Loopback>, _>(conn, 2, |_| None, &Config::default()),
        Ended::ServerClosed(1000)
    );
    refused(&output.borrow(), Reason::FrameVersionUnsupported);
}

#[test]
fn ordinary_attach_refusal_precedes_configuration_refusal() {
    let wrong = CatalogRef {
        hash: SECOND.hash,
        ..FIRST
    };
    let conn = connection("ridl-ws-v3", vec![attach(wrong, 3, 1)]);
    let output = conn.output.clone();
    assert_eq!(
        serve_selected(
            conn,
            3,
            |_| Some(Polling(store(FIRST, 11))),
            &Config::default()
        ),
        Ended::ServerClosed(1000)
    );
    refused(&output.borrow(), Reason::CatalogMismatch);
}

#[test]
fn selection_preserves_attach_checks() {
    let rt = store(FIRST, 11);
    for (catalog, version, encoding, protocol, reason) in [
        (SECOND, 2, 1, "ridl-ws-v2", Reason::UnknownCatalog),
        (
            CatalogRef {
                hash: SECOND.hash,
                ..FIRST
            },
            2,
            1,
            "ridl-ws-v2",
            Reason::CatalogMismatch,
        ),
        (FIRST, 2, 2, "ridl-ws-v2", Reason::EncodingUnsupported),
        (FIRST, 3, 1, "ridl-ws-v2", Reason::FrameVersionUnsupported),
        (FIRST, 1, 1, "ridl-ws-v1", Reason::FrameVersionUnsupported),
    ] {
        let conn = connection(protocol, vec![attach(catalog, version, encoding)]);
        let output = conn.output.clone();
        assert_eq!(
            serve_selected(conn, 2, |_| Some(Polling(rt.attach())), &Config::default()),
            Ended::ServerClosed(1000)
        );
        refused(&output.borrow(), reason);
    }
}

struct Tracked {
    rt: Loopback,
    table: Option<MemberTable>,
    drops: Rc<Cell<usize>>,
}
impl Drop for Tracked {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
impl Backend for Tracked {
    type Ports = Loopback;
    fn ports(&mut self) -> &mut Loopback {
        &mut self.rt
    }
    fn members(&self) -> Option<&MemberTable> {
        self.table.as_ref()
    }
}
fn table() -> MemberTable {
    let mut table = MemberTable::new(FIRST.name, FIRST.hash);
    table.insert(SIGNAL.iface, SIGNAL.ord, SIGNAL.kind, None);
    table
}
#[test]
fn selected_capability_mismatch_is_config_refused() {
    // Isolate the missing table and the missing stream capability.
    for has_table in [false, true] {
        let conn = connection(
            "ridl-ws-v3",
            vec![attach(FIRST, 3, 1), wire(Frame::Subscribe(SIGNAL))],
        );
        let output = conn.output.clone();
        let drops = Rc::new(Cell::new(0));
        let backend = Tracked {
            rt: store(FIRST, 11),
            table: has_table.then(table),
            drops: drops.clone(),
        };
        let ended = if has_table {
            serve_selected(conn, 3, |_| Some(backend), &Config::default())
        } else {
            serve_selected(conn, 3, |_| Some(Streaming(backend)), &Config::default())
        };
        assert_eq!(ended, Ended::ConfigRefused);
        assert_eq!(drops.get(), 1);
        refused(&output.borrow(), Reason::FrameVersionUnsupported);
    }
}
#[test]
fn selected_limit_failure_releases_backend() {
    let conn = connection(
        "ridl-ws-v2",
        vec![attach(FIRST, 2, 1), wire(Frame::Subscribe(SIGNAL))],
    );
    let output = conn.output.clone();
    let drops = Rc::new(Cell::new(0));
    let notes = Arc::new(Mutex::new(Vec::new()));
    let sink = notes.clone();
    let config = Config::default()
        .with_max_message_bytes(294)
        .unwrap()
        .with_record(move |note| sink.lock().unwrap().push(*note));
    let ended = serve_selected(
        conn,
        2,
        |_| {
            Some(Tracked {
                rt: store(FIRST, 11),
                table: Some(table()),
                drops: drops.clone(),
            })
        },
        &config,
    );
    assert_eq!(ended, Ended::ConfigRefused);
    assert_eq!(drops.get(), 1);
    refused(&output.borrow(), Reason::Range);
    assert_eq!(
        *notes.lock().unwrap(),
        vec![Note::MessageLimitTooSmall {
            target: SIGNAL,
            needed: 36 + ridl_rt::contract::SIGNAL_EVENT_CEILING,
        }]
    );
}
#[test]
fn serve_preflight_still_reads_nothing() {
    let conn = connection("ridl-ws-v2", vec![]);
    let reads = conn.reads.clone();
    let rt = store(FIRST, 11);
    let backend = Described::new(Polling(rt), Arc::new(table())).unwrap();
    assert_eq!(
        serve(
            conn,
            backend,
            &Config::default().with_max_message_bytes(294).unwrap()
        ),
        Ended::ConfigRefused
    );
    assert_eq!(reads.get(), 0);
}
#[test]
fn selected_preserves_bytes_after_attach() {
    let rt = store(FIRST, 33);
    let conn = connection(
        "ridl-ws-v2",
        vec![attach(FIRST, 2, 1), wire(Frame::Subscribe(SIGNAL))],
    );
    let output = conn.output.clone();
    assert_eq!(
        serve_selected(conn, 2, |_| Some(Polling(rt.attach())), &Config::default()),
        Ended::Io(io::ErrorKind::UnexpectedEof)
    );
    let messages = frames(&output.borrow());
    assert_eq!(messages.len(), 3); // attached, answer, initial publication
    assert!(matches!(
        frame::decode(&messages[2].1),
        Ok(Frame::Publish {
            target: SIGNAL,
            state: PublishState::Live([33]),
            ..
        })
    ));
}
#[test]
fn selected_invalid_capability_reads_nothing() {
    for version in [0, 1, 4, u32::MAX] {
        let conn = connection("ridl-ws-v2", vec![]);
        let reads = conn.reads.clone();
        assert_eq!(
            serve_selected::<_, Polling<Loopback>, _>(
                conn,
                version,
                |_| panic!("must not select"),
                &Config::default()
            ),
            Ended::ConfigRefused
        );
        assert_eq!(reads.get(), 0);
    }
}

#[test]
fn selected_accepts_capable_version_three_backend() {
    let conn = connection(
        "ridl-ws-v3",
        vec![attach(FIRST, 3, 1), wire(Frame::Subscribe(SIGNAL))],
    );
    let output = conn.output.clone();
    let backend = Streaming(Described::new(Polling(store(FIRST, 44)), Arc::new(table())).unwrap());
    assert_eq!(
        serve_selected(conn, 3, |_| Some(backend), &Config::default()),
        Ended::Io(io::ErrorKind::UnexpectedEof)
    );
    let bytes = output.borrow();
    assert!(String::from_utf8_lossy(&bytes).contains("Sec-WebSocket-Protocol: ridl-ws-v3"));
    let messages = frames(&bytes);
    assert_eq!(
        frame::decode(&messages[0].1).unwrap(),
        Frame::Attached(AttachOutcome::Accepted)
    );
    assert!(matches!(
        frame::decode(&messages[2].1),
        Ok(Frame::Publish {
            state: PublishState::Live([44]),
            ..
        })
    ));
}
#[test]
fn selected_origin_refusal_precedes_selection() {
    let conn = connection("ridl-ws-v2", vec![attach(FIRST, 2, 1)]);
    let output = conn.output.clone();
    let config = Config::default().with_origin_check(|_| false);
    assert_eq!(
        serve_selected::<_, Polling<Loopback>, _>(conn, 2, |_| panic!("must not select"), &config),
        Ended::Rejected
    );
    assert!(output.borrow().starts_with(b"HTTP/1.1 403"));
}
#[test]
fn selected_requires_attach_before_selection() {
    for message in [
        wire(Frame::Subscribe(SIGNAL)),
        vec![0x82, 0x81, 0, 0, 0, 0, 0xff],
    ] {
        let conn = connection("ridl-ws-v2", vec![message]);
        let output = conn.output.clone();
        assert_eq!(
            serve_selected::<_, Polling<Loopback>, _>(
                conn,
                2,
                |_| panic!("must not select"),
                &Config::default()
            ),
            Ended::ServerClosed(1002)
        );
        assert_eq!(
            frames(&output.borrow()),
            vec![(8, 1002u16.to_be_bytes().to_vec())]
        );
    }
}

#[test]
fn selected_backend_released_on_session_exits() {
    for (opening, tail, expected) in [
        (
            attach(FIRST, 2, 1),
            vec![],
            Ended::Io(io::ErrorKind::UnexpectedEof),
        ),
        (
            attach(FIRST, 2, 1),
            wire(Frame::Subscribe(SIGNAL)),
            Ended::Io(io::ErrorKind::UnexpectedEof),
        ),
        (
            attach(FIRST, 2, 1),
            attach(FIRST, 2, 1),
            Ended::ServerClosed(1002),
        ),
        (
            attach(FIRST, 2, 1),
            vec![0x88, 0x82, 0, 0, 0, 0, 3, 232],
            Ended::ClientClosed(Some(1000)),
        ),
        (attach(FIRST, 2, 2), vec![], Ended::ServerClosed(1000)),
    ] {
        let conn = connection("ridl-ws-v2", vec![opening, tail]);
        let drops = Rc::new(Cell::new(0));
        let count = Cell::new(0);
        let ended = serve_selected(
            conn,
            2,
            |_| {
                count.set(count.get() + 1);
                Some(Tracked {
                    rt: store(FIRST, 11),
                    table: Some(table()),
                    drops: drops.clone(),
                })
            },
            &Config::default(),
        );
        assert_eq!(ended, expected);
        assert_eq!(count.get(), 1);
        assert_eq!(drops.get(), 1);
    }
}

#[test]
fn none_selection_with_wrong_attach_version_refuses_version() {
    for version in [1, 3] {
        let conn = connection("ridl-ws-v2", vec![attach(FIRST, version, 1)]);
        let output = conn.output.clone();
        assert_eq!(
            serve_selected::<_, Polling<Loopback>, _>(conn, 2, |_| None, &Config::default()),
            Ended::ServerClosed(1000)
        );
        refused(&output.borrow(), Reason::FrameVersionUnsupported);
    }
}

#[test]
fn selected_max_version_controls_handshake() {
    for (offer, accepted) in [("ridl-ws-v3, ridl-ws-v2", true), ("ridl-ws-v3", false)] {
        let conn = connection(offer, vec![attach(FIRST, 2, 1)]);
        let output = conn.output.clone();
        let ended = serve_selected(
            conn,
            2,
            |_| Some(Polling(store(FIRST, 11))),
            &Config::default(),
        );
        let bytes = output.borrow();
        if accepted {
            assert_eq!(ended, Ended::Io(io::ErrorKind::UnexpectedEof));
            assert!(
                String::from_utf8_lossy(&bytes).contains("Sec-WebSocket-Protocol: ridl-ws-v2\r\n")
            );
            assert_eq!(
                frame::decode(&frames(&bytes)[0].1).unwrap(),
                Frame::Attached(AttachOutcome::Accepted)
            );
        } else {
            assert_eq!(ended, Ended::Rejected);
            assert!(bytes.starts_with(b"HTTP/1.1 400"));
        }
    }
}

#[test]
fn selected_capability_refusal_wins_over_limit_but_records_limit() {
    let conn = connection("ridl-ws-v3", vec![attach(FIRST, 3, 1)]);
    let output = conn.output.clone();
    let notes = Arc::new(Mutex::new(Vec::new()));
    let sink = notes.clone();
    let config = Config::default()
        .with_max_message_bytes(294)
        .unwrap()
        .with_record(move |note| sink.lock().unwrap().push(*note));
    let backend = Described::new(Polling(store(FIRST, 11)), Arc::new(table())).unwrap();
    assert_eq!(
        serve_selected(conn, 3, |_| Some(backend), &config),
        Ended::ConfigRefused
    );
    refused(&output.borrow(), Reason::FrameVersionUnsupported);
    assert_eq!(
        *notes.lock().unwrap(),
        [Note::MessageLimitTooSmall {
            target: SIGNAL,
            needed: 3876
        }]
    );
}

#[test]
fn selected_first_message_records_exact_note() {
    use frame::ProtocolViolation;
    let stream = Frame::StreamCredit {
        target: Target {
            kind: Kind::Command,
            ..SIGNAL
        },
        correlation: 1,
        stream: 0,
        credit: 1,
    };
    for (message, note) in [
        (wire(Frame::Subscribe(SIGNAL)), Note::MessageBeforeAttach),
        (
            vec![0x82, 0x80, 0, 0, 0, 0],
            Note::Violation(ProtocolViolation::TooShort),
        ),
        (
            vec![0x82, 0x81, 0, 0, 0, 0, 0xff],
            Note::Violation(ProtocolViolation::UnknownCode),
        ),
        (
            wire(stream),
            Note::Violation(ProtocolViolation::UnknownCode),
        ),
    ] {
        let conn = connection("ridl-ws-v2", vec![message]);
        let notes = Arc::new(Mutex::new(Vec::new()));
        let sink = notes.clone();
        let config = Config::default().with_record(move |note| sink.lock().unwrap().push(*note));
        assert_eq!(
            serve_selected::<_, Polling<Loopback>, _>(
                conn,
                2,
                |_| panic!("must not select"),
                &config
            ),
            Ended::ServerClosed(1002)
        );
        assert_eq!(*notes.lock().unwrap(), [note]);
    }
}

#[test]
fn ordinary_refusal_precedes_short_limit_and_records_no_limit_note() {
    let wrong = CatalogRef {
        hash: SECOND.hash,
        ..FIRST
    };
    let conn = connection("ridl-ws-v2", vec![attach(wrong, 2, 1)]);
    let output = conn.output.clone();
    let notes = Arc::new(Mutex::new(Vec::new()));
    let sink = notes.clone();
    let config = Config::default()
        .with_max_message_bytes(294)
        .unwrap()
        .with_record(move |note| sink.lock().unwrap().push(*note));
    let backend = Described::new(Polling(store(FIRST, 11)), Arc::new(table())).unwrap();
    assert_eq!(
        serve_selected(conn, 2, |_| Some(backend), &config),
        Ended::ServerClosed(1000)
    );
    refused(&output.borrow(), Reason::CatalogMismatch);
    assert!(
        notes.lock().unwrap().is_empty(),
        "ordinary refusal must not record a limit failure"
    );
}

#[test]
fn selected_v3_stream_before_attach_records_message_before_attach() {
    let message = wire(Frame::StreamCredit {
        target: Target {
            kind: Kind::Command,
            ..SIGNAL
        },
        correlation: 1,
        stream: 0,
        credit: 1,
    });
    assert_first_note("ridl-ws-v3", message, Note::MessageBeforeAttach);
}

#[test]
fn selected_discard_before_attach_records_message_before_attach() {
    use ridl_rt::sample::{Envelope, Timestamp};
    let frame = Frame::Publish {
        target: SIGNAL,
        envelope: Envelope {
            stamp: Timestamp(0),
            seq: 1,
        },
        state: PublishState::Init,
    };
    let mut bytes = vec![0; frame.encoded_len()];
    frame.encode(&mut bytes).unwrap();
    // An Init publication may not carry a payload.
    bytes.push(99);
    assert_eq!(
        frame::decode(&bytes),
        Err(frame::DecodeError::Discard(
            frame::Discarded::UnexpectedPayload
        ))
    );
    let mut message = vec![0x82, 0x80 | bytes.len() as u8, 0, 0, 0, 0];
    message.extend(bytes);
    assert_first_note("ridl-ws-v2", message, Note::MessageBeforeAttach);
}

fn assert_first_note(protocol: &str, message: Vec<u8>, note: Note) {
    let conn = connection(protocol, vec![message]);
    let output = conn.output.clone();
    let notes = Arc::new(Mutex::new(Vec::new()));
    let sink = notes.clone();
    let config = Config::default().with_record(move |note| sink.lock().unwrap().push(*note));
    assert_eq!(
        serve_selected::<_, Polling<Loopback>, _>(conn, 3, |_| panic!("must not select"), &config),
        Ended::ServerClosed(1002)
    );
    assert_eq!(
        frames(&output.borrow()),
        [(8, 1002u16.to_be_bytes().to_vec())]
    );
    assert_eq!(*notes.lock().unwrap(), [note]);
}
