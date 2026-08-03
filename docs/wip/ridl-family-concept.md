# Concept Note — The RIDL Family

**One platform, five languages, one grammar: `typl` · `ridl` · `uxdl` · `rmdl` ·
`rsdl`**

Status: **Draft — exploratory concept note** (pre-ADR). Written to align on
direction before committing to ADR-0003 and per-language specification drafts.

Scope: this note proposes extending RIDL from a single interface-description
language into a **family of five languages sharing one grammar, one toolchain,
and one set of reusable semantic cores**. It records the reasoning settled in
design discussion and frames the open questions for follow-up ADRs.

---

## 1. Motivation

RIDL today answers one question: _what does a service produce, consume, and
guarantee?_ It is the single source of truth (SSOT) for the contract boundary,
and it is transport-neutral by design.

Four questions sit next to that one and are currently unanswered by any language
in the stack:

1. _What is the shared vocabulary?_ — data. Types, ranges, units, constants.
   Today these live inside RIDL; they deserve to stand alone as the foundation
   every other concern builds on.
2. _How does the user interact with the system?_ — user interaction.
   Viewmodel/view contracts (activate, toggle, display) are structurally the
   same problem as service contracts, aimed at a different audience.
3. _How does a component compute its outputs from its inputs?_ — behaviour.
   Today this is hand-written implementation code, disconnected from the
   contract. The intent is that behaviour be **described** (Lustre-style
   synchronous dataflow) and **generated** into processing code as a second
   step.
4. _How are components assembled into a system?_ — architecture. Instances,
   wiring across broker boundaries, transport and ECU deployment.

Data, interface, user interaction, behaviour, and architecture are the complete
decomposition of a component-based reactive system — RIDL currently owns only
the interface. The proposal is to own all five **without fragmenting the source
of truth**.

RIDL's design philosophy (ADR-0002) constrains how: _strict beats flexible_,
_small surface_, _every keyword is a keyword every reader must learn_, _faithful
codegen mapping_. Any extension must respect these.

---

## 2. The family at a glance

| Language | Expands to                              | Describes                                                        | Audience                      | Nature                   |
| -------- | --------------------------------------- | ---------------------------------------------------------------- | ----------------------------- | ------------------------ |
| **typl** | type language                           | data — types, ranges, units, constants, namespacing              | data architects               | descriptive (vocabulary) |
| **ridl** | reactive interface description language | system interactions — `signal` `event` `command` `query` `fixed` | service teams                 | descriptive (contract)   |
| **uxdl** | user-experience description language    | user interactions — activate, toggle, display, …                 | UX / frontend engineers       | descriptive (contract)   |
| **rmdl** | reactive model description language     | behaviour — synchronous/functional computing                     | control / algorithm engineers | **executable**           |
| **rsdl** | reactive system description language    | architecture — instances, wiring, deployment                     | integrators                   | structural (topology)    |

**RIDL** is the platform and family name, taken from its flagship member. (It is
treated as a _proper name_ at the platform level — see §10 — not read as
"…Interface DL," which would undercover rmdl and typl.)

### 2.1 The dependency lattice

```
                 typl            ← root: the vocabulary, mandatory for everyone
     ┌────────────┼────────────┐
     ▼            ▼            ▼
   ridl         uxdl         rmdl   ← three sibling capabilities
(system IF)   (user IF)   (behaviour; realizes ridl/uxdl contracts)
     └────────────┼────────────┘
                  ▼
                rsdl             ← apex: composes instances, deploys
```

Acyclic, single root, single sink — consistent with ADR-0002's cross-package
cycle prohibition.

**"Standalone" means you can stop at any layer, not that layers are
independent.** typl is the only truly standalone language (a units-aware schema
language, useful by itself). ridl, uxdl, and rmdl are each usable as _typl +
that layer_, in any subset — typl+ridl is RIDL as it exists today; typl+uxdl is
a pure view-interface SSOT; typl+rmdl is typed synchronous compute with no I/O
contract. rsdl is the exception: it can never stand alone, because it wires
instances of things defined below it. It composes **by import**, never by
inclusion.

So the scope model is: **typl (always) + any subset of {ridl, uxdl, rmdl} +
optionally rsdl on top.**

---

## 3. Reusable cores vs. surface languages

