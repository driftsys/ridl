# The Rust generated surface — types first, interfaces second

Status: working note, 2026-08-08. Scope: what `ridl-backend-rust` emits, in what
order, and why the interaction layer it ships today is not the thing to repair.

This note proposes a two-phase sequence. **Phase 1** makes the generated types
carry their typl contract and cross a wire. **Phase 2** restores an interaction
face in the shape proto uses — a generated client and a server trait, driven by
the interface. Phase 1 is self-sufficient: it is usable with no interaction
model at all.

Nothing here is implemented. The note records the evidence, the two phases, and
the questions each leaves open, so the decisions are taken deliberately rather
than inherited from the order the work happened to land in.

## 1. The question

The workspace has shipped a Rust backend since E2, and it emits two things: the
typl types of a package, and an interaction layer over them — consumer and
provider trait pairs, `SignalHandle`, `EventHandle`, `Provenance`, timing
constants, and contract stubs. The interaction layer is the elaborate half.

A review of that output for Rust idiom found defects at three levels, and the
deepest of them is not an idiom question at all: the interaction layer cannot be
connected to the runtime the platform plans to have. That moves the question
from "how should this code be written" to "what should be generated, and in what
order".

## 2. What the backend emits today

Every claim in this section is from the shipped snapshots and the generator.

### 2.1 The types carry no constraints

A named scalar becomes a newtype with a public field
(`crates/ridl-backend-rust/src/snapshots/ridl_backend_rust__tests__named_scalar_backings.snap`):

```rust
#[repr(transparent)]
pub struct Speed(pub f64);
impl Default for Speed {
    fn default() -> Self {
        Speed(0.0)
    }
}
```

`Speed(9999.0)` and `Speed(f64::NAN)` both construct. The typl range, unit and
step reach Rust as doc comments and nothing else, so the newtype supplies
nominal typing and no invariant. typl exists to declare those constraints; none
of them arrives.

Composite types keep the ridl spelling of their fields, and carry no derives
(`ridl_backend_rust__tests__appendix_a_rust_snapshot.snap`):

```rust
#[repr(C)]
pub struct DoorPayload {
    pub sensorId: i64,
    pub isOpen: bool,
}
```

`DoorPayload` cannot be printed, cloned, or compared. Enum variants come out as
`FILTER_INVALID`. The corpus compile test concedes the naming in its own
comment: the generated code "carries non-fatal lints".

### 2.2 The interaction vocabulary is regenerated per package

`crates/ridl-backend-rust/src/interact.rs:81` emits `vocabulary()` once for
every package that declares an interface or a service. Each package therefore
gets its own `Provenance`, `SignalHandle`, `EventHandle`, `TimingConst` and
`ContractStub`.

A component consuming interfaces from two packages receives two unrelated
`Provenance` enums and two unrelated `SignalHandle` traits. A helper generic
over `SignalHandle` can be written against one of them and not the other.

This is what makes the layer unconnectable to `ridl-rt`. The family overview
lists that runtime as planned and not started, and rsdl §667 assigns per-target
binding and glue code to it. A runtime crate cannot ship
`impl SignalHandle<T> for Signal<T>`, because `SignalHandle` does not exist
until codegen runs and then exists once per package. Codegen could emit those
impls itself — the trait is local, so the orphan rule permits it — but the
traits stay distinct and cross-package generic code stays impossible.

None of the vocabulary varies by contract. `Provenance` is three variants,
`Envelope` (ridl §3.1, not emitted at all today) is two integers with a fixed
epoch. Runtime vocabulary that does not vary per contract does not belong in
per-package generated code.

### 2.3 The interaction layer is never compiled

Two tests run `rustc` over generated output —
`veh_common_generated_rust_compiles_with_rustc` and
`workspace_two_members_composed_compiles_with_rustc`, both in
`crates/ridlc/tests/corpus.rs`. Both corpora contain `.typl` files only.

Every trait, handle and constant described above is covered by snapshot tests
and by nothing else. That is consistent with
[ADR-0016](../decisions/ADR-0016-schema-projection-and-the-name-transform.md),
which describes emitted Rust that `rustc` rejects as a shipped defect rather
than a hypothetical one.

### 2.4 A discriminant beside data, in three places

The backend emits a tag next to optional data where Rust has a sum type:

- `TimingConst` carries `mode: TimingMode` beside `min_us: Option<u64>` and
  `max_us: Option<u64>`. The mode decides which options are populated and the
  type does not enforce it.
