# ridl Language Reference

**Reactive Interface Description Language** — the system-interaction layer of
the RIDL family: transport-neutral reactive contracts (`signal` · `event` ·
`command` · `query` · `fixed`) over the typl vocabulary.

Version: 0.2.0 — Draft

> **Provenance and supersession.** This document supersedes the _interaction
> half_ of the RIDL Language Reference v0.1 (§12 Streams, §13 Interfaces, and
> the interaction rows of §16–§17). The _vocabulary half_ (§1–§11) is now the
> **typl Language Reference**, which this document builds on and never restates.
> v0.1 errata applied here: wildcard imports are an error (ADR-0002), no
> semicolons, float width inference is count-based (typl §4.3). New in v0.2,
> from the prior-art review (Appendix F): the error model (§10), the last-value
> subscription guarantee (§4.4), and interaction identity & evolution (§11).
>
> **Reconciled with the shipped toolchain at the epic E2 close-out** (ADR-0008
> decisions 1 and 16 to 21). Four supersessions from general form §6 are now
> absorbed rather than pending: inline `T | E` returns (§7, §10.1, Appendix A,
> Appendix C), generic `min`/`max` timing with the per-kind table as a
> derivation (§4.3, §5.2, §9), ordinal drift reported at the desk (RIDL-407),
> and the Stratum-3 wording "infrastructure failure — detected, undeclared"
> (§10.3). §2.1's duration table, §9.2's `@[X..X]` rule, §16.1's RIDL-108 and
> RIDL-110 rows, and Appendix C's grammar are corrected to the compiler.

---

## Table of Contents

