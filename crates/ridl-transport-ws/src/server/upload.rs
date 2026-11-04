//! One parameter stream received from the client.

use crate::frame::INITIAL_CREDIT;
use ridl_rt::error::{CallError, Transport};
use ridl_rt::port::{Correlation, PushError, StreamCaller, StreamNo};
use std::collections::VecDeque;
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UploadState {
    Open,
    Ended,
    Stopped,
}
#[derive(Clone)]
pub(super) struct Upload {
    pub(super) no: StreamNo,
    pub(super) seq_in: u64,
    pub(super) credit: u32,
    pub(super) queue: VecDeque<Vec<u8>>,
    pub(super) end_received: bool,
    pub(super) state: UploadState,
    pub(super) last_activity: Instant,
}
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Receive {
    Queued,
    Gap,
    CreditExceeded,
    Dropped,
}
pub(super) struct Drain {
    pub(super) taken: u32,
    pub(super) end: Option<DrainEnd>,
}
#[derive(Debug, PartialEq, Eq)]
pub(super) enum DrainEnd {
    Ended,
    Refused(CallError),
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
}
