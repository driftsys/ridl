# Using the generated Rust code

`ridl build --emit rust` turns a package into a Rust crate. This chapter shows
how an application uses that crate: the types it holds, the two ways to call an
interface — async and blocking — the Cargo features that select what the crate
contains, and the failures a call can report. The flag itself is described in
the [CLI reference](cli-reference.md#ridl-build).

Every Rust snippet in this chapter but one is taken from
`examples/cabin/consumer/src/main.rs`, a program that the repository's
`just demo` gate builds and runs against the crate generated from
`examples/cabin/cabin.ridl`. The two exceptions are marked as illustrations.
The interface the snippets call, from that schema:

```ridl,ignore
package veh.cabin

type Level: integer [0..100]
type Window: integer [0..100000]
type Average: integer [0..1000]

interface Cabin {
  signal temperature: Temperature @10ms
  event warning: Warning @[100ms..1s]
  command setLevel(level: Level) @[..50ms] [
    require level < 100
  ]
  query average(window: Window): Average @[..200ms] [
    require window > 0
    ensure result >= 0
  ]
}
```

## What the crate contains

The build writes one `<package>.rs` file per package, a `lib.rs` that declares
the module tree, and a `Cargo.toml`. A package `veh.cabin` is the module
`veh::cabin` of a crate named `veh_cabin`:

```rust,noplayground
use veh_cabin::veh::cabin as api; // the package: types and interfaces
use api::cabin;                    // the face of the interface `Cabin`
```

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

The face opens no socket and holds no runtime. Each `Client`, `Publisher` and
`serve` runs over a _port_ that the application passes in, and a port comes from
a runtime. The one runtime in this repository is `ridl-loopback`, which connects
the faces of one process to each other. The cabin program binds every face to
it:

```rust,noplayground
{{#include ../../examples/cabin/consumer/src/main.rs:bind}}
```

`CATALOG` is the catalog the face was generated from. Binding a face compares
the port's catalog with it, and panics when they differ; see
[Failures](#failures).

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
runtime gives the application:

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
hand, the way a frame loop does, with the waker `ridl_rt::task::noop_waker()`:

```rust,noplayground
{{#include ../../examples/cabin/consumer/src/main.rs:async-command}}
```

A `serve` future settles at most 32 calls in one poll. When it reaches that
limit, it wakes itself and returns `Pending`, so that one provider does not hold
a single-threaded executor. A
future that resolved panics when it is polled again.

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
can call it in a loop:

```rust,noplayground
{{#include ../../examples/cabin/consumer/src/main.rs:serve-loop}}
```

The cabin program runs that loop on a second thread and calls the blocking
client on the first:

```rust,noplayground
{{#include ../../examples/cabin/consumer/src/main.rs:blocking-command}}
```

With no timeout, a blocking call returns only with its outcome, blocking
`next_event` only with an event or a read failure, and blocking `serve` only
with a failure. At the timeout, blocking `next_event` returns `Ok(None)`.

## Cargo features

The generated `Cargo.toml` declares two features, both on by default:

| Feature            | Enables                                                                                                                                                                          |
| ------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `std`              | The `blocking` module of every face, and `ridl-rt/std`. With it off, the crate and `ridl-rt` build as `no_std`, and only the async face remains.                                 |
| `validate-pattern` | The check of a typl `match` pattern in a constructor, through the `regex` crate. With it off, `new` does not check patterns; range and length checks are not affected. |

A target without the standard library, or one where `regex` is too large,
turns the defaults off and selects what it needs:

```toml
[dependencies]
veh_cabin = { path = "generated", default-features = false }
```

The generated crate depends on `ridl-rt` with its `flatbuffers` feature, which
the codec needs. An application that names `ridl-rt` itself, for example to use
`ridl_rt::task::noop_waker` or the error types, selects from its features:

| `ridl-rt` feature | Default | Enables                                                                                                    |
| ----------------- | ------- | ---------------------------------------------------------------------------------------------------------- |
| `flatbuffers`     | off     | The `flatbuffers` module the generated codec calls.                                                        |
| `std`             | off     | The standard library, and the `task` module: `block_on`, `noop_waker` and `flag_waker`.                    |
| `proto3`          | off     | Nothing yet. It names the proto3 encoding.                                                                 |
| `repr-c`          | off     | Nothing yet. It names the `repr(C)` encoding.                                                              |

`ridl-rt` has no dependency in any feature combination. It builds for `wasm32`,
and its minimum Rust version is 1.83.

## Failures

A call reports its failure as a value. The types are in `ridl_rt::error`.

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
measured on the port's clock, so it depends on the runtime: `ridl-loopback`
measures no bound, so over it a call that is never served returns only at the
blocking client's own timeout.

A signal read returns `Result<Sample<T>, ReadError>`. `serve` resolves to
`ProviderError` when the handler port fails; the calls it settled before the
failure stay settled.

**A catalog mismatch panics.** Binding a `Client` or a `Publisher`, and starting
`serve`, compare the port's catalog — the package name and the catalog hash —
with the `CATALOG` the face was generated from. When they differ, the face was
generated from a different version of the package than the one the runtime
serves, and the bind panics with a message that names the interface and both
catalogs. A program that must not panic compares the two itself before it
binds. This snippet is an illustration and is not compiled:

```rust,noplayground
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
