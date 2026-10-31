//! The tests of what only this runtime can express.
//!
//! The port contract tests that any runtime can run live in
//! `crates/ridl-rt-conformance`, and `tests/conformance.rs` runs them over
//! this runtime (driftsys/ridl#514). They came from this file.
//! What stays here, and why each test is not in the suite. Each reason is an
//! item of the list "What the suite leaves out" in that crate's
//! documentation, or a choice of this runtime alone:
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
//! - `a_provisioned_fixed_reads_back_and_an_unprovisioned_one_reports_it`
//!   and `a_fixed_longer_than_its_limit_is_refused_and_reads_as_unprovisioned`
//!   — `FixedReader`. Provisioning a `fixed` is `Loopback::provision_fixed`,
//!   a runtime's own API with no port, and `Contract::UnknownInteraction` for
//!   an unprovisioned one is this runtime's answer with no member table.
//! - The tests under "Member limits" — a runtime's own API beyond the
//!   factory: `Loopback::set_limit`, how this runtime is told the limit a
//!   `[limits.interaction]` entry gives a member. The suite pins the default
//!   ceiling of each kind, which a runtime with no descriptor applies.
//! - `every_split_handle_carries_the_catalog` — a runtime's own API beyond
//!   the factory: `Loopback::split`. The suite checks the catalog on the
//!   aggregate and on the handles its factory makes.
//! - `an_attached_aggregate_carries_the_catalog`,
//!   `a_value_committed_through_one_aggregate_is_read_through_an_attached_one`,
//!   `an_attached_aggregate_advances_the_one_clock`,
//!   `an_attached_aggregate_starts_with_none_of_the_originals_handle_state`,
//!   `an_attached_aggregate_has_its_own_subscriptions_and_queue`,
//!   `a_call_sent_through_one_aggregate_is_served_through_an_attached_one`,
//!   `dropping_an_attached_aggregate_leaves_the_originals_calls_and_subscriptions`
//!   and `an_attached_aggregate_keeps_the_store_after_the_original_is_dropped`
//!   — a runtime's own API beyond the factory: `Loopback::attach`
//!   (driftsys/ridl#488).
//! - `the_injected_settle_failure_is_too_large_with_no_capacity` — which
//!   error the injected fault reports is left to a runtime. The suite asks
//!   only for an error other than `UnknownClaim`.
//! - `a_writer_handle_publishes_on_one_thread_while_a_reader_reads_on_another`
//!   and `a_reader_handle_is_shared_between_threads` — the threading model of
//!   ADR-0021 decision 12. That decision holds a single-threaded runtime to
//!   neither `Send` nor `Sync`, and both tests reach the role handles through
//!   `Loopback::split`.
//!
//! The tests under "Waking" and "The bounded call table" pin this runtime's
//! `Wakeable` (driftsys/ridl#510) and its move onto
//! `ridl_rt::correlate` (driftsys/ridl#512). The suite covers
//! the contract of both: its `wakeable` module, and the slot count its
//! factory states (driftsys/ridl#514). The tests that stay
//! here pin what the contract leaves to a runtime, and each falls under one
//! of the reasons below. Not every one of these reasons is an item of the
//! list in `ridl-rt-conformance`'s documentation: that list names what the
//! suite leaves out, and some of the choices below are this runtime's alone.
//!
//! - **A registration whose key already holds is woken at once.** The
//!   contract has the task read the port after it registers, so a runtime
//!   may instead store the waker and let the read find the change:
//!   `a_waiter_registered_after_the_settlement_is_woken_at_once`,
//!   `a_waiting_occurrence_of_another_interface_wakes_an_event_registration_at_once`,
//!   `a_waiting_call_on_another_interface_wakes_a_claim_registration_at_once`,
//!   `a_waiter_woken_at_once_is_not_stored`,
//!   `a_registration_on_a_forgotten_call_is_woken_at_once`,
//!   `a_registration_on_a_correlation_whose_slot_was_reused_is_woken_at_once`,
//!   `a_registration_whose_key_already_holds_is_woken_at_once`,
//!   `a_key_no_role_of_the_handle_observes_is_woken_at_once` and
//!   `a_slot_registration_while_a_slot_is_free_is_woken_at_once`.
//! - **A change wakes no waiter but its key's.** The contract allows a
//!   spurious wake, so the suite asserts that a change leaves a waker unwoken
//!   only where the contract rules the wake out — a refresh, or a waker
//!   already woken or displaced and so cleared — and this runtime's narrower
//!   wakes are pinned here:
//!   `each_settlement_wakes_only_its_own_calls_waiter`,
//!   `a_raise_wakes_a_subscribed_source_and_not_an_unsubscribed_one`,
//!   `a_send_wakes_the_handler_that_serves_the_member` (a handler serving
//!   another member of the interface is not woken),
//!   `a_claim_registration_is_not_woken_by_a_call_the_handler_does_not_serve`
//!   and `a_serve_that_admits_nothing_does_not_wake_the_handler`.
//! - **Every serving handler is woken.**
//!   `every_subscribed_source_and_every_serving_handler_is_woken`: the
//!   contract says each handle waiting for the same events is woken, and the
//!   suite asserts that for sources; a call is presented to one handler, and
//!   which serving handlers a runtime wakes for it is the runtime's.
//! - **A serve after the registration, or a call that waits for a serve.**
//!   The suite serves before it registers or sends; whether a call to a
//!   member no handler serves waits for a later serve is this runtime's
//!   queueing: `a_serve_that_admits_a_waiting_call_wakes_the_handler` and
//!   `a_serve_with_no_call_waiting_keeps_the_handlers_claim_waker`.
//! - **A forget wakes its own call's waiter.** The task that forgets a call
//!   is the task that waited on it, and the port states no rule for that
//!   waker; this runtime wakes it (driftsys/ridl#551, and decision 2 in the
//!   description of driftsys/ridl#553): `a_forgotten_call_wakes_its_waiter` and
//!   `forgetting_a_claimed_call_wakes_its_outcome_waiter`.
//! - **The drop of a handle.** The suite drops no handle. What a dropped
//!   handler's claims and a dropped caller's calls become, and that a drop
//!   leaves no waiter behind and no lock held, is this runtime's `Drop`:
//!   `a_drop_returns_only_the_dropped_handlers_claims_and_wakes_every_serving_handler`,
//!   `a_dropped_handler_returns_its_claims_to_the_waiting_calls`,
//!   `a_dropped_handler_leaves_no_waiter_behind`,
//!   `a_dropped_source_leaves_no_waiter_behind`,
//!   `a_returned_claim_keeps_its_place_by_send_order`,
//!   `a_returned_claim_keeps_its_send_order_across_a_reused_slot`,
//!   `a_dropped_caller_leaves_no_slot_waiter_behind`,
//!   `a_dropped_caller_forgets_its_calls_and_their_slots_come_back`,
//!   `a_dropped_caller_forgets_only_its_own_calls`,
//!   `a_dropped_callers_call_in_flight_wakes_its_outcome_waiter`,
//!   `a_dropped_callers_own_slot_waiter_is_not_woken_by_its_drop`,
//!   `a_caller_whose_stored_waker_owns_another_handle_drops_without_deadlock`,
//!   `a_source_whose_stored_waker_owns_another_handle_drops_without_deadlock`,
//!   `a_handler_whose_stored_waker_owns_another_handle_drops_without_deadlock`
//!   and
//!   `a_stream_waker_whose_task_holds_a_handle_drops_without_deadlock`.
//! - **The withdrawal of a forgotten call no handler holds.** The suite
//!   accepts either result for such a call, withdrawn or held until settled,
//!   because a transport that has already sent a request cannot recall it;
//!   this runtime withdraws it:
//!   `forgotten_calls_to_an_unserved_member_leave_room_for_another_send`,
//!   `a_withdrawn_call_is_never_presented_to_a_handler_that_serves_the_member_later`,
//!   `a_withdrawal_from_the_middle_of_the_queue_leaves_the_calls_around_it_in_order`,
//!   `a_stale_correlation_does_not_withdraw_the_call_now_in_its_slot`,
//!   `a_call_forgotten_while_claimed_is_withdrawn_when_its_handler_is_dropped`,
//!   `a_forget_withdrawal_wakes_no_claim_waiter_of_a_handler_serving_another_member`,
//!   `a_dropped_handler_withdraws_only_its_forgotten_claim_and_returns_the_others`,
//!   `a_dropped_callers_claimed_call_is_withdrawn_when_its_handler_is_dropped`,
//!   `a_withdrawal_at_a_handlers_drop_wakes_no_claim_waiter` and
//!   `a_dropped_callers_unclaimed_calls_are_withdrawn`.
//! - **This runtime's own numbers and choices.**
//!   `the_seventeenth_in_flight_call_is_busy` names this runtime's sixteen
//!   slots; `a_refused_send_draws_no_sequence_number` pins a choice the
//!   contract leaves open; `a_claimed_then_forgotten_call_holds_its_slot_until_its_settlement`
//!   pins when the slot comes back, which the suite leaves to a runtime as
//!   long as it is back once the call is settled;
//!   `a_providers_busy_settlement_reaches_the_caller` stays because this
//!   runtime originates no `Transport::Busy` of its own;
//!   `a_caller_handle_reads_the_clock` pins `Clock` on `CallerHandle`, which
//!   the factory does not ask of its second caller;
//!   `the_aggregate_routes_each_key_to_the_handle_that_observes_it` pins
//!   `Loopback`'s own routing; and `every_wake_is_run_with_the_lock_released`
//!   takes the wake paths the suite does not: a registration woken at once,
//!   a serve, a handler's drop and a withdrawal, and
//!   `every_wake_at_once_and_every_displacement_is_run_with_the_lock_released`
//!   takes the registrations woken at once of each kind and the displacement
//!   of a stored waker of each kind.
//!
//! The stream tests that stay here, each under one of the reasons above or
//! under a choice of this runtime alone:
//!
//! - **The drop of a handle.** What a dropped handler's claims become when
//!   their streams moved, and what its drop does to the streams of a claim it
//!   settled:
//!   `a_dropped_handler_interrupts_a_taken_call_whose_streams_moved`,
//!   `a_dropped_handler_interrupts_a_call_on_which_it_pushed_pulled_an_end_or_aborted`,
//!   `a_dropped_handler_that_moved_no_stream_returns_the_call`,
//!   `a_dropped_handlers_interruption_releases_a_stream_parameter_not_yet_named`,
//!   `a_dropped_handler_stops_the_parameter_streams_of_a_command_it_acknowledged`,
//!   `a_dropped_handler_releases_a_stream_parameter_of_an_acknowledged_command_not_yet_named`,
//!   `a_dropped_handler_completes_a_forget_deferred_on_an_acknowledged_command`,
//!   `a_dropped_handlers_claim_stream_waiter_is_woken`,
//!   `a_claim_only_offered_has_no_streams_and_another_handler_reads_them_whole`
//!   (its second half: the next holder after the drop) and
//!   `a_handler_whose_stored_claim_stream_waker_owns_another_handle_drops_without_deadlock`.
//! - **When the slot comes back.** The suite asks that a forgotten call's slot
//!   is back once the provider is done with its streams; when this runtime
//!   defers the forget, what it answers meanwhile, and which paths complete
//!   it are its own:
//!   `a_deferred_forget_answers_none_from_ack_and_reply_while_the_slot_is_held`,
//!   `a_deferred_forget_frees_the_slot_once_the_provider_is_done_with_the_parameter_stream`,
//!   `a_second_forget_of_a_deferred_call_leaves_its_slot_held`,
//!   `a_forgotten_unacknowledged_commands_ended_stream_is_read_after_the_acknowledgment`,
//!   `a_parameter_stream_the_provider_refused_does_not_hold_the_slot_after_the_forget`,
//!   `a_query_settled_with_an_unread_parameter_stream_frees_its_slot_at_the_forget`,
//!   `a_stream_parameter_first_named_after_a_querys_settlement_starts_released`,
//!   `a_commands_refusal_releases_its_parameter_streams_and_the_forget_frees_its_slot`
//!   and `a_forget_before_the_claim_withdraws_the_call_with_its_ended_stream`
//!   (the withdrawal of a call no handler holds).
//! - **A registration whose key already holds is woken at once**, and a
//!   forget wakes its own call's waiters, on the stream keys:
//!   `a_stream_registration_on_a_forgotten_call_is_woken_at_once`,
//!   `a_claim_stream_registration_on_a_finished_stream_is_woken_at_once`,
//!   `a_claim_stream_registration_refreshes_the_same_task_and_displaces_another`,
//!   `a_querys_settlement_wakes_the_providers_waiter_on_a_parameter_stream`,
//!   `a_forget_wakes_the_stream_waiters_of_a_call_with_no_stream_parameter`,
//!   `a_deferred_forget_wakes_the_outcome_waiter_and_a_later_registration_is_woken_at_once`
//!   and
//!   `a_producers_end_and_abort_wake_the_receiver_and_a_receivers_refusal_wakes_the_producer`
//!   (the suite pins the wake by a chunk, an end of the reply stream and a
//!   freed window; the other wakes are pinned here).
//! - **This runtime's own numbers and choices.**
//!   `the_window_counts_only_the_non_empty_chunks` names this runtime's window
//!   of four; `a_settle_ok_on_a_streamed_reply_ends_the_stream_and_reports_the_bytes`
//!   pins what the contract leaves to a runtime for non-empty bytes;
//!   `abort_claim_after_the_callers_refusal_is_ended_and_settles_with_the_providers_error`
//!   pins which outcome this runtime records after the caller's refusal;
//!   `a_settlement_refused_through_end_claim_answers_too_large` pins the
//!   injected fault's report, which the suite leaves to a runtime; and
//!   `a_providers_push_and_end_on_the_reply_stream_after_a_deferred_forget_are_interrupted`,
//!   `end_claim_after_the_callers_refusal_settles_the_claim_with_the_refusal`
//!   and `the_providers_push_after_its_own_abort_of_the_reply_stream_is_ended`
//!   pin a contract rule on a path of this store the suite's form of the same
//!   rule does not take.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::task::{Wake, Waker};

