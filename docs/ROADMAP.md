# RIDL Implementation Backlog — Epics & Stories

Companion to
[ADR-0004](decisions/ADR-0004-implementation-sequencing-and-stack.md)
(sequencing + stack) and [ADR-0005](decisions/ADR-0005-agent-enablement.md)
(agent enablement). Each **Epic is a milestone** with its own shippable value
and exit criteria; **Stories** are the work items under it. Sizing is rough (S ≈
days, M ≈ 1–2 weeks, L ≈ 3–6 weeks) and relative, not a schedule.

**This page is the forward plan.** What has already shipped — the delivery
narratives for Epics 0, 1, 2 and 9 — is in
[the landed record](archive/roadmap-landed-record.md). The scope below was set
by the re-scope of 2026-09-12, recorded in
[`2026-09-12-release-scope-and-plugin-system-design.md`](wip/2026-09-12-release-scope-and-plugin-system-design.md):
§1 is the scope, §3 the thirteen decisions, most with the alternative each
rejected, §4 the open items it carries.

The plan runs in two steps. **Step 1** finalizes rsdl, builds the runtime
library, clears the typl debt, and finalizes the Rust codegen with its three
payload codecs. **Step 2** adds the TypeScript framework and the codegen plugin
system. Kotlin, the first external plugin, follows step 2 because it depends on
the backend contract.

**Supersedes
[ADR-0004](decisions/ADR-0004-implementation-sequencing-and-stack.md)'s
sequencing and its release definitions.** ADR-0004 sequenced E5 (rmdl) before E6
(rsdl) and defined two releases, V1 the contract platform and V2 the executable
platform. This plan runs rsdl first and replaces the two releases with the two
steps above. ADR-0004 records both changes in its 2026-09-12 amendment, and
[ADR-0018](decisions/ADR-0018-runtime-core-and-generated-surface.md) decision
16's own V1 framing is amended there for the same reason.

## What this repository is, and is not

**The core is domain-agnostic.** No automotive, avionics or medical vocabulary
belongs in it. Assurance is three ordered scales — safety integrity, cyber
threat, privacy — as ridl levels `0..N`, and the mapping to ASIL, CAL, DAL or
SIL is a plugin's job
([ADR-0018](decisions/ADR-0018-runtime-core-and-generated-surface.md) decision
9). The core carries the ordering and the comparison, never the standard's name.

**A domain lives in its own repository.** `ridl-plugin-can`,
`ridl-plugin-someip`, `ridl-plugin-dds`, `ridl-automotive`, `ridl-avionics`,
`ridl-robotics` — each a separate crate with its own lifecycle.

Robotics is worth naming because it is the domain that most tests whether the
core is genuinely domain-agnostic. ROS 2 is DDS underneath, and DDS is one of
the two targets ADR-0013 says maps _cleanly_ onto ridl's interaction model — it
has a native primitive for continuous state with a retained current value, which
is what ridl §4.4 requires and what proto3 lacks. A robotics plugin would
therefore exercise the plugin protocol against a transport that fits better than
the one core ships, which is the more informative test.

**Domain specifics are requirement inputs, not backlog items.** They belong on
this page only as the enablers that make them possible. CAN is why the store
must carry scaled integers and why ADR-0013 decision 7's in-band sentinel
exists; SOME/IP's field-with-notifier is why ridl §4.4's last-value is normative
rather than a convenience; AUTOSAR E2E is why ridl §3.1's envelope carries a
counter. Each of those shaped the core and none of them is an epic here. The
test for whether something belongs on this page is not "does a domain need it"
but "would the core need it if no domain existed" — and if the answer is no,
what belongs here is the **extension point**, not the extension.

The domains in view are **automotive, avionics including drones, robotics, rail
and medical**. Read as a set they settle two things the core had asserted rather
than checked.

**The assurance scale is right to be `0..N` rather than any standard's.** The
five bring five differently shaped ladders — ASIL QM and A to D, CAL 1 to 4,
DO-178C DAL A to E, EN 50128 SIL 1 to 4, IEC 62304 classes A to C. Different
arities, different directions of severity, and in one case a three-point scale.
No single borrowed vocabulary serves them, which is exactly the case
[ADR-0018](decisions/ADR-0018-runtime-core-and-generated-surface.md) decision 9
makes. Rail is the useful confirmation: EN 50128 says "safety integrity level"
literally, so the core's name for that dimension is the general term rather than
a loan from one field.

