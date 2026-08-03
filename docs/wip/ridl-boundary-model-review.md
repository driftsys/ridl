# Boundary Model Review — uxdl Scope, and What Replaces It

**Spike record — design session of 2026-08-03.** Not a decision, not a
specification. This document captures a review of the uxdl language reference
that ended by rejecting uxdl's shape and proposing a boundary model in ridl
instead. It records the findings, the arguments that produced them, the
positions that were reversed mid-session, and the tests that decide what remains
open.

Status: **superseded by ADR-0012** (the interaction boundary model), which was
written from this review and is authoritative wherever the two disagree. This
record is kept for the reasoning trail — the arguments, the retractions, and the
falsification tests — not for its conclusions.

**Three things below are known to be wrong**, and are left in place rather than
edited because the reasoning that corrected them is the useful part:

- **§5's "Truth versus Representation" table** uses "truth" in two incompatible
  senses across its rows (physical fact for the sensor row, reference value for
  the actuator row) and is missing the person-inbound case. The corrected form
  is four rows on a reference/realisation axis, with one rule: _the causally
  upstream side is the reference, the downstream side is the realisation._ See
  ADR-0012 Context.
- **§5.2** says the obligations sit on pairs. They also **compose along a path**
  — a four-hop chain from wheel to driver's belief — and an end-to-end budget is
  computable only if every hop declares its own. See ADR-0012 decision 3.
- **§8's proposed shape** (fold uxdl into ridl, with the operation shapes
  demoted to a `gesture` attribute) is superseded by ADR-0012 decisions 4 and 6.
  The demotion was wrong: it moves the discriminating classifier out of R1
  position. §4.5's claim that the operation shapes are boundary-agnostic is also
  wrong in the sense that matters — their semantics generalise, but their
  consumers do not, so they belong to the intent family.

---

## 1. Scope of the session

The starting request was a step-by-step review of
`docs/specification/uxdl-language-reference.md` for design validation. The
review did not finish the document. It stopped at section 7 (`fetch`), because
the questions raised there turned out to be about uxdl's existence rather than
its contents.

The session produced a proposed replacement shape (§8) and a list of open
questions with the evidence that would settle each (§9).

## 2. The goal, restated

The goal uxdl exists to serve, as stated in session:

> Define interactions so that UI element identifiers can be defined and bound,
> so that end-to-end validation tests remain valid when the user interface
> changes but the user experience does not.

Landscape and portrait, responsive and adaptive layout, and theming are
**variants over one interaction**. Touch, mouse, command line, voice, and a
physical button are **bindings of one interaction**. What is declared is what
the user can experience and do, not how it is presented or actuated.

This produced the criterion used throughout the session:

> **Modality independence.** If two user-interface variants afford the same user
> capability, they must produce the same declaration. A verb that fails this
> criterion is naming a modality, not an interaction.

## 3. Method — the falsification test

Every proposed language, profile, and keyword was put to one test:

> **What can it express that the existing languages cannot express without
> distortion?**

The test was applied three times and reversed two of the session's own
conclusions (§7). It is recorded here because the answer to "should this be a
language" was wrong twice when argued any other way.

An important refinement emerged: the first form of the test — _can ridl express
it?_ — is too weak. ridl can express almost anything structurally. The correct
form is:

> **Is there an obligation the contract must carry for which the existing
> languages have no vocabulary?**

That distinction is what separates a rename from a semantic (§5).

## 4. Findings — settled

### 4.1 `fetch` does not belong in the user-interaction layer

Three independent reasons:

1. It is not among uxil's eleven interaction verbs. It was imported from ridl's
   `query`, without a user-interaction design pass.
2. Every scenario tested decomposes into _interaction → display update_:
   autocomplete is `input` plus a display; a detail pane is `select` plus a
   display; pagination is `select` plus a display; pull-to-refresh is `activate`
   plus a display.
3. It carries no perception obligation (§5). A person never perceives a request;
   they perceive its resulting display.

`fetch` describes how the provider obtains data, which is the inward face of the
contract — rmdl and rsdl territory, not the user boundary.

### 4.2 `scroll` and `drag` fail the modality-independence criterion