use ridl_loopback::{Handles, Loopback};
use ridl_rt::contract::{CatalogHash, CatalogRef, InterfaceNo, Ordinal};
use ridl_rt::error::{CallError, Contract, Transport};
use ridl_rt::port::{
    Attached, Caller, ClaimId, Clock, EventSink, EventSource, FixedReader, Handler, Interest,
    RaiseError, ReadError, SendError, SettleError, SignalReader, SignalWriter, Wakeable,
    WriteError, REPLY_STREAM,
};
use ridl_rt::port::{Pull, PushError, StreamCaller, StreamHandler, StreamNo};
use ridl_rt::sample::{Duration, Freshness, Provenance, Timestamp};

const IFACE: InterfaceNo = InterfaceNo(1);
const ORD: Ordinal = Ordinal(1);
const OTHER: Ordinal = Ordinal(2);

/// The catalog every test attaches to. The hash is all zeros, which is
/// enough here: the runtime carries the catalog without examining it. The
/// generated `Bind::new` and `serve` compare the catalogs (ADR-0023
/// decision 8), but these tests do not run generated code.
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

    first.raise(IFACE, ORD, &[1], None).expect("raise");
    second.raise(IFACE, ORD, &[2], None).expect("raise");
    first.raise(IFACE, ORD, &[3], None).expect("raise");

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
    caller
        .command(InterfaceNo(2), OTHER, &[1], None)
        .expect("send");

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
    rt.command(IFACE, ORD, &[1], None).expect("send");
    let mut buf = [0u8; 8];
    let claim = rt.next_claim(&mut buf).expect("read").expect("waiting");

    rt.fail_next_settle();
    assert_eq!(
        rt.settle(claim.id, Ok(&[])),
        Err(SettleError::TooLarge { cap: 0 }),
        "the one fault this runtime injects"
    );
}

/// A size refusal is made before the armed fault is consumed, so the fault
/// still fails the next settlement that is within the limit.
#[test]
fn a_size_refusal_does_not_use_up_the_armed_settle_failure() {
    let mut rt = runtime();
    rt.serve(IFACE, &[ORD]).expect("serve");
    rt.set_limit(IFACE, ORD, 2);
    rt.query(IFACE, ORD, &[1], None).expect("send");
    let mut buf = [0u8; 8];
    let claim = rt.next_claim(&mut buf).expect("read").expect("waiting");

    rt.fail_next_settle();
    assert_eq!(
        rt.settle(claim.id, Ok(&[1, 2, 3])),
        Err(SettleError::TooLarge { cap: 2 }),
        "the size refusal names the member's limit"
    );
    assert_eq!(
        rt.settle(claim.id, Ok(&[1, 2])),
        Err(SettleError::TooLarge { cap: 0 }),
        "the armed fault is still there for the next settlement"
    );
    rt.settle(claim.id, Ok(&[1, 2]))
        .expect("the fault was consumed by the settlement it failed");
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

    rt.provision_fixed(IFACE, ORD, &[1, 2])
        .expect("the loopback stores the fixed");
    assert_eq!(rt.read_fixed(IFACE, ORD, &mut out).expect("read"), 2);
    assert_eq!(&out[..2], &[1, 2]);

    let mut short = [0u8; 1];
    assert_eq!(
        rt.read_fixed(IFACE, ORD, &mut short),
        Err(ReadError::Short { needed: 2 })
    );
}

#[test]
fn a_fixed_longer_than_its_limit_is_refused_and_reads_as_unprovisioned() {
    let mut rt = runtime();
    assert_eq!(
        rt.provision_fixed(IFACE, ORD, &[0u8; 65_281]),
        Err(WriteError::TooLarge { cap: 65_280 }),
        "one byte above the default ceiling of a fixed is refused"
    );
    let mut out = [0u8; 8];
    assert_eq!(
        rt.read_fixed(IFACE, ORD, &mut out),
        Err(ReadError::Contract(Contract::UnknownInteraction)),
        "the refused value was not stored, so the read is answered as for an \
         unprovisioned fixed"
    );

    let mut large = vec![0u8; 65_280];
    rt.provision_fixed(IFACE, ORD, &large)
        .expect("a value of exactly the default ceiling is stored");
    assert_eq!(rt.read_fixed(IFACE, ORD, &mut large).expect("read"), 65_280);

    rt.set_limit(IFACE, ORD, 2);
    assert_eq!(
        rt.provision_fixed(IFACE, ORD, &[1, 2, 3]),
        Err(WriteError::TooLarge { cap: 2 }),
        "a limit given to the loopback lowers the cap of a fixed"
    );
    assert_eq!(
        rt.read_fixed(IFACE, ORD, &mut large).expect("read"),
        65_280,
        "a refused provision leaves the stored value as it was"
    );
    rt.provision_fixed(IFACE, ORD, &[1, 2])
        .expect("a value within the given limit is stored");
    assert_eq!(rt.read_fixed(IFACE, ORD, &mut out).expect("read"), 2);
}

// ---------------------------------------------------------------------------
// Member limits: `Loopback::set_limit` in place of the kind's default
// ceiling, applied by every sending port and by `provision_fixed`.
// ---------------------------------------------------------------------------

#[test]
fn a_limit_given_to_the_loopback_lowers_and_raises_a_signals_cap() {
    let mut rt = runtime();
    rt.set_limit(IFACE, ORD, 2);
    assert_eq!(
        rt.set(IFACE, ORD, &[1, 2, 3]),
        Err(WriteError::TooLarge { cap: 2 })
    );
    rt.set(IFACE, ORD, &[1, 2]).expect("within the given limit");
    assert_eq!(
        rt.set(IFACE, OTHER, &[0u8; 3_841]),
        Err(WriteError::TooLarge { cap: 3_840 }),
        "another member keeps the default ceiling"
    );

    rt.set_limit(IFACE, ORD, 16_128);
    rt.set(IFACE, ORD, &[0u8; 16_128])
        .expect("a limit above the default ceiling raises the cap");
    assert_eq!(
        rt.set(IFACE, ORD, &[0u8; 16_129]),
        Err(WriteError::TooLarge { cap: 16_128 })
    );
}

#[test]
fn a_limit_given_to_the_loopback_lowers_and_raises_an_events_cap() {
    let mut rt = runtime();
    rt.subscribe(IFACE, &[ORD]).expect("subscribe");
    rt.set_limit(IFACE, ORD, 2);
    assert_eq!(
        rt.raise(IFACE, ORD, &[1, 2, 3], None),
        Err(RaiseError::TooLarge { cap: 2 })
    );
    rt.raise(IFACE, ORD, &[1, 2], None)
        .expect("within the given limit");

    rt.set_limit(IFACE, ORD, 16_128);
    rt.raise(IFACE, ORD, &[0u8; 16_128], None)
        .expect("a limit above the default ceiling raises the cap");
    assert_eq!(
        rt.raise(IFACE, ORD, &[0u8; 16_129], None),
        Err(RaiseError::TooLarge { cap: 16_128 })
    );

    let mut out = vec![0u8; 16_384];
    let first = rt
        .next(&mut out)
        .expect("next")
        .expect("the first occurrence");
    assert_eq!((first.len, first.envelope.seq), (2, 1));
    let second = rt
        .next(&mut out)
        .expect("next")
        .expect("the second occurrence");
    assert_eq!(
        (second.len, second.envelope.seq),
        (16_128, 2),
        "a refused occurrence draws no sequence number"
    );
    assert!(rt.next(&mut out).expect("next").is_none());
}

#[test]
fn a_limit_given_to_the_loopback_lowers_and_raises_a_calls_cap() {
    let mut rt = runtime();
    rt.serve(IFACE, &[ORD]).expect("serve");
    rt.set_limit(IFACE, ORD, 2);
    assert_eq!(
        rt.command(IFACE, ORD, &[1, 2, 3], None),
        Err(SendError::TooLarge { cap: 2 })
    );
    assert_eq!(
        rt.query(IFACE, ORD, &[1, 2, 3], None),
        Err(SendError::TooLarge { cap: 2 })
    );
    let c = rt
        .query(IFACE, ORD, &[1, 2], None)
        .expect("within the given limit");
    let mut buf = [0u8; 8];
    let claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("the accepted call");
    assert_eq!(
        claim.envelope.seq, 1,
        "a refused call draws no sequence number"
    );
    assert_eq!(
        rt.settle(claim.id, Ok(&[1, 2, 3])),
        Err(SettleError::TooLarge { cap: 2 }),
        "the reply is held to the same limit"
    );
    rt.settle(claim.id, Ok(&[1, 2]))
        .expect("within the given limit");
    assert_eq!(rt.reply(c, &mut buf).expect("reply"), Some(Ok(2)));
    rt.forget(c);

    rt.set_limit(IFACE, ORD, 65_280);
    let c = rt
        .query(IFACE, ORD, &[0u8; 65_280], None)
        .expect("a limit above the default ceiling raises the cap");
    assert_eq!(
        rt.query(IFACE, ORD, &[0u8; 65_281], None),
        Err(SendError::TooLarge { cap: 65_280 })
    );
    let mut large = vec![0u8; 65_280];
    let claim = rt
        .next_claim(&mut large)
        .expect("next_claim")
        .expect("the accepted call");
    rt.settle(claim.id, Ok(&large))
        .expect("a reply at the raised limit is recorded");
    assert_eq!(rt.reply(c, &mut large).expect("reply"), Some(Ok(65_280)));
}

