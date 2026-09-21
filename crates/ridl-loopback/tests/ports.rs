//! The runtime's own tests: each port role exercised against the contract
//! `ridl_rt::port` states for it.
//!
//! The first six tests are the `support_*` tests of
//! `crates/ridl-backend-rust/tests/interaction_face.rs`, which exercised the
//! disposable double this crate replaces (story E11.15, driftsys/ridl#445).
//! They move here unchanged in substance, because what they pin — a command
//! delivered and acknowledged, a query replied, an injected settlement
//! failure, a signal round trip, an event round trip, and a clock that is not
//! wall-clock time — is this crate's behaviour, not the Rust backend's. The
//! tests after them cover what the double did not implement: the two signal
//! extensions, the per handle sequence counters, `fixed`, and the threading
//! model.

use ridl_loopback::Loopback;
use ridl_rt::contract::{CatalogHash, CatalogRef, InterfaceNo, Ordinal};
use ridl_rt::error::{CallError, Contract};
use ridl_rt::port::{
    Attached, Caller, Changed, ClaimId, Clock, CoherentSignals, EventSink, EventSource,
    FixedReader, Handler, RawSample, ReadError, ScannableSignals, SettleError, SignalReader,
    SignalWriter, Watermark,
};
use ridl_rt::sample::{Cause, Duration, Envelope, Freshness, Provenance, Timestamp};

const IFACE: InterfaceNo = InterfaceNo(1);
const ORD: Ordinal = Ordinal(1);
const OTHER: Ordinal = Ordinal(2);

/// The catalog every test attaches to. The hash is all zeros, which is the
/// placeholder the descriptor emitter writes until story E16.2
/// (driftsys/ridl#378) computes a real one.
fn catalog() -> CatalogRef {
    CatalogRef {
        name: "face.demo",
        hash: CatalogHash([0u8; 32]),
    }
}

fn runtime() -> Loopback {
    Loopback::new(catalog())
}

// ---------------------------------------------------------------------------
// The six tests that came from the double.
// ---------------------------------------------------------------------------

#[test]
fn a_command_is_delivered_and_acknowledged() {
    let mut rt = runtime();
    let correlation = rt.command(IFACE, ORD, &[1, 2, 3]).expect("command sent");

    assert_eq!(rt.ack(correlation), None, "not yet settled");

    let mut buf = [0u8; 8];
    let claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("a claim is waiting");
    assert_eq!(claim.iface, IFACE);
    assert_eq!(claim.ord, ORD);
    assert_eq!(&buf[..claim.len], &[1, 2, 3]);

    rt.settle(claim.id, Ok(&[])).expect("settle succeeds");
    assert_eq!(
        rt.ack(correlation),
        Some(Ok(())),
        "settlement is observable through ack"
    );
}

#[test]
fn a_query_is_delivered_and_replied() {
    let mut rt = runtime();
    let correlation = rt.query(IFACE, ORD, &[9]).expect("query sent");

    let mut buf = [0u8; 8];
    let claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("a claim is waiting");
    assert_eq!(&buf[..claim.len], &[9]);

    rt.settle(claim.id, Ok(&[7, 7])).expect("settle succeeds");

    let mut out = [0u8; 8];
    let reply = rt.reply(correlation, &mut out).expect("reply read");
    let Some(Ok(len)) = reply else {
        panic!("expected a successful reply, got {reply:?}");
    };
    assert_eq!(&out[..len], &[7, 7]);
    // A query's correlation always answers `None` from `ack` (`Caller::ack`'s
    // own documentation).
    assert_eq!(rt.ack(correlation), None);
}

#[test]
fn settle_can_be_made_to_fail_once_then_succeed() {
    // The generated `dispatch` advances its returned count only past a
    // settlement the handler accepted; this is the injected failure that
    // makes that path reachable.
    let mut rt = runtime();
    let correlation = rt.command(IFACE, ORD, &[1]).expect("command sent");
    let mut buf = [0u8; 8];
    let claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("a claim is waiting");

    rt.fail_next_settle();
    assert!(
        rt.settle(claim.id, Ok(&[])).is_err(),
        "the injected failure surfaces from settle"
    );
    assert_eq!(
        rt.ack(correlation),
        None,
        "a failed settle records no outcome"
    );

    rt.settle(claim.id, Ok(&[]))
        .expect("the next settle succeeds");
    assert_eq!(rt.ack(correlation), Some(Ok(())));
}

