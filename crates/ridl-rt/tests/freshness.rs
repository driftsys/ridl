//! `Freshness::of`: a value's freshness from its envelope's timestamp, the
//! current time and its member's timing (ridl §4, §9; frame specification §8).

#![forbid(unsafe_code)]

use ridl_rt::contract::{Timing, TimingMode};
use ridl_rt::sample::{Duration, Envelope, Freshness, Timestamp};

/// `@[20ms..500ms]`, in microseconds.
const RANGE: Timing = Timing {
    mode: TimingMode::Range,
    min: Some(Duration(20_000)),
    max: Some(Duration(500_000)),
};

/// An envelope stamped at `us` microseconds. Its `seq` plays no part in
/// freshness.
fn stamped(us: i64) -> Envelope {
    Envelope {
        stamp: Timestamp(us),
        seq: 1,
    }
}

/// ridl §9 and frame §8: `Fresh` while `now − stamp ≤ max`.
#[test]
fn a_value_younger_than_max_is_fresh() {
    let freshness = Freshness::of(&stamped(1_000_000), Timestamp(1_200_000), Some(RANGE));
    assert_eq!(freshness, Freshness::Fresh);
}

/// ridl §9 and frame §8: the bound is inclusive, so an age equal to `max` is
/// still `Fresh`.
#[test]
fn a_value_exactly_max_old_is_fresh() {
    let freshness = Freshness::of(&stamped(1_000_000), Timestamp(1_500_000), Some(RANGE));
    assert_eq!(freshness, Freshness::Fresh);
}

/// ridl §9 and frame §8: past the bound, `Stale { by: now − stamp − max }`.
#[test]
fn a_value_older_than_max_is_stale_by_the_excess() {
    let freshness = Freshness::of(&stamped(1_000_000), Timestamp(1_500_001), Some(RANGE));
    assert_eq!(freshness, Freshness::Stale { by: Duration(1) });
    let freshness = Freshness::of(&stamped(1_000_000), Timestamp(2_750_000), Some(RANGE));
    assert_eq!(
        freshness,
        Freshness::Stale {
            by: Duration(1_250_000)
        }
    );
}

/// ridl §9 and frame §8: the rate floor `min` plays no part in freshness; only
/// `max` does.
#[test]
fn min_does_not_affect_freshness() {
    let no_min = Timing { min: None, ..RANGE };
    for timing in [RANGE, no_min] {
        assert_eq!(
            Freshness::of(&stamped(0), Timestamp(500_000), Some(timing)),
            Freshness::Fresh
        );
        assert_eq!(
            Freshness::of(&stamped(0), Timestamp(600_000), Some(timing)),
            Freshness::Stale {
                by: Duration(100_000)
            }
        );
    }
}

/// ridl §4.1 and §9: under a strict period `@10ms`, `max` holds the period, so
/// the period is the staleness bound.
#[test]
fn a_strict_period_is_the_staleness_bound() {
    let period = Timing {
        mode: TimingMode::StrictPeriodic,
        min: Some(Duration(10_000)),
        max: Some(Duration(10_000)),
    };
    assert_eq!(
        Freshness::of(&stamped(0), Timestamp(10_000), Some(period)),
        Freshness::Fresh
    );
    assert_eq!(
        Freshness::of(&stamped(0), Timestamp(10_500), Some(period)),
        Freshness::Stale { by: Duration(500) }
    );
}

/// ridl §9 (`@[20ms..]`, a lower bound only) and frame §8: with no `max`, the
/// value is `Unbounded` whatever its age.
#[test]
fn a_timing_with_no_max_is_unbounded() {
    let lower_only = Timing { max: None, ..RANGE };
    assert_eq!(
        Freshness::of(&stamped(0), Timestamp(i64::MAX), Some(lower_only)),
        Freshness::Unbounded
    );
}

/// `Member::timing` (`None`: the IR carries no timing) and frame §8: with no
/// timing there is no `max`, so the value is `Unbounded`.
#[test]
fn a_member_with_no_timing_is_unbounded() {
    assert_eq!(
        Freshness::of(&stamped(0), Timestamp(1_000_000_000), None),
        Freshness::Unbounded
    );
}

/// Neither specification discusses a stamp later than `now`. The result
/// follows from the frame §8 formula: `now − stamp` is then negative, which is
/// at most any non-negative `max`, so the value is `Fresh`.
#[test]
fn a_stamp_later_than_now_is_fresh() {
    assert_eq!(
        Freshness::of(&stamped(2_000_000), Timestamp(1_000_000), Some(RANGE)),
        Freshness::Fresh
    );
}

/// ridl §3.1: timestamps are `int64` microseconds, so the age computation must
/// not overflow at the ends of the range.
#[test]
fn extreme_timestamps_do_not_overflow() {
    assert_eq!(
        Freshness::of(&stamped(i64::MIN), Timestamp(i64::MAX), Some(RANGE)),
        Freshness::Stale {
            by: Duration(i64::MAX - 500_000)
        }
    );
    assert_eq!(
        Freshness::of(&stamped(i64::MAX), Timestamp(i64::MIN), Some(RANGE)),
        Freshness::Fresh
    );
}
