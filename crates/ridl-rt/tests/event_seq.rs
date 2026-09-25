//! `EventSeqTracker`: event loss from the envelope's sequence number, tracked
//! per channel (ridl §3.1; frame specification §5.2, §6.2 and §7).

#![forbid(unsafe_code)]

use ridl_rt::contract::{InterfaceNo, Ordinal};
use ridl_rt::sample::{Continuity, EventSeqTracker, TrackerFull};

const IFACE: InterfaceNo = InterfaceNo(1);
const OTHER_IFACE: InterfaceNo = InterfaceNo(2);
const SHIFT_DONE: Ordinal = Ordinal(2);
const DOOR_OPENED: Ordinal = Ordinal(3);

/// Frame §6.2: only occurrences raised after the subscription is answered are
/// delivered, so the first occurrence a consumer sees may carry any `seq`.
/// Frame §5.2 and §7 define a loss only between two accepted occurrences of one
/// channel, and attribute that rule to ridl §3.1.
#[test]
fn the_first_occurrence_of_a_channel_reports_no_loss() {
    let mut tracker = EventSeqTracker::<2>::new();
    assert_eq!(
        tracker.observe(IFACE, SHIFT_DONE, 41),
        Ok(Continuity::First)
    );
}

/// ridl §3.1 and frame §7: the counter is monotonic per channel, so `last + 1`
/// is the next occurrence with nothing lost.
#[test]
fn consecutive_seqs_report_no_loss() {
    let mut tracker = EventSeqTracker::<2>::new();
    assert_eq!(tracker.observe(IFACE, SHIFT_DONE, 1), Ok(Continuity::First));
    assert_eq!(tracker.observe(IFACE, SHIFT_DONE, 2), Ok(Continuity::Next));
    assert_eq!(tracker.observe(IFACE, SHIFT_DONE, 3), Ok(Continuity::Next));
}

/// ridl §3.1 ("sequence gaps make loss detectable") and frame §5.2 and §7: a
/// gap between two accepted occurrences of one event channel is a loss, of as
/// many occurrences as the gap is wide.
#[test]
fn a_gap_is_reported_as_a_loss_of_its_width() {
    let mut tracker = EventSeqTracker::<2>::new();
    assert_eq!(tracker.observe(IFACE, SHIFT_DONE, 1), Ok(Continuity::First));
    assert_eq!(
        tracker.observe(IFACE, SHIFT_DONE, 3),
        Ok(Continuity::Lost { count: 1 })
    );
    assert_eq!(
        tracker.observe(IFACE, SHIFT_DONE, 10),
        Ok(Continuity::Lost { count: 6 })
    );
    assert_eq!(tracker.observe(IFACE, SHIFT_DONE, 11), Ok(Continuity::Next));
}

/// ridl §3.1 and frame §7: the counter is per channel, per provider instance,
/// not per interface. Two channels whose occurrences interleave each count
/// from their own last `seq`, so the interleaving is not a loss.
#[test]
fn interleaved_channels_of_one_interface_report_no_loss() {
    let mut tracker = EventSeqTracker::<2>::new();
    assert_eq!(tracker.observe(IFACE, SHIFT_DONE, 1), Ok(Continuity::First));
    assert_eq!(
        tracker.observe(IFACE, DOOR_OPENED, 1),
        Ok(Continuity::First)
    );
    assert_eq!(tracker.observe(IFACE, SHIFT_DONE, 2), Ok(Continuity::Next));
    assert_eq!(tracker.observe(IFACE, DOOR_OPENED, 2), Ok(Continuity::Next));
    assert_eq!(
        tracker.observe(IFACE, DOOR_OPENED, 4),
        Ok(Continuity::Lost { count: 1 })
    );
    assert_eq!(tracker.observe(IFACE, SHIFT_DONE, 3), Ok(Continuity::Next));
}

/// Frame §3 and §6.2: a channel is one `(interface, ordinal)` in the session's
/// catalog, so one ordinal in two interfaces is two channels.
#[test]
fn one_ordinal_in_two_interfaces_is_two_channels() {
    let mut tracker = EventSeqTracker::<2>::new();
    assert_eq!(tracker.observe(IFACE, SHIFT_DONE, 5), Ok(Continuity::First));
    assert_eq!(
        tracker.observe(OTHER_IFACE, SHIFT_DONE, 9),
        Ok(Continuity::First)
    );
    assert_eq!(tracker.observe(IFACE, SHIFT_DONE, 6), Ok(Continuity::Next));
    assert_eq!(
        tracker.observe(OTHER_IFACE, SHIFT_DONE, 10),
        Ok(Continuity::Next)
    );
}

