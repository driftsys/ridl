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
`ridl-rt-conformance = "=0.14.0"`, and raises the pin when it chooses to.

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
`Wakeable`, `trace` the tests of a runtime that carries the trace context, and
`streams` the tests of the two stream extensions, `StreamCaller` and
`StreamHandler`, which also require `Wakeable`. `suite!(MyFactory)` alone runs
the tests of the ports every runtime presents. The cases of the sending ports
require a payload above the member's limit to be refused with
`TooLarge { cap }`, `cap` being that limit, at the default ceiling of the
member's kind when the runtime was given no other limit (ADR-0030 decision 19).
The reference runtime
[`ridl-loopback`](https://github.com/driftsys/ridl/blob/main/crates/ridl-loopback/tests/conformance.rs)
runs the suite this way.

## Identity tests for a runtime with descriptor attachments

A runtime that attaches an instance identity to a catalog from a deployment
descriptor can run two more groups. They are opt-in and leave `Factory`
unchanged: a runtime that does not name them runs the same tests as before.

Implement `ridl_rt_conformance::identity::IdentityFactory` on the same type as
`Factory`. It adds a `Node` type, an `Attachment` type and an `Error` type, and
two functions: `node(bytes, machine)` builds the named machine from a deployment
descriptor, and `attach(node, who, catalog)` reserves an identity on a catalog
or refuses it. The tests use the descriptor that this crate packages as
`identity::DESCRIPTOR`, with the catalogs
`identity::{ALPHA, BETA, MAPPED,
REMOTE}`, so a runtime needs no compiler
dependency. Then add a flag to `suite!`:

- `identity` checks that a positive instance receives the catalogs it offers and
  maps, and no other.
- `identity_debug` adds the twin's grant, the exclusion of `N` and `-N` in both
  orders, the release of an identity on the last drop, the listener's read of
  every catalog, and the refusal of every write by the listener. It requires
  this crate's `debug` feature, which enables `ridl-rt/debug`; without the
  feature, `suite!` stops with a compile error that names the feature.

```rust,ignore
use ridl_rt_conformance::identity::IdentityFactory;

impl IdentityFactory for MyFactory {
    type Node = MyNode;
    type Attachment = MyAttachment;
    type Error = MyAttachError;

    fn node(bytes: &'static [u8], machine: &str) -> MyNode { /* ... */ }
    fn attach(
        node: &MyNode,
        who: ridl_rt::contract::InstanceNo,
        catalog: &CatalogRef,
    ) -> Result<MyAttachment, MyAttachError> { /* ... */ }
}

ridl_rt_conformance::suite!(MyFactory; identity, identity_debug);
```

`ridl-loopback` runs both groups with its `node` and `debug` features.

## What the suite does not check

The crate documentation lists, in one place, what the suite leaves out:
<https://docs.rs/ridl-rt-conformance>, section "What the suite leaves out". A
runtime that wants one of these pinned keeps its own test of it.

## License

MIT. See [LICENSE](https://github.com/driftsys/ridl/blob/main/LICENSE).
