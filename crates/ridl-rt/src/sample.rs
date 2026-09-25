//! Time, the envelope, and the values a read returns (ridl §3.1, §4.5, §9),
//! with the two computations a consuming runtime makes on an envelope: a
//! value's freshness ([`Freshness::of`]) and event loss ([`EventSeqTracker`]).

use crate::contract::{InterfaceNo, Ordinal, Timing};
use crate::payload::Violation;

/// A point in time, in microseconds since the PTP epoch, on the TAI time scale
/// (ridl §3.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Timestamp(pub i64);

/// A length of time, in microseconds.
///
/// This is not `core::time::Duration`: generated code writes
/// `ridl_rt::sample::Duration` in full.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Duration(pub i64);

/// The sender's timestamp and sequence number (ridl §3.1). The runtime stamps
/// it from its clock, and no relay changes it.
///
/// Before a signal's first publication, its envelope has `seq` 0 and the time
/// at which the channel was created. On a call, `seq` is unique for each
/// caller, not for each channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Envelope {
    /// When the sender published, raised or called the instance (ridl §3.1).
    pub stamp: Timestamp,
    /// The sender's sequence number. On an event channel, a gap is a loss
    /// (ridl §3.1).
    pub seq: u64,
}

/// Where a signal's value comes from (ridl §4.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provenance {
    /// No publication yet: the value is the init value.
    Init,
    /// The value is the latest publication.
    Live,
    /// The channel is in the invalid state: the value is the last good value,
    /// or the init value when there is none.
    Invalid(Cause),
}

/// Why a channel is invalid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cause {
    /// The provider declared the invalid state (ridl §4.5).
    Declared,
    /// The consumer's binding detected an invalid payload.
    Detected(Detection),
}

/// What a consumer's binding detected in a payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Detection {
    /// The payload breaks a typl constraint: `INVALID_VALUE`, ridl §10.2.
    InvalidValue(Violation),
    /// The payload is not a well-formed encoding: a serialization failure,
    /// ridl §10.3.
    Corrupt,
}

/// How old a value is, measured against its staleness bound (ridl §9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Freshness {
    /// Within the bound.
    Fresh,
    /// Older than the bound.
    Stale {
        /// How far past the bound the value is.
        by: Duration,
    },
    /// The value has no staleness bound: its member's timing has no `max`, as
    /// under `@[1s..]`. A signal with no `@` annotation is not unbounded,
    /// because it receives the default range (ridl §9.1).
    Unbounded,
}

impl Freshness {
    /// The freshness of a value stamped at `stamp`, read at `now`, against the
    /// `max` of its member's `timing` (ridl §4, §9; frame specification §8).
    ///
    /// `Fresh` while `now − stamp ≤ max`, `Stale { by: now − stamp − max }`
    /// past it, and `Unbounded` when `timing` is `None` or carries no `max`.
    /// `min` plays no part. Under a strict period `@Xms`, `max` holds the
    /// period, so the period is the bound. A `stamp` later than `now` gives a
    /// negative age, which is `Fresh`. The subtractions saturate at the ends
    /// of the `i64` range instead of overflowing.
    ///
    /// Pass the member's `timing` as the descriptor holds it:
    /// `Freshness::of(envelope.stamp, now, member.timing)`.
    pub fn of(stamp: Timestamp, now: Timestamp, timing: Option<Timing>) -> Freshness {
        let Some(max) = timing.and_then(|timing| timing.max) else {
            return Freshness::Unbounded;
        };
        let age = now.0.saturating_sub(stamp.0);
        if age <= max.0 {
            Freshness::Fresh
        } else {
            Freshness::Stale {
                by: Duration(age.saturating_sub(max.0)),
            }
        }
    }
}

/// A signal value with its provenance, its freshness and its envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample<T> {
    /// The value. Never absent: the init value under `Init`, and the last good
    /// value or the init value under `Invalid`.
    pub value: T,
    /// Where the value comes from.
    pub provenance: Provenance,
    /// How old the value is.
    pub freshness: Freshness,
    /// The sender's timestamp and sequence number.
    pub envelope: Envelope,
}

impl<T> Sample<T> {
    /// `true` when the provenance is `Live` and the freshness is not `Stale`.
    pub fn usable(&self) -> bool {
        matches!(self.provenance, Provenance::Live)
            && !matches!(self.freshness, Freshness::Stale { .. })
    }
}

/// An event occurrence as a consumer receives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Occurrence<T> {
    /// The payload, or what the binding detected when the payload failed its
    /// check.
    pub payload: Result<T, Detection>,
    /// The sender's timestamp and sequence number.
    pub envelope: Envelope,
}

/// What [`EventSeqTracker::observe`] found when it compared an occurrence's
/// `seq` with the last one its channel accepted (ridl §3.1; frame
/// specification §7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Continuity {
    /// The first occurrence the tracker has seen on this channel. No loss is
    /// reported, because a consumer receives only the occurrences raised after
    /// its subscription was answered (frame specification §6.2), so the first
    /// one may carry any `seq`.
    First,
    /// The `seq` is one more than the last: nothing was lost.
    Next,
    /// The `seq` is more than one past the last: `count` occurrences between
    /// the two were lost (ridl §3.1: "sequence gaps make loss detectable").
    Lost {
        /// How many sequence numbers the gap skipped: `seq − last − 1`.
        count: u64,
    },
    /// The `seq` is not greater than the last: a duplicate or a reordered
    /// occurrence. The tracker keeps `last`. What the caller does with the
    /// occurrence is the caller's decision.
    NotNewer {
        /// The last `seq` the channel accepted, unchanged.
        last: u64,
    },
}

