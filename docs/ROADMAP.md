# RIDL Implementation Backlog — Epics & Stories

Companion to ADR-0004 (sequencing + stack) and ADR-0005 (agent enablement). Each
**Epic is a milestone** with its own shippable value and exit criteria;
**Stories** are the work items under it. Sizing is rough (S ≈ days, M ≈ 1–2
weeks, L ≈ 3–6 weeks) and relative, not a schedule.

The release boundary is **descriptive vs executable**:

- **V1 — the contract platform:** E0–E4 (typl · ridl · uxdl · ecosystem). The
  SSOT for system and user contracts, with codegen, LSP, diff, docs.
- **V2 — the executable platform:** E5–E7 (rmdl · rsdl · rxdl). The ambitious,
  higher-risk half, built against a V1 IR and toolchain already hardened by
  three profiles and multiple backends.
- **E8 — agent enablement (ADR-0005):** threads V1→V2 alongside E1–E7 — the
  skill/rules, the MCP over the compiler, evals, and (V2) the behaviour oracle
  and subagent.

Dependency spine: E0 → E1 → E2 → E3 → E4 ‖ E5 → E6 → E7. E4 (ecosystem) and E8
(agents) thread through; their items ship as the layer they ride lands.

**Forward-compatibility constraint (V1 protects V2):** the `expr`/function core
shipped in V1 for `require`/`ensure` (E2.4) must be a genuine forward-compatible
_subset_ of the family `expr` core — the same grammar rmdl's function layer
extends in V2 (E5.1), never a throwaway. Hold this line and rmdl's function
layer is an extension, not a rewrite. E2.12 writes the expr-core specification
that fixes the family grammar this subset is verified against — it lands before
or with E2.4.

**What E5.1 and E7.3 inherit, and the hole in it (recorded at E2 close,
2026-07-26).** E2 carries a contract clause in the IR as canonical source text
(`Contract.source`, ADR-0008 decision 14). E5.1 replaces that with an expression
tree, and E7.3 discharges the same terms deductively; both inherit
`crates/ridlc/tests/corpus/` as the regression set that says a restructured
representation still means what the text meant. **That set does not exercise the
whole subset.** The subset grammar admits thirteen binary operators and two
prefix operators. Of the thirteen binary, four — `<`, `-`, `/`, `%` — appear in
no contract clause that reaches a snapshotted IR; of the two prefix, `!` appears
in none, and `-` appears only on numeric literals (`-10.0`, `-40.0`), never on a
reference. All five are implemented and unit-tested in
`crates/ridl-sem/src/expr.rs` and `crates/ridl-sem/src/expr_eval.rs` — this is a
coverage hole, not a correctness one. `<` is the near miss: the diagnostic
showcase writes it, but that package compiles with errors, so its IR, Rust, and
TypeScript snapshots are one-line placeholders and nothing pins a lowered form.
Widen the corpus before restructuring, so the restructuring has something to
regress against.

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
is complete over IR v1 (exact decimal). Deferred per ADR-0007: E1.8 ships no
`wire` width floor yet (typl §17.11 / ADR-0007 d7) — nominal unit checking
itself ships; of the profile-boundary and doc diagnostics only TYPL-302 ships —
TYPL-301/303/304 and TYPL-107/205/401/402/403 are recorded debt (ADR-0007 d10).
Cutting the v0.1 preview tag is a maintainer act (ADR-0007 d14).

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
ADR-0008: `persist` (d3), the `final` → `fixed`/`provisioned` reconsideration
(d5, still open), the general-form §4.7 promotion of `labels`/`deprecated` to
attributes, and diagnostic codes RIDL-111 and RIDL-142 — reserved by d21 and
still unminted. `ridlc build` emits Rust, the extern-C header, and IR JSON;
TypeScript is produced through the backend library and pinned by the corpus
snapshots, with no CLI emit path. E2 also paid three codes of the E1 debt
ADR-0007 d10 recorded: TYPL-301, TYPL-303, and TYPL-304 ship, emitted by the
parser once the family grammar made the constructs they reject parseable, each
with a showcase entry. E2.10's "alias-not-required" row needed no new work —
TYPL-008 has covered it from the resolver since E1. The consolidated E2 debt
roll-up is **#172**, on the E1 (#135) pattern.

