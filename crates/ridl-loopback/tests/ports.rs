//! The tests of what only this runtime can express.
//!
//! The port contract tests that any runtime can run live in
//! `crates/ridl-rt-conformance`, and `tests/conformance.rs` runs them over
//! this runtime (story E11.20, driftsys/ridl#514). They came from this file.
//! What stays here, and why each test is not in the suite. Each reason is an
//! item of the list "What the suite leaves out" in that crate's
//! documentation:
//!
//! - `two_runtimes_start_at_the_same_logical_time` — where a clock starts is
//!   left to a runtime. The suite asks only that the clock move through the
//!   factory's `advance` hook and through nothing else; this runtime's clock
//!   starts at `Timestamp` 0.
//! - `the_clock_refuses_to_run_backwards` — what `advance` does with a
//!   negative duration is left to a runtime. This panic is
//!   `Loopback::advance`'s own precondition.
//! - `every_value_is_unbounded_because_the_runtime_has_no_member_table` —
//!   a report from a catalog descriptor. `Freshness::Unbounded` follows from
//!   this runtime holding none; a runtime that reads a staleness bound
//!   reports `Fresh` or `Stale`.
//! - `a_sink_counts_its_own_channel_and_not_another_sinks` — two sinks on one
//!   event channel. An event channel has one provider (ridl §5), so another
//!   runtime may refuse the second sink; this runtime does not police the
//!   misuse.
//! - `serve_records_what_it_was_asked_to_present` — a runtime's own API
//!   beyond the factory: `HandlerHandle::served`.
//! - `a_handler_that_served_nothing_is_presented_every_call` — a handler that
//!   has served nothing. This runtime deviates on purpose from
//!   `Handler::serve`, which starts presentation at the members listed, and
//!   presents every call, because the generated `dispatch` never calls
//!   `serve`.
//! - `a_provisioned_fixed_reads_back_and_an_unprovisioned_one_reports_it` —
//!   `FixedReader`. Provisioning a `fixed` is `Loopback::provision_fixed`, a
//!   runtime's own API with no port, and `Contract::UnknownInteraction` for an
//!   unprovisioned one is this runtime's answer with no member table.
//! - `every_split_handle_carries_the_catalog` — a runtime's own API beyond
//!   the factory: `Loopback::split`. The suite checks the catalog on the aggregate and on
//!   the handles its factory makes.
//! - `the_injected_settle_failure_is_too_large_with_no_capacity` — which
//!   error the injected fault reports is left to a runtime. The suite asks
//!   only for an error other than `UnknownClaim`.
//! - `a_writer_handle_publishes_on_one_thread_while_a_reader_reads_on_another`
//!   and `a_reader_handle_is_shared_between_threads` — the threading model of
//!   ADR-0021 decision 12. That decision holds a single-threaded runtime to
//!   neither `Send` nor `Sync`, and both tests reach the role handles through
//!   `Loopback::split`.
//!
//! The tests under "Waking" pin this runtime's `Wakeable` (story E11.16,
//! driftsys/ridl#510). The suite does not cover `Wakeable` yet; the second
//! half of story E11.20 adds the contract cases to it. Some of the tests here
//! stay after that, because each pins a choice the contract leaves to a
//! runtime: `a_registration_whose_key_already_holds_is_woken_at_once`,
//! `a_waiting_occurrence_of_another_interface_wakes_an_event_registration_at_once`,
//! `a_waiting_call_on_another_interface_wakes_a_claim_registration_at_once`,
//! `a_key_no_role_of_the_handle_observes_is_woken_at_once`,
//! `a_dropped_handler_returns_its_claims_to_the_waiting_calls`,
//! `a_drop_returns_only_the_dropped_handlers_claims_and_wakes_every_serving_handler`,
//! `a_returned_claim_keeps_its_place_by_send_order`,
//! `a_dropped_handler_leaves_no_waiter_behind`,
//! `a_returned_claim_keeps_its_send_order_across_a_reused_slot`,
//! `a_providers_busy_settlement_reaches_the_caller` (this runtime originates
//! no `Transport::Busy` of its own),
//! `a_waker_is_woken_after_the_lock_is_released` and
//! `every_wake_is_run_with_the_lock_released`.
//!
//! The tests under "The bounded call table" pin this runtime's move onto
//! `ridl_rt::correlate` (story E11.18, driftsys/ridl#512). The suite gains a
//! slot and reclaim case in the second half of story E11.20, over the slot
//! count its factory states. Of these, `the_seventeenth_in_flight_call_is_busy`
//! names this runtime's own count, `a_refused_send_draws_no_sequence_number`
//! and `a_slot_registration_while_a_slot_is_free_is_woken_at_once` pin choices
//! the contract leaves to a runtime, and
//! `a_dropped_caller_leaves_no_slot_waiter_behind` pins this runtime's
//! `Drop`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::task::{Wake, Waker};

use ridl_loopback::{Handles, Loopback};
use ridl_rt::contract::{CatalogHash, CatalogRef, InterfaceNo, Ordinal};
use ridl_rt::error::{CallError, Contract, Transport};
use ridl_rt::port::{
    Attached, Caller, Clock, EventSink, EventSource, FixedReader, Handler, Interest, ReadError,
    SendError, SettleError, SignalReader, SignalWriter, Wakeable,
};
use ridl_rt::sample::{Duration, Freshness, Timestamp};

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
// The clock.
// ---------------------------------------------------------------------------

#[test]
fn two_runtimes_start_at_the_same_logical_time() {
    // Two independently constructed runtimes must start at the same logical
    // time regardless of when, in real time, each was constructed — a clock
    // that read wall-clock time could not guarantee that.
    let first = runtime();
    std::thread::sleep(std::time::Duration::from_millis(5));
    let second = runtime();
    assert_eq!(first.now(), Timestamp(0), "the clock starts at 0");
    assert_eq!(
        first.now(),
        second.now(),
        "the clock must not read wall-clock time"
    );
}

#[test]
#[should_panic(expected = "the clock advances forward")]
fn the_clock_refuses_to_run_backwards() {
    let mut rt = runtime();
    rt.advance(Duration(-1));
}

