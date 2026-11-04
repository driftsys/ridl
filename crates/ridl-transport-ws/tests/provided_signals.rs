//! Reversed vectors through the public connection entry point, without sockets.
use ridl_loopback::Loopback;
use ridl_rt::contract::{CatalogHash, CatalogRef, InterfaceNo, Kind, Ordinal};
use ridl_rt::sample::{Occurrence, Provenance, Sample, Timestamp};
use ridl_transport_ws::frame::{self, Frame, Target};
use ridl_transport_ws::{serve_provided, Config, Connection, Ended, MemberTable, ProvidedBackend};
use std::cell::RefCell;
use std::io::{self, Cursor, Read, Write};
use std::rc::Rc;
use std::time::Duration;
struct Memory {
    input: Cursor<Vec<u8>>,
    output: Rc<RefCell<Vec<u8>>>,
}
impl Read for Memory {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        self.input.read(b)
    }
}
impl Write for Memory {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.output.borrow_mut().extend_from_slice(b);
        Ok(b.len())
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
type Delivery = (Target, Provenance, Option<Vec<u8>>, Timestamp);
struct Backend {
    ports: Loopback,
    table: MemberTable,
    seen: Rc<RefCell<Vec<Delivery>>>,
}
impl ProvidedBackend for Backend {
    type Ports = Loopback;
    fn ports(&mut self) -> &mut Loopback {
        &mut self.ports
    }
    fn members(&self) -> &MemberTable {
        &self.table
    }
    fn sample(&mut self, t: Target, s: Sample<Option<&[u8]>>, _: &Config) {
        self.seen.borrow_mut().push((
            t,
            s.provenance,
            s.value.map(<[u8]>::to_vec),
            s.envelope.stamp,
        ));
    }
    fn occurrence(&mut self, _: Target, _: Occurrence<&[u8]>, _: Option<u64>, _: &Config) {
        panic!("no event in this vector")
    }
    fn commit(&mut self, _: Timestamp) {}
}
fn vector(id: &str) -> Vec<u8> {
    let text = include_str!("../../../docs/specification/frame-specification.md");
    let row = text
        .lines()
        .find(|line| line.starts_with(&format!("| `{id}`")))
        .unwrap();
    let hex = row.split('|').nth(3).unwrap().replace(['`', ' '], "");
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}
fn masked(bytes: &[u8]) -> Vec<u8> {
    assert!(bytes.len() < 126);
    let mut out = vec![0x82, 0x80 | bytes.len() as u8, 1, 2, 3, 4];
    out.extend(
        bytes
            .iter()
            .enumerate()
            .map(|(i, b)| b ^ [1, 2, 3, 4][i % 4]),
    );
    out
}
#[test]
fn reversed_vectors_deliver_after_subscription_answer() {
    // The normative vector has signal ordinal 2. It is a different catalog
    // from the identity deployment, where ordinal 2 is an event.
    let attach_bytes = vector("provider-attach-2");
    let Frame::Attach(attach) = frame::decode(&attach_bytes).unwrap() else {
        panic!()
    };
    let catalog = CatalogRef {
        name: "veh.cabin",
        hash: CatalogHash(core::array::from_fn(|i| i as u8)),
    };
    assert_eq!(attach.hash(), &catalog.hash);
    let target = Target {
        kind: Kind::Signal,
        iface: InterfaceNo(1),
        ord: Ordinal(2),
    };
    let mut table = MemberTable::new(catalog.name, catalog.hash);
    table.insert(target.iface, target.ord, target.kind, None);
    let mut input=b"GET / HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Protocol: ridl-ws-v2-provider\r\n\r\n".to_vec();
    input.extend(masked(&vector("provider-attached-accepted")));
    let answer = Frame::Answer {
        target,
        outcome: frame::AnswerOutcome::Accepted,
    };
    let mut bytes = vec![0; answer.encoded_len()];
    answer.encode(&mut bytes).unwrap();
    input.extend(masked(&bytes));
    input.extend(masked(&vector("provider-publish-live")));
    let seen = Rc::default();
    let output = Rc::default();
    let ended = serve_provided(
        Memory {
            input: Cursor::new(input),
            output: Rc::clone(&output),
        },
        Backend {
            ports: Loopback::new(catalog),
            table,
            seen: Rc::clone(&seen),
        },
        &Config::default(),
    );
    assert_eq!(ended, Ended::Io(io::ErrorKind::UnexpectedEof));
    assert_eq!(
        *seen.borrow(),
        [(
            target,
            Provenance::Live,
            Some(vec![0xff, 0x01]),
            Timestamp(0x0123456789ab)
        )]
    );
    let output = output.borrow();
    let start = output.windows(4).position(|b| b == b"\r\n\r\n").unwrap() + 4;
    assert_eq!(
        &output[start + 2..start + 2 + attach_bytes.len()],
        attach_bytes
    );
}
