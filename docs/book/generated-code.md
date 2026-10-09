# Using the generated Rust code

`ridl build --emit rust` turns a package into a Rust crate. This chapter shows
how an application uses that crate: the types it holds, the two ways to call an
interface — async and blocking — the Cargo features that select what the crate
contains, and the failures a call can report. The flag itself is described in
the [CLI reference](cli-reference.md#ridl-build).

Most code snippets in this chapter are taken from
`examples/cabin/consumer/src/main.rs`, a program that the repository's
`just demo` gate builds and runs against the crate generated from
`examples/cabin/cabin.ridl`. A snippet that is taken neither from that program
nor from that schema is marked as an illustration in the sentence before it,
and is not compiled. The interface the snippets call, and the types it uses,
from that schema:

```ridl,ignore
{{#include ../../examples/cabin/cabin.ridl:schema}}
```

## What the crate contains

The build writes one `<package>.rs` file per package, a `lib.rs` that declares
the module tree, and a `Cargo.toml`. A package `veh.cabin` is the module
`veh::cabin`. The crate is named after the `[package]` name of the `ridl.toml`
manifest, with each `.` replaced by `_`: `examples/cabin` declares the package
`veh.cabin`, so its crate is `veh_cabin`. A `[workspace]` manifest names no
package, and a build from one names the crate `ridl_generated`.

The cabin program imports the package module as `api`, the face of the
interface `Cabin` as `cabin`, and the face's `prelude`:

```rust,noplayground
{{#include ../../examples/cabin/consumer/src/main.rs:imports}}
```

`prelude` brings into scope the traits that carry the fixed methods of the
face: `ridl_rt::face::Bind` (`new`), `Events` (`next_event`), `Publish`
(`commit`), and, under the crate's `std` feature, `Timeout` (`with_timeout` and
`set_timeout`); and, without a name, the interface's own `Subscribe`
(`subscribe_<event>`) and `Invalidate` (`invalidate_<signal>`) traits. Each
interface's prelude holds only the traits its face implements. These methods
are trait methods rather than inherent methods so that a member of the
interface can have one of these names. Without the `use` of the prelude,
`cabin::Client::new` does not compile. The other imports are for the snippets
below: `Loopback` is the runtime, `CatalogRef` and `Interface` name the catalog
the face was generated from, and `Provenance` is read from a signal sample.

The package module holds:

- **The domain types.** A struct, an enum, an enum set, a union and a named
  scalar each become a Rust type. A constrained type is built with `new`, which
  checks the typl constraints and returns an error when the value breaks one;
  `new_unchecked` skips the check and is for a value already known to satisfy
  them. `get` returns the value of a named scalar.
- **The FlatBuffers codec** of each payload type: `encode`, `verify`, `decode`
  and a `MAX_SIZE` bound. The face calls it; an application calls it only when
  it moves payloads by itself.
- **The interaction descriptors**: one unit type per interface (here
  `api::Cabin`) that implements `ridl_rt::contract::Interface` and carries the
  interface's `CATALOG`, `NUMBER` and `NAME`.
- **The face**: one module per interface, named after it (here `api::cabin`).
  The face is what the application calls.

## The face

Each interface module holds the parts its own interactions need:

| Item                | Present when the interface carries | Used by                                            |
| ------------------- | ---------------------------------- | -------------------------------------------------- |
| `Client`            | any interaction                    | the consumer: reads, subscriptions, calls          |
| `Publisher`         | a signal or an event               | the producer: publishes values, raises events      |
| `Provider` (trait)  | a command or a query               | the provider: the application implements it        |
| `serve`             | a command or a query               | the provider: settles the calls waiting on a port  |
| `Event` (enum)      | an event                           | the consumer: one variant per event                |
| `blocking` (module) | an event, a command or a query     | a thread that waits: blocking `Client` and `serve` |

An interface that carries only `fixed` declarations gets no face module. A
signal-only interface gets a `Client` and a `Publisher` and no `blocking`
module, because a signal read returns at once.

