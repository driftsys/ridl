//! Checks the `websocket` binding row that the toolchain renders into the
//! deployment section against the frames this codec writes.

use ridl_ir::codegen::known_bindings;
use ridl_rt::contract::{CatalogHash, InterfaceNo, Kind, Ordinal, CALL_CEILING};
use ridl_rt::sample::{Envelope, Timestamp};
use ridl_transport_ws::frame::{
    Attach, Frame, PublishState, ResponseOutcome, Target, BLOCK_BYTES, ENVELOPE_BYTES,
    FRAME_VERSION, MAX_MESSAGE_LIMIT, MIN_MESSAGE_LIMIT,
};

/// The frame header maximum and the envelope size of the `websocket` row.
fn row() -> (usize, usize) {
    let bindings = known_bindings();
    let row = bindings
        .iter()
        .find(|b| b.name == "websocket")
        .expect("the websocket row exists");
    assert_eq!(row.version, FRAME_VERSION.to_string());
    (
        row.frame_header_max_bytes.expect("the row states a header") as usize,
        row.envelope_bytes.expect("the row states an envelope") as usize,
    )
}

fn target(kind: Kind) -> Target {
    Target {
        kind,
        iface: InterfaceNo(1),
        ord: Ordinal(2),
    }
}

/// Encodes `frame` into a buffer and checks the written length against both
/// the returned length and `encoded_len`.
fn written_len(frame: &Frame<'_>) -> usize {
    let mut buf = vec![0u8; 32 * 1024];
    let written = frame.encode(&mut buf).expect("the buffer is large enough");
    assert_eq!(written, frame.encoded_len());
    written
}

fn envelope() -> Envelope {
    Envelope {
        stamp: Timestamp(3),
        seq: 4,
    }
}

#[test]
fn the_largest_data_plane_message_is_header_plus_envelope_plus_payload() {
    let (header, envelope_bytes) = row();
    assert_eq!(envelope_bytes, ENVELOPE_BYTES);
    for len in [0usize, 1, 255] {
        let payload = vec![0u8; len];
        let frame = Frame::Response {
            target: target(Kind::Query),
            envelope: envelope(),
            correlation: 5,
            outcome: ResponseOutcome::Reply(&payload),
        };
        assert_eq!(written_len(&frame), header + envelope_bytes + len);
    }
}

#[test]
fn every_other_data_plane_form_is_no_larger() {
    let (header, envelope_bytes) = row();
    for len in [0usize, 1, 255] {
        let payload = vec![0u8; len];
        let bound = header + envelope_bytes + len;
        let frames = [
            Frame::Publish {
                target: target(Kind::Signal),
                envelope: envelope(),
                state: PublishState::Live(&payload),
            },
            Frame::Occurrence {
                target: target(Kind::Event),
                envelope: envelope(),
                payload: &payload,
            },
            Frame::Request {
                target: target(Kind::Command),
                envelope: envelope(),
                payload: &payload,
            },
            Frame::Block {
                target: target(Kind::Fixed),
                envelope: envelope(),
                total: len.max(1) as u32,
                index: 0,
                payload: &payload,
            },
        ];
        for frame in frames {
            assert!(written_len(&frame) <= bound, "{frame:?} exceeds {bound}");
        }
    }
}

#[test]
fn the_largest_attach_is_294_bytes() {
    let name = "a".repeat(255);
    let attach = Attach::new(1, 1, CatalogHash([0; 32]), &name).expect("a valid attach");
    assert_eq!(written_len(&Frame::Attach(attach)), MIN_MESSAGE_LIMIT);
    assert_eq!(MIN_MESSAGE_LIMIT, 294);
}

#[test]
fn the_largest_message_limit_is_the_largest_data_plane_message() {
    assert_eq!(MAX_MESSAGE_LIMIT, ENVELOPE_BYTES + 20 + 16_128);
    assert_eq!(MAX_MESSAGE_LIMIT, ENVELOPE_BYTES + 20 + CALL_CEILING);
    let payload = vec![0u8; 16_128];
    let frame = Frame::Response {
        target: target(Kind::Query),
        envelope: envelope(),
        correlation: 5,
        outcome: ResponseOutcome::Reply(&payload),
    };
    assert_eq!(written_len(&frame), MAX_MESSAGE_LIMIT);
}

#[test]
fn a_block_message_is_its_34_byte_header_and_at_most_4096_bytes() {
    assert_eq!(BLOCK_BYTES, 4_096);
    let payload = vec![0u8; BLOCK_BYTES];
    let frame = Frame::Block {
        target: target(Kind::Fixed),
        envelope: envelope(),
        total: BLOCK_BYTES as u32,
        index: 0,
        payload: &payload,
    };
    assert_eq!(frame.encoded_len(), 4_130);
    assert_eq!(written_len(&frame), 4_130);
    assert_eq!(written_len(&frame), ENVELOPE_BYTES + 18 + BLOCK_BYTES);
}
