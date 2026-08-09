# ADR-0017 — The runtime core, two encodings, and what the backends emit

## Status

Proposed — 2026-08-09. Scope: the layers between a contract and a running system
— what the toolchain generates, what the runtime provides, what the platform
supplies — and the retraction of the interaction layer the language backends
ship today. It is not epic-scoped: it binds every backend and the runtime, in
the way ADR-0013 binds what a backend may emit and ADR-0016 binds how identity
projects.

It **answers the interaction-face half of
[ADR-0013](ADR-0013-codegen-backend-scope.md) open item 1** and **retires
[ADR-0007](ADR-0007-e1-execution.md) decision 13's extern-C face**.

Open item 1 asks whether the wire-backend emit ceiling binds the language
backends, and it has two halves that resolve differently. Roadmap epic E10 and
its plan of record settle the **validator** half — a language backend emits
constraint-checking constructors where a wire backend does not — and amend
ADR-0013 to say so. This record settles the **interaction face** half, and
settles it the other way for now: the face is retracted and restored in a second
phase (decision 15). The two are complementary and neither supersedes the other.

Its reasoning trail is
[`docs/wip/2026-08-08-runtime-and-codegen-architecture.md`](../wip/2026-08-08-runtime-and-codegen-architecture.md),
which carries the evidence this record summarises.

## Context

The Rust backend has shipped an interaction layer since E2 — consumer and
provider trait pairs, `SignalHandle`, `EventHandle`, `Provenance`, timing
constants, contract stubs. Reviewing that output for Rust idiom found defects at
three levels, and the deepest is not an idiom question.

**The layer cannot be connected to the runtime the platform plans to have.**
`crates/ridl-backend-rust/src/interact.rs:81` emits the interaction vocabulary
once per package, so each package declares its own `Provenance` and its own
`SignalHandle`. Two packages produce two incompatible types, and a runtime crate
cannot ship `impl SignalHandle<T> for Signal<T>` for a trait that does not exist
until codegen runs and then exists once per package. Cross-package generic code
over the vocabulary is impossible.

**It has never been compiled.** The two tests that run `rustc` over generated
output, both in `crates/ridlc/tests/corpus.rs`, cover `.typl`-only corpora.
Every trait, handle and constant in the layer is snapshot-tested and nothing
else, which is why
[ADR-0016](ADR-0016-schema-projection-and-the-name-transform.md) could describe
emitted Rust that `rustc` rejects as a shipped defect rather than a hypothetical
one.

**And the types below it carry no contract.** A named scalar becomes
`pub struct Speed(pub f64)`, so `Speed(9999.0)` and `Speed(f64::NAN)` both
construct and typl's range, unit and step reach Rust as doc comments. Composite
fields keep the ridl spelling, enum variants come out `FILTER_INVALID`, and no
generated type carries a derive.

So the elaborate half of the output is unusable and the thin half is unfinished.
That inverts the question from "how should this code be written" to "what should
be generated, and in what order", which is what this record answers.

## Decision

1. **Five layers, with the platform reduced to four traits.** Application code
   (4) sits on generated bindings (3), which sit on a portable runtime core
   `ridl-rt` (2), which sits on a platform abstraction of `Socket`, `Region`,
   `Notify` and `Clock` (1), which sits on the OS (0). **Layer 2 is identical on
   every target.** Porting is the four traits plus a driving loop.

2. **`ridl-rt` is sans-IO.** It performs no I/O and owns no socket: bytes in via
   `on_bytes`, events and outbound bytes out via `poll` and `pending_out`, with
   `poll` returning a deadline so rate floors, staleness bounds and
   acknowledgment timeouts work with no executor.

   The property that decides it is asymmetric: **a sans-IO core can present an
   async face; an async core cannot present a sans-IO face.** An async wrapper
   is a thin adapter over readiness; the reverse needs an executor, which is the
   dependency being avoided. One core therefore drives from an epoll loop, an
   RTOS task, a browser callback or a Tokio task unchanged, a `poll` does
   bounded work so worst-case execution time is arguable, and tests need no
   executor, no sockets and no real time. An async wrapper ships for the Tokio
   and Deno side, and per-OS optimisation happens in the driving loop, so layer
   2 neither changes nor needs reverification.

