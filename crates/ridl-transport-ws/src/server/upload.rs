//! One parameter stream received from the client.

use crate::frame::INITIAL_CREDIT;
use ridl_rt::error::{CallError, Transport};
use ridl_rt::port::{Correlation, PushError, StreamCaller, StreamNo};
use std::collections::VecDeque;
use std::time::Instant;

/// The receiving state of one parameter stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UploadState {
    /// The stream accepts chunks until the client's end arrives.
    Open,
    /// The backend accepted the client's end after all queued chunks.
    Ended,
    /// A refusal or interruption stopped the stream.
    Stopped,
}
/// One parameter stream and its bounded queue of chunks awaiting the backend.
#[derive(Clone)]
pub(super) struct Upload {
    /// The parameter stream number within its call.
    pub(super) no: StreamNo,
    /// The sequence of the last accepted chunk or end; 0 before either.
    pub(super) seq_in: u64,
    /// The number of chunks the client may still send.
    pub(super) credit: u32,
    /// Received chunks not yet accepted by the backend, in order.
    pub(super) queue: VecDeque<Vec<u8>>,
    /// Whether the client sent an end after its final chunk.
    pub(super) end_received: bool,
    /// Whether the stream is open, ended or stopped.
    pub(super) state: UploadState,
    /// The last receive or backend progress, or the request time before either.
    pub(super) last_activity: Instant,
}
/// The result of receiving one parameter stream frame.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Receive {
    /// The frame was accepted for delivery to the backend.
    Queued,
    /// The frame did not carry the next expected sequence.
    Gap,
    /// A chunk arrived without available credit.
    CreditExceeded,
    /// The stream was stopped or had already received its end.
    Dropped,
}
/// Backend progress during one attempt to drain an upload.
pub(super) struct Drain {
    /// The number of queued chunks the backend accepted, returning this credit.
    pub(super) taken: u32,
    /// The terminal result, or `None` while the stream remains open.
    pub(super) end: Option<DrainEnd>,
}
/// Why draining an upload reached a terminal state.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum DrainEnd {
    /// The backend accepted the client's end.
    Ended,
    /// The backend refused a chunk or end with this call error.
    Refused(CallError),
    /// The backend no longer has an attached stream for this call.
    Detached,
}
impl Upload {
    pub(super) fn new(no: StreamNo, now: Instant) -> Self {
        Self {
            no,
            seq_in: 0,
            credit: INITIAL_CREDIT,
            queue: VecDeque::new(),
            end_received: false,
            state: UploadState::Open,
            last_activity: now,
        }
    }
    pub(super) fn receive_chunk(&mut self, seq: u64, payload: &[u8], now: Instant) -> Receive {
        if self.state != UploadState::Open || self.end_received {
            return Receive::Dropped;
        }
        if self.credit == 0 {
            return Receive::CreditExceeded;
        }
        if self.seq_in.checked_add(1) != Some(seq) {
            return Receive::Gap;
        }
        self.seq_in = seq;
        self.credit -= 1;
        self.queue.push_back(payload.to_vec());
        self.last_activity = now;
        Receive::Queued
    }
    pub(super) fn receive_end(&mut self, seq: u64, now: Instant) -> Receive {
        if self.state != UploadState::Open || self.end_received {
            return Receive::Dropped;
        }
        if self.seq_in.checked_add(1) != Some(seq) {
            return Receive::Gap;
        }
        self.seq_in = seq;
        self.end_received = true;
        self.last_activity = now;
        Receive::Queued
    }
    pub(super) fn drain(
        &mut self,
        c: Correlation,
        ports: &mut dyn StreamCaller,
        now: Instant,
    ) -> Drain {
        let mut drain = Drain {
            taken: 0,
            end: None,
        };
        if self.state != UploadState::Open {
            return drain;
        }
        while let Some(chunk) = self.queue.front() {
            match ports.push(c, self.no, chunk) {
                Ok(0) if !chunk.is_empty() => break,
                Ok(_) => {
                    self.queue.pop_front();
                    drain.taken += 1;
                    self.credit += 1;
                    self.last_activity = now;
                }
                Err(e) => {
                    drain.end = Some(self.failed(e));
                    return drain;
                }
            }
        }
        if self.end_received && self.queue.is_empty() {
            drain.end = Some(match ports.end(c, self.no) {
                Ok(()) => {
                    self.state = UploadState::Ended;
                    DrainEnd::Ended
                }
                Err(e) => self.failed(e),
            });
        }
        drain
    }
    fn failed(&mut self, error: PushError) -> DrainEnd {
        self.state = UploadState::Stopped;
        self.queue.clear();
        let error = match error {
            PushError::Failed(e) => e,
            PushError::Contract(c) => CallError::Contract(c),
            PushError::TooLarge { .. } => CallError::Transport(Transport::Corrupt),
            PushError::Ended => CallError::Transport(Transport::Interrupted),
            _ => return DrainEnd::Detached,
        };
        DrainEnd::Refused(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ridl_loopback::Loopback;
    use ridl_rt::contract::{CatalogHash, CatalogRef, InterfaceNo, Ordinal};
    use ridl_rt::error::Contract;
    use ridl_rt::payload::{Rule, Violation};
    use ridl_rt::port::{Caller, ClaimId, Handler, Pull, StreamHandler};

    fn rig() -> (Upload, Loopback, Loopback, Correlation, ClaimId, Instant) {
        let mut c = Loopback::new(CatalogRef {
            name: "upload.test",
            hash: CatalogHash([7; 32]),
        });
        let mut p = c.attach();
        p.serve(InterfaceNo(1), &[Ordinal(1)]).unwrap();
        let call = c.command(InterfaceNo(1), Ordinal(1), &[], None).unwrap();
        let claim = p.next_claim(&mut []).unwrap().unwrap().id;
        let now = Instant::now();
        (Upload::new(StreamNo(1), now), c, p, call, claim, now)
    }
    fn four(u: &mut Upload, now: Instant) {
        for seq in 1..=4 {
            assert_eq!(u.receive_chunk(seq, &[seq as u8], now), Receive::Queued);
        }
    }
    #[test]
    fn a_fifth_chunk_without_credit_is_credit_exceeded() {
        let (mut u, _, _, _, _, now) = rig();
        four(&mut u, now);
        assert_eq!(u.receive_chunk(5, &[5], now), Receive::CreditExceeded);
    }
    #[test]
    fn a_chunk_taken_by_the_backend_returns_one_credit() {
        let (mut u, mut c, _, call, _, now) = rig();
        four(&mut u, now);
        assert_eq!(u.credit, 0);
        assert_eq!(u.drain(call, &mut c, now).taken, 4);
        assert_eq!(u.credit, 4);
        // An empty chunk is taken even while four non-empty chunks fill the window.
        assert_eq!(u.receive_chunk(5, &[], now), Receive::Queued);
        assert_eq!(u.drain(call, &mut c, now).taken, 1);
        assert_eq!(u.credit, 4);
    }
    #[test]
    fn a_full_backend_window_keeps_the_chunk_queued() {
        let (mut u, mut c, _, call, _, now) = rig();
        four(&mut u, now);
        assert_eq!(u.drain(call, &mut c, now).taken, 4);
        assert_eq!(u.receive_chunk(5, &[5], now), Receive::Queued);
        assert_eq!(u.drain(call, &mut c, now).taken, 0);
        assert_eq!(u.queue.iter().cloned().collect::<Vec<_>>(), vec![vec![5]]);
    }
    #[test]
    fn a_seq_gap_is_reported() {
        for end in [false, true] {
            let (mut u, _, _, _, _, now) = rig();
            assert_eq!(u.receive_chunk(1, &[1], now), Receive::Queued);
            let result = if end {
                u.receive_end(3, now)
            } else {
                u.receive_chunk(3, &[3], now)
            };
            assert_eq!(result, Receive::Gap);
        }
    }
    #[test]
    fn the_end_is_pushed_after_the_queued_chunks() {
        let (mut u, mut c, mut p, call, claim, now) = rig();
        four(&mut u, now);
        assert_eq!(u.drain(call, &mut c, now).taken, 4);
        for seq in 5..=6 {
            assert_eq!(u.receive_chunk(seq, &[seq as u8], now), Receive::Queued);
        }
        assert_eq!(u.receive_end(7, now), Receive::Queued);
        let d = u.drain(call, &mut c, now);
        assert_eq!(d.taken, 0);
        assert_eq!(d.end, None);
        let mut buf = [0; 1];
        for n in 1..=2 {
            assert_eq!(
                p.pull_claim(claim, StreamNo(1), &mut buf),
                Ok(Pull::Chunk(1))
            );
            assert_eq!(buf, [n]);
        }
        let d = u.drain(call, &mut c, now);
        assert_eq!(d.taken, 2);
        assert_eq!(d.end, Some(DrainEnd::Ended));
        for n in 3..=6 {
            assert_eq!(
                p.pull_claim(claim, StreamNo(1), &mut buf),
                Ok(Pull::Chunk(1))
            );
            assert_eq!(buf, [n]);
        }
        assert_eq!(p.pull_claim(claim, StreamNo(1), &mut buf), Ok(Pull::End));
    }
    #[test]
    fn a_receiver_refusal_is_refused_with_its_error() {
        let (mut u, mut c, mut p, call, claim, now) = rig();
        let error = CallError::Contract(Contract::InvalidValue(Violation {
            type_name: Some("Block"),
            rule: Rule::Range,
        }));
        p.abort_claim(claim, StreamNo(1), error).unwrap();
        assert_eq!(u.receive_chunk(1, &[1], now), Receive::Queued);
        assert_eq!(
            u.drain(call, &mut c, now).end,
            Some(DrainEnd::Refused(error))
        );
        assert_eq!(u.state, UploadState::Stopped);
        assert!(u.queue.is_empty());
    }
    #[test]
    fn a_stopped_upload_drops_frames() {
        let (mut u, _, _, _, _, now) = rig();
        u.state = UploadState::Stopped;
        assert_eq!(u.receive_chunk(1, &[1], now), Receive::Dropped);
        assert_eq!(u.receive_end(1, now), Receive::Dropped);
        assert!(u.queue.is_empty());
        assert!(!u.end_received);
        assert_eq!(u.seq_in, 0);
    }
    #[test]
    fn an_upload_that_received_its_end_drops_frames() {
        let (mut u, _, _, _, _, now) = rig();
        assert_eq!(u.receive_end(1, now), Receive::Queued);
        assert_eq!(u.receive_chunk(2, &[2], now), Receive::Dropped);
        assert_eq!(u.receive_end(2, now), Receive::Dropped);
        assert!(u.queue.is_empty());
        assert_eq!(u.seq_in, 1);
    }
    #[test]
    fn the_credit_check_precedes_the_sequence_check() {
        let (mut u, _, _, _, _, now) = rig();
        four(&mut u, now);
        assert_eq!(u.receive_chunk(9, &[9], now), Receive::CreditExceeded);
    }
}