| ID    | Story                                                                                                                                                   | Done when                                                               | Size |
| ----- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------- | ---- |
| E2.1  | `interact` core surface + semantics: `signal`/`event`/`command`/`query`/`final`                                                                         | all five kinds parse, check, resolve payloads                           | L    |
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

## Epic 3 — uxdl (User Interface)

**Milestone:** viewmodel/view SSOT — the second contract profile. **Value:**
user-interaction contracts reusing the Epic 2 machinery; near-free by design,
and a third profile that further hardens the IR before V2. **Exit criteria:** a
`view` binds a ridl contract, checks, and generates viewmodel bindings
(TS/MVVM); the descriptive `.rxdl` slice compiles (E3.5).

| ID   | Story                                                                                                                                                             | Done when                                            | Size |
| ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------- | ---- |
| E3.1 | Surface: `view`/`display`/`input`/`action`/`fetch`/`fixed` refinements as an `interact` profile                                                                   | profile-maps onto E2 core                            | M    |
| E3.2 | `states` + `during` gating                                                                                                                                        | state-scoped availability checks                     | M    |
| E3.3 | Binding codegen: viewmodel/widget bindings (TS first)                                                                                                             | `display <- signal`, `activate -> command` generated | M    |
| E3.4 | LSP + lint for uxdl                                                                                                                                               | hovers/lints on a real view                          | S    |
| E3.5 | `.rxdl` descriptive slice: one file carrying typl + ridl + uxdl declarations — the canonical agent/eval unit (ADR-0005 §7); E7.1 extends it to behaviour + wiring | the cruise-control descriptive `.rxdl` compiles      | S    |

## Epic 4 — Ecosystem & Adoption (V1)

**Milestone:** the contract platform is _approachable_ — a stranger can learn
it, try it, and depend on it. **This rounds out the public V1.0.** **Value:**
the V1 adoption multipliers; each ships when its underlying layer is ready, not
big-bang. **Exit criteria:** error-index site live, playground live,
getting-started + contract tutorial published, IR plugin protocol documented and
versioned.

| ID   | Story                                                                                                    | Done when                                         | Size |
| ---- | -------------------------------------------------------------------------------------------------------- | ------------------------------------------------- | ---- |
| E4.1 | `ridl doc`: interfaces/views rendered as tables/HTML                                                     | doc output for a real package                     | M    |
| E4.2 | Error-index website: every `TYPL-`/`RIDL-`/`UXDL-` code with explanation + fix (rustc `--explain` style) | codes cross-link from diagnostics                 | M    |
| E4.3 | `.typl`+`.ridl`+`.uxdl` getting-started + contract tutorial (types → interface → view)                   | a newcomer compiles it unaided                    | M    |
| E4.4 | Browser playground: compiler-to-WASM, live edit→codegen                                                  | edit typl/ridl/uxdl, see generated output in-page | L    |
| E4.5 | IR plugin protocol spec + versioning (IR stability policy)                                               | a third-party backend consumes the IR             | L    |
| E4.6 | `ridl init`/`ridl new` scaffolding + `ridl vendor` (air-gap)                                             | scaffolds a valid workspace; vendors deps         | S    |
| E4.7 | Governance CI: keyword-registry + attribute-registry collision test                                      | colliding key across profiles fails CI            | S    |

---

# V2 — The Executable Platform

## Epic 5 — rmdl (Behaviour)

**Milestone:** executable behaviour with a working reference oracle. **Value:**
generated processing code for contract-blind models — pure reactions that rsdl
components bind to contracts in E6 (rmdl §7); deterministic replay/oracle
machinery. The novel, hard core — built on a proven IR. **Exit criteria:** the
cruise-control model computes its reaction, runs native and as a WASM component
with identical step traces, and the oracle diffs tick-by-tick against an
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
| E5.11 | `jco` browser path for uxdl-side execution                                                                                                                                                           | component runs in a browser host                                   | M    |
| E5.12 | Minimal `ridl-rt` scheduler/timeline (input + deadline activation, coalescing)                                                                                                                       | reactive stepping, quiescent when idle                             | L    |
| E5.13 | `ridl.std.flow` / `std.control` Tier-1 adapters (rmdl-defined)                                                                                                                                       | Hold/Changes/Filter/Accumulate/Deadband/Latch compile              | M    |

