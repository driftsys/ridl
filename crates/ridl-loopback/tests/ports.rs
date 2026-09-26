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
//! The tests under "Waking" are here for a different reason: they arrived
//! with `Wakeable` (story E11.16, driftsys/ridl#510), before the suite covers
//! it. The suite gains the `Wakeable` contract tests in the second half of
//! story E11.20; until then these are the contract's only tests over a
//! runtime. Some of them pin this runtime's own answers rather than the
//! contract: an `Event` or `Claim` registration woken at once when its key's
//! change already happened, a `Slot` waiter woken at once because nothing is
//! bounded, a forgotten correlation's waiter, a key a role handle does not
//! carry, one slot for `Event` and `Claim` that a registration under another
//! interface displaces, a dropped handler's waiter, the aggregate's routing,
//! `Clock` on the caller handle, a provider's `Transport::Busy` carried back
//! to the caller, and a waker run only after the store's lock is released.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::task::{Wake, Waker};

use ridl_loopback::{Loopback, ReaderHandle};
use ridl_rt::contract::{CatalogHash, CatalogRef, InterfaceNo, Ordinal};
use ridl_rt::error::{CallError, Contract, Transport};
use ridl_rt::port::{
    Attached, Caller, Clock, EventSink, EventSource, FixedReader, Handler, Interest, ReadError,
    SettleError, SignalReader, SignalWriter, Wakeable,
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
// Waking (story E11.16).
// ---------------------------------------------------------------------------

/// A waker that counts its wakes, built over `std::task::Wake` so the test
/// needs no `unsafe` and no toolchain newer than the crate's.
struct Counter(AtomicUsize);

impl Wake for Counter {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

/// A fresh counter and a waker over it.
fn counter() -> (Arc<Counter>, Waker) {
    let counter = Arc::new(Counter(AtomicUsize::new(0)));
    let waker = Waker::from(Arc::clone(&counter));
    (counter, waker)
}

fn wakes(counter: &Counter) -> usize {
    counter.0.load(Ordering::SeqCst)
}

#[test]
fn a_waiter_on_an_outcome_is_woken_exactly_once_by_its_settlement() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    let mut buf = [0u8; 8];

    let c = caller.command(IFACE, ORD, &[1]).expect("send");
    let (count, waker) = counter();
    caller.wake_on(Interest::Outcome(c), &waker);
    assert_eq!(wakes(&count), 0, "nothing is known about the call yet");

    let claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    assert_eq!(wakes(&count), 0, "a presented claim is not an outcome");

    handler.settle(claim.id, Ok(&[])).expect("settle");
    assert_eq!(wakes(&count), 1, "the settlement wakes the waiter once");
    assert_eq!(caller.ack(c), Some(Ok(())), "and the outcome is readable");

    // A second call's settlement is not a change of the first call's key, and
    // the stored waker was cleared when it was woken.
    caller.command(IFACE, ORD, &[2]).expect("send");
    let second = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    handler.settle(second.id, Ok(&[])).expect("settle");
    assert_eq!(wakes(&count), 1, "a woken waker is not woken again");

    // Had the settlement left its waker stored, a later registration under
    // the same key would displace it and wake it a second time.
    let (later, later_waker) = counter();
    caller.wake_on(Interest::Outcome(c), &later_waker);
    assert_eq!(wakes(&later), 1, "the outcome is known, so at once");
    assert_eq!(wakes(&count), 1, "the settlement cleared the stored waker");
}

#[test]
fn each_settlement_wakes_only_its_own_calls_waiter() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    let mut buf = [0u8; 8];

    let first_call = caller.command(IFACE, ORD, &[1]).expect("send");
    let second_call = caller.command(IFACE, ORD, &[2]).expect("send");
    let (first, first_waker) = counter();
    let (second, second_waker) = counter();
    caller.wake_on(Interest::Outcome(first_call), &first_waker);
    caller.wake_on(Interest::Outcome(second_call), &second_waker);
    assert_eq!(
        (wakes(&first), wakes(&second)),
        (0, 0),
        "two keys, no displacement"
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
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    let mut buf = [0u8; 8];

    let c = caller.query(IFACE, ORD, &[1]).expect("send");
    let claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    handler.settle(claim.id, Ok(&[7])).expect("settle");

    let (count, waker) = counter();
    caller.wake_on(Interest::Outcome(c), &waker);
    assert_eq!(wakes(&count), 1, "the outcome is already known");
    assert_eq!(caller.reply(c, &mut buf), Ok(Some(Ok(1))));
}

#[test]
fn a_second_registration_under_a_key_wakes_the_displaced_waker() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    let mut buf = [0u8; 8];

    let c = caller.command(IFACE, ORD, &[1]).expect("send");
    let (first, first_waker) = counter();
    let (second, second_waker) = counter();
    caller.wake_on(Interest::Outcome(c), &first_waker);
    caller.wake_on(Interest::Outcome(c), &second_waker);
    assert_eq!(wakes(&first), 1, "the displaced waker is woken");
    assert_eq!(wakes(&second), 0, "the new one waits");

    let claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    handler.settle(claim.id, Ok(&[])).expect("settle");
    assert_eq!(wakes(&first), 1, "the displaced waker is not stored");
    assert_eq!(wakes(&second), 1, "the settlement wakes the new one");
}