An interface the face cannot carry gets none of these items and no interaction
descriptor. In their place, the package module holds a
`__RIDL_NO_FACE_<NAME>` constant whose documentation names the interface and
the reason; the [CLI reference](cli-reference.md#ridl-build) lists the cases.

The face opens no socket and holds no runtime. Each `Client`, `Publisher` and
`serve` runs over a _port_ that the application passes in, and a port comes from
a runtime. The one runtime in this repository is `ridl-loopback`, which connects
the faces of one process to each other. The cabin program binds every face to
it:

```rust,noplayground
{{#include ../../examples/cabin/consumer/src/main.rs:bind}}
```

`CATALOG` is a constant of the program that holds the catalog the face was
generated from, read from `<api::Cabin as Interface>::CATALOG`. Binding a face
compares the port's catalog with it, and panics when they differ; see
[Failures](#failures). `CLIENT_TIMEOUT` is a `Duration` the program declares,
and `with_timeout` comes from the `Timeout` trait of the prelude.

### Implementing the provider

The application implements one method per command and per query. A command's
method returns nothing, and a query's method returns the reply type:

```rust,noplayground
{{#include ../../examples/cabin/consumer/src/main.rs:provider}}
```

The method is called only with a request that passed every check: the payload
verified, its typl constraints hold, and its `require` clauses are true. For a
query, `serve` evaluates the `ensure` clauses on the reply before it sends it.
The provider does not repeat those checks.

## Async calls

The `Client` methods for a command and a query, `next_event`, and `serve` each
return a named future. The futures are executor-agnostic: they take no
dependency on an async runtime, and any executor can poll them. A call is sent
when the method runs, not when its future is first polled.

Inside an async function, the calls read like this. This snippet is an
illustration and is not compiled; `port` and `handler` stand for ports a
runtime gives the application, and `provider` for a value that implements
`cabin::Provider`:

```rust,noplayground
// Illustration: any executor polls these futures.
let mut client = cabin::Client::new(port);
client.set_level(api::Level::new(42).expect("in range")).await?;
let average = client.average(api::Window::new(10).expect("in range")).await?;

// On the provider side, `serve` settles calls until the handler port fails.
// Its output is `Result<Infallible, ProviderError>`: it never resolves to `Ok`.
let failure = cabin::serve(handler, &mut provider).await;
```

No executor is required either. The cabin program polls the same futures by
hand, the way a frame loop does. `cx` is a `Context` built from the waker
`ridl_rt::task::noop_waker()`, and `poll_once` is a function of the program
that polls a future once with it:

```rust,noplayground
{{#include ../../examples/cabin/consumer/src/main.rs:async-command}}
```

`ridl_rt::task` exists only under `ridl-rt`'s `std` feature, which the
generated crate's own `std` feature turns on. A frame loop built without it
uses `core::task::Waker::noop()` instead, which Rust 1.85 and later provide.

A `serve` future settles at most 32 calls in one poll. When it reaches that
limit, it wakes its own waker and returns `Pending`, so that an executor can run
other tasks before it polls the `serve` future again. Under `noop_waker` that
wake is lost, so a frame loop that polls once per frame settles at most 32 calls
per frame.

The future of a command or a query call, and the `serve` future, panic when
they are polled again after they resolved. The `next_event` future does not:
polled again, it waits for the next occurrence.

## Blocking calls

The `blocking` module holds the same `Client` and `serve` for code that waits on
a thread instead of polling. Each blocking call is
`ridl_rt::task::block_on` over the async call, so the two forms cannot differ
in what a call does; the blocking form only adds the wait and a timeout.

A blocking client takes its timeout with `with_timeout` (or `set_timeout`). The
default is no timeout. A call returns its outcome, or the timeout result when
the timeout passes first. A signal read and a `subscribe_*` call return at
once, as in the async client.

Blocking `serve` takes the handler, the provider and an optional timeout. It
returns `Ok(())` when the timeout passes, so a thread that also does other work
can call it in a loop. `SERVE_PASS` is a `Duration` of 50 ms the program
declares:

```rust,noplayground
{{#include ../../examples/cabin/consumer/src/main.rs:serve-loop}}
```

The cabin program runs that loop on a second thread and calls the blocking
client on the first. `DoneOnDrop` is a type of the program that sets `done`
when it is dropped, so that the serving loop also ends when the first thread
panics:

```rust,noplayground
{{#include ../../examples/cabin/consumer/src/main.rs:blocking-command}}
```

With no timeout, a blocking call returns only with its outcome, blocking
`next_event` only with an event or a read failure, and blocking `serve` only
with a failure. At the timeout, blocking `next_event` returns `Ok(None)`.

## Cargo features

The generated `Cargo.toml` declares two features, both on by default:

| Feature            | Enables                                                                                                                                                                                                                  |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `std`              | The `blocking` module of every face, the `Timeout` trait in the prelude of each interface that has a `blocking` module, and `ridl-rt/std`. With it off, only the async face remains.                                     |
| `validate-pattern` | The check of a typl `match` pattern in a constructor, through the `regex` crate. With it on, the crate links the standard library. With it off, `new` does not check patterns; range and length checks are not affected. |

With `std` and `validate-pattern` both off, the generated crate and `ridl-rt`
are both `no_std` and build for a target that has no standard library. Its
`lib.rs` declares this with
`#![cfg_attr(not(any(feature = "std", feature = "validate-pattern")), no_std)]`.
The crate still needs an allocator: under the same condition its `lib.rs`
links `alloc` under the name `std` (`extern crate alloc as std;`), because the
package files name `::std::string::String` and `::std::vec::Vec` for strings,
bytes, arrays and maps. Single-file mode writes no `lib.rs`, so the crate that
includes the file decides. A crate that is always `no_std` declares
`extern crate alloc as std;` in its own root. A crate that is `no_std` only
under a condition declares it under the same condition: where the alias is
declared, `::std` names `alloc`, so a path that only the standard library has,
such as `::std::sync::LazyLock`, no longer resolves.

A target where `regex` is too large, or one that does not need the blocking
face, turns the defaults off and selects what it needs. This `Cargo.toml`
fragment is an illustration and is not compiled:

```toml
[dependencies]
veh_cabin = { path = "generated", default-features = false }
```

The generated crate depends on `ridl-rt` with its `flatbuffers` feature, which
the codec needs. An application that names `ridl-rt` itself, for example to use
`ridl_rt::task::noop_waker` or the error types, selects from its features:

| `ridl-rt` feature | Default | Enables                                                                                                                                                                           |
| ----------------- | ------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `flatbuffers`     | off     | The `flatbuffers` module the generated codec calls.                                                                                                                               |
| `std`             | off     | The standard library; the `task` module: `block_on`, `noop_waker` and `flag_waker`; the `face::Timeout` trait; and the propagation hook of the `trace` module, `set_propagation`. |
| `proto3`          | off     | Nothing yet. It names the proto3 encoding.                                                                                                                                        |
| `repr-c`          | off     | Nothing yet. It names the `repr(C)` encoding.                                                                                                                                     |

`ridl-rt` has no dependency in any feature combination. It builds for `wasm32`,
and its minimum Rust version is 1.83. The generated crate declares edition 2024,
so it needs Rust 1.85 or later.

## Failures

A call reports its failure as a value. `ClientError`, `CallError`, `Contract`,
`Transport` and `ProviderError` are in `ridl_rt::error`; the port errors, such
as `SendError` and `ReadError`, are in `ridl_rt::port`.

A command or query call resolves to `Result<_, ClientError>`:

- **`ClientError::Send`** — the call was not sent. A `require` clause that is
  false is reported here, as `SendError::Contract(PreconditionFailed)`, because
  the client evaluates the `require` clauses before it sends anything.
- **`ClientError::Call`** — the call was sent, and its outcome is a failure.
  `CallError::Contract` is a contract error the provider side reported:
  `InvalidValue` (the payload breaks its typl constraints), `PreconditionFailed`,
  `ContractBroken` (an `ensure` clause is false) or `UnknownInteraction` (the
  peers disagree on an interface number or an ordinal). `CallError::Transport`
  is an infrastructure failure: `Timeout`, `Undelivered`, `Down`, `Corrupt` or
  `Busy`.
- **`ClientError::Read`** — the port failed while the reply was read.

When a call's `max` bound passes, a sent command resolves to
`Transport::Undelivered` and a sent query to `Transport::Timeout`. The bound is
measured on the port's clock, so it depends on the runtime. The clock of
`ridl-loopback` moves only when the program calls `Loopback::advance`, so over
it a bound passes only after the program advances the clock past it; until
then, a call that is never served returns only at the blocking client's own
timeout.

A signal read returns `Result<Sample<T>, ReadError>`. `serve` resolves to
`ProviderError::Serve` when the handler refuses the interface's members, which
it reports on the first poll, and to `ProviderError::Claim` when the handler
port fails while a call is read. The calls it settled before a failure stay
settled.

**A catalog mismatch panics.** Binding a `Client` or a `Publisher`, and starting
`serve`, compare the port's catalog — the unit name and the catalog hash —
with the `CATALOG` the face was generated from. When they differ, the face was
generated from a different version of the unit than the one the runtime
serves, and the bind panics with a message that names the interface and both
catalogs. A program that must not panic compares the two itself before it
binds. This snippet is an illustration and is not compiled; `port` stands for
the port the program is about to bind:

```rust,noplayground
use ridl_rt::contract::Interface; // carries `CATALOG`
use ridl_rt::port::Attached;      // carries `catalog()`

if port.catalog() != <api::Cabin as Interface>::CATALOG {
    // refuse the port
}
```

The catalog and its hash are described in [The catalog
descriptor](catalog-descriptor.md).

## TypeScript

`ridl build --emit typescript` writes the domain types of a package as one
`.ts` file: branded scalars, interfaces, enums, enum sets, discriminated unions
and init functions. It emits no face, no codec and no feature selection, so
there is no async or blocking API to choose between.

## Further reading

- [The interaction face][face] — the design record of everything the Rust
  backend emits for an interface.
- [ridl-rt][ridl-rt] — the design record of the runtime library: its ports,
  features and error types.

[face]: https://github.com/driftsys/ridl/blob/main/docs/design/interaction-face.md
[ridl-rt]: https://github.com/driftsys/ridl/blob/main/docs/design/ridl-rt.md
