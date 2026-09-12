# Multi-Interface Services

| Field      | Value                                                                                                                                      |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| Status     | design, for review — nothing ratified                                                                                                      |
| Date       | 2026-08-03                                                                                                                                 |
| Origin     | a service can carry exactly one interface today, which makes the coherence group and the generation unit indistinguishable from a service  |
| Scope      | the `ServiceDef` grammar change, identity and ordering, addressing, three diagnostics, and five diff categories                            |
| Companions | `2026-08-03-rpc-response-bound-design.md` §11.1 records the decision; `2026-08-03-schema-projection-design.md` §2.1 supplies the numbering |

A bare section reference — §11, §14.5, Appendix B — is to the **ridl Language
Reference**. References to this document are marked _above_ or _below_;
companion notes are named in full.

## 1. Why

`ServiceDef` admits exactly one named shape or one inline shape. Three
consequences of lifting that restriction, in descending order of weight.

**It gives §17.3 its grouping construct for free.** The response-bound note
makes coherence a property of a provided interface and the schema-projection
note makes the interface the generation unit — one store, one dispatcher. At 1:1
those grains are indistinguishable from "the service", so neither claim can be
exercised. With several interfaces per service, the interface becomes the group
that §17.3 open question 3 was looking for, with no new syntax invented for it.

**It is a better answer to §17.2 than the candidate recorded there.** §17.2
notes that flat interfaces duplicate recurring patterns — the heartbeat, version
and diagnostics triad — and names compile-time mixins as the candidate. Mixins
_flatten_, and flattening is exactly what ridl's identity model cannot absorb:
ordinals are per-interface (§11), so folding one shared block into three
interfaces gives its interactions three unrelated ordinal sets, and editing the
block renumbers all three. Composition leaves each interface's ordinal space
intact and independent of what it sits beside.

**It supplies the middle rung of the identity chain.** The schema-projection
note §2 has ordinals local to an interface (owned by ridl), a service number
global to the addressing domain (owned by deployment, and still underived), and
nothing in between. An interface's slot within its service is exactly as local
as an interaction's ordinal within its interface, so the same argument that
keeps ordinals in ridl puts interface ids there too.

## 2. The grammar change

```ungram
ServiceDef =
  'service' name:DottedName
  ( ':' shapes:ServiceShape (',' shapes:ServiceShape)* ','?
  | '{' (inline_members:InterfaceMember ','?)* '}' )

ServiceShape =
  PathType
| ReservedEntry
```

```ridl
service veh.adas.cruise : CruiseControl, DiagBlock

service veh.body.doors :
  DoorControl,
  DiagBlock,
  reserved LegacyDoorDiag,
  HealthBlock
```

### 2.1 `ServiceShape` mirrors `InterfaceMember`

An interface body is already a list whose alternatives include `ReservedEntry`,
and `ReservedEntry = 'reserved' (Name | Literal)` is a shared node that typl
also uses for struct and union tombstones. Making a service's shape list the
same shape one level up means **service-level `reserved` needs no new syntax at
all** — the tombstone problem is solved by reusing the mechanism rather than
inventing a second one.

### 2.2 Commas are required between shapes

This is the one place the change should diverge from the family's separator
convention, and the reason is structural.

Everywhere else the family writes `(X ','?)*` — optional commas — because a
closing `}` terminates the list. **This list has no terminator.** It ends where
the next declaration begins.

It still parses: every top-level declaration starts with a keyword, and a bare
`CamelCase` identifier never does, so a newline-separated continuation is
decidable. But it parses **greedily**, and that is the problem. A mistyped
declaration on a following line — `Struct Foo {` for `struct Foo {` — is a
`CamelCase` identifier, so it is absorbed as another service shape, and the
error surfaces at the `{` with no connection to the mistake. Requiring a comma
between shapes removes the hazard at no cost to readability; a trailing comma
stays optional.

### 2.3 Named shapes or one inline shape, never both

Mixing raises "which slot holds the inline shape" for no gain. Keeping the
either/or also keeps `ServiceDef` a two-branch alternation rather than a
three-branch one.

The migration path is not free, and §3.3 below says why — but it is a one-time
cost, not a recurring one.

### 2.4 What the change costs beyond the grammar

`ServiceDef::interface_ref()` becomes a plural accessor over `ServiceShape`,
which means the typed-AST layer is regenerated with `cargo xtask codegen`; a
drift test enforces that the generated layer matches the grammar. Every consumer
that reads a service's shape — the checker, the IR lowering, both backends, and
`ridl-diff`'s walk — moves from one reference to a list.

## 3. Identity and ordering

### 3.1 The rules

Interface ids follow §11's model exactly, one level up:

- **1-based by declaration order**, matching interaction ordinals and proto
  field numbers.
- **Append-only.** Adding a shape at the end appends; inserting or reordering
  shifts ids and is breaking; removing one requires a tombstone to hold its
  slot.
- **An inline shape is slot 1**, which makes the inline form a degenerate case
  of the general one rather than a separate construct.

### 3.2 Transport identity stays per interface, keyed by name

Renumbering interactions across a service was rejected outright: an interface's
wire identity would then depend on what else the service happens to carry, which
is the exact coupling §14.1 rejected inheritance to avoid. Each interface keeps
its own ordinal space and the binding keeps the spaces apart.

Appendix B already points this way, mapping a SOME/IP eventgroup to an
interface, so a multi-interface service maps to several transport-level
groupings under one logical name. Keying on the interface **name** rather than
its list position also makes reordering the list invisible to transport identity
— which is a property worth having, though §6 below still classifies a reorder
as breaking, because the _id_ moves even when the transport key does not.

