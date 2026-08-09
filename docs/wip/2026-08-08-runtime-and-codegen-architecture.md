# Runtime and codegen architecture

Status: working note, 2026-08-08. Supersedes the generated-surface framing this
file previously carried, which asked a narrower question — how the Rust
backend's output should be written — and was overtaken by the answer.

Scope: what the toolchain generates, what the runtime provides, what the
platform supplies, and the order the work lands in. It covers the emit model,
the two encodings, the store and frame protocol, and the trust rules that govern
access to them.

Nothing here is implemented.

## 1. Summary

The system is five layers. Application code sits on generated bindings, which
sit on a portable runtime core (`ridl-rt`), which sits on a small platform
abstraction, which sits on the OS. Everything above the platform layer is
identical on every target.

Two encodings and no more: **proto3 on the network, FlatBuffers in memory.**
Signals live in a shared-memory store with a seqlock; everything else is framed
messages. The frame is one protocol with several bindings — Unix socket, Binder,
WebSocket, wasm host call.

All allocation is static, computed from the SSOT: slot sizes from typl's bounds,
ring depths from the contract's timing and the target's jitter, region sizes and
channel inventories from the wiring graph.

## 2. What the backend emits today

Every claim here is from the shipped snapshots and the generator. It is recorded
because it is the starting point, not because any of it survives.

### 2.1 The types carry no constraints

A named scalar becomes a newtype with a public field:

```rust
#[repr(transparent)]
pub struct Speed(pub f64);
```

`Speed(9999.0)` and `Speed(f64::NAN)` both construct, so the typl range, unit
and step reach Rust as doc comments and nothing else. Composite types keep the
ridl spelling of their fields (`pub sensorId: i64`), enum variants come out
`FILTER_INVALID`, and no type carries any derive — `DoorPayload` cannot be
printed, cloned or compared.

### 2.2 The interaction vocabulary is regenerated per package

`crates/ridl-backend-rust/src/interact.rs:81` emits `vocabulary()` once per
package that declares an interface, so each package gets its own `Provenance`,
`SignalHandle`, `EventHandle`, `TimingConst` and `ContractStub`. Two packages
produce two incompatible `Provenance` enums.

That is what makes the layer unconnectable to a runtime: a crate cannot ship
`impl SignalHandle<T> for Signal<T>` for a trait that does not exist until
codegen runs and then exists once per package.

### 2.3 The interaction layer compiles, which is not the same as usable

`crates/ridlc/tests/corpus.rs` runs `rustc` over the generated Rust for the
interaction-bearing corpora — `veh_cluster_generated_rust_compiles_with_rustc`
and `services_workspace_composed_compiles_with_rustc` — and `tsc` over the
TypeScript, each with an anti-vacuity guard asserting the faces are in the
source the compiler sees.

So the layer is syntactically sound. What no test can show is an implementation
of it, because none can exist for the reason §2.2 gives.

### 2.4 A discriminant beside data

`TimingConst` carries `mode` beside two `Option<u64>`; `ContractStub` carries
`kind` beside a `uses_result` flag meaningful only for `Ensure`; `read` returns
`(T, Provenance)`, so `let (v, _) = h.read();` silently discards the provenance
that ridl §4.5 exists to make unmissable.

The third case also cannot accommodate ridl §3.1's envelope: an `Init` value was
never published and has no envelope, which a flat pair can only express as
`Option<Envelope>` with the invariant held in prose.

### 2.5 The extern-C header is layout-only

It emits fixed-layout structs and drops every type with a string, an optional or
a collection, and every interface. For a realistic package that is one struct
and one enum retained, three structs and the whole interface dropped. No
`extern "C"` functions are emitted to match it.

## 3. The layered architecture

| Layer | Contents                                                                                              | Origin              |
| ----- | ----------------------------------------------------------------------------------------------------- | ------------------- |
| 4     | components, models                                                                                    | hand-written        |
| 3     | types and validation, store layout, slot table, frame codecs, wire codecs, bridge transcoders         | generated from SSOT |
| 2     | seqlock discipline, ring protocol, control-plane state machine, subscription table, envelope stamping | `ridl-rt`           |
| 1     | socket, region, notify, clock                                                                         | platform traits     |
| 0     | Linux, QNX, Android, RTOS, wasm host, browser                                                         | —                   |

Layer 1 is four traits, and layer 2 is identical on every target. Porting is
those traits plus a driving loop of roughly fifteen lines.

### 3.1 Layer 2 is sans-IO

`ridl-rt` performs no I/O. It is a state machine — bytes in, events and outbound
bytes out — and the caller owns every read and write:

