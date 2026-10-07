//! The propagation hook is set once for the process, and the second attempt
//! is refused. The hook is global, so this file is one test binary with one
//! test function.
#![cfg(feature = "std")]

use ridl_rt::trace::{propagation, set_propagation, AlreadySet, Propagation, TraceContext};

struct Marker(u8);

impl Propagation for Marker {
    fn current(&self) -> Option<TraceContext> {
        Some(TraceContext {
            trace_id: [self.0; 16],
            span_id: [self.0; 8],
            flags: self.0,
        })
    }
    fn enter(&self, _received: Option<TraceContext>) {}
    fn leave(&self) {}
}

static A: Marker = Marker(1);
static B: Marker = Marker(2);

fn marker_of(hook: Option<&'static dyn Propagation>) -> Option<u8> {
    hook.and_then(|hook| hook.current()).map(|ctx| ctx.flags)
}

#[test]
fn set_once_then_refuse() {
    assert!(propagation().is_none());
    assert_eq!(set_propagation(&A), Ok(()));
    assert_eq!(marker_of(propagation()), Some(1));
    assert_eq!(set_propagation(&B), Err(AlreadySet));
    let error: &dyn std::error::Error = &AlreadySet;
    assert_eq!(
        error.to_string(),
        "a trace propagation hook is already registered"
    );
    assert_eq!(marker_of(propagation()), Some(1));
}
