# rmdl Language Reference

**Reactive Model Description Language** — the behaviour layer of the RIDL
family: pure total functions and reactive state models, executed in
runtime-scheduled steps, over the typl vocabulary. Models are contract-blind;
binding a reaction to a service is rsdl's job.

Version: 0.1.0 — Draft

> **Provenance.** rmdl is the family's only **executable** language (concept
> note §2) and the only surface adding genuinely new semantics: memory (`last`,
> `init`) and the step. Its lineage is the synchronous dataflow tradition
> (Lustre/SCADE) with one deliberate departure — execution is **reactive, not
> periodic**: steps are scheduled by the runtime on input arrival and timing
> constraints, only when required (§6). Everything below the behaviour layer is
> inherited: types/units/ranges (typl), contracts and their timing (ridl/uxdl),
> the envelope and system time (ridl §3.1), errors-as-data (ridl §10), evolution
> (typl §7.4). This document specifies the two layers the user-facing design
> settled: a **total function layer** shared with the family `expr` core, and
> the **model** — the unified reactive construct (no separate `node`).

---

## Table of Contents

1. [Scope and Position in the Family](#1-scope-and-position-in-the-family)
2. [Lexical Additions](#2-lexical-additions)
3. [The Function Layer](#3-the-function-layer)
4. [Expressions](#4-expressions)
5. [The Model](#5-the-model)
6. [Execution — Steps, Not Ticks](#6-execution--steps-not-ticks)
7. [Models Are Contract-Blind](#7-models-are-contract-blind)
8. [Step Faults](#8-step-faults)
9. [Observers](#9-observers)
10. [Conventions](#10-conventions)
11. [Diagnostics](#11-diagnostics)
12. [Open Questions](#12-open-questions)

- [Appendix A — Full Example](#appendix-a--full-example)
- [Appendix B — Codegen Targets](#appendix-b--codegen-targets)
- [Appendix C — Formal Grammar (EBNF)](#appendix-c--formal-grammar-ebnf)
- [Appendix D — Prior Art Survey](#appendix-d--prior-art-survey)
- [Appendix E — Coverage Analysis: Behaviour Languages](#appendix-e--coverage-analysis-behaviour-languages)
- [Appendix F — Glossary](#appendix-f--glossary)
- [Appendix G — ridl.std.flow: The Retained Operator Set](#appendix-g--ridlstdflow-the-retained-operator-set)

---

## 1. Scope and Position in the Family

### 1.1 What rmdl is

rmdl answers: _how does a component compute its outputs from its inputs?_ An
rmdl **model** is described behaviour — deterministic, analyzable, replayable —
that the toolchain **generates into processing code** (Rust native, WASM
component). A model is a **pure reaction** — it computes output flows from input
flows and state, knowing nothing of any contract. A component (rsdl) is what
binds a reaction to a _service_ (ridl) or _view_ (uxdl); the same model can back
either. This keeps the layers clean: ridl declares contracts, rmdl computes,
rsdl connects.

A `.rmdl` file accepts `function` and `model` declarations plus everything typl
accepts. It rejects interaction declarations (ridl/uxdl) and architecture
(rsdl).

### 1.2 Two layers, one discipline

| Layer         | Keyword    | Nature                                  | Owns                                 |
| ------------- | ---------- | --------------------------------------- | ------------------------------------ |
| **functions** | `function` | pure, **total**, timeless — mathematics | algorithms: `let y = a*x + b`        |
| **models**    | `model`    | reactive, stateful over steps           | memory (`last`, `init`), composition |

The separation is semantic, not stylistic: a function has no notion of time or
state and may therefore be evaluated _anywhere_ — inside a model step, inside a
`require`/`ensure` clause, inside a test oracle, at compile time in constraint
positions. **Functions are the family `expr` core's function layer** (decision
on record): one expression language across contracts, behaviour, and the test
plane — concept note open question 6, resolved in the affirmative.

### 1.3 The sync/async wall, restated

Inside a model, composition is synchronous: instantiated models step together,
atomically (§6.5). Across a broker there is no shared step — writing model
dataflow across an async boundary is a compile error (RMDL-401). Models talk to
the world only through their signature flows, wired by rsdl components.

### 1.4 Naming decision — `model`, no `node`

The reactive construct is the **model itself** — the concept-note's model/node
split is collapsed (decision on record, provisional: _`model` a priori, unless
better is found_). One construct: a model has typed input/output flows, per-step
semantics, and helpers are just other models it instantiates. Rejected: `node`
(Lustre jargon, meaningless to the audiences, mentally collides with topology
vocabulary rsdl needs), `reactor` (Lingua Franca's term, foreign to automotive),
`block` (Simulink-familiar but generic), `process` (OS connotation), `machine`
(pre-empts the deferred state-machine sugar). "Model" is the word control
engineers already use — Simulink model, plant model, and this family's own
reference-oracle story deploys "the model" beside the implementation.

---

## 2. Lexical Additions

rmdl inherits the family lexical conventions (typl §2) and ridl's duration
literals.

Keywords **used** by the rmdl profile, beyond typl's set:

```
function  let  model
last  init  if  then  else  case
when  emit  signal  event
```

**Ambient time values — not keywords.** `now` (the current logical instant) and
`dt` (elapsed since the previous activation) are **contextual identifiers
available inside model bodies and contract clauses** — the `result`-in-`ensure`
mechanism, not reserved words. typl fields named `now` or `dt` remain legal
everywhere; declaring a _flow_ named `now`/`dt` inside a model is an error
(RMDL-110). `step` is **not** a surface word at all — the step is
runtime/semantics vocabulary (§6), never syntax, by the same doctrine that keeps
async/await out of the language.

(`init` is shared with ridl's channel-init attribute — one concept,
initialization, two positions, like `match`. `pre` and the followed-by `->` were
considered and dropped for `last`/`init` — §5.3 decision note; `->` survives
only as the case/when arm arrow.)

`signal` and `event` here are the **interact core's concept words** surfacing in
model signatures (§5.1) — the same two concepts ridl and uxdl profile, marking
flow kinds at the model boundary. One keyword, one concept, three surfaces —
registry-clean.

New token: `?:` (default-value operator, Kotlin heritage — §4.4). `->` is the
case/when arm arrow only.

Reserved for future rmdl use: `merge`, `current` (clock calculus, if ever needed
— §12.1), `state`, `transition`, `automaton` (state-machine sugar — §12.3).
`when` is **no longer reserved** — it found its real job (§5.6).

---

## 3. The Function Layer

### 3.1 Declaration

A `function` is a named, pure, **total** computation — Kotlin/OCaml style,
`let`-bindings, expression-bodied:

```ridl
// expression form
function celsiusToRatio(t: Temperature): Ratio = (t + 40.0) / 165.0 * 100.0

// block form — let bindings, last expression is the result
function interpolate(x: Speed, a: float, b: float): Speed {
  let scaled = a * x
  let offset = scaled + b
  clamp(offset, 0.0, MAX_SPEED)
}

function clamp(x: Speed, lo: Speed, hi: Speed): Speed =
  if x < lo then lo else if x > hi then hi else x
```

- Parameters and result are typl types (named types preferred; bare
  `integer`/`float` permitted as in struct fields)
- `let` binds immutable locals; no reassignment, no shadowing (RMDL-104)
- Package-scoped, importable, `internal`-able — functions are
  vocabulary-adjacent citizens under the same `ns` rules

### 3.2 Totality — the contract of the layer

A function **always terminates and never faults on its own**:

- **No recursion**, direct or mutual (RMDL-101)
- **No unbounded loops** — there is no loop statement at all. Iteration exists
  only as **bounded combinators over typl bounded collections** (§4.5): `map`,
  `fold`, `any`, `all`, `count`. Every bound is a typl bound, so every iteration
  count is statically known
- **No state, no time** — `last`, `init`, `now`, and `dt` are model-layer
  constructs; inside a function they are errors (RMDL-102, RMDL-103). A
  computation that needs elapsed time takes it as an ordinary parameter, staying
  timeless

What totality buys, and why it is non-negotiable: per-call **WCET is decidable**
(safety scheduling), functions are **legal in contract positions**
(`require`/`ensure`, and the const-evaluable subset in typl constraint bounds —
the fencing typl §17.7 demanded), and the test plane can evaluate any function
as an oracle without a sandbox.

The two residual runtime hazards — division by zero and range violation at a
typed boundary — are not function faults but **step faults**, defined once in §8
(a function is total over its _valid_ inputs; guarding divisors is the author's
obligation, checked where decidable, RMDL-105 otherwise).

### 3.3 Unit discipline

Functions inherit typl's nominal unit safety (typl §5.7):

- same-type addition/subtraction/comparison: `Speed + Speed : Speed` ✓
- scalar multiplication/division: `Speed * float : Speed` ✓
- cross-unit arithmetic (`Speed + Torque`, `Speed * RPM`) is an error (RMDL-106)
  — until the unit-algebra question (typl §17.5) is settled, crossing units
  requires an explicit named conversion function, which is itself ordinary
  vocabulary

Intermediate expressions compute in the language layer (`float64`/`int64` — typl
§4) and are unconstrained; constraints re-apply at every **typed boundary** — a
function's result, a model output, a contract flow (§8).

---

## 4. Expressions

The expression grammar below is shared verbatim with the `expr` core: what is
written here is what `require`/`ensure` accept (ridl §13), with contract
positions restricted to the const-evaluable subset.

### 4.1 Operators

Arithmetic `+ - * / %`, comparison `== != < <= > >=`, boolean `&& || !` —
C/Kotlin lineage, standard precedence. Integer division truncates toward zero;
`%` follows the dividend's sign (C17).

### 4.2 Conditionals

`if c then a else b` is an **expression**; `else` is mandatory (totality —
RMDL-107).

### 4.3 `case` — exhaustive dispatch

`case` dispatches on enums, unions, and optionals; arms use `->`; exhaustiveness
is required, with `else` as the explicit catch-all (RMDL-108 when incomplete):

```ridl
case gear {
  PARK    -> 0.0
  REVERSE -> -maxCreep
  else    -> creepFor(gear)
}

case result {                     // union dispatch binds the arm's payload
  ok  v -> v.value
  err e -> fallbackFor(e)
}
```

(`case`, not `match` — the registry assigns each keyword one _concept_, and the
family's dispatch triad is: **`case`** chooses by _shape/value_ — as an
expression here, and as a reactive **mode-dispatch equation** in models (§5.2:
exactly one branch per step, total definition); **`when`** chooses by
_occurrence/time_ (§5.6 — ordered, first-match, hold semantics); **`match`**
names _pattern conformance_ (typl constraint position today, expr infix
predicate later — typl §17.7c). Migration traps are known and diagnosed:
Rust/OCaml/GRust hands typing `match x {` and Kotlin hands typing `when (x)` for
value dispatch both get a fix-it suggesting `case`. `case` is also Ada's word —
the safety audience's own lineage. A `~` sigil for pattern matching was
considered and rejected: the family is deliberately sigil-poor for its
non-programmer audiences.)

### 4.4 Optionals

An optional value (`T?`) is consumed by `case` (`some v -> … , none -> …`) or by
the default operator:

```ridl
let cmd = setLever ?: LeverCmd.NONE     // occurrence absent this step → NONE
```

### 4.5 Bounded combinators

Over typl bounded arrays/maps — the only iteration in the language:

```ridl
function meanSpeed(xs: [Speed; 1..16]): Speed =
  fold(xs, 0.0, function(acc, x) = acc + x) / count(xs)

function anyCritical(fs: [FaultEvent; 0..32]): boolean =
  any(fs, function(f) = f.severity >= 4)
```

Anonymous `function(…) = expr` literals are permitted **only** as combinator
arguments (no closures over mutable anything — there is nothing mutable; no
escaping function values — RMDL-109). `map` preserves bounds; `fold` consumes
them; iteration count ≤ the typl bound, always.

---

## 5. The Model

### 5.1 Declaration

A model is declared with an explicit signature — inputs, then outputs after `:`.
**There is no `realizes` clause** (dropped by decision — §7): a model names no
contract. Every flow carries a **kind** — `signal` (continuous state, sampled)
or `event` (discrete occurrence) — the interact core's two data-carrying
primitives at the model boundary:

```ridl
// a reusable behaviour brick — pure reaction, contract-blind
model Latch(signal set: boolean, signal reset: boolean) : (signal out: boolean) {
  init out = false
  out = if reset then false
        else if set then true
        else last out
}

// a controller — still just a signature; the CruiseControl service is bound
// to this reaction by an rsdl component, not named here
model CruiseController(signal current: Speed, signal brake: boolean, event lever: LeverCmd)
                     : (signal engaged: boolean, signal target: Speed) {
  ...
}
```

- `signal name : T` — a state flow: inside the body it reads as a plain `T`,
  sampled at step start (latest value, provenance-aware)
- `event name : T` — an occurrence flow: inside the body it reads as `T?` —
  present with its payload in a step where the occurrence arrived, absent
  otherwise. A payloadless occurrence is `event name` (unit payload)
- Unmarked parameters default to `signal` (the common case); assurance profiles
  may require explicit kinds
- **All inputs are just parameters.** There is no distinction between "contract"
  inputs and "abstract" inputs anymore — a model has inputs, full stop. An rsdl
  component wires each input to a real signal (a service member or another
  reaction's output) and binds the outputs to a service it provides (family open
  question 1, resolved: rsdl-wired)
- A model body is a set of **equations** (§5.2), `when` blocks (§5.6), `let`
  locals, and instantiations. Equations are unordered — the compiler schedules
  them by data dependency (§5.4)

### 5.2 Equations

Each output and each local flow is defined by exactly one equation (RMDL-201:
multiply defined; RMDL-202: undefined output):

```ridl
out      = expr            // flow definition
let mid  = expr            // local flow (single assignment, like everything)
```

**Case equations — mode dispatch.** `case` also has a _reactive_ form (GRust's
match equation): dispatch on a value that exists every step — typically a mode
signal — selecting which **set of equations** is active:

```ridl
case drivingMode {
  ECO   -> { torqueLimit = ECO_LIMIT,  boost = 0.0 }
  SPORT -> { torqueLimit = MAX_TORQUE, boost = boostCurve(rpm) }
  else  -> { torqueLimit = STD_LIMIT,  boost = 0.0 }
}
```

Semantics — deliberately the _opposite discipline_ from `when` (§5.6):

|                   | `case` equation                                                 | `when` block                                  |
| ----------------- | --------------------------------------------------------------- | --------------------------------------------- |
| selected by       | a **value**, present every step                                 | an **occurrence** or edge                     |
| branches per step | exactly one, always                                             | zero or one                                   |
| definition        | **total** — every branch defines the _same_ flow set (RMDL-213) | partial — signals **hold** where unassigned   |
| exhaustiveness    | required, `else` as catch-all (RMDL-108)                        | not applicable (no branch matching is normal) |
| `emit`            | not permitted — a mode is not an occurrence (RMDL-212)          | the only place events are raised              |

One keyword, one concept, two positions — `case` chooses by shape/value whether
as an expression (§4.3) or as an equation block; the same pattern as `match` and
`init`. The single-definition rule (above) holds at block granularity: the case
block is _the_ equation of every flow it defines.

### 5.3 Memory — `last` and `init`

_(GRust's memory model, adopted — replacing the draft's Lustre `pre`/`->`;
decision note below.)_ Two constructs:

- `last x` — the value of flow `x` at the **previous step**. **Total**: defined
  even at the first step, because every memory has a seed
- `init x = e` — the seed equation: what `last x` yields at the first step. `e`
  is evaluated once, at the first step; it may reference constants and the
  first-step values of inputs (capture-at-t₀), and nothing else (RMDL-210)

Seeding rules:

- When an rsdl component binds a flow to a typed channel (a service member),
  that flow is **implicitly seeded with its channel init value** (ridl §4.4,
  typl §5.8): the never-empty channel gives never-empty memory, one doctrine end
  to end. A standalone model flow with no `init` and no binding is seeded per
  the derived rules of typl §5.8
- Any other flow read through `last` requires an explicit `init` equation
  (RMDL-203 — a _presence check_, not a dominance analysis)
- A `when` block's `init` branch _is_ the init equations of the signals it
  defines (§5.6) — one initialization mechanism everywhere

```ridl
init out = false
out = if reset then false
      else if set then true
      else last out                    // the latch idiom

edgeUp = x && !last x                  // edge detection — x seeded by its channel init

init n = 0
n = last n + 1                         // counts 1, 2, 3, … — the seed is PRE-HISTORY, not the first output
```

Note the counter: the seed is the value _before the first step_; the first
output is computed from it. (Lustre's `0 -> pre n + 1` made the init the first
_output_ — the seed model is the one that composes with `when`-init and channel
init values.)

**Decision note — why `last`/`init` over `pre`/`->`** (settled in design
review): memory is total by construction — no nil value at the first instant,
and no initialization-dominance analysis to specify, implement, and
tool-qualify; `last speed` reads as English where `pre` is Lustre jargon (the
argument that killed `node`, applied consistently); one initialization mechanism
instead of the draft's two coexisting styles (`->` in equations, `init` branches
in `when`); and seeds existing at construction align with the ridl §4.4 channel
doctrine. Given up: `->`'s expression-valued first _outputs_ — covered in
practice by `when` init branches and by `init` expressions referencing
first-step inputs (the capture-at-t₀ case GRust's constant-only `init` could not
express).

### 5.4 Causality

Within a step, the dependency graph of equations must be acyclic **except
through `last`** — `last` is the only legal way a flow depends on itself
(RMDL-204: instantaneous cycle). The compiler's schedule is a topological order;
authors never order equations by hand.

### 5.5 Composition — instantiation

Models instantiate models; each instantiation site owns its instance's state:

```ridl
model CruiseController(signal current: Speed, signal brake: boolean, event lever: LeverCmd)
                     : (signal engaged: boolean) {
  let cmd = lever ?: LeverCmd.NONE

  let (engagedRaw) = Latch(cmd == LeverCmd.ENGAGE,
                           brake || cmd == LeverCmd.CANCEL)

  engaged = engagedRaw
  ...
}
```

Instantiated models step **synchronously with their parent** — one step, one
atomic reaction through the whole tree (§6.5). Instantiation is static: no
conditional instantiation, no arrays of instances in v0.1 (§12.4).

### 5.6 Event Flows — `when` and `emit`

_Adopted from GRust (Appendix D), adapted to family syntax._ The `when` block is
the event-triggered equation form — the natural surface of the scheduled-step
execution model: it says _what happens when_, and its trigger conditions are
exactly the step-activation causes of §6.1.

```ridl
when {
  init                    -> { engaged = false, targetSpeed = SPEED_LIMIT_EU }
  brake                   -> { engaged = false }
  setLever? if setLever == LeverCmd.CANCEL
                          -> { engaged = false }
  setLever? if setLever == LeverCmd.ENGAGE
                          -> { engaged = true, targetSpeed = current
                               emit engagedChanged = true }
}
```

**Branch conditions** — five forms:

| Condition              | Triggers when                                                    |
| ---------------------- | ---------------------------------------------------------------- |
| `init`                 | the first step — initializes the signals this block defines      |
| `e?`                   | the occurrence `e` arrived this step                             |
| `e? if c`              | `e` arrived and `c` holds                                        |
| `(e1?, e2?)`           | both occurrences arrived this step                               |
| boolean expression `c` | `c` has a **rising edge** — false at the previous step, true now |

**Semantics:**

- Branches are **ordered; the first matching branch runs, alone** — one branch
  per step. A branch subsumed by an earlier, less restrictive one is a linter
  finding (RMDL-207)
- **Signals hold**: a signal defined by a `when` block keeps its previous value
  in any step where no branch assigns it — the latch generalized. The `init`
  branch supplies the first value (mandatory for every signal the block defines,
  RMDL-209); it is _initialization, not default_ — once another branch has run,
  `init` is unreachable
- **Events don't hold**: an event output is raised with `emit name = expr`
  inside a branch and is **absent in every step where nothing emits it**. `emit`
  is valid only inside `when` branches and only targets `event`-kind flows
  (RMDL-208)
- Inside a branch, ordinary equations and `let`s apply; across branches, the
  block is one equation per flow it defines (§5.2's single-definition rule holds
  at block granularity)

`last`/`init` remain the primitive memory; `when` compiles to them. Use plain
`last`-recurrence equations for numeric state evolving every step (integrators,
filters); use `when` for mode logic driven by occurrences and edges.

### 5.7 Side Effects Are Emissions

A model **never calls anything**. The only way behaviour affects the world is
data: signal outputs (state) and event emissions (occurrences). An emitted event
becomes a _side effect_ only at the wiring layer — **rsdl binds a model's output
event to another service's command**:

```
# rsdl (manifest sketch)
instance cruise : CruiseController
  bind engagedChanged -> veh.cluster.Indicators.setCruiseLamp   # event → command
```

This is the family's whole side-effect story, and it is load-bearing: the model
stays pure and replayable (its step trace fully determines its emissions), the
command's ack/retry machinery (ridl §6.1) belongs to the wiring not the
behaviour, and the test plane can observe every would-be side effect as data
without executing it.

---

## 6. Execution — Steps, Not Ticks

**The defining decision of this language** (settled in design review): rmdl is
_reactive but not periodic_. A model is a state machine whose **step** — one
synchronous reaction — is **scheduled by the runtime, based on inputs and
constraints, only when required**. There is no free-running base clock; there is
no polling.

### 6.1 What triggers a step

The runtime activates a model when:

1. **An input arrives** — an occurrence or update on an input flow, or
2. **An output deadline falls due** — a refresh ceiling (`@[..max]`) or strict
   period (`@Xms`) of an output flow, demanded by the service the binding
   component provides (§7), or an occurrence TTL expiry

and **coalesces** activations per those same bounds: an output's `min` bound
(debounce) is a floor on step spacing attributable to that flow — the bound
service's `@[min..max]` _is_ the scheduler's constraint set, supplied by the
binding component. Strict periodic (`@10ms`) degenerates to periodic stepping:
classic Lustre is the special case, not the default.

Between steps the model is quiescent: state persists, nothing computes, nothing
is polled. A model with no input changes and no deadline pending consumes
nothing.

The `when` branch conditions (§5.6) are these activation causes made syntactic:
`e?` is input-triggered activation, a rising-edge condition is change-triggered
activation — what schedules the step is what the equation reads.

### 6.2 The step contract

A step is **atomic and synchronous**: inputs are sampled once at step start (a
consistent snapshot), all equations evaluate against that snapshot per the
causal schedule, outputs publish at step end. The synchronous hypothesis holds
as in the Lustre tradition — the reaction is logically instantaneous relative to
its environment; physically, per-step WCET is decidable (§3.2) and the deadline
analysis is the scheduler's obligation.

Input sampling per kind: **state flows** (signals/displays, abstract inputs)
sample their latest value — provenance included (`init/live/invalid`, ridl
§4.5), so a model can _see_ that an input is invalid and behave accordingly;
**occurrence flows** (events/commands/inputs) deliver **at most one occurrence
per step, in envelope order** — a queue drains one occurrence per activation,
preserving determinism (a burst of N commands yields N steps, not one step with
a list).

### 6.3 Step context — `now` and `dt`

Aperiodic execution makes elapsed time a first-class need (an integrator cannot
assume a fixed period). Every model body may read the **ambient time values** —
the step never appears; the author reads _the current instant_ and _the elapsed
time_:

| Expression | Type        | Meaning                                                                                                                                                                                                                                |
| ---------- | ----------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `now`      | `Timestamp` | the reaction's **logical time** — the time of its _cause_, assigned by the runtime (below); system time domain (ridl §3.1). Conceptually an implicit ambient **signal of time** — the FRP Behavior of time, sampled at each activation |
| `dt`       | `Duration`  | elapsed logical time since this instance's previous activation — **definitionally `now - last now`**, seed making it `0` at the first reaction                                                                                         |
| `time(f)`  | `Timestamp` | the envelope timestamp of flow `f`'s current value (signal) or this step's occurrence (event) — sender-stamped (ridl §3.1)                                                                                                             |

**Logical time, not execution time** (decision on record). `now` is the time
**of the step's cause**, never the wall-clock moment the code happens to run:
for an input-triggered step, the triggering occurrence's envelope (sender)
timestamp; for a deadline-triggered step, the deadline instant itself. The
runtime _assigns_ it; the model _computes with_ it. Consequences:

- **Time physics is exact and jitter-immune.** An integrator (`last x + v * dt`)
  integrates over the time that _physically elapsed between causes_, regardless
  of scheduling latency, CPU load, or how late the step actually executed
- **Replay is not a special mode.** Replaying a step trace — or computing over
  _past_ events in a backlog — feeds the recorded logical times: the model runs
  the same instants, the same `dt`s, the same physics, bit-for-bit. There is no
  "recompute against today's clock" failure class, because today's clock is not
  reachable from a model
- **Scheduler lag is observable, not corrupting.** The runtime tracks
  `physical − logical` per step as a health metric (feeding freshness/deadline
  supervision, ridl §10.4); when lag exceeds the contract's bounds that is a
  _detected timing failure_ — the model's arithmetic stays correct either way

**Time is language, not library** (decision on record). GRust exposed current
time as a library operator (`time()` in GReact); rmdl deliberately promotes it:
`now`, `dt`, and `time(f)` are language-level, because time is what contracts
constrain (§9.2) and what the scheduler reasons about — it cannot be an optional
import. The complementary decision: **async/await is runtime, not language**
(§6.6) — the two concepts sit on opposite sides of the surface, by design.

**One time, and only one** (decision on record). The language knows a single
notion of time: the platform instant — system time domain, TAI/PTP epoch (ridl
§3.1) — frozen per step. There is no datetime type, no wall-clock, no second
clock domain at the surface; civil datetime, time zones, leap handling, and
clock-domain mechanics are **absorbed by the runtime** and the presentation
edges. Any confusion between "time" and "datetime" is therefore not expressible
in a model — which is the point.

```ridl
// aperiodic integrator — correct under any activation pattern
init distance = 0.0
distance = last distance + current * dt
```

`step` is a keyword; the context is read-only and step-scoped. Functions cannot
read it (§3.2) — a computation that needs `dt` takes it as an ordinary
parameter, staying timeless.

### 6.4 Determinism and replay

A model's execution is fully determined by its **step trace**: the sequence of
(input snapshot, logical `now`) pairs. Same trace, same outputs — bit-for-bit,
on every target (Rust native and WASM produce identical traces; float evaluation
is IEEE 754, no fused reassociation). Because `now` is logical (§6.3), live
execution, backlog processing, and replay are **the same computation over the
same times** — the runtime records step traces from the envelope (ridl §3.1),
and replaying a field incident is re-feeding a trace. This is the
reference-oracle machinery of concept note §9.2, inherited by construction.

### 6.5 Composition semantics

An instantiated model has no scheduler of its own — it steps when its parent
steps, synchronously. One activation, one atomic reaction through the
instantiation tree; `now`/`dt` are uniform across the tree for a given
activation. (Whether a _sub_-model may declare its own activation constraints —
multi-activation, the successor of Lustre's multi-rate clocks under this
execution model — is deferred: §12.1.)

### 6.6 Parallelism by Causality

The causal analysis (§5.4) produces more than a schedule — it produces a
**partition**. Equations with no dependency path between them form independent
islands of the dataflow graph, and the compiler may compile each island as its
**own step unit**, activated by its own triggering inputs and constraints,
executed concurrently by the runtime (the GRust precedent: causal analysis
feeding an asynchronous Rust runtime).

The guarantee is **observational equivalence**: per flow, the sequence of values
and timestamps is identical to the single-step synchronous semantics —
parallelism is a compiler/runtime optimization, never a semantic choice the
author makes. Consequences:

- **`async`/`await` never appear in the language.** Concurrency, task spawning,
  and completion are `ridl-rt`'s business; generated code may be a single loop
  or a task-per-island — the model text and its step traces are identical either
  way
- An island that shares no inputs with another needs no synchronization with it
  — this is what dissolves the "everything wakes at 10ms" bus-and-CPU waste the
  thesis documents (97% unused periodic traffic at Ampère): only the island
  whose input changed computes
- Islands are a _unit of replay_ too: a per-island step trace is smaller and
  independently re-runnable
- The partition is derived, never declared — no `parallel` keyword exists or
  will

### 6.7 The Scheduler Has No Clock — It Has a Timeline

The runtime scheduler is not driven by a clock; it maintains a **timeline**: an
ordered set of _due instants_, each carrying its cause, **projected from inputs
and real time**:

| Instant source   | Projected from                                                                                                                           |
| ---------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| input arrival    | the occurrence's sender-stamped envelope time (logical)                                                                                  |
| output deadline  | the bound service's `@[..max]` refresh ceiling / `@Xms` period, anchored to the last publication (supplied by the binding component, §7) |
| coalescing floor | the contract's `min` bound — the earliest next instant a flow may cause                                                                  |
| timer            | future instants a watchdog/timeout registers (§12.9)                                                                                     |

The scheduler advances along the timeline, executing each instant as a step
whose `now` is that instant (§6.3). **Real time has exactly one role: gating.**
In live mode, an instant may not execute before the synchronized clock reaches
it — and executes as soon after as the platform allows, the difference being the
lag metric. In replay or backlog processing, the gate is simply open: the
timeline drains at compute speed, and nothing else changes — same instants, same
steps, same physics.

This is what "reactive, not periodic" means operationally: there is no tick
looking for work — instants exist only where causes exist, and a quiescent
system has an empty timeline. (Precedent: Lingua Franca's event queue over
logical time; the timeline is that idea with the instants _derived from the
contracts_ rather than declared.)

---

## 7. Models Are Contract-Blind

**Decision on record (this section replaces a former "Realization" section).** A
model does **not** name, realize, or expose a contract. It is a pure reaction:
`(O, S) = M(I, S)`, a signature of typed input and output flows with internal
state. Binding that reaction to a ridl `service` or uxdl `view` is entirely an
**rsdl component**'s job (rsdl §3–§4). This purifies the layer boundary: ridl
declares contracts, rmdl computes, rsdl connects — and rmdl references ridl only
for the _kinds_ (`signal`/`event`) and typl for _types_, never for contracts.

What used to be "realization" is now three things the component supplies from
outside, none of them in the model:

| Concern                                   | Where it lives now                                                                                                                                                   |
| ----------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| which contract member each flow maps to   | the component's `provides`/`requires` wiring (rsdl §4) — by application notation `(engaged, target) = M(current, brake, lever)`                                      |
| output **timing** (`@10ms`, refresh, TTL) | the **service** the component provides (ridl §14.5); its bounds drive the model's output-deadline demand (§6.1). The model itself carries no timing                  |
| output **init value**                     | the bound channel's init (ridl §4.4); if the model also writes `init out = …`, the component checks the two agree (a boundary check in rsdl, not here)               |
| contract `require`/`ensure`               | the service's clauses, compiled by the component as observers over the bound flows (§9). A model may _also_ carry its own `require`/`ensure` (§9.2); the two compose |

Consequences for the model author: write inputs and outputs as plain kinded
flows; do not annotate timing (there is nothing to annotate — a model reacts to
input arrivals and to output-deadline demands passed in by its binding, §6.1);
seed memory with `init` where first-step behaviour matters. The reaction is
complete and testable on its own — the test plane drives its inputs and reads
its outputs without any contract present.

**Command / query, at the model boundary.** A ridl `command` a service accepts
arrives at the reaction as an `event` input (`T?` per step, §6.2), typically
consumed in a `when` branch; the component routes it. A `query` is not realized
by model equations in v0.1 — the direction of travel is a pure `function` over
the model's state flows (§12.2), invoked by the component, consuming no step.

---

## 8. Step Faults

Total functions leave exactly two runtime hazards: **division by zero** (and `%`
by zero) and **range violation** when a value crosses a typed boundary (function
result, model output, contract flow — typl constraints re-apply there, §3.3).
Both are **step faults**, and the semantics are transactional:

1. The faulting **step aborts atomically** — no partial state update, no partial
   outputs; the model's memory (`last` state) remains that of the last completed
   step
2. **Output flows transition to the invalid state** (ridl §4.5) — subscribers
   _see_ that the producer faulted, with last-good values retained; nothing is
   silently stale
3. The fault is **recorded** — step trace, flow, equation, fault kind — through
   the observability hooks; a negative ack is returned where the triggering
   input was an acked occurrence (ridl §6.1)
4. What happens _next_ — retry the step without the poisoning input, hold,
   degrade, failsafe — is **failure management** (ridl §10.4), not language
   semantics. The language's job ends at: no fault is silent, no fault corrupts
   state

Statically excluded faults stay excluded: exhaustiveness (§4.3), causality
(§5.4), initialization (§5.3), and unit discipline (§3.3) are compile-time;
where a divisor or boundary can be proven safe (ranges exclude zero; result
range ⊆ target range), the check is discharged at compile time (RMDL-105 asks
for a guard only when it cannot).

---

## 9. Observers

An observer is a boolean flow evaluated every step, with no effect on the
model's outputs — the four-way assertion machinery (concept note §9.2) anchored
in the language:

- **Model-local observers**: a model's own `require`/`ensure` clauses (§9.2),
  evaluated every step — a violated `ensure` is a `CONTRACT_BROKEN`-class
  finding (ridl §10.2), surfaced per §8's recording rules
- **Service observers**: when an rsdl component binds this reaction to a
  service, that service's `require`/`ensure` clauses are compiled by the
  component as observers over the bound flows — they apply at the component, not
  written here (§7)

Observers never write flows, never fault the step (a false observer is a
_finding_, not a fault), and are compiled out of production builds or kept, per
deployment (rsdl) — the reference-oracle build keeps them all.

### 9.2 Contracts on Functions and Models

_GReusot-inspired (the thesis's verification extension), family syntax._
Functions and models carry the same `[ require … ensure … ]` attribute block as
ridl interactions — the grammar supports expressing **safety and performance
requirements** directly on behaviour:

```ridl
function isqrt(x: integer [0..1000000]): integer [
  ensure result * result <= x
] { ... }

model CruiseController(signal current: Speed, signal brake: boolean, event lever: LeverCmd)
                     : (signal engaged: boolean, signal target: Speed,
                        event disengaged: DisengageCause) [
  // safety invariant — evaluated every step
  ensure engaged implies !brake

  // performance requirement — time is in the language (§6.3), so deadlines are contract terms:
  // whenever disengagement is emitted, it came within 50ms of the brake edge
  ensure disengaged? implies (now - time(brake)) <= 50ms
] { ... }
```

- `require` — assumption on inputs, checked at the boundary before the body runs
  (violation is a Stratum 2 contract error, ridl §10.2)
- `ensure` — guarantee over outputs/flows: on a function, over `result`; on a
  model, evaluated **every step** as an observer (§9.1)
- Contract terms use the full expression grammar plus the temporal vocabulary:
  `last` (past values), `now`/`time(f)` (deadlines, latency, freshness), and
  `e? implies …` — the eventful conditional (GReusot's `when EPat ⇒ Term`)
- Verification is layered: the four ways (static where decidable, CI property
  tests, online observers, reference oracle) **plus a fifth the thesis proved
  out — deductive proof**: the clause grammar is deliberately
  Creusot-compatible, so function and model contracts can be discharged by a
  deductive verifier where they are provable, not just tested

This resolves the §9.1 deferral in the affirmative for the _positions_; the full
term grammar still lands with the expr core.

---

## 10. Conventions

- Functions for mathematics, models for memory — if a model reads no `last`, it
  should be a function (linter, RMDL-501)
- Name models as controllers/filters/estimators (`CruiseController`,
  `SpeedFilter`); functions as verbs or formulae (`clamp`, `interpolate`,
  `meanSpeed`)
- Write the `init` equation adjacent to the equation that reads `last` — the
  seed and the recurrence review as a pair. Channel-bound flows are implicitly
  seeded (§5.3), but an explicit `init` on one is legal and preferred when
  first-step behaviour is safety-relevant
- Guard divisors with `if`/`?:` at the use site, not with wide try-anything
  ranges
- Keep abstract inputs few and typed narrow — every abstract input is a wiring
  obligation on rsdl and a test-plane injection point

---

## 11. Diagnostics

Coded `RMDL-`, same lifecycle rules as typl §16.

### 11.1 Functions and Expressions (RMDL-1xx)

| Code     | Rule                                                                  | Severity |
| -------- | --------------------------------------------------------------------- | -------- |
| RMDL-101 | recursion, direct or mutual                                           | error    |
| RMDL-102 | `last` or `init` inside a `function`                                  | error    |
| RMDL-103 | `now` or `dt` inside a `function`                                     | error    |
| RMDL-104 | `let` reassignment or shadowing                                       | error    |
| RMDL-105 | division/modulo whose divisor is not provably nonzero and not guarded | error    |
| RMDL-106 | cross-unit arithmetic without an explicit conversion                  | error    |
| RMDL-107 | `if` without `else`                                                   | error    |
| RMDL-108 | non-exhaustive `case` (no `else`, arms incomplete)                    | error    |
| RMDL-109 | function literal outside a combinator argument position               | error    |
| RMDL-110 | flow declared with the name `now` or `dt` inside a model              | error    |

### 11.2 Models (RMDL-2xx)

| Code     | Rule                                                                         | Severity |
| -------- | ---------------------------------------------------------------------------- | -------- |
| RMDL-201 | flow defined by more than one equation                                       | error    |
| RMDL-202 | declared output with no defining equation                                    | error    |
| RMDL-203 | flow read through `last` with neither a channel seed nor an `init` equation  | error    |
| RMDL-204 | instantaneous dependency cycle (cycle not broken by `last`)                  | error    |
| RMDL-205 | conditional or dynamic instantiation                                         | error    |
| RMDL-206 | unused input or abstract input                                               | warning  |
| RMDL-207 | `when` branch unreachable — subsumed by an earlier, less restrictive branch  | warning  |
| RMDL-208 | `emit` outside a `when` branch, or targeting a non-`event` flow              | error    |
| RMDL-209 | signal defined by a `when` block without an `init` branch value              | error    |
| RMDL-210 | `init` expression references anything beyond constants and first-step inputs | error    |
| RMDL-211 | duplicate `init` equation for the same flow                                  | error    |
| RMDL-212 | `emit` inside a `case` equation — a mode is not an occurrence                | error    |
| RMDL-213 | `case` equation branches define differing flow sets                          | error    |

### 11.3 Signatures (RMDL-3xx)

| Code     | Rule                                                                               | Severity |
| -------- | ---------------------------------------------------------------------------------- | -------- |
| RMDL-301 | declared output flow with no defining equation                                     | error    |
| RMDL-302 | `realizes` (or any contract reference) in a model — models are contract-blind (§7) | error    |
| RMDL-303 | input flow never read                                                              | warning  |

(Contract mapping, timing transfer, and init-consistency checks moved to rsdl,
where the component binds a reaction to a service — rsdl §4, §8.)

### 11.4 Boundaries (RMDL-4xx / 5xx)

| Code     | Rule                                                          | Severity |
| -------- | ------------------------------------------------------------- | -------- |
| RMDL-401 | model dataflow across an async boundary (the sync/async wall) | error    |
| RMDL-402 | behaviour declaration in a non-`.rmdl`/`.rxdl` context        | error    |
| RMDL-501 | model reading no `last` (should be a function)                | warning  |

---

## 12. Open Questions

1. **Multi-activation** (successor of multi-rate clocks). `when` found its role
   as the event-triggered equation form (§5.6); what remains open is whether a
   _sub-model_ may declare its own activation constraints (step slower/faster
   than its parent) — `merge`/`current` stay reserved for that clock calculus if
   it is ever needed.
2. **Query/fetch behaviour.** Direction of travel: a query answered by a **pure
   function over the model's state flows** — request arrives, function evaluates
   against current state, no step consumed (or an explicitly step-consuming
   variant for mutating queries, which CQRS says shouldn't exist). The component
   routes the query to that function; needs the expr/function story §3 proved
   first.
3. **State-machine sugar.** `automaton`/`state`/`transition` reserved; compiles
   to enum + latch equations; lands once the equation core is proven in real
   models.
4. **Static instance arrays.** `[Latch; 4]`-style instantiation for symmetric
   structure (per-wheel controllers). Bounded, so admissible in principle;
   deferred for grammar and diff-story clarity.
5. **Saturation vs fault at typed boundaries.** §8 makes range violation a step
   fault; control practice sometimes wants declared saturation instead (`clamp`
   semantics at the boundary). Candidate: a per-flow attribute (`[ saturate ]`)
   opting into clamping — property, not mechanism, per the safety-extension
   doctrine (ridl §10.4). Evidence first.
6. **Standard behaviour library — semantics re-derived against the
   signal/event/timeline model.** Two Rust-implemented stdlib packages:
   `ridl.std.math` (pure `function`s: clamp, lerp, abs/min/max, interpolation
   tables, `dt`-correct filter/integrator kernels) and `ridl.std.flow` (adapter
   `model`s). GReact's vocabulary is re-based with audience-correct names (its
   `scan`/`throttle` collide with Rx tradition) **and** with kind signatures,
   seed behaviour, and timeline classification made explicit — the analysis
   GReact could skip because its services were quasi-periodic, ticks ambient:

**Instant-preserving adapters** — ride existing steps, create no timeline
instants; implementable with v0.1 semantics as ordinary models:

| Adapter                  | Kind signature              | Semantics (seed behaviour included)                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| ------------------------ | --------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Changes(s)`             | signal T → event T          | emits when `s != last s`; the seed is the comparison base, so a first live value equal to the channel init emits nothing — no spurious start-of-world occurrence (GReact `on_change`)                                                                                                                                                                                                                                                                                 |
| `Hold(e, init)`          | event T → signal T          | zero-order hold; `init` seeds the output channel explicitly (GReact `persist`, which left the pre-first-occurrence value implicit)                                                                                                                                                                                                                                                                                                                                    |
| `Deadband(s, Δ)`         | signal T → signal T         | forwards only when \|value − last forwarded\| > Δ; seeded by the channel init (GReact `throttle` — renamed: mainstream throttle is time-rate limiting; this is the control term)                                                                                                                                                                                                                                                                                      |
| `Accumulate(e, init, f)` | event T → signal U          | running fold over occurrences — `scan`'s _actual_ Rx meaning, restored under its honest name; sugar for `init acc = init` + `when e? -> acc = f(last acc, e)`                                                                                                                                                                                                                                                                                                         |
| `Merge(e1, e2)`          | event T × event T → event T | forwards whichever occurrence is present. **Same-step simultaneity is real in our model** (two flows may each deliver in one step — the `when (e1?, e2?)` case): `e1` wins, `e2`'s occurrence is _dropped and counted_ (observability), because a step emits at most one occurrence per flow and inputs cannot be pushed back. GReact's priority rule, made loss-explicit — prefer binding both events into one model's `when` branches when every occurrence matters |

**Instant-creating adapters** — they _register timer instants on the timeline_
(§6.7); gated on the §12.9 timer capability, because they are scheduling
constructs, not dataflow:

| Adapter                         | Kind signature                                                                          | Semantics                                                                                                                                                                                                                 |
| ------------------------------- | --------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Clock(d: Duration) : event ()` | the primitive instant source: an occurrence at every multiple of `d` from instantiation | see rationale below                                                                                                                                                                                                       |
| `Sample(s, d)`                  | signal T → **event** T                                                                  | emits `s`'s current value at each timer instant of period `d`. _Kind corrected from GReact's `scan(s,d) : signal`_ — a periodic reading is a sequence of occurrences; it only becomes state again if something `Hold`s it |
| `Coalesce(e, d)`                | event T → event T                                                                       | at most one occurrence per window `d`, keeping the latest; the consumer-side counterpart of the contract's provider-side `min` throttle                                                                                   |
| `Watchdog(e, d)`                | event T → event `()`                                                                    | absence detection: arms a timer instant at `time(e) + d`, re-armed on each occurrence, fires if reached (GReact `timeout`)                                                                                                |

**The architectural finding**: several GReact operators are, in this family,
**not library calls but channel timing annotations** — `scan(s, 10)` on an
exported flow is realizing the signal `@10ms`; time-throttling is the contract's
`min` bound; TTL is `max`. GRust needed them as operators because timing lived
in service bodies; we pushed timing into contracts, so `ridl.std.flow` only
serves _internal_ flows and consumer-side reshaping. `time()` is likewise not an
operator here — time is language (§6.3). **The retained set with normative
definitions: Appendix G.** 9. **Timeout / reaction to absence.** A watchdog —
_raise an event if no occurrence within d_ (GRust's `timeout(e, d)`) — needs the
scheduler to fire **timer-triggered steps**, a third activation cause beyond
§6.1's two. Semantically clean under scheduled steps; deferred until the
scheduler is formalized. Likely surface: a stdlib `Timeout` model or a
`when … timeout d ->` branch form. 7. **WCET annotation surface.** Whether
models may declare per-step budgets (`@wcet 200us`?) for the scheduler to verify
against, or budgets stay entirely in deployment (rsdl). Leaning rsdl. 8. **Unit
algebra.** Cross-unit multiplication/division with derived dimensions
(`Speed * Duration : Length`) — typl §17.5's question, felt most acutely here.
RMDL-106's explicit-conversion rule is the stopgap.

---

## Appendix A — Full Example

The cruise control of concept note §6, reworked to the settled design — unified
`model`, abstract inputs, scheduled steps, `dt`:

```ridl
package veh.adas.model

import veh.common.Speed
import veh.common.LeverCmd
import veh.common.DisengageCause
import veh.common.SPEED_LIMIT_EU
import veh.common.DisengageCause

// ---------- function layer — pure, total, shared with expr ----------

function clamp(x: Speed, lo: Speed, hi: Speed): Speed =
  if x < lo then lo else if x > hi then hi else x

// ---------- the model — a pure reaction, contract-blind (§7) ----------

/**
 * Cruise control behaviour. Inputs and outputs are plain kinded flows;
 * no contract is named. An rsdl component binds this reaction to the
 * CruiseControl service, wiring `current`/`brake`/`lever` to real signals
 * and providing `engaged`/`target`/`disengaged`.
 * @labels SIL_2
 */
model CruiseController(signal current: Speed, signal brake: boolean, event lever: LeverCmd)
                     : (signal engaged: boolean, signal target: Speed,
                        event disengaged: DisengageCause) {

  // mode logic — event-triggered, signals hold between branches (§5.6)
  when {
    init -> { engaged = false, target = SPEED_LIMIT_EU }

    brake -> { engaged = false                     // rising edge of a signal
               emit disengaged = DisengageCause.BRAKE }

    lever? if lever == LeverCmd.CANCEL
          -> { engaged = false
               emit disengaged = DisengageCause.DRIVER }

    lever? if lever == LeverCmd.ENGAGE
          -> { engaged = true, target = current }   // capture-on-engage
  }

  // numeric state — evolves every step, aperiodic-safe (§6.3)
  let err   = target - current
  init integ = 0.0
  let integ = if engaged then clamp(last integ + err * dt, -50.0, 50.0)
              else 0.0
}
```

The seam, end to end: `Speed` arrives with unit, range, and step (typl) — the
equations inherit dimensional checking and boundary enforcement; the model names
**no** timing and **no** contract — an rsdl component binds it to
`CruiseControl`, and the service's `@20ms` becomes the scheduler's deadline
(§6.1) at that binding; `lever` is the command routed in as an `event`; `dt`
makes the integral correct whether steps come at 20ms, on command bursts, or on
refresh deadlines.

---

## Appendix B — Codegen Targets

Per the platform decision (concept note §8.3): **behaviour compiles twice — Rust
native and WASM — bindings go everywhere.**

| Aspect                 | Rust (native)                                                                     | WASM component                                                              |
| ---------------------- | --------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| model                  | a struct (state) + `fn step(&mut self, inputs: &Inputs, ctx: StepCtx) -> Outputs` | same, behind the component model; WIT signature generated from the contract |
| function               | `fn` — pure, `#[no_panic]`-style discipline                                       | core function                                                               |
| `last` state (+ seeds) | struct fields, initialized at construction                                        | linear memory, initialized at instantiation                                 |
| step scheduling        | `ridl-rt` scheduler: input arrival + deadline queue from contract bounds          | host runtime drives `step()` — wasmtime on ECU/edge, browser beside uxdl    |
| step faults            | `Result`-shaped step return → invalid-state propagation via bindings              | trap-free: fault is a returned status, never a WASM trap                    |
| determinism            | IEEE 754 strict, no fast-math, fixed evaluation order                             | identical — the reference-oracle guarantee (§6.4)                           |
| observers              | compiled in for oracle/test builds, out for production (rsdl choice)              | always available in oracle builds                                           |

Kotlin/TypeScript get **bindings** to models (call a WASM build, subscribe to
its flows), never behaviour codegen — the matrix stays collapsed.

---

## Appendix C — Formal Grammar (EBNF)

Additions to the typl grammar (shared productions per typl Appendix E):

```ebnf
definition    = [ "internal" ] ( typl_definition | function_def | model_def ) ;

(* ---------- Functions ---------- *)

function_def  = doc_comment? "function" camelCase_id "(" param_list ")" ":" type_ref
                attr_block? fn_body ;
              (* attr_block: [ require expr … ensure expr … ] — §9.2, ridl grammar *)
fn_body       = "=" expr | block ;
block         = "{" { let_binding sep? } expr "}" ;
let_binding   = "let" camelCase_id [ ":" type_ref ] "=" expr ;

(* ---------- Models ---------- *)

model_def     = doc_comment? "model" CamelCase_id signature
                attr_block? "{" { model_item sep? } "}" ;
signature     = "(" param_list ")" ":" "(" param_list ")" ;
              (* no `realizes`, no `returns` — a model is a pure signature; contract
                 binding is an rsdl component's job (§7). Params carry kinds (§5.1). *)

model_item    = equation | init_eq | let_binding | instance_bind | when_block | case_block ;

case_block    = "case" expr "{" { case_eq_branch sep? } "}" ;   (* mode dispatch — §5.2 *)
case_eq_branch = pattern "->" "{" { ( equation | let_binding ) sep? } "}" ;
equation      = camelCase_id "=" expr ;
init_eq       = "init" camelCase_id "=" expr ;          (* seed — §5.3; expr limited to
                                                           constants and first-step inputs *)
instance_bind = "let" "(" camelCase_id { "," camelCase_id } ")" "="
                CamelCase_id "(" arg_list ")" ;

(* ---------- When blocks — §5.6 ---------- *)

when_block    = "when" "{" { when_branch sep? } "}" ;
when_branch   = when_cond "->" "{" { ( equation | let_binding | emit_stmt ) sep? } "}" ;
when_cond     = "init"
              | camelCase_id "?" [ "if" expr ]
              | "(" camelCase_id "?" { "," camelCase_id "?" } ")"
              | expr ;                                   (* rising edge *)
emit_stmt     = "emit" camelCase_id "=" expr ;

(* ---------- Expressions ---------- *)

expr          = expr binop expr | unop expr
              | "if" expr "then" expr "else" expr
              | "case" expr "{" { case_arm sep? } "}"
              | expr "?:" expr                           (* default for optionals *)
              | "last" camelCase_id                      (* previous-step value — model only *)
              | "now" | "dt"                             (* ambient time values — contextual
                                                            identifiers, model context only *)
              | camelCase_id "(" arg_list ")"            (* function call / combinator *)
              | fn_literal
              | primary ;

case_arm      = pattern "->" expr ;
pattern       = SCREAMING_SNAKE_ID                       (* enum value *)
              | camelCase_id camelCase_id                (* union arm + binding *)
              | "some" camelCase_id | "none"             (* optional *)
              | "else" ;

fn_literal    = "function" "(" param_list ")" "=" expr ; (* combinator args only *)

binop         = "+" | "-" | "*" | "/" | "%" | "==" | "!=" | "<" | "<=" | ">" | ">="
              | "&&" | "||" ;
unop          = "-" | "!" ;
primary       = literal | camelCase_id | qualified_id | "(" expr ")"
              | expr "." camelCase_id ;                  (* field access *)
```

---

## Appendix D — Prior Art Survey

| Source                                                        | Taken / rejected                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| ------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **GRust** (langrust, Renault PhD — É. Thomé; thesis reviewed) | the closest sibling effort, convergent by shared ancestry. Structure: GRfrp (async services + import/export interfaces) · GReact (Rx-inspired operator library) · GRsync (functions/components, Lustre-derived, **clock-free by design**) · GReusot (requires/ensures contracts, deductively verified via Creusot). **Adopted**: `when`/`emit` (§5.6), event flows as `T?`, signal-hold semantics, event→command side effects (§5.7), causality-partitioned async compilation (§6.6), contract clauses on functions/models with deductive proof as a verification way (§9.2). **Adapted**: GReact's operator library → `ridl.std.flow` with corrected naming (§12.6: `scan`/`throttle` collide with Rx tradition). **Memory**: `last x` + `init x` **adopted outright** (§5.3), replacing the draft's Lustre `pre`/`->` — total memory, no nil, no dominance analysis; rmdl extends GRust's constant-only `init` to expressions over first-step inputs (capture-at-t₀) and adds implicit seeding from channel init values. **Differs**: component/service/interface split maps onto model + rsdl wiring + ridl contracts; `time()` as library operator → promoted to language (`now`, §6.3); untyped int/float vocabulary → typl units/ranges |
| **Lustre**                                                    | the foundation: equational dataflow, `pre`/`->`, causality analysis, the synchronous hypothesis. Departed from: the implicit periodic activation — rmdl steps are runtime-scheduled (§6); Lustre's semantics were always activation-agnostic in principle, rmdl makes it the definition                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| **SCADE**                                                     | the industrial proof that this lineage certifies (DO-178C/ISO 26262 qualified codegen); state-machine sugar studied and deferred (§12.3); its "one construct, hierarchic composition" informed collapsing node into `model`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| **Esterel / Signal**                                          | the imperative and relational siblings; Signal's polychrony is the intellectual ancestor of the multi-activation question (§12.1)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| **Lingua Franca**                                             | the closest contemporary for §6: event-driven reactions, logical time, deadlines as first-class scheduler constraints. Departed from: LF's distributed coordination ambitions — rmdl stays inside the sync island and lets rsdl own distribution                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| **Simulink**                                                  | the audience's home tool: `model` naming, the numeric-library expectation (§12.6), and the cautionary tale — untyped lines and implicit rate transitions are precisely what typl types and the sync/async wall forbid                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| **Zélus / hybrid synchronous**                                | continuous-time ODEs beside discrete flows — out of scope, but `dt` keeps the door open for host-side integration schemes                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| **Kotlin / OCaml**                                            | the `function` surface: expression-bodied definitions, `let` bindings, `?:`; OCaml's discipline that everything is an expression                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| **SPARK Ada / MISRA**                                         | the totality doctrine: no recursion, no unbounded loops, decidable WCET as a language property rather than a coding rule                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| **TLA+/model checkers**                                       | rejected as surface (specification-only, not executable-first), but the observer machinery (§9) is designed to hand equations to a checker later                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |

---

## Appendix E — Coverage Analysis: Behaviour Languages

✓ covered, ≈ covered differently, ✗ not expressible (deliberate or open).

| Construct                                                           | rmdl equivalent                                             | Status                                                       |
| ------------------------------------------------------------------- | ----------------------------------------------------------- | ------------------------------------------------------------ |
| Lustre node / SCADE operator                                        | `model`                                                     | ✓ unified                                                    |
| `pre`, `->`                                                         | `last` + `init` (seed model)                                | ≈ adopted from GRust — total, no nil                         |
| node instantiation, hierarchic composition                          | model instantiation, synchronous                            | ✓                                                            |
| Lustre clocks `when`/`merge`/`current`                              | multi-activation                                            | ✗ deferred §12.1, keywords reserved                          |
| SCADE state machines                                                | enum + `when` mode logic; sugar deferred                    | ≈ §12.3                                                      |
| GRust `when`/`emit` event equations                                 | `when`/`emit` (§5.6)                                        | ✓ adopted                                                    |
| GRust flow operators (sample/scan/on_change/persist/throttle/merge) | stdlib adapter models                                       | ≈ §12.6 candidate                                            |
| timeout / reaction to absence (GRust `timeout`)                     | timer-triggered steps                                       | ✗ open §12.9                                                 |
| periodic execution                                                  | degenerate case of scheduled steps (periodic output demand) | ✓                                                            |
| event-driven execution (LF reactions)                               | input-triggered steps                                       | ✓ native                                                     |
| LF deadlines                                                        | contract `@[..max]` → scheduler constraint                  | ✓ from the contract                                          |
| elapsed time / timers                                               | `now`, `dt`                                                 | ✓                                                            |
| Simulink continuous blocks (ODE solvers)                            | —                                                           | ✗ out of scope (discrete only)                               |
| Simulink saturation blocks                                          | `clamp` + §12.5 boundary policy                             | ≈ open                                                       |
| lookup tables / interpolation                                       | bounded arrays + functions                                  | ✓                                                            |
| iterators over buses/vectors                                        | bounded combinators                                         | ✓ bounded by construction                                    |
| recursion                                                           | —                                                           | ✗ deliberate (totality)                                      |
| exceptions                                                          | —                                                           | ✗ deliberate: step faults (§8) + errors-as-data at contracts |
| side effects / imperative state                                     | —                                                           | ✗ deliberate: `last` is the only memory                      |
| assertions/observers                                                | inherited contract observers; local invariants pending expr | ≈ §9                                                         |
| formal verification hooks                                           | deterministic step semantics + observers → checker-ready    | ≈ designed-for, not shipped                                  |

**Verdict.** rmdl covers the discrete synchronous working set (Lustre/SCADE) and
the event-driven working set (Lingua Franca) with one construct and two
operators, because the scheduled-step semantics subsumes both periodic and
reactive execution. The deliberate refusals — recursion, exceptions, side
effects, continuous time — are each load-bearing for a guarantee (WCET,
no-silent-fault, determinism, discreteness). The honest gaps are
multi-activation and state-machine sugar, both reserved rather than improvised.

---

## Appendix F — Glossary

Family terms are defined in the typl/ridl/uxdl glossaries and mean the same
here. rmdl-specific:

| Term                       | Definition                                                                                                                                                                                                                                                            |
| -------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **function**               | a pure, total, timeless computation — no state, no recursion, no unbounded iteration; the expr core's function layer                                                                                                                                                  |
| **totality**               | the function-layer contract: always terminates, WCET decidable — bought by banning recursion and unbounded loops                                                                                                                                                      |
| **model**                  | the unified reactive construct: a pure reaction `(O,S)=M(I,S)` — typed kinded flows, equations, memory, per-step semantics; contract-blind (no `realizes`); there is no `node`                                                                                        |
| **equation**               | the single defining expression of a flow; unordered in source, scheduled by data dependency                                                                                                                                                                           |
| **flow**                   | a typed value evolving over steps — an input, output, or local of a model; every flow has a kind: `signal` or `event`                                                                                                                                                 |
| **signal flow**            | a state-kind flow: reads as `T`, sampled at step start, holds between assignments                                                                                                                                                                                     |
| **event flow**             | an occurrence-kind flow: reads as `T?` — present with payload when the occurrence arrived this step, absent otherwise; raised by `emit`, never held                                                                                                                   |
| **`when` equation**        | the event-triggered equation block (GRust heritage): ordered branches on `init` / `e?` / rising edges; first match runs; signals it defines hold, events it emits don't                                                                                               |
| **`case` equation**        | the mode-dispatch equation block (GRust's match equation): selects an equation set by a per-step value; exactly one branch every step, total definition, no hold — `when`'s opposite discipline                                                                       |
| **`emit`**                 | raises an event flow with a payload inside a `when` branch — the only way behaviour produces occurrences; a side effect only once rsdl binds the event to a command (§5.7)                                                                                            |
| **`last`**                 | the value of a flow at the previous step — the only memory in the language; total, thanks to seeds                                                                                                                                                                    |
| **`init` (equation)**      | the seed: what `last x` yields at the first step — explicit (`init x = e`) or implicit from the channel init value (§5.3)                                                                                                                                             |
| **step**                   | one atomic synchronous reaction — inputs snapshotted, equations evaluated, outputs published; **scheduled by the runtime on inputs and constraints, never by polling**                                                                                                |
| **activation**             | the runtime's decision to run a step — input-triggered or deadline-triggered, coalesced per contract `min` bounds                                                                                                                                                     |
| **ambient time values**    | `now` and `dt` — contextual identifiers (not keywords) supplying logical time inside models and contracts; never read from a wall clock, never available in functions. The step itself is semantics vocabulary, not surface syntax                                    |
| **logical time**           | the time of a step's _cause_ (triggering occurrence's sender timestamp, or the deadline instant) — what models compute with; identical in live execution, backlog, and replay. `physical − logical` is the runtime's lag metric, a health signal, never a model input |
| **timeline**               | the scheduler's ordered set of due instants, projected from inputs (envelope times) and the contracts (deadlines, floors, timers); real time only _gates_ it in live mode — in replay the gate is open (§6.7)                                                         |
| **step trace**             | the sequence of (input snapshot, `now`) pairs — the total determinant of a model's behaviour; the replay artifact                                                                                                                                                     |
| **`time(f)`**              | language-level accessor for a flow's envelope timestamp — the basis of latency/deadline contract terms (§9.2)                                                                                                                                                         |
| **island**                 | a causally independent partition of a model's equation graph — compiled as its own step unit, run concurrently by the runtime with observational equivalence guaranteed (§6.6)                                                                                        |
| **step fault**             | a runtime fault (division by zero, boundary range violation) aborting the step atomically — state preserved, output flows go invalid, everything recorded                                                                                                             |
| **observer**               | a boolean flow evaluated each step with no effect on outputs — the four-way assertion machinery anchored in the model                                                                                                                                                 |
| **causality**              | the acyclicity requirement on same-step dependencies; `last` is the only legal self-reference                                                                                                                                                                         |
| **synchronous hypothesis** | the reaction is logically instantaneous relative to its environment — inherited from the Lustre tradition, honoured per step                                                                                                                                          |

---

## Appendix G — ridl.std.flow: The Retained Operator Set

**Normative sketch.** Curation criteria: an adapter is retained only if it is
**(a)** not redundant with contract timing bounds (`min`/`max` already provide
provider-side debounce, throttle, refresh, TTL), **(b)** not a trivial inline
expression, and **(c)** precisely definable under the signal/event/timeline
semantics — preferably _in rmdl itself_, making the language its own stdlib
spec.

Two dispensations the stdlib enjoys and user code does not: adapters are
**intrinsics** — polymorphic in one payload type (the family rejects user-facing
generics; intrinsics get the same special status as typl's built-in collection
constructors), and they may take **static function parameters**
(compile-time-bound function references, no closures — the GRust
`merge_int(order: …)` precedent).

### G.1 Tier 1 — Instant-preserving (v0.1; definitions are rmdl)

**`Hold(event e: T, init: T) : signal T`** — zero-order hold, the event→signal
bridge.

```ridl
model Hold(event e: T, init0: T) : (signal out: T) {
  when {
    init -> { out = init0 }
    e?   -> { out = e }
  }        // hold semantics of `when` do the rest
}
```

**`Changes(signal s: T) : event T`** — the signal→event bridge. Emits the new
value when `s != last s`; the comparison seed is `s`'s channel init, so a first
live value equal to the init emits nothing — no start-of-world occurrence.

```ridl
model Changes(signal s: T) : (event out: T) {
  when { s != last s -> { emit out = s } }
}
```

**`RisingEdge(signal s: boolean) : event ()`** / **`FallingEdge(s)`** — edge
extraction; `Changes` specialised to the two boolean transitions. Named in full
so each is self-glossing out of context (`Rising` alone reads as "increasing" on
a numeric). Retained because button/flag edges are ubiquitous.

**`Filter(event e: T, p: function(T): boolean) : event T`** — keeps occurrences
satisfying `p`. The Rx essential GReact omitted. (Disambiguation note for
control readers: this is occurrence filtering, not frequency filtering — the
signature (event in, predicate arg) is the tell; frequency work lives in
`std.control` as `LowPass`.)

```ridl
model Filter(event e: T, p: function(T): boolean) : (event out: T) {
  when { e? if p(e) -> { emit out = e } }
}
```

**`Accumulate(event e: T, init: U, f: function(U, T): U) : signal U`** — running
fold over occurrences (`scan`'s true Rx meaning, honest name). Subsumes
counters, totals, running min/max.

```ridl
model Accumulate(event e: T, init0: U, f: function(U, T): U) : (signal acc: U) {
  when {
    init -> { acc = init0 }
    e?   -> { acc = f(last acc, e) }
  }
}
```

**`Deadband(signal s: T, delta: T) : signal T`** — forwards only when the value
moved more than `delta` from the last _forwarded_ value (not the last input).
`T` numeric/unit-typed; seeded by `s`'s channel init.

```ridl
model Deadband(signal s: T, delta: T) : (signal out: T) {
  when {
    init                        -> { out = s }
    abs(s - last out) > delta   -> { out = s }
  }
}
```

**`Prefer(event e1: T, event e2: T) : event T`** — forwards whichever occurrence
is present; on same-step simultaneity `e1` wins and `e2`'s occurrence is
**dropped and counted** (observability). _Renamed from `Merge` by the naming
review_: Rx `merge` is famously lossless, and our simultaneity semantics make
loss real — `Prefer` puts the priority, and therefore the loss, in the name.
When every occurrence matters, bind both events into one model's `when` instead.

**`Latch(signal set: boolean, signal reset: boolean) : signal boolean`** — SR
latch, **reset priority** (the safety convention). Four lines everyone otherwise
writes differently; standardising the reset priority is the point.

### G.2 Tier 2 — Instant-creating (gated on §12.9 timer steps)

These register instants on the timeline (§6.7); their definitions are scheduling
semantics, not equations:

**`Clock(d: Duration) : event ()`** — the primitive instant source: an
occurrence at every multiple of `d` from instantiation. Everything periodic
derives from it (GReact's `period`; `Ticker` was the draft name — rejected:
Go-only lineage plus the stock-ticker false friend, where `Clock` reads
instantly for EE, control, and synchronous-tradition audiences alike). **The
doctrine, precisely layered**: there is no clock in the _runtime_ or the _causal
analysis_ — the scheduler runs a timeline (§6.7) and causality never assumes
ticks; a clock in the _functional description_ is legitimate and explicit — a
declared, local, owned source of instants, projected onto the timeline like any
other cause. The lineage is exact twice over: a hardware clock line _is_ a
periodic pulse train, and a Lustre clock _is_ the stream of instants a
computation activates on — making `Clock` the embryo of multi-activation
(§12.1): a sub-model activated "on" a `Clock` is Lustre's `when`, rediscovered.

**`Sample(signal s: T, d: Duration) : event T`** — `s`'s current value at each
`Clock(d)` instant. _(≡ `when tick? -> emit out = s` over a Clock —
definitional.)_ Output kind is **event**, deliberately: a periodic reading is a
sequence of occurrences; `Hold` it if state is wanted.

**`Coalesce(event e: T, d: Duration) : event T`** — burst conflation,
**trailing-window**: the first occurrence opens a window and schedules an
instant at `time(e) + d`; at that instant the _latest_ occurrence received in
the window is emitted and the window closes. Bounded latency `d`, at most one
output per window, no occurrence-content lost except superseded values.

**`Watchdog(event e: T, d: Duration) : event ()`** — absence detection: each
occurrence (re)arms an instant at `time(e) + d`; if reached without re-arm,
fires once and disarms until the next occurrence. Logical-time based (§6.3), so
replay reproduces watchdog fires exactly.

### G.3 Control tier (`ridl.std.control`, instant-preserving, `dt`-correct)

**`Integrator(signal x: T, init: T, lo: T, hi: T) : signal T`** — saturating
rectangular integration: `out = clamp(last out + x * dt, lo, hi)`.
**`RateLimiter(signal x: T, up: T, down: T) : signal T`** — slew limiting:
output moves toward `x` by at most `up * dt` / `down * dt` per step.
**`LowPass(signal x: T, tau: Duration) : signal T`** — first-order filter,
aperiodic-correct: `alpha = dt / (tau + dt)`. (Doc alias: _PT1_ — the
German-automotive term for the same block.)

### G.4 Cut, with reasons

| Not retained                              | Why                                                                                                               |
| ----------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| time-based debounce/throttle of a channel | the contract's `min` bound — annotation, not operator                                                             |
| TTL / staleness                           | the contract's `max` bound                                                                                        |
| `time()`                                  | language: `now`, `time(f)` (§6.3)                                                                                 |
| `Map` over events                         | inline expression: `emit out = f(e)`                                                                              |
| `Delay(e, d)`                             | needs a bounded pending-occurrence queue (multiple in flight) — deferred until the bound story is designed        |
| `Zip` / windows / buffers                 | unbounded or alignment-fragile; no demonstrated automotive need                                                   |
| `Derivative`                              | a noise amplifier shipped as a one-liner invites misuse; provide filtered variants in `std.control` when demanded |

---

_End of rmdl Language Reference v0.1.0 — Draft._
