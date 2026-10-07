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

### D-2. One Cargo feature, `tracing`, off by default

The manifest that `ridlc` renders (`render_cargo_toml` in
`crates/ridlc/src/lib.rs`) gains:

```toml
[features]
tracing = ["dep:tracing"]
std = ["ridl-rt/std", "tracing?/std"]

[dependencies]
tracing = { version = "0.1", default-features = false, optional = true }
```

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

### D-6. Trace context: incoming is recorded, outgoing waits on an open decision

`ridl-rt` carries an optional `TraceContext` (W3C ids: `trace_id`, `span_id`,
`flags`) on `Claim`, `RawOccurrence`, `Caller::command`, `Caller::query` and
`EventSink::raise` (ADR-0021 decision 21). A `tracing` span has only a local
`u64` id. Turning one into the other needs OpenTelemetry, which the generated
crate does not depend on.

- **Incoming (decided).** The claim and occurrence spans, and the events of D-5,
  record the received context as fields `trace.trace_id`, `trace.parent_span_id`
  and `trace.flags`. The values are written as lowercase hexadecimal by a
  `Display` wrapper that does not allocate. A subscriber that knows
  OpenTelemetry can use them to set the remote parent.
- **Outgoing (open, see OD-1).** Until OD-1 is decided, the generated code keeps
  passing `None`.

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
- The bare-`rustc` cell of `just compat-check` runs offline and does not get a
  `tracing` cell. The minimum Rust version of `tracing` is checked against
  `rust-version` in the plan.

## Open decision

### OD-1. Where the outgoing trace context comes from

To pass `Some(TraceContext)` on a call or a raise, the generated code needs the
W3C ids of the current span. Options:

- **(a) A hook in `ridl-rt`.** `ridl_rt::trace` gains a function-pointer slot
  that the application sets once, for example from `tracing-opentelemetry`, and
  that returns the current context. It costs one atomic load per call and adds
  no dependency. It is a `ridl-rt` API change. _Recommended_, because every
  generated crate in a process then shares one setting.
- **(b) A hook per generated crate.** The same slot, in each generated crate's
  `observe` module. It needs no `ridl-rt` change, but the application must set
  it once for each generated crate it links.
- **(c) No outgoing context in phase 1.** Spans stay local to each process,
  linked only by the incoming fields of D-6.

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

## Records to amend

- **ADR-0023**, the amendment "the face passes no trace context", and decision
  6: what the face emits for a call, with the feature on.
- **ADR-0021 decision 21**, the bullet "Generated code".
- **`docs/design/interaction-face.md`**: a new section on observation, and "The
  consumer face", "The provider face" and the settlement table.
- **`render_cargo_toml`** in `crates/ridlc/src/lib.rs`, its doc comment, and the
  guard test that keeps the manifest literals in sync.
- **The book:** a chapter, or a section of an existing one, on enabling and
  filtering observation, with the two settings of D-3.

## Dependencies

- The trace context of driftsys/ridl#752 is merged and ships in `ridl-rt` 0.6.0.
  The generated manifest pins `ridl-rt = "0.5"` today. Phase 1 needs that pin
  moved to 0.6.

## Trace

- Satisfies: driftsys/ridl#754.
- Related: ADR-0018, ADR-0020 decision 5, ADR-0021 decisions 8 and 21, ADR-0023
  decision 6.