```rust
pub fn on_bytes(&mut self, buf: &[u8]) -> Result<(), Error>;
pub fn poll(&mut self, now: Timestamp, out: &mut EventBuf) -> Deadline;
pub fn pending_out(&self) -> &[u8];
pub fn consume_out(&mut self, n: usize);
```

`poll` returns a deadline, which is how rate floors, staleness bounds and
acknowledgment timeouts work with no executor: the runtime says when to wake it
and the caller's poller honours that.

Three properties follow. One layer 2 drives from an epoll loop, an RTOS task, a
browser callback or a Tokio task without change. A `poll` does bounded work, so
worst-case execution time is arguable. And tests need no executor, no sockets
and no real time — which is what E5.10's native-versus-wasm trace equality
requires.

The decisive property is asymmetry: **a sans-IO core can present an async face;
an async core cannot present a sans-IO face.** An async wrapper is a thin
adapter; the reverse needs an executor, which is the dependency being avoided.
An async wrapper ships for the Tokio and Deno side.

Per-OS optimisation — `io_uring`, busy-polling, batched syscalls,
interrupt-driven loops — happens in the driving loop, so layer 2 neither changes
nor needs reverification.

## 4. What the SSOT computes

- slot sizes and offsets, region size per interface, generation counter
  placement
- the channel inventory: how many, frame size, direction, ring depth
- frame codecs, wire codecs, bridge transcoders
- the control-plane message set and subscription table shape
- the total memory budget

That last item earns its own check. rsdl §8.2's RSDL-801 already makes a timing
infeasibility a deploy-time error; memory infeasibility should be its sibling —
a deployment needing more store than the target declares fails the build.

## 5. Two encodings

**proto3 on the network. FlatBuffers in memory.** The rule that decides which:
serialized into a stream is proto3; mapped or passed within a node is
FlatBuffers.

| Path                               | Encoding    |
| ---------------------------------- | ----------- |
| shm store and queue                | FlatBuffers |
| local socket, Binder               | FlatBuffers |
| CAN, SOME/IP, DDS, WebSocket, wasm | proto3      |

proto3 is chosen for the network because it is compact and schema-evolvable at
once: a tagless positional encoding would be 30–50% smaller on small messages
but cannot survive version skew, and the guest updates independently of the
host. FlatBuffers is chosen for memory because Binder's single copy into a
mapped buffer and an mmap'd store both give the receiver a buffer it can read in
place.

The codec is generated, not delegated to a third-party library, so
`--emit rust --wire proto` produces code with no external crates. It is a
serializer for known types, not a protobuf library: no descriptors, no dynamic
messages, no schema parsing.

Interoperability is at the **bytes**, mediated by the emitted schema. Our codec
and someone's `protoc`-generated Java interoperate without sharing a library.
That places one obligation: byte-level conformance must be tested against a
`protoc`-generated implementation, or the emitted schema is a claim rather than
a guarantee.

### 5.1 Encoding and representation are different axes

proto3-in-Rust is prost's structs, rust-protobuf's, or quick-protobuf's — same
encoding, different types. Encoding belongs to the wire; representation is a
library choice. Generating the codec ourselves means no representation is
imposed on the consumer, and the CLI needs only two axes.

## 6. The CLI

```text
ridlc build --emit rust                 domain types, no transport, no dependency
ridlc build --wire proto                the .proto schema, language-free
ridlc build --emit rust --wire proto    the codec joining the two
```

Both flags repeat and combine as a cartesian product. `--emit ir-json`,
`ir-txtpb` and `ir-binpb` stay where they are as a documented exception: the IR
is the compiler's own artifact and sits on neither axis.

Targets: Rust as the primary language with optional FFI wrappers, TypeScript for
tooling and the embedded web UI, Kotlin for a native Android app. C is dropped;
C++ and a JVM backend are out of scope. TypeScript and Kotlin receive generated
types and interfaces, with the codec being the Rust one — via wasm on the
TypeScript side, via JNI on Android.

An FFI wrapper uses opaque handles and accessors, never exposed layout. That is
what lets it carry strings, collections, optionals and sum types, all of which
defeated the header described in §2.5.

## 7. The frame and the control plane

One frame, several bindings:

```text
ordinal      varint   ridl §11 identity
kind         u8       publication | occurrence | call | reply
seq          varint   §3.1 envelope
timestamp    i64      §3.1 envelope, µs, PTP epoch
provenance   u8       init | live | invalid   (signals only)
correlation  varint   calls only
payload_len  varint
payload      bytes    in the encoding for this path
```

