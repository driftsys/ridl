use super::*;
use ridl_rt::port::ClaimId;

const UPLOAD: Target = Target {
    kind: Kind::Command,
    iface: IFACE,
    ord: Ordinal(11),
};
type UploadSession = Session<Described<Streaming<Scanning<Loopback>>>>;
struct Rig {
    s: UploadSession,
    p: Loopback,
    notes: Notes,
    now: Instant,
}

fn rig(version: u32, limit: u32) -> Rig {
    let rt = Loopback::new(CATALOG);
    let mut p = rt.attach();
    p.serve(IFACE, &[UPLOAD.ord, COMMAND.ord]).unwrap();
    let mut table = MemberTable::new(CATALOG.name, CATALOG.hash);
    table.insert(
        IFACE,
        UPLOAD.ord,
        Kind::Command,
        Some(std::time::Duration::from_secs(1)),
    );
    table.insert(IFACE, COMMAND.ord, Kind::Command, None);
    table.set_parameter_streams(IFACE, UPLOAD.ord, 1);
    table.set_limit(IFACE, UPLOAD.ord, limit);
    let notes = Notes::default();
    let mut s = Session::new(
        Described::new(Streaming(Scanning(rt)), Arc::new(table)).unwrap(),
        &config(&notes),
    );
    s.upgraded(version);
    let now = Instant::now();
    let mut out = Vec::new();
    assert_eq!(
        s.on_message(
            &attach_msg(version, 1, [7; 32], CATALOG.name),
            now,
            &mut out
        ),
        Step::Continue
    );
    Rig { s, p, notes, now }
}
fn start(r: &mut Rig, outcome: Option<Result<&[u8], CallError>>) -> ClaimId {
    let (step, out) = send_at(r, &request(UPLOAD, 1, &[]), r.now);
    assert_eq!(step, Step::Continue);
    assert!(out.is_empty());
    let claim = r.p.next_claim(&mut []).unwrap().unwrap().id;
    // Name the backend parameter before settlement so the loopback retains it.
    assert_eq!(
        r.p.pull_claim(claim, StreamNo(1), &mut [0; 8]),
        Ok(Pull::Pending)
    );
    if let Some(outcome) = outcome {
        r.p.settle(claim, outcome).unwrap();
    }
    claim
}
fn send_at(r: &mut Rig, bytes: &[u8], now: Instant) -> (Step, Vec<Vec<u8>>) {
    let mut out = Vec::new();
    let step = r.s.on_message(bytes, now, &mut out);
    (step, out)
}
fn chunk(r: &mut Rig, seq: u64, stream: u8, payload: &[u8]) -> (Step, Vec<Vec<u8>>) {
    let bytes = msg(Frame::Chunk {
        target: UPLOAD,
        envelope: Envelope {
            stamp: Timestamp(0),
            seq,
        },
        correlation: 1,
        stream,
        payload,
    });
    send_at(r, &bytes, r.now)
}
fn end(r: &mut Rig, seq: u64, outcome: StreamOutcome) -> (Step, Vec<Vec<u8>>) {
    send_at(
        r,
        &msg(Frame::StreamEnd {
            target: UPLOAD,
            envelope: Envelope {
                stamp: Timestamp(0),
                seq,
            },
            correlation: 1,
            stream: 1,
            outcome,
        }),
        r.now,
    )
}
fn poll(r: &mut Rig, now: Instant) -> (Step, Vec<Vec<u8>>) {
    let mut out = Vec::new();
    let step = r.s.tick(now, &mut out);
    (step, out)
}
fn assert_end(out: &[Vec<u8>], outcome: StreamOutcome) {
    assert_eq!(out.len(), 1);
    assert!(
        matches!(decode(&out[0]).unwrap(), Frame::StreamEnd { target: UPLOAD, envelope: Envelope { seq: 1, .. }, correlation: 1, stream: 1, outcome: actual } if actual == outcome)
    );
}
fn credits(out: &[Vec<u8>]) -> u32 {
    out.iter()
        .map(|bytes| {
            assert!(
                matches!(decode(bytes).unwrap(), Frame::StreamCredit { .. }),
                "assertion failed: expected upload credit"
            );
            match decode(bytes).unwrap() {
                Frame::StreamCredit {
                    target: UPLOAD,
                    correlation: 1,
                    stream: 1,
                    credit,
                } => credit,
                other => unreachable!("credit was checked: {other:?}"),
            }
        })
        .sum()
}
fn accepted(r: &mut Rig) {
    let (step, out) = poll(r, r.now);
    assert_eq!(step, Step::Continue);
    assert_eq!(out.len(), 1);
    assert!(matches!(
        decode(&out[0]).unwrap(),
        Frame::Response {
            correlation: 1,
            outcome: ResponseOutcome::Accepted,
            ..
        }
    ));
}
fn pull(r: &mut Rig, claim: ClaimId, expected: Pull) {
    assert_eq!(
        r.p.pull_claim(claim, StreamNo(1), &mut [0; 8]),
        Ok(expected)
    );
}

