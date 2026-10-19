//! Every code of frame specification §4 and §9.6, written out as bytes and
//! checked in both directions: the bytes decode to the frame, and the frame
//! encodes to the bytes. The expected bytes are written by hand, so a wrong
//! code constant in the codec cannot cancel itself out in a round trip.

use ridl_rt::contract::{InterfaceNo, Kind, Ordinal};
use ridl_rt::error::Contract;
use ridl_rt::payload::{Rule, Violation};
use ridl_rt::sample::{Envelope, Timestamp};
use ridl_transport_ws::frame::{
    decode, AnswerOutcome, AttachOutcome, Frame, PublishState, Reason, ResponseOutcome, Target,
};

/// Interface 1, ordinal 2, kind byte as the caller writes it.
const IDS: [u8; 8] = [1, 0, 0, 0, 2, 0, 0, 0];
/// Stamp 0x0123456789AB, seq 7.
const ENV: [u8; 16] = [
    0xab, 0x89, 0x67, 0x45, 0x23, 0x01, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0,
];
/// Correlation 5.
const CORR: [u8; 8] = [5, 0, 0, 0, 0, 0, 0, 0];

fn target(kind: Kind) -> Target {
    Target {
        kind,
        iface: InterfaceNo(1),
        ord: Ordinal(2),
    }
}

fn envelope() -> Envelope {
    Envelope {
        stamp: Timestamp(0x0123_4567_89AB),
        seq: 7,
    }
}

fn control(form: u8, kind: u8, tail: &[u8]) -> Vec<u8> {
    [&[form, kind][..], &IDS, tail].concat()
}

fn publish(provenance: u8, payload: &[u8]) -> Vec<u8> {
    [&control(7, 1, &ENV)[..], &[provenance], payload].concat()
}

fn response(kind: u8, outcome: u8, detail: u8, payload: &[u8]) -> Vec<u8> {
    [
        &control(10, kind, &ENV)[..],
        &CORR,
        &[outcome, detail],
        payload,
    ]
    .concat()
}

fn invalid(rule: Rule) -> Contract {
    Contract::InvalidValue(Violation {
        type_name: None,
        rule,
    })
}

fn check(name: &str, bytes: &[u8], frame: Frame<'_>) {
    assert_eq!(decode(bytes), Ok(frame), "decode of {name}");
    assert_eq!(frame.encoded_len(), bytes.len(), "encoded_len of {name}");
    let mut out = vec![0u8; bytes.len()];
    let n = frame.encode(&mut out).unwrap();
    assert_eq!(&out[..n], bytes, "encode of {name}");
}

#[test]
fn every_code_decodes_and_encodes_as_written() {
    // Forms 5 and 6, and every kind code.
    check(
        "unsubscribe",
        &control(5, 2, &[]),
        Frame::Unsubscribe(target(Kind::Event)),
    );
    check(
        "read",
        &control(6, 5, &[]),
        Frame::Read(target(Kind::Fixed)),
    );
    check(
        "subscribe command",
        &control(3, 3, &[]),
        Frame::Subscribe(target(Kind::Command)),
    );
    check(
        "subscribe query",
        &control(3, 4, &[]),
        Frame::Subscribe(target(Kind::Query)),
    );

    // Provenance codes 1 and 3, and an occurrence.
    check(
        "publish init",
        &publish(1, &[]),
        Frame::Publish {
            target: target(Kind::Signal),
            envelope: envelope(),
            state: PublishState::Init,
        },
    );
    check(
        "publish invalid-declared",
        &publish(3, &[]),
        Frame::Publish {
            target: target(Kind::Signal),
            envelope: envelope(),
            state: PublishState::InvalidDeclared,
        },
    );
    check(
        "occurrence",
        &[&control(8, 2, &ENV)[..], &[0xaa, 0xbb]].concat(),
        Frame::Occurrence {
            target: target(Kind::Event),
            envelope: envelope(),
            payload: &[0xaa, 0xbb],
        },
    );

    // Response outcomes 2, 4, 5, 6, 7 and 8.
    check(
        "response reply",
        &response(4, 2, 0, &[9, 8]),
        Frame::Response {
            target: target(Kind::Query),
            envelope: envelope(),
            correlation: 5,
            outcome: ResponseOutcome::Reply(&[9, 8]),
        },
    );
    check(
        "response empty reply",
        &response(4, 2, 0, &[]),
        Frame::Response {
            target: target(Kind::Query),
            envelope: envelope(),
            correlation: 5,
            outcome: ResponseOutcome::Reply(&[]),
        },
    );
    for (name, code, outcome) in [
        ("corrupt", 7, ResponseOutcome::Corrupt),
        ("busy", 8, ResponseOutcome::Busy),
        (
            "precondition-failed",
            4,
            ResponseOutcome::Contract(Contract::PreconditionFailed),
        ),
        (
            "contract-broken",
            5,
            ResponseOutcome::Contract(Contract::ContractBroken),
        ),
        (
            "unknown-interaction",
            6,
            ResponseOutcome::Contract(Contract::UnknownInteraction),
        ),
    ] {
        check(
            name,
            &response(3, code, 0, &[]),
            Frame::Response {
                target: target(Kind::Command),
                envelope: envelope(),
                correlation: 5,
                outcome,
            },
        );
    }

    // Rule codes 2 to 6 (1 is in the vectors), in a response and an answer.
    for (code, rule) in [
        (2, Rule::Step),
        (3, Rule::Length),
        (4, Rule::Pattern),
        (5, Rule::Variant),
        (6, Rule::Unique),
    ] {
        check(
            "response rule",
            &response(4, 3, code, &[]),
            Frame::Response {
                target: target(Kind::Query),
                envelope: envelope(),
                correlation: 5,
                outcome: ResponseOutcome::Contract(invalid(rule)),
            },
        );
        check(
            "answer rule",
            &control(4, 1, &[3, code]),
            Frame::Answer {
                target: target(Kind::Signal),
                outcome: AnswerOutcome::Contract(invalid(rule)),
            },
        );
    }

    // An answer carrying each contract error.
    for (code, contract) in [
        (4, Contract::PreconditionFailed),
        (5, Contract::ContractBroken),
        (6, Contract::UnknownInteraction),
    ] {
        check(
            "answer contract",
            &control(4, 1, &[code, 0]),
            Frame::Answer {
                target: target(Kind::Signal),
                outcome: AnswerOutcome::Contract(contract),
            },
        );
    }

    // Refusal reasons 1, 3, 4 and 5 (2 is in the vectors).
    for (code, reason) in [
        (1, Reason::UnknownCatalog),
        (3, Reason::EncodingUnsupported),
        (4, Reason::FrameVersionUnsupported),
        (5, Reason::Range),
    ] {
        check(
            "attached refused",
            &[2, 9, code],
            Frame::Attached(AttachOutcome::Refused(reason)),
        );
    }
}

/// Every `Rule` of this `ridl-rt` has a tag: the six of frame specification
/// §9.6, in its order.
#[test]
fn every_rule_has_its_tag() {
    use ridl_rt::payload::Rule;
    use ridl_transport_ws::frame::rule_tag;
    let rules = [
        Rule::Range,
        Rule::Step,
        Rule::Length,
        Rule::Pattern,
        Rule::Variant,
        Rule::Unique,
    ];
    for (i, rule) in rules.into_iter().enumerate() {
        assert_eq!(rule_tag(rule), Some(i as u8 + 1), "{rule:?}");
    }
}