#[test]
fn a_limit_is_the_stores_so_an_attached_aggregate_applies_it() {
    let mut rt = runtime();
    rt.set_limit(IFACE, ORD, 2);
    let mut attached = rt.attach();
    assert_eq!(
        attached.set(IFACE, ORD, &[1, 2, 3]),
        Err(WriteError::TooLarge { cap: 2 })
    );
    let mut writer = rt.writer();
    assert_eq!(
        writer.set(IFACE, ORD, &[1, 2, 3]),
        Err(WriteError::TooLarge { cap: 2 })
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
fn an_attached_aggregate_carries_the_catalog() {
    let attached = runtime().attach();
    // A handle made from the attached aggregate reads the aggregate's own
    // catalog, not one of the six handles'.
    assert_eq!(*attached.caller().catalog(), catalog());
    let handles = attached.split();
    assert_eq!(*handles.reader.catalog(), catalog());
    assert_eq!(*handles.writer.catalog(), catalog());
    assert_eq!(*handles.source.catalog(), catalog());
    assert_eq!(*handles.sink.catalog(), catalog());
    assert_eq!(*handles.caller.catalog(), catalog());
    assert_eq!(*handles.handler.catalog(), catalog());
}

#[test]
fn a_value_committed_through_one_aggregate_is_read_through_an_attached_one() {
    // The published signals are in the store, and staging is on the writer
    // handle: one aggregate's commit publishes only what that aggregate staged,
    // and a value staged before the attach is not carried over.
    let mut rt = runtime();
    rt.set(IFACE, ORD, &[1]).expect("staged");
    let mut attached = rt.attach();
    attached.set(IFACE, OTHER, &[2]).expect("staged");
    attached.commit();

    let mut out = [0u8; 8];
    let unpublished = rt.read(IFACE, ORD, &mut out).expect("read");
    assert_eq!(unpublished.provenance, Provenance::Init);
    let published = rt.read(IFACE, OTHER, &mut out).expect("read");
    assert_eq!(&out[..published.len], &[2]);

    rt.commit();
    let sample = attached.read(IFACE, ORD, &mut out).expect("read");
    assert_eq!(&out[..sample.len], &[1]);
}

#[test]
fn an_attached_aggregate_advances_the_one_clock() {
    let rt = runtime();
    let mut attached = rt.attach();
    attached.advance(Duration(5));
    assert_eq!(rt.now(), Timestamp(5));
}

#[test]
fn an_attached_aggregate_starts_with_none_of_the_originals_handle_state() {
    // Subscriptions, the served set and the sequence counters are on a handle,
    // so an aggregate attached after the original used all of them starts with
    // none of them.
    let mut rt = runtime();
    rt.subscribe(IFACE, &[ORD]).expect("subscribe");
    rt.serve(IFACE, &[ORD]).expect("serve");
    rt.set(IFACE, ORD, &[1]).expect("staged");
    rt.commit();
    rt.raise(IFACE, ORD, &[1], None).expect("raise");
    rt.command(IFACE, ORD, &[1], None).expect("send");
    let mut attached = rt.attach();

    let mut out = [0u8; 8];
    attached.set(IFACE, ORD, &[2]).expect("staged");
    attached.commit();
    let sample = attached.read(IFACE, ORD, &mut out).expect("read");
    assert_eq!(
        sample.envelope.seq, 1,
        "the attached writer's first publication"
    );

    attached.raise(IFACE, ORD, &[2], None).expect("raise");
    assert!(
        attached.next(&mut out).expect("next").is_none(),
        "the attached source is subscribed to nothing"
    );
    let mut raised = Vec::new();
    while let Some(occurrence) = rt.next(&mut out).expect("next") {
        raised.push(occurrence.envelope.seq);
    }
    assert_eq!(
        raised,
        vec![1, 1],
        "each sink's first raise is its own seq 1"
    );

    attached.command(IFACE, ORD, &[2], None).expect("send");
    let mut sent = Vec::new();
    while let Some(claim) = rt.next_claim(&mut out).expect("next_claim") {
        sent.push(claim.envelope.seq);
    }
    assert_eq!(
        sent,
        vec![1, 1],
        "each caller's first call is its own seq 1"
    );

    assert!(attached.split().handler.served().is_empty());
}

#[test]
fn an_attached_aggregate_has_its_own_subscriptions_and_queue() {
    let mut rt = runtime();
    let mut attached = rt.attach();
    rt.subscribe(IFACE, &[ORD]).expect("subscribe");

    attached.raise(IFACE, ORD, &[1], None).expect("raise");
    let mut out = [0u8; 8];
    assert!(
        attached.next(&mut out).expect("next").is_none(),
        "the attached aggregate's source is subscribed to nothing"
    );

    attached.subscribe(IFACE, &[ORD]).expect("subscribe");
    attached.raise(IFACE, ORD, &[2], None).expect("raise");
    let mut drain = |port: &mut Loopback| {
        let mut payloads = Vec::new();
        while let Some(occurrence) = port.next(&mut out).expect("next") {
            payloads.push(out[..occurrence.len].to_vec());
        }
        payloads
    };
    assert_eq!(drain(&mut rt), vec![vec![1], vec![2]]);
    assert_eq!(drain(&mut attached), vec![vec![2]]);
}

#[test]
fn a_call_sent_through_one_aggregate_is_served_through_an_attached_one() {
    let mut rt = runtime();
    let mut attached = rt.attach();
    let sent = rt.command(IFACE, ORD, &[1], None).expect("send");

    let mut buf = [0u8; 8];
    let claim = attached
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    attached.settle(claim.id, Ok(&[])).expect("settle");
    assert_eq!(rt.ack(sent), Some(Ok(())));
}

#[test]
fn dropping_an_attached_aggregate_leaves_the_originals_calls_and_subscriptions() {
    // Each handle's `Drop` closes that handle's own identity in the store, and
    // the handles of an attached aggregate have identities of their own: the
    // original's claim stays the original's. The attached aggregate's own
    // unclaimed call is withdrawn with it.
    let mut rt = runtime();
    rt.subscribe(IFACE, &[ORD]).expect("subscribe");
    let sent = rt.command(IFACE, ORD, &[1], None).expect("send");
    let mut attached = rt.attach();
    attached.subscribe(IFACE, &[ORD]).expect("subscribe");
    attached.command(IFACE, ORD, &[2], None).expect("send");
    let mut out = [0u8; 8];
    let claim = rt
        .next_claim(&mut out)
        .expect("next_claim")
        .expect("the original's call, sent first");
    assert_eq!(out[0], 1);
    drop(attached);

    rt.raise(IFACE, ORD, &[3], None).expect("raise");
    let occurrence = rt.next(&mut out).expect("next").expect("subscribed");
    assert_eq!(&out[..occurrence.len], &[3]);

    rt.settle(claim.id, Ok(&[]))
        .expect("the claim is still the original's");
    assert_eq!(rt.ack(sent), Some(Ok(())));
    assert!(
        rt.next_claim(&mut out).expect("next_claim").is_none(),
        "the dropped aggregate's call was withdrawn"
    );
}

#[test]
fn an_attached_aggregate_keeps_the_store_after_the_original_is_dropped() {
    // The store outlives the original, and so do the attached aggregate's own
    // subscription and call.
    let mut rt = runtime();
    let mut attached = rt.attach();
    attached.subscribe(IFACE, &[ORD]).expect("subscribe");
    let sent = attached.command(IFACE, ORD, &[5], None).expect("send");
    rt.set(IFACE, ORD, &[4]).expect("staged");
    rt.commit();
    drop(rt);

    let mut out = [0u8; 8];
    let sample = attached.read(IFACE, ORD, &mut out).expect("read");
    assert_eq!(&out[..sample.len], &[4]);

    attached.raise(IFACE, ORD, &[6], None).expect("raise");
    let occurrence = attached.next(&mut out).expect("next").expect("subscribed");
    assert_eq!(&out[..occurrence.len], &[6]);

    let claim = attached
        .next_claim(&mut out)
        .expect("next_claim")
        .expect("the attached aggregate's call");
    assert_eq!(out[0], 5);
    attached.settle(claim.id, Ok(&[])).expect("settle");
    assert_eq!(attached.ack(sent), Some(Ok(())));
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
fn each_settlement_wakes_only_its_own_calls_waiter() {
    // An `Outcome` waker is per call, not per handle: two calls' wakers are
    // held at once, and a settlement wakes the one of the call it settles.
    let Handles {
        mut caller,
        mut handler,
        ..
    } = runtime().split();
    let mut buf = [0u8; 8];

    let first_call = caller.command(IFACE, ORD, &[1], None).expect("send");
    let second_call = caller.command(IFACE, ORD, &[2], None).expect("send");
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
    let c = caller.query(IFACE, ORD, &[1], None).expect("send");
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

    sink.raise(IFACE, ORD, &[1], None).expect("raise");
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
fn a_waiting_occurrence_of_another_interface_wakes_an_event_registration_at_once() {
    let rt = runtime();
    let mut sink = rt.sink();
    let mut source = rt.source();
    source.subscribe(InterfaceNo(2), &[ORD]).expect("subscribe");
    sink.raise(InterfaceNo(2), ORD, &[1], None).expect("raise");

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
    caller
        .command(InterfaceNo(2), ORD, &[1], None)
        .expect("send");

    let (count, waker) = counting();
    handler.wake_on(Interest::Claim(IFACE), &waker);
    assert_eq!(wakes(&count), 1, "a call on another interface is waiting");
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

    let kept = caller.command(IFACE, ORD, &[1], None).expect("send");
    let kept_claim = keeper
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    caller.command(IFACE, ORD, &[2], None).expect("send");
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
fn a_waiter_woken_at_once_is_not_stored() {
    let rt = runtime();
    let mut sink = rt.sink();
    let mut source = rt.source();
    source.subscribe(IFACE, &[ORD]).expect("subscribe");
    sink.raise(IFACE, ORD, &[1], None).expect("raise");
    let (count, waker) = counting();
    source.wake_on(Interest::Event(IFACE), &waker);
    assert_eq!(wakes(&count), 1, "an occurrence is waiting");

    sink.raise(IFACE, ORD, &[2], None).expect("raise");
    assert_eq!(wakes(&count), 1, "a waker woken at once was not stored");
}

#[test]
fn a_registration_on_a_forgotten_call_is_woken_at_once() {
    // A forgotten call a handler has claimed is still in the call table, but
    // no outcome will be recorded for it.
    let Handles {
        mut caller,
        mut handler,
        ..
    } = runtime().split();
    let c = caller.command(IFACE, ORD, &[1], None).expect("send");
    handler
        .next_claim(&mut [0u8; 8])
        .expect("next_claim")
        .expect("waiting");
    caller.forget(c);
    let (count, waker) = counting();
    caller.wake_on(Interest::Outcome(c), &waker);
    assert_eq!(wakes(&count), 1, "no outcome will be recorded");
}

#[test]
fn a_registration_on_a_correlation_whose_slot_was_reused_is_woken_at_once() {
    // The slot of `old` now holds a call under a higher generation. No
    // outcome will ever be recorded under `old`, so the registration is woken
    // at once, and the call now in the slot does not wake it again.
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    let mut buf = [0u8; 8];
    let old = caller.command(IFACE, ORD, &[1], None).expect("send");
    let claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    handler.settle(claim.id, Ok(&[])).expect("settle");
    caller.forget(old);
    let new = caller
        .command(IFACE, ORD, &[2], None)
        .expect("send into the same slot");
    assert_ne!(new, old, "the slot is reused under a new generation");

    let (count, waker) = counting();
    caller.wake_on(Interest::Outcome(old), &waker);
    assert_eq!(wakes(&count), 1, "no outcome will be recorded under it");

    let claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("the new call");
    handler.settle(claim.id, Ok(&[])).expect("settle");
    assert_eq!(wakes(&count), 1, "the waker was not stored");
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

    sink.raise(IFACE, ORD, &[1], None).expect("raise");
    assert_eq!(wakes(&wakers[0].0), 1, "the first source");
    assert_eq!(wakes(&wakers[1].0), 1, "the second source");

    caller.command(IFACE, ORD, &[1], None).expect("send");
    assert_eq!(wakes(&wakers[2].0), 1, "the first handler");
    assert_eq!(wakes(&wakers[3].0), 1, "the second handler");
}

#[test]
fn a_claim_registration_is_not_woken_by_a_call_the_handler_does_not_serve() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    handler.serve(IFACE, &[OTHER]).expect("serve");
    caller.command(IFACE, ORD, &[1], None).expect("send");
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

    let c = caller.command(IFACE, ORD, &[1], None).expect("send");
    first
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    let later = caller.command(IFACE, ORD, &[2], None).expect("send");
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

/// The number of commands `caller` accepts before it answers
/// `SendError::Busy`, counting at most one more than `Loopback::SLOTS`.
fn sends_until_busy(caller: &mut impl Caller) -> usize {
    for sent in 0..=Loopback::SLOTS {
        match caller.command(IFACE, ORD, &[9], None) {
            Ok(_) => {}
            Err(SendError::Busy) => return sent,
            Err(error) => panic!("a send failed other than busy: {error:?}"),
        }
    }
    Loopback::SLOTS + 1
}

/// Offers `handler` the next waiting call through a buffer too short for it,
/// and returns the id `ShortClaim` carried.
fn offer(handler: &mut impl Handler) -> ClaimId {
    let mut short = [0u8; 1];
    match handler.next_claim(&mut short) {
        Err(ReadError::ShortClaim { claim, .. }) => claim,
        other => panic!("a buffer shorter than the arguments reports ShortClaim: {other:?}"),
    }
}

/// driftsys/ridl#569: a call offered through `ShortClaim` whose caller then
/// forgets it stays among the waiting calls, so the provider's retry with a
/// larger buffer presents it under the same id, and its settlement frees the
/// slot as for a forgotten taken claim.
#[test]
fn a_forgotten_offered_call_is_presented_again_under_the_same_id_and_its_settlement_frees_the_slot()
{
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    handler.serve(IFACE, &[ORD]).expect("serve");
    let c = caller.command(IFACE, ORD, &[1, 2, 3], None).expect("send");
    let claim = offer(&mut handler);
    caller.forget(c);

    let mut buf = [0u8; 8];
    let retry = handler
        .next_claim(&mut buf)
        .expect("read")
        .expect("a forgotten offered call stays presentable");
    assert_eq!(retry.id, claim, "under the id ShortClaim carried");
    assert_eq!(&buf[..retry.len], &[1, 2, 3]);
    handler.settle(retry.id, Ok(&[])).expect("settle");
    assert_eq!(
        sends_until_busy(&mut caller),
        Loopback::SLOTS,
        "the settlement freed the slot"
    );
}

/// driftsys/ridl#569: a dropped handler's offered claim never left the
/// waiting calls, so it is not re-inserted and no serving handler is woken for
/// it; another handler that serves the member takes it exactly once, under the
/// same id, and its settlement reaches the caller.
#[test]
fn a_dropped_handlers_offered_claim_is_taken_once_by_another_handler_under_the_same_id() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut first = rt.handler();
    let mut second = rt.handler();
    first.serve(IFACE, &[ORD]).expect("serve");
    second.serve(IFACE, &[ORD]).expect("serve");
    let c = caller.command(IFACE, ORD, &[1, 2, 3], None).expect("send");
    let claim = offer(&mut first);
    // Registered while the offered call is already waiting, so the registration
    // is woken at once; the count is read before the drop and compared after.
    let (count, waker) = counting();
    second.wake_on(Interest::Claim(IFACE), &waker);
    let before = wakes(&count);

    drop(first);
    assert_eq!(
        wakes(&count),
        before,
        "the drop wakes no handler for an offered claim: the call never left the waiting calls"
    );
    let mut buf = [0u8; 8];
    let taken = second
        .next_claim(&mut buf)
        .expect("read")
        .expect("the offered call is still waiting");
    assert_eq!(taken.id, claim, "the id stays on the call");
    assert_eq!(
        second.next_claim(&mut buf).expect("read"),
        None,
        "the call is presented once, not re-inserted"
    );
    second.settle(taken.id, Ok(&[])).expect("settle");
    assert_eq!(
        caller.ack(c),
        Some(Ok(())),
        "the settlement reaches the caller"
    );
}

/// driftsys/ridl#569: an offered call whose caller forgot it is withdrawn when
/// its handler drops, as a forgotten taken claim is: no handler is presented
/// it, and its slot is reclaimed.
#[test]
fn a_forgotten_offered_call_is_withdrawn_when_its_handler_drops() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut first = rt.handler();
    let mut second = rt.handler();
    first.serve(IFACE, &[ORD]).expect("serve");
    second.serve(IFACE, &[ORD]).expect("serve");
    let c = caller.command(IFACE, ORD, &[1, 2, 3], None).expect("send");
    offer(&mut first);
    caller.forget(c);

    drop(first);
    let mut buf = [0u8; 8];
    assert_eq!(
        second.next_claim(&mut buf).expect("read"),
        None,
        "a forgotten offered call is withdrawn at the drop"
    );
    assert_eq!(
        sends_until_busy(&mut caller),
        Loopback::SLOTS,
        "and its slot is reclaimed"
    );
}

/// driftsys/ridl#569: another handler that serves the member may take a call
/// offered to the first, under the same id; the claim moves with the take, so
/// the first handler's settlement of that id is unknown.
#[test]
fn an_offered_call_taken_by_another_handler_moves_the_claim_to_it() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut first = rt.handler();
    let mut second = rt.handler();
    first.serve(IFACE, &[ORD]).expect("serve");
    second.serve(IFACE, &[ORD]).expect("serve");
    let c = caller.command(IFACE, ORD, &[1, 2, 3], None).expect("send");
    let claim = offer(&mut first);

    let mut buf = [0u8; 8];
    let taken = second
        .next_claim(&mut buf)
        .expect("read")
        .expect("an offered call can be taken by another serving handler");
    assert_eq!(taken.id, claim);
    second
        .settle(taken.id, Ok(&[]))
        .expect("the taker settles it");
    assert_eq!(
        first.settle(claim, Ok(&[])),
        Err(SettleError::UnknownClaim),
        "the claim moved to the taker"
    );
    assert_eq!(caller.ack(c), Some(Ok(())));
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

    caller.command(IFACE, ORD, &[1], None).expect("send");
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
    caller.command(IFACE, ORD, &[1], None).expect("send");
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

    sink.raise(IFACE, ORD, &[1], None).expect("raise");
    let (event, event_waker) = counting();
    source.wake_on(Interest::Event(IFACE), &event_waker);
    assert_eq!(wakes(&event), 1, "an occurrence is waiting");

    caller.command(IFACE, ORD, &[1], None).expect("send");
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
    let c = caller.command(IFACE, ORD, &[1], None).expect("send");
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
    caller.command(IFACE, ORD, &[1], None).expect("send");
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
    caller.command(IFACE, ORD, &[2], None).expect("send");
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
    sink.raise(IFACE, ORD, &[1], None).expect("raise");
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

    let command = caller.command(IFACE, ORD, &[1], None).expect("send");
    let query = caller.query(IFACE, ORD, &[2], None).expect("send");
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

    caller.command(IFACE, ORD, &[1], None).expect("send");
    first
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    caller.command(IFACE, ORD, &[2], None).expect("send");
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

    let old = caller.command(IFACE, ORD, &[0], None).expect("send");
    let claim = first
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    first.settle(claim.id, Ok(&[])).expect("settle");
    caller.forget(old);

    let reused = caller
        .command(IFACE, ORD, &[1], None)
        .expect("send into slot 0");
    let fresh = caller
        .command(IFACE, ORD, &[2], None)
        .expect("send into slot 1");
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
                .command(IFACE, ORD, &[u8::try_from(n).expect("small")], None)
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

    assert_eq!(
        caller.command(IFACE, ORD, &[99], None),
        Err(SendError::Busy)
    );
    assert_eq!(caller.query(IFACE, ORD, &[99], None), Err(SendError::Busy));
    assert_eq!(
        other.command(IFACE, ORD, &[99], None),
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
        caller.command(IFACE, ORD, &[99], None),
        Err(SendError::Busy),
        "a settled call keeps its slot until it is forgotten"
    );

    caller.forget(calls[0]);
    other
        .command(IFACE, ORD, &[99], None)
        .expect("the forget freed a slot");
    assert_eq!(
        caller.command(IFACE, ORD, &[100], None),
        Err(SendError::Busy)
    );
}

#[test]
fn a_refused_send_draws_no_sequence_number() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    let mut buf = [0u8; 8];
    let calls = fill(&mut caller);
    assert_eq!(
        caller.command(IFACE, ORD, &[99], None),
        Err(SendError::Busy)
    );

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
    caller
        .command(IFACE, ORD, &[99], None)
        .expect("a slot is free");
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
fn a_claimed_then_forgotten_call_holds_its_slot_until_its_settlement() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    let mut buf = [0u8; 8];
    let calls = fill(&mut caller);
    let (count, waker) = counting();
    caller.wake_on(Interest::Slot, &waker);

    let claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    assert_eq!(&buf[..claim.len], &[0], "the claim is calls[0]");
    caller.forget(calls[0]);
    assert_eq!(wakes(&count), 0, "the claimed call still holds its slot");
    assert_eq!(
        caller.command(IFACE, ORD, &[99], None),
        Err(SendError::Busy)
    );

    handler
        .settle(claim.id, Ok(&[]))
        .expect("the provider's settlement is still accepted");
    assert_eq!(wakes(&count), 1, "its settlement reclaims the slot");
    caller
        .command(IFACE, ORD, &[99], None)
        .expect("the slot is free");
}

#[test]
fn forgotten_calls_to_an_unserved_member_leave_room_for_another_send() {
    // No handler serves `OTHER`, so these calls are never claimed and never
    // settled. Each `forget` withdraws its call and reclaims its slot at
    // once; without that, the table stays full for the life of the runtime.
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    handler.serve(IFACE, &[ORD]).expect("serve");
    let calls: Vec<_> = (0..Loopback::SLOTS)
        .map(|n| {
            caller
                .command(IFACE, OTHER, &[u8::try_from(n).expect("small")], None)
                .expect("a slot is free")
        })
        .collect();
    assert_eq!(
        caller.command(IFACE, OTHER, &[99], None),
        Err(SendError::Busy)
    );
    let (count, waker) = counting();
    caller.wake_on(Interest::Slot, &waker);

    for c in &calls {
        caller.forget(*c);
    }
    assert_eq!(wakes(&count), 1, "the withdrawal reclaimed a slot");
    for n in 0..Loopback::SLOTS {
        caller
            .command(IFACE, OTHER, &[u8::try_from(n).expect("small")], None)
            .expect("every withdrawn call's slot is free");
    }
    assert_eq!(
        caller.command(IFACE, OTHER, &[99], None),
        Err(SendError::Busy)
    );
}

#[test]
fn a_withdrawn_call_is_never_presented_to_a_handler_that_serves_the_member_later() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    let mut buf = [0u8; 8];
    handler.serve(IFACE, &[OTHER]).expect("serve");
    let (count, waker) = counting();
    handler.wake_on(Interest::Claim(IFACE), &waker);

    let withdrawn = caller.command(IFACE, ORD, &[1], None).expect("send");
    let kept = caller.command(IFACE, ORD, &[2], None).expect("send");
    caller.forget(withdrawn);

    handler.serve(IFACE, &[ORD]).expect("serve");
    assert_eq!(wakes(&count), 1, "the call not forgotten is waiting");
    let claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("the call not forgotten");
    assert_eq!(&buf[..claim.len], &[2], "the withdrawn call is skipped");
    handler.settle(claim.id, Ok(&[])).expect("settle");
    assert_eq!(caller.ack(kept), Some(Ok(())));
    assert_eq!(
        handler.next_claim(&mut buf).expect("next_claim"),
        None,
        "the withdrawn call is never presented"
    );
}

