# Observation from the generated Rust face — design

Status: draft for review, 2026-10-07. Tracking issue: driftsys/ridl#754.

## Goal

Make a running RIDL system observable for diagnosis with Perfetto and for health
campaigns on long-running processes, without any cost to a production build that
does not ask for it.

Three kinds of observation are in view:

1. **Spans** for each call, claim, raise and occurrence, shown as slices in
   Perfetto.
2. **Warning and error events** for failures that the application cannot see.
3. **Metrics**: counters and latency buckets for each interface member, sampled
   by a reader, shown as Perfetto counter tracks and collected by health
   campaigns.

Items 1 and 2 are **phase 1** and are the subject of this design. Item 3 is
**phase 2**. The section "Phase 2: metrics" records the decisions already taken
for it, and phase 2 gets its own issue and plan.

## What the user asked for

- Diagnosis with Perfetto: spans, and warnings and errors on the timeline.
- Health campaigns that answer questions such as "process X got slower after
  three days" and "events between A and B started being dropped".
- Levels, so that a production build pays nothing for what it does not enable.
- Metrics behind their own feature, in a second phase.
- No dependency on the OpenTelemetry crates, in RIDL or in an application that
  does not choose them.

## Three use cases for trace context

| Case                                                                    | What RIDL does                                                           | Ids on the wire           |
| ----------------------------------------------------------------------- | ------------------------------------------------------------------------ | ------------------------- |
| 1. Perfetto, no W3C ids                                                 | `tracing` spans only; nesting inside a process comes from `tracing`      | `None`                    |
| 2. The application uses OpenTelemetry                                   | Calls the application's propagation hook, implemented with OpenTelemetry | The application's W3C ids |
| 3. The application uses another, lighter telemetry library with W3C ids | Calls the same hook, implemented with that library                       | The application's W3C ids |

Cases 2 and 3 need the same mechanism: a hook that does not depend on any
telemetry library (D-6). No case needs RIDL to create W3C ids itself.

## Constraints

- `ridl-rt` keeps no dependency in any feature combination (ADR-0020 decision 5,
  ADR-0021 decision 8). Nothing in this design adds a dependency to `ridl-rt`.
- The generated crate is `no_std` capable and allocates nothing on the hot path.
  With every observation feature off, the compiled code of the generated crate
  is the same as today.
- The targets include QNX Neutrino (`aarch64-unknown-nto-qnx710`,
  `x86_64-pc-nto-qnx710`). These are Tier 3 Rust targets with `std`.

## Decisions

### D-1. The generated code calls a private `observe` module, which forwards to `tracing`

The generated crate gains a private module, `observe`. The generated code
invokes only the items of that module. Only that module names `tracing`.

- With the `tracing` feature on, the module forwards to the `tracing` crate.
- With the feature off, every item of the module expands to nothing.

The reason for the indirection is that a later backend, for example a port in
`ridl-rt` for targets without an allocator, replaces one module and not every
call site. See "Alternatives considered".

Span names are fixed per call site in `tracing`: a name is a `&'static str` in
the static metadata of the call site. A single shared function would give every
member the same span name. So the module defines `macro_rules!` macros (for
example `observe::call_span!`), and the generated code invokes them once per
member. Each member has its own call site, and the shape of every span is still
defined in one place.

### D-2. Two Cargo features, `tracing` and `trace-context`, both off by default

The manifest that `ridlc` renders (`render_cargo_toml` in
`crates/ridlc/src/lib.rs`) gains:

```toml
[features]
tracing = ["dep:tracing"]
trace-context = ["std"]
std = ["ridl-rt/std", "tracing?/std"]

[dependencies]
tracing = { version = "0.1", default-features = false, optional = true }
```

- `tracing` turns on the spans of D-4 and the events of D-5.
- `trace-context` turns on the calls to the propagation hook of D-6. It does not
  need `tracing`, so case 3 works with a telemetry library that does not consume
  `tracing` spans. It needs `std`, because the hook does (D-6).