// ---------------------------------------------------------------------------
// Signals.
// ---------------------------------------------------------------------------

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
// Events.
// ---------------------------------------------------------------------------

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
fn the_injected_settle_failure_is_too_large_with_no_capacity() {
    let mut rt = runtime();
    rt.command(IFACE, ORD, &[1]).expect("send");
    let mut buf = [0u8; 8];
    let claim = rt.next_claim(&mut buf).expect("read").expect("waiting");

    rt.fail_next_settle();
    assert_eq!(
        rt.settle(claim.id, Ok(&[])),
        Err(SettleError::TooLarge { cap: 0 }),
        "the one fault this runtime injects"
    );
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
fn every_split_handle_carries_the_catalog() {
    let handles = runtime().split();
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

// ---------------------------------------------------------------------------
// Waking.
// ---------------------------------------------------------------------------

/// A waker that counts its wakes.
struct Count(AtomicUsize);

impl Wake for Count {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

/// A fresh waker and the counter it increments.
fn counting() -> (Arc<Count>, Waker) {
    let count = Arc::new(Count(AtomicUsize::new(0)));
    (Arc::clone(&count), Waker::from(count))
}

fn wakes(count: &Count) -> usize {
    count.0.load(Ordering::SeqCst)
}

#[test]
fn a_waiter_on_an_outcome_is_woken_exactly_once_by_its_settlement() {
    let Handles {
        mut caller,
        mut handler,
        ..
    } = runtime().split();
    let (count, waker) = counting();
    let c = caller.command(IFACE, ORD, &[1]).expect("send");
    caller.wake_on(Interest::Outcome(c), &waker);
    assert_eq!(wakes(&count), 0, "nothing is known about the call yet");

    let mut buf = [0u8; 8];
    let claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    assert_eq!(wakes(&count), 0, "a presented call has no outcome yet");

    handler.settle(claim.id, Ok(&[])).expect("settle");
    assert_eq!(wakes(&count), 1, "the settlement wakes the waiter");
    assert_eq!(caller.ack(c), Some(Ok(())));

    // The waker was cleared when it was woken, so a later settlement of
    // another call does not reach it.
    caller.command(IFACE, ORD, &[2]).expect("send");
    let claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    handler.settle(claim.id, Ok(&[])).expect("settle");
    assert_eq!(wakes(&count), 1, "a waiter is woken at most once");
}

#[test]
fn each_settlement_wakes_only_its_own_calls_waiter() {
    // An `Outcome` waker is per call, not per handle: two calls' wakers are
    // held at once, and a settlement wakes the one of the call it settles.
    let Handles {
        mut caller,
        mut handler,
        ..
    } = runtime().split();
    let mut buf = [0u8; 8];

    let first_call = caller.command(IFACE, ORD, &[1]).expect("send");
    let second_call = caller.command(IFACE, ORD, &[2]).expect("send");
    let (first, first_waker) = counting();
    let (second, second_waker) = counting();
    caller.wake_on(Interest::Outcome(first_call), &first_waker);
    caller.wake_on(Interest::Outcome(second_call), &second_waker);
    assert_eq!(
        (wakes(&first), wakes(&second)),
        (0, 0),
        "two calls, two wakers, no displacement"
    );

    let claim_one = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    let claim_two = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    handler.settle(claim_two.id, Ok(&[])).expect("settle");
    assert_eq!(
        (wakes(&first), wakes(&second)),
        (0, 1),
        "only the second call is settled"
    );
    handler.settle(claim_one.id, Ok(&[])).expect("settle");
    assert_eq!((wakes(&first), wakes(&second)), (1, 1));
}

#[test]
fn a_waiter_registered_after_the_settlement_is_woken_at_once() {
    let Handles {
        mut caller,
        mut handler,
        ..
    } = runtime().split();
    let c = caller.query(IFACE, ORD, &[1]).expect("send");
    let mut buf = [0u8; 8];
    let claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    handler.settle(claim.id, Ok(&[7])).expect("settle");

    let (count, waker) = counting();
    caller.wake_on(Interest::Outcome(c), &waker);
    assert_eq!(wakes(&count), 1, "the outcome is already known");
    assert_eq!(caller.reply(c, &mut buf), Ok(Some(Ok(1))));
}

#[test]
fn a_second_registration_under_a_key_wakes_the_displaced_waker() {
    let rt = runtime();
    let mut sink = rt.sink();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    let mut source = rt.source();
    source.subscribe(IFACE, &[ORD]).expect("subscribe");
    let mut buf = [0u8; 8];

    // An outcome.
    let c = caller.command(IFACE, ORD, &[1]).expect("send");
    let (first, first_waker) = counting();
    let (second, second_waker) = counting();
    caller.wake_on(Interest::Outcome(c), &first_waker);
    caller.wake_on(Interest::Outcome(c), &second_waker);
    assert_eq!(wakes(&first), 1, "the displaced waker is woken");
    assert_eq!(wakes(&second), 0);
    let claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    handler.settle(claim.id, Ok(&[])).expect("settle");
    assert_eq!(wakes(&first), 1, "the displaced waker is not stored");
    assert_eq!(wakes(&second), 1, "the settlement wakes the stored waker");

    // An event.
    let (first, first_waker) = counting();
    let (second, second_waker) = counting();
    source.wake_on(Interest::Event(IFACE), &first_waker);
    source.wake_on(Interest::Event(IFACE), &second_waker);
    assert_eq!(wakes(&first), 1, "the displaced waker is woken");
    sink.raise(IFACE, ORD, &[1]).expect("raise");
    assert_eq!(wakes(&first), 1, "the displaced waker is not stored");
    assert_eq!(wakes(&second), 1, "the raise wakes the stored waker");

    // A claim.
    let (first, first_waker) = counting();
    let (second, second_waker) = counting();
    handler.wake_on(Interest::Claim(IFACE), &first_waker);
    handler.wake_on(Interest::Claim(IFACE), &second_waker);
    assert_eq!(wakes(&first), 1, "the displaced waker is woken");
    caller.command(IFACE, ORD, &[2]).expect("send");
    assert_eq!(wakes(&first), 1, "the displaced waker is not stored");
    assert_eq!(wakes(&second), 1, "the send wakes the stored waker");
}

#[test]
fn a_registration_of_the_waker_already_stored_does_not_wake_it() {
    // A task registers on every poll. If registering its own waker again
    // woke it, every poll would schedule the next one and the task would
    // never become idle.
    let Handles {
        mut caller,
        mut handler,
        ..
    } = runtime().split();
    let c = caller.command(IFACE, ORD, &[1]).expect("send");
    let (count, waker) = counting();
    caller.wake_on(Interest::Outcome(c), &waker);
    caller.wake_on(Interest::Outcome(c), &waker.clone());
    assert_eq!(wakes(&count), 0, "the same task registered twice");

    let mut buf = [0u8; 8];
    let claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    handler.settle(claim.id, Ok(&[])).expect("settle");
    assert_eq!(wakes(&count), 1, "and is still woken by the settlement");
}

#[test]
fn the_same_task_registering_an_event_or_claim_key_again_wakes_nothing() {
    // The refresh rule for the two per-kind slots, under the same key each
    // time: the stored waker is replaced, not woken, and the change still
    // wakes the task.
    let rt = runtime();
    let mut sink = rt.sink();
    let mut caller = rt.caller();
    let mut source = rt.source();
    let handler = rt.handler();
    source.subscribe(IFACE, &[ORD]).expect("subscribe");

    let (event, event_waker) = counting();
    source.wake_on(Interest::Event(IFACE), &event_waker);
    source.wake_on(Interest::Event(IFACE), &event_waker.clone());
    assert_eq!(wakes(&event), 0, "`Event`: the same task is refreshed");
    sink.raise(IFACE, ORD, &[1]).expect("raise");
    assert_eq!(wakes(&event), 1, "and the raise wakes it");

    let (claim, claim_waker) = counting();
    handler.wake_on(Interest::Claim(IFACE), &claim_waker);
    handler.wake_on(Interest::Claim(IFACE), &claim_waker.clone());
    assert_eq!(wakes(&claim), 0, "`Claim`: the same task is refreshed");
    caller.command(IFACE, ORD, &[1]).expect("send");
    assert_eq!(wakes(&claim), 1, "and the send wakes it");
}

#[test]
fn a_raise_wakes_a_subscribed_source_and_not_an_unsubscribed_one() {
    let rt = runtime();
    let mut sink = rt.sink();
    let mut subscribed = rt.source();
    let mut elsewhere = rt.source();
    subscribed.subscribe(IFACE, &[ORD]).expect("subscribe");
    // Subscribed to another event of the same interface: its key is the same
    // `Event(IFACE)`, and the occurrence below is still not its.
    elsewhere.subscribe(IFACE, &[OTHER]).expect("subscribe");
    let unsubscribed = rt.source();

    let (woken, woken_waker) = counting();
    let (other, other_waker) = counting();
    let (none, none_waker) = counting();
    subscribed.wake_on(Interest::Event(IFACE), &woken_waker);
    elsewhere.wake_on(Interest::Event(IFACE), &other_waker);
    unsubscribed.wake_on(Interest::Event(IFACE), &none_waker);

    sink.raise(IFACE, ORD, &[1]).expect("raise");
    assert_eq!(wakes(&woken), 1, "the subscribed source is woken");
    assert_eq!(
        wakes(&other),
        0,
        "a source subscribed to another event is not"
    );
    assert_eq!(wakes(&none), 0, "an unsubscribed source is not");

    let mut out = [0u8; 8];
    assert!(subscribed.next(&mut out).expect("next").is_some());
}

#[test]
fn a_source_registered_under_two_interfaces_is_woken_by_either() {
    // One waker per kind of key: the second registration of the same task
    // refreshes the `Event` waker rather than narrowing it to either
    // interface (ADR-0021 decision 13). Each interface is tried in turn.
    let rt = runtime();
    let mut sink = rt.sink();
    let mut source = rt.source();
    source.subscribe(IFACE, &[ORD]).expect("subscribe");
    source.subscribe(InterfaceNo(2), &[ORD]).expect("subscribe");
    let mut out = [0u8; 8];
    let (count, waker) = counting();

    for (turn, iface) in [IFACE, InterfaceNo(2)].into_iter().enumerate() {
        source.wake_on(Interest::Event(IFACE), &waker);
        source.wake_on(Interest::Event(InterfaceNo(2)), &waker);
        assert_eq!(wakes(&count), turn, "the same task registered twice");
        sink.raise(iface, ORD, &[1]).expect("raise");
        assert_eq!(
            wakes(&count),
            turn + 1,
            "an occurrence of interface {} wakes it",
            iface.0
        );
        while source.next(&mut out).expect("next").is_some() {}
    }
}

#[test]
fn a_handler_registered_under_two_interfaces_is_woken_by_either() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    handler.serve(IFACE, &[ORD]).expect("serve");
    handler.serve(InterfaceNo(2), &[ORD]).expect("serve");
    let mut buf = [0u8; 8];
    let (count, waker) = counting();

    for (turn, iface) in [IFACE, InterfaceNo(2)].into_iter().enumerate() {
        handler.wake_on(Interest::Claim(IFACE), &waker);
        handler.wake_on(Interest::Claim(InterfaceNo(2)), &waker);
        assert_eq!(wakes(&count), turn, "the same task registered twice");
        caller.command(iface, ORD, &[1]).expect("send");
        assert_eq!(
            wakes(&count),
            turn + 1,
            "a call on interface {} wakes it",
            iface.0
        );
        let claim = handler
            .next_claim(&mut buf)
            .expect("next_claim")
            .expect("waiting");
        handler.settle(claim.id, Ok(&[])).expect("settle");
    }
}

#[test]
fn a_registration_under_one_interface_is_woken_by_a_change_on_another() {
    // Per-kind storage, with one registration only: the interface of the key
    // is not kept, so a store that keeps one waker per key, and wakes it only
    // for that key, leaves both tasks asleep here.
    let rt = runtime();
    let mut sink = rt.sink();
    let mut caller = rt.caller();
    let mut source = rt.source();
    let mut handler = rt.handler();
    source.subscribe(InterfaceNo(2), &[ORD]).expect("subscribe");
    handler.serve(InterfaceNo(2), &[ORD]).expect("serve");

    let (event, event_waker) = counting();
    source.wake_on(Interest::Event(IFACE), &event_waker);
    sink.raise(InterfaceNo(2), ORD, &[1]).expect("raise");
    assert_eq!(wakes(&event), 1, "an occurrence of interface 2 wakes it");

    let (claim, claim_waker) = counting();
    handler.wake_on(Interest::Claim(IFACE), &claim_waker);
    caller.command(InterfaceNo(2), ORD, &[1]).expect("send");
    assert_eq!(wakes(&claim), 1, "a call on interface 2 wakes it");
}

#[test]
fn two_tasks_under_two_interfaces_of_one_kind_share_the_one_slot() {
    // Per-kind storage, with two tasks: task A registers under interface 1
    // and task B under interface 2 of the same kind on one handle. B
    // displaces A, so A is woken at once, and a change on interface 1 then
    // wakes B, the one stored waker. A per-key store would keep both, wake
    // neither at registration, and wake A rather than B on the change.
    let rt = runtime();
    let mut sink = rt.sink();
    let mut caller = rt.caller();
    let mut source = rt.source();
    let mut handler = rt.handler();
    source.subscribe(IFACE, &[ORD]).expect("subscribe");
    handler.serve(IFACE, &[ORD]).expect("serve");

    let (a, a_waker) = counting();
    let (b, b_waker) = counting();
    source.wake_on(Interest::Event(IFACE), &a_waker);
    source.wake_on(Interest::Event(InterfaceNo(2)), &b_waker);
    assert_eq!((wakes(&a), wakes(&b)), (1, 0), "B displaces A");
    sink.raise(IFACE, ORD, &[1]).expect("raise");
    assert_eq!((wakes(&a), wakes(&b)), (1, 1), "interface 1 wakes B");

    let (a, a_waker) = counting();
    let (b, b_waker) = counting();
    handler.wake_on(Interest::Claim(IFACE), &a_waker);
    handler.wake_on(Interest::Claim(InterfaceNo(2)), &b_waker);
    assert_eq!((wakes(&a), wakes(&b)), (1, 0), "B displaces A");
    caller.command(IFACE, ORD, &[1]).expect("send");
    assert_eq!((wakes(&a), wakes(&b)), (1, 1), "interface 1 wakes B");
}

#[test]
fn a_waiting_occurrence_of_another_interface_wakes_an_event_registration_at_once() {
    let rt = runtime();
    let mut sink = rt.sink();
    let mut source = rt.source();
    source.subscribe(InterfaceNo(2), &[ORD]).expect("subscribe");
    sink.raise(InterfaceNo(2), ORD, &[1]).expect("raise");

    let (count, waker) = counting();
    source.wake_on(Interest::Event(IFACE), &waker);
    assert_eq!(
        wakes(&count),
        1,
        "an occurrence of another interface is waiting"
    );
}

#[test]
fn a_waiting_call_on_another_interface_wakes_a_claim_registration_at_once() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    handler.serve(InterfaceNo(2), &[ORD]).expect("serve");
    caller.command(InterfaceNo(2), ORD, &[1]).expect("send");

    let (count, waker) = counting();
    handler.wake_on(Interest::Claim(IFACE), &waker);
    assert_eq!(wakes(&count), 1, "a call on another interface is waiting");
}

#[test]
fn an_event_or_claim_waiter_is_cleared_when_woken() {
    let rt = runtime();
    let mut sink = rt.sink();
    let mut caller = rt.caller();
    let mut source = rt.source();
    let mut handler = rt.handler();
    source.subscribe(IFACE, &[ORD]).expect("subscribe");
    handler.serve(IFACE, &[ORD]).expect("serve");

    let (event, event_waker) = counting();
    source.wake_on(Interest::Event(IFACE), &event_waker);
    sink.raise(IFACE, ORD, &[1]).expect("raise");
    sink.raise(IFACE, ORD, &[2]).expect("raise");
    assert_eq!(wakes(&event), 1, "the first raise cleared the waker");

    let (claim, claim_waker) = counting();
    handler.wake_on(Interest::Claim(IFACE), &claim_waker);
    caller.command(IFACE, ORD, &[1]).expect("send");
    caller.command(IFACE, ORD, &[2]).expect("send");
    assert_eq!(wakes(&claim), 1, "the first send cleared the waker");
}

#[test]
fn a_drop_returns_only_the_dropped_handlers_claims_and_wakes_every_serving_handler() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut keeper = rt.handler();
    let mut dropped = rt.handler();
    let mut other = rt.handler();
    let mut elsewhere = rt.handler();
    keeper.serve(IFACE, &[ORD]).expect("serve");
    dropped.serve(IFACE, &[ORD]).expect("serve");
    other.serve(IFACE, &[ORD]).expect("serve");
    elsewhere.serve(IFACE, &[OTHER]).expect("serve");
    let mut buf = [0u8; 8];

    let kept = caller.command(IFACE, ORD, &[1]).expect("send");
    let kept_claim = keeper
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    caller.command(IFACE, ORD, &[2]).expect("send");
    dropped
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");

    let (first, first_waker) = counting();
    let (second, second_waker) = counting();
    let (third, third_waker) = counting();
    keeper.wake_on(Interest::Claim(IFACE), &first_waker);
    other.wake_on(Interest::Claim(IFACE), &second_waker);
    elsewhere.wake_on(Interest::Claim(IFACE), &third_waker);
    drop(dropped);
    assert_eq!(
        wakes(&first),
        1,
        "every serving handler is woken: the first"
    );
    assert_eq!(
        wakes(&second),
        1,
        "every serving handler is woken: the second"
    );
    assert_eq!(
        wakes(&third),
        0,
        "a handler serving another member is not woken"
    );
    assert_eq!(
        elsewhere.next_claim(&mut buf).expect("next_claim"),
        None,
        "and is presented nothing"
    );

    let returned = other
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("returned");
    assert_eq!(
        &buf[..returned.len],
        &[2],
        "only the dropped handler's call"
    );
    assert_eq!(
        other.next_claim(&mut buf).expect("next_claim"),
        None,
        "the call another handler holds stays with it"
    );
    keeper
        .settle(kept_claim.id, Ok(&[]))
        .expect("the keeper still holds its claim");
    assert_eq!(caller.ack(kept), Some(Ok(())));
}

