# Trace context propagation in `ridl-rt` — calls and events

Status: design note for driftsys/ridl#752, written 2026-10-06 against `main` at
4ce1baf4 (release 0.5.1). Sebastien approved the scope and both design sections
on 2026-10-06. Nothing here is implemented. It graduates into a new ADR-0021
decision, amendments to ADR-0021 decision 10 and ADR-0023, and the records
listed in §7, and is archived with its plan when the change lands.

Generated spans are a separate change: driftsys/ridl#754, which depends on this
one.

## 1. The problem

A call or an event sent through a ridl port can cross a process boundary, for
example over an AIDL transport. Nothing in `ridl-rt` links the sender's work to
the receiver's work, so a trace cannot follow a call or an event across that
boundary. A transport that wants to carry a trace context has no place in the
port API to take it from or to deliver it to.

## 2. Scope

In scope:

- a transport-neutral `TraceContext` type in `ridl-rt`;
- a `trace: Option<TraceContext>` argument on `Caller::command`, `Caller::query`
  and `EventSink::raise`;
- a `trace: Option<TraceContext>` field on `Claim` and `RawOccurrence`;
- the delivery contract (§3.4);
- `ridl-loopback` carrying the context;
- conformance cases in `ridl-rt-conformance`;
- the generated face passing `None`;
- the records in §7.

Out of scope:

