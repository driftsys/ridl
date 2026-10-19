//! Checks the `websocket` binding row that the toolchain renders into the
//! deployment section against the frames this codec writes.

use ridl_ir::codegen::known_bindings;
use ridl_rt::contract::{CatalogHash, InterfaceNo, Kind, Ordinal};
use ridl_rt::sample::{Envelope, Timestamp};
use ridl_transport_ws::frame::{
    Attach, Frame, PublishState, ResponseOutcome, Target, ENVELOPE_BYTES, MIN_MESSAGE_LIMIT,
};

/// The frame header maximum and the envelope size of the `websocket` row.
fn row() -> (usize, usize) {
    let bindings = known_bindings();
    let row = bindings
        .iter()
        .find(|b| b.name == "websocket")
        .expect("the websocket row exists");
    assert_eq!(row.version, "1");
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
        assert_eq!(frame.encoded_len(), header + envelope_bytes + len);
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
        ];
        for frame in frames {
            assert!(frame.encoded_len() <= bound, "{frame:?} exceeds {bound}");
        }
    }
}

#[test]
fn the_largest_attach_is_294_bytes() {
    let name = "a".repeat(255);
    let attach = Attach::new(1, 1, CatalogHash([0; 32]), &name).expect("a valid attach");
    assert_eq!(Frame::Attach(attach).encoded_len(), MIN_MESSAGE_LIMIT);
    assert_eq!(MIN_MESSAGE_LIMIT, 294);
}
