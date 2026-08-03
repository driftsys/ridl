# rxdl Language Reference

**The unrestricted profile and the domain spellings** — the layer of the RIDL
family that lifts restrictions rather than adding semantics. A `.rxdl` file may
carry any layer and any interaction domain; the domain spellings give the person
and world boundaries readable keywords over ridl's kinds.

Version: 0.1.0 — Draft

> **Provenance.** rxdl replaces uxdl. ADR-0012 retired uxdl as a family member
> on the finding that its content was ridl's and its shape was not: at every
> boundary with the non-software world the datum and its referent come apart,
> and the obligations that follow are the **same four at every boundary**, so
> they belong in one place. That place is ridl. What is left over — readable
> words for the combinations ridl already understands, and a file kind that
> forbids nothing — is this document. Its precursor lineage runs uxil → uxdl →
> rxdl; uxil's eleven interaction verbs and its corpus-registry discipline are
> mined in Appendix B.
>
> **This document is deliberately thin, and that is the point.** ADR-0012
> decision 7 gives an extension **no grammar, no IR nodes, and no semantics.**
> Everything normative about families, obligations, availability, timing,
> evolution, and errors lives in the ridl reference and is cited, never
> restated. uxdl's defect was a document that was mostly pointers while claiming
> to be a language; rxdl claims to be a spelling table, so pointers are the
> correct content. **If a rule appears here that is not in ridl, it is a bug in
> this document.**
>
> **Specified, not built.** No compiler in this repository accepts rxdl. This
> reference lands as normative with epic E7a; ridl's boundary-model core (E3) is
> its precondition. See `docs/ROADMAP.md`.

---

## Table of Contents

