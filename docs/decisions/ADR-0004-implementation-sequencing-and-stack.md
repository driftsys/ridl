# ADR-0004: Implementation Sequencing and Toolchain Stack

## Status

Proposed.

Assumes ADR-0002 (module system) as accepted and ADR-0003 (the family decision —
profiles, cores, platform model) as the direction of record even though 0003 is
not yet written; this ADR depends only on the concept note's §7–§8 conclusions,
which 0003 will formalize.

The Epic/Story breakdown of §1 lives in the companion
`implementation-backlog.md`.

## Context

The RIDL family is specified at the language level — typl, ridl, uxdl, and rmdl
each have a reference draft, the general-form spec fixes the shared surface, and
ADR-0002 fixes the module system. What is _not_ yet decided is how the platform
gets built: in what order the layers and tools are implemented, and which
concrete Rust libraries the compiler, IR, language server, and codegen backends
are built on.

The concept note (§8) already locks the frame: Rust, compiler-as-library, one
platform monorepo, a stable serializable IR as the centre of gravity, and an
asymmetric backend matrix (bindings everywhere, behaviour twice — Rust native +
WASM). This ADR fills in the two things that concept note deliberately left open
and that are expensive to change once code exists:

1. **Sequencing.** The lattice says "bottom-up" (typl → ridl → rmdl → uxdl →
   rsdl), but a naïve reading — finish each layer before starting the next —
   front-loads the wrong risk. The real risk sits in the IR and the incremental
   query graph (which only reveal their flaws under cross-layer pressure) and in
   rmdl (the sole executable, genuinely novel layer). The family also has a
   natural product seam — **descriptive contracts vs executable behaviour** —
   that the sequence should exploit.
2. **The stack.** "Rust" is not a stack. Parser strategy, the incremental
   engine, the IR serialization, the diagnostics model, the LSP framework,
   codegen, and the numeric core each carry a decision whose reversal is a
   rewrite, not a refactor.

The design constraints from ADR-0002 carry over unchanged: _strict beats
flexible_, _small surface_, _faithful codegen mapping_. Two more apply
specifically to implementation:

- **The compiler is a library first.** `ridl` and `ridlc` are thin binaries over
  shared crates; the LSP, `ridl-diff`, and the test plane are first-class
  consumers of those crates, never side-band reparsers. Any library choice that
  forces a second parse or a divergent semantic model is disqualified.
- **The IR is polyglot.** Backends exist in Rust, Kotlin, and TypeScript. Any
  artifact those backends consume must be readable from all three languages, not
  just Rust.

## Decision

### 1. Build sequence — two releases along the descriptive/executable seam

Implementation splits into two releases at the family's natural product
boundary. **V1 ships the descriptive contract platform; V2 ships the executable
platform.** Within each release the ordering is IR-first — the sequence puts
maximum cross-layer pressure on the IR as early and as cheaply as possible,
rather than completing language layers in isolation.

Epics map to milestones (full stories in `implementation-backlog.md`):

**V1 — the contract platform**

- **E0 — Walking skeleton.** Before any layer is finished, push one trivial
  `.typl` program (one `type`, one `const`) end to end: lex → parse → resolve →
  check → IR → generated Rust. Throwaway quality; the sole purpose is to stress
  the IR shape and the salsa query graph while both are cheap to change.
- **E1 — typl + the tooling spine.** Full lexer, hand-written lossless parser,
  the ADR-0002 resolver (manifest, lockfile, cache), and the hard typl
  semantics: exact range/step arithmetic, wire-width derivation, nominal unit
  checking, init/default derivation. IR frozen at v1. One backend (Rust +
  extern-C). Then the spine — `ridlc`, the `ridl` facade, `ridl fmt`, first
  `ridl-lsp`. Ships typl as a standalone units-aware schema language (v0.1
  preview): the first external validation.
- **E2 — ridl.** The `interact` core (`signal`/`event`/`command`/`query`/
  `final`), generic min/max timing, errors-as-data with inline `T | E`, and the
  `expr` **guaranteed subset** for `require`/`ensure`. A **second backend**
  (proto or TypeScript) is added here specifically to force the IR to prove it
  is language-neutral. `ridl diff` (exit codes 0/1/2) lands here. Result: RIDL
  as it exists today, with codegen and an evolution gate.
- **E3 — uxdl.** Deliberately cheap: a second profile over E2's `interact` core
  — `view`/`display`/`input`/`action`, `states`/`during`, binding codegen.
  Pulling it into V1 also adds a **third profile** that hardens the IR before
  the executable layer commits to it.