#[test]
fn a_signal_publish_and_read_round_trips() {
    let mut rt = runtime();
    rt.set(IFACE, ORD, &[42]).expect("set staged");
    rt.commit();

    let mut out = [0u8; 8];
    let raw = rt.read(IFACE, ORD, &mut out).expect("read");
    assert_eq!(raw.provenance, Provenance::Live);
    assert_eq!(&out[..raw.len], &[42]);
}

#[test]
fn an_event_raise_and_receive_round_trips() {
    let mut rt = runtime();
    rt.subscribe(IFACE, &[ORD]).expect("subscribe");
    rt.raise(IFACE, ORD, &[5, 6]).expect("raise");

    let mut out = [0u8; 8];
    let occurrence = rt
        .next(&mut out)
        .expect("next")
        .expect("an occurrence is waiting");
    assert_eq!(occurrence.iface, IFACE);
    assert_eq!(occurrence.ord, ORD);
    assert_eq!(&out[..occurrence.len], &[5, 6]);
}

#[test]
fn the_clock_is_hand_driven_not_wall_clock() {
    // Two independently constructed runtimes must start at the same logical
    // time regardless of when, in real time, each was constructed — a clock
    // that read wall-clock time could not guarantee that.
    let first = runtime();
    std::thread::sleep(std::time::Duration::from_millis(5));
    let second = runtime();
    assert_eq!(
        first.now(),
        second.now(),
        "the clock must not read wall-clock time"
    );

    let mut rt = second;
    let before = rt.now();
    rt.advance(Duration(1_000));
    assert_eq!(
        rt.now().0,
        before.0 + 1_000,
        "advance moves the hand-driven clock by exactly the given amount"
    );
}

// ---------------------------------------------------------------------------
// Signals: staging, the invalid state, the envelope, and a short buffer.
// ---------------------------------------------------------------------------

#[test]
fn a_signal_with_no_publication_reads_as_init_and_copies_nothing() {
    let rt = runtime();
    let mut out = [0u8; 8];
    let raw = rt.read(IFACE, ORD, &mut out).expect("read");
    assert_eq!(raw.provenance, Provenance::Init);
    assert_eq!(raw.len, 0);
    assert_eq!(
        raw.envelope,
        Envelope {
            stamp: Timestamp(0),
            seq: 0
        }
    );
}

#[test]
fn a_staged_value_is_not_visible_until_commit() {
    let mut rt = runtime();
    rt.set(IFACE, ORD, &[7]).expect("set staged");

    let mut out = [0u8; 8];
    assert_eq!(
        rt.read(IFACE, ORD, &mut out).expect("read").provenance,
        Provenance::Init,
        "staging is private to the writer until commit"
    );

    rt.commit();
    assert_eq!(
        rt.read(IFACE, ORD, &mut out).expect("read").provenance,
        Provenance::Live
    );
}

#[test]
fn one_commit_publishes_every_staged_signal_under_one_timestamp() {
    let mut rt = runtime();
    rt.advance(Duration(500));
    rt.set(IFACE, ORD, &[1]).expect("set");
    rt.set(IFACE, OTHER, &[2]).expect("set");
    rt.commit();

    let mut out = [0u8; 8];
    let first = rt.read(IFACE, ORD, &mut out).expect("read");
    let second = rt.read(IFACE, OTHER, &mut out).expect("read");
    assert_eq!(first.envelope.stamp, Timestamp(500));
    assert_eq!(
        first.envelope.stamp, second.envelope.stamp,
        "one commit stamps every signal it publishes with one time"
    );
}

#[test]
fn a_channel_sequence_number_counts_that_channel_publications() {
    let mut rt = runtime();
    for value in 1..=3u8 {
        rt.set(IFACE, ORD, &[value]).expect("set");
        rt.set(IFACE, OTHER, &[value]).expect("set");
        rt.commit();
    }

    let mut out = [0u8; 8];
    // The first publication is seq 1, because seq 0 is the envelope of a
    // channel with no publication (ADR-0021 decision 5).
    assert_eq!(rt.read(IFACE, ORD, &mut out).expect("read").envelope.seq, 3);
    assert_eq!(
        rt.read(IFACE, OTHER, &mut out).expect("read").envelope.seq,
        3
    );
}