#[test]
fn a_waiter_is_cleared_when_woken() {
    let Handles {
        mut caller,
        mut handler,
        ..
    } = runtime().split();
    let c = caller.command(IFACE, ORD, &[1]).expect("send");
    let (first, first_waker) = counting();
    caller.wake_on(Interest::Outcome(c), &first_waker);
    let mut buf = [0u8; 8];
    let claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    handler.settle(claim.id, Ok(&[])).expect("settle");
    assert_eq!(wakes(&first), 1);

    // Another task registers on the settled call. Had the settlement left the
    // first waker stored, this registration would displace and wake it again.
    let (second, second_waker) = counting();
    caller.wake_on(Interest::Outcome(c), &second_waker);
    assert_eq!(wakes(&second), 1, "the outcome is known");
    assert_eq!(wakes(&first), 1, "the first waker was cleared when woken");
}

#[test]
fn a_waiter_woken_at_once_is_not_stored() {
    let rt = runtime();
    let mut sink = rt.sink();
    let mut source = rt.source();
    source.subscribe(IFACE, &[ORD]).expect("subscribe");
    sink.raise(IFACE, ORD, &[1]).expect("raise");
    let (count, waker) = counting();
    source.wake_on(Interest::Event(IFACE), &waker);
    assert_eq!(wakes(&count), 1, "an occurrence is waiting");

    sink.raise(IFACE, ORD, &[2]).expect("raise");
    assert_eq!(wakes(&count), 1, "a waker woken at once was not stored");
}

