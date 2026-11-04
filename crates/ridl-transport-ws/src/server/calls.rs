//! The session's calls: the correlation map of the calls in flight, and the
//! settled responses kept for a repeated `request`, each until its bound has
//! passed when the member table gives one, and the last 64 otherwise.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::time::Instant;

use ridl_rt::port::Correlation;
use ridl_rt::sample::Timestamp;

use super::upload::Upload;
use crate::frame::Target;

/// The number of settled responses kept when their bound is not known: a
/// member the table gives no `max`, or a server with no table. A stated
/// deviation from frame specification §11.1 item 6 (driftsys/ridl#986).
const RETAINED: usize = 64;

/// One backend call with a pending outcome or an open parameter stream.
#[derive(Clone)]
pub(super) struct InFlight {
    /// The backend's name for the call.
    pub(super) correlation: Correlation,
    /// The command or query, as the client named it.
    pub(super) target: Target,
    /// The client's `seq`, which the response echoes as its correlation.
    pub(super) seq: u64,
    /// When the request arrived, or the upload end arrived for a query with
    /// a plain reply; the abandon bound is measured from this instant.
    pub(super) sent: Instant,
    /// The request's stamp, replaced by the upload end's stamp for a query
    /// with a plain reply; response retention is measured from it.
    pub(super) stamp: Timestamp,
    /// The backend's clock at the same receipt as `stamp`; caps retention
    /// when the sender's stamp is ahead of it.
    pub(super) receipt: Timestamp,
    /// The state of the reply stream, for a query whose reply is a stream;
    /// `None` for a call answered by one `response`.
    pub(super) stream: Option<ReplyStream>,
    /// Parameter streams that remain owned after an accepted acknowledgment.
    pub(super) uploads: Vec<Upload>,
    /// Whether the response was sent, or the reply stream ended.
    pub(super) settled: bool,
}

/// The server's side of the reply stream of one call in flight.
#[derive(Clone)]
pub(super) struct ReplyStream {
    /// The `seq` of the last frame sent on the stream; 0 before the first.
    pub(super) seq_out: u64,
    /// The number of chunks the client allows the server to send.
    pub(super) credit: u32,
    /// When the last `chunk` was sent or the last `stream_credit` arrived, or
    /// the request arrived before either: the abandon bound of a streamed
    /// call is measured from it.
    pub(super) last_activity: Instant,
    /// One chunk pulled from the backend while the credit was 0, sent first
    /// when credit arrives. Pulling it is how the server learns, with no
    /// credit left, that the stream has ended.
    pub(super) held: Option<Vec<u8>>,
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
    /// The encoded `response`.
    bytes: Vec<u8>,
    /// The last instant of the backend's clock at which the response is
    /// kept, or `None` when its bound is not known. The same value indexes
    /// the entry in `Calls::by_until` or `Calls::ring`.
    until: Option<Timestamp>,
}

