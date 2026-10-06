//! `ridl-rt-conformance` — the port contract tests, generic over a runtime.
//!
//! Each public function in this crate's modules is one test of what
//! [`ridl_rt::port`](https://docs.rs/ridl-rt) states a runtime does behind a
//! port. The functions are generic over a [`Factory`], the one thing a runtime
//! crate writes to run them: how to build a runtime, how to get a second
//! handle of three port roles on it, the size of its call table
//! ([`Factory::SLOTS`]), and two hooks the port traits do not offer. A
//! runtime runs the whole suite from its own test suite with the [`suite!`]
//! macro, which writes one `#[test]` per function:
//!
//! ```ignore
//! struct MyFactory;
//!
//! impl ridl_rt_conformance::Factory for MyFactory {
//!     // ...
//! }
//!
//! // Every test of the ports every runtime presents, and those of the three
//! // extensions this runtime implements.
//! ridl_rt_conformance::suite!(MyFactory; scannable, coherent, wakeable);
//! ```
//!
//! `ridl-loopback` is the first runtime to run it
//! (`crates/ridl-loopback/tests/conformance.rs`). The tests came from that
//! crate's `tests/ports.rs` (driftsys/ridl#514), which keeps
//! the tests only the loopback can express, each listed with its reason in
//! that file's module documentation.
//!
//! # What the suite leaves out
//!
//! This is the one list of what the suite does not test. A runtime that
//! wants one of these pinned keeps its own test of it;
//! `crates/ridl-loopback/tests/ports.rs` names the loopback's tests and the
//! reason each one stays, which is an item of this list or a choice that is
//! the loopback's alone.
//!
//! - **What the port contract leaves to a runtime.** Where the clock starts,
//!   what [`Factory::advance`] does with a negative duration, and which error
//!   the fault of [`Factory::fail_next_settle`] reports beyond not being
//!   `UnknownClaim`.
//! - **A handler that has served nothing.** [`Handler::serve`] starts
//!   presentation at the members listed, and every handler in the suite calls
//!   it before it takes a claim. `ridl-loopback` deviates from that on
//!   purpose and presents every call to such a handler, because the generated
//!   `dispatch` never calls `serve`, so the suite has no test of the case.
//! - **Two event sinks on one event channel.** An event channel has one
//!   provider (ridl §5). A runtime may refuse the second sink or, as
//!   `ridl-loopback` does, not police the misuse.
//! - **`FixedReader`.** How a `fixed` is provisioned into a runtime is the
//!   runtime's own API, not a port, and what an unprovisioned `fixed` reads
//!   as is not fixed by the port contract.
//! - **Anything a runtime reports from a catalog descriptor**: an unknown
//!   ordinal, an unowned member, a freshness of `Fresh` or `Stale` measured
//!   against a staleness bound. The suite attaches to a catalog with no
//!   descriptor, and a runtime with none reports `Freshness::Unbounded`.
//! - **The threading model.** ADR-0021 decision 12 holds a runtime whose
//!   handles are used from more than one thread to `Send` and `Sync` bounds,
//!   and a single-threaded runtime to neither, so no test here moves a handle
//!   to another thread. The one test that runs on a thread of its own,
//!   `wakeable::every_wake_runs_with_the_runtime_lock_released`, builds its
//!   runtime on that thread.
//! - **A wake the contract allows but does not require.** A runtime may wake
//!   a waiter spuriously, for a change that is not its key's, so a test
//!   asserts that a change leaves a waker unwoken only where the contract
//!   rules the wake out: a refresh, which wakes nothing, and a waker already
//!   woken or displaced, which the contract clears. Whether a registration
//!   whose key already holds is woken at once is the runtime's too: the task
//!   reads the port after it registers, and finds the change either way. The
//!   tests assume a change made through a port is visible, and its waker
//!   woken, before that port call returns, on the thread that made it.
//! - **Which serving handlers a call wakes.** Each source subscribed to an
//!   occurrence is woken by it; a call is presented to one handler, and the
//!   suite asserts only that a handler serving the member is woken.
//! - **What a forget does to a call no handler has claimed.** A runtime may
//!   withdraw it, as `ridl-loopback` does, or hold it until it is presented
//!   and settled, because a transport that has already sent a request cannot
//!   recall it. The suite accepts either, and asserts only that the call's
//!   slot is back once it is withdrawn or settled. It does not assert when a
//!   forgotten call that a handler has claimed gives its slot back, only
//!   that it is back once the call is settled.
//! - **The drop of a handle.** No test drops a handle to observe the
//!   effect: what becomes of a dropped handler's claims or a dropped
//!   caller's calls, and of their waiters, is not tested here.
//! - **A runtime's own API beyond the factory**, such as how it hands out all
//!   of its role handles at once (`Loopback::split`) or reports a handler's
//!   served set (`HandlerHandle::served`).