3. **Two encodings and no more: proto3 on the network, FlatBuffers in memory.**
   The rule: serialized into a stream is proto3; mapped or passed within a node
   is FlatBuffers. So the shm store and queue, a local socket and Binder carry
   FlatBuffers; CAN, SOME/IP, DDS, a WebSocket and the wasm guest boundary carry
   proto3.

   proto3 is chosen for the network because it is compact and schema-evolvable
   at once. A tagless positional encoding is 30–50% smaller on small messages
   but cannot survive version skew, and decision 14 makes the guest updatable
   independently of the host. FlatBuffers is chosen for memory because Binder's
   single copy into a mapped buffer and an `mmap`'d store both hand the receiver
   a buffer it reads in place.

4. **The codec is generated, not delegated to a third-party library.** It is a
   serializer for known types — no descriptors, no dynamic messages, no schema
   parsing — so `--emit rust --wire proto` produces code with no external
   crates.

   **Interoperability is at the bytes**, mediated by the emitted schema: our
   codec and a `protoc`-generated consumer interoperate without sharing a
   library. That places one obligation, which is a test rather than an
   architecture: byte-level conformance against a `protoc`-generated
   implementation, covering packed repeated scalars, canonical field ordering
   and malformed-input robustness.

   This does not reintroduce the second-source-of-truth problem
   [ADR-0013](ADR-0013-codegen-backend-scope.md) rejects for widths. The single
   source is the IR plus ADR-0016's projection rules, which are already required
   to be deterministic and total; a `.proto` and a codec derived from them agree
   by construction.

5. **Encoding and representation are different axes.** proto3-in-Rust is prost's
   structs, rust-protobuf's or quick-protobuf's — same bytes, different types.
   Generating the codec means no representation is imposed on a consumer, and
   the CLI needs only two axes:

   ```text
   ridlc build --emit rust                 domain types, no transport
   ridlc build --wire proto                the schema, language-free
   ridlc build --emit rust --wire proto    the codec joining them
   ```

   Both flags repeat and combine as a cartesian product. `--emit ir-json`,
   `ir-txtpb` and `ir-binpb` stay as a documented exception: the IR is the
   compiler's own artifact and sits on neither axis.

6. **Language scope narrows to Rust, TypeScript and Kotlin.** Rust is primary,
   with **optional** FFI wrappers built from opaque handles and accessors rather
   than exposed layout — which is what lets them carry strings, collections,
   optionals and sum types. TypeScript serves the tooling plane and the embedded
   web UI. Kotlin serves a native Android app. TypeScript and Kotlin receive
   generated types and interfaces only; the codec is the Rust one, reached via
   wasm and via JNI respectively.

   **C is dropped**, and with it ADR-0007 decision 13's extern-C face. The
   header it produces is layout-only: it omits every type with a string, an
   optional or a collection, omits every interface, and no `extern "C"`
   functions are emitted to match it. Interop with a legacy C consumer is a
   `--wire` concern, not an `--emit` one. This supersedes
   [ADR-0013](ADR-0013-codegen-backend-scope.md) decision 1's target list.

7. **One frame, several bindings.** Ordinal, kind, envelope, provenance,
   correlation and an opaque payload in the encoding for that path. The header
   carries what proto3 and AIDL both lack. The control plane — `attach`,
   `subscribe`, `unsubscribe`, `read`, `call` — is uniform across a Unix socket,
   Binder, a WebSocket and the wasm host boundary; only the data plane varies
   between mapped and framed.

   **`subscribe` delivers the current value immediately.** That is ridl §4.4,
   not a convenience, and it is what a gRPC server-streaming binding cannot do.

8. **The store is one region per provided interface**, which is simultaneously
   the coherence unit (ADR-0015, ridl §14.5), the mapping unit and the
   permission unit. Slot offsets derive from the ordinal and tombstones hold
   their slots, so ridl §11's append-only numbering is what keeps layout stable
   under compatible change. Slot sizes come from typl's bounds, which is why
   every payload has a computable maximum.

   **Three counters answer three questions**: an interface generation counter
   for coherence, a per-slot generation counter for torn payloads, and ridl
   §3.1's envelope sequence — stamped at origin and preserved across a gateway —
   for change and loss. Using only the interface counter couples every reader to
   every writer; using only slot counters gives no coherence.