The header carries what proto3 and AIDL both lack — the envelope and provenance
— and the payload stays in an encoding that already exists.

The control plane is uniform across bindings:

```text
attach(service, instance)   → region handle + contract version
subscribe(ordinal)          → slot offset + notification, current value delivered
unsubscribe(ordinal)
read(ordinal)               → current value + provenance
call(ordinal, payload)      → correlation id
```

`subscribe` delivering the current value immediately is normative, not a
convenience: ridl §4.4 makes the channel never empty, and it is precisely what a
gRPC server-streaming binding cannot do.

| Binding       | Control         | Data              |
| ------------- | --------------- | ----------------- |
| in-process    | direct          | direct            |
| same node     | Unix socket     | shm store + queue |
| Android       | Binder / AIDL   | shm store + queue |
| browser, Deno | WebSocket       | framed proto3     |
| wasm guest    | host call       | framed proto3     |
| cross node    | the transport's | the transport's   |

AIDL is a control-plane binding, not a separate architecture. Its parcelable
carries the frame; `oneway` fits `command` exactly, since ridl §6.1 makes a
command fire-and-forget and §10.3 states there is no error channel back to a
publisher. Binder's per-transaction permission check and `linkToDeath` are what
it is genuinely for.

## 8. The store

One region per provided interface — which is the coherence unit (ADR-0015, ridl
§14.5), the natural mapping unit, and the permission unit.

Slot offsets derive from the ordinal, and tombstones hold their slots, so ridl
§11's append-only numbering is what makes layout stable under compatible change.
Slot sizes come from typl's bounds: because typl bounds every collection and
string, every payload has a computable maximum and every slot is fixed-size.

**Two counters, two questions.**

| Counter         | Scope           | Answers                              |
| --------------- | --------------- | ------------------------------------ |
| interface `gen` | interface table | is this snapshot consistent?         |
| slot `gen`      | one signal      | is this payload torn?                |
| envelope `seq`  | one channel     | did this change, and did I miss any? |

The writer brackets a step with the interface counter and each payload with its
slot counter:

```text
interface.gen++            // odd — step begins
  slot.gen++ ; write ; slot.gen++
interface.gen++            // even — step ends
```

A reader of one signal uses only that slot's counter and is immune to unrelated
publications. A reader needing a coherent set brackets with the interface
counter. Using only the interface counter couples every reader to every writer;
using only slot counters gives no coherence.

The envelope `seq` is not the slot counter: it is stamped by the sender at
origin, and a gateway forwarding a sample preserves it, which is what makes loss
detection and replay ordering work across hops.

`fixed` interactions get real slots (ADR-0016 decision 9), written once at
binding initialisation.

### 8.1 Why the interface is the coherence unit

Because its members are computed together. An rmdl step produces one consistent
set from one input snapshot, and the seqlock brackets exactly that step.

Stated at the ridl layer this is an obligation, not a reference to rmdl: an
interface's members are published as a consistent set and observed as one
sample. An rmdl provider satisfies it structurally; a driver or hand-written
component must satisfy it some other way. A single-signal publication is
trivially coherent, so the model degrades correctly where no step exists.

## 9. Trust, assurance, and access

### 9.1 Three dimensions, ridl levels, domain names in plugins

Safety integrity, cyber threat and privacy are each an ordered scale `0..N`
defined by ridl. The mapping to ASIL, CAL, DAL, SIL or a medical class belongs
to a domain plugin, following ADR-0012 decision 7's model of a domain extension
as a spelling table plus backends with no core semantics.

The core needs only the ordering and the comparison. It never needs to know a
level is spelled ASIL-D.

**They do not compare in the same direction.**

| Dimension        | Rule        | Constrains                       |
| ---------------- | ----------- | -------------------------------- |
| safety integrity | no write up | the write mode                   |
| cyber threat     | no write up | the write mode, and verification |
| privacy          | no read up  | whether a region may be mapped   |

Safety and cyber are integrity dimensions; privacy is confidentiality and runs
the other way. A single comparison rule would get privacy backwards.

### 9.2 Write mode derives from trust

| Publisher relationship        | Path                                      | Validation      | Verification   |
| ----------------------------- | ----------------------------------------- | --------------- | -------------- |
| owns the region, same process | direct write                              | by type         | none           |
| same zone, other process      | writable mapping of **its own interface** | by type         | none           |
| cross zone                    | pipe, owner drains                        | owner validates | owner verifies |

Consumers are read-only in every row. Read-only mapping makes ridl §4.2's
publish/subscribe direction an MMU property rather than a convention.

