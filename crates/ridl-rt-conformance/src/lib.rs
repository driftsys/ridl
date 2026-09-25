//! `ridl-rt-conformance` — the port contract tests, generic over a runtime.
//!
//! Each public function in this crate's modules is one test of what
//! [`ridl_rt::port`](https://docs.rs/ridl-rt) states a runtime does behind a
//! port. The functions are generic over a [`Factory`], the one thing a runtime
//! crate writes to run them: how to build a runtime, how to get a second
//! handle of three port roles on it, and two hooks the port traits do not
//! offer. A runtime runs the whole suite from its own test suite with the
//! [`suite!`] macro, which writes one `#[test]` per function:
//!
//! ```ignore
//! struct MyFactory;
//!
//! impl ridl_rt_conformance::Factory for MyFactory {
//!     // ...
//! }
//!
//! // Every test of the ports every runtime presents, and those of the two
//! // signal extensions this runtime implements.
//! ridl_rt_conformance::suite!(MyFactory; scannable, coherent);
//! ```
//!
//! `ridl-loopback` is the first runtime to run it
//! (`crates/ridl-loopback/tests/conformance.rs`). The tests came from that
//! crate's `tests/ports.rs` (story E11.20, driftsys/ridl#514), which keeps
//! the tests only the loopback can express, each listed with its reason in
//! that file's module documentation.
//!
//! # What the suite does not cover
//!
//! - **`FixedReader`.** How a `fixed` is provisioned into a runtime is the
//!   runtime's own API, not a port, and what an unprovisioned `fixed` reads
//!   as is not fixed by the port contract.
//! - **The threading model.** ADR-0021 decision 12 holds a runtime whose
//!   handles are used from more than one thread to `Send` and `Sync` bounds,
//!   and a single-threaded runtime to neither, so no test here moves a handle
//!   to another thread.
//! - **Anything a runtime reports from a catalog descriptor**: an unknown
//!   ordinal, an unowned member, a freshness measured against a staleness
//!   bound. The suite attaches to a catalog with no descriptor.

use ridl_rt::contract::{CatalogHash, CatalogRef, InterfaceNo, Ordinal};
use ridl_rt::port::{
    Attached, Caller, Changed, Clock, EventSink, EventSource, Handler, RawSample, SignalReader,
    SignalWriter,
};
use ridl_rt::sample::{Duration, Envelope, Freshness, Provenance, Timestamp};

pub mod attached;
pub mod calls;
pub mod clock;
pub mod coherent;
pub mod events;
pub mod scannable;
pub mod signals;

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
    /// The two signal extensions are not in this bound, because a runtime
    /// may omit them; the tests of each ask for it where they need it.
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
/// `suite!(F)` writes the tests of the ports every runtime presents.
/// `suite!(F; scannable)`, `suite!(F; coherent)` and
/// `suite!(F; scannable, coherent)` add the tests of the signal extensions
/// named, each of which requires `F::Runtime` to implement it. Each test is
/// named after the function it calls, so a failure names the contract it
/// breaks.
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
            calls::a_command_is_delivered_and_acknowledged,
            calls::a_query_is_delivered_and_replied,
            calls::settle_can_be_made_to_fail_once_then_succeed,
            calls::two_callers_on_one_provider_are_two_claims_under_one_seq,
            calls::a_caller_sequence_number_counts_that_caller_calls,
            calls::a_settled_outcome_reports_the_contract_error_the_provider_settled,
            calls::a_claim_is_presented_once_and_settled_once,
            calls::a_short_buffer_leaves_the_claim_for_the_next_call,
            calls::forget_releases_a_settled_correlation,
            calls::forget_before_the_claim_is_presented_leaves_the_call_for_the_provider,
            calls::forget_between_the_claim_and_the_settlement_leaves_the_settlement_valid,
            calls::a_claim_that_was_never_presented_cannot_be_settled,
            calls::an_injected_settle_failure_is_not_spent_on_an_unknown_claim,
            calls::a_handler_cannot_settle_another_handlers_claim,
            calls::two_handlers_each_receive_only_what_they_served,
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

/// The catalog every test attaches to. The hash is all zeros, which is the
/// placeholder the descriptor emitter writes until story E16.2
/// (driftsys/ridl#378) computes a real one.
fn catalog() -> CatalogRef {
    CatalogRef {
        name: "face.demo",
        hash: CatalogHash([0u8; 32]),
    }
}

fn runtime<F: Factory>() -> F::Runtime {
    F::runtime(catalog())
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
    /// Every public function of a test module, paired with its module name.
    fn test_functions() -> Vec<String> {
        let modules = [
            ("attached", include_str!("attached.rs")),
            ("calls", include_str!("calls.rs")),
            ("clock", include_str!("clock.rs")),
            ("coherent", include_str!("coherent.rs")),
            ("events", include_str!("events.rs")),
            ("scannable", include_str!("scannable.rs")),
            ("signals", include_str!("signals.rs")),
        ];
        let mut found = Vec::new();
        for (module, source) in modules {
            for line in source.lines() {
                if let Some(rest) = line.strip_prefix("pub fn ") {
                    let name = rest.split('<').next().expect("a function name");
                    found.push(format!("{module}::{name}"));
                }
            }
        }
        found
    }

    #[test]
    fn the_suite_macro_names_every_test_function() {
        // A test function the macro does not name is never run by any
        // runtime, and nothing else would report it.
        let lib = include_str!("lib.rs");
        let (macro_body, _) = lib
            .split_once("// What every test shares.")
            .expect("the macro precedes the shared items");
        let functions = test_functions();
        assert!(functions.len() > 40, "the scan found the test functions");
        for function in functions {
            assert!(
                macro_body.contains(&format!("{function},")),
                "`suite!` does not name `{function}`"
            );
        }
    }
}