9. **Assurance is three dimensions on ridl's own scale, `0..N`** — safety
   integrity, cyber threat, privacy — with the mapping to ASIL, CAL, DAL, SIL or
   a medical class owned by a domain plugin, following
   [ADR-0012](ADR-0012-interaction-boundary-model.md) decision 7's model of a
   domain extension as a spelling table plus backends with no core semantics.
   The core needs the ordering and the comparison and never the standard's name.

   **They do not compare in the same direction.** Safety and cyber are integrity
   dimensions — no write up — and constrain the write path. Privacy is
   confidentiality — no read up — and constrains whether a region may be mapped
   at all. A single comparison rule gets privacy backwards.

10. **Write mode and read verification derive from the trust relationship.** A
    publisher owning the region writes directly; a same-zone publisher writes a
    mapping of **its own interface only**; a cross-zone publisher sends through
    a channel the owner drains, validates and verifies. Consumers are read-only
    in every case, which makes ridl §4.2's direction an MMU property rather than
    a convention.

    **Verification is paid once, at the boundary, by the trusted side** — not
    per read by every consumer forever. A writable mapping across a trust
    boundary is refused because it bypasses validation entirely and lets one
    writer corrupt the generation counter for every reader.

    Cross-zone publication coalesces per ordinal, because a signal is state and
    ridl §4.3 permits coalescing, and applies a step's outputs under one
    generation increment.

11. **A privacy level above zero is never mapped.** A mapping is a capability
    and capabilities are not revocable, so anything whose access can be
    withdrawn is read by call with a per-call grant check, and its slot is
    absent from the region rather than present and stale. ridl §3.4 already
    models this as the **policy** source of unavailability, and RIDL-505 makes
    the grant state declared, consumer-visible state so a presentation boundary
    can disable a control before it is used.

    The cost lands correctly: location, identity and biometrics are low rate and
    never needed the mapped path.

12. **Ring depth derives; subscriber count comes from the wiring graph.**
    `depth ≈ ceil((service_period + jitter) / rate_floor)`, where the rate floor
    is ridl §9's `min`, the service period comes from rsdl — which needs it for
    RSDL-801 regardless — and jitter is a target property with one conservative
    default for real-time platforms and one for the rest, calibrated against
    measurement later. An rsdl override covers consumers whose pattern the rates
    do not capture; an underivable or infeasible depth is a deploy-time error.

    Subscriber count is the connection count in static posture and a declared
    bound in discovered posture (rsdl §8.1). Signals need no depth — they are
    the store. **The percentile derives from the interaction's safety integrity
    level**: above a threshold, size for worst case and treat overflow as a
    fault with a defined reaction rather than as telemetry.

13. **A bridge keeps a domain-mediated reference path with generated streaming
    transcoders beside it.** Decode to the validated domain type and re-encode
    is the correctness oracle. Streaming transcoders are generated per encoding
    pair where a link needs one, avoiding heap allocation and owned types while
    still validating inline — range and step checks are per-scalar and need no
    materialised object. An equivalence test asserts identical bytes against the
    reference, from the same corpus.

14. **A runtime update is admissible when it is compatible and fits the reserved
    span.** Ordinals are append-only, so a compatible change appends and never
    moves an existing offset; reserving a generous virtual span and committing
    pages on demand means growth does not invalidate existing mappings. Anything
    `ridl-diff` calls breaking, or anything exceeding the reservation, is a
    redeployment. The reservation is declared per region, and targets without an
    MMU fall back to re-attach.

15. **The interaction layer is retracted from the language backends, and
    restored in a second phase as a client and a server.** This answers the
    interaction-face half of [ADR-0013](ADR-0013-codegen-backend-scope.md) open
    item 1 by sequencing rather than by argument. The validator half is settled
    the other way by roadmap epic E10, which this record does not disturb: types
    that carry their constraints are phase 1's substance, not something the
    ceiling removes.

    **Phase 1 is that ADR's ceiling** — shape and identity, its decisions 2 and
    3 — with the types finally carrying their typl contract: a private field and
    a checked constructor, an unchecked constructor for the decoder, derives,
    Rust naming, and the types induced by interactions. **Phase 2 restores the
    interaction face** on the grounds of its decision 1, that a backend is
    classified by what its target can represent, and Rust can represent
    last-value, provenance and asynchronous calls.

    Phase 2 takes the shape proto uses — a concrete client generic over a
    transport and a server trait the provider implements — and converges on
    E9.11 rather than being a third answer, because a stateful client half is a
    store and a server-side ordinal router is a dispatcher.