#[test]
fn a_command_upload_is_acknowledged_and_stays_open() {
    let mut r = rig(3, 4);
    let claim = start(&mut r, Some(Ok(&[])));
    accepted(&mut r);
    for n in 1..=3 {
        let (step, out) = chunk(&mut r, n, 1, &[n as u8]);
        assert_eq!(step, Step::Continue);
        assert_eq!(credits(&out), 1);
    }
    assert_eq!(end(&mut r, 4, StreamOutcome::End), (Step::Continue, vec![]));
    let mut buf = [0; 8];
    for n in 1..=3 {
        assert_eq!(
            r.p.pull_claim(claim, StreamNo(1), &mut buf),
            Ok(Pull::Chunk(1))
        );
        assert_eq!(buf[0], n);
    }
    pull(&mut r, claim, Pull::End);
    assert!(r.notes.lock().unwrap().is_empty());
}
#[test]
fn the_server_grants_one_credit_per_chunk_the_backend_takes() {
    for resume in [false, true] {
        let mut r = rig(3, 4);
        let claim = start(&mut r, Some(Ok(&[])));
        accepted(&mut r);
        let mut total = 0;
        for n in 1..=4 {
            let (step, out) = chunk(&mut r, n, 1, &[1]);
            assert_eq!(step, Step::Continue);
            total += credits(&out);
        }
        assert_eq!(total, 4);
        for n in 5..=8 {
            assert_eq!(chunk(&mut r, n, 1, &[1]), (Step::Continue, vec![]));
        }
        if resume {
            pull(&mut r, claim, Pull::Chunk(1));
            let now = r.now;
            let (step, out) = poll(&mut r, now);
            assert_eq!(step, Step::Continue);
            assert_eq!(out.len(), 1);
            assert_eq!(credits(&out), 1);
        } else {
            assert_eq!(chunk(&mut r, 9, 1, &[1]).0, Step::Close(1002));
            assert_eq!(
                *r.notes.lock().unwrap(),
                vec![Note::StreamCreditExceeded { seq: 1 }]
            );
        }
    }
}
#[test]
fn a_chunk_over_the_member_limit_is_stream_end_corrupt_and_forgotten() {
    for len in [4, 5] {
        let mut r = rig(3, 4);
        let claim = start(&mut r, Some(Ok(&[])));
        accepted(&mut r);
        let (step, out) = chunk(&mut r, 1, 1, &vec![1; len]);
        assert_eq!(step, Step::Continue);
        if len == 4 {
            assert_eq!(credits(&out), 1);
            pull(&mut r, claim, Pull::Chunk(4));
        } else {
            assert_end(&out, StreamOutcome::Corrupt);
            assert_eq!(
                *r.notes.lock().unwrap(),
                vec![Note::ChunkTooLarge {
                    seq: 1,
                    stream: 1,
                    len: 5
                }]
            );
            pull(
                &mut r,
                claim,
                Pull::Failed(CallError::Transport(Transport::Interrupted)),
            );
            r.notes.lock().unwrap().clear();
            assert_eq!(chunk(&mut r, 2, 1, &[1]), (Step::Continue, vec![]));
            assert!(r.notes.lock().unwrap().is_empty());
        }
    }
}
#[test]
fn a_provider_refusal_after_the_acknowledgment_is_stream_end_contract_on_the_upload() {
    let mut r = rig(3, 4);
    let claim = start(&mut r, Some(Ok(&[])));
    accepted(&mut r);
    for n in 1..=2 {
        assert_eq!(chunk(&mut r, n, 1, &[1]).0, Step::Continue);
        pull(&mut r, claim, Pull::Chunk(1));
    }
    let error = CallError::Contract(Contract::InvalidValue(Violation {
        type_name: Some("Block"),
        rule: Rule::Range,
    }));
    r.p.abort_claim(claim, StreamNo(1), error).unwrap();
    let (step, out) = chunk(&mut r, 3, 1, &[1]);
    assert_eq!(step, Step::Continue);
    assert_end(
        &out,
        StreamOutcome::Contract(Contract::InvalidValue(Violation {
            type_name: None,
            rule: Rule::Range,
        })),
    );
    for n in 4..=6 {
        assert_eq!(chunk(&mut r, n, 1, &[1]), (Step::Continue, vec![]));
    }
    assert_eq!(end(&mut r, 7, StreamOutcome::End), (Step::Continue, vec![]));
    assert!(r.notes.lock().unwrap().is_empty());
}
#[test]
fn a_client_stream_end_on_an_upload_is_direction_aware() {
    for outcome in [
        StreamOutcome::End,
        StreamOutcome::Interrupted,
        StreamOutcome::Contract(Contract::PreconditionFailed),
        StreamOutcome::Corrupt,
        StreamOutcome::Busy,
    ] {
        let mut r = rig(3, 4);
        let claim = start(&mut r, Some(Ok(&[])));
        accepted(&mut r);
        let (step, out) = end(&mut r, 1, outcome);
        assert!(out.is_empty());
        match outcome {
            StreamOutcome::End => {
                assert_eq!(step, Step::Continue);
                pull(&mut r, claim, Pull::End);
            }
            StreamOutcome::Interrupted => {
                assert_eq!(step, Step::Continue);
                pull(
                    &mut r,
                    claim,
                    Pull::Failed(CallError::Transport(Transport::Interrupted)),
                );
            }
            _ => {
                assert_eq!(step, Step::Close(1002));
                assert_eq!(
                    *r.notes.lock().unwrap(),
                    vec![Note::Violation(ProtocolViolation::OutcomeNotAllowed)]
                );
            }
        }
    }
}
#[test]
fn a_chunk_for_a_call_not_in_flight_is_dropped_without_a_note() {
    let mut r = rig(3, 4);
    let unknown = msg(Frame::Chunk {
        target: UPLOAD,
        envelope: Envelope {
            stamp: Timestamp(0),
            seq: 1,
        },
        correlation: 99,
        stream: 1,
        payload: &[1],
    });
    let now = r.now;
    assert_eq!(send_at(&mut r, &unknown, now), (Step::Continue, vec![]));
    start(
        &mut r,
        Some(Err(CallError::Contract(Contract::PreconditionFailed))),
    );
    poll(&mut r, now);
    assert_eq!(chunk(&mut r, 1, 1, &[1]), (Step::Continue, vec![]));
    assert!(r.notes.lock().unwrap().is_empty());
}
#[test]
fn a_chunk_on_a_stream_the_member_does_not_declare_closes_1002() {
    for (target, stream) in [(UPLOAD, 2), (COMMAND, 1)] {
        let mut r = rig(3, 4);
        let now = r.now;
        assert_eq!(
            send_at(&mut r, &request(target, 1, &[]), now).0,
            Step::Continue
        );
        assert_eq!(chunk(&mut r, 1, stream, &[1]).0, Step::Close(1002));
        assert_eq!(
            *r.notes.lock().unwrap(),
            vec![Note::StreamCreditExceeded { seq: 1 }]
        );
    }
}
#[test]
fn a_chunk_seq_gap_interrupts_the_upload() {
    let mut r = rig(3, 4);
    let claim = start(&mut r, Some(Ok(&[])));
    accepted(&mut r);
    assert_eq!(chunk(&mut r, 1, 1, &[1]).0, Step::Continue);
    pull(&mut r, claim, Pull::Chunk(1));
    let (step, out) = chunk(&mut r, 3, 1, &[3]);
    assert_eq!(step, Step::Continue);
    assert_end(&out, StreamOutcome::Interrupted);
    pull(
        &mut r,
        claim,
        Pull::Failed(CallError::Transport(Transport::Interrupted)),
    );
    assert!(r.notes.lock().unwrap().is_empty());
}
#[test]
fn an_idle_open_upload_is_abandoned_interrupted() {
    let mut r = rig(3, 4);
    start(&mut r, Some(Ok(&[])));
    accepted(&mut r);
    let t0 = r.now;
    r.now = t0 + std::time::Duration::from_secs(50);
    assert_eq!(chunk(&mut r, 1, 1, &[1]).0, Step::Continue);
    assert_eq!(
        poll(&mut r, t0 + std::time::Duration::from_secs(60)),
        (Step::Continue, vec![])
    );
    let (step, out) = poll(&mut r, t0 + std::time::Duration::from_secs(110));
    assert_eq!(step, Step::Continue);
    assert_end(&out, StreamOutcome::Interrupted);
    assert_eq!(
        *r.notes.lock().unwrap(),
        vec![Note::CallAbandoned { seq: 1 }]
    );
}
#[test]
fn a_refusing_acknowledgment_forgets_the_call_and_sends_no_stream_end() {
    for queued in [false, true] {
        let mut r = rig(3, 4);
        let claim = start(&mut r, None);
        if queued {
            for seq in 1..=5 {
                assert_eq!(chunk(&mut r, seq, 1, &[1]).0, Step::Continue);
            }
        }
        r.p.settle(
            claim,
            Err(CallError::Contract(Contract::PreconditionFailed)),
        )
        .unwrap();
        let now = r.now;
        let (step, out) = poll(&mut r, now);
        assert_eq!(step, Step::Continue);
        assert_eq!(
            out.len(),
            1,
            "a refusing response covers queued uploads too"
        );
        assert!(matches!(
            decode(&out[0]).unwrap(),
            Frame::Response {
                outcome: ResponseOutcome::Contract(Contract::PreconditionFailed),
                ..
            }
        ));
        assert!(r.s.calls.in_flight().is_empty());
    }
}

