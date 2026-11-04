use super::*;
use crate::frame::{AnswerOutcome, PublishState, Target};
use ridl_loopback::Loopback;
use ridl_rt::contract::{CatalogHash, CatalogRef, InterfaceNo, Kind, Ordinal};
use ridl_rt::sample::{Cause, Detection, Envelope, Provenance, Sample, Timestamp};

const SIGNAL: Target = Target {
    kind: Kind::Signal,
    iface: InterfaceNo(1),
    ord: Ordinal(1),
};
const EVENT: Target = Target {
    kind: Kind::Event,
    iface: InterfaceNo(1),
    ord: Ordinal(2),
};
type SeenEvent = (Target, Result<Vec<u8>, Detection>, Envelope, Option<u64>);
#[derive(Default)]
struct Seen {
    samples: Vec<(Target, Provenance, Option<Vec<u8>>, Envelope)>,
    events: Vec<SeenEvent>,
    groups: Vec<Timestamp>,
    freshness: Vec<ridl_rt::sample::Freshness>,
}
struct Capture {
    ports: Loopback,
    table: super::super::MemberTable,
    seen: Seen,
}
impl ProvidedBackend for Capture {
    type Ports = Loopback;
    fn ports(&mut self) -> &mut Loopback {
        &mut self.ports
    }
    fn members(&self) -> &super::super::MemberTable {
        &self.table
    }
    fn sample(&mut self, t: Target, s: Sample<Option<&[u8]>>, _: &Config) {
        self.seen.freshness.push(s.freshness);
        self.seen
            .samples
            .push((t, s.provenance, s.value.map(<[u8]>::to_vec), s.envelope));
    }
    fn occurrence(
        &mut self,
        t: Target,
        e: ridl_rt::sample::Occurrence<&[u8]>,
        gap: Option<u64>,
        _: &Config,
    ) {
        self.seen
            .events
            .push((t, e.payload.map(<[u8]>::to_vec), e.envelope, gap));
    }
    fn commit(&mut self, stamp: Timestamp) {
        self.seen.groups.push(stamp);
    }
}
fn session() -> Provided<Capture> {
    let catalog = CatalogRef {
        name: "alpha",
        hash: CatalogHash([7; 32]),
    };
    let mut table = super::super::MemberTable::new(catalog.name, catalog.hash);
    for t in [
        SIGNAL,
        EVENT,
        Target {
            kind: Kind::Command,
            ord: Ordinal(3),
            ..SIGNAL
        },
    ] {
        table.insert(t.iface, t.ord, t.kind, None);
        table.set_limit(t.iface, t.ord, 2);
    }
    Provided::new(
        Capture {
            ports: Loopback::new(catalog),
            table,
            seen: Seen::default(),
        },
        &Config::default(),
    )
}
fn send<B: ProvidedBackend>(
    s: &mut Provided<B>,
    f: Frame<'_>,
    now: Instant,
) -> (Step, Vec<Vec<u8>>) {
    let mut bytes = vec![0; f.encoded_len()];
    f.encode(&mut bytes).unwrap();
    let mut out = vec![];
    let step = s.on_message(&bytes, now, &mut out, &Config::default());
    (step, out)
}
fn ready(s: &mut Provided<Capture>, now: Instant) {
    send(s, Frame::Attached(AttachOutcome::Accepted), now);
    for target in [SIGNAL, EVENT] {
        send(
            s,
            Frame::Answer {
                target,
                outcome: AnswerOutcome::Accepted,
            },
            now,
        );
    }
}
fn publish(target: Target, seq: u64, stamp: i64, state: PublishState<'_>) -> Frame<'_> {
    Frame::Publish {
        target,
        envelope: Envelope {
            stamp: Timestamp(stamp),
            seq,
        },
        state,
    }
}
#[test]
fn provider_subscribes_to_exact_offered_members() {
    let mut s = session();
    let now = Instant::now();
    let (_, out) = send(&mut s, Frame::Attached(AttachOutcome::Accepted), now);
    let frames: Vec<_> = out.iter().map(|b| frame::decode(b).unwrap()).collect();
    assert_eq!(frames, [Frame::Subscribe(SIGNAL), Frame::Subscribe(EVENT)]);
}
#[test]
fn publication_before_subscription_is_rejected() {
    let mut s = session();
    let now = Instant::now();
    send(&mut s, Frame::Attached(AttachOutcome::Accepted), now);
    send(
        &mut s,
        publish(SIGNAL, 1, 90, PublishState::Live(&[1])),
        now,
    );
    assert!(s.backend.seen.samples.is_empty());
    send(
        &mut s,
        Frame::Answer {
            target: SIGNAL,
            outcome: AnswerOutcome::Accepted,
        },
        now,
    );
    send(
        &mut s,
        publish(SIGNAL, 1, 90, PublishState::Live(&[2])),
        now,
    );
    assert_eq!(s.backend.seen.samples.len(), 1);
}
#[test]
fn invalidate_preserves_frame_rules() {
    let mut s = session();
    let now = Instant::now();
    ready(&mut s, now);
    for (seq, state) in [
        (0, PublishState::Init),
        (1, PublishState::Live(&[5, 6])),
        (2, PublishState::InvalidDeclared),
        (3, PublishState::Live(&[9; 3])),
        (4, PublishState::Live(&[7])),
    ] {
        send(&mut s, publish(SIGNAL, seq, 90 + seq as i64, state), now);
    }
    let seen = &s.backend.seen.samples;
    assert_eq!(seen.len(), 5);
    for (i, (provenance, value)) in [
        (Provenance::Init, None),
        (Provenance::Live, Some(vec![5, 6])),
        (Provenance::Invalid(Cause::Declared), Some(vec![5, 6])),
        (
            Provenance::Invalid(Cause::Detected(Detection::Corrupt)),
            Some(vec![5, 6]),
        ),
        (Provenance::Live, Some(vec![7])),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!((seen[i].1, seen[i].2.clone()), (provenance, value));
        assert_eq!(
            seen[i].3,
            Envelope {
                stamp: Timestamp(90 + i as i64),
                seq: i as u64
            }
        );
    }
}
#[test]
fn same_stamp_values_commit_together_and_idle_tick_flushes() {
    let mut s = session();
    let now = Instant::now();
    let second = Target {
        ord: Ordinal(5),
        ..SIGNAL
    };
    s.backend
        .table
        .insert(second.iface, second.ord, second.kind, None);
    ready(&mut s, now);
    send(
        &mut s,
        Frame::Answer {
            target: second,
            outcome: AnswerOutcome::Accepted,
        },
        now,
    );
    send(
        &mut s,
        publish(SIGNAL, 1, 900, PublishState::Live(&[1])),
        now,
    );
    send(
        &mut s,
        publish(second, 1, 900, PublishState::Live(&[2])),
        now,
    );
    assert!(s.backend.seen.groups.is_empty());
    s.tick(now, &mut vec![]);
    assert!(s.backend.seen.groups.is_empty());
    s.tick(now + Config::default().poll_interval, &mut vec![]);
    assert_eq!(s.backend.seen.groups, [Timestamp(900)]);
    s.tick(now + Config::default().poll_interval * 2, &mut vec![]);
    assert_eq!(s.backend.seen.groups.len(), 1);
}
#[test]
fn stamp_change_flushes_and_stale_frames_do_not_flush() {
    let mut s = session();
    let now = Instant::now();
    ready(&mut s, now);
    for (seq, stamp) in [(1, 900), (1, 901), (0, 902)] {
        send(
            &mut s,
            publish(SIGNAL, seq, stamp, PublishState::Live(&[1])),
            now,
        );
    }
    assert!(s.backend.seen.groups.is_empty());
    send(
        &mut s,
        publish(SIGNAL, 2, 903, PublishState::Live(&[2])),
        now,
    );
    assert_eq!(s.backend.seen.groups, [Timestamp(900)]);
}
#[test]
fn event_payload_and_sequence_dispositions() {
    let mut s = session();
    let now = Instant::now();
    ready(&mut s, now);
    for (seq, payload) in [
        (7, &[1, 2][..]),
        (7, &[8][..]),
        (6, &[8][..]),
        (10, &[3; 3][..]),
        (u64::MAX, &[4][..]),
        (0, &[8][..]),
    ] {
        send(
            &mut s,
            Frame::Occurrence {
                target: EVENT,
                envelope: Envelope {
                    stamp: Timestamp(99),
                    seq,
                },
                payload,
            },
            now,
        );
    }
    let events = &s.backend.seen.events;
    assert_eq!(events.len(), 3);
    assert_eq!(
        events[0],
        (
            EVENT,
            Ok(vec![1, 2]),
            Envelope {
                stamp: Timestamp(99),
                seq: 7
            },
            None
        )
    );
    assert_eq!(
        events[1],
        (
            EVENT,
            Err(Detection::Corrupt),
            Envelope {
                stamp: Timestamp(99),
                seq: 10
            },
            Some(2)
        )
    );
    assert_eq!(events[2].3, Some(u64::MAX - 11));
}
#[test]
fn signal_sequence_does_not_wrap() {
    let mut s = session();
    let now = Instant::now();
    ready(&mut s, now);
    for seq in [u64::MAX, 0, u64::MAX - 1, u64::MAX] {
        send(
            &mut s,
            publish(SIGNAL, seq, 99, PublishState::Live(&[1])),
            now,
        );
    }
    assert_eq!(s.backend.seen.samples.len(), 1);
}

#[test]
fn invalid_payload_forms_are_discarded_without_changing_state() {
    let mut s = session();
    let now = Instant::now();
    ready(&mut s, now);
    let f = publish(SIGNAL, 1, 99, PublishState::InvalidDeclared);
    let mut bytes = vec![0; f.encoded_len() + 1];
    f.encode(&mut bytes).unwrap();
    assert!(matches!(
        s.on_message(&bytes, now, &mut vec![], &Config::default()),
        Step::Continue
    ));
    assert!(s.backend.seen.samples.is_empty());
    send(
        &mut s,
        publish(SIGNAL, 1, 99, PublishState::Live(&[1])),
        now,
    );
    assert_eq!(s.backend.seen.samples.len(), 1);
}
#[test]
fn unknown_and_wrong_kind_frames_do_not_deliver() {
    let mut s = session();
    let now = Instant::now();
    ready(&mut s, now);
    for t in [
        Target {
            ord: Ordinal(999),
            ..SIGNAL
        },
        EVENT,
        Target {
            kind: Kind::Signal,
            ..EVENT
        },
        Target {
            kind: Kind::Fixed,
            ..SIGNAL
        },
    ] {
        send(&mut s, publish(t, 1, 99, PublishState::Live(&[1])), now);
    }
    assert!(s.backend.seen.samples.is_empty());
    send(
        &mut s,
        Frame::Occurrence {
            target: SIGNAL,
            envelope: Envelope {
                stamp: Timestamp(99),
                seq: 1,
            },
            payload: &[1],
        },
        now,
    );
    assert!(s.backend.seen.events.is_empty());
}
#[test]
fn reversed_stream_forms_close_1002() {
    let mut s = session();
    let now = Instant::now();
    ready(&mut s, now);
    let target = Target {
        kind: Kind::Query,
        ord: Ordinal(4),
        ..SIGNAL
    };
    let envelope = Envelope {
        stamp: Timestamp(900),
        seq: 1,
    };
    for frame in [
        Frame::Chunk {
            target,
            envelope,
            correlation: 1,
            stream: 0,
            payload: &[1],
        },
        Frame::StreamEnd {
            target,
            envelope,
            correlation: 1,
            stream: 0,
            outcome: crate::frame::StreamOutcome::End,
        },
        Frame::StreamCredit {
            target,
            correlation: 1,
            stream: 0,
            credit: 1,
        },
    ] {
        assert!(matches!(send(&mut s, frame, now).0, Step::Close(1002)));
    }
}

#[cfg(feature = "descriptor")]
mod mapping {
    use super::super::super::super::{MemberTable, Providing};
    use super::*;
    use ridl_loopback::{Attachment, Node};
    use ridl_rt::contract::InstanceNo;
    use ridl_rt::port::{Clock, EventSource, SignalReader, SignalWriter};
    use std::sync::{Arc, Mutex};
    type Notes = Arc<Mutex<Vec<Note>>>;
    fn setup() -> (Provided<Providing<Attachment>>, Attachment, Config, Notes) {
        let bytes = include_bytes!(
            "../../../../ridl-rt-conformance/fixtures/identity/identity.deployment.binfb"
        );
        let node = Node::from_descriptor(bytes, "Local").unwrap();
        let catalog = *node.catalogs().find(|c| c.name == "alpha").unwrap();
        let mut original = node.attach(InstanceNo::new(1).unwrap(), &catalog).unwrap();
        original.set(SIGNAL.iface, SIGNAL.ord, &[42]).unwrap();
        original.commit();
        drop(original);
        let ports = node
            .attach(InstanceNo::new(1).unwrap().twin().unwrap(), &catalog)
            .unwrap();
        let mut observer = node.attach(InstanceNo::LISTENER, &catalog).unwrap();
        observer.subscribe(EVENT.iface, &[EVENT.ord]).unwrap();
        let view = ridl_descriptor::deployment::verify(bytes).unwrap();
        let embedded = view
            .catalogs()
            .unwrap()
            .into_iter()
            .map(Result::unwrap)
            .find(|c| c.name().unwrap() == "alpha")
            .unwrap();
        let mut table = MemberTable::from_descriptor(embedded.bytes().unwrap()).unwrap();
        assert_eq!(
            table
                .iter()
                .map(|(t, _, _)| (t.kind, t.ord.0))
                .collect::<Vec<_>>(),
            [
                (Kind::Signal, 1),
                (Kind::Event, 2),
                (Kind::Command, 3),
                (Kind::Query, 4)
            ]
        );
        table.set_limit(SIGNAL.iface, SIGNAL.ord, 2);
        table.set_limit(EVENT.iface, EVENT.ord, 2);
        let notes = Notes::default();
        let recorded = notes.clone();
        let config = Config::default().with_record(move |n| recorded.lock().unwrap().push(*n));
        (
            Provided::new(Providing::new(ports, table).unwrap(), &config),
            observer,
            config,
            notes,
        )
    }
    fn ready(s: &mut Provided<Providing<Attachment>>, now: Instant) {
        send(s, Frame::Attached(AttachOutcome::Accepted), now);
        for target in [SIGNAL, EVENT] {
            send(
                s,
                Frame::Answer {
                    target,
                    outcome: AnswerOutcome::Accepted,
                },
                now,
            );
        }
    }
    fn sample(observer: &Attachment) -> (Vec<u8>, Provenance, Envelope) {
        let mut out = [0; 16];
        let s = observer.read(SIGNAL.iface, SIGNAL.ord, &mut out).unwrap();
        (out[..s.len].to_vec(), s.provenance, s.envelope)
    }
    fn receive(
        s: &mut Provided<Providing<Attachment>>,
        frame: Frame<'_>,
        now: Instant,
        config: &Config,
    ) {
        let mut bytes = vec![0; frame.encoded_len()];
        frame.encode(&mut bytes).unwrap();
        assert!(matches!(
            s.on_message(&bytes, now, &mut vec![], config),
            Step::Continue
        ));
    }
    #[test]
    fn init_leaves_machine_value_and_live_group_uses_local_stamp() {
        let (mut s, observer, config, notes) = setup();
        let now = Instant::now();
        ready(&mut s, now);
        receive(
            &mut s,
            publish(SIGNAL, 0, 900, PublishState::Init),
            now,
            &config,
        );
        s.tick(now + config.poll_interval, &mut vec![]);
        assert_eq!(sample(&observer).0, [42]);
        assert_eq!(sample(&observer).1, Provenance::Live);
        receive(
            &mut s,
            publish(SIGNAL, 1, 901, PublishState::Live(&[1])),
            now,
            &config,
        );
        receive(
            &mut s,
            publish(SIGNAL, 2, 901, PublishState::Live(&[2])),
            now,
            &config,
        );
        assert_eq!(sample(&observer).0, [1]);
        s.tick(now + config.poll_interval, &mut vec![]);
        let (bytes, provenance, envelope) = sample(&observer);
        assert_eq!(bytes, [2]);
        assert_eq!(provenance, Provenance::Live);
        assert_eq!(envelope.stamp, observer.now());
        assert_ne!(envelope.stamp, Timestamp(901));
        assert_eq!(envelope.seq, 2); // repeated channel starts a second group
        assert!(notes.lock().unwrap().is_empty());
    }
    #[test]
    fn declared_and_detected_invalid_map_to_declared_with_only_detection_noted() {
        let (mut s, observer, config, notes) = setup();
        let now = Instant::now();
        ready(&mut s, now);
        receive(
            &mut s,
            publish(SIGNAL, 1, 900, PublishState::InvalidDeclared),
            now,
            &config,
        );
        s.tick(now + config.poll_interval, &mut vec![]);
        assert_eq!(
            (sample(&observer).0, sample(&observer).1),
            (vec![42], Provenance::Invalid(Cause::Declared))
        );
        assert!(notes.lock().unwrap().is_empty());
        receive(
            &mut s,
            publish(SIGNAL, 2, 901, PublishState::Live(&[7, 8])),
            now,
            &config,
        );
        s.tick(now + config.poll_interval, &mut vec![]);
        receive(
            &mut s,
            publish(SIGNAL, 3, 902, PublishState::Live(&[9; 3])),
            now,
            &config,
        );
        s.tick(now + config.poll_interval, &mut vec![]);
        assert_eq!(
            (sample(&observer).0, sample(&observer).1),
            (vec![7, 8], Provenance::Invalid(Cause::Declared))
        );
        assert_eq!(
            *notes.lock().unwrap(),
            [Note::DetectedInvalid {
                target: SIGNAL,
                cause: Detection::Corrupt
            }]
        );
    }
    #[test]
    fn good_event_is_raised_corrupt_is_not_and_gap_is_noted() {
        let (mut s, mut observer, config, notes) = setup();
        let now = Instant::now();
        ready(&mut s, now);
        for (seq, payload) in [(7, &[1, 2][..]), (10, &[3; 3][..]), (11, &[4][..])] {
            receive(
                &mut s,
                Frame::Occurrence {
                    target: EVENT,
                    envelope: Envelope {
                        stamp: Timestamp(900),
                        seq,
                    },
                    payload,
                },
                now,
                &config,
            );
        }
        let mut out = [0; 16];
        let first = observer.next(&mut out).unwrap().unwrap();
        assert_eq!(&out[..first.len], &[1, 2]);
        assert_eq!(first.envelope.seq, 1);
        assert_eq!(first.envelope.stamp, observer.now());
        let second = observer.next(&mut out).unwrap().unwrap();
        assert_eq!(&out[..second.len], &[4]);
        assert_eq!(second.envelope.seq, 2);
        assert!(observer.next(&mut out).unwrap().is_none());
        assert_eq!(
            *notes.lock().unwrap(),
            [
                Note::EventGap {
                    target: EVENT,
                    count: 2
                },
                Note::CorruptOccurrence {
                    target: EVENT,
                    cause: Detection::Corrupt
                }
            ]
        );
    }
    #[test]
    fn refused_local_writes_are_recorded() {
        let (s, observer, config, notes) = setup();
        let mut backend = Providing::new(observer, s.backend.members().clone()).unwrap();
        let envelope = Envelope {
            stamp: Timestamp(900),
            seq: 1,
        };
        backend.sample(
            SIGNAL,
            Sample {
                value: Some(&[1]),
                provenance: Provenance::Live,
                freshness: ridl_rt::sample::Freshness::Fresh,
                envelope,
            },
            &config,
        );
        backend.occurrence(
            EVENT,
            ridl_rt::sample::Occurrence {
                payload: Ok(&[1]),
                envelope,
            },
            None,
            &config,
        );
        assert_eq!(
            *notes.lock().unwrap(),
            [
                Note::PortRefused { target: SIGNAL },
                Note::PortRefused { target: EVENT }
            ]
        );
    }
    #[test]
    fn live_sample_without_bytes_is_refused() {
        let (mut s, observer, config, notes) = setup();
        s.backend.sample(
            SIGNAL,
            Sample {
                value: None,
                provenance: Provenance::Live,
                freshness: ridl_rt::sample::Freshness::Fresh,
                envelope: Envelope {
                    stamp: Timestamp(900),
                    seq: 1,
                },
            },
            &config,
        );
        s.backend.commit(Timestamp(900));
        assert_eq!(sample(&observer).0, [42]);
        assert_eq!(
            *notes.lock().unwrap(),
            [Note::PortRefused { target: SIGNAL }]
        );
    }
    #[test]
    fn disconnect_discards_pending_group() {
        let (mut s, observer, config, _) = setup();
        let now = Instant::now();
        ready(&mut s, now);
        receive(
            &mut s,
            publish(SIGNAL, 1, 900, PublishState::Live(&[8])),
            now,
            &config,
        );
        drop(s);
        assert_eq!(sample(&observer).0, [42]);
    }
}

#[test]
fn default_payload_ceiling_is_enforced_without_schema_verification() {
    let mut s = session();
    let now = Instant::now();
    for target in [SIGNAL, EVENT] {
        s.backend
            .table
            .insert(target.iface, target.ord, target.kind, None);
    }
    ready(&mut s, now);
    let bytes = vec![0xff; 3840];
    send(
        &mut s,
        publish(SIGNAL, 1, 9, PublishState::Live(&bytes)),
        now,
    );
    let too_long = vec![0xff; 3841];
    send(
        &mut s,
        publish(SIGNAL, 2, 9, PublishState::Live(&too_long)),
        now,
    );
    assert_eq!(s.backend.seen.samples[0].1, Provenance::Live);
    assert_eq!(
        s.backend.seen.samples[1].1,
        Provenance::Invalid(Cause::Detected(Detection::Corrupt))
    );
    assert_eq!(s.backend.seen.samples[1].2.as_ref().unwrap(), &bytes);
    for (seq, payload) in [(1, &bytes[..]), (2, &too_long[..])] {
        send(
            &mut s,
            Frame::Occurrence {
                target: EVENT,
                envelope: Envelope {
                    stamp: Timestamp(9),
                    seq,
                },
                payload,
            },
            now,
        );
    }
    assert_eq!(s.backend.seen.events[0].1, Ok(bytes));
    assert_eq!(s.backend.seen.events[1].1, Err(Detection::Corrupt));
}
#[test]
fn timing_uses_consumer_clock_and_event_expiry_is_strict() {
    use ridl_rt::sample::{Duration as Micros, Freshness};
    let mut s = session();
    let now = Instant::now();
    for target in [SIGNAL, EVENT] {
        s.backend.table.insert(
            target.iface,
            target.ord,
            target.kind,
            Some(std::time::Duration::from_micros(10)),
        );
    }
    s.backend.ports.advance(Micros(100));
    ready(&mut s, now);
    for (seq, stamp) in [(1, 90), (2, 89), (3, 101)] {
        send(
            &mut s,
            publish(SIGNAL, seq, stamp, PublishState::Live(&[1])),
            now,
        );
        send(
            &mut s,
            Frame::Occurrence {
                target: EVENT,
                envelope: Envelope {
                    stamp: Timestamp(stamp),
                    seq,
                },
                payload: &[2],
            },
            now,
        );
    }
    assert_eq!(
        s.backend.seen.freshness,
        [
            Freshness::Fresh,
            Freshness::Stale { by: Micros(1) },
            Freshness::Fresh
        ]
    );
    assert_eq!(
        s.backend
            .seen
            .events
            .iter()
            .map(|e| e.2.seq)
            .collect::<Vec<_>>(),
        [1, 3]
    );
    assert_eq!(s.backend.seen.events[1].3, None); // expiry is not wire loss
}
#[test]
fn refused_subscription_does_not_deliver_or_flush() {
    let mut s = session();
    let now = Instant::now();
    send(&mut s, Frame::Attached(AttachOutcome::Accepted), now);
    send(
        &mut s,
        Frame::Answer {
            target: SIGNAL,
            outcome: AnswerOutcome::Contract(ridl_rt::error::Contract::UnknownInteraction),
        },
        now,
    );
    send(
        &mut s,
        publish(SIGNAL, 1, 99, PublishState::Live(&[1])),
        now,
    );
    s.tick(now + Config::default().poll_interval, &mut vec![]);
    assert!(s.backend.seen.samples.is_empty());
    assert!(s.backend.seen.groups.is_empty());
}
#[test]
fn multiple_signal_channels_are_invisible_until_one_local_commit() {
    use super::super::super::Providing;
    use ridl_rt::port::{Clock, SignalReader};
    // A synthetic two-signal catalog, separate from the identity fixture.
    let catalog = CatalogRef {
        name: "provided.two_signals",
        hash: CatalogHash([9; 32]),
    };
    let mut root = Loopback::new(catalog);
    root.advance(ridl_rt::sample::Duration(71));
    let observer = root.attach();
    let second = Target {
        iface: InterfaceNo(2),
        ..SIGNAL
    };
    let mut table = super::super::MemberTable::new(catalog.name, catalog.hash);
    for t in [SIGNAL, second] {
        table.insert(t.iface, t.ord, t.kind, None);
    }
    let config = Config::default();
    let now = Instant::now();
    let mut s = Provided::new(Providing::new(root, table).unwrap(), &config);
    send(&mut s, Frame::Attached(AttachOutcome::Accepted), now);
    for t in [SIGNAL, second] {
        send(
            &mut s,
            Frame::Answer {
                target: t,
                outcome: AnswerOutcome::Accepted,
            },
            now,
        );
        send(
            &mut s,
            publish(t, 1, 900, PublishState::Live(&[t.iface.0 as u8])),
            now,
        );
        assert_eq!(
            observer
                .read(t.iface, t.ord, &mut [0; 8])
                .unwrap()
                .provenance,
            Provenance::Init
        );
    }
    s.tick(now + config.poll_interval, &mut vec![]);
    let first = observer
        .read(SIGNAL.iface, SIGNAL.ord, &mut [0; 8])
        .unwrap();
    let second = observer
        .read(second.iface, second.ord, &mut [0; 8])
        .unwrap();
    assert_eq!(first.envelope.stamp, Timestamp(71));
    assert_eq!(first.envelope.stamp, second.envelope.stamp);
    assert_eq!(first.envelope.stamp, observer.now());
}

#[test]
fn malformed_publication_before_attached_closes() {
    let mut s = session();
    let now = Instant::now();
    let f = publish(SIGNAL, 0, 0, PublishState::Init);
    let mut bytes = vec![0; f.encoded_len() + 1];
    f.encode(&mut bytes).unwrap();
    assert!(matches!(
        s.on_message(&bytes, now, &mut vec![], &Config::default()),
        Step::Close(1002)
    ));
    assert!(s.backend.seen.samples.is_empty());
}
#[test]
fn idle_deadline_tracks_last_delivery_and_equal_stamp_after_flush_starts_again() {
    let mut s = session();
    let now = Instant::now();
    let second = Target {
        ord: Ordinal(5),
        ..SIGNAL
    };
    s.backend
        .table
        .insert(second.iface, second.ord, second.kind, None);
    ready(&mut s, now);
    send(
        &mut s,
        Frame::Answer {
            target: second,
            outcome: AnswerOutcome::Accepted,
        },
        now,
    );
    let interval = Config::default().poll_interval;
    send(
        &mut s,
        publish(SIGNAL, 1, 900, PublishState::Live(&[1])),
        now,
    );
    send(
        &mut s,
        publish(second, 1, 900, PublishState::Live(&[2])),
        now + interval / 2,
    );
    s.tick(now + interval, &mut vec![]);
    assert!(s.backend.seen.groups.is_empty());
    s.tick(now + interval + interval / 2, &mut vec![]);
    assert_eq!(s.backend.seen.groups, [Timestamp(900)]);
    send(
        &mut s,
        publish(SIGNAL, 3, 900, PublishState::Live(&[3])),
        now + interval * 2,
    );
    s.tick(now + interval * 3, &mut vec![]);
    assert_eq!(s.backend.seen.groups, [Timestamp(900), Timestamp(900)]);
}

#[test]
fn repeated_channel_commits_without_idle_or_stamp_change() {
    let mut s = session();
    let now = Instant::now();
    ready(&mut s, now);
    let step = Config::default().poll_interval / 4;
    for seq in 1..=8 {
        let at = now + step * seq as u32;
        send(
            &mut s,
            publish(SIGNAL, seq, 900, PublishState::Live(&[seq as u8])),
            at,
        );
        s.tick(at, &mut vec![]);
        assert_eq!(
            s.backend.seen.groups,
            vec![Timestamp(900); seq as usize - 1]
        );
    }
}

#[test]
fn interleaved_occurrence_does_not_split_or_extend_signal_group() {
    let mut s = session();
    let now = Instant::now();
    let second = Target {
        ord: Ordinal(5),
        ..SIGNAL
    };
    s.backend
        .table
        .insert(second.iface, second.ord, second.kind, None);
    ready(&mut s, now);
    send(
        &mut s,
        Frame::Answer {
            target: second,
            outcome: AnswerOutcome::Accepted,
        },
        now,
    );
    send(
        &mut s,
        publish(SIGNAL, 1, 900, PublishState::Live(&[1])),
        now,
    );
    send(
        &mut s,
        Frame::Occurrence {
            target: EVENT,
            envelope: Envelope {
                stamp: Timestamp(901),
                seq: 1,
            },
            payload: &[2],
        },
        now + Config::default().poll_interval / 2,
    );
    assert!(s.backend.seen.groups.is_empty());
    send(
        &mut s,
        publish(second, 1, 900, PublishState::Live(&[3])),
        now + Config::default().poll_interval / 2,
    );
    s.tick(now + Config::default().poll_interval * 3 / 2, &mut vec![]);
    assert_eq!(s.backend.seen.groups, [Timestamp(900)]);
    send(
        &mut s,
        Frame::Occurrence {
            target: EVENT,
            envelope: Envelope {
                stamp: Timestamp(902),
                seq: 2,
            },
            payload: &[4],
        },
        now + Config::default().poll_interval * 2,
    );
    s.tick(now + Config::default().poll_interval * 3, &mut vec![]);
    assert_eq!(s.backend.seen.groups, [Timestamp(900)]);
    assert_eq!(s.backend.seen.events.len(), 2);
}

#[test]
fn expired_occurrence_records_wire_gap_before_next_fresh_occurrence() {
    use std::sync::{Arc, Mutex};
    let notes = Arc::new(Mutex::new(Vec::new()));
    let recorded = notes.clone();
    let config = Config::default().with_record(move |n| recorded.lock().unwrap().push(*n));
    let mut s = session();
    s.backend.table.insert(
        EVENT.iface,
        EVENT.ord,
        Kind::Event,
        Some(std::time::Duration::from_micros(10)),
    );
    s.backend.ports.advance(ridl_rt::sample::Duration(100));
    let now = Instant::now();
    ready(&mut s, now);
    for (seq, stamp) in [(1, 100), (4, 89), (5, 100)] {
        let frame = Frame::Occurrence {
            target: EVENT,
            envelope: Envelope {
                stamp: Timestamp(stamp),
                seq,
            },
            payload: &[1],
        };
        let mut bytes = vec![0; frame.encoded_len()];
        frame.encode(&mut bytes).unwrap();
        s.on_message(&bytes, now, &mut vec![], &config);
    }
    assert_eq!(
        s.backend
            .seen
            .events
            .iter()
            .map(|e| e.2.seq)
            .collect::<Vec<_>>(),
        [1, 5]
    );
    assert_eq!(
        *notes.lock().unwrap(),
        [Note::EventGap {
            target: EVENT,
            count: 2
        }]
    );
    assert_eq!(s.backend.seen.events[1].3, None);
}

#[test]
fn wrong_kind_answer_does_not_accept_or_consume_signal_subscription() {
    let mut s = session();
    let now = Instant::now();
    send(&mut s, Frame::Attached(AttachOutcome::Accepted), now);
    send(
        &mut s,
        Frame::Answer {
            target: Target {
                kind: Kind::Event,
                ..SIGNAL
            },
            outcome: AnswerOutcome::Accepted,
        },
        now,
    );
    send(
        &mut s,
        publish(SIGNAL, 1, 900, PublishState::Live(&[1])),
        now,
    );
    assert!(s.backend.seen.samples.is_empty());
    send(
        &mut s,
        Frame::Answer {
            target: SIGNAL,
            outcome: AnswerOutcome::Accepted,
        },
        now,
    );
    send(
        &mut s,
        publish(SIGNAL, 1, 900, PublishState::Live(&[2])),
        now,
    );
    assert_eq!(s.backend.seen.samples[0].2, Some(vec![2]));
}

fn ready_second_signal(s: &mut Provided<Capture>, now: Instant) -> Target {
    let second = Target {
        ord: Ordinal(5),
        ..SIGNAL
    };
    s.backend
        .table
        .insert(second.iface, second.ord, second.kind, None);
    ready(s, now);
    send(
        s,
        Frame::Answer {
            target: second,
            outcome: AnswerOutcome::Accepted,
        },
        now,
    );
    second
}

#[test]
fn different_channel_stamp_change_flushes_without_tick() {
    let mut s = session();
    let now = Instant::now();
    let second = ready_second_signal(&mut s, now);
    send(
        &mut s,
        publish(SIGNAL, 1, 900, PublishState::Live(&[1])),
        now,
    );
    send(
        &mut s,
        publish(second, 1, 901, PublishState::Live(&[2])),
        now,
    );
    assert_eq!(s.backend.seen.groups, [Timestamp(900)]);
}

#[test]
fn stamp_flush_clears_previous_group_channels() {
    let mut s = session();
    let now = Instant::now();
    let second = ready_second_signal(&mut s, now);
    send(
        &mut s,
        publish(SIGNAL, 1, 900, PublishState::Live(&[1])),
        now,
    );
    send(
        &mut s,
        publish(second, 1, 901, PublishState::Live(&[2])),
        now,
    );
    assert_eq!(s.backend.seen.groups, [Timestamp(900)]);
    send(
        &mut s,
        publish(SIGNAL, 2, 901, PublishState::Live(&[3])),
        now,
    );
    assert_eq!(s.backend.seen.groups, [Timestamp(900)]);
    s.tick(now + Config::default().poll_interval, &mut vec![]);
    assert_eq!(s.backend.seen.groups, [Timestamp(900), Timestamp(901)]);
}

#[test]
fn provided_attached_wait_is_bounded_despite_pongs() {
    super::super::attach_tests::assert_attach_timeout(&mut session());
}

#[test]
fn provided_acceptance_cancels_attach_deadline() {
    let mut s = session();
    ready(&mut s, Instant::now());
    super::super::attach_tests::assert_pong_end(
        &mut s,
        Ended::Io(std::io::ErrorKind::UnexpectedEof),
        5,
    );
}
#[test]
fn provided_attach_timeout_releases_twin_reservation() {
    use ridl_rt::contract::InstanceNo;
    let node = ridl_loopback::Node::from_descriptor(
        include_bytes!(
            "../../../../ridl-rt-conformance/fixtures/identity/identity.deployment.binfb"
        ),
        "Local",
    )
    .unwrap();
    let catalog = *node.catalogs().find(|c| c.name == "alpha").unwrap();
    let who = InstanceNo::new(1).unwrap();
    let ports = node.attach(who.twin().unwrap(), &catalog).unwrap();
    let mut s = Provided::new(
        super::super::Providing::new(
            ports,
            super::super::MemberTable::new(catalog.name, catalog.hash),
        )
        .unwrap(),
        &Config::default(),
    );
    assert!(node.attach(who, &catalog).is_err());
    super::super::attach_tests::assert_attach_timeout(&mut s);
    drop(s);
    assert!(node.attach(who, &catalog).is_ok());
}