- The two features are independent. Any combination is valid.
- `tracing` is optional, as `regex` already is (TYPL-220).
- `default-features = false` keeps the generated crate usable on `no_std`
  targets that have an allocator. `tracing` without `std` needs `alloc` and
  allows only a global dispatcher.
- The generated source with the feature off is **not** byte-identical to today:
  the call sites of the `observe` macros stay in the source and expand to
  nothing. The compiled code is the same. The statement "byte-identical" in
  driftsys/ridl#754 is replaced by "the same compiled code".

### D-3. Levels: two filters, both owned by the application

1. **At compile time:** the application selects the maximum level with the
   `tracing` features `max_level_*` and `release_max_level_*`. A call site above
   that level is compiled out.
2. **At run time:** the installed subscriber filters the rest. A call site that
   is compiled in but disabled costs one check of a cached value in a static.

The design assigns a level to each observation. The application chooses the
filters. The book documents two settings:

- **Field:** `tracing` feature on, `release_max_level_warn`. Errors and warnings
  stay, spans are compiled out.
- **Diagnosis:** `tracing` feature on, no level feature. Everything is compiled
  in, and the subscriber filters at run time.

### D-4. Spans

All spans are at level `trace`.

| Span                  | Opened                                                          | Closed                                                                | Fields                             |
| --------------------- | --------------------------------------------------------------- | --------------------------------------------------------------------- | ---------------------------------- |
| Call (command, query) | When the generated `Client` method sends                        | When the future yields its outcome, or when it is dropped             | identity, `outcome`                |
| Claim                 | When `dispatch` reads the claim                                 | When the handler returns, or when the claim is settled with a failure | identity, `outcome`, remote parent |
| Raise                 | Around the `EventSink::raise` call of the generated `Publisher` | After `raise` returns                                                 | identity, `outcome`                |
| Occurrence            | When `poll_next_event` reads the occurrence                     | When the event is decoded, or the read fails                          | identity, `outcome`, remote parent |

Rules:

- **The call span lives in the call future.** The client methods return
  hand-written future structs (`face/futures.rs`), not `async fn`. The struct
  gains a field, present only with the feature on, that holds the span, and
  every `poll` enters the span for the duration of that poll. This is what
  `tracing::Instrument` does. A span guard is never held across a suspension
  point.
- **A command's claim span covers the handler.** `dispatch` settles a command
  `Ok` before it calls the provider. The span stays open until the provider
  method returns, so a slow handler is visible.
- **The occurrence span covers receipt and decoding only.** The decoded event is
  returned to the application, which handles it after the future completes. The
  handling is not inside the span. Making it so would change the type the future
  returns, which is out of scope.
- **A blocking client is covered.** The `blocking` module is `block_on` over the
  same futures.
- **Identity fields** come from the static descriptors that the generated code
  already references: `<Iface as Interface>::CATALOG.name`,
  `<Iface as Interface>::NAME`, and `MEMBER.name` and `MEMBER.kind` of
  `<Descriptor as Interaction>::MEMBER`. They are `&'static str`. No payload
  byte is ever recorded.
- **`outcome`** is a static string: `ok`, or the variant of the error, for
  example `Transport::Timeout` or `Contract::PreconditionFailed`. A call future
  dropped before its outcome records `cancelled`.

**Names and keys.** The span name follows the OpenTelemetry convention for its
kind: `<Interface>/<member>` for calls and claims (RPC), and
`publish <Interface>.<member>` or `process <Interface>.<member>` for events
(messaging). The fields use the OpenTelemetry keys:

| Kind              | Fields                                                                                                                      |
| ----------------- | --------------------------------------------------------------------------------------------------------------------------- |
| Call, claim       | `rpc.system = "ridl"`, `rpc.service`, `rpc.method`, `ridl.catalog`                                                          |
| Raise, occurrence | `messaging.system = "ridl"`, `messaging.destination.name`, `messaging.operation.type` (`send` or `process`), `ridl.catalog` |