A writable mapping across a trust boundary bypasses validation entirely — a peer
writes raw bytes past every range and step check — and lets one writer corrupt
the generation counter and every other reader. Hence the pipe.

**Verification happens once, at the boundary, on the trusted side.** The owner
verifies the incoming buffer and validates the values before writing the store,
so no downstream consumer verifies anything. That is far better than every
reader verifying forever, and it is why the cross-zone hop is worth its cost.

Cross-zone publication needs two things a raw pipe does not give:
**coalescing**, because a signal is state and a FIFO backlog delivers samples
that are already superseded (latest-wins per ordinal at the publisher), and
**grouping**, because a step's outputs must be applied under one generation
increment.

### 9.3 Privacy is runtime and revocable

Safety and cyber levels are deployment facts. Privacy is not: consent is granted
and withdrawn, and guest mode, valet mode and driver identity all change it
while the system runs.

**A mapping is a capability, and capabilities are not revocable.** Once a
consumer maps a region there is no mechanism to take it back. Therefore:

```text
privacy level 0   → in the store region, mapped, zero-IPC reads
privacy level > 0 → not in the region; read via call, per-call grant check
```

Absent rather than present-but-stale, so a slot's absence leaks no last-known
value.

The cost lands correctly: location, identity, biometrics and occupancy are all
low rate and did not need the fast path; the 100 Hz vehicle dynamics that do are
privacy-0.

ridl already models this. §3.4's five sources of unavailability include
**policy** — "not permitted in this condition", a predicate over declared policy
state. Withdrawn consent makes an interaction unavailable, not absent. And
RIDL-505's consumer-evaluability rule means the grant state must be **declared,
consumer-visible state**, so a UI can disable a control before it is used rather
than fail on use.

## 10. Resolved questions

**Ring depth — derived, with override.**
`depth ≈ ceil((service_period + jitter) / rate_floor)`. The rate floor is ridl
§9's `min`; the service period comes from rsdl, which needs it for RSDL-801
anyway; jitter is a target property with one conservative default for RT
platforms and one for non-RT, calibrated against measurement later. An rsdl
override covers consumers whose pattern the rates do not capture, and an
underivable or infeasible depth is a deploy-time error.

Subscriber count comes from the wiring graph in static posture and a declared
bound in discovered posture (rsdl §8.1).

Signals need no depth at all — they are the store, and §4.3 permits coalescing.
Events are where depth costs memory, since an occurrence is not state. Commands
have §6.1's acknowledgment and retry, so p90 plus a small floor keeps retry
exceptional rather than routine. **The percentile derives from the interaction's
safety integrity level**: above a threshold, size for worst case and treat
overflow as a fault with a defined reaction rather than as telemetry.

**Bridge — domain path is the reference, streaming transcoders are generated.**
Decode to the validated domain type and re-encode is the correctness oracle.
Streaming transcoders are generated per encoding pair for links that need them,
avoiding heap allocation and owned types while still validating inline — range
and step checks are per-scalar and need no materialised object. An equivalence
test asserts identical bytes against the reference, from the same corpus.

**Layer 1 — sans-IO.** See §3.1.

**Runtime update — compatible and within the reserved span.** Ordinals are
append-only, so a compatible change appends and never moves an existing offset;
reserving a generous virtual span and committing pages on demand means growth
does not invalidate existing mappings. An update is admissible when `ridl-diff`
calls the change compatible and the resulting layout fits the reservation;
anything else is a redeployment. The reservation size is declared per region in
rsdl, and no-MMU targets fall back to re-attach.

**`ridl-rt` — a new V1 epic at the ridl layer.** The runtime core (store,
control plane, frame protocol, subscriptions, sans-IO loop, platform traits) is
not a behaviour concern and does not belong inside the rmdl epic. E5.12 stays
where it is, correctly describing the rmdl scheduler. Without this, V1 ships a
compiler whose output no runtime can consume.

## 11. Phasing