use ridl_rt::contract::{CatalogHash, CatalogRef, InterfaceNo, Ordinal};
use ridl_rt::port::{
    Attached, Caller, Changed, Clock, Correlation, EventSink, EventSource, Handler, RawSample,
    SignalReader, SignalWriter,
};
use ridl_rt::sample::{Duration, Envelope, Freshness, Provenance, Timestamp};

pub mod attached;
pub mod calls;
pub mod clock;
pub mod coherent;
pub mod events;
pub mod scannable;
pub mod signals;
pub mod wakeable;

/// What the suite needs from a runtime beyond its port traits.
///
/// A runtime crate implements this on a type of its own test crate — a unit
/// struct is enough — because the orphan rule forbids implementing it on the
/// runtime's own type from a test crate outside the runtime. Every method is
/// an associated function: the factory holds no state, and every runtime it
/// builds is independent of every other.
pub trait Factory {
    /// The runtime the suite drives: one value implementing every port trait
    /// the suite calls. Under ADR-0021 decision 12 that is an aggregate
    /// handle, one the runtime offers or one its test crate writes over the
    /// role handles.
    ///
    /// The three extensions — the two signal extensions and `Wakeable` — are
    /// not in this bound, because a runtime may omit them; the tests of each
    /// ask for it where they need it, on this type and on the role handles
    /// below.
    type Runtime: Attached
        + Clock
        + SignalReader
        + SignalWriter
        + EventSource
        + EventSink
        + Caller
        + Handler;

    /// A second event source on a runtime, with its own subscriptions and its
    /// own queue.
    type Source: EventSource;

    /// A second caller on a runtime, with its own sequence counter.
    type Caller: Caller;

    /// A second handler on a runtime, with its own served set and its own
    /// claims.
    type Handler: Handler;

    /// The number of calls a runtime holds at once: the size of its call
    /// table. It counts the calls sent through the runtime and through every
    /// caller [`caller`](Factory::caller) makes on it, because the table is
    /// the runtime's and not a caller's. A call holds its slot from its send
    /// until it is reclaimed by [`Caller::forget`], at once for a settled
    /// call and at the settlement for a call in flight (ADR-0021 decision
    /// 15), and a send with every slot held is refused with
    /// [`SendError::Busy`](ridl_rt::port::SendError::Busy). At least 1.
    ///
    /// The tests fill the table by sending this many calls, so a runtime
    /// whose bound is large runs them more slowly, and one with no bound
    /// cannot run them.
    const SLOTS: usize;

    /// A new runtime attached to `catalog`, with nothing published, raised or
    /// sent.
    ///
    /// Its clock moves only when [`advance`](Factory::advance) moves it: it
    /// never reads wall-clock time. Where it starts is the runtime's choice.
    fn runtime(catalog: CatalogRef) -> Self::Runtime;

    /// A new event source on `runtime`, attached to the same catalog. An
    /// occurrence raised through `runtime` reaches it once it subscribes.
    fn source(runtime: &Self::Runtime) -> Self::Source;

    /// A new caller on `runtime`, attached to the same catalog. Its calls are
    /// presented to the handlers of `runtime`.
    fn caller(runtime: &Self::Runtime) -> Self::Caller;

    /// A new handler on `runtime`, attached to the same catalog. It is
    /// presented the calls to the members it serves.
    fn handler(runtime: &Self::Runtime) -> Self::Handler;