- **E4 — V1 ecosystem.** `ridl doc`, the coded error-index website, the contract
  getting-started + tutorial (types → interface → view), the browser playground,
  the IR plugin protocol + stability policy, scaffolding, and the
  registry-collision governance test. **Completes the public V1.0.**

**V2 — the executable platform**

- **E5 — rmdl.** The novel, hard work: the function/`expr` core, causality
  analysis, the scheduled-step/timeline model, `last`/`init` seeding, Rust +
  WASM component codegen, and the wasmtime reference-oracle. Sits on E2's
  `interact` core and its function layer **extends E2.4's expr subset**. Does
  not begin until the IR has survived three profiles and two backends.
- **E6 — rsdl (manifest-first).** Instances, bindings (incl. event→command from
  rmdl emissions), deployment, and test-topology emission. Config-shaped; no
  grammar profile until topology earns it.
- **E7 — rxdl + V2 ecosystem.** The total single-file profile, the
  reference-oracle test plane (spy/control bridge, online observers), the
  deductive-proof path, the package registry, and the full end-to-end tutorial.
  **Completes the public V2.0.**

**Rationale.** The lattice ordering is correct for _dependency_; it is
misleading for _risk_ and for _product_. Three forces shape the split: (a) the
IR, not any single layer, is what every backend and tool depends on, so the IR
is pressured early (E0 skeleton, second backend in E2, third profile in E3); (b)
rmdl concentrates all the ambition and execution risk, so deferring it to V2
lets that work land against a toolchain and IR already hardened by the entire
descriptive half; (c) the descriptive/executable seam is a real product boundary
— V1 is a complete, valuable IDL platform (system + user contracts, codegen,
LSP, diff, docs) that competes without needing the engine, and V2 adds the
differentiated executable layer on top.

**Forward-compatibility constraint.** The `expr`/function core shipped in V1 for
`require`/`ensure` (E2.4) must be a genuine forward-compatible _subset_ of the
family `expr` core — the same grammar rmdl's function layer extends in V2
(E5.1), never a throwaway. This is the one V1 decision that would be expensive
to get wrong for V2; the language specs already mandate a shared expr core, so
this is a matter of holding the line, not new design.

### 2. Front-end: `logos` + hand-written parser + `rowan`

| Concern     | Choice                         | Rejected                               |
| ----------- | ------------------------------ | -------------------------------------- |
| Lexer       | `logos`                        | hand-written DFA                       |
| Parser      | hand-written recursive descent | `chumsky`, `winnow`/`nom`, LALRPOP     |
| Syntax tree | `rowan` (lossless red/green)   | typed-only AST, parser-generator trees |

**Rationale — hand-written parser over combinators.** `ridl fmt` and the LSP
both need error recovery and a full-fidelity tree over _broken_ source. A
hand-written recursive-descent parser gives direct control over recovery points
and diagnostics; combinator and generator frameworks optimize for authoring
convenience at the cost of recovery control. This is the rust-analyzer playbook,
and it composes with the rest of the stack.

**Rationale — rowan.** Lossless red/green trees preserve trivia (whitespace,
comments), which is what makes `ridl fmt` round-trip faithfully and lets the LSP
operate on incomplete code. Accepted cost: rowan trees are untyped, so a thin
typed-AST accessor layer is written over them — generated from an
`ungrammar`-style grammar description, as rust-analyzer does, to avoid
hand-maintaining accessors.

**Rationale — logos.** Derive-macro lexer compiling to a fast DFA, actively
maintained. Requirement carried forward: for rowan's losslessness the lexer must
emit trivia as real tokens rather than skipping them.

### 3. Incremental engine: `salsa`

The demand-driven, memoized query engine underlying both `ridlc` and `ridl-lsp`.

**Rationale.** Salsa gives incremental recomputation and memoization for free,
which is the mechanism that lets `ridl check` and the language server share
_one_ computation instead of being two implementations of the same analysis —
the "compiler is a library first" constraint made real. rust- analyzer and Ruff
both run on it at production scale.

**Accepted trade-off.** Salsa is pre-1.0 (0.27 as of mid-2026) and its 0.x line
breaks across releases; its own README still reads "work in progress." This
reflects API-polish caution, not runtime instability. Mitigation: pin an exact
version, treat a salsa migration as a scheduled cost once or twice over the
project's life, and do not fight its synchronous model (see §6). The alternative
— a hand-rolled rustc-style query system — is months of work a small team should
not spend.