1. [Scope and Position in the Family](#1-scope-and-position-in-the-family)
2. [Lexical Additions](#2-lexical-additions)
3. [The Interaction Model](#3-the-interaction-model) — families (§3.2),
   correspondence obligations (§3.3), availability (§3.4)
4. [Signal](#4-signal)
5. [Event](#5-event)
6. [Command](#6-command)
7. [Query](#7-query)
8. [Fixed](#8-fixed)
9. [Timing Annotations](#9-timing-annotations)
10. [Errors](#10-errors)
11. [Interaction Identity and Evolution](#11-interaction-identity-and-evolution)
12. [Streams](#12-streams)
13. [Contracts — require / ensure](#13-contracts--require--ensure)
14. [Interfaces and Services](#14-interfaces-and-services)
15. [Conventions](#15-conventions)
16. [Diagnostics](#16-diagnostics)
17. [Open Questions](#17-open-questions)

- [Appendix A — Full Example](#appendix-a--full-example)
- [Appendix B — Codegen Targets](#appendix-b--codegen-targets)
- [Appendix C — Formal Grammar (EBNF)](#appendix-c--formal-grammar-ebnf)
- [Appendix D — Standards References](#appendix-d--standards-references)
- [Appendix E — Prior Art Survey](#appendix-e--prior-art-survey)
- [Appendix F — Coverage Analysis: Other Interface Languages](#appendix-f--coverage-analysis-other-interface-languages)
- [Appendix G — Glossary](#appendix-g--glossary)

---

## 1. Scope and Position in the Family

### 1.1 What ridl is

ridl answers one question: _what does a service produce, consume, and
guarantee?_ It is the single source of truth for the contract boundary of a
reactive system, and it is transport-neutral by design — the same `.ridl`
contract binds to SOME/IP, proto/gRPC, DBC, AIDL, MQTT/AsyncAPI, or an
in-process broker without modification.

ridl is a profile of the one family grammar. A `.ridl` file accepts
**interaction declarations plus everything typl accepts** — types and their
interfaces naturally travel together; profile purity per package remains a
`ridl.toml` policy (typl §1.2). What `.ridl` rejects: behaviour declarations
(rmdl), architecture declarations (rsdl).

### 1.2 What ridl adds to typl

Exactly four things, each owned by a family core:

| Addition          | Core           | Surface                                                     |
| ----------------- | -------------- | ----------------------------------------------------------- |
| interaction kinds | `interact`     | `interface`, `signal`, `event`, `command`, `query`, `fixed` |
| timing            | `time`         | `@Xms`, `@[min..max]`, duration literals                    |
| contracts         | `expr`         | `require`, `ensure` attribute clauses                       |
| stream container  | — (ridl-owned) | `<T>` in interaction position                               |

Everything else — types, units, ranges, constants, composites, packages,
visibility, doc comments, diagnostic-code practice — is typl, unchanged.

### 1.3 ridl describes every boundary

The five interaction kinds are named, typed, directed interactions on a contract
boundary — **any** boundary. **ADR-0012** retired uxdl as a separate language
and gave ridl an interaction **family** on every declaration: `dispatch` (system
to system), `presentation` and `intent` (the person boundary), `acquisition` and
`control` (the physical boundary). Only `dispatch` carries no correspondence
obligation, because there the datum and its referent are the same thing.

The kinds below are family-neutral: a `signal` is a continuous state value
whoever reads it. What a non-dispatch family adds is the four correspondence
obligations, not a different kind. Readable per-family spellings — `present`,
`measure`, `actuate` and the rest — are the rxdl reference's, and add no
semantics to what is defined here.

---

## 2. Lexical Additions

ridl inherits the family lexical conventions (typl §2) and activates two token
classes typl rejects:

### 2.1 Duration Literals

A positive integer followed by a time-unit suffix. Used exclusively in timing
annotations and time-typed contract expressions. The five suffixes below are the
complete set:

| Suffix | Unit         | Example | Microseconds  |
| ------ | ------------ | ------- | ------------- |
| `us`   | microseconds | `500us` | 1             |
| `ms`   | milliseconds | `10ms`  | 1 000         |
| `s`    | seconds      | `1s`    | 1 000 000     |
| `min`  | minutes      | `5min`  | 60 000 000    |
| `h`    | hours        | `1h`    | 3 600 000 000 |

The suffixes are UCUM time atoms, but they are a **proper subset** of the UCUM
atom table typl uses for unit types (typl §5.1): `d` is a unit atom there and is
not a duration suffix here, so `@[1min..1d]` is a parse error.

Zero duration is not permitted (RIDL-102). Fractions are not supported — use a
smaller unit (`500us`, not `0.5ms`); a fractional literal is FORM-102.

### 2.2 The `@` Sigil

`@` introduces a timing annotation and is used for nothing else in the family.

### 2.3 Keywords

Keywords **used** by the ridl profile, beyond typl's set:

```
interface  service  signal  event  command  query  fixed
```

`interface` names the abstract contract shape; `service` names a global
published declaration of one or more of them (§14). Both are ridl's — the
contract SSOT — while _providing_ and _requiring_ them is rsdl's.

A signal's init override carries **no keyword**: it is a bare `= value` suffix
(§4.4), the same idiom typl uses for a type or field init (typl §5.8). There is
no `init` keyword in ridl — `init` belongs to rmdl alone, where `init x = value`
seeds a memory recurrence and the keyword is needed to disambiguate it from the
equation.

There is deliberately **no error keyword**: functional errors are typl
vocabulary (`error` types, result unions — typl §10.1–10.2), not interaction
syntax (§10.1).

(`reserved` and the evolution machinery are inherited from typl §7.4.
`require`/`ensure` are expr-core words activated here. All family keywords
remain reserved in every profile — typl §1.4.)

---

## 3. The Interaction Model

Five interaction kinds, each a distinct answer to "who initiates, and what does
delivery mean":

| Keyword   | Pattern     | Semantic                                           | Initiator             |
| --------- | ----------- | -------------------------------------------------- | --------------------- |
| `signal`  | pub/sub     | continuous state value — the latest sample matters | provider publishes    |
| `event`   | pub/sub     | discrete occurrence — every occurrence matters     | provider publishes    |
| `command` | RPC         | fire-and-forget action — no reply channel          | consumer calls        |
| `query`   | RPC         | request/response — reply mandatory                 | consumer calls        |
| `fixed`   | provisioned | immutable for the software-instance lifetime       | neither (provisioned) |

The signal/event split is the load-bearing distinction, borrowed from the
automotive tradition (AUTOSAR `isQueued`): **state** may be sampled, coalesced,
and cached — missing an intermediate value is acceptable; **occurrences** are
queued and each one is meaningful. Everything else in this document (timing
semantics, late-joiner behaviour, error handling) follows from which side of
that line an interaction sits on.

Payloads are always **named typl types** — the interaction layer never defines
shapes, it references vocabulary. This is the family's anti-coupling rule
(concept note §7.1).

### 3.1 The Implicit Envelope

Every interaction instance — a signal publication, an event occurrence, a
command call, a query request and reply — carries an **envelope** supplied by
the runtime and never declared in the contract:

| Envelope field      | Meaning                                                                                                                  |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| **timestamp**       | when the instance was published/raised/called — **stamped by the sender**, at the producing binding, before transmission |
| **sequence number** | per-channel monotonic counter (frame number), assigned by the sender per provider instance                               |

Timestamping is a **sender-side act, always**: no broker, relay, or receiver
ever (re)stamps the envelope. Under the synchronized time base (below) this is
what makes the semantics right — an event's TTL measures age since it was
_raised_, not since it was forwarded; freshness measures the provider's
publication, not the broker's hop; and one-way latency is simply receive-side
clock minus envelope timestamp. Receive time is a local runtime observable,
never envelope data.

The envelope is what the machinery runs on: TTL, debounce, and freshness (§9)
are evaluated on envelope timestamps; command duplicate suppression and retry
(§6.1) on sequence numbers; **sequence gaps make loss detectable** on pub/sub (a
Stratum 3 detection, feeding §10.4 management); AUTOSAR E2E protection consumes
the counter; deterministic replay (concept note §9.3) is ordered by it;
observability spans derive from both. Generated subscriber/caller APIs expose
the envelope alongside the value (value + provenance + envelope); request/reply
correlation is likewise runtime-internal.

**System time.** The platform assumes **one synchronized time base across the
system** — gPTP/PTP (IEEE 802.1AS in vehicle networks) or an equivalent shared
realtime clock. Envelope timestamps are stamped in that domain and are therefore
**comparable system-wide**: cross-node latency, end-to-end freshness, and global
event ordering are meaningful by assumption, not by luck. The concrete mechanism
and sync topology are deployment properties (rsdl declares the time base per
system); _that a synchronized base exists_ is a platform assumption every
contract may rely on. Loss of synchronization is a detectable system failure
like any other — surfaced by the runtime, handled by failure management (§10.4),
never silently absorbed.

**Epoch.** Platform time is **`int64` microseconds since the PTP epoch —
1970-01-01 00:00:00 TAI** — the PTP timescale: continuous, leap-second-free,
monotonic. This is the domain of every envelope timestamp, and `ridl.std`'s
`Timestamp` type shares the same epoch and resolution, so domain time in
payloads and transport time in envelopes subtract cleanly. Civil datetime
(UTC/local, leap seconds, time zones) is a _presentation_ conversion at the
edges — never the computation domain.

Because the envelope always exists, **contract payloads should not re-declare
it**: a payload field carrying publication time or a frame counter draws an info
lint (RIDL-406; §16.4 names the eight field spellings it matches). The
legitimate exception is _domain_ time distinct from transport time — a
`FaultEvent.timestamp` recording when the fault _occurred_ belongs in the
payload, because the envelope of a streamed history reply timestamps delivery,
not occurrence.

### 3.2 Interaction Families

Every interaction declares a **family** — who is on the other side of the
boundary. The counterparty set is closed, because there are only three kinds of
thing a system can talk to: **another piece of software, a person, the physical
world** (ADR-0012 decision 2).

| Family           | Direction       | The provider's promise          |
| ---------------- | --------------- | ------------------------------- |
| **dispatch**     | system → system | _transfer, nothing interpreted_ |
| **presentation** | system → person | _I offer this to be perceived_  |
| **intent**       | person → system | _I capture what you meant_      |
| **acquisition**  | world → system  | _I report what is_              |
| **control**      | system → world  | _I cause this to happen_        |

`dispatch` is the default and the baseline: an interaction that declares no
family is `dispatch`, and everything in §4 through §14 reads unchanged for it.
The other four are where the datum and the thing it stands for come apart, and
they carry the obligations of §3.3.

**The family is a property of the declaration**, not of the file, the package,
or the deployment, and it is **selected by the declaring keyword** — there is no
family clause. `signal`, `event`, `command`, and `query` are `dispatch`, so a
`.ridl` file declares nothing else. The non-dispatch spellings belong to the
rxdl reference and lower to these same kinds carrying a family; this document
owns what a family _means_ and every rule one can violate, and rxdl owns only
the words. The family reaches the IR as a closed enum alongside the kind.

#### The agent asymmetry

Two families are epistemic (presentation, acquisition) and two volitional
(intent, control), and **the agent swaps sides**: at the person boundary the
person is the agent and the system informs; at the world boundary the system is
the agent and the world informs.

Every structural rule below follows from that one fact, so it is worth stating
plainly rather than deriving each rule separately.

#### Which kinds each family admits

| Family           | continuous (`signal`) | occurrence (`event`) | operation (`command`)    | request/response (`query`) |
| ---------------- | --------------------- | -------------------- | ------------------------ | -------------------------- |
| **dispatch**     | ✓                     | ✓                    | ✓                        | ✓                          |
| **presentation** | ✓                     | ✓                    | —                        | —                          |
| **intent**       | —                     | ✓                    | ✓ — a **shape** (§3.2.1) | —                          |
| **acquisition**  | ✓                     | ✓                    | —                        | _reserved_                 |
| **control**      | ✓                     | —                    | ✓                        | —                          |

A combination outside this table is **RIDL-501**. Each absence is a consequence
of the agent asymmetry, not an unfilled gap:

- **presentation admits no operation.** Nothing can be invoked on an agent. A
  provider may present, and the person may then act; it may not call them.
- **intent admits no continuous value.** A person's continuous state is knowable
  only by measuring a physical proxy, and measuring a physical proxy is
  `acquisition`. An accelerator pedal is an acquisition signal, not an intent
  one: there is no interpretation gap between a foot's position and its meaning,
  and all four obligations instantiate exactly as they do for a wheel-speed
  sensor. **A person therefore appears at two boundaries** — as an agent that
  means things, and as a physical object that is measured.
- **intent admits no query.** A provider cannot block on a human. A confirmation
  is a presentation followed by an operation — two interactions, not a
  request/response.
- **acquisition admits no operation.** Commanding a device to recalibrate acts
  on the world, so it is `control`.
- **control admits no occurrence.** Nothing leaves a system uninvited.
- **control admits no query.** Effect is never returned; it is observed by
  measuring back, which is a separate acquisition interaction paired with the
  control one (§3.3).

The reserved acquisition query is the explicit one-time read — see §17.10.

#### 3.2.1 Operation shapes

An intent operation declares a **shape**, which constrains its parameters:

| Shape      | Meaning                     | Parameters                       |
| ---------- | --------------------------- | -------------------------------- |
| `activate` | invoke                      | none                             |
| `toggle`   | flip a binary state         | none                             |
| `select`   | choose one from a set       | exactly one — the key            |
| `adjust`   | set a value within a range  | exactly one — a ranged/unit type |
| `dismiss`  | cancel or close a transient | none                             |

Violating a shape's parameter rule is **RIDL-502**.

**The set is closed and has no generic form.** A person performs one act at a
time; a multi-parameter operation is a function call, not a gesture. Sending a
message is an occurrence carrying the body, a `select` of the recipient, and an
`activate` — three interactions, each separately observable and testable.
Repositioning an item, stripped of modality, is `select` then `adjust`.

An intent operation without a shape is **RIDL-503**. The world boundary has no
such restriction: a `control` operation takes whatever parameters it needs,
because nothing there is an agent constrained to single gestures.

Shapes are machine-readable gesture semantics. They drive interface scaffolding,
accessibility roles, and the test plane's parameter generation — which is what
earns them keywords rather than an attribute, and what a generic escape hatch
would quietly destroy.

### 3.3 Correspondence Obligations

Between two software peers **the datum is the truth**: both sides agree on a
number and there is nothing behind it. At every boundary with the non-software
world, **the datum and its referent come apart**.

The canonical case is a speedometer. A vehicle's true speed and the number on
its cluster are deliberately different quantities: the indication is legally
constrained never to read below the true value, is quantised with hysteresis, is
damped, and is unit-switched per market. Declaring both as a plain `signal`
erases the distinction that matters, and the wrong one eventually reaches the
cluster.

**Direction rule.** In every non-dispatch family one side causes the other. The
causally upstream side is the **reference**; the downstream side is the
**realisation**; the obligation is that the realisation corresponds to the
reference within declared bounds.

| Family       | Reference             | Realisation               |
| ------------ | --------------------- | ------------------------- |
| presentation | the provider's value  | what the person perceives |
| intent       | what the person meant | the captured value        |
| acquisition  | the physical quantity | the reported value        |
| control      | the commanded value   | the resulting world state |

#### The four obligations

| Obligation                    | Presentation                 | Acquisition                          | Control                     |
| ----------------------------- | ---------------------------- | ------------------------------------ | --------------------------- |
| **relationship**              | never under-reads; quantised | transfer function; calibration       | authority limits; slew rate |
| **uncertainty**               | display resolution           | tolerance per reading                | positioning tolerance       |
| **latency of correspondence** | perception delay             | sample instant ≠ publication instant | actuation lag               |
| **failure to correspond**     | shown but not perceivable    | plausible but false                  | commanded but not achieved  |

Two are load-bearing enough to state separately.

**Latency of correspondence is not delivery latency.** The envelope is
sender-stamped at publication (§3.1). For a transducer with a response time, the
value describes the world as it was _before_ that stamp, so any model computing
with the time of the cause is silently wrong by the transducer's lag unless the
contract declares it. The obligation has **two forms**: an _instant_ offset, and
a **span** for a value acquired over an interval rather than at a point — a
swept sensor frame corresponds to the world across its sweep, which is why
motion distortion exists in such frames.

**Failure to correspond is not invalidity.** §4.5's invalid state detects a
malformed or unavailable value. A sensor can report a well-formed, in-range,
perfectly fresh value that is **false** — a wheel spinning on ice. Nothing in
the datum reveals it, which is why plausibility must be declared rather than
inferred.

#### Obligations relate two declarations

An obligation frequently qualifies a **pair** rather than a single declaration:
raw against indicated speed, commanded against achieved angle, a toggle against
the state it flips, a set value against the value shown back. The pair is where
the obligation lives, and one mechanism serves every family.

This is also a real failure mode with no other expression: a person acts, the
indication does not follow, the person acts again — and both declarations were
individually satisfied.

#### Obligations compose along a path

Correspondences chain, and the chain is the point:

```text
true wheel speed
   │  acquisition — measurement error, sensor lag
   ▼
the provider's speed value
   │  relationship — legal bias, quantisation, damping
   ▼
the indicated value
   │  presentation — refresh rate, legibility, perception delay
   ▼
what the driver believes
```

The legal requirement is stated at the **end** of that chain, so it is
verifiable only if every hop declares its own contribution. An end-to-end budget
is computable exactly when the path is fully declared, and not otherwise.

#### Metrology is the reference

These are metrology's concepts, and the correspondence with **VIM** (JCGM 200 /
ISO-IEC Guide 99) is term for term:

| VIM term                                                                                                | Here                                  |
| ------------------------------------------------------------------------------------------------------- | ------------------------------------- |
| **measurand**                                                                                           | what an acquisition interaction names |
| **calibration** — the relation between quantity values and **indications**                              | the relationship obligation           |
| **measurement uncertainty**                                                                             | the uncertainty obligation            |
| **indication** — the value a measuring instrument provides                                              | a presentation-family value           |
| **detector** — indicates presence when a threshold is exceeded                                          | an acquisition occurrence             |
| **metrological traceability** — an unbroken chain of calibrations, each contributing to the uncertainty | the composition property above        |

Where this layer needs to name something further, VIM is the reference to reach
for before inventing a word. One VIM concept has no expression here yet — the
**influence quantity**, which does not affect the measurand but does affect the
relation between indication and result, such as ambient temperature shifting a
pressure sensor's characteristic. It is neither uncertainty (which is
dispersion) nor the relationship (it is what perturbs the relationship); see
§17.11.

#### Declaration

Obligations are **attributes** in the tail block (general-form §4.2), because
they are authored, heterogeneous, and mix the assignment and predicate forms.
The working spellings are below; they are fixed with the obligation
implementation and an author should expect them to settle rather than change in
substance.

The example below is written in the **rxdl spellings**, because a `.ridl` file
spells only `dispatch` and these are non-dispatch interactions. `measure` is a
`signal` in the `acquisition` family and `present` a `signal` in `presentation`;
both lower to the kinds and attributes this section defines.

```ridl
interface Cluster {
  measure wheelSpeed: Speed @10ms [
    tolerance = 0.5 km/h,
    lag       = 40ms,
    plausible abs(delta) < 10.0
  ]

  present clusterSpeed: IndicatedSpeed @[50ms..200ms] [
    derives    = wheelSpeed,
    resolution = 1 km/h,
    ensure clusterSpeed >= wheelSpeed
  ]
}
```

A non-dispatch interaction that declares **no** obligation at all draws
**RIDL-504**: the family exists precisely to make these facts explicit, and a
family tag with nothing behind it is a declaration wearing a label.

### 3.4 Availability

An interaction may be unavailable, and unavailability has **five distinct
sources**. `during` (§13.3) covers the first; the rest are predicates.

| Source           | Example                                    | Carried by                                  |
| ---------------- | ------------------------------------------ | ------------------------------------------- |
| **mode**         | only while the system is in `ERROR`        | `during` — a state gate                     |
| **data**         | disabled until a value validates           | a predicate over declared state             |
| **in progress**  | already running, cannot be re-invoked      | a predicate over a declared progress value  |
| **policy**       | not permitted in this condition            | a predicate over declared policy state      |
| **provisioning** | this variant does not have the interaction | `fixed` — and the interaction is **absent** |

The first four render as _present but refused_; provisioning renders as _not
there at all_. Those lead a consumer — and a person — to different conclusions,
and they are different tests.

#### The consumer-evaluability rule

> At a **presentation-obligated** boundary, an availability condition must be
> **evaluable by the consumer**.

A person must perceive that something is unavailable _before_ attempting it, not
discover it by rejection. A renderer cannot disable a control whose condition it
cannot evaluate, so an availability predicate on an `intent` interaction may
reference **only declared, consumer-visible state** — never provider-internal
state. Violating this is **RIDL-505**.

At the `dispatch` and `control` boundaries the rule does not apply: a software
caller or an actuator may simply be rejected, and §13's ordinary `require`
semantics hold unchanged.

**Availability is general; pre-visibility is not.** Every family has all five
sources — an actuator is mode-gated, derated, mid-stroke, policy-gated, and
fitted or not. Only the person boundary requires that unavailability be
perceivable in advance.

---

## 4. Signal

A **continuous volatile state value**. The latest sample is the truth;
intermediate samples may be missed.

```ridl
signal currentSpeed : Speed @10ms
signal engineTemp   : Temperature @[20ms..100ms]
signal warnings     : WarningFlags @[50ms..1s]
```

### 4.1 Rules

- Single named-type payload
- Timing annotation optional: `@Xms` (strict periodic) or `@[min..max]`
  (change-driven with bounds). An untimed signal receives the **configurable
  default** `@[100ms..1000ms]` — a warning, and concrete bounds in the IR either
  way (§9.1)
- Optional bare `= <value>` suffix overriding the payload type's init value
  (§4.4)
- Stream `<T>` not valid (RIDL-201)
- No `require`/`ensure` — a signal has no call site; validity is the typl
  constraint (RIDL-301). Error-typed _payloads_ are legal — errors are data
  (§10.1)

### 4.2 Direction

The interface provider publishes; consumers subscribe. (Consumer-published flows
belong to the consuming component's own provided interface — every flow has
exactly one owning provider.)

### 4.3 Publication Semantics

With `@Xms`, the provider publishes every X ms regardless of change. With
`@[min..max]`, the provider publishes on change, no faster than the rate floor
`min` and at least every staleness bound `max` even unchanged. On a signal those
two generic bounds derive as debounce and refresh ceiling, because state may be
coalesced and survives being unchanged — see §9.

### 4.4 The Channel Is Never Empty — Init Value and Last-Value

**Normative:** a signal channel holds a value from the moment it exists.
Subscribing delivers a value **immediately** (within the transport's delivery
latency): the **init value** before the provider's first publication, the
**latest published value** thereafter. A signal is _state_ — state exists even
while unchanged, and it exists _before anything happens_.

- **Init value.** Defaults to the payload type's init value (typl §5.8 — a
  declared bare `= value` or derived). A signal may override it with the same
  bare `= value` idiom, written directly after the payload type — one syntax for
  init everywhere at the vocabulary and interaction layers, no keyword:

```ridl
signal targetSpeed : Speed = SPEED_LIMIT_EU @[20ms..500ms]
```

The init value is the channel-level counterpart of the type's own init: a
property of _this signal_, layered over the type's default. A payload type with
no derivable init and no `= value` override is a compile error (RIDL-109); an
override violating the payload constraints likewise (RIDL-110).

- **Last-value cache.** The binding/broker maintains it per signal, seeded with
  the init value at channel creation — this is the contract's demand on every
  transport mapping (SOME/IP field with initial value, MQTT retained message
  seeded at provisioning, DDS `TRANSIENT_LOCAL`; Appendix B).
- Application code can always distinguish _init_ from _published_ — the channel
  state carries provenance (init / live / invalid, §4.5), so consumers that must
  not act on a mere init value can tell.
- Events carry **none** of this (§5) — occurrences are not state.

_Precedent: AUTOSAR `initValue`, DDS durability, MQTT retained, SOME/IP field
semantics._

### 4.5 Invalid Values Propagate — As Invalid State

A payload violating the typl constraints is **never hidden**. The malformed
value itself is not delivered, but its _invalidity is_: the channel transitions
to the **invalid state**, propagated to every subscriber like any other state
change. Silent quarantine — subscribers unknowingly holding stale "last good"
data — is exactly the failure mode a safe system must not have.

The generated subscriber API therefore exposes the channel as: **value +
provenance**, where provenance is `init | live | invalid` (with the last good
value still accessible in the invalid state). What the application _does_ on
invalid — hold last-good, apply failsafe, degrade — is failure management
(§10.4), not contract semantics; the contract's job is that invalidity is
visible, typed, and timely.

Transport realisation: on CAN/AUTOSAR, channel invalidity is carried **in-band
as the SNA/invalid sentinel** (`0xFF`-style, AUTOSAR `invalidValue`) — which is
what typl's sentinel open question (typl §17.8) becomes at this layer; on
SOME/IP/DDS/MQTT it is a marked update or metadata flag per binding.
Observability hooks record every invalid transition (§10.3). There is no error
channel back to a publisher on pub/sub — a publisher emitting invalid values is
a provider bug surfacing through telemetry, not through consumers.

---

## 5. Event

A **discrete occurrence**. Every occurrence matters; occurrences are queued, not
coalesced.

```ridl
event doorOpened         : DoorPayload @[50ms..500ms]
event speedLimitExceeded : SpeedLimitPayload @[100ms..2000ms]
```

### 5.1 Rules

- Single named-type payload
- Timing annotation optional: `@[min..max]` only — strict periodic `@Xms` is
  meaningless for occurrences (RIDL-103). An untimed event receives the
  configurable default `@[100ms..1000ms]` (§9.1)
- Stream `<T>` not valid (RIDL-201)
- No `require`/`ensure` (RIDL-301)
- **No late-joiner delivery**: subscribing to an event delivers only occurrences
  raised after the subscription. No cache, no replay — an occurrence that
  happened before you were listening did not happen _to you_. (Replaying history
  is the test/observability plane's job, not the contract's.)

### 5.2 Timing Semantics

`min` is the rate floor and `max` the staleness bound, as everywhere (§9). On an
event they derive as a **throttle** (the provider must not raise occurrences
faster than `min`) and a **TTL** (an occurrence processed later than `max` after
being raised is stale and is discarded by the binding), because an occurrence is
individually meaningful and cannot be coalesced into its successor.

---

## 6. Command

A **fire-and-forget action request**. The consumer requests; the provider acts;
there is no reply.

```ridl
command setGear(position: GearPosition) [
  require position != GearPosition.PARK || currentSpeed == 0.0
]
command resetFaults()
command uploadFirmware(data: <FwBlock>)
```

### 6.1 Rules

- Parameters are named typl types; stream `<T>` permitted on parameters. A tuple
  is not a parameter type — pass a named `struct` (a tuple _is_ a query return
  type, §7.1)
- Always returns `()` — writing a return type is an error (RIDL-104); if the
  caller needs a result or a completion signal, use `query`
- **No functional-error channel, by construction**: a command has no return, so
  there is nowhere for a fallible return. A command whose _outcome_ the caller
  must know about is a `query` returning `T | E` — the language forces that
  honesty
- `require` permitted (§13); `ensure` not (nothing to observe)
- **A range timing annotation is permitted, and an undeclared response bound
  draws a warning** (§9.3): the `max` bound is a response bound on acceptance —
  the delivery acknowledgment below, never execution — and the `min` bound is a
  call throttle on the caller. RIDL-112 reports a command that declares no
  response bound; no default is applied
- **Fire-and-forget describes the contract, not the wire.** The runtime protocol
  carries a **delivery acknowledgment** beneath every command: the receiving
  binding confirms _received and accepted for execution_ (payload valid,
  precondition passed) or negatively acknowledges with the Stratum 2 category
  (§10.2). The ack carries no functional payload, never reaches the contract
  surface, and is not application-visible as a return value — it exists so the
  runtime can implement retries, delivery supervision, and duplicate suppression
  (envelope sequence numbers, §3.1), and so a rejected command is a _detected_
  event in a safety context rather than a silent one
- **A command is therefore fallible like everything else** — invalid payload or
  failed precondition (Stratum 2, negative ack), or undelivered (Stratum 3, ack
  timeout) — all visible to the calling _runtime_, none to the _contract_. What
  the contract does not promise is execution _outcome_: observable results of a
  command travel back as state (e.g. `setGear` → observe `currentGear`). This is
  the CQRS discipline: commands mutate, state reports.

### 6.2 Provider-Side Rejection

The provider binding validates parameters (typl constraints) and `require`
clauses before invoking application code. A violating command is rejected —
never partially executed — and the rejection surfaces twice: as a **negative
acknowledgment** carrying the Stratum 2 category back to the calling runtime
(§6.1), and through the provider's observability hooks. Application code on both
sides stays uninvolved.

---

## 7. Query

A **request/response** interaction. The reply is mandatory; failure is
expressible.

```ridl
query getAverageSpeed(window: Duration): Speed [
  require window > 0ms
  ensure  result >= 0.0
]
query getMinMax(window: Duration): (min: Speed, max: Speed)
query streamFaults(filter: DiagFilter): <FaultEvent>
query calibrate(axle: Axle): CalReport | CalError     // inline T | E — fallible query, §10.1
```

### 7.1 Rules

- Must return non-void — `()` return is an error, use `command` (RIDL-105)
- Return type: named type, named-field tuple, inline `T | E`, or stream `<T>`;
  streams also permitted on parameters (bidirectional streaming supported)
- `require` and `ensure` permitted (§13); `ensure` constrains `result`
- Functional failure is expressed **in the return type**: the inline `T | E`
  form makes the query fallible — §10.1. There is no `throws` clause in ridl
- **A range timing annotation is permitted, and an undeclared response bound
  draws a warning** (§9.3): the `max` bound is a response bound on the reply,
  and the `min` bound is a call throttle on the caller. RIDL-112 reports a query
  that declares no response bound; no default is applied

### 7.2 Concurrency and Idempotence

The contract does not serialize queries — providers may answer concurrently and
out of order. Queries **should** be read-only or idempotent; a state-mutating
request belongs to `command` (with state observation) unless the mutation
inherently needs a result (e.g. `allocate`, `calibrate`). Linters flag
verb-named queries (`set…`, `reset…`) as probable commands; §16.4 names the six
verbs RIDL-404 matches.

---

## 8. Fixed

A value **provisioned externally** — build, factory, FOTA — and **immutable for
the lifetime of the running software instance**.

```ridl
fixed vin             : Vin
fixed softwareVersion : Version
fixed capabilities    : [Label; 0..32]
```

- Single named-type payload (collections permitted); no timing, no attribute
  block (RIDL-106)
- Read-only: no interaction can mutate it; safe to cache unconditionally
- Reading a `fixed` is free of the query machinery — bindings expose it as a
  plain accessor, populated at binding initialization
- Naming decision on record: `fixed`, not `final` and not `config` — one word
  for one concept at every boundary (ADR-0011); `fixed` is family-neutral,
  because a provisioned constant carries no correspondence anywhere. `final` was
  the earlier spelling and misled: a Java or Kotlin reader takes it for a
  compile-time constant, which this is not. `config` connotes hot-reload and is
  reserved vocabulary space for rsdl. See the concept-note naming ledger

Maps to: Android `ro.*` properties, AUTOSAR `CalibrationParameter`, SOME/IP
field with getter only.

---

## 9. Timing Annotations

Timing is the `time` core surfacing in ridl. The `@` annotation is **part of the
contract**, not documentation — it generates freshness monitoring, drives rmdl
base clocks, and defines "late" for the observability plane.

**Strict periodic — signal only:**

```ridl
signal currentSpeed : Speed @10ms
```

**Range — signal, event, command, and query:**

```ridl
@[20ms..100ms]    // both bounds
@[20ms..]         // lower bound only
@[..100ms]        // upper bound only
```

**The bounds have one generic meaning**, whatever the declaring keyword (general
form §6.2):

> **`min` = rate floor** — the minimum interval between publications. **`max` =
> staleness bound** — the maximum age, measured on envelope sender timestamps
> (§3.1), before the value or occurrence is stale.

So the naive reading of `@[20ms..500ms]` — "not faster than 20ms, not staler
than 500ms" — is the correct one, and it is the same reading on a signal and on
an event.

What differs per kind is **what the runtime does** at each bound, and that is
_derived_ from the semantics of the declaring keyword (§3), not from the
annotation. The familiar names are the derivation, not the definition:

| Bound                   | On a `signal` (state)                                            | On an `event` (occurrence)                                    | On a `command` / `query` (call)                       |
| ----------------------- | ---------------------------------------------------------------- | ------------------------------------------------------------- | ----------------------------------------------------- |
| `min` — rate floor      | **debounce** — a faster update is coalesced into the next sample | **throttle** — the provider must not raise occurrences faster | **call throttle** — the caller must not call faster   |
| `max` — staleness bound | **refresh ceiling** — re-publish even if unchanged               | **TTL** — an occurrence older than `max` is discarded         | **response bound** — the provider must respond within |

A state value that is stale is refreshed and a fast one coalesced, because state
survives being unchanged and only the latest sample matters; an occurrence that
is stale is discarded and a fast one throttled, because every occurrence is
individually meaningful and cannot be merged into its successor. A call has no
sample to refresh or discard: its two bounds oblige the two ends of the call
directly — §9.3 (ADR-0015). Editor hover expands the per-kind consequence from
the declaring keyword.

### 9.1 Default Timing

A signal or event with no `@` annotation receives the default range
**`@[100ms..1000ms]`** — a 100ms rate floor and a 1s staleness bound, derived
per kind as above. The default is configurable at compile time in `ridl.toml`:

```toml
[defaults]
timing = "[100ms..1000ms]"    # applies to untimed signals and events
```

Resolution follows the ADR-0002 precedence: package-level `[defaults]` shadows
workspace-level, which shadows the built-in `[100ms..1000ms]`. Three properties
keep this safe:

- **The IR always carries concrete bounds.** The default is applied at compile
  time, so every downstream consumer — freshness SLOs, observability
  conventions, rmdl, test generators — sees a fully timed contract; "untimed"
  does not exist beyond the parser.
- **An untimed interaction draws a warning** (RIDL-100: default applied); an
  active profile may escalate to an error and require explicit timing — the same
  idiom as typl's default string bounds (typl §4.4). Safety-graded packages
  should require explicit timing.
- **Changing the configured default is a contract change.** Because `ridl-diff`
  compares _resolved_ IR, editing `[defaults].timing` flips the bounds of every
  untimed interaction in the package and is flagged accordingly — the default is
  a convenience, not a loophole.

Strict periodic `@Xms` is never defaulted — an isochronous rate is always an
explicit engineering decision (it drives rmdl base clocks).

**A `command` or `query` is never defaulted either** (ADR-0015 decision 4). An
RPC with no declared response bound draws RIDL-112 (warning; an active profile
may escalate it to an error, the same two-step as RIDL-100 above). What it does
not get is a default: there is no plausible generic value, because what the
provider does differs by orders of magnitude between interactions; and a
defaulted response bound is worse than none, because it is a provider obligation
callers size their own timeouts against, so inventing one manufactures a promise
nobody made. Absent means undeclared in the IR, so an undeclared RPC bound stays
clear of the changed-default machinery above.

### 9.2 Validity Rules

Rules: `@0ms` is an error (RIDL-102); `@[X..Y]` with `X > Y` is an error
(RIDL-101); `@[X..X]` draws a warning (RIDL-108) on every kind that admits the
range — it is a degenerate range, a rate floor equal to its staleness bound,
which is almost always a mistake. It is **not** a spelling of the strict period
`@Xms`: a strict period is a separate mode, admitted on signals only, never
defaulted (§9.1), recorded in the IR beside the bounds, and a change between the
two modes is breaking whatever the bounds do.

Timing belongs to every kind but `fixed`. A `signal` takes either form; an
`event`, a `command`, and a `query` take the range form only, so a strict period
`@Xms` on any of the three is RIDL-103 — an isochronous rate is meaningless for
occurrences, and a caller is not isochronous by contract (ADR-0015 decision 5).
An `@` annotation on a `fixed` is RIDL-106. The grammar admits the annotation on
every interaction kind so that each narrowing is a semantic rule with a semantic
message; none is a parse error.

A signal's `@Xms` or `@[..max]` is an alertable **freshness SLO**: a subscriber
that has not seen a publication within the bound may treat the value as stale —
generated bindings expose staleness, and the observability conventions map the
bound to an OTel attribute. Every timing bound — the rate floor, the staleness
bound, and the derived debounce, refresh, throttle and TTL behaviour — is
evaluated on **envelope timestamps** (§3.1), never on payload content.

_DDS students will recognise `max` as the DEADLINE QoS and staleness as
LIVELINESS — ridl puts them in the contract instead of a QoS profile; see
Appendix F._

### 9.3 RPC Bounds — the Call Throttle and the Response Bound

`command` and `query` admit the range form, and only the range form (ADR-0015
decisions 2, 3, and 5). The two bounds keep their generic meaning; the per-kind
consequence is derived from the declaring keyword, exactly as it is for state
and occurrences:

- **`min` is the call throttle** — the minimum interval between calls. It
  constrains the **caller**, not the provider. That is not an inconsistency:
  `min` always constrains whoever initiates, and on an RPC the initiator is the
  consumer. It is enforceable at the provider's admission point, and it is what
  a rate-limiting binding already implements.
- **`max` is the response bound** — the provider must respond within it. What
  responding means is derived per kind. For a `query` it is the **reply**. For a
  `command` it is **acceptance** — the delivery acknowledgment of §6.1 — since
  §6.1 promises no execution outcome: a command's bound covers admission and
  queueing at the provider, and **not** execution.

The response bound is a bound on the response, not on delivery. Delivery latency
is a property of the link, so a per-interaction delivery bound would declare the
same number on every declaration in the package and carry no information.
Response time is what varies per interaction — which is why gRPC, DDS-RPC, and
AIDL all bound the whole call.

The half-open spellings carry the two partial cases: `@[..100ms]` is a response
bound with no throttle, and `@[20ms..]` a throttle with no response bound. An
RPC whose response bound is undeclared — a bare declaration, or the `@[20ms..]`
spelling — draws RIDL-112 and is **never defaulted** (§9.1). The warning is
about `max` specifically, not about the annotation; a missing `min` draws
nothing, because an unbounded call rate is the sensible default and is what
every RPC has today.

In the IR the declared bounds reuse the `Timing` message unchanged (ADR-0015
decision 7): the mode is always the range mode, `min_us` is the call throttle,
`max_us` the response bound, `default_applied` always false, and an undeclared
bound is absent. The response bound is for RPC what §10.3 calls the freshness
machinery for pub/sub: the contract-level view of transport health.

---

## 10. Errors

**New in v0.2.** The error model has three strata. None of them adds interaction
syntax: the first is expressed as _data_ from the typl vocabulary; the other two
never appear in source at all.

### 10.1 Stratum 1 — Functional Errors Are Data

ridl has **no error syntax** — no `throws`, no exceptions, no status codes. A
query that can fail _as part of its domain semantics_ says so in its return
type, with vocabulary declared in typl:

```ridl
error enum CalError {            // typl §10.1 — failure vocabulary
  SENSOR_UNAVAILABLE = 0
  VEHICLE_MOVING     = 1
  OUT_OF_RANGE       = 2
}

query calibrate(axle: Axle): CalReport | CalError    // inline T | E — the canonical spelling
```

Rules:

- An **inline `T | E`** in query-return position makes a **fallible query** —
  the canonical spelling (general form §6.1). The left arm is any non-error
  named type (success); the right arm is exactly one **error type**. Both arms
  stay named typl types; only the union _container_ is structural, and its
  transport identity is synthesized from the interface, the interaction ordinal,
  and the ordered arm types (ADR-0008 decision 4)
- Bindings split it mechanically: the success arm is the reply payload; the
  error arm maps to the transport's _native_ error channel — SOME/IP return
  code, gRPC rich status, AIDL exception — and to `Result<T, E>` (Rust) / sealed
  result (Kotlin) in language targets, with exhaustive handling enforced
  wherever the target can express it
- Several failure kinds compose into one `error union` _before_ appearing as the
  error arm (typl §10.2) — one query still carries one closed failure set
- A **named result union** (typl §10.2) remains legal typl data — for storing an
  outcome in a struct or a log — but in return position it draws RIDL-308, a
  lint steering to the inline spelling
- A query with no success path is an error (RIDL-303): a bare error type in
  return position, or an `error`-typed left arm of an inline `T | E`
- `command` remains failure-free **by construction**: it has no return, so there
  is nowhere for a fallible return — nothing to ban. A command's observable
  failure is state, like every other command outcome: publish a fault signal
  (§6.1)
- Because errors are ordinary vocabulary, **pub/sub can carry them**:
  `signal lastFault : ServiceFault @[1s..]` is legal and idiomatic for fault
  reporting — structurally impossible in throws-based designs, where failure
  lives in a control-flow channel that pub/sub does not have

**Rationale** (decided v0.2, replacing a draft `throws` clause). One concept
instead of two: failure shapes are typl types like every other shape — nominally
typed, importable, reusable across interfaces, evolvable under the same ordinal
rules. ridl adds zero error surface; the runtime keeps its own stratum below
without the contract pretending to model it. The cost — naming the failure type
on the signature — is exactly the honesty wanted: the failure set is part of the
signature, as data. The inline spelling (general form §6.1) puts it _at_ the
signature rather than one hop away in a named union.

### 10.2 Stratum 2 — Contract Errors: implicit, standardized, derived

Invalid values and violated preconditions are **contract violations, not
functional errors**. They are never declared — the typl constraints and
`require` clauses already describe them completely — and never occupy an error
type. The spec defines the categories once; bindings implement them uniformly:

| Category              | Trigger                                                                        | RPC surface (caller sees)                                                                                         | Pub/sub surface                                                            |
| --------------------- | ------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| `INVALID_VALUE`       | payload violates typl constraints (range, step, bounds, pattern) at a boundary | standard error, distinct from any result-union error arm — gRPC `INVALID_ARGUMENT`, SOME/IP `E_MALFORMED_MESSAGE` | channel transitions to **invalid state**, propagated to subscribers (§4.5) |
| `PRECONDITION_FAILED` | a `require` clause evaluates false                                             | standard error — gRPC `FAILED_PRECONDITION`, SOME/IP `E_NOT_OK`                                                   | command: negative ack to calling runtime + observability (§6.2)            |
| `CONTRACT_BROKEN`     | an `ensure` clause evaluates false — a **provider bug**                        | standard error to caller; incident-grade telemetry on provider                                                    | n/a                                                                        |
| `UNKNOWN_INTERACTION` | ordinal/version mismatch between peers (§11)                                   | standard error — SOME/IP `E_UNKNOWN_METHOD`                                                                       | subscription fails at bind time                                            |

Provider bindings evaluate constraints and `require` **before** application code
runs, and `ensure` after; application code never sees a contract-violating
input, and callers can always distinguish "the service said no" (Stratum 1) from
"the call was ill-formed" (Stratum 2).

### 10.3 Stratum 3 — Transport Errors: infrastructure failure — detected, undeclared

Timeouts, broker loss, serialization failures, connection resets, and a
command's **missing delivery acknowledgment** (§6.1): these exist for every
interaction identically and say nothing about the contract, so the language
never declares them. The normative phrasing is **"infrastructure failure —
detected, undeclared"** (general form §6.4), and both halves are load-bearing.
_Detected_: the runtime observes every one of them — acks, timeouts, staleness —
and nothing fails silently (§10.4). _Undeclared_: the contract language has no
vocabulary for them, because a contract author has no knowledge to express. This
is the opposite of undefined behaviour, where a system may do anything; the
generated code names the stratum in exactly these words. Generated caller-side
types carry a transport-error variant supplied by the runtime (`ridl-rt`), not
by the `.ridl` source — including command delivery status for callers that
choose to supervise it. The freshness machinery (§9) is the contract-level view
of transport health for pub/sub; the response bound (§9.3) is the same view for
RPC.

**The invariant across all three strata:** what the contract _declares_ is
exactly the failure knowledge a domain engineer possesses (Stratum 1); what the
contract _implies_ is derived mechanically and enforced uniformly (Stratum 2);
what the contract cannot know, it does not pretend to know (Stratum 3).

### 10.4 Failure Management Is Out of Scope — Deliberately

Detection is not management. This section specifies how failures are _detected
and classified_; what a system _does_ about them — failsafe states, fallbacks,
degraded modes, health monitoring, halt management — is a
**safety/quality-management concern**, specified separately and never expressed
in ridl syntax. The division of labour:

- **ridl declares** which failures a fallible query can answer with (the error
  arm of its `T | E` return — the vocabulary itself is typl's, §10.1) and the
  bounds whose violation _defines_ failure (typl constraints,
  `require`/`ensure`, freshness)
- **the runtime detects** every failure across all three strata — quarantines,
  negative acks, ack timeouts, staleness — nothing fails silently; this total
  detection is precisely what makes failure management _implementable_
- **the management layer decides** — supervision policies, fallback wiring,
  degradation ladders, health state machines. Its natural homes are the
  deployment topology (rsdl: which component supervises which, what replaces a
  failed provider) and the runtime specification (`ridl-rt` supervision hooks),
  informed by `@labels` assurance levels

**Planned direction — properties, not mechanisms.** The family is expected to
grow declarative safety/high-availability _properties_ across its layers: what
must hold, never how the runtime achieves it. Candidate surfaces, gated through
the `@labels`/profile system first and promoted to keywords only when earned:
typl — invalid/SNA sentinel values (typl §17.8); ridl — per-signal failsafe
values, availability requirements on interactions; rmdl — degraded-mode models
and observers acting as safety monitors; rsdl — redundancy, supervision
topology, fallback wiring. Deferred as a body of work of its own (§17.8) — in
v0.2 the contract's job ends at making every failure _visible and typed_.

---

## 11. Interaction Identity and Evolution

typl §7.4's model, applied to interfaces. Tag-based transports need stable
numeric identities per interaction (SOME/IP method/event IDs, proto RPC
identity).

- **Implicit ordinals** by declaration order (1-based) across all interactions
  of an interface, one sequence regardless of kind
- **Append-only**: new interactions go at the end; insert/reorder shifts
  ordinals — wire break, rejected by `ridl-diff`
- **`reserved` tombstones** retire removed interactions by name:

```ridl
interface VehicleStatus {
  signal currentSpeed : Speed @10ms
  reserved legacyTemp             // was ordinal 2 — retired
  event doorOpened : DoorPayload @[50ms..500ms]
}
```

- Transport IDs derive deterministically from ordinals (e.g. SOME/IP: method ID
  = ordinal for RPC kinds, event ID = ordinal with the event flag bit; Appendix
  B) — readable from source, no sidecar state
- **No in-language version block.** Franca's `version { major minor }` was
  considered and rejected: the package version (`ridl.toml`, ADR-0002) is the
  one version, and `ridl-diff` decides compatibility mechanically — a
  hand-maintained version pair inside the source is a second source of truth
  that drifts. Major-version coexistence is a deployment topology question
  (rsdl), not a contract question
- Changing an interaction's **kind** (e.g. event → signal), payload type, timing
  bound, or a fallible return's error arm in a breaking direction is detected by
  `ridl-diff` per its category rules; the diff exit-code contract (concept note
  §9.1) applies

**The same model applies one level up, to the interfaces of a service** (§14.5,
ADR-0015 decision 15): a service's shape list assigns each composed interface an
implicit **interface id**, 1-based by declaration order, with an inline shape at
slot 1. Appending a shape at the end is the sanctioned evolution; inserting or
reordering shifts ids and is breaking; removing one requires a service-level
`reserved` tombstone to hold its slot. An interaction's ordinal space stays
local to its interface — a binding keys the spaces on the interface **name**,
never on the list position (§14.5) — so the two levels never renumber each
other.

---

## 12. Streams

The stream container `<T>` is **ridl-owned** (typl §1.3): an unbounded sequence
valid only in interaction position — `command`/`query` parameters and `query`
returns.

```ridl
query streamFaults(filter: DiagFilter): <FaultEvent>     // server produces
command uploadFirmware(data: <FwBlock>)                  // client produces
query pipe(samples: <SensorSample>): <ProcessedSample>   // bidirectional
```

### 12.1 Direction by Position

| Position    | Direction                            |
| ----------- | ------------------------------------ |
| return type | provider produces, consumer consumes |
| parameter   | consumer produces, provider consumes |
| both        | bidirectional, full-duplex           |

### 12.2 Element Type

A named typl type, or raw `string`/`bytes` when the element is genuinely
unstructured (the one exception to typl §15.3, inherited from v0.1). The element
type's bound describes one logical element; the stream itself has no bound —
framing, buffering, and backpressure are transport concerns.

### 12.3 Restrictions

`<T>` on `signal`/`event` (RIDL-201), struct fields, or collections (typl
TYPL-301 territory) is an error. A signal _is_ the better stream for state; an
unbounded push of occurrences is an `event`; `<T>` exists for RPC-scoped
transfers with a beginning and an end.

Maps to: gRPC streaming, Kotlin `Flow`, Rust `Stream`, WIT `stream<T>` (WASI
0.3), SOME/IP segmented transfer.

### 12.4 Errors on Streams

Functional failure around a stream is data, like everywhere else: a fallible
_start or mid-stream abort_ is expressed by making the **element type a result
union** — the producer emits a terminal error element and closes (the recorded
idiom, pending §17.6). Element validity is Stratum 2 per element (invalid
elements quarantined; continue-vs-abort is §17.6); transport interruption is
Stratum 3. There is no channel-level functional abort construct in v0.2.

---

## 13. Contracts — require / ensure

The `expr` core surfacing in ridl. Attribute blocks `[ ]` on `command` and
`query`:

```ridl
command setRange(min: Speed, max: Speed) [
  require min < max
  require max <= MAX_SPEED
]

query getAverageSpeed(window: Duration): Speed [
  require window > 0ms
  ensure  result >= 0.0
]
```

| Attribute        | Meaning                                                      | Valid on           |
| ---------------- | ------------------------------------------------------------ | ------------------ |
| `require <expr>` | precondition over parameters and the interface's own signals | `command`, `query` |
| `ensure <expr>`  | postcondition over `result` (and parameters)                 | `query` only       |

- Expressions are type-checked, unquoted, side-effect-free; they reference
  parameters, `result`, constants, enum values, and the interface's own signals
  (a `require` may read `currentSpeed` — the provider's latest published state)
- Violations are Stratum 2 errors (§10.2) — never the error arm of a `T | E`
- The full expression grammar is the **expr-core specification**. This document
  fixes the _positions_ and the _checking discipline_; the expr-core
  specification §3.1 fixes the grammar of the **guaranteed subset** —
  comparison, boolean connectives, arithmetic, enum access, tuple-field access,
  and duration comparison, over parameters, `result`, constants, enum values and
  the interface's own signals — and §8 fixes the RIDL-306 boundary that rejects
  everything outside it
- One assertion, four executions (concept note §9.2): static where decidable,
  property test in CI, online observer on live flows, synchronous observer in
  the rmdl reference oracle. `require`/`ensure` written here are the _source_
  for all four

---

## 14. Interfaces and Services

ridl names contracts at two levels — the shape and its global declaration — the
split the whole SDV industry converged on (AUTOSAR Adaptive _ServiceInterface_
vs _Service Instance_; VSS branch type vs instance; uProtocol interface vs UUri
endpoint):

- **`interface`** — the **abstract shape**: a reusable, identity-less group of
  interactions. A contract _type_. Defined once, realized by many services. The
  analogue of a class or a proto service definition.
- **`service`** — a **global, named, published declaration of one or more
  interfaces**: the SSOT catalog entry that gives contracts concrete identity in
  the system, addressable as `service.member`. The analogue of a global named
  instance. What actually gets provided, deployed, and (optionally) discovered.

`interface : service :: type : global-named-instance`. This section covers both.

### 14.0 Interface — the abstract shape

An `interface` groups interactions under a named, reusable contract shape. It
has **no identity and no location** — it is not a runtime unit, not addressable,
not deployed. It exists to be _realized_ by services (§14.5) and to type them.
Multiple services may share one interface (a redundant twin, a second vehicle
variant, a family of sensors).

```ridl
/**
 * Main vehicle status contract shape.
 * @labels SIL_B, CAL_2, PRIVATE
 */
interface VehicleStatus {
  signal currentSpeed : Speed @10ms
  signal warnings     : WarningFlags @[50ms..1s]

  event doorOpened : DoorPayload @[50ms..500ms]

  command setGear(position: GearPosition) [
    require position != GearPosition.PARK || currentSpeed == 0.0
  ]

  query getAverageSpeed(window: Duration): Speed [
    require window > 0ms
    ensure  result >= 0.0
  ]

  fixed softwareVersion : Version
}
```

### 14.1 Rules

- Contains interactions and `reserved` tombstones only — type declarations live
  at package level (they are typl), never inside an interface (RIDL-107)
- **Flat: there is no interface inheritance.** `extends` was considered and
  rejected (Franca/CORBA precedent notwithstanding): inheritance couples the
  derived interface's wire identity to base evolution (a base insertion
  renumbers every child), complicates audit ("what is this service's complete
  contract?" requires closure computation), and has no faithful mapping to proto
  services or DBC. Shared _shapes_ are typl; a shared _interaction set_ is
  **composed at the service** — a service lists several interfaces (§14.5), so a
  recurring block keeps its own ordinal space beside every contract it joins and
  is never flattened into one (§17.2, ADR-0015)
- Interface names are `CamelCase`; interaction names `camelCase`
- `@labels` classification and profiles as in typl §14.3 — assurance labels
  (`SIL_B`, …) gate the test plane's injection rights (concept note §9.2)

### 14.5 Service — the global published declaration

A `service` is a **global, named declaration** that contracts of the given
interface shapes exist in the system. It is the **SSOT catalog entry**:
system-visible at design time, addressable by name, and the unit that rsdl
components _provide_ (§14.6) and that deployment realizes.

```ridl
service veh.adas.cruise      : CruiseControl
service veh.body.doors       : DoorControl, DiagBlock
service veh.powertrain.motor : MotorControl
```

- A service has a **dotted global name** (reverse-domain, like packages) and a
  **comma-separated list of interface shapes** after `:` (ADR-0015 decision 12).
  Its members are addressed `service.member` — `veh.adas.cruise.engaged`,
  `veh.adas.cruise.setLever`.
- **Commas are required between shapes** — the one list in the family whose
  separators are not optional (ADR-0015 decision 13). The reason is structural:
  every other list is terminated by a closing token, and this one is not — it
  ends where the next declaration begins. Without required commas the list would
  parse greedily, and a mistyped declaration on a following line (`Struct Foo {`
  for `struct Foo {`) would be absorbed as another shape, with the error
  surfacing at the `{` and saying nothing about the mistake. A trailing comma
  stays optional.
- The name is **unique across the system** (RIDL-140) — the service catalog is a
  flat global namespace, the SSOT every component agrees on. This is what makes
  `veh.adas.cruise.engaged` an unambiguous system-wide address (the
  VSS/uProtocol model).
- A service is **always public**: it takes no `internal` modifier, and every
  shape it lists must be public too. Publishing an `internal` interface at a
  global address is RIDL-143, reported against the offending list element — the
  address would name a contract no importer can implement. Drop `internal` from
  the interface, or give the service an inline shape. A shape that is not an
  interface at all is RIDL-141, likewise per element.
- A service may instead declare **one inline shape**, for a one-off global
  contract not worth a reusable interface — never both forms in one declaration
  (ADR-0015 decision 14):

```ridl
service veh.hvac.cabin {
  signal  temperature : Temperature @[1s..10s]
  command setTarget(t: Temperature)
}
```

#### Composing interfaces — ids, flat addressing, and the rules

A service composing several interfaces gives each a slot in its list, and the
list follows §11's identity model **one level up** (ADR-0015 decision 15):

- **Interface ids are implicit, 1-based, by declaration order.** An inline shape
  is slot 1, which makes the inline form a degenerate case of the general one.
- **Append-only.** Adding a shape at the end appends; inserting or reordering
  shifts ids and is breaking; removing one requires a **service-level `reserved`
  tombstone** to hold its slot. The tombstone spells the retired interface's
  name — the `reserved <Name>` form the family uses everywhere; the literal
  spellings typl's `reserved` grammar admits name no interface and are RIDL-148
  here (ADR-0015 decision 24):

```ridl
service veh.body.doors :
  DoorControl,
  reserved LegacyDoorDiag,
  HealthBlock
```

- **Ordinals stay per-interface, and a binding separates the ordinal spaces by
  interface name** (ADR-0015 decision 17), not by list position. Renumbering
  interactions across a service was rejected: an interface's wire identity would
  then depend on what else the service happens to carry — the coupling §14.1
  rejected inheritance to avoid. Appendix B already maps a SOME/IP eventgroup to
  an interface, so a multi-interface service maps to several transport-level
  groupings under one logical name. Keying on the name also makes reordering the
  list invisible to transport identity — though `ridl-diff` still classifies a
  reorder as breaking, because the _id_ moves even when the transport key does
  not.
- **Addressing stays flat** (ADR-0015 decision 16): members remain
  `service.member`, so every address written before composition existed stays
  valid. A member name duplicated across a service's interfaces is a compile
  error (RIDL-144) — `service.status` with two referents cannot be expressed.
  The accepted cost is stated rather than discovered: **two independently
  written interfaces that share a member name cannot be composed into one
  service without renaming one of them.**
- **The same interface named twice in one service is RIDL-145** — its own code
  rather than one RIDL-144 per member, which would bury the actual mistake; the
  duplicate listing is dropped and holds no slot.
- **A shape re-declared under a service-level `reserved` name is RIDL-146** —
  the analogue of RIDL-401 one level up: a tombstone retires an interface name
  permanently, at the service level as inside an interface body.
- **Two shapes whose interface names collide even though their references differ
  is RIDL-147** (ADR-0015 decision 24) — a binding separates the ordinal spaces
  by interface name, so a service carrying `fleet.c1.DiagBlock` and
  `fleet.c2.DiagBlock` leaves the binding no way to tell the two apart. An
  import alias cannot fix it — the name is the interface's own — only renaming
  one interface or composing it into a different service can, which is why the
  case has its own code rather than folding into RIDL-145. The rule is over
  every shape, live or retired: a name spelled by two `reserved` tombstones
  draws the same code, because two slots under one name would leave the shape
  list without the per-name key `ridl-diff` matches slots by — a name is retired
  once.
- **Extracting an inline shape into a named interface is breaking, by design.**
  Slot numbering survives the refactor (the inline shape is slot 1), but a
  fallible return's transport identity derives from the enclosing interface name
  (ADR-0008 decision 4), and an inline shape uses the service's dotted name
  instead — so extraction rewrites the identity of every fallible query in the
  shape, and `ridl-diff` classifies the form switch as breaking. Making that
  refactor compatible would be a wire-identity decision in its own right
  (ADR-0015 decision 15).

- **Posture-neutral by design.** A service declaration says _nothing_ about how
  it is realized on the wire. rsdl and deployment choose the **posture** —
  static (its signals/events packed into bus frames, Classic/CAN) or discovered
  (SOME/IP/DDS/uProtocol, Adaptive) — from the same declaration (rsdl §8). This
  is the whole point of ridl being the bus SSOT: `service veh.adas.cruise`
  compiles down to static CAN signals _or_ a discovered service, no rewrite. One
  constraint follows from transport physics: a `command`/`query` member (RPC)
  cannot be realized on a pure static bus — buses carry dataflow, not calls — so
  a service deployed statically realizes only its `signal`/`event` members; its
  control API requires the discovered posture (enforced at deploy time, rsdl
  §8).
- **Providing and redundancy** are rsdl concerns (§14.6): a component `provides`
  a service; two components providing the same service is _declared redundancy_
  (rsdl §10), not a conflict.

#### Coherence — the published set is simultaneous

> The signals of one **provided interface** are published coherently: the set a
> provider publishes in one step is a set of values it held simultaneously. A
> consumer reading two or more of them observes such a set wherever the binding
> preserves the grouping; where a binding cannot, that is a deploy-time
> constraint, not a weaker contract. The group's identity is the **interface
> name** where a service names a shape, and the **service name** where the shape
> is inline.

The rule states **production** coherence (ADR-0015 decision 9). The three rules
below establish that the provider's published set is simultaneous; they
establish nothing about what survives an arbitrary transport, which is why the
consumer sentence is conditional on the binding. What a consumer observes under
a given binding is delivery coherence, treated separately below.

The rule is **implicit, not declared** — there is no `coherent` keyword. Three
rules the family already states produce it: §4.2 gives every flow exactly one
owning provider; a provider computes its outputs in one step (rmdl's topological
schedule, rmdl §6); and each provider realizes the service it publishes as a
whole (rsdl §5.3, RSDL-502). So the values a provider publishes in one step are
a simultaneous state by construction. Signals publishing at different rates do
not break this: in a given step some cells are written and others are not, but
every value present is one that provider held at that step, so the observed set
remains a state that existed.

Declared redundancy does not weaken the rule. Two components may provide the
same service (§14.6 above, rsdl §10), and the rule then holds of each provider's
published set — which is what a consumer reads, since a consumer reads one
provider at a time.

Declaring `coherent` would declare a consequence of how the platform executes,
which the general form §4.1 deletion test exists to reject. It would also be
false in the one place it appeared to help: marking one interface coherent would
imply the others are not, when all of them are.

#### Production coherence and delivery coherence

Production coherence and delivery coherence are different, and the difference is
a demand on every transport mapping (ADR-0015 decision 10) — in the same sense
the §4.4 last-value guarantee already is. The provider producing a coherent set
is implicit; whether a consumer observes it coherently depends on the binding:

| Transport        | How the grouping is preserved                         |
| ---------------- | ----------------------------------------------------- |
| shared memory    | one versioned block per group, one generation counter |
| DDS              | GROUP-scope PRESENTATION with `coherent_access`       |
| SOME/IP          | one notifier per field; preserved only within a field |
| static bus (CAN) | preserved only within one frame; not across frames    |

Two consequences follow:

- Where a consumer needs the guarantee to survive an **arbitrary** binding, the
  answer is the struct idiom (§17.3): make the values one struct, so they are
  one payload on one channel — atomic on every transport.
- Where a binding cannot preserve the grouping, that is a **deploy-time
  constraint**, not a weaker contract. The precedent is the posture constraint
  above — a statically deployed service's control API requires the discovered
  posture — and the owner is the same: rsdl derives the transport and checks
  feasibility against the contract at deploy time (rsdl §8).

Appendix B carries the per-target realization.

#### The generation unit and the addressing unit

A **provided interface is the generation unit** (ADR-0015 decision 11).
Coherence (above), ordinals (§11), and group identity all key on the same
boundary, and that convergence is what makes a store and a dispatcher
generatable from the IR alone. From one provided interface:

- one **store** — its signals, laid out as one coherent block behind a single
  generation counter; each cell seeded with the payload's init value (§4.4),
  carrying provenance and the envelope (§3.1), and evaluated against its own
  `max` staleness bound (§9);
- one **dispatcher** — its events, commands, and queries, routed by the
  interface's single ordinal sequence (§11), with the typl constraints checked
  before the handler and the response bound applied per its per-kind derivation
  (§9.3) — around the reply for a query, around acceptance for a command.

The **service is the addressing unit**: it contributes the global name and the
`service.member` addresses, never the shape of what is generated. The store and
the dispatcher multiply with the interfaces a service publishes, not with the
service — one store and one dispatcher per published interface, under one
logical name.

### 14.6 How components relate to services (forward reference)

Interfaces and services are ridl (the contract SSOT); _providing_ and
_requiring_ them is rsdl. In brief (full treatment in the rsdl reference): a
`component` **provides** a service (implements its members — produces its
signals/events, accepts its commands/queries) and **requires** services or
individual members it consumes. A pure behaviour (`model`, rmdl) knows nothing
of any of this — it is a contract-blind reaction; the component is what binds a
reaction to a service. This keeps the layers clean: ridl declares contracts,
rmdl computes, rsdl connects.

---

## 15. Conventions

- Signals for state, events for occurrences — if you are tempted to publish an
  "event" carrying full current state, it is a signal
- Commands mutate, queries read; observable command outcomes return as state
  (signals), not as return values
- Name signals and fixed values as nouns (`currentSpeed`), events as past-tense
  occurrences (`doorOpened`), commands as imperatives (`setGear`), queries as
  `get…`/`stream…`
- Reuse error types across queries whose failure domains genuinely coincide —
  not one `error enum` per query by reflex; compose several with an
  `error union`
- Prefer `@[min..max]` change-driven signals over strict `@Xms` unless a
  consumer genuinely needs isochronous samples (rmdl clocks do)

---

## 16. Diagnostics

Coded `RIDL-`, grouped by hundreds, same lifecycle rules as typl §16 (codes
never renumbered or reused). The tables below are the `RIDL-` profile codes
only. A `.ridl` file also draws the two namespaces no profile owns — `FORM-`
(surface syntax: lexical, parse, and the attribute-block rules) and `MANI-` (the
manifest). Both are tabulated once in the family overview §7 and are not
restated here.

### 16.1 Timing (RIDL-1xx)

| Code     | Rule                                                                                                                                 | Severity                                                   |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------- |
| RIDL-100 | `signal` or `event` without timing annotation — default `[100ms..1000ms]` (or configured `[defaults].timing`) applied                | warning; error if active profile requires explicit timing  |
| RIDL-101 | `@[X..Y]` where `X > Y`                                                                                                              | error                                                      |
| RIDL-102 | zero or negative duration                                                                                                            | error                                                      |
| RIDL-103 | strict periodic `@Xms` on a kind other than `signal` (§9.2, §9.3) — widened from `event` only by ADR-0015                            | error                                                      |
| RIDL-104 | explicit return type on `command`                                                                                                    | error                                                      |
| RIDL-105 | `query` returning `()`                                                                                                               | error                                                      |
| RIDL-106 | timing annotation on `fixed`, the one kind that carries none (§9); attribute block on `fixed` (§8) — narrowed by ADR-0015            | error                                                      |
| RIDL-107 | type declaration inside an `interface` or a `service` body — raised at parse time, where the declaration is recognised and recovered | error                                                      |
| RIDL-108 | `@[X..X]` — a degenerate range, the rate floor equal to its staleness bound (§9.2); every kind that admits the range                 | warning                                                    |
| RIDL-109 | signal payload type has no derivable init value and no `= value` override (§4.4)                                                     | error                                                      |
| RIDL-110 | signal `= value` init override violates a scalar payload's range, string length bound, or `match` pattern                            | error                                                      |
| RIDL-112 | `command` or `query` with no declared response bound (§9.3) — a bare declaration, or the half-open `@[min..]`; never defaulted       | warning; error if active profile requires a response bound |

**Known gap — RIDL-110.** The check runs only where the payload names a scalar
`type` declaration, and covers exactly the three violations the row names: a
numeric literal (or a constant reference resolving to a numeric value) outside
the declared range, a string literal outside the declared length bound, and a
string literal that does not match the type's `match` pattern. Three cases are
accepted in silence: a literal of the wrong kind (`= true` on an integer-backed
payload), a value off the declared `step` grid, and an override on a `struct`,
`enum`, or `union` payload, which has no scalar bounds to violate. The leniency
is inherited from the typl layer — a struct field's declared init is treated the
same way — so widening it is one change across both, recorded on the
consolidated `debt(E2)` issue (driftsys/ridl#172) rather than closed here in
either direction.

### 16.2 Streams (RIDL-2xx)

| Code     | Rule                                                       | Severity |
| -------- | ---------------------------------------------------------- | -------- |
| RIDL-201 | stream `<T>` on `signal` or `event`                        | error    |
| RIDL-202 | stream element type not a named type, `string`, or `bytes` | error    |

### 16.3 Contracts and Errors (RIDL-3xx)

| Code     | Rule                                                                                                                    | Severity |
| -------- | ----------------------------------------------------------------------------------------------------------------------- | -------- |
| RIDL-301 | `require` or `ensure` on `signal`, `event`, or `fixed`                                                                  | error    |
| RIDL-302 | `ensure` on `command`                                                                                                   | error    |
| RIDL-303 | query with no success path — a bare `error` type in return position, or an `error`-typed left arm of an inline `T \| E` | error    |
| RIDL-304 | `error`-typed or result-union **parameter** on `command`/`query` — failure flowing toward a provider                    | warning  |
| RIDL-305 | `ensure` references no `result`                                                                                         | warning  |
| RIDL-306 | `require`/`ensure` expression outside the guaranteed subset (expr-core specification §8)                                | error    |
| RIDL-307 | contract-error category name (`INVALID_VALUE`, …) declared in an `error` enum                                           | warning  |
| RIDL-308 | named result union in query-return position — the inline `T \| E` spelling is canonical there (§10.1)                   | warning  |

### 16.4 Evolution and Profile (RIDL-4xx)

| Code     | Rule                                                                                                                                                                                                                                                                                                                             | Severity |
| -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- |
| RIDL-401 | interaction re-declared under a `reserved` name                                                                                                                                                                                                                                                                                  | error    |
| RIDL-402 | duplicate interaction name within an interface                                                                                                                                                                                                                                                                                   | error    |
| RIDL-403 | behaviour/user-interaction/architecture declaration in `.ridl` context                                                                                                                                                                                                                                                           | error    |
| RIDL-404 | query named like a mutation — the name begins with `set`, `reset`, `clear`, `apply`, `write`, or `update` followed by an upper-case letter (`setGear`, `resetCounters`)                                                                                                                                                          | warning  |
| RIDL-405 | one `error` type shared across unrelated failure domains — it is the failure arm of queries in 3 or more interaction scopes (heuristic)                                                                                                                                                                                          | info     |
| RIDL-406 | payload field duplicating envelope metadata (§3.1) — a `signal` or `event` payload struct declaring `timestamp`, `time`, `seq`, `seqNo`, `sequence`, `sequenceNumber`, `frameCounter`, or `frameNo`; domain time or a domain counter distinct from transport metadata is legitimate                                              | info     |
| RIDL-407 | interaction ordinal changed against the published baseline (§11) — the desk-time drift check, emitted by `ridl check`, never by `ridlc`                                                                                                                                                                                          | warning  |
| RIDL-140 | duplicate `service` name across the system — the service catalog is a flat global namespace                                                                                                                                                                                                                                      | error    |
| RIDL-141 | `service` names a type that is not an `interface`, and has no inline shape                                                                                                                                                                                                                                                       | error    |
| RIDL-143 | `service` publishes an `internal` interface — a global published address must name a public shape (§14.5)                                                                                                                                                                                                                        | error    |
| RIDL-144 | duplicate member name across a service's interfaces — flat addressing (§14.5) cannot give `service.member` two referents                                                                                                                                                                                                         | error    |
| RIDL-145 | the same interface named twice in one service — its own code, so the mistake is one diagnostic rather than one RIDL-144 per member; the duplicate listing holds no slot                                                                                                                                                          | error    |
| RIDL-146 | interface re-declared under a service-level `reserved` name — RIDL-401 one level up (§14.5, §11)                                                                                                                                                                                                                                 | error    |
| RIDL-147 | two shapes of one service whose interface names collide even though their references differ — a binding separates the ordinal spaces by interface name, so the two would be indistinguishable; the rule is over every shape, live or retired, so a name spelled by two tombstones is the same code (§14.5; ADR-0015 decision 24) | error    |
| RIDL-148 | service-level `reserved` tombstone without an interface name — the literal spelling lowers to a nameless tombstone no shape can ever match, so the sanctioned retirement would silently not work (§14.5; ADR-0015 decision 24)                                                                                                   | error    |
| RIDL-149 | two names in one scope that collide after the pinned name transform — the transform is not injective and no case-folding transform can be, so two names distinct in source can project to one identifier; scoped to the members of one interface and the parameters of one interaction (§11; ADR-0016 decision 3)                | error    |

RIDL-141 and RIDL-143 apply **per shape in the service's shape list** (ADR-0015
decision 18): neither rule changed when composition arrived, only the span each
reports against — the offending list element, not the reference position as a
whole.

(Classifying a reorder, insert, or delete as breaking or compatible is
`ridl-diff`'s jurisdiction, not the compiler's — typl §7.4 discussion applies.
RIDL-407 is the desk-time warning that an ordinal moved at all: it is emitted by
the `ridl check` facade against a workspace-local baseline, which is outside
`ridlc`'s source-to-IR function, and it neither classifies nor gates.)

A public `interface` exposing an `internal` typl declaration is **TYPL-005**
(typl §3.3), not a RIDL code: it is the same rule the vocabulary layer states,
applied to the interaction positions — a signal, event or `fixed` payload, a
command or query parameter, a query return, a tuple-return field, an array or
stream element, either arm of an inline `T | E`, a collection length bound, and
a constant or enum type named by a `require`/`ensure` clause. A `service` is
RIDL-143 instead, because what leaks there is an interface rather than a type
and a service takes no `internal` modifier.

The two clause kinds do not expose the same names, because they do not bind the
same names: a `require` reads the interface's own signals (§13), so a signal
spelled like a package constant shadows it, while an `ensure` reads no signal
and the same spelling is the constant.

### 16.5 Families and Obligations (RIDL-5xx)

| Code     | Rule                                                                                                                                                                        | Severity |
| -------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- |
| RIDL-501 | interaction kind not admitted by its family (§3.2) — an operation on `presentation`, a continuous value on `intent`, an occurrence on `control`, and the rest of that table | error    |
| RIDL-502 | operation shape's parameter rule violated (§3.2.1) — parameters on `activate`/`toggle`/`dismiss`, or not exactly one on `select`/`adjust`                                   | error    |
| RIDL-503 | `intent` operation with no shape — the shape set is closed and has no generic form (§3.2.1)                                                                                 | error    |
| RIDL-504 | non-`dispatch` interaction declaring no correspondence obligation (§3.3) — a family tag with nothing behind it                                                              | warning  |
| RIDL-505 | availability predicate on an `intent` interaction referencing state the consumer cannot evaluate (§3.4)                                                                     | error    |
| RIDL-506 | obligation attribute on a `dispatch` interaction — between software peers the datum is the truth, so there is nothing to correspond to                                      | error    |
| RIDL-507 | `derives` names an interaction in a family that cannot be its reference (§3.3 direction rule)                                                                               | error    |
| RIDL-508 | a `control` interaction with no paired acquisition observing its effect (heuristic, §3.3)                                                                                   | info     |

---

## 17. Open Questions

1. **Selective broadcasts.** Franca's `broadcast selective` (per-client
   delivery) and SOME/IP eventgroups (subscription granularity below the
   interface) have no ridl surface — subscription is currently per-interaction.
   Grouping interactions for subscription efficiency may be an rsdl deployment
   concern rather than a contract concern; needs a worked example.
2. ~~**Interaction-set reuse.**~~ **Answered by ADR-0015 — composition, not
   mixins.** Flat interfaces (§14.1) mean recurring interaction patterns — the
   heartbeat, version and diagnostics triad — used to be duplicated, and the
   candidate recorded here was compile-time flattening mixins
   (`include DiagBlock`). Composition (§14.5: a service carries a
   comma-separated list of interfaces) is the better answer, and the reason is
   the identity model: mixins **flatten**, and ordinals are per-interface (§11),
   so one shared block folded into three interfaces would get three unrelated
   ordinal sets and editing the block would renumber all three. Composing
   `DiagBlock` beside each service's own interface leaves every ordinal space
   intact and independent of what it sits beside. The accepted cost is §14.5's:
   interfaces written to be composed must not collide on member names
   (RIDL-144).
3. ~~**Signal groups / atomic multi-signal updates.**~~ **Closed by ADR-0015 —
   the struct idiom is confirmed.** The provisional answer here — a coherent
   multi-value sample is "make the payload a struct" — holds, and the rule it
   sits inside is now stated in §14.5: the signals of one provided interface are
   published coherently, and whether a consumer observes the grouping depends on
   the binding. A struct payload is one payload on one channel, so it is atomic
   on every transport — the form to use where the guarantee must survive an
   arbitrary binding. Where a binding cannot preserve the interface grouping,
   that is a deploy-time constraint (§14.5), not a weaker contract.
4. **Actions (long-running operations).** ROS 2 actions = goal + feedback +
   result. Composable today as `command` + progress `signal` + completion
   `event`/`query`, and the person boundary needs the same triple for long user
   operations. Decide whether the composition idiom is documented convention or
   deserves sugar.
5. ~~**QoS beyond timing.**~~ **Answered by ADR-0015 — the absorption
   principle.** The earlier wording here — QoS "deliberately absent from the
   contract" — described exclusion, and reading it literally is what produced
   the recurring request for a QoS block. The boundary it drew was correct; the
   description of it was not. The principle is:

   > ridl expresses QoS as **semantic obligation**, never as a transport knob. A
   > binding maps each obligation onto its native QoS; a transport lacking the
   > mechanism either satisfies the obligation by construction or fails at bind
   > time.

   That is what preserves the §1.1 transport-neutrality claim: the same contract
   binds to SOME/IP, proto/gRPC, DBC, AIDL, MQTT, or an in-process broker
   without carrying any of their vocabulary, because it carries none of their
   mechanisms — only the outcomes a consumer may rely on. Mapping the
   contract-bearing DDS policy set against ridl:

   | DDS policy                     | ridl                                                                                                | verdict                         |
   | ------------------------------ | --------------------------------------------------------------------------------------------------- | ------------------------------- |
   | RELIABILITY                    | the interaction kind is the reliability class (§3)                                                  | covered by construction         |
   | DURABILITY (TRANSIENT_LOCAL)   | §4.4 last-value guarantee; events explicitly carry none (§5.1)                                      | covered                         |
   | DEADLINE                       | §9 `max` staleness bound                                                                            | covered                         |
   | LIVELINESS                     | §9 freshness SLO, §10.3 detection                                                                   | covered                         |
   | TIME_BASED_FILTER              | §9 `min` rate floor, derived as debounce                                                            | covered                         |
   | LIFESPAN                       | §9 `max`, derived as event TTL (§5.2)                                                               | covered                         |
   | OWNERSHIP / OWNERSHIP_STRENGTH | §4.2 — exactly one owning provider per flow                                                         | covered by construction         |
   | DESTINATION_ORDER              | §3.1 envelope timestamp and per-channel sequence number                                             | covered                         |
   | RESOURCE_LIMITS (payload)      | typl bounded collections and length bounds                                                          | covered, in the type system     |
   | HISTORY (KEEP_LAST N)          | §4 signal is latest-value; §5.1 rules out event replay                                              | out of scope, deliberately      |
   | DURABILITY (PERSISTENT)        | `persist`, reserved by ADR-0008 decision 3                                                          | reserved                        |
   | LATENCY_BUDGET                 | a delivery-delay hint permitting batching; the DDS specification is explicit it is not a commitment | no analogue needed, none wanted |
   | RPC reply timeout (DDS-RPC)    | `@[..max]` response bound on `command`/`query`                                                      | covered — §9.3 (ADR-0015)       |

   The table is the contract-bearing subset — PARTITION, TRANSPORT_PRIORITY, the
   writer/reader lifecycle policies and the reader-side content filters are
   deployment or reader-local concerns (rsdl). The last row was the one genuine
   gap, and it is not a core DDS QoS policy at all: the reply timeout lives in
   DDS-RPC, and its relatives are gRPC's deadline, SOME/IP's configured timeout,
   and AIDL's transaction timeout. §9.3 closed it.
6. **Mid-stream invalid elements** (§12.4): quarantine-and-continue vs abort —
   per-binding policy or contract-level choice?
7. **Reflection/discovery service.** The spy/control bridge (concept note §9.2)
   implies a generated meta-interface (enumerate interfaces, subscribe by name).
   Specify it as a normative `ridl.reflect` package once the IR spec lands.
8. **Failure management and safety/HA properties** (§10.4). Failsafe states,
   fallbacks, degraded modes, health/halt management — a
   safety/quality-management layer over the runtime's total failure detection.
   Likely split: supervision topology and fallback wiring in rsdl; supervision
   hooks in the `ridl-rt` runtime spec; assurance policy via `@labels` profiles.
   The family may later grow **declarative property surfaces** for this — e.g. a
   signal's failsafe value in ridl, degraded-mode models in rmdl, redundancy
   requirements in rsdl — always properties (what must hold), never mechanisms
   (how). Needs its own document before any keyword lands; start via profile
   vocabulary, promote to syntax only when earned.
9. ~~**uxdl divergence budget.**~~ **Closed by ADR-0012.** The question was
   which rules here are shared with uxdl and which are ridl-only. The answer is
   that none diverge: uxdl is retired, every rule in this document is
   family-neutral, and what a non-dispatch family adds is the four
   correspondence obligations rather than a different rule set. Transports
   remain a binding concern, not a family one.
10. **The explicit one-time read.** The `acquisition` request/response cell is
    reserved (§3.2). What separates it from a continuous acquisition is
    **maintenance**, not duration: a continuous acquisition obliges the provider
    to hold a current value, and forcing an on-request read into it would make
    the binding poll forever for something wanted only occasionally. Periodic
    and on-change acquisition need no new cell — `@[min..max]` already
    distinguishes rate floor from staleness bound (§9.2).
11. **The influence quantity** (VIM). A quantity that does not affect the
    measurand but does affect the relation between indication and result —
    ambient temperature shifting a pressure sensor's characteristic. It is
    neither uncertainty, which is dispersion, nor the relationship, because it
    is what perturbs the relationship. Whether it becomes a fifth obligation or
    a qualifier on the relationship is open (§3.3).

---

## Appendix A — Full Example

```ridl
package veh.cluster

import veh.common.Speed
import veh.common.Temperature
import veh.common.MAX_SPEED
import veh.common.GearPosition
import veh.common.WarningFlags

// --- vocabulary local to this contract (typl declarations, package level) ---

struct DoorPayload {
  sensorId : integer [0..15]
  isOpen   : boolean
}

struct DiagFilter {
  severity : integer [0..5]
  category : Label?
}

struct FaultEvent {
  code      : integer [0..65535]
  message   : Message
  timestamp : Timestamp
}

error enum DiagError {           // failure vocabulary — typl §10.1
  FILTER_INVALID   = 0
  STORAGE_BUSY     = 1
  ACCESS_DENIED    = 2
}

struct FaultPage {
  faults : [FaultEvent; 0..64]
}

// --- the contract ---

/**
 * Main vehicle status interface.
 * @labels SIL_B, CAL_2, PRIVATE
 */
interface VehicleStatus {

  /// Current vehicle speed — isochronous, drives rmdl clocks downstream
  signal currentSpeed : Speed @10ms

  /// Engine temperature — change-driven, 100ms freshness SLO
  signal engineTemp : Temperature @[20ms..100ms]

  /// Active warnings — last-value delivered on subscribe (§4.4)
  signal warnings : WarningFlags @[50ms..1s]

  /// Raised on every door state change; stale after 500ms
  event doorOpened : DoorPayload @[50ms..500ms]

  /// Request a gear change — outcome observed via currentGear, not returned;
  /// callers throttle to one request per 20ms, acceptance within 200ms (§9)
  command setGear(position: GearPosition) [
    require position != GearPosition.PARK || currentSpeed == 0.0
  ] @[20ms..200ms]

  reserved resetCounters          // retired ordinal — never reused

  /// Sliding-window average — replied within 50ms (§9)
  query getAverageSpeed(window: Duration): Speed [
    require window > 0ms
    ensure  result >= 0.0
  ] @[10ms..50ms]

  /// Fault history as a finite stream
  query streamFaults(filter: DiagFilter): <FaultEvent> @[..1s]

  /// Paged fault snapshot — fallible query via inline `T | E` (§10.1)
  query getFaultPage(filter: DiagFilter): FaultPage | DiagError @[..100ms]

  fixed softwareVersion : Version
  fixed capabilities    : [Label; 0..32]
}
```

---

## Appendix B — Codegen Targets

Interaction mapping per target. Type/width mapping is typl Appendix D; the two
compose.

| ridl                             | SOME/IP                                                                      | proto3 / gRPC                                                  | AIDL                                                | DDS                                                   | MQTT / AsyncAPI               |
| -------------------------------- | ---------------------------------------------------------------------------- | -------------------------------------------------------------- | --------------------------------------------------- | ----------------------------------------------------- | ----------------------------- |
| `signal`                         | field notifier (+ auto-derived getter from last-value cache)                 | server-streaming RPC or pub/sub sidecar                        | callback / `oneway` listener                        | topic, `TRANSIENT_LOCAL` durability, DEADLINE = `max` | retained message on channel   |
| coherent-set delivery (§14.5)    | per-field only — no grouping across notifiers                                | per-message only — no grouping across streams                  | per-parcel only — one callback per signal           | GROUP-scope PRESENTATION with `coherent_access`       | per-message only, per channel |
| `event`                          | event (eventgroup)                                                           | server-streaming RPC                                           | callback                                            | topic, `VOLATILE` durability                          | non-retained publish          |
| `command`                        | request w/ empty response (= ack, §6.1)                                      | unary RPC → `Empty` (= ack)                                    | `oneway` + runtime ack shim                         | reliable-QoS request topic (DDS ack)                  | publish QoS 1 (puback = ack)  |
| `query`                          | request/response method                                                      | unary/streaming RPC                                            | method                                              | request/reply (RPC over DDS)                          | request/reply channel pair    |
| RPC response bound, `max` (§9.3) | derived request/response timeout, supervised by the binding                  | server-side deadline; per-call client override stays Stratum 3 | transaction-timeout supervision in the runtime shim | DDS-RPC reply timeout                                 | reply-channel timeout         |
| RPC call throttle, `min` (§9.3)  | admission-side rate limit at the provider                                    | server-side admission rate limit                               | admission check in the runtime shim                 | requester-side rate limit                             | inbound publish rate limit    |
| `fixed`                          | field with getter only                                                       | unary getter RPC (cacheable)                                   | constant/property                                   | —                                                     | retained provisioning channel |
| result-union error arm (§10.1)   | method return code table                                                     | `google.rpc.Status` + typed detail                             | `ServiceSpecificException` code                     | reply union arm                                       | error payload schema          |
| ordinals (§11)                   | method ID = ordinal; event ID = ordinal + event flag; eventgroup = interface | RPC name (identity is nominal)                                 | transaction code = ordinal                          | topic name suffix                                     | channel path segment          |
| Stratum 2 (§10.2)                | `E_MALFORMED_MESSAGE` / `E_NOT_OK` / `E_UNKNOWN_METHOD`                      | `INVALID_ARGUMENT` / `FAILED_PRECONDITION` / `UNIMPLEMENTED`   | `IllegalArgumentException` mapping                  | reply status                                          | error topic convention        |

**Notes.** The command **delivery acknowledgment** (§6.1) is realised with each
transport's cheapest confirmed primitive, as shown in the command row — where a
transport's fire-and-forget primitive has no confirmation (SOME/IP fire&forget,
AIDL `oneway`), the binding uses the confirmed variant or adds a runtime shim;
the ack never surfaces in generated application APIs as a return value. The §4.4
last-value guarantee is what makes the SOME/IP _getter_ derivable — a ridl
signal generates the full SOME/IP field triple minus setter (setters are
explicit `command`s, by design). DBC/CAN binds signals only
(`event`/`command`/`query` do not exist on classic CAN — profile error), and a
coherent set survives only within one frame (§14.5); a set that must stay
simultaneous on every target is one struct payload (§17.3). The WASM component
target maps `query` streams to WIT `stream<T>` (native in WASI 0.3) — the
rmdl-behind-an-interface story stays whole in the browser.

---

## Appendix C — Formal Grammar (EBNF)

The ridl profile adds to the typl grammar (typl Appendix E — `definition` gains
`interface_def` and `service_def`; everything else inherited):

```ebnf
definition    = [ "internal" ] ( typl_definition | interface_def )
              | service_def ;                        (* a service takes no `internal` — §14.5 *)

interface_def = doc_comment? "interface" CamelCase_id "{" { interaction sep? } "}" ;

service_def   = doc_comment? "service" dotted_name
                ( ":" service_shape { "," service_shape } [ "," ]
                                                     (* the shape list — §14.5; commas are
                                                        REQUIRED between shapes, the one
                                                        non-optional separator in the family
                                                        (ADR-0015 decision 13): the list has
                                                        no closing token, so it ends where
                                                        the next declaration begins *)
                | "{" { interaction sep? } "}" ) ;   (* one inline shape — §14.5; never both
                                                        forms (ADR-0015 decision 14) *)
service_shape = type_ref | service_reserved ;       (* a composed interface, or a
                                                       service-level tombstone — §14.5, §11 *)
service_reserved = "reserved" CamelCase_id ;        (* the tombstone spells the retired
                                                       interface's name; typl's literal
                                                       `reserved` spellings derive no name
                                                       and are RIDL-148 here — §14.5 *)
dotted_name   = camelCase_id { "." camelCase_id } ; (* reverse-domain global name,
                                                       every segment lowercase — §14.5 *)

interaction   = signal_def | event_def | command_def | query_def | fixed_def | reserved ;

signal_def    = doc_comment? "signal"  camelCase_id ":" type_ref init_value? timing?
                attr_block? ;
event_def     = doc_comment? "event"   camelCase_id ":" type_ref timing? attr_block? ;
              (* untimed → configurable default [100ms..1000ms], §9.1 *)

              (* There is no family clause. The family (§3.2) is selected by the
                 declaring keyword, and `.ridl` spells only `dispatch` — so every
                 production here is a dispatch interaction. The non-dispatch
                 spellings, and the operation shapes, are the rxdl reference's;
                 they lower to these same kinds carrying a family and a shape.
                 The rules they can violate are still this document's, reported
                 as RIDL-501 through RIDL-508 (§16.5). *)

init_value    = "=" ( literal | SCREAMING_SNAKE_ID ) ;   (* bare init override — §4.4 *)
command_def   = doc_comment? "command" camelCase_id "(" param_list ")" attr_block? timing? ;
query_def     = doc_comment? "query"   camelCase_id "(" param_list ")" ":" return_type
                attr_block? timing? ;
              (* range form only on command and query — a strict period is RIDL-103, §9.3 *)
fixed_def     = doc_comment? "fixed"   camelCase_id ":" fixed_type ;
              (* no error syntax — a fallible_type return makes a query fallible, §10.1 *)

param_list    = "" | param { "," param } ;
param         = camelCase_id ":" param_type ;
param_type    = type_ref | stream_type ;
return_type   = type_ref | tuple_type | fallible_type | stream_type ;
fallible_type = type_ref "|" type_ref ;             (* inline T | E — §10.1, gf §6.1;
                                                       both arms are named types, the
                                                       right one an `error` type *)
fixed_type    = type_ref | array_type ;             (* array_type per typl grammar *)
              (* correspondence obligations (§3.3) are ordinary attributes in the tail
                 `attr_block` — assignment form for tolerance, resolution, lag, span and
                 `derives`; predicate form for `plausible` and `ensure`. They are legal
                 only on a non-dispatch family — RIDL-506 *)
stream_type   = "<" ( type_ref | "string" | "bytes" ) ">" ;

(* ---------- Timing ---------- *)

timing        = "@" duration
              | "@" "[" timing_range "]" ;
timing_range  = duration ".." duration | duration ".." | ".." duration ;
duration      = int_lit ( "us" | "ms" | "s" | "min" | "h" ) ;   (* §2.1 *)

(* ---------- Attribute Block ---------- *)

attr_block    = "[" { attribute sep? } "]" ;
attribute     = "require" expr | "ensure" expr ;
expr          = (* expr-core specification §3.1 — the guaranteed subset; §13 *) ;

(* reserved, sep, type_ref, tuple_type, identifiers, literals: typl Appendix E *)
```

---

## Appendix D — Standards References

Additions over typl Appendix C:

| Reference                                                                                                                                                                        | Used for                                                                    |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| [AUTOSAR Classic R22-11](https://www.autosar.org/)                                                                                                                               | signal/event split (`isQueued`), CalibrationParameter, signal groups        |
| [SOME/IP (AUTOSAR PRS)](https://www.autosar.org/)                                                                                                                                | field/method/event mapping, return codes, IDs                               |
| [Franca IDL](https://franca.sourceforge.net/)                                                                                                                                    | attribute/method/broadcast precedent, error enums, rejected `version` block |
| [OMG DDS 1.4 / DDS-RPC](https://www.omg.org/spec/DDS/)                                                                                                                           | durability, deadline, liveliness — QoS-in-contract comparison               |
| [gRPC / google.rpc.Status](https://grpc.io/)                                                                                                                                     | status model, streaming RPC                                                 |
| [Android AIDL](https://developer.android.com/guide/components/aidl)                                                                                                              | `oneway`, transaction codes                                                 |
| [Fuchsia FIDL](https://fuchsia.dev/fuchsia-src/development/languages/fidl)                                                                                                       | method ordinals (hash model rejected), protocol evolution                   |
| [WASM Component Model / WIT](https://component-model.bytecodealliance.org/design/wit.html)                                                                                       | `stream<T>`/`future<T>` (WASI 0.3), resource model                          |
| [AsyncAPI 3.0](https://www.asyncapi.com/)                                                                                                                                        | channel/operation vocabulary for the MQTT binding                           |
| [ROS 2](https://docs.ros.org/)                                                                                                                                                   | msg/srv/action decomposition                                                |
| [Eclipse uProtocol / COVESA uServices](https://uprotocol.org/)                                                                                                                   | contemporary transport-neutral automotive service layer                     |
| [Eiffel](https://www.eiffel.com/values/design-by-contract/introduction/), [SPARK Ada](https://docs.adacore.com/spark2014-docs/html/ug/en/source/contract_based_programming.html) | Design by Contract                                                          |

---

## Appendix E — Prior Art Survey

What each interface language contributed to, or was rejected from, this design:

| Language                         | Taken / rejected                                                                                                                                                                                                                                                                                                                                                |
| -------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **CORBA IDL**                    | the `interface` grouping and the cautionary tale: `inout` params, exceptions as control flow, and interface inheritance all rejected here as evolution and codegen hazards — data-carrying failures are `error struct`s in result unions instead                                                                                                                |
| **Franca IDL**                   | closest European-automotive ancestor: attributes ≈ signals, broadcasts ≈ events, `fireAndForget` ≈ command, error enums → typl `error` types. Rejected: `extends`, in-language `version`, `selective` broadcasts (→ §17.1), deployment `.fdepl` files (rsdl + `ridl.toml` instead)                                                                              |
| **AUTOSAR Classic**              | the signal/event split (`isQueued`), CalibrationParameter → `fixed`, signal groups → the struct idiom (§14.5). ridl exists partly to be the legible source ARXML is generated _from_                                                                                                                                                                            |
| **SOME/IP**                      | field triple (getter derivable from §4.4, setter as explicit command, notifier = signal), return codes → Stratum 2 mapping, method/event IDs → ordinal derivation                                                                                                                                                                                               |
| **DDS**                          | the QoS lesson: DEADLINE/durability/liveliness are _contract-relevant_ — ridl promotes exactly the state-vs-occurrence and freshness subset into the language (`@`, §4.4) and leaves reliability/history to deployment (§17.5). DDS proves both that QoS matters and that 22 orthogonal QoS policies on one topic is too many degrees of freedom for a contract |
| **gRPC / proto**                 | streaming model (§12 direction-by-position), `google.rpc.Status` → Stratum 2 categories, `Empty`-returning unary → command mapping. Rejected: errors as open-ended status strings — ridl functional errors are closed typl types, and errors are data, not a status channel                                                                                     |
| **AIDL**                         | `oneway` validates fire-and-forget as a first-class kind; transaction-code stability → ordinals                                                                                                                                                                                                                                                                 |
| **FIDL**                         | strict/flexible interaction evolution and hashed method ordinals studied; hashing rejected (renames silently break wire, IDs unreadable from source) in favour of positional ordinals + `reserved`                                                                                                                                                              |
| **WIT / component model**        | `stream<T>`/`future<T>` (now native in WASI 0.3) confirm the stream container at the interface boundary; the WASM backend maps ridl streams onto them directly                                                                                                                                                                                                  |
| **AsyncAPI**                     | naming for the MQTT/broker binding; validates "schema language + interaction language as siblings over one type vocabulary" (its schemas are JSON Schema — structurally the typl/ridl split)                                                                                                                                                                    |
| **ROS 2**                        | msg/srv ≈ signal/query; **actions** (goal+feedback+result) recorded as the composition idiom → §17.4                                                                                                                                                                                                                                                            |
| **OPC-UA**                       | industrial precedent for typed nodes with subscriptions; its reference model is heavier than a contract language needs — profiles/labels carry the assurance metadata instead                                                                                                                                                                                   |
| **MQTT**                         | retained messages are the §4.4 last-value guarantee in the wild; Sparkplug's birth/death certificates prefigure provider-activation semantics (initial publish, §4.4)                                                                                                                                                                                           |
| **uProtocol / COVESA uServices** | the contemporary validation: transport-neutral pub/sub+RPC over SOME/IP·Zenoh·MQTT with proto-defined services. ridl differs by owning the vocabulary layer (typl ranges/units vs bare proto types), timing in the contract, and contracts/behaviour above it                                                                                                   |

---

## Appendix F — Coverage Analysis: Other Interface Languages

Method as in typl Appendix G: can ridl express what a contract author reaches
for in each major interface language? ✓ covered, ≈ covered differently (usually
stricter), ✗ not expressible (deliberate or open).

| Foreign construct                                                | ridl equivalent                                                                              | Status                                                      |
| ---------------------------------------------------------------- | -------------------------------------------------------------------------------------------- | ----------------------------------------------------------- |
| CORBA/Franca **attribute** (readable, subscribable state)        | `signal` + §4.4 last-value                                                                   | ✓                                                           |
| attribute **setter**                                             | explicit `command` (never implicit)                                                          | ≈ deliberate — mutation is always a visible verb            |
| Franca **broadcast**                                             | `event`                                                                                      | ✓                                                           |
| Franca **selective broadcast** (per-client)                      | —                                                                                            | ✗ open §17.1                                                |
| Franca/AIDL **fireAndForget / oneway**                           | `command`                                                                                    | ✓ first-class                                               |
| method with **error** (Franca error enum, SOME/IP return code)   | result-union return; error arm → native error channel                                        | ✓                                                           |
| **exception hierarchies with payloads** (CORBA, Java)            | `error struct` / `error union` arms in result unions                                         | ≈ deliberate — closed sets, no hierarchies, errors are data |
| gRPC **unary / server-stream / client-stream / bidi**            | `query T` / `query : <T>` / `command(<T>)` or `query(<T>)` / `query(<T>): <U>`               | ✓                                                           |
| gRPC **deadline**                                                | `@[..max]` response bound on `command`/`query` (§9.3); the per-call override stays Stratum 3 | ✓ in-contract (ADR-0015)                                    |
| DDS **DEADLINE QoS**                                             | `@[..max]` refresh ceiling / freshness SLO                                                   | ✓ in-contract                                               |
| DDS **durability (TRANSIENT_LOCAL)**                             | §4.4 last-value guarantee (signals)                                                          | ✓ normative, not tunable                                    |
| DDS **reliability / history depth**                              | — (transport/rsdl)                                                                           | ✗ deliberate §17.5                                          |
| DDS **liveliness**                                               | staleness via freshness bounds + observability                                               | ≈                                                           |
| SOME/IP **field get/set/notify**                                 | signal (get derived, notify native) + command (set)                                          | ✓ decomposed                                                |
| SOME/IP **eventgroups**                                          | interface-level subscription granularity                                                     | ≈ coarser; §17.1                                            |
| AUTOSAR **signal groups** (atomic sample)                        | struct payload on one signal                                                                 | ≈ idiom, confirmed (§14.5, ADR-0015)                        |
| AUTOSAR **client/server + sender/receiver ports**                | query/command + signal/event                                                                 | ✓                                                           |
| AIDL **in/out/inout parameters**                                 | parameters in, tuple returns out                                                             | ≈ deliberate — no out-params                                |
| FIDL **events** (server-initiated on protocol)                   | `event`                                                                                      | ✓                                                           |
| FIDL **strict/flexible evolution**                               | ordinals + `reserved` + `ridl-diff` categories                                               | ≈                                                           |
| ROS 2 **action** (goal/feedback/result)                          | command + progress signal + result query/event                                               | ≈ composition idiom §17.4                                   |
| WIT **resource** (capability handle with methods)                | —                                                                                            | ✗ open — relevant when rmdl/WASM host interfaces mature     |
| WIT **future / stream**                                          | `query` return / `<T>`                                                                       | ✓                                                           |
| AsyncAPI **channel + operation**                                 | interaction + transport binding                                                              | ✓                                                           |
| MQTT **retained message**                                        | §4.4                                                                                         | ✓                                                           |
| OPC-UA **historical access**                                     | — (test/observability plane, not contract)                                                   | ≈ relocated                                                 |
| **service discovery** (SOME/IP-SD, DDS discovery)                | rsdl topology + reflection service §17.7                                                     | ≈ relocated                                                 |
| **interface versioning** (Franca `version`, SOME/IP major/minor) | package version + `ridl-diff`                                                                | ≈ relocated, mechanical                                     |

**Verdict.** ridl covers the working set of every surveyed interface language
through five kinds plus two orthogonal clauses (timing, contracts) — where the
others reach for per-feature keywords (field triples, selective broadcasts,
exception clauses, QoS matrices), ridl decomposes into the same five primitives,
expresses failure as vocabulary, and pushes tunables to deployment. The honest
gaps: selective/per-client delivery (§17.1), WIT-style resource handles, and the
action idiom's lack of sugar — all recorded, none blocking v0.2. What no
surveyed language has that ridl does: timing as first-class contract with
derived freshness SLOs, the three-strata error model with errors-as-data (no
error syntax at all) and derived Stratum 2, typl vocabulary (units/ranges) under
every payload, and one evolution mechanism (ordinals + `reserved` + diff)
uniform from struct fields to interface methods.

---

## Appendix G — Glossary

| Term                                 | Definition                                                                                                                                                                                                                                                   |
| ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **RIDL** (capitals)                  | the platform and family name; **ridl** (lowercase) is this language, the family's interaction layer and flagship member                                                                                                                                      |
| **family**                           | the four languages — typl, ridl, rmdl, rsdl — sharing one grammar, one toolchain, one IR; rxdl is a profile and a spelling layer over ridl, not a fifth language                                                                                             |
| **interaction family**               | who is on the other side of a boundary — `dispatch` · `presentation` · `intent` · `acquisition` · `control` (§3.2). A property of the declaration, a closed enum in the IR                                                                                   |
| **correspondence obligation**        | what a non-dispatch contract must declare about the gap between a datum and its referent: relationship, uncertainty, latency of correspondence, failure to correspond (§3.3)                                                                                 |
| **reference / realisation**          | in a non-dispatch family, the causally upstream and downstream sides; the realisation must correspond to the reference within declared bounds (§3.3)                                                                                                         |
| **operation shape**                  | `activate` · `toggle` · `select` · `adjust` · `dismiss` — a closed set constraining an intent operation's parameters and carrying machine-readable gesture semantics (§3.2.1)                                                                                |
| **indication**                       | metrology's term (VIM) for the value a measuring instrument provides; a presentation-family value is one (§3.3)                                                                                                                                              |
| **acquisition span**                 | the interval over which a value acquired by sweeping corresponds to the world, as against an instant offset (§3.3)                                                                                                                                           |
| **profile**                          | the restriction of the family grammar accepted by a file extension; `.ridl` accepts interactions + typl declarations; `.rxdl` is the total profile accepting every layer                                                                                     |
| **core**                             | a reusable semantic unit beneath the surface languages: `ns` (namespacing), `typl-core` (types), `expr` (predicates), `time` (timing), `interact` (interaction primitives)                                                                                   |
| **interaction**                      | a named, typed, directed exchange on a contract boundary; ridl defines five kinds                                                                                                                                                                            |
| **signal**                           | pub/sub interaction carrying a continuous **state** value — latest sample matters, intermediate samples may be missed                                                                                                                                        |
| **event**                            | pub/sub interaction carrying a discrete **occurrence** — every occurrence matters, queued not coalesced                                                                                                                                                      |
| **command**                          | fire-and-forget RPC at the contract level — no functional reply; the runtime carries a delivery acknowledgment beneath it                                                                                                                                    |
| **delivery acknowledgment (ack)**    | runtime-level confirmation that a command was received and accepted (validated, precondition passed) — or negatively acknowledged with a Stratum 2 category; enables retries and supervision, never visible in the contract                                  |
| **query**                            | request/response RPC — reply mandatory; an inline `T \| E` return makes it fallible                                                                                                                                                                          |
| **fixed**                            | a value provisioned externally (build/factory/FOTA), immutable for the software-instance lifetime, safe to cache                                                                                                                                             |
| **provider**                         | the component that owns an interface: publishes its signals/events, executes its commands/queries                                                                                                                                                            |
| **consumer**                         | any component bound to an interface it does not own: subscribes, calls                                                                                                                                                                                       |
| **state vs occurrence**              | the load-bearing distinction behind signal/event: state exists while unchanged and may be cached; an occurrence happens once and is meaningful individually                                                                                                  |
| **timing annotation**                | the `@` clause making publication timing part of the contract: `@Xms` strict periodic or `@[min..max]`                                                                                                                                                       |
| **rate floor / staleness bound**     | the one generic meaning of `min` / `max` on any timing range (§9): minimum interval between publications / maximum age on envelope timestamps before the value or occurrence is stale                                                                        |
| **debounce / refresh ceiling**       | the signal _derivation_ of rate floor / staleness bound (§9): coalesce updates faster than `min` / re-publish at least every `max` even unchanged                                                                                                            |
| **throttle / TTL**                   | the event _derivation_ of rate floor / staleness bound (§9): the provider must not raise faster than `min` / an occurrence older than `max` is discarded                                                                                                     |
| **default timing**                   | the compile-time-configurable range (`[100ms..1000ms]`, `ridl.toml [defaults].timing`) applied to untimed signals and events; always resolved to concrete bounds in the IR                                                                                   |
| **freshness SLO**                    | the alertable staleness bound derived from a signal's timing — the contract's definition of "late"                                                                                                                                                           |
| **last-value guarantee**             | §4.4: a signal channel is never empty — subscribing delivers a value immediately (init before first publication, latest published value after); the normative demand behind broker caches, MQTT retained, DDS `TRANSIENT_LOCAL`                              |
| **late joiner**                      | a subscriber that binds after publication began; served by the last-value guarantee on signals, receives nothing retroactive on events                                                                                                                       |
| **quarantine**                       | binding behaviour for an invalid _stream element_ (§12.4): withheld from application code, observability recorded. Signals do not quarantine — invalidity propagates as channel state (§4.5)                                                                 |
| **functional error (Stratum 1)**     | a domain-level failure expressed as data — the error arm of a fallible query's return; the provider _answered_: no                                                                                                                                           |
| **contract error (Stratum 2)**       | an implicit, standardized violation derived from the contract itself: `INVALID_VALUE`, `PRECONDITION_FAILED`, `CONTRACT_BROKEN`, `UNKNOWN_INTERACTION` — never declared, never an error-type value                                                           |
| **transport error (Stratum 3)**      | **infrastructure failure — detected, undeclared** (general form §6.4): a timeout, broker loss or reset, observed by the runtime and carried by runtime types, with no vocabulary in the contract language                                                    |
| **error type**                       | a typl `enum`/`struct`/`union` carrying the `error` modifier — failure vocabulary, ordinary data (typl §10.1)                                                                                                                                                |
| **fallible query / inline `T \| E`** | a query whose return names a success type and one error type — the family's `Result<T, E>`, written at the signature (§10.1, general form §6.1); bindings map the error arm to native transport error channels, exhaustively handled in every codegen target |
| **result union**                     | the two-arm named union (one success arm, one error arm) of typl §10.2 — legal data anywhere, but in query-return position it draws RIDL-308 steering to the inline spelling                                                                                 |
| **`require` / `ensure`**             | pre-/postcondition contract clauses (expr core); violations are Stratum 2, and each assertion also runs as CI property test, online observer, and rmdl oracle check                                                                                          |
| **stream**                           | the `<T>` container: an unbounded element sequence, valid only in interaction position; direction determined by position (parameter = consumer produces, return = provider produces)                                                                         |
| **ordinal**                          | an interaction's implicit 1-based declaration-order identity within its interface; source of transport IDs (typl §7.4 model)                                                                                                                                 |
| **`reserved`**                       | tombstone keeping a retired interaction's ordinal slot occupied so wire identities are never reused                                                                                                                                                          |
| **append-only**                      | the evolution discipline implied by ordinals: new interactions at the end, deletions by tombstone, reorder = wire break                                                                                                                                      |
| **interface**                        | the abstract contract _shape_ — a reusable, identity-less group of interactions; a contract type, realized by services (`interface : service :: type : instance`)                                                                                            |
| **service**                          | a global, named, published declaration of an interface — the SSOT catalog entry, addressed `service.member`; posture-neutral (deploys static or discovered); what components provide                                                                         |
| **service catalog**                  | the flat global namespace of all `service` declarations — the system-wide SSOT of contracts                                                                                                                                                                  |
| **posture**                          | how a service is realized on the wire — static (bus signals/events, Classic) or discovered (SOME/IP/DDS/uProtocol, Adaptive); chosen at deployment, not in the contract                                                                                      |
| **binding**                          | generated per-transport code realising a contract: validation, caching, error mapping, (de)serialization                                                                                                                                                     |
| **envelope**                         | runtime-supplied metadata on every interaction instance — timestamp + per-channel sequence number — never declared, never in payloads; powers timing evaluation, dedup, loss detection, E2E counters, and replay (§3.1)                                      |
| **system time**                      | the platform's one synchronized time base (gPTP/PTP or shared realtime clock) — an assumed platform property; envelope timestamps live in it and are comparable system-wide (§3.1)                                                                           |
| **epoch (platform)**                 | 1970-01-01 00:00:00 TAI (the PTP epoch); platform time = `int64` microseconds since it — continuous, leap-second-free; civil datetime is presentation only                                                                                                   |
| **init value**                       | the value a signal channel holds before the provider's first publication — the payload type's init (typl §5.8) or the signal's bare `= value` override (§4.4); no keyword — `init` is rmdl's alone                                                           |
| **invalid state**                    | the propagated channel state entered when a received payload violates typl constraints — visible to all subscribers with last-good value retained; realised as SNA sentinels on CAN (§4.5)                                                                   |
| **provenance (channel)**             | the subscriber-visible origin of a signal's current value: `init` / `live` / `invalid`                                                                                                                                                                       |
| **broker**                           | the asynchronous message plane between components; ridl contracts cross it, rmdl models never do (the sync/async wall)                                                                                                                                       |
| **IR**                               | the stable serialized intermediate representation — resolved names, types, timings, ordinals — consumed by every backend and tool                                                                                                                            |
| **`ridl-diff`**                      | the IR-comparison tool classifying contract changes as breaking/compatible; plumbing-grade CI gate enforcing the evolution rules                                                                                                                             |
| **profile (assurance)**              | an external plug-in validating `@labels` vocabulary and escalating optional rules (explicit timing, explicit bounds) to errors — distinct from _grammar_ profile                                                                                             |
| **SSOT**                             | single source of truth — the design goal: one contract file from which bindings, docs, tests, and topologies derive                                                                                                                                          |

---

_End of ridl Language Reference v0.2.0 — Draft._