Both name gestures rather than intents. "Scrolling" is one of several ways to
express "show me more of this sequence" (touch swipe, keyboard paging,
pagination controls, a voice command). "Dragging" is one of several ways to
express "put this item in a different position".

Candidate replacements, not settled: `advance` for sequence traversal, `reorder`
for positional change.

The same test rejects `scan` (§4.7) — it names a sensing mechanism. A lidar
scans; a staring infrared array does not; both measure the same quantity.

### 4.3 uxil and uxdl are different kinds of artifact

|                  | uxil (markspec, ADR-034)                | uxdl (as specified)                    |
| ---------------- | --------------------------------------- | -------------------------------------- |
| Written          | after the interface exists              | before it exists                       |
| Relation to code | annotates                               | generates                              |
| Identifiers      | declared beside a UI; can drift from it | derived from the contract              |
| Payload typing   | parked (S11, zero code)                 | typl types — the reason it exists      |
| Consumers        | LSP, validator                          | codegen, wiring, test plane, diff gate |

uxil's founding problem, from ADR-034's context statement, is that the deployed
`screenId/elementId[:itemKey]` convention declares nothing — so specifications,
tests, journeys, and telemetry cannot be validated against the surface.

A generative layer dissolves that problem structurally: **the identifier is the
declaration**, so identifiers cannot drift from affordances. The citation
grammar becomes a projection of the contract rather than a second thing to
author and keep synchronised.

### 4.4 uxil's parked payload bridge is unparked by this work

markspec issue #729 (S11) was parked with zero code written because no
downstream consumer needed a typed payload. ADR-034 lists the revisit trigger:
_"a concrete downstream surface needs a published-typl payload beyond its verb's
canonical shape."_ A formal interface layer is that consumer.

Direction matters: **validate uxil citations against ridl declarations; do not
generate contracts from prose.** A one-time scaffolding tool (uxil surface →
ridl skeleton) is reasonable for bootstrapping; after that the contract is the
source of truth for structure.

### 4.5 Operation shapes are boundary-agnostic

`activate`, `toggle`, `select`, `adjust`, and `dismiss` describe **operation
shapes**, not gestures:

- `toggle` — flip a binary state
- `select` — choose one from a set
- `adjust` — set a continuous value within a range
- `activate` — invoke with no parameters
- `dismiss` — cancel or close a transient

Each is meaningful between software peers. A diagnostic orchestrator can toggle
a feature flag, select a calibration set, adjust a threshold. They correspond to
gestures only because gestures are shaped by what the operation does — which is
why they satisfy the modality-independence criterion.

They are therefore **an operation taxonomy ridl never had**, not user-interface
vocabulary. The user-interface case is where the gap became visible.

Consequence for their surface form: they must be **keywords, not an attribute**.
Demoting them to `[ gesture = activate ]` moves the informative classifier out
of keyword position and violates invariant R1 of `family-general-form.md` —
after which `grep '^command'` returns everything and distinguishes nothing.

### 4.6 The environment boundary needs no language of its own

Proposed during the session and then retracted (§7.1). Every candidate concern
decomposes:

| Candidate                                | Where it already lives                                        |
| ---------------------------------------- | ------------------------------------------------------------- |
| no acknowledgment of effect              | ridl `command` — no return, outcome observed via signals      |
| stuck, drift, out of range, disconnected | ridl §4.5 invalid-state provenance (designed for SNA)         |
| plausibility bounds                      | typl ranges plus `require`/`ensure`                           |
| tolerance, noise, resolution             | typl — a measurement type property                            |
| redundancy, voting                       | rsdl — several sources feeding one value is wiring            |
| calibration parameters                   | `fixed` — provisioned, immutable per instance, FOTA-updatable |
| transfer function (raw to physical)      | rmdl — it is a computation                                    |
| sampling, aliasing                       | ridl §9 timing                                                |

What survives is not a language. It is the same obligation set the user boundary
needs (§5), which is the reason the retraction was later found to be half wrong
(§7.2).

### 4.7 `sidl` rejected; rsdl rejected as a home

A new "system interface description language" was considered and rejected for
three independent reasons:

1. `ridl` already means _Reactive Interface Description Language_ — the name is
   boundary-neutral as it stands, and the concept note already treats the
   "Interface" gloss as belonging to the member.
