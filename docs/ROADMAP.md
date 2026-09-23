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
the backend contract. The plugin protocol, E4.5a and E4.5b, runs ahead of the
two remaining Rust payload codecs, E11.8 and E11.12, and ahead of the TypeScript
framework, E12; the sequence below states why.

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

**Filed 2026-09-15, with the lock and the catalog descriptor.** Two epics are
new, each with its own milestone. Epic 15's stories are driftsys/ridl#371 to
driftsys/ridl#376; Epic 16's are driftsys/ridl#377 to driftsys/ridl#382. No
identifier reuses a parked row — the highest used before them were E9.12, E11.12
and E14.3. E11.0's issue, driftsys/ridl#316, closed as completed when the story
landed.

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

**The lock is first.** Epic 15 records each interface's number in its package,
so an interface's identity stops depending on where its declaration sits. Three
scheduled pieces of work wait on it: the catalog hash of Epic 16, which covers
every number and its provisional flag; E6.17, which embeds that hash in each
region; and E14.2, because the lock retires three diagnostics and amends
ADR-0015, which the ridl open questions cite. Anything that writes a hash or a
golden file before the lock lands would be written twice.

**rsdl is finalized beside it.** The language is rewritten around the
topology-vocabulary note's nouns and lowered to the IR the way ridl is, and its
specification is the rsdl reference v0.2.0. Only E6.17 waits for the lock and
for the catalog descriptor; the rest of Epic 6 runs beside them.

**The runtime library ran beside both, and its story has landed.** `ridl-rt` —
identity, the envelope, provenance, freshness and the sample, the payload
wrapper, the interaction descriptors and the ports — needed nothing from rsdl
and nothing from the lock but the identity widths, so it did not wait for
either. This is the library generated code links, not an engine: the store, the
seqlock, the sans-IO session, the platform traits and the scheduler are outside
this repository (§3.7) and reopened by rmdl.

**Then the typl debt**, then the Rust codegen finalized with its three payload
codecs, proto3, FlatBuffers and `repr(C)`. `repr(C)` is the third payload
encoding (§3.3); its layout rules are decided in a projection record of the
shape of ADR-0017 and ADR-0019, written when the backend is.

**Sequence.**

```text
E15 the lock ─┬─→ E16 the catalog descriptor ─→ E6.17 the catalog hash per region
              └─→ E14.2 ridl §17 dispositions ─┐
                                               │
E14.1 typl §17 dispositions ───────────────────┼─→ E14.3 both references
E10 value objects, E10.10 last ────────────────┘     drop "Draft"

E14.1 · E10, the typl debt ─→ Rust codegen finalized
      ─→ E11.7 FlatBuffers ─→ E4.5a IR stability ─→ E4.5b plugin protocol
                                                 ─→ E11.8 proto3 · E11.12 repr(C)

E6 rsdl finalized and lowered to the IR — beside the lock; only E6.17 waits
E11.0 ridl-rt, landed ─┬─→ E11.1 frame spec ─→ E11.9 ridl-transport-ws
                       └─→ E11.15 ridl-loopback — no frame, no socket
E11.13 interaction face MVP — deliberately out of sequence, before E11.1 and E11.9
      ─→ E11.14 the face and a codec reach ridl build — after E11.15 and one codec
E3.1–E3.3 · E9.10 · E9.12 · E8 — a thread beside all of it
```

**The sequence changed on 2026-09-15.** It ran E6 before E11.0, and it had no
place for the lock or for the catalog descriptor. E11.0 needed only the identity
widths the lock's design fixes and nothing from the rsdl language, so it ran
beside both rather than after rsdl. The lanes plan records that as its P-3, with
the three decisions this page also carries: P-4, the lock's priority over the
rsdl lowering and the catalog descriptor; P-5, the two Rust naming defects that
move to Epic 10; and P-6, where E14.2 and E14.3 sit
([`2026-09-13-step1-lanes-plan.md`](wip/2026-09-13-step1-lanes-plan.md) §2).

**Two prerequisites block work already scheduled**, and neither is an epic: typl
§17.11's deferred width floor, because widening a range flips the resolved
width; and E3.1 plus ADR-0008 decision 3's deferred `labels` promotion, which
block every derivation over assurance levels, since `SIL_B` and `CAL_2` are
free-form tokens today. The width floor is inside the typl debt above, as E14.1;
the second is not — E3.1 sits in the Epic 3 thread that runs beside this step,
and the grammar edit the `labels` promotion needs is E9.12.

