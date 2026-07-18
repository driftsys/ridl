# The General Form — Cross-Profile Syntax, Typing, and Attributes

**RIDL family working spec** — the normative surface rules shared by every
profile (typl · ridl · uxdl · rmdl · rsdl). Expands concept note §4.1 (syntax
heritage, keyword discipline) from a style statement into checkable rules.

Version: 0.2.0 — Draft (design sessions of 2026-07-16)

> **Status.** Pre-ADR working spec. The invariants in §3 were _discovered, not
> invented_: an audit of the typl v0.1, ridl v0.2, and uxdl v0.1 references
> showed they already hold everywhere — this document makes them law so they
> keep holding. §4 (attributes), §5 (formatting), and §6 (readability hardening)
> record decisions taken in design session; §7 lists the errata those decisions
> imply for the existing references.

---

## 1. Purpose

One grammar, five profiles, five audiences (concept note §4). The promise that
"crossing layers never means switching syntax families" is only real if the
_shape_ of a declaration is identical across profiles — same reading order, same
meaning for every punctuation mark, same place for every kind of information.
This document defines that shape: the three declaration forms, the nine surface
invariants, the attribute model, and the formatting rules.

The heritage is the **C / TypeScript / Rust / Kotlin lineage** — postfix
`name: Type` typing, brace blocks, `?` optionality, `//` and `///` comments, no
semicolons — and explicitly **not** the Python/YAML lineage: no significant
indentation, no bare key-value prose, structure always carried by explicit
tokens.

---

## 2. The Three Declaration Shapes

Every declaration in every profile is one of three shapes, sharing one prefix:

```
declaration = doc_comment? modifier* shape
modifier    = "internal" | "error"          // prefix keywords, Kotlin-style
```

### Shape 1 — value declaration

A named, typed thing.

```
kw name: Backing [shape]? clauses? @timing? [ attrs ]?   ( "=" value )?
```

| Profile        | Instances                                                                   |
| -------------- | --------------------------------------------------------------------------- |
| typl           | `type`, `const` (with `= value` tail), struct field, union arm, tuple field |
| ridl           | `signal`, `event`, `final`                                                  |
| uxdl           | `display`, `input`, `fixed`                                                 |
| rmdl (planned) | flow declarations                                                           |

```ridl
type    Speed: km/h [0.0..250.0 step 0.5]
type    Counter: integer [0..250] wire uint16
const   MAX_SPEED: Speed = 250.0
signal  targetSpeed: Speed @[20ms..500ms] [ init = SPEED_LIMIT_EU, persist ]
display muted: boolean [ init = false ]
input   searchText: SearchQuery @[100ms..2s]
final   softwareVersion: Version
```

### Shape 2 — callable declaration

A named, invocable thing. The return, when one exists, is introduced by `:` like
every other type. A fallible return spells its expected errors inline (§6.1).

```
kw name(param: Type, …)? (":" Return)? clauses? [ attrs ]?
```

| Profile        | Instances                                                                             |
| -------------- | ------------------------------------------------------------------------------------- |
| ridl           | `command` (no return), `query` (return mandatory)                                     |
| uxdl           | `action` + refinements (`activate`, `toggle`, `select`, `adjust`, `dismiss`), `fetch` |
| rmdl (planned) | `node` — **decided:** outputs via `:` + named tuple, not `returns` (§7.4)             |

```ridl
command  setGear(position: GearPosition) [ require position != GearPosition.PARK || currentSpeed == 0.0 ]
query    getMinMax(window: Duration): (min: Speed, max: Speed)
query    calibrate(target: Axle): CalReport | CalError          // fallible — §6.1
select   trackItem(id: TrackId) during READY
node     control(current: Speed, brake: boolean): (engaged: boolean, target: Speed) { … }
```

### Shape 3 — container declaration

A named group of members, with an optional relation clause.

```
kw Name relation? [ attrs ]? "{" member* "}"
```

| Profile        | Instances                                                     | Relation                                                       |
| -------------- | ------------------------------------------------------------- | -------------------------------------------------------------- |
| typl           | `struct`, `enum`, `enumset`, `union`                          | `: Backing` (enumset derivation — a true backing, hence colon) |
| ridl           | `interface`                                                   | —                                                              |
| uxdl           | `view`                                                        | `states EnumRef`                                               |
| rmdl (planned) | `model`                                                       | `realizes InterfaceRef`                                        |
| rsdl (future)  | `instance x: Type` in manifest — colon again means "typed as" |                                                                |