#[test]
fn registering_the_same_task_again_wakes_nothing() {
    // A task registers on every poll. If its own earlier registration counted
    // as displaced and was woken, every poll would schedule another poll.
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    let mut buf = [0u8; 8];

    let c = caller.command(IFACE, ORD, &[1]).expect("send");
    let (count, waker) = counter();
    caller.wake_on(Interest::Outcome(c), &waker);
    caller.wake_on(Interest::Outcome(c), &waker.clone());
    assert_eq!(
        wakes(&count),
        0,
        "a waker that will wake the same task is kept"
    );

    let claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    handler.settle(claim.id, Ok(&[])).expect("settle");
    assert_eq!(wakes(&count), 1, "and the settlement still wakes it");
}

#[test]
fn a_forgotten_calls_waiter_is_not_woken_by_its_settlement() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    let mut buf = [0u8; 8];

    let c = caller.command(IFACE, ORD, &[1]).expect("send");
    let (count, waker) = counter();
    caller.wake_on(Interest::Outcome(c), &waker);
    assert_eq!(Arc::strong_count(&count), 3, "the store holds a clone");
    caller.forget(c);
    assert_eq!(
        Arc::strong_count(&count),
        2,
        "forgetting a call in flight drops its waiter"
    );
    caller.wake_on(Interest::Outcome(c), &waker);
    assert_eq!(
        Arc::strong_count(&count),
        2,
        "a forgotten call in flight takes no new waiter"
    );

    let claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("the provider still sees a forgotten call");
    handler.settle(claim.id, Ok(&[])).expect("settle");
    assert_eq!(wakes(&count), 0, "nothing can read the outcome");

    caller.wake_on(Interest::Outcome(c), &waker);
    assert_eq!(
        wakes(&count),
        0,
        "a correlation the store no longer holds is not registered"
    );
}

#[test]
fn a_raise_wakes_a_subscribed_source_and_not_an_unsubscribed_one() {
    let mut rt = runtime();
    let mut subscribed = rt.source();
    let unsubscribed = rt.source();
    subscribed.subscribe(IFACE, &[ORD]).expect("subscribe");

    let (woken, woken_waker) = counter();
    let (idle, idle_waker) = counter();
    subscribed.wake_on(Interest::Event(IFACE), &woken_waker);
    unsubscribed.wake_on(Interest::Event(IFACE), &idle_waker);

    rt.raise(IFACE, ORD, &[1]).expect("raise");
    assert_eq!(wakes(&woken), 1, "the subscribed source is woken");
    assert_eq!(wakes(&idle), 0, "nothing was queued for the other one");
    rt.raise(IFACE, ORD, &[2]).expect("raise");
    assert_eq!(wakes(&woken), 1, "the woken waker was cleared");

    let mut out = [0u8; 8];
    assert!(
        subscribed.next(&mut out).expect("next").is_some(),
        "the woken source finds the occurrence"
    );
}