### 4. IR serialization: protobuf via `prost`

The canonical, stable IR — the plugin-protocol wire format — is a protobuf
schema compiled with `prost`. A `serde`→JSON rendering of the same types is kept
for debugging and golden tests; an internal binary cache format
(`postcard`/`bincode`) is permitted as a Rust-only fast path but is never the
canonical artifact.

| Alternative                           | Why rejected                                                                                                                       |
| ------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| `bincode` / `postcard` / serde-native | Rust-only; Kotlin/TS backends cannot read it without reimplementing the format                                                     |
| Cap'n Proto / FlatBuffers             | value is zero-copy mmap; a compiler reads its IR once, so the cost (worse cross-language tooling, clunkier evolution) buys nothing |
| JSON / CBOR / MessagePack             | schemaless — fine as a debug view, not a typed versioned contract between compiler and plugins                                     |

**Rationale.** The IR is polyglot by requirement, which eliminates every
Rust-only option as the _canonical_ form. protobuf has a language-neutral schema
every backend language can compile, mature tooling, and — the clincher — a
field-number evolution model that is the same philosophy as typl's ordinal
evolution and the `ridl-diff` gate. It is also exactly the "descriptor set / buf
image" precedent the concept note cites. `prost` produces idiomatic Rust with no
runtime reflection; `prost-reflect` is available if a reflection path
(`ridl doc`) later wants it.

### 5. Diagnostics: own `Diagnostic` model + `codespan-reporting` renderer

Diagnostics are a first-class homegrown struct — coded (`TYPL-405`), severity,
primary and secondary spans, labels, optional fix-its — held as the single
source of truth and accumulated in a `Vec`, never modeled as an error return.
That struct maps two ways: to LSP `Diagnostic` for editors, and to a terminal
renderer for the CLI. Terminal rendering uses `codespan-reporting` (`ariadne` is
the prettier alternative, chosen purely on output aesthetics).

**Rationale — not `miette` as the core model.** miette is built around
`std::error::Error`: diagnostics _are_ the error type. That is ideal for a CLI
that fails with one rich error and wrong for a compiler that accumulates
hundreds of diagnostics and must hand them all to the LSP as structured data.
Welding the diagnostic model to the error trait would fight both the
accumulation pattern and the LSP consumer. miette would only be right if the CLI
were the sole consumer, which it is not. `codespan-reporting` is the
battle-tested renderer used across the Rust compiler ecosystem; keeping it as a
pure renderer over the homegrown struct preserves the struct as the SSOT for
every consumer.

### 6. LSP framework: `lsp-server`

The language server is built on `lsp-server` (rust-analyzer's minimal
synchronous transport-and-dispatch crate), owning its own main loop that calls
directly into salsa queries.

| Alternative                         | Status / why rejected                                                                          |
| ----------------------------------- | ---------------------------------------------------------------------------------------------- |
| original `tower-lsp`                | effectively unmaintained (last meaningful release ~2023), outdated LSP types                   |
| `tower-lsp-server` (community fork) | maintained revival; acceptable _only if_ tower-lsp ergonomics are wanted                       |
| `async-lsp`                         | actively maintained, good middleware model; adds an async layer that fights salsa's sync model |
| **`lsp-server`**                    | **chosen** — sync loop, salsa-native cancellation, lowest magic                                |

**Rationale.** Salsa is synchronous and a language server's heavy work is
CPU-bound analysis, not I/O — so an async framework buys little and actively
complicates cancellation, which salsa already models. `lsp-server` is a
synchronous loop that dispatches straight into salsa queries with salsa-driven
cancellation, exactly how rust-analyzer is built. Given rowan+salsa are already
adopted, `lsp-server` is the coherent, lowest-magic choice. `async-lsp` is the
fallback if batteries-included routing/middleware is later judged worth an async
layer; both flavors of tower-lsp are passed over.

### 7. Codegen

| Target                      | Tooling                  | Rationale                                                                                                                   |
| --------------------------- | ------------------------ | --------------------------------------------------------------------------------------------------------------------------- |
| Rust (+ extern C)           | `quote` + `prettyplease` | build a `TokenStream`, format without shelling to rustfmt; the prost-proven combo. `syn` only if constructing via typed AST |
| proto / Kotlin / TypeScript | `minijinja`              | runtime template loading                                                                                                    |
| behaviour (rmdl)            | see §8                   | —                                                                                                                           |