#[test]
fn a_withdrawal_from_the_middle_of_the_queue_leaves_the_calls_around_it_in_order() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    let mut buf = [0u8; 8];
    caller.command(IFACE, ORD, &[1], None).expect("send");
    let middle = caller.command(IFACE, ORD, &[2], None).expect("send");
    caller.command(IFACE, ORD, &[3], None).expect("send");
    caller.forget(middle);

    handler.serve(IFACE, &[ORD]).expect("serve");
    for expected in [1u8, 3] {
        let claim = handler
            .next_claim(&mut buf)
            .expect("next_claim")
            .expect("waiting");
        assert_eq!(
            &buf[..claim.len],
            &[expected],
            "send order, less the middle"
        );
    }
    assert_eq!(handler.next_claim(&mut buf).expect("next_claim"), None);
}

#[test]
fn forgetting_a_claimed_call_wakes_its_outcome_waiter() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    let c = caller.command(IFACE, ORD, &[1], None).expect("send");
    handler
        .next_claim(&mut [0u8; 8])
        .expect("next_claim")
        .expect("waiting");
    let (count, waker) = counting();
    caller.wake_on(Interest::Outcome(c), &waker);
    assert_eq!(wakes(&count), 0, "the claimed call is in flight");

    caller.forget(c);
    assert_eq!(
        wakes(&count),
        1,
        "no outcome will be readable for a forgotten call"
    );
}

#[test]
fn a_stale_correlation_does_not_withdraw_the_call_now_in_its_slot() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    let mut buf = [0u8; 8];
    let old = caller.command(IFACE, ORD, &[1], None).expect("send");
    let claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    handler.settle(claim.id, Ok(&[])).expect("settle");
    caller.forget(old);

    let new = caller
        .command(IFACE, ORD, &[2], None)
        .expect("send into the same slot");
    assert_ne!(new, old, "the slot is reused under a new generation");
    caller.forget(old);
    let claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("the stale correlation withdrew nothing");
    assert_eq!(&buf[..claim.len], &[2]);
    handler.settle(claim.id, Ok(&[])).expect("settle");
    assert_eq!(caller.ack(new), Some(Ok(())));
}

#[test]
fn a_call_forgotten_while_claimed_is_withdrawn_when_its_handler_is_dropped() {
    // Sixteen rounds of claim and forget, each call held by its own handler,
    // then every handler dropped. No other handler serves the member, so a
    // call returned to the waiting calls would hold its slot for the life of
    // the runtime.
    let rt = runtime();
    let mut caller = rt.caller();
    let mut buf = [0u8; 8];
    let mut handlers = Vec::new();
    for n in 0..Loopback::SLOTS {
        let c = caller
            .command(IFACE, ORD, &[u8::try_from(n).expect("small")], None)
            .expect("a slot is free");
        let mut handler = rt.handler();
        handler.serve(IFACE, &[ORD]).expect("serve");
        handler
            .next_claim(&mut buf)
            .expect("next_claim")
            .expect("waiting");
        caller.forget(c);
        handlers.push(handler);
    }
    assert_eq!(
        caller.command(IFACE, ORD, &[99], None),
        Err(SendError::Busy),
        "a claimed call keeps its slot after its forget"
    );
    let (count, waker) = counting();
    caller.wake_on(Interest::Slot, &waker);
    assert_eq!(wakes(&count), 0, "no slot is free: the waker is stored");

    drop(handlers);
    assert_eq!(wakes(&count), 1, "the drops reclaimed the slots");
    for n in 0..Loopback::SLOTS {
        caller
            .command(IFACE, ORD, &[100 + u8::try_from(n).expect("small")], None)
            .expect("every withdrawn call's slot is free");
    }
    assert_eq!(
        caller.command(IFACE, ORD, &[99], None),
        Err(SendError::Busy)
    );
    let mut later = rt.handler();
    later.serve(IFACE, &[ORD]).expect("serve");
    for n in 0..Loopback::SLOTS {
        let claim = later
            .next_claim(&mut buf)
            .expect("next_claim")
            .expect("waiting");
        assert_eq!(
            &buf[..claim.len],
            &[100 + u8::try_from(n).expect("small")],
            "no withdrawn call is presented again"
        );
    }
    assert_eq!(later.next_claim(&mut buf).expect("next_claim"), None);
}

#[test]
fn a_forget_withdrawal_wakes_no_claim_waiter_of_a_handler_serving_another_member() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    handler.serve(IFACE, &[OTHER]).expect("serve");
    let c = caller.command(IFACE, ORD, &[1], None).expect("send");
    let (count, waker) = counting();
    handler.wake_on(Interest::Claim(IFACE), &waker);
    assert_eq!(wakes(&count), 0, "no call it serves waits: stored");

    caller.forget(c);
    assert_eq!(
        wakes(&count),
        0,
        "a withdrawal adds no call to claim, so it wakes no claim waiter"
    );
    caller
        .command(IFACE, OTHER, &[2], None)
        .expect("send a call it serves");
    assert_eq!(wakes(&count), 1, "the waker was still stored");
}

#[test]
fn a_dropped_handler_withdraws_only_its_forgotten_claim_and_returns_the_others() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut first = rt.handler();
    let mut buf = [0u8; 8];
    first.serve(IFACE, &[ORD]).expect("serve");
    let sent: Vec<_> = (1u8..=3)
        .map(|n| caller.command(IFACE, ORD, &[n], None).expect("send"))
        .collect();
    for _ in &sent {
        first
            .next_claim(&mut buf)
            .expect("next_claim")
            .expect("waiting");
    }
    for n in 3..Loopback::SLOTS {
        caller
            .command(IFACE, OTHER, &[u8::try_from(n).expect("small")], None)
            .expect("a slot is free");
    }
    caller.forget(sent[1]);
    assert_eq!(
        caller.command(IFACE, OTHER, &[99], None),
        Err(SendError::Busy),
        "the claimed call keeps its slot after its forget"
    );

    drop(first);
    caller
        .command(IFACE, OTHER, &[99], None)
        .expect("the withdrawn claim's slot came back");
    assert_eq!(
        caller.command(IFACE, OTHER, &[100], None),
        Err(SendError::Busy),
        "exactly one slot came back"
    );
    let mut later = rt.handler();
    later.serve(IFACE, &[ORD]).expect("serve");
    for expected in [1u8, 3] {
        let claim = later
            .next_claim(&mut buf)
            .expect("next_claim")
            .expect("a returned claim");
        assert_eq!(&buf[..claim.len], &[expected], "send order, less 2");
    }
    assert_eq!(later.next_claim(&mut buf).expect("next_claim"), None);
}