16. **`ridl-rt` becomes an epic at the ridl layer, in V1.** The runtime core —
    store, control plane, frame protocol, subscriptions, the sans-IO loop, the
    platform traits — is not a behaviour concern and does not belong inside the
    rmdl epic. E5.12 stays where it is, correctly describing the rmdl scheduler.
    Without this, V1 ships a compiler whose output no runtime can consume.

17. **The deployment schema is designed first; rsdl is its authoring surface.**
    What the generator needs is facts, not syntax: placement, target properties
    (jitter profile, memory budget, MMU presence), the wiring graph, protection
    domains, region reservations and consumer service periods. Those become a
    schema the generator consumes, and descriptors are hand-written against it
    while the generator is built.

    **rsdl is not deferred as a goal — it is sequenced.** It remains the surface
    a person writes, and it lowers to this schema exactly as ridl lowers to the
    IR. Designing the schema first is what lets rsdl be designed against
    something real rather than against a guess, and it means the two fields rsdl
    does not have today — protection domain and region reservation — are settled
    by use before they are given syntax. A hand-written descriptor is a stopgap
    for the toolchain's own bootstrapping, never the interface a user is
    expected to live with.

    rmdl is the one that needs nothing at the language level for now: what the
    architecture requires is the **component binding contract** — activation,
    input read, output publish, and what a step means — which is a `ridl-rt`
    concern that hand-written components exercise as well as generated ones, and
    better for finding holes.

## Alternatives considered

| Candidate                                         | Verdict  | Reason                                                                                                                                                                                                                |
| ------------------------------------------------- | -------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Repair the shipped interaction layer in place     | rejected | it cannot be connected to a runtime at all, and has never been compiled; repairing idiom on an unusable layer spends effort on something phase 2 replaces                                                             |
| gRPC or ConnectRPC as the local bridge            | rejected | four of five interaction kinds lose a normative property, and the §3.1 envelope has nowhere to live — the mismatch ADR-0013 already documents, at the worst place                                                     |
| `async fn` at the platform layer                  | rejected | bakes an execution model into layer 2, makes future size a compiler artifact against static sizing, and leaves certification of a generated state machine open                                                        |
| Blocking calls at the platform layer              | rejected | excludes the browser outright — no blocking socket API, and the main thread may not block — and needs a thread per connection in the tooling plane                                                                    |
| Descriptor-driven codec at endpoints              | rejected | pays interpretation on the publication path; the dynamic case the descriptors would serve is already served by the IR, which ships in three encodings                                                                 |
| Delegate the codec to prost                       | rejected | serves exactly one wire; CAN and AUTOSAR Classic have no toolchain to delegate to, so the other targets stay unserved                                                                                                 |
| A tagless positional encoding on the network      | rejected | 30–50% smaller on small messages, but any version skew between independently updated host and guest is fatal rather than survivable                                                                                   |
| Full AIDL parcelables as the Android surface      | rejected | a fourth projection with ADR-0016's obligations, and two evolution systems over one contract that disagree about what is compatible                                                                                   |
| A writable shared mapping across a trust boundary | rejected | bypasses every range and step check, and lets one writer corrupt the generation counter and the offsets every other reader depends on                                                                                 |
| `repr(C)` for the store                           | deferred | verification is boundary-scoped rather than universal, build-then-flip is more robust to a writer crash than a seqlock, and vtables absorb the width flips of typl §17.11 — it survives only for the constrained tier |
| Keep C as a language target                       | rejected | the emitted header cannot represent strings, optionals or collections, so it drops most of a realistic package; legacy interop is a wire concern                                                                      |

## Consequences

- **Positive — V1 becomes a platform rather than a code generator.** Decision 16
  puts the runtime core in V1, which is what makes the generated artifacts
  consumable by anything.
- **Positive — one codec, one conformance obligation.** Decisions 4 and 6 mean
  the Rust codec serves Rust, TypeScript and Kotlin, and byte-level conformance
  is discharged once.
- **Positive — the porting surface is four traits and a loop.** Decisions 1 and
  2 mean wasmtime, WAMR, an RTOS and a browser are ports rather than variants.
- **Positive — two shipped defects close.** The per-package vocabulary and the
  never-compiled layer both go with decision 15's retraction rather than needing
  separate repair.
- **Negative — E2's exit criterion is retracted.** "ridl interfaces compile to
  Rust and a second backend from one IR" was met literally by output that no
  runtime can implement and no test compiles. Removing it removes shipped
  surface, and the roadmap records why.
