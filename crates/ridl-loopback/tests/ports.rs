//! The tests of what only this runtime can express.
//!
//! The port contract tests that any runtime can run live in
//! `crates/ridl-rt-conformance`, and `tests/conformance.rs` runs them over
//! this runtime (story E11.20, driftsys/ridl#514). They came from this file.
//! What stays here, and why each test is not in the suite:
//!
//! - `two_runtimes_start_at_the_same_logical_time` — where a clock starts is
//!   a runtime's choice. The suite asks only that the clock move through the
//!   factory's `advance` hook and through nothing else; this runtime's clock
//!   starts at `Timestamp` 0.
//! - `the_clock_refuses_to_run_backwards` — the panic is
//!   `Loopback::advance`'s own precondition, not a port's behaviour.
//! - `every_value_is_unbounded_because_the_runtime_has_no_member_table` —
//!   `Freshness::Unbounded` follows from this runtime holding no catalog
//!   descriptor. A runtime that reads a staleness bound reports `Fresh` or
//!   `Stale`.
//! - `a_sink_counts_its_own_channel_and_not_another_sinks` — two sinks on one
//!   event channel is a misuse this runtime does not police. An event channel
//!   has one provider (ridl §5), so another runtime may refuse the second
//!   sink.
//! - `serve_records_what_it_was_asked_to_present` — `HandlerHandle::served`
//!   is this runtime's own API.
//! - `a_handler_that_served_nothing_is_presented_every_call` — what a handler
//!   that has served nothing is presented is left to a runtime by
//!   `Handler::serve`. This runtime presents every call, because the
//!   generated `dispatch` never calls `serve`.
//! - `a_provisioned_fixed_reads_back_and_an_unprovisioned_one_reports_it` —
//!   provisioning a `fixed` is `Loopback::provision_fixed`, a runtime's own
//!   API with no port, and `Contract::UnknownInteraction` for an
//!   unprovisioned one is this runtime's answer with no member table.
//! - `every_split_handle_carries_the_catalog` — `Loopback::split` is this
//!   runtime's own API. The suite checks the catalog on the aggregate and on
//!   the handles its factory makes.
//! - `the_injected_settle_failure_is_too_large_with_no_capacity` — which
//!   error the injected fault reports is this runtime's choice. The suite asks
//!   only for an error other than `UnknownClaim`.
//! - `a_writer_handle_publishes_on_one_thread_while_a_reader_reads_on_another`
//!   and `a_reader_handle_is_shared_between_threads` — the threading model of
//!   ADR-0021 decision 12. That decision holds a single-threaded runtime to
//!   neither `Send` nor `Sync`, and both tests reach the role handles through
//!   `Loopback::split`.

use ridl_loopback::Loopback;
use ridl_rt::contract::{CatalogHash, CatalogRef, InterfaceNo, Ordinal};
use ridl_rt::error::Contract;
use ridl_rt::port::{
    Attached, Caller, Clock, EventSink, EventSource, FixedReader, Handler, ReadError, SettleError,
    SignalReader, SignalWriter,
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
