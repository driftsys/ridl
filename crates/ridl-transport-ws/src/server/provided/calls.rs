//! Local claims forwarded to one remote provider, with session-owned identities.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use super::super::session::Step;
use ridl_rt::contract::{Kind, CALL_CEILING, FIXED_CEILING};
use ridl_rt::error::{CallError, Contract, Transport};
use ridl_rt::port::{ClaimId, Clock, Handler, ReadError, SettleError};
use ridl_rt::sample::Envelope;

use super::super::{Config, Note, ProvidedBackend};
use super::signals::record;
use crate::frame::{Frame, ResponseOutcome, Target};

struct Pending {
    claim: ClaimId,
    target: Target,
    // None means the deadline lies beyond the representable Instant range.
    deadline: Option<Instant>,
    stamp: ridl_rt::sample::Timestamp,
    bound: Option<Duration>,
}

pub(super) struct Calls {
    pending: BTreeMap<u64, Pending>,
    next: u64,
    short: Option<ClaimId>,
    pub(super) capacity: usize,
    bytes: Vec<u8>,
    config: Config,
}
impl Calls {
    pub(super) fn new(config: &Config) -> Self {
        Self {
            pending: BTreeMap::new(),
            next: 0,
            short: None,
            capacity: 64,
            bytes: vec![0; CALL_CEILING],
            config: config.clone(),
        }
    }
    pub(super) fn start(&mut self, backend: &mut impl ProvidedBackend) -> bool {
        let targets: Vec<_> = backend
            .members()
            .iter()
            .map(|(t, _, _)| t)
            .filter(|t| matches!(t.kind, Kind::Command | Kind::Query))
            .collect();
        // The claim buffer holds the largest request any member allows: the
        // default ceiling of a call, or more when an entry raises a member,
        // up to the largest payload. A catalog that raises nothing keeps the
        // default size.
        let largest = targets
            .iter()
            .map(|t| backend.members().payload_limit(*t))
            .max()
            .unwrap_or(0);
        self.bytes
            .resize(largest.clamp(CALL_CEILING, FIXED_CEILING), 0);
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
    ) -> Step {
        let local_now = backend.ports().now();
        self.expire(backend, now, local_now);
        if !backend
            .members()
            .iter()
            .any(|(target, _, _)| matches!(target.kind, Kind::Command | Kind::Query))
        {
            return Step::Continue;
        }
        while self.pending.len() < self.capacity && self.next < u64::MAX {
            let claim = match backend.ports().next_claim(&mut self.bytes) {
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
                Err(ReadError::Detached) => return Step::Close(1001),
                _ => break,
            };
            let member = backend.members().member(claim.iface, claim.ord);
            let kind = member.map(|(kind, _)| kind).unwrap_or(Kind::Command);
            let target = Target {
                kind,
                iface: claim.iface,
                ord: claim.ord,
            };
            let limit = backend.members().payload_limit(target);
            let max = member.and_then(|(_, max)| max);
            let age = local_now.0.saturating_sub(claim.envelope.stamp.0);
            let bound = max.map(|max| max.saturating_sub(Duration::from_micros(age.max(0) as u64)));
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
                    deadline: now.checked_add(remaining),
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
                payload: &self.bytes[..claim.len],
            };
            let error = if !matches!(kind, Kind::Command | Kind::Query)
                || member.is_none()
                || backend.members().reply_stream(target.iface, target.ord)
            {
                Some(CallError::Contract(Contract::UnknownInteraction))
            } else if claim.remaining.is_some_and(|remaining| remaining.0 < 0)
                || max.is_some_and(|max| age > i64::try_from(max.as_micros()).unwrap_or(i64::MAX))
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
        Step::Continue
    }
    pub(super) fn response(
        &mut self,
        backend: &mut impl ProvidedBackend,
        target: Target,
        correlation: u64,
        outcome: ResponseOutcome<'_>,
        now: Instant,
    ) {
        let local_now = backend.ports().now();
        self.expire(backend, now, local_now);
        if !self
            .pending
            .get(&correlation)
            .is_some_and(|p| p.target == target)
        {
            record(
                &self.config,
                Note::UnknownCorrelation {
                    target,
                    correlation,
                },
            );
            return;
        }
        let limit = backend.members().payload_limit(target);
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
    fn expire(
        &mut self,
        backend: &mut impl ProvidedBackend,
        now: Instant,
        local_now: ridl_rt::sample::Timestamp,
    ) {
        let due: Vec<_> = self
            .pending
            .iter()
            .filter(|(_, p)| {
                p.deadline.is_some_and(|deadline| now > deadline)
                    || p.bound.is_some_and(|max| {
                        let age = local_now.0.saturating_sub(p.stamp.0);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::{MemberTable, Providing};
    use ridl_loopback::Loopback;
    use ridl_rt::contract::{CatalogHash, CatalogRef, InterfaceNo, Ordinal};
    use ridl_rt::port::Caller;

    #[test]
    fn ticks_read_claims_into_the_same_owned_buffer() {
        let config = Config::default();
        let ports = Loopback::new(CatalogRef {
            name: "calls",
            hash: CatalogHash([7; 32]),
        });
        let mut caller = ports.attach();
        let mut members = MemberTable::new("calls", CatalogHash([7; 32]));
        members.insert(InterfaceNo(1), Ordinal(3), Kind::Command, None);
        let mut backend = Providing::new(ports, members).unwrap();
        let mut calls = Calls::new(&config);
        assert!(calls.start(&mut backend));
        let allocation = calls.bytes.as_ptr();
        for value in [73, 91] {
            caller
                .command(InterfaceNo(1), Ordinal(3), &[value], None)
                .unwrap();
            let mut out = vec![];
            calls.tick(&mut backend, Instant::now(), &mut out);
            assert_eq!(out.len(), 1);
            assert_eq!(calls.bytes.as_ptr(), allocation);
            assert_eq!(calls.bytes[0], value);
        }
    }

    /// The length of the claim buffer after `start`, for a table whose
    /// command 3 and query 4 have the limits given, in that order, and none
    /// where `None`.
    fn buffer_after_start(command: Option<u32>, query: Option<u32>) -> usize {
        let ports = Loopback::new(CatalogRef {
            name: "calls",
            hash: CatalogHash([7; 32]),
        });
        let mut members = MemberTable::new("calls", CatalogHash([7; 32]));
        for (ord, kind, limit) in [(3, Kind::Command, command), (4, Kind::Query, query)] {
            members.insert(InterfaceNo(1), Ordinal(ord), kind, None);
            if let Some(limit) = limit {
                members.set_limit(InterfaceNo(1), Ordinal(ord), limit);
            }
        }
        let mut backend = Providing::new(ports, members).unwrap();
        let mut calls = Calls::new(&Config::default());
        assert!(calls.start(&mut backend));
        calls.bytes.len()
    }

    /// The claim buffer holds the largest limit of any command or query,
    /// whichever comes first in the table, and never less than 16,128 or more
    /// than 65,280 bytes.
    ///
    /// Mutation: take `min`, `next` or `last` instead of `max` over the limits.
    #[test]
    fn the_claim_buffer_is_the_largest_limit_clamped_to_the_call_range() {
        assert_eq!(buffer_after_start(Some(20_000), Some(40_000)), 40_000);
        assert_eq!(buffer_after_start(Some(40_000), Some(20_000)), 40_000);
        assert_eq!(buffer_after_start(Some(40_000), None), 40_000);
        assert_eq!(buffer_after_start(None, Some(40_000)), 40_000);
        assert_eq!(buffer_after_start(None, None), CALL_CEILING);
        assert_eq!(buffer_after_start(Some(100), Some(200)), CALL_CEILING);
        assert_eq!(buffer_after_start(Some(1_000_000), Some(5)), FIXED_CEILING);
    }

    #[test]
    fn the_claim_buffer_of_an_empty_table_is_the_default_size() {
        let ports = Loopback::new(CatalogRef {
            name: "calls",
            hash: CatalogHash([7; 32]),
        });
        let members = MemberTable::new("calls", CatalogHash([7; 32]));
        let mut backend = Providing::new(ports, members).unwrap();
        let mut calls = Calls::new(&Config::default());
        assert!(calls.start(&mut backend));
        assert_eq!(calls.bytes.len(), CALL_CEILING);
    }
}

#[cfg(test)]
mod regression_tests {
    use super::*;
    use crate::server::MemberTable;
    use ridl_rt::contract::{CatalogHash, CatalogRef, InterfaceNo, Ordinal};
    use ridl_rt::port::{Attached, Claim, Encoded, ServeError};
    use ridl_rt::sample::{Occurrence, Sample, Timestamp};
    use std::cell::Cell;
    use std::collections::VecDeque;

    struct Ports {
        catalog: CatalogRef,
        claims: VecDeque<Claim>,
        detached: bool,
        refuse_serve: bool,
        reads: usize,
        clock_reads: Cell<usize>,
        time: Timestamp,
        too_large: bool,
        outcomes: Vec<(ClaimId, Result<Vec<u8>, CallError>)>,
    }
    impl Attached for Ports {
        fn catalog(&self) -> &CatalogRef {
            &self.catalog
        }
    }
    impl Encoded for Ports {
        type Encoding = <ridl_loopback::Loopback as Encoded>::Encoding;
    }
    impl Clock for Ports {
        fn now(&self) -> Timestamp {
            self.clock_reads.set(self.clock_reads.get() + 1);
            self.time
        }
    }
    impl Handler for Ports {
        fn serve(&mut self, _: InterfaceNo, _: &[Ordinal]) -> Result<(), ServeError> {
            if self.refuse_serve {
                Err(ServeError::Detached)
            } else {
                Ok(())
            }
        }
        fn next_claim(&mut self, _: &mut [u8]) -> Result<Option<Claim>, ReadError> {
            self.reads += 1;
            if self.detached {
                Err(ReadError::Detached)
            } else {
                Ok(self.claims.pop_front())
            }
        }
        fn settle(
            &mut self,
            id: ClaimId,
            outcome: Result<&[u8], CallError>,
        ) -> Result<(), SettleError> {
            if self.too_large {
                self.too_large = false;
                return Err(SettleError::TooLarge { cap: 0 });
            }
            self.outcomes.push((id, outcome.map(Vec::from)));
            Ok(())
        }
    }
    struct Backend {
        ports: Ports,
        members: MemberTable,
    }
    impl ProvidedBackend for Backend {
        type Ports = Ports;
        fn ports(&mut self) -> &mut Ports {
            &mut self.ports
        }
        fn members(&self) -> &MemberTable {
            &self.members
        }
        fn sample(&mut self, _: Target, _: Sample<Option<&[u8]>>, _: &Config) {}
        fn occurrence(&mut self, _: Target, _: Occurrence<&[u8]>, _: Option<u64>, _: &Config) {}
        fn commit(&mut self, _: Timestamp) {}
    }
    fn backend(kind: Kind, max: Option<Duration>, remaining: Option<i64>) -> Backend {
        let mut members = MemberTable::new("test", CatalogHash([7; 32]));
        members.insert(InterfaceNo(1), Ordinal(1), kind, max);
        Backend {
            ports: Ports {
                catalog: CatalogRef {
                    name: "test",
                    hash: CatalogHash([7; 32]),
                },
                claims: VecDeque::from([Claim {
                    id: ClaimId(1),
                    iface: InterfaceNo(1),
                    ord: Ordinal(1),
                    envelope: Envelope {
                        stamp: Timestamp(0),
                        seq: 1,
                    },
                    trace: None,
                    remaining: remaining.map(ridl_rt::sample::Duration),
                    len: 0,
                }]),
                detached: false,
                refuse_serve: false,
                reads: 0,
                clock_reads: Cell::new(0),
                time: Timestamp(0),
                too_large: false,
                outcomes: vec![],
            },
            members,
        }
    }
    #[test]
    fn no_offered_calls_never_reads_the_handler_queue() {
        let mut b = backend(Kind::Signal, None, None);
        let mut calls = Calls::new(&Config::default());
        assert!(calls.start(&mut b));
        calls.tick(&mut b, Instant::now(), &mut vec![]);
        assert_eq!(b.ports.reads, 0);
        assert_eq!(b.ports.claims.len(), 1);
    }
    #[test]
    fn detached_handler_closes_and_cleanup_settles_owned_claims() {
        use super::super::{Driver, Provided, Step};
        let config = Config::default();
        let mut s = Provided::new(backend(Kind::Command, None, None), &config);
        assert!(s.calls.start(&mut s.backend));
        s.attached = true;
        s.tick(Instant::now(), &mut vec![]);
        s.backend.ports.detached = true;
        assert!(matches!(
            s.tick(Instant::now(), &mut vec![]),
            Step::Close(1001)
        ));
        s.calls.down(&mut s.backend);
        assert_eq!(
            s.backend.ports.outcomes,
            vec![(ClaimId(1), Err(CallError::Transport(Transport::Down)))]
        );
    }
    #[test]
    fn instant_overflow_retains_a_far_future_deadline() {
        let mut config = Config::default();
        config.call_abandon_after = Duration::MAX;
        for max in [None, Some(Duration::MAX)] {
            let mut calls = Calls::new(&config);
            let mut b = backend(Kind::Query, max, None);
            assert!(calls.start(&mut b));
            let now = Instant::now();
            let mut out = vec![];
            calls.tick(&mut b, now, &mut out);
            calls.tick(&mut b, now + Duration::from_secs(1), &mut out);
            assert_eq!(out.len(), 1);
            assert!(b.ports.outcomes.is_empty());
        }
    }
    #[test]
    fn local_remaining_and_table_bound_use_the_earlier_deadline() {
        for (max, remaining, duration) in [
            (None, Some(100), 100),
            (Some(Duration::from_micros(200)), Some(100), 100),
            (Some(Duration::from_micros(100)), Some(200), 100),
        ] {
            let mut b = backend(Kind::Query, max, remaining);
            let mut calls = Calls::new(&Config::default());
            assert!(calls.start(&mut b));
            let now = Instant::now();
            let mut out = vec![];
            calls.tick(&mut b, now, &mut out);
            assert_eq!(out.len(), 1);
            calls.tick(&mut b, now + Duration::from_micros(duration), &mut out);
            assert!(b.ports.outcomes.is_empty());
            calls.tick(
                &mut b,
                now + Duration::from_micros(duration) + Duration::from_nanos(1),
                &mut out,
            );
            assert_eq!(
                b.ports.outcomes,
                vec![(ClaimId(1), Err(CallError::Transport(Transport::Timeout)))]
            );
        }
    }
    #[test]
    fn expired_local_remaining_is_refused_before_forwarding_for_each_call_kind() {
        for (kind, error) in [
            (Kind::Command, Transport::Undelivered),
            (Kind::Query, Transport::Timeout),
        ] {
            let mut b = backend(kind, None, Some(-1));
            let mut calls = Calls::new(&Config::default());
            assert!(calls.start(&mut b));
            let mut out = vec![];
            calls.tick(&mut b, Instant::now(), &mut out);
            assert!(out.is_empty());
            assert_eq!(
                b.ports.outcomes,
                vec![(ClaimId(1), Err(CallError::Transport(error)))]
            );
        }
    }
    #[test]
    fn default_capacity_leaves_the_sixty_fifth_call_queued() {
        let mut b = backend(Kind::Command, None, None);
        let claim = b.ports.claims[0];
        b.ports.claims = (1..=65)
            .map(|id| Claim {
                id: ClaimId(id),
                ..claim
            })
            .collect();
        let mut calls = Calls::new(&Config::default());
        assert!(calls.start(&mut b));
        let mut out = vec![];
        calls.tick(&mut b, Instant::now(), &mut out);
        assert_eq!(out.len(), 64);
        assert_eq!(b.ports.claims.len(), 1);
    }
    #[test]
    fn too_large_reply_falls_back_to_corrupt_and_releases_pending_entry() {
        let mut b = backend(Kind::Query, None, None);
        let mut calls = Calls::new(&Config::default());
        assert!(calls.start(&mut b));
        let now = Instant::now();
        calls.tick(&mut b, now, &mut vec![]);
        b.ports.too_large = true;
        calls.response(
            &mut b,
            Target {
                kind: Kind::Query,
                iface: InterfaceNo(1),
                ord: Ordinal(1),
            },
            1,
            ResponseOutcome::Reply(&[1]),
            now,
        );
        assert_eq!(
            b.ports.outcomes,
            vec![(ClaimId(1), Err(CallError::Transport(Transport::Corrupt)))]
        );
        calls.down(&mut b);
        assert_eq!(b.ports.outcomes.len(), 1);
    }
    #[test]
    fn refused_handler_closes_1011_and_records_the_member() {
        use super::super::{Driver, Provided, Step};
        use crate::frame::AttachOutcome;
        let notes = std::sync::Arc::new(std::sync::Mutex::new(vec![]));
        let captured = notes.clone();
        let config =
            Config::default().with_record(move |note| captured.lock().unwrap().push(*note));
        let mut b = backend(Kind::Command, None, None);
        b.ports.refuse_serve = true;
        b.members
            .insert(InterfaceNo(2), Ordinal(1), Kind::Signal, None);
        let mut s = Provided::new(b, &config);
        let frame = Frame::Attached(AttachOutcome::Accepted);
        let mut bytes = vec![0; frame.encoded_len()];
        frame.encode(&mut bytes).unwrap();
        let mut out = vec![];
        assert!(matches!(
            s.on_message(&bytes, Instant::now(), &mut out, &config),
            Step::Close(1011)
        ));
        assert!(!s.attached);
        assert!(out.is_empty());
        assert_eq!(
            *notes.lock().unwrap(),
            vec![Note::PortRefused {
                target: Target {
                    kind: Kind::Command,
                    iface: InterfaceNo(1),
                    ord: Ordinal(1)
                }
            }]
        );
    }
    #[test]
    fn expired_table_command_is_undelivered_before_forwarding() {
        let mut b = backend(Kind::Command, Some(Duration::from_micros(1)), None);
        b.ports.time = Timestamp(2);
        let mut calls = Calls::new(&Config::default());
        assert!(calls.start(&mut b));
        let mut out = vec![];
        calls.tick(&mut b, Instant::now(), &mut out);
        assert!(out.is_empty());
        assert_eq!(
            b.ports.outcomes,
            vec![(
                ClaimId(1),
                Err(CallError::Transport(Transport::Undelivered))
            )]
        );
    }
    #[test]
    fn one_clock_read_per_tick_and_response_with_multiple_bounded_calls() {
        let mut b = backend(Kind::Query, Some(Duration::from_secs(1)), None);
        let claim = b.ports.claims[0];
        b.ports.claims.push_back(Claim {
            id: ClaimId(2),
            ..claim
        });
        let mut calls = Calls::new(&Config::default());
        assert!(calls.start(&mut b));
        let now = Instant::now();
        calls.tick(&mut b, now, &mut vec![]);
        assert_eq!(b.ports.clock_reads.get(), 1);
        calls.tick(&mut b, now, &mut vec![]);
        assert_eq!(b.ports.clock_reads.get(), 2);
        calls.response(
            &mut b,
            Target {
                kind: Kind::Query,
                iface: InterfaceNo(1),
                ord: Ordinal(1),
            },
            1,
            ResponseOutcome::Reply(&[]),
            now,
        );
        assert_eq!(b.ports.clock_reads.get(), 3);
    }
}