### 3.3 Extraction is breaking, and that is not a defect of this design

Making the inline shape slot 1 preserves numbering across "extract the inline
shape into a named interface". It does **not** make that refactor compatible,
and the schema-projection note §2.1 carries the correction.

ADR-0008 decision 4 derives a fallible return's transport identity from the
enclosing **interface name** plus the ordinal plus both arms —
`fallible_transport_identity` renders it `I#1:CalReport|CalError` — and an
inline shape, whose `Interface.name` is empty by construction, uses the
service's dotted name instead. Extraction therefore rewrites the identity of
every fallible query in the shape. `ridl-diff` already classifies a switch
between the two forms as breaking, and it is right to.

Making extraction compatible would mean changing how that identity is derived.
That is a wire-identity decision in its own right and must not be taken as a
side effect of this one.

## 4. Addressing stays flat

Members remain `service.member`, as §14.5 has it. A member name duplicated
across a service's interfaces is a compile error (§5.1 below).

This keeps the property §14.5 calls the point — that a dotted member name is an
unambiguous system-wide address — and leaves every address written today valid.
The alternative, growing a segment to `service.Interface.member`, composes
unconditionally but changes the shape of every existing address and abandons the
flat namespace the catalog is built on.

The accepted cost is real and should be stated rather than discovered: **two
independently written interfaces that share a member name cannot be composed
into one service without renaming one of them.** For the recurring-block case
§17.2 describes — heartbeat, version, diagnostics — that is unlikely to bite,
because such blocks are written to be composed. It will bite when two
general-purpose interfaces both declare something like `status`.

## 5. Diagnostics

The service codes occupy 140 to 143 (ADR-0008 decision 21 records the 1xx-band
placement as a documented anomaly), and RIDL-112 is minted by the response-bound
note, so the free codes here begin at **144**.

### 5.1 RIDL-144 — duplicate member name across a service's interfaces

Error. Two interfaces of one service both declaring `status` would give
`service.status` two referents, which §14.5's flat addressing cannot express.

### 5.2 RIDL-145 — the same interface named twice in one service

Error, and worth its own code rather than falling through to RIDL-144. Listing a
shape twice makes _every_ member collide, so RIDL-144 alone would emit one
diagnostic per member and bury the actual mistake. One clear error naming the
repeated shape is the better report.

### 5.3 RIDL-146 — an interface re-declared under a service-level reserved name

Error. The analogue of RIDL-401 — "interaction re-declared under a `reserved`
name" — one level up. A tombstone retires a name permanently, at the service
level as inside an interface body.

### 5.4 Two existing codes become per-element

RIDL-141 ("`service` names a type that is not an `interface`, and has no inline
shape") and RIDL-143 ("`service` publishes an `internal` interface") both apply
per shape in the list rather than to a single reference. Neither rule changes;
the span each reports against does.

## 6. Diff categories

Five new categories, mirroring the interaction set one level up. The verdicts
are **inherited, not invented** — each is the service-level reading of the rule
`classify.rs` already applies to interactions.

| Category                | Verdict                                                                                                                             |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| `ServiceShapeAppended`  | compatible when the slot it takes was never occupied; breaking when the slot was freed by an untombstoned removal and is now reused |
| `ServiceShapeInserted`  | breaking always — every later id shifts                                                                                             |
| `ServiceShapeReordered` | breaking always — ids move                                                                                                          |
| `ServiceShapeRemoved`   | breaking always — the freed slot becomes reusable, so the id is no longer permanent                                                 |
| `ServiceShapeRetired`   | compatible always — the sanctioned retirement: the slot stays occupied and every later id holds                                     |

The last row is the one to read twice. `InteractionRetired` is classified
"compatible always" today, on the reasoning that `ridl-diff` judges **wire
identity**, not source-level API surface: a consumer of the retired member
breaks at compile time, but the identity model is intact and every other
member's wire position holds. The service level inherits that reading unchanged.

**Distinct categories rather than branching inside `ServiceChanged`.** ADR-0012
decision 9 rules that an unclassified change is "classified breaking, never
compatible", and the same reasoning applied to `RpcBoundChanged` in the
response-bound note applies here: a missed branch inside an existing category
fails by reporting a verdict inherited from a different rule, while a missing
arm over a new variant is a compile error, because three matches over `Category`
deny `clippy::wildcard_enum_match_arm`.

`ServiceChanged` narrows accordingly. Its present rule covers "a changed
`interface_ref`, or a switch between an interface reference and an inline
shape"; the first half is superseded by the five categories above, and the
second half stays — a switch between the two forms remains breaking, for the
reason §3.3 above gives.

## 7. Open

1. **Whether a service-level tombstone should be checkable against history.**
   RIDL-146 stops a retired name being re-declared, but nothing verifies that a
   `reserved` entry names an interface the service ever carried. typl's
   tombstones are unchecked in the same way, so the inconsistency would be
   introduced rather than inherited if this one were checked.
2. **Whether the shape list should admit a visibility modifier per element.** A
   service is always public (§14.5) and RIDL-143 rejects publishing an
   `internal` interface, so the answer is probably no — but composition is where
   the question first has a reason to be asked.
3. **How a multi-interface service renders in `ridl doc`** (E4.1). One service
   with several contracts is a different table shape from one service with one,
   and the doc target has not been designed against it.
