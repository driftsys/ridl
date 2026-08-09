# RIDL Implementation Backlog — Epics & Stories

Companion to
[ADR-0004](decisions/ADR-0004-implementation-sequencing-and-stack.md)
(sequencing + stack) and [ADR-0005](decisions/ADR-0005-agent-enablement.md)
(agent enablement). Each **Epic is a milestone** with its own shippable value
and exit criteria; **Stories** are the work items under it. Sizing is rough (S ≈
days, M ≈ 1–2 weeks, L ≈ 3–6 weeks) and relative, not a schedule.

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
protocol exists**. That is the sequencing consequence: E4.5 moves onto the
critical path, out of the ecosystem tail where it sat behind a browser
playground.

What stays in core: the two encodings ADR-0018 decision 3 fixes — proto3 for the
network and FlatBuffers for memory — because they are how the runtime talks to
itself and to a generic consumer, not a domain's choice.

## The platform ladder

Ordered. Each rung is a layer-1 implementation of four traits plus a driving
loop, so a rung is a port rather than a variant.

| # | Platform                                | Role                   | Note                                                              |
| - | --------------------------------------- | ---------------------- | ----------------------------------------------------------------- |
| 1 | Desktop (Linux, macOS, Windows)         | tooling and simulation | the toolchain, the test plane, and the emulation and twin servers |
| 2 | Mobile — Android and web                | UI, bridges, demo      | the frame over a socket; not a production control surface         |
| 3 | Embedded Android                        | production UI, tooling | native or JVM over AIDL/Binder — Kotlin is a real target          |
| 4 | QNX 7.1                                 | production             | the first serious real-time target                                |
| 5 | Edge and IoT gateway (ARM Linux, Yocto) | **speculative**        | no confirmed requirement; near-free to add if one appears         |
| 6 | FreeRTOS / SAFERTOS                     | production             | where `repr(C)` may return as a store fallback                    |
| 7 | Baremetal, WAMR                         | production             | the most interesting and the latest                               |

Rungs 1 to 5 have an MMU and can map a shared store, so ADR-0018 decision 8's
FlatBuffers store holds across all of them. Rungs 6 and 7 are where its
alternatives — `repr(C)` layout, re-attach instead of demand-paged growth — stop
being hypothetical.

**Rung 5 is recorded as speculative rather than planned.** No target in view
requires it today, and it is listed only so the ladder is honest about where
such a device would sit if one appeared. It is kept separate from baremetal
because the two cost very differently: an edge gateway on ARM Linux is the same
four traits and the same driving loop as rung 1 with a different target triple,
where baremetal under WAMR has no MMU, no filesystem, and needs the store's
alternatives rather than merely permitting them. Nothing on this page depends on
rung 5 and no story assumes it.

The release boundary is **descriptive vs executable**:

- **V1 — the contract platform:** E0–E4 plus E9, E10, E11 and E12 (typl · ridl ·
  the boundary model · wire projection · value objects · the runtime core · the
  tooling plane · ecosystem). The SSOT for contracts at every boundary — system,
  person, and world — with codegen, LSP, diff, docs, schemas a non-Rust runtime
  can consume, a runtime that consumes them, and the plugin protocol every
  domain extends through. **E11 is in V1 by ADR-0018 decision 16**: without it
  V1 ships a compiler whose output nothing can run, which is what made the E2
  interaction layer unimplementable.
- **V2 — the system platform:** E5a · E6 · E7 · E13 (rmdl as a _language_ · rsdl
  · rxdl · the gateway). The whole architecture becomes describable and
  checkable — behaviour, assembly, deployment, and domain vocabulary — with
  **nothing executing yet**. rmdl's expressions and equations reach the IR; no
  code is generated from them.
- **V3 — the executable platform:** E5b and E7's ecosystem tail. The compute
  runtime, codegen, oracle, replay, and deductive proof. The ambitious,
  higher-risk half, deferred until the architecture above it is settled.

**E8 — agent enablement ([ADR-0005](decisions/ADR-0005-agent-enablement.md)):**
threads V1→V3 alongside E1–E7 — the skill/rules, the MCP over the compiler,
evals, and (V3) the behaviour oracle and subagent.

**Sequence:**

```text
E0 → E1 → E2 → E10 → E4.5 → E11 → E12 → E9 → E3 → E5a → E6 → E13 → E7(rxdl)
                                                         → E5b → E7(ecosystem)
          ╰──────── E4 (rest of ecosystem), E8 (agents) thread ────────╯
```

**E4.5 is pulled out of Epic 4 and put on the critical path.** Four things now
depend on the plugin protocol: every domain extension, which lives in another
repository; every wire beyond the two core encodings; the gateway's descriptor;
and the contract-generic half of the tooling plane. It was scheduled behind the
browser playground, which nothing depends on.

**E10 → E4.5 → E11 → E12, ahead of E9's remainder.** E10 gives the types their
constraints and a compiling crate; E4.5 opens the extension seam; E11 builds the
runtime; E12 is the first consumer that is not the compiler itself. E9.8 landed,
and E9.11's store and dispatcher moved into E11 as ADR-0018 decision 16
requires, so what remains of E9 is the FlatBuffers schema projection, the schema
hash, and the recorded general-form drift.

**Two prerequisites block work already scheduled**, and neither is an epic: typl
§17.11's deferred width floor blocks E11.2, because widening a range flips the
resolved width and shifts every slot offset after it; and E3.1 plus ADR-0008
decision 3's deferred `labels` promotion block every derivation over assurance
levels, since `SIL_B` and `CAL_2` are free-form tokens today.

**E9 before E3** — both alter ridl's surface and IR, so they must not run
concurrently, and E9 is the nearer-term product path: it is ridl used as the
SSOT for a real system bus. **E10 threads** — it is typl backend work with no
dependency on either, and it closes a promise the shipped documentation already
makes.

**Epic numbers are identifiers, not positions.** The sequence above changed
after [ADR-0012](decisions/ADR-0012-interaction-boundary-model.md) and the rmdl
phase split; the numbers did not. Renumbering would have invalidated ninety-six
references across seventeen files — including source comments,
`ridl/ir/v2/ir.proto`, and the archived epic plans, which are verbatim
historical records. This is the family's own evolution discipline (ordinals are
identity, never reordered) applied one level up. **Read the sequence line, not
the numbering, for what comes next.**

**Amends
[ADR-0004](decisions/ADR-0004-implementation-sequencing-and-stack.md)**, which
sequenced E5 (rmdl) before E6 (rsdl) and put both in a single V2. rsdl now runs
first, because composition, deployment, transport/posture derivation, and the
test plane are all reachable with rmdl as a language and no runtime — and rmdl's
runtime is the highest-risk work in the programme, so it goes last.

**Forward-compatibility constraint (V1 protects V2):** the `expr`/function core
shipped in V1 for `require`/`ensure` (E2.4) must be a genuine forward-compatible
_subset_ of the family `expr` core — the same grammar rmdl's function layer
extends in V2 (E5.1), never a throwaway. Hold this line and rmdl's function
layer is an extension, not a rewrite. E2.12 writes the expr-core specification
that fixes the family grammar this subset is verified against — it lands before
or with E2.4.

**What E5.1 and E7.3 inherit, and the hole in it (recorded at E2 close,
2026-07-26).** E2 carries a contract clause in the IR as canonical source text
(`Contract.source`, [ADR-0008](decisions/ADR-0008-e2-execution.md) decision 14).
E5.1 replaces that with an expression tree, and E7.3 discharges the same terms
deductively; both inherit `crates/ridlc/tests/corpus/` as the regression set
that says a restructured representation still means what the text meant. **That
set does not exercise the whole subset.** The subset grammar admits thirteen
binary operators and two prefix operators. Of the thirteen binary, four — `<`,
`-`, `/`, `%` — appear in no contract clause that reaches a snapshotted IR; of
the two prefix, `!` appears in none, and `-` appears only on numeric literals
(`-10.0`, `-40.0`), never on a reference. All five are implemented and
unit-tested in `crates/ridl-sem/src/expr.rs` and
`crates/ridl-sem/src/expr_eval.rs` — this is a coverage hole, not a correctness
one. `<` is the near miss: the diagnostic showcase writes it, but that package
compiles with errors, so its IR, Rust, and TypeScript snapshots are one-line
placeholders and nothing pins a lowered form. Widen the corpus before
restructuring, so the restructuring has something to regress against.