#[test]
fn invalidate_keeps_the_last_good_value_and_reports_the_declared_cause() {
    let mut rt = runtime();
    rt.set(IFACE, ORD, &[9]).expect("set");
    rt.commit();
    rt.invalidate(IFACE, ORD).expect("invalidate staged");
    rt.commit();

    let mut out = [0u8; 8];
    let raw = rt.read(IFACE, ORD, &mut out).expect("read");
    assert_eq!(raw.provenance, Provenance::Invalid(Cause::Declared));
    assert_eq!(
        &out[..raw.len],
        &[9],
        "the invalid state keeps the last good value (ridl 4.5)"
    );

    rt.set(IFACE, ORD, &[10]).expect("set");
    rt.commit();
    let raw = rt.read(IFACE, ORD, &mut out).expect("read");
    assert_eq!(
        raw.provenance,
        Provenance::Live,
        "a set clears the invalid state"
    );
}

#[test]
fn touch_republishes_the_current_value_without_changing_it() {
    let mut rt = runtime();
    rt.set(IFACE, ORD, &[4]).expect("set");
    rt.commit();
    rt.advance(Duration(100));
    rt.touch(IFACE, ORD).expect("touch staged");
    rt.commit();

    let mut out = [0u8; 8];
    let raw = rt.read(IFACE, ORD, &mut out).expect("read");
    assert_eq!(&out[..raw.len], &[4], "touch carries no new value");
    assert_eq!(
        raw.envelope.stamp,
        Timestamp(100),
        "touch re-affirms at the new time"
    );
    assert_eq!(raw.envelope.seq, 2, "touch is a publication of the channel");
}

#[test]
fn touch_on_a_channel_with_no_publication_publishes_nothing() {
    // A re-affirmation of nothing is nothing. Publishing here would put a
    // zero-length value on the channel as `Live`, which a consumer's binding
    // reads as a corrupt payload rather than as the init value it should see.
    let mut rt = runtime();
    rt.touch(IFACE, ORD).expect("touch staged");
    rt.commit();

    let mut out = [0u8; 8];
    assert_eq!(
        rt.read(IFACE, ORD, &mut out).expect("read").provenance,
        Provenance::Init
    );
    assert_eq!(
        rt.generation(IFACE),
        0,
        "and a commit whose every staged change was such a touch changes nothing"
    );
}

#[test]
#[should_panic(expected = "the clock advances forward")]
fn the_clock_refuses_to_run_backwards() {
    let mut rt = runtime();
    rt.advance(Duration(-1));
}

#[test]
fn a_short_buffer_reports_what_the_read_needs_and_consumes_nothing() {
    let mut rt = runtime();
    rt.set(IFACE, ORD, &[1, 2, 3, 4]).expect("set");
    rt.commit();

    let mut short = [0u8; 2];
    assert_eq!(
        rt.read(IFACE, ORD, &mut short),
        Err(ReadError::Short { needed: 4 })
    );

    let mut out = [0u8; 8];
    assert_eq!(rt.read(IFACE, ORD, &mut out).expect("read").len, 4);
}

#[test]
fn every_value_is_unbounded_because_the_runtime_has_no_member_table() {
    let mut rt = runtime();
    rt.set(IFACE, ORD, &[1]).expect("set");
    rt.commit();
    let mut out = [0u8; 8];
    assert_eq!(
        rt.read(IFACE, ORD, &mut out).expect("read").freshness,
        Freshness::Unbounded,
        "freshness is measured against a staleness bound this runtime cannot read"
    );
}

// ---------------------------------------------------------------------------
// The two signal extensions.
// ---------------------------------------------------------------------------

#[test]
fn a_commit_advances_the_interface_generation_once() {
    let mut rt = runtime();
    assert_eq!(rt.generation(IFACE), 0, "no publication yet");

    rt.set(IFACE, ORD, &[1]).expect("set");
    rt.set(IFACE, OTHER, &[2]).expect("set");
    rt.commit();
    assert_eq!(
        rt.generation(IFACE),
        1,
        "two signals in one commit advance the generation once"
    );

    rt.set(IFACE, ORD, &[3]).expect("set");
    rt.commit();
    assert_eq!(rt.generation(IFACE), 2);
    assert_eq!(
        rt.generation(InterfaceNo(2)),
        0,
        "another interface is untouched"
    );
}