#[test]
fn an_occurrence_already_queued_wakes_its_registration_at_once() {
    let mut rt = runtime();
    let mut source = rt.source();
    source.subscribe(IFACE, &[ORD]).expect("subscribe");
    rt.raise(IFACE, ORD, &[1]).expect("raise");

    let (other, other_waker) = counter();
    source.wake_on(Interest::Event(InterfaceNo(2)), &other_waker);
    assert_eq!(wakes(&other), 0, "nothing is queued for interface 2");
    let (count, waker) = counter();
    source.wake_on(Interest::Event(IFACE), &waker);
    assert_eq!(wakes(&count), 1, "an occurrence of interface 1 is waiting");
}

#[test]
fn a_send_wakes_the_handler_that_serves_the_member() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    handler.serve(IFACE, &[ORD]).expect("serve");

    let (count, waker) = counter();
    handler.wake_on(Interest::Claim(IFACE), &waker);
    assert_eq!(wakes(&count), 0, "no call is waiting");

    caller.command(IFACE, ORD, &[1]).expect("send");
    assert_eq!(wakes(&count), 1, "the send wakes the serving handler");

    let mut buf = [0u8; 8];
    assert!(
        handler.next_claim(&mut buf).expect("next_claim").is_some(),
        "the woken handler finds the claim"
    );
}

#[test]
fn a_call_already_waiting_wakes_the_handlers_registration_at_once() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    handler.serve(IFACE, &[ORD]).expect("serve");
    caller.command(IFACE, OTHER, &[1]).expect("send");

    let (count, waker) = counter();
    handler.wake_on(Interest::Claim(IFACE), &waker);
    assert_eq!(
        wakes(&count),
        0,
        "the waiting call is for a member this handler does not serve"
    );

    caller.command(IFACE, ORD, &[2]).expect("send");
    assert_eq!(wakes(&count), 1);
    handler.wake_on(Interest::Claim(IFACE), &waker);
    assert_eq!(wakes(&count), 2, "a call this handler serves is waiting");

    // A handler that has served nothing is presented every call, so any call
    // on the interface it registers for wakes it at once, and a call on
    // another interface does not.
    let unfiltered = rt.handler();
    let (other, other_waker) = counter();
    unfiltered.wake_on(Interest::Claim(InterfaceNo(2)), &other_waker);
    assert_eq!(wakes(&other), 0, "no call on interface 2 is waiting");
    let (open, open_waker) = counter();
    unfiltered.wake_on(Interest::Claim(IFACE), &open_waker);
    assert_eq!(wakes(&open), 1, "calls on interface 1 are waiting");
}

#[test]
fn a_raise_or_a_send_on_another_interface_leaves_the_waiter_asleep() {
    let mut rt = runtime();
    let mut source = rt.source();
    let mut caller = rt.caller();
    let handler = rt.handler();
    let two = InterfaceNo(2);
    source.subscribe(IFACE, &[ORD]).expect("subscribe");
    source.subscribe(two, &[ORD]).expect("subscribe");

    let (event, event_waker) = counter();
    let (claim, claim_waker) = counter();
    source.wake_on(Interest::Event(two), &event_waker);
    handler.wake_on(Interest::Claim(two), &claim_waker);

    rt.raise(IFACE, ORD, &[1]).expect("raise");
    caller.command(IFACE, ORD, &[1]).expect("send");
    assert_eq!(
        (wakes(&event), wakes(&claim)),
        (0, 0),
        "interface 1 changed"
    );

    rt.raise(two, ORD, &[1]).expect("raise");
    caller.command(two, ORD, &[1]).expect("send");
    assert_eq!(
        (wakes(&event), wakes(&claim)),
        (1, 1),
        "interface 2 changed"
    );
}

