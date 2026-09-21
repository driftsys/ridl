# ADR-0019 — The FlatBuffers projection: the union wrapper, the table form, and the target's own scopes

## Status

Accepted — 2026-08-09. Scope: seven rules the second wire backend needed and no
earlier record supplied — where a typl union lives when the target's native
union owns two id slots, what a union arm that is not itself a table becomes,
which of `table` and `struct` a typl struct projects to, what a map becomes on a
target with no map type, whose name scopes the collision guard models, what
default an enum-typed field carries when its enum declares no zero member, and
what happens to a name that reaches a word the validity oracle reserves. All
seven are FlatBuffers-scoped; none binds another backend.

**Amended 2026-09-21 — decision 8, the root rule.** An eighth decision was added
at the end of E11.7's implementation, from the disposition on driftsys/ridl#470:
every declaration has a root table, and a named scalar, an enum and an enum set
are rooted in a box table. It answers a question this record did not ask —
nothing here stated what a payload's root is — and it changes what the schema
emitter writes for those three kinds, so every emitted schema gained tables.
Decisions 1 to 7 are unchanged; decision 5's list of generated names gained
decision 8's box, and Open item 3 records the one thing decision 8 deliberately
did not settle. Decision 8 is FlatBuffers-scoped like the other seven.

Written from roadmap story E9.9, which built `crates/ridl-backend-flatbuffers`.
The reasoning trail is
[`docs/archive/2026-08-08-flatbuffers-projection-design.md`](../archive/2026-08-08-flatbuffers-projection-design.md)
(archived at story close, the way E9.8's pair was); read the design note as a
design, not as a description of what shipped — decision 2 below records a rule
the note did not state and execution had to.

Every structural claim below was verified against `flatc` 25.12.19 and `planus`
1.3.0 rather than reasoned from the FlatBuffers documentation. Where a record
and a compiler disagreed, the compiler won: decision 1 supersedes the remedy the
schema-projection note §4.4 prescribed, and decision 3 amends typl Appendix D's
`struct` allowance for this projection.

This ADR was accepted under the delegated authority recorded in
[ADR-0005](ADR-0005-agent-enablement.md)'s working model.

It does not supersede [ADR-0013](ADR-0013-codegen-backend-scope.md),
[ADR-0016](ADR-0016-schema-projection-and-the-name-transform.md) or
[ADR-0017](ADR-0017-proto3-projection-rules.md). ADR-0013 fixes what a wire
backend may emit (its decision 6, amended in place, records how E9.9 closed the
width-floor precondition), ADR-0016 fixes how identity projects, ADR-0017 fixes
the foreign-reference and name-totality rules this backend inherits, and this
record fills the gaps that are properties of one target.
[ADR-0018](ADR-0018-runtime-core-and-generated-surface.md), accepted the same
day, is orthogonal to this record: its decision 18 resolves the ADR-0013
decision 2 versus ADR-0016 decision 10 conflict over the `service` block, and
its decision 16 moves the store and dispatcher into Epic 11 — both bear on where
the questions this record leaves open are answered, not on the projection rules
below.

## Context

ADR-0016 decision 6 ratified four properties every projection must satisfy —
deterministic, total, stable under compatible change, injective in scope — and
ADR-0017 decision 4 extended totality from numbers to names, obliging a backend
that projects onto a namespaced target to model that target's scopes. E9.9 built
the second wire backend, onto a target whose constructs differ from proto3's in
ways those records could not anticipate, and six questions arose that no record
answered — or answered wrongly.

**A native union breaks the id mapping.** ADR-0016's identity chain requires
`id = ordinal − 1`, contiguous. A FlatBuffers union field is sugar for two
fields — a hidden `_type` discriminant and the value — so a union declared
`(id: N)` owns ids `N−1` and `N`, and one placed in an ordinal-owned slot shifts
every later field. The schema-projection note §4.4 saw this and prescribed a
remedy — a hand-rolled table carrying a discriminant and the arms — that
measurement disproved.

**A union arm need not be a table, but a FlatBuffers union member must be.**
typl §10 permits any named type as an arm, and a named scalar is a named type; a
FlatBuffers union member must be a table. No record said what happens to a named
scalar, enum or enum set arm, and the first implementation refused them.

**typl Appendix D permits a form the projection contract cannot hold.** It
allows a struct whose fields are all fixed-width and non-optional to be emitted
as a FlatBuffers `struct` — inline, zero indirection — and the IR carries
`fixed_layout` for it. Measurement showed the form fabricates a value after a
compatible append.

**FlatBuffers has no map type, and the idiom carries an attribute with an
unchecked obligation.** The vector-of-entry-tables idiom optionally marks one
field `(key)`, which obliges the producer to sort and generates a binary search.

**FlatBuffers' name scopes are not proto3's.** The proto backend's collision
guard registers enum values in its package scope, because proto3 scopes them as
siblings of their enum. FlatBuffers does not.

**Every table field carries a default, and an enum need not declare zero.**
`flatc` refuses a field whose implicit default of 0 is not a member of its enum,
and refuses `required` on any scalar or enum field, so some rendering had to be
chosen for a field typed by a zero-less enum.

A seventh question arrived at the branch's final review: **the validity oracle's
grammar is narrower than the reference compiler's.** `planus` reserves nine
words that `flatc` treats as contextual identifiers, an emitted name can reach
one through five positions, and the pinned transform can manufacture one from a
name that never contained it. No record said whether such a schema is emitted or
refused.

## Decision

1. **A union is isolated in a wrapper table holding a native union.** The
   declared name goes to the wrapper —
   `table <Name> { value: <Name>Union
   (id: 1); }` — because the wrapper is
   what a field position references. The wrapper occupies one slot in its
   parent, so `id = ordinal − 1` holds; the union's two slots live in the
   wrapper's own id space, where contiguity binds nothing else. FlatBuffers
   maintains the discriminant, generates the typed accessors, and its verifier
   checks the pairing. The union list uses alias syntax — `member: Type`, the
   member name being the typl arm name through the pinned transform — which both
   compilers accept and which keeps two arms sharing one type distinct.

   This **supersedes the schema-projection note §4.4**, which drew the right
   conclusion — never a native union in a slot the ordinal mapping owns — and
   over-specified the remedy as a hand-rolled table carrying a discriminant and
   the arms. That form cannot hold typl §10's guarantee that exactly one arm is
   active: `flatc` accepts a payload whose discriminant names one arm while a
   different arm is set, and one that claims a type with no arm set at all —
   both well-formed buffers a consumer mis-reads. Emitting a construct that
   cannot enforce a guarantee the contract makes is what ADR-0013 decision 2
   forbids. Wire cost is identical either way, measured at 80 bytes for the same
   payload; a native union in an ordinal-owned slot is refused by `flatc`
   outright ("field id's must be consecutive from 0").

2. **A union arm that is not itself a table is wrapped in a generated table.** A
   named scalar, enum or enum set arm becomes
   `table <Union><Arm>Box { value: <resolved type> (id: 0); }` — the same idiom
   decision 1 uses to isolate the union itself — with the wrapped field resolved
   exactly as an ordinary field would be: its scalar or qualified name, its
   `= null` default when decision 6 calls for one, its constraint comment. A
   struct or union arm references its own table directly and gets no box.

   This was decided mid-implementation, **reversing an earlier instruction to
   refuse such arms**. The reversal was right because refusing made a large
   class of legal typl unprojectable: typl §10 permits any named type as an arm,
   a named scalar is a named type, and the target represents the arm fine with
   one more table — so the refusal rejected contracts the target can carry,
   which fails ADR-0016 decision 6's totality property in the direction no
   diagnostic can excuse. Verified in both `flatc` 25.12.19 and `planus` 1.3.0.

3. **A struct is always emitted as a `table`, never as a FlatBuffers `struct`.**
   This **amends typl Appendix D**, which permits the `struct` form — inline,
   zero indirection — for a struct whose fields are all fixed-width and
   non-optional. A FlatBuffers `struct` has a fixed inline layout and no vtable,
   so a reader with a newer schema reads past what an older writer wrote.
   Appending a struct field is a compatible change in typl, and measurement
   shows what follows: v1 data read with the v2 schema returns the appended
   field fabricated from padding under `struct`, and correctly absent under
   `table`. The `struct` form does not reject the buffer; it invents a value —
   which makes ADR-0016 decision 6 property 3 unsatisfiable on the construct,
   silently. The `fixed_layout` flag stays in the IR for a target where a fixed
   layout is safe.

4. **A map is a vector of generated entry tables, with no `(key)`.** FlatBuffers
   has no map type; each map field emits
   `table <Owner><Field>Entry { key: K (id: 0); value: V (id: 1); }` and the
   field becomes a vector of it. The `(key)` attribute is not emitted: it
   obliges the producer to write the vector sorted by that field and nothing
   checks the obligation at read time — an unsorted vector with `(key)` makes
   `LookupByKey` return the wrong element silently — while typl §12.2 gives a
   map no ordering at all, so the schema would assert an ordering the contract
   never states. `planus` also cannot parse the attribute, so emitting it would
   put the map path outside the validity oracle.

5. **The collision guard models FlatBuffers' own name scopes, and the proto
   backend's `SymbolScope` is deliberately not reused.** Verified against
   `flatc`, the scopes are three: the **namespace** holds every table, struct,
   enum and union name in one shared space; a **table** holds its own field
   names; an **enum** holds its own value names. The third row is where
   FlatBuffers differs from proto3, which scopes enum values as siblings of
   their enum — two FlatBuffers enums may each declare a value named `OK`, so
   **no value prefixing is emitted and none is needed**, where the proto backend
   prefixes every value. Reusing `SymbolScope` would over-refuse, because its
   package scope registers enum values. What the guard must still catch is a
   **generated** name colliding with a declared one — the wrapper, box and entry
   tables of decisions 1, 2, 4 and 8 and the identity-table enums all mint names
   into the namespace scope, and `flatc` rejects a duplicate with "datatype
   already exists" (ADR-0017 decision 4's obligation, discharged over this
   target's scopes).

6. **An enum-typed field takes `= null` when its enum declares no zero member.**
   Every FlatBuffers table field carries a default, and `flatc` refuses the bare
   form when the implicit default of 0 is not a member of the enum ("default
   value of `0` for field `g` is not part of enum `Gear`"). The alternative —
   defaulting to some declared value, say the lowest — would make a truncated or
   malformed buffer read silently as that value, fabricating a reading the
   contract never stated; `= null` never fabricates, because a missing value
   reads as missing. This applies whether or not the typl field is optional:
   FlatBuffers cannot mark a scalar or enum field `required` in any case ("only
   non-scalar fields in tables may be 'required'"), so the requiredness was
   already not representable on this target, and surfacing absence beats
   inventing a value.

7. **A name that reaches a word the validity oracle reserves is emitted as-is,
   never refused.** `planus` 1.3.0 reserves nine words that `flatc` 25.12.19
   treats as contextual identifiers — `table`, `namespace`, `attribute`,
   `include`, `root_type`, `rpc_service`, `file_extension`, `file_identifier`,
   `native_include`. They reach an emitted schema through five positions — a
   table field name, a union arm alias, an enum value name, a declared type
   name, and a namespace segment — and the pinned transform (ADR-0016 decision
   2) can manufacture one: a field written `rootType : integer [0..10]` emits
   `root_type`. Measured in all five positions: `flatc` accepts every one of the
   nine, and `planus` rejects each with "unrecognized token".

   The emitted schema is therefore valid FlatBuffers; it is the oracle that is
   narrower than the reference compiler. Refusing would let a test dependency
   constrain what the language can express — rejecting a contract over a field
   the author spelled `rootType` — which fails ADR-0016 decision 6's totality
   property the same way the refusal decision 2 reversed did. No renaming or
   escaping is applied either: the pinned transform is the whole of the name
   projection (ADR-0016 decision 2), and a target-specific escape would fork it.
   The cost this accepts is recorded under Consequences: such a name is emitted
   and is not checked by the oracle, and a consumer whose toolchain is
   planus-based cannot compile the schema. Ruled by the repository owner at the
   branch's final review.

8. **Every declaration has a root table, and a named scalar, an enum and an enum
   set are rooted in a box** — added 2026-09-21, from the disposition on
   driftsys/ridl#470, at the end of E11.7's implementation. A struct's root is
   its own table (decision 3); a union's is its wrapper table (decision 1); a
   named scalar, an enum and an enum set are rooted in
   `table <Name>Box { value: <resolved type> (id: 0); }`, the box idiom decision
   2 already uses for a non-table union arm, with the wrapped field resolved
   exactly as an ordinary field would be — its scalar or qualified name, its
   `= null` default when decision 6 calls for one, its constraint comment. The
   `.fbs` emitter writes the box for every such declaration, whether or not an
   interaction carries it. The bound of such a root is the box table's, charged
   as decision 2's arm box is. **No `root_type` is written**, as none was
   before. `<Name>Box` joins the generated names decision 5 checks against
   declared ones.

   _On the numbering._ The disposition on driftsys/ridl#470 said the box joins
   the names "decision 7 checks against declared ones". Decision 7 checks
   nothing — it is the rule that a name reaching a `planus` reserved word is
   emitted as-is and never refused. The collision guard, and the only decision
   that checks a generated name against a declared one, is decision 5, so that
   is what this names.

   The question this answers is the one the codec could not get past: a
   FlatBuffers root is a table, and a named scalar or an enum as an interaction
   payload is idiomatic ridl — `signal target: Speed`,
   `command setLever(cmd: LeverCmd)` — so a payload with no root table has no
   `Payload<FlatBuffers>` to name. This record stated no root rule at all
   before, and a wrapper only the Rust codec knew about would break the
   agreement between the schema and the codec that the whole design rests on.

   Three alternatives were rejected. **A box only for the declarations an
   interaction carries** makes the projection context-sensitive: adding a signal
   would add a table to the schema and an implementation to a type that had
   none, where ADR-0016 decision 6 asks every projection to be total over the IR
   — and an unused box table costs nothing. **An induced per-interaction
   argument struct as the root** changes the face's payload types, which is an
   ADR-0023 change, and boxes one scalar once per use; the induced argument
   struct for a multi-parameter call is a separate parked follow-up, and an
   induced struct is a struct, so it composes with this rule when it comes. **A
   raw scalar root** does not exist: FlatBuffers has no root that is not a
   table, the `planus` oracle could not read one, and the conformance suite
   would have nothing to compare.

   The same question binds E11.8 (proto3) and E11.12 (`repr(C)`); each answers
   it in its own projection record in its own terms, and nothing here decides
   for them.

## Alternatives considered

| Candidate                                                         | Verdict    | Reason                                                                                                                                                                                                           |
| ----------------------------------------------------------------- | ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A native union in the ordinal-owned slot                          | rejected   | a union field owns two id slots, shifting every later field; `flatc` refuses the schema ("field id's must be consecutive from 0")                                                                                |
| A hand-rolled discriminant-plus-arms table (note §4.4's remedy)   | superseded | cannot hold typl §10's exactly-one-arm guarantee — `flatc` accepts a discriminant naming one arm while another is set, and one with no arm set — and saves nothing: wire cost measured identical (80 bytes both) |
| Refusing a named scalar, enum or enum set union arm               | reversed   | typl §10 permits any named type as an arm; the refusal made legal typl unprojectable where the target represents it fine with one more table (decision 2)                                                        |
| The FlatBuffers `struct` form for a `fixed_layout` struct         | rejected   | after a compatible field append, v1 data read with the v2 schema returns the appended field fabricated from padding — ADR-0016 decision 6 property 3 fails silently                                              |
| Emitting `(key)` on the map entry's key field                     | deferred   | obliges the producer to sort, unchecked at read time, asserting an ordering typl §12.2 never states; `planus` cannot parse it — parked, not reopenable in a named story (see Open item 1, amended 2026-09-21)    |
| Lifting `ridl-backend-proto`'s `SymbolScope`                      | rejected   | its package scope registers enum values, because proto3 scopes them as namespace siblings; FlatBuffers scopes them inside the enum, so the lift would over-refuse                                                |
| Defaulting a zero-less enum field to its lowest declared value    | rejected   | a truncated or malformed buffer would read silently as that value — a fabricated reading, where `= null` surfaces absence as absence                                                                             |
| Refusing or escaping a name that reaches a `planus` reserved word | rejected   | the schema is valid FlatBuffers — `flatc` accepts all nine words — so a refusal would let a test dependency constrain the language, and an escape would fork the pinned transform (decision 7)                   |
| A box table only for the declarations an interaction carries      | rejected   | a projection that depends on which interactions exist is context-sensitive: adding a signal would add a table and an implementation to a type that had none, against ADR-0016 decision 6's totality (decision 8) |
| An induced per-interaction argument struct as the payload's root  | rejected   | it changes the face's payload types, which is an ADR-0023 change, and boxes one scalar once per use; an induced struct is a struct, so it composes with decision 8 if it ever comes                              |
| A raw scalar root, with no table at all                           | rejected   | FlatBuffers has no root that is not a table; `planus` could not read one and the conformance suite would have nothing to compare (decision 8)                                                                    |

## Consequences

- **Positive — every emitted construct can hold the guarantee it implies.** The
  wrapper's pairing of discriminant and value is maintained and verified by the
  target itself (decision 1); an appended field reads as absent rather than as a
  value invented from padding (decision 3); a missing enum value reads as
  missing (decision 6).
- **Positive — `planus` validates every emitted construct.** Its two parsing
  gaps against `flatc` at the construct level — `(key)` and fixed-length arrays
  — both fall outside what this projection emits, by decision 4 and decision 3
  respectively. The oracle's remaining gap is in the identifier dimension, not
  the construct dimension: a name that reaches one of the nine words decision 7
  lists is emitted, accepted by `flatc`, and not checked by `planus`.
- **Negative — a schema whose names reach a `planus` reserved word is valid but
  outside the oracle, and a planus-based consumer cannot compile it.** Decision
  7 keeps the projection total; the price is that the oracle does not cover such
  a schema, and interoperating with a consumer that generates its code with
  `planus` requires avoiding the nine words in the five positions decision 7
  lists — including a name the transform manufactures, such as `rootType`.
- **Negative — indirection is paid for evolvability.** Every union costs a
  wrapper table, every non-table arm costs a box, and every struct pays the
  vtable and one indirection the `struct` form would have avoided. That is the
  price of remaining evolvable under compatible change, which is what the
  projection contract exists to protect.
- **Negative — a required enum field reads as optional in generated code.**
  Decision 6's `= null` renders absence where the contract states requiredness —
  a faithfulness loss, though not a new one: the target cannot express a
  required scalar or enum field at all.
- **Negative — every schema over a package that declares one of the three kinds
  gained tables, and a box table is emitted that nothing may reference.**
  Decision 8 adds one table per named scalar, per enum and per enum set. A
  schema over a package that declares none of them is unchanged, which is why
  one of this repository's three `.fbs` snapshots did not move. The earlier
  stage constraint that no FlatBuffers snapshot may move was a constraint on one
  stage, not a standing rule; every emitted schema that has a snapshot is
  compiled by the `planus` oracle. An unreferenced box table costs nothing on
  the wire, because nothing writes it unless it is a root.
- **Negative — decision 8 gives the default-elision divergence a root-level
  reach.** A conforming FlatBuffers writer omits a field equal to its declared
  default, and a box's `value` is a field like any other. At a field position
  that costs one field, which
  [`../design/flatbuffers-codec.md`](../design/flatbuffers-codec.md) records as
  driftsys/ridl#472; at a **root** it costs the whole payload, because the
  payload is that one field. A box carrying a scalar zero, an enum at its zero
  member or an empty string, written by another implementation, is a buffer this
  projection's codec refuses. Deciding otherwise is #472's, not decision 8's: it
  is the same question, met at a position where it is harder to ignore.
- **Negative — a map lookup is linear.** With no `(key)` there is no generated
  `LookupByKey` and no binary search. The Open item names the story that may
  restore it.
- **Neutral — two scope models exist by design, not by drift.** The proto
  backend's `SymbolScope` and this backend's `Namespace` each model their own
  target's scopes, and merging them would misdescribe one target or the other.

## Open

1. **`(key)` and sorted-vector lookup, in E11.7.** A generated producer could
   hold the sortedness obligation the attribute implies, which is what decision
   4 found missing — that producer is the FlatBuffers codec, story E11.7, since
   [ADR-0018](ADR-0018-runtime-core-and-generated-surface.md) decision 16 moved
   E9.11's scope into Epic 11. Taking it would also want a contract-level
   statement that the map is ordered — which typl does not have — and a validity
   oracle that parses the attribute, which `planus` today does not.

   **Amended 2026-09-21, at the end of E11.7's implementation.** **E11.7 did not
   take it, and this item no longer names a story.** The codec landed and writes
   a map as a vector of entry tables with no `(key)`, with entries in the order
   of the generated `Vec<(K, V)>` and nothing sorting — decision 4 as written.
   **The ground is the language, and it has not moved: typl states no ordering
   on a map (§12.2).** Emitting `(key)` would assert an ordering the contract
   does not make, and oblige every producer to sort for a guarantee no typl
   declaration asks for. That is independent of any tool and is sufficient on
   its own.

   A practical note, subordinate to it and deliberately **not** the reason: this
   repository's validity oracle and its conformance round trip both run on
   `planus`, which parses no `(key)`
   ([`../design/flatbuffers-codec.md`](../design/flatbuffers-codec.md)), so
   emitting the attribute today would cost both. This must not become the ground
   — it is the reasoning the alternatives table above rejects for the
   reserved-word question, where a refusal would let a test dependency constrain
   the language, and it evaporates the moment planus gains support or a second
   oracle arrives.

   The item is parked rather than closed, and reopens on either of two
   observations: a consumer needs keyed lookup into a map without decoding it,
   which nothing in the roadmap asks for; or the oracle changes — planus gaining
   `(key)`, or a second implementation joining it — which removes the practical
   note and leaves the language ground to be answered on its own. Whoever
   reopens it files the story then.
2. **The `fixed_layout` flag has no consumer.** Decision 3 leaves it in the IR
   for a target where a fixed layout is safe — one whose schema evolution is
   closed, or a deployment that pins both schema versions. The first backend
   that takes the `struct` form owes its own record of when the form's
   fabrication hazard does not apply.
3. **Whether a non-table union arm should reference `<Name>Box`.** Decision 8
   gives a named scalar, an enum and an enum set a box named `<Name>Box`, and
   decision 2 gives a non-table union arm a box named `<Union><Arm>Box`. The two
   boxes have the same shape, so one type now has two of them wherever a union
   carries it as an arm. Decision 8 deliberately did not change decision 2: the
   arm box's name and shape are what every merged FlatBuffers snapshot and every
   emitted discriminant already carry, and collapsing the two is a wire change
   rather than a tidying. Whoever reopens it decides whether one type has one
   box, and files the story then.

## References

- [`docs/archive/2026-08-08-flatbuffers-projection-design.md`](../archive/2026-08-08-flatbuffers-projection-design.md)
  — the reasoning trail, including the measurements each decision cites; read as
  a design, not as what shipped
- [ADR-0013](ADR-0013-codegen-backend-scope.md) — the emit ceiling (decision 2),
  the transport width layer (decision 4), and decision 6's width-floor
  precondition, whose closure by E9.9 is recorded there as an amendment
- [ADR-0016](ADR-0016-schema-projection-and-the-name-transform.md) — decision 6
  (the four projection properties decisions 2 and 3 turn on)
- [ADR-0017](ADR-0017-proto3-projection-rules.md) — decision 1 (the
  `generate_with` API and the inlining rule this backend inherits), decision 4
  (the name-totality obligation decision 5 discharges over this target's scopes)
- `docs/specification/typl-language-reference.md` — §10 (unions and the
  exactly-one-arm guarantee), §12.2 (maps carry no ordering), Appendix D (the
  transport width table, and the `struct` allowance decision 3 amends)
- [`docs/ROADMAP.md`](../ROADMAP.md) — E9.9 (the story this record comes from),
  E11.7 (the FlatBuffers codec, where the Open item 1 question reopens —
  ADR-0018 decision 16 moved E9.11's store and dispatcher into Epic 11)
- `crates/ridl-backend-flatbuffers/src/lib.rs` — the implementation