The target of every call site is `ridl::<catalog>`, so a subscriber can filter
one catalog with an `EnvFilter` directive.

### D-5. Warning and error events only for failures the application cannot see

The application receives every client-side failure as a `Result`. For those, the
outcome is a field of the call span, and no event is emitted. This avoids two
problems: duplicates of the application's own logging, and a flood of events
when a transport goes down under load, because `tracing` has no rate limiting.

Events are emitted only where the generated `dispatch` handles a failure that
the provider application never sees:

| Failure in `dispatch`                              | Settled with                   | Level   |
| -------------------------------------------------- | ------------------------------ | ------- |
| Claim for another interface, or an unknown ordinal | `Contract::UnknownInteraction` | `warn`  |
| Arguments that do not decode or verify             | the decode error               | `warn`  |
| `ReadError::ShortClaim`                            | `Transport::Corrupt`           | `error` |
| `ensure` fails after the provider returned         | `Contract::ContractBroken`     | `error` |
| `Handler::settle` refuses the settlement           | (none)                         | `warn`  |

`Contract::PreconditionFailed` on the provider side is a normal reply to the
caller, so it is recorded only in the claim span's `outcome`. `ContractBroken`
is at `error` because it means the provider's own code broke its contract.

Each event carries the identity fields of D-4 and the claim's remote parent
fields, so the event can be found from either side of the call.

### D-6. Trace context: an application hook in `ridl-rt`

`ridl-rt` carries an optional `TraceContext` (W3C ids: `trace_id`, `span_id`,
`flags`) on `Claim`, `RawOccurrence`, `Caller::command`, `Caller::query` and
`EventSink::raise` (ADR-0021 decision 21). A `tracing` span has only a local
`u64` id, which the subscriber assigns and reuses after the span closes, so it
cannot be sent on the wire. RIDL does not create W3C ids itself. The ids come
from the application, through a hook.

**The hook.** `ridl_rt::trace` gains, behind its `std` feature and with no
dependency:

```rust
pub trait Propagation: Sync {
    /// Called just before a call or a raise is sent.
    /// Returns the W3C context to send, or `None`.
    fn current(&self) -> Option<TraceContext>;

    /// Called when a claim is received, before the claim span is created
    /// and before the handler runs.
    /// `received` is the context that came over the wire.
    fn enter(&self, received: Option<TraceContext>);

    /// Called after the handler has returned, and also when it panics.
    fn leave(&self);
}

/// Registers the hook once for the process. A second call returns an error.
pub fn set_propagation(p: &'static dyn Propagation) -> Result<(), AlreadySet>;
```

**Disabled by default.** An application does not have to do anything. The
`trace-context` feature is off by default, and with the feature on, no hook is
registered until the application calls `set_propagation`. Until then, `None` is
sent, as today.

An application that wants propagation implements `Propagation` with its own
telemetry library and registers it once at start-up, as `log::set_logger` does
for the `log` crate. RIDL calls the hook at fixed points and carries the bytes.
Which span is current, how ids are created and where traces are exported belong
to the application's library.

- **One hook per process**, in `ridl-rt`, so that every generated crate in the
  process uses the same one. A handler in one generated crate that calls a
  client of another generated crate continues the same trace.
- **`std` only.** Storing a function or trait object that is set at run time,
  without `unsafe` and without `std`, is not possible with `core` atomics, and
  `ridl-rt` has no `unsafe` code. The hook is held in a `OnceLock`. An
  application with a telemetry library has `std`.