#[test]
fn a_second_registration_under_an_event_or_claim_key_wakes_the_displaced_waker() {
    let mut rt = runtime();
    let mut source = rt.source();
    let mut caller = rt.caller();
    let handler = rt.handler();
    source.subscribe(IFACE, &[ORD]).expect("subscribe");

    let (first_event, first_event_waker) = counter();
    let (second_event, second_event_waker) = counter();
    source.wake_on(Interest::Event(IFACE), &first_event_waker);
    source.wake_on(Interest::Event(IFACE), &second_event_waker);
    assert_eq!(
        wakes(&first_event),
        1,
        "the displaced `Event` waker is woken"
    );
    rt.raise(IFACE, ORD, &[1]).expect("raise");
    assert_eq!((wakes(&first_event), wakes(&second_event)), (1, 1));

    let (first_claim, first_claim_waker) = counter();
    let (second_claim, second_claim_waker) = counter();
    handler.wake_on(Interest::Claim(IFACE), &first_claim_waker);
    handler.wake_on(Interest::Claim(IFACE), &second_claim_waker);
    assert_eq!(
        wakes(&first_claim),
        1,
        "the displaced `Claim` waker is woken"
    );
    caller.command(IFACE, ORD, &[1]).expect("send");
    assert_eq!((wakes(&first_claim), wakes(&second_claim)), (1, 1));
}

#[test]
fn the_same_task_under_another_interface_is_woken_for_the_first() {
    // The slot holds one key. A task that moves its registration from
    // interface 1 to interface 2 is woken, so it cannot miss a change of
    // interface 1 it registered for first.
    let mut rt = runtime();
    let mut source = rt.source();
    let two = InterfaceNo(2);
    source.subscribe(two, &[ORD]).expect("subscribe");
    let (count, waker) = counter();
    source.wake_on(Interest::Event(IFACE), &waker);
    source.wake_on(Interest::Event(two), &waker);
    assert_eq!(wakes(&count), 1);
    rt.raise(two, ORD, &[1]).expect("raise");
    assert_eq!(
        wakes(&count),
        2,
        "the registration under interface 2 is kept"
    );
}

#[test]
fn the_same_task_registering_an_event_or_claim_key_again_wakes_nothing() {
    let mut rt = runtime();
    let mut source = rt.source();
    let mut caller = rt.caller();
    let handler = rt.handler();
    source.subscribe(IFACE, &[ORD]).expect("subscribe");

    let (event, event_waker) = counter();
    source.wake_on(Interest::Event(IFACE), &event_waker);
    source.wake_on(Interest::Event(IFACE), &event_waker.clone());
    assert_eq!(
        wakes(&event),
        0,
        "`Event`: the same task is kept, not woken"
    );
    rt.raise(IFACE, ORD, &[1]).expect("raise");
    assert_eq!(wakes(&event), 1);

    let (claim, claim_waker) = counter();
    handler.wake_on(Interest::Claim(IFACE), &claim_waker);
    handler.wake_on(Interest::Claim(IFACE), &claim_waker.clone());
    assert_eq!(
        wakes(&claim),
        0,
        "`Claim`: the same task is kept, not woken"
    );
    caller.command(IFACE, ORD, &[1]).expect("send");
    assert_eq!(wakes(&claim), 1);
}