The five languages are _surfaces_. Beneath them, the reuse analysis identifies
the independently reusable cores. The test for corehood: **(a)** specifiable and
compilable without anything above it, and **(b)** needed by two or more
surfaces. What passes:

| Core          | Owns                                                                                                | Needed by              |
| ------------- | --------------------------------------------------------------------------------------------------- | ---------------------- |
| **ns**        | namespacing — `package`, `import`, `as`, `internal`, resolver, manifest, lockfile, cache (ADR-0002) | everything             |
| **typl-core** | types, ranges, constraints, units, composites                                                       | all surfaces           |
| **expr**      | the predicate/expression language — constraints, `require`/`ensure`, observers, test assertions     | typl, ridl, uxdl, rmdl |
| **time**      | timing/clock annotations — `@Xms`, `@[min..max]`                                                    | ridl, uxdl, rmdl       |
| **interact**  | interaction primitives: state value · occurrence · action · request/response · provisioned constant | **ridl and uxdl**      |

Two findings matter:

**ns is its own core, beneath typl.** Namespacing and typing are orthogonal —
rsdl must reference packages and instances without touching range machinery, and
the ADR-0002 resolver already operates on the package as a unit of _names_
regardless of what the names denote. (At the surface level, typl still presents
"types + namespacing" together — types and names always travel together in
source. The split is an implementation factoring.)

**ridl and uxdl are two profiles of one `interact` core.** Both describe _named,
typed, directed interactions on a contract boundary_. Only the vocabulary and
binding domain differ:

| `interact` primitive   | ridl (system)      | uxdl (user)          |
| ---------------------- | ------------------ | -------------------- |
| continuous state value | `signal`           | displayed view-state |
| discrete occurrence    | `event`            | gesture / input      |
| fire-and-forget action | `command`          | `activate`           |
| stateful mutation      | command-over-state | `toggle`             |
| request / response     | `query`            | fetch-for-display    |
| provisioned constant   | `fixed`            | static capability    |

ridl binds to transports (SOME/IP, proto, CAN, …); uxdl binds to view frameworks
(MVVM viewmodels, widget bindings). Everything else — typed payloads, optional
timing, contracts — is shared. This is the original "signal broker **and**
viewmodel/view interface" goal, realised as siblings over one core.

**rmdl is the only surface adding genuinely new semantics** — the synchronous
engine: `pre`, `->`, clocks, causality analysis. Because behaviour consumes
`interact` flows regardless of profile, one rmdl model can describe the logic
behind a _service_ (ridl) or a _viewmodel_ (uxdl). One behaviour language, both
interaction profiles.

---

## 4. One grammar, six extensions

The surface/core split leaves a thin final decision: one unified surface, or
five? The resolution is **both, via profiles**.

There is **one grammar** (one lexer, one AST, one toolchain, one IR). Each
language is a **profile** of that grammar — a restriction to its layer —
selected by file extension:

| Extension   | Profile    | Accepts                                                                                                                                                     |
| ----------- | ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `.typl`     | restricted | type declarations only                                                                                                                                      |
| `.ridl`     | restricted | system-interaction declarations                                                                                                                             |
| `.uxdl`     | restricted | user-interaction declarations                                                                                                                               |
| `.rmdl`     | restricted | behaviour declarations                                                                                                                                      |
| `.rsdl`     | restricted | architecture declarations                                                                                                                                   |
| **`.rxdl`** | **total**  | **the unrestricted profile — any layer _and_ any interaction domain. Widened by ADR-0012's amendment; `x` reads as both the layer wildcard and "extended"** |

**Rationale — why five named surfaces rather than one facade.** The five layers
are written by five different professions — in a vehicle program, often
different departments or companies. Each audience gets _its_ language: learnable
whole, documented for them, evolving on its own track. This follows ADR-0002's
own principle (every keyword is a keyword every reader must learn) and matches
the strongest precedents: JSON Schema / OpenAPI / AsyncAPI (a type vocabulary
reused by separately-named sibling interface specs — exactly the typl/ridl/uxdl
shape), and HTML/CSS/JS (three languages, three audiences, one runtime).
Restricted extensions also make the safety-critical question — _which files
contain executable behaviour?_ — answerable from the filesystem:
`find . -name '*.rmdl'`.

