# The General Form — Cross-Profile Syntax, Typing, and Attributes

The normative surface rules shared by every language of the family (typl, ridl,
rmdl, rsdl). Expands concept note §4.1 (syntax heritage, keyword discipline)
from a style statement into checkable rules.

Version: 0.3.0 — Ratified (2026-10-15)

> **Status.** Normative. The invariants in §3 were _discovered, not invented_:
> an audit of the typl, ridl and uxdl references showed they already hold
> everywhere, and this document makes them law so they keep holding. This
> version states what the toolchain implements. Where the code and this document
> differ, the code and the language references are right and this document has a
> defect to correct. A part that is specified but not built says so where it
> appears: rmdl has no implementation, `during` and `states` are reserved words
> without a grammar (§4.8), and `persist` and `invariant` are not built (§4.3,
> §4.6). §4 (attributes), §5 (formatting) and §6 (readability hardening) record
> the decisions taken in design sessions, as amended by the records they cite;
> §7 lists what became of the errata those decisions implied for the references.

---

## 1. Purpose

One grammar, four languages, one audience per language (concept note §4): typl,
ridl, rmdl and rsdl. The promise that "crossing layers never means switching
syntax families" is only real if the _shape_ of a declaration is identical
across the languages — same reading order, same meaning for every punctuation
mark, same place for every kind of information. This document defines that
shape: the three declaration forms, the nine surface invariants, the attribute
model, and the formatting rules.

uxdl is retired as a family member (ADR-0012): its boundary model moved into
ridl, and the rxdl reference is a Draft that adds domain spellings over ridl and
adds no semantics. rmdl is a Proposed draft with no implementation, so every
rmdl remark in this document is a constraint on a future grammar, not a
description of a built one.

The heritage is the **C / TypeScript / Rust / Kotlin lineage** — postfix
`name: Type` typing, brace blocks, `?` optionality, `//` and `///` comments, no
semicolons — and explicitly **not** the Python/YAML lineage: no significant
indentation, no bare key-value prose, structure always carried by explicit
tokens.

---

## 2. The Three Declaration Shapes

Every declaration in every language is one of three shapes, sharing one prefix:

```
declaration = doc_comment? modifier* shape
modifier    = "internal" | "error"          // prefix keywords, Kotlin-style
```

The modifiers are narrower than the prefix allows:

- `internal` applies to a typl declaration (`type`, `const`, `struct`, `enum`,
  `enumset`, `union`) and to a ridl `interface`. A `service` takes none, because
  a service is always public (ridl §14.5); an rsdl declaration takes none,
  because the closure is workspace-wide (rsdl §2). `internal` before either is a
  parse error (FORM-102).
- `error` applies to `struct`, `enum` and `union` only. On any other declaration
  it is TYPL-212 (typl §10.1).

### Shape 1 — value declaration

A named, typed thing.

```
kw name: Backing [shape]? ( "=" value )? @timing? [ attrs ]?
```

| Language       | Instances                                                                   |
| -------------- | --------------------------------------------------------------------------- |
| typl           | `type`, `const` (with `= value` tail), struct field, union arm, tuple field |
| ridl           | `signal`, `event`, `fixed`                                                  |
| rmdl (planned) | flow declarations                                                           |

```ridl
type Speed: km/h [0.0..250.0 step 0.5]
const MAX_SPEED: Speed = 250.0
signal targetSpeed: Speed = MAX_SPEED @[20ms..500ms]
event doorOpened: DoorPayload @[50ms..500ms]
fixed softwareVersion: Version
```

### Shape 2 — callable declaration

A named, invocable thing. The return, when one exists, is introduced by `:` like
every other type. A fallible return spells its expected errors inline (§6.1).

```
kw name(param: Type, …)? (":" Return)? @timing? [ attrs ]?
```

| Language       | Instances                                                                     |
| -------------- | ----------------------------------------------------------------------------- |
| ridl           | `command` (no return), `query` (return mandatory)                             |
| rmdl (planned) | callable declarations; their outputs follow R2 (`:` and a named tuple) (§7.4) |

```ridl
command setGear(position: GearPosition) [
  require position != GearPosition.PARK || currentSpeed == 0.0
]
query getMinMax(window: Duration): (min: Speed, max: Speed)
query calibrate(target: Axle): CalReport | CalError          // fallible — §6.1
```