**The sequence changed again on 2026-09-22.** E4.5a and E4.5b, the plugin
protocol, now run ahead of E11.8, E11.12 and the TypeScript framework, E12,
rather than after them: the Kotlin plugin depends on E4.5a and E4.5b, so the
plugin seam precedes the remaining payload codecs and the TypeScript framework.
Epic 4 keeps its place under the step 2 heading and the milestone summary keeps
its step column: the steps are the release scope's, and this reorder did not
re-scope the release. See the [lane P driver](wip/2026-09-22-lane-p-driver.md)
decision D-P1.

## Epic 15 — interface identity and the lock

**Milestone:** an interface's number is recorded in its package, and identity
stops depending on where a declaration sits. **Value:** everything that names an
interface from outside the source — a catalog hash, a routing table, a diff
verdict, a published baseline — rests on that number, so while the number comes
from a declaration's position, each of those is a fact a later edit can move
without saying so. **Exit criteria:** a package carries an `interfaces.lock`,
`ridl diff` matches interfaces by number rather than by name, and
`ridl baseline` refuses to publish an interface whose number is not recorded.

Design and plan of record:
[`2026-09-13-lock-design.md`](archive/2026-09-13-lock-design.md) and
[`2026-09-15-lock-plan.md`](archive/2026-09-15-lock-plan.md). The design
satisfies D-7 of
[`2026-09-12-rsdl-rewrite-decisions.md`](wip/2026-09-12-rsdl-rewrite-decisions.md)
and driftsys/ridl#315 at the interface level.

**Six stories, twelve plan tasks, one pull request.** The plan runs its twelve
tasks on one branch, because they change one set of files in sequence — the IR,
then the loader, then the commands, then the diff. The rows below group those
tasks by the outcome each delivers, and they close together.

**The service slot model goes with it.** ADR-0015 numbered a service's
interfaces by their position in its list, which is what made the list
append-only and needed the three slot diagnostics. With a number on the
interface itself, the list becomes a set: RIDL-146, RIDL-147 and RIDL-148 are
retired and never reused, and `reserved` in a service's list goes with them.

| ID    | Story                                                                                                                                                                                                                                                                                            | Done when                                                                                                                                    | Size |
| ----- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------- | ---- |
| E15.1 | The IR carries identity — `Interface.number`, `Interface.provisional`, `Package.retired` — and `interfaces.lock` has a parser, a writer and its edits (plan Tasks 1, 2)                                                                                                                          | a lock file round-trips through the parser and the writer, and the IR carries a number and its provisional flag                              | M    |
| E15.2 | The loader reads the lock and the checker numbers every interface: a frozen number from its entry, a provisional one otherwise; RIDL-409 for a live entry with no declaration, RIDL-410 for a malformed file (plan Tasks 3, 4)                                                                   | a package with a lock file lowers with frozen numbers; an orphan entry draws RIDL-409, and a file with conflict markers draws RIDL-410       | M    |
| E15.3 | `ridl lock` — allocation, `--rename`, `--retire` — `ridl lock merge` as the git merge driver, and the `ridl check` desk check that runs while RIDL-409 stands and names the command that fixes it; the CLI reference and ADR-0010 rows for both (plan Tasks 5, 6, 7)                             | `ridl lock` allocates a number for a new interface, a merge of two branches keeps both sides' numbers, and the rename hint names one command | L    |
| E15.4 | The slot model retired: `ServiceInterfaceAdded` and `ServiceInterfaceRemoved` replace the slot categories, and RIDL-146 to RIDL-148 and `reserved` in a service's list are removed (plan Tasks 8, 9)                                                                                             | a service's interface list is a set, reordering it is no change, and the three codes are gone from the catalogue and never reused            | M    |
| E15.5 | `ridl diff` matches interfaces by number, and `ridl baseline` refuses a provisional number (RIDL-411) or a published number that is absent from the fresh side and not retired (RIDL-412) (plan Tasks 10, 11)                                                                                    | a renamed interface that keeps its number diffs as `InterfaceRenamed` and exits 0, and a package with a provisional number cannot publish    | M    |
| E15.6 | The records: the ADR-0015 and ADR-0016 amendments, the decisions index, ridl reference §16.4 and Appendices B and C, one line of the family overview's open-question index, one sentence of rsdl reference §13, and the topology-vocabulary and family general-form working notes (plan Task 12) | no record still describes a service slot, an implicit interface id, or numbering by position                                                 | S    |

