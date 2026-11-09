use super::*;
use crate::frame::{ResponseOutcome, Target};
use crate::server::{MemberTable, Providing};
use ridl_loopback::Loopback;
use ridl_rt::contract::{CatalogHash, CatalogRef, InterfaceNo, Kind, Ordinal};
use ridl_rt::error::{CallError, Contract, Transport};
use ridl_rt::port::{Caller, Handler};
use ridl_rt::sample::{Envelope, Timestamp};
use std::time::Duration;

const COMMAND: Target = Target {
    kind: Kind::Command,
    iface: InterfaceNo(1),
    ord: Ordinal(3),
};
const QUERY: Target = Target {
    kind: Kind::Query,
    ord: Ordinal(4),
    ..COMMAND
};
fn setup(config: &Config) -> (Provided<Providing<Loopback>>, Loopback) {
    let rt = Loopback::new(CatalogRef {
        name: "calls",
        hash: CatalogHash([7; 32]),
    });
    let caller = rt.attach();
    let mut table = MemberTable::new("calls", CatalogHash([7; 32]));
    for t in [COMMAND, QUERY] {
        table.insert(t.iface, t.ord, t.kind, Some(Duration::from_secs(1)));
        table.set_limit(t.iface, t.ord, 2);
    }
    let mut s = Provided::new(Providing::new(rt, table).unwrap(), config);
    message(
        &mut s,
        Frame::Attached(AttachOutcome::Accepted),
        Instant::now(),
        config,
    );
    (s, caller)
}
fn message(
    s: &mut Provided<Providing<Loopback>>,
    f: Frame<'_>,
    now: Instant,
    config: &Config,
) -> Step {
    let mut bytes = vec![0; f.encoded_len()];
    f.encode(&mut bytes).unwrap();
    s.on_message(&bytes, now, &mut vec![], config)
}
fn requests(
    s: &mut Provided<Providing<Loopback>>,
    now: Instant,
) -> Vec<(Target, Envelope, Vec<u8>)> {
    let mut out = vec![];
    assert!(matches!(s.tick(now, &mut out), Step::Continue));
    out.iter()
        .map(|b| match frame::decode(b).unwrap() {
            Frame::Request {
                target,
                envelope,
                payload,
            } => (target, envelope, payload.to_vec()),
            f => panic!("{f:?}"),
        })
        .collect()
}
fn response(
    s: &mut Provided<Providing<Loopback>>,
    target: Target,
    correlation: u64,
    outcome: ResponseOutcome<'_>,
    now: Instant,
    config: &Config,
) {
    assert!(matches!(
        message(
            s,
            Frame::Response {
                target,
                correlation,
                envelope: Envelope {
                    stamp: Timestamp(999),
                    seq: 1
                },
                outcome
            },
            now,
            config
        ),
        Step::Continue
    ));
}
#[test]
fn provider_answers_command_and_query() {
    let config = Config::default();
    let now = Instant::now();
    let (mut s, mut c) = setup(&config);
    c.advance(ridl_rt::sample::Duration(123));
    let cmd = c
        .command(COMMAND.iface, COMMAND.ord, &[1, 2], None)
        .unwrap();
    let qry = c.query(QUERY.iface, QUERY.ord, &[3], None).unwrap();
    let r = requests(&mut s, now);
    assert_eq!(r.len(), 2);
    assert_eq!(
        (r[0].1.stamp, r[1].1.stamp),
        (Timestamp(123), Timestamp(123))
    );
    assert_eq!((r[0].0, r[0].2.clone()), (COMMAND, vec![1, 2]));
    assert_eq!((r[1].0, r[1].2.clone()), (QUERY, vec![3]));
    assert_eq!((r[0].1.seq, r[1].1.seq), (1, 2));
    response(
        &mut s,
        QUERY,
        2,
        ResponseOutcome::Reply(&[7, 8]),
        now,
        &config,
    );
    let mut buf = [0; 8];
    assert_eq!(c.reply(qry, &mut buf), Ok(Some(Ok(2))));
    assert_eq!(&buf[..2], &[7, 8]);
    response(
        &mut s,
        COMMAND,
        1,
        ResponseOutcome::Contract(Contract::PreconditionFailed),
        now,
        &config,
    );
    assert_eq!(
        c.ack(cmd),
        Some(Err(CallError::Contract(Contract::PreconditionFailed)))
    );
}
#[test]
fn provider_serves_only_offered_ordinals() {
    let config = Config::default();
    let (mut s, mut c) = setup(&config);
    let mut other = c.attach();
    other.serve(InterfaceNo(1), &[Ordinal(8)]).unwrap();
    c.command(InterfaceNo(1), Ordinal(8), &[8], None).unwrap();
    assert!(requests(&mut s, Instant::now()).is_empty());
    let mut buf = [0; 8];
    assert_eq!(other.next_claim(&mut buf).unwrap().unwrap().ord, Ordinal(8));
}
#[test]
fn provider_response_duplicate_is_not_resettled() {
    let config = Config::default();
    let now = Instant::now();
    let (mut s, mut c) = setup(&config);
    let a = c.command(COMMAND.iface, COMMAND.ord, &[], None).unwrap();
    let b = c.command(COMMAND.iface, COMMAND.ord, &[], None).unwrap();
    assert_eq!(requests(&mut s, now).len(), 2);
    response(&mut s, QUERY, 1, ResponseOutcome::Reply(&[4]), now, &config);
    response(
        &mut s,
        COMMAND,
        999,
        ResponseOutcome::Accepted,
        now,
        &config,
    );
    assert_eq!(c.ack(a), None);
    assert_eq!(c.ack(b), None);
    response(&mut s, COMMAND, 1, ResponseOutcome::Accepted, now, &config);
    assert_eq!(c.ack(a), Some(Ok(())));
    response(&mut s, COMMAND, 1, ResponseOutcome::Busy, now, &config);
    assert_eq!(c.ack(b), None);
    response(&mut s, COMMAND, 2, ResponseOutcome::Corrupt, now, &config);
    assert_eq!(
        c.ack(b),
        Some(Err(CallError::Transport(Transport::Corrupt)))
    );
}
#[test]
fn provider_call_timeout_and_disconnect_settle() {
    let config = Config::default();
    let now = Instant::now();
    let (mut s, mut c) = setup(&config);
    let a = c.command(COMMAND.iface, COMMAND.ord, &[], None).unwrap();
    let b = c.query(QUERY.iface, QUERY.ord, &[], None).unwrap();
    requests(&mut s, now);
    requests(&mut s, now + Duration::from_secs(1));
    assert_eq!(c.ack(a), None);
    requests(
        &mut s,
        now + Duration::from_secs(1) + Duration::from_nanos(1),
    );
    assert_eq!(
        c.ack(a),
        Some(Err(CallError::Transport(Transport::Undelivered)))
    );
    assert_eq!(
        c.reply(b, &mut []),
        Ok(Some(Err(CallError::Transport(Transport::Timeout))))
    );
    let a = c.command(COMMAND.iface, COMMAND.ord, &[], None).unwrap();
    let b = c.query(QUERY.iface, QUERY.ord, &[], None).unwrap();
    requests(&mut s, now);
    // Keep the ports alive: cleanup must settle claims, not rely on handler drop.
    let _keep_alive = s.backend.ports().attach();
    drop(s);
    assert_eq!(c.ack(a), Some(Err(CallError::Transport(Transport::Down))));
    assert_eq!(
        c.reply(b, &mut []),
        Ok(Some(Err(CallError::Transport(Transport::Down))))
    );
    for _ in 0..64 {
        let x = c.command(COMMAND.iface, COMMAND.ord, &[], None).unwrap();
        c.forget(x);
    }
}
#[test]
fn provider_calls_respect_limits_and_capacity() {
    let config = Config::default();
    let now = Instant::now();
    let (mut s, mut c) = setup(&config);
    let too_large = c.query(QUERY.iface, QUERY.ord, &[1; 3], None).unwrap();
    assert!(requests(&mut s, now).is_empty());
    assert_eq!(
        c.reply(too_large, &mut []),
        Ok(Some(Err(CallError::Transport(Transport::Corrupt))))
    );
    let q = c.query(QUERY.iface, QUERY.ord, &[1; 2], None).unwrap();
    assert_eq!(requests(&mut s, now)[0].2, [1; 2]);
    response(
        &mut s,
        QUERY,
        1,
        ResponseOutcome::Reply(&[2; 3]),
        now,
        &config,
    );
    assert_eq!(
        c.reply(q, &mut []),
        Ok(Some(Err(CallError::Transport(Transport::Corrupt))))
    );
    // A deliberately smaller session capacity makes the bound observable with loopback slots.
    s.calls.capacity = 2;
    let mut ids = vec![];
    for _ in 0..3 {
        ids.push(c.command(COMMAND.iface, COMMAND.ord, &[], None).unwrap());
    }
    let r = requests(&mut s, now);
    assert_eq!(r.len(), 2);
    assert!(requests(&mut s, now).is_empty());
    response(
        &mut s,
        COMMAND,
        r[0].1.seq,
        ResponseOutcome::Accepted,
        now,
        &config,
    );
    assert_eq!(requests(&mut s, now).len(), 1);
    drop(s);
    for id in ids {
        assert!(c.ack(id).is_some());
    }
}
#[test]
fn provider_fixed_read_is_unknown_and_wrong_direction_closes() {
    let config = Config::default();
    let now = Instant::now();
    let (mut s, _) = setup(&config);
    let fixed = Target {
        kind: Kind::Fixed,
        ord: Ordinal(5),
        ..COMMAND
    };
    // A provider's refusal of an unknown fixed read is a legal provider form.
    assert!(matches!(
        message(
            &mut s,
            Frame::Answer {
                target: fixed,
                outcome: crate::frame::AnswerOutcome::Contract(Contract::UnknownInteraction)
            },
            now,
            &config
        ),
        Step::Continue
    ));
    assert!(matches!(
        message(
            &mut s,
            Frame::Request {
                target: COMMAND,
                envelope: Envelope {
                    stamp: Timestamp(0),
                    seq: 1
                },
                payload: &[]
            },
            now,
            &config
        ),
        Step::Close(1002)
    ));
    assert!(matches!(
        message(&mut s, Frame::Read(fixed), now, &config),
        Step::Close(1002)
    ));
}