- `ContractStub` carries `kind: ContractKind` beside `uses_result: bool`, which
  is meaningful only for `Ensure`.
- `SignalHandle::read` returns `(T, Provenance)`, so `let (v, _) = h.read();`
  discards the provenance. ridl §4.5 exists to prevent a subscriber holding
  stale last-good data without knowing it, and the signature permits exactly
  that.

The third case also fails to accommodate ridl §3.1. That section states that
generated APIs expose value, provenance and envelope. An `Init` value was never
published and therefore has no envelope, so adding the envelope to a flat pair
requires `Option<Envelope>` with the invariant "absent exactly when `Init`" held
in prose. A sum type states it:

```rust
pub enum Sample<T> {
    Init(T),
    Live { value: T, envelope: Envelope },
    Invalid { last_good: T, envelope: Envelope },
}
```

## 3. What is generated

The question underneath the idiom review is what the artifact set is. Five
things, and they vary along different axes:

| Artifact             | What it is                                                                                                 | Varies by        |
| -------------------- | ---------------------------------------------------------------------------------------------------------- | ---------------- |
| Domain types         | validated, idiomatic, language width per typl Appendix D                                                   | language         |
| Wire schemas         | `.proto`, `.fbs`, DBC — artifacts other toolchains consume                                                 | wire             |
| Wire representation  | the bytes, and any language structs mirroring them — transport width, scaled integers, absence realisation | wire             |
| Codecs               | domain to wire and back                                                                                    | language × wire  |
| Interaction identity | the ordinal table (ADR-0013 decision 3)                                                                    | neither — shared |

**The domain type is the hub.** There may be several wires for one contract, and
a deployment may need to move a value between two of them — a CAN frame arriving
on one side of a gateway and a proto message leaving the other. Routing every
conversion through the domain type makes that N codecs rather than N by N
converters, and puts validation in one place. It also constrains the domain
type: it must carry everything any wire representation can carry.

**Encoding is total; decoding is not.** typl §4.2 derives the wire width from
the declared range, so a validated domain value always fits the wire the checker
chose for it. [ADR-0013](../decisions/ADR-0013-codegen-backend-scope.md)
decision 7's sentinel case — an optional field whose range leaves no value spare
— fails at generation time rather than at encode time. Decoding is fallible
because bytes arrive from outside the type system.

The exception is loss rather than failure. typl §4.3 quantization means a value
carried as a scaled integer round-trips at that encoding's resolution, so domain
to wire to domain is not the identity for a quantized type. Across a gateway
that is the correct result — the reading has that resolution — but it is a
property to state rather than to discover.

## 4. The two phases

**Phase 1 — types, payloads, validation, wire.** The generated types enforce
their typl constraints, derive what Rust expects, use Rust naming, include the
types induced by interactions, and encode to and decode from a wire. No
interaction face.

**Phase 2 — client and server, interface-driven.** A generated client struct
generic over a transport, and a server trait the provider implements.

The split is the one the Rust ecosystem already draws between prost and tonic,
and it holds for the same reason: the types are useful without the service face,
and the service face is not useful without the types.

The phases also answer
[ADR-0013](../decisions/ADR-0013-codegen-backend-scope.md) open item 1, which
asks whether the wire backend emit ceiling binds the language backends. Phase 1
is that ceiling — shape plus identity, its decisions 2 and 3. Phase 2 restores
the interaction face on the grounds of its decision 1: a backend is classified
by what its target can represent, and Rust can represent last-value, provenance
and asynchronous calls. The sequence is the argument for why both hold.

## 5. Phase 1

### 5.1 Validation lives in the constructor

A named scalar gets a private field and a checked constructor:

```rust
pub struct Speed(f64);

impl TryFrom<f64> for Speed {
    type Error = RangeError;
    fn try_from(v: f64) -> Result<Self, Self::Error> { /* range, step */ }
}

impl Speed {
    /// The caller has already validated `v`. Used by the generated decoder.
    pub fn new_unchecked(v: f64) -> Self { Speed(v) }
}
```

Once a `Speed` exists it is in range, and no transport can produce one that is
not. The unchecked constructor keeps a provider publishing at a high rate from
validating twice.

This is a narrower claim than it looks. ridl §4.5's invalid state is a property
of a _channel_, detected by the runtime and propagated to subscribers; it is not
this. What the constructor removes is application code manufacturing an
out-of-range value, which the specification classifies as a provider bug
surfacing through telemetry. A type can prevent it outright.