/// The error [`EventSeqTracker::observe`] returns when the occurrence is on a
/// channel the tracker does not hold and every one of its `N` slots is taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrackerFull;

/// The last `seq` accepted on each event channel of one session, and the loss
/// each new occurrence reveals (ridl §3.1; frame specification §5.2 and §7).
///
/// The sequence counter of an event is per channel, per provider instance, and
/// starts over with each session (frame specification §3, §7). A channel is
/// one `(InterfaceNo, Ordinal)` in the session's catalog, so the tracker keys
/// on that pair and never on the interface alone: two channels of one
/// interface whose occurrences interleave are not a loss. One tracker serves
/// one session, and so one provider instance; a consumer holding several
/// sessions holds one tracker for each, and a new session starts with a new
/// tracker.
///
/// The storage is the caller's: `N` slots, one per channel, held inline with
/// no allocation. A channel that finds no free slot is refused with
/// [`TrackerFull`]; [`forget`](Self::forget) frees a slot, for example when
/// the consumer unsubscribes (frame specification §6.2).
///
/// Feed the tracker every occurrence the channel accepts, in the order
/// received. A loss is a gap between two accepted occurrences (ridl §3.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventSeqTracker<const N: usize> {
    slots: [Option<Channel>; N],
}

/// One tracked channel and the last `seq` it accepted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Channel {
    interface: InterfaceNo,
    ordinal: Ordinal,
    last: u64,
}

impl<const N: usize> EventSeqTracker<N> {
    /// A tracker holding no channel.
    pub const fn new() -> Self {
        EventSeqTracker { slots: [None; N] }
    }

    /// Records `seq` as the latest occurrence on the channel
    /// `(interface, ordinal)` and reports what it reveals.
    ///
    /// The first occurrence on a channel takes a free slot and reports
    /// [`Continuity::First`]. A later one reports [`Continuity::Next`] or
    /// [`Continuity::Lost`] and becomes the channel's last `seq`, or reports
    /// [`Continuity::NotNewer`] and leaves the last `seq` as it was.
    ///
    /// # Errors
    ///
    /// [`TrackerFull`] when the channel is not tracked and no slot is free.
    /// Every tracked channel is unchanged.
    pub fn observe(
        &mut self,
        interface: InterfaceNo,
        ordinal: Ordinal,
        seq: u64,
    ) -> Result<Continuity, TrackerFull> {
        let mut free = None;
        for (index, slot) in self.slots.iter_mut().enumerate() {
            match slot {
                Some(channel) if channel.interface == interface && channel.ordinal == ordinal => {
                    let last = channel.last;
                    if seq <= last {
                        return Ok(Continuity::NotNewer { last });
                    }
                    channel.last = seq;
                    let count = seq - last - 1;
                    return Ok(if count == 0 {
                        Continuity::Next
                    } else {
                        Continuity::Lost { count }
                    });
                }
                None if free.is_none() => free = Some(index),
                _ => {}
            }
        }
        let index = free.ok_or(TrackerFull)?;
        self.slots[index] = Some(Channel {
            interface,
            ordinal,
            last: seq,
        });
        Ok(Continuity::First)
    }

    /// Stops tracking the channel `(interface, ordinal)` and frees its slot.
    /// The next occurrence on that channel reports [`Continuity::First`]. A
    /// channel the tracker does not hold is left alone.
    pub fn forget(&mut self, interface: InterfaceNo, ordinal: Ordinal) {
        for slot in &mut self.slots {
            if matches!(slot, Some(channel) if channel.interface == interface && channel.ordinal == ordinal)
            {
                *slot = None;
            }
        }
    }
}

impl<const N: usize> Default for EventSeqTracker<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{Cause, Detection, Duration, Envelope, Freshness, Provenance, Sample, Timestamp};
    use crate::payload::{Rule, Violation};

    fn sample(provenance: Provenance, freshness: Freshness) -> Sample<u8> {
        Sample {
            value: 0,
            provenance,
            freshness,
            envelope: Envelope {
                stamp: Timestamp(0),
                seq: 0,
            },
        }
    }

    #[test]
    fn only_a_live_value_that_is_not_stale_is_usable() {
        let violation = Violation {
            type_name: "Speed",
            rule: Rule::Range,
        };
        let provenances = [
            Provenance::Init,
            Provenance::Live,
            Provenance::Invalid(Cause::Declared),
            Provenance::Invalid(Cause::Detected(Detection::InvalidValue(violation))),
            Provenance::Invalid(Cause::Detected(Detection::Corrupt)),
        ];
        let freshnesses = [
            Freshness::Fresh,
            Freshness::Stale { by: Duration(1) },
            Freshness::Unbounded,
        ];
        for provenance in provenances {
            for freshness in freshnesses {
                let expected =
                    provenance == Provenance::Live && !matches!(freshness, Freshness::Stale { .. });
                assert_eq!(
                    sample(provenance, freshness).usable(),
                    expected,
                    "{provenance:?} with {freshness:?}"
                );
            }
        }
    }
}