**Two things they surface that the core does not yet answer.** ARINC 653's time
and space partitioning is a standardised form of the protection domain rsdl is
missing (ADR-0018 open item 1) — that concept should be designed against 653
rather than invented. And IEC 62304 grades a _software item_, where ridl's
labels sit on an interface, so the medical mapping has a granularity mismatch a
plugin cannot paper over on its own.

So ridl Appendix B's target list — SOME/IP, DDS, CAN/DBC, AIDL, JSON Schema — is
a backlog of plugins elsewhere, and **none of it can start until the plugin
protocol exists**. That is the sequencing consequence: E4.5 is step 2's subject,
out of the ecosystem tail where it sat behind a browser playground — and nothing
outside this workspace can be generated until it lands.

What stays in core: the payload encodings the re-scope fixes — **proto3** for
the network, **FlatBuffers** for memory, and **`repr(C)`** as the third, added
by the 2026-09-12 note §3.3. They are how the runtime talks to itself and to a
generic consumer, not a domain's choice. ADR-0018 decision 3's "two encodings
and no more" is amended in place by
[ADR-0020](decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
decisions 1 and 2, which carry the matrix all three encodings sit in.

## The platform ladder

Ordered. Each rung is a layer-1 implementation of the runtime library's ports
plus a driving loop, so a rung is a port rather than a variant.

| # | Platform                        | Role                   | Note                                                      |
| - | ------------------------------- | ---------------------- | --------------------------------------------------------- |
| 1 | Desktop (Linux, macOS, Windows) | tooling and simulation | the toolchain, the test plane, and the emulator           |
| 2 | Mobile — Android and web        | UI, bridges, demo      | the frame over a socket; not a production control surface |
| 3 | Embedded Android                | production UI, tooling | native or JVM over AIDL/Binder — Kotlin is a real target  |
| 4 | QNX 7.1                         | production             | the first serious real-time target                        |

Every rung above has an MMU and can map a shared store, so a FlatBuffers store
holds across all of them.

**No target in view requires a rung above these four.** An edge or IoT gateway
(ARM Linux, Yocto), FreeRTOS or SAFERTOS, and baremetal under WAMR were listed
on earlier revisions of this page; none has a confirmed requirement, nothing
here depends on one, and no story assumes one. They are the rungs where a store
without an MMU would need `repr(C)` layout and re-attach instead of demand-paged
growth — which is one reason `repr(C)` is now a payload encoding in its own
right rather than a contingency.

The roadmap-simplification note's S-17 narrowed the ladder to three rungs —
desktop, embedded Android, QNX 7.1. This page keeps four: mobile stays, because
this release ships for it.

## Tracker correspondence

**This document is the source of truth. The issue tracker mirrors it, one issue
per story, titled `E<epic>.<story> — <story text>`.** A story's issue carries
its `Done when` and size verbatim from the table here, plus whatever cross-story
dependency is worth stating on the issue itself. When the two disagree, this
document is right and the issue is stale.

That direction matters because the tracker has drifted from here, and the drift
was silent each time. A story issue is therefore never evidence about what the
design is — only about what work is outstanding.

**Reconciled 2026-09-12, with the re-scope.** 61 issues closed as not planned —
the rmdl implementation epic, the engine block, the tooling plane beyond the two
re-homed stories, the gateway, rxdl's ecosystem half and its domain spellings,
E3.4–E3.6, E4.1–E4.4, E10.9 and E10.11 (Epic 10's TypeScript vocabulary and its
parity story), E10.8 (closed in error; see Epic 10), nine parked
agent-enablement stories, and two rsdl forms the rewrite drops. Each carries a
comment naming the decision that parks it and the observation that reopens it.
One issue closed as completed (E9.9, delivered by #303). Milestone E5 closed;
milestone E7 stays open holding E7.1 alone. Two specification defects recorded
during the pass were filed rather than lost: driftsys/ridl#308 (the envelope
sequence number has no caller scope) and driftsys/ridl#309 (an invalid event
payload has no defined behaviour).

Two conventions worth keeping, both learned from the earlier reconciliation:

- **Closing a story issue never rewrites its body.** The GitHub update API
  replaces the body wholesale, so an explanation written onto a story destroys
  the description that is its historical record. Explanations go on the epic
  issue or in a comment.