#[test]
fn provider_response_deadline_uses_request_stamp_and_local_clock() {
    let config = Config::default();
    let now = Instant::now();
    let (mut s, mut c) = setup(&config);
    let q = c.query(QUERY.iface, QUERY.ord, &[], None).unwrap();
    c.advance(ridl_rt::sample::Duration(900_000));
    assert_eq!(requests(&mut s, now).len(), 1);
    c.advance(ridl_rt::sample::Duration(100_001));
    response(&mut s, QUERY, 1, ResponseOutcome::Reply(&[9]), now, &config);
    assert_eq!(
        c.reply(q, &mut []),
        Ok(Some(Err(CallError::Transport(Transport::Timeout))))
    );
}

#[test]
fn provider_calls_wait_for_attached_and_reject_streamed_reply_members() {
    let config = Config::default();
    let now = Instant::now();
    let (mut s, mut c) = setup(&config);
    s.attached = false;
    let q = c.query(QUERY.iface, QUERY.ord, &[], None).unwrap();
    assert!(requests(&mut s, now).is_empty());
    s.attached = true;
    let mut members = s.backend.members().clone();
    members.set_reply_stream(QUERY.iface, QUERY.ord);
    let ports = s.backend.ports().attach();
    s.backend = Providing::new(ports, members).unwrap();
    assert!(s.calls.start(&mut s.backend));
    assert!(requests(&mut s, now).is_empty());
    assert_eq!(
        c.reply(q, &mut []),
        Ok(Some(Err(CallError::Contract(Contract::UnknownInteraction))))
    );
}