#[test]
fn scan_reports_the_changes_since_a_mark_and_moves_it_forward() {
    let mut rt = runtime();
    rt.set(IFACE, ORD, &[1]).expect("set");
    rt.set(IFACE, OTHER, &[2]).expect("set");
    rt.commit();

    let mut marks = [Watermark {
        iface: IFACE,
        generation: 0,
        seq: 0,
    }];
    let mut out = [blank_change(); 4];
    let written = rt.scan(&mut marks, &mut out);
    assert_eq!(written, 2);
    assert_eq!(out[0].ord, ORD);
    assert_eq!(out[1].ord, OTHER);
    assert_eq!(
        marks[0].generation, 1,
        "the mark moved to the current generation"
    );
    assert_eq!(marks[0].seq, 1);

    assert_eq!(
        rt.scan(&mut marks, &mut out),
        0,
        "a second scan with the moved mark reports nothing"
    );

    rt.set(IFACE, OTHER, &[3]).expect("set");
    rt.commit();
    let written = rt.scan(&mut marks, &mut out);
    assert_eq!(written, 1, "only the signal that changed since the mark");
    assert_eq!(out[0].ord, OTHER);
    assert_eq!(out[0].seq, 2);
}

#[test]
fn scan_writes_an_interface_changes_all_together_or_not_at_all() {
    let mut rt = runtime();
    rt.set(IFACE, ORD, &[1]).expect("set");
    rt.set(IFACE, OTHER, &[2]).expect("set");
    rt.commit();

    let mut marks = [Watermark {
        iface: IFACE,
        generation: 0,
        seq: 0,
    }];
    let mut one = [blank_change(); 1];
    assert_eq!(
        rt.scan(&mut marks, &mut one),
        0,
        "two changes do not fit in one entry, so none is written"
    );
    assert_eq!(marks[0].generation, 0, "and the mark is not moved");

    // Growing the output is what makes progress, which is the loop
    // `ScannableSignals::scan` documents.
    let mut two = [blank_change(); 2];
    assert_eq!(rt.scan(&mut marks, &mut two), 2);
    assert_eq!(marks[0].generation, 1);
}

#[test]
fn a_coherent_read_answers_every_ordinal_from_one_publication() {
    let mut rt = runtime();
    rt.set(IFACE, ORD, &[1, 1]).expect("set");
    rt.set(IFACE, OTHER, &[2]).expect("set");
    rt.commit();

    let mut out = [0u8; 8];
    let mut samples = [blank_sample(); 2];
    let written = rt
        .read_coherent(IFACE, &[ORD, OTHER], &mut out, &mut samples)
        .expect("coherent read");
    assert_eq!(written, 3);
    assert_eq!(&out[..3], &[1, 1, 2]);
    assert_eq!(samples[0].len, 2);
    assert_eq!(samples[1].len, 1);
    assert_eq!(
        samples[0].envelope.stamp, samples[1].envelope.stamp,
        "both values come from one publication"
    );
}

#[test]
fn a_coherent_read_reports_a_short_output_and_a_short_sample_slice() {
    let mut rt = runtime();
    rt.set(IFACE, ORD, &[1, 1]).expect("set");
    rt.set(IFACE, OTHER, &[2]).expect("set");
    rt.commit();

    let mut out = [0u8; 2];
    let mut samples = [blank_sample(); 2];
    assert_eq!(
        rt.read_coherent(IFACE, &[ORD, OTHER], &mut out, &mut samples),
        Err(ReadError::Short { needed: 3 }),
        "the whole set's size, not the first value's"
    );

    let mut out = [0u8; 8];
    let mut one = [blank_sample(); 1];
    assert_eq!(
        rt.read_coherent(IFACE, &[ORD, OTHER], &mut out, &mut one),
        Err(ReadError::TooFewSamples { needed: 2 })
    );
}

// ---------------------------------------------------------------------------
// Events.
// ---------------------------------------------------------------------------

#[test]
fn an_occurrence_raised_before_the_subscription_is_not_delivered() {
    let mut rt = runtime();
    rt.raise(IFACE, ORD, &[1]).expect("raise");
    rt.subscribe(IFACE, &[ORD]).expect("subscribe");

    let mut out = [0u8; 8];
    assert!(
        rt.next(&mut out).expect("next").is_none(),
        "a late joiner receives nothing retroactive on an event"
    );
}

#[test]
fn unsubscribe_stops_delivery_of_what_is_already_queued() {
    let mut rt = runtime();
    rt.subscribe(IFACE, &[ORD]).expect("subscribe");
    rt.raise(IFACE, ORD, &[1]).expect("raise");
    rt.unsubscribe(IFACE, &[ORD]);

    let mut out = [0u8; 8];
    assert!(rt.next(&mut out).expect("next").is_none());
}