## Tracker correspondence

**This document is the source of truth. The issue tracker mirrors it, one issue
per story, titled `E<epic>.<story> — <story text>`.** A story's issue carries
its `Done when` and size verbatim from the table here, plus whatever cross-story
dependency is worth stating on the issue itself. When the two disagree, this
document is right and the issue is stale.

That direction matters because the tracker has drifted from here twice, and both
times the drift was silent. The stories were imported in one pass on 2026-07-18
and not maintained: E0 through E2 shipped without their issues being closed, and
ADR-0012 and the rsdl reference then changed what several open issues meant
while their titles stayed as written. A story issue is therefore never evidence
about what the design is — only about what work is outstanding.

**Reconciled 2026-08-09.** 44 issues closed (E0.1–E2.11 as delivered; the five
retired `uxdl` stories and the C header defect as not planned), 12 retitled
where a row changed meaning, and 56 opened for the stories that never had one —
E3, E6.2/E6.4/E6.6–E6.9, E7.6–E7.9, E9.9/E9.10/E9.12, E10, E11, E12 and E13. The
reasoning is on the two epic debt roll-ups, driftsys/ridl#135 and
driftsys/ridl#172, which stay open because they hold carried findings the
stories did not deliver.

Two conventions worth keeping, both learned from the reconciliation:

- **Closing a story issue never rewrites its body.** The GitHub update API
  replaces the body wholesale, so an explanation written onto a story destroys
  the description that is its historical record. Explanations go on the epic
  roll-up.
- **A retired story is closed as _not planned_, not as completed**, with the ADR
  that retired it named on the issue. A reader landing on a closed issue should
  not have to guess which of the two happened.

Milestones exist for E0 through E8 only; the epics added since have none.

---

# V1 — The Contract Platform

## Epic 0 — Walking Skeleton

**Milestone:** one trivial `.typl` file compiles end to end, snapshot-tested.
**Value:** the IR shape and the salsa query graph are proven under real
cross-layer flow while both are still throwaway-cheap to change. **Exit
criteria:** `type Speed` + one `const` → lex → parse → resolve → check → IR →
generated Rust, with a green `insta` snapshot; no feature depth anywhere.