```ridl
enumset WarningFlags: Warning
view MediaHome states MediaHomeState [ labels = (QM, PUBLIC) ] { … }
model CruiseController realizes CruiseControl { … }
```

**Relation rule:** `:` is used when the relation is _backed-by / typed-as_
(enumset derivation, instance typing); every other relationship gets a named
keyword clause (`states`, `realizes`). Never a second meaning for the colon.

---

## 3. The Nine Surface Invariants

Normative, family-wide, enforced by the one grammar.

**R1 — Keyword-first.** Every declaration opens with its kind keyword. A reader
never infers what a line is from its shape; `grep '^signal'` enumerates every
signal.

**R2 — The colon invariant.** `:` means exactly one thing everywhere it appears:
_is typed as / backed by_. Declarations, parameters, tuple fields, union arms,
map entries (`[Label: Name; 0..32]`), enumset derivation, callable returns, rsdl
instance typing. No other use exists or may be added.

**R3 — Position-typed brackets.** `[ … ]` in **type position** is _data shape_ —
a value constraint (`[0..250 step 1]`) or a collection bound (`[T; 0..32]`,
`[K: V; 0..8]`). `[ … ]` in **tail position** (after the signature, before any
body) is the _attribute block_ — declaration semantics (§4). Shape describes the
value; attributes describe the declaration.

**R4 — `@` is time.** In source, `@` introduces a timing annotation and nothing
else. (Doc-comment tags live inside comments — a different plane, javadoc
heritage; after the §4.7 promotion, `@see` is the only tag left and carries no
semantics.)

**R5 — The sentence order.** Postfix elements always appear in this order,
skipping absent slots:

```
name → (params) → : Type → [shape] → wire W → @timing → during S → [ attrs ] → = value → { body }
```

This order already held in every grammar of every reference before it was
stated; it is now normative. New clauses must be slotted into this order
explicitly.

**R6 — Payloads are named types.** Interaction and behaviour layers never define
shapes inline; they reference typl vocabulary. Contract lines read as domain
sentences, not structure dumps. (The inline fallible return `T | E` respects
this: both arms are named types; only the union container is structural — §6.1.)

**R7 — Case is role.** `CamelCase` = types and containers; `camelCase` = members
and interactions; `SCREAMING_SNAKE` = constants, enum values, states;
`lowercase.dot` = packages. No exceptions, so case alone identifies what a name
denotes.

**R8 — Separator discipline.** Newline and comma interchangeable in every block;
trailing comma legal; no semicolons anywhere. (The `;` inside collection bounds
`[T; N]` is a Rust-heritage type-level delimiter, not a statement separator.)

**R9 — Profile isomorphism.** Sibling profiles rename keywords, never reshape
syntax. `display muted: boolean [ init = false ]` and
`signal engaged: boolean [ init = false ]` are the same sentence with a
different verb. A new construct in any profile must be expressible as one of the
three shapes of §2 or it does not enter the family.

---

## 4. Attributes

### 4.1 The deletion test (normative boundary)

The dividing line between doc comments and attributes:

> **Delete every doc comment in a workspace: `ridlc` output, generated bindings,
> the test plane, `ridl-diff` verdicts, and runtime behavior must be
> bit-identical** (only rendered documentation changes). Anything whose deletion
> would change tool output must be an attribute, clause, or keyword — never a
> doc tag.
>
> **Conversely, every attribute must have at least one machine consumer** —
> compiler, codegen, runtime, diff, test plane, or lint/assurance profile. An
> attribute nobody consumes is documentation wearing brackets, and is rejected.

Consequences (decided): `labels` and `deprecated` are **attributes** (assurance
gating, lint warnings, and generated `@Deprecated`/`#[deprecated]` are machine
consumers — §4.7). `@see` and prose remain doc-comment material. The human
_narrative_ around a deprecation stays in the doc comment; the machine-readable
fact goes in brackets.

### 4.2 The three attribute forms

```ebnf
attr_block = "[" attribute { sep attribute } "]" ;
attribute  = key                              (* flag       — persist            *)
           | key "=" const_value              (* assignment — init = X, default = 3,
                                                 deprecated = "use v2",
                                                 labels = (SIL_2, PRIVATE)       *)
           | pred_key expr ;                  (* predicate  — require e, ensure e,
                                                 invariant e (future expr-core)  *)
```