#[test]
fn a_registration_on_a_forgotten_call_is_woken_at_once() {
    // A forgotten call in flight is still in the call table, but no outcome
    // will be recorded for it.
    let Handles { mut caller, .. } = runtime().split();
    let c = caller.command(IFACE, ORD, &[1]).expect("send");
    caller.forget(c);
    let (count, waker) = counting();
    caller.wake_on(Interest::Outcome(c), &waker);
    assert_eq!(wakes(&count), 1, "no outcome will be recorded");
}

#[test]
fn every_subscribed_source_and_every_serving_handler_is_woken() {
    let rt = runtime();
    let mut sink = rt.sink();
    let mut caller = rt.caller();
    let mut first_source = rt.source();
    let mut second_source = rt.source();
    first_source.subscribe(IFACE, &[ORD]).expect("subscribe");
    second_source.subscribe(IFACE, &[ORD]).expect("subscribe");
    let mut first_handler = rt.handler();
    let mut second_handler = rt.handler();
    first_handler.serve(IFACE, &[ORD]).expect("serve");
    second_handler.serve(IFACE, &[ORD]).expect("serve");

    let wakers: Vec<_> = (0..4).map(|_| counting()).collect();
    first_source.wake_on(Interest::Event(IFACE), &wakers[0].1);
    second_source.wake_on(Interest::Event(IFACE), &wakers[1].1);
    first_handler.wake_on(Interest::Claim(IFACE), &wakers[2].1);
    second_handler.wake_on(Interest::Claim(IFACE), &wakers[3].1);

    sink.raise(IFACE, ORD, &[1]).expect("raise");
    assert_eq!(wakes(&wakers[0].0), 1, "the first source");
    assert_eq!(wakes(&wakers[1].0), 1, "the second source");

    caller.command(IFACE, ORD, &[1]).expect("send");
    assert_eq!(wakes(&wakers[2].0), 1, "the first handler");
    assert_eq!(wakes(&wakers[3].0), 1, "the second handler");
}

