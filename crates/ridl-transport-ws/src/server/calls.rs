//! The session's calls: the correlation map of the calls in flight, and the
//! settled responses kept for a repeated `request`.

use std::collections::VecDeque;
use std::time::Instant;

use ridl_rt::port::Correlation;

use crate::frame::Target;

/// The number of settled responses kept for a repeated `request`. The
/// server holds no descriptor, so it cannot keep a response until the
/// request's bound has passed (frame specification §7) and keeps a fixed
/// number instead.
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

#[derive(Default)]
pub(super) struct Calls {
    in_flight: Vec<InFlight>,
    /// The responses sent last, oldest first, each under the client's `seq`.
    settled: VecDeque<(u64, Vec<u8>)>,
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
        match self.settled.iter().find(|(s, _)| *s == seq) {
            Some((_, bytes)) => Seen::Settled(bytes.clone()),
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

    /// Keeps the response sent under `seq`, and lets the oldest go once more
    /// than [`RETAINED`] are kept.
    pub(super) fn settle(&mut self, seq: u64, response: &[u8]) {
        self.settled.push_back((seq, response.to_vec()));
        if self.settled.len() > RETAINED {
            self.settled.pop_front();
        }
    }
}