- **Epic numbers are identifiers, not positions.** Stories keep their numbers
  when they are parked, re-homed or superseded. Renumbering would invalidate
  references across source comments, `ridl/ir/v2/ir.proto`, and the archived
  epic plans, which are verbatim historical records. This is the family's own
  evolution discipline — ordinals are identity, never reordered — applied one
  level up. **Read the sequence, not the numbering, for what comes next.**

---

# Step 1 — the system description and the Rust runtime

## Scope

**rsdl is finalized first.** The language is rewritten around the
topology-vocabulary note's nouns and lowered to the IR the way ridl is, and its
specification is being authored now. Everything after it in this step either
consumes the IR rsdl completes or runs beside it.

**Then the runtime library**, `ridl-rt` — identity, the envelope, provenance,
freshness and the sample, the payload wrapper, the interaction descriptors and
the ports. This is the library generated code links, not an engine: the store,
the seqlock, the sans-IO session, the platform traits and the scheduler are
outside this repository (§3.7) and reopened by rmdl.

**Then the typl debt**, then the Rust codegen finalized with its three payload
codecs, proto3, FlatBuffers and `repr(C)`. `repr(C)` is the third payload
encoding (§3.3); its layout rules are decided in a projection record of the
shape of ADR-0017 and ADR-0019, written when the backend is.

**Sequence.**

```text
E6 rsdl finalized and lowered to the IR
      → E11.0 ridl-rt → E11.1 frame spec → E11.9 transport and loopback
            → typl debt (E14 dispositions · E10 value objects)
                  → Rust codegen finalized → E11.7 FlatBuffers · E11.8 proto3 · E11.12 repr(C)
        ╰────────────── E3.1–E3.3 · E9.10 · E9.12 · E8 thread ──────────────╯
```

**Two prerequisites block work already scheduled**, and neither is an epic: typl
§17.11's deferred width floor, because widening a range flips the resolved
width; and E3.1 plus ADR-0008 decision 3's deferred `labels` promotion, which
block every derivation over assurance levels, since `SIL_B` and `CAL_2` are
free-form tokens today. The width floor is inside the typl debt above, as E14.1;
the second is not — E3.1 sits in the Epic 3 thread that runs beside this step,
and the grammar edit the `labels` promotion needs is E9.12.

## Epic 6 — rsdl, rewritten as a language

**Milestone:** a system is described in rsdl and lowered to the IR. **Value:**
the IR then carries what a runtime derives its node descriptor from, so a
runtime's descriptor is an emitter over the IR rather than an agreement
negotiated outside the toolchain. **Exit criteria:** a system compiles from
`.rsdl` and the IR carries the region map, the link set, the routing table, the
permission list, the surface set and the catalog hash.

**Rewritten, not extended.** The rewrite designs the facts and the syntax
together and the IR is the schema, so there is no separate deployment-descriptor
step and no TOML descriptor as the user-facing surface (§3.1). Story E11.6, the
hand-written deployment-facts schema, closed as superseded.