- **The signature is fixed.** The `enter` and `leave` pair lets an OpenTelemetry
  implementation make the received context the remote parent before the claim
  span is created (`tracing-opentelemetry` chooses a span's parent at creation,
  and its `set_parent` returns `AlreadyStarted` once the span is entered). The
  first step of the plan was a spike that implemented the hook with
  `tracing-opentelemetry` in a test. The signature above did not change; the
  call order below is what the spike fixed.

**Call order.** Proven by `crates/ridl-backend-rust/tests/otel_propagation.rs`
against `tracing-opentelemetry` 0.34.0, with `opentelemetry` and
`opentelemetry_sdk` 0.33.0, and with the layer's default configuration. The test
runs `dispatch` inside an enclosing application span, which is the case most
likely to give the claim span the wrong parent. `dispatch` calls the hook and
opens the claim span in this order:

1. `enter(claim.trace)`, before the claim span exists. An OpenTelemetry hook
   attaches a context that carries the received span context
   (`Context::current().with_remote_span_context(..).attach()`), and keeps the
   guard.
2. Create the claim span with no `parent:` argument, and enter it.
3. Run the handler.
4. Exit and drop the claim span.
5. `leave()`. An OpenTelemetry hook drops the guard it kept in step 1.

The guard that calls `leave` is created before the claim span and dropped after
it, so steps 4 and 5 keep this order on a panic as well.

Why this order, from the source of `tracing-opentelemetry` 0.34.0
(`src/layer.rs`): `OpenTelemetryLayer::on_new_span` chooses the parent once,
when the `tracing` span is created, through `parent_context`. A span with no
`parent:` argument takes `opentelemetry::Context::current()`, the attached
context, when the layer's context activation is on; it is on by default since
0.32.0 (`with_context_activation`). A span created with `parent: None` takes an
empty context. The OpenTelemetry span itself is built later, by `start_cx`, the
first time a started context is needed, and `on_enter` is one of those times:
entering the `tracing` span starts the OpenTelemetry span so that its context
can be attached. From then on `OpenTelemetrySpanExt::set_parent`
(`src/span_ext.rs`) returns `SetParentError::AlreadyStarted`.

The rejected orders, and what the test observed for each:

- Create and enter the claim span, then `enter(received)` calling `set_parent`
  on `Span::current()` (the order this section first described): `set_parent`
  returned `Err(AlreadyStarted)`, and the exported claim span had the enclosing
  application span as its parent, in the application's trace.
- `enter(received)` first, then the claim span as an explicit root
  (`parent: None`): the exported claim span had an all-zero parent id and a new
  trace id. The attached context was ignored.

With context activation turned off, a span with no `parent:` argument takes the
started context of the current `tracing` span instead, and the attached context
is read only when no `tracing` span is current. The kept order relies on the
default, and the rustdoc of `Propagation` states it.

**What the generated code does, with the `trace-context` feature on:**

- **Sending.** A generated `Client` method or `Publisher` raise calls
  `propagation().and_then(|p| p.current())` and passes the result to
  `Caller::command`, `Caller::query` or `EventSink::raise`. With the `tracing`
  feature also on, it does so while the call span is entered, so the remote
  parent is the call span.
- **Receiving a claim.** `dispatch` calls `enter(claim.trace)` before it creates
  the claim span and runs the handler, and `leave()` after the claim span is
  dropped, through a guard that also runs on panic, in the order the "Call
  order" paragraph fixes. A call or raise made inside the handler then gets the
  application's child context from `current()`.
- **No hook registered.** Each point costs one atomic load and one branch, and
  `None` is sent. This is case 1.
- **Feature off.** Nothing is compiled, and `None` is sent, as today.

**Receiving an event.** `poll_next_event` decodes the occurrence and returns the
event to the application, which handles it after the future completes. There is
no point inside the face where the handling runs, so `enter` and `leave` are not
called for an occurrence. A call that the application makes in reaction to an
event is not linked to the event's trace. Phase 1 documents this limit. Closing
it needs the received context exposed with the event, which changes the face's
API, and waits for an event chain that needs it.

