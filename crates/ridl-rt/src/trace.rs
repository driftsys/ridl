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
extern crate std;

#[cfg(feature = "std")]
use std::sync::OnceLock;

/// The hook through which an application connects its telemetry library to the
/// trace context that crosses a port. Available with the `std` feature.
///
/// `ridl-rt` carries the bytes and calls the hook at fixed points. Which span
/// is current, how ids are created and where traces are exported belong to the
/// application's library. No hook is registered until the application calls
/// [`set_propagation`], and until then a sender passes `None`.
///
/// # Call points
///
/// - [`current`](Propagation::current) is called just before a call or a raise
///   is sent. It returns the context to send, or `None`.
/// - [`enter`](Propagation::enter) is called when a claim is received, before
///   the claim span exists and before the handler runs.
/// - [`leave`](Propagation::leave) is called after the handler has returned,
///   and also when it panics.
///
/// # Call order for a claim
///
/// A dispatcher that serves a claim calls the hook in this order:
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
#[cfg(feature = "std")]
pub trait Propagation: Sync {
    /// Called just before a call or a raise is sent. Returns the context to
    /// send, or `None`.
    fn current(&self) -> Option<TraceContext>;

    /// Called when a claim is received, before the claim span is created and
    /// before the handler runs. `received` is the context that came over the
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