#[test]
fn provider_server_pings_and_closes_unresponsive_client_core() {
    use super::super::{heartbeat, Heartbeat};
    let config = Config::default();
    let now = Instant::now();
    assert_eq!(
        heartbeat(
            now + config.ping_interval - Duration::from_nanos(1),
            now,
            now,
            &config
        ),
        Heartbeat::Wait
    );
    assert_eq!(
        heartbeat(now + config.ping_interval, now, now, &config),
        Heartbeat::Ping
    );
    assert_eq!(
        heartbeat(now + config.liveness_timeout, now, now, &config),
        Heartbeat::Expired
    );
    let later = now + config.liveness_timeout;
    assert_eq!(heartbeat(later, later, later, &config), Heartbeat::Wait);
}

#[test]
fn provider_disconnect_settles_without_dropping_handler_and_reuses_all_slots() {
    let config = Config::default();
    let now = Instant::now();
    let mut ports = Loopback::new(CatalogRef {
        name: "calls",
        hash: CatalogHash([7; 32]),
    });
    let mut caller = ports.attach();
    let mut table = MemberTable::new("calls", CatalogHash([7; 32]));
    table.insert(COMMAND.iface, COMMAND.ord, COMMAND.kind, None);
    table.insert(QUERY.iface, QUERY.ord, QUERY.kind, None);
    let mut s = Provided::new(Providing::new(&mut ports, table).unwrap(), &config);
    let f = Frame::Attached(AttachOutcome::Accepted);
    let mut bytes = vec![0; f.encoded_len()];
    f.encode(&mut bytes).unwrap();
    s.on_message(&bytes, now, &mut vec![], &config);
    let a = caller
        .command(COMMAND.iface, COMMAND.ord, &[], None)
        .unwrap();
    let b = caller.query(QUERY.iface, QUERY.ord, &[], None).unwrap();
    let mut out = vec![];
    s.tick(now, &mut out);
    assert_eq!(out.len(), 2);
    drop(s); // The exact Handler remains alive in `ports`.
    assert_eq!(
        caller.ack(a),
        Some(Err(CallError::Transport(Transport::Down)))
    );
    assert_eq!(
        caller.reply(b, &mut []),
        Ok(Some(Err(CallError::Transport(Transport::Down))))
    );
    caller.forget(a);
    caller.forget(b);
    let mut all = vec![];
    for _ in 0..16 {
        all.push(
            caller
                .command(COMMAND.iface, COMMAND.ord, &[], None)
                .unwrap(),
        );
    }
    assert!(caller
        .command(COMMAND.iface, COMMAND.ord, &[], None)
        .is_err());
    for c in all {
        caller.forget(c);
    }
}

