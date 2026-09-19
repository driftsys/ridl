# The generated interaction face (E11.13, the MVP)

The Rust backend's generated `Client`/`Publisher`/`Provider`/`dispatch` face
over `ridl-rt`, as built by roadmap story E11.13. ADR-0018 decision 15 restores
this face as the runtime layer's "phase 2", sequenced after the frame
specification (E11.1) and the transport and loopback runtime (E11.9). E11.13 is
a deliberate exception to that sequence: an in-process-only MVP, built ahead of
both so the team has a face to write against. It generates, for one example
package, the consumer and provider faces of one interface, proves them with an
in-process round trip in a test, and carries several placeholders each named
below and tied to the story that replaces it. This is the architecture as built;
the generation decisions that bind future work on it are
[ADR-0023](../decisions/ADR-0023-interaction-face-generation.md). Read this
record together with [the `ridl-rt` design record](ridl-rt.md), which the face
binds against, and the
[ridl language reference](../specification/ridl-language-reference.md) §6 and §7
(command and query), which `dispatch`'s settlement table implements.

## Scope and the two entry points

`crates/ridl-backend-rust` gained two new modules and one new public function:

- `src/descriptors.rs` — one `ridl_rt::contract::Interface` implementation per
  named interface, one `Interaction` implementation per member, and the
  generated buffer-size constants.
- `src/face.rs` — `Client`, `Publisher`, `Provider` and `dispatch`.
- `src/clauses.rs` — the contract-clause translator. Only `src/descriptors.rs`
  calls it, when it emits a `Command`'s or a `Query`'s `require` and `ensure`
  bodies. `src/face.rs` names those generated methods from the `dispatch` body
  it writes, but does not translate a clause itself.

`generate(package)` — the existing pipeline entry point `ridl --emit rust` calls
— keeps its pre-E11.13 output exactly: the domain types, naming no runtime.
`generate_face(package)` is a companion entry point that emits what `generate`
emits, plus the descriptor and face items. It is the only caller of the clause
translator, and the only entry point whose output names `::ridl_rt::…`.
**`ridl --emit rust` does not yet emit the face** — the pipeline
(`crates/ridlc/src/lib.rs`) still calls `generate`, not `generate_face` — which
is correct for this story rather than a shortfall: ADR-0018 decision 15 makes
the face phase 2, and nothing in E11.13 ships a runtime for a pipeline consumer
to link against. Why a companion entry point rather than folding the face into
`generate` is [ADR-0023](../decisions/ADR-0023-interaction-face-generation.md)
decision 2.

## The descriptors

For each named interface reached through `Package::shapes()` (a service's inline
shape is skipped — its identity name is a dotted service name, not a single Rust
identifier, and descriptors for one are a follow-up), the emitter writes:

- a unit struct with an `Interface` impl carrying `CATALOG`, `NUMBER`,
  `PROVISIONAL`, `NAME` and `MEMBERS`, plus two associated constants,
  `MAX_BUFFER_SIZE` and `EVENT_SOURCE_BUFFER_SIZE`;
- a unit struct per interaction with an `Interaction` impl (`type Iface`,
  `const MEMBER`) and the kind's trait — `Signal`, `Event`, `Command`, `Query`
  or `Fixed`.