#[test]
fn a_query_wakes_the_handler_that_serves_it() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    handler.serve(IFACE, &[ORD]).expect("serve");
    let (count, waker) = counting();
    handler.wake_on(Interest::Claim(IFACE), &waker);
    caller.query(IFACE, ORD, &[1]).expect("send");
    assert_eq!(wakes(&count), 1, "the query wakes the serving handler");
}

#[test]
fn a_claim_registration_is_not_woken_by_a_call_the_handler_does_not_serve() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    handler.serve(IFACE, &[OTHER]).expect("serve");
    caller.command(IFACE, ORD, &[1]).expect("send");
    let (count, waker) = counting();
    handler.wake_on(Interest::Claim(IFACE), &waker);
    assert_eq!(wakes(&count), 0, "the waiting call is not this handler's");
}

#[test]
fn a_serve_that_admits_nothing_does_not_wake_the_handler() {
    let rt = runtime();
    let mut handler = rt.handler();
    handler.serve(IFACE, &[OTHER]).expect("serve");
    let (count, waker) = counting();
    handler.wake_on(Interest::Claim(IFACE), &waker);
    handler.serve(IFACE, &[ORD]).expect("serve");
    assert_eq!(wakes(&count), 0, "no call is waiting");
}

#[test]
fn a_dropped_handler_returns_its_claims_to_the_waiting_calls() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut first = rt.handler();
    let mut second = rt.handler();
    first.serve(IFACE, &[ORD]).expect("serve");
    second.serve(IFACE, &[ORD]).expect("serve");
    let mut buf = [0u8; 8];

    let c = caller.command(IFACE, ORD, &[1]).expect("send");
    first
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    let later = caller.command(IFACE, ORD, &[2]).expect("send");
    first
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    assert_eq!(
        second.next_claim(&mut buf).expect("next_claim"),
        None,
        "both calls are held by the first handler"
    );
    let (count, waker) = counting();
    second.wake_on(Interest::Claim(IFACE), &waker);
    assert_eq!(wakes(&count), 0);
    // The caller waits on the first call across the drop: the return does not
    // wake it, and the settlement by the handler that takes it does.
    let (outcome, outcome_waker) = counting();
    caller.wake_on(Interest::Outcome(c), &outcome_waker);

    drop(first);
    assert_eq!(
        wakes(&count),
        1,
        "the returned claims wake a serving handler"
    );
    assert_eq!(wakes(&outcome), 0, "a returned claim is not an outcome");

    // They return in send order, and are settled by the handler that takes
    // them.
    let again = second
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("returned");
    assert_eq!(&buf[..again.len], &[1], "the earlier call first");
    second.settle(again.id, Ok(&[])).expect("settle");
    assert_eq!(
        wakes(&outcome),
        1,
        "the settlement by the second handler wakes the caller"
    );
    let then = second
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("returned");
    assert_eq!(&buf[..then.len], &[2]);
    second.settle(then.id, Ok(&[])).expect("settle");
    assert_eq!(caller.ack(c), Some(Ok(())));
    assert_eq!(caller.ack(later), Some(Ok(())));
}

#[test]
fn a_send_wakes_the_handler_that_serves_the_member() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut serving = rt.handler();
    let mut elsewhere = rt.handler();
    serving.serve(IFACE, &[ORD]).expect("serve");
    // Serves another member of the same interface: its key is the same
    // `Claim(IFACE)`, and the call below is still not presented to it.
    elsewhere.serve(IFACE, &[OTHER]).expect("serve");

    let (woken, woken_waker) = counting();
    let (other, other_waker) = counting();
    serving.wake_on(Interest::Claim(IFACE), &woken_waker);
    elsewhere.wake_on(Interest::Claim(IFACE), &other_waker);

    caller.command(IFACE, ORD, &[1]).expect("send");
    assert_eq!(wakes(&woken), 1, "the serving handler is woken");
    assert_eq!(wakes(&other), 0, "a handler serving another member is not");

    let mut buf = [0u8; 8];
    assert!(serving.next_claim(&mut buf).expect("next_claim").is_some());
}

#[test]
fn a_serve_that_admits_a_waiting_call_wakes_the_handler() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    handler.serve(IFACE, &[OTHER]).expect("serve");
    let (count, waker) = counting();
    handler.wake_on(Interest::Claim(IFACE), &waker);
    caller.command(IFACE, ORD, &[1]).expect("send");
    assert_eq!(wakes(&count), 0, "the handler does not serve the member");

    handler.serve(IFACE, &[ORD]).expect("serve");
    assert_eq!(wakes(&count), 1, "the call is now waiting for this handler");
}

#[test]
fn a_caller_handle_reads_the_clock() {
    let mut rt = runtime();
    let caller = rt.caller();
    rt.advance(Duration(250));
    assert_eq!(caller.now(), Timestamp(250));
    assert_eq!(caller.now(), rt.now());
}

#[test]
fn a_registration_whose_key_already_holds_is_woken_at_once() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut sink = rt.sink();
    let mut source = rt.source();
    let handler = rt.handler();
    source.subscribe(IFACE, &[ORD]).expect("subscribe");

    // Nothing is in flight, so a slot is free.
    let (slot, slot_waker) = counting();
    caller.wake_on(Interest::Slot, &slot_waker);
    assert_eq!(wakes(&slot), 1, "a slot is free");

    sink.raise(IFACE, ORD, &[1]).expect("raise");
    let (event, event_waker) = counting();
    source.wake_on(Interest::Event(IFACE), &event_waker);
    assert_eq!(wakes(&event), 1, "an occurrence is waiting");

    caller.command(IFACE, ORD, &[1]).expect("send");
    let (claim, claim_waker) = counting();
    handler.wake_on(Interest::Claim(IFACE), &claim_waker);
    assert_eq!(wakes(&claim), 1, "a call is waiting");

    // A correlation that names no call in flight has no outcome to wait for;
    // storing its waker would leave it waiting on nothing.
    let (unknown, unknown_waker) = counting();
    caller.wake_on(
        Interest::Outcome(ridl_rt::port::Correlation(99)),
        &unknown_waker,
    );
    assert_eq!(wakes(&unknown), 1, "no call has that correlation");
}