#[test]
fn a_providers_busy_settlement_reaches_the_caller() {
    // The loopback never refuses a call itself; a provider that settles with
    // `Transport::Busy` has it carried back unchanged.
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
fn a_dropped_handler_leaves_no_waiter_behind() {
    let rt = runtime();
    let mut caller = rt.caller();
    let handler = rt.handler();
    let (count, waker) = counter();
    handler.wake_on(Interest::Claim(IFACE), &waker);
    assert_eq!(Arc::strong_count(&count), 3, "the store holds a clone");
    drop(handler);
    assert_eq!(Arc::strong_count(&count), 2, "the drop released it");
    caller.command(IFACE, ORD, &[1]).expect("send");
    assert_eq!(wakes(&count), 0);
}

#[test]
fn a_slot_waiter_is_woken_at_once_because_nothing_is_bounded() {
    let rt = runtime();
    let caller = rt.caller();
    let (count, waker) = counter();
    caller.wake_on(Interest::Slot, &waker);
    assert_eq!(wakes(&count), 1, "the loopback always has a slot free");
}

#[test]
fn a_key_the_handles_role_does_not_carry_is_never_stored() {
    let mut rt = runtime();
    let mut caller = rt.caller();
    let mut source = rt.source();
    let handler = rt.handler();
    source.subscribe(IFACE, &[ORD]).expect("subscribe");

    let (count, waker) = counter();
    caller.wake_on(Interest::Event(IFACE), &waker);
    handler.wake_on(Interest::Event(IFACE), &waker);
    source.wake_on(Interest::Claim(IFACE), &waker);
    rt.raise(IFACE, ORD, &[1]).expect("raise");
    caller.command(IFACE, ORD, &[1]).expect("send");
    assert_eq!(wakes(&count), 0);
}

#[test]
fn the_aggregate_routes_each_key_to_the_handle_that_carries_it() {
    let mut rt = runtime();
    let mut sink = rt.sink();
    let mut caller = rt.caller();
    rt.subscribe(IFACE, &[ORD]).expect("subscribe");
    rt.serve(IFACE, &[ORD]).expect("serve");

    let (event, event_waker) = counter();
    rt.wake_on(Interest::Event(IFACE), &event_waker);
    sink.raise(IFACE, ORD, &[1]).expect("raise");
    assert_eq!(wakes(&event), 1, "`Event` reaches the aggregate's source");

    let (claim, claim_waker) = counter();
    rt.wake_on(Interest::Claim(IFACE), &claim_waker);
    caller.command(IFACE, ORD, &[1]).expect("send");
    assert_eq!(wakes(&claim), 1, "`Claim` reaches the aggregate's handler");

    let c = rt.command(IFACE, ORD, &[2]).expect("send");
    let (outcome, outcome_waker) = counter();
    rt.wake_on(Interest::Outcome(c), &outcome_waker);
    let mut buf = [0u8; 8];
    while let Some(claim) = rt.next_claim(&mut buf).expect("next_claim") {
        rt.settle(claim.id, Ok(&[])).expect("settle");
    }
    assert_eq!(
        wakes(&outcome),
        1,
        "`Outcome` reaches the aggregate's caller"
    );

    let (slot, slot_waker) = counter();
    rt.wake_on(Interest::Slot, &slot_waker);
    assert_eq!(wakes(&slot), 1, "`Slot` reaches the aggregate's caller");
}

#[test]
fn a_waker_is_woken_after_the_store_lock_is_released() {
    // A waker that reads the runtime from another thread inside `wake`. If
    // the waking thread still held the store's lock, that read could not
    // finish, and `wake` records the timeout instead of hanging the test.
    struct Reads {
        reader: Arc<ReaderHandle>,
        blocked: AtomicBool,
        woken: AtomicUsize,
    }
    impl Wake for Reads {
        fn wake(self: Arc<Self>) {
            self.wake_by_ref();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            let (done, finished) = mpsc::channel();
            let reader = Arc::clone(&self.reader);
            std::thread::spawn(move || {
                let _ = done.send(reader.now());
            });
            if finished
                .recv_timeout(std::time::Duration::from_secs(2))
                .is_err()
            {
                self.blocked.store(true, Ordering::SeqCst);
            }
            self.woken.fetch_add(1, Ordering::SeqCst);
        }
    }

    let mut rt = runtime();
    let reads = Arc::new(Reads {
        reader: Arc::new(rt.reader()),
        blocked: AtomicBool::new(false),
        woken: AtomicUsize::new(0),
    });
    let waker = Waker::from(Arc::clone(&reads));

    let mut caller = rt.caller();
    let mut handler = rt.handler();
    let mut source = rt.source();
    source.subscribe(IFACE, &[ORD]).expect("subscribe");

    let c = caller.command(IFACE, ORD, &[1]).expect("send");
    caller.wake_on(Interest::Outcome(c), &waker);
    handler.wake_on(Interest::Claim(InterfaceNo(2)), &waker);
    source.wake_on(Interest::Event(IFACE), &waker);

    let mut buf = [0u8; 8];
    let claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    handler.settle(claim.id, Ok(&[])).expect("settle");
    caller.command(InterfaceNo(2), ORD, &[1]).expect("send");
    rt.raise(IFACE, ORD, &[1]).expect("raise");

    assert_eq!(reads.woken.load(Ordering::SeqCst), 3, "settle, send, raise");
    assert!(
        !reads.blocked.load(Ordering::SeqCst),
        "a waker ran while the store's lock was held"
    );
}

#[test]
fn a_caller_handle_reads_the_clock() {
    let mut rt = runtime();
    let caller = rt.caller();
    rt.advance(Duration(250));
    assert_eq!(caller.now(), Timestamp(250));
    assert_eq!(caller.now(), rt.now());
}