## Epic 6 — rsdl (System Assembly)

**Milestone:** a system is assembled from components and its deployment
artifacts generated. **Value:** components situate the contract-blind reactions
E5 delivers — binding them to services, wiring event→command side effects, and
deriving transport, posture, and deployment from the SSOT. **Exit criteria:**
the cruise-control system (interface + model + view) assembles from `.rsdl`
components — composition and deployment as two regions of one grammar (rsdl §2)
— and emits topology + integration artifacts.

| ID    | Story                                                                                                                                                                                     | Done when                                                                    | Size |
| ----- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- | ---- |
| E6.1  | `component` declarations: `provides`/`requires` boundary at three grains (inline / interface / service), leaf vs composite (rsdl §3)                                                      | components parse and check; all three boundary grains resolve                | L    |
| E6.2  | Application-notation wiring: applications as instances, fused `provides … = …`, destructuring, `let` intermediates; composite cycles legal, leaf sync cycles rejected (rsdl §4, RSDL-407) | cruise-control wiring compiles; a leaf-level sync cycle is rejected          | M    |
| E6.3  | Cross-layer resolution (references to typl/ridl/uxdl/rmdl)                                                                                                                                | instance typing + binding resolve                                            | M    |
| E6.4  | Contract binding: service member completeness (RSDL-303), timing transfer + init-consistency boundary checks (moved out of rmdl — rmdl §7), declared redundancy (RSDL-502) (rsdl §5)      | every provided member covered; an accidental second provider fails the build | M    |
| E6.5  | Event→command wiring (rmdl §5.7, rsdl §4.2, RSDL-405)                                                                                                                                     | emitted event wires to a command                                             | S    |
| E6.6  | `system` root: external boundary, assurance profile, one per workspace (rsdl §6)                                                                                                          | the system compiles as the root component                                    | S    |
| E6.7  | `deployment` region: targets by capability class, complete placement (RSDL-701), time base (rsdl §7)                                                                                      | every instance placed or RSDL-701 fires                                      | M    |
| E6.8  | Transport + posture derivation: static vs discovered per connection, physics constraint (RSDL-803), contract-timing feasibility (RSDL-801) (rsdl §8)                                      | one composition deploys both postures; infeasible timing rejected            | L    |
| E6.9  | Bundles: versioned distribution artifacts, `tier` dependency gating (rsdl §9, RSDL-901)                                                                                                   | bundle manifests emitted for the cruise-control system                       | M    |
| E6.10 | Topology + integration-artifact emission                                                                                                                                                  | deployable manifest/topology generated                                       | M    |
| E6.11 | Test topology as a `deployment`: injectors/oracle swapped in (rsdl §2)                                                                                                                    | rest-bus-style test deployment derived                                       | M    |

## Epic 7 — rxdl & Executable-Platform Ecosystem

**Milestone:** the total profile and the behaviour-dependent ecosystem —
completing the public V2.0. **Value:** the single-file adoption gradient, the
full four-/five-way verification story, and the registry. **Exit criteria:** an
`.rxdl` file carries types + interface + model + wiring and compiles; the
reference-oracle test plane and deductive-proof path work; registry publishes
and resolves.

| ID   | Story                                                                                                                                           | Done when                                                          | Size |
| ---- | ----------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------ | ---- |
| E7.1 | `.rxdl` total profile: extend the E3.5 descriptive slice with the behaviour + wiring layers; per-package profile purity enforced in `ridl.toml` | full mixed-layer file compiles; purity is a policy error where set | M    |
| E7.2 | Reference-oracle test plane: spy/control bridge as a generated interface, online observers                                                      | live flows spied/asserted from contracts                           | L    |
| E7.3 | Deductive-proof verification path (Creusot-compatible `expr` discharge)                                                                         | a provable contract discharged deductively                         | L    |
| E7.4 | Package registry service (separate repo/lifecycle)                                                                                              | publish + resolve a remote package                                 | L    |
| E7.5 | Full `.rxdl` getting-started + end-to-end tutorial (types → interface → model → wiring)                                                         | a newcomer builds the whole cruise-control system unaided          | M    |
| E7.6 | Error-index website extended with `RMDL-`/`RSDL-` codes (completes E4.2)                                                                        | every V2 code has an explanation + fix entry                       | S    |

