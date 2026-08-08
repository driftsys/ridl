# Generating code: what ridl emits, and in what order

Status: working note, 2026-08-08. Scope: what the code generators emit, how the
output decomposes, and the order the work should land in. It covers the Rust
backend concretely and the emit model generally.

The short form. There are three artifacts, not one: **domain types** in a target
language, a **wire schema** for an encoding, and a **codec** joining them. They
sit on two independent axes, language and encoding, which the CLI should express
as two flags rather than as a combinatorial list of emit names. The work splits
into two phases: phase 1 delivers validated types and their codec, phase 2
delivers an interaction face in the shape proto uses — a client and a server.

Nothing here is implemented. The note records the evidence, the decomposition,
and the questions each part leaves open.

## 1. The question

The Rust backend has shipped since E2 and emits two things: the typl types of a
package, and an interaction layer over them — consumer and provider trait pairs,
`SignalHandle`, `EventHandle`, `Provenance`, timing constants, contract stubs.
The interaction layer is the elaborate half.

Reviewing that output for Rust idiom turned up defects at three levels, and the
deepest is not an idiom question: the interaction layer cannot be connected to
the runtime the platform plans to have, and the types do not carry the contract
typl declares. That moves the question from "how should this be written" to
"what should be generated, and in what order".

## 2. What the backend emits today

Every claim here is from the shipped snapshots and the generator.

### 2.1 The types carry no constraints

A named scalar becomes a newtype with a public field
(`ridl_backend_rust__tests__named_scalar_backings.snap`):

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
nominal typing and no invariant. typl exists to declare those constraints. None
of them arrives.

Composite types keep the ridl spelling of their fields and carry no derives
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
lists that runtime as planned and not started, and rsdl assigns per-target
binding and glue code to it. A runtime crate cannot ship
`impl SignalHandle<T> for Signal<T>`, because `SignalHandle` does not exist
until codegen runs and then exists once per package. Codegen could emit those
impls itself — the trait is local, so the orphan rule permits it — but the
traits stay distinct and cross-package generic code stays impossible.

None of the vocabulary varies by contract. Runtime vocabulary that does not vary
per contract does not belong in per-package generated code.

### 2.3 The interaction layer is never compiled

Two tests run `rustc` over generated output —
`veh_common_generated_rust_compiles_with_rustc` and
`workspace_two_members_composed_compiles_with_rustc`, both in
`crates/ridlc/tests/corpus.rs`. Both corpora contain `.typl` files only.

Every trait, handle and constant above is covered by snapshot tests and by
nothing else. That is consistent with
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

The third case also fails to accommodate ridl §3.1, which states that generated
APIs expose value, provenance and envelope. An `Init` value was never published
and so has no envelope, which a flat pair can only express as `Option<Envelope>`
with the invariant "absent exactly when `Init`" held in prose. A sum type states
it:

```rust
pub enum Sample<T> {
    Init(T),
    Live { value: T, envelope: Envelope },
    Invalid { last_good: T, envelope: Envelope },
}
```

## 3. What we generate

Three artifacts, on two independent axes.

| Artifact         | What it is                                               | Varies by         |
| ---------------- | -------------------------------------------------------- | ----------------- |
| **Domain types** | validated, idiomatic, language width per typl Appendix D | language          |
| **Wire schema**  | the encoding, as an artifact other toolchains consume    | encoding          |
| **Codec**        | domain type to bytes and back                            | language×encoding |
| Identity         | the ordinal table (ADR-0013 decision 3)                  | neither           |

The domain type is the hub. With several encodings, conversion routes through it
— encoding A to domain to encoding B — so N encodings need N codecs, not N².
That is what makes a generated gateway possible: a CAN frame arriving on one
side and a proto message leaving the other, mediated by one validated
representation.

Two properties follow from the IR rather than from choice:

- **Encode is infallible; decode is fallible.** typl §4.2 derives the wire width
  from the declared range, so the wire always fits a validated domain value.
  [ADR-0013](../decisions/ADR-0013-codegen-backend-scope.md) decision 7's
  sentinel case fails at generation time, not at encode time. Decode is fallible
  because bytes arrive from outside the type system.
- **Quantization is lossy, not fallible.** typl §4.3's scaled-integer encoding
  means a CAN value round-trips at its own resolution, so domain to wire to
  domain is not the identity for quantized types. For a gateway that is correct
  behaviour, and it should be stated rather than discovered.

### 3.1 Encoding and representation are different axes

Proto3-in-Rust is at least three things: prost's structs, rust-protobuf's,
quick-protobuf's. Same encoding, same bytes, different types and different APIs.
Encoding is a property of the wire; representation is a library choice in a
language.

