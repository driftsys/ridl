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
the face was generated from, and `Provenance` is read from a signal sample. A
`Loopback` carries FlatBuffers, which is its default encoding; the face takes
the encoding from the port it is bound to and names none itself.

The package module holds:

- **The domain types.** A struct, an enum, an enum set, a union and a named
  scalar each become a Rust type. A constrained type is built with `new`, which
  checks the typl constraints and returns an error when the value breaks one;
  `new_unchecked` skips the check and is for a value already known to satisfy
  them. `get` returns the value of a named scalar.
- **The codecs** of each payload type, one per encoding the manifest names
  under `[backend.rust] encodings`: `flatbuffers`, `proto3`, or both. Each
  codec is `encode`, `verify`, `decode` and a `MAX_SIZE` bound; with no list
  the module holds the types only. Each payload type also has a
  `<T as Payload<FlatBuffers>>::Buffer` type and a zeroed `BUFFER` value, sized
  by that type's own `MAX_SIZE`, and the same in `Proto3`. In proto3 a type
  whose encoding has no bound, such as a `string` with no length bound, gets a
  codec without a size: it implements `ridl_rt::payload::Codec<Proto3>`, with
  no `MAX_SIZE` and no `BUFFER`, and the caller supplies a buffer that is large
  enough. The face calls the codec; an application calls it only when it moves
  payloads by itself.
- **The interaction descriptors**: one unit type per interface (here
  `api::Cabin`) that implements `ridl_rt::contract::Interface` and carries the
  interface's `CATALOG`, `NUMBER` and `NAME`. Each interface also implements
  `ridl_rt::contract::Sizes<E>` for each encoding the package carries, the
  size table of that codec for the interface: its reservations, its largest
  buffer (`MAX_BUFFER_SIZE`), and the claim and event buffers the face holds.
  An interface with a payload that has no proto3 bound implements
  `ridl_rt::contract::Bounds<Proto3>` instead of `Sizes<Proto3>`, and its face
  cannot be bound to a proto3 port. Like the codecs, the descriptors are
  present only when the manifest names an encoding under
  `[backend.rust] encodings`.
- **The face**: one module per interface that has one, named after it (here
  `api::cabin`), present under the same condition. The face is what the
  application calls.

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

An interface that carries only `fixed` declarations gets no face module, and
neither does an `internal` interface, whose descriptors are crate-visible (the
[design record][consumer-face] gives the reason). A signal-only interface gets a
`Client` and a `Publisher` and no `blocking` module, because a signal read
returns at once.

