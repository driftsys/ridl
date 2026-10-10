# ridl-rt-conformance

The port contract tests of [`ridl-rt`](https://crates.io/crates/ridl-rt), as
functions generic over a runtime. A runtime that implements the port traits of
`ridl-rt` adds this crate as a dev-dependency and runs the whole suite from its
own test suite.

Each test checks what the `ridl_rt::port` documentation states a runtime does
behind a port: reads and writes, scanning, events, calls and claims, and the
optional extensions. The public surface follows that port contract, so use the
same version of this crate as of `ridl-rt`.

A patch release can add tests to `suite!`. A runtime that wants a fixed set of
tests pins this crate with `=` in its `dev-dependencies`, for example
`ridl-rt-conformance = "=0.7.0"`, and raises the pin when it chooses to.

## Usage

Implement `Factory` once for the runtime. It states how to build a runtime, how
to get a second event source, caller and handler on it, the size of its call
table (`Factory::SLOTS`), and two hooks the port traits do not offer: advance
the runtime's clock, and make the next settlement fail. Then call `suite!`,
which writes one `#[test]` per function:

```rust,ignore
use ridl_rt::contract::CatalogRef;
use ridl_rt::sample::Duration;
use ridl_rt_conformance::Factory;

struct MyFactory;

impl Factory for MyFactory {
    type Runtime = MyRuntime;
    type Source = MySource;
    type Caller = MyCaller;
    type Handler = MyHandler;

    const SLOTS: usize = MyRuntime::SLOTS;

    fn runtime(catalog: CatalogRef) -> MyRuntime { /* ... */ }
    fn source(runtime: &MyRuntime) -> MySource { /* ... */ }
    fn caller(runtime: &MyRuntime) -> MyCaller { /* ... */ }
    fn handler(runtime: &MyRuntime) -> MyHandler { /* ... */ }
    fn advance(runtime: &mut MyRuntime, by: Duration) { /* ... */ }
    fn fail_next_settle(runtime: &mut MyRuntime) { /* ... */ }
}

// The tests of the ports every runtime presents, those of the extensions this
// runtime implements, and those of a runtime that carries the trace context.
ridl_rt_conformance::suite!(MyFactory; scannable, coherent, wakeable, trace);
```

The extension flags after the factory are optional. `scannable` and `coherent`
select the tests of the two signal extensions, `wakeable` the tests of
`Wakeable`, and `trace` the tests of a runtime that carries the trace context.
`suite!(MyFactory)` alone runs the tests of the ports every runtime presents.
The reference runtime
[`ridl-loopback`](https://github.com/driftsys/ridl/blob/main/crates/ridl-loopback/tests/conformance.rs)
runs the suite this way.

## What the suite does not check

The crate documentation lists, in one place, what the suite leaves out:
<https://docs.rs/ridl-rt-conformance>, section "What the suite leaves out". A
runtime that wants one of these pinned keeps its own test of it.

## License

MIT. See [LICENSE](https://github.com/driftsys/ridl/blob/main/LICENSE).
