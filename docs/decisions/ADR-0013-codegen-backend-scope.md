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
   Appendix D) — a wire backend refuses rather than approximates.

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

## Alternatives considered

| Candidate                                                    | Verdict  | Reason                                                                                                                                            |
| ------------------------------------------------------------ | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| Emit a proto3 `service` with a pub/sub sidecar convention    | rejected | moves §4.4 and §4.5 semantics into an undocumented sidecar the schema does not describe; the generated artifact then understates the contract     |
| Emit a `ServiceStore` message holding each signal's value    | deferred | a store slot needs value, provenance (§4.5), and envelope (§3.1) to answer freshness, which makes it a runtime contract rather than a wire schema |
| Per-kind identity tables — one enum per signal/event/command | rejected | ridl §11 numbers interactions in one interface-wide, kind-blind sequence; four tables would imply four numberings the language does not have      |
| Let each wire backend infer its own widths from the range    | rejected | typl §4.2 resolves width once in the checker so every backend agrees; a second inference site is a second source of truth that drifts silently    |
| Emit typl constants as proto3 enum members                   | rejected | proto3 enums are `int32`, so every float and regex constant is unrepresentable, and the integer ones would misstate a compile-time value as a tag |

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
  inference), §6 (constants), §17.11 (the deferred `wire` floor), Appendix D
  (codegen targets, the language and transport layers)
- `docs/specification/ridl-language-reference.md` — §3.1 (the implicit
  envelope), §4.4 (init value and last-value), §4.5 (invalid state), §6.1
  (delivery acknowledgment), §11 (interaction identity and evolution), §14.6
  (components and services), §17 open questions 5, 7, and 9, Appendix B
  (interaction mapping per target)
- ADR-0007 decision 9 — widths ride alongside the exact constraint strings in
  the IR
- ADR-0008 decision 7 — the TypeScript backend and the `internal` mapping
- `crates/ridl-ir/proto/ridl/ir/v2/ir.proto` — `Decl.ordinal`, `IntWidth`,
  `FloatWidth`, `TypeDef.width`, `EnumSetDef.width`
- `docs/ROADMAP.md` — E4.5 (the IR plugin protocol), E6.8 (transport and posture
  derivation)