| ID   | Story                                                                                                   | Done when                                                                                                                    | Size |
| ---- | ------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- | ---- |
| E0.1 | Cargo workspace scaffold per concept note §8.1 (`ridl-syntax`, `ridl-core`, `ridl-ir`, `ridlc`, `ridl`) | crates build, CI runs `cargo test`; family crate names reserved on crates.io (concept note §10 — done 2026-07-18, issue #92) | S    |
| E0.2 | Minimal `logos` lexer for `type`/`const`/idents/number literals + trivia                                | token stream incl. whitespace/comments                                                                                       | S    |
| E0.3 | Hand-written parser → `rowan` CST for the two declarations                                              | lossless tree round-trips to source                                                                                          | S    |
| E0.4 | Salsa spike: one memoized query (parse-of-file)                                                         | edit invalidates only the changed file                                                                                       | S    |
| E0.5 | Trivial resolver — single package, no imports                                                           | names resolve within one file                                                                                                | S    |
| E0.6 | IR v0 proto schema skeleton + `prost` build                                                             | `.proto` compiles, Rust IR types generated                                                                                   | M    |
| E0.7 | Minimal checker: AST → IR for `type`/`const`                                                            | IR emitted for the skeleton input                                                                                            | S    |
| E0.8 | Trivial Rust backend: IR → a Rust `struct`/`const`                                                      | emitted Rust compiles                                                                                                        | S    |
| E0.9 | `ridlc` wiring + `insta` golden test                                                                    | one command, one snapshot, green                                                                                             | S    |

## Epic 1 — typl + Tooling Spine

**Milestone:** typl ships as a standalone units-aware schema language with a
real editor experience (public v0.1 preview). **Value:** first external users
and feedback; the compiler-as-library spine every later layer reuses. **Exit
criteria:** arbitrary typl packages compile with full diagnostics, format with
`ridl fmt`, generate Rust+extern-C, and edit live in VS Code; IR stabilized at
v1 for the typl subset (frozen only with the E4.5 stability policy).

**Status:** landed — all nineteen stories (E1.1–E1.19) shipped as PRs #107–#133;
the typl v0.1 preview toolchain (compiler, `ridl fmt`, LSP, VS Code extension)
is complete over IR v1 (exact decimal). Deferred per
[ADR-0007](decisions/ADR-0007-e1-execution.md): E1.8 ships no `wire` width floor
yet (typl §17.11 / [ADR-0007](decisions/ADR-0007-e1-execution.md) d7) — nominal
unit checking itself ships; of the profile-boundary and doc diagnostics only
TYPL-302 ships — TYPL-301/303/304 and TYPL-107/205/401/402/403 are recorded debt
([ADR-0007](decisions/ADR-0007-e1-execution.md) d10). Cutting the v0.1 preview
tag is a maintainer act ([ADR-0007](decisions/ADR-0007-e1-execution.md) d14).

| ID    | Story                                                                                                                    | Done when                                                    | Size |
| ----- | ------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------ | ---- |
| E1.1  | Full lexer: all typl tokens, C17 int/float literal rules, durations, regex consts, trivia                                | lexer corpus tests pass                                      | M    |
| E1.2  | Full typl parser + generated typed-AST layer (`ungrammar`-style) over rowan                                              | typed AST accessors for every node; recovery on broken input | L    |
| E1.3  | Module system: `package`↔directory correspondence, `import`/`as`/`internal`                                              | mismatch is a hard error (ADR-0002)                          | M    |
| E1.4  | Resolver order (workspace member → pkg `[imports]` → workspace `[imports]` → error) + cross-package cycle detection      | resolver honors ADR-0002 §5–6                                | M    |
| E1.5  | Manifest: `ridl.toml` standalone + workspace modes; `serde`/`toml`                                                       | both modes parse; workspace nesting rejected                 | S    |
| E1.6  | Lockfile + cache + fetch: SHA-256 pinning, `~/.ridl/cache`, `ureq` fetch, `ridlc --frozen`                               | frozen verifies strictly; cache hit skips fetch              | M    |
| E1.7  | Type system: struct/enum/enumset/union/tuple/optional + bounded collections                                              | all typl composites check                                    | L    |
| E1.8  | Ranges & units: exact range/step (`num-rational`), nominal unit checking, wire-width derivation, int64 cap, `wire` floor | width/range diagnostics correct; exactness verified          | L    |
| E1.9  | Init-value derivation (typl §5.8; `default` is retired — ledger #31)                                                     | derived inits match spec                                     | M    |
| E1.10 | Diagnostics framework: coded `Diagnostic` struct, `codespan-reporting` renderer, LSP mapping, attribute-diagnostic set   | one struct → both terminal and LSP output                    | M    |
| E1.11 | IR v1 (typl) stabilized + `serde`/JSON debug rendering                                                                   | IR documented; JSON dump round-trips                         | M    |
| E1.12 | Rust + extern-C backend for typl types (`quote`/`prettyplease`)                                                          | generated Rust compiles + snapshot-tested                    | M    |
| E1.13 | `ridlc` stable flags + `ridl` facade (`check`/`build`/`fmt`)                                                             | plumbing/porcelain split real                                | S    |
| E1.14 | `ridl fmt`: rowan-based, tight `name: Type`, diff-minimal                                                                | idempotent; corpus reformats clean                           | M    |
| E1.15 | `ridl-lsp` MVP on `lsp-server`: diagnostics, hover (units/ranges), goto-def, find-refs, completion, rename               | features work on a real typl package                         | L    |
| E1.16 | Inlay hints: ordinal visibility + unit expansion (general-form §6.3)                                                     | ordinals render beside fields                                | S    |
| E1.17 | VS Code extension (LSP client + grammar)                                                                                 | installs, connects, highlights                               | S    |
| E1.18 | Test spine: corpus + `insta` snapshots + first `proptest` (ranges→generators)                                            | ranges generate boundary/step-violation corpora              | M    |
| E1.19 | CI `wasm32` build check for the compiler crates (feature-gate fetch/fs) — guards the E4.4 playground                     | `cargo check --target wasm32-unknown-unknown` green in CI    | S    |

## Epic 2 — ridl (the Interface Layer)

**Milestone:** RIDL as it exists today — the SSOT contract boundary with an
evolution gate. **Value:** real interface contracts, a second backend,
breaking-change detection in CI. The IR is proven language-neutral. **Exit
criteria:** ridl interfaces compile to Rust _and_ a second backend from one IR;
`ridl diff` gates breaking changes in CI.

**Status:** landed — all thirteen stories (E2.1–E2.13) shipped as PRs #136–#181.
`.ridl` packages carrying the five interaction kinds, timing, inline `T | E`
returns, `require`/`ensure` contracts, streams, interfaces and services compile
to IR v2, and that one IR drives both the Rust and the TypeScript backend; the
facade gained `ridl diff`, `ridl baseline`, and `ridl test`. Deferred per
[ADR-0008](decisions/ADR-0008-e2-execution.md): `persist` (d3) and the
general-form §4.7 promotion of `labels`/`deprecated` to attributes, and
diagnostic codes RIDL-111 and RIDL-142 — reserved by d21 and still unminted.
`ridlc build` emits Rust, the extern-C header, IR JSON, and TypeScript. The epic
itself shipped the TypeScript backend as a library only, pinned by the corpus
snapshots and reachable from no command; the `--emit typescript` path landed
afterwards, as a prerequisite for E3.3 (driftsys/ridl#172).

**The interaction layer this epic shipped is retracted by
[ADR-0018](decisions/ADR-0018-runtime-core-and-generated-surface.md) decision
15.** The exit criterion above was met literally and by output that no runtime
can implement: the interaction vocabulary is emitted once per package, so two
packages produce two incompatible `Provenance` types and no runtime crate can
implement a trait that does not exist until codegen runs. The layer does compile
— `corpus.rs` runs `rustc` and `tsc` over it with anti-vacuity guards — but
compiling proves syntax, not that anything can implement it. Epic 11 restores
the face as a client and a server over a runtime that exists.

E2 also paid three codes of the E1 debt
[ADR-0007](decisions/ADR-0007-e1-execution.md) d10 recorded: TYPL-301, TYPL-303,
and TYPL-304 ship, emitted by the parser once the family grammar made the
constructs they reject parseable, each with a showcase entry. E2.10's
"alias-not-required" row needed no new work — TYPL-008 has covered it from the
resolver since E1. The consolidated E2 debt roll-up is **#172**, on the E1
(#135) pattern.

| ID    | Story                                                                                                                                                   | Done when                                                               | Size |
| ----- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------- | ---- |
| E2.1  | `interact` core surface + semantics: `signal`/`event`/`command`/`query`/`fixed`                                                                         | all five kinds parse, check, resolve payloads                           | L    |
| E2.2  | Envelope + timing: `@Xms`, `@[min..max]` as generic rate-floor/staleness-bound                                                                          | timing attached to interactions; defaults applied                       | M    |
| E2.3  | Errors-as-data: `error` types, result unions, inline `T \| E` returns (general-form §6.1)                                                               | fallible query/fetch checks; three-strata respected                     | M    |
| E2.4  | `expr` **guaranteed subset** for `require`/`ensure` — forward-compatible with the E5.1 function layer                                                   | contract clauses parse + type-check; subset documented as V2-extensible | M    |
| E2.5  | Contract lowering to observer stubs (full checking deferred to E5)                                                                                      | observers represented in IR                                             | S    |
| E2.6  | Second backend: TypeScript bindings (chosen over proto — E3.3 builds on it)                                                                             | IR-neutrality proven; snapshot-tested                                   | L    |
| E2.7  | IR v2 (interaction layer)                                                                                                                               | interactions, timing, errors in IR                                      | M    |
| E2.8  | `ridl diff`: IR-snapshot compare, exit codes 0/1/2, ordinal + wire-width categories                                                                     | breaking vs compatible classified correctly                             | L    |
| E2.9  | Baseline-aware `ridlc`: desk-time reorder/insertion detection from lockfile/cache                                                                       | reorder flagged before CI (general-form §6.3)                           | M    |
| E2.10 | LSP + lint for ridl: interaction hovers, timing display, alias-not-required, canonical `T\|E` lints                                                     | lints fire on a real interface                                          | M    |
| E2.11 | Generated property tests wired into `ridl test` / CI                                                                                                    | range-derived corpora run as tests                                      | M    |
| E2.12 | expr-core specification (document, not code): the full contract-term grammar (family overview §2, ADR-0004 open q) — precedes or accompanies E2.4       | spec drafted; the E2.4 subset is checked against it                     | M    |
| E2.13 | `interface` vs `service` (ridl §14): abstract shape vs global published declaration, service catalog SSOT, `service.member` references, posture-neutral | services declare, resolve, and appear in the IR (E6 binds them)         | M    |

## Epic 3 — ridl Boundary Model, core ([ADR-0012](decisions/ADR-0012-interaction-boundary-model.md))

**Milestone:** ridl describes every boundary, not only system-to-system.
**Value:** at the person and world boundaries the datum and the thing it stands
for come apart, and the four correspondence obligations make that difference
declarable — a cluster reading's derivation from true speed, a sensor's
tolerance and sample lag, an actuator's authority and slew rate. **Exit
criteria:** a cluster telltale, a wheel-speed sensor, and a steering actuator
each compile with their obligations, classify correctly under `ridl diff`, and
generate bindings from the dispatch-family spellings alone.

**Core only.** [ADR-0012](decisions/ADR-0012-interaction-boundary-model.md)
decision 7 makes a domain extension a spelling table plus backends, with no
grammar, no IR nodes, and no semantics of its own. Those spellings are
**descoped from this epic** and land in E7 (rxdl). What is here is what ridl
must gain whether or not anyone ever writes `present` — and E3.1 through E3.4
are hard preconditions for E7 under
[ADR-0012](decisions/ADR-0012-interaction-boundary-model.md) decisions 8 and 9.

| ID   | Story                                                                                                                                                                                                                                                                                                                                                                                                                                               | Done when                                                                                     | Size |
| ---- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- | ---- |
| E3.1 | Attribute registry — name · owner · form · allow-list · consumer · diff category; namespacing outside core (ADR-0012 d8)                                                                                                                                                                                                                                                                                                                            | every key resolves to one owner; a colliding key fails the gate                               | M    |
| E3.2 | Fail-closed classification: unregistered key is a compile error; uncategorised key diffs as **breaking** (d9)                                                                                                                                                                                                                                                                                                                                       | a typo'd key errors; an unclassified change never reports compatible                          | S    |
| E3.3 | Core IR: `family` and `shape` closed enums on the interaction node; invalid-combination rejection (d2, d6)                                                                                                                                                                                                                                                                                                                                          | families round-trip; `(command, intent, no shape)` is rejected structurally                   | M    |
| E3.4 | The four correspondence obligations as core attributes — relationship, uncertainty, latency of correspondence, failure to correspond — with their diff categories; the paired form (commanded vs achieved, raw vs indicated) and the chained form (d3). Latency needs an **instant** form and a **span** form (a swept frame corresponds over an interval). VIM is the naming reference; the **influence quantity** is unresolved (ADR-0012 open 6) | obligations parse, type-check, reach the IR, and classify; tightening a tolerance is breaking | L    |
| E3.5 | Availability beyond `during`: the five sources, and consumer-evaluability at presentation boundaries (ADR-0012 open 4)                                                                                                                                                                                                                                                                                                                              | a predicate a consumer cannot evaluate is rejected at a presentation boundary                 | M    |
| E3.6 | LSP + lint over families and obligations                                                                                                                                                                                                                                                                                                                                                                                                            | hovers show family and obligations on a real contract                                         | S    |

## Epic 4 — Ecosystem & Adoption (V1)

**Milestone:** the contract platform is _approachable_ — a stranger can learn
it, try it, and depend on it. **This rounds out the public V1.0.** **Value:**
the V1 adoption multipliers; each ships when its underlying layer is ready, not
big-bang. **Exit criteria:** error-index site live, playground live,
getting-started + contract tutorial published, IR plugin protocol documented and
versioned.

| ID   | Story                                                                                                                   | Done when                                    | Size |
| ---- | ----------------------------------------------------------------------------------------------------------------------- | -------------------------------------------- | ---- |
| E4.1 | `ridl doc`: interfaces rendered as tables/HTML, obligations included                                                    | doc output for a real package                | M    |
| E4.2 | Error-index website: every `TYPL-`/`RIDL-` code with explanation + fix (rustc `--explain` style)                        | codes cross-link from diagnostics            | M    |
| E4.3 | `.typl`+`.ridl` getting-started + contract tutorial (types → interface → boundaries)                                    | a newcomer compiles it unaided               | M    |
| E4.4 | Browser playground: compiler-to-WASM, live edit→codegen                                                                 | edit typl/ridl, see generated output in-page | L    |
| E4.5 | IR plugin protocol spec + versioning (IR stability policy) — settles which encoding is canonical, per driftsys/ridl#231 | a third-party backend consumes the IR        | L    |
| E4.6 | `ridl init`/`ridl new` scaffolding + `ridl vendor` (air-gap)                                                            | scaffolds a valid workspace; vendors deps    | S    |
| E4.7 | Governance CI: keyword-registry collision test, and the E3.1 attribute registry enforced in CI                          | colliding key across profiles fails CI       | S    |

**E4.5 inherits an open question it must answer rather than inherit.**
[ADR-0014](decisions/ADR-0014-ir-encodings.md) decision 9 makes binary the
canonical encoding, JSON derived and conformance-obliged, prototext for
inspection. Decision 14 records that binary cannot round-trip IR this toolchain
produces — prost's decode limit refuses what its own encoder wrote, between
roughly 50 and 128 nesting levels — while JSON now round-trips everything the
front end admits. So the encoding named canonical is the one with the weaker
guarantee, and no consumer reads it today. E4.5 is where stability is defined
against something, so E4.5 is where that is settled, not before: deciding it in
isolation means deciding it twice. Recorded on driftsys/ridl#231.

## Epic 9 — Wire SSOT: signal store and dispatcher from the IR

**Milestone:** a `.ridl` package generates a working signal store and an
event/command/query dispatcher for a target whose schema carries its own field
numbering. **Value:** this is ridl used as the SSOT for a real system bus, and
it is the first end-to-end demand on the IR from outside the workspace — the
proving ground for whether the IR is a stable public artifact or a Rust
implementation detail. **Exit criteria:** the cruise-control package emits
canonical protobuf JSON that a non-Rust runtime parses, projects to proto3 and
FlatBuffers schemas, and generates a store and dispatcher whose identity is
stable under `ridl-diff`.

**Design of record** — four notes in `docs/wip/`, all sharing one origin:
[`2026-08-03-ir-protobuf-encodings-design.md`](wip/2026-08-03-ir-protobuf-encodings-design.md)
·
[`2026-08-03-rpc-response-bound-design.md`](wip/2026-08-03-rpc-response-bound-design.md)
·
[`2026-08-03-multi-interface-services-design.md`](wip/2026-08-03-multi-interface-services-design.md)
·
[`2026-08-03-schema-projection-design.md`](wip/2026-08-03-schema-projection-design.md).
Three ADRs fall out: **ADR-0014** (IR encodings, superseding
[ADR-0004](decisions/ADR-0004-implementation-sequencing-and-stack.md) §4's
rendering clause), **ADR-0015** (the QoS absorption principle and RPC bounds),
and **ADR-0016** (the projection contract and the pinned name transform,
correcting the fourth note where execution disproved it).

**Sequencing caution.** E9.4 to E9.6 alter ridl's surface and IR, as E3 does.
The two epics must not run concurrently on the IR. E9 is the nearer-term product
path; E3 is the larger, more speculative one.

**Status: E9.1 to E9.6 landed 2026-08-04**, as PRs driftsys/ridl#215,
driftsys/ridl#217 and driftsys/ridl#219 through driftsys/ridl#224, with three
follow-ups on the IR encodings — driftsys/ridl#225 through driftsys/ridl#228.
Both records are ratified and implemented.

[ADR-0014](decisions/ADR-0014-ir-encodings.md) is complete: canonical protobuf
JSON replaced the `serde` rendering on every surface, prototext and binary
joined it as emits, and the `ridl.std` filter became an exhaustive
classification over `Emit`. The JSON mechanism then moved again, from
`prost-reflect` to `pbjson`-generated impls (decision 14): the transcode carried
prost's non-configurable recursion limit, which made `--emit ir-json` fail on
legal source that three other emits handled. Output is byte-identical, so no
golden changed. [ADR-0015](decisions/ADR-0015-qos-absorption-and-rpc-bounds.md)
is complete for this block: `command` and `query` carry the range form with
RIDL-112 warning an undeclared response bound, the coherence rule is normative
prose in ridl §14.5, and a service composes several interfaces with
per-interface ordinals keyed by name. On 2026-08-05,
[ADR-0016](decisions/ADR-0016-schema-projection-and-the-name-transform.md)
ratified the fourth design note — the schema projection — and corrected three of
its statements: the transform choice, the injectivity requirement, and the
inline-shape unification.

Four ADR amendments came out of review rather than out of design, and all are
recorded in place. **ADR-0014 decision 12** retracts decision 7's infallible
`to_json_pretty`: `prost-reflect` transcodes through the wire encoding and
prost's `RECURSION_LIMIT` is not configurable, so deep composite nesting
panicked on source the checker accepts and the other three emits handle.
**ADR-0015 decision 24** requires an interface name to be unique across a
service's shapes, live or retired, and makes a retargeted slot breaking — two
shape changes had been diffing as compatible by omission, one of them a
regression against the comparison decision 19 superseded. **ADR-0014 decision
13** contains the prototext reader as `#[cfg(test)]`, because its parser
exhausts a 2 MiB stack below prost's own limit, so the error return is
unreachable there and a stack overflow cannot be caught. **ADR-0014 decision
14** records the `pbjson` move, its measured ratios, and two limits it leaves
standing: the JSON reader is stricter than the one it replaced, and prost's
decode limit binds on the binary path for IR this toolchain produces.

**Status: E9.7 landed 2026-08-07** as PR driftsys/ridl#238, closing out the
projection contract ADR-0016 ratified above: `ridl_ir::name::snake_case` became
the one pinned transform, both existing copies were deleted, and RIDL-149
rejects two members of one interface or two parameters of one interaction that
collide after it. PR driftsys/ridl#239 amended
[ADR-0013](decisions/ADR-0013-codegen-backend-scope.md) decision 7 the same
week, narrowing "a wire backend refuses what its target cannot represent" to
name absence specifically: a target that can carry absence in-band, by reserving
a value the declared range does not use, does so rather than refusing the field
outright.

**E9.8 landed 2026-08-08** as `ridl-backend-proto`, the first wire backend
(ADR-0013 decision 2). The typl surface — structs, enums, enum sets and unions —
projects to proto3 messages and enums; the interaction identity table projects
to one ordinal enum per interface. No `service` block, no call face, no value
store. Struct fields joined the pinned transform and RIDL-149 in this story,
discharging ADR-0016 decision 4's exclusion in the same commit that starts
projecting them. Validity is established by compiling every emitted schema with
`protox` inside the test suite, and the stability property is driven from
`ridl_diff::diff_packages` rather than hand-picked examples.

The design took two decisions of its own, neither recorded in an ADR.
**Constraints are carried as comments, and only as comments:** a named scalar's
unit, range and step have no proto3 construct to occupy, so they are recorded as
a generated comment at each use site — leaving an inline scalar field, which
names no type to hang a comment on, with no home for its constraint information
at all, recorded as an open question rather than solved. **The emit ceiling for
this story is tier 1 and tier 2, and nothing above them:** E9.8 emits no
`service` block, leaving that to E9.11 rather than resolving the tension below
as a side effect of adding a backend.

**The conflict left for E9.11 — now resolved by
[ADR-0018](decisions/ADR-0018-runtime-core-and-generated-surface.md) decision
18.** ADR-0013 decision 2 says a wire backend emits "no `service` block, no call
face, no value store"; ADR-0016 decision 10 describes the dispatcher as "one
service definition per provided interface" — in proto3, a `service` block. E9.8
avoided the conflict by emitting neither, and E9.11 could not, so decision 18
separates the two readings: **no service block that projects interactions as RPC
methods** (decision 2's concern, since such a schema understates the contract),
but one **access service** whose methods are kind-blind access operations keyed
by ordinal, which carries §4.4's last value, §4.5's provenance and §3.1's
envelope in its messages. That service is optional, independent of the generated
package, and not generated at all — the operations do not vary by contract, so
it is one published schema a consumer takes or ignores in favour of their own
binding. The store and dispatcher work now sits in Epic 11. A second consequence
for E9.11 to inherit: tier 2 emits only the ordinal enum and never an
interaction's payload type, so no payload type reaches the import path today —
the store and the dispatcher will need to import the payload types tier 2 never
touches.

**E9.9 to E9.12 are not in this block.** The IR is settled and E3 is unblocked;
the FlatBuffers projection, the schema hash, the store and dispatcher, and the
recorded general form R5 drift remain. Consolidated debt: driftsys/ridl#218.

| ID    | Story                                                                                                                                                                                                                                                                                      | Done when                                                                                                                               | Size |
| ----- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------- | ---- |
| E9.1  | `ridl-ir` serialization rewrite — the emitted `.ir.json` is a serde rendering of Rust structs, not protobuf JSON, so no non-Rust protobuf runtime can parse it (ADR-0014)                                                                                                                  | a non-Rust protobuf runtime parses the emitted IR                                                                                       | L    |
| E9.2  | Two new emit values — prototext and binary — alongside canonical protobuf JSON                                                                                                                                                                                                             | all three encodings round-trip to the same IR                                                                                           | M    |
| E9.3  | The latent emit-filter defect the new emits expose (§5.1 of the encodings note)                                                                                                                                                                                                            | the filter is correct for every emit value                                                                                              | S    |
| E9.4  | **RPC bounds and the response bound** — ridl §9 gains its RPC column; RIDL-112 minted for a missing bound; RIDL-106 narrows to `fixed`; RIDL-103 widens (ADR-0015)                                                                                                                         | an RPC declares a response bound; an undeclared one warns, profile-escalable                                                            | M    |
| E9.5  | The **coherence rule** — coherence is implicit and the interface is the generation unit — as normative prose in ridl §14                                                                                                                                                                   | the rule is stated where the service definition it keys on lives                                                                        | S    |
| E9.6  | **Multi-interface services** — `ServiceDef` carries several interfaces; ordinals stay per-interface keyed by name; addressing stays flat; duplicate member is an error                                                                                                                     | a service composes two interfaces; reordering the list leaves transport identity untouched and diffs as breaking (ADR-0015 decision 19) | L    |
| E9.7  | The **projection contract** and its pinned name transform — the collision rule (RIDL-149) replaces the injectivity requirement; the divergent `interact.rs` implementation is deleted in favour of `c_header.rs`'s; specified with E4.5's stability policy (ADR-0016)                      | one transform, in `ridl-ir`; a package whose member or parameter names collide after it is rejected                                     | M    |
| E9.8  | **proto3 projection** — schemas from IR identity, per ADR-0013's shape-and-identity ceiling                                                                                                                                                                                                | the cruise-control package emits valid proto3                                                                                           | L    |
| E9.9  | **FlatBuffers projection** — the column Appendix B was missing. The _schema_ emit; the FlatBuffers _codec_ for the store and queue is E11.7                                                                                                                                                | the cruise-control package emits a valid FlatBuffers schema                                                                             | L    |
| E9.10 | The **schema hash over the IR**, not over the emitted schema                                                                                                                                                                                                                               | two targets of one IR agree on identity                                                                                                 | M    |
| E9.11 | **Moved to Epic 11** by [ADR-0018](decisions/ADR-0018-runtime-core-and-generated-surface.md) decision 16 — the store is E11.2 and the dispatcher is E11.4, because both are the runtime's shape rather than a wire projection. The row stays so the identifier keeps meaning what it meant | — see E11.2 and E11.4                                                                                                                   | —    |
| E9.12 | Drift the design surfaced: general-form R5's postfix order contradicts the shipped grammar (`@timing` is last, not before attributes); `InterfaceDef`/`ServiceDef` gain the `AttrBlock` the deferred `labels`/`deprecated` promotion also needs                                            | R5 matches `family.ungram`; one grammar edit serves both                                                                                | S    |

## Epic 10 — typl value objects

**Milestone:** a typl package generates types that cannot hold an invalid value.
**Value:** closes a promise the shipped documentation already makes — typl §1.1
says a pure typl package "generates data types, **validators**, and
documentation across every backend", and the glossary defines SSOT the same way,
while **neither language backend emits a validator**. Until this lands, the
constraint layer is documentation rather than a guarantee. **Exit criteria:** a
constrained named scalar cannot be constructed out of range in Rust or
TypeScript, and `--emit rust` writes a crate that compiles.

**Design and plan of record:**
[`typl-value-objects-design.md`](wip/typl-value-objects-design.md) and
[`typl-value-objects-plan.md`](wip/typl-value-objects-plan.md), the latter
already task-decomposed. It amends
**[ADR-0013](decisions/ADR-0013-codegen-backend-scope.md)** rather than minting
a record — that ADR is still Proposed and already classifies backends by what
they may emit, and this settles the **validator half** of its open item 1: a
language backend emits constraint-checking constructors where a wire backend
does not. The **interaction-face half** is settled the other way by
[ADR-0018](decisions/ADR-0018-runtime-core-and-generated-surface.md) decision
15, which retracts the shipped interaction layer and restores it as a client and
a server in a second phase. The two are complementary — this epic is that
record's phase 1, and E10.7's compiling crate is the compile gate the retraction
needs to be verifiable.

| ID     | Story                                                                                                                                                                                                                                                                                   | Done when                                                         | Size |
| ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------- | ---- |
| E10.1  | The shared vacuous-constraint classifier — one definition of "constrains nothing"                                                                                                                                                                                                       | both backends agree on which types need a fallible constructor    | M    |
| E10.2  | The Rust `ConstraintError` vocabulary                                                                                                                                                                                                                                                   | one error type carries every constraint failure                   | S    |
| E10.3  | Constrained named scalars — private inner, `new`, `TryFrom`                                                                                                                                                                                                                             | an out-of-range value is unconstructible                          | L    |
| E10.4  | Vacuous named scalars — infallible construction                                                                                                                                                                                                                                         | a type that constrains nothing takes no fallible path             | M    |
| E10.5  | `TryFrom<i64>` for enum and enum set                                                                                                                                                                                                                                                    | an undefined discriminant is rejected                             | M    |
| E10.6  | Sound derives — no derive that could reconstruct an invalid value                                                                                                                                                                                                                       | no path bypasses the validating seam                              | M    |
| E10.7  | `--emit rust` writes a compiling crate                                                                                                                                                                                                                                                  | the emitted crate builds standalone                               | M    |
| E10.8  | Pattern validation behind a `validate-pattern` feature                                                                                                                                                                                                                                  | regex constraints check without forcing the dependency            | M    |
| E10.9  | TypeScript vocabulary and factories                                                                                                                                                                                                                                                     | the TS backend refuses an invalid value at construction           | L    |
| E10.10 | Amend ADR-0013 and typl §5.7; verify the `ridl-diff` classification                                                                                                                                                                                                                     | the decision is recorded and a constraint change classifies right | S    |
| E10.11 | **Rebuild cross-backend parity over the type layer** — `crates/ridlc/tests/parity.rs` was deleted with the interaction layer it compared ([ADR-0018](decisions/ADR-0018-runtime-core-and-generated-surface.md) decision 15), and this epic gives both backends a new comparable surface | one assertion relates both backends over the whole corpus again   | M    |

## Epic 11 — `ridl-rt`, the runtime core ([ADR-0018](decisions/ADR-0018-runtime-core-and-generated-surface.md))

**Milestone:** a generated contract runs. A provider publishes into a shared
store, a consumer reads it coherently, and a call crosses a process boundary.
**Value:** this is what makes V1 a platform rather than a code generator —
ADR-0018 decision 16 records that without it V1 ships a compiler whose output no
runtime can consume, which is also why the E2 interaction layer was
unimplementable. **Exit criteria:** the cruise-control package's signals
round-trip through a shared-memory store between two processes on one node, with
a hand-written provider and consumer; the same contract reaches a Deno tool over
the frame protocol; and the store layout is stable under a compatible change.

**Sequencing.** E11.1 and E11.2 are specification and block the rest. The epic
depends on E10 for types that carry their constraints, and on typl §17.11's
deferred width floor — ADR-0018 decision 8 makes that a prerequisite rather than
a deferral, because widening a range flips the resolved width and shifts every
subsequent slot offset.

**Deliberately out of scope.** The rmdl language (E11.10 fixes the binding
contract; hand-written components exercise it), the rsdl grammar (E11.6 defines
the facts the generator needs, and rsdl becomes their authoring surface later
per ADR-0018 decision 17), and phase 2's client and server, which follow this
epic.

| ID     | Story                                                                                                                            | Done when                                                            | Size |
| ------ | -------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- | ---- |
| E11.1  | The frame and control-plane specification — ordinal, kind, envelope, provenance, correlation; `attach`/`subscribe`/`read`/`call` | one document a second implementation could be written from           | M    |
| E11.2  | The store layout projection — region per interface, slot offsets from ordinals, the three counters                               | layout is deterministic, total, and stable under a compatible change | L    |
| E11.3  | Platform traits (`Socket`, `Region`, `Notify`, `Clock`) and the reference Linux implementation                                   | the four traits plus a driving loop run the core                     | M    |
| E11.4  | The sans-IO core — seqlock discipline, subscription table, envelope stamping, `poll` returning a deadline                        | the core runs with no executor, no sockets and no real time in tests | L    |
| E11.5  | The ring and its derived depth — rate floor × service period × target jitter, with the rsdl override                             | an underivable or infeasible depth fails the build                   | M    |
| E11.6  | The deployment-facts schema — placement, target properties, wiring, protection domains, reservations, service periods            | a hand-written descriptor drives generation end to end               | L    |
| E11.7  | FlatBuffers codec for the store and the queue                                                                                    | a payload round-trips through a mapped slot                          | L    |
| E11.8  | proto3 codec plus byte-level conformance against a `protoc`-generated implementation                                             | our bytes parse there and its bytes parse here                       | L    |
| E11.9  | The socket binding and the Deno tooling client                                                                                   | a tool subscribes and renders a live signal                          | M    |
| E11.10 | The component binding contract — activation, input read, output publish, what a step means                                       | a hand-written provider and consumer compile against it and run      | M    |
| E11.11 | Memory feasibility as a deploy-time check, sibling to RSDL-801                                                                   | a deployment exceeding the target's declared budget fails the build  | S    |

---

## Epic 12 — The tooling plane ([ADR-0018](decisions/ADR-0018-runtime-core-and-generated-surface.md))

**Milestone:** the first consumers of the platform that are not the compiler.
**Value:** observability, emulation and test-harness drivers were each
identified as missing when the architecture was written — the specifications
reference observability hooks (ridl §10.3, §3.1 spans) and a replay harness
(E5.10) without any story building them. **Exit criteria:** a Deno tool
subscribes to a running store and renders a live signal with its provenance; a
hand-written emulator stands in for a provider and the consumer cannot tell; a
harness drives a system under test and asserts against the contract.

**The plane splits in two.** A **contract-specific** tool — a driver for one
contract, an emulator for one component — takes generated TypeScript types with
the Rust codec behind wasm. A **contract-generic** tool — an observability
server that must display a contract it was not built against — loads the IR at
runtime and decodes with the descriptor-driven engine. Only the first needs a
backend; the second needs E4.5.

**The twin is a smaller step than it sounds, because the store already is one.**
ADR-0018 decision 8's store holds the current value of every signal in an
interface with its provenance and envelope — a twin of that interface at an
instant. A twin of a _system_ is the union of those stores, retained over time
and queryable. So E12.10 adds aggregation, history and query to something E11
already builds, rather than modelling state a second time.

Emulation and twins are why rung 1's role is simulation as well as tooling: the
servers run on a desk, and a system under test should not be able to tell them
from the real providers.

**Platform rungs 1 and 2.** Desktop for the tooling itself, mobile for the UI
and bridge demonstrations.

| ID     | Story                                                                                                                                    | Done when                                                                            | Size |
| ------ | ---------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ | ---- |
| E12.1  | The TypeScript surface — generated types and interfaces, with the Rust codec reached through wasm                                        | a Deno program constructs and validates a payload without a TS codec                 | L    |
| E12.2  | Observability server — subscribe by ordinal, render value, provenance and envelope                                                       | a live signal is displayed with its provenance distinguishable                       | L    |
| E12.3  | Contract-generic decoding — load the IR at runtime, decode against it with the descriptor engine                                         | a contract the tool was not built against is displayed correctly                     | L    |
| E12.4  | Emulator — a hand-written provider standing in for a component, driven by the contract                                                   | a consumer cannot distinguish the emulator from the real provider                    | M    |
| E12.5  | Test-harness drivers — drive a system under test, assert against contract terms                                                          | a `require` violation is reported against the contract, not the transport            | M    |
| E12.6  | Trace capture and replay over the frame                                                                                                  | a captured session replays and produces an identical trace                           | M    |
| E12.7  | Embedded web UI — the person boundary rendered in a webview, honouring §3.4 availability and RIDL-505                                    | a control is disabled before use rather than failing on use                          | L    |
| E12.8  | Emulation server — host many contract-conformant providers at once, with lifecycle and addressing, rather than one hand-written stand-in | a system under test binds to emulated providers it cannot distinguish from real ones | L    |
| E12.9  | Scenario scripting — drive a signal along a declared profile, inject an invalid value, withdraw a provider                               | a §4.5 invalid transition and a provider loss are both reproducible on demand        | M    |
| E12.10 | Digital twin server — aggregate stores across components, retain history, answer queries over it                                         | the state of a whole system at a past instant is answerable                          | L    |

---

# V2 — The Executable Platform

## Epic 5 — rmdl (Behaviour) — split into two phases

**This epic runs in two separated phases.** The language half is a V2
prerequisite for rsdl; the runtime half is V3 and runs last.

**E5a — the language (E5.1–E5.7), before E6.** Expressions, equations, memory,
time, and signature checking, all the way to the IR. **No code is generated from
a model.** This is what E6 needs in order to bind a reaction to a contract: rsdl
requires the model's _shape and semantics_, not its execution. **Exit
criteria:** the cruise-control model parses, type-checks, passes causality and
completeness analysis, and reaches the IR with its equations intact.

**E5b — the compute runtime (E5.8–E5.13), last.** Rust and WASM codegen, the
scheduler, the reference oracle, replay, and the flow stdlib. The novel, hard
core, deferred until every layer above it is settled. **Exit criteria:** the
cruise-control model computes its reaction, runs native and as a WASM component
with identical step traces, and the oracle diffs step-by-step against an
implementation.

| ID    | Story                                                                                                                                                                                                | Done when                                                          | Size |
| ----- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------ | ---- |
| E5.1  | Function/expr core: total functions, `let`, `if`/`case`/`match`, bounded combinators + totality checks (RMDL-1xx) — **extends E2.4**                                                                 | recursion/loops/`last`-in-fn rejected; E2.4 subset subsumed        | L    |
| E5.2  | Model: equations, single-definition, causality (acyclic-except-`last`), topo schedule                                                                                                                | instantaneous cycles rejected; schedule derived                    | L    |
| E5.3  | Memory: `last`/`init` seeding + implicit channel-init seeding                                                                                                                                        | first-step values correct; RMDL-203 fires                          | M    |
| E5.4  | `when`/`emit` event equations + `case` mode equations                                                                                                                                                | hold-vs-emit semantics correct                                     | L    |
| E5.5  | Ambient time: `now`/`dt`/`time(f)`, logical-time step context                                                                                                                                        | integrator correct under any activation pattern                    | M    |
| E5.6  | Signature checks (RMDL-3xx): contract-blindness — no `realizes` (RMDL-302) — and output completeness (RMDL-301)                                                                                      | contract references in models rejected; undefined outputs rejected | S    |
| E5.7  | Step faults (§8): atomic abort, invalid-state propagation, recording                                                                                                                                 | fault preserves state, marks outputs invalid                       | M    |
| E5.8  | Rust-native codegen: state struct + `step()` fn, IEEE-754-strict                                                                                                                                     | model runs natively, deterministic                                 | L    |
| E5.9  | WASM-component codegen: `wit-bindgen` + `cargo-component`, WIT from contract                                                                                                                         | component builds and runs under wasmtime                           | L    |
| E5.10 | Reference-oracle + replay harness (`wasmtime`); native vs WASM trace equality — needs a cross-target deterministic-math strategy (IEEE-754 ops only, or a shipped deterministic libm), fixed at E5.1 | traces bit-identical across targets                                | M    |
| E5.11 | `jco` browser path for person-boundary execution                                                                                                                                                     | component runs in a browser host                                   | M    |
| E5.12 | Minimal `ridl-rt` scheduler/timeline (input + deadline activation, coalescing)                                                                                                                       | reactive stepping, quiescent when idle                             | L    |
| E5.13 | `ridl.std.flow` / `std.control` Tier-1 adapters (rmdl-defined)                                                                                                                                       | Hold/Changes/Filter/Accumulate/Deadband/Latch compile              | M    |

## Epic 6 — rsdl (System Assembly)

**Runs after E5a, before E7 and E5b.**

**Milestone:** a system is assembled from components and its deployment
artifacts generated. **Value:** components situate the contract-blind reactions
E5a describes — binding them to services, wiring event→command side effects, and
deriving transport, posture, and deployment from the SSOT. Nothing here needs a
model to _run_: E6.3, E6.4, and E6.5 need rmdl's IR, which E5a delivers, and the
test plane (E6.11) is derivable from contracts alone. **Exit criteria:** the
cruise-control system (interface + model + boundaries) assembles from `.rsdl`
components — composition and deployment as two regions of one grammar (rsdl §2)
— and emits topology + integration artifacts.

| ID    | Story                                                                                                                                                                                     | Done when                                                                    | Size |
| ----- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- | ---- |
| E6.1  | `component` declarations: `provides`/`requires` boundary at three grains (inline / interface / service), leaf vs composite (rsdl §3)                                                      | components parse and check; all three boundary grains resolve                | L    |
| E6.2  | Application-notation wiring: applications as instances, fused `provides … = …`, destructuring, `let` intermediates; composite cycles legal, leaf sync cycles rejected (rsdl §4, RSDL-407) | cruise-control wiring compiles; a leaf-level sync cycle is rejected          | M    |
| E6.3  | Cross-layer resolution (references to typl/ridl/rmdl)                                                                                                                                     | instance typing + binding resolve                                            | M    |
| E6.4  | Contract binding: service member completeness (RSDL-303), timing transfer + init-consistency boundary checks (moved out of rmdl — rmdl §7), declared redundancy (RSDL-502) (rsdl §5)      | every provided member covered; an accidental second provider fails the build | M    |
| E6.5  | Event→command wiring (rmdl §5.7, rsdl §4.2, RSDL-405)                                                                                                                                     | emitted event wires to a command                                             | S    |
| E6.6  | `system` root: external boundary, assurance profile, one per workspace (rsdl §6)                                                                                                          | the system compiles as the root component                                    | S    |
| E6.7  | `deployment` region: targets by capability class, complete placement (RSDL-701), time base (rsdl §7)                                                                                      | every instance placed or RSDL-701 fires                                      | M    |
| E6.8  | Transport + posture derivation: static vs discovered per connection, physics constraint (RSDL-803), contract-timing feasibility (RSDL-801) (rsdl §8)                                      | one composition deploys both postures; infeasible timing rejected            | L    |
| E6.9  | Bundles: versioned distribution artifacts, `tier` dependency gating (rsdl §9, RSDL-901)                                                                                                   | bundle manifests emitted for the cruise-control system                       | M    |
| E6.10 | Topology + integration-artifact emission                                                                                                                                                  | deployable manifest/topology generated                                       | M    |
| E6.11 | Test topology as a `deployment`: injectors/oracle swapped in (rsdl §2)                                                                                                                    | rest-bus-style test deployment derived                                       | M    |

## Epic 13 — The gateway ([ADR-0018](decisions/ADR-0018-runtime-core-and-generated-surface.md))

**Milestone:** one contract, two encodings, traffic crossing between them.
**Value:** this is what a single source of truth buys that a schema language
does not — the mapping is derived rather than hand-maintained, and rsdl §8.2's
feasibility check catches an infeasible bridge at build time rather than in the
vehicle. **Exit criteria:** a bridge carries signals between two encodings with
provenance and envelope preserved end to end, and a timing-infeasible bridge
fails the build.

**The engine is core; the wires are plugins.** A gateway is two codecs plus a
routing decision. The codecs for the core encodings are E11's; a bridge to CAN
or SOME/IP is that plugin's, and this epic is what those plugins bridge
_through_. It is in V2 because the routing decision needs the topology rsdl
carries — which components, which links, which protection domains.

| ID    | Story                                                                                                  | Done when                                                        | Size |
| ----- | ------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------- | ---- |
| E13.1 | The domain-mediated bridge — decode, validate, re-encode, as the correctness oracle                    | a payload crosses two encodings and round-trips                  | M    |
| E13.2 | Generated streaming transcoders, with the equivalence test against E13.1                               | the fast path and the reference produce identical bytes          | L    |
| E13.3 | Quantization across a bridge — a scaled value keeps its resolution and says so                         | a coarse source does not appear precise downstream               | M    |
| E13.4 | Feasibility — a bridge whose added latency breaks a staleness bound fails the build (RSDL-801 sibling) | an infeasible bridge is a deploy-time error                      | M    |
| E13.5 | The descriptor a contract-generic gateway consumes, shared with E12.3                                  | one engine serves the gateway and the generic observability tool | L    |

## Epic 7 — rxdl & Executable-Platform Ecosystem — split into two phases

**This epic runs in two separated phases.** The rxdl half is V2 and runs after
E6; the ecosystem half is V3 and trails E5b, because it needs models to execute.

**E7a — rxdl (E7.1, E7.7–E7.9), after E6.** The unrestricted profile and the
domain extensions. `.rxdl` **absorbs both meanings of the wildcard**: a `.rxdl`
file lifts the _layer_ restriction (any layer, the original total profile) and
the _domain_ restriction (the person and world spellings). One rule — `.rxdl` is
the profile with no restrictions — and per-package tightening in `ridl.toml` is
how a production package gets strictness back.

**E7b — the ecosystem (E7.2–E7.6), after E5b.** Oracle test plane, deductive
proof, registry, tutorial, and the V3 error-index codes.

**Exit criteria (E7a):** a person-boundary and a world-boundary contract compile
from their domain spellings, generate bindings, and diff correctly; an `.rxdl`
file carries mixed layers and compiles. **Exit criteria (E7b):** the
reference-oracle test plane and deductive-proof path work; the registry
publishes and resolves.

| ID   | Story                                                                                                                                                                                                                   | Done when                                                                                           | Size |
| ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- | ---- |
| E7.1 | `.rxdl` unrestricted profile: lifts both the layer restriction (types + interface + model + wiring in one file) and the domain restriction (person and world spellings); per-package tightening enforced in `ridl.toml` | full mixed-layer file compiles; a package that tightens rejects what it forbids                     | M    |
| E7.2 | Reference-oracle test plane: spy/control bridge as a generated interface, online observers                                                                                                                              | live flows spied/asserted from contracts                                                            | L    |
| E7.3 | Deductive-proof verification path (Creusot-compatible `expr` discharge)                                                                                                                                                 | a provable contract discharged deductively                                                          | L    |
| E7.4 | Package registry service (separate repo/lifecycle)                                                                                                                                                                      | publish + resolve a remote package                                                                  | L    |
| E7.5 | Full `.rxdl` getting-started + end-to-end tutorial (types → interface → model → wiring)                                                                                                                                 | a newcomer builds the whole cruise-control system unaided                                           | M    |
| E7.6 | Error-index website extended with `RMDL-`/`RSDL-` codes (completes E4.2)                                                                                                                                                | every `RMDL-`/`RSDL-` code has an explanation + fix entry                                           | S    |
| E7.7 | Domain spellings over the E3 core: `present` `notify` `measure` `detect` `actuate` `trigger`, the closed intent shapes, and the intent occurrence once named (ADR-0012 d4, d5)                                          | lowering to (kind, family, shape) is bijective; `ridl fmt` and IR rendering round-trip the spelling | M    |
| E7.8 | **hmi domain** — spelling table + viewmodel bindings (TS first)                                                                                                                                                         | a person-boundary contract generates a viewmodel                                                    | M    |
| E7.9 | **env domain** — spelling table + device/bus binding                                                                                                                                                                    | a world-boundary contract generates its binding                                                     | M    |

---

# Cross-Cutting

## Epic 8 — Agent Enablement ([ADR-0005](decisions/ADR-0005-agent-enablement.md), threads V1→V2)

**Milestone:** an AI agent turns a natural-language spec into correct, idiomatic
RIDL and evolves it _provably_ (compiles clean, `ridl diff` compatible) — for
types and interfaces in V1, behaviour in V2. **Value:** RIDL is niche, so a
model has ~zero priors; the knowledge layer is the highest-leverage, cheapest
piece and needs no compiler. The MCP is near-free because it reuses the IR,
diagnostic SSOT, and `ridl-diff` built for other consumers
([ADR-0005](decisions/ADR-0005-agent-enablement.md) "one engine, three faces").
**Exit criteria (V1 slice):** skill + rules author valid typl/ridl; MCP
`ridl_check`/`ridl_explain`/`ridl_diff` + IR-query tools back a verify/evolve
loop; the eval corpus runs in CI. **V2 slice:** behaviour skill + oracle eval;
`ridl-architect` subagent.

Sequencing note — [ADR-0005](decisions/ADR-0005-agent-enablement.md) §8 maps
agent work onto the old Phase 1–5; under the V1/V2 re-cut those become: typl→E1,
ridl→E2, the boundary model→E3, `ridl doc`→E4, rmdl→E5, rsdl→E6,
family-whole→E7. Each story below carries the epic it rides.

**Preserve, don't build ([ADR-0005](decisions/ADR-0005-agent-enablement.md) §7
invariants — constraints on other epics):** every diagnostic stays coded +
fix-it (E1.10); `.rxdl` is the canonical agent target and eval unit (E7.1, the
unrestricted profile); sigil poverty is kept; IR + diagnostic-code + `ridl-diff`
stability _is_ the agent-contract stability (E4.5 / IR-stability open question).

_Layer A — Knowledge (skill + rules; build first, no compiler dependency)_

| ID   | Story                                                                                                                                                                                                                                     | Rides                 | Done when                                                                                                   | Size |
| ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------- | ----------------------------------------------------------------------------------------------------------- | ---- |
| E8.1 | Rules file — 10–20 always-on "never/always" constraints, distilled from doctrines + the _error_ diagnostics (no semicolons, named-typl payloads, errors-as-data, command≠query, no inheritance, no upward refs, append-only + `reserved`) | E1 (may precede code) | every rule cites a diagnostic code or doctrine; loads in Claude Code/Cursor/Cowork                          | S    |
| E8.2 | Skill v0 (typl) — dense decision tables + worked examples for types/ranges/units/evolution, per `skill-ridl-authoring-outline.md`                                                                                                         | E1                    | authors valid `.typl`; content traceable to the typl reference                                              | M    |
| E8.3 | Skill extended to the ridl `interact` core — 5-kind selection table, timing, errors-as-data / `T\|E`, common-mistakes table keyed to codes                                                                                                | E2                    | covers ridl ref §3–§10; cruise-control example round-trips clean (`.rxdl` descriptive form once E3.5 lands) | M    |
| E8.4 | Skill profile for the boundary model — the five families and their obligations                                                                                                                                                            | E3                    | authors a valid person-boundary contract with its obligations                                               | S    |

_Layer B — Capability (MCP over the compiler; build second, cheap given
[ADR-0004](decisions/ADR-0004-implementation-sequencing-and-stack.md))_

| ID   | Story                                                                                                                                            | Rides | Done when                                                                | Size |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------ | ----- | ------------------------------------------------------------------------ | ---- |
| E8.5 | MCP server skeleton — thin binary over the shared salsa crates, sibling of the LSP, **no second parser**; stdio transport                        | E2    | server starts, advertises tools, shares the compiler crates              | M    |
| E8.6 | Verify/evolve tools — `ridl_check` (coded diagnostics + **fix-its verbatim**), `ridl_explain` (error index), `ridl_diff` (0/1/2 + breaking list) | E2    | outputs are byte-identical to CLI/LSP (one diagnostic SSOT)              | M    |
| E8.7 | Grounding / IR-query tools — `ridl_describe_type`, `ridl_list_interactions`, `ridl_resolve`                                                      | E2    | return real IR data; agent cites existing symbols, not hallucinated ones | M    |
| E8.8 | CLI fallback face — `ridl` subcommands cover the MCP tool semantics for headless/cron hosts lacking interactive MCP                              | E2/E4 | same results obtainable via CLI (ADR-0005 open q)                        | S    |

_Evals (part of the deliverable, not an afterthought)_

| ID    | Story                                                                                                                                                                                                              | Rides          | Done when                                                          | Size |
| ----- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------- | ------------------------------------------------------------------ | ---- |
| E8.9  | Eval corpus — natural-language-spec → expected-RIDL tasks, scored by `ridl_check` (compiles) + `ridl_diff` (intended category for evolution tasks); lives beside the snapshot corpus (E1.18), runs in the CI plane | E1 (seed) → E2 | corpus runs in CI; skill/language edits that regress it are caught | M    |
| E8.10 | Idiomaticity scoring beyond "compiles" — rubric or LLM-judge over a golden set (signal-vs-event, error composition)                                                                                                | E2             | idiomatic vs merely-valid distinguished (ADR-0005 open q)          | M    |

_Generation & portability (V1 tail)_

| ID    | Story                                                                                                                                                          | Rides           | Done when                                                      | Size |
| ----- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------- | -------------------------------------------------------------- | ---- |
| E8.11 | Skill generation from `ridl doc` / references — skill tables + examples become a `ridl doc` emit target so the skill **cannot drift**; hand-forking disallowed | E4 (needs E4.1) | skill regenerates from specs; a drift check runs in CI         | M    |
| E8.12 | Rules-file portability — one canonical rules text → N host formats (Claude skill / Cursor rules / …), or hand-maintain until a 2nd host demands it             | E4              | canonical text + generator, or documented single-host decision | S    |

_Layer extensions & packaging (V2)_

| ID    | Story                                                                                                                                            | Rides | Done when                                                                         | Size |
| ----- | ------------------------------------------------------------------------------------------------------------------------------------------------ | ----- | --------------------------------------------------------------------------------- | ---- |
| E8.13 | Skill extended to rmdl behaviour — `function`/`model`, `last`/`init`, `when`/`emit`, `case`, realization; flow-kind tables                       | E5    | authors a valid contract-blind `.rmdl` model an rsdl component can bind (rmdl §7) | M    |
| E8.14 | MCP reference-oracle / replay hooks — execute-and-diff (`ridl_run`/`ridl_oracle`); the strongest verify signal, and the behaviour eval oracle    | E5    | agent-generated model executed and tick-diffed vs expected (ADR-0005 open q)      | M    |
| E8.15 | Skill profile for rsdl — components, `provides`/`requires`, application-notation wiring, event→command routing                                   | E6    | authors a valid `.rsdl` assembly                                                  | S    |
| E8.16 | `ridl-architect` subagent — composition of skill + MCP with a built-in verify loop (iterate until `ridl_check` clean and `ridl_diff` compatible) | E7    | completes a multi-interface design task autonomously with green checks            | M    |
| E8.17 | _(deferred, gated)_ Spy/control bridge as an MCP surface for live-system introspection — behind the same security/assurance model                | E7.2  | not started until the bridge exists; gated on assurance labels                    | —    |

---

## Milestone summary

| Epic | Milestone                   | Release                                                                       |
| ---- | --------------------------- | ----------------------------------------------------------------------------- |
| E0   | Walking skeleton            | internal — IR/query graph proven                                              |
| E1   | typl schema language        | **v0.1 preview**                                                              |
| E2   | ridl contract boundary      | v0.x — RIDL-as-today + diff gate                                              |
| E10  | typl value objects          | v0.x — types that cannot hold an invalid value                                |
| E4.5 | IR plugin protocol          | v0.x — the extension seam every domain reaches through                        |
| E11  | `ridl-rt`, the runtime core | v0.x — generated contracts run                                                |
| E12  | the tooling plane           | v0.x — observability, emulation, harness drivers                              |
| E9   | wire SSOT                   | v0.x — schema projections and the schema hash                                 |
| E3   | boundary model              | v0.x — person and world boundaries (ADR-0012)                                 |
| E4   | V1 ecosystem (rest)         | **V1.0 — the contract platform**                                              |
| E5   | rmdl behaviour + oracle     | v2.0-alpha — executable models, replay                                        |
| E6   | rsdl assembly               | v2.0-beta — deployable systems                                                |
| E13  | the gateway                 | v2.0-beta — one contract, two encodings                                       |
| E7   | rxdl + V2 ecosystem         | **V2.0 — the executable platform**                                            |
| E8   | agent enablement            | threads V1→V2 (skill+rules & evals in V1; MCP by E2; oracle & subagent in V2) |

Rows are in sequence order, not numeric order — the numbering is identity, as
the note at the top of this page explains.