An interface the face cannot carry gets none of these items and no interaction
descriptor. In their place, the package module holds a
`__RIDL_NO_FACE_<NAME>` constant whose documentation names the interface and
the reason; the [CLI reference](cli-reference.md#ridl-build) lists the cases.

A call always sends one named type. A command or query with exactly one
parameter of a named type sends that type. Any other parameter list gets an
argument struct that the compiler names `<Interface><Call>Params`, with one
field per parameter in declaration order, each carrying the parameter's
documentation. A call with no parameter sends the same kind of struct with no
field. A query whose reply is a tuple, or that declares no return type, gets a
reply struct named `<Interface><Call>Reply` in the same way. These structs are
generated types of the package module, and the face methods take and return
them like any other payload.
The wire backends emit the matching positional `.proto` messages and `.fbs`
tables under the names assigned by lowering. When the manifest names
`flatbuffers` and an induced payload has no finite FlatBuffers bound, its
interface is skipped with the refusal constant described above rather than
emitting a descriptor that names a missing codec. The proto3 codec has a codec
for every induced payload, so it skips no interface.

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

The generated crate depends on `ridl-rt` with one feature per encoding that the
manifest names under `[backend.rust] encodings` — `flatbuffers`, `proto3`, or
both, which the codecs need — and no feature for a types-only build. An application that names `ridl-rt` itself, for example to use
`ridl_rt::task::noop_waker` or the error types, selects from its features:

| `ridl-rt` feature | Default | Enables                                                                                                                                                                           |
| ----------------- | ------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `flatbuffers`     | off     | The `flatbuffers` module the generated codec calls.                                                                                                                               |
| `std`             | off     | The standard library; the `task` module: `block_on`, `noop_waker` and `flag_waker`; the `face::Timeout` trait; and the propagation hook of the `trace` module, `set_propagation`. |
| `proto3`          | off     | The `proto3` module the generated proto3 codec calls.                                                                                                                             |
| `repr-c`          | off     | Nothing yet. It names the `repr(C)` encoding.                                                                                                                                     |

`ridl-rt` has no dependency in any feature combination. It builds for `wasm32`,
and its minimum Rust version is 1.83. The generated crate declares edition 2021
and builds with Rust 1.83 or later with every cargo feature on or with default
features off. To keep Rust 1.83, resolve its dependencies as
[Stability](stability.md) describes. When
you build the generated crate as its own workspace root, its edition 2021
selects resolver 2, which by default does not read `rust-version`, and its
manifest declares no `rust-version`. Resolve with cargo 1.84 or later and
`CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback`; cargo then selects the
dependency releases that the resolving Rust supports. To resolve for Rust 1.83
with a later cargo, first add `rust-version = "1.83"` to the emitted manifest,
as `just compat-check` does.

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

## Documentation in the generated code

The doc comment of a declaration or a member (see
[Documenting your API](documenting.md)) becomes the doc comment of the item
generated from it: rustdoc in the Rust crate, TSDoc in the TypeScript module.
The generated doc also lists the item's _rules_: the facts the schema states
about the values the item admits and the way it is used. They are the same
lines the editor's hover shows in its **Contract** list.

A generated doc holds these parts, in this order, and leaves out each part that
is empty:

| Part          | Rust                                       | TypeScript                                   |
| ------------- | ------------------------------------------ | -------------------------------------------- |
| doc text      | `///` lines; each link an intra-doc link   | the comment's text; each link `{@link Name}` |
| rules         | a `# Rules` section, one bullet per rule   | `@remarks`, then a `Rules:` list             |
| typl tags     | none                                       | `@unit`, `@range`, `@bounds`                 |
| `@see`        | a `# See also` section, one bullet each    | one `@see` tag each                          |
| `@since`      | one `Available since <version>.` line each | one `@since` tag each                        |
| `@deprecated` | the `#[deprecated]` attribute              | one `@deprecated` tag                        |

A link, or an `@see` target, whose target has no item in the same output is
written as a code span of the target's qualified name, such as
`veh.common.Speed`, so that no doc in the crate or the module that `ridl build`
emits holds a link that does not resolve. Both backends link only to a
declaration of the package itself or to a declaration of another package that
this package reaches through a type. A target that this package does not reach
through a type is written as a code span. In both languages, so is an
`internal` declaration, a constant the backend does not emit (such as a `bytes`
constant), and a member of a named scalar or of a constant, which has no member
item. In Rust it is also the case for an event, a `fixed`, an interface with no
face module, a target whose path Rust spells with a raw identifier (`r#move`),
and an item whose name the module of a child package takes, which a program
reaches only through the hidden `__ridl_package` module. In TypeScript it is
also the case for an interface, an interaction and a union arm, which the module
holds no item for, and for a declaration of another package that the module
does not import. `@labels` is not rendered.

A Rust link uses the short form `[Speed]` when that text is the target's Rust
name and the target is in the module that holds the doc. When a Markdown
reference definition uses the same label, the generated shortcut uses code
formatting to preserve the resolved target. Other links use an
absolute path from the crate root, such as `crate::veh::common::Speed`, which
resolves in the crate `ridl build` emits. A program that brings a generated file
in with `include!` inside a module of its own moves the items away from those
absolute paths, so rustdoc reports those links as unresolved.

In the Rust face, the doc of an interaction goes on these methods: for a signal,
the method of the async `Client`, of the blocking `Client` and of the
`Publisher`; for a command or a query, the method of the async `Client`, of the
blocking `Client` and of the `Provider` trait; for an event, the
`subscribe_<event>` method of the async and of the blocking `Client`, and the
method of the `Publisher`. The `Client` and `Provider` methods also carry the
interaction's `#[deprecated]`; a `Publisher` method and a `subscribe_<event>`
method carry the doc without `#[deprecated]`. The variant of an event in the
`Event` enum carries only a sentence the backend writes. A method's doc is the
interaction's doc text, then a sentence the backend writes about the method
itself, then the `# Rules` section, then, for a command or a query, a
`# Parameters` section (one bullet per parameter with its name and its doc, and
the parameter's rules as nested bullets), then the `# See also` section and the
`Available since` lines. The doc of an interface goes on its face module, in
the same order around the module's own sentence.

### What the generated code checks

The rules lists state the contract, not what the code checks. On each item the
backend emits, every rule the schema declares is listed, in the same words in
both languages, whether or not the generated code checks it. The sentence the
backend writes about a method describes what that method does, its checks
included: the doc of a named scalar's `new` says that it returns an error for a
value that breaks a constraint the generated code checks, the doc of a `Client`
call method says what happens when a
`require` clause fails, and the doc of a query's `Provider` method says that
`serve` discards a reply that breaks an `ensure` clause.

This table records, for each kind of rule, the generated item that checks it:

| Rule             | Rust                                                      | TypeScript  |
| ---------------- | --------------------------------------------------------- | ----------- |
| range            | checked by `new`, `TryFrom` and `verify`                  | not checked |
| step             | checked by `new`, `TryFrom` and `verify`                  | not checked |
| unit             | not checked                                               | not checked |
| length           | `new`, `TryFrom`, `verify`; `encode` for anonymous fields | not checked |
| pattern          | checked by `new`, `TryFrom` and `verify`, under a feature | not checked |
| collection bound | checked by `encode` and `verify` | not checked |
| timing           | not checked                                               | not checked |
| response bound   | `max` checked by the future of a `Client` call            | not checked |
| errors           | not checked                                               | not checked |
| `require`        | checked by the call methods of `Client` and by `serve`    | not checked |
| `ensure`         | checked by `serve`                                        | not checked |

In the Rust column:

- `new` and `TryFrom` are the constructors of a named scalar. `verify` is the
  FlatBuffers codec's check of a received payload. It also checks a field whose
  type is written inline, which has no constructor. `encode` also checks
  anonymous string and byte lengths and variable-length collection counts.
  Exact array counts are enforced by their Rust type. Other field constraints
  and map-key uniqueness are checked by `verify`.
- The pattern is checked only when the crate is built with the
  `validate-pattern` feature (see [Cargo features](#cargo-features)).
- A call's future, in the async and in the blocking `Client`, resolves to a
  failure when the response bound's `max` passes (see [Failures](#failures)).
  Its `min` is not checked.
- A query that declares an error type gets no face, so no generated item
  carries the errors rule.
- A `Client` call method evaluates the `require` clauses before it sends.
  `serve` evaluates them before it calls the `Provider`, and evaluates the
  `ensure` clauses on the reply before it sends the reply. The `Client` does not
  evaluate `ensure`.
- `verify` and the face exist only when the manifest names `flatbuffers` or
  `proto3` under `[backend.rust] encodings` (see
  [What the crate contains](#what-the-crate-contains)).

The TypeScript module holds types, constants, the `<Name>Bits` objects and init
functions only, and none of them checks a value.

An item that the generated code needs but a program does not name carries no
doc. In Rust it is `pub(crate)`, such as the constraint check behind `new` and
the codec's functions for each table, or `pub` under `#[doc(hidden)]` where code
outside the crate must reach it, such as the descriptor of each interaction,
which a runtime reads. Two private Rust constants are notes rather than such
items: `__RIDL_NO_FACE_<NAME>`, left where an interface's face was not emitted,
and `__RIDL_FB_NO_CODEC_<Name>`, left where a declaration's FlatBuffers codec
was not emitted. Each carries a doc that says what was not emitted and why, for
a reader of the generated source; rustdoc does not show them.

## Further reading

- [The interaction face][face] — the design record of everything the Rust
  backend emits for an interface.
- [ridl-rt][ridl-rt] — the design record of the runtime library: its ports,
  features and error types.

[face]: https://github.com/driftsys/ridl/blob/main/docs/design/interaction-face.md
[consumer-face]: https://github.com/driftsys/ridl/blob/main/docs/design/interaction-face.md#the-consumer-face
[ridl-rt]: https://github.com/driftsys/ridl/blob/main/docs/design/ridl-rt.md
