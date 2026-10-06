//! The trace context a call or an event can carry.
//!
//! [`TraceContext`] is the W3C Trace Context `traceparent` layout without the
//! version byte, as plain data. `ridl-rt` carries it unvalidated: an all-zero
//! id is carried like any other value, and rejecting one is the choice of
//! whatever exports the trace.

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
