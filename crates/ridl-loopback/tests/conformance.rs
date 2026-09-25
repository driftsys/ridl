//! The port contract suite of `ridl-rt-conformance`, run over this runtime.
//!
//! Every test the suite has runs here, including those of both signal
//! extensions, which the loopback implements. The tests of what only this
//! runtime can express are in `tests/ports.rs`.

use ridl_loopback::{CallerHandle, HandlerHandle, Loopback, SourceHandle};
use ridl_rt::contract::CatalogRef;
use ridl_rt::sample::Duration;
use ridl_rt_conformance::Factory;

/// The loopback as the suite builds it: the aggregate, the additional role
/// handles `Loopback` hands out, and its two test hooks.
struct LoopbackFactory;

impl Factory for LoopbackFactory {
    type Runtime = Loopback;
    type Source = SourceHandle;
    type Caller = CallerHandle;
    type Handler = HandlerHandle;

    fn runtime(catalog: CatalogRef) -> Loopback {
        Loopback::new(catalog)
    }

    fn source(runtime: &Loopback) -> SourceHandle {
        runtime.source()
    }

    fn caller(runtime: &Loopback) -> CallerHandle {
        runtime.caller()
    }

    fn handler(runtime: &Loopback) -> HandlerHandle {
        runtime.handler()
    }

    fn advance(runtime: &mut Loopback, by: Duration) {
        runtime.advance(by);
    }

    fn fail_next_settle(runtime: &mut Loopback) {
        runtime.fail_next_settle();
    }
}

ridl_rt_conformance::suite!(LoopbackFactory; scannable, coherent);