### Shape 3 — container declaration

A named group of members, with an optional relation clause.

```
kw Name relation? [ attrs ]? "{" member* "}"
```

| Language       | Instances                                                      | Relation                                                                 |
| -------------- | -------------------------------------------------------------- | ------------------------------------------------------------------------ |
| typl           | `struct`, `enum`, `enumset`, `union`                           | `: Backing` (enumset derivation — a true backing, hence colon)           |
| ridl           | `interface`                                                    | —                                                                        |
| ridl           | `service`                                                      | `: Interface, …` (a composition list), or an inline body of interactions |
| rmdl (planned) | `model`                                                        | not decided                                                              |
| rsdl           | `system`, `component`, `distribution`, `deployment`, `machine` | `for SystemRef` (`deployment`)                                           |

A `service` is either a named list —
`service veh.body.doors: DoorControl,
DiagBlock` — or one inline body of
interactions, never both (ridl §14.5). It takes no attribute block.

```ridl
enumset WarningFlags: Warning
service veh.adas.cruise: CruiseControl
service veh.body.doors: DoorControl, DiagBlock
```

**Relation rule:** `:` is used when the relation is _backed-by / typed-as_
(enumset derivation); every other relationship gets a named keyword clause
(rsdl's `for`). The one other place a colon introduces a relation is the
composition list of a `service` (the exception to R2, §3).

**Two member forms** appear inside container bodies and have precedent in more
than one language (amended 2026-09-13 by the rsdl rewrite):

- the **bare member line** — a reference, not a declaration. rsdl's `system`,
  `distribution` and `machine` bodies list their members this way, and ridl's
  `service` lists its interfaces by bare name after its colon (ridl §14.5);
- the **keyword-plus-reference line** — a keyword followed by one reference.
  rsdl's `offers X` and `requires X`, ridl's `reserved Name`, and the file-level
  `import` have this form.

In rsdl, both forms may carry the attribute block after the reference (§4.4).

---

## 3. The Nine Surface Invariants

Normative, family-wide, enforced by the one grammar.

**R1 — Keyword-first.** Every declaration opens with its kind keyword. A reader
never infers what a line is from its shape; `grep '^signal'` enumerates every
signal.

**R2 — The colon invariant.** `:` means exactly one thing everywhere it appears:
_is typed as / backed by_. Declarations, parameters, tuple fields, union arms,
map entries (`[Label: Name; 0..32]`), enumset derivation, callable returns. No
other use exists or may be added, with one decided exception: the composition
list of a ridl `service` (`service name: A, B`) uses the colon to introduce the
interfaces that the service composes. The exception is closed: it covers that
list and nothing else (ridl §14.5, Appendix C; ADR-0015 decision 12).

**R3 — Position-typed brackets.** `[ … ]` in **type position** is _data shape_ —
a value constraint (`[0..250 step 1]`) or a collection bound (`[T; 0..32]`,
`[K: V; 0..8]`). `[ … ]` in **tail position** (after the signature, before any
body) is the _attribute block_ — declaration semantics (§4). Shape describes the
value; attributes describe the declaration.

**R4 — `@` is time.** In source, `@` introduces a timing annotation and nothing
else. (Doc-comment tags live inside comments — a different plane, javadoc
heritage. The §4.7 promotion is declined for typl and ridl (ADR-0028 decision
4), so `@see`, `@since`, `@labels` and `@deprecated` stay doc tags.)

**R5 — The sentence order.** Postfix elements always appear in this order,
skipping absent slots:

```
name → (params) → : Type → [shape] → = value → @timing → [ attrs ] → { body }
```

A relation clause that is a keyword takes the slot of the type, because the two
never occur together: `deployment Name for System [ attrs ] { body }`. New
clauses must be slotted into this order explicitly. The parser accepts `@timing`
and `[ attrs ]` in either order, and `ridl fmt` writes `@timing` before
`[ attrs ]` (maintainer decision D-4 of `fmt-ridl-rsdl-layout.md`); `ridl fmt`
reorders no other slot. The parser accepts `= value` only before `@timing`,
which follows decision 2 of ADR-0008. The reserved word `during` has no slot: no
parser in the workspace accepts it, and no recorded decision places it (ridl
§3.4). No `wire` clause exists; the explicit width clause is deferred (typl
§17.11).

**R6 — Payloads are named types.** Interaction and behaviour layers never define
shapes inline; they reference typl vocabulary. Contract lines read as domain
sentences, not structure dumps. (The inline fallible return `T | E` respects
this: both arms are named types; only the union container is structural — §6.1.)

**R7 — Case is role.** `CamelCase` = types and containers; `camelCase` = members
and interactions; `SCREAMING_SNAKE` = constants and enum values; `lowercase.dot`
= packages and service names. No exceptions, so case alone identifies what a
name denotes.

**R8 — Separator discipline.** Newline and comma interchangeable in every block;
trailing comma legal; no semicolons anywhere. (The `;` inside collection bounds
`[T; N]` is a Rust-heritage type-level delimiter, not a statement separator.)
One list is an exception: the shape list of a named `service` requires commas
between its shapes, because it has no closing token and ends where the next
declaration begins (ADR-0015 decision 13). A trailing comma there stays
optional.

**R9 — Language isomorphism.** Sibling languages rename keywords, never reshape
syntax. `signal`, `event` and `fixed` are one sentence with a different verb:
`signal engaged: Engaged @10ms`, `event doorOpened: DoorPayload @[50ms..500ms]`,
`fixed softwareVersion: Version`. A new construct in any language must be
expressible as one of the three shapes of §2 or it does not enter the family.

---

## 4. Attributes

### 4.1 The deletion test (normative boundary)

The dividing line between doc comments and attributes:

> **Delete every doc comment in a workspace: compiler output, generated
> bindings, the test plane, `ridl diff` verdicts, and runtime behavior must be
> bit-identical** (only rendered documentation changes). Anything whose deletion
> would change tool output must be an attribute, clause, or keyword — never a
> doc tag.
>
> **Conversely, every attribute must have at least one machine consumer** —
> compiler, codegen, runtime, diff, test plane, or lint/assurance profile. An
> attribute nobody consumes is documentation wearing brackets, and is rejected.

Consequences: `@see` and `@since` pass the test, and prose remains doc-comment
material. `@deprecated` and `@labels` are the recorded exception for typl and
ridl: the IR carries them and generated code reads deprecation from the tag
(ADR-0026 decision 4, ADR-0028 decision 4), because the promotion of both to
attributes is declined (§4.7). On an rsdl declaration both are attribute keys
(§4.3).

### 4.2 The three attribute forms

```ebnf
attr_block = "[" attribute { sep attribute } "]" ;
attribute  = key                              (* flag       — external           *)
           | key "=" const_value              (* assignment — tier = PLATFORM,
                                                 depth = 8,
                                                 labels = (SIL_2, PRIVATE)       *)
           | pred_key expr ;                  (* predicate  — require e, ensure e *)
```

Every attribute starts with its key — the reader rule is uniform. `const_value`
is a literal, constant reference, or parenthesised list of same; `expr` is the
expr-core surface (guaranteed subset, ridl §13). An empty block is FORM-101. A
key is one name, or in an `.rsdl` file also `namespace.key`, the backend pattern
(§4.3). rsdl widens `const_value` in two positions, the value and the list item,
to admit a bare name: `instances = (primary, backup)` lists camelCase instance
names, which are neither literals nor constant references, and a misspelt
`instances = solo` or a declared `Unit` still parses so that its rsdl diagnostic
reports it (amended 2026-09-13, rsdl reference Appendix B).

### 4.3 One production, allow-lists by diagnostics

There is one `attr_block` production, above. Which keys are legal on which
declaration kind is a _semantic_ allow-list enforced by diagnostics, not
separate grammar rules. The allow-list is one table, the attribute registry
`ridl_core::attributes::ATTRIBUTES` (`crates/ridl-core/src/attributes.rs`): one
row per key, with its form, its sites, its machine consumer, and what
`ridl diff` does with a change. The ridl checker and the rsdl attribute reader
read that table and no other list. The rsdl keys are tabulated in rsdl reference
§5; this document does not copy the table, so that the code and one reference
own it.

As built, the registry holds `require` and `ensure` (ridl), `instances`,
`external`, `tier`, `labels`, `deprecated`, the sizing keys `depth`, `slots` and
`budget` (rsdl), and the backend pattern `namespace.key` (a backend that claims
the namespace through `[backend.<name>]` in the manifest, ADR-0012 decision 8).
Their placement:

- `require` is legal on `command` and `query` only; `ensure` on `query` only. A
  `signal`, an `event` and a `fixed` take no attribute (RIDL-301 for a predicate
  on a signal or event, RIDL-106 for any attribute block on a `fixed`).
- `labels` and `deprecated` are legal on the rsdl declarations only.
- `instances`, `external` and `tier` are legal on the rsdl declaration their row
  names; the sizing keys on a `deployment` and a placement line.

**Not attribute keys.** `default`, `init`, `persist` and `invariant` are not
keys: they have no machine consumer, so by §4.1 they are not attributes, and
ADR-0012 decision 8 records that. A `default`, `persist` or `invariant` key
draws FORM-106 (the expression after `invariant` also draws FORM-102, because it
does not parse as a value). `init` is a reserved word, so it draws FORM-105. A
signal's initial value is the bare `= value` suffix (ADR-0008 decision 2), and a
type's or field's initial value is `= value` (typl §5.8).

Diagnostics (shared form namespace): unknown attribute key (FORM-106); key not
allowed on this declaration kind (FORM-107); duplicate key in one block
(FORM-108). The missing-reason rule for `deprecated` is the lint
`deprecated-without-reason` (TYPL-405), which fires on a bare `@deprecated` doc
tag (typl §14.2, ADR-0026); it is current and nothing supersedes it.

### 4.4 Placement

Attributes trail the **signature**, before any body — the same slot in all three
shapes. On ridl, the members that take an attribute block are `command` and
`query`, at the end of the line. On rsdl, a container takes it between its name
(or its `for` clause) and the opening brace, and a body line takes it after its
reference. An `interface` and a `service` take no attribute block; one written
there is a parse error. Multi-line rendering is ordinary block formatting:

```ridl
query getAverageSpeed(window: Duration): Speed [
  require window > 0ms
  ensure result >= 0.0
]
```

### 4.5 Attribute keys are not registry words

An attribute key is an ordinary identifier outside its `[ ]` block: it does not
enter the family keyword registry. Rationale: attributes are the family's
designated cheap extension mechanism (§4.8); if every new key burned an
identifier family-wide, extension would be expensive exactly where it should be
cheap. `external`, `tier`, `instances`, `labels`, `deprecated`, `depth`, `slots`
and `budget` are identifiers everywhere else. A key that is a reserved word is
FORM-105: `init` stays reserved (it is rmdl's memory-seed keyword), so it cannot
be a key. `require` and `ensure` are registry words because the expr core uses
them beyond attribute position.

### 4.6 `persist` — not built

A lifecycle attribute `persist` ("survives software restarts") was proposed in
the design sessions. It is not built: it has no machine consumer, and ADR-0008
decision 3 deferred it, because its wire-evolution category was undecided and a
signal carries no attribute block.

### 4.7 Promoted metadata — `labels`, `deprecated`

The proposal moved `labels` and `deprecated` from doc tags to attributes on
every declaration. It is **declined for typl and ridl** (ADR-0028 decision 4,
which resolves the deferral of ADR-0026 decision 4): they stay doc tags, beside
`@see` and `@since`, and generated code takes its deprecation metadata from the
`@deprecated` tag. It is **implemented as attributes on rsdl declarations**: the
five rsdl declarations take `labels` and `deprecated` as attribute keys (§4.3,
rsdl reference §5).

The reason for the decline is the grammar. An attribute block exists on
`command` and `query` (the grammar also parses one on `signal`, `event` and
`fixed`, where no key is legal) and on the rsdl declarations and body lines.
`type`, `const`, `struct`, `enum`, `enumset`, `union`, `interface`, `service`,
fields, enum values and `fixed` have none, so the promotion would add the
attribute block to the grammar on these eleven carriers to move two facts that
the doc tags already deliver to every consumer. Label vocabulary, profile
validation and pass-through semantics are those of typl §14.3.

### 4.8 The promotion path

New capabilities enter the language in this order, each step earned by evidence:

1. **Attribute** — a row in the attribute registry with a machine consumer,
   cheap, allow-listed (`external` and `tier` are here).
2. **Clause** — a bare keyword in the R5 sentence order, for structural
   at-most-once concerns that affect type identity or availability. The clause
   words built are `for` (rsdl `deployment`) and `as` (the `import` alias).
   `during` and `states` are reserved clause words without a grammar (ADR-0012
   decision 4, as amended): the grammar of `during` waits for the boundary model
   (ridl §3.4).
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
interface VehicleStatus {
  signal currentSpeed: Speed @10ms
  signal engineTemp: Temperature @[20ms..100ms]
  signal warnings: WarningFlags @[50ms..1s]

  event doorOpened: DoorPayload @[50ms..500ms]

  command setGear(position: GearPosition) [
    require position != GearPosition.PARK || currentSpeed == 0.0
  ]

  query getAverageSpeed(window: Duration): Speed [
    require window > 0ms
    ensure result >= 0.0
  ]

  fixed softwareVersion: Version
}
```

The formatter implements these rules (`crates/ridl-fmt/src/lib.rs`):

- **Tight colon.** No space before `:`, one space after it, in every position.
- **No alignment.** A member is never padded to line up with its neighbours.
- **One space** around `=` in an initialiser or an enum value, and between a
  keyword and its name.
- **Width.** A line breaks at 100 columns by default. The width is the
  `max_line_length` of the matching `.editorconfig`; `off` disables breaking.
- **Timing before attributes.** A timing annotation is written before the
  attribute block (decision D-4 of the layout note).
- **No commas in a broken block.** Inside braces, one member per line with no
  separator comma. Tuple types, parameter lists, attribute value lists and
  service shape lists that break go one item per line, with commas.

The full layout — every declaration, the breaking rules, and the treatment of
comments — is `docs/wip/fmt-ridl-rsdl-layout.md`.

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

Source stays diff-minimal; a tabular view of an interface is a presentation
concern for a tool and is not part of the source bytes.

_Errata note:_ some examples in the existing references still use the
spaced/aligned style; they are reformatted as documents are next touched (§7
item 8). This document's examples are written in the decided style.

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
```

Rules:

- `return_type` has `fallible_type = type_ref "|" type_ref` (ridl Appendix C) —
  valid **only** in `query` return position.
- Left arm: any non-error named type (success). Right arm: exactly one **error
  type** — several failure kinds compose into an `error union` _before_
  appearing here (typl §10.2 discipline unchanged: one closed failure set per
  callable).
- Both arms are named types — R6 holds; only the union _container_ is
  structural. Codegen, transport error-channel mapping, and `ridl diff` treat it
  exactly as a named result union; the union's transport identity is synthesized
  (interface, ordinal and ordered arm types — ADR-0008 decision 4).
- **Canonical form.** A _named_ result union remains legal typl data (for
  storing outcomes in structs, logs), but in return position the inline form is
  canonical — a named result union as a return draws the lint RIDL-308 steering
  to `T | E`. One way to say it where it matters ("strict beats flexible").
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
ridl §9 is a derivation, not a definition. LSP hover expands the per-kind
consequence.

### 6.3 Ordinal visibility (decided — tooling)

Declaration order is wire identity, and a reorder looks like tidying. Two
mitigations, no syntax:

- **LSP ordinal inlay hints** — the derived ordinal rendered beside every
  field/interaction. Reordering visibly becomes renumbering; "the wire layout is
  readable from source" becomes literal.
- **Baseline-aware desk check** — `ridl baseline` publishes a snapshot of the
  workspace, and `ridl check --baseline` compares the checked workspace against
  it and warns (RIDL-407) on a reorder or an insertion at the desk, instead of
  leaving detection to `ridl diff` in CI. The typl §7.4 trade-off ("unprotected
  without the CI gate") shrinks to "unprotected only with no baseline at all."

**Interface numbers follow the same rule** (rsdl decision D-7; the lock design,
2026-09-15). One level up, an interface's number is wire identity too, and it is
never written in the source: it lives in the package's `interfaces.lock`, which
only `ridl lock` writes, so a rename keeps the number and a reorder of a
service's list moves nothing. A declaration with no entry compiles with a
provisional number that `ridl baseline` refuses to publish (ridl §11, RIDL-411).
Materialising numbers into the source is a possible later feature at every level
at once, not a per-level choice.

### 6.4 Error-surface terminology (normative wording)

Stratum 3 is never described as _undefined behavior_: UB means the system may do
anything, which is the opposite of the design — Stratum 3 failures are **fully
detected by the runtime** (acks, timeouts, staleness; "nothing fails silently",
ridl §10.4) and merely **undeclared in the contract language**. Normative
phrasing: **"infrastructure failure — detected, undeclared."**

Boundary rule of thumb for "invalid state": _could the caller have known before
calling?_ Observable via a signal or a `require` gate → **Stratum 2** (declare
the precondition, the rejection is derived — never an error type). Only the
provider can know (battery actually empty) → **Stratum 1** (an explicit `error`
value in the inline return). One question decides the home. A `during` gate
belongs to the same case once its grammar exists (ridl §3.4).

### 6.5 `final` naming — decided: `fixed`

A Java/Kotlin reader may confidently misread `final` as a compile-time constant
rather than _provisioned, immutable per software instance, FOTA-updatable
between instances_. Three options were weighed: keep `final` with doc-hover
mitigation; spell it `fixed` (a word with no compile-time-constant prior);
rename to `provisioned` (unambiguous, long).

**Decided (2026-07-27, ADR-0011): ridl spells it `fixed`.** ridl's `final` was
renamed and left the reserved-word registry.

---

## 7. Errata and Amendments Implied for Existing Documents

Each item records what became of the amendment the design sessions implied.

1. **typl Appendix E, ridl Appendix C, uxdl Appendix C** — replace `init_attr`,
   `default_attr`, `field_attrs`, `attr_block` with the single §4.2 `attr_block`
   production; add the attribute-diagnostic set (§4.3). _Applied:_ ridl Appendix
   C and rsdl Appendix B carry the single production, and FORM-106 to FORM-108
   are the diagnostic set. typl takes no attribute block, and uxdl is retired.
2. **typl §14.2–14.3** — `@labels` and `@deprecated` move from doc tags to
   attributes (§4.7); `@see` remains; TYPL-405 re-anchors to attribute position.
   _Declined:_ ADR-0028 decision 4; `@labels` and `@deprecated` stay doc tags.
3. **ridl §2.3** — `init` downgraded from registry keyword to contextual
   attribute key (§4.5). _Superseded:_ ridl has no `init` keyword and no `init`
   key; `init` is a reserved word that belongs to rmdl, and a signal's initial
   value is the bare `= value` suffix (ADR-0008 decision 2).
4. **Concept note §6 (cruise example)** — rmdl node outputs use `: (…)`
   named-tuple form, not `returns (…)`. _Superseded:_ rmdl dropped `node` and
   `returns` (typl §1.4); rmdl's callable declarations stay undecided.
5. **ridl §7 / §10.1, uxdl §7** — fallible queries adopt the inline `T | E`
   return (§6.1); named-result-union returns draw the canonical-form lint;
   RIDL-303 family adapts; grammar `return_type` gains `fallible_type`.
   _Applied_ (RIDL-308 is the lint; uxdl is retired).
6. **ridl §9** — timing semantics restated generically per §6.2 (min = rate
   floor, max = staleness bound); the per-kind table becomes a derivation.
   Glossary entries (debounce/refresh/throttle/TTL) re-anchor as derived
   readings. _Applied._
7. **ridl §10.3 / glossaries** — Stratum 3 wording standardized to
   "infrastructure failure — detected, undeclared" (§6.4). _Applied._
8. **All references** — example style migrates to tight colon (§5) as documents
   are touched. _Partly applied:_ the migration continues as documents are
   touched, and some reference examples still use the aligned style.
9. **rsdl (future)** — the manifest sketch's `bind x <- y` / `-> f()` arrows
   were to be reconciled with R2–R5 when rsdl graduated to a grammar.
   _Superseded:_ the rsdl rewrite has no arrows: wiring is a `requires` line
   (rsdl reference §3). `|` stays taken by fallible returns, and rsdl does not
   overload it.
10. **Minor, on record** — regex constants are the one `const` form without a
    type annotation (`const VIN_PATTERN = /…/`); rule: regex consts are untyped
    by definition. _Applied:_ typl §6.2.

---

## 8. Decisions Ledger (these sessions)

| Decision                       | Outcome                                                                                                                                                                            |
| ------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Attribute syntax               | **Trailing `[ … ]` block** — position-distinguished from shape brackets (R3); prefix sigils (`#[…]`) and above-the-line placement rejected                                         |
| Doc tag vs attribute boundary  | **The deletion test** (§4.1), both directions normative; `@deprecated` and `@labels` are the recorded exception                                                                    |
| `labels`, `deprecated`         | **Declined for typl and ridl** (ADR-0028 decision 4): both stay doc tags; **implemented as attributes on rsdl declarations**                                                       |
| Attribute grammar              | **Three forms** — flag / assignment / predicate; one `attr_block` production; allow-lists via diagnostics and the registry (§4.3)                                                  |
| Attribute keys                 | **Ordinary identifiers**, not registry keywords; a reserved word as a key is FORM-105. The contextual-key proposal for `init` was not built (§4.5)                                 |
| `persist`                      | **Deferred** (ADR-0008 decision 3); not built (§4.6)                                                                                                                               |
| `default`, `init`, `invariant` | **Not attribute keys**: no machine consumer (ADR-0012 decision 8)                                                                                                                  |
| rmdl node outputs              | **Dropped**: rmdl has no `node` and no `returns`; its callable shape is undecided (R2 still governs any return)                                                                    |
| `ridl fmt` colon               | **Tight `name: Type`, no alignment**; a tabular view is a tooling concern (§5)                                                                                                     |
| Clause vs attribute principle  | Clauses = closed set of structural at-most-once modifiers. Built: `for`, `as`. `during` and `states` are reserved without a grammar; everything else enters as an attribute (§4.8) |
| Fallible returns               | **Inline `T \| E`** — both arms named, right arm an `error` type; named result union in return position draws RIDL-308 (§6.1)                                                      |
| Timing semantics               | **Generic min/max** (rate floor / staleness bound); per-kind behavior derived from signal-vs-event (§6.2)                                                                          |
| Ordinal safety                 | **Tooling**: LSP ordinal inlays, and `ridl baseline` with `ridl check --baseline` (§6.3)                                                                                           |
| Stratum 3 wording              | **"Infrastructure failure — detected, undeclared"**; never "UB" (§6.4)                                                                                                             |
| `final` naming                 | **Decided** — ridl spells it `fixed` (§6.5, ADR-0011)                                                                                                                              |
| Colon exception                | **The `service` composition list** uses `:` (R2); its commas are required (R8, ADR-0015 decisions 12 and 13)                                                                       |

## 9. Open Questions

The numbers are stable: other records cite them. Questions 3, 4 and 6 are closed
and keep no entry body.

1. **`invariant` placement.** Header attribute block on `struct` (uniform with
   require/ensure) versus a body form. Not built: typl §17.7 records the likely
   surface as a struct-level `invariant` block, to be decided with the expr
   core.
2. **`labels` list syntax.** As built, an rsdl `labels` value is a parenthesised
   list of bare `SCREAMING_SNAKE` names, `labels = (SIL_2, PRIVATE)`. The
   suffixed form `MY_LABEL(D)` of the `@labels` doc tag (typl §14.3) is not
   accepted there. Whether it must nest in the attribute form stays open.
3. _Closed._ `persist` is deferred (§4.6), so its diff classification is moot.
4. _Closed._ The family attribute registry exists and is the allow-list
   (ADR-0012 decisions 8 and 9, `ridl_core::attributes::ATTRIBUTES`).
5. **`during` residence.** `during` stays a reserved word with no grammar (ridl
   §3.4). It would be a clause by the §4.8 principle (structural availability,
   at-most-once). Revisit when the boundary model grammar fixes its slot (R5).
6. _Closed._ The synthesized identity of an inline fallible union derives from
   the interface, the interaction ordinal and the ordered arm types (ADR-0008
   decision 4).

---

_End of the General Form specification v0.3.0 — Ratified._