#[test]
fn two_sources_each_receive_their_own_copy_of_one_occurrence() {
    let rt = runtime();
    let mut first = rt.source();
    let mut second = rt.source();
    let mut sink = rt.sink();
    first.subscribe(IFACE, &[ORD]).expect("subscribe");
    second.subscribe(IFACE, &[ORD]).expect("subscribe");

    sink.raise(IFACE, ORD, &[8]).expect("raise");

    let mut out = [0u8; 8];
    assert_eq!(
        first.next(&mut out).expect("next").expect("waiting").len,
        1,
        "the first source consumes its own copy"
    );
    assert_eq!(
        second.next(&mut out).expect("next").expect("waiting").len,
        1,
        "and does not consume the second source's"
    );
}

#[test]
fn a_short_buffer_leaves_the_occurrence_for_the_next_call() {
    let mut rt = runtime();
    rt.subscribe(IFACE, &[ORD]).expect("subscribe");
    rt.raise(IFACE, ORD, &[1, 2, 3]).expect("raise");

    let mut short = [0u8; 1];
    assert_eq!(rt.next(&mut short), Err(ReadError::Short { needed: 3 }));

    let mut out = [0u8; 8];
    let occurrence = rt.next(&mut out).expect("next").expect("still waiting");
    assert_eq!(&out[..occurrence.len], &[1, 2, 3]);
}

#[test]
fn a_sink_sequence_number_counts_one_channel_publications() {
    // One sink raising on two of its events, with a consumer subscribed to
    // one of them. A counter per handle rather than per channel would number
    // this consumer's two occurrences 1 and 3, and `EventSource::next` states
    // that a gap in seq is a loss -- so the consumer would read a loss that
    // did not happen.
    let rt = runtime();
    let mut source = rt.source();
    let mut sink = rt.sink();
    source.subscribe(IFACE, &[ORD]).expect("subscribe");

    sink.raise(IFACE, ORD, &[1]).expect("raise");
    sink.raise(IFACE, OTHER, &[2]).expect("raise");
    sink.raise(IFACE, ORD, &[3]).expect("raise");

    let mut out = [0u8; 8];
    let mut seqs = Vec::new();
    while let Some(occurrence) = source.next(&mut out).expect("next") {
        seqs.push(occurrence.envelope.seq);
    }
    assert_eq!(
        seqs,
        vec![1, 2],
        "no gap: the other event has its own counter"
    );
}

#[test]
fn a_sink_counts_its_own_channel_and_not_another_sinks() {
    // Two sinks on one event channel is a misuse the loopback does not police,
    // the same way two writer handles on one signal are: an event channel has
    // one provider (ridl 5). What this pins is that a sink's counters are its
    // own, so the second sink's first raise is its own seq 1.
    let rt = runtime();
    let mut source = rt.source();
    let mut first = rt.sink();
    let mut second = rt.sink();
    source.subscribe(IFACE, &[ORD]).expect("subscribe");

    first.raise(IFACE, ORD, &[1]).expect("raise");
    second.raise(IFACE, ORD, &[2]).expect("raise");
    first.raise(IFACE, ORD, &[3]).expect("raise");

    let mut out = [0u8; 8];
    let mut seqs = Vec::new();
    while let Some(occurrence) = source.next(&mut out).expect("next") {
        seqs.push(occurrence.envelope.seq);
    }
    assert_eq!(seqs, vec![1, 1, 2]);
}

// ---------------------------------------------------------------------------
// Calls.
// ---------------------------------------------------------------------------

