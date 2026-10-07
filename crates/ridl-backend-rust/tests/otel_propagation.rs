//! The order in which a generated `dispatch` calls the propagation hook and
//! opens the claim span, proven against `tracing-opentelemetry`.
//!
//! `ridl_rt::trace::Propagation` is the hook the application implements:
//! `current` reads the context to send, `enter` makes a received context the
//! parent of what follows, and `leave` undoes `enter`. This test implements it
//! with `tracing-opentelemetry` over an in-memory exporter, simulates one call
//! and one claim in one thread, and reads the exported OpenTelemetry spans
//! back. The hook object is passed directly and is not registered, so the test
//! does not depend on the process-wide hook.
//!
//! The order that passes, and that the generated `dispatch` must follow, as
//! recorded in ADR-0021 decision 22:
//!
//! 1. `enter(received)`, before the claim span exists. The hook attaches an
//!    OpenTelemetry context that carries the received span context.
//! 2. Create the claim span contextually (no `parent:` argument) and enter it.
//!    `OpenTelemetryLayer::on_new_span` reads the attached context as the
//!    parent at creation, and `on_enter` starts the OpenTelemetry span.
//! 3. Run the handler. A call made inside takes the claim span as its parent.
//! 4. Exit and drop the claim span.
//! 5. `leave()`. The hook detaches what `enter` attached. The hook asserts
//!    that the claim span is not the current span when `leave` runs. This
//!    catches a `leave` that runs while the claim span is still entered. It
//!    does not check that the claim span is dropped before `leave`.
//!
//! Setting the parent after the span is entered (`set_parent` on
//! `Span::current()`) returns `AlreadyStarted`, because entering the span
//! starts it. Creating the claim span as an explicit root (`parent: None`)
//! ignores the attached context and makes a new trace. These two facts come
//! from the source of `tracing-opentelemetry` 0.34.0 and are recorded in
//! ADR-0021 decision 22; no test here pins them.

use std::cell::RefCell;

use opentelemetry::trace::{
    SpanContext, SpanId, TraceContextExt, TraceFlags, TraceId, TraceState, TracerProvider,
};
use opentelemetry::{Context, ContextGuard};
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider, SpanData};
use ridl_rt::trace::{Propagation, TraceContext};
use tracing::{Span, span};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::Registry;
use tracing_subscriber::layer::SubscriberExt;

thread_local! {
    /// What `enter` attached, popped by `leave`. An entry is `None` when
    /// nothing was received, so that `leave` always pops what `enter` pushed.
    static ATTACHED: RefCell<Vec<Option<ContextGuard>>> = const { RefCell::new(Vec::new()) };

    /// The ids of the claim spans that are open, innermost last. The harness
    /// pushes one when it creates a claim span and pops it after `leave`, so
    /// that `leave` can check that its claim span is closed.
    static OPEN_CLAIMS: RefCell<Vec<Option<span::Id>>> = const { RefCell::new(Vec::new()) };
}

/// The hook implemented with OpenTelemetry.
struct Otel;

impl Propagation for Otel {
    fn current(&self) -> Option<TraceContext> {
        let context = Span::current().context();
        let span = context.span();
        let span_context = span.span_context();
        span_context.is_valid().then(|| TraceContext {
            trace_id: span_context.trace_id().to_bytes(),
            span_id: span_context.span_id().to_bytes(),
            flags: span_context.trace_flags().to_u8(),
        })
    }

    fn enter(&self, received: Option<TraceContext>) {
        let guard = received.map(|received| {
            Context::current()
                .with_remote_span_context(SpanContext::new(
                    TraceId::from_bytes(received.trace_id),
                    SpanId::from_bytes(received.span_id),
                    TraceFlags::new(received.flags),
                    true,
                    TraceState::default(),
                ))
                .attach()
        });
        ATTACHED.with(|stack| stack.borrow_mut().push(guard));
    }

    fn leave(&self) {
        let claim = OPEN_CLAIMS.with(|claims| claims.borrow().last().cloned().flatten());
        assert!(
            claim.is_some() && Span::current().id() != claim,
            "leave() ran while the claim span was still the current span"
        );
        ATTACHED.with(|stack| {
            stack.borrow_mut().pop();
        });
    }
}

/// Simulates `dispatch` for one claim in the order the module comment states:
/// `enter`, the claim span around `handler`, then `leave`.
fn serve_claim<R>(hook: &Otel, received: Option<TraceContext>, handler: impl FnOnce() -> R) -> R {
    hook.enter(received);
    let result = {
        let claim = tracing::trace_span!("Cabin/setLevel");
        OPEN_CLAIMS.with(|claims| claims.borrow_mut().push(claim.id()));
        let _entered = claim.enter();
        handler()
    };
    hook.leave();
    OPEN_CLAIMS.with(|claims| claims.borrow_mut().pop());
    result
}