    /// Advances the clock of `runtime` by `by`, which the suite never passes
    /// negative. After it, [`Clock::now`] reads exactly `by` later than
    /// before.
    fn advance(runtime: &mut Self::Runtime, by: Duration);

    /// Makes the next [`Handler::settle`] through `runtime` of a claim
    /// `runtime` holds fail, with an error other than
    /// [`SettleError::UnknownClaim`](ridl_rt::port::SettleError::UnknownClaim),
    /// and record no outcome. A `settle` of a claim `runtime` does not hold
    /// still fails with `UnknownClaim`, and does not spend the fault. The
    /// fault is injected once: the settlement after the failed one succeeds.
    fn fail_next_settle(runtime: &mut Self::Runtime);
}

/// Writes one `#[test]` for each test of the suite, over the [`Factory`]
/// named first.
///
/// `suite!(F)` writes the tests of the ports every runtime presents. Naming
/// an extension after a semicolon adds its tests: `scannable` and
/// `coherent`, the two signal extensions, and `wakeable`, in any
/// combination, as in `suite!(F; scannable, coherent, wakeable)`. Each
/// requires what that extension's tests ask for: `scannable` and `coherent`
/// that `F::Runtime` implements the extension named, and `wakeable` that
/// `F::Runtime` and the three role handles `F` makes implement `Wakeable`,
/// and that `F` and those handles are `'static`, because one test runs on a
/// thread of its own. Each test is named after the function it calls, so a
/// failure names the contract it breaks.
#[macro_export]
macro_rules! suite {
    ($factory:ty) => {
        $crate::suite!(@tests $factory;
            attached::the_catalog_is_the_one_the_runtime_was_built_with,
            clock::the_clock_is_hand_driven_not_wall_clock,
            signals::a_signal_publish_and_read_round_trips,
            signals::a_signal_with_no_publication_reads_as_init_and_copies_nothing,
            signals::a_staged_value_is_not_visible_until_commit,
            signals::one_commit_publishes_every_staged_signal_under_one_timestamp,
            signals::a_channel_sequence_number_counts_that_channel_publications,
            signals::invalidate_keeps_the_last_good_value_and_reports_the_declared_cause,
            signals::touch_republishes_the_current_value_without_changing_it,
            signals::a_touch_does_not_discard_a_value_staged_before_it,
            signals::a_touch_does_not_discard_an_invalidation_staged_before_it,
            signals::a_set_after_a_touch_replaces_it,
            signals::touch_on_a_channel_with_no_publication_publishes_nothing,
            signals::a_short_buffer_reports_what_the_read_needs_and_consumes_nothing,
            events::an_event_raise_and_receive_round_trips,
            events::an_occurrence_raised_before_the_subscription_is_not_delivered,
            events::unsubscribe_stops_delivery_of_what_is_already_queued,
            events::two_sources_each_receive_their_own_copy_of_one_occurrence,
            events::a_short_buffer_leaves_the_occurrence_for_the_next_call,
            events::a_sink_sequence_number_counts_one_channel_publications,
            events::a_raised_events_context_arrives_on_every_subscribers_occurrence,
            events::two_occurrences_each_keep_their_own_context,
            events::an_event_raised_without_a_context_arrives_without_one,
            events::a_short_buffer_keeps_the_occurrences_context,
            calls::a_command_is_delivered_and_acknowledged,
            calls::a_query_is_delivered_and_replied,
            calls::settle_can_be_made_to_fail_once_then_succeed,
            calls::two_callers_on_one_provider_are_two_claims_under_one_seq,
            calls::a_caller_sequence_number_counts_that_caller_calls,
            calls::a_settled_outcome_reports_the_contract_error_the_provider_settled,
            calls::a_claim_is_presented_once_and_settled_once,
            calls::an_oversized_claim_is_reported_with_its_id_and_is_not_consumed,
            calls::an_unread_claim_is_settled_by_its_id,
            calls::the_calls_behind_an_oversized_claim_are_presented_once_it_is_settled,
            calls::forget_releases_a_settled_correlation,
            calls::forget_before_the_claim_is_presented_withdraws_or_leaves_the_call,
            calls::a_send_with_every_slot_taken_is_busy_for_every_caller,
            calls::a_reclaimed_slots_old_correlation_answers_none,
            calls::forget_between_the_claim_and_the_settlement_leaves_the_settlement_valid,
            calls::forget_between_the_offer_and_the_settlement_leaves_the_settlement_valid,
            calls::a_claim_that_was_never_presented_cannot_be_settled,
            calls::an_injected_settle_failure_is_not_spent_on_an_unknown_claim,
            calls::a_handler_cannot_settle_another_handlers_claim,
            calls::two_handlers_each_receive_only_what_they_served,
            calls::a_commands_context_arrives_on_its_claim,
            calls::a_querys_context_arrives_on_its_claim,
            calls::a_call_sent_without_a_context_arrives_without_one,
            calls::two_calls_in_flight_each_keep_their_own_context,
            calls::an_oversized_claims_context_survives_its_second_presentation,
            calls::a_reused_call_slot_does_not_keep_the_previous_context,
        );
    };
    ($factory:ty; $($extension:ident),+ $(,)?) => {
        $crate::suite!($factory);
        $( $crate::suite!(@$extension $factory); )+
    };
    (@scannable $factory:ty) => {
        $crate::suite!(@tests $factory;
            scannable::a_commit_advances_the_interface_generation_once,
            scannable::a_commit_of_only_touches_on_unpublished_channels_changes_no_generation,
            scannable::scan_reports_the_changes_since_a_mark_and_moves_it_forward,
            scannable::scan_writes_an_interface_changes_all_together_or_not_at_all,
        );
    };
    (@coherent $factory:ty) => {
        $crate::suite!(@tests $factory;
            coherent::a_coherent_read_answers_every_ordinal_from_one_publication,
            coherent::a_coherent_read_reports_a_short_output_and_a_short_sample_slice,
        );
    };
    (@wakeable $factory:ty) => {
        $crate::suite!(@tests $factory;
            wakeable::an_outcome_waker_is_kept_with_its_call_and_woken_once_by_the_settlement,
            wakeable::every_slot_waker_is_woken_by_a_reclaim_and_the_send_that_follows_succeeds,
            wakeable::every_subscribed_source_is_woken_once_by_a_raise,
            wakeable::a_call_to_a_served_member_wakes_the_handler_once,
            wakeable::another_tasks_registration_displaces_the_stored_waker_and_wakes_it,
            wakeable::the_same_tasks_registration_is_a_refresh_that_wakes_nothing,
            wakeable::one_waker_per_kind_is_woken_by_a_change_on_any_interface_of_that_kind,
            wakeable::every_wake_runs_with_the_runtime_lock_released,
        );
    };
    (@tests $factory:ty; $($module:ident :: $test:ident),+ $(,)?) => {
        $(
            #[test]
            fn $test() {
                $crate::$module::$test::<$factory>();
            }
        )+
    };
}