- **Negative — typl §17.11 becomes a prerequisite rather than a deferral.**
  Widening a range flips the resolved width and shifts every subsequent slot
  offset. ADR-0013 decision 6 already required it closed before a FlatBuffers
  backend ships; decision 8 makes it block the store layout too.
- **Negative — E3.1 and the deferred `labels` promotion block decision 9.**
  `SIL_B` and `CAL_2` are free-form tokens today, outside the attribute registry
  by [ADR-0008](ADR-0008-e2-execution.md) decision 3, so every derivation over
  assurance levels waits on structure being given to them.
- **Neutral — deployment facts are hand-written until rsdl catches up**
  (decision 17). Two of the fields needed — protection domain and region
  reservation — do not exist in rsdl's specification today, so they would be
  designed either way.

## Open

1. **rsdl has no protection-domain concept.** §7 places components on targets;
   nothing says two components share a memory protection domain, which is the
   boundary decision 10 derives from.
2. **Whether the assurance zone is one attribute or two.** Safety and cyber
   collapse for decision 10 but partition a system differently — safety by
   criticality, security by exposure — and a QM infotainment stack is the
   highest-value attack surface in a vehicle.
3. **Unknown fields on decode.** proto3 preserves them for proxy round-trip;
   carrying fields the contract does not describe is arguably wrong for an SSOT,
   and dropping them breaks the gateway case decision 13 supports.
4. **Whose envelope an invalid sample carries** — the rejected publication's or
   the last good value's. ridl §4.5 implies the former and states neither.
5. **Whether `async fn` on a `command` is a conformance defect.** The shipped
   consumer trait has the application await an acknowledgment ridl §6.1 says is
   not application-visible and §6.2 says application code stays uninvolved in.
6. **The interface-granularity rule is unstated in ridl §14.** Splitting an
   interface splits the computation that produces it; nothing says so.
7. **Scoping.** Which deployment tiers are targets, and whether Android
   Automotive and Classic AUTOSAR are. `repr(C)` as a constrained-tier fallback,
   wasm viability and the FFI wrapper's value all resolve once these are
   settled.

## Documents amended

| Document                                      | Change                                                                                                        |
| --------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| [ADR-0013](ADR-0013-codegen-backend-scope.md) | open item 1 answered by decision 15; decision 1's target list superseded by decision 6                        |
| [ADR-0007](ADR-0007-e1-execution.md)          | decision 13's extern-C face retired by decision 6                                                             |
| [`docs/ROADMAP.md`](../ROADMAP.md)            | a runtime epic at the ridl layer (decision 16); the E2 exit criterion annotated with decision 15's retraction |

## References

- [`docs/wip/2026-08-08-runtime-and-codegen-architecture.md`](../wip/2026-08-08-runtime-and-codegen-architecture.md)
  — the reasoning trail, with the evidence
- [ADR-0012](ADR-0012-interaction-boundary-model.md) — decision 7, the model
  decision 9 follows for domain extensions
- [ADR-0013](ADR-0013-codegen-backend-scope.md) — the emit ceiling, the identity
  table, decision 4 on widths per class, decision 6 on width flips, open item 1
- [ADR-0014](ADR-0014-ir-encodings.md) — the IR encodings the plugin protocol
  and the tooling plane consume
- [ADR-0015](ADR-0015-qos-absorption-and-rpc-bounds.md) — the coherence rule
  decision 8 keys the store on
- [ADR-0016](ADR-0016-schema-projection-and-the-name-transform.md) — the
  projection properties decisions 4 and 8 rely on, decision 8 on deployment
  facts, decision 9 on `fixed`
- [ridl language reference](../specification/ridl-language-reference.md) — §3.1
  the envelope, §3.4 availability and RIDL-505, §4.2 direction, §4.3 coalescing,
  §4.4 and §4.5 last-value and provenance, §6.1 and §6.2 the command
  acknowledgment, §9 timing, §11 identity, §14.5 coherence
- [rsdl language reference](../specification/rsdl-language-reference.md) — §4
  wiring, §7 targets and placement, §8 transport and posture, RSDL-801
- [`docs/ROADMAP.md`](../ROADMAP.md) — E3.1, E4.5, E5.10, E5.12, E9.8, E9.9,
  E9.11
- `crates/ridl-backend-rust/src/interact.rs` — `vocabulary()` at line 81
- `crates/ridlc/tests/corpus.rs` — the two `rustc` compile tests