**Rationale — why a total profile exists.** `.rxdl` gives an adoption gradient.
A solo developer, a demo, or a getting-started guide writes one `.rxdl` file
containing types + interface + model + wiring. A production program enforces
profile purity per package in `ridl.toml` (the package is already ADR-0002's
unit of everything), making mixed files a policy error where it matters. Same
grammar, same compiler, same IR throughout — `.rxdl` is a _packaging_
convenience, never a semantic merge.

**Rationale — naming the total profile `.rxdl`, the platform RIDL.** Making
`.ridl` the total profile would give it two meanings (interface layer _and_
superset); `.rxdl` keeps every extension single-meaning. Making _rsdl_ the
facade was rejected: rsdl composes by import, not inclusion — the apex should
not swallow the family — and "RSDL" carries an ITU-SDL echo plus poor search
results. RIDL stays the platform name: it is the flagship concern, most users'
front door, pronounceable, unique, and already carries the project's equity.
`ridlc`, `ridl.toml`, `ridl.lock`, and `~/.ridl/cache` are therefore already
correctly named as platform-level artifacts — **ADR-0002 requires no renaming.**

### 4.1 Surface syntax and keyword discipline

**Syntax heritage: the C / TypeScript / Rust / Kotlin lineage.** Brace-delimited
blocks; postfix `name : Type` typing (the Kotlin/Rust/TS side — C contributes
literal conventions and braces, not its prefix types); `?` optional suffix
(Kotlin); `//`, `/* */`, `///` comments; C17 integer/float literal rules
(already cited in the Language Reference); newline/comma as interchangeable
separators; no semicolons. Existing RIDL already follows this style — the
decision here is that it binds **family-wide**: rmdl, uxdl, and rsdl inherit the
same lexical conventions, so crossing layers never means switching syntax
families.

**Keyword discipline.** One grammar implies one **family-wide reserved-word
registry**. Each keyword names exactly one concept in exactly one layer
(`signal` is ridl's, `node` is rmdl's, …). A new keyword in any profile is an
addition to the shared registry, collision-checked against all layers. Keywords
are precise nouns/verbs of their domain, consistent in form — this registry is
the enforcement mechanism behind "consistent and precise," not a style
aspiration.

_Errata surfaced by this rule:_ ADR-0002's example writes
`package veh.common.types;` with a trailing semicolon; the Language Reference
grammar has none. Reconcile to **no semicolons**.

---

## 5. Layering discipline

Three rules are enforced by the compiler in every profile, including `.rxdl`:

1. **Single-concern regions.** Every declaration belongs to exactly one layer;
   the compiler routes it to exactly one semantic core.
2. **The acyclic lattice.** typl ← {ridl, uxdl, rmdl} ← rsdl. Upward references
   are a compile error.
3. **The sync/async wall.** rmdl is tick-synchronous; ridl/uxdl/rsdl cross
   asynchronous broker boundaries. Inside a synchronous island, rmdl nodes
   compose tick-by-tick (the Lustre/SCADE lineage) — hierarchical reactive
   components need no new construct. Across a broker there is no shared clock,
   so writing synchronous dataflow across an async boundary is a **compile
   error**, not a warning.

---

## 6. Worked example — cruise control

_Illustrative only; all syntax beyond today's typl/ridl is not final. The point
is the layers meeting, not the keywords._

**typl — vocabulary (`veh/common/`, `.typl`):**

```ridl
type Speed : km/h [0.0..250.0 step 0.5]

const MAX_SPEED      : Speed = 250.0
const SPEED_LIMIT_EU : Speed = 130.0

enum LeverCmd { NONE = 0, ENGAGE = 1, CANCEL = 2, RESUME = 3 }
```

**ridl — the async system contract (`.ridl`, RIDL as it exists today):**

```ridl
/// Cruise control service contract.
/// @labels SIL_2, SEC_2, PRIVATE
interface CruiseControl {
  signal currentSpeed : Speed   @20ms
  signal targetSpeed  : Speed   @[20ms..500ms]
  signal engaged      : boolean @[20ms..1s]

  command setLever(cmd: LeverCmd)
}
```

**rmdl — the synchronous behaviour that realises it (`.rmdl`):**