/// Frame §5.2 and §7 (which attribute the rule to ridl §3.1): a loss is a gap
/// between two accepted occurrences of one channel. A `seq` not greater than the last one is a duplicate or a reordered
/// occurrence, not a gap: the tracker reports it and keeps the last `seq`.
#[test]
fn a_seq_not_newer_than_the_last_is_reported_and_leaves_the_last_unchanged() {
    let mut tracker = EventSeqTracker::<2>::new();
    assert_eq!(tracker.observe(IFACE, SHIFT_DONE, 5), Ok(Continuity::First));
    assert_eq!(
        tracker.observe(IFACE, SHIFT_DONE, 5),
        Ok(Continuity::NotNewer { last: 5 })
    );
    assert_eq!(
        tracker.observe(IFACE, SHIFT_DONE, 3),
        Ok(Continuity::NotNewer { last: 5 })
    );
    assert_eq!(tracker.observe(IFACE, SHIFT_DONE, 6), Ok(Continuity::Next));
}

/// The storage is the caller's, of `N` channels: a channel beyond `N` is
/// refused, and every channel already tracked keeps its count.
#[test]
fn a_channel_beyond_the_capacity_is_refused() {
    let mut tracker = EventSeqTracker::<1>::new();
    assert_eq!(tracker.observe(IFACE, SHIFT_DONE, 1), Ok(Continuity::First));
    assert_eq!(tracker.observe(IFACE, DOOR_OPENED, 1), Err(TrackerFull));
    assert_eq!(tracker.observe(IFACE, SHIFT_DONE, 2), Ok(Continuity::Next));
}

/// Frame §6.2: frames of an unsubscribed interaction are discarded, and a
/// later subscription delivers only occurrences raised after it. A forgotten
/// channel starts again at `First`, and its slot is free for another channel.
#[test]
fn a_forgotten_channel_starts_again_and_frees_its_slot() {
    let mut tracker = EventSeqTracker::<1>::new();
    assert_eq!(tracker.observe(IFACE, SHIFT_DONE, 1), Ok(Continuity::First));
    tracker.forget(IFACE, SHIFT_DONE);
    assert_eq!(
        tracker.observe(IFACE, DOOR_OPENED, 7),
        Ok(Continuity::First)
    );
    tracker.forget(IFACE, DOOR_OPENED);
    assert_eq!(tracker.observe(IFACE, SHIFT_DONE, 9), Ok(Continuity::First));
}

/// Frame §3: a session's sequence counters start over when the session is
/// opened, so a new tracker holds no channel. `Default` is `new`.
#[test]
fn a_default_tracker_holds_no_channel() {
    let mut tracker: EventSeqTracker<1> = Default::default();
    assert_eq!(tracker.observe(IFACE, SHIFT_DONE, 3), Ok(Continuity::First));
}

/// `Envelope::seq` is a `u64`, the width frame §2 and §4 give the envelope's
/// sequence number; the largest value follows its predecessor with no
/// overflow.
#[test]
fn the_largest_seq_follows_its_predecessor() {
    let mut tracker = EventSeqTracker::<1>::new();
    assert_eq!(tracker.observe(IFACE, SHIFT_DONE, 0), Ok(Continuity::First));
    assert_eq!(
        tracker.observe(IFACE, SHIFT_DONE, u64::MAX),
        Ok(Continuity::Lost {
            count: u64::MAX - 1
        })
    );
    assert_eq!(
        tracker.observe(IFACE, SHIFT_DONE, u64::MAX),
        Ok(Continuity::NotNewer { last: u64::MAX })
    );
}

/// The tracker allocates nothing, so a tracker can be a `const` or a `static`.
#[test]
fn a_tracker_can_be_built_in_a_const_context() {
    const TRACKER: EventSeqTracker<4> = EventSeqTracker::new();
    let mut tracker = TRACKER;
    assert_eq!(tracker.observe(IFACE, SHIFT_DONE, 1), Ok(Continuity::First));
}

/// ridl §3.1 and frame §7: forgetting one channel leaves every other channel's
/// last `seq` in place, even when the freed slot comes before it.
#[test]
fn forgetting_one_channel_leaves_the_others_counting() {
    let mut tracker = EventSeqTracker::<2>::new();
    assert_eq!(tracker.observe(IFACE, SHIFT_DONE, 1), Ok(Continuity::First));
    assert_eq!(
        tracker.observe(IFACE, DOOR_OPENED, 4),
        Ok(Continuity::First)
    );
    tracker.forget(IFACE, SHIFT_DONE);
    assert_eq!(tracker.observe(IFACE, DOOR_OPENED, 5), Ok(Continuity::Next));
    assert_eq!(tracker.observe(IFACE, SHIFT_DONE, 8), Ok(Continuity::First));
}

/// Frame §3 and §6.2: a channel is one `(interface, ordinal)`, so forgetting a
/// channel of one interface leaves the channel of the same ordinal in another
/// interface in place.
#[test]
fn forgetting_a_channel_leaves_the_same_ordinal_in_another_interface() {
    let mut tracker = EventSeqTracker::<2>::new();
    assert_eq!(tracker.observe(IFACE, SHIFT_DONE, 1), Ok(Continuity::First));
    assert_eq!(
        tracker.observe(OTHER_IFACE, SHIFT_DONE, 1),
        Ok(Continuity::First)
    );
    tracker.forget(IFACE, SHIFT_DONE);
    assert_eq!(
        tracker.observe(OTHER_IFACE, SHIFT_DONE, 2),
        Ok(Continuity::Next)
    );
}