#[test]
fn provider_request_counter_is_shared_across_local_callers() {
    let config = Config::default();
    let now = Instant::now();
    let (mut s, mut a) = setup(&config);
    let mut b = a.attach();
    let x = a.command(COMMAND.iface, COMMAND.ord, &[1], None).unwrap();
    let y = b.command(COMMAND.iface, COMMAND.ord, &[2], None).unwrap();
    let r = requests(&mut s, now);
    assert_eq!((r[0].1.seq, r[1].1.seq), (1, 2));
    response(&mut s, COMMAND, 2, ResponseOutcome::Busy, now, &config);
    assert_eq!(a.ack(x), None);
    assert_eq!(b.ack(y), Some(Err(CallError::Transport(Transport::Busy))));
}

#[test]
fn provider_does_not_forward_already_expired_claim() {
    let config = Config::default();
    let now = Instant::now();
    let (mut s, mut c) = setup(&config);
    let q = c.query(QUERY.iface, QUERY.ord, &[], None).unwrap();
    c.advance(ridl_rt::sample::Duration(1_000_001));
    assert!(requests(&mut s, now).is_empty());
    assert_eq!(
        c.reply(q, &mut []),
        Ok(Some(Err(CallError::Transport(Transport::Timeout))))
    );
}

#[test]
fn provider_failed_short_claim_settlement_is_owned_until_disconnect() {
    let config = Config::default();
    let now = Instant::now();
    let mut ports = Loopback::new(CatalogRef {
        name: "calls",
        hash: CatalogHash([7; 32]),
    });
    let mut caller = ports.attach();
    ports.set_limit(
        COMMAND.iface,
        COMMAND.ord,
        ridl_rt::contract::CALL_CEILING + 1,
    );
    ports.fail_next_settle();
    let mut table = MemberTable::new("calls", CatalogHash([7; 32]));
    table.insert(COMMAND.iface, COMMAND.ord, COMMAND.kind, None);
    let mut s = Provided::new(Providing::new(&mut ports, table).unwrap(), &config);
    assert!(s.calls.start(&mut s.backend));
    s.attached = true;
    let c = caller
        .command(
            COMMAND.iface,
            COMMAND.ord,
            &vec![1; ridl_rt::contract::CALL_CEILING + 1],
            None,
        )
        .unwrap();
    let mut out = vec![];
    s.tick(now, &mut out);
    assert!(out.is_empty());
    assert_eq!(caller.ack(c), None);
    drop(s);
    assert_eq!(
        caller.ack(c),
        Some(Err(CallError::Transport(Transport::Down)))
    );
}

#[test]
fn provider_wrong_outcome_settles_corrupt() {
    let config = Config::default();
    let now = Instant::now();
    let (mut s, mut c) = setup(&config);
    let q = c.query(QUERY.iface, QUERY.ord, &[], None).unwrap();
    let a = c.command(COMMAND.iface, COMMAND.ord, &[], None).unwrap();
    let r = requests(&mut s, now);
    for (target, envelope, _) in r {
        let outcome = if target == QUERY {
            ResponseOutcome::Accepted
        } else {
            ResponseOutcome::Reply(&[1])
        };
        response(&mut s, target, envelope.seq, outcome, now, &config);
    }
    assert_eq!(
        c.reply(q, &mut []),
        Ok(Some(Err(CallError::Transport(Transport::Corrupt))))
    );
    assert_eq!(
        c.ack(a),
        Some(Err(CallError::Transport(Transport::Corrupt)))
    );
}