ridl generates the codec itself, straight from domain type to bytes, so no
representation is chosen and none is imposed on the consumer. Interoperability
is at the **bytes**, mediated by the emitted schema — our Rust codec and
someone's protoc-generated Java interoperate without sharing a library.

This is a smaller undertaking than it sounds, because it is not a protobuf
library. The bulk of prost is schema parsing, descriptors, dynamic messages,
reflection, and generating Rust types from a `.proto`. The types and the field
numbers already exist here; what remains is encode and decode per generated type
against a format with six wire types, varints and zigzag.

It carries two obligations, recorded as open questions 1 and 2 below.

### 3.2 The codec follows serde's architecture over ridl's data model

The codec is not written once per (type, encoding) pair by hand. It follows the
principles serde established, with a different vocabulary:

- **The type describes itself; the format drives the mechanics.** N types plus M
  formats, not N times M.
- **Compile time, no reflection.** No descriptors shipped at runtime, no dynamic
  dispatch on the publication path.
- **One description, both directions, and validation.** Encode, decode and the
  typl constraint checks fall out of the same declaration, rather than being
  three mechanisms that have to agree.

What does not transfer is serde's data model. Its 29 types carry no field
number, no bit offset and no scaling factor, and those are exactly what the
encodings here differ over — which is why protobuf support in Rust is prost
rather than a serde format.

**ridl already has the right model: the IR's.** Ordinal identity, resolved width
and signedness, range, step, unit, scale, and optionality with its realisation
strategy are computed once by the checker and are precisely the axes the
encodings vary on.

Two places the architecture strains, both worth knowing before it is built:

- **FlatBuffers does not stream.** It builds back to front — children complete
  before their parent, then a vtable is written — so a push-style
  `encode_field(ordinal, value)` in declaration order cannot drive it directly.
  The format implementation has to buffer, which is what `FlatBufferBuilder`
  does internally. It works, and the cost falls on the format rather than on the
  type.
- **Some encodings need layout the ordinal does not carry.** For proto3 the
  projection is field number equals ordinal, so the ordinal is sufficient. CAN
  needs a bit offset. The projection can derive one by packing in ordinal order
  at resolved widths, which is right for a greenfield bus and wrong against an
  existing DBC matrix, where the layout is given. That makes it a deployment
  input — rsdl's territory — rather than something the type describes.

So the seam is type description, plus format driver, plus an optional layout
input for encodings that need one.

Whether the description is **code-driven** — the generator emits encode calls,
monomorphized, as serde does — or **descriptor-driven** — the type carries a
`const SHAPE` table that one interpreter walks — is open question 3 below.

## 4. The CLI: two flags for two axes

```text
ridlc build --emit rust                 domain types, no transport, no dependency
ridlc build --wire proto                the .proto schema, language-free
ridlc build --emit rust --wire proto    the codec joining the two
```

Phase 1 and phase 2 become expressible in the CLI: `--emit rust` alone is phase
1's output and stays useful with no wire at all.

Both flags are repeatable and combine as a cartesian product, so
`--emit rust --emit typescript --wire proto --wire fb` yields two type sets, two
schemas and four codecs.

**`--emit ir-json` stays where it is.** `--emit` today mixes languages (`rust`,
`typescript`), a header (`c-header`) and IR encodings (`ir-json`, `ir-txtpb`,
`ir-binpb`). Once `--emit` means "target language" the IR values do not belong
on either axis. Moving them is a breaking change to a shipped CLI for a cosmetic
gain, so they stay and the exception is documented.

## 5. Phase 1 — types, payloads, codec

### 5.1 Validation is already designed — Epic 10

[`typl-value-objects-design.md`](typl-value-objects-design.md) is the design of
record for the validation half of phase 1, and it is more detailed than this
note should restate: private inner value, `new` returning `Result` with a safe
`new_unchecked` for hot paths, `TryFrom<Inner>` inbound and `From<Type>`
infallible outbound, vacuous constraints emitting an infallible `const fn new`,
`ConstraintError` in the package vocabulary, and the derive set. It carries a
ten-task plan and is sequenced as Epic 10.

Phase 1 does not redesign any of it. What this note adds is the sequencing —
Epic 10 is phase 1's first half, not a parallel track — and two points where the
codec meets it:

- **`new_unchecked` is what the decoder calls** once it has validated during the
  parse, so a decoded value is not checked twice. That is the hot path the
  design's decision 1 anticipates.
- **The typl range check and the width narrowing of
  [ADR-0013](../decisions/ADR-0013-codegen-backend-scope.md) decision 4 are the
  same check at the same boundary.** A field resolved to `uint8` on the wire is
  `i64` in Rust; decoding widens, and the range check is what makes the widened
  value legal. They should be one operation, not two that run next to each
  other.