**Recording.** With the `tracing` feature on, the claim and occurrence spans,
and the events of D-5, record the received context as fields `trace.trace_id`,
`trace.parent_span_id` and `trace.flags`. The values are written as lowercase
hexadecimal by a `Display` wrapper that does not allocate. In case 1 these
fields are absent, because nothing is sent.

### D-7. Signals are out of phase 1

Signals carry no trace context on the wire. The generated code has no
notification path, and a signal read can happen at a very high rate. A signal
write made inside a claim handler already appears inside the claim span, because
it runs inside the handler. Signal spans are left to a later request.

### D-8. Verification

- **Feature off:** a test of `ridl-backend-rust` asserts that every `tracing`
  token in the generated source is inside an `observe` item or a
  `cfg(feature = "tracing")` item. `just compat-check` and `just wasm-check`
  keep building the generated crate without the feature, as they do today.
- **Feature on:** the `just demo` consumer enables `tracing` and installs a test
  subscriber that records spans and events. The demo asserts one call span for
  each round trip, with its identity fields and its outcome. The plan decides
  whether this is a second consumer or a variant of the existing one.
- **Dispatch events:** a test drives `dispatch` with an unknown ordinal, a short
  claim and a broken `ensure`, through `ridl-loopback`, and asserts each event
  and its level.
- **Propagation:** a test registers a `Propagation` that records each call, and
  drives a chain A → B → C through `ridl-loopback`, where B's handler calls C.
  It asserts that A's `current()` value reaches B's `enter`, that B's
  `current()` value reaches C, and that `leave` runs after each handler,
  including one that panics. A second test, with no hook registered, asserts
  that `None` is sent.
- **The spike:** the plan's first step implements `Propagation` with
  `tracing-opentelemetry` in a test only, as a dev-dependency, and fixes the
  hook's signature before anything else is built on it.
- The bare-`rustc` cell of `just compat-check` runs offline and does not get a
  `tracing` cell. The minimum Rust version of `tracing` is checked against
  `rust-version` in the plan.

### D-9. The case 3 library consumes `tracing` spans

Decided 2026-10-07: the telemetry library of case 3 consumes `tracing` spans, as
a `tracing` subscriber or layer. So the features `tracing` and `trace-context`
together cover case 3, and no public observation port is needed. A library with
its own span API would need that port, which stays under "Alternatives
considered".

## Phase 2: metrics (decided, not designed in detail)

Phase 2 has its own issue and plan. These decisions are already taken:

- **Its own feature, `metrics`**, off by default and independent of `tracing`. A
  production build can enable metrics without spans.
- **Counters per interface member, in the private `observe` module**, as
  zero-initialized statics. They need no allocation, no lock and no dependency.
- **Every counter is a monotonic `u64`, and no reader resets one.** A reader
  computes deltas and rates from two snapshots. A Perfetto exporter and a
  campaign collector can then read the same counters at different rates. A
  gauge, such as calls in flight, is derived as a difference of two counters.
- **Latency is a fixed set of histogram buckets**, each a monotonic counter, not
  a maximum that resets on read.
- **The first set is small**, because each counter becomes a contract for
  campaign tooling: for a command or query, `calls`, `ok`, failures by
  `Transport` kind, failures by `Contract` kind, `send_rejected` and latency
  buckets; for a claim, `claims`, `settled_ok`, `settled_failed` and
  handling-time buckets; for an event, `raised`, `raise_rejected`, `received`
  and `unreadable`; for a signal, `writes` and `write_rejected`.
- **Each member's counters are aligned to a cache line**, so that one busy
  member does not slow down another.
- **The application reads and exports a snapshot.** The generated crate does not
  export anything.

Open for phase 2: totals per member against counts per instance, and the bucket
boundaries.

## Division of observation between layers

Each layer counts what only it can see.

