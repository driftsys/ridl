# ADR-0013 — Codegen backend scope: wire backends emit shape and identity, not faces

## Status

Proposed — 2026-08-03. Scope: what a codegen backend is permitted to emit, and
which width layer it reads. It is not epic-scoped: it binds every backend the
workspace grows, in the way ADR-0009 binds the gate and ADR-0010 binds the CLI
contract.

Decision 2's ceiling is settled for wire backends. **Whether the same ceiling
binds the language backends is open** — see Open item 1. Nothing here is
implemented; no proto3 or FlatBuffers backend exists.

## Context

The workspace has two backends, `ridl-backend-rust` (Rust source plus an
extern-C header) and `ridl-backend-ts`. Both are **language** backends: they
emit source a developer compiles against. typl Appendix D and ridl Appendix B
name a second class that does not exist yet — proto3, FlatBuffers, SOME/IP, DDS,
CAN/DBC, AIDL, JSON Schema — targets that describe bytes on a wire rather than
an API in a language.

The question that produced this ADR was whether a proto3 or FlatBuffers backend
should emit a `service` block alongside the messages. It should not, and the
reason generalises into a rule about backend classes.

### What a wire target cannot represent

A proto3 `service` is a method table over one primitive: request/response,
optionally streamed. Of ridl's five interaction kinds, exactly one — `query` —
is natively that. The other four each lose a normative property:

- **`signal`** — ridl §4.4 makes the channel never empty: an init value before
  the provider's first publication, the latest published value after, with
  provenance (init, live, invalid) per §4.5. A late subscriber to a
  server-streaming RPC receives nothing until the next publication. ridl
  Appendix B already concedes this, offering "server-streaming RPC **or pub/sub
  sidecar**".
- **`event`** — server-streaming makes subscription client-initiated and
  unicast; there is no eventgroup fan-out.
- **`command`** — §6.1 requires a delivery acknowledgment that never surfaces in
  the generated application API as a return value. A gRPC unary return is
  exactly such a value.
- **`fixed`** — a provisioning-time constant becomes a runtime getter call.

The deeper mismatch reaches `query` as well: §3.1's implicit envelope
(sender-stamped timestamp, per-channel monotonic sequence number) is what TTL,
debounce, freshness, duplicate suppression, loss detection, E2E protection, and
deterministic replay all run on. Neither proto3 nor FlatBuffers has an envelope.

SOME/IP and DDS map cleanly in ridl Appendix B because each has a native
primitive for continuous state with a retained current value — SOME/IP fields
with a notifier and a derivable getter, DDS `TRANSIENT_LOCAL` durability. gRPC
has only calls. This is a property of the target, not a shortfall in the
emitter, and no amount of emitter work removes it.

### What the IR already carries

Nothing below needs new compiler work. `Decl.ordinal` carries ridl §11's 1-based
interaction ordinal, one sequence per interface across all kinds with tombstones
counted. `TypeDef.width` carries the resolved wire width as a
`oneof { IntWidth, FloatWidth }`, computed once by the checker per typl §4.2 and
§4.3, with all eight integer widths distinct so `ridl-diff` sees a width flip as
a change; `EnumSetDef.width` carries the same for enum sets. The existing
backends discard the width field on purpose, because typl Appendix D fixes the
language layer at `int64`/`float64`.

## Decision

1. **A backend is classified by what its target can faithfully represent.** A
   **language backend** emits source in a general-purpose language (Rust,
   TypeScript, and later Kotlin and C++). A **wire backend** emits a schema
   describing bytes in transit (proto3, FlatBuffers, and the remaining targets
   of typl Appendix D and ridl Appendix B). The class fixes both the emit
   ceiling and the width layer.

2. **A wire backend emits shape and identity, and no interaction face.** Two
   tiers, and nothing above them:

   | Tier | Emit                                               |
   | ---- | -------------------------------------------------- |
   | 1    | the typl surface — messages, tables, enums, unions |
   | 2    | the interaction identity table (decision 3)        |

   No `service` block, no call face, no value store. A wire backend that cannot
   represent §4.4 last-value, §4.5 provenance, or the §3.1 envelope must not
   emit a construct that implies it does. Where the language already refuses a
   construct on a target — optionals and unions on AUTOSAR Classic and CAN (typl
   Appendix D) — a wire backend refuses rather than approximates. **Decision 7
   below narrows this for optionality specifically:** a target that can carry
   the fact of absence, but not structurally, realises it rather than refusing.