#[test]
fn a_forgotten_call_wakes_its_waiter() {
    // After `forget`, a settlement records nothing, so the stored waker would
    // never be woken.
    let Handles { mut caller, .. } = runtime().split();
    let c = caller.command(IFACE, ORD, &[1]).expect("send");
    let (count, waker) = counting();
    caller.wake_on(Interest::Outcome(c), &waker);
    caller.forget(c);
    assert_eq!(wakes(&count), 1, "the forget wakes the waiter");
}

#[test]
fn a_key_no_role_of_the_handle_observes_is_woken_at_once() {
    // A reader handle holds no call, no queue and no served set, so nothing
    // it could observe changes under any key; a stored waker would never be
    // woken.
    let Handles {
        reader,
        writer,
        sink,
        caller,
        source,
        handler,
    } = runtime().split();
    let (count, waker) = counting();
    reader.wake_on(Interest::Slot, &waker);
    writer.wake_on(Interest::Event(IFACE), &waker);
    sink.wake_on(Interest::Claim(IFACE), &waker);
    caller.wake_on(Interest::Event(IFACE), &waker);
    caller.wake_on(Interest::Claim(IFACE), &waker);
    source.wake_on(Interest::Slot, &waker);
    source.wake_on(Interest::Claim(IFACE), &waker);
    source.wake_on(Interest::Outcome(ridl_rt::port::Correlation(0)), &waker);
    handler.wake_on(Interest::Slot, &waker);
    handler.wake_on(Interest::Event(IFACE), &waker);
    handler.wake_on(Interest::Outcome(ridl_rt::port::Correlation(0)), &waker);
    assert_eq!(wakes(&count), 11);
}

#[test]
fn a_dropped_handler_leaves_no_waiter_behind() {
    // The one `Drop` removes the handler's state, its stored waker with it,
    // and returns its claims; a later send reaches no waker of the dropped
    // handler.
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    let mut buf = [0u8; 8];
    // The handler holds a claim, so its drop returns one; the handler's own
    // state must be gone before the return wakes the handlers that serve the
    // member, or the dropped handler's waker would be woken with them.
    caller.command(IFACE, ORD, &[1]).expect("send");
    handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    let (count, waker) = counting();
    handler.wake_on(Interest::Claim(IFACE), &waker);
    assert_eq!(Arc::strong_count(&count), 3, "the store holds a clone");
    drop(handler);
    assert_eq!(Arc::strong_count(&count), 2, "the drop released it");
    assert_eq!(wakes(&count), 0, "the returned claim does not wake it");
    caller.command(IFACE, ORD, &[2]).expect("send");
    assert_eq!(wakes(&count), 0, "nothing wakes a dropped handler's waker");
}

#[test]
fn a_dropped_source_leaves_no_waiter_behind() {
    let rt = runtime();
    let mut sink = rt.sink();
    let mut source = rt.source();
    source.subscribe(IFACE, &[ORD]).expect("subscribe");
    let (count, waker) = counting();
    source.wake_on(Interest::Event(IFACE), &waker);
    assert_eq!(Arc::strong_count(&count), 3, "the store holds a clone");
    drop(source);
    assert_eq!(Arc::strong_count(&count), 2, "the drop released it");
    sink.raise(IFACE, ORD, &[1]).expect("raise");
    assert_eq!(wakes(&count), 0, "nothing wakes a dropped source's waker");
}

#[test]
fn a_providers_busy_settlement_reaches_the_caller() {
    // The loopback never refuses a call itself; a provider that settles with
    // `Transport::Busy` has it carried back unchanged, to `ack` for a
    // command and to `reply` for a query.
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    let mut buf = [0u8; 8];
    let busy = CallError::Transport(Transport::Busy);

    let command = caller.command(IFACE, ORD, &[1]).expect("send");
    let query = caller.query(IFACE, ORD, &[2]).expect("send");
    while let Some(claim) = handler.next_claim(&mut buf).expect("next_claim") {
        handler.settle(claim.id, Err(busy)).expect("settle");
    }
    assert_eq!(caller.ack(command), Some(Err(busy)));
    assert_eq!(caller.reply(query, &mut buf), Ok(Some(Err(busy))));
}

#[test]
fn a_returned_claim_keeps_its_place_by_send_order() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut first = rt.handler();
    let mut second = rt.handler();
    let mut buf = [0u8; 8];

    caller.command(IFACE, ORD, &[1]).expect("send");
    first
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    caller.command(IFACE, ORD, &[2]).expect("send");
    drop(first);

    let claim = second
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    assert_eq!(
        &buf[..claim.len],
        &[1],
        "the returned call was sent first, so it is presented first"
    );
}

#[test]
fn a_returned_claim_keeps_its_send_order_across_a_reused_slot() {
    // A reused slot carries a higher generation, so a call sent into it has a
    // larger correlation than a call sent later into a fresh slot. The order
    // a returned claim goes back in is the order the calls were sent, not the
    // order of their correlations.
    let rt = runtime();
    let mut caller = rt.caller();
    let mut first = rt.handler();
    let mut second = rt.handler();
    let mut buf = [0u8; 8];

    let old = caller.command(IFACE, ORD, &[0]).expect("send");
    let claim = first
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    first.settle(claim.id, Ok(&[])).expect("settle");
    caller.forget(old);

    let reused = caller.command(IFACE, ORD, &[1]).expect("send into slot 0");
    let fresh = caller.command(IFACE, ORD, &[2]).expect("send into slot 1");
    assert!(
        reused.0 > fresh.0,
        "the reused slot's correlation is the larger one"
    );
    first
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("the call sent first");
    drop(first);

    let claim = second
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    assert_eq!(
        &buf[..claim.len],
        &[1],
        "the returned call was sent first, so it is presented first"
    );
}

// ---------------------------------------------------------------------------
// The bounded call table: `Loopback::SLOTS` calls in flight, reclaimed by
// `forget` (ADR-0021 decision 15, note F-9).
// ---------------------------------------------------------------------------

/// Sends `Loopback::SLOTS` commands through `caller`, which fills the table.
fn fill(caller: &mut impl Caller) -> Vec<ridl_rt::port::Correlation> {
    (0..Loopback::SLOTS)
        .map(|n| {
            caller
                .command(IFACE, ORD, &[u8::try_from(n).expect("small")])
                .expect("a slot is free")
        })
        .collect()
}

