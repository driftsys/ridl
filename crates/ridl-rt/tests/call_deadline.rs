//! `Member::call_deadline`: a call's response bound from its member's timing
//! (ridl §9.3; frame specification §8).

#![forbid(unsafe_code)]

use ridl_rt::contract::{Kind, Member, Ordinal, PayloadInfo, Timing, TimingMode};
use ridl_rt::sample::Duration;

const fn member(kind: Kind, timing: Option<Timing>) -> Member {
    Member {
        ordinal: Ordinal(1),
        kind,
        name: "lock",
        timing,
        payloads: &[] as &[PayloadInfo],
    }
}

const fn command(timing: Option<Timing>) -> Member {
    member(Kind::Command, timing)
}

/// ridl §9.3: on a `command` or a `query`, `max` is the response bound, so
/// `@[..100ms]` gives a deadline of 100 ms.
#[test]
fn the_deadline_is_the_response_bound() {
    let member = command(Some(Timing {
        mode: TimingMode::Range,
        min: None,
        max: Some(Duration(100_000)),
    }));
    assert_eq!(member.call_deadline(), Some(Duration(100_000)));
}

/// ridl §9.3: on a `query`, `max` is the response bound too, covering the
/// reply.
#[test]
fn the_deadline_of_a_query_is_its_response_bound() {
    let query = member(
        Kind::Query,
        Some(Timing {
            mode: TimingMode::Range,
            min: None,
            max: Some(Duration(400_000)),
        }),
    );
    assert_eq!(query.call_deadline(), Some(Duration(400_000)));
}

/// ridl §9: `max` is read whatever the member's kind. On a `signal` it is the
/// staleness bound, not a call deadline, and `call_deadline` returns it all
/// the same, as its documentation states.
#[test]
fn the_max_of_a_signal_is_returned_as_it_is() {
    let signal = member(
        Kind::Signal,
        Some(Timing {
            mode: TimingMode::StrictPeriodic,
            min: Some(Duration(10_000)),
            max: Some(Duration(10_000)),
        }),
    );
    assert_eq!(signal.call_deadline(), Some(Duration(10_000)));
}

/// ridl §9.3: `min` is the call throttle, not part of the deadline.
#[test]
fn the_call_throttle_does_not_change_the_deadline() {
    let member = command(Some(Timing {
        mode: TimingMode::Range,
        min: Some(Duration(20_000)),
        max: Some(Duration(250_000)),
    }));
    assert_eq!(member.call_deadline(), Some(Duration(250_000)));
}

/// ridl §9.3: `@[20ms..]` is a throttle with no response bound: the member
/// declares no response bound, so `call_deadline` is `None`.
#[test]
fn a_throttle_alone_gives_no_deadline() {
    let member = command(Some(Timing {
        mode: TimingMode::Range,
        min: Some(Duration(20_000)),
        max: None,
    }));
    assert_eq!(member.call_deadline(), None);
}

/// ridl §9.1 and §9.3: a bare `command` or `query` is never given a default
/// response bound, and `Member::timing` is then `None`: the member declares no
/// response bound, so `call_deadline` is `None`.
#[test]
fn a_member_with_no_timing_has_no_deadline() {
    assert_eq!(command(None).call_deadline(), None);
}
