//! Local claims forwarded to one remote provider, with session-owned identities.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use ridl_rt::contract::{default_ceiling, Kind, CALL_CEILING};
use ridl_rt::error::{CallError, Contract, Transport};
use ridl_rt::port::{ClaimId, Clock, Handler, ReadError, SettleError};
use ridl_rt::sample::Envelope;

use super::super::{Config, Note, ProvidedBackend};
use super::signals::record;
use crate::frame::{Frame, ResponseOutcome, Target};

struct Pending {
    claim: ClaimId,
    target: Target,
    deadline: Instant,
    stamp: ridl_rt::sample::Timestamp,
    bound: Option<Duration>,
}

pub(super) struct Calls {
    pending: BTreeMap<u64, Pending>,
    next: u64,
    short: Option<ClaimId>,
    pub(super) capacity: usize,
    config: Config,
}
impl Calls {
    pub(super) fn new(config: &Config) -> Self {
        Self {
            pending: BTreeMap::new(),
            next: 0,
            short: None,
            capacity: 64,
            config: config.clone(),
        }
    }
    pub(super) fn start(&self, backend: &mut impl ProvidedBackend) -> bool {
        let targets: Vec<_> = backend
            .members()
            .iter()
            .map(|(t, _, _)| t)
            .filter(|t| matches!(t.kind, Kind::Command | Kind::Query))
            .collect();
        for target in targets {
            if backend.ports().serve(target.iface, &[target.ord]).is_err() {
                record(&self.config, Note::PortRefused { target });
                return false;
            }
        }
        true
    }
    pub(super) fn tick(
        &mut self,
        backend: &mut impl ProvidedBackend,
        now: Instant,
        out: &mut Vec<Vec<u8>>,
    ) {
        self.expire(backend, now);
        let mut bytes = vec![0; CALL_CEILING];
        while self.pending.len() < self.capacity && self.next < u64::MAX {
            let claim = match backend.ports().next_claim(&mut bytes) {
                Ok(Some(claim)) => claim,
                Err(ReadError::ShortClaim { claim, .. }) => {
                    // ShortClaim remains queued until settled; stop if the port refuses.
                    if backend
                        .ports()
                        .settle(claim, Err(CallError::Transport(Transport::Corrupt)))
                        .is_err()
                    {
                        self.short = Some(claim);
                        break;
                    }
                    self.short = None;
                    continue;
                }
                _ => break,
            };
            let member = backend.members().member(claim.iface, claim.ord);
            let kind = member.map(|(kind, _)| kind).unwrap_or(Kind::Command);
            let target = Target {
                kind,
                iface: claim.iface,
                ord: claim.ord,
            };
            let limit = backend
                .members()
                .limit(claim.iface, claim.ord)
                .map(|n| n as usize)
                .unwrap_or(default_ceiling(kind));
            let max = member.and_then(|(_, max)| max);
            let bound = max.map(|max| {
                let age = backend
                    .ports()
                    .now()
                    .0
                    .saturating_sub(claim.envelope.stamp.0)
                    .max(0) as u64;
                max.saturating_sub(Duration::from_micros(age))
            });
            let remaining = match (bound, claim.remaining) {
                (Some(a), Some(b)) => a.min(Duration::from_micros(b.0.max(0) as u64)),
                (Some(a), None) => a,
                (None, Some(b)) => Duration::from_micros(b.0.max(0) as u64),
                (None, None) => self.config.call_abandon_after,
            };
            let seq = self.next + 1;
            self.pending.insert(
                seq,
                Pending {
                    claim: claim.id,
                    target,
                    deadline: now + remaining,
                    stamp: claim.envelope.stamp,
                    bound: max,
                },
            );
            let frame = Frame::Request {
                target,
                envelope: Envelope {
                    stamp: claim.envelope.stamp,
                    seq,
                },
                payload: &bytes[..claim.len],
            };
            let error = if !matches!(kind, Kind::Command | Kind::Query)
                || member.is_none()
                || backend.members().reply_stream(target.iface, target.ord)
            {
                Some(CallError::Contract(Contract::UnknownInteraction))
            } else if claim.remaining.is_some_and(|remaining| remaining.0 < 0)
                || max.is_some_and(|max| {
                    backend
                        .ports()
                        .now()
                        .0
                        .saturating_sub(claim.envelope.stamp.0)
                        > i64::try_from(max.as_micros()).unwrap_or(i64::MAX)
                })
            {
                Some(CallError::Transport(if kind == Kind::Command {
                    Transport::Undelivered
                } else {
                    Transport::Timeout
                }))
            } else if claim.len > limit || frame.encoded_len() > self.config.max_message_bytes() {
                Some(CallError::Transport(Transport::Corrupt))
            } else {
                None
            };
            if let Some(error) = error {
                self.settle(backend, seq, Err(error));
                if self.pending.contains_key(&seq) {
                    self.next = seq;
                    break;
                }
            } else {
                let mut encoded = vec![0; frame.encoded_len()];
                frame.encode(&mut encoded).expect("request fits");
                self.next = seq;
                out.push(encoded);
            }
        }
    }
    pub(super) fn response(
        &mut self,
        backend: &mut impl ProvidedBackend,
        target: Target,
        correlation: u64,
        outcome: ResponseOutcome<'_>,
        now: Instant,
    ) {
        self.expire(backend, now);
        if !self
            .pending
            .get(&correlation)
            .is_some_and(|p| p.target == target)
        {
            record(&self.config, Note::UnknownInteraction { target });
            return;
        }
        let limit = backend
            .members()
            .limit(target.iface, target.ord)
            .map(|n| n as usize)
            .unwrap_or(default_ceiling(target.kind));
        let result = match outcome {
            ResponseOutcome::Accepted if target.kind == Kind::Command => Ok(&[][..]),
            ResponseOutcome::Reply(bytes) if target.kind == Kind::Query && bytes.len() <= limit => {
                Ok(bytes)
            }
            ResponseOutcome::Contract(c) => Err(CallError::Contract(c)),
            ResponseOutcome::Busy => Err(CallError::Transport(Transport::Busy)),
            _ => Err(CallError::Transport(Transport::Corrupt)),
        };
        self.settle(backend, correlation, result);
    }
    fn settle(
        &mut self,
        backend: &mut impl ProvidedBackend,
        seq: u64,
        result: Result<&[u8], CallError>,
    ) {
        let Some(p) = self.pending.get(&seq) else {
            return;
        };
        let mut settled = backend.ports().settle(p.claim, result);
        if matches!(settled, Err(SettleError::TooLarge { .. })) {
            settled = backend
                .ports()
                .settle(p.claim, Err(CallError::Transport(Transport::Corrupt)));
        }
        if matches!(settled, Ok(()) | Err(SettleError::UnknownClaim)) {
            self.pending.remove(&seq);
        }
    }
    fn expire(&mut self, backend: &mut impl ProvidedBackend, now: Instant) {
        let due: Vec<_> = self
            .pending
            .iter()
            .filter(|(_, p)| {
                now > p.deadline
                    || p.bound.is_some_and(|max| {
                        let age = backend.ports().now().0.saturating_sub(p.stamp.0);
                        age > i64::try_from(max.as_micros()).unwrap_or(i64::MAX)
                    })
            })
            .map(|(&seq, p)| {
                (
                    seq,
                    if p.target.kind == Kind::Command {
                        Transport::Undelivered
                    } else {
                        Transport::Timeout
                    },
                )
            })
            .collect();
        for (seq, error) in due {
            self.settle(backend, seq, Err(CallError::Transport(error)));
        }
    }
    pub(super) fn down(&mut self, backend: &mut impl ProvidedBackend) {
        for p in self.pending.values() {
            let _ = backend
                .ports()
                .settle(p.claim, Err(CallError::Transport(Transport::Down)));
        }
        self.pending.clear();
        if let Some(claim) = self.short.take() {
            let _ = backend
                .ports()
                .settle(claim, Err(CallError::Transport(Transport::Down)));
        }
    }
}
