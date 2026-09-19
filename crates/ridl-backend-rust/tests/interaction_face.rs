//! The interaction-face round trip (Lane M stage M3, Tasks 4 and 5,
//! docs/wip/2026-09-16-interaction-face-v0-design.md §7).
//!
//! Task 4 exercises the disposable loopback ports of `tests/support/loopback.rs`
//! directly, against `ridl-rt`'s trait contracts, ahead of the checked-in
//! generated face that Task 5 adds. A test name containing `support` isolates
//! this task's tests: `cargo test -p ridl-backend-rust --test interaction_face
//! support`.

mod support;

use ridl_rt::contract::{InterfaceNo, Ordinal};
use ridl_rt::port::{Caller, Clock, EventSink, EventSource, Handler, SignalReader, SignalWriter};
use ridl_rt::sample::{Duration, Provenance};

use support::loopback::Loopback;

const IFACE: InterfaceNo = InterfaceNo(1);
const ORD: Ordinal = Ordinal(1);

#[test]
fn support_command_is_delivered_and_acknowledged() {
    let mut lb = Loopback::new("face.demo");
    let correlation = lb.command(IFACE, ORD, &[1, 2, 3]).expect("command sent");

    assert_eq!(lb.ack(correlation), None, "not yet settled");

    let mut buf = [0u8; 8];
    let claim = lb
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("a claim is waiting");
    assert_eq!(claim.iface, IFACE);
    assert_eq!(claim.ord, ORD);
    assert_eq!(&buf[..claim.len], &[1, 2, 3]);

    lb.settle(claim.id, Ok(&[])).expect("settle succeeds");
    assert_eq!(
        lb.ack(correlation),
        Some(Ok(())),
        "settlement is observable through ack"
    );
}

#[test]
fn support_query_is_delivered_and_replied() {
    let mut lb = Loopback::new("face.demo");
    let correlation = lb.query(IFACE, ORD, &[9]).expect("query sent");

    let mut buf = [0u8; 8];
    let claim = lb
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("a claim is waiting");
    assert_eq!(&buf[..claim.len], &[9]);

    lb.settle(claim.id, Ok(&[7, 7])).expect("settle succeeds");

    let mut out = [0u8; 8];
    let reply = lb.reply(correlation, &mut out).expect("reply read");
    let Some(Ok(len)) = reply else {
        panic!("expected a successful reply, got {reply:?}");
    };
    assert_eq!(&out[..len], &[7, 7]);
    // A query's correlation always answers `None` from `ack` (`Caller::ack`'s
    // own documentation).
    assert_eq!(lb.ack(correlation), None);
}

#[test]
fn support_settle_can_be_made_to_fail_once_then_succeed() {
    // Task 5 asserts that `dispatch`'s returned count only advances past a
    // settlement the handler accepted; this proves the loopback can inject
    // that one failure ahead of a later, successful claim.
    let mut lb = Loopback::new("face.demo");
    let correlation = lb.command(IFACE, ORD, &[1]).expect("command sent");
    let mut buf = [0u8; 8];
    let claim = lb
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("a claim is waiting");

    lb.fail_next_settle();
    assert!(
        lb.settle(claim.id, Ok(&[])).is_err(),
        "the injected failure surfaces from settle"
    );
    assert_eq!(
        lb.ack(correlation),
        None,
        "a failed settle records no outcome"
    );

    lb.settle(claim.id, Ok(&[]))
        .expect("the next settle succeeds");
    assert_eq!(lb.ack(correlation), Some(Ok(())));
}

#[test]
fn support_signal_publish_and_read_round_trips() {
    let mut lb = Loopback::new("face.demo");
    lb.set(IFACE, ORD, &[42]).expect("set staged");
    lb.commit();

    let mut out = [0u8; 8];
    let raw = lb.read(IFACE, ORD, &mut out).expect("read");
    assert_eq!(raw.provenance, Provenance::Live);
    assert_eq!(&out[..raw.len], &[42]);
}

#[test]
fn support_event_raise_and_receive_round_trips() {
    let mut lb = Loopback::new("face.demo");
    lb.subscribe(IFACE, &[ORD]).expect("subscribe");
    lb.raise(IFACE, ORD, &[5, 6]).expect("raise");

    let mut out = [0u8; 8];
    let occurrence = lb
        .next(&mut out)
        .expect("next")
        .expect("an occurrence is waiting");
    assert_eq!(occurrence.iface, IFACE);
    assert_eq!(occurrence.ord, ORD);
    assert_eq!(&out[..occurrence.len], &[5, 6]);
}

#[test]
fn support_clock_is_hand_driven_not_wall_clock() {
    // Two independently constructed ports must start at the same logical time
    // regardless of when, in real time, each was constructed — a clock that
    // reads wall-clock time could not guarantee that.
    let first = Loopback::new("face.demo");
    std::thread::sleep(std::time::Duration::from_millis(5));
    let second = Loopback::new("face.demo");
    assert_eq!(
        first.now(),
        second.now(),
        "the clock must not read wall-clock time"
    );

    let mut lb = second;
    let before = lb.now();
    lb.advance(Duration(1_000));
    let after = lb.now();
    assert_eq!(
        after.0,
        before.0 + 1_000,
        "advance moves the hand-driven clock by exactly the given amount"
    );
}
