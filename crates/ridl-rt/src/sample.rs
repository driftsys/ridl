//! Time, the envelope, and the values a read returns (ridl §3.1, §4.5, §9).

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
    /// When the sender produced the value.
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