#[test]
fn two_callers_on_one_provider_are_two_claims_under_one_seq() {
    // driftsys/ridl#308, and the rule ADR-0021 decision 5 fixes over it: a
    // caller's sequence number is unique per caller, not per channel, so two
    // callers on their first call both carry seq 1 -- and they are still two
    // claims, never merged. A provider deduplicating on the number alone is
    // what #308 reports as wrong; what tells these two apart here is the
    // claim.
    let rt = runtime();
    let mut first = rt.caller();
    let mut second = rt.caller();
    let mut handler = rt.handler();

    let a = first.command(IFACE, ORD, &[1]).expect("send");
    let b = second.command(IFACE, ORD, &[2]).expect("send");
    assert_ne!(a, b, "the correlations are distinct");

    let mut buf = [0u8; 8];
    let first_claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    assert_eq!(&buf[..first_claim.len], &[1]);
    let second_claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    assert_eq!(&buf[..second_claim.len], &[2]);
    assert_eq!(first_claim.envelope.seq, 1);
    assert_eq!(
        second_claim.envelope.seq, 1,
        "both callers are on their first call, so both carry seq 1"
    );
    assert_ne!(
        first_claim.id, second_claim.id,
        "and they are two claims, not one"
    );

    handler.settle(first_claim.id, Ok(&[])).expect("settle");
    handler.settle(second_claim.id, Ok(&[])).expect("settle");
    assert_eq!(first.ack(a), Some(Ok(())));
    assert_eq!(second.ack(b), Some(Ok(())));
}

#[test]
fn a_caller_sequence_number_counts_that_caller_calls() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    caller.command(IFACE, ORD, &[1]).expect("send");
    caller.query(IFACE, ORD, &[2]).expect("send");

    let mut buf = [0u8; 8];
    let first = handler
        .next_claim(&mut buf)
        .expect("read")
        .expect("waiting");
    let second = handler
        .next_claim(&mut buf)
        .expect("read")
        .expect("waiting");
    assert_eq!(first.envelope.seq, 1);
    assert_eq!(
        second.envelope.seq, 2,
        "a command and a query share one counter"
    );
}

#[test]
fn a_settled_outcome_reports_the_contract_error_the_provider_settled() {
    let mut rt = runtime();
    let correlation = rt.command(IFACE, ORD, &[1]).expect("send");
    let mut buf = [0u8; 8];
    let claim = rt.next_claim(&mut buf).expect("read").expect("waiting");
    rt.settle(
        claim.id,
        Err(CallError::Contract(Contract::PreconditionFailed)),
    )
    .expect("settle");

    assert_eq!(
        rt.ack(correlation),
        Some(Err(CallError::Contract(Contract::PreconditionFailed)))
    );
}

#[test]
fn a_claim_is_presented_once_and_settled_once() {
    let mut rt = runtime();
    rt.command(IFACE, ORD, &[1]).expect("send");
    let mut buf = [0u8; 8];
    let claim = rt.next_claim(&mut buf).expect("read").expect("waiting");
    assert!(
        rt.next_claim(&mut buf).expect("read").is_none(),
        "the claim is presented once"
    );

    rt.settle(claim.id, Ok(&[])).expect("settle");
    assert_eq!(
        rt.settle(claim.id, Ok(&[])),
        Err(SettleError::UnknownClaim),
        "a claim already settled is unknown to a second settlement"
    );
}

#[test]
fn a_short_buffer_leaves_the_claim_for_the_next_call() {
    let mut rt = runtime();
    rt.command(IFACE, ORD, &[1, 2, 3]).expect("send");

    let mut short = [0u8; 1];
    assert_eq!(
        rt.next_claim(&mut short),
        Err(ReadError::Short { needed: 3 })
    );

    let mut buf = [0u8; 8];
    let claim = rt
        .next_claim(&mut buf)
        .expect("read")
        .expect("still waiting");
    assert_eq!(&buf[..claim.len], &[1, 2, 3]);
}

#[test]
fn forget_releases_a_settled_correlation() {
    let mut rt = runtime();
    let correlation = rt.command(IFACE, ORD, &[1]).expect("send");
    let mut buf = [0u8; 8];
    let claim = rt.next_claim(&mut buf).expect("read").expect("waiting");
    rt.settle(claim.id, Ok(&[])).expect("settle");
    assert_eq!(rt.ack(correlation), Some(Ok(())));

    rt.forget(correlation);
    assert_eq!(
        rt.ack(correlation),
        None,
        "the outcome is no longer retrievable"
    );
}

#[test]
fn forget_before_the_claim_is_presented_leaves_the_call_for_the_provider() {
    // `Caller::forget` releases the caller's interest in an outcome. It is not
    // a cancellation: `Handler`'s contract is that every claim is settled, and
    // a call already sent is the provider's.
    let mut rt = runtime();
    let correlation = rt.command(IFACE, ORD, &[1]).expect("send");
    rt.forget(correlation);

    let mut buf = [0u8; 8];
    let claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("the call is still presented");
    assert_eq!(&buf[..claim.len], &[1]);
    rt.settle(claim.id, Ok(&[])).expect("and is still settled");
    assert_eq!(
        rt.ack(correlation),
        None,
        "but the caller asked not to be told"
    );
}