#[test]
fn a_dropped_callers_claimed_call_is_withdrawn_when_its_handler_is_dropped() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut other = rt.caller();
    let mut first = rt.handler();
    let mut buf = [0u8; 8];
    first.serve(IFACE, &[ORD]).expect("serve");
    caller.command(IFACE, ORD, &[1], None).expect("send");
    first
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");

    drop(caller);
    drop(first);
    for n in 0..Loopback::SLOTS {
        other
            .command(IFACE, ORD, &[100 + u8::try_from(n).expect("small")], None)
            .expect("the withdrawn call's slot is free");
    }
    let mut later = rt.handler();
    later.serve(IFACE, &[ORD]).expect("serve");
    let claim = later
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    assert_eq!(
        &buf[..claim.len],
        &[100],
        "the dropped caller's call is not presented again"
    );
}

#[test]
fn a_withdrawal_at_a_handlers_drop_wakes_no_claim_waiter() {
    // A handler that serves the withdrawn member holds a stored `Claim`
    // waker only at this withdrawal, the one at a handler's drop: while a
    // call it serves is waiting, its registration is woken at once. A
    // handler that serves another member can hold one at a `forget`
    // withdrawal too, which
    // `a_forget_withdrawal_wakes_no_claim_waiter_of_a_handler_serving_another_member`
    // pins.
    let rt = runtime();
    let mut caller = rt.caller();
    let mut first = rt.handler();
    let mut second = rt.handler();
    first.serve(IFACE, &[ORD]).expect("serve");
    second.serve(IFACE, &[ORD]).expect("serve");
    let c = caller.command(IFACE, ORD, &[1], None).expect("send");
    first
        .next_claim(&mut [0u8; 8])
        .expect("next_claim")
        .expect("waiting");
    let (count, waker) = counting();
    second.wake_on(Interest::Claim(IFACE), &waker);
    assert_eq!(wakes(&count), 0, "nothing is waiting: the waker is stored");

    caller.forget(c);
    drop(first);
    assert_eq!(
        wakes(&count),
        0,
        "a withdrawal adds no call to claim, so it wakes no claim waiter"
    );
    assert_eq!(second.next_claim(&mut [0u8; 8]).expect("next_claim"), None);
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
    caller.command(IFACE, ORD, &[99], None).expect("send");
    caller.wake_on(Interest::Slot, &waker);
    assert_eq!(wakes(&count), 1, "the table is full again: stored");
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
fn a_dropped_caller_forgets_its_calls_and_their_slots_come_back() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut other = rt.caller();
    let mut handler = rt.handler();
    let mut buf = [0u8; 8];
    fill(&mut caller);
    for _ in 0..8 {
        let claim = handler
            .next_claim(&mut buf)
            .expect("next_claim")
            .expect("waiting");
        handler.settle(claim.id, Ok(&[])).expect("settle");
    }
    let held: Vec<_> = (0..8)
        .map(|_| {
            handler
                .next_claim(&mut buf)
                .expect("next_claim")
                .expect("waiting")
        })
        .collect();
    assert_eq!(other.command(IFACE, ORD, &[99], None), Err(SendError::Busy));
    let (count, waker) = counting();
    other.wake_on(Interest::Slot, &waker);

    drop(caller);
    assert_eq!(
        wakes(&count),
        1,
        "the drop reclaimed the settled calls' slots"
    );
    for n in 0..8 {
        other
            .command(IFACE, ORD, &[n], None)
            .expect("a slot of a settled call of the dropped caller");
    }
    assert_eq!(
        other.command(IFACE, ORD, &[99], None),
        Err(SendError::Busy),
        "the dropped caller's claimed calls keep their slots until settled"
    );

    // The provider still settles the eight claims it holds; each settlement
    // reclaims a slot, because the drop forgot the call.
    for claim in held {
        handler.settle(claim.id, Ok(&[])).expect("settle");
    }
    for n in 0..8 {
        other
            .command(IFACE, ORD, &[n], None)
            .expect("a slot of a claimed call of the dropped caller");
    }
    assert_eq!(other.command(IFACE, ORD, &[99], None), Err(SendError::Busy));
}

#[test]
fn a_dropped_callers_unclaimed_calls_are_withdrawn() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut other = rt.caller();
    let mut handler = rt.handler();
    let mut buf = [0u8; 8];
    fill(&mut caller);
    let (count, waker) = counting();
    other.wake_on(Interest::Slot, &waker);

    drop(caller);
    assert_eq!(wakes(&count), 1, "the drop reclaimed the unclaimed calls");
    for n in 0..Loopback::SLOTS {
        other
            .command(IFACE, ORD, &[100 + u8::try_from(n).expect("small")], None)
            .expect("every unclaimed call of the dropped caller was withdrawn");
    }
    assert_eq!(other.command(IFACE, ORD, &[99], None), Err(SendError::Busy));

    for n in 0..Loopback::SLOTS {
        let claim = handler
            .next_claim(&mut buf)
            .expect("next_claim")
            .expect("waiting");
        assert_eq!(
            &buf[..claim.len],
            &[100 + u8::try_from(n).expect("small")],
            "only the other caller's calls are presented"
        );
    }
    assert_eq!(handler.next_claim(&mut buf).expect("next_claim"), None);
}

#[test]
fn a_dropped_caller_forgets_only_its_own_calls() {
    let rt = runtime();
    let mut first = rt.caller();
    let mut second = rt.caller();
    let mut handler = rt.handler();
    let mut buf = [0u8; 8];
    first.command(IFACE, ORD, &[1], None).expect("send");
    let theirs = second.command(IFACE, ORD, &[2], None).expect("send");
    while let Some(claim) = handler.next_claim(&mut buf).expect("next_claim") {
        handler.settle(claim.id, Ok(&[])).expect("settle");
    }

    drop(first);
    assert_eq!(
        second.ack(theirs),
        Some(Ok(())),
        "the other caller's settled call is still readable"
    );
}

#[test]
fn a_dropped_callers_call_in_flight_wakes_its_outcome_waiter() {
    let rt = runtime();
    let mut caller = rt.caller();
    let c = caller.command(IFACE, ORD, &[1], None).expect("send");
    let (count, waker) = counting();
    caller.wake_on(Interest::Outcome(c), &waker);
    assert_eq!(wakes(&count), 0, "the call is in flight");

    drop(caller);
    assert_eq!(
        wakes(&count),
        1,
        "the drop forgot the call, so no outcome will be readable for it"
    );
}

#[test]
fn a_dropped_callers_own_slot_waiter_is_not_woken_by_its_drop() {
    let rt = runtime();
    let mut caller = rt.caller();
    let other = rt.caller();
    let mut handler = rt.handler();
    let mut buf = [0u8; 8];
    fill(&mut caller);
    while let Some(claim) = handler.next_claim(&mut buf).expect("next_claim") {
        handler.settle(claim.id, Ok(&[])).expect("settle");
    }
    let (own, own_waker) = counting();
    let (theirs, their_waker) = counting();
    caller.wake_on(Interest::Slot, &own_waker);
    other.wake_on(Interest::Slot, &their_waker);

    drop(caller);
    assert_eq!(
        (wakes(&own), wakes(&theirs)),
        (0, 1),
        "the dropped caller's waiters leave before its calls are reclaimed"
    );
    assert_eq!(Arc::strong_count(&own), 2, "and its waker is released");
}

#[test]
fn a_serve_with_no_call_waiting_keeps_the_handlers_claim_waker() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    let (count, waker) = counting();
    handler.wake_on(Interest::Claim(IFACE), &waker);
    handler.serve(IFACE, &[ORD]).expect("serve");
    assert_eq!(wakes(&count), 0, "nothing is waiting");

    caller.command(IFACE, ORD, &[1], None).expect("send");
    assert_eq!(
        wakes(&count),
        1,
        "the waker is still stored, so the send wakes it"
    );
}

/// A waker that owns a handle of the runtime it is registered with. When the
/// store holds the last reference to it, dropping the waker drops the handle,
/// and the handle's own `Drop` takes the store's lock.
struct OwnsAHandle<H>(std::sync::Mutex<Option<H>>);

impl<H: Send + 'static> Wake for OwnsAHandle<H> {
    /// Waking reaches the owned handle and leaves it in place: what the tests
    /// below exercise is the drop of the last reference, not the wake.
    fn wake(self: Arc<Self>) {
        let owned = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        assert!(
            owned.is_some(),
            "the handle is owned until the waker is dropped"
        );
    }
}

fn owning<H: Send + 'static>(handle: H) -> Waker {
    Waker::from(Arc::new(OwnsAHandle(std::sync::Mutex::new(Some(handle)))))
}

/// Drops `handle` on another thread and fails, rather than hangs, when the
/// drop does not finish.
///
/// The thread is not joined, on purpose: a deadlocked drop never finishes,
/// and a join would wait for it forever instead of failing after the
/// timeout.
fn assert_drop_finishes<H: Send + 'static>(handle: H, what: &str) {
    let (done, finished) = mpsc::channel();
    std::thread::spawn(move || {
        drop(handle);
        let _ = done.send(());
    });
    assert_eq!(
        finished.recv_timeout(std::time::Duration::from_secs(5)),
        Ok(()),
        "{what}: the drop finished, so no waker was dropped under the store's lock"
    );
}

// Each of the four builds its own runtime and leaks it before the drop under
// test. Were the drop to deadlock, the assertion's panic would otherwise drop
// the runtime's own handles while the lock is still held, and hang the test
// instead of failing it.

#[test]
fn a_caller_whose_stored_waker_owns_another_handle_drops_without_deadlock() {
    let rt = runtime();
    let mut caller = rt.caller();
    fill(&mut caller);
    let waker = owning(rt.caller());
    caller.wake_on(Interest::Slot, &waker);
    drop(waker);
    std::mem::forget(rt);
    assert_drop_finishes(caller, "a caller");
}

#[test]
fn a_source_whose_stored_waker_owns_another_handle_drops_without_deadlock() {
    let rt = runtime();
    let source = rt.source();
    let waker = owning(rt.source());
    source.wake_on(Interest::Event(IFACE), &waker);
    drop(waker);
    std::mem::forget(rt);
    assert_drop_finishes(source, "a source");
}

#[test]
fn a_handler_whose_stored_waker_owns_another_handle_drops_without_deadlock() {
    let rt = runtime();
    let handler = rt.handler();
    let waker = owning(rt.handler());
    handler.wake_on(Interest::Claim(IFACE), &waker);
    drop(waker);
    std::mem::forget(rt);
    assert_drop_finishes(handler, "a handler");
}

#[test]
fn a_dropped_handler_interrupts_a_taken_call_whose_streams_moved() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut first = rt.handler();
    let mut second = rt.handler();
    let mut buf = [0u8; 8];
    let c = caller.query(IFACE, ORD, &[1], None).expect("send");
    let claim = first
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting")
        .id;
    let param = StreamNo(1);
    assert_eq!(caller.push(c, param, &[1]), Ok(1));
    assert_eq!(caller.push(c, param, &[2]), Ok(1));
    assert_eq!(first.pull_claim(claim, param, &mut buf), Ok(Pull::Chunk(1)));
    let (outcome, outcome_waker) = counting();
    let (reply, reply_waker) = counting();
    caller.wake_on(Interest::Outcome(c), &outcome_waker);
    caller.wake_on(Interest::Stream(c, REPLY_STREAM), &reply_waker);
    drop(first);

    assert_eq!(
        second.next_claim(&mut buf),
        Ok(None),
        "a call whose streams moved is not presented to another handler"
    );
    let interrupted = CallError::Transport(Transport::Interrupted);
    assert_eq!(
        wakes(&outcome),
        1,
        "the settlement wakes the outcome waiter"
    );
    assert_eq!(wakes(&reply), 1, "the settlement wakes the stream waiter");
    assert_eq!(caller.reply(c, &mut buf), Ok(Some(Err(interrupted))));
    assert_eq!(
        caller.push(c, param, &[3]),
        Err(PushError::Failed(interrupted)),
        "the parameter stream stopped with the handler"
    );
    assert_eq!(
        caller.pull(c, REPLY_STREAM, &mut buf),
        Ok(Pull::Failed(interrupted))
    );
}

#[test]
fn a_claim_stream_registration_refreshes_the_same_task_and_displaces_another() {
    let mut rt = runtime();
    let mut buf = [0u8; 8];
    let _c = rt.query(IFACE, ORD, &[1], None).expect("send");
    let claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting")
        .id;
    let key = Interest::ClaimStream(claim, StreamNo(1));
    let (first, first_waker) = counting();
    rt.wake_on(key, &first_waker);
    rt.wake_on(key, &first_waker);
    assert_eq!(
        wakes(&first),
        0,
        "a registration of the same task is a refresh"
    );
    let (second, second_waker) = counting();
    rt.wake_on(key, &second_waker);
    assert_eq!(
        wakes(&first),
        1,
        "another task's waker displaces the stored one"
    );
    assert_eq!(wakes(&second), 0, "the new waker is stored, not woken");
}

