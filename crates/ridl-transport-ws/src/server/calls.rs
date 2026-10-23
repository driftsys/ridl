//! The session's calls: the correlation map of the calls in flight, and the
//! settled responses kept for a repeated `request`, each until its bound has
//! passed when the member table gives one, and the last 64 otherwise.

use std::collections::VecDeque;
use std::time::Instant;

use ridl_rt::port::Correlation;
use ridl_rt::sample::Timestamp;

use crate::frame::Target;

/// The number of settled responses kept when their bound is not known: a
/// member the table gives no `max`, or a server with no table. A stated
/// deviation from frame specification §11.1 item 6 (driftsys/ridl#986).
const RETAINED: usize = 64;

/// One call sent to the backend whose outcome is not known yet.
#[derive(Clone, Copy)]
pub(super) struct InFlight {
    /// The backend's name for the call.
    pub(super) correlation: Correlation,
    /// The command or query, as the client named it.
    pub(super) target: Target,
    /// The client's `seq`, which the response echoes as its correlation.
    pub(super) seq: u64,
    /// When the request arrived, which the abandon bound is measured from.
    pub(super) sent: Instant,
    /// The request's `stamp`, which the retention of its response is
    /// measured from (frame specification §7).
    pub(super) stamp: Timestamp,
    /// The backend's clock when the request arrived; caps the retention when
    /// `stamp` is ahead of it.
    pub(super) receipt: Timestamp,
}

/// Where a request's `seq` stands in the session.
pub(super) enum Seen {
    /// Above every `seq` seen: a new call.
    New,
    /// A call in flight: the request is not sent again.
    InFlight,
    /// A settled call whose response is kept: the same bytes are sent again.
    Settled(Vec<u8>),
    /// Not above the highest `seq` seen and not kept: dropped.
    Older,
}

/// One kept response.
struct Kept {
    /// The client's `seq` the response answers.
    seq: u64,
    /// The encoded `response`.
    bytes: Vec<u8>,
    /// The last instant of the backend's clock at which the response is
    /// kept, or `None` when its bound is not known.
    until: Option<Timestamp>,
}

#[derive(Default)]
pub(super) struct Calls {
    in_flight: Vec<InFlight>,
    /// The responses kept, oldest first.
    settled: VecDeque<Kept>,
    /// The number of entries of `settled` whose `until` is `None`.
    unbounded: usize,
    /// The sum of the lengths of the responses in `settled`.
    bytes: usize,
    /// The highest request `seq` taken as new.
    highest: u64,
}

impl Calls {
    pub(super) fn seen(&self, seq: u64) -> Seen {
        if seq > self.highest {
            return Seen::New;
        }
        if self.in_flight.iter().any(|call| call.seq == seq) {
            return Seen::InFlight;
        }
        match self.settled.iter().find(|kept| kept.seq == seq) {
            Some(kept) => Seen::Settled(kept.bytes.clone()),
            None => Seen::Older,
        }
    }

    /// Takes `seq` as the newest request. The caller has checked it is
    /// [`Seen::New`].
    pub(super) fn accept(&mut self, seq: u64) {
        self.highest = seq;
    }

    pub(super) fn track(&mut self, call: InFlight) {
        self.in_flight.push(call);
    }

    /// A copy of the calls in flight, for a pass that settles some of them.
    pub(super) fn in_flight(&self) -> Vec<InFlight> {
        self.in_flight.clone()
    }

    pub(super) fn remove(&mut self, correlation: Correlation) {
        self.in_flight
            .retain(|call| call.correlation != correlation);
    }

    /// Every correlation still in flight, for the end of the session.
    pub(super) fn drain(&mut self) -> impl Iterator<Item = Correlation> + '_ {
        self.in_flight.drain(..).map(|call| call.correlation)
    }

    /// Keeps the response sent under `seq`. With `Some(until)` it is kept
    /// until the backend's clock passes `until`; with `None` it is one of the
    /// last [`RETAINED`] such responses.
    pub(super) fn settle(&mut self, seq: u64, response: &[u8], until: Option<Timestamp>) {
        self.bytes += response.len();
        self.settled.push_back(Kept {
            seq,
            bytes: response.to_vec(),
            until,
        });
        if until.is_some() {
            return;
        }
        self.unbounded += 1;
        if self.unbounded > RETAINED {
            if let Some(oldest) = self.settled.iter().position(|kept| kept.until.is_none()) {
                if let Some(kept) = self.settled.remove(oldest) {
                    self.bytes -= kept.bytes.len();
                    self.unbounded -= 1;
                }
            }
        }
    }

    /// Lets go every response kept until an instant before `now`.
    pub(super) fn expire(&mut self, now: Timestamp) {
        let bytes = &mut self.bytes;
        self.settled.retain(|kept| match kept.until {
            Some(until) if now > until => {
                *bytes -= kept.bytes.len();
                false
            }
            _ => true,
        });
    }

    /// The bytes of every response kept.
    pub(super) fn retained_bytes(&self) -> usize {
        self.bytes
    }
}