#[test]
fn the_seventeenth_in_flight_call_is_busy() {
    assert_eq!(Loopback::SLOTS, 16);
    let rt = runtime();
    let mut caller = rt.caller();
    let mut other = rt.caller();
    let mut handler = rt.handler();
    let mut buf = [0u8; 8];
    let calls = fill(&mut caller);

    assert_eq!(caller.command(IFACE, ORD, &[99]), Err(SendError::Busy));
    assert_eq!(caller.query(IFACE, ORD, &[99]), Err(SendError::Busy));
    assert_eq!(
        other.command(IFACE, ORD, &[99]),
        Err(SendError::Busy),
        "the table is the runtime's, shared by every caller"
    );

    let claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    handler.settle(claim.id, Ok(&[])).expect("settle");
    assert_eq!(caller.ack(calls[0]), Some(Ok(())));
    assert_eq!(
        caller.command(IFACE, ORD, &[99]),
        Err(SendError::Busy),
        "a settled call keeps its slot until it is forgotten"
    );

    caller.forget(calls[0]);
    other
        .command(IFACE, ORD, &[99])
        .expect("the forget freed a slot");
    assert_eq!(caller.command(IFACE, ORD, &[100]), Err(SendError::Busy));
}

#[test]
fn a_refused_send_draws_no_sequence_number() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    let mut buf = [0u8; 8];
    let calls = fill(&mut caller);
    assert_eq!(caller.command(IFACE, ORD, &[99]), Err(SendError::Busy));

    let mut last = 0;
    for _ in &calls {
        let claim = handler
            .next_claim(&mut buf)
            .expect("next_claim")
            .expect("waiting");
        last = claim.envelope.seq;
        handler.settle(claim.id, Ok(&[])).expect("settle");
    }
    caller.forget(calls[0]);
    caller.command(IFACE, ORD, &[99]).expect("a slot is free");
    let claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    assert_eq!(
        claim.envelope.seq,
        last + 1,
        "nothing was sent, so no number was used"
    );
}

#[test]
fn a_reclaimed_slots_old_correlation_answers_none() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    let mut buf = [0u8; 8];

    let old = caller.query(IFACE, ORD, &[1]).expect("send");
    let claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    handler.settle(claim.id, Ok(&[7])).expect("settle");
    caller.forget(old);

    let new = caller.query(IFACE, ORD, &[2]).expect("send");
    let claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    handler.settle(claim.id, Ok(&[8, 8])).expect("settle");
    assert_ne!(new, old, "the slot is reused under a new correlation");

    assert_eq!(caller.reply(old, &mut buf), Ok(None), "the old one is gone");
    assert_eq!(caller.ack(old), None);
    let (count, waker) = counting();
    caller.wake_on(Interest::Outcome(old), &waker);
    assert_eq!(wakes(&count), 1, "no outcome is to come under it");

    caller.forget(old);
    assert_eq!(
        caller.reply(new, &mut buf),
        Ok(Some(Ok(2))),
        "forgetting the old correlation leaves the new call alone"
    );
    assert_eq!(&buf[..2], &[8, 8]);
}

#[test]
fn a_slot_registration_is_stored_while_every_slot_is_taken_and_woken_by_a_forget() {
    let rt = runtime();
    let mut caller = rt.caller();
    let other = rt.caller();
    let mut handler = rt.handler();
    let mut buf = [0u8; 8];
    let calls = fill(&mut caller);

    let (mine, my_waker) = counting();
    let (theirs, their_waker) = counting();
    caller.wake_on(Interest::Slot, &my_waker);
    other.wake_on(Interest::Slot, &their_waker);
    assert_eq!((wakes(&mine), wakes(&theirs)), (0, 0), "no slot is free");

    let claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    handler.settle(claim.id, Ok(&[])).expect("settle");
    assert_eq!(
        (wakes(&mine), wakes(&theirs)),
        (0, 0),
        "a settlement frees no slot"
    );

    caller.forget(calls[0]);
    assert_eq!(
        (wakes(&mine), wakes(&theirs)),
        (1, 1),
        "a reclaim wakes every caller's slot waiter, in no order"
    );
    caller.forget(calls[1]);
    assert_eq!(
        (wakes(&mine), wakes(&theirs)),
        (1, 1),
        "a woken waiter is cleared; forgetting a call in flight reclaims nothing"
    );
}

#[test]
fn the_settlement_of_a_forgotten_call_wakes_the_slot_waiters() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    let mut buf = [0u8; 8];
    let calls = fill(&mut caller);
    let (count, waker) = counting();
    caller.wake_on(Interest::Slot, &waker);

    caller.forget(calls[0]);
    assert_eq!(wakes(&count), 0, "the forgotten call still holds its slot");
    assert_eq!(caller.command(IFACE, ORD, &[99]), Err(SendError::Busy));

    let claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("the provider still sees the forgotten call");
    handler.settle(claim.id, Ok(&[])).expect("settle");
    assert_eq!(wakes(&count), 1, "its settlement reclaims the slot");
    caller.command(IFACE, ORD, &[99]).expect("the slot is free");
}

#[test]
fn a_slot_registration_while_a_slot_is_free_is_woken_at_once() {
    let rt = runtime();
    let mut caller = rt.caller();
    let calls = fill(&mut caller);
    let mut handler = rt.handler();
    let mut buf = [0u8; 8];
    let claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    handler.settle(claim.id, Ok(&[])).expect("settle");
    caller.forget(calls[0]);

    let (count, waker) = counting();
    caller.wake_on(Interest::Slot, &waker);
    assert_eq!(wakes(&count), 1, "one slot is free");
    caller.command(IFACE, ORD, &[99]).expect("send");
    caller.wake_on(Interest::Slot, &waker);
    assert_eq!(wakes(&count), 1, "the table is full again: stored");
}

#[test]
fn a_slot_registration_by_the_same_task_is_a_refresh_and_by_another_task_displaces() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    let mut buf = [0u8; 8];
    let calls = fill(&mut caller);

    let (first, first_waker) = counting();
    caller.wake_on(Interest::Slot, &first_waker);
    caller.wake_on(Interest::Slot, &first_waker.clone());
    assert_eq!(wakes(&first), 0, "a refresh wakes nothing");

    let (second, second_waker) = counting();
    caller.wake_on(Interest::Slot, &second_waker);
    assert_eq!(wakes(&first), 1, "another task's waker displaces the first");

    let claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    handler.settle(claim.id, Ok(&[])).expect("settle");
    caller.forget(calls[0]);
    assert_eq!(
        (wakes(&first), wakes(&second)),
        (1, 1),
        "the reclaim wakes the one stored"
    );
}

#[test]
fn a_dropped_caller_leaves_no_slot_waiter_behind() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    let mut buf = [0u8; 8];
    let calls = fill(&mut caller);

    let other = rt.caller();
    let (count, waker) = counting();
    other.wake_on(Interest::Slot, &waker);
    assert_eq!(Arc::strong_count(&count), 3, "the store holds a clone");
    drop(other);
    assert_eq!(Arc::strong_count(&count), 2, "the drop released it");

    let claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    handler.settle(claim.id, Ok(&[])).expect("settle");
    caller.forget(calls[0]);
    assert_eq!(wakes(&count), 0);
}