// ---------------------------------------------------------------------------
// What every test shares.
// ---------------------------------------------------------------------------

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

/// A trace context the trace cases send, and the one they tell it apart from.
///
/// Every byte differs from every other byte of this value and of
/// [`TRACE_B`], so a runtime that reverses, truncates or masks a field, or
/// that delivers one context's field in place of the other's, fails a case.
const TRACE_A: ridl_rt::trace::TraceContext = ridl_rt::trace::TraceContext {
    trace_id: [
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
        0x0F,
    ],
    span_id: [0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17],
    flags: 0x5A,
};

/// The second trace context, with different identifiers and flags than
/// [`TRACE_A`].
const TRACE_B: ridl_rt::trace::TraceContext = ridl_rt::trace::TraceContext {
    trace_id: [
        0xF0, 0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA, 0xFB, 0xFC, 0xFD, 0xFE,
        0xFF,
    ],
    span_id: [0xE0, 0xE1, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7],
    flags: 0xA5,
};

fn runtime<F: Factory>() -> F::Runtime {
    F::runtime(catalog())
}

/// Takes every slot of `rt`'s call table: sends [`Factory::SLOTS`] commands
/// through `rt`, and has `rt` claim and settle each one as it is sent, so
/// every slot holds a settled call that nobody has forgotten. Returns the
/// correlations in send order. `rt` must serve `IFACE`/`ORD` already.
fn fill<F: Factory>(rt: &mut F::Runtime) -> Vec<Correlation> {
    let mut buf = [0u8; 8];
    (0..F::SLOTS)
        .map(|_| {
            let c = rt.command(IFACE, ORD, &[1], None).expect("a slot is free");
            let claim = rt
                .next_claim(&mut buf)
                .expect("next_claim")
                .expect("the call just sent");
            rt.settle(claim.id, Ok(&[])).expect("settle");
            c
        })
        .collect()
}

