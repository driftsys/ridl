# Roadmap — Landed Record

This file holds the landed-work record extracted from `docs/ROADMAP.md` by the
2026-09-12 re-scope. It is history: what shipped, in which pull request, and
with which amendment, for the epics that are complete. The forward plan is in
[`../ROADMAP.md`](../ROADMAP.md).

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
[ADR-0007](../decisions/ADR-0007-e1-execution.md): E1.8 ships no `wire` width
floor yet (typl §17.11 / [ADR-0007](../decisions/ADR-0007-e1-execution.md) d7) —
nominal unit checking itself ships; of the profile-boundary and doc diagnostics
only TYPL-302 ships — TYPL-301/303/304 and TYPL-107/205/401/402/403 are recorded
debt ([ADR-0007](../decisions/ADR-0007-e1-execution.md) d10). Cutting the v0.1
preview tag is a maintainer act
([ADR-0007](../decisions/ADR-0007-e1-execution.md) d14).

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
[ADR-0008](../decisions/ADR-0008-e2-execution.md): `persist` (d3) and the
general-form §4.7 promotion of `labels`/`deprecated` to attributes, and
diagnostic codes RIDL-111 and RIDL-142 — reserved by d21 and still unminted.
`ridlc build` emits Rust, the extern-C header, IR JSON, and TypeScript. The epic
itself shipped the TypeScript backend as a library only, pinned by the corpus
snapshots and reachable from no command; the `--emit typescript` path landed
afterwards, as a prerequisite for E3.3 (driftsys/ridl#172).

**The interaction layer this epic shipped is retracted by
[ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md) decision
15.** The exit criterion above was met literally and by output that no runtime
can implement: the interaction vocabulary is emitted once per package, so two
packages produce two incompatible `Provenance` types and no runtime crate can
implement a trait that does not exist until codegen runs. The layer does compile
— `corpus.rs` runs `rustc` and `tsc` over it with anti-vacuity guards — but
compiling proves syntax, not that anything can implement it. Epic 11 restores
the face as a client and a server over a runtime that exists.

E2 also paid three codes of the E1 debt
[ADR-0007](../decisions/ADR-0007-e1-execution.md) d10 recorded: TYPL-301,
TYPL-303, and TYPL-304 ship, emitted by the parser once the family grammar made
the constructs they reject parseable, each with a showcase entry. E2.10's
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

**Design of record** — four notes, archived in `docs/archive/` once their
records were ratified, all sharing one origin:
[`2026-08-03-ir-protobuf-encodings-design.md`](2026-08-03-ir-protobuf-encodings-design.md)
·
[`2026-08-03-rpc-response-bound-design.md`](2026-08-03-rpc-response-bound-design.md)
·
[`2026-08-03-multi-interface-services-design.md`](2026-08-03-multi-interface-services-design.md)
·
[`2026-08-03-schema-projection-design.md`](2026-08-03-schema-projection-design.md).
Three ADRs fall out: **ADR-0014** (IR encodings, superseding
[ADR-0004](../decisions/ADR-0004-implementation-sequencing-and-stack.md) §4's
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

[ADR-0014](../decisions/ADR-0014-ir-encodings.md) is complete: canonical
protobuf JSON replaced the `serde` rendering on every surface, prototext and
binary joined it as emits, and the `ridl.std` filter became an exhaustive
classification over `Emit`. The JSON mechanism then moved again, from
`prost-reflect` to `pbjson`-generated impls (decision 14): the transcode carried
prost's non-configurable recursion limit, which made `--emit ir-json` fail on
legal source that three other emits handled. Output is byte-identical, so no
golden changed.
[ADR-0015](../decisions/ADR-0015-qos-absorption-and-rpc-bounds.md) is complete
for this block: `command` and `query` carry the range form with RIDL-112 warning
an undeclared response bound, the coherence rule is normative prose in ridl
§14.5, and a service composes several interfaces with per-interface ordinals
keyed by name. On 2026-08-05,
[ADR-0016](../decisions/ADR-0016-schema-projection-and-the-name-transform.md)
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
[ADR-0013](../decisions/ADR-0013-codegen-backend-scope.md) decision 7 the same
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
[ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md) decision
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

**E9.9 landed 2026-08-09** as `ridl-backend-flatbuffers`, the second wire
backend, under the same ceiling as E9.8: the typl surface plus the interaction
identity table, no `service` block, no call face, no value store. Its rules are
recorded in [ADR-0019](../decisions/ADR-0019-flatbuffers-projection-rules.md),
every one of them measured against `flatc` 25.12.19 and `planus` 1.3.0 rather
than reasoned from the records: a union is isolated in a wrapper table holding a
native union, a union arm that is not itself a table is boxed in a generated
one, a struct is always a `table`, a map is a vector of entry tables with no
`(key)`, the collision guard models FlatBuffers' own three name scopes, and a
field whose enum declares no zero member takes `= null`. Two records were
amended in consequence: typl Appendix D's fixed-layout `struct` allowance is not
taken by this projection, because the form fabricates a value from padding after
a compatible append; and ADR-0013 decision 6's width-floor precondition was
closed by decision — `ridl-diff` remains the sole guard for v0.1, with the
measured cost of always-widest (2.2× on an 8-signal table, 2.6× on a 7-field
fixed-layout struct) and the forcing case that reopens typl §17.11 recorded in
the amendment. Validity is established by compiling every emitted schema with
`planus-translation`, which parses every construct this projection emits but not
every name — a name that reaches one of the nine words `planus` reserves is
emitted as-is, accepted by `flatc`, and not checked by the oracle (ADR-0019
decision 7) — and the stability property is driven from `ridl-diff`'s
classifier, where it additionally guards the `(deprecated)` slot filling.

Two questions this story would have passed to E9.11 are already settled above.
The ADR-0013 decision 2 versus ADR-0016 decision 10 conflict over the `service`
block — which E9.9 avoided the same way E9.8 did, by emitting neither — is
resolved by ADR-0018 decision 18, exactly as the E9.8 section records: decision
2 holds with its scope made explicit, decision 10 holds unqualified. And `(key)`
plus sorted-vector lookup is reopenable in E11.7, the FlatBuffers codec, where a
generated producer could hold the sortedness obligation the attribute implies
and nothing in E9.9 could (ADR-0018 decision 16 moved that work into Epic 11).

| ID   | Story                                                                                                                                                                                                                                                                 | Done when                                                                                                                               | Size |
| ---- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- | ---- |
| E9.1 | `ridl-ir` serialization rewrite — the emitted `.ir.json` is a serde rendering of Rust structs, not protobuf JSON, so no non-Rust protobuf runtime can parse it (ADR-0014)                                                                                             | a non-Rust protobuf runtime parses the emitted IR                                                                                       | L    |
| E9.2 | Two new emit values — prototext and binary — alongside canonical protobuf JSON                                                                                                                                                                                        | all three encodings round-trip to the same IR                                                                                           | M    |
| E9.3 | The latent emit-filter defect the new emits expose (§5.1 of the encodings note)                                                                                                                                                                                       | the filter is correct for every emit value                                                                                              | S    |
| E9.4 | **RPC bounds and the response bound** — ridl §9 gains its RPC column; RIDL-112 minted for a missing bound; RIDL-106 narrows to `fixed`; RIDL-103 widens (ADR-0015)                                                                                                    | an RPC declares a response bound; an undeclared one warns, profile-escalable                                                            | M    |
| E9.5 | The **coherence rule** — coherence is implicit and the interface is the generation unit — as normative prose in ridl §14                                                                                                                                              | the rule is stated where the service definition it keys on lives                                                                        | S    |
| E9.6 | **Multi-interface services** — `ServiceDef` carries several interfaces; ordinals stay per-interface keyed by name; addressing stays flat; duplicate member is an error                                                                                                | a service composes two interfaces; reordering the list leaves transport identity untouched and diffs as breaking (ADR-0015 decision 19) | L    |
| E9.7 | The **projection contract** and its pinned name transform — the collision rule (RIDL-149) replaces the injectivity requirement; the divergent `interact.rs` implementation is deleted in favour of `c_header.rs`'s; specified with E4.5's stability policy (ADR-0016) | one transform, in `ridl-ir`; a package whose member or parameter names collide after it is rejected                                     | M    |
| E9.8 | **proto3 projection** — schemas from IR identity, per ADR-0013's shape-and-identity ceiling                                                                                                                                                                           | the cruise-control package emits valid proto3                                                                                           | L    |
| E9.9 | **FlatBuffers projection** — the column Appendix B was missing. The _schema_ emit; the FlatBuffers _codec_ for the store and queue is E11.7                                                                                                                           | the cruise-control package emits a valid FlatBuffers schema                                                                             | L    |

## Tracker correspondence

**Reconciled 2026-08-09.** 44 issues closed (E0.1–E2.11 as delivered; the five
retired `uxdl` stories and the C header defect as not planned), 12 retitled
where a row changed meaning, and 56 opened for the stories that never had one —
E3, E6.2/E6.4/E6.6–E6.9, E7.6–E7.9, E9.9/E9.10/E9.12, E10, E11, E12 and E13. The
reasoning is on the two epic debt roll-ups, driftsys/ridl#135 and
driftsys/ridl#172, which stay open because they hold carried findings the
stories did not deliver.
