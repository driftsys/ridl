# Writing a port

This chapter is for the author of a runtime: a crate that implements the port
traits of `ridl-rt` for a platform or a transport, so that a generated face can
run over it. It names the traits by role, shows how to test an implementation
with the published conformance suite, and lists what a runtime that carries
frames over a transport must decide for itself. The exact signature of every
trait is in the [`ridl_rt::port` documentation][port-docs].

## The runtime and the face

A generated face holds no runtime. Each `Client`, `Publisher` and `serve` of
the face runs over a value that implements some of the traits of
`ridl_rt::port`, and calls only those traits; it never names the runtime that
implements them ([Using the generated Rust code](generated-code.md#the-face)).
A runtime is a separate crate that implements those traits.

Three properties hold for every port method. It returns without waiting: a face
that waits registers its interest with the runtime and reads the port again
when the runtime wakes it. It names no payload type: it carries an interface
number, an ordinal and bytes, and the generated code encodes and decodes. It
takes no current time: the runtime reads its own clock.

A runtime exposes one handle type for each port trait it implements, and it may
also offer an aggregate handle that implements every trait one interface's face
needs. `ridl-rt` already implements every port trait for `&mut P`, and the
traits whose methods all take `&self` also for `&P`, so a runtime does not
write those implementations. No port trait has `Send` or `Sync` as a
supertrait. A runtime whose handles are used from more than one thread makes a
handle `Send + Sync` when all its traits take `&self`, and `Send` when one of
them has a `&mut self` method, and checks this itself.

## The port traits

| Trait              | Role                                                          | Used by                                                            |
| ------------------ | ------------------------------------------------------------- | ------------------------------------------------------------------ |
| `Attached`         | names the catalog the port serves                             | the face, when it binds; a supertrait of every interaction port    |
| `Clock`            | gives the time envelopes are stamped with                     | the runtime; a `Client` with calls, to measure a call's bound      |
| `SignalReader`     | reads a signal's current value                                | the consumer: `Client`                                             |
| `SignalWriter`     | stages signal changes and publishes them with `commit`        | the producer: `Publisher`                                          |
| `EventSource`      | subscribes to events and takes the next occurrence            | the consumer: `Client`                                             |
| `EventSink`        | raises an occurrence                                          | the producer: `Publisher`                                          |
| `Caller`           | sends a command or a query and reads its outcome              | the consumer: `Client`                                             |
| `Handler`          | serves members, takes each call, and settles it               | the provider: `serve`                                              |
| `FixedReader`      | reads a provisioned `fixed` value                             | the application, directly; the face has no method over it          |
| `ScannableSignals` | walks the signals that changed since a mark (extension)       | a consumer that scans a store; no generated face calls it          |
| `CoherentSignals`  | reads several signals from one publication (extension)        | a consumer that needs one coherent set; no generated face calls it |
| `Wakeable`         | wakes a waiting task when a port may have changed (extension) | a `Client` with events or calls, and `serve`                       |

**`Attached` and `Clock`.** `Attached` has one method, `catalog`, which returns
the `CatalogRef` the port serves; every interaction port has it as a
supertrait. `Clock` has one method, `now`. A runtime has one clock and stamps
every envelope from it. Neither method returns an error.

**Signals.** `SignalReader::read` copies a signal's current value into a buffer
and returns its provenance, its freshness and its envelope. `SignalWriter`
stages a change with `set`, `invalidate` or `touch`, and `commit` publishes
every staged change with one timestamp per interface, so that several signals
of one interface updated together appear as one publication. A read fails with
`ReadError`, a staged change with `WriteError`; `commit` cannot fail.

**Events.** `EventSource` subscribes to and unsubscribes from the events of an
interface, and `next` takes the next occurrence of anything subscribed, with
its interface and its ordinal. `EventSink::raise` sends one occurrence; an
occurrence is not staged and has no `commit`. A subscription fails with
`SubscribeError`, `next` with `ReadError`, and `raise` with `RaiseError`.

**Calls.** `Caller` sends a command or a query and returns a `Correlation`, the
identifier of that call to its caller. `ack` reads a command's acknowledgment
and `reply` a query's reply, once each is known, and `forget` releases a
correlation whose outcome the caller no longer needs. `Handler::serve` starts
presenting the calls to the listed members, `next_claim` presents each
delivered call once as a `Claim`, and `settle` gives the claim its outcome:
the reply bytes, or the `CallError` the caller sees. A send fails with
`SendError`, a `serve` with `ServeError`, `next_claim` and `reply` with
`ReadError`, and a settlement with `SettleError`. The outcome of a call is a
`CallError` from `ridl_rt::error`, not a port error.

**Fixed.** `FixedReader::read_fixed` copies a provisioned value into a buffer.
It fails with `ReadError`. How a value is provisioned into the runtime is the
runtime's own API, not a port.

**Extensions.** A runtime may omit `ScannableSignals` and `CoherentSignals`,
because they describe mechanisms that some runtimes have.
`ScannableSignals` reports a generation counter for each interface and writes
the signals that changed since a `Watermark`; neither method returns an error.
`CoherentSignals::read_coherent` answers several ordinals of one interface from
one publication and fails with `ReadError`. A runtime whose transport delivers
each field separately cannot present it. `Wakeable::wake_on` stores a waker
for an `Interest` — a call's outcome, a free call slot, an event, or a claim —
and wakes it when that interest may have changed. A runtime that serves a
generated async client implements `Wakeable`, because every face future that
waits is bounded on it.

The documentation of the event and call traits also states how a runtime
delivers the trace context a sender passes. Every error enum of
`ridl_rt::port` is `#[non_exhaustive]`.

## Testing a port

The crate `ridl-rt-conformance` holds the port contract tests as functions
generic over a runtime. A runtime runs the whole suite from its own tests. Add
it as a dev-dependency at the same version as `ridl-rt`, because its public
surface follows the port contract of that version. A patch release can add
tests to the suite, so a runtime that wants a fixed set of tests pins the crate
with `=`. This `Cargo.toml` fragment is an illustration and is not compiled:

```toml
[dependencies]
ridl-rt = "0.7"

[dev-dependencies]
ridl-rt-conformance = "=0.7.0"
```

In the test crate, implement the `Factory` trait on a type of that crate — a
unit struct is enough, and the orphan rule forbids implementing it on the
runtime's own type from outside the runtime. `Runtime` is one value that
implements `Attached`, `Clock`, `SignalReader`, `SignalWriter`, `EventSource`,
`EventSink`, `Caller` and `Handler`: an aggregate handle. `Source`, `Caller` and
`Handler` are a second event source, caller and handler on the same runtime.
`SLOTS` is the number of calls the runtime holds at once, at least 1; the tests
fill the call table, so a runtime with no bound cannot run them. `runtime`
builds a runtime attached to a catalog, with a clock that moves only when
`advance` moves it. `fail_next_settle` makes the next settlement fail once.

Then call `suite!`, which writes one `#[test]` per test. The flags after the
semicolon add the tests of the extensions the runtime implements: `scannable`,
`coherent` and `wakeable`, and `trace` for a runtime that carries the trace
context. `suite!(MyFactory)` alone runs the tests of the ports every runtime
presents. This skeleton is an illustration and is not compiled; `MyRuntime` and
the handle types stand for the runtime's own types:

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

ridl_rt_conformance::suite!(MyFactory; scannable, coherent, wakeable, trace);
```

The `wakeable` flag also requires that the three role handles implement
`Wakeable`, and that the factory and the handles are `'static`, because one
test runs on a thread of its own. The working example is
[`crates/ridl-loopback/tests/conformance.rs`][loopback-conformance], which runs
every test of the suite, with all four flags, over `ridl-loopback`.

The suite does not test every case. The one list of
what it leaves out is the section [What the suite leaves out][leaves-out] of the
crate documentation; a runtime that wants one of those cases tested keeps its
own test of it.

## What a binding decides

A runtime that carries frames to a second process does so through a _binding_:
the [frame specification](reference/frame.md) carried over one transport. A
binding is written from that specification, the [`ridl-rt` design record][ridl-rt]
for the exact shape of each name, and the projection record of the encoding its
path carries. Its own document states the
nine items of [§10](reference/frame.md#10-what-a-binding-decides), each a
choice the frame leaves to the transport:

1. How a consumer reaches a provider, what opens and ends the session, and what
   an ended session is on that transport, so that `Detached` and
   `Transport::Down` have a cause.
2. The byte layout, order and width of each header field, and how the frame's
   form is carried. An interface number or an ordinal keeps its 32-bit range,
   or the binding refuses it at attach time.
3. Which payload encoding the path carries, as an encoding tag.
4. Where one frame ends and the next starts on a stream, how a payload's length
   is carried, and what happens when that framing is lost.
5. What the provider derives a caller's identity from for duplicate
   suppression, and whether that identity survives a reconnect.
6. How long a settled response is kept for a call that has no response bound.
7. Whether the frames of one commit cross as one unit, and so whether a runtime
   over the binding presents `CoherentSignals`.
8. On a path that maps a buffer instead of sending a frame, which header fields
   are in the mapped region's own layout and which are in a frame beside it.
   The control plane is framed on every binding.
9. The answer to a call that arrives faster than its rate floor, when the
   binding enforces one.

A binding adds nothing to the frame. It does not change the field set, the
meaning of a field, what a receiver does with a frame, the per-kind presence
rules, the invalid payload rules, or the control-plane operations. [§11](reference/frame.md#11-the-bindings) names the bindings, and
[§12](reference/frame.md#12-conformance) lists the sixteen statements a second
implementation — a runtime, a binding, or both — shows with its own tests to
conform.

No binding is built in this repository. The WebSocket transport is planned and
not built, and on Android a runtime binds the ports over its own binder
contract, outside this repository. `ridl-loopback` runs in process and speaks no
frame. The frame chapter is a specification, not a description of shipped
behaviour; see the note at the top of [frame](reference/frame.md).

## Attaching to a catalog

A port attaches to one catalog. The rule for attaching is
[§6.1](reference/frame.md#61-attach-and-attached) of the frame specification.

## Further reading

- [`ridl_rt::port`][port-docs] — the exact shape of every port trait and error.
- [`ridl-rt-conformance`][conformance-docs] — the `Factory` trait, the `suite!`
  macro, and what the suite leaves out.
- [ridl-rt][ridl-rt] — the design record of the runtime library, including its
  ports.

[port-docs]: https://docs.rs/ridl-rt/latest/ridl_rt/port/index.html
[conformance-docs]: https://docs.rs/ridl-rt-conformance/latest/ridl_rt_conformance/
[leaves-out]: https://docs.rs/ridl-rt-conformance/latest/ridl_rt_conformance/#what-the-suite-leaves-out
[loopback-conformance]: https://github.com/driftsys/ridl/blob/main/crates/ridl-loopback/tests/conformance.rs
[ridl-rt]: https://github.com/driftsys/ridl/blob/main/docs/design/ridl-rt.md#the-ports
