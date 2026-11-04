//! Consumer dispositions and remote-stamp commit scheduling.

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use ridl_rt::contract::{default_ceiling, InterfaceNo, Kind, Ordinal};
use ridl_rt::port::Clock;
use ridl_rt::sample::{
    Cause, Detection, Envelope, Freshness, Occurrence, Provenance, Sample, Timestamp,
};

use super::super::{Config, Note, ProvidedBackend};
use crate::frame::{AnswerOutcome, Frame, PublishState, Target};

type Key = (InterfaceNo, Ordinal);
#[derive(Default)]
struct Channel {
    accepted: bool,
    answered: bool,
    seq: Option<u64>,
    good: Option<Vec<u8>>,
}

pub(super) struct Consumer {
    channels: BTreeMap<Key, Channel>,
    group: Option<(Timestamp, Instant)>,
    group_channels: BTreeSet<Key>,
    interval: Duration,
}
impl Consumer {
    pub(super) fn new(interval: Duration) -> Self {
        Self {
            channels: BTreeMap::new(),
            group: None,
            group_channels: BTreeSet::new(),
            interval,
        }
    }
    pub(super) fn subscribe(&mut self, backend: &impl ProvidedBackend, out: &mut Vec<Vec<u8>>) {
        for (target, _, _) in backend.members().iter() {
            if matches!(target.kind, Kind::Signal | Kind::Event) {
                self.channels
                    .insert((target.iface, target.ord), Channel::default());
                let f = Frame::Subscribe(target);
                let mut bytes = vec![0; f.encoded_len()];
                f.encode(&mut bytes).expect("subscribe fits");
                out.push(bytes);
            }
        }
    }
    pub(super) fn answer(&mut self, target: Target, outcome: AnswerOutcome) {
        if let Some(channel) = self.channels.get_mut(&(target.iface, target.ord)) {
            if !channel.answered {
                channel.answered = true;
                channel.accepted = matches!(outcome, AnswerOutcome::Accepted);
            }
        }
    }
    pub(super) fn publication<B: ProvidedBackend>(
        &mut self,
        backend: &mut B,
        target: Target,
        envelope: Envelope,
        state: PublishState<'_>,
        now: Instant,
        config: &Config,
    ) {
        let Some(channel) = self.channel(backend, target, Kind::Signal, envelope.seq, config)
        else {
            return;
        };
        let limit = backend
            .members()
            .limit(target.iface, target.ord)
            .map(|n| n as usize)
            .unwrap_or(default_ceiling(target.kind));
        let provenance = match state {
            PublishState::Init => {
                channel.good = None;
                Provenance::Init
            }
            PublishState::InvalidDeclared => Provenance::Invalid(Cause::Declared),
            PublishState::Live(bytes) if bytes.len() > limit => {
                Provenance::Invalid(Cause::Detected(Detection::Corrupt))
            }
            PublishState::Live(bytes) => {
                channel.good = Some(bytes.to_vec());
                Provenance::Live
            }
        };
        // Sequence validation precedes grouping, so a discarded frame cannot
        // flush a group or extend its idle deadline.
        self.begin(backend, target, envelope.stamp, now);
        let channel = &self.channels[&(target.iface, target.ord)];
        let max = backend
            .members()
            .member(target.iface, target.ord)
            .and_then(|(_, max)| max);
        let freshness = match max {
            None => Freshness::Unbounded,
            Some(max) => {
                let age = backend.ports().now().0.saturating_sub(envelope.stamp.0);
                let excess = age.saturating_sub(i64::try_from(max.as_micros()).unwrap_or(i64::MAX));
                if excess > 0 {
                    Freshness::Stale {
                        by: ridl_rt::sample::Duration(excess),
                    }
                } else {
                    Freshness::Fresh
                }
            }
        };
        backend.sample(
            target,
            Sample {
                value: channel.good.as_deref(),
                provenance,
                freshness,
                envelope,
            },
            config,
        );
    }
    pub(super) fn occurrence<B: ProvidedBackend>(
        &mut self,
        backend: &mut B,
        target: Target,
        envelope: Envelope,
        payload: &[u8],
        config: &Config,
    ) {
        let last = self
            .channels
            .get(&(target.iface, target.ord))
            .and_then(|c| c.seq);
        if self
            .channel(backend, target, Kind::Event, envelope.seq, config)
            .is_none()
        {
            return;
        }
        let gap =
            last.and_then(|last| (envelope.seq - last > 1).then_some(envelope.seq - last - 1));
        // Like EventSource::next, expiry is evaluated against the consumer's
        // clock. Sequence tracking still observes an expired occurrence.
        if let Some((_, Some(max))) = backend.members().member(target.iface, target.ord) {
            let age = backend.ports().now().0.saturating_sub(envelope.stamp.0);
            if age > i64::try_from(max.as_micros()).unwrap_or(i64::MAX) {
                if let Some(count) = gap {
                    record(config, Note::EventGap { target, count });
                }
                return;
            }
        }
        let limit = backend
            .members()
            .limit(target.iface, target.ord)
            .map(|n| n as usize)
            .unwrap_or(default_ceiling(target.kind));
        let payload = if payload.len() > limit {
            Err(Detection::Corrupt)
        } else {
            Ok(payload)
        };
        backend.occurrence(target, Occurrence { payload, envelope }, gap, config);
    }
    fn channel<'a>(
        &'a mut self,
        backend: &impl ProvidedBackend,
        target: Target,
        kind: Kind,
        seq: u64,
        config: &Config,
    ) -> Option<&'a mut Channel> {
        if target.kind != kind
            || !matches!(backend.members().member(target.iface,target.ord),Some((k,_)) if k==kind)
        {
            record(config, Note::UnknownInteraction { target });
            return None;
        }
        let channel = self.channels.get_mut(&(target.iface, target.ord))?;
        if !channel.accepted {
            record(config, Note::NotSubscribed { target });
            return None;
        }
        if channel.seq.is_some_and(|last| seq <= last) {
            record(config, Note::DuplicateDropped { seq });
            return None;
        }
        channel.seq = Some(seq);
        Some(channel)
    }
    fn begin(
        &mut self,
        backend: &mut impl ProvidedBackend,
        target: Target,
        stamp: Timestamp,
        now: Instant,
    ) {
        if self.group.is_some_and(|(held, _)| held != stamp)
            || self.group_channels.contains(&(target.iface, target.ord))
        {
            self.flush(backend)
        }
        self.group_channels.insert((target.iface, target.ord));
        self.group = Some((stamp, now));
    }
    fn flush(&mut self, backend: &mut impl ProvidedBackend) {
        if let Some((stamp, _)) = self.group.take() {
            self.group_channels.clear();
            backend.commit(stamp)
        }
    }
    pub(super) fn tick(&mut self, backend: &mut impl ProvidedBackend, now: Instant) {
        if self
            .group
            .is_some_and(|(_, last)| now.saturating_duration_since(last) >= self.interval)
        {
            self.flush(backend)
        }
    }
}
pub(super) fn record(config: &Config, note: Note) {
    if let Some(record) = config.record() {
        record(&note)
    }
}