3. **Every backend emits the interaction identity table.** ridl §11's ordinals
   become a generated, keyed table per interface, with retired ordinals held
   against reuse — proto `reserved` on the enum, the equivalent construct in
   each language. This gives every binding one shared numbering with no sidecar
   file, which is what §11 already promises ("readable from source, no sidecar
   state").

   The table is **interface-wide and kind-blind**, matching §11's single
   sequence. Per-kind identifiers such as SOME/IP's method ID and event ID are
   binding transformations applied over that identity, never a second numbering.
   In proto3 the enum needs an `UNSPECIFIED = 0` member, because ridl ordinals
   are 1-based.

   This is new work in `ridl-backend-rust` and `ridl-backend-ts`, which today
   carry ordinals only as `@ordinal` doc comments and JSDoc tags.

4. **A wire backend reads the transport width; a language backend reads the
   language width.** Both read `TypeDef.width` and `EnumSetDef.width` from the
   same IR; the class decides whether the field is honoured or discarded for
   typl Appendix D's widest-always rule. Three consequences bind the emitter:

   - proto3 has no `uint8` or `uint16`; both resolve to `uint32`, and varint
     encoding is what keeps small values small. The width decision that matters
     in proto3 is signedness — a range containing negatives resolves to `sint32`
     or `sint64`, because plain `int32` varint costs 10 bytes for every negative
     value.
   - FlatBuffers uses the full `uint8..uint64` palette, where the narrow width
     is real bytes saved.
   - A quantized float keeps its native `float`/`double` form on proto3 and
     FlatBuffers. The scaled-integer encoding of typl §4.3 belongs to CAN/DBC
     always and SOME/IP per deployment, and a wire backend must not apply it
     unasked.

5. **typl `const` has no wire form and is not emitted by a wire backend.**
   Neither proto3 nor FlatBuffers has a constant declaration: proto3 offers enum
   values (always `int32`) and descriptor options; FlatBuffers offers per-field
   scalar defaults. Neither is a standalone constant. This is a category
   difference rather than a gap — a typl constant is a compile-time value used
   in range constraints, `match` patterns, and init declarations (typl §6), and
   no instance of it ever crosses a wire. Constants stay with the language
   backends, which already emit them. A wire backend may emit them as comments
   and must not encode them as enums.

6. **A FlatBuffers backend makes typl §17.11 a precondition.** Because the width
   is derived from the declared range, widening `[0..255]` to `[0..300]` flips
   `uint8` to `uint16` with no edit to any width declaration. On FlatBuffers
   that is a hard wire break. typl Appendix D names `ridl-diff` as the sole
   guard in v0.1 and defers an explicit width floor — a `wire` clause — to typl
   §17.11. That open question must be closed before a FlatBuffers backend ships,
   not after.

7. **Amendment (2026-08-07) — field absence is declared once and realised per
   target.** typl §7.1's `?` states that a field may carry no value. That is a
   contract fact, not a transport fact, so it binds every backend; what differs
   is realisation.

   - **A target that can represent absence structurally does so** — `Option<T>`
     in Rust, `T | undefined` in TypeScript, proto3 presence tracking, an
     omitted CBOR map key, a DDS optional member.
   - **A target that cannot must realise absence in-band**, taking a value the
     type's declared range does not use, and **must not surface that value in
     the generated application API**. A CAN consumer and a DDS consumer of one
     contract both read "no value"; neither reads a number whose meaning is
     private to the protocol. A realisation that leaks the value into consumer
     code is the approximation decision 2 forbids, not an instance of this
     decision.
   - **An in-band realisation requires the range to have room.** Where the
     declared range leaves no value spare at the resolved width, the backend
     fails with a diagnostic rather than choosing silently. This is ADR-0016's
     totality property — a projection is defined for every input or the backend
     refuses — applied to values rather than to identifiers.

   This narrows decision 2 for optionality alone. Refusal remains correct where
   the target cannot carry the fact at all; realisation is correct where it can
   carry the fact but not the structure, which is what typl Appendix D's "never
   silently default-filled" was guarding against.

   **Nothing changes today.** No current backend is in the second class: Rust,
   the extern-C header, and TypeScript all represent absence structurally. typl
   Appendix D's existing rule — a `?` field on a Classic/CAN-bound struct is a
   codegen error — stands until a backend exists that this decision governs.
   What is recorded here is the rule that will govern it, so the first such
   backend implements a decision rather than inventing one.

   The remaining question is not how a backend chooses a value but what happens
   when it may not choose: conformance to a published standard that fixes the
   value, and in some cases fixes more than one with distinct meanings. That is
   typl §17 open question 8, narrowed to that case by this amendment.

## Alternatives considered

| Candidate                                                    | Verdict  | Reason                                                                                                                                                                                                                                 |
| ------------------------------------------------------------ | -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Emit a proto3 `service` with a pub/sub sidecar convention    | rejected | moves §4.4 and §4.5 semantics into an undocumented sidecar the schema does not describe; the generated artifact then understates the contract                                                                                          |
| Emit a `ServiceStore` message holding each signal's value    | deferred | a store slot needs value, provenance (§4.5), and envelope (§3.1) to answer freshness, which makes it a runtime contract rather than a wire schema                                                                                      |
| Per-kind identity tables — one enum per signal/event/command | rejected | ridl §11 numbers interactions in one interface-wide, kind-blind sequence; four tables would imply four numberings the language does not have                                                                                           |
| Let each wire backend infer its own widths from the range    | rejected | typl §4.2 resolves width once in the checker so every backend agrees; a second inference site is a second source of truth that drifts silently                                                                                         |
| Emit typl constants as proto3 enum members                   | rejected | proto3 enums are `int32`, so every float and regex constant is unrepresentable, and the integer ones would misstate a compile-time value as a tag                                                                                      |
| Let a type declare its own reserved wire values in typl now  | deferred | decision 7 needs no syntax: `?` already declares the fact and the backend chooses the value. Syntax earns its place only when a published standard fixes the value, and no such target is being implemented — typl §17 open question 8 |
| Surface the reserved value to consumers as a named constant  | rejected | puts the protocol's private knowledge in every consumer, which is the leak decision 7's second clause forbids; the value is a wire fact, never an API fact                                                                             |

## Consequences

- **Positive — a wire backend becomes small and buildable.** Tiers 1 and 2 read
  fields the IR already carries. No new IR nodes, no new checker pass, no new
  diagnostics beyond the unrepresentable-construct refusals the class implies.
- **Positive — one identity across every binding.** Decision 3 gives Rust,
  TypeScript, and every wire target the same interface-wide numbering, derived
  from the contract and already guarded as append-only by `ridl-diff`.
- **Positive — the generated schema stops claiming to be the contract.** A
  `.proto` carries no ranges, no units, no steps, no `match` patterns, and no
  constants. Scoping it to shape plus identity makes that honest, and keeps the
  ridl source as the single source of truth.
- **Negative — a wire target needs a binding to be useful.** Tiers 1 and 2 do
  not produce a callable API. The service face is a deployment concern (ridl
  §14.6, open questions 5 and 9; `docs/ROADMAP.md` E6.8), so a consumer wanting
  gRPC needs work this ADR places outside the backend.
- **Negative — decision 3 is work on shipped code.** Both existing backends gain
  an emit and the snapshot tests that cover them.
- **Neutral — the ceiling may not describe the language backends.** See Open
  item 1.
- **Positive — the first sentinel-emitting backend implements a rule instead of
  inventing one** (decision 7). The choice of value, the obligation not to leak
  it, and the failure mode when the range is full are settled before any target
  needs them, which is the cheapest moment to settle them.
- **Neutral — decision 7 costs nothing today and is unexercised.** No current
  backend is in its second class, so the rule ships untested against a real
  target. That is deliberate: writing the syntax a conformance target would need
  before such a target exists would be building for a case whose shape is not
  yet known.

## Open

1. **Does the decision 2 ceiling bind the language backends?** Read literally it
   would retract `ridl-backend-rust`'s and `ridl-backend-ts`'s interaction layer
   — the consumer and provider trait pairs, `SignalHandle`, `EventHandle`,
   `Provenance`, the metadata constants, and the service table — which is the E2
   exit criterion. The argument against is decision 1: the ceiling follows from
   what a target can represent, and Rust and TypeScript can represent
   last-value, provenance, and asynchronous calls, which is why those backends
   already do. Recorded as open because it was not settled when this ADR was
   drafted.
2. **Where a `ServiceStore` lives if it is built.** The candidates are the
   `ridl-rt` runtime specification and the reflection interface of ridl open
   question 7. Whatever the home, its slot keys must be the interaction ordinals
   of decision 3, so that one contract has one identity space.
3. **The concrete form of the identity table per language.** A Rust `enum` with
   explicit discriminants, a set of associated constants, or a lookup table;
   likewise for TypeScript. Decision 3 fixes the content and the numbering, not
   the construct.

## References

- `docs/specification/typl-language-reference.md` — §4.2 and §4.3 (width
  inference), §6 (constants), §7.1 (`?` optionality — the fact decision 7
  realises), §17.11 (the deferred `wire` floor), §17 open question 8 (the
  standards-conformance case decision 7 leaves open), Appendix D (codegen
  targets, the language and transport layers)
- `docs/specification/ridl-language-reference.md` — §3.1 (the implicit
  envelope), §3.4 (availability, and why field absence is not one of its five
  sources), §4.4 (init value and last-value), §4.5 (invalid state), §6.1
  (delivery acknowledgment), §11 (interaction identity and evolution), §14.6
  (components and services), §17 open questions 5, 7, and 9, Appendix B
  (interaction mapping per target)
- ADR-0016 decision 6 — the projection contract's totality property, which
  decision 7's range-has-room obligation instantiates for values
- ADR-0007 decision 9 — widths ride alongside the exact constraint strings in
  the IR
- ADR-0008 decision 7 — the TypeScript backend and the `internal` mapping
- `crates/ridl-ir/proto/ridl/ir/v2/ir.proto` — `Decl.ordinal`, `IntWidth`,
  `FloatWidth`, `TypeDef.width`, `EnumSetDef.width`
- `docs/ROADMAP.md` — E4.5 (the IR plugin protocol), E6.8 (transport and posture
  derivation)