**The interface number is the IR's own, never invented.** The example package
carries no `interfaces.lock`, so `Interface::NUMBER` and
`Interface::PROVISIONAL` come straight from the IR's provisional assignment
(`Interface.number`, `Interface.provisional`, added by the lock design's L4).

**The two buffer constants are computed, not counted.** `MAX_BUFFER_SIZE` is the
maximum over every argument and reply `<T as Payload<ReprC>>::MAX_SIZE` the
interface's calls use — arguments alone would under-size a dispatch buffer
whenever a query's reply is larger than its argument, because a reply is encoded
into the same buffer. `EVENT_SOURCE_BUFFER_SIZE` is the maximum over event
payload sizes only, because `EventSource::next`'s payload type is not known
until the occurrence's ordinal is read. Both are emitted as a const-evaluable
block (a `while` loop over an array), not as a folded literal, so the maximum is
computed by the same code a consumer compiles, not precomputed and trusted by
the emitter.

**Two placeholders, until their replacing stories land:**

- `CATALOG.hash` is `CatalogHash([0u8; 32])`. The real hash (SHA-256 over the
  package's reachable closure) is story E16.2 (driftsys/ridl#378). This is safe
  only because `CatalogRef` equality compares the name and the hash together,
  and the example package is the only catalog in the process — a second
  all-zero-hash catalog with the same name would collide.
- Every `PayloadInfo.max_size` field
  (`EncodedSizes { proto3, flatbuffers,
  repr_c }`) is `None`. `ridl-rt`'s own
  reading of `None` is the broad one — "no size is available here", never "this
  payload cannot be encoded this way" (`crates/ridl-rt/src/contract.rs`) — and
  the MVP genuinely cannot derive any of the three sizes, so the emitted `None`
  is honest under it. The doc comment the emitter writes into every generated
  file (`crates/ridl-backend-rust/src/descriptors.rs`) still says that `ridl-rt`
  states the other, narrower reading and that E16.2 reconciles the two. That
  note is stale: the `ridl-rt` doc-comment change it refers to has landed.
  Dropping it is a change to the emitter and to the checked-in fixture the
  byte-equality guard compares against, so it is not made here. `dispatch` never
  reads these fields — it sizes buffers from `<T as Payload<ReprC>>::MAX_SIZE`
  directly — so the absent sizes cost the face nothing and cost a future catalog
  consumer everything, which is the right way round for a placeholder.

## The contract-clause translator

The IR carries a `require`/`ensure` clause only as canonical ridl source text
(`Contract.source`, e.g. `"window > 0"`), not as an expression tree — E5.1 is
the story that adds the tree. `src/clauses.rs` is a narrow, total translator
built to make the two clause-driven settlement rows real without pre-empting
that story:

- **Accepted form:** `<subject> <comparison> <numeric literal>`, where
  `<subject>` is the interaction's single declared parameter (or `result` on a
  query's `ensure`) and `<comparison>` is one of `< <= > >= == !=`. The
  subject's named type must be an integer- or float-backed scalar.
- It emits `args.0 <op> <literal>` for a parameter and `reply.0 <op> <literal>`
  for `result`; several clauses of one kind conjoin with `&&`.
- **Every other clause form is refused with a `GenerateError`**, never dropped
  and never silently emitted as `Ok(())`. A dropped clause would generate a
  provider that accepts arguments its own contract forbids.
- A command or query declaring no clause of a kind emits `Ok(())`.

Why the translator refuses rather than widens, and why it lives behind
`generate_face` rather than `generate`, is
[ADR-0023](../decisions/ADR-0023-interaction-face-generation.md) decision 1.

## The consumer and provider faces

`src/face.rs` emits, per named interface with at least one member the face
covers, one `pub mod` holding:

- **`Client<'a, P: ...>`** — one read method per signal, a `subscribe_*` and a
  shared `next_event` per interface's events, one send method per command and
  query, a `*_reply` poll per query, and `ack` if the interface declares a
  command. The trait bounds on `P` are computed from the interface's actual
  interaction kinds and no others: `SignalReader` only if it declares a signal,
  `EventSource` only if it declares an event, `Caller` only if it declares a
  command or a query. This is RA-19: a `Client` never carries a bound its own
  interface does not need. It is proven, not merely asserted, by
  `ra19_a_minimal_signal_only_port_constructs_the_signal_only_client` in
  `tests/interaction_face.rs`, which constructs the fixture's signal-only `Horn`
  interface's `Client` with a port implementing only `SignalReader` and
  `Attached` — a bound the emitter should not have added would fail this to
  compile. (The converse — that a bound the interface does need is never missing
  — is not separately proven: under-bounding cannot compile at all, because a
  method body that needs a port trait the bound omits fails to build.)
- **`Publisher<'a, W: ...>`** — over `SignalWriter` and `EventSink` on the same
  rule, with `invalidate_*` per signal and `commit`.
- **`trait Provider`** — one method per command and query, generated only when
  the interface declares one. **A method takes its argument by reference**
  (`fn set_level(&mut self, level: &Level)`), because `dispatch` reads the
  argument again when it evaluates a query's `ensure` clause after the provider
  returns, and the generated payload types implement neither `Copy` nor `Clone`.
  A command method returns nothing — a command has no failure the application
  reports (ridl §6.1) — and a query method returns its declared reply type.
- **`fn dispatch<H: Handler, P: Provider>(h: &mut H, p: &mut P, buf: &mut [u8])
  -> usize`**
  — see the next section.

**A `Client` method's send call returns `Result<Correlation, SendError>`.**
`SendError` is the `Caller` port's own return type and already carries
`Contract`, so a `require` clause that fails client-side is reported as
`SendError::Contract(Contract::PreconditionFailed)` with no lossy mapping into
the settlement side's `CallError`. Why the parameter is by reference and why the
return type is `SendError` rather than `CallError` — a gap the M1 design left
open — is [ADR-0023](../decisions/ADR-0023-interaction-face-generation.md)
decisions 3 and 4.

**Nothing here waits (RA-20).** No generated method spawns a thread, holds a
future, opens a socket, or reads a timer. A query send returns a `Correlation`
immediately; a separate `*_reply` method polls it without blocking. `dispatch`
makes one pass over the claims a handler already has and returns; the loop that
calls it repeatedly belongs to the application or the runtime.

## `dispatch` and the settlement table

`dispatch` requires `buf.len() >= <Interface>::MAX_BUFFER_SIZE` before it does
anything else; a shorter buffer returns `0` immediately, without consuming a
claim, so the caller can retry with a correctly sized one. It then loops over
`Handler::next_claim`, routing and settling every claim it takes — including one
this interface does not recognise, because `Handler`'s own contract requires
every claim to be settled:

| Cause                                       | Settled as                     |
| ------------------------------------------- | ------------------------------ |
| `claim.ord` or `claim.iface` matches no arm | `Contract::UnknownInteraction` |
| `VerifyError::Structure(_)`                 | `Transport::Corrupt`           |
| `VerifyError::Contract(v)`                  | `Contract::InvalidValue(v)`    |
| `require` returns `Err(())`                 | `Contract::PreconditionFailed` |
| `ensure` returns `Err(())` (query only)     | `Contract::ContractBroken`     |

The two `VerifyError` rows are kept separate rather than both settled as
`Transport::Corrupt`, because `Payload::verify` reports a structural failure and
a typl-constraint violation as two distinct variants, and collapsing them would
report a range or enum-variant violation in the wrong error stratum (ridl §10.2
vs §10.3).

**A command settles before the provider method runs; a query settles after.** A
command's acknowledgment is a delivery acknowledgment, not a completion one
(ridl §6.1), so once its arguments and `require` pass, `dispatch` calls
`Handler::settle(claim.id, Ok(&[]))` and only then calls the provider's method.
A query settles after the provider returns and `ensure` is evaluated, because
its settlement carries the reply. **This ordering is pinned only by an
exact-text assertion** in `tests/dispatch_generation.rs`; no behavioural test
exercises it, because neither `Handler::settle` nor a provider call in the
test-only loopback of the next section has an observable side effect a
reordering would change. E11.9's real runtime is what would make the ordering
observable — this is a limitation of the test double, not a defect in the
generated code.

**`EncodeError::Capacity` is a provider-side invariant violation, never a
manufactured contract error.** Every buffer `dispatch` and the face encode into
is sized from `<T as Payload<ReprC>>::MAX_SIZE`, the largest encoded size of any
legal value, so a legal value cannot exceed it. If `Ref::encode` still returns
`Capacity`, the provider returned a value outside its own type's range, or a
hand-written `Payload` implementation does not honor `MAX_SIZE` — a defect the
generated code has no vocabulary to describe as one of the five settlement
outcomes, because `EncodeError::Capacity` carries no received bytes and no
violated rule. The generated branch is an explicit `unreachable!` naming the
type, the needed size and the available size. No runtime test drives this
branch: doing so needs a `Payload` implementation that lies about `MAX_SIZE`
without corrupting the buffers the round trip's other assertions share, and
nothing in the fixture isolates one type enough to do that safely under the test
binary's parallel execution.

**`dispatch`'s returned count is the number of claims `Handler::settle`
accepted, not the number of claims taken.** A `SettleError` is left to the
handler — which already owns that claim's settlement — and is not turned into a
different `CallError`; `dispatch` continues to the next claim regardless.
`tests/interaction_face.rs`'s
`round_trip_dispatch_counts_only_accepted_settlements` injects one settlement
failure followed by one successful claim and asserts the counts are `0` then
`1`. It is a runtime test through `dispatch`, not a source-text assertion;
`tests/dispatch_generation.rs` holds only the latter.

## The payload stand-in and the test-only ports

**No codec is generated.** ADR-0020 sanctions three payload encodings — proto3,
FlatBuffers, `repr(C)` — and none is built (E11.7, E11.8, E11.12 are all out of
scope). The face is generic over `T: Payload<E>` and calls only `Ref::encode`,
`Ref::verify` and `Ref::decode`, so it does not matter to the generated code
whether a `Payload` implementation is generated or hand-written. `ReprC` was
chosen as the marker because its stand-in needs no dependency and no schema, and
because E11.12's real codec is expected to disturb it least.
`tests/interaction_face.rs` hand-writes `Payload<ReprC>` for the fixture's
fixed-width scalars, its one enum, and its one all-fixed-size struct; the
implementation is explicitly marked throwaway in its own module documentation
and deleted when E11.12 lands.

**The ports are a disposable, test-only loopback**, not a runtime. `ridl-rt`
ships no runtime; the first real one is E11.9's in-process loopback (ADR-0020
decision 6). `tests/support/loopback.rs` implements exactly `Attached`, `Clock`,
`SignalReader`, `SignalWriter`, `EventSource`, `EventSink`, `Caller`, `Handler`
and `FixedReader` over in-memory queues and a hand-advanced counter clock — no
I/O, no thread, no real time — and deliberately implements neither
`ScannableSignals` nor `CoherentSignals` (both are optional extensions a runtime
may omit). Its own module documentation names E11.9 as its replacement.

## The fixture and the round trip

`crates/ridl-backend-rust/tests/fixtures/interaction_face.ridl` is a
single-file, single-catalog package (`face.demo`) cut to what the `ReprC`
stand-in can carry — fixed-width named scalars, one enum, one all-fixed-size
struct, no optional field, sequence, map, union, string or bytes — declaring two
interfaces:

- **`Cabin`** — one signal, one event, one command (`setLevel`, with a `require`
  clause) and one query (`average`, with a `require` and an `ensure` clause).
  This is the interface the round trip runs against: publish and read a signal,
  raise and receive an event, send the command and the query through `Client`,
  run `dispatch` against a `Provider`, and observe the acknowledgment and the
  reply, plus the `PreconditionFailed` and `ContractBroken` settlements the two
  clauses make reachable. Every call declares exactly one parameter of a
  declared named type, and every query replies with one declared named type —
  the face emits no induced argument struct, so a call needing more than one
  parameter is refused (`descriptors::single_param_type`), and a multi-parameter
  call is a recorded follow-up, not E11.13 work.
- **`Horn`** — one signal and nothing else, existing only to make RA-19's claim
  testable in the direction described above: a minimal port cannot construct a
  `Client` unless the emitted bounds are exactly what the interface needs.

The fixture declares no `fixed` interaction; the descriptor emitter's `Fixed`
path is covered instead by a hand-built-IR unit test in `src/descriptors.rs`.

**The generated face is checked in, not regenerated at test time.**
`tests/generated/interaction_face.rs` is `generate_face`'s output for the
fixture, brought into `tests/interaction_face.rs` with `include!` under a
handwritten module carrying only outer `#[allow(...)]` lint attributes — the
emitter itself must never emit an inner attribute, because one inside an
`include!`d file is a hard compile error, and this constraint is unchanged by
E11.13. A second test in the same file regenerates the fixture and compares the
result byte-for-byte against the checked-in file, failing with the instruction
to regenerate
(`RIDL_UPDATE_GENERATED=1 cargo test -p
ridl-backend-rust --test interaction_face`)
when it drifts. This is what makes the generated face genuinely compiled and
linked by `cargo test`, rather than only snapshotted — the defect ADR-0018's
Alternatives-considered table records against the retracted interaction layer
("it cannot be connected to a runtime at all, and has never been compiled").

Four test files exercise the layers separately before the round trip:
`tests/descriptor_generation.rs`, `tests/face_generation.rs`,
`tests/dispatch_generation.rs` (source-text assertions on the generated code),
and `tests/interaction_face.rs` (the compiled round trip, the loopback support
module, and the byte-equality guard).

## Coupling with Lane C's Epic 10

`crates/ridl-backend-rust/src/lib.rs` is shared with Lane C's Epic 10, which
reshapes the domain-type emission this face's generated code names as argument
and return types. The expected order was Epic 10's Task 4 before E11.13; when
that does not hold, the cost is a touch-up pass to the checked-in fixture and
the hand-written `Payload<ReprC>` implementations, accepted as rework rather
than a blocker.

## What is provisional

| Placeholder                                                                                    | Replaced by                               |
| ---------------------------------------------------------------------------------------------- | ----------------------------------------- |
| The hand-written `Payload<ReprC>` implementations                                              | E11.7, E11.8 or E11.12                    |
| The test-only loopback ports (`tests/support/loopback.rs`)                                     | E11.9                                     |
| The zero `CatalogHash`                                                                         | E16.2 (driftsys/ridl#378)                 |
| The all-`None` `EncodedSizes` columns                                                          | E16.2                                     |
| The narrow contract-clause translator (`src/clauses.rs`)                                       | E5.1                                      |
| One declared parameter per call, no induced argument struct                                    | a recorded follow-up story                |
| The command-settled-before / query-settled-after ordering, pinned only by exact-text assertion | E11.9 (makes it behaviourally observable) |

`ridl --emit rust` emitting the face itself is not on this list as a defect:
ADR-0018 decision 15 places that behind the frame specification and the
transport, and nothing in E11.13 changes that gate.

## Trace

- Roadmap: `docs/ROADMAP.md` — E11.13
- Tracking issue: driftsys/ridl#393
- Binds: [ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md)
  decision 15;
  [ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
  decisions 1, 5, 6 and 7;
  [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md);
  [ADR-0023](../decisions/ADR-0023-interaction-face-generation.md) — the
  generation decisions specific to this face
- Depends on: `crates/ridl-rt` 0.1.0 (E11.0, landed); the IR's provisional
  interface numbering (the lock design's L4, driftsys/ridl#391)
- Replaced later by: E11.7, E11.8 or E11.12 (the payload stand-in), E11.9 (the
  test-only ports), E16.2 (the catalog hash and the encoded sizes), E5.1 (the
  clause translator)
- Reasoning trail (archived):
  [`2026-09-15-lane-m-driver.md`](../archive/2026-09-15-lane-m-driver.md),
  [`2026-09-16-interaction-face-v0-design.md`](../archive/2026-09-16-interaction-face-v0-design.md),
  [`2026-09-17-interaction-face-v0-plan.md`](../archive/2026-09-17-interaction-face-v0-plan.md)
- `crates/ridl-backend-rust/src/descriptors.rs`, `src/face.rs`,
  `src/clauses.rs`, `src/lib.rs` (`generate_face`) — the emitter as built
- `crates/ridl-backend-rust/tests/fixtures/interaction_face.ridl`,
  `tests/generated/interaction_face.rs`, `tests/support/loopback.rs`,
  `tests/interaction_face.rs` — the fixture, the checked-in output, the
  test-only ports, and the round trip
