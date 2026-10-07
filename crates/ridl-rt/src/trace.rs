//! The trace context a call or an event can carry.
//!
//! [`TraceContext`] is the W3C Trace Context `traceparent` layout without the
//! version byte, as plain data. `ridl-rt` carries it unvalidated: an all-zero
//! id is carried like any other value, and rejecting one is the choice of
//! whatever exports the trace.
//!
//! With the `std` feature, [`Propagation`] is the hook through which an
//! application gives generated code the context to send and receives the
//! context that arrived.

/// A trace id, a span id and the trace flags, in the layout of the W3C Trace
/// Context `traceparent` header without its version byte.
///
/// The value is not validated. `size_of::<TraceContext>()` is 25 bytes and
/// `size_of::<Option<TraceContext>>()` is 26 bytes, both with alignment 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TraceContext {
    /// The identifier of the whole trace.
    pub trace_id: [u8; 16],
    /// The identifier of the span that sent the call or raised the event.
    pub span_id: [u8; 8],
    /// The trace flags byte, carried as given.
    pub flags: u8,
}

#[cfg(feature = "std")]
use std::sync::OnceLock;

/// The hook through which an application connects its telemetry library to the
/// trace context that crosses a port. Available with the `std` feature.
///
/// `ridl-rt` carries the bytes. The code that a generated crate emits is meant
/// to call the hook at the points below. No generated code calls it yet: the
/// generated face still passes `None` as the trace context (driftsys/ridl#754
/// tracks the change). Which span is current, how ids are created and where traces are
/// exported belong to the application's library. No hook is registered until
/// the application calls [`set_propagation`].
///
/// # Call points
///
/// Generated code that calls the hook calls:
///
/// - [`current`](Propagation::current) just before it sends a call or a raise.
///   It returns the context to send, or `None`.
/// - [`enter`](Propagation::enter) when it receives a claim, before the claim
///   span exists and before the handler runs.
/// - [`leave`](Propagation::leave) after the handler has returned, and also
///   when the handler panics.
///
/// # Call order for a claim
///
/// Generated code that serves a claim and calls the hook follows this order:
///
/// 1. `enter(received)`, where `received` is the context that came over the
///    wire. It is `None` for a claim that carried no context, and `leave` is
///    still called to pair with that `enter`.
/// 2. The claim span is created with no explicit parent, and entered.
/// 3. The handler runs.
/// 4. The claim span is exited and dropped.
/// 5. `leave()`.
///
/// An OpenTelemetry implementation of `enter` attaches a context that carries
/// the received span context as a remote parent, keeps the guard, and drops
/// the guard in `leave`. It relies on `tracing-opentelemetry` choosing the
/// parent of a span when the span is created, from the attached context. That
/// context activation is on by default since `tracing-opentelemetry` 0.32.0.
/// Setting the parent after the claim span is entered fails, because entering
/// the span starts it.
///
/// The guard that calls `leave` is created only after `enter` returns, so a
/// panic inside `enter` does not call `leave`. An implementation's `enter` and
/// `leave` should not panic: a panic in `leave` while another panic unwinds
/// aborts the process, and a panic in `enter` leaves nothing attached.
///
/// # Nesting and threads
///
/// Each `enter` is paired with one `leave`. Pairs nest: a handler that serves
/// another claim produces an inner pair inside the outer one. The `leave` of a
/// pair runs on the thread that ran its `enter`, so an implementation can keep
/// what `enter` attached on a per-thread stack.
#[cfg(feature = "std")]
pub trait Propagation: Sync {
    /// Called just before a call or a raise is sent. Returns the context to
    /// send, or `None`.
    fn current(&self) -> Option<TraceContext>;

    /// Called when a claim is received, before the claim span is created and
    /// before the handler runs. The caller of the hook calls `leave` after the
    /// handler, also when `received` is `None`. `received` is the context that came over the
    /// wire, or `None` when the claim carried none.
    fn enter(&self, received: Option<TraceContext>);

    /// Called after the handler has returned, and also when it panics. Undoes
    /// the matching [`enter`](Propagation::enter).
    fn leave(&self);
}

/// [`set_propagation`] was called after a hook was already registered.
#[cfg(feature = "std")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AlreadySet;

#[cfg(feature = "std")]
impl core::fmt::Display for AlreadySet {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("a trace propagation hook is already registered")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for AlreadySet {}

#[cfg(feature = "std")]
static HOOK: OnceLock<&'static dyn Propagation> = OnceLock::new();

/// Registers the hook once for the process, as `log::set_logger` does for the
/// `log` crate. A second call returns [`AlreadySet`] and keeps the first hook.
///
/// One hook serves every generated crate in the process, so a handler in one
/// generated crate that calls a client of another continues the same trace.
#[cfg(feature = "std")]
pub fn set_propagation(p: &'static dyn Propagation) -> Result<(), AlreadySet> {
    HOOK.set(p).map_err(|_| AlreadySet)
}

/// The registered hook, or `None` while the application has not called
/// [`set_propagation`].
#[cfg(feature = "std")]
pub fn propagation() -> Option<&'static dyn Propagation> {
    HOOK.get().copied()
}