One interaction the value-objects design does not account for. It places
`ConstraintError` in "the dependency-free package vocabulary the `interact`
module already emits beside `Provenance` and `SignalHandle`" — which is the
per-package duplication of §2.2 above. Two packages would carry two unrelated
`ConstraintError` types, so a consumer cannot write one error path across both,
and a decoder spanning packages cannot return a single error type. Whatever
settles §2.2 has to settle `ConstraintError` with it.

### 5.2 Payloads

Phase 1 covers the types induced by interactions as well as the declared ones:
tuple returns, inline `T | E` result unions, and command and query request
shapes. These exist only because an interaction declared them, and they are data
types like any other.

### 5.3 Naming, and the two constraints it inherits

Fields become `snake_case` and enum variants `CamelCase`, through the pinned
transform in `crates/ridl-ir/src/name.rs`. Both carry a prescribed obligation:

- [ADR-0016](../decisions/ADR-0016-schema-projection-and-the-name-transform.md)
  decision 4 excludes struct fields from the transform and from RIDL-149 until
  E9.8 "extends both the transform and this check to them in the commit that
  starts projecting them, so that the rule and its application change in the
  same commit". Phase 1 is that commit.
- Enum variant renaming meets driftsys/ridl#237: the Rust backend's union-arm
  transform already collides, so `foo_bar` and `fooBar` both emit `FooBar` and
  the file does not compile. Renaming variants does not create that defect but
  does walk into it.

### 5.4 Derives

Specified by [`typl-value-objects-design.md`](typl-value-objects-design.md)
along with the constructors, and not restated here. §2.1 above is the evidence
that none is emitted today.

### 5.5 A compile gate, first

Phase 1 adds a corpus entry containing interactions to the `rustc` tests of
§2.3, so generated output is compiled rather than only compared to a snapshot.
This is independent of every other decision here and is what makes the rest
verifiable. It should land first.

## 6. Phase 2 — client and server

Not designed here. What follows is the shape phase 1 must not foreclose.

**A client struct and a server trait**, the split proto uses. The client is
concrete and generic over a transport supplied by `ridl-rt`, rather than a trait
the consumer implements — which removes the `&mut dyn SignalHandle` accessors,
the vtable, and the per-package vocabulary of §2.2 in one change. The server
stays a trait the provider implements, which is what keeps the compiler checking
a component against its contract.

**Three of the five kinds are calls; two are not.** `query`, `command` and
`event` map onto request/response, request-without-reply and server-streaming.
`signal` and `fixed` do not: ridl §4.4 requires a retained last value and §4.5
requires provenance, and a call has nowhere to hold either. The client owns
state for those two kinds.

**Which means phase 2 converges on E9.11.** A stateful client half is a store; a
server-side ordinal router is a dispatcher. That is what
[`2026-08-03-schema-projection-design.md`](2026-08-03-schema-projection-design.md)
§4 already specifies. Phase 2 is that design expressed in Rust, not a third
answer, and the E2 trait pair is the outlier this sequence retires.

**The async surface is smaller than the one shipped.**

| Kind      | Async | Why                                                       |
| --------- | ----- | --------------------------------------------------------- |
| `signal`  | no    | §4.4 — the channel is never empty; a read always succeeds |
| `event`   | no    | registration, not a call                                  |
| `fixed`   | no    | §8 — provisioned at binding initialization                |
| `command` | no    | §6.1 — fire-and-forget; see open question 4               |
| `query`   | yes   | request/response with a response bound                    |

Phase 1 does not decide this. It should avoid deciding it by accident.

## 7. The seam is a plugin protocol

Extension should not be an ever-growing list of in-tree backends. The roadmap
already names the mechanism: E4.5 is "IR plugin protocol spec + versioning — a
third-party backend consumes the IR".

Most of the request half exists.
[ADR-0014](../decisions/ADR-0014-ir-encodings.md) made the IR a real protobuf
with three encodings, ADR-0016 requires projections off it to be deterministic
and total, and [ADR-0008](../decisions/ADR-0008-e2-execution.md) decision 9
keeps `ridlc` a pure source-to-IR function, which is the shape a plugin host
wants underneath it. Missing: a response message, a discovery and invocation
convention, and version negotiation.

The two CLI axes give two plugin kinds — `--wire someip` resolving to a wire
plugin, `--emit kotlin` to a language plugin — on protoc's `protoc-gen-*` naming
pattern.

**Specify the protocol; treat the mechanism as pluggable.** A plugin can then be
a subprocess or a WebAssembly module, because the bytes are identical either
way. Subprocess first: no new dependencies, works immediately. A wasm host earns
real properties — sandboxing, one artifact per plugin rather than one per
platform, and determinism that extends ADR-0016 property 1 from the projection
to the whole generator — but it should arrive behind a feature flag, hosted in
the `ridl` facade rather than in `ridlc`, and without betting on the Component
Model before it settles.