1. [What rxdl Is](#1-what-rxdl-is)
2. [The Unrestricted Profile](#2-the-unrestricted-profile)
3. [Domains and Families](#3-domains-and-families)
4. [The hmi Domain](#4-the-hmi-domain)
5. [The env Domain](#5-the-env-domain)
6. [Lowering](#6-lowering)
7. [Admission Tests](#7-admission-tests)
8. [What rxdl Is Not](#8-what-rxdl-is-not)
9. [Package Tightening](#9-package-tightening)
10. [Diagnostics](#10-diagnostics)
11. [Open Questions](#11-open-questions)

- [Appendix A — Worked Example](#appendix-a--worked-example)
- [Appendix B — Prior Art: uxil and uxdl](#appendix-b--prior-art-uxil-and-uxdl)

---

## 1. What rxdl Is

Two things, and nothing else:

1. **A file kind that restricts nothing** (§2). Every other profile is a
   restriction — `.typl` accepts type declarations, `.ridl` accepts
   dispatch-family interactions, `.rmdl` accepts behaviour. `.rxdl` accepts all
   of it.
2. **Spelling tables for the non-dispatch families** (§4, §5). `present` is not
   a new kind; it is a readable way to write the combination ridl already
   describes as a continuous state value at the person boundary.

The `x` reads both ways at once — the wildcard over the family pattern (r·i·dl,
r·m·dl, r·s·dl) and _extended_. One rule covers both: **`.rxdl` is the profile
with no restrictions.**

## 2. The Unrestricted Profile

| Extension | Accepts                                   |
| --------- | ----------------------------------------- |
| `.typl`   | type declarations                         |
| `.ridl`   | dispatch-family interactions, over typl   |
| `.rmdl`   | behaviour declarations                    |
| `.rsdl`   | architecture declarations                 |
| `.rxdl`   | **any layer, and any interaction domain** |

Two restrictions are lifted, independently:

- **the layer restriction** — types, interfaces, behaviour, and wiring may share
  one file. This is the adoption gradient: a demo, a getting-started guide, or a
  solo developer writes one file (concept note §4).
- **the domain restriction** — the person and world spellings of §4 and §5 are
  available. In a `.ridl` file only the dispatch family is spelled.

**Consequence, recorded.** Because a production package that uses the hmi
spellings must be `.rxdl`, the file extension no longer answers _"which files
contain executable behaviour?"_ on its own. Package tightening (§9) is the
mitigation, and it is where that question is answered.

## 3. Domains and Families

A **family** is a property of a declaration and is ridl's (ADR-0012 decision 2).
A **domain** is a packaging unit: which spellings a file may use, and which
backends consume them. Domains do not appear in the IR — code generation selects
on `family`.

| Domain | Families                  |
| ------ | ------------------------- |
| `hmi`  | presentation · intent     |
| `env`  | acquisition · control     |
| —      | dispatch (always spelled) |

The four correspondence obligations each family carries, what they mean, and how
they compose along a path are **ridl's**, not restated here.

## 4. The hmi Domain

The person boundary. **presentation** is what the system offers to be perceived;
**intent** is what the person means and the system captures.

| Family       | continuous | occurrence | operation (no return)                           | operation (returns) |
| ------------ | ---------- | ---------- | ----------------------------------------------- | ------------------- |
| presentation | `present`  | `notify`   | —                                               | —                   |
| intent       | —          | _(open)_   | `activate` `toggle` `select` `adjust` `dismiss` | —                   |

```ridl
present  clusterSpeed: IndicatedSpeed @[50ms..200ms]
present  nowPlaying: TrackInfo
notify   overspeedChime: Alert
activate playButton
toggle   muteButton
select   trackItem(id: TrackId)
adjust   volumeSlider(level: Ratio)
dismiss  errorBanner
```

Payload rules on the operation shapes — none for `activate`, `toggle`, and
`dismiss`; exactly one key for `select`; exactly one ranged value for `adjust` —
are ridl's, and are what earns those five their keywords under §7.

**The set is closed and has no generic.** A person performs one act at a time; a
multi-parameter operation is a function call, not a gesture. Sending a message
is three interactions, not one. Drag-and-drop, stripped of modality, is `select`
then `adjust`.

**The empty cells are consequences, not gaps.** Nothing can be invoked on an
agent, so presentation has no operations. A person's continuous state is
knowable only by measuring a physical proxy, so a pedal is `measure` (§5), not
an intent spelling. A system cannot block on a human, so intent has no query:
_"are you sure?"_ is `present` plus `activate`/`dismiss`.

## 5. The env Domain

The physical boundary. **acquisition** is what the system takes from the world;
**control** is what the system causes in it.

| Family      | continuous | occurrence | operation (no return) | operation (returns) |
| ----------- | ---------- | ---------- | --------------------- | ------------------- |
| acquisition | `measure`  | `detect`   | —                     | _(open)_            |
| control     | `actuate`  | —          | `trigger`             | —                   |

```ridl
measure wheelSpeed: Speed @10ms
measure pedalPosition: Ratio @20ms
detect  wheelSlip: SlipEvent
actuate targetAngle: Angle
trigger airbagDeploy(zone: Zone)
```

Note `pedalPosition`. A pedal is measured, not intended: there is no
interpretation gap between a foot's position and its meaning, and all four
obligations instantiate exactly as they do for a wheel-speed sensor. **A person
appears at both boundaries** — as an agent that means things (hmi) and as a
physical object that is measured (env) — and which one applies is decided by
whether the value carries meaning or magnitude.

**The empty cells, again as consequences.** Commanding a device to recalibrate
is system → world, so it is control, not acquisition. Nothing leaves the system
uninvited, so control has no occurrence. Effect is observed by measuring back
and never returned, so control has no query.

## 6. Lowering

Every spelling resolves to a triple ridl already understands. The mapping is
**bijective**.

| Spelling               | kind    | family       | shape  |
| ---------------------- | ------- | ------------ | ------ |
| `signal`               | signal  | dispatch     | —      |
| `event`                | event   | dispatch     | —      |
| `command`              | command | dispatch     | —      |
| `query`                | query   | dispatch     | —      |
| `present`              | signal  | presentation | —      |
| `notify`               | event   | presentation | —      |
| `activate` … `dismiss` | command | intent       | _each_ |
| `measure`              | signal  | acquisition  | —      |
| `detect`               | event   | acquisition  | —      |
| `actuate`              | signal  | control      | —      |
| `trigger`              | command | control      | —      |

`fixed` is family-neutral: a provisioned constant carries no correspondence at
any boundary.

Three properties follow from bijectivity, and each is load-bearing:

- **Round-tripping.** `ridl fmt`, IR rendering, and diagnostics reproduce the
  authored spelling. A `present` never renders back as a `signal`.
- **One way to say it.** The family is an IR field, not an attribute, so
  `[ presentation ]` is not an alternative spelling anyone can hand-write.
- **No drift.** There is no desugaring that could change between releases and
  make a stored diff baseline disagree with a fresh snapshot on identical
  source.

`(command, intent, no shape)` is not a valid triple — §4's closed-set rule
expressed structurally rather than as a lint.

## 7. Admission Tests

Two tests bind every future spelling. Both are ADR-0012 decision 4.

**Modality independence.** If two realisations afford the same capability they
must produce the same declaration. A word naming _how_ is rejected. This is what
rules out `scroll` and `drag` (gestures — one of several ways to express "show
me more" or "put this elsewhere"), `scan` (a sensing mechanism — a lidar scans,
a staring infrared array does not, and both measure the same quantity), and
`display` (visual — a speaker does not display, and a haptic seat has no view).

**Constraint bundling.** A spelling is warranted if and only if it bundles
constraints that would otherwise be re-declared by hand at every use. A word
adding nothing over its core kind is rejected — which is precisely the test uxdl
failed.

## 8. What rxdl Is Not

- **Not a language.** No grammar of its own. A `.rxdl` file is parsed by the one
  family grammar, with no profile restriction applied.
- **Not semantics.** Families, the four obligations, availability and its five
  sources, timing, init and invalid channels, errors, ordinals and evolution are
  all ridl's. This document defines none of them and may not.
- **Not IR.** `family` and `shape` are ridl's IR fields. rxdl adds no node, no
  field, and no attribute key.
- **Not layout, styling, navigation, or localisation.** A `toggle` may scaffold
  a switch; nothing here says switch, checkbox, or voice command. Screen flow is
  a graph over interactions and has no home yet (§11).
- **Not a second evolution model.** `ridl-diff` classifies a lowered declaration
  by its core kind, so a spelling cannot weaken the gate.

## 9. Package Tightening

`.rxdl` forbids nothing, so strictness is a package decision, declared in
`ridl.toml` (ADR-0002's profile-purity mechanism, not a new one). A production
package narrows what its files may contain — permitted layers, permitted domains
— and the compiler enforces the narrowing.

This is where _"which files contain executable behaviour?"_ is answered once
`.rxdl` is in production use (§2).

## 10. Diagnostics

**No `RXDL-` code family exists, and none should.** Every rule a spelling can
violate is a ridl rule, reported under `RIDL-`: a payload rule on `select`, an
invalid (kind, family, shape) triple, an availability predicate a consumer
cannot evaluate, an obligation missing where its family requires one.

Two diagnostics are genuinely rxdl's, and both concern the profile rather than
the spellings — a domain spelling used in a file whose profile does not permit
it, and a layer used in a package that has tightened against it. Their codes are
allocated with E7a.

## 11. Open Questions

1. **The intent occurrence keyword.** The person-supplies-content spelling —
   typing, scanning, dictation. Event-shaped: debounce, TTL, no acknowledgment.
   `supply` is the working candidate; `provide` and `tell` remain live. `enter`
   was rejected for modality, `submit` for implying a commit and an
   acknowledgment an occurrence does not have.
2. **The acquisition/query cell.** A polled or diagnostic sensor read. Possibly
   `measure` under different timing, possibly `query` crossing a non-software
   boundary unchanged, possibly its own word.
3. **Interaction citation paths.** The projection from a declaration to the
   stable identifier that specifications, journeys, tests, and telemetry cite —
   uxil's founding problem. **Not rxdl's**: the observability semantic
   conventions need it for every interaction, so it is ridl's, and it is
   recorded here only because uxil's readers will look for it.
4. **Journeys and screen flow.** A sequence of interactions across surfaces.
   Neither ridl nor rsdl expresses a graph over interfaces. Likely a
   requirements-and-traceability artifact rather than a language.
5. **Accessibility metadata.** Roles, labels, reading order. The operation
   shapes already carry the machine-readable gesture semantics such tooling
   consumes; whether more is needed is unproven.

---

## Appendix A — Worked Example

```ridl
package veh.cockpit.cluster

import veh.common.Speed
import veh.common.Angle
import veh.common.Ratio

// --- vocabulary (typl) ---

type IndicatedSpeed: km/h [0.0..250.0 step 1.0]

// --- one interface, four families ---

interface Cluster {

  /// The sensor's reading of the world.
  measure wheelSpeed: Speed @10ms

  /// What the driver reads. Legally may never indicate below true speed.
  present clusterSpeed: IndicatedSpeed @[50ms..200ms]

  /// The driver's foot — measured, not intended: position is meaning.
  measure pedalPosition: Ratio @20ms

  /// Audible warning; momentary, no init value.
  notify overspeedChime: Alert

  /// The setpoint the steering actuator tracks.
  actuate targetAngle: Angle

  /// The driver's acts.
  activate resetTrip
  toggle   headlamps
  adjust   brightness(level: Ratio)
}
```

Obligation attributes are omitted above because their spellings are ridl's and
are not yet fixed (E3.4). The point of the example is that four families sit in
one interface, each declaring what it corresponds to, and that the first word of
every line says which.

## Appendix B — Prior Art: uxil and uxdl

| Source                               | Taken / rejected                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **uxil** (markspec, ADR-034)         | Taken: the closed interaction-verb set, compiler-enforced; the discipline of a corpus registry with stable diagnostics; five of the eleven verbs survive as the intent operation shapes. Rejected: `scroll` and `drag` (modality — §7); prose-embedded declarations; untyped string key templates, which become typed parameters. Deferred: `navigate` (§11.4), `observe`, `ask`. Its citation grammar is the open question of §11.3 and became ridl's, not rxdl's. |
| **uxdl** v0.1 (retired, ADR-0012)    | Taken: the operation-shape taxonomy and its payload rules; the states-and-availability model; the coverage analysis against MVVM and its lineage. Rejected: `view` as a container (an interface is one); `display`, `input`, `action`, `fetch` as kinds (renames of ridl's, and `display` fails the modality test); `fetch` entirely (a person perceives a result, never a request); a separate `UXDL-` diagnostic family.                                          |
| **MVVM, Elm, Compose, SwiftUI, QML** | The seam itself — observable state and commands across a viewmodel boundary — and the confirmation that a seeded state value is the natural binding for a continuous presentation. Unchanged from uxdl's survey; see the archived reference for the full table.                                                                                                                                                                                                     |
| **AUTOSAR HMI and cluster practice** | The reason init, invalid, staleness, and derivation are contractual at the person boundary: a cluster telltale has legally relevant init and invalid renderings, and an indicated speed has a legally constrained relationship to the true one. This is what became the correspondence obligations, and it generalised to sensors and actuators.                                                                                                                    |

The full uxdl v0.1 reference is preserved at
[`../archive/uxdl-language-reference-v0.1.md`](../archive/uxdl-language-reference-v0.1.md).
Read it as prior work, never as current design.

---

_End of rxdl Language Reference v0.1.0 — Draft._