2. SIDL collides with the Scientific Interface Definition Language (Babel,
   LLNL), a live IDL in high-performance computing. The naming ledger already
   records an ITU-SDL echo as a cost on `rsdl`; this would double it.
3. Renaming unwinds `ridlc`, `ridl.toml`, `ridl.lock`, `~/.ridl/cache`, the
   crates.io reservations, and the platform name — all of which concept note §4
   deliberately preserved.

rsdl was rejected as a home for interface declarations: it is the apex of the
lattice and composes by import. An interface must be declarable independently of
any deployment, or it cannot be reused across topologies.

## 5. The central finding — datum and referent

This is the finding the rest of the session reorganised around, and it is in no
existing specification.

The example that produced it: `signal rawSpeed` and `display clusterSpeed` are
**not the same value**. Indicated speed is legally constrained never to
under-read, is quantised with hysteresis, is damped on purpose, and is
unit-switched per market. Declaring both as `signal` loses the distinction that
matters, and eventually the wrong one is bound to the cluster.

Generalised:

> **Between two software peers, the datum is the truth** — both sides agree on a
> number and there is nothing behind it. **At every boundary with the
> non-software world, the datum and its referent come apart.**

| Boundary | Truth                | Representation  | Gap governed by                       |
| -------- | -------------------- | --------------- | ------------------------------------- |
| human    | actual vehicle speed | indicated speed | law, legibility, perception           |
| sensor   | actual wheel speed   | measured value  | transfer function, noise, calibration |
| actuator | commanded angle      | achieved angle  | slew rate, authority, saturation      |

### 5.1 The four correspondence obligations

Wherever datum and referent come apart, the contract must carry four things.
They are the same four at every boundary; only their instantiation differs.

| Obligation                | Human                               | Sensor                                         | Actuator                    |
| ------------------------- | ----------------------------------- | ---------------------------------------------- | --------------------------- |
| relationship              | never under-read; quantised; damped | transfer function; calibration                 | authority limits; slew rate |
| uncertainty               | display resolution                  | tolerance on each reading                      | positioning tolerance       |
| latency of correspondence | perception delay                    | measurement instant is not publication instant | actuation lag               |
| failure to correspond     | shown but not perceivable           | plausible-looking but false                    | commanded but not achieved  |

None of the four exists in ridl today. Two are load-bearing:

- **Latency of correspondence.** ridl's envelope is sender-stamped at
  publication. For a transducer with a response time, that timestamp records
  when the value was produced, not when the world was in that state. Any model
  computing "with the time of the cause" (family doctrine 9) is off by the
  transducer's lag, silently.
- **Failure to correspond.** A sensor can report a well-formed, in-range,
  perfectly fresh value that is false. ridl's invalid-state provenance detects
  malformed, not implausible.

### 5.2 The obligation lives on the pair, not on either declaration

Every case examined is a **pair**, and the obligation is a relation between the
two members:

| Pair                                          | Relationship the contract should carry   |
| --------------------------------------------- | ---------------------------------------- |
| `signal rawSpeed` → `display clusterSpeed`    | never under-reads; quantised; damped     |
| `toggle muteButton` → `display muted`         | flips within the perceptual budget       |
| `adjust volume` → `display volumeShown`       | tracks the commanded value               |
| `actuate targetAngle` → `measure actualAngle` | converges within slew rate and tolerance |

This is a real failure mode with no current expression: a person toggles a
control, the indicator does not update, the person toggles again — and both
declarations were individually satisfied.