#[test]
fn the_end_of_a_session_interrupts_an_open_upload() {
    // Keep the caller handle alive after close so its Drop cannot hide a missing forget.
    let mut rt = Loopback::new(CATALOG);
    let mut p = rt.attach();
    p.serve(IFACE, &[UPLOAD.ord]).unwrap();
    let mut table = MemberTable::new(CATALOG.name, CATALOG.hash);
    table.insert(IFACE, UPLOAD.ord, Kind::Command, None);
    table.set_parameter_streams(IFACE, UPLOAD.ord, 1);
    let mut s = Session::new(
        Described::new(Streaming(Scanning(&mut rt)), Arc::new(table)).unwrap(),
        &Config::default(),
    );
    s.upgraded(3);
    assert_eq!(
        send(&mut s, &attach_msg(3, 1, [7; 32], CATALOG.name)).0,
        Step::Continue
    );
    assert_eq!(send(&mut s, &request(UPLOAD, 1, &[])).0, Step::Continue);
    let claim = p.next_claim(&mut []).unwrap().unwrap().id;
    assert_eq!(p.pull_claim(claim, StreamNo(1), &mut []), Ok(Pull::Pending));
    p.settle(claim, Ok(&[])).unwrap();
    let (_, out) = tick(&mut s);
    assert_eq!(out.len(), 1);
    s.close();
    assert_eq!(
        p.pull_claim(claim, StreamNo(1), &mut []),
        Ok(Pull::Failed(CallError::Transport(Transport::Interrupted)))
    );
}