**This forces the E4.5 encoding decision.**
[ADR-0014](../decisions/ADR-0014-ir-encodings.md) decision 9 names binary
canonical; decision 14 records that binary cannot round-trip IR this toolchain
produces, while JSON now round-trips everything the front end admits. The
encoding named canonical is the one that would fail on a plugin's stdin. No
consumer reads it today, so nothing breaks; the first plugin changes that.

## 8. What this settles about ADR-0013 open item 1

That item asks whether the wire backend emit ceiling binds the language
backends. The two phases answer it: phase 1 **is** that ceiling — shape plus
identity, its decisions 2 and 3 — and phase 2 restores the interaction face on
the grounds of its decision 1, that a backend is classified by what its target
can represent and Rust can represent last-value, provenance and asynchronous
calls. The sequence is the argument for why both hold.

## 9. Open questions

1. **Byte-level conformance testing.** Owning the codec means our bytes must
   parse in protoc-generated code and its bytes must parse here. Packed repeated
   scalars, canonical field ordering, and malformed-input robustness are where
   hand-rolled encoders go wrong. The test must exist or the emitted schema is a
   claim rather than a guarantee.
2. **Unknown fields on decode.** proto3 preserves them so a proxy can round-trip
   what it does not understand. For a platform where the schema is the SSOT and
   `ridl-diff` gates evolution, carrying undescribed fields is arguably wrong —
   but dropping them breaks the gateway case §3 relies on.
3. **Code-driven or descriptor-driven codecs.** §3.2 fixes the architecture and
   leaves the form open. Code-driven is what serde does: the generator emits the
   encode calls, monomorphized, and it is the faster of the two on a signal
   publishing at a high rate. Descriptor-driven has the type carry a `const`
   shape table that one interpreter walks, which is smaller, serves encode,
   decode and validation from one declaration, and can also drive a gateway that
   does not know the type at compile time. The gateway case of §3 pulls toward
   descriptors and the publication path pulls toward code.
4. **Where the envelope sits on an invalid sample.** The `Sample` sketch in §2.4
   gives `Invalid` an envelope without saying whose. ridl §4.5 says the
   malformed value is not delivered but its invalidity is, which suggests the
   rejected publication's rather than the last good one's. It is not stated
   anywhere.
5. **Whether `async fn` on a `command` is a conformance defect.** The shipped
   consumer trait declares `async fn set_gear(&self)`, so the application awaits
   the delivery acknowledgment. ridl §6.1 states the acknowledgment "carries no
   functional payload, never reaches the contract surface, and is not
   application-visible", and §6.2 ends "application code on both sides stays
   uninvolved". This is a question about ridl §6.1, not about Rust.
6. **Whether the vocabulary's home is `ridl-rt`.** §2.2 shows per-package
   generation is wrong; it does not establish that a runtime crate is the answer
   rather than a generated shared module. Phase 2 needs this settled; phase 1
   does not.
7. **Whether the in-tree backends become plugins.** protoc keeps first-party
   languages compiled in and everything else a plugin. Making
   `ridl-backend-rust` and `ridl-backend-ts` plugins would prove the protocol
   properly, at the cost of restructuring working code.

## 10. References

- [ADR-0013](../decisions/ADR-0013-codegen-backend-scope.md) — backend
  classification, the emit ceiling, the identity table, widths per class,
  decision 7 on absence, open item 1
- [ADR-0014](../decisions/ADR-0014-ir-encodings.md) — the IR encodings, decision
  9 on canonicity and decision 14 on the binary round-trip limit
- [ADR-0016](../decisions/ADR-0016-schema-projection-and-the-name-transform.md)
  — the pinned name transform, RIDL-149, decision 4's exclusion of struct
  fields, decision 6's projection properties
- [`typl-value-objects-design.md`](typl-value-objects-design.md) — the design of
  record for validating constructors and derives, which is phase 1's first half
  (Epic 10)
- [`2026-08-03-schema-projection-design.md`](2026-08-03-schema-projection-design.md)
  — the store and dispatcher shapes phase 2 converges on
- [ridl language reference](../specification/ridl-language-reference.md) — §3.1
  the envelope, §4.4 and §4.5 last-value and provenance, §6.1 and §6.2 the
  command acknowledgment, §8 `fixed`
- [`docs/ROADMAP.md`](../ROADMAP.md) — E4.5 (plugin protocol and IR stability),
  E9.8 (proto3 projection), E9.9 (FlatBuffers), E9.11 (store and dispatcher)
- `crates/ridl-backend-rust/src/interact.rs` — the generator, `vocabulary()` at
  line 81
- `crates/ridlc/tests/corpus.rs` — the two `rustc` compile tests
- driftsys/ridl#236 and driftsys/ridl#237 — the C header type-name collision and
  the union-arm transform collision