```ridl
model CruiseController realizes CruiseControl {

  // base clock derived from the interface timing (@20ms)
  node control(current: Speed, lever: LeverCmd, brake: boolean)
       returns (engaged: boolean, target: Speed) {

    engaged = false -> if brake or lever == LeverCmd.CANCEL then false
                       else if lever == LeverCmd.ENGAGE     then true
                       else pre engaged                       // latched state

    target  = SPEED_LIMIT_EU -> if lever == LeverCmd.ENGAGE then current
                                else pre target               // capture-on-engage
  }
  // require/ensure contracts become synchronous observers, checked every tick
}
```

The seam: interface signals become input/output **flows**; the `@20ms` timing
becomes the **base clock**; `require`/`ensure` become **synchronous observers**.
`pre`/`->` are the only genuinely new surface — every name (`Speed`,
`SPEED_LIMIT_EU`, `LeverCmd`, `CruiseControl`) is a shared symbol resolved
through the one module system, not a copy. And because `Speed` arrives with its
unit, range, and step, rmdl inherits dimensional checking and range-driven
saturation — richer than a bare synchronous language that only knows `real`.

**uxdl — the user contract (`.uxdl`, vocabulary deliberately unsettled):**

```ridl
view CruisePanel binds CruiseControl {
  display speedReadout : Speed   <- currentSpeed
  display engagedLamp  : boolean <- engaged

  activate resumeButton -> setLever(LeverCmd.RESUME)
  activate cancelButton -> setLever(LeverCmd.CANCEL)
}
```

**rsdl — system assembly (`.rsdl`, sketch; manifest-shaped first, see §7.3):**

```
instance cruise : CruiseController
  bind currentSpeed <- veh.cluster.VehicleStatus.currentSpeed
  deploy on ecu.adas   transport SOME/IP

instance panel : CruisePanel
  bind CruiseControl <- cruise
  deploy on ecu.cockpit
```

rsdl says _which instances exist, how ports connect across the broker, and where
they run_ — the only layer that references all four below it.

---

## 7. Build sequence

### 7.1 Bottom-up, by the lattice

| Stage        | Adds                                       | Shippable value on its own                                                                      |
| ------------ | ------------------------------------------ | ----------------------------------------------------------------------------------------------- |
| **1 — typl** | type/schema layer + ADR-0002 module system | a units-aware, range-constrained **schema language** generating data types across every backend |
| **2 — ridl** | system interactions + contracts            | RIDL as it exists today — the SSOT contract boundary                                            |
| **3 — rmdl** | synchronous behaviour, observers           | generated processing code from realised interfaces                                              |
| **4 — uxdl** | user interactions over the `interact` core | viewmodel/view SSOT, reusing stage-2 machinery                                                  |
| **5 — rsdl** | system assembly (manifest first)           | deployment and integration artifacts                                                            |

(Stages 3 and 4 can swap or overlap — both sit directly on stage 2's `interact`
core.)

> **Sequencing refined post-note.** ADR-0004 and `implementation-backlog.md`
> re-cut this stage list into two releases along the **descriptive vs
> executable** seam: **V1** = typl + ridl + uxdl + ecosystem (the contract
> platform); **V2** = rmdl + rsdl + rxdl (the executable platform, where the
> risk concentrates). uxdl is pulled forward into V1 (cheap second profile;
> hardens the IR before rmdl commits), and rmdl/rsdl are deferred so they land
> against a proven IR and toolchain. The lattice ordering here remains correct
> for _dependency_; the V1/V2 split is the _build/release_ ordering.

The anti-pattern to avoid is **building the interface layer with types folded
in** ("ridl = types + interactions" as one grammar block). That bakes the
vocabulary into one layer and forces a later extraction to reuse it in
rmdl/uxdl/rsdl — precisely the coupling this design exists to prevent.

Stage 1 is a **refactor, not greenfield**: Language Reference §3–§5 already _is_
typl. The step is to recognise and name that layer as the reusable foundation,
not to invent it.

### 7.2 Content repositories

Already answered by ADR-0002: one workspace per system, packages layered by
profile (`veh.common` = typl, `veh.cluster` = ridl, `veh.cluster.model` = rmdl,
…), one lockfile. Split a package into its own repo only when independent
versioning or an external consumer forces it.

### 7.3 rsdl: config first, language later

rsdl's content is config-shaped — instances, connections, mappings. ADR-0002 set
the precedent: _"distribution is handled entirely by manifest, lockfile, and
cache — not by language constructs."_ So rsdl starts as a **structured
manifest**, and graduates to a full grammar profile when topology grows real
structure — multiplicity, redundancy, hierarchical subsystems. The name and the
semantic layer are reserved now; the syntax is not built until it earns itself.