#[test]
fn a_parameter_stream_the_provider_refused_does_not_hold_the_slot_after_the_forget() {
    let mut rt = runtime();
    let mut buf = [0u8; 8];
    let param = StreamNo(1);
    let refusal = CallError::Contract(Contract::PreconditionFailed);
    for round in 0..Loopback::SLOTS * 2 {
        let c = rt
            .command(IFACE, ORD, &[1], None)
            .unwrap_or_else(|e| panic!("round {round}: the send answered {e:?}"));
        let claim = rt
            .next_claim(&mut buf)
            .expect("next_claim")
            .expect("waiting")
            .id;
        rt.settle(claim, Ok(&[])).expect("the acknowledgment");
        assert_eq!(rt.push(c, param, &[1]), Ok(1));
        assert_eq!(rt.pull_claim(claim, param, &mut buf), Ok(Pull::Chunk(1)));
        rt.abort_claim(claim, param, refusal).expect("the refusal");
        assert_eq!(rt.push(c, param, &[2]), Err(PushError::Failed(refusal)));
        rt.forget(c);
    }
}

#[test]
fn a_query_settled_with_an_unread_parameter_stream_frees_its_slot_at_the_forget() {
    let mut rt = runtime();
    let mut buf = [0u8; 8];
    let param = StreamNo(1);
    let old = rt.query(IFACE, ORD, &[1], None).expect("send");
    let claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting")
        .id;
    assert_eq!(rt.push(old, param, &[1]), Ok(1));
    let error = CallError::Contract(Contract::PreconditionFailed);
    rt.settle(claim, Err(error)).expect("settle");
    assert_eq!(
        rt.push(old, param, &[2]),
        Err(PushError::Failed(CallError::Transport(
            Transport::Interrupted
        ))),
        "the settlement of a query releases its parameter streams"
    );
    for _ in 1..Loopback::SLOTS {
        rt.command(IFACE, ORD, &[1], None).expect("a slot is free");
    }
    assert_eq!(
        rt.command(IFACE, ORD, &[1], None),
        Err(SendError::Busy),
        "every other slot is taken"
    );
    rt.forget(old);
    let new = rt
        .query(IFACE, ORD, &[2], None)
        .expect("the forget freed the old call's slot");
    assert_ne!(new, old);
}

#[test]
fn end_claim_after_the_callers_refusal_settles_the_claim_with_the_refusal() {
    let mut rt = runtime();
    let mut buf = [0u8; 8];
    let c = rt.query(IFACE, ORD, &[1], None).expect("send");
    let claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting")
        .id;
    let refusal = CallError::Contract(Contract::PreconditionFailed);
    rt.abort(c, REPLY_STREAM, refusal)
        .expect("the caller refuses");
    assert_eq!(
        rt.end_claim(claim, REPLY_STREAM),
        Err(PushError::Failed(refusal))
    );
    assert_eq!(
        rt.reply(c, &mut buf),
        Ok(Some(Err(refusal))),
        "the claim is settled with the refusal, not Ok"
    );
    assert_eq!(rt.settle(claim, Ok(&[])), Err(SettleError::UnknownClaim));
}

#[test]
fn a_dropped_handler_stops_the_parameter_streams_of_a_command_it_acknowledged() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    let mut buf = [0u8; 8];
    let param = StreamNo(1);
    let c = caller.command(IFACE, ORD, &[1], None).expect("send");
    let claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting")
        .id;
    handler.settle(claim, Ok(&[])).expect("the acknowledgment");
    assert_eq!(caller.push(c, param, &[1]), Ok(1));
    let (waiter, waiter_waker) = counting();
    caller.wake_on(Interest::Stream(c, param), &waiter_waker);
    drop(handler);
    assert_eq!(wakes(&waiter), 1, "the drop wakes the producer's waiter");
    assert_eq!(
        caller.push(c, param, &[2]),
        Err(PushError::Failed(CallError::Transport(
            Transport::Interrupted
        )))
    );
    assert_eq!(caller.ack(c), Some(Ok(())), "the acknowledgment stands");
}

#[test]
fn a_stream_registration_on_a_forgotten_call_is_woken_at_once() {
    let mut rt = runtime();
    let mut buf = [0u8; 8];
    let param = StreamNo(1);
    let c = rt.command(IFACE, ORD, &[1], None).expect("send");
    let claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting")
        .id;
    rt.settle(claim, Ok(&[])).expect("the acknowledgment");
    assert_eq!(rt.push(c, param, &[1]), Ok(1));
    rt.forget(c);
    let (waiter, waiter_waker) = counting();
    rt.wake_on(Interest::Stream(c, param), &waiter_waker);
    assert_eq!(wakes(&waiter), 1, "no stream of a forgotten call changes");
}

#[test]
fn a_querys_settlement_wakes_the_providers_waiter_on_a_parameter_stream() {
    let mut rt = runtime();
    let mut buf = [0u8; 8];
    let param = StreamNo(1);
    let _c = rt.query(IFACE, ORD, &[1], None).expect("send");
    let claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting")
        .id;
    let (waiter, waiter_waker) = counting();
    rt.wake_on(Interest::ClaimStream(claim, param), &waiter_waker);
    assert_eq!(rt.pull_claim(claim, param, &mut buf), Ok(Pull::Pending));
    assert_eq!(wakes(&waiter), 0, "nothing changed since the registration");
    rt.settle(
        claim,
        Err(CallError::Contract(Contract::PreconditionFailed)),
    )
    .expect("settle");
    assert_eq!(wakes(&waiter), 1, "the settlement releases the stream");
    assert_eq!(
        rt.pull_claim(claim, param, &mut buf),
        Ok(Pull::Failed(CallError::Transport(Transport::Interrupted)))
    );
}

#[test]
fn a_claim_stream_registration_on_a_finished_stream_is_woken_at_once() {
    let mut rt = runtime();
    let mut buf = [0u8; 8];
    let param = StreamNo(1);
    let c = rt.query(IFACE, ORD, &[1], None).expect("send");
    let claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting")
        .id;
    rt.end(c, param).expect("end");
    let (waiter, waiter_waker) = counting();
    rt.wake_on(Interest::ClaimStream(claim, param), &waiter_waker);
    assert_eq!(wakes(&waiter), 1, "an ended stream does not change again");
}

#[test]
fn a_stream_waker_whose_task_holds_a_handle_drops_without_deadlock() {
    let rt = runtime();
    let mut caller = rt.caller();
    let c = caller.query(IFACE, ORD, &[1], None).expect("send");
    let waker = owning(rt.caller());
    caller.wake_on(Interest::Stream(c, REPLY_STREAM), &waker);
    drop(waker);
    std::mem::forget(rt);
    // The drop forgets the call, which takes the stream waker, the last
    // reference to the handle it owns.
    assert_drop_finishes(caller, "a caller with a stream waker");
}

/// The next claim `handler` presents, with its arguments read.
fn take_claim(handler: &mut impl Handler) -> ClaimId {
    let mut buf = [0u8; 8];
    handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("a call is waiting")
        .id
}

/// Sends `Loopback::SLOTS` commands through `caller` and checks that each one
/// takes a slot: every slot of the table is free.
fn assert_every_slot_is_free(caller: &mut impl Caller, what: &str) {
    for round in 0..Loopback::SLOTS {
        if let Err(e) = caller.command(IFACE, ORD, &[1], None) {
            panic!("{what}: send {round} answered {e:?}");
        }
    }
}

#[test]
fn a_stream_parameter_first_named_after_a_querys_settlement_starts_released() {
    let mut rt = runtime();
    let interrupted = CallError::Transport(Transport::Interrupted);
    let c = rt.query(IFACE, ORD, &[1], None).expect("send");
    let claim = take_claim(&mut rt);
    rt.settle(claim, Ok(&[])).expect("settle");
    let param = StreamNo(1);
    assert_eq!(
        rt.push(c, param, &[2]),
        Err(PushError::Failed(interrupted)),
        "the settlement released the stream before anything named it"
    );
    assert_eq!(rt.end(c, param), Err(PushError::Failed(interrupted)));
    rt.forget(c);
    assert_every_slot_is_free(&mut rt, "after the forget");
}

#[test]
fn a_dropped_handler_completes_a_forget_deferred_on_an_acknowledged_command() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    let c = caller.command(IFACE, ORD, &[1], None).expect("send");
    let claim = take_claim(&mut handler);
    handler.settle(claim, Ok(&[])).expect("the acknowledgment");
    assert_eq!(caller.push(c, StreamNo(1), &[1]), Ok(1));
    caller.forget(c);
    drop(handler);
    let _other = rt.handler();
    assert_every_slot_is_free(&mut caller, "after the handler's drop");
}

#[test]
fn a_commands_refusal_releases_its_parameter_streams_and_the_forget_frees_its_slot() {
    let interrupted = CallError::Transport(Transport::Interrupted);
    let param = StreamNo(1);
    // A claim taken and settled with an error, and a claim offered through
    // `ShortClaim` and settled by its id with an error, as the generated
    // dispatch does when the arguments do not fit.
    for offered in [false, true] {
        let mut rt = runtime();
        let c = rt.command(IFACE, ORD, &[1, 2, 3], None).expect("send");
        assert_eq!(rt.push(c, param, &[1]), Ok(1));
        let claim = if offered {
            let mut short = [0u8; 1];
            match rt.next_claim(&mut short) {
                Err(ReadError::ShortClaim { claim, .. }) => claim,
                other => panic!("an offer, not {other:?}"),
            }
        } else {
            take_claim(&mut rt)
        };
        let error = if offered {
            CallError::Transport(Transport::Corrupt)
        } else {
            CallError::Contract(Contract::UnknownInteraction)
        };
        rt.settle(claim, Err(error)).expect("the refusal");
        assert_eq!(rt.ack(c), Some(Err(error)));
        assert_eq!(
            rt.push(c, param, &[2]),
            Err(PushError::Failed(interrupted)),
            "offered {offered}: the refusal released the stream"
        );
        rt.forget(c);
        assert_every_slot_is_free(&mut rt, "after the forget of a refused command");
    }
}

#[test]
fn a_forget_before_the_claim_withdraws_the_call_with_its_ended_stream() {
    let mut rt = runtime();
    rt.serve(IFACE, &[ORD]).expect("serve");
    let c = rt.command(IFACE, ORD, &[1], None).expect("send");
    let param = StreamNo(1);
    assert_eq!(rt.push(c, param, &[9]), Ok(1));
    rt.end(c, param).expect("end");
    rt.forget(c);
    let mut buf = [0u8; 8];
    assert_eq!(
        rt.next_claim(&mut buf),
        Ok(None),
        "no provider is presented a call withdrawn before its claim"
    );
    assert_every_slot_is_free(&mut rt, "after the withdrawal");
}

#[test]
fn a_claim_only_offered_has_no_streams_and_another_handler_reads_them_whole() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut first = rt.handler();
    let mut second = rt.handler();
    let param = StreamNo(1);
    let c = caller.query(IFACE, ORD, &[1, 2, 3], None).expect("send");
    assert_eq!(caller.push(c, param, &[1]), Ok(1));
    let mut short = [0u8; 1];
    let claim = match first.next_claim(&mut short) {
        Err(ReadError::ShortClaim { claim, .. }) => claim,
        other => panic!("an offer, not {other:?}"),
    };
    let unknown = Contract::UnknownInteraction;
    let mut buf = [0u8; 8];
    assert_eq!(
        first.pull_claim(claim, param, &mut buf),
        Err(ReadError::Contract(unknown)),
        "an offered claim is taken before its streams are read"
    );
    assert_eq!(
        first.push_claim(claim, REPLY_STREAM, &[7]),
        Err(PushError::Contract(unknown))
    );
    assert_eq!(
        first.end_claim(claim, REPLY_STREAM),
        Err(PushError::Contract(unknown))
    );
    drop(first);
    let taken = take_claim(&mut second);
    assert_eq!(taken, claim, "the call is presented under the same id");
    assert_eq!(
        second.pull_claim(taken, param, &mut buf),
        Ok(Pull::Chunk(1)),
        "the next holder reads the stream from its first chunk"
    );
    assert_eq!(&buf[..1], &[1]);
}

#[test]
fn a_dropped_handler_that_moved_no_stream_returns_the_call() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut first = rt.handler();
    let mut second = rt.handler();
    let param = StreamNo(1);
    let c = caller.query(IFACE, ORD, &[1], None).expect("send");
    let claim = take_claim(&mut first);
    let mut buf = [0u8; 8];
    assert_eq!(first.pull_claim(claim, param, &mut buf), Ok(Pull::Pending));
    let (waiter, waiter_waker) = counting();
    first.wake_on(Interest::ClaimStream(claim, param), &waiter_waker);
    drop(first);
    drop(waiter);
    assert_eq!(
        take_claim(&mut second),
        claim,
        "a Pending pull and a registration move nothing, so the call is presented again"
    );
    assert_eq!(
        caller.reply(c, &mut buf),
        Ok(None),
        "the call is not settled"
    );
}

