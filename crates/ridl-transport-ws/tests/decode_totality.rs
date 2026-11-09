use ridl_rt::contract::{CatalogHash, InterfaceNo, Kind, Ordinal};
use ridl_rt::error::Contract;
use ridl_rt::payload::{Rule, Violation};
use ridl_rt::sample::{Envelope, Timestamp};
use ridl_transport_ws::frame::{
    decode, AnswerOutcome, Attach, AttachOutcome, Frame, PublishState, Reason, ResponseOutcome,
    StreamOutcome, Target,
};

fn probe(bytes: &[u8]) {
    if let Ok(frame) = decode(bytes) {
        let mut out = vec![0; frame.encoded_len()];
        let len = frame
            .encode(&mut out)
            .expect("an accepted frame re-encodes");
        assert_eq!(decode(&out[..len]), Ok(frame), "input: {bytes:?}");
    }
}

fn seeds() -> Vec<Frame<'static>> {
    let signal = Target {
        kind: Kind::Signal,
        iface: InterfaceNo(1),
        ord: Ordinal(2),
    };
    let event = Target {
        kind: Kind::Event,
        ..signal
    };
    let command = Target {
        kind: Kind::Command,
        ..signal
    };
    let query = Target {
        kind: Kind::Query,
        ..signal
    };
    let fixed = Target {
        kind: Kind::Fixed,
        ..signal
    };
    let envelope = Envelope {
        stamp: Timestamp(-1),
        seq: 7,
    };
    let mut frames = vec![
        Frame::Attach(Attach::new(2, 1, CatalogHash([0; 32]), "u").unwrap()),
        Frame::Attach(Attach::new(3, 3, CatalogHash([0xff; 32]), "unit.µ").unwrap()),
        Frame::Attached(AttachOutcome::Accepted),
        Frame::Attached(AttachOutcome::Refused(Reason::CatalogMismatch)),
        Frame::Subscribe(signal),
        Frame::Unsubscribe(event),
        Frame::Read(fixed),
        Frame::Answer {
            target: signal,
            outcome: AnswerOutcome::Accepted,
        },
        Frame::Answer {
            target: event,
            outcome: AnswerOutcome::Contract(Contract::UnknownInteraction),
        },
        Frame::Occurrence {
            target: event,
            envelope,
            payload: &[0, 0xff],
        },
        Frame::Request {
            target: command,
            envelope,
            payload: &[],
        },
        Frame::Request {
            target: query,
            envelope,
            payload: &[1, 2],
        },
        Frame::Block {
            target: fixed,
            envelope,
            total: 1,
            index: 0,
            payload: &[0xff],
        },
        Frame::Block {
            target: fixed,
            envelope,
            total: 4098,
            index: 1,
            payload: &[1, 2],
        },
        Frame::Chunk {
            target: command,
            envelope,
            correlation: 7,
            stream: 0,
            payload: &[],
        },
        Frame::Chunk {
            target: query,
            envelope,
            correlation: 7,
            stream: 1,
            payload: &[0, 0xff],
        },
        Frame::StreamCredit {
            target: query,
            correlation: 7,
            stream: 1,
            credit: 4,
        },
    ];
    for state in [
        PublishState::Init,
        PublishState::Live(&[]),
        PublishState::Live(&[0, 0xff]),
        PublishState::InvalidDeclared,
    ] {
        frames.push(Frame::Publish {
            target: signal,
            envelope,
            state,
        });
    }
    let violation = Contract::InvalidValue(Violation {
        type_name: None,
        rule: Rule::Range,
    });
    for outcome in [
        ResponseOutcome::Accepted,
        ResponseOutcome::Reply(&[]),
        ResponseOutcome::Reply(&[0, 0xff]),
        ResponseOutcome::Contract(violation),
        ResponseOutcome::Corrupt,
        ResponseOutcome::Busy,
    ] {
        frames.push(Frame::Response {
            target: query,
            envelope,
            correlation: 7,
            outcome,
        });
    }
    for outcome in [
        StreamOutcome::End,
        StreamOutcome::Contract(violation),
        StreamOutcome::Corrupt,
        StreamOutcome::Busy,
        StreamOutcome::Interrupted,
    ] {
        frames.push(Frame::StreamEnd {
            target: command,
            envelope,
            correlation: 7,
            stream: 0,
            outcome,
        });
    }
    frames
}

#[test]
fn decode_is_total_over_arbitrary_bytes() {
    for len in 0..=2 {
        for n in 0..(1usize << (8 * len)) {
            probe(&n.to_le_bytes()[..len]);
        }
    }
    // Repeated and ascending bytes, with each form tag, reach header readers.
    for len in [3, 4, 7, 8, 15, 16, 31, 32, 63, 64, 127, 256] {
        for byte in [0, 0xff, 0x80, 0x55, 0xaa] {
            let mut bytes = vec![byte; len];
            probe(&bytes);
            for form in 1..=14 {
                bytes[0] = form;
                probe(&bytes);
            }
        }
        let mut bytes: Vec<_> = (0..len).map(|i| i as u8).collect();
        for form in 1..=14 {
            bytes[0] = form;
            probe(&bytes);
        }
    }
    for frame in seeds() {
        let mut bytes = vec![0; frame.encoded_len()];
        let len = frame.encode(&mut bytes).expect("the seed encodes");
        bytes.truncate(len);
        assert_eq!(decode(&bytes), Ok(frame), "the seed decodes");
        probe(&bytes);
        for end in 0..bytes.len() {
            probe(&bytes[..end]);
        }
        for at in 0..bytes.len() {
            let original = bytes[at];
            for replacement in 0..=255 {
                bytes[at] = replacement;
                probe(&bytes);
            }
            bytes[at] = original;
        }
    }
}