---

# Cross-Cutting

## Epic 8 — Agent Enablement (ADR-0005, threads V1→V2)

**Milestone:** an AI agent turns a natural-language spec into correct, idiomatic
RIDL and evolves it _provably_ (compiles clean, `ridl diff` compatible) — for
types and interfaces in V1, behaviour in V2. **Value:** RIDL is niche, so a
model has ~zero priors; the knowledge layer is the highest-leverage, cheapest
piece and needs no compiler. The MCP is near-free because it reuses the IR,
diagnostic SSOT, and `ridl-diff` built for other consumers (ADR-0005 "one
engine, three faces"). **Exit criteria (V1 slice):** skill + rules author valid
typl/ridl; MCP `ridl_check`/`ridl_explain`/`ridl_diff` + IR-query tools back a
verify/evolve loop; the eval corpus runs in CI. **V2 slice:** behaviour skill +
oracle eval; `ridl-architect` subagent.

Sequencing note — ADR-0005 §8 maps agent work onto the old Phase 1–5; under the
V1/V2 re-cut those become: typl→E1, ridl→E2, uxdl→E3, `ridl doc`→E4, rmdl→E5,
rsdl→E6, family-whole→E7. Each story below carries the epic it rides.

**Preserve, don't build (ADR-0005 §7 invariants — constraints on other epics):**
every diagnostic stays coded + fix-it (E1.10); `.rxdl` is the canonical agent
target and eval unit (E3.5 descriptive slice in V1, E7.1 total in V2); sigil
poverty is kept; IR + diagnostic-code + `ridl-diff` stability _is_ the
agent-contract stability (E4.5 / IR-stability open question).

_Layer A — Knowledge (skill + rules; build first, no compiler dependency)_

| ID   | Story                                                                                                                                                                                                                                     | Rides                 | Done when                                                                                                   | Size |
| ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------- | ----------------------------------------------------------------------------------------------------------- | ---- |
| E8.1 | Rules file — 10–20 always-on "never/always" constraints, distilled from doctrines + the _error_ diagnostics (no semicolons, named-typl payloads, errors-as-data, command≠query, no inheritance, no upward refs, append-only + `reserved`) | E1 (may precede code) | every rule cites a diagnostic code or doctrine; loads in Claude Code/Cursor/Cowork                          | S    |
| E8.2 | Skill v0 (typl) — dense decision tables + worked examples for types/ranges/units/evolution, per `skill-ridl-authoring-outline.md`                                                                                                         | E1                    | authors valid `.typl`; content traceable to the typl reference                                              | M    |
| E8.3 | Skill extended to the ridl `interact` core — 5-kind selection table, timing, errors-as-data / `T\|E`, common-mistakes table keyed to codes                                                                                                | E2                    | covers ridl ref §3–§10; cruise-control example round-trips clean (`.rxdl` descriptive form once E3.5 lands) | M    |
| E8.4 | Skill profile for uxdl — `view`/`display`/`action` over the interact core                                                                                                                                                                 | E3                    | authors a valid `.uxdl` view bound to a ridl contract                                                       | S    |

_Layer B — Capability (MCP over the compiler; build second, cheap given
ADR-0004)_

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

| Epic | Milestone               | Release                                                                       |
| ---- | ----------------------- | ----------------------------------------------------------------------------- |
| E0   | Walking skeleton        | internal — IR/query graph proven                                              |
| E1   | typl schema language    | **v0.1 preview**                                                              |
| E2   | ridl contract boundary  | v0.x — RIDL-as-today + diff gate                                              |
| E3   | uxdl view SSOT          | v0.x — second contract profile                                                |
| E4   | V1 ecosystem            | **V1.0 — the contract platform**                                              |
| E5   | rmdl behaviour + oracle | v2.0-alpha — executable models, replay                                        |
| E6   | rsdl assembly           | v2.0-beta — deployable systems                                                |
| E7   | rxdl + V2 ecosystem     | **V2.0 — the executable platform**                                            |
| E8   | agent enablement        | threads V1→V2 (skill+rules & evals in V1; MCP by E2; oracle & subagent in V2) |