#[test]
fn forget_between_the_claim_and_the_settlement_leaves_the_settlement_valid() {
    let mut rt = runtime();
    let correlation = rt.query(IFACE, ORD, &[1]).expect("send");
    let mut buf = [0u8; 8];
    let claim = rt.next_claim(&mut buf).expect("read").expect("waiting");
    rt.forget(correlation);

    rt.settle(claim.id, Ok(&[7]))
        .expect("the provider's settlement is not the caller's to revoke");
    let mut out = [0u8; 8];
    assert!(
        rt.reply(correlation, &mut out)
            .expect("reply read")
            .is_none()
    );
}

#[test]
fn a_claim_that_was_never_presented_cannot_be_settled() {
    // A correlation is not a claim. Before this call is presented there is no
    // claim to settle, and a settlement accepted here would acknowledge a call
    // the provider has not seen.
    let mut rt = runtime();
    let correlation = rt.command(IFACE, ORD, &[1]).expect("send");
    assert_eq!(
        rt.settle(ClaimId(correlation.0), Ok(&[])),
        Err(SettleError::UnknownClaim)
    );
    assert_eq!(rt.ack(correlation), None, "and nothing was acknowledged");

    let mut buf = [0u8; 8];
    let claim = rt.next_claim(&mut buf).expect("read").expect("waiting");
    rt.settle(claim.id, Ok(&[]))
        .expect("the real claim settles");
    assert_eq!(rt.ack(correlation), Some(Ok(())));
}

#[test]
fn an_injected_settle_failure_is_not_spent_on_an_unknown_claim() {
    let mut rt = runtime();
    rt.command(IFACE, ORD, &[1]).expect("send");
    let mut buf = [0u8; 8];
    let claim = rt.next_claim(&mut buf).expect("read").expect("waiting");

    rt.fail_next_settle();
    assert_eq!(
        rt.settle(ClaimId(9999), Ok(&[])),
        Err(SettleError::UnknownClaim),
        "the claim is checked before the injected failure is consumed"
    );
    assert_eq!(
        rt.settle(claim.id, Ok(&[])),
        Err(SettleError::TooLarge { cap: 0 }),
        "so the injected failure still has the next real settlement to fail"
    );
}

#[test]
fn serve_records_what_it_was_asked_to_present() {
    let rt = runtime();
    let mut handler = rt.handler();
    handler.serve(IFACE, &[ORD, OTHER]).expect("serve");
    handler.serve(IFACE, &[ORD]).expect("serve again");
    assert_eq!(handler.served(), &[(IFACE, ORD), (IFACE, OTHER)]);
}

#[test]
fn a_handler_that_served_nothing_is_presented_every_call() {
    // The generated `dispatch` never calls `serve`, so an empty served set is
    // no filter rather than no members.
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    caller.command(InterfaceNo(2), OTHER, &[1]).expect("send");

    let mut buf = [0u8; 8];
    let claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    assert_eq!(claim.iface, InterfaceNo(2));
}

#[test]
fn two_handlers_each_receive_only_what_they_served() {
    // Two components providing different interfaces in one process: the
    // obvious use of an in-process runtime. A handler presented another
    // handler's call would settle it `UnknownInteraction` through the
    // generated dispatch, and the call would be lost.
    let rt = runtime();
    let mut caller = rt.caller();
    let mut first = rt.handler();
    let mut second = rt.handler();
    first.serve(IFACE, &[ORD]).expect("serve");
    second.serve(InterfaceNo(2), &[ORD]).expect("serve");

    caller.command(InterfaceNo(2), ORD, &[7]).expect("send");
    caller.command(IFACE, ORD, &[8]).expect("send");

    let mut buf = [0u8; 8];
    let claim = first
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    assert_eq!(claim.iface, IFACE, "the call the first handler served");
    assert_eq!(&buf[..claim.len], &[8]);

    let claim = second
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    assert_eq!(claim.iface, InterfaceNo(2), "and the second handler's own");
    assert_eq!(&buf[..claim.len], &[7]);

    assert!(
        first.next_claim(&mut buf).expect("next_claim").is_none(),
        "neither handler consumed the other's call"
    );
    assert!(second.next_claim(&mut buf).expect("next_claim").is_none());
}

