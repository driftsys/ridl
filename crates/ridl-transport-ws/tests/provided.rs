//! Reversed-role negotiation over deterministic, masked in-memory frames.

use std::cell::{Cell, RefCell};
use std::io::{self, Cursor, Read, Write};
use std::rc::Rc;
use std::time::Duration;

use ridl_loopback::Loopback;
use ridl_rt::contract::{CatalogHash, CatalogRef, InterfaceNo, Kind, Ordinal};
use ridl_transport_ws::frame::{self, Attach, AttachOutcome, Frame, Reason};
use ridl_transport_ws::{
    serve, serve_provided, serve_provided_selected, serve_selected, Config, Connection, Ended,
    MemberTable, Polling, ProvidedBackend, Providing, SelectionError,
};

const CATALOG: CatalogRef = CatalogRef {
    name: "provided.test",
    hash: CatalogHash([7; 32]),
};
struct Memory {
    input: Cursor<Vec<u8>>,
    output: Rc<RefCell<Vec<u8>>>,
    head: u64,
    require_attach: bool,
    fail_write: Option<usize>,
    writes: usize,
}
impl Read for Memory {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.require_attach && self.input.position() >= self.head {
            // A peer that does not answer until it has received server attach.
            assert_eq!(messages(&self.output.borrow())[0].0, 2);
            assert!(matches!(
                frame::decode(&messages(&self.output.borrow())[0].1),
                Ok(Frame::Attach(_))
            ));
        }
        self.input.read(out)
    }
}
impl Write for Memory {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        if self.fail_write == Some(self.writes) {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        self.output.borrow_mut().extend_from_slice(bytes);
        Ok(bytes.len())
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
fn connection(protocol: &str, path: &str, tail: &[u8]) -> Memory {
    let mut bytes = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Protocol: {protocol}\r\n\r\n").into_bytes();
    let head = bytes.len() as u64;
    bytes.extend_from_slice(tail);
    Memory {
        input: Cursor::new(bytes),
        output: Rc::default(),
        head,
        require_attach: false,
        fail_write: None,
        writes: 0,
    }
}
fn wire(frame: Frame<'_>) -> Vec<u8> {
    let mut bytes = vec![0; frame.encoded_len()];
    frame.encode(&mut bytes).unwrap();
    masked(2, &bytes)
}
fn masked(op: u8, bytes: &[u8]) -> Vec<u8> {
    assert!(bytes.len() < 126);
    let mask = [1, 2, 3, 4];
    let mut out = vec![0x80 | op, 0x80 | bytes.len() as u8];
    out.extend(mask);
    out.extend(bytes.iter().enumerate().map(|(i, byte)| byte ^ mask[i % 4]));
    out
}
fn messages(output: &[u8]) -> Vec<(u8, Vec<u8>)> {
    let head = output.windows(4).position(|b| b == b"\r\n\r\n").unwrap() + 4;
    let mut rest = &output[head..];
    let mut out = Vec::new();
    while !rest.is_empty() {
        assert_eq!(rest[1] & 0x80, 0, "server frames must be unmasked");
        let len = rest[1] as usize;
        assert!(len < 126);
        out.push((rest[0] & 15, rest[2..2 + len].to_vec()));
        rest = &rest[2 + len..];
    }
    out
}
fn table() -> MemberTable {
    let mut table = MemberTable::new(CATALOG.name, CATALOG.hash);
    table.insert(InterfaceNo(1), Ordinal(2), Kind::Signal, None);
    assert!(table.set_limit(InterfaceNo(1), Ordinal(2), 300));
    table
}
fn backend() -> Providing<Loopback> {
    Providing::new(Loopback::new(CATALOG), table()).unwrap()
}
struct Tracked {
    inner: Providing<Loopback>,
    drops: Rc<Cell<usize>>,
}
impl Drop for Tracked {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
impl ProvidedBackend for Tracked {
    type Ports = Loopback;
    fn ports(&mut self) -> &mut Loopback {
        self.inner.ports()
    }
    fn members(&self) -> &MemberTable {
        self.inner.members()
    }
}
#[test]
fn listeners_accept_only_their_role() {
    for selected in [false, true] {
        let conn = connection(frame::SUBPROTOCOL_PROVIDER, "/", &[]);
        let output = conn.output.clone();
        let ended = if selected {
            serve_selected(
                conn,
                2,
                |_| Some(Polling(Loopback::new(CATALOG))),
                &Config::default(),
            )
        } else {
            serve(conn, Polling(Loopback::new(CATALOG)), &Config::default())
        };
        assert_eq!(ended, Ended::Rejected);
        assert!(output.borrow().starts_with(b"HTTP/1.1 400"));
        let conn = connection("ridl-ws-v3, ridl-ws-v2, ridl-ws-v1", "/", &[]);
        let output = conn.output.clone();
        let ended = if selected {
            serve_provided_selected::<_, Providing<Loopback>, _>(
                conn,
                |_| panic!("must not select"),
                &Config::default(),
            )
        } else {
            serve_provided(conn, backend(), &Config::default())
        };
        assert_eq!(ended, Ended::Rejected);
        assert!(output.borrow().starts_with(b"HTTP/1.1 400"));
    }
}
#[test]
fn mixed_offer_selects_consuming_role() {
    for selected in [false, true] {
        let tail = wire(Frame::Attach(
            Attach::new(2, 1, CATALOG.hash, CATALOG.name).unwrap(),
        ));
        let conn = connection("ridl-ws-v2-provider, ridl-ws-v2", "/", &tail);
        let output = conn.output.clone();
        if selected {
            serve_selected(
                conn,
                2,
                |_| Some(Polling(Loopback::new(CATALOG))),
                &Config::default(),
            );
        } else {
            serve(conn, Polling(Loopback::new(CATALOG)), &Config::default());
        }
        assert!(String::from_utf8_lossy(&output.borrow())
            .contains("Sec-WebSocket-Protocol: ridl-ws-v2\r\n"));
        assert_eq!(
            frame::decode(&messages(&output.borrow())[0].1).unwrap(),
            Frame::Attached(AttachOutcome::Accepted)
        );
    }
}
#[test]
fn mixed_offer_selects_provider_role() {
    for selected in [false, true] {
        let conn = connection(
            "ridl-ws-v3, ridl-ws-v2, ridl-ws-v2-provider",
            "/",
            &wire(Frame::Attached(AttachOutcome::Accepted)),
        );
        let output = conn.output.clone();
        let ended = if selected {
            serve_provided_selected(conn, |_| Ok(backend()), &Config::default())
        } else {
            serve_provided(conn, backend(), &Config::default())
        };
        assert_eq!(ended, Ended::Io(io::ErrorKind::UnexpectedEof));
        assert!(String::from_utf8_lossy(&output.borrow())
            .contains("Sec-WebSocket-Protocol: ridl-ws-v2-provider\r\n"));
        assert!(matches!(
            frame::decode(&messages(&output.borrow())[0].1),
            Ok(Frame::Attach(_))
        ));
    }
}
#[test]
fn provider_selector_gets_exact_path() {
    let path = "/a.b/primary/c.d?x=%20";
    let count = Cell::new(0);
    let conn = connection(frame::SUBPROTOCOL_PROVIDER, path, &[]);
    serve_provided_selected(
        conn,
        |got| {
            assert_eq!(got, path);
            count.set(count.get() + 1);
            Ok(backend())
        },
        &Config::default(),
    );
    assert_eq!(count.get(), 1);
    for (error, status) in [
        (SelectionError::NotFound, "404 Not Found"),
        (SelectionError::Conflict, "409 Conflict"),
    ] {
        let conn = connection(frame::SUBPROTOCOL_PROVIDER, path, &[]);
        let output = conn.output.clone();
        assert_eq!(
            serve_provided_selected::<_, Providing<Loopback>, _>(
                conn,
                |_| Err(error),
                &Config::default()
            ),
            Ended::Rejected
        );
        assert!(output
            .borrow()
            .starts_with(format!("HTTP/1.1 {status}\r\n").as_bytes()));
        assert!(messages(&output.borrow()).is_empty());
    }
}
#[test]
fn origin_refusal_precedes_selection() {
    let conn = connection(frame::SUBPROTOCOL_PROVIDER, "/", &[]);
    let output = conn.output.clone();
    assert_eq!(
        serve_provided_selected::<_, Providing<Loopback>, _>(
            conn,
            |_| panic!("origin must precede selection"),
            &Config::default().with_origin_check(|_| false)
        ),
        Ended::Rejected
    );
    assert!(output.borrow().starts_with(b"HTTP/1.1 403"));
}
#[test]
fn provider_server_attaches_first() {
    let mut conn = connection(
        frame::SUBPROTOCOL_PROVIDER,
        "/",
        &wire(Frame::Attached(AttachOutcome::Accepted)),
    );
    conn.require_attach = true;
    let output = conn.output.clone();
    assert_eq!(
        serve_provided(conn, backend(), &Config::default()),
        Ended::Io(io::ErrorKind::UnexpectedEof)
    );
    let messages = messages(&output.borrow());
    assert_eq!(messages.len(), 1);
    assert_eq!(
        frame::decode(&messages[0].1).unwrap(),
        Frame::Attach(Attach::new(2, 1, CATALOG.hash, CATALOG.name).unwrap())
    );
}
#[test]
fn provider_refusal_or_bad_attached_releases_backend() {
    for (tail, expected) in [
        (
            wire(Frame::Attached(AttachOutcome::Refused(
                Reason::CatalogMismatch,
            ))),
            Ended::ServerClosed(1000),
        ),
        (masked(2, &[2, 1, 1]), Ended::ServerClosed(1002)),
        (
            wire(Frame::Attach(
                Attach::new(2, 1, CATALOG.hash, CATALOG.name).unwrap(),
            )),
            Ended::ServerClosed(1002),
        ),
        (vec![], Ended::Io(io::ErrorKind::UnexpectedEof)),
        (
            wire(Frame::Attached(AttachOutcome::Accepted)),
            Ended::Io(io::ErrorKind::UnexpectedEof),
        ),
        (
            [
                wire(Frame::Attached(AttachOutcome::Accepted)),
                wire(Frame::Attached(AttachOutcome::Accepted)),
            ]
            .concat(),
            Ended::ServerClosed(1002),
        ),
        (
            masked(8, &1000u16.to_be_bytes()),
            Ended::ClientClosed(Some(1000)),
        ),
        (masked(1, b"text"), Ended::ServerClosed(1003)),
        (vec![0x82, 3, 2, 1, 0], Ended::ServerClosed(1002)),
    ] {
        for selected in [false, true] {
            let conn = connection(frame::SUBPROTOCOL_PROVIDER, "/", &tail);
            let output = conn.output.clone();
            let drops = Rc::new(Cell::new(0));
            let tracked = Tracked {
                inner: backend(),
                drops: drops.clone(),
            };
            assert_eq!(
                if selected {
                    serve_provided_selected(conn, |_| Ok(tracked), &Config::default())
                } else {
                    serve_provided(conn, tracked, &Config::default())
                },
                expected
            );
            assert_eq!(drops.get(), 1);
            let messages = messages(&output.borrow());
            if let Ended::ServerClosed(code) = expected {
                assert_eq!(messages.last().unwrap(), &(8, code.to_be_bytes().to_vec()));
            }
        }
    }
}
#[test]
fn provider_backend_checks_table_and_limits() {
    for (name, hash) in [
        ("other", CATALOG.hash),
        (CATALOG.name, CatalogHash([8; 32])),
    ] {
        let err = match Providing::new(Loopback::new(CATALOG), MemberTable::new(name, hash)) {
            Err(err) => err,
            Ok(_) => panic!("mismatch accepted"),
        };
        assert_eq!(err.ports, CATALOG);
        assert_eq!((err.name.as_str(), err.hash), (name, hash));
    }
    for selected in [false, true] {
        let conn = connection(frame::SUBPROTOCOL_PROVIDER, "/", &[]);
        let output = conn.output.clone();
        let drops = Rc::new(Cell::new(0));
        let tracked = Tracked {
            inner: backend(),
            drops: drops.clone(),
        };
        let notes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = notes.clone();
        let config = Config::default()
            .with_max_message_bytes(335)
            .unwrap()
            .with_record(move |n| sink.lock().unwrap().push(*n));
        let ended = if selected {
            serve_provided_selected(conn, |_| Ok(tracked), &config)
        } else {
            serve_provided(conn, tracked, &config)
        };
        assert_eq!(ended, Ended::ConfigRefused);
        assert_eq!(drops.get(), 1);
        assert!(output.borrow().starts_with(b"HTTP/1.1 400"));
        assert_eq!(
            *notes.lock().unwrap(),
            vec![ridl_transport_ws::Note::MessageLimitTooSmall {
                target: frame::Target {
                    iface: InterfaceNo(1),
                    ord: Ordinal(2),
                    kind: Kind::Signal
                },
                needed: 336,
            }]
        );
    }
    let conn = connection(frame::SUBPROTOCOL_PROVIDER, "/", &[]);
    let output = conn.output.clone();
    serve_provided(
        conn,
        backend(),
        &Config::default().with_max_message_bytes(336).unwrap(),
    );
    assert!(output.borrow().starts_with(b"HTTP/1.1 101"));
}
#[test]
fn existing_consuming_handshake_vectors_unchanged() {
    for (offers, selected, version) in [
        ("ridl-ws-v1", "ridl-ws-v1", 1),
        ("ridl-ws-v2, ridl-ws-v1", "ridl-ws-v2", 2),
    ] {
        let conn = connection(
            offers,
            "/",
            &wire(Frame::Attach(
                Attach::new(version, 1, CATALOG.hash, CATALOG.name).unwrap(),
            )),
        );
        let output = conn.output.clone();
        serve(conn, Polling(Loopback::new(CATALOG)), &Config::default());
        assert!(String::from_utf8_lossy(&output.borrow())
            .contains(&format!("Sec-WebSocket-Protocol: {selected}\r\n")));
        assert_eq!(
            frame::decode(&messages(&output.borrow())[0].1).unwrap(),
            Frame::Attached(if version == 1 {
                AttachOutcome::Refused(Reason::FrameVersionUnsupported)
            } else {
                AttachOutcome::Accepted
            })
        );
    }
}

#[test]
fn provider_attach_uses_ports_encoding() {
    let conn = connection(frame::SUBPROTOCOL_PROVIDER, "/", &[]);
    let output = conn.output.clone();
    let backend = Providing::new(
        Loopback::<ridl_rt::encoding::Proto3>::with_encoding(CATALOG),
        table(),
    )
    .unwrap();
    serve_provided(conn, backend, &Config::default());
    assert_eq!(
        frame::decode(&messages(&output.borrow())[0].1).unwrap(),
        Frame::Attach(Attach::new(2, 2, CATALOG.hash, CATALOG.name).unwrap())
    );
}

#[test]
fn provider_releases_backend_when_upgrade_or_attach_write_fails() {
    for write in [1, 2] {
        let mut conn = connection(frame::SUBPROTOCOL_PROVIDER, "/", &[]);
        conn.fail_write = Some(write);
        let drops = Rc::new(Cell::new(0));
        let tracked = Tracked {
            inner: backend(),
            drops: drops.clone(),
        };
        assert_eq!(
            serve_provided_selected(conn, |_| Ok(tracked), &Config::default()),
            Ended::Io(io::ErrorKind::BrokenPipe)
        );
        assert_eq!(drops.get(), 1);
    }
}

#[test]
fn provider_ignores_ping_before_attached_and_answers_it() {
    let tail = [
        masked(9, b"ping"),
        wire(Frame::Attached(AttachOutcome::Accepted)),
    ]
    .concat();
    let conn = connection(frame::SUBPROTOCOL_PROVIDER, "/", &tail);
    let output = conn.output.clone();
    assert_eq!(
        serve_provided(conn, backend(), &Config::default()),
        Ended::Io(io::ErrorKind::UnexpectedEof)
    );
    let messages = messages(&output.borrow());
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1], (10, b"ping".to_vec()));
}