**Phase 1 — types, payloads, codec.** Constraints enforced in the type (private
field, `TryFrom`, `new_unchecked` for the decoder — the same operation as
ADR-0013 decision 4's width narrowing). Derives. Rust naming. The types induced
by interactions: tuple returns, inline `T | E` unions, request shapes. The
proto3 projection and the codec.

Phase 1 adds a corpus entry containing interactions to the `rustc` tests of §2.3
**first**, since it depends on no decision here and is what makes everything
else verifiable.

**Phase 2 — client and server.** A concrete client generic over a transport, and
a server trait the provider implements. `query`, `command` and `event` are
calls; `signal` and `fixed` are the store. Which is why phase 2 converges on
E9.11 rather than being a third answer: a stateful client half is a store, a
server-side ordinal router is a dispatcher.

Async touches two of five kinds. `signal`, `event` and `fixed` are not calls,
and `command` is fire-and-forget by §6.1 — so the shipped
`async fn set_gear(&self)` is a conformance question, listed below.

The two phases also answer ADR-0013 open item 1: phase 1 is the wire ceiling,
phase 2 restores the interaction face on the grounds of decision 1, that a
backend is classified by what its target can represent.

## 12. Open

1. **typl §17.11's width floor is now a prerequisite.** Widening `[0..255]` to
   `[0..300]` flips `uint8` to `uint16`, and loosening a string bound does the
   same — both shift every subsequent offset in a laid-out store. ADR-0013
   decision 6 already required this closed before a FlatBuffers backend ships.
2. **E3.1 plus the deferred label promotion.** `SIL_B`, `CAL_2` and `PRIVATE`
   are free-form tokens; ADR-0008 decision 3 deferred promoting `labels` to
   attributes. The assurance dimensions need structure — a declared dimension,
   scale and direction — plus a diff category, where both raising and lowering a
   level are breaking.
3. **rsdl has no protection-domain concept.** §7 places components on targets
   but nothing says two components share a memory protection domain, which is
   the partition boundary §9.2's write mode derives from.
4. **Whether the assurance zone is one attribute or two.** Safety and cyber
   collapse for the write-mode decision but partition the system differently —
   safety by criticality, security by exposure. A QM infotainment stack is the
   highest-value attack surface in the vehicle.
5. **Scoping.** Which deployment tiers, whether AAOS is a target, whether
   Classic AUTOSAR is. Several decisions above — `repr(C)` as a constrained-tier
   fallback, wasm viability, the FFI wrapper's value — resolve once these are
   answered.
6. **Byte-level conformance testing** against a `protoc`-generated
   implementation: packed repeated scalars, canonical field ordering,
   malformed-input robustness.
7. **Unknown fields on decode.** proto3 preserves them for proxy round-trip;
   carrying fields the contract does not describe is arguably wrong for an SSOT,
   but dropping them breaks the gateway case.
8. **Whose envelope an invalid sample carries** — the rejected publication's or
   the last good value's. ridl §4.5 suggests the former and states neither.
9. **`async fn` on a `command`.** The shipped consumer trait makes the
   application await an acknowledgment §6.1 says is not application-visible and
   §6.2 says application code stays uninvolved in.
10. **The interface-granularity rule is unstated in ridl §14.** Splitting an
    interface splits the computation; nothing says so.
11. **driftsys/ridl#237**, the union-arm transform collision, which idiomatic
    variant naming walks into.

## 13. References

- [ADR-0013](../decisions/ADR-0013-codegen-backend-scope.md) — backend
  classification, the emit ceiling, the identity table, widths per class,
  decision 6 on width flips, decision 7 on absence, open item 1
- [ADR-0014](../decisions/ADR-0014-ir-encodings.md) — the IR encodings, decision
  9 on canonicity and decision 14 on the binary round-trip limit
- [ADR-0015](../decisions/ADR-0015-qos-absorption-and-rpc-bounds.md) — the
  coherence rule and the RPC response bound
- [ADR-0016](../decisions/ADR-0016-schema-projection-and-the-name-transform.md)
  — the pinned transform, RIDL-149, decision 4's field exclusion, decision 6's
  projection properties, decision 8 on deployment facts, decision 9 on `fixed`
- [`2026-08-03-schema-projection-design.md`](2026-08-03-schema-projection-design.md)
  — the store and dispatcher shapes phase 2 converges on
- [ridl language reference](../specification/ridl-language-reference.md) — §3.1
  the envelope, §3.4 availability and RIDL-505, §4.2 direction, §4.3 coalescing,
  §4.4 and §4.5 last-value and provenance, §6.1 and §6.2 the command
  acknowledgment, §9 timing, §10.3 and §10.4 detection versus management, §11
  identity, §14.5 coherence
- [rsdl language reference](../specification/rsdl-language-reference.md) — §4
  wiring, §5 provides and requires, §7 targets and placement, §8 transport and
  posture, RSDL-801 and RSDL-803
- [`docs/ROADMAP.md`](../ROADMAP.md) — E3.1, E4.5, E5.10, E5.12, E7.1, E9.8,
  E9.9, E9.11
- `crates/ridl-backend-rust/src/interact.rs` — `vocabulary()` at line 81
- `crates/ridlc/tests/corpus.rs` — the two `rustc` compile tests
- driftsys/ridl#236 and driftsys/ridl#237
