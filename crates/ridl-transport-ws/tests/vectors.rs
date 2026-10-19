//! The test vectors of frame specification §11.1, decoded and encoded again.

use ridl_rt::contract::{CatalogHash, InterfaceNo, Kind, Ordinal};
use ridl_rt::error::Contract;
use ridl_rt::payload::{Rule, Violation};
use ridl_rt::sample::{Envelope, Timestamp};
use ridl_transport_ws::frame::{
    decode, AnswerOutcome, Attach, AttachOutcome, Frame, PublishState, Reason, ResponseOutcome,
    Target,
};

fn hex(text: &str) -> Vec<u8> {
    let digits: Vec<u8> = text
        .bytes()
        .filter(|b| !b.is_ascii_whitespace() && *b != b'`')
        .collect();
    assert_eq!(digits.len() % 2, 0, "odd number of hex digits in {text}");
    digits
        .chunks(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn target(kind: Kind, iface: u32, ord: u32) -> Target {
    Target {
        kind,
        iface: InterfaceNo(iface),
        ord: Ordinal(ord),
    }
}

fn envelope(stamp: i64, seq: u64) -> Envelope {
    Envelope {
        stamp: Timestamp(stamp),
        seq,
    }
}

const KNOWN: [&str; 9] = [
    "attach-1",
    "attached-accepted",
    "attached-mismatch",
    "subscribe-signal",
    "answer-accepted",
    "publish-live",
    "request-command",
    "response-accepted",
    "response-violation",
];

fn expected(id: &str) -> Option<Frame<'static>> {
    let mut hash = [0u8; 32];
    for (i, b) in hash.iter_mut().enumerate() {
        *b = i as u8;
    }
    Some(match id {
        "attach-1" => Frame::Attach(Attach::new(1, 1, CatalogHash(hash), "veh.cabin").unwrap()),
        "attached-accepted" => Frame::Attached(AttachOutcome::Accepted),
        "attached-mismatch" => Frame::Attached(AttachOutcome::Refused(Reason::CatalogMismatch)),
        "subscribe-signal" => Frame::Subscribe(target(Kind::Signal, 1, 2)),
        "answer-accepted" => Frame::Answer {
            target: target(Kind::Signal, 1, 2),
            outcome: AnswerOutcome::Accepted,
        },
        "publish-live" => Frame::Publish {
            target: target(Kind::Signal, 1, 2),
            envelope: envelope(0x0123_4567_89AB, 7),
            state: PublishState::Live(&[0xff, 0x01]),
        },
        "request-command" => Frame::Request {
            target: target(Kind::Command, 1, 3),
            envelope: envelope(0x0123_4567_89AB, 1),
            payload: &[0x01],
        },
        "response-accepted" => Frame::Response {
            target: target(Kind::Command, 1, 3),
            envelope: envelope(0x0123_4567_89AC, 1),
            correlation: 1,
            outcome: ResponseOutcome::Accepted,
        },
        "response-violation" => Frame::Response {
            target: target(Kind::Query, 1, 4),
            envelope: envelope(0x0123_4567_89AC, 2),
            correlation: 2,
            outcome: ResponseOutcome::Contract(Contract::InvalidValue(Violation {
                type_name: None,
                rule: Rule::Range,
            })),
        },
        _ => return None,
    })
}

#[test]
fn every_spec_vector_decodes_and_reencodes_byte_for_byte() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/specification/frame-specification.md"
    );
    let text = std::fs::read_to_string(path).unwrap();
    let start = text.find("**Test vectors.**").expect("vectors paragraph");
    let rows: Vec<&str> = text[start..]
        .lines()
        .skip_while(|l| !l.starts_with('|'))
        .take_while(|l| l.starts_with('|'))
        .skip(2)
        .collect();
    assert!(!rows.is_empty(), "no vector rows found");
    let mut seen = Vec::new();
    for row in rows {
        let cells: Vec<&str> = row.split('|').map(str::trim).collect();
        let id = cells[1].trim_matches('`');
        seen.push(id.to_string());
        let bytes = hex(cells[3]);
        let want = expected(id).unwrap_or_else(|| panic!("vector {id} is not known to this test"));
        let got = decode(&bytes).unwrap_or_else(|e| panic!("vector {id} did not decode: {e:?}"));
        assert_eq!(got, want, "vector {id}");
        assert_eq!(want.encoded_len(), bytes.len(), "encoded_len of {id}");
        let mut out = vec![0u8; bytes.len()];
        let n = want.encode(&mut out).unwrap();
        assert_eq!(&out[..n], &bytes[..], "encoding of {id}");
    }
    seen.sort();
    let mut known: Vec<String> = KNOWN.iter().map(|s| s.to_string()).collect();
    known.sort();
    assert_eq!(
        seen, known,
        "the table and this test disagree on the vectors"
    );
}