### 5.2 The codec

§3 fixes the arrangement: the domain type is the hub, each wire has its own
representation, and a codec joins them in both directions. Two questions about
the codec itself are open, and are recorded in §7 rather than settled here.

One argument was considered and rejected during drafting, and is recorded
because it is the obvious one to reach for. The argument was that the Rust
backend must not emit its own encoder, because a hand-written encoder and the
`.proto` that E9.8 projects would be two implementations of one format, and
[ADR-0013](../decisions/ADR-0013-codegen-backend-scope.md) rejects exactly that
reasoning for width inference — a second inference site is a second source of
truth that drifts silently. The conclusion drawn was to let a proto toolchain
generate the wire structs and to emit only the conversions.

That does not follow. The single source is the IR together with the projection
rules, which
[ADR-0016](../decisions/ADR-0016-schema-projection-and-the-name-transform.md)
decision 6 already requires to be deterministic and total. A schema and an
encoder both derived from those rules agree by construction; holding them in
step is a round-trip test against the target's own toolchain, not a second
source of truth. The width case is different because it was a second
_inference_, not a second rendering of one inference.

The conclusion also fails on its own terms once more than one wire exists. CAN
and AUTOSAR Classic have no schema toolchain to delegate to, so delegation
serves exactly one wire and leaves the others with no codec at all.

What survives is that a `.proto` remains worth emitting — as the interop
artifact other languages and toolchains consume — whether or not it is what this
backend's own codec is built from.

### 5.3 Validation and width resolution are one operation

[ADR-0013](../decisions/ADR-0013-codegen-backend-scope.md) decision 4 has a
language backend read the language width and a wire backend read the transport
width. A field resolved to `uint8` on the wire is `i64` in Rust, so decoding
widens and encoding narrows.

The two directions are not symmetric, for the reason §3 gives. Encoding narrows
into a width typl §4.2 derived from the declared range, so a validated domain
value fits by construction and the narrowing needs no runtime check. Decoding
widens, and the widened value is unconstrained until it is checked — so the typl
range check and the decode boundary are the same place, and phase 1 should
implement them as one operation rather than two that run next to each other.

This is also why §5.1's unchecked constructor is safe to generate: the decoder
validates once, on the way in, and constructs without validating a second time.

### 5.4 Naming, and the two constraints it inherits

Fields become `snake_case` and enum variants become `CamelCase`, through the
pinned transform in `crates/ridl-ir/src/name.rs`.

Both carry a prescribed obligation:

- [ADR-0016](../decisions/ADR-0016-schema-projection-and-the-name-transform.md)
  decision 4 excludes struct fields from the transform and from RIDL-149 until
  E9.8 "extends both the transform and this check to them in the commit that
  starts projecting them, so that the rule and its application change in the
  same commit". Phase 1 is that commit.
- Enum variant renaming meets driftsys/ridl#237: the Rust backend's union-arm
  transform already collides, so `foo_bar` and `fooBar` both emit `FooBar` and
  the file does not compile. Renaming variants does not create that defect but
  does walk into it.

### 5.5 Derives

Generated types derive `Debug`, `Clone`, `PartialEq`, and `Copy`, `Eq`, `Hash`
and `PartialOrd` where the backing type permits. A hand-written `Default` stays
where the init value is not the backing type's default, because typl §5.8 allows
a declared init that `derive(Default)` cannot express.

### 5.6 A compile gate

Phase 1 adds a corpus entry containing interactions to the `rustc` tests of
§2.3, so that generated output is compiled and not only compared to a snapshot.
Without it, every change in this note is verified by inspection.

This gate is worth landing first. It is independent of every decision here, and
it is what makes the rest verifiable.

## 6. Phase 2, in outline

Phase 2 is not designed here. What follows is the shape phase 1 should not
foreclose.

**A client struct and a server trait.** The client is concrete and generic over
a transport supplied by `ridl-rt`, rather than a trait the consumer implements.
That removes the `&mut dyn SignalHandle` accessors, the vtable, and the
per-package vocabulary of §2.2 in one change. The server stays a trait the
provider implements, which is what keeps the compiler checking a component
against its contract.

**Three of the five kinds are calls; two are not.** `query`, `command` and
`event` map onto request/response, request-without-reply and server-streaming.
`signal` and `fixed` do not: ridl §4.4 requires a retained last value and §4.5
requires provenance, and a call has nowhere to hold either. So the client owns
state for those two kinds.