#[test]
fn a_retransmitted_upload_request_is_dropped_and_the_upload_continues() {
    let mut r = rig(3, 4);
    let claim = start(&mut r, Some(Ok(&[])));
    accepted(&mut r);
    assert_eq!(chunk(&mut r, 1, 1, &[1]).0, Step::Continue);
    pull(&mut r, claim, Pull::Chunk(1));
    let now = r.now;
    assert_eq!(
        send_at(&mut r, &request(UPLOAD, 1, &[]), now),
        (Step::Continue, vec![])
    );
    assert_eq!(
        *r.notes.lock().unwrap(),
        vec![Note::DuplicateDropped { seq: 1 }]
    );
    assert_eq!(chunk(&mut r, 2, 1, &[2]).0, Step::Continue);
    pull(&mut r, claim, Pull::Chunk(1));
}
#[test]
fn a_v2_session_answers_an_upload_member_unknown_interaction() {
    let mut r = rig(2, 4);
    let now = r.now;
    let (step, out) = send_at(&mut r, &request(UPLOAD, 1, &[]), now);
    assert_eq!(step, Step::Continue);
    assert_eq!(out.len(), 1);
    assert!(matches!(
        decode(&out[0]).unwrap(),
        Frame::Response {
            outcome: ResponseOutcome::Contract(Contract::UnknownInteraction),
            ..
        }
    ));
    assert_eq!(
        *r.notes.lock().unwrap(),
        vec![Note::UnknownInteraction { target: UPLOAD }]
    );
    assert!(r.p.next_claim(&mut []).unwrap().is_none());
}