Every attribute starts with its key — the reader rule is uniform. `const_value`
is a literal, constant reference, or parenthesised list of same; `expr` is the
expr-core surface (guaranteed subset until the core lands, ridl §13).

### 4.3 One production, allow-lists by diagnostics

The existing grammars define `init_attr`, `default_attr`, `field_attrs`, and
`attr_block` as four look-alike productions. **Replace all four with the single
`attr_block` above.** Which keys are legal on which declaration kind is a
_semantic_ allow-list enforced by diagnostics, not four grammar rules:

| Key          | Form       | Legal on                               | Consumer                                   |
| ------------ | ---------- | -------------------------------------- | ------------------------------------------ |
| `default`    | assignment | `type`, struct field                   | init derivation (typl §5.8)                |
| `init`       | assignment | `signal`, `display`                    | channel seeding (ridl §4.4)                |
| `persist`    | flag       | `signal`, `display`                    | lifecycle codegen, provenance, diff (§4.6) |
| `require`    | predicate  | `command`, `query`, `action`*, `fetch` | Stratum 2, tests, observers                |
| `ensure`     | predicate  | `query`, `fetch`                       | Stratum 2, tests, observers                |
| `invariant`  | predicate  | `struct` (future, expr-core)           | cross-field validation                     |
| `labels`     | assignment | any declaration                        | assurance profiles, injection gating       |
| `deprecated` | assignment | any declaration                        | lint, generated deprecation metadata       |

Proposed diagnostics (shared form namespace): unknown attribute key (error); key
not allowed on this declaration kind (error); duplicate key in one block
(error); `deprecated` without a reason string (warning — supersedes TYPL-405).

### 4.4 Placement

Attributes trail the **signature**, before any body — the same slot in all three
shapes. Members: at end of line. Containers: between the relation clause and the
opening brace
(`view MediaHome states MediaHomeState [ labels = (QM, PUBLIC) ] {`). Multi-line
rendering is ordinary block formatting:

```ridl
query getAverageSpeed(window: Duration): Speed [
  require window > 0ms
  ensure  result >= 0.0
]
```

### 4.5 Attribute keys are contextual, not reserved

Attribute keys are recognised **only inside `[ ]` tail blocks** and do not enter
the family keyword registry. Rationale: attributes are the family's designated
cheap extension mechanism (§4.8); if every new key burned an identifier
family-wide, extension would be expensive exactly where it should be cheap.
_Amendment implied:_ ridl §2.3 currently lists `init` as a full keyword —
downgrade to contextual. (`require`/`ensure` remain registry words because the
expr core uses them beyond attribute position.)

### 4.6 Worked example — `persist`

The lifecycle attribute, run through the deletion test: it changes codegen
(storage hooks), runtime behavior (restart survival), the subscriber API
(provenance), and contract identity (`ridl-diff` must classify it). Every
consumer fires → attribute, and a **flag** — the form that motivated closing the
grammar at three shapes.

```ridl
signal  targetSpeed: Speed @[20ms..500ms] [ init = SPEED_LIMIT_EU, persist ]
display volume: Ratio [ init = 50.0, persist ]        // remembered across ignition cycles
```

Semantics:

- **Seeding.** At channel creation: the stored value if one exists, else the
  declared `init`, else the payload type's init. `[ init = X, persist ]` makes
  `X` the first-boot seed.
- **Provenance** gains a fourth origin: `init | stored | live | invalid` —
  consumers can distinguish factory state from restored state.
- **Restored values are validated like any received payload.** A stored value
  that violates the (possibly OTA-tightened) constraints is discarded; the
  channel seeds from `init`; observability records the discard. Constraint
  evolution can never resurrect an illegal value.
- **What the contract does _not_ say:** storage medium, write cadence, wear
  policy — runtime (`ridl-rt`) and deployment (rsdl) concerns. The contract's
  promise is exactly "survives software restarts."
- **Validity:** `signal` and `display` only. Events are not state;
  `final`/`fixed` are provisioned, not persisted; callables have no channel.
- **Evolution:** adding or removing `persist` is a behavioral contract change;
  `ridl-diff` category to be defined with the diff spec.

