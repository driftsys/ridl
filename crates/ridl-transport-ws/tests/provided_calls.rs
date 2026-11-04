//! Reversed calls and liveness through the public socket entry point.
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use ridl_loopback::Loopback;
use ridl_rt::contract::{CatalogHash, CatalogRef, InterfaceNo, Kind, Ordinal};
use ridl_rt::error::{CallError, Transport};
use ridl_rt::port::Caller;
use ridl_rt::sample::{Envelope, Timestamp};
use ridl_transport_ws::frame::{self, AttachOutcome, Frame, ResponseOutcome};
use ridl_transport_ws::{serve_provided, Config, Ended, MemberTable, Providing};
use tungstenite::client::IntoClientRequest;
use tungstenite::{Message, WebSocket};

const CATALOG: CatalogRef = CatalogRef {
    name: "calls",
    hash: CatalogHash([7; 32]),
};
const IFACE: InterfaceNo = InterfaceNo(1);
fn send(ws: &mut WebSocket<TcpStream>, f: Frame<'_>) {
    let mut bytes = vec![0; f.encoded_len()];
    f.encode(&mut bytes).unwrap();
    ws.send(Message::Binary(bytes.into())).unwrap();
}
fn start(
    config: Config,
    ports: Loopback,
) -> (
    WebSocket<TcpStream>,
    mpsc::Receiver<Ended>,
    thread::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let mut table = MemberTable::new(CATALOG.name, CATALOG.hash);
    for (ord, kind) in [(3, Kind::Command), (4, Kind::Query)] {
        table.insert(IFACE, Ordinal(ord), kind, None);
    }
    let backend = Providing::new(ports, table).unwrap();
    let (tx, rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        let (socket, _) = listener.accept().unwrap();
        socket.set_nonblocking(false).unwrap();
        tx.send(serve_provided(socket, backend, &config)).unwrap();
    });
    let mut request = format!("ws://{addr}/").into_client_request().unwrap();
    request.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        "ridl-ws-v2-provider".parse().unwrap(),
    );
    let socket = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    socket
        .set_write_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let (mut ws, _) = tungstenite::client(request, socket).unwrap();
    let msg = ws.read().unwrap();
    assert!(matches!(
        frame::decode(&msg.into_data()).unwrap(),
        Frame::Attach(_)
    ));
    send(&mut ws, Frame::Attached(AttachOutcome::Accepted));
    (ws, rx, worker)
}
#[test]
fn provider_answers_command_and_query_over_wire_then_disconnects() {
    let ports = Loopback::new(CATALOG);
    let mut caller = ports.attach();
    let command = caller.command(IFACE, Ordinal(3), &[1, 2], None).unwrap();
    let query = caller.query(IFACE, Ordinal(4), &[3, 4], None).unwrap();
    let (mut ws, ended, worker) = start(Config::default(), ports);
    let mut requests = vec![];
    for _ in 0..2 {
        let bytes = ws.read().unwrap().into_data();
        let Frame::Request {
            target,
            envelope,
            payload,
        } = frame::decode(&bytes).unwrap()
        else {
            panic!("request expected")
        };
        requests.push((target, envelope.seq, payload.to_vec()));
    }
    assert_eq!(
        (requests[0].0.kind, requests[0].0.ord, &requests[0].2),
        (Kind::Command, Ordinal(3), &vec![1, 2])
    );
    assert_eq!(
        (requests[1].0.kind, requests[1].0.ord, &requests[1].2),
        (Kind::Query, Ordinal(4), &vec![3, 4])
    );
    send(
        &mut ws,
        Frame::Response {
            target: requests[1].0,
            correlation: requests[1].1,
            envelope: Envelope {
                stamp: Timestamp(50),
                seq: 1,
            },
            outcome: ResponseOutcome::Reply(&[9, 8]),
        },
    );
    // Close is ordered after the response; receiving it proves the response was handled.
    ws.close(None).unwrap();
    let _ = ws.read();
    assert!(matches!(
        ended.recv_timeout(Duration::from_secs(3)).unwrap(),
        Ended::ClientClosed(_)
    ));
    worker.join().unwrap();
    assert_eq!(
        caller.ack(command),
        Some(Err(CallError::Transport(Transport::Down)))
    );
    let mut out = [0; 2];
    assert_eq!(caller.reply(query, &mut out), Ok(Some(Ok(2))));
    assert_eq!(out, [9, 8]);
}
#[test]
fn provider_server_pings_and_closes_unresponsive_client() {
    let mut config = Config::default();
    config.poll_interval = Duration::from_millis(2);
    config.ping_interval = Duration::from_millis(20);
    config.liveness_timeout = Duration::from_millis(150);
    let (mut ws, ended, worker) = start(config, Loopback::new(CATALOG));
    assert!(matches!(ws.read().unwrap(), Message::Ping(_)));
    // Do not flush tungstenite's queued automatic pong. The server must end
    // the session itself, with the socket still open and no client traffic.
    assert_eq!(
        ended.recv_timeout(Duration::from_secs(3)).unwrap(),
        Ended::LivenessTimeout
    );
    worker.join().unwrap();
}