/// `start` moved forward by `by`, for a test that compares a timestamp with
/// the clock reading it started from.
fn later(start: Timestamp, by: i64) -> Timestamp {
    Timestamp(start.0 + by)
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

#[cfg(test)]
mod tests {
    /// The arm of `suite!` a test belongs in: the base arm, or the arm of the
    /// one signal extension its signature asks `F::Runtime` for.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Arm {
        Base,
        Scannable,
        Coherent,
        Wakeable,
    }

    /// Every public function of a test module: its path under the crate, and
    /// the arm its signature places it in.
    fn test_functions() -> Vec<(String, Arm)> {
        let modules = [
            ("attached", include_str!("attached.rs")),
            ("calls", include_str!("calls.rs")),
            ("clock", include_str!("clock.rs")),
            ("coherent", include_str!("coherent.rs")),
            ("events", include_str!("events.rs")),
            ("scannable", include_str!("scannable.rs")),
            ("signals", include_str!("signals.rs")),
            ("wakeable", include_str!("wakeable.rs")),
        ];
        let mut found = Vec::new();
        for (module, source) in modules {
            for (start, _) in source.match_indices("\npub fn ") {
                let rest = &source[start + "\npub fn ".len()..];
                let name = rest.split('<').next().expect("a function name");
                // The signature runs to the body's opening brace, and holds
                // the where clause that asks for an extension.
                let signature = rest.split('{').next().expect("a function body");
                let asked: Vec<Arm> = [
                    ("ScannableSignals", Arm::Scannable),
                    ("CoherentSignals", Arm::Coherent),
                    ("Wakeable", Arm::Wakeable),
                ]
                .into_iter()
                .filter(|(extension, _)| signature.contains(extension))
                .map(|(_, arm)| arm)
                .collect();
                let arm = match asked[..] {
                    [] => Arm::Base,
                    [arm] => arm,
                    _ => panic!("`{module}::{name}` asks for more than one extension"),
                };
                found.push((format!("{module}::{name}"), arm));
            }
        }
        found
    }

    /// The text of each arm of `suite!` that lists tests.
    fn macro_arms() -> [(Arm, &'static str); 4] {
        let lib = include_str!("lib.rs");
        let between = |from: &str, to: &str| {
            let start = lib.find(from).expect("the arm's opening") + from.len();
            let end = start + lib[start..].find(to).expect("the arm's end");
            &lib[start..end]
        };
        [
            (Arm::Base, between("($factory:ty) => {", "($factory:ty; $(")),
            (
                Arm::Scannable,
                between("(@scannable $factory:ty) => {", "(@coherent"),
            ),
            (
                Arm::Coherent,
                between("(@coherent $factory:ty) => {", "(@wakeable"),
            ),
            (
                Arm::Wakeable,
                between("(@wakeable $factory:ty) => {", "(@tests $factory:ty"),
            ),
        ]
    }

    #[test]
    fn the_suite_macro_names_every_test_function_in_its_own_arm() {
        // A test function the macro does not name is never run by any
        // runtime, and nothing else would report it. A test that asks for an
        // extension but sits in the base arm stops `suite!(F)` compiling for
        // every runtime that omits that extension, and nothing in this
        // workspace expands `suite!` over such a runtime.
        let functions = test_functions();
        assert!(functions.len() > 40, "the scan found the test functions");
        for (function, arm) in functions {
            let entry = format!("{function},");
            for (listed, text) in macro_arms() {
                assert_eq!(
                    text.contains(&entry),
                    listed == arm,
                    "`{function}` belongs in the {arm:?} arm of `suite!`, and \
                     only there; the {listed:?} arm is wrong about it"
                );
            }
        }
    }
}