## Epic 16 — the catalog descriptor

**Milestone:** an engine reads what a catalog contains without decoding the IR.
**Value:** a bus configurator, a broker that hosts catalogs it was not compiled
against, a gateway or an engine written in another language is not a codegen
backend and receives no generated code; without a descriptor it needs the same
facts — what is addressed, how it is numbered, and how large each payload can be
— as an agreement negotiated outside the toolchain. **Exit criteria:**
`ridlc build --emit catalog` writes a verified descriptor for every package that
declares an interface, `ridl describe` prints it, and each payload carries its
maximum encoded size for proto3 and for FlatBuffers.

Design and plan of record:
[`2026-09-13-runtime-descriptors-design.md`](wip/2026-09-13-runtime-descriptors-design.md)
and
[`2026-09-13-catalog-descriptor-plan.md`](wip/2026-09-13-catalog-descriptor-plan.md)
(driftsys/ridl#324). The plan's seven dispositions on the design's open items
are confirmed with the maintainer before it is executed.

**The system descriptor is not here.** The design defines two artifacts; the
per-deployment system descriptor embeds the catalog descriptors of its closure
and waits for the rsdl lowering, so it takes its own rows when Epic 6 has
landed.

**Epic 15 lands first**, so the IR already carries each interface's number and
its provisional flag. The plan's Task 3 numbers interfaces inside the descriptor
crate because it was written before the lock; that task is reconciled with the
IR's numbers when this epic is executed.

**The `repr(C)` column stays empty** until E11.12 defines the C-representable
layout. The encoding is present in the descriptor's enum from the start, and a
payload with no entry for it means the toolchain cannot size that payload for
that encoding.

| ID    | Story                                                                                                                                                                                        | Done when                                                                                                                        | Size |
| ----- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- | ---- |
| E16.1 | The `ridl-descriptor` crate: the hand-written `catalog.fbs`, the accessors generated by `cargo xtask descriptor-codegen` and committed with a drift test, and the verifier (plan Tasks 1, 2) | a catalog round-trips through the builder and the reader, and a buffer with a wrong identifier or version is rejected as a whole | L    |
| E16.2 | Interface numbering in the descriptor and the catalog hash over the reachable closure (plan Tasks 3, 4)                                                                                      | two packages with the same declarations and the same numbering hash alike, and changing one number changes the hash              | M    |
| E16.3 | The size context, the type leaves and the string byte capacity (plan Task 5)                                                                                                                 | every type leaf yields a byte bound or is reported as unsizable                                                                  | M    |
| E16.4 | The proto3 and FlatBuffers upper bound per payload (plan Tasks 6, 7)                                                                                                                         | each payload carries a maximum encoded size for both encodings                                                                   | M    |
| E16.5 | The lowering from the IR and `ridlc build --emit catalog` (plan Tasks 8, 9)                                                                                                                  | the corpus package writes a descriptor that verifies, and two runs write the same bytes                                          | M    |
| E16.6 | The JSON view, `ridl describe`, and the records: ADR-0010's exit-code row and the CLI reference entry (plan Tasks 10, 11, 12)                                                                | `ridl describe` prints a catalog's contents as JSON, and a rejected buffer exits 2 with its cause named                          | M    |

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

**The specification is the
[rsdl reference v0.2.0](specification/rsdl-language-reference.md)**, and it
decides the story breakdown below. The eleven earlier E6 story rows were written
against the previous grammar; they stay preserved verbatim under "Parked stories
(unscheduled)" in
[the landed record](archive/roadmap-landed-record.md#parked-stories-unscheduled),
their issues are closed as not planned, and the new stories take new
identifiers, from E6.12. The noun set is settled: `component` stays and
`process` goes (rsdl §1.3, §1.4).

**The keyword registry changed with the checker.** E6.12 made the change rsdl §2
states, in the family registry (typl §1.4) and in the implemented registry
together: each retired v0.1 rsdl word left the registry, because no other
profile uses it (`let` stays, rmdl's), and is a legal identifier in every
profile again; `offers`, `distribution` and `machine` are reserved in both
registries and are no longer legal identifiers in any profile.

**The catalog hash is received, not computed.** The lowering embeds each
catalog's hash (rsdl §13); Epic 16 computes it, after Epic 15 has given every
interface its number. E6.16 lowers every other fact and E6.17 adds the hash once
both have landed.

| ID    | Story                                                                                                                                                                                                                                                                                                                      | Done when                                                                                                             | Size |
| ----- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- | ---- |
| E6.12 | The `.rsdl` profile: the file recognised wherever a tool selects a profile by extension; the keyword registry change; the parser for the five declarations, member lines, `offers`/`requires` lines and the attribute block; the attribute keys (rsdl §2–§5, Appendix B; RSDL-604, RSDL-305, RSDL-313, RSDL-804, RSDL-908) | Appendix A and the §3 examples parse, and a type, interface or service declared in an `.rsdl` file is RSDL-604        | M    |
| E6.13 | Closure checks: `system`, component lines, instances and the unit instance, the implicit component, declaration names, resolution (rsdl §3 introduction, §3.1, §3.2, §6–§8; RSDL-306 to RSDL-312, RSDL-403, RSDL-408, RSDL-409, RSDL-502, RSDL-504, RSDL-601 to RSDL-603, TYPL-009)                                        | Appendix A checks with no error, and each code of the group has a fixture that draws it                               | L    |
| E6.14 | Deployment and distribution checks: placement, external machines, distribution membership and tier (rsdl §3.3–§3.5, §9; RSDL-701, RSDL-702, RSDL-704 to RSDL-708, RSDL-901, RSDL-903 to RSDL-907)                                                                                                                          | each code of the group has a fixture that draws it, and Appendix A's deployments check with no error                  | M    |
| E6.15 | Editor support for `.rsdl`: the language server's diagnostics, hover and go-to-definition on rsdl references; the VS Code language and grammar; the MCP server's profile                                                                                                                                                   | an `.rsdl` file opened in the editor shows the checker's diagnostics, and a `requires` line resolves to its interface | M    |
| E6.16 | The lowering: the closure and per-deployment facts in the IR — producers, machines and placement, the link set with crossing kinds, the routing table, the permission list, the surface set, the region map, the attribute maps, distribution installation and dependency (rsdl §10, §11, §13)                             | Appendix A's system compiles from `.rsdl`, and its IR carries every fact of rsdl §13 except the catalog hash          | L    |
| E6.17 | The catalog hash in each region of the lowered system (rsdl §13); needs Epic 15's numbers and Epic 16's hash                                                                                                                                                                                                               | each region carries the hash the catalog descriptor writes for its catalog                                            | S    |
| E6.18 | `ridl diff` at the system: the closure's contracts compared by the ridl categories, with the "placement changed" and "composition changed" headings (rsdl §14)                                                                                                                                                             | moving an instance to another machine is listed under "placement changed" with no verdict                             | M    |
| E6.19 | The rsdl book chapter, written as built                                                                                                                                                                                                                                                                                    | the chapter's examples compile in the book-example harness                                                            | S    |

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

**E11.0 landed on 2026-09-14** (driftsys/ridl#348 built the crate,
driftsys/ridl#352 prepared its release; the story issue, driftsys/ridl#316, is
closed). Its row moved to
[the landed record](archive/roadmap-landed-record.md#epic-11--the-runtime-library-the-landed-part).
The crates.io release of 0.1.0 is a maintainer act and is not a story here. The
first half of the exit criteria above is therefore met; the transport half is
E11.9.

**E11.9 was split on 2026-09-20.** It held two deliverables, and only one of
them needs the frame specification: `ridl-transport-ws` binds E11.1's frame onto
a socket, while the in-process loopback runtime speaks no frame and opens no
socket. The loopback is what E11.13's face needs to stop running against a
test-only double, and what a package emitted by E11.14 is exercised over, so
holding it behind E11.1 would hold both. It is now **E11.15**
(driftsys/ridl#445), with the handle clause of E11.9's `Done when` moved to it
unchanged; E11.9 keeps the transport. The two stories stay independent of each
other: the loopback is not a degenerate transport, and the transport does not
link it.

| ID     | Story                                                                                                                     | Done when                                                                                                                                                                                                                                                                                                                                      | Size |
| ------ | ------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---- |
| E11.1  | The frame specification — a logical frame with one binding per encoding; ordinal, kind, envelope, provenance, correlation | one document a second implementation could be written from                                                                                                                                                                                                                                                                                     | M    |
| E11.9  | `ridl-transport-ws` — the WebSocket transport crate                                                                       | a contract reaches a second process over the transport, and E11.15's loopback runs the same tests with no socket                                                                                                                                                                                                                               | M    |
| E11.15 | `ridl-loopback` — the in-process reference runtime: every port over a queue and a map, no IO                              | the loopback exposes one handle per port role with a `Sync` reader handle, plus the aggregate handle the generated face is built over, which ADR-0021 decision 12 permits a runtime to offer and this story requires; the interaction-face round trips run over the crate, and `crates/ridl-backend-rust/tests/support/loopback.rs` is deleted | M    |

**E11.1 landed** (driftsys/ridl#257). The frame specification is
[`docs/specification/frame-specification.md`](specification/frame-specification.md):
the logical frame, binding-agnostic — what crosses a boundary per interaction
kind, the ordinal, the kind, the envelope, the provenance, the correlation and
the payload, in `ridl-rt`'s own vocabulary — the control plane, the
invalid-payload behaviour, the rule that a binding is written from the document
alone, and the two bindings by name: E11.9's WebSocket transport and the Kotlin
backend's AIDL over Binder. No binding is written yet, and nothing in this
workspace speaks the frame; `ridl-loopback` runs in process.

**E11.15 landed** (driftsys/ridl#445). `crates/ridl-loopback` is the first
runtime in this workspace: six handles, one per port role, with a `Send + Sync`
reader handle carrying the two signal extensions, and an aggregate handle
implementing all eleven port traits by delegation. Every interaction-face round
trip builds its face over it, and the test-only double it replaced is deleted.
Its as-built record is
[the `ridl-loopback` design record](design/ridl-loopback.md). It is not a
transport: E11.9 still owns that, and it does not link this crate.

**E11.13 landed in driftsys/ridl#418.** It is the MVP of the generated
interaction face, taken deliberately out of sequence: ADR-0018 decision 15
places the face after E11.1 and E11.9, and this story ran before both so the
team has a face to write against. It is in-process only, and it carries four
explicit placeholders that later stories retire — a hand-written payload
implementation (retired by E11.7's D-11, which moved the face onto the generated
FlatBuffers codec), test-only ports (retired by E11.15), a zero catalog hash and
all-absent encoded sizes (E16.2), and a narrow contract-clause translator
(E5.1). The as-built record is
[the interaction-face design record](design/interaction-face.md) and
[ADR-0023](decisions/ADR-0023-interaction-face-generation.md); its reasoning
trail is archived at
[`2026-09-16-interaction-face-v0-design.md`](archive/2026-09-16-interaction-face-v0-design.md).

| ID     | Story                                                                                                                                                                                                                                   | Done when                                                                                                        | Size |
| ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- | ---- |
| E11.13 | The generated interaction face, MVP — interface and interaction descriptors, a `Client` generic over exactly the ports it needs, a `Publisher`, a `Provider` trait and a generated `dispatch`, for one example package, in process only | an example package's generated face compiles, and a signal, an event, a command and a query round-trip in a test | M    |

**What E11.13 did not do is reach the command line.** `ridl build --emit rust`
calls `ridl_backend_rust::generate`, and the face is emitted by the companion
`generate_face` ([ADR-0023](decisions/ADR-0023-interaction-face-generation.md)
decision 2), so a package built through the CLI carried its domain types and
nothing else — no descriptors, no face, no codec, and a `Cargo.toml` that named
`ridl-rt` with no encoding feature. (That was the state at E11.13; the codec
arrived with E11.7 and the descriptors and the face with E11.14, both below.)
The interaction-face record states that as correct for E11.13 rather than a
shortfall, because there was no codec to emit and no runtime to link. Both of
those change in this step, and closing the gap is its own story, **E11.14**
(driftsys/ridl#444). It is the story that makes the generated surface reachable
by a consumer who runs the compiler rather than a test in this workspace, and
the first of the three codecs to land is what unblocks it.

| ID     | Story                                                                                                                                            | Done when                                                                                                                                                                               | Size |
| ------ | ------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---- |
| E11.14 | The face reaches `ridl build` — `--emit rust` writes the descriptors and the face beside the domain types and the codec `generate` already emits | a package built by `ridl build --emit rust` writes a crate that compiles against `ridl-rt`, round-trips a payload through its generated codec, and needs no hand-written implementation | M    |

**E11.7 moved two of that row's clauses out of it, 2026-09-21.** The row above
said `--emit rust` had still to learn to write "a codec's `Payload`
implementations" and a `Cargo.toml` naming the encoding's `ridl-rt` feature.
Both are already true. E11.7's design note D-1, as its disposition amended it,
puts the `Payload<FlatBuffers>` implementations in `generate`'s own output
rather than behind a third entry point, and `ridlc::run_build` calls `generate`
— so `ridl build --emit rust` has carried the codec since stage K5
(driftsys/ridl#465), and `crates/ridlc/src/lib.rs` has rendered
`ridl-rt = { version = "0.1", features = ["flatbuffers"] }` since the same
stage. What remained to E11.14 was the descriptors and the face, which
`generate_face` emitted and which `run_build` did not call — and which it now
calls `generate_pipeline` for.

**E11.14 landed** (driftsys/ridl#479). `ridlc::run_build` calls
`generate_pipeline`, so `ridl build --emit rust` writes the descriptors and the
interaction face beside the domain types and the codec. The proof is
`crates/ridlc/tests/cabin_example.rs`, which builds `examples/cabin/`, compiles
the emitted crate and `examples/cabin/consumer/src/main.rs` against it with
plain `rustc`, runs the program, and requires one round trip of every
interaction kind that carries a payload — a signal, an event, a command and a
query — through the generated `Client`, `Publisher`, `Provider` and `dispatch`
over `ridl-loopback`. What is new in it is the path rather than the running:
other proofs run generated code, and the interaction-face round trips already
run these same four over `ridl-loopback`, but each runs the backend's own output
or a checked-in fixture. This one runs what the CLI wrote, linked as a separate
crate into a separate process. The limit that stood beside the one below closed
with it: the story resolves a cross-package reference, so no type is withheld a
codec for that reason any more. Its as-built record is the E11.14 section of
[`interaction-face.md`](design/interaction-face.md).

**One limit on what that emitted codec covers**, not E11.14's to close. An
interaction whose payload is a named scalar or an enum used to carry no
`Payload` implementation, because a FlatBuffers root is a table and the
projection minted one only for a `struct` or a `union`; that was
**driftsys/ridl#470**, and it blocked E11.7's own D-11, the generated face
moving off `ReprC`. ADR-0019 decision 8 closed it on 2026-09-21 — every
declaration has a root table, and a named scalar, an enum and an enum set are
rooted in a box — and D-11 landed over it in stage K9b. E11.14's `Done when` is
written over a payload that has a codec.

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

**Sequence.** E14.2 follows Epic 15: the lock retires RIDL-146 to RIDL-148 and
amends ADR-0015, and every ridl open question that cites ADR-0015 can change
with it. E14.3 follows three things, because each of them still edits a
reference: E14.1, E10.10, and E14.2. The lane that merges the last of the three
opens it.

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

**Two Rust naming defects moved here on 2026-09-15** from the Rust codegen
section: driftsys/ridl#243 (a struct field name is emitted verbatim, so
generated Rust draws `non_snake_case`) and driftsys/ridl#237 (union arm names
collide under `camel_case`, emitting two variants of one name). E10.6 changes
the struct and union declarations both defects are about, and E10.3 changes the
named-scalar emission in the same emitter, so clearing them here changes the
snapshots once instead of twice. The value-objects plan gains a task for the
pair. driftsys/ridl#302 stays in the Rust codegen section: it is a wire
discriminant question, not a naming one.

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

**E11.7's `Done when` is met, and every decision of its design note is built.**
A payload round-trips through the library, and the conformance obligation the
story's design note took beyond that row — a round trip through an independent
FlatBuffers implementation rather than byte equality against one, since
FlatBuffers fixes no canonical encoding — is discharged against `planus` in
`crates/ridl-backend-rust/tests/flatbuffers_conformance.rs`. The last decision
to land was **D-11**, the generated interaction face moving off the `ReprC`
placeholder and onto the codec, which needed a `Payload<FlatBuffers>`
implementation for a named scalar and an enum payload that the projection minted
no root table for. That projection decision is **driftsys/ridl#470**, ADR-0019
decision 8, 2026-09-21; D-11 followed it in stage K9b, and the face now names
one per-package `Wire` alias and runs its round trips over the generated codec
and `ridl-loopback`. Two narrower gaps are tracked beside it:
**driftsys/ridl#467**, a type reaching a cross-package reference carried no
codec — closed by E11.14 on 2026-09-21 — and **driftsys/ridl#469**, an anonymous
inline constraint, a `step` and a map key's uniqueness are not checked by the
generated `verify`. A fourth, **driftsys/ridl#472**, is a decided divergence
rather than a gap: this codec refuses a buffer in which a conforming FlatBuffers
writer omitted a default-valued non-optional field, which design note D-9 chose
and the conformance suite measures.

**Known defects to clear with this work:** driftsys/ridl#302 (a union-arm
retirement would shift FlatBuffers wire discriminants silently).
driftsys/ridl#243 and driftsys/ridl#237 were listed here until 2026-09-15 and
are now Epic 10's, with the reason in that epic.

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
(`ridl_core::diag::to_json`), so each diagnostic object has the same shape, but
the two run different front ends (the CLI resolves a workspace, the tool checks
one standalone source), and the tool wraps its array in `{"diagnostics": [ … ]}`
for MCP's structured-output requirement, and `span.path` differs by construction
— the tool's input is a source string with no file, so it registers under the
fixed synthetic name `input.typl`/`input.ridl` rather than a real path. The LSP
side of the original wording was never a JSON comparison to begin with:
`ridl-lsp` renders diagnostics through `lsp_types::Diagnostic`
(`crates/ridl-lsp/src/convert.rs`), not `ridl_core::diag::to_json`, so there was
no shared serializer for "byte-identical to ... LSP" to hold through. The row
above states the reachable criterion; see `crates/ridl-mcp/README.md`, "What
this tool shares with `ridl check --format json <file>`, and where it differs."

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
and the in-tree Rust backend run through the process host produces
byte-identical output to the in-process path.

| ID    | Story                                                                                                                                         | Done when                                                                                           | Size |
| ----- | --------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- | ---- |
| E4.5a | IR stability policy and the canonical encoding — driftsys/ridl#231 is its first item                                                          | the policy names a canonical encoding that round-trips every IR the front end admits                | M    |
| E4.5b | The lowering step, the backend contract (`generate(CodegenRequest) → CodegenResponse`), and the process host; the Rust backend ported onto it | **landed** — the Rust backend run through the process host is byte-identical to the in-process path | L    |
| E4.6  | `ridl init`/`ridl new` scaffolding + `ridl vendor` (air-gap)                                                                                  | scaffolds a valid workspace; vendors deps                                                           | S    |
| E4.7  | Governance CI: keyword-registry collision test, and the E3.1 attribute registry enforced in CI                                                | colliding key across profiles fails CI                                                              | S    |

**E4.5a landed** (driftsys/ridl#321). The policy is
[`docs/specification/ir-specification.md`](specification/ir-specification.md):
the canonical encoding and what canonical fixes, the nesting bound in the front
end's units and the encoding's, the compatibility rule, and the versioning rule.
[ADR-0014](decisions/ADR-0014-ir-encodings.md) decision 9 is amended in place to
name canonical protobuf JSON as the canonical encoding, with binary and
prototext derived, because the encoding it had named canonical cannot read back
every package the front end admits and the one it had named derived can
(driftsys/ridl#231). No artifact and no golden moved: only the label moved.

**The lowering step is the reason the contract is worth having.** Each backend
re-derives the same semantics from the raw IR today — the name transforms, the
width derivation, the init resolution, the tombstone handling. A codegen model
between the IR and every backend, with names already transformed per ADR-0016's
pinned rules and widths, inits, descriptors and ordinals already resolved, makes
every backend mostly a printer. A plugin over the raw IR would re-implement all
of it a third time (§3.8, alternative (a)). Porting the Rust backend onto the
contract is part of E4.5b, and it is a second pass over what step 1 finalized;
the TypeScript, proto3 and FlatBuffers backends follow in their own stories (the
[lane P driver](wip/2026-09-22-lane-p-driver.md) decision D-P1, corrected on
2026-09-22 from "both in-tree backends").

**E4.5b landed (2026-09-22).** The lowering step
(`ridl build --emit codegen-model`, `ridl.codegen.v1`), the contract
(`CodegenRequest` and `CodegenResponse` in the same package, with `schema` and
`toolchain` leading per the IR specification §7), the in-process host — every
in-tree backend behind one trait, `ridl_ir::codegen::Backend` — the process host
behind `--plugin <language>[=<path>]` with `ridlc-gen-<language>` on `PATH`, and
the reference plugin `ridlc-gen-model`, whose parity test runs under
`just test`. Then the Rust backend itself, ported onto the lowered model in
three layers — the domain types, the FlatBuffers codec, the descriptors and the
face — each byte-identical against the snapshots that pin its output, and run as
the reference plugin `ridlc-gen-rust`: the row's `Done when` is
`crates/ridlc-gen-rust/tests/parity.rs`, over every corpus package at the
contract's level and every corpus entry at the command's level. The as-built
record is [`docs/design/codegen-plugins.md`](design/codegen-plugins.md). The
TypeScript, proto3 and FlatBuffers backends still read the raw IR and follow in
their own stories.

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

Not parked, and not in either step above: it depends on E4.5a and E4.5b, the
plugin protocol, and on E11.1, the logical frame specification — not on the
TypeScript framework and not on E11.9, the WebSocket transport. A Kotlin
consumer needs Kotlin types with validation, deserialization, and a Kotlin
runtime library to read and write signals — the full shape of a language: a
hand-written runtime library plus a generated layer of value classes with
checked constructors, a codec, and the faces. The generated layer is a
`ridlc-gen-kotlin` executable over the backend contract, most likely written in
Kotlin by the people who maintain the Kotlin side.

**The Kotlin backend owns its IPC binding.** It is AIDL over Binder on Android,
generated by the Kotlin backend from the same lowered model as its types and
faces, not a binding this repository writes. E11.9's WebSocket transport is not
on Kotlin's path.

It is the real test of the plugin protocol from outside, which the in-tree round
trip of E4.5b cannot be. No JNI binding is planned.

Until it lands, a language without a ridl backend reads payloads through the
emitted schema and its own generator, restricted to inline or trusted reads,
because such a reader has no verifier and no typl constraint checks (§3.13).

**O-P3 is out of scope for this repository.** A Rust Binder runtime
(`ridl-transport-binder`, Android only) belongs to a later repository and is not
needed for the first Kotlin demo.

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

| Epic | Milestone                       | Step                                                                               |
| ---- | ------------------------------- | ---------------------------------------------------------------------------------- |
| E0   | walking skeleton                | landed — internal                                                                  |
| E1   | typl schema language            | landed — **v0.1 preview**                                                          |
| E2   | ridl contract boundary          | landed — v0.x                                                                      |
| E9   | wire SSOT                       | **step 1** — the hash over the IR and the R5 drift removed; the projections landed |
| E15  | interface identity and the lock | **step 1** — an interface's number is recorded in its package                      |
| E16  | the catalog descriptor          | **step 1** — an engine reads a catalog without decoding the IR                     |
| E6   | rsdl, rewritten                 | **step 1** — a system is described and lowered to the IR                           |
| E11  | the runtime library             | **step 1** — generated code links a library                                        |
| E14  | typl and ridl finalization      | **step 1** — the references stop being drafts                                      |
| E10  | typl value objects              | **step 1** — types that cannot hold an invalid value                               |
| none | Rust codegen finalized          | **step 1** — the three payload codecs, byte-conformant                             |
| E3   | boundary model, core            | **step 1** — the attribute layer enforced                                          |
| E8   | agent enablement                | **step 1** — threads alongside                                                     |
| E12  | the TypeScript framework        | **step 2** — a second language, and an emulator                                    |
| E4.5 | the plugin protocol             | **step 2** — the extension seam every domain reaches through                       |
| E7   | the `.rxdl` profile, trimmed    | **step 2** — types, interfaces and wiring in one file                              |
| none | Kotlin, the first plugin        | after step 2                                                                       |

Rows are in step order, not numeric order — the numbering is identity, as the
tracker section above explains. Two rows have no epic: the Rust codegen section
is a section of this page holding three Epic 11 stories (E11.7, E11.8, E11.12),
and Kotlin is a plugin written outside this workspace with no story identifier
here.