/// What one call and one claim leave behind.
struct Observed {
    /// The context the client sent, read inside its call span.
    sent: TraceContext,
    /// The application span that encloses `dispatch`.
    serve_loop: TraceContext,
    /// What a nested call inside the handler would send.
    inside_handler: Option<TraceContext>,
    /// The OpenTelemetry context attached once `dispatch` has returned.
    otel_after_handler: SpanContext,
    exported: Vec<SpanData>,
}

/// Simulates `dispatch` in the order the module comment states, inside an
/// enclosing application span, with the layer's default configuration. When
/// `received` is false, the claim carries no context.
fn run(received: bool) -> Observed {
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .build();
    let layer = tracing_opentelemetry::layer().with_tracer(provider.tracer("test"));
    let subscriber = Registry::default().with(layer);
    let hook = Otel;

    let observed = tracing::subscriber::with_default(subscriber, || {
        // The client side: the call span is entered while the context is read.
        let sent = {
            let call = tracing::trace_span!("Cabin/setLevel");
            let _entered = call.enter();
            hook.current().expect("the call span has a context")
        };

        // The provider side: `dispatch` runs inside the application's loop.
        let serve = tracing::trace_span!("serve loop");
        let _serve_entered = serve.enter();
        let serve_loop = hook.current().expect("the serve loop span has a context");

        let inside_handler = serve_claim(&hook, received.then_some(sent), || hook.current());

        Observed {
            sent,
            serve_loop,
            inside_handler,
            otel_after_handler: Context::current().span().span_context().clone(),
            exported: Vec::new(),
        }
    });
    provider.force_flush().expect("the exporter flushes");
    let exported = exporter
        .get_finished_spans()
        .expect("the exporter holds the finished spans");
    Observed {
        exported,
        ..observed
    }
}

/// The exported claim span: it shares its name with the call span, and is
/// the one that is not the span the client sent.
fn claim_span(observed: &Observed) -> &SpanData {
    let claims: Vec<&SpanData> = observed
        .exported
        .iter()
        .filter(|span| {
            span.name == "Cabin/setLevel"
                && span.span_context.span_id().to_bytes() != observed.sent.span_id
        })
        .collect();
    assert_eq!(
        claims.len(),
        1,
        "exactly one claim span is exported: {claims:?}"
    );
    claims[0]
}

#[test]
fn claim_span_has_the_remote_parent() {
    let observed = run(true);
    let claim = claim_span(&observed);
    assert_eq!(claim.parent_span_id.to_bytes(), observed.sent.span_id);
    assert_eq!(
        claim.span_context.trace_id().to_bytes(),
        observed.sent.trace_id
    );
    assert!(claim.parent_span_is_remote);
}

#[test]
fn current_inside_the_claim_is_the_claim_span_context() {
    let observed = run(true);
    let claim = claim_span(&observed);
    let inside = observed
        .inside_handler
        .expect("the handler runs inside a span with a context");
    assert_eq!(inside.trace_id, observed.sent.trace_id);
    assert_eq!(inside.span_id, claim.span_context.span_id().to_bytes());
}

#[test]
fn leave_restores_the_application_span() {
    let observed = run(true);
    assert_eq!(
        observed.otel_after_handler.span_id().to_bytes(),
        observed.serve_loop.span_id
    );
    assert_eq!(
        observed.otel_after_handler.trace_id().to_bytes(),
        observed.serve_loop.trace_id
    );
}

#[test]
fn claim_without_context_is_a_child_of_the_application_span() {
    let observed = run(false);
    let claim = claim_span(&observed);
    assert_eq!(claim.parent_span_id.to_bytes(), observed.serve_loop.span_id);
    assert_eq!(
        claim.span_context.trace_id().to_bytes(),
        observed.serve_loop.trace_id
    );
    assert!(!claim.parent_span_is_remote);
    assert_eq!(
        observed.otel_after_handler.span_id().to_bytes(),
        observed.serve_loop.span_id
    );
}

#[test]
fn inner_pair_restores_the_outer_context() {
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(exporter)
        .build();
    let layer = tracing_opentelemetry::layer().with_tracer(provider.tracer("test"));
    let subscriber = Registry::default().with(layer);
    let hook = Otel;
    let outer_received = TraceContext {
        trace_id: [1; 16],
        span_id: [1; 8],
        flags: 1,
    };
    let inner_received = TraceContext {
        trace_id: [2; 16],
        span_id: [2; 8],
        flags: 1,
    };

    tracing::subscriber::with_default(subscriber, || {
        serve_claim(&hook, Some(outer_received), || {
            let outer = hook.current().expect("the outer claim span has a context");
            assert_eq!(outer.trace_id, outer_received.trace_id);
            let inner = serve_claim(&hook, Some(inner_received), || {
                hook.current().expect("the inner claim span has a context")
            });
            assert_eq!(inner.trace_id, inner_received.trace_id);
            let restored = Context::current().span().span_context().clone();
            assert_eq!(restored.span_id().to_bytes(), outer.span_id);
            assert_eq!(restored.trace_id().to_bytes(), outer.trace_id);
        });
    });
}