#[test]
fn the_window_counts_only_the_non_empty_chunks() {
    let mut rt = runtime();
    let c = rt.query(IFACE, ORD, &[1], None).expect("send");
    let _claim = take_claim(&mut rt);
    let param = StreamNo(1);
    for _ in 0..Loopback::STREAM_WINDOW {
        assert_eq!(rt.push(c, param, &[]), Ok(0), "an empty chunk is taken");
    }
    for n in 0..Loopback::STREAM_WINDOW {
        assert_eq!(
            rt.push(c, param, &[1]),
            Ok(1),
            "non-empty chunk {n} fits the window beside the empty chunks"
        );
    }
    assert_eq!(rt.push(c, param, &[1]), Ok(0), "the window is full");
}

#[test]
fn abort_claim_after_the_callers_refusal_is_ended_and_settles_with_the_providers_error() {
    let mut rt = runtime();
    let mut buf = [0u8; 8];
    let c = rt.query(IFACE, ORD, &[1], None).expect("send");
    let claim = take_claim(&mut rt);
    let refusal = CallError::Contract(Contract::PreconditionFailed);
    let error = CallError::Transport(Transport::Corrupt);
    rt.abort(c, REPLY_STREAM, refusal)
        .expect("the caller refuses");
    assert_eq!(
        rt.abort_claim(claim, REPLY_STREAM, error),
        Err(PushError::Ended),
        "the reply stream had already stopped"
    );
    assert_eq!(
        rt.reply(c, &mut buf),
        Ok(Some(Err(error))),
        "the provider's abort is its verdict"
    );
}

#[test]
fn a_settle_ok_on_a_streamed_reply_ends_the_stream_and_reports_the_bytes() {
    let mut rt = runtime();
    let mut buf = [0u8; 8];
    let c = rt.query(IFACE, ORD, &[1], None).expect("send");
    let claim = take_claim(&mut rt);
    assert_eq!(rt.push_claim(claim, REPLY_STREAM, &[1]), Ok(1));
    rt.settle(claim, Ok(&[1, 2, 3])).expect("settle");
    assert_eq!(rt.reply(c, &mut buf), Ok(Some(Ok(3))));
    assert_eq!(rt.pull(c, REPLY_STREAM, &mut buf), Ok(Pull::Chunk(1)));
    assert_eq!(rt.pull(c, REPLY_STREAM, &mut buf), Ok(Pull::End));
}

#[test]
fn the_providers_push_after_its_own_abort_of_the_reply_stream_is_ended() {
    let mut rt = runtime();
    let _c = rt.query(IFACE, ORD, &[1], None).expect("send");
    let claim = take_claim(&mut rt);
    rt.abort_claim(
        claim,
        REPLY_STREAM,
        CallError::Transport(Transport::Interrupted),
    )
    .expect("abort_claim");
    assert_eq!(
        rt.push_claim(claim, REPLY_STREAM, &[1]),
        Err(PushError::Ended),
        "a stream its producer stopped is Ended for that producer"
    );
}

#[test]
fn a_deferred_forget_answers_none_from_ack_and_reply_while_the_slot_is_held() {
    let mut rt = runtime();
    let mut buf = [0u8; 8];
    let c = rt.command(IFACE, ORD, &[1], None).expect("send");
    let claim = take_claim(&mut rt);
    rt.settle(claim, Ok(&[])).expect("the acknowledgment");
    assert_eq!(rt.push(c, StreamNo(1), &[1]), Ok(1));
    rt.forget(c);
    assert_eq!(rt.ack(c), None, "a forgotten correlation has no outcome");
    assert_eq!(rt.reply(c, &mut buf), Ok(None));
    for _ in 1..Loopback::SLOTS {
        rt.command(IFACE, ORD, &[1], None).expect("a slot is free");
    }
    assert_eq!(
        rt.command(IFACE, ORD, &[1], None),
        Err(SendError::Busy),
        "the forgotten call still holds its slot"
    );
}

#[test]
fn a_dropped_handlers_interruption_releases_a_stream_parameter_not_yet_named() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    let mut buf = [0u8; 8];
    let c = caller.query(IFACE, ORD, &[1], None).expect("send");
    let claim = take_claim(&mut handler);
    assert_eq!(caller.push(c, StreamNo(1), &[1]), Ok(1));
    assert_eq!(
        handler.pull_claim(claim, StreamNo(1), &mut buf),
        Ok(Pull::Chunk(1))
    );
    let later = StreamNo(2);
    let (waiter, waiter_waker) = counting();
    caller.wake_on(Interest::Stream(c, later), &waiter_waker);
    drop(handler);
    assert_eq!(
        wakes(&waiter),
        1,
        "the interruption wakes the waiter on a stream nothing named yet"
    );
    let interrupted = CallError::Transport(Transport::Interrupted);
    assert_eq!(
        caller.push(c, later, &[2]),
        Err(PushError::Failed(interrupted)),
        "a stream parameter first named after the interruption starts stopped"
    );
    assert_eq!(caller.end(c, later), Err(PushError::Failed(interrupted)));
    assert_eq!(caller.reply(c, &mut buf), Ok(Some(Err(interrupted))));
}

#[test]
fn a_dropped_handler_releases_a_stream_parameter_of_an_acknowledged_command_not_yet_named() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    let param = StreamNo(1);
    let c = caller.command(IFACE, ORD, &[1], None).expect("send");
    let claim = take_claim(&mut handler);
    handler.settle(claim, Ok(&[])).expect("the acknowledgment");
    let (waiter, waiter_waker) = counting();
    caller.wake_on(Interest::Stream(c, param), &waiter_waker);
    drop(handler);
    assert_eq!(
        wakes(&waiter),
        1,
        "the drop wakes the waiter on a stream nothing named yet"
    );
    let interrupted = CallError::Transport(Transport::Interrupted);
    assert_eq!(
        caller.push(c, param, &[1]),
        Err(PushError::Failed(interrupted)),
        "no provider is left to read a stream parameter first named after the drop"
    );
    assert_eq!(caller.end(c, param), Err(PushError::Failed(interrupted)));
    assert_eq!(caller.ack(c), Some(Ok(())), "the acknowledgment stands");
}

/// Each way the provider is done with a stream parameter completes a forget
/// deferred on it, and the slot comes back: pulling `End`, pulling `Failed`,
/// aborting the interrupted stream, aborting the ended stream, and the
/// settlement of a query. A forget stops every stream that was not ended, so
/// after it the provider's abort always finds a finished stream and answers
/// `Ended`. Every other slot is taken by a call in flight, so the slot is
/// shown held before the provider is done and free after.
#[test]
fn a_deferred_forget_frees_the_slot_once_the_provider_is_done_with_the_parameter_stream() {
    let param = StreamNo(1);
    let error = CallError::Contract(Contract::PreconditionFailed);
    let interrupted = CallError::Transport(Transport::Interrupted);
    for done in 0..5 {
        let mut rt = runtime();
        let mut buf = [0u8; 8];
        let (c, claim) = if done == 4 {
            let c = rt.query(IFACE, ORD, &[1], None).expect("send");
            (c, take_claim(&mut rt))
        } else {
            let c = rt.command(IFACE, ORD, &[1], None).expect("send");
            let claim = take_claim(&mut rt);
            rt.settle(claim, Ok(&[])).expect("the acknowledgment");
            (c, claim)
        };
        assert_eq!(rt.push(c, param, &[1]), Ok(1));
        if done == 0 || done == 3 {
            rt.end(c, param).expect("end");
        }
        rt.forget(c);
        for _ in 1..Loopback::SLOTS {
            rt.command(IFACE, ORD, &[1], None).expect("a slot is free");
        }
        assert_eq!(
            rt.command(IFACE, ORD, &[1], None),
            Err(SendError::Busy),
            "case {done}: the forgotten call holds its slot while the provider has its stream to read"
        );
        match done {
            0 => {
                assert_eq!(rt.pull_claim(claim, param, &mut buf), Ok(Pull::Chunk(1)));
                assert_eq!(rt.pull_claim(claim, param, &mut buf), Ok(Pull::End));
            }
            1 => {
                assert_eq!(rt.pull_claim(claim, param, &mut buf), Ok(Pull::Chunk(1)));
                assert_eq!(
                    rt.pull_claim(claim, param, &mut buf),
                    Ok(Pull::Failed(interrupted))
                );
            }
            2 => assert_eq!(
                rt.abort_claim(claim, param, error),
                Err(PushError::Ended),
                "the abort of a stream the forget interrupted"
            ),
            3 => assert_eq!(
                rt.abort_claim(claim, param, error),
                Err(PushError::Ended),
                "the abort of an ended stream"
            ),
            _ => rt.settle(claim, Ok(&[])).expect("the query's settlement"),
        }
        rt.command(IFACE, ORD, &[1], None)
            .unwrap_or_else(|e| panic!("case {done}: the slot did not come back: {e:?}"));
    }
}

#[test]
fn a_producers_end_and_abort_wake_the_receiver_and_a_receivers_refusal_wakes_the_producer() {
    let mut rt = runtime();
    let c = rt.query(IFACE, ORD, &[1], None).expect("send");
    let claim = take_claim(&mut rt);
    let error = CallError::Contract(Contract::PreconditionFailed);

    let (ended, ended_waker) = counting();
    rt.wake_on(Interest::ClaimStream(claim, StreamNo(1)), &ended_waker);
    assert_eq!(wakes(&ended), 0, "nothing changed since the registration");
    rt.end(c, StreamNo(1)).expect("end");
    assert_eq!(
        wakes(&ended),
        1,
        "the caller's end wakes the provider's waiter"
    );

    let (aborted, aborted_waker) = counting();
    rt.wake_on(Interest::ClaimStream(claim, StreamNo(2)), &aborted_waker);
    assert_eq!(wakes(&aborted), 0, "nothing changed since the registration");
    rt.abort(c, StreamNo(2), error).expect("abort");
    assert_eq!(
        wakes(&aborted),
        1,
        "the caller's abort wakes the provider's waiter"
    );

    let (refused, refused_waker) = counting();
    rt.wake_on(Interest::Stream(c, StreamNo(3)), &refused_waker);
    assert_eq!(wakes(&refused), 0, "nothing changed since the registration");
    rt.abort_claim(claim, StreamNo(3), error)
        .expect("the provider refuses");
    assert_eq!(
        wakes(&refused),
        1,
        "the provider's refusal wakes the caller's waiter"
    );
}

/// Each operation that moves a stream marks the call, so that the handler's
/// drop interrupts it instead of presenting it again: a push, a pull that
/// answers `End`, a pull that answers `Failed`, and an abort. The pull of a
/// chunk is pinned by
/// `a_dropped_handler_interrupts_a_taken_call_whose_streams_moved`.
#[test]
fn a_dropped_handler_interrupts_a_call_on_which_it_pushed_pulled_an_end_or_aborted() {
    let param = StreamNo(1);
    let error = CallError::Contract(Contract::PreconditionFailed);
    let interrupted = CallError::Transport(Transport::Interrupted);
    for moved in 0..4 {
        let rt = runtime();
        let mut caller = rt.caller();
        let mut first = rt.handler();
        let mut second = rt.handler();
        let mut buf = [0u8; 8];
        let c = caller.query(IFACE, ORD, &[1], None).expect("send");
        let claim = take_claim(&mut first);
        match moved {
            0 => assert_eq!(first.push_claim(claim, REPLY_STREAM, &[1]), Ok(1)),
            1 => {
                caller.end(c, param).expect("end");
                assert_eq!(first.pull_claim(claim, param, &mut buf), Ok(Pull::End));
            }
            2 => {
                caller.abort(c, param, error).expect("abort");
                assert_eq!(
                    first.pull_claim(claim, param, &mut buf),
                    Ok(Pull::Failed(error))
                );
            }
            _ => first
                .abort_claim(claim, param, error)
                .expect("the provider refuses"),
        }
        drop(first);
        assert_eq!(
            second.next_claim(&mut buf),
            Ok(None),
            "case {moved}: a call whose streams moved is not presented again"
        );
        assert_eq!(
            caller.reply(c, &mut buf),
            Ok(Some(Err(interrupted))),
            "case {moved}: the call settles with the interruption"
        );
    }
}

#[test]
fn a_forget_wakes_the_stream_waiters_of_a_call_with_no_stream_parameter() {
    let mut rt = runtime();
    let c = rt.query(IFACE, ORD, &[1], None).expect("send");
    let _claim = take_claim(&mut rt);
    let (reply, reply_waker) = counting();
    rt.wake_on(Interest::Stream(c, REPLY_STREAM), &reply_waker);
    assert_eq!(wakes(&reply), 0, "nothing changed since the registration");
    rt.forget(c);
    assert_eq!(
        wakes(&reply),
        1,
        "the forget of a call in flight wakes its reply stream waiter"
    );

    let c = rt.command(IFACE, ORD, &[1], None).expect("send");
    let claim = take_claim(&mut rt);
    rt.settle(claim, Ok(&[])).expect("the acknowledgment");
    let (param, param_waker) = counting();
    rt.wake_on(Interest::Stream(c, StreamNo(1)), &param_waker);
    assert_eq!(
        wakes(&param),
        0,
        "a settled command's stream parameter goes on, so the waker is stored"
    );
    rt.forget(c);
    assert_eq!(
        wakes(&param),
        1,
        "the forget that reclaims the slot wakes its stream waiter"
    );
}