---

## 8. Platform repository, IR, and backends

### 8.1 One platform monorepo

One grammar means one front-end — a single grammar cannot be cleanly split
across repositories. The rule: **crates separate concerns; repos separate
release cadences and contributor sets.** Nothing in the platform has a divergent
cadence yet.

```
ridl/                     (platform monorepo)
├── spec/                 language reference, ADRs, IR spec, observability semantic conventions
├── crates/
│   ├── ridl-syntax       lexer + unified grammar → CST (one grammar, one parser)
│   ├── ridl-core         the reusable cores: ns · typl-core · expr · time · interact
│   ├── ridl-sem          per-profile semantic passes: ridl / uxdl / rmdl (causality + clocks) / rsdl
│   ├── ridl-ir           the stable, serializable IR        ← the centre of gravity
│   ├── ridlc             the compiler — plumbing: stable flags, scriptable, what CI and
│   │                     build systems call directly (`ridlc --frozen` lives here, per ADR-0002)
│   ├── ridl              the toolchain facade — porcelain, cargo/deno-style:
│   │                     fmt · check · build · test · diff · lint · doc · vendor · init
│   └── ridl-lsp          language server (must live with syntax + sem)
├── backends/             rust + c-abi · wasm-component · kotlin · typescript   (IR consumers)
├── runtimes/             ridl-rt (Rust) · rt-kotlin · rt-ts   (published to crates.io/maven/npm from here)
└── tools/                ridl-test · ridl-diff · ridl-fmt · ridl-lint
                          (IR consumers, surfaced as `ridl` subcommands)
```

**Tooling model — porcelain and plumbing.** `ridl` is the developer front door
(the cargo/deno role); `ridlc` is the compiler proper beneath it (the rustc
role). The deno comparison runs deeper than the CLI: ADR-0002's distribution
model — URL imports, content-hashed lockfile, per-user cache — _is_ Deno's
model, so a deno-style single toolchain binary completes it coherently: one
installed `ridl` carrying the whole toolchain, `ridlc` as the stable plumbing
layer for CI and build-system integration.

**Implementation language: Rust, compiler-as-library.** The platform is
implemented in Rust — the workspace layout, the cargo/rustc tooling pattern, and
the rmdl → Rust → WASM backend already presume it, so compiler and generated
behaviour share one language. Supporting arguments: the Rust language-tooling
ecosystem (`salsa` incremental computation on the rust-analyzer model, so
`ridl-lsp` and `ridlc` share one incremental core instead of being two
implementations; `lsp-server`; `wasmtime` embeds natively for the
reference-oracle/replay machinery — concrete library picks are fixed in
ADR-0004); single static binaries with no runtime dependencies, which suits both
the deno-style install experience and qualified/air-gapped automotive
environments; and a memory-safe, deterministic toolchain as the easy ISO 26262
tool-qualification story. Both `ridl` and `ridlc` are thin binaries over the
**same library crates** — the compiler is a library first, which is what makes
the LSP, `ridl-diff`, and the test tooling first-class consumers rather than
side-band reparsers.

What does **not** live here: content workspaces (§7.2) and any hosted service (a
registry has a deployment lifecycle, not a release lifecycle). Those are
separate repos from day one.

### 8.2 The IR is the centre of gravity