#[derive(Default)]
pub(super) struct Calls {
    in_flight: Vec<InFlight>,
    /// The responses kept, by the request `seq` each answers. A `seq` is
    /// settled at most once: the caller sends a request to the backend only
    /// when [`seen`](Self::seen) returned [`Seen::New`].
    settled: BTreeMap<u64, Kept>,
    /// The `seq` of every entry of `settled` with a bound, in the order the
    /// bounds pass: a later request may have an earlier bound.
    by_until: BTreeSet<(Timestamp, u64)>,
    /// The `seq` of every entry of `settled` with no bound, oldest first.
    ring: VecDeque<u64>,
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
        match self.settled.get(&seq) {
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

    /// The call whose request carried `seq`.
    pub(super) fn by_seq(&mut self, seq: u64) -> Option<&mut InFlight> {
        self.in_flight.iter_mut().find(|call| call.seq == seq)
    }

    /// Saves the state of a call still in flight.
    pub(super) fn save(&mut self, call: InFlight) {
        if let Some(current) = self.by_seq(call.seq) {
            *current = call;
        }
    }

    /// The call in flight whose reply is a stream and whose request carried
    /// `seq`.
    pub(super) fn streamed(&mut self, seq: u64) -> Option<&mut InFlight> {
        self.in_flight
            .iter_mut()
            .find(|call| call.seq == seq && call.stream.is_some())
    }

    /// Replaces the reply stream state of the call `correlation`, when it is
    /// still in flight.
    pub(super) fn update(&mut self, correlation: Correlation, stream: ReplyStream) {
        if let Some(call) = self
            .in_flight
            .iter_mut()
            .find(|call| call.correlation == correlation)
        {
            call.stream = Some(stream);
        }
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
        let replaced = self.settled.insert(
            seq,
            Kept {
                bytes: response.to_vec(),
                until,
            },
        );
        debug_assert!(replaced.is_none(), "seq {seq} settled twice");
        if let Some(old) = replaced {
            self.unkeep(seq, old);
        }
        match until {
            Some(until) => {
                self.by_until.insert((until, seq));
            }
            None => {
                self.ring.push_back(seq);
                if self.ring.len() > RETAINED {
                    if let Some(oldest) = self.ring.pop_front() {
                        self.forget(oldest);
                    }
                }
            }
        }
    }

    /// Lets go every response kept until an instant before `now`.
    pub(super) fn expire(&mut self, now: Timestamp) {
        while let Some(&(until, seq)) = self.by_until.first() {
            if now <= until {
                break;
            }
            self.by_until.pop_first();
            self.forget(seq);
        }
    }

    /// Takes the bytes and the index entry of `old`, the response kept under
    /// `seq` that a second settle has replaced, out of the count and out of
    /// `by_until` or `ring`. The caller settles a `seq` once, so this runs
    /// only when that contract is broken in a release build; the count and
    /// the indexes then follow the response kept.
    fn unkeep(&mut self, seq: u64, old: Kept) {
        self.bytes -= old.bytes.len();
        match old.until {
            Some(until) => {
                self.by_until.remove(&(until, seq));
            }
            None => self.ring.retain(|s| *s != seq),
        }
    }

    /// Removes the kept response of `seq` and its bytes from the count. The
    /// caller has removed `seq` from its index.
    fn forget(&mut self, seq: u64) {
        if let Some(kept) = self.settled.remove(&seq) {
            self.bytes -= kept.bytes.len();
        }
    }

    /// The bytes of every response kept.
    pub(super) fn retained_bytes(&self) -> usize {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settled(calls: &Calls, seq: u64) -> Option<Vec<u8>> {
        match calls.seen(seq) {
            Seen::Settled(bytes) => Some(bytes),
            _ => None,
        }
    }

    fn settle(calls: &mut Calls, seq: u64, len: usize, until: Option<i64>) {
        calls.accept(seq);
        calls.settle(seq, &vec![seq as u8; len], until.map(Timestamp));
    }

    /// A later request may have an earlier bound (a shorter `max`, or a
    /// stamp behind the clock), so expiry is by `until`, not by `seq`; two
    /// responses may share a bound, and one pass lets go every response due.
    ///
    /// Mutations: expire from the front by `seq`; expire one response per
    /// pass.
    #[test]
    fn expiry_follows_the_bound_not_the_seq_and_the_bytes_stay_exact() {
        let mut calls = Calls::default();
        settle(&mut calls, 1, 10, Some(100));
        settle(&mut calls, 2, 20, Some(50));
        settle(&mut calls, 3, 30, None);
        settle(&mut calls, 4, 40, Some(100));
        assert_eq!(calls.retained_bytes(), 100);
        calls.expire(Timestamp(50)); // not after 50: nothing goes
        assert_eq!(calls.retained_bytes(), 100);
        calls.expire(Timestamp(51));
        assert_eq!(calls.retained_bytes(), 80);
        assert_eq!(settled(&calls, 1), Some(vec![1; 10]));
        assert!(matches!(calls.seen(2), Seen::Older));
        assert_eq!(settled(&calls, 3), Some(vec![3; 30]));
        assert_eq!(settled(&calls, 4), Some(vec![4; 40]));
        calls.expire(Timestamp(101)); // 1 and 4 are due in the same pass
        assert_eq!(calls.retained_bytes(), 30);
        assert!(matches!(calls.seen(1), Seen::Older));
        assert!(matches!(calls.seen(4), Seen::Older));
        assert_eq!(settled(&calls, 3), Some(vec![3; 30]));
    }

    /// The caller settles a `seq` once; a second settle is a defect of the
    /// caller, which a debug build reports.
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "seq 1 settled twice")]
    fn a_seq_settled_twice_is_a_defect_of_the_caller() {
        let mut calls = Calls::default();
        settle(&mut calls, 1, 5, None);
        calls.settle(1, &[9; 5], Some(Timestamp(10)));
    }

    /// Mutations: leave the replaced bytes in the count; leave the ring or
    /// the by-bound entry in place; swap the two arms.
    #[test]
    fn unkeep_takes_a_replaced_response_out_of_the_count_and_its_index() {
        let mut calls = Calls::default();
        settle(&mut calls, 1, 5, None);
        settle(&mut calls, 2, 7, Some(100));
        let old = calls.settled.remove(&1).expect("kept");
        calls.unkeep(1, old);
        assert_eq!(calls.retained_bytes(), 7);
        assert!(calls.ring.is_empty());
        assert_eq!(calls.by_until.len(), 1);
        let old = calls.settled.remove(&2).expect("kept");
        calls.unkeep(2, old);
        assert_eq!(calls.retained_bytes(), 0);
        assert!(calls.by_until.is_empty());
    }

    /// Mutation: count a bounded response against the ring.
    #[test]
    fn the_ring_evicts_only_responses_with_no_bound_and_counts_their_bytes_out() {
        let mut calls = Calls::default();
        settle(&mut calls, 1, 5, None);
        settle(&mut calls, 2, 7, Some(1_000));
        for seq in 3..=(RETAINED as u64 + 1) {
            settle(&mut calls, seq, 1, None);
        }
        assert_eq!(calls.retained_bytes(), 5 + 7 + (RETAINED - 1));
        // The 65th unbounded response pushes seq 1 out; the bounded one stays.
        settle(&mut calls, RETAINED as u64 + 2, 1, None);
        assert!(matches!(calls.seen(1), Seen::Older));
        assert_eq!(settled(&calls, 2), Some(vec![2; 7]));
        assert_eq!(calls.retained_bytes(), 7 + RETAINED);
    }
}