**The specification is being authored now**, and it decides the story breakdown.
The eleven existing E6 story rows were written against the previous grammar;
they are preserved verbatim under "Parked stories (unscheduled)" in
[the landed record](archive/roadmap-landed-record.md#parked-stories-unscheduled)
and are refiled in one pass when the rsdl specification lands, so that the
rewrite is not made to inherit a breakdown built for the grammar it replaces.
The one open question the rewrite confirms against the first system is rsdl's
noun set: the topology-vocabulary note's §7 keeps `component` and the
roadmap-simplification note's S-36 dropped it; the vocabulary note is later and
takes precedence.

## Epic 11 — the runtime library and the frame

**Milestone:** generated code has a library to be written against, and a
contract crosses a process boundary. **Value:** every codegen ecosystem has a
runtime library its generated code links — `prost`, `serde`, the `flatbuffers`
runtime — and without one, generated code either carries its own copy of the
envelope and the provenance rules or invents them per project. **Exit
criteria:** a hand-written program links `ridl-rt` and reads a sample with its
provenance, freshness and envelope, and the same contract reaches a second
process over the WebSocket transport. The generated-package form of the same
read is what the Rust codegen section later in this step demonstrates.

**The engine is not here.** The store, the seqlock, the sans-IO session, the
subscription table, the platform traits, the scheduler and the ring depth are
outside this repository (§3.7), parked as `ridl-engine` and reopened by rmdl.
This epic builds the library, the frame specification and the transport — not a
runtime that owns them. The first runtime is the consumer's.

**Three rows are redefined under their identifiers.** E11.1, E11.7 and E11.8
were written for the engine block — the control plane, the store and the queue —
which the re-scope moved out of this repository (§3.7). The re-scope redefines
them as the frame specification and two of the payload codecs, keeping the
identifiers; their issues are retitled to match.

| ID    | Story                                                                                                                       | Done when                                                                                                   | Size |
| ----- | --------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- | ---- |
| E11.0 | `ridl-rt` — identity, the envelope, `Provenance`/`Freshness`/`Sample`, `Payload<E>`, the interaction descriptors, the ports | a hand-written program links it and reads a sample with its provenance                                      | L    |
| E11.1 | The frame specification — a logical frame with one binding per encoding; ordinal, kind, envelope, provenance, correlation   | one document a second implementation could be written from                                                  | M    |
| E11.9 | `ridl-transport-ws` — the WebSocket transport crate, plus the in-process loopback runtime for tests                         | a contract reaches a second process over the transport, and the loopback runs the same tests with no socket | M    |

## Epic 14 — typl and ridl finalization

**Milestone:** typl and ridl stop being drafts. **Value:** a reference that says
"Draft" cannot be depended on from outside this repository, and an open question
with no disposition is an unbounded liability for every backend written against
it. **Exit criteria:** every §17 open question in both references is resolved or
deferred to a named version, and both references drop "Draft".

| ID    | Story                                                                                         | Done when                                                      | Size |
| ----- | --------------------------------------------------------------------------------------------- | -------------------------------------------------------------- | ---- |
| E14.1 | typl §17 disposition pass, including §17.11's width floor and the two new rows (§3.10, §3.11) | every open question is resolved or deferred to a named version | M    |
| E14.2 | ridl §17 disposition pass                                                                     | as above, and the QoS and bound terms agree with ADR-0015      | M    |
| E14.3 | Both references drop "Draft"; the rxdl reference gains its status line                        | neither the typl nor the ridl reference is marked Draft        | S    |

## Epic 10 — typl value objects

**Milestone:** a typl package generates Rust types that cannot hold an invalid
value. **Value:** closes a promise the shipped documentation already makes —
typl §1.1 says a pure typl package "generates data types, **validators**, and
documentation across every backend", while no language backend emits a
validator. **Exit criteria:** a constrained named scalar cannot be constructed
out of range in Rust, and `--emit rust` writes a crate that compiles.

**The TypeScript half moved to step 2.** E10.9 and E10.11 were written against
the previous backend shape and are closed; TypeScript's value layer is part of
the TypeScript work in step 2. E10.8 is a Rust story and stays in this epic; its
issue, driftsys/ridl#253, was closed in error during the re-scope pass and is
being reopened.

Design and plan of record:
[`typl-value-objects-design.md`](wip/typl-value-objects-design.md) and
[`typl-value-objects-plan.md`](wip/typl-value-objects-plan.md).

| ID     | Story                                                                             | Done when                                                         | Size |
| ------ | --------------------------------------------------------------------------------- | ----------------------------------------------------------------- | ---- |
| E10.1  | The shared vacuous-constraint classifier — one definition of "constrains nothing" | both backends agree on which types need a fallible constructor    | M    |
| E10.2  | The Rust `ConstraintError` vocabulary                                             | one error type carries every constraint failure                   | S    |
| E10.3  | Constrained named scalars — private inner, `new`, `TryFrom`                       | an out-of-range value is unconstructible                          | L    |
| E10.4  | Vacuous named scalars — infallible construction                                   | a type that constrains nothing takes no fallible path             | M    |
| E10.5  | `TryFrom<i64>` for enum and enum set                                              | an undefined discriminant is rejected                             | M    |
| E10.6  | Sound derives — no derive that could reconstruct an invalid value                 | no path bypasses the validating seam                              | M    |
| E10.7  | `--emit rust` writes a compiling crate                                            | the emitted crate builds standalone                               | M    |
| E10.8  | Pattern validation behind a `validate-pattern` feature                            | regex constraints check without forcing the dependency            | M    |
| E10.10 | Amend ADR-0013 and typl §5.7; verify the `ridl-diff` classification               | the decision is recorded and a constraint change classifies right | S    |

E10.1's `Done when` names both backends. The TypeScript backend arrives in step
2, so that criterion completes then; the Rust half completes here.

**Carried typl defects**, to be cleared in this step: driftsys/ridl#245 (a
name-based `reserved` in an enum body is accepted with no defined meaning),
driftsys/ridl#203 (a user package named `ridl.std` is silently shadowed and its
artifact overwritten), driftsys/ridl#244 (exact-duplicate fields report a
name-transform collision rather than a duplicate).