**Which means phase 2 converges on E9.11.** A stateful client half is a store; a
server-side ordinal router is a dispatcher. That is what
[`2026-08-03-schema-projection-design.md`](2026-08-03-schema-projection-design.md)
§4 already specifies — a store table per provided interface carrying a
generation counter, and a dispatcher as a routing table keyed by ordinal. Phase
2 is that design expressed in Rust, not a third answer. The E2 trait pair is the
outlier, and this sequence retires it rather than repairing it.

**The async surface is smaller than the one shipped today.** Taking the kinds
individually:

| Kind      | Async | Why                                                       |
| --------- | ----- | --------------------------------------------------------- |
| `signal`  | no    | §4.4 — the channel is never empty; a read always succeeds |
| `event`   | no    | Registration, not a call                                  |
| `fixed`   | no    | §8 — provisioned at binding initialization                |
| `command` | no    | §6.1 — fire-and-forget; see below                         |
| `query`   | yes   | Request/response with a response bound                    |

The `command` row contradicts the shipped signature. Today the consumer trait
declares `async fn set_gear(&self)`, so the application awaits the delivery
acknowledgment. ridl §6.1 states that the acknowledgment "carries no functional
payload, never reaches the contract surface, and is not application-visible",
and §6.2 ends "application code on both sides stays uninvolved". Whether
awaiting it is legitimate backpressure or a conformance defect is question 4
below.

Generated code that performs no I/O — the caller drives it — is what would let
one contract serve both a gRPC deployment and an AUTOSAR Classic one, where no
async runtime exists. Phase 1 does not decide this. It should avoid deciding it
by accident.

## 7. Open questions

1. **Does a codec materialise an intermediate wire struct, or go straight to
   bytes?** A language struct per wire per type is something another toolchain
   can consume and makes the projection inspectable, at the cost of a second
   type set and a second conversion hop. Going straight from domain type to
   bytes is leaner and is the only option where no schema toolchain exists, but
   leaves no artifact to point at. The `.proto` of §5.2 is emitted either way,
   so the question is what this backend's own codec is built from.
2. **Does phase 1 implement one wire, or the seam for several?** §3 states that
   several wires and gateway conversion between them are in scope, which argues
   for the seam existing before the first wire's assumptions reach the domain
   types. Against it: a format-generic abstraction written against one
   implementation is usually the wrong abstraction. The middle position — one
   wire implemented, with the domain types and the codec boundary shaped so a
   second wire adds a module rather than changing a type — is what this note
   assumes without having established it.
3. **Where the envelope sits on an invalid sample.** The `Sample` sketch in §2.4
   gives `Invalid` an envelope without saying whose. ridl §4.5 says the
   malformed value is not delivered but its invalidity is, which suggests the
   rejected publication's envelope rather than the last good one. It is not
   stated anywhere.
4. **Whether `async fn` on a `command` is a conformance defect.** §5 states the
   case on both sides. It is a question about ridl §6.1, not about Rust, and
   should be answered against the specification.
5. **Whether the vocabulary's home is `ridl-rt`.** §2.2 shows that per-package
   generation is wrong. It does not establish that a runtime crate is the answer
   rather than a generated shared module. Phase 2 needs this settled; phase 1
   does not.

## 8. References

- [ADR-0013](../decisions/ADR-0013-codegen-backend-scope.md) — backend
  classification, the emit ceiling, the identity table, widths per class, open
  item 1
- [ADR-0016](../decisions/ADR-0016-schema-projection-and-the-name-transform.md)
  — the pinned name transform, RIDL-149, and decision 4's exclusion of struct
  fields
- [`2026-08-03-schema-projection-design.md`](2026-08-03-schema-projection-design.md)
  — the store and dispatcher shapes phase 2 converges on
- [ridl §3.1](../specification/ridl-language-reference.md) — the implicit
  envelope; §4.4 and §4.5 — last-value and provenance; §6.1 and §6.2 — command
  acknowledgment; §8 — `fixed`
- [`docs/ROADMAP.md`](../ROADMAP.md) — E9.8 (proto3 projection), E9.9
  (FlatBuffers), E9.11 (store and dispatcher), E4.5 (IR stability policy)
- `crates/ridl-backend-rust/src/interact.rs` — the generator, and `vocabulary()`
  at line 81
- `crates/ridlc/tests/corpus.rs` — the two `rustc` compile tests
- driftsys/ridl#236 and driftsys/ridl#237 — the C header type-name collision and
  the union-arm transform collision
