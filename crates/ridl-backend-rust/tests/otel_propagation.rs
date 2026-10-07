//! The order in which a generated `dispatch` calls the propagation hook and
//! opens the claim span, proven against `tracing-opentelemetry`.
//!
//! The design of generated observation gives `ridl_rt::trace::Propagation`
//! to the application: `current` reads the context to send, `enter` makes a
//! received context the parent of what follows, and `leave` undoes `enter`.
//! This test holds a local copy of that trait, implements it with
//! `tracing-opentelemetry` over an in-memory exporter, simulates one call and
//! one claim in one thread, and reads the exported OpenTelemetry spans back.
//!
//! The order that passes, and that the generated `dispatch` follows:
//!
//! 1. `enter(received)`, before the claim span exists. The hook attaches an
//!    OpenTelemetry context that carries the received span context.
//! 2. Create the claim span contextually (no `parent:` argument) and enter it.
//!    `OpenTelemetryLayer::on_new_span` reads the attached context as the
//!    parent at creation, and `on_enter` starts the OpenTelemetry span.
//! 3. Run the handler. A call made inside takes the claim span as its parent.
//! 4. Exit and drop the claim span.
//! 5. `leave()`. The hook detaches what `enter` attached.
//!
//! Setting the parent after the span is entered (`set_parent` on
//! `Span::current()`) returns `AlreadyStarted`, because entering the span
//! starts it. Creating the claim span as an explicit root (`parent: None`)
//! ignores the attached context and makes a new trace. The design note's
//! "Call order" paragraph records both.

use std::cell::RefCell;

use opentelemetry::trace::{
    SpanContext, SpanId, TraceContextExt, TraceFlags, TraceId, TraceState, TracerProvider,
};
use opentelemetry::{Context, ContextGuard};
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider, SpanData};
use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::Registry;
use tracing_subscriber::layer::SubscriberExt;

/// A local copy of `ridl_rt::trace::TraceContext`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TraceContext {
    trace_id: [u8; 16],
    span_id: [u8; 8],
    flags: u8,
}

/// A local copy of the hook the design note gives to `ridl_rt::trace`.
trait Propagation: Sync {
    fn current(&self) -> Option<TraceContext>;
    fn enter(&self, received: Option<TraceContext>);
    fn leave(&self);
}

thread_local! {
    /// What `enter` attached, popped by `leave`. An entry is `None` when
    /// nothing was received, so that `leave` always pops what `enter` pushed.
    static ATTACHED: RefCell<Vec<Option<ContextGuard>>> = const { RefCell::new(Vec::new()) };
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
        ATTACHED.with(|stack| {
            stack.borrow_mut().pop();
        });
    }
}

/// What one call and one claim leave behind.
struct Observed {
    /// The context the client sent, read inside its call span.
    sent: TraceContext,
    /// The application span that encloses `dispatch`.
    serve_loop: TraceContext,
    /// What a nested call inside the handler would send.
    inside_handler: Option<TraceContext>,
    /// What the hook reads once `dispatch` has returned.
    after_handler: Option<TraceContext>,
    /// The OpenTelemetry context attached once `dispatch` has returned.
    otel_after_handler: SpanContext,
    exported: Vec<SpanData>,
}

/// Simulates `dispatch` in the order the module comment states, inside an
/// enclosing application span, with the layer's default configuration.
fn run() -> Observed {
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

        hook.enter(Some(sent));
        let inside_handler = {
            let claim = tracing::trace_span!("Cabin/setLevel");
            let _entered = claim.enter();
            hook.current()
        };
        hook.leave();

        Observed {
            sent,
            serve_loop,
            inside_handler,
            after_handler: hook.current(),
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
    let observed = run();
    let claim = claim_span(&observed);
    assert_eq!(claim.parent_span_id.to_bytes(), observed.sent.span_id);
    assert_eq!(
        claim.span_context.trace_id().to_bytes(),
        observed.sent.trace_id
    );
    assert!(claim.parent_span_is_remote);
}

#[test]
fn nested_call_is_a_child_of_the_claim() {
    let observed = run();
    let claim = claim_span(&observed);
    let inside = observed
        .inside_handler
        .expect("the handler runs inside a span with a context");
    assert_eq!(inside.trace_id, observed.sent.trace_id);
    assert_eq!(inside.span_id, claim.span_context.span_id().to_bytes());
}

#[test]
fn leave_restores_the_application_span() {
    let observed = run();
    assert_eq!(observed.after_handler, Some(observed.serve_loop));
    assert_eq!(
        observed.otel_after_handler.span_id().to_bytes(),
        observed.serve_loop.span_id
    );
    assert_eq!(
        observed.otel_after_handler.trace_id().to_bytes(),
        observed.serve_loop.trace_id
    );
}