**Rationale — `minijinja` over `askama` for text targets.** minijinja loads
templates at runtime, so an out-of-tree or plugin backend can ship its own
template directory without recompiling `ridlc` — which the IR-consumer/plugin
model requires. askama compiles templates into the binary; its compile-time
template checking is real but redundant here, since generated-output correctness
is guaranteed by `insta` snapshots (§9), and its build-time coupling forecloses
plugin backends. minijinja is Jinja2-compatible and well-maintained. The Rust
backend uses quote/prettyplease, not templates.

### 8. Behaviour and WASM: `wasmtime` + `wit-bindgen` + `cargo-component` (+ `jco`)

For E5 (rmdl, V2): `wasmtime` is the embedded component-model runtime backing
the reference-oracle and replay machinery; `cargo-component` builds rmdl → Rust
→ WASM component; `wit-bindgen` generates the WIT-derived interface from the
contract. `jco` transpiles the same component to run in JS/the browser, which is
how one rmdl WASM build runs beside a uxdl view. The WIT surface is designed
jco-friendly from the start. (Noted here in V1 because it constrains the IR and
WIT-mapping decisions the descriptive half must not foreclose.)

**Rationale.** This is the concept note's "behaviour twice — Rust native + WASM"
realized with the Bytecode Alliance's own toolchain; wasmtime embeds natively in
Rust, so the oracle needs no external process, and the WASM build is what
collapses the backend matrix (Kotlin/TS get bindings, never behaviour codegen).

### 9. Numeric core, package fetch, testing

| Concern                  | Choice                           | Rationale                                                                                                                                                                               |
| ------------------------ | -------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Exact arithmetic         | `num-bigint` + `num-rational`    | wire-width derivation, range/step and boundary math need exact rationals; floats would silently miscompute widths. `malachite` only if bignum profiling later demands it                |
| URL-import fetch         | `ureq`                           | synchronous, minimal dependency tree; `ridlc` core is sync (salsa), so dragging `tokio` in via `reqwest` is unjustified                                                                 |
| Snapshot tests           | `insta` (or `expect-test`)       | snapshot IR, diagnostics, and generated code against a corpus — the guard against silent multi-backend regressions. `expect-test` is the rust-analyzer-native inline alternative; taste |
| Property tests           | `proptest`                       | both test infrastructure and the shipped "typl ranges _are_ generators" feature                                                                                                         |
| CLI / manifest / hashing | `clap` / `toml`+`serde` / `sha2` | ADR-0002 lockfile is SHA-256 content-hashed                                                                                                                                             |

**Rationale — `ureq` over `reqwest`.** The compiler front-end is synchronous
around salsa; the LSP uses async only at its transport edge. Pulling an async
HTTP client and its runtime into `ridlc` solely to download packages is weight
with no payoff. `ureq` fetches URL imports synchronously with a small dependency
footprint.

### 10. Ecosystem features and when they land

Grouped by ring; the first two rings are table stakes for a usable V1, the last
two are adoption multipliers. Mapped to epics in `implementation-backlog.md` (V1
ecosystem = E4; behaviour-dependent items = E7).

**Inner loop (E1–E3, mandatory):** `ridl fmt` (tight-colon, rowan-based,
diff-minimal); `ridl-lsp` with diagnostics, hover showing units/ranges,
go-to-def, find-refs, completion, rename, semantic tokens, and the designed
inlay hints (ordinal visibility, unit expansion); fast `ridl check`. VS Code
extension first; Zed/nvim follow over LSP.

**Quality & CI (E2):** `ridlc --frozen`; `ridl diff` as the breaking-change
gate; `ridl lint` (convention checks — alias-not-required-by-collision,
canonical `T | E` form, model-should-be-function); `ridl test` driving generated
property tests; the coded-diagnostic **error index as a website**
(rustc-`--explain` style, every code with explanation and fix).

**Docs & learning (E4):** `ridl doc` (interfaces/views rendered as tables — the
tabular reading the fmt decision deferred to tooling); a contract
getting-started (types → interface → view); a **browser playground** compiling
the compiler itself to WASM for live edit-and-see-codegen — near-free given the
WASM story and high-leverage for a niche language. The full `.rxdl`
types→interface→model→wiring tutorial waits for V2 (E7), when its subject
exists.

**Extensibility & governance (E4):** the IR plugin protocol (backends as IR
consumers behind one boundary); `ridl init`/`ridl new` scaffolding;
`ridl
vendor` for air-gap; the family-wide keyword registry and attribute
registry enforced as a CI test so profiles cannot define colliding keys; the IR
stability policy versioned early (every plugin and `ridl diff` depend on it).
The package registry and the spy/control bridge are V2 (E7).