## Rust codegen, finalized

**Milestone:** the Rust backend emits a complete, compiling surface with all
three payload codecs. **Exit criteria:** a generated package links `ridl-rt`,
constructs and validates its types, and round-trips a payload through
FlatBuffers, through proto3 and through the `repr(C)` layout, with byte-level
conformance against a `protoc`-generated implementation for proto3.

**E11.12 is a new identifier**, the next free one in Epic 11, for the `repr(C)`
payload codec the 2026-09-12 note §3.3 adds. Its layout rules — the
fixed-capacity layout of a bounded string, optional and collection, the string
capacity and terminator, alignment and endianness — are its prerequisite, and
[ADR-0020](decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
decision 4 places them in a projection record of the shape of ADR-0017 and
ADR-0019, written when the backend is — not in that record and not in the
ADR-0018 amendments.

| ID     | Story                                                                                                                                                                                                                                                                                               | Done when                                                                                                                                                                                                                   | Size |
| ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---- |
| E11.7  | The FlatBuffers payload codec                                                                                                                                                                                                                                                                       | a payload round-trips through the library                                                                                                                                                                                   | L    |
| E11.8  | The proto3 payload codec plus byte-level conformance against a `protoc`-generated implementation                                                                                                                                                                                                    | our bytes parse there and its bytes parse here                                                                                                                                                                              | L    |
| E11.12 | The `repr(C)` payload codec — a `#[repr(C)]` layout struct per type in the C-representable subset, a C header emitted from the same IR, and the codec between the layout struct and the domain type; also removes `#[repr(C)]` from the generated domain structs, which ADR-0020 decision 3 retires | a payload round-trips through the layout struct, the emitted header compiles as C, and no generated domain struct carries `#[repr(C)]` (a scalar newtype keeps `#[repr(transparent)]`, which ADR-0020 decision 3 preserves) | L    |

**Known defects to clear with this work:** driftsys/ridl#243 (a struct field
name is emitted verbatim, so generated Rust draws `non_snake_case`),
driftsys/ridl#237 (union arm names collide under `camel_case`, emitting two
variants of one name), driftsys/ridl#302 (a union-arm retirement would shift
FlatBuffers wire discriminants silently).

**The Rust backend is ported onto the backend contract in step 2, not here.**
Finalizing the backend before the lowering step means the porting is a second
pass over code this step writes. That is the accepted cost of getting a working
Rust runtime before the extension seam.

## Epic 3 — ridl boundary model, core ([ADR-0012](decisions/ADR-0012-interaction-boundary-model.md))

**Milestone:** the attribute layer the boundary model needs is enforced rather
than described. **Exit criteria:** every attribute key resolves to one owner, a
colliding key fails the gate, and an unclassified change never reports
compatible.

**Core only, and trimmed.** E3.4 to E3.6 are parked; see the parked table.

| ID   | Story                                                                                                                    | Done when                                                                   | Size |
| ---- | ------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------- | ---- |
| E3.1 | Attribute registry — name · owner · form · allow-list · consumer · diff category; namespacing outside core (ADR-0012 d8) | every key resolves to one owner; a colliding key fails the gate             | M    |
| E3.2 | Fail-closed classification: unregistered key is a compile error; uncategorised key diffs as **breaking** (d9)            | a typo'd key errors; an unclassified change never reports compatible        | S    |
| E3.3 | Core IR: `family` and `shape` closed enums on the interaction node; invalid-combination rejection (d2, d6)               | families round-trip; `(command, intent, no shape)` is rejected structurally | M    |

## Epic 9 — wire SSOT, remaining

**Milestone:** the schema projections are complete and the general-form drift is
removed. The projections themselves landed; see the landed record.