// ---------------------------------------------------------------------------
// `fixed`.
// ---------------------------------------------------------------------------

#[test]
fn a_provisioned_fixed_reads_back_and_an_unprovisioned_one_reports_it() {
    let mut rt = runtime();
    let mut out = [0u8; 8];
    assert_eq!(
        rt.read_fixed(IFACE, ORD, &mut out),
        Err(ReadError::Contract(Contract::UnknownInteraction)),
        "nothing was provisioned at that ordinal"
    );

    rt.provision_fixed(IFACE, ORD, &[1, 2]);
    assert_eq!(rt.read_fixed(IFACE, ORD, &mut out).expect("read"), 2);
    assert_eq!(&out[..2], &[1, 2]);

    let mut short = [0u8; 1];
    assert_eq!(
        rt.read_fixed(IFACE, ORD, &mut short),
        Err(ReadError::Short { needed: 2 })
    );
}

// ---------------------------------------------------------------------------
// The handle model.
// ---------------------------------------------------------------------------

#[test]
fn the_catalog_is_the_one_the_runtime_was_built_with() {
    let rt = runtime();
    assert_eq!(*rt.catalog(), catalog());

    let handles = rt.split();
    assert_eq!(*handles.reader.catalog(), catalog());
    assert_eq!(*handles.writer.catalog(), catalog());
    assert_eq!(*handles.source.catalog(), catalog());
    assert_eq!(*handles.sink.catalog(), catalog());
    assert_eq!(*handles.caller.catalog(), catalog());
    assert_eq!(*handles.handler.catalog(), catalog());
}

#[test]
fn a_writer_handle_publishes_on_one_thread_while_a_reader_reads_on_another() {
    // ADR-0021 decision 12's split, exercised rather than asserted: the reader
    // handle is `Sync` and stays here, the writer handle is `Send` and moves.
    let handles = runtime().split();
    let reader = handles.reader;
    let mut writer = handles.writer;

    let publisher = std::thread::spawn(move || {
        for value in 1..=50u8 {
            // Four bytes that must agree: a read that saw part of one
            // publication and part of the next would not.
            writer.set(IFACE, ORD, &[value; 4]).expect("set");
            writer.commit();
        }
    });

    // Reading while the other thread publishes: every read succeeds, and the
    // value seen is one whole publication, never a mixture of two.
    let mut out = [0u8; 8];
    let mut seen = 0u32;
    while seen < 200 {
        let raw = reader.read(IFACE, ORD, &mut out).expect("read");
        if raw.len == 0 {
            continue;
        }
        assert_eq!(raw.len, 4);
        let value = out[0];
        assert!((1..=50).contains(&value));
        assert_eq!(
            &out[..4],
            &[value; 4],
            "a publication is read whole or not at all"
        );
        assert_eq!(
            raw.envelope.seq,
            u64::from(value),
            "and its envelope belongs to the value read"
        );
        seen += 1;
    }

    publisher.join().expect("the publishing thread finished");
    let raw = reader.read(IFACE, ORD, &mut out).expect("read");
    assert_eq!(&out[..raw.len], &[50; 4]);
    assert_eq!(raw.envelope.seq, 50);
}

#[test]
fn a_reader_handle_is_shared_between_threads() {
    let handles = runtime().split();
    let mut writer = handles.writer;
    writer.set(IFACE, ORD, &[3]).expect("set");
    writer.commit();

    let reader = std::sync::Arc::new(handles.reader);
    let readers: Vec<_> = (0..4)
        .map(|_| {
            let reader = std::sync::Arc::clone(&reader);
            std::thread::spawn(move || {
                let mut out = [0u8; 8];
                let raw = reader.read(IFACE, ORD, &mut out).expect("read");
                out[..raw.len].to_vec()
            })
        })
        .collect();

    for reader in readers {
        assert_eq!(reader.join().expect("the reading thread finished"), vec![3]);
    }
}

// A `RawSample` and a `Changed` to fill an output slice with before a call
// writes over it. Neither type derives `Default`, so the tests write one out.
fn blank_sample() -> RawSample {
    RawSample {
        provenance: Provenance::Init,
        freshness: Freshness::Unbounded,
        envelope: Envelope {
            stamp: Timestamp(0),
            seq: 0,
        },
        len: 0,
    }
}

fn blank_change() -> Changed {
    Changed {
        iface: IFACE,
        ord: ORD,
        seq: 0,
    }
}