#[test]
fn the_aggregate_routes_each_key_to_the_handle_that_observes_it() {
    let mut rt = runtime();
    let mut buf = [0u8; 8];

    let c = rt.command(IFACE, ORD, &[1]).expect("send");
    let (outcome, outcome_waker) = counting();
    rt.wake_on(Interest::Outcome(c), &outcome_waker);
    assert_eq!(wakes(&outcome), 0, "stored, not woken at once");
    let claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    rt.settle(claim.id, Ok(&[])).expect("settle");
    assert_eq!(wakes(&outcome), 1, "woken by the settlement");

    rt.subscribe(IFACE, &[ORD]).expect("subscribe");
    let (event, event_waker) = counting();
    rt.wake_on(Interest::Event(IFACE), &event_waker);
    assert_eq!(wakes(&event), 0, "stored, not woken at once");
    rt.raise(IFACE, ORD, &[1]).expect("raise");
    assert_eq!(wakes(&event), 1, "woken by the raise");

    let (claim, claim_waker) = counting();
    rt.wake_on(Interest::Claim(IFACE), &claim_waker);
    assert_eq!(wakes(&claim), 0, "stored, not woken at once");
    rt.caller().command(IFACE, ORD, &[1]).expect("send");
    assert_eq!(wakes(&claim), 1, "woken by the send");

    let (slot, slot_waker) = counting();
    rt.wake_on(Interest::Slot, &slot_waker);
    assert_eq!(
        wakes(&slot),
        1,
        "the caller wakes it at once: a slot is free"
    );
}

/// A waker that, when woken, reads the clock from another thread and reports
/// whether that read finished within a second.
struct ReadsTheStore {
    reader: Arc<ridl_loopback::ReaderHandle>,
    done: std::sync::Mutex<Option<mpsc::Sender<bool>>>,
}

impl Wake for ReadsTheStore {
    fn wake(self: Arc<Self>) {
        let (tx, rx) = mpsc::channel();
        let reader = Arc::clone(&self.reader);
        std::thread::spawn(move || {
            reader.now();
            let _ = tx.send(());
        });
        // Under the store's lock, the read above cannot finish until this
        // wake returns and the lock is released.
        let finished = rx.recv_timeout(std::time::Duration::from_secs(1)).is_ok();
        if let Some(done) = self.done.lock().expect("not poisoned").take() {
            let _ = done.send(finished);
        }
    }
}

#[test]
fn a_waker_is_woken_after_the_lock_is_released() {
    let rt = runtime();
    let reader = Arc::new(rt.reader());
    let Handles {
        mut caller,
        mut handler,
        ..
    } = rt.split();
    let (tx, rx) = mpsc::channel();
    let waker = Waker::from(Arc::new(ReadsTheStore {
        reader,
        done: std::sync::Mutex::new(Some(tx)),
    }));

    let c = caller.command(IFACE, ORD, &[1]).expect("send");
    caller.wake_on(Interest::Outcome(c), &waker);
    let mut buf = [0u8; 8];
    let claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    handler.settle(claim.id, Ok(&[])).expect("settle");

    assert_eq!(
        rx.recv_timeout(std::time::Duration::from_secs(5)),
        Ok(true),
        "the waker ran with the store's lock released"
    );
}

/// A waker whose wake reports whether the store's lock was released, and the
/// receiver of that report.
fn lock_probe(rt: &Loopback) -> (Waker, mpsc::Receiver<bool>) {
    let (tx, rx) = mpsc::channel();
    let waker = Waker::from(Arc::new(ReadsTheStore {
        reader: Arc::new(rt.reader()),
        done: std::sync::Mutex::new(Some(tx)),
    }));
    (waker, rx)
}

fn assert_released(rx: &mpsc::Receiver<bool>, path: &str) {
    assert_eq!(
        rx.recv_timeout(std::time::Duration::from_secs(5)),
        Ok(true),
        "{path}: the waker ran with the store's lock released"
    );
}

#[test]
fn every_wake_is_run_with_the_lock_released() {
    let rt = runtime();
    let mut sink = rt.sink();
    let mut source = rt.source();
    let mut caller = rt.caller();
    let mut first = rt.handler();
    let mut second = rt.handler();
    source.subscribe(IFACE, &[ORD]).expect("subscribe");
    first.serve(IFACE, &[ORD]).expect("serve");
    second.serve(IFACE, &[OTHER]).expect("serve");
    let mut buf = [0u8; 8];

    let (waker, rx) = lock_probe(&rt);
    source.wake_on(Interest::Event(IFACE), &waker);
    sink.raise(IFACE, ORD, &[1]).expect("raise");
    assert_released(&rx, "a raise");

    let (waker, rx) = lock_probe(&rt);
    source.wake_on(Interest::Event(IFACE), &waker);
    assert_released(&rx, "a registration whose key already holds");

    let (waker, rx) = lock_probe(&rt);
    first.wake_on(Interest::Claim(IFACE), &waker);
    caller.command(IFACE, ORD, &[1]).expect("send");
    assert_released(&rx, "a command");

    first
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    let (waker, rx) = lock_probe(&rt);
    first.wake_on(Interest::Claim(IFACE), &waker);
    let c = caller.query(IFACE, ORD, &[1]).expect("send");
    assert_released(&rx, "a query");

    let (waker, rx) = lock_probe(&rt);
    caller.wake_on(Interest::Outcome(c), &waker);
    caller.forget(c);
    assert_released(&rx, "a forget");

    let (waker, rx) = lock_probe(&rt);
    second.wake_on(Interest::Claim(IFACE), &waker);
    second.serve(IFACE, &[ORD]).expect("serve");
    assert_released(&rx, "a serve");

    // The second handler takes the query, and its drop returns it to the
    // first.
    second
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    let (waker, rx) = lock_probe(&rt);
    first.wake_on(Interest::Claim(IFACE), &waker);
    assert!(first.next_claim(&mut buf).expect("next_claim").is_none());
    drop(second);
    assert_released(&rx, "a handler's drop");

    // Fill the table, then reclaim a slot both ways: a forget of a settled
    // call, and the settlement of a forgotten one.
    let claim = first
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("the returned query");
    first.settle(claim.id, Ok(&[])).expect("settle");
    let mut sent = Vec::new();
    while let Ok(c) = caller.command(IFACE, ORD, &[2]) {
        sent.push(c);
    }
    let claim = first
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("the first command");
    first.settle(claim.id, Ok(&[])).expect("settle");

    let (waker, rx) = lock_probe(&rt);
    caller.wake_on(Interest::Slot, &waker);
    caller.forget(sent[0]);
    assert_released(&rx, "a forget that reclaims a slot");

    caller
        .command(IFACE, ORD, &[3])
        .expect("the reclaimed slot");
    caller.forget(sent[1]);
    let (waker, rx) = lock_probe(&rt);
    caller.wake_on(Interest::Slot, &waker);
    let claim = first
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("the forgotten command");
    first.settle(claim.id, Ok(&[])).expect("settle");
    assert_released(&rx, "a settlement that reclaims a slot");
}