| ID    | Story                                                                                                                                                                                                                                           | Done when                                                | Size |
| ----- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------- | ---- |
| E9.10 | The **schema hash over the IR**, not over the emitted schema                                                                                                                                                                                    | two targets of one IR agree on identity                  | M    |
| E9.12 | Drift the design surfaced: general-form R5's postfix order contradicts the shipped grammar (`@timing` is last, not before attributes); `InterfaceDef`/`ServiceDef` gain the `AttrBlock` the deferred `labels`/`deprecated` promotion also needs | R5 matches `family.ungram`; one grammar edit serves both | S    |

## Epic 8 — agent enablement ([ADR-0005](decisions/ADR-0005-agent-enablement.md))

**Milestone:** an agent authors valid typl and ridl and evolves it provably.
**Exit criteria:** skill and rules author valid typl/ridl; the MCP's
`ridl_check`/`ridl_explain`/`ridl_diff` and the IR-query tools back a
verify/evolve loop.

**Six stories here, not seventeen.** E8.4 and E8.8 onwards are parked, with one
exception: E8.15, the rsdl skill profile (driftsys/ridl#87), stays open and
waits for Epic 6's specification, which decides its content. What remains here
is the knowledge layer and the MCP over the compiler.

| ID   | Story                                                                                                                                                                                                                                     | Rides | Done when                                                                                                                                                                                      | Size |
| ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---- |
| E8.1 | Rules file — 10–20 always-on "never/always" constraints, distilled from doctrines + the _error_ diagnostics (no semicolons, named-typl payloads, errors-as-data, command≠query, no inheritance, no upward refs, append-only + `reserved`) | E1    | every rule cites a diagnostic code or doctrine; loads in Claude Code/Cursor/Cowork                                                                                                             | S    |
| E8.2 | Skill v0 (typl) — dense decision tables + worked examples for types/ranges/units/evolution, per `skill-ridl-authoring-outline.md`                                                                                                         | E1    | authors valid `.typl`; content traceable to the typl reference                                                                                                                                 | M    |
| E8.3 | Skill extended to the ridl `interact` core — 5-kind selection table, timing, errors-as-data / `T\|E`, common-mistakes table keyed to codes                                                                                                | E2    | covers ridl ref §3–§10; cruise-control example round-trips clean (`.rxdl` descriptive form once E3.5 lands)                                                                                    | M    |
| E8.5 | MCP server skeleton — thin binary over the shared salsa crates, sibling of the LSP, **no second parser**; stdio transport                                                                                                                 | E2    | server starts, advertises tools, shares the compiler crates                                                                                                                                    | M    |
| E8.6 | Verify/evolve tools — `ridl_check` (coded diagnostics + **fix-its verbatim**), `ridl_explain` (error index), `ridl_diff` (0/1/2 + breaking list)                                                                                          | E2    | the per-diagnostic structure is identical to the CLI's, because both go through one `ridl_core::diag::to_json`; the envelope and `span.path` differ by construction (amended — see note below) | M    |
| E8.7 | Grounding / IR-query tools — `ridl_describe_type`, `ridl_list_interactions`, `ridl_resolve`                                                                                                                                               | E2    | return real IR data; agent cites existing symbols, not hallucinated ones                                                                                                                       | M    |

**E8.6's acceptance criterion was amended during implementation (the `ridl-mcp`
v0 slice, 2026-09-13).** The original wording, "outputs are byte-identical to
CLI/LSP," is not reachable by this design, and not only because of the envelope:
`ridl_check` and `ridl check --format json` share one diagnostic serializer
(`ridl_core::diag::to_json`), so each diagnostic object is identical, but the
tool wraps its array in `{"diagnostics": [ … ]}` for MCP's structured-output
requirement, and `span.path` differs by construction — the tool's input is a
source string with no file, so it registers under the fixed synthetic name
`input.typl`/`input.ridl` rather than a real path. The LSP side of the original
wording was never a JSON comparison to begin with: `ridl-lsp` renders
diagnostics through `lsp_types::Diagnostic` (`crates/ridl-lsp/src/convert.rs`),
not `ridl_core::diag::to_json`, so there was no shared serializer for
"byte-identical to ... LSP" to hold through. The row above states the reachable
criterion; see
[`docs/wip/2026-09-13-ridl-mcp-v0-design.md`](wip/2026-09-13-ridl-mcp-v0-design.md),
"What the two faces share, and where they differ."