## Consequences

### Positive

- V1 is a complete, shippable product at the descriptive/executable seam —
  system + user contracts with codegen, LSP, diff, and docs — that delivers
  value and gathers users before the ambitious executable layer is attempted.
- The IR is stressed by cross-layer flow (E0), a second backend (E2), and a
  third profile (E3) before rmdl commits to it, so IR mistakes surface when they
  are cheap; rmdl then lands against a hardened IR and toolchain.
- One incremental core (salsa) serves `ridlc`, `ridl check`, and the LSP — no
  divergent reparser, the "library first" promise kept.
- The canonical IR is readable by every backend language, so Kotlin and TS
  backends are pure IR consumers with no reimplementation of RIDL semantics.
- Diagnostics have one structured SSOT feeding both the terminal and the LSP, so
  an editor and the CLI never disagree about an error.
- typl ships standalone at the end of E1 — external feedback before the whole
  family exists.

### Negative / accepted trade-offs

- **rmdl's differentiator lands only in V2.** V1 competes with existing IDLs
  (protobuf/AsyncAPI/Franca) without the knockout executable-behaviour feature.
  Accepted: shipping the contract platform sooner and de-risking rmdl outweighs
  leading with the hardest layer. Mitigated by keeping the reference-oracle/WASM
  story visible on the roadmap.
- **V1 contracts are not executable-verified.** `require`/`ensure` ship with
  only the static + generated-property-test verification ways; online observers,
  the reference oracle, and deductive proof arrive with rmdl (V2). Accepted: the
  verification story degrades gracefully and nothing is redone.
- **salsa is pre-1.0.** Breaking changes across 0.x are a scheduled migration
  cost. Accepted for the leverage it provides; mitigated by version pinning.
- **rowan trees are untyped.** A typed-AST accessor layer must be written (and
  ideally generated). Accepted as the cost of losslessness.
- **Hand-written parser is more code than a combinator grammar.** Accepted for
  error-recovery and LSP control.
- **protobuf as the IR** imposes protobuf's schema-evolution discipline on the
  IR and adds a codegen step to the build. Judged aligned with the family's own
  ordinal-evolution philosophy rather than a burden.
- **`lsp-server` is lower-level than a framework** — more dispatch boilerplate
  than tower-lsp-server or async-lsp. Accepted for sync/salsa coherence.
- **minijinja defers template errors to runtime/snapshot tests** rather than
  compile time. Accepted for plugin-backend flexibility.

## Open questions

Deferred to implementation or a later ADR.

- **expr-core subset boundary.** The exact V1 `require`/`ensure` subset (E2.4)
  that stays forward-compatible with rmdl's function layer (E5.1) needs fixing
  when the expr-core spec lands — the one V1/V2 interface that must not drift.
- **IR stability policy.** When the IR is declared stable, its versioning and
  compatibility rules become a spec of their own (concept note §8.2, general-
  form open question). Blocks the plugin protocol's stability contract.
- **Typed-AST generation.** Adopt rust-analyzer's `ungrammar` toolchain
  wholesale, or hand-roll a lighter generator?
- **Minimal rsdl in V1?** A wiring-only manifest could make V1 contracts
  deployable-as-SSOT earlier. Deferred to V2 (E6) by default; revisit only if a
  design partner needs deployment SSOT before behaviour exists.
- **Snapshot tool.** `insta` vs `expect-test` — decide once the test corpus
  shape is known.
- **`async-lsp` escape hatch.** Define the concrete trigger (which LSP feature
  or middleware need) that would justify revisiting §6.
- **Diamond-conflict and `[replace]` policy** (inherited from ADR-0002 open
  questions) — resolver behavior the fetch/cache implementation must eventually
  honor.
- **Self-hosting the IR schema in typl.** Attractive long-game once typl is
  real; explicitly out of scope for V1 to avoid a bootstrap dependency.

## References

- ADR-0002: module system and package management.
- `implementation-backlog.md` — the Epic/Story breakdown of §1.
- Concept note — the RIDL family, §7 (build sequence) and §8 (platform repo, IR,
  backends).
- General-form working spec, §5 (`ridl fmt`) and §6.3 (ordinal visibility
  tooling).
- rust-analyzer — the rowan + salsa + `lsp-server` architecture this stack
  follows.
- Ruff — salsa at production scale in a non-rust-analyzer compiler.
- prost / protobuf — descriptor-set precedent for the IR.
- Bytecode Alliance — wasmtime, wit-bindgen, cargo-component, jco.