#[test]
fn provider_unbounded_call_uses_abandon_deadline() {
    let mut config = Config::default();
    config.call_abandon_after = Duration::from_millis(17);
    let now = Instant::now();
    let (mut s, mut c) = setup(&config);
    let mut table = s.backend.members().clone();
    for t in [COMMAND, QUERY] {
        table.insert(t.iface, t.ord, t.kind, None);
    }
    let ports = s.backend.ports().attach();
    s.backend = Providing::new(ports, table).unwrap();
    assert!(s.calls.start(&mut s.backend));
    let a = c.command(COMMAND.iface, COMMAND.ord, &[], None).unwrap();
    let q = c.query(QUERY.iface, QUERY.ord, &[], None).unwrap();
    assert_eq!(requests(&mut s, now).len(), 2);
    requests(&mut s, now + config.call_abandon_after);
    assert_eq!(c.ack(a), None);
    assert_eq!(c.reply(q, &mut []), Ok(None));
    requests(
        &mut s,
        now + config.call_abandon_after + Duration::from_nanos(1),
    );
    assert_eq!(
        c.ack(a),
        Some(Err(CallError::Transport(Transport::Undelivered)))
    );
    assert_eq!(
        c.reply(q, &mut []),
        Ok(Some(Err(CallError::Transport(Transport::Timeout))))
    );
}

#[test]
fn provider_unknown_correlation_has_own_note() {
    let notes = std::sync::Arc::new(std::sync::Mutex::new(vec![]));
    let captured = notes.clone();
    let config = Config::default().with_record(move |note| captured.lock().unwrap().push(*note));
    let (mut s, _) = setup(&config);
    response(
        &mut s,
        COMMAND,
        999,
        ResponseOutcome::Accepted,
        Instant::now(),
        &config,
    );
    assert_eq!(
        *notes.lock().unwrap(),
        vec![Note::UnknownCorrelation {
            target: COMMAND,
            correlation: 999
        }]
    );
}

/// A command and a query whose limit is raised above 16,128 bytes cross the
/// provided session: the claim buffer holds the largest request the member
/// table allows, and the reply of the query is settled up to its limit.
///
/// Mutation: size the claim buffer from `CALL_CEILING` alone.
#[test]
fn provider_forwards_a_raised_command_and_query() {
    let config = Config::default().with_max_message_bytes(65_316).unwrap();
    let now = Instant::now();
    let mut rt = Loopback::new(CatalogRef {
        name: "calls",
        hash: CatalogHash([7; 32]),
    });
    let mut c = rt.attach();
    let mut table = MemberTable::new("calls", CatalogHash([7; 32]));
    for t in [COMMAND, QUERY] {
        table.insert(t.iface, t.ord, t.kind, Some(Duration::from_secs(1)));
        table.set_limit(t.iface, t.ord, 65_280);
        rt.set_limit(t.iface, t.ord, 65_280);
    }
    let mut s = Provided::new(Providing::new(rt, table).unwrap(), &config);
    message(
        &mut s,
        Frame::Attached(AttachOutcome::Accepted),
        now,
        &config,
    );
    let cmd = c
        .command(COMMAND.iface, COMMAND.ord, &vec![5; 47_000], None)
        .unwrap();
    let qry = c
        .query(QUERY.iface, QUERY.ord, &vec![6; 65_280], None)
        .unwrap();
    let r = requests(&mut s, now);
    assert_eq!(r.len(), 2);
    assert_eq!((r[0].0, r[0].2.len()), (COMMAND, 47_000));
    assert_eq!((r[1].0, r[1].2.len()), (QUERY, 65_280));
    response(&mut s, COMMAND, 1, ResponseOutcome::Accepted, now, &config);
    assert_eq!(c.ack(cmd), Some(Ok(())));
    response(
        &mut s,
        QUERY,
        2,
        ResponseOutcome::Reply(&vec![7; 50_000]),
        now,
        &config,
    );
    let mut buf = vec![0; 65_280];
    assert_eq!(c.reply(qry, &mut buf), Ok(Some(Ok(50_000))));
    assert!(buf[..50_000].iter().all(|b| *b == 7));
}