---

# Step 2 — TypeScript and the plugin system

**Goal:** a second language reaches the platform, and a third can be written
outside this workspace.

## The TypeScript framework

**Milestone:** a TypeScript program uses a contract as a first-class client.
**Exit criteria:** a Deno program constructs and validates a payload without a
TypeScript codec, and an emulator written in TypeScript stands in for a provider
without the consumer distinguishing it.

The TypeScript side is the same three layers as Rust: the runtime package (the
port interfaces spelled in TypeScript, plus the loader that instantiates a
package's wasm codec), the generated faces, and a separate transport package.
The codec itself is per package — the generated Rust compiled to `wasm32`
against `ridl-rt`, not a build of that library, which carries no per-type codec
([ADR-0020](decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
decision 7). The WebSocket transport is what the getting-started path uses and
what a remote server or an emulator links by default; nothing links it unless
asked (§3.5).

**Two stories keep their E12 identifiers**, because identifiers are identity —
they are re-homed here, not renumbered.

| ID    | Story                                                                                                                                                                       | Done when                                                                    | Size |
| ----- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- | ---- |
| E12.1 | The TypeScript surface — generated types and faces, the codec reached through the package's own generated Rust compiled to `wasm32` against `ridl-rt` (ADR-0020 decision 7) | a Deno program constructs and validates a payload without a TypeScript codec | L    |
| E12.4 | Emulator — a hand-written provider standing in for a component, driven by the contract                                                                                      | a consumer cannot distinguish the emulator from the real provider            | M    |

**The encoding at the codec-in-wasm boundary is FlatBuffers.** ADR-0018 decision
3 put proto3 on the wasm guest boundary because a guest updates independently of
its host, but a codec compiled to wasm and its TypeScript host are generated
together and have no version skew, which decision 3's own rule classifies as
within a node.
[ADR-0020](decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
decision 2 settles it that way, so a TypeScript consumer reads the buffer in
place through generated accessors and no materialized object crosses the wasm
boundary on a read.

## Epic 4 — the plugin protocol and the scaffolding

**Milestone:** a backend can be written outside this workspace. **Value:** every
domain extension, every wire beyond the three core encodings, and every language
beyond Rust and TypeScript reaches the platform through this seam — and until it
exists, each of them would enter this workspace instead. **Exit criteria:** an
out-of-tree executable generates from the IR through the documented contract,
and the in-tree TypeScript backend run through the process host produces
byte-identical output to the in-process path.

| ID    | Story                                                                                                                                              | Done when                                                                            | Size |
| ----- | -------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ | ---- |
| E4.5a | IR stability policy and the canonical encoding — driftsys/ridl#231 is its first item                                                               | the policy names a canonical encoding that round-trips every IR the front end admits | M    |
| E4.5b | The lowering step, the backend contract (`generate(CodegenRequest) → CodegenResponse`), and the process host; both in-tree backends ported onto it | `ridlc-gen-ts` through the process host is byte-identical to the in-process path     | L    |
| E4.6  | `ridl init`/`ridl new` scaffolding + `ridl vendor` (air-gap)                                                                                       | scaffolds a valid workspace; vendors deps                                            | S    |
| E4.7  | Governance CI: keyword-registry collision test, and the E3.1 attribute registry enforced in CI                                                     | colliding key across profiles fails CI                                               | S    |

**The lowering step is the reason the contract is worth having.** Each backend
re-derives the same semantics from the raw IR today — the name transforms, the
width derivation, the init resolution, the tombstone handling. A codegen model
between the IR and every backend, with names already transformed per ADR-0016's
pinned rules and widths, inits, descriptors and ordinals already resolved, makes
every backend mostly a printer. A plugin over the raw IR would re-implement all
of it a third time (§3.8, alternative (a)). Porting the two in-tree backends
onto the contract is part of E4.5b, and for Rust that is a second pass over what
step 1 finalized.

## Epic 7 — the `.rxdl` unrestricted profile, trimmed

**Milestone:** types, interfaces and wiring compile from one file. The row runs
after rsdl has landed, because wiring is rsdl's layer. It is narrower than the
original E7.1, which also admitted the model layer and lifted the domain
restriction; the model layer and the domain spellings wait for rmdl (§3.2), so
the row does not depend on the parked E3.4. Issue driftsys/ridl#68 is retitled
to match.

| ID   | Story                                                                                                                                                                      | Done when                                                                                            | Size |
| ---- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- | ---- |
| E7.1 | `.rxdl` unrestricted profile, narrowed: typl, ridl and rsdl declarations in one file — no model layer, no domain spellings; per-package tightening enforced in `ridl.toml` | a file mixing types, interfaces and wiring compiles; a package that tightens rejects what it forbids | M    |

---

# After step 2 — Kotlin, the first external plugin

Not parked, and not in either step above: it depends on the backend contract of
E4.5b. A Kotlin consumer needs Kotlin types with validation, deserialization,
and a Kotlin runtime library to read and write signals — the full shape of a
language: a hand-written runtime library plus a generated layer of value classes
with checked constructors, a codec, and the faces. The generated layer is a
`ridlc-gen-kotlin` executable over the backend contract, most likely written in
Kotlin by the people who maintain the Kotlin side.

It is the real test of the plugin protocol from outside, which the in-tree round
trip of E4.5b cannot be. No JNI binding is planned.

Until it lands, a language without a ridl backend reads payloads through the
emitted schema and its own generator, restricted to inline or trusted reads,
because such a reader has no verifier and no typl constraint checks (§3.13).

---

# Parked, and what reopens each

Every parked story keeps its row, its `Done when` and its size under "Parked
stories (unscheduled)" in
[the landed record](archive/roadmap-landed-record.md#parked-stories-unscheduled)
and in its closed issue. Each block has one line saying what reopens it, so that
reopening is an observation rather than an argument.

| Parked                                   | Reopened by                                                                           |
| ---------------------------------------- | ------------------------------------------------------------------------------------- |
| E3.4–E3.6                                | a contract at the person or world boundary that must diff                             |
| E4.1–E4.4                                | a second organisation adopting the language                                           |
| E5 (rmdl implementation)                 | its draft finalised and rsdl shipped, or a consumer whose behaviour must be generated |
| E7 (rxdl's ecosystem half and spellings) | rmdl — rxdl's backend and its domain spellings are reopened with it                   |
| `ridl-engine`                            | rmdl — execution is its subject                                                       |
| E12.2                                    | the runtime library — there is nothing to observe without it                          |
| E12.3                                    | E4.5b, plus a contract-generic consumer                                               |
| E12.5–E12.10                             | a test plane with an owner                                                            |
| E13 (the gateway)                        | a second consumer                                                                     |
| E8 parked stories                        | a regression the kept six did not catch                                               |
| The plugin protocol's wasm host          | the browser playground                                                                |

---

# Milestone summary

| Epic | Milestone                    | Step                                                                               |
| ---- | ---------------------------- | ---------------------------------------------------------------------------------- |
| E0   | walking skeleton             | landed — internal                                                                  |
| E1   | typl schema language         | landed — **v0.1 preview**                                                          |
| E2   | ridl contract boundary       | landed — v0.x                                                                      |
| E9   | wire SSOT                    | **step 1** — the hash over the IR and the R5 drift removed; the projections landed |
| E6   | rsdl, rewritten              | **step 1** — a system is described and lowered to the IR                           |
| E11  | the runtime library          | **step 1** — generated code links a library                                        |
| E14  | typl and ridl finalization   | **step 1** — the references stop being drafts                                      |
| E10  | typl value objects           | **step 1** — types that cannot hold an invalid value                               |
| none | Rust codegen finalized       | **step 1** — the three payload codecs, byte-conformant                             |
| E3   | boundary model, core         | **step 1** — the attribute layer enforced                                          |
| E8   | agent enablement             | **step 1** — threads alongside                                                     |
| E12  | the TypeScript framework     | **step 2** — a second language, and an emulator                                    |
| E4.5 | the plugin protocol          | **step 2** — the extension seam every domain reaches through                       |
| E7   | the `.rxdl` profile, trimmed | **step 2** — types, interfaces and wiring in one file                              |
| none | Kotlin, the first plugin     | after step 2                                                                       |

Rows are in step order, not numeric order — the numbering is identity, as the
tracker section above explains. Two rows have no epic: the Rust codegen section
is a section of this page holding three Epic 11 stories (E11.7, E11.8, E11.12),
and Kotlin is a plugin written outside this workspace with no story identifier
here.