use ridl_transport_ws::Connection;
use std::io::{self, Cursor, Read, Write};

#[derive(Clone, Copy)]
enum Exit {
    Eof,
    ReadError,
    WriteError,
    WrongDirection,
    Close,
    Response,
}
struct Scripted {
    head: Cursor<Vec<u8>>,
    phase: u8,
    output: Vec<u8>,
    exit: Exit,
    binaries: usize,
}
fn masked(op: u8, payload: &[u8]) -> Vec<u8> {
    assert!(payload.len() < 126);
    let mut out = vec![0x80 | op, 0x80 | payload.len() as u8, 1, 2, 3, 4];
    out.extend(
        payload
            .iter()
            .enumerate()
            .map(|(i, b)| b ^ [1, 2, 3, 4][i % 4]),
    );
    out
}
fn binary(f: Frame<'_>) -> Vec<u8> {
    let mut bytes = vec![0; f.encoded_len()];
    f.encode(&mut bytes).unwrap();
    masked(2, &bytes)
}
impl Read for Scripted {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.head.position() < self.head.get_ref().len() as u64 {
            return self.head.read(out);
        }
        let phase = self.phase;
        self.phase += 1;
        let bytes = if phase == 0 {
            binary(Frame::Attached(AttachOutcome::Accepted))
        } else if phase == 1 {
            let header = self
                .output
                .windows(4)
                .position(|b| b == b"\r\n\r\n")
                .unwrap()
                + 4;
            let attach_len = self.output[header + 1] as usize;
            let request = &self.output[header + 2 + attach_len..];
            let Frame::Request {
                target,
                envelope,
                payload,
            } = frame::decode(&request[2..]).unwrap()
            else {
                panic!("request expected")
            };
            assert_eq!(payload, &[6]);
            assert_eq!(envelope.seq, 1);
            match self.exit {
                Exit::Eof | Exit::WriteError => return Ok(0),
                Exit::ReadError => return Err(io::ErrorKind::ConnectionReset.into()),
                Exit::WrongDirection => binary(Frame::Request {
                    target,
                    envelope,
                    payload: &[],
                }),
                Exit::Close => masked(8, &[]),
                Exit::Response => binary(Frame::Response {
                    target,
                    envelope,
                    correlation: envelope.seq,
                    outcome: ResponseOutcome::Accepted,
                }),
            }
        } else {
            return Ok(0);
        };
        assert!(out.len() >= bytes.len());
        out[..bytes.len()].copy_from_slice(&bytes);
        Ok(bytes.len())
    }
}
impl Write for Scripted {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.first() == Some(&0x82) {
            self.binaries += 1;
            if self.binaries == 2 && matches!(self.exit, Exit::WriteError) {
                return Err(io::ErrorKind::BrokenPipe.into());
            }
        }
        self.output.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Connection for Scripted {
    fn set_read_timeout(&self, _: Option<Duration>) -> io::Result<()> {
        Ok(())
    }
    fn set_write_timeout(&self, _: Option<Duration>) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn provider_public_entry_settles_on_eof_io_close_and_protocol_exit() {
    for exit in [
        Exit::Eof,
        Exit::ReadError,
        Exit::WriteError,
        Exit::WrongDirection,
        Exit::Close,
        Exit::Response,
    ] {
        let mut ports = Loopback::new(CATALOG);
        let mut caller = ports.attach();
        let call = caller.command(IFACE, Ordinal(3), &[6], None).unwrap();
        let mut table = MemberTable::new(CATALOG.name, CATALOG.hash);
        table.insert(IFACE, Ordinal(3), Kind::Command, None);
        let conn = Scripted { head: Cursor::new(b"GET / HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Protocol: ridl-ws-v2-provider\r\n\r\n".to_vec()), phase:0, output: vec![], exit, binaries:0 };
        let mut config = Config::default();
        config.poll_interval = Duration::ZERO;
        let ended = serve_provided(conn, Providing::new(&mut ports, table).unwrap(), &config);
        assert_eq!(
            ended,
            match exit {
                Exit::Eof | Exit::Response => Ended::Io(io::ErrorKind::UnexpectedEof),
                Exit::ReadError => Ended::Io(io::ErrorKind::ConnectionReset),
                Exit::WriteError => Ended::Io(io::ErrorKind::BrokenPipe),
                Exit::WrongDirection => Ended::ServerClosed(1002),
                Exit::Close => Ended::ClientClosed(None),
            }
        );
        assert_eq!(
            caller.ack(call),
            Some(if matches!(exit, Exit::Response) {
                Ok(())
            } else {
                Err(CallError::Transport(Transport::Down))
            })
        );
    }
}