uxdl noticed half of this and parked it as open question §16.2 ("two-way binding
sugar"). Under this framing it is not sugar. It is where the correspondence
obligation lives, and the actuator boundary has exactly the same gap, which
means **one mechanism serves both**.

### 5.3 Perception is the human boundary's specific obligation

A `signal` states that a value is available on a boundary. A `display` states
that **a person perceives this value**. The second is a stronger claim, and it
generates rules the first does not:

| Rule       | Why it differs under a perception obligation                                                     |
| ---------- | ------------------------------------------------------------------------------------------------ |
| staleness  | a stale display misleads a person making a decision                                              |
| init       | the pre-live state is what the person sees at power-on; for a telltale this is legally specified |
| invalid    | must render as a defined indication, never as silent absence                                     |
| timing     | the bound is a perceptual budget — flicker floor, readability ceiling                            |
| derivation | the displayed value may lawfully differ from the true one                                        |

The same obligation runs inbound: an affordance must be **perceivably
unavailable before the attempt**. A software peer needs none of this; it can
call and take the rejection.

### 5.4 Direction is real; bidirectional devices are a binding concern

Interactions are directional even where devices are not. A servo takes a command
and reports achieved position — two declarations, one device. A toggle switch
shows state and accepts a gesture — two declarations, one widget. In both cases
the device is an rsdl binding concern.

**Information flows one way. Declarations are directional. Bidirectionality is a
property of the representation site, not of the flow.**

### 5.5 The counterparty determines the boundary, not the transducer

A steering-wheel switch is physically a contact closure — a sensor. But its
referent is the user's intent, so the counterparty is the human and the
transducer is a binding detail.

> The boundary kind is determined by who is on the other side, not by how the
> signal is transduced.

The session's original modality-independence criterion (§2) therefore **falls
out of the model** rather than being asserted as a style rule.

### 5.6 A gap this exposes — outbound occurrence to a person

The human boundary's directions are not symmetric in kind count. Inbound has
state-carrying gestures (`input`) and operations (`activate`, `toggle`, …).
Outbound has only `display`, a continuous state value.

There is no outbound **occurrence**. A warning chime, a haptic pulse, a "trip
complete" notification are momentary, non-persistent, and have no init value.
Modelling a chime as a display is wrong — it has no initial value to render.
ridl's `event` is the software analogue; the human boundary has none.

## 6. Availability has five sources, and uxdl covers one

| Source       | Example                                   | Where it should land                                  |
| ------------ | ----------------------------------------- | ----------------------------------------------------- |
| mode         | banner exists only in `ERROR`             | `during` — view state (covered)                       |
| data         | submit disabled until the form validates  | predicate over declared displays                      |
| in progress  | purchase pending, cannot re-invoke        | predicate over a display carrying pending state       |
| policy       | not permitted while driving; not licensed | display of the governing state, or a label            |
| provisioning | this trim does not have the control       | `fixed` — and the control is **absent**, not disabled |

uxdl covers the first row only, and [uxdl §14] forbids the obvious workaround:
_"State sets small and coarse; fine-grained modes are viewmodel behaviour, not
contract."_ So "disabled while buffering" cannot become a view state, and
nothing else covers it.

uxil could express this and uxdl lost it: uxil declared availability **per
element** (`` `/play : activate @enabled, disabled` ``). Flattening to
view-level `during` was a loss of expressiveness, not a simplification.

### 6.1 The consumer-evaluability rule

The perception obligation forces a rule that is checkable and is in no
specification:

> At a perception-obligated boundary, the availability condition must be
> **evaluable by the consumer** — because the person must perceive
> unavailability before attempting, not discover it by rejection.
>
> Therefore an availability predicate may reference **only declared displays**.

A `require` over provider-internal state is legal at the software boundary
(call, get rejected) and illegal at the human boundary (the renderer cannot
disable a control whose condition it cannot evaluate).

ridl's own example already satisfies this by accident —
`require position !=
GearPosition.PARK || currentSpeed == 0.0` references a
signal in the same interface, so a subscriber can evaluate it. The rule is
working and nothing enforces it.

This is the **first rule found that is specific to the perception obligation**
rather than shared with the physical boundary.

### 6.2 Absent is not disabled

Provisioning produces a different outcome from the other four: the control is
not present rather than shown-and-disabled. Those lead a person to different
conclusions — wait and retry, versus this does not exist here — and they are
different end-to-end assertions. Neither uxil nor uxdl distinguishes them.

Argued in session as contract rather than presentation, because it concerns what
the person can conclude, which is perception. Not settled.

### 6.3 Availability is general; pre-visibility is not

An actuator has all five sources of unavailability. What it does not have is the
requirement that unavailability be perceivable in advance — a software caller
can simply be rejected.

> Availability is general to every boundary. The requirement that it be
> perceivable in advance is specific to the human one.

This is a good sign for the design: one mechanism, with one boundary-conditional
rule on top, rather than a parallel human-only concept.

## 7. Positions reversed during the session

Recorded because the reversals are part of the evidence.

### 7.1 "The environment boundary deserves a profile" — retracted

Proposed on the strength of an apparent third counterparty kind, then retracted
under the falsification test (§4.6). Everything decomposed.

### 7.2 "uxdl is four renames plus metadata plus sugar" — reversed

The merge recommendation rested on this premise. `signal rawSpeed` versus
`display clusterSpeed` falsified it: `display` is not `signal` with a visibility
label, it is a different quantity carrying a different obligation (§5).

This reversal also **partially un-retracts §7.1**, because it exposed that the
first retraction used the weak form of the test (_can ridl express it?_) rather
than the strong form (_is there an obligation ridl has no vocabulary for?_). The
environment boundary does not need a language, but it does need the same
obligation set as the human one.

### 7.3 "Refinements should become a `gesture` attribute" — reversed

Demoting them buries the discriminating classifier in an attribute and violates
R1 (§4.5).

## 8. Proposed shape

**uxdl's content is sound; uxdl's shape is not.** Packaging it as a parallel
language that renames ridl's kinds is what produced the synonym problem, the
document that is largely pointers to ridl, and the recurring language-or-profile
question.

The proposed replacement:

> **ridl gains a boundary model in core. Human and sensor/actuator vocabulary
> become extensions over it. uxdl as a family member goes away.**

### 8.1 Two evolution axes, not one

|                 | Feature sets (v1, v2, v3)           | Extensions / plugins              |
| --------------- | ----------------------------------- | --------------------------------- |
| Who receives it | everyone, gated by language version | only programs that opt in         |
| Owns            | core semantics                      | domain vocabulary                 |
| Example         | observability conventions           | domain-specific interaction kinds |
| Evolution       | language versioning                 | independent release cadence       |

The trap to avoid: the boundary kinds are **not** domain extensions. The
obligations behind them (§5.1) are general to any system touching the
non-software world. If a human-interface extension and a sensor extension each
invent their own uncertainty model, the result is two incompatible ones that
rmdl and rsdl cannot reason across.

### 8.2 The split that makes "no IR change" tractable

> **Mechanism in core. Vocabulary in extensions.**

| Core owns (IR schema)                                                         | An extension ships                         |
| ----------------------------------------------------------------------------- | ------------------------------------------ |
| interaction structure: name, payload, parameters, timing, ordinal, attributes | which kinds exist and what they are called |
| the four correspondence obligations                                           | kind-specific payload rules                |
| availability and its five sources                                             | the generation layer                       |
| the consumer-evaluability rule                                                | diagnostics                                |
| interaction identity and citation paths                                       | diff categories for its kinds              |

Under this split the IR is open at **exactly one place** — the kind tag — and
closed everywhere semantically rich. Adding a kind changes the IR's _content_,
not its _schema_.

The "no IR change" constraint is therefore tractable **conditionally**: it holds
under this split and fails under the other one. If extensions carry semantics,
the IR must be open over semantics, which is very difficult to keep stable.

### 8.3 Precedent and timing

The family already has extensions on the **output** side: concept note §8.2 —
_"Every backend and every ecosystem tool is an IR consumer behind one plugin
protocol."_ The proposal extends this to the **input** side, which is the new
part. The IR specification's declared scope already lists "plugin protocol"
alongside serialization and diff categories.

**The IR specification is not started.** Designing openness over interaction
kinds in from the beginning is cheap; retrofitting it after the IR is frozen is
not. Similarly, ADR-0003 — which would freeze the five-language decision — is
not started, and uxdl is unimplemented. Everything proposed here is
documentation work, not unbuilding.

Closest external precedent: protobuf's descriptor set with custom options. The
descriptor schema is fixed, extensions add typed options, plugins consume them,
and breaking-change detection works structurally without understanding any
particular extension's options.

### 8.4 The part that can quietly break — diff

`ridl-diff` is a normative gate with defined exit codes. It can detect
_structural_ change to an extension's kind (added, removed, reordered, retyped)
without understanding it. It cannot know that tightening a measurement's
uncertainty bound is breaking, or that removing a lifecycle attribute is
behavioural.

An extension must therefore ship **diff categories for its kinds**, and the gate
must fail closed when they are missing. Otherwise an extension silently weakens
the compatibility guarantee the platform rests on. This is the piece to design
first.

Secondary risk: fragmentation — several organisations defining incompatible
kinds under the same name. Mitigated by core owning the obligations plus a kind
registry, the same discipline as the family-wide keyword registry.

## 9. Open questions, with the evidence that settles each

1. **Kinds per boundary, or one kind set with a boundary axis?** Option A:
   `display`, `measure`, `actuate`, `input`, and a notify-shaped kind —
   keyword-first, satisfies R1, reads well, costs keywords and grows per
   boundary. Option B: five kinds unchanged plus an orthogonal boundary clause
   (`signal clusterSpeed: IndicatedSpeed to human`) — fully compositional, costs
   R1 readability for the boundary. **Settles by:** does each boundary need
   kind-specific _rules_, or only kind-specific _obligations_? The obligations
   proved uniform (§5.1), which leans toward B. Test by writing one worked pair
   per boundary — cluster telltale, wheel-speed sensor, steering actuator — both
   ways.

2. **Does the correspondence obligation hold uniformly?** The load-bearing claim
   of §5. **Settles by:** instantiating all four obligations against the same
   three worked cases. If they instantiate cleanly in all three, the design is
   settled in shape.

3. **Do user-facing contracts need different evolution rules than bus
   contracts?** Asked twice in session, unanswered. **Settles by:** a real case
   — a shipped human-machine interface that could not take a change the bus
   could, or the reverse. If neither boundary answers yes, this is one language
   with kinds, and that is the ADR.

4. **`input` and direction.** A consumer-to-provider occurrence with TTL and
   debounce rather than command acknowledgment. A ridl gap either way; a service
   pushing an occurrence upstream has the same shape.

5. **Element-centric or interaction-centric?** uxil: one element identifier
   affords several verbs. uxdl: one declaration, one kind, and two declarations
   cannot share a name (UXDL-102). Test selectors want stable element identity
   with the verb as the action. Unresolved, and it shapes the citation grammar.

6. **Verb set refinement.** `advance` and `reorder` as replacements for `scroll`
   and `drag`; `observe` (telemetry and accessibility both consume it); `ask`,
   which uxdl dropped entirely without even reserving it.

7. **Absent versus disabled** (§6.2) — contract or presentation.

8. **Journeys and critical user journeys.** A journey is a sequence of
   interactions across surfaces; neither ridl nor rsdl expresses a graph over
   interfaces. Probably a requirements-and-traceability artifact rather than a
   language, but this needs checking, because the described workflow puts
   critical user journeys at the top of the pipeline.

9. **Naming.** Deferred until scope settles. On record: "interaction" is the
   more truthful noun than "description"; `typl` sets the precedent for a family
   member breaking the `-DL` suffix when semantics warrant it; and markspec's
   `typl` kept its name across supersession into this family, which makes
   renaming `uxil` to `uxdl` the inconsistent move.

## 10. Documents this would affect

None have been changed. Listed so the blast radius is visible before anything is
ratified.

- `docs/specification/uxdl-language-reference.md` — content migrates into ridl;
  the document is superseded rather than edited.
- `docs/specification/ridl-language-reference.md` — gains the boundary model,
  correspondence obligations, availability sources, operation-shape kinds.
- `docs/wip/ridl-family-concept.md` — §2 (five languages), §3 (`interact` core
  as two profiles), §10 naming ledger row for uxdl.
- `docs/specification/ridl-family-overview.md` — §1 map, §2 inventory, §4
  reading paths, §5 decision ledger entry 14.
- `docs/ROADMAP.md` — whichever epic holds uxdl.
- ADR-0003 (not started) — would record the four-language decision instead of
  five.
- The IR specification (not started) — openness over interaction kinds, and diff
  categories for extension kinds.

---

_End of spike record. Nothing here is ratified. The next step is question 3 of
§9 — the evolution-rules evidence — which decides whether the boundary model is
kinds in ridl or a profile above it._