Between the front-end and everything else sits a **stable, serializable IR**:
resolved names, checked types with ranges and units intact, interactions with
timing, behaviour graphs with clocks, topology. (Analogue: protobuf's descriptor
set / buf's image.) Every backend and every ecosystem tool is an IR consumer
behind one plugin protocol.

The IR is the **pre-planned fission line**: while it is young, every IR change
touches every backend — which is exactly what a monorepo's atomic commits are
for. When a backend earns a different cadence or an external owner, it splits
out cleanly _because_ the boundary already exists. If a separate repo for parts
of the platform ever feels necessary before that, it is a signal the design is
drifting back toward separate surface toolchains — a legitimate choice, but one
to take deliberately at the design level, not by accident at the repo level.

### 8.3 Backends are in scope — asymmetrically

A contract language with no codegen is a paper spec. But the layer × target
matrix is **not full**, which changes the economics:

| Layer                         | Rust (+ extern C)                                                   | Kotlin | TypeScript | WASM component |
| ----------------------------- | ------------------------------------------------------------------- | ------ | ---------- | -------------- |
| typl (types)                  | ✓                                                                   | ✓      | ✓          | ✓              |
| ridl / uxdl (bindings, stubs) | ✓                                                                   | ✓      | ✓          | ✓              |
| rmdl (executable behaviour)   | ✓ native                                                            | —      | —          | ✓ compile once |
| rsdl (deployment)             | not per-language — emits manifests, topology, integration artifacts |        |            |                |

Types and bindings go everywhere — that is the SSOT promise, and it is the cheap
part. **Behaviour does not need N native backends**: rmdl is synchronous,
deterministic, self-contained compute — the ideal WASM payload. Compile rmdl →
Rust → WASM component once; run it under wasmtime on the ECU/edge side and in
the browser on the uxdl side. Kotlin and TypeScript then need only _binding_
generation plus a thin host shim, never behaviour codegen. The matrix collapses
to: **bindings everywhere, behaviour twice (Rust native + WASM)**.
Rust-with-extern-C is the systems anchor; WASM is the portability anchor;
Kotlin/TS are Tier-2 binding targets.

---

## 9. Ecosystem: two planes over one IR

Observability, contract testing, and automation are not products bolted onto the
language — **they fall out of the IR**, each powered by a different layer. This
is the strongest retroactive validation of the layered design.

### 9.1 CI plane

- `ridlc --frozen` (already in ADR-0002) — reproducible resolution.
- **`ridl-diff`** — breaking-change detection between IR snapshots (the
  buf-breaking analogue), gated in CI against the lockfile. Placement clarified:
  diff is **not part of `ridlc`** — the compiler stays a pure source→IR function
  (the minimal ISO 26262 tool-qualification boundary), while diff compares two
  IR snapshots across time and needs a baseline (registry/git/lockfile), which
  is workflow. But it is **not mere porcelain either**: typl §7.4 (field-ordinal
  evolution) and §5.6 (wire-width flips) make diff a normative gate, so
  `ridl diff` carries **plumbing-grade stability despite living in the facade**
  — stable flags, machine-readable output, defined exit codes (0 compatible / 1
  breaking / 2 error) that CI may depend on, the same contract class as
  `ridlc --frozen`. Rule of thumb: stability attaches to commands, not binaries
  (deno's `deno check` and buf's `buf breaking` precedents).
- **Generated property tests** — typl ranges/steps/patterns are _generators_
  (`[0.0..250.0 step 0.5]` derives its fuzz corpus mechanically: boundaries,
  step violations, pattern breakers); `expr` contracts are _oracles_.

### 9.2 Runtime test plane

The same IR that generates production bindings generates their instrumented
mirror images — a **test plane** beside the data plane:

- **E2E injection.** In a broker architecture an injector is just another
  participant. Per interface, the harness generates publishers that drive any
  signal/event with typl-valid (or deliberately invalid) values, and callers for
  any command/query. rsdl knows the wiring, so it emits **test topologies** —
  the real system with selected components swapped for injectors/simulators:
  rest-bus simulation derived from the SSOT instead of hand-maintained in a
  vendor tool.
- **Model assertion.** One assertion, written once in `expr`, runs four ways:
  statically where decidable; as CI property tests; as **online observers**
  subscribed to live flows; and — strongest — the rmdl model itself deployed as
  a **reference oracle**: the WASM build runs in lockstep beside the real
  implementation, fed the same inputs, with tick-by-tick output diffing.
- **Spy/control bridge.** The bridge is _itself a generated RIDL interface_ — a
  reflection service per system: subscribe to any flow (spy), publish into any
  flow (control), evaluate an `expr` predicate remotely (assert), enumerate
  interfaces (discovery). XCP/DLT's role, derived from the contracts instead of
  maintained beside them, typed rather than address-based.
- **Security gating — not optional in this domain.** The bridge is a diagnostic
  attack surface. rsdl decides whether the bridge exists in a given topology
  (test/dev builds only, or behind an authenticated diagnostic session); the
  `@labels`/profile system marks what is spyable or injectable at which
  assurance level (an ASIL-D flow may spy but never inject outside HIL).
  Injection rights are part of the contract, not a tool setting.

### 9.3 Observability

- **Semantic conventions** (a spec, `spec/`): package/interface/interaction
  names and timing bounds map to OTel attributes. A signal's `@10ms` is an
  alertable **freshness SLO** — the contract already defines "late."
- **Codegen hooks**: generated bindings emit spans/metrics automatically — zero
  instrumentation debt.
- **Deterministic replay**: rmdl's synchronous determinism means recorded input
  flows per tick replay any field incident against the model, exactly. Spy →
  record → replay → assert is one pipeline. This is inherited from choosing
  Lustre semantics, not added.

---

## 10. Naming ledger

| Name                                        | Status              | Notes                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| ------------------------------------------- | ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **RIDL** (platform)                         | locked              | Platform / family name — treated as a **proper name**, pronounced "riddle," **not read as an acronym** at the platform level. Taken from the flagship member, but deliberately _not_ expanded as "…**Interface** Description Language" for the platform: that undercovers rmdl (behaviour, executable) and typl (vocabulary). Optional docs gloss, if one is wanted: **Reactive Integrated Description Languages** (plural; "integrated" = one grammar, one IR, one toolchain). Pronounceable, unique, carries the project's equity; `ridlc`, `ridl.toml`, `ridl.lock`, `~/.ridl/cache` unchanged.                                                                                                                                                                                                                                                                                                                                             |
| **ridl** (member)                           | locked              | the interface layer, flagship — **R**eactive **I**nterface **D**escription **L**anguage. The "Interface" gloss lives _here_, at the member, not at the platform.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| **typl**                                    | locked              | deliberately `-L` not `-DL`: it is the vocabulary the others describe _with_                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| **uxdl**                                    | locked              | was "uxil" in early discussion                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| **rmdl**                                    | locked              | was "mdl"; avoids the Microsoft-adjacent "MIDL"; makes the r·dl siblings consistent                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| **rsdl**                                    | locked, with caveat | "system" = architecture + deployment (AADL tradition). Caveat on record: echoes ITU-T SDL; the bare string "RSDL" searches poorly. Alternatives considered: `radl` (collides with a RESTful API DL; "architecture" undersells deployment), `rcdl`. Revisit only if the collision proves annoying.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| **`.rxdl`** (file kind)                     | locked              | the total profile; `x` as wildcard over the family pattern                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| **`rxdl` / `rxdlc` (as platform/CLI name)** | rejected            | Considered because the `ridl` crate name is taken on crates.io, and because `x`-as-wildcard would drop the "interface" claim the platform name otherwise carries (the real motivation — see the RIDL row's resolution). Declined: (a) `x` already denotes the total-profile wildcard, so reusing it as the platform name double-books the letter and muddies `.rxdl`; (b) re-invites the ReactiveX/"Rx" association rmdl deliberately pruned (`scan`/`throttle`/`merge` renamed to `Accumulate`/`Deadband`/`Prefer`), plus a pharmacy false-friend; (c) "riddle" is a pronounceable word, "RXDL" a clunky initialism; (d) forces ADR-0002 artifact renames (`ridl.toml`, `ridlc`, cache). The "IDL undercovers the family" concern is resolved instead by treating **RIDL as a proper name** and keeping the _Interface_ gloss on the `ridl` member.                                                                                           |
| crate names (crates.io)                     | on record           | `ridl` is **taken** (an unrelated dormant crate, last published 2023). Mitigation, no brand impact: **binary name ≠ crate name** — publish the CLI crate as `ridlc` (free) producing a `ridl` binary; the workspace crates `ridl-syntax`/`ridl-core`/`ridl-ir`/`ridl-lsp` are free. `typl`, `rmdl`, `rsdl`, `uxdl`, `rxdl` were all free — **reserved 2026-07-18** as 0.0.0 placeholders, together with `ridlc`, `ridl-syntax`, `ridl-core`, `ridl-ir`, and `ridl-lsp` (issue #92). Acquiring the bare `ridl` crate via crates.io's name-transfer process is optional, not required.                                                                                                                                                                                                                                                                                                                                                           |
| **`fixed`** (keyword)                       | locked              | the family's provisioned-constant interaction kind: immutable for the software-instance lifetime, safe to cache. `config` (the markspec-typl precursor's kind) rejected: familiar but promises the wrong thing — no immutability guarantee (hot-reload connotation), names the provider's workflow rather than the consumer contract, and collides with vocabulary rsdl will want for deployment configuration. Precedents are immutability-framed: AUTOSAR `CalibrationParameter`, Android `ro.*` properties, SOME/IP getter-only field. `provisioned` and `readonly` also considered; `readonly` suggests a read-only view of a value that may still change — the opposite of cacheable. **Superseded in part (2026-07-27, ADR-0011): the keyword is `fixed`, not `final` — a Java or Kotlin reader takes `final` for a compile-time constant, and uxdl already spelled the same kind `fixed`. The rejection of `config` stands unchanged.** |
| **`ridl` (CLI)**                            | locked              | the toolchain facade — cargo/deno-style porcelain: fmt, check, build, test, diff, doc, vendor                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| **`ridlc`**                                 | locked              | the compiler proper — plumbing beneath the facade; stable flags for CI and build systems                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| syntax heritage                             | locked              | C/TypeScript/Rust/Kotlin lineage, family-wide; one reserved-word registry across all profiles (§4.1)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| implementation language                     | locked              | Rust, compiler-as-library; `ridl` and `ridlc` are thin binaries over the same crates (§8.1)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| ~~MIDL, MBSDL, ADL, Triad, Strand~~         | rejected            | MIDL is Microsoft COM IDL; SDL is ITU-T; "adl" is a generic term of art (AADL, EAST-ADL); a new umbrella brand discards RIDL's equity and forces ADR-0002 renames                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |

---

## 11. Impact on existing documents

- **ADR-0002** — unchanged in substance. A short amendment should note that the
  module system (`ns` core) serves all five languages, and that package-level
  profile purity is declared in `ridl.toml`.
- **Language Reference** — §3–§5 are re-framed as **typl**; the interaction
  sections gain a note that `signal`/`event`/`command`/`query`/`fixed` are the
  ridl profile of the shared `interact` core; new reference documents grow per
  language as they are built.
- **Getting Started** — eventually gains the `.rxdl` single-file demo as the
  first-contact experience.

---

## 12. Open questions

1. **Consumed-interface binding.** A model's inputs that come from _other_
   services (e.g. `CruiseController` consuming `VehicleStatus.currentSpeed`) —
   declared in rmdl (`uses` clause) or wired purely in rsdl? Leaning rsdl-wired
   with rmdl declaring abstract input flows, but needs its own treatment.
2. **rmdl operator set for v0.1.** `pre`, `->`, node instantiation are certain;
   `when`/`merge`/`current` (multi-rate clocks) and state-machine sugar are
   open.
3. **uxdl vocabulary.** `display`/`activate`/`toggle` are placeholders; the real
   vocabulary needs a design pass with UX engineers (and a decision on whether
   navigation/screen-flow is in scope or is rsdl-adjacent).
4. **Profile enforcement granularity.** Per package via `ridl.toml` (ADR-0002
   spirit) is settled as the mechanism; whether a stricter per-file rule adds
   value on top is open.
5. **rsdl graduation trigger.** The concrete signal that promotes rsdl from
   manifest to grammar profile needs definition.
6. **Contract/expression unification.** Confirm `require`/`ensure`, rmdl
   observers, and test-plane assertions share the identical `expr` surface and
   checker.
7. **Bridge authentication model.** What "authenticated diagnostic session"
   means concretely for the spy/control bridge in shipping topologies.
8. **IR stability policy.** When the IR is declared stable, its versioning and
   compatibility rules become a spec of their own.

---

## 13. Next steps

1. Circulate this note for direction sign-off.
2. **ADR-0003 — The RIDL family: profiles, cores, and the platform model.**
   Captures the decision and the rejected alternatives (single facade with one
   extension; five fully separate languages/toolchains; new umbrella brand;
   rsdl-as-language-now) in ADR-0002 style.
3. **Extract typl** (Stage 1) — re-frame Language Reference §3–§5 as the named
   foundation; define the `.typl` profile. _Done: see
   `typl-language-reference.md` (v0.1 draft)._
4. **Draft the rmdl reference** — full clock and causality treatment, anchored
   on the cruise-control example.
5. **Specify the rsdl manifest schema** as the interim system layer.
6. **uxdl vocabulary workshop** — settle the user-interaction primitives over
   the `interact` core.
7. **IR spec skeleton** — the serialization format and plugin protocol, since
   every backend and ecosystem tool hangs off it.