### 4.7 Promoted metadata — `labels`, `deprecated`

```ridl
/// Cruise control service contract.
interface CruiseControl [ labels = (SIL_2, SEC_2, PRIVATE) ] {
  /// Superseded by setLever2 — kept for M1 fleet.
  command setLever(cmd: LeverCmd) [ deprecated = "use setLever2" ]
  …
}
```

`@labels` and `@deprecated` doc tags are removed; `@see` remains the only doc
tag. Label vocabulary, profile validation, and pass-through semantics are
unchanged from typl §14.3 — only the _home_ moves, per the deletion test.

### 4.8 The promotion path

New capabilities enter the language in this order, each step earned by evidence:

1. **Attribute** — contextual key, cheap, allow-listed (`persist` is here).
2. **Clause** — a bare keyword in the R5 sentence order, for structural
   at-most-once concerns that affect type identity or availability (`wire`,
   `during`, `states`, `realizes` live here).
3. **Keyword / declaration kind** — a new verb in the registry, the expensive
   step (concept note §4.1 discipline).

This mirrors the family's existing doctrine for safety properties (ridl §10.4:
profile vocabulary first, keywords only when earned) and gives it a syntactic
mechanism.

---

## 5. Formatting — `ridl fmt`

**Decided: Kotlin/Rust-tight colon, no column alignment.** `name: Type` in every
position — declarations, parameters, tuple fields, map types, returns,
relations.

```ridl
interface VehicleStatus [ labels = (SIL_B, CAL_2, PRIVATE) ] {
  signal currentSpeed: Speed @10ms
  signal engineTemp: Temperature @[20ms..100ms]
  signal warnings: WarningFlags @[50ms..1s]

  event doorOpened: DoorPayload @[50ms..500ms]

  command setGear(position: GearPosition) [
    require position != GearPosition.PARK || currentSpeed == 0.0
  ]

  query getAverageSpeed(window: Duration): Speed [
    require window > 0ms
    ensure  result >= 0.0
  ]

  final softwareVersion: Version
}
```

Why, on the record:

1. **It is the heritage.** Kotlin, Rust, TypeScript, and Swift all write
   `name: Type` tight. The family claims that lineage; readers from it should
   never hit a spacing surprise.
2. **One rule, all positions.** The aligned style needed an inline/block
   asymmetry (`(window: Duration)` tight, members spaced). Tight-everywhere
   removes the asymmetry — a direct win for the "obviousness" goal: there is
   nothing to learn.
3. **The colon binds to the name.** `currentSpeed:` reads as a label,
   natural-language punctuation (no space before, one after).
4. **Minimal diffs, trivial formatter.** No alignment recomputation, no churn on
   neighbor lines, no blame noise — the rustfmt lesson.

The tabular reading that alignment provided is recovered **by tooling, not
source bytes**: `ridl-lsp` inlay alignment / editor elastic tabstops for those
who want columns on screen, and `ridl doc` renders interfaces as actual tables.
Source stays diff-minimal; presentation is a view concern.

_Errata note:_ all examples in the existing references use the spaced/aligned
style; they are reformatted opportunistically as documents are next touched, and
mechanically once `ridl fmt` exists. This document's examples are written in the
decided style.

---

## 6. Readability Hardening

Outcome of the obviousness review (session 2): the language's surface reads
easily; the risks are places where _semantics hide behind light syntax_. Four
hardenings, three of them costing no new syntax.

### 6.1 Inline fallible returns — `T | E` (decided)