#[test]
fn a_forgotten_unacknowledged_commands_ended_stream_is_read_after_the_acknowledgment() {
    let mut rt = runtime();
    let mut buf = [0u8; 8];
    let param = StreamNo(1);
    let c = rt.command(IFACE, ORD, &[1], None).expect("send");
    let claim = take_claim(&mut rt);
    assert_eq!(rt.push(c, param, &[1]), Ok(1));
    rt.end(c, param).expect("end");
    rt.forget(c);
    rt.settle(claim, Ok(&[]))
        .expect("the acknowledgment of a forgotten call");
    assert_eq!(
        rt.pull_claim(claim, param, &mut buf),
        Ok(Pull::Chunk(1)),
        "the ended stream is kept for the provider across the acknowledgment"
    );
    assert_eq!(&buf[..1], &[1]);
    assert_eq!(rt.pull_claim(claim, param, &mut buf), Ok(Pull::End));
    assert_every_slot_is_free(&mut rt, "after the provider read the stream to its end");
}

#[test]
fn a_second_forget_of_a_deferred_call_leaves_its_slot_held() {
    let mut rt = runtime();
    let mut buf = [0u8; 8];
    let param = StreamNo(1);
    let c = rt.command(IFACE, ORD, &[1], None).expect("send");
    let claim = take_claim(&mut rt);
    rt.settle(claim, Ok(&[])).expect("the acknowledgment");
    assert_eq!(rt.push(c, param, &[1]), Ok(1));
    rt.forget(c);
    rt.forget(c);
    for _ in 1..Loopback::SLOTS {
        rt.command(IFACE, ORD, &[1], None).expect("a slot is free");
    }
    assert_eq!(
        rt.command(IFACE, ORD, &[1], None),
        Err(SendError::Busy),
        "the slot is held until the provider reads the stream, however often the call is forgotten"
    );
    assert_eq!(rt.pull_claim(claim, param, &mut buf), Ok(Pull::Chunk(1)));
    assert_eq!(
        rt.pull_claim(claim, param, &mut buf),
        Ok(Pull::Failed(CallError::Transport(Transport::Interrupted)))
    );
    rt.command(IFACE, ORD, &[1], None)
        .expect("the slot came back once the provider was done");
}

#[test]
fn a_deferred_forget_wakes_the_outcome_waiter_and_a_later_registration_is_woken_at_once() {
    let mut rt = runtime();
    let c = rt.query(IFACE, ORD, &[1], None).expect("send");
    let _claim = take_claim(&mut rt);
    assert_eq!(rt.push(c, StreamNo(1), &[1]), Ok(1));
    let (outcome, outcome_waker) = counting();
    rt.wake_on(Interest::Outcome(c), &outcome_waker);
    assert_eq!(wakes(&outcome), 0, "nothing changed since the registration");
    rt.forget(c);
    assert_eq!(
        wakes(&outcome),
        1,
        "the deferred forget wakes the outcome waiter"
    );
    let (late, late_waker) = counting();
    rt.wake_on(Interest::Outcome(c), &late_waker);
    assert_eq!(
        wakes(&late),
        1,
        "no outcome will be readable for a forgotten call whose slot is still held"
    );
}

#[test]
fn a_providers_push_and_end_on_the_reply_stream_after_a_deferred_forget_are_interrupted() {
    let mut rt = runtime();
    let c = rt.command(IFACE, ORD, &[1], None).expect("send");
    let claim = take_claim(&mut rt);
    rt.settle(claim, Ok(&[])).expect("the acknowledgment");
    assert_eq!(rt.push(c, StreamNo(1), &[1]), Ok(1));
    rt.forget(c);
    let interrupted = CallError::Transport(Transport::Interrupted);
    assert_eq!(
        rt.push_claim(claim, REPLY_STREAM, &[1]),
        Err(PushError::Failed(interrupted)),
        "the forget answers before the ended reply stream does"
    );
    assert_eq!(
        rt.end_claim(claim, REPLY_STREAM),
        Err(PushError::Failed(interrupted))
    );
}

#[test]
fn a_settlement_refused_through_end_claim_answers_too_large() {
    let mut rt = runtime();
    let _c = rt.query(IFACE, ORD, &[1], None).expect("send");
    let claim = take_claim(&mut rt);
    rt.fail_next_settle();
    assert_eq!(
        rt.end_claim(claim, REPLY_STREAM),
        Err(PushError::TooLarge { cap: 0 }),
        "the injected settlement failure is reported as the push error of its kind"
    );
}

#[test]
fn a_dropped_handlers_claim_stream_waiter_is_woken() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    let _c = caller.query(IFACE, ORD, &[1], None).expect("send");
    let claim = take_claim(&mut handler);
    let (waiter, waiter_waker) = counting();
    handler.wake_on(Interest::ClaimStream(claim, StreamNo(1)), &waiter_waker);
    assert_eq!(wakes(&waiter), 0, "nothing changed since the registration");
    drop(handler);
    assert_eq!(
        wakes(&waiter),
        1,
        "the dropped handler's claim stream waiter is woken: its claim is gone"
    );
}

#[test]
fn a_handler_whose_stored_claim_stream_waker_owns_another_handle_drops_without_deadlock() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    let _c = caller.query(IFACE, ORD, &[1], None).expect("send");
    let claim = take_claim(&mut handler);
    let waker = owning(rt.handler());
    handler.wake_on(Interest::ClaimStream(claim, StreamNo(1)), &waker);
    drop(waker);
    std::mem::forget(rt);
    std::mem::forget(caller);
    // The drop takes the claim's stream waker, the last reference to the
    // handle it owns, and drops it after the lock is released.
    assert_drop_finishes(handler, "a handler with a claim stream waker");
}

#[test]
fn the_aggregate_routes_each_key_to_the_handle_that_observes_it() {
    let mut rt = runtime();
    let mut buf = [0u8; 8];

    let c = rt.command(IFACE, ORD, &[1], None).expect("send");
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
    rt.raise(IFACE, ORD, &[1], None).expect("raise");
    assert_eq!(wakes(&event), 1, "woken by the raise");

    let (claim, claim_waker) = counting();
    rt.wake_on(Interest::Claim(IFACE), &claim_waker);
    assert_eq!(wakes(&claim), 0, "stored, not woken at once");
    rt.caller().command(IFACE, ORD, &[1], None).expect("send");
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
    sink.raise(IFACE, ORD, &[1], None).expect("raise");
    assert_released(&rx, "a raise");

    let (waker, rx) = lock_probe(&rt);
    source.wake_on(Interest::Event(IFACE), &waker);
    assert_released(&rx, "a registration whose key already holds");

    let (waker, rx) = lock_probe(&rt);
    first.wake_on(Interest::Claim(IFACE), &waker);
    caller.command(IFACE, ORD, &[1], None).expect("send");
    assert_released(&rx, "a command");

    first
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    let (waker, rx) = lock_probe(&rt);
    first.wake_on(Interest::Claim(IFACE), &waker);
    let c = caller.query(IFACE, ORD, &[1], None).expect("send");
    assert_released(&rx, "a query");

    let (waker, rx) = lock_probe(&rt);
    caller.wake_on(Interest::Outcome(c), &waker);
    caller.forget(c);
    assert_released(&rx, "a forget");

    // No handler had claimed the query, so the forget withdrew it. Another
    // query waits for the second handler's serve.
    caller.query(IFACE, ORD, &[1], None).expect("send");
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

    // Fill the table, then reclaim a slot three ways: a forget of a settled
    // call, the settlement of a claimed call that was forgotten, and a
    // forget that withdraws a waiting call.
    let claim = first
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("the returned query");
    first.settle(claim.id, Ok(&[])).expect("settle");
    let mut sent = Vec::new();
    while let Ok(c) = caller.command(IFACE, ORD, &[2], None) {
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
        .command(IFACE, ORD, &[3], None)
        .expect("the reclaimed slot");
    let claim = first
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("the second command");
    caller.forget(sent[1]);
    let (waker, rx) = lock_probe(&rt);
    caller.wake_on(Interest::Slot, &waker);
    first.settle(claim.id, Ok(&[])).expect("settle");
    assert_released(&rx, "a settlement that reclaims a slot");

    caller
        .command(IFACE, ORD, &[4], None)
        .expect("the reclaimed slot");
    let (waker, rx) = lock_probe(&rt);
    caller.wake_on(Interest::Slot, &waker);
    caller.forget(sent[2]);
    assert_released(&rx, "a forget that withdraws an unclaimed call");
}

#[test]
fn every_wake_at_once_and_every_displacement_is_run_with_the_lock_released() {
    let rt = runtime();
    let mut caller = rt.caller();
    let mut handler = rt.handler();
    let mut source = rt.source();
    source.subscribe(IFACE, &[ORD]).expect("subscribe");
    handler.serve(IFACE, &[ORD]).expect("serve");
    let mut buf = [0u8; 8];

    // A registration whose key already holds: a waiting call for the
    // handler, a free slot, a settled call.
    let c = caller.command(IFACE, ORD, &[1], None).expect("send");
    let (waker, rx) = lock_probe(&rt);
    handler.wake_on(Interest::Claim(IFACE), &waker);
    assert_released(&rx, "a claim registration while a call is waiting");

    let (waker, rx) = lock_probe(&rt);
    caller.wake_on(Interest::Slot, &waker);
    assert_released(&rx, "a slot registration while a slot is free");

    let claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    handler.settle(claim.id, Ok(&[])).expect("settle");
    let (waker, rx) = lock_probe(&rt);
    caller.wake_on(Interest::Outcome(c), &waker);
    assert_released(&rx, "an outcome registration after the settlement");

    // A correlation that names no call in flight.
    caller.forget(c);
    let (waker, rx) = lock_probe(&rt);
    caller.wake_on(Interest::Outcome(c), &waker);
    assert_released(&rx, "an outcome registration on a forgotten call");

    // A kind no role of the handle observes.
    let (waker, rx) = lock_probe(&rt);
    source.wake_on(Interest::Slot, &waker);
    assert_released(
        &rx,
        "a registration under a kind the handle does not observe",
    );

    // A waker of another task displaces the stored one and wakes it. Nothing
    // is waiting, so each first registration is stored.
    let (first, first_rx) = lock_probe(&rt);
    let (second, _second_rx) = lock_probe(&rt);
    source.wake_on(Interest::Event(IFACE), &first);
    source.wake_on(Interest::Event(IFACE), &second);
    assert_released(&first_rx, "a displaced event waker");

    let (first, first_rx) = lock_probe(&rt);
    let (second, _second_rx) = lock_probe(&rt);
    handler.wake_on(Interest::Claim(IFACE), &first);
    handler.wake_on(Interest::Claim(IFACE), &second);
    assert_released(&first_rx, "a displaced claim waker");

    let c = caller.command(IFACE, ORD, &[2], None).expect("send");
    let (first, first_rx) = lock_probe(&rt);
    let (second, _second_rx) = lock_probe(&rt);
    caller.wake_on(Interest::Outcome(c), &first);
    caller.wake_on(Interest::Outcome(c), &second);
    assert_released(&first_rx, "a displaced outcome waker");

    // With the table full, slot registrations are stored.
    while caller.command(IFACE, ORD, &[3], None).is_ok() {}
    let (first, first_rx) = lock_probe(&rt);
    let (second, _second_rx) = lock_probe(&rt);
    caller.wake_on(Interest::Slot, &first);
    caller.wake_on(Interest::Slot, &second);
    assert_released(&first_rx, "a displaced slot waker");
}

/// The `.rs` files under `dir`, read into one string each.
fn rust_sources(dir: &std::path::Path, found: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).expect("a readable directory") {
        let path = entry.expect("a directory entry").path();
        if path.is_dir() {
            rust_sources(&path, found);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push(std::fs::read_to_string(&path).expect("a readable source"));
        }
    }
}

/// The backticked names in `text` that have four or more words, which is how
/// a test is cited. A name with fewer words is a method or a type.
fn cited_test_names(text: &str) -> Vec<String> {
    text.split('`')
        .skip(1)
        .step_by(2)
        .filter(|span| {
            span.starts_with(|c: char| c.is_ascii_lowercase())
                && span
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
                && span.matches('_').count() >= 3
        })
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_records_cite_only_tests_that_exist_and_the_suite_keeps_the_list_they_cite() {
    // The design record and the conformance crate are not in the packaged
    // crate, so this check runs in the workspace only.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let record = root.join("docs/design/ridl-loopback.md");
    let suite = root.join("crates/ridl-rt-conformance/src/lib.rs");
    if !record.is_file() || !suite.is_file() {
        return;
    }
    let mut sources = Vec::new();
    rust_sources(&root.join("crates"), &mut sources);

    let documentation: String = include_str!("ports.rs")
        .lines()
        .filter(|line| line.starts_with("//!"))
        .collect::<Vec<_>>()
        .join("\n");
    let record = std::fs::read_to_string(record).expect("the design record");
    for (cited_in, text) in [("ports.rs", documentation), ("ridl-loopback.md", record)] {
        let names = cited_test_names(&text);
        assert!(
            names.len() > 10,
            "{cited_in}: the scan found the cited tests"
        );
        for name in names {
            assert!(
                sources.iter().any(|source| {
                    source.contains(&format!("fn {name}("))
                        || source.contains(&format!("fn {name}<"))
                }),
                "{cited_in} cites `{name}`, and no function has that name"
            );
        }
    }

    // `ports.rs` sends the reader to this list by its title.
    let suite = std::fs::read_to_string(suite).expect("the conformance crate");
    assert!(
        suite.contains("# What the suite leaves out"),
        "ports.rs cites the list \"What the suite leaves out\", which the \
         conformance crate's documentation no longer has"
    );
}