- spans, generated or otherwise (driftsys/ridl#754);
- a trace context on signal samples (#752 gives the reasons: latest-value state,
  lock-free readers, and a trace that explains only the last writer);
- `tracestate` and baggage;
- a trace context on the frame (§3.5).

## 3. Design

### 3.1 The type

A new module, `ridl_rt::trace`, with no feature gate, `no_std`, and no
dependency:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TraceContext {
    pub trace_id: [u8; 16],
    pub span_id: [u8; 8],
    pub flags: u8,
}
```

It is the W3C Trace Context `traceparent` layout without the version byte, as
plain data. `ridl-rt` does not validate it: an all-zero id is carried like any
other value, and rejecting one is the exporter's choice. `size_of` is 25 bytes,
and `size_of::<Option<TraceContext>>()` is 26 bytes (alignment 1).

### 3.2 The send side

Each of the three send methods gains a last argument:

```rust
fn command(&mut self, iface: InterfaceNo, ord: Ordinal, args: &[u8],
           trace: Option<TraceContext>) -> Result<Correlation, SendError>;
fn query(&mut self, iface: InterfaceNo, ord: Ordinal, args: &[u8],
         trace: Option<TraceContext>) -> Result<Correlation, SendError>;
fn raise(&mut self, iface: InterfaceNo, ord: Ordinal, bytes: &[u8],
         trace: Option<TraceContext>) -> Result<(), RaiseError>;
```

The `impl<P: T + ?Sized> T for &mut P` forwarding impls pass the argument
through unchanged.

### 3.3 The receive side

`Claim` gains `pub trace: Option<TraceContext>` after `envelope`.
`RawOccurrence` gains the same field after its envelope. `Envelope` is not
changed: ADR-0021 decision 5 keeps it at two fields, and it is shared by every
interaction kind and by the frame.

### 3.4 The delivery contract

Stated on `Caller`, `EventSink`, `Handler` and `EventSource`:

1. A runtime that carries the trace context delivers, on the `Claim` a command
   or query produces, the value its sender passed, unchanged.
2. A runtime that carries the trace context delivers, on every `RawOccurrence` a
   `raise` produces — one for each subscriber — the value its sender passed,
   unchanged.
3. A runtime or transport that does not carry the trace context delivers `None`.
4. A sender's `None` is delivered as `None`.

Rule 3 is what lets a transport, or a future shared-memory region
(driftsys/ridl#317), choose not to reserve the 26 bytes per slot.

### 3.5 The frame

The frame specification does not carry the trace context. `TraceContext` joins
the list of `ridl-rt` names that deliberately do not appear on the frame (frame
specification, §2), so a frame transport delivers `None` under rule 3. Carrying
it on the frame is a later decision, filed when a frame transport needs it.

## 4. `ridl-loopback`

- `CallEntry` gains `trace: Option<TraceContext>`. `command` and `query` store
  the argument, and the `Claim` presented for that call carries it.
- `QueuedEvent` gains `trace: Option<TraceContext>`. `raise` copies the argument
  into every subscribed source's queue, and `EventSource::next` returns it on
  the `RawOccurrence`.

Loopback carries the context: it is a runtime under rules 1 and 2, not rule 3.

## 5. The generated face

The Rust backend's emitted `port.command(..)`, `port.query(..)` and
`port.raise(..)` calls gain a last argument, `None`. No generated method
signature changes, and the generated dispatch does not read `claim.trace`.
driftsys/ridl#754 replaces both behaviours.

The checked-in fixture
`crates/ridl-backend-rust/tests/generated/interaction_face.rs` is regenerated.
`examples/cabin/generated/` is not in git: `ridl build` writes it when the gate
runs.

## 6. Tests

Written before the implementation they pin.

- `ridl-rt-conformance`, run by every runtime:
  - a command's context arrives unchanged on its `Claim`;
  - a query's context arrives unchanged on its `Claim`;
  - a raised event's context arrives unchanged on the occurrence of each of two
    subscribers;
  - a `None` sent on each of the three methods arrives as `None`;
  - two calls in flight with different contexts each arrive with their own.
- `ridl-rt`: the `&mut P` forwarding test passes a context through each of the
  three methods.
- `ridl-backend-rust`: the generated-face golden file pins the `None` argument.
- Task review mutates the loopback implementation: dropping the context (always
  `None`) and swapping the contexts of two in-flight calls must each fail a
  conformance case.

About 200 existing call sites gain `, None` or `trace: None`, most of them in
`crates/ridl-loopback/tests/ports.rs`, `crates/ridl-rt-conformance/src/` and
`xtask/tests/calibrate_cli.rs`.

## 7. Records

- ADR-0021: a new decision (21) for the `TraceContext` type, the argument, the
  fields and the delivery contract; an amendment note on decision 10 naming this
  as a breaking change.
- ADR-0023: an amendment noting that the generated `Client` and event methods
  pass `None`, and that generated method signatures are unchanged.
- `docs/design/ridl-rt.md`: the port section, and the `Claim` and
  `RawOccurrence` descriptions.
- `docs/technotes/ridl-rt-by-example.md`: the call and raise examples.
- `docs/specification/frame-specification.md`: the §2 list of names not on the
  frame.

## 8. Release

A breaking `ridl-rt` change under ADR-0021 decision 10. Every `Caller` and
`EventSink` implementer and every code that builds a `Claim` or a
`RawOccurrence` literal must change. It ships in the next workspace minor
(0.6.0). The commit is `feat(ridl-rt)!:` so that `git std` records the break in
the CHANGELOG.

## 9. Alternatives considered

- **The context as a field of `Envelope` (#752 option A).** Rejected. ADR-0021
  decision 5 keeps `Envelope` at two fields and rejects adding a field to it.
  `Envelope` is shared by signals, events, calls and the frame, so every stored
  envelope would grow from 16 to 48 bytes, and ridl §3.1 and the frame
  specification would change.
- **Additive `*_traced` send methods with default implementations.** Rejected. A
  runtime breaks anyway, because it builds `Claim` and `RawOccurrence` as struct
  literals. The methods would save only the generated call sites, at the cost of
  two methods per operation.
- **A context held on the port (`set_trace`), applied to the next send.**
  Rejected. It is stateful, and a context that is not cleared attaches to an
  unrelated call.
- **`#[non_exhaustive]` and constructors on `Claim` and `RawOccurrence`,** so
  that a later `tracestate` field is additive. Rejected for now: `tracestate` is
  speculative, and the change would move every runtime from struct literals to
  constructors and set a precedent the other thirteen structs under decision 10
  do not follow.
- **Events deferred to a later study (#752 as filed).** Rejected. The reason
  given, measuring the cost on a shared-memory region layout, has no object: no
  such layout exists (driftsys/ridl#317). Rule 3 lets a future region decline
  the cost, and deferring would make the events field a second breaking release.

Satisfies: driftsys/ridl#752. Related: driftsys/ridl#754, driftsys/ridl#317.
