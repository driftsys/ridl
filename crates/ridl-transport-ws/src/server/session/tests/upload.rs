use super::*;
use ridl_rt::port::ClaimId;

const UPLOAD: Target = Target {
    kind: Kind::Command,
    iface: IFACE,
    ord: Ordinal(11),
};
type UploadSession = Session<Described<Streaming<FaultyBackend>>>;
struct Rig {
    s: UploadSession,
    p: Loopback,
    notes: Notes,
    now: Instant,
    fail: Rc<Cell<Fail>>,
    target: Target,
}

fn rig(version: u32, limit: u32) -> Rig {
    rig_with_streams(version, limit, 1)
}

fn rig_with_streams(version: u32, limit: u32, count: u8) -> Rig {
    rig_target(version, limit, count, UPLOAD)
}

fn rig_target(version: u32, limit: u32, count: u8, target: Target) -> Rig {
    let rt = Loopback::new(CATALOG);
    let mut p = rt.attach();
    p.serve(IFACE, &[target.ord, COMMAND.ord]).unwrap();
    let mut table = MemberTable::new(CATALOG.name, CATALOG.hash);
    table.insert(
        IFACE,
        target.ord,
        target.kind,
        Some(std::time::Duration::from_secs(1)),
    );
    table.insert(IFACE, COMMAND.ord, Kind::Command, None);
    table.set_parameter_streams(IFACE, target.ord, count);
    table.set_limit(IFACE, target.ord, limit);
    if target == DUPLEX {
        table.set_reply_stream(IFACE, target.ord);
    }
    let notes = Notes::default();
    let fail = Rc::new(Cell::new(Fail::None));
    let backend = FaultyBackend(Faulty {
        inner: rt,
        fail: Rc::clone(&fail),
        forgotten: Rc::default(),
        aborted: Rc::default(),
    });
    let mut s = Session::new(
        Described::new(Streaming(backend), Arc::new(table)).unwrap(),
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
    Rig {
        s,
        p,
        notes,
        now,
        fail,
        target,
    }
}
fn start(r: &mut Rig, outcome: Option<Result<&[u8], CallError>>) -> ClaimId {
    let (step, out) = send_at(r, &request(r.target, 1, &[]), r.now);
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
        target: r.target,
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
            target: r.target,
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
        let frame = msg(Frame::Chunk {
            target,
            envelope: Envelope {
                stamp: Timestamp(0),
                seq: 1,
            },
            correlation: 1,
            stream,
            payload: &[1],
        });
        assert_eq!(send_at(&mut r, &frame, now).0, Step::Close(1002));
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

/// Mutation: accept an arbitrary sequence number in `Session::end_upload`.
#[test]
fn an_upload_end_seq_gap_interrupts_the_provider_and_the_client() {
    let mut r = rig(3, 4);
    let claim = start(&mut r, None);
    assert_eq!(credits(&chunk(&mut r, 1, 1, &[1]).1), 1);
    pull(&mut r, claim, Pull::Chunk(1));
    let (step, out) = end(&mut r, 3, StreamOutcome::End);
    assert_eq!(step, Step::Continue);
    assert_end(&out, StreamOutcome::Interrupted);
    pull(
        &mut r,
        claim,
        Pull::Failed(CallError::Transport(Transport::Interrupted)),
    );
    assert!(r.notes.lock().unwrap().is_empty());
}

/// Mutation: skip `interrupt_uploads` before a non-refusing forget.
#[test]
fn a_non_refusing_command_outcome_interrupts_its_open_upload() {
    let mut r = rig(3, 4);
    let claim = start(&mut r, None);
    // Inject only the caller's outcome, keeping the provider's upload open.
    r.fail.set(Fail::AckTimeout);
    let now = r.now;
    let (step, out) = poll(&mut r, now);
    assert_eq!(step, Step::Continue);
    assert_end(&out, StreamOutcome::Interrupted);
    pull(
        &mut r,
        claim,
        Pull::Failed(CallError::Transport(Transport::Interrupted)),
    );
    assert!(r.s.calls.in_flight().is_empty());
    assert!(r.notes.lock().unwrap().is_empty());
}

/// Mutation: remove `Busy` from either refusing-outcome check in `poll_calls`.
#[test]
fn a_busy_command_outcome_covers_open_and_queued_uploads() {
    for queued in [false, true] {
        let mut r = rig(3, 4);
        let claim = start(&mut r, None);
        if queued {
            for seq in 1..=5 {
                assert_eq!(chunk(&mut r, seq, 1, &[1]).0, Step::Continue);
            }
            pull(&mut r, claim, Pull::Chunk(1));
        }
        r.fail.set(Fail::AckBusy);
        let now = r.now;
        let (step, out) = poll(&mut r, now);
        assert_eq!(step, Step::Continue);
        assert_eq!(
            out.len(),
            1,
            "busy covers the whole call without upload credit or end"
        );
        assert!(matches!(
            decode(&out[0]).unwrap(),
            Frame::Response {
                target: UPLOAD,
                correlation: 1,
                outcome: ResponseOutcome::Busy,
                ..
            }
        ));
        assert!(r.s.calls.in_flight().is_empty());
        assert!(r.notes.lock().unwrap().is_empty());
    }
}

/// Mutation: skip the backend abort for a client `stream_end interrupted`.
#[test]
fn a_client_interrupt_before_acknowledgment_aborts_the_upload() {
    let mut r = rig(3, 4);
    let claim = start(&mut r, None);
    assert_eq!(
        end(&mut r, 1, StreamOutcome::Interrupted),
        (Step::Continue, vec![])
    );
    pull(
        &mut r,
        claim,
        Pull::Failed(CallError::Transport(Transport::Interrupted)),
    );
    // The acknowledgment is still pending, so forgetting cannot hide a missing abort.
    r.p.settle(claim, Ok(&[])).unwrap();
    accepted(&mut r);
    assert!(r.notes.lock().unwrap().is_empty());
}

/// Mutation: a drain does not restart the upload's activity clock.
#[test]
fn backend_drain_progress_restarts_the_upload_abandonment_clock() {
    let mut r = rig(3, 4);
    let claim = start(&mut r, Some(Ok(&[])));
    accepted(&mut r);
    for seq in 1..=5 {
        assert_eq!(chunk(&mut r, seq, 1, &[1]).0, Step::Continue);
    }
    let t0 = r.now;
    pull(&mut r, claim, Pull::Chunk(1));
    let (step, out) = poll(&mut r, t0 + std::time::Duration::from_secs(50));
    assert_eq!(step, Step::Continue);
    assert_eq!(credits(&out), 1);
    assert_eq!(
        poll(&mut r, t0 + std::time::Duration::from_secs(100)),
        (Step::Continue, vec![])
    );
    assert!(r.notes.lock().unwrap().is_empty());
}

/// Mutation: forget drained uploads even while the acknowledgment is pending.
#[test]
fn uploads_drained_before_settlement_still_receive_the_acknowledgment() {
    let mut r = rig(3, 4);
    let claim = start(&mut r, None);
    assert_eq!(credits(&chunk(&mut r, 1, 1, &[1]).1), 1);
    assert_eq!(end(&mut r, 2, StreamOutcome::End), (Step::Continue, vec![]));
    pull(&mut r, claim, Pull::Chunk(1));
    pull(&mut r, claim, Pull::End);
    let now = r.now;
    assert_eq!(poll(&mut r, now), (Step::Continue, vec![]));
    r.p.settle(claim, Ok(&[])).unwrap();
    accepted(&mut r);
    assert!(r.notes.lock().unwrap().is_empty());
}

/// Mutation: an oversized chunk does not interrupt the other open upload.
#[test]
fn an_oversized_chunk_interrupts_the_other_parameter_stream() {
    let mut r = rig_with_streams(3, 4, 2);
    let claim = start(&mut r, None);
    assert_eq!(
        r.p.pull_claim(claim, StreamNo(2), &mut []),
        Ok(Pull::Pending)
    );
    let (step, out) = chunk(&mut r, 1, 1, &[1; 5]);
    assert_eq!(step, Step::Continue);
    assert_eq!(
        out.len(),
        2,
        "both uploads must receive their terminal outcome"
    );
    for (bytes, stream, outcome) in [
        (&out[0], 1, StreamOutcome::Corrupt),
        (&out[1], 2, StreamOutcome::Interrupted),
    ] {
        assert!(matches!(decode(bytes).unwrap(), Frame::StreamEnd {
            target: UPLOAD, envelope: Envelope { seq: 1, .. }, correlation: 1,
            stream: actual_stream, outcome: actual_outcome
        } if actual_stream == stream && actual_outcome == outcome));
    }
    for stream in [1, 2] {
        assert_eq!(
            r.p.pull_claim(claim, StreamNo(stream), &mut []),
            Ok(Pull::Failed(CallError::Transport(Transport::Interrupted)))
        );
    }
    assert_eq!(
        *r.notes.lock().unwrap(),
        vec![Note::ChunkTooLarge {
            seq: 1,
            stream: 1,
            len: 5
        }]
    );
}

/// Mutation: finish the call when any upload ends rather than all of them.
#[test]
fn ending_one_of_two_uploads_keeps_the_other_open() {
    let mut r = rig_with_streams(3, 4, 2);
    let claim = start(&mut r, None);
    assert_eq!(
        r.p.pull_claim(claim, StreamNo(2), &mut []),
        Ok(Pull::Pending)
    );
    r.p.settle(claim, Ok(&[])).unwrap();
    accepted(&mut r);
    assert_eq!(end(&mut r, 1, StreamOutcome::End), (Step::Continue, vec![]));
    pull(&mut r, claim, Pull::End);
    let (step, out) = chunk(&mut r, 1, 2, &[2]);
    assert_eq!(step, Step::Continue);
    assert_eq!(out.len(), 1, "the second upload must still return credit");
    assert!(matches!(
        decode(&out[0]).unwrap(),
        Frame::StreamCredit {
            target: UPLOAD,
            correlation: 1,
            stream: 2,
            credit: 1
        }
    ));
    let mut buf = [0; 1];
    assert_eq!(
        r.p.pull_claim(claim, StreamNo(2), &mut buf),
        Ok(Pull::Chunk(1))
    );
    assert_eq!(buf, [2]);
    let now = r.now;
    let frame = msg(Frame::StreamEnd {
        target: UPLOAD,
        envelope: Envelope {
            stamp: Timestamp(0),
            seq: 2,
        },
        correlation: 1,
        stream: 2,
        outcome: StreamOutcome::End,
    });
    assert_eq!(send_at(&mut r, &frame, now), (Step::Continue, vec![]));
    assert_eq!(r.p.pull_claim(claim, StreamNo(2), &mut []), Ok(Pull::End));
    assert!(r.notes.lock().unwrap().is_empty());
}

/// Mutation: abandon a pending call from `sent` while an upload is active.
#[test]
fn active_upload_chunks_keep_an_unacknowledged_command_alive_past_sent_bound() {
    let mut r = rig(3, 4);
    let claim = start(&mut r, None);
    let t0 = r.now;
    for (seq, seconds) in [(1, 50), (2, 100), (3, 150)] {
        r.now = t0 + std::time::Duration::from_secs(seconds);
        assert_eq!(credits(&chunk(&mut r, seq, 1, &[1]).1), 1);
        pull(&mut r, claim, Pull::Chunk(1));
        let now = r.now;
        assert_eq!(poll(&mut r, now), (Step::Continue, vec![]));
        assert!(r.notes.lock().unwrap().is_empty());
    }
    r.p.settle(claim, Ok(&[])).unwrap();
    accepted(&mut r);
}

fn mismatched_targets(target: Target) -> [Target; 3] {
    [
        Target {
            kind: Kind::Query,
            ..target
        },
        Target {
            iface: InterfaceNo(99),
            ..target
        },
        Target {
            ord: Ordinal(99),
            ..target
        },
    ]
}

/// Mutation: route a chunk by correlation without checking its full target.
#[test]
fn a_mismatched_chunk_reaches_no_provider_and_consumes_no_credit() {
    for target in mismatched_targets(UPLOAD) {
        let mut r = rig(3, 4);
        let claim = start(&mut r, None);
        let frame = msg(Frame::Chunk {
            target,
            envelope: Envelope {
                stamp: Timestamp(0),
                seq: 1,
            },
            correlation: 1,
            stream: 1,
            payload: &[9],
        });
        let now = r.now;
        assert_eq!(send_at(&mut r, &frame, now), (Step::Continue, vec![]));
        pull(&mut r, claim, Pull::Pending);
        assert_eq!(
            *r.notes.lock().unwrap(),
            vec![Note::UnknownInteraction { target }]
        );
        // Four backend slots plus four queued chunks remain available.
        for seq in 1..=8 {
            let (step, out) = chunk(&mut r, seq, 1, &[1]);
            assert_eq!(step, Step::Continue);
            assert_eq!(credits(&out), if seq <= 4 { 1 } else { 0 });
        }
        assert_eq!(chunk(&mut r, 9, 1, &[1]).0, Step::Close(1002));
    }
}

/// Mutation: route a stream end by correlation without checking its target.
#[test]
fn a_mismatched_stream_end_does_not_end_an_upload() {
    for target in mismatched_targets(UPLOAD) {
        let mut r = rig(3, 4);
        let claim = start(&mut r, None);
        let frame = msg(Frame::StreamEnd {
            target,
            envelope: Envelope {
                stamp: Timestamp(0),
                seq: 1,
            },
            correlation: 1,
            stream: 1,
            outcome: StreamOutcome::End,
        });
        let now = r.now;
        assert_eq!(send_at(&mut r, &frame, now), (Step::Continue, vec![]));
        pull(&mut r, claim, Pull::Pending);
        assert_eq!(
            *r.notes.lock().unwrap(),
            vec![Note::UnknownInteraction { target }]
        );
        assert_eq!(credits(&chunk(&mut r, 1, 1, &[1]).1), 1);
        pull(&mut r, claim, Pull::Chunk(1));
        assert_eq!(end(&mut r, 2, StreamOutcome::End), (Step::Continue, vec![]));
        pull(&mut r, claim, Pull::End);
    }
}

const UPLOAD_QUERY: Target = Target {
    kind: Kind::Query,
    iface: IFACE,
    ord: Ordinal(12),
};
const DUPLEX: Target = Target {
    kind: Kind::Query,
    iface: IFACE,
    ord: Ordinal(14),
};

fn query_rig(target: Target) -> Rig {
    rig_target(3, 16_128, 1, target)
}

fn query_end(r: &mut Rig, seq: u64, stamp: Timestamp) -> (Step, Vec<Vec<u8>>) {
    send_at(
        r,
        &msg(Frame::StreamEnd {
            target: r.target,
            envelope: Envelope { stamp, seq },
            correlation: 1,
            stream: 1,
            outcome: StreamOutcome::End,
        }),
        r.now,
    )
}
fn check_upload_end(bytes: &[u8], target: Target, outcome: StreamOutcome) {
    assert!(matches!(decode(bytes).unwrap(), Frame::StreamEnd {
        target: actual_target, envelope: Envelope { seq: 1, .. },
        correlation: 1, stream: 1, outcome: actual_outcome,
    } if actual_target == target && actual_outcome == outcome));
}
fn check_reply(out: &[Vec<u8>], payload: &[u8]) {
    assert_eq!(out.len(), 1);
    assert!(matches!(decode(&out[0]).unwrap(), Frame::Response {
        target: UPLOAD_QUERY, correlation: 1, outcome: ResponseOutcome::Reply(p), ..
    } if p == payload));
}

// Mutation: forget a drained upload before its query settles.
#[test]
fn a_query_upload_replies_after_the_end() {
    let mut r = query_rig(UPLOAD_QUERY);
    let claim = start(&mut r, None);
    chunk(&mut r, 1, 1, &[9]);
    let now = r.now;
    assert_eq!(poll(&mut r, now), (Step::Continue, vec![]));
    assert_eq!(end(&mut r, 2, StreamOutcome::End), (Step::Continue, vec![]));
    pull(&mut r, claim, Pull::Chunk(1));
    pull(&mut r, claim, Pull::End);
    r.p.settle(claim, Ok(&[9, 8])).unwrap();
    check_reply(&poll(&mut r, now).1, &[9, 8]);
    assert!(r.s.calls.in_flight().is_empty());

    // A provider may also settle early; that reply must stop the open upload.
    let mut r = query_rig(UPLOAD_QUERY);
    let claim = start(&mut r, None);
    r.p.settle(claim, Ok(&[9, 8])).unwrap();
    let now = r.now;
    let out = poll(&mut r, now).1;
    assert_eq!(out.len(), 2);
    check_reply(&out[..1], &[9, 8]);
    check_upload_end(&out[1], UPLOAD_QUERY, StreamOutcome::Interrupted);
}

// Mutation: retain the request stamp or receipt instead of the end's.
#[test]
fn a_query_upload_retention_runs_from_the_end() {
    for future_stamp in [false, true] {
        let mut r = query_rig(UPLOAD_QUERY);
        let claim = start(&mut r, None);
        r.p.advance(ridl_rt::sample::Duration(5_000_000));
        let stamp = Timestamp(if future_stamp { 50_000_000 } else { 5_000_000 });
        assert_eq!(query_end(&mut r, 1, stamp), (Step::Continue, vec![]));
        r.p.settle(claim, Ok(&[4])).unwrap();
        let now = r.now;
        let response = poll(&mut r, now).1;
        check_reply(&response, &[4]);
        r.p.advance(ridl_rt::sample::Duration(500_000));
        assert_eq!(
            send_at(&mut r, &request(UPLOAD_QUERY, 1, &[]), now).1,
            response
        );
        r.p.advance(ridl_rt::sample::Duration(600_000));
        assert!(send_at(&mut r, &request(UPLOAD_QUERY, 1, &[]), now)
            .1
            .is_empty());
        assert_eq!(
            *r.notes.lock().unwrap(),
            vec![Note::DuplicateDropped { seq: 1 }]
        );
    }
}

// Mutation: drop the plain-reply guard on the upload-end clock reset.
#[test]
fn a_duplex_upload_keeps_the_request_clocks_and_retention() {
    let mut r = query_rig(DUPLEX);
    let claim = start(&mut r, None);
    let request_call = r.s.calls.in_flight().pop().unwrap();
    r.p.advance(ridl_rt::sample::Duration(5_000_000));
    r.now += std::time::Duration::from_secs(5);
    assert_eq!(
        query_end(&mut r, 1, Timestamp(5_000_000)),
        (Step::Continue, vec![])
    );
    let call = r.s.calls.in_flight().pop().unwrap();
    pull(&mut r, claim, Pull::End);
    r.p.end_claim(claim, REPLY_STREAM).unwrap();
    let now = r.now;
    let out = poll(&mut r, now).1;
    assert_eq!(out.len(), 1);
    assert!(matches!(
        decode(&out[0]).unwrap(),
        Frame::StreamEnd {
            stream: 0,
            outcome: StreamOutcome::End,
            ..
        }
    ));
    r.p.advance(ridl_rt::sample::Duration(500_000));
    assert_eq!(
        send_at(&mut r, &request(DUPLEX, 1, &[]), now),
        (Step::Continue, vec![]),
        "duplex retention expires from the request"
    );
    assert_eq!(
        *r.notes.lock().unwrap(),
        vec![Note::DuplicateDropped { seq: 1 }]
    );
    assert_eq!(
        call.sent, request_call.sent,
        "duplex sent stays at the request"
    );
    assert_eq!(
        call.stamp, request_call.stamp,
        "duplex stamp stays at the request"
    );
    assert_eq!(
        call.receipt, request_call.receipt,
        "duplex receipt stays at the request"
    );
}

// Mutation: measure abandonment from the request's sent instant.
#[test]
fn a_query_upload_abandon_runs_from_the_end() {
    let mut r = query_rig(UPLOAD_QUERY);
    let claim = start(&mut r, None);
    let t0 = r.now;
    for seq in 1..=2 {
        r.now = t0 + std::time::Duration::from_secs(seq * 50);
        chunk(&mut r, seq, 1, &[1]);
        pull(&mut r, claim, Pull::Chunk(1));
        let now = r.now;
        assert_eq!(poll(&mut r, now), (Step::Continue, vec![]));
    }
    end(&mut r, 3, StreamOutcome::End);
    assert_eq!(
        poll(&mut r, t0 + std::time::Duration::from_secs(159)),
        (Step::Continue, vec![])
    );
    assert!(r.notes.lock().unwrap().is_empty());
    assert_eq!(
        poll(&mut r, t0 + std::time::Duration::from_secs(160)),
        (Step::Continue, vec![])
    );
    assert_eq!(
        *r.notes.lock().unwrap(),
        vec![Note::CallAbandoned { seq: 1 }]
    );
    assert!(r.s.calls.in_flight().is_empty());
}

// Mutation: forget the query when its upload is refused.
#[test]
fn a_query_upload_refusal_reaches_the_upload_and_the_reply() {
    let mut r = query_rig(UPLOAD_QUERY);
    let claim = start(&mut r, None);
    let error = CallError::Contract(Contract::InvalidValue(Violation {
        type_name: Some("Text"),
        rule: Rule::Utf8,
    }));
    r.p.abort_claim(claim, StreamNo(1), error).unwrap();
    r.p.settle(claim, Err(error)).unwrap();
    let (step, out) = chunk(&mut r, 1, 1, &[0xff]);
    assert_eq!(step, Step::Continue);
    assert_eq!(out.len(), 1);
    let contract = Contract::InvalidValue(Violation {
        type_name: None,
        rule: Rule::Utf8,
    });
    check_upload_end(&out[0], UPLOAD_QUERY, StreamOutcome::Contract(contract));
    assert_eq!(out[0][36], 7);
    let now = r.now;
    let (step, out) = poll(&mut r, now);
    assert_eq!(step, Step::Continue);
    assert_eq!(out.len(), 1);
    assert!(matches!(decode(&out[0]).unwrap(), Frame::Response {
        outcome: ResponseOutcome::Contract(c), .. } if c == contract));
    assert_eq!(*out[0].last().unwrap(), 7);
    assert!(r.s.calls.in_flight().is_empty());
}

// Mutation: classify a successful reply as a refusal when forgetting.
#[test]
fn a_query_upload_reply_before_the_end_interrupts_the_upload() {
    let mut r = query_rig(UPLOAD_QUERY);
    let claim = start(&mut r, None);
    r.p.settle(claim, Ok(&[7])).unwrap();
    let now = r.now;
    let (step, out) = poll(&mut r, now);
    assert_eq!(step, Step::Continue);
    assert_eq!(out.len(), 2);
    check_reply(&out[..1], &[7]);
    check_upload_end(&out[1], UPLOAD_QUERY, StreamOutcome::Interrupted);
    assert!(r.s.calls.in_flight().is_empty());
}

// Mutation: wait for the upload end before polling the reply stream.
#[test]
fn a_duplex_reply_starts_before_the_upload_ends() {
    let mut r = query_rig(DUPLEX);
    let claim = start(&mut r, None);
    chunk(&mut r, 1, 1, &[9]);
    let mut buf = [0; 8];
    assert_eq!(
        r.p.pull_claim(claim, StreamNo(1), &mut buf),
        Ok(Pull::Chunk(1))
    );
    assert_eq!(buf[0], 9);
    r.p.push_claim(claim, REPLY_STREAM, &buf[..1]).unwrap();
    let now = r.now;
    let (step, out) = poll(&mut r, now);
    assert_eq!(step, Step::Continue);
    assert_eq!(out.len(), 1);
    assert!(matches!(
        decode(&out[0]).unwrap(),
        Frame::Chunk {
            target: DUPLEX,
            correlation: 1,
            stream: 0,
            envelope: Envelope { seq: 1, .. },
            payload: [9],
            ..
        }
    ));
    assert_eq!(r.s.calls.in_flight().len(), 1);
}

// Mutation: forget the duplex query without interrupting its open upload.
#[test]
fn a_duplex_reply_end_with_the_upload_open_interrupts_the_upload() {
    let mut r = query_rig(DUPLEX);
    let claim = start(&mut r, None);
    r.p.end_claim(claim, REPLY_STREAM).unwrap();
    let now = r.now;
    let (step, out) = poll(&mut r, now);
    assert_eq!(step, Step::Continue);
    assert_eq!(out.len(), 2);
    assert!(matches!(
        decode(&out[0]).unwrap(),
        Frame::StreamEnd {
            target: DUPLEX,
            correlation: 1,
            stream: 0,
            outcome: StreamOutcome::End,
            ..
        }
    ));
    check_upload_end(&out[1], DUPLEX, StreamOutcome::Interrupted);
    assert!(r.s.calls.in_flight().is_empty());
}

// Mutation: accept a wrong upload end sequence in either query form.
#[test]
fn query_upload_end_gaps_interrupt_both_query_forms() {
    for target in [UPLOAD_QUERY, DUPLEX] {
        let mut r = query_rig(target);
        let claim = start(&mut r, None);
        chunk(&mut r, 1, 1, &[1]);
        pull(&mut r, claim, Pull::Chunk(1));
        let (step, out) = end(&mut r, 3, StreamOutcome::End);
        assert_eq!(step, Step::Continue);
        assert_eq!(out.len(), 1);
        check_upload_end(&out[0], target, StreamOutcome::Interrupted);
        pull(
            &mut r,
            claim,
            Pull::Failed(CallError::Transport(Transport::Interrupted)),
        );
    }
}

// Mutation: forget the duplex query when its upload ends.
#[test]
fn a_duplex_reply_can_end_after_the_upload() {
    let mut r = query_rig(DUPLEX);
    let claim = start(&mut r, None);
    assert_eq!(end(&mut r, 1, StreamOutcome::End), (Step::Continue, vec![]));
    pull(&mut r, claim, Pull::End);
    assert_eq!(r.p.push_claim(claim, REPLY_STREAM, &[3]), Ok(1));
    r.p.end_claim(claim, REPLY_STREAM).unwrap();
    let now = r.now;
    let out = poll(&mut r, now).1;
    assert_eq!(out.len(), 2);
    assert!(matches!(
        decode(&out[0]).unwrap(),
        Frame::Chunk {
            stream: 0,
            payload: [3],
            ..
        }
    ));
    assert!(matches!(
        decode(&out[1]).unwrap(),
        Frame::StreamEnd {
            stream: 0,
            outcome: StreamOutcome::End,
            ..
        }
    ));
    assert!(r.s.calls.in_flight().is_empty());
}

// Mutation: do not restart upload activity on receipt or backend drain.
#[test]
fn progress_on_query_uploads_restarts_abandonment() {
    for target in [UPLOAD_QUERY, DUPLEX] {
        for drain in [false, true] {
            let mut r = query_rig(target);
            let claim = start(&mut r, None);
            let t0 = r.now;
            for seq in 1..=4 {
                chunk(&mut r, seq, 1, &[1]);
            }
            if drain {
                chunk(&mut r, 5, 1, &[1]);
                pull(&mut r, claim, Pull::Chunk(1));
            }
            r.now = t0 + std::time::Duration::from_secs(50);
            if !drain {
                chunk(&mut r, 5, 1, &[1]);
            }
            let now = r.now;
            poll(&mut r, now);
            assert_eq!(
                poll(&mut r, t0 + std::time::Duration::from_secs(100)),
                (Step::Continue, vec![])
            );
            assert!(r.notes.lock().unwrap().is_empty());
            let out = poll(&mut r, t0 + std::time::Duration::from_secs(110)).1;
            if target == DUPLEX {
                check_duplex_abandoned(&mut r, &out);
            } else {
                assert_eq!(out.len(), 1);
                check_upload_end(&out[0], target, StreamOutcome::Interrupted);
                assert_eq!(
                    *r.notes.lock().unwrap(),
                    vec![Note::CallAbandoned { seq: 1 }]
                );
                assert!(r.s.calls.in_flight().is_empty());
            }
        }
    }
}

fn check_duplex_abandoned(r: &mut Rig, out: &[Vec<u8>]) {
    assert_eq!(out.len(), 2, "abandonment ends both duplex streams");
    assert!(matches!(
        decode(&out[0]).unwrap(),
        Frame::StreamEnd {
            target: DUPLEX,
            correlation: 1,
            stream: 0,
            outcome: StreamOutcome::Interrupted,
            ..
        }
    ));
    check_upload_end(&out[1], DUPLEX, StreamOutcome::Interrupted);
    assert!(r.s.calls.in_flight().is_empty());
    assert_eq!(
        *r.notes.lock().unwrap(),
        vec![Note::CallAbandoned { seq: 1 }]
    );
    let now = r.now;
    assert_eq!(poll(r, now), (Step::Continue, vec![]));
    assert_eq!(
        *r.notes.lock().unwrap(),
        vec![Note::CallAbandoned { seq: 1 }]
    );
}

// Mutation: use only the reply clock for duplex abandonment.
#[test]
fn duplex_upload_progress_keeps_an_idle_reply_alive() {
    for progress in ["receive", "drain", "end"] {
        let mut r = query_rig(DUPLEX);
        let claim = start(&mut r, None);
        let t0 = r.now;
        for seq in 1..=4 {
            chunk(&mut r, seq, 1, &[1]);
        }
        if progress == "drain" {
            chunk(&mut r, 5, 1, &[1]);
            pull(&mut r, claim, Pull::Chunk(1));
        }
        r.now = t0 + std::time::Duration::from_secs(50);
        match progress {
            "receive" => {
                chunk(&mut r, 5, 1, &[1]);
            }
            "end" => {
                end(&mut r, 5, StreamOutcome::End);
            }
            _ => {}
        }
        let now = r.now;
        assert_eq!(poll(&mut r, now).0, Step::Continue);
        assert_eq!(
            poll(&mut r, t0 + std::time::Duration::from_secs(100)),
            (Step::Continue, vec![]),
            "upload {progress} keeps the reply alive"
        );
        assert_eq!(r.s.calls.in_flight().len(), 1);
        assert!(r.notes.lock().unwrap().is_empty());
    }
}

// Mutation: use only the upload clock for duplex abandonment.
#[test]
fn duplex_reply_progress_keeps_an_idle_upload_open() {
    for credit in [false, true] {
        let mut r = query_rig(DUPLEX);
        let claim = start(&mut r, None);
        let t0 = r.now;
        r.now = t0 + std::time::Duration::from_secs(50);
        let now = r.now;
        if credit {
            let frame = msg(Frame::StreamCredit {
                target: DUPLEX,
                correlation: 1,
                stream: 0,
                credit: 1,
            });
            assert_eq!(send_at(&mut r, &frame, now), (Step::Continue, vec![]));
        } else {
            r.p.push_claim(claim, REPLY_STREAM, &[2]).unwrap();
            let out = poll(&mut r, now).1;
            assert_eq!(out.len(), 1);
            assert!(matches!(
                decode(&out[0]).unwrap(),
                Frame::Chunk {
                    target: DUPLEX,
                    stream: 0,
                    payload: [2],
                    ..
                }
            ));
        }
        r.now = t0 + std::time::Duration::from_secs(100);
        let now = r.now;
        assert_eq!(
            poll(&mut r, now),
            (Step::Continue, vec![]),
            "reply progress keeps the upload open"
        );
        pull(&mut r, claim, Pull::Pending);
        let out = chunk(&mut r, 1, 1, &[9]).1;
        assert_eq!(out.len(), 1);
        assert!(matches!(
            decode(&out[0]).unwrap(),
            Frame::StreamCredit {
                target: DUPLEX,
                stream: 1,
                credit: 1,
                ..
            }
        ));
        pull(&mut r, claim, Pull::Chunk(1));
        assert!(r.notes.lock().unwrap().is_empty());
    }
}

// Mutation: use only the upload clock for duplex abandonment.
#[test]
fn a_duplex_idle_on_both_streams_is_abandoned_once() {
    let mut r = query_rig(DUPLEX);
    let claim = start(&mut r, None);
    let t0 = r.now;
    r.now = t0 + std::time::Duration::from_secs(10);
    chunk(&mut r, 1, 1, &[1]);
    pull(&mut r, claim, Pull::Chunk(1));
    r.now = t0 + std::time::Duration::from_secs(20);
    r.p.push_claim(claim, REPLY_STREAM, &[2]).unwrap();
    let now = r.now;
    assert_eq!(poll(&mut r, now).1.len(), 1);
    assert_eq!(
        poll(&mut r, t0 + std::time::Duration::from_secs(79)),
        (Step::Continue, vec![])
    );
    assert!(r.notes.lock().unwrap().is_empty());
    r.now = t0 + std::time::Duration::from_secs(80);
    let now = r.now;
    let (step, out) = poll(&mut r, now);
    assert_eq!(step, Step::Continue);
    check_duplex_abandoned(&mut r, &out);
    pull(
        &mut r,
        claim,
        Pull::Failed(CallError::Transport(Transport::Interrupted)),
    );
}