- **The generated face:** one span, outcome and counter for each interaction.
  This answers "which interaction became slower". Comparing `raised` at the
  producer with `received` at the consumer shows **that** events are lost, and
  between which processes.
- **A runtime (separate sub-project):** queue depth, back pressure, reconnects
  and lost events detected from sequence gaps. This answers **why** events are
  lost.
- **The application:** what an interaction means for the business, the campaign
  policy (which processes, how often, where to send), and the export.
- **The operating system:** CPU, memory and file descriptors.

## Alternatives considered

- **Generated code calls the `tracing` macros directly at every call site.** The
  compiled code is the same as D-1. Rejected because replacing `tracing` later
  would mean rewriting every call site the backend emits, and because names and
  keys could become inconsistent between kinds.
- **A public observation port in `ridl-rt`, with adapter crates.** This works
  without an allocator. Rejected for now, because no target needs it, a public
  trait is a stability promise made before the right shape is known, and the
  levels and exporters would have to be built. D-1 keeps this path open: the
  `observe` module would forward to that port.
- **A `ridlc` option instead of a Cargo feature.** Rejected, because the emitted
  crate would then depend on how it was generated, and Cargo features already
  express the choice.
- **A warning event for every failed client call.** Rejected, because the
  application already receives the `Result`, and because events would flood when
  a transport goes down. The outcome is on the span instead.
- **Metrics from the `metrics` crate facade.** Rejected, because it needs `std`
  and its cost per call depends on the recorder that the application installs.
- **Metrics derived from spans in a subscriber.** Rejected, because metrics
  would disappear when spans are compiled out, which is the field setting.
- **Signal spans in phase 1.** Deferred; see D-7.
- **RIDL creates its own W3C ids**, with a random generator per process and a
  thread-local "current context" set while `dispatch` runs a handler. This
  follows the W3C format, so it would work with OpenTelemetry. Rejected, because
  none of the three use cases needs it: case 1 sends no ids, and cases 2 and 3
  have their own source of ids. It would also put RIDL on the path to its own
  context propagation framework.
- **The generated crate depends on `opentelemetry` and `tracing-opentelemetry`**
  to read and set the context itself. Rejected, because those crates are large,
  change their API often, and would tie the generated crate to the application's
  OpenTelemetry version. The hook keeps that code in the application, and only
  in an application that chooses OpenTelemetry.
- **A hook in each generated crate's `observe` module.** Rejected, because a
  handler in one generated crate that calls a client of another would not see
  the context, and the application would have to register the hook once for each
  generated crate.
- **A `tracing::span::Id` as the `span_id` on the wire.** Rejected, because the
  subscriber assigns it, it is local to one process, and it is reused after the
  span closes.

## Records to amend

- **ADR-0023**, the amendment "the face passes no trace context", and decision
  6: what the face emits for a call, with the feature on.
- **ADR-0021 decision 21**, the bullet "Generated code", and a new decision for
  the `Propagation` trait, `set_propagation` and `propagation` in
  `ridl_rt::trace`, behind `std`. This is an addition to the `ridl-rt` API.
- **`docs/design/interaction-face.md`**: a new section on observation, and "The
  consumer face", "The provider face" and the settlement table.
- **`render_cargo_toml`** in `crates/ridlc/src/lib.rs`, its doc comment, and the
  guard test that keeps the manifest literals in sync.
- **The book:** a chapter, or a section of an existing one, on enabling and
  filtering observation, with the two settings of D-3, the three use cases, an
  example `Propagation`, and the limit for events of D-6.

## Dependencies

- The trace context of driftsys/ridl#752 is merged and ships in `ridl-rt` 0.6.0.
  The generated manifest pins `ridl-rt = "0.5"` today. Phase 1 needs that pin
  moved to 0.6.

## Trace

- Satisfies: driftsys/ridl#754.
- Related: ADR-0018, ADR-0020 decision 5, ADR-0021 decisions 8 and 21, ADR-0023
  decision 6.