**Principle (owner's phrasing, now doctrine):** _expected errors must be
explicitly defined through the language; infrastructure failures are invisible
in the language._ The three-strata model (ridl §10) already implements the
split; this hardening makes the declared half visible **at the signature**, not
one hop away in a named union.

```ridl
error enum CalError { SENSOR_UNAVAILABLE = 0, VEHICLE_MOVING = 1, OUT_OF_RANGE = 2 }

query calibrate(target: Axle): CalReport | CalError
fetch trackDetails(id: TrackId): TrackDetails | MediaError
```

Rules:

- `return_type` gains `fallible_type = type_ref "|" type_ref` — valid **only**
  in `query`/`fetch` return position.
- Left arm: any non-error named type (success). Right arm: exactly one **error
  type** — several failure kinds compose into an `error union` _before_
  appearing here (typl §10.2 discipline unchanged: one closed failure set per
  callable).
- Both arms are named types — R6 holds; only the union _container_ is
  structural. Codegen, transport error-channel mapping, and `ridl-diff` treat it
  exactly as today's result union; the union's transport identity is synthesized
  (interface + ordinal).
- **Canonical form.** A _named_ result union remains legal typl data (for
  storing outcomes in structs, logs), but in return position the inline form is
  canonical — a named result union as a return draws a lint steering to `T | E`.
  One way to say it where it matters ("strict beats flexible").
- No `throws`, no exceptions, no status channel — this is a spelling of
  errors-as-data, not a second mechanism.

### 6.2 Timing bounds — one generic meaning (decided)

`@[min..max]` previously carried four cell-by-cell meanings (signal:
debounce/refresh; event: throttle/TTL). Respecified **once, generically**:

> **`min` = rate floor** — minimum interval between publications. **`max` =
> staleness bound** — maximum age (on envelope sender timestamps) before the
> value/occurrence is stale.

The per-kind behavioral difference — a stale _state_ value is refreshed and a
fast one coalesced; a stale _occurrence_ is discarded and a fast one throttled —
is **derived from the state-vs-occurrence semantics of the declaring keyword**,
not from the annotation. The naive reading of `@[20ms..500ms]` ("not faster than
20ms, not staler than 500ms") is now the correct reading. The four-cell table in
ridl §9 becomes a derivation, not a definition. LSP hover expands the per-kind
consequence.

### 6.3 Ordinal visibility (decided — tooling)

Declaration order is wire identity, and a reorder looks like tidying. Two
mitigations, no syntax:

- **LSP ordinal inlay hints** — the derived ordinal rendered beside every
  field/interaction. Reordering visibly becomes renumbering; "the wire layout is
  readable from source" becomes literal.
- **Baseline-aware `ridlc`** — when a previously published IR snapshot is
  available (lockfile/cache, ADR-0002 infrastructure), the compiler itself flags
  reorder/insertion at the desk instead of leaving detection to `ridl-diff` in
  CI. The typl §7.4 trade-off ("unprotected without the CI gate") shrinks to
  "unprotected only with no baseline at all."

### 6.4 Error-surface terminology (normative wording)

Stratum 3 is never described as _undefined behavior_: UB means the system may do
anything, which is the opposite of the design — Stratum 3 failures are **fully
detected by the runtime** (acks, timeouts, staleness; "nothing fails silently",
ridl §10.4) and merely **undeclared in the contract language**. Normative
phrasing: **"infrastructure failure — detected, undeclared."**

Boundary rule of thumb for "invalid state": _could the caller have known before
calling?_ Observable via a signal or a `during`/`require` gate → **Stratum 2**
(declare the precondition, the rejection is derived — never an error type). Only
the provider can know (battery actually empty) → **Stratum 1** (an explicit
`error` value in the inline return). One question decides the home.

### 6.5 `final` naming — reopened, undecided

A Java/Kotlin reader may confidently misread `final` as a compile-time constant
rather than _provisioned, immutable per software instance, FOTA-updatable
between instances_. Options on the table: keep `final` + doc-hover mitigation;
unify both siblings on **`fixed`** (uxdl's word — no compile-time-constant
prior, shortens the interact-core table); rename to `provisioned` (unambiguous,
long). The naming-ledger entry is formally reopened; no decision yet.

---

## 7. Errata and Amendments Implied for Existing Documents

1. **typl Appendix E, ridl Appendix C, uxdl Appendix C** — replace `init_attr`,
   `default_attr`, `field_attrs`, `attr_block` with the single §4.2 `attr_block`
   production; add the attribute-diagnostic set (§4.3).
2. **typl §14.2–14.3** — `@labels` and `@deprecated` move from doc tags to
   attributes (§4.7); `@see` remains; TYPL-402/403/405 re-anchor to attribute
   position.
3. **ridl §2.3** — `init` downgraded from registry keyword to contextual
   attribute key (§4.5).
4. **Concept note §6 (cruise example)** — rmdl node outputs use `: (…)`
   named-tuple form, not `returns (…)`; carry into the rmdl reference draft
   (Shape 2, R2).
5. **ridl §7 / §10.1, uxdl §7** — fallible queries/fetches adopt the inline
   `T | E` return (§6.1); named-result-union returns draw the canonical-form
   lint; RIDL-303 family adapts; grammar `return_type` gains `fallible_type`.
6. **ridl §9** — timing semantics restated generically per §6.2 (min = rate
   floor, max = staleness bound); the per-kind table becomes a derivation.
   Glossary entries (debounce/refresh/throttle/TTL) re-anchor as derived
   readings.
7. **ridl §10.3 / glossaries** — Stratum 3 wording standardized to
   "infrastructure failure — detected, undeclared" (§6.4).
8. **All references** — example style migrates to tight colon (§5) as documents
   are touched.
9. **rsdl (future)** — the manifest sketch's `bind x <- y` / `-> f()` arrows are
   the family's first directional tokens; when rsdl graduates to a grammar
   profile, wiring syntax must be reconciled with R2–R5 (arrows are candidates
   for wiring _only_, never for typing or returns). Note: `|` is now also taken
   (fallible returns) — rsdl may not overload it.
10. **Minor, on record** — regex constants are the one `const` form without a
    type annotation (`const VIN_PATTERN = /…/`); rule: regex consts are untyped
    by definition. No change, documented here.

---

## 8. Decisions Ledger (these sessions)

| Decision                      | Outcome                                                                                                                                         |
| ----------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| Attribute syntax              | **Trailing `[ … ]` block** — position-distinguished from shape brackets (R3); prefix sigils (`#[…]`) and above-the-line placement rejected      |
| Doc tag vs attribute boundary | **The deletion test** (§4.1), both directions normative                                                                                         |
| `labels`, `deprecated`        | **Promoted to attributes**; `@see` remains the only doc tag                                                                                     |
| Attribute grammar             | **Three forms** — flag / assignment / predicate; one `attr_block` production; allow-lists via diagnostics                                       |
| Attribute keys                | **Contextual**, not registry keywords                                                                                                           |
| `persist`                     | Admitted as the reference flag attribute; semantics per §4.6; diff category open                                                                |
| rmdl node outputs             | **`: (named tuple)`** — colon invariant restored; `returns` rejected                                                                            |
| `ridl fmt` colon              | **Tight `name: Type`, no alignment**; columns are a tooling/rendering concern                                                                   |
| Clause vs attribute principle | Clauses = closed set of structural at-most-once modifiers (`wire`, `during`, `states`, `realizes`); everything else enters as attributes (§4.8) |
| Fallible returns              | **Inline `T                                                                                                                                     |
| Timing semantics              | **Generic min/max** (rate floor / staleness bound); per-kind behavior derived from signal-vs-event (§6.2)                                       |
| Ordinal safety                | **Tooling**: LSP ordinal inlays + baseline-aware `ridlc` (§6.3)                                                                                 |
| Stratum 3 wording             | **"Infrastructure failure — detected, undeclared"**; never "UB" (§6.4)                                                                          |
| `final` naming                | **Reopened**, undecided (§6.5)                                                                                                                  |

## 9. Open Questions

1. **`invariant` placement.** Header attr block on `struct` (uniform with
   require/ensure) vs body-line form — decide with the expr core. Leaning header
   for uniformity; body form reads better for long lists.
2. **`labels` list syntax.** `labels = (SIL_2, PRIVATE)` parenthesised list is
   the working form; confirm against the assurance-profile spec (suffixed labels
   `MY_LABEL(D)` must nest cleanly).
3. **`persist` diff classification** and whether a `persist`-capable transport
   binding is a deployment precondition rsdl must check.
4. **Attribute-key governance.** Contextual keys avoid the registry, but a
   family-wide _attribute_ registry (name → form → allow-list → consumer) is
   still needed in the platform spec so profiles cannot define colliding keys
   with different meanings.
5. **`during` residence.** Stays a clause by the §4.8 principle (structural
   availability, at-most-once, drives scaffolding). Revisit only if per-state
   attribute needs (uxdl §16.6 per-state timing) force a more general gating
   syntax.
6. **`final` vs `fixed` vs `provisioned`** — the reopened §6.5 naming question.
7. **Synthesized identity of inline fallible unions** — exact derivation rule
   (interface + ordinal + arms?) to be fixed in the IR spec so transport IDs are
   stable under compatible evolution.

---

_End of General Form working spec v0.2.0 — Draft._
