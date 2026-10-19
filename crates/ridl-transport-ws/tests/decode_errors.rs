//! Messages the codec must refuse, discard or accept at the edges.

use ridl_rt::contract::{CatalogHash, InterfaceNo, Kind, Ordinal};
use ridl_transport_ws::frame::{
    decode, Attach, DecodeError, Discarded, Frame, NameError, ProtocolViolation, PublishState,
    Short, Target,
};

const PUBLISH_LIVE: [u8; 29] = [
    0x07, 0x01, 1, 0, 0, 0, 2, 0, 0, 0, 0xab, 0x89, 0x67, 0x45, 0x23, 0x01, 0, 0, 7, 0, 0, 0, 0, 0,
    0, 0, 0x02, 0xff, 0x01,
];
const RESPONSE_ACCEPTED: [u8; 36] = [
    0x0a, 0x03, 1, 0, 0, 0, 3, 0, 0, 0, 0xac, 0x89, 0x67, 0x45, 0x23, 0x01, 0, 0, 1, 0, 0, 0, 0, 0,
    0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0x01, 0x00,
];
const SUBSCRIBE: [u8; 10] = [0x03, 0x01, 1, 0, 0, 0, 2, 0, 0, 0];

fn violation(v: ProtocolViolation) -> Result<(), DecodeError> {
    Err(DecodeError::Violation(v))
}

fn decode_err(msg: &[u8]) -> Result<(), DecodeError> {
    decode(msg).map(|_| ())
}

fn attach_message(name_len: u8, name: &[u8]) -> Vec<u8> {
    let mut m = vec![0x01, 1, 0, 0, 0, 1];
    m.extend_from_slice(&[0u8; 32]);
    m.push(name_len);
    m.extend_from_slice(name);
    m
}

#[test]
fn an_empty_message_is_too_short() {
    assert_eq!(decode_err(&[]), violation(ProtocolViolation::TooShort));
}

#[test]
fn a_header_cut_short_is_too_short() {
    let cut = &PUBLISH_LIVE[..26];
    assert_eq!(decode_err(cut), violation(ProtocolViolation::TooShort));
}

#[test]
fn a_subscribe_with_trailing_bytes_is_control_too_long() {
    let mut m = SUBSCRIBE.to_vec();
    m.push(0);
    assert_eq!(decode_err(&m), violation(ProtocolViolation::ControlTooLong));
}

#[test]
fn form_0_and_form_11_are_unknown_codes() {
    assert_eq!(decode_err(&[0]), violation(ProtocolViolation::UnknownCode));
    assert_eq!(decode_err(&[11]), violation(ProtocolViolation::UnknownCode));
}

#[test]
fn kind_6_is_an_unknown_code() {
    let mut m = SUBSCRIBE;
    m[1] = 6;
    assert_eq!(decode_err(&m), violation(ProtocolViolation::UnknownCode));
}

#[test]
fn provenance_4_is_an_unknown_code() {
    let mut m = PUBLISH_LIVE;
    m[26] = 4;
    assert_eq!(decode_err(&m), violation(ProtocolViolation::UnknownCode));
}

#[test]
fn a_refused_response_is_an_outcome_not_allowed() {
    let mut m = RESPONSE_ACCEPTED;
    m[34] = 9;
    m[35] = 2;
    assert_eq!(
        decode_err(&m),
        violation(ProtocolViolation::OutcomeNotAllowed)
    );
}

#[test]
fn an_accepted_attached_with_detail_1_is_nonzero_detail() {
    assert_eq!(
        decode_err(&[0x02, 0x01, 0x01]),
        violation(ProtocolViolation::NonzeroDetail)
    );
}

#[test]
fn interface_0_is_a_zero_number() {
    let mut m = SUBSCRIBE;
    m[2] = 0;
    assert_eq!(decode_err(&m), violation(ProtocolViolation::ZeroNumber));
}

#[test]
fn ordinal_0_is_a_zero_number() {
    let mut m = SUBSCRIBE;
    m[6] = 0;
    assert_eq!(decode_err(&m), violation(ProtocolViolation::ZeroNumber));
}

#[test]
fn name_len_0_is_a_bad_name_length() {
    assert_eq!(
        decode_err(&attach_message(0, b"")),
        violation(ProtocolViolation::BadNameLength)
    );
}

#[test]
fn name_len_longer_than_the_message_is_a_bad_name_length() {
    assert_eq!(
        decode_err(&attach_message(9, b"veh")),
        violation(ProtocolViolation::BadNameLength)
    );
}

#[test]
fn a_name_that_is_not_utf8_is_a_bad_name() {
    assert_eq!(
        decode_err(&attach_message(2, &[0xff, 0xfe])),
        violation(ProtocolViolation::BadName)
    );
}

#[test]
fn an_init_publish_with_bytes_after_its_header_is_discarded() {
    let mut m = PUBLISH_LIVE;
    m[26] = 1;
    assert_eq!(
        decode_err(&m),
        Err(DecodeError::Discard(Discarded::UnexpectedPayload))
    );
}

#[test]
fn an_accepted_response_with_bytes_after_its_header_is_discarded() {
    let mut m = RESPONSE_ACCEPTED.to_vec();
    m.push(0);
    assert_eq!(
        decode_err(&m),
        Err(DecodeError::Discard(Discarded::UnexpectedPayload))
    );
}

#[test]
fn a_live_publish_with_an_empty_payload_decodes() {
    let m = &PUBLISH_LIVE[..27];
    match decode(m) {
        Ok(Frame::Publish {
            target,
            state: PublishState::Live(payload),
            ..
        }) => {
            assert_eq!(
                target,
                Target {
                    kind: Kind::Signal,
                    iface: InterfaceNo(1),
                    ord: Ordinal(2)
                }
            );
            assert!(payload.is_empty());
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn attach_new_refuses_an_empty_name_and_a_256_byte_name() {
    let hash = CatalogHash([0; 32]);
    assert_eq!(Attach::new(1, 1, hash, "").unwrap_err(), NameError::Empty);
    let long = "a".repeat(256);
    assert_eq!(
        Attach::new(1, 1, hash, &long).unwrap_err(),
        NameError::TooLong { len: 256 }
    );
    let longest = "a".repeat(255);
    assert!(Attach::new(1, 1, hash, &longest).is_ok());
}

#[test]
fn encode_into_a_short_buffer_reports_the_needed_length() {
    let frame = decode(&PUBLISH_LIVE).unwrap();
    let mut out = [0u8; 28];
    assert_eq!(frame.encode(&mut out), Err(Short { needed: 29 }));
    assert_eq!(out, [0u8; 28], "a short buffer is left untouched");
}
