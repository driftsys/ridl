//! The test vectors of frame specification §11.1, decoded and encoded again.

use ridl_rt::contract::{CatalogHash, InterfaceNo, Kind, Ordinal};
use ridl_rt::error::Contract;
use ridl_rt::payload::{Rule, Violation};
use ridl_rt::sample::{Envelope, Timestamp};
use ridl_transport_ws::frame::{
    decode, AnswerOutcome, Attach, AttachOutcome, DecodeError, Frame, ProtocolViolation,
    PublishState, Reason, ResponseOutcome, Target, BLOCK_BYTES,
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

// -- Frame version 2 (driftsys/ridl#1129) ------------------------------------
//
// `attach-2`, `block-last` and `block-one` are read from the version 2 table of
// the frame specification. The other block vectors are test constants: they
// are 4,096 bytes or more, or they are violations. Every hex string is one
// whole message.

const BLOCK_HEADER: &str = "0b 05 01000000 05000000 ab89674523010000 0000000000000000";

/// The pattern `P(n)`: byte `i` is `i mod 251`.
fn pattern(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i % 251) as u8).collect()
}

fn fixed_block(total: u32, index: u32, payload: &[u8]) -> Frame<'_> {
    Frame::Block {
        target: target(Kind::Fixed, 1, 5),
        envelope: envelope(0x0123_4567_89AB, 0),
        total,
        index,
        payload,
    }
}

/// The header of a `block` of the shared vectors followed by `payload`.
fn block_message(total_hex: &str, index_hex: &str, payload: &[u8]) -> Vec<u8> {
    let mut m = hex(&format!("{BLOCK_HEADER} {total_hex} {index_hex}"));
    m.extend_from_slice(payload);
    m
}

/// The bytes of one row of the version 2 vector table of the frame
/// specification, by its `Id`.
fn spec_version_2_vector(id: &str) -> Vec<u8> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/specification/frame-specification.md"
    );
    let text = std::fs::read_to_string(path).unwrap();
    let start = text
        .find("**Version 2 test vectors.**")
        .expect("version 2 vectors paragraph");
    text[start..]
        .lines()
        .filter(|l| l.starts_with('|'))
        .map(|l| l.split('|').map(str::trim).collect::<Vec<_>>())
        .find(|cells| cells[1].trim_matches('`') == id)
        .map(|cells| hex(cells[3]))
        .unwrap_or_else(|| panic!("no version 2 vector {id} in the specification"))
}

fn assert_round_trip(id: &str, want: Frame<'_>, bytes: &[u8]) {
    let got = decode(bytes).unwrap_or_else(|e| panic!("vector {id} did not decode: {e:?}"));
    assert_eq!(got, want, "vector {id}");
    assert_eq!(want.encoded_len(), bytes.len(), "encoded_len of {id}");
    let mut out = vec![0u8; bytes.len()];
    let n = want.encode(&mut out).unwrap();
    assert_eq!(&out[..n], bytes, "encoding of {id}");
}

#[test]
fn every_version_2_vector_decodes_and_reencodes_byte_for_byte() {
    let mut hash = [0u8; 32];
    for (i, b) in hash.iter_mut().enumerate() {
        *b = i as u8;
    }
    let attach_2 = spec_version_2_vector("attach-2");
    assert_eq!(attach_2.len(), 48);
    assert_round_trip(
        "attach-2",
        Frame::Attach(Attach::new(2, 1, CatalogHash(hash), "veh.cabin").unwrap()),
        &attach_2,
    );

    let block_last = spec_version_2_vector("block-last");
    assert_eq!(block_last.len(), 36);
    assert_round_trip(
        "block-last",
        fixed_block(4_098, 1, &[0xab, 0xcd]),
        &block_last,
    );

    let block_one = spec_version_2_vector("block-one");
    assert_eq!(block_one.len(), 35);
    assert_round_trip("block-one", fixed_block(1, 0, &[0xab]), &block_one);
}

#[test]
fn the_first_block_of_a_4098_byte_payload_carries_4096_bytes_of_the_pattern() {
    let payload = pattern(BLOCK_BYTES);
    let message = block_message("02100000", "00000000", &payload);
    assert_eq!(message.len(), 4_130);
    assert_round_trip("block-first", fixed_block(4_098, 0, &payload), &message);
    match decode(&message) {
        Ok(Frame::Block { payload: got, .. }) => assert_eq!(got, &pattern(4_096)[..]),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn a_last_block_of_the_largest_total_decodes_without_overflow() {
    let zeros = vec![0u8; 4_096];
    let max_last = block_message("ffffffff", "ffff0f00", &zeros[..4_095]);
    assert_eq!(max_last.len(), 4_129);
    assert_round_trip(
        "block-max-last",
        fixed_block(u32::MAX, 1_048_575, &zeros[..4_095]),
        &max_last,
    );

    let full = block_message("00100000", "00000000", &zeros);
    assert_eq!(full.len(), 4_130);
    assert_round_trip("block-full", fixed_block(4_096, 0, &zeros), &full);

    let max_last_full = block_message("ffffffff", "ffff0f00", &zeros);
    assert_eq!(max_last_full.len(), 4_130);
    assert_eq!(
        decode(&max_last_full),
        Err(DecodeError::Violation(ProtocolViolation::BadBlock))
    );
}
