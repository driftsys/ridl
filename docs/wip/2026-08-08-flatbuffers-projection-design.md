# The FlatBuffers Projection — the second wire backend

Roadmap story E9.9. Written 2026-08-08, after E9.8 shipped the proto3 backend
and ADR-0017 fixed the rules that bind every backend projecting onto a
namespaced target.

Every structural claim in this note was verified against **`flatc` 25.12.19**
and **`planus` 1.3.0** rather than reasoned from the specification. Where a
record and a compiler disagreed, the compiler won and the record is amended
below.

Throughout, "the note" is
[`2026-08-03-schema-projection-design.md`](2026-08-03-schema-projection-design.md),
and a reference of the form note §4.3 is to it.

## 1. What is already decided, and by what

| Question                      | Answer                                                      | Fixed by                     |
| ----------------------------- | ----------------------------------------------------------- | ---------------------------- |
| What may a wire backend emit? | two tiers — the typl surface, and the identity table        | ADR-0013 decision 2          |
| Field id                      | `id = ordinal − 1`, contiguous                              | note §4.3                    |
| Retired slots                 | a `(deprecated)` field holding the slot                     | note §4.3                    |
| Widths                        | the full `uint8..uint64` palette — the narrow width is real | typl Appendix D, ADR-0013 d4 |
| A foreign named scalar        | inlines; no import for it                                   | ADR-0017 decision 1          |
| Totality                      | over names as well as numbers                               | ADR-0017 decision 4          |
| The API                       | `generate_with(package, others)`                            | ADR-0017 decision 1          |

Five decisions are taken here: the union remedy (§3.1), the struct remedy
(§3.2), the map remedy (§3.3), the width-floor closure (§4), and the validity
oracle (§5). Execution added a sixth — a union arm that is not itself a table is
boxed rather than refused — recorded as an amendment inside §3.1.

## 2. Scope — the same ceiling E9.8 held

Tier 1 and tier 2 only. No `rpc_service`, no reply carriers, no store. The
conflict between ADR-0013 decision 2 and ADR-0016 decision 10 over whether a
wire backend emits a service construct is **still unresolved** and still belongs
to E9.11, which owns the dispatcher; both records carry a Status note saying so.
E9.9 avoids it the same way E9.8 did, by emitting neither.

**Which ids the mapping actually governs here.** Note §4.3 states
`id = ordinal − 1` over a store table, whose ordinals span all five interaction
kinds and are therefore sparse — which is what its gap-filling with
`(deprecated)` placeholders exists for. **E9.9 emits no store**, so no
interaction-keyed table exists in this story. The ids this story assigns are
**struct field ordinals and union arm ordinals** (typl §7.4), which are
declaration-order and already contiguous with tombstones counted. So a
`(deprecated)` placeholder is needed here only for a **tombstone**, never for a
kind that does not belong in the table — the sparse case arrives with the store,
in E9.11, and §4.3's gap-filling rule is inherited unused until then.

## 3. Tier 1 — three remedies, each forced by a measurement

### 3.1 A union is isolated in a wrapper table, not hand-rolled

A FlatBuffers union field is sugar for **two** fields: a hidden `_type`
discriminant plus the value. A union declared `(id: N)` owns ids `N-1` and `N`.
Since note §4.3 requires ids contiguous from 0 and `id = ordinal − 1`, a native
union placed in an ordinal-owned slot shifts every later field. `flatc` refuses
it outright:

```text
error: field id's must be consecutive from 0, id 1 missing or set twice,
field: payload_type, id: 0
```

Note §4.4 draws the right conclusion — never a native union in a slot the
ordinal mapping owns — and then over-specifies the remedy as "a table carrying a
discriminant and the arms". **That remedy is withdrawn.** A hand-rolled
discriminant cannot hold typl §10's guarantee that exactly one arm is active,
and `flatc` accepts both of these:

- `kind = SPEED` while `gear_index` is the field actually set — the discriminant
  lies about the payload;
- `kind = SPEED` with no arm set at all — a payload claiming a type it does not
  carry.

Both are well-formed FlatBuffers that a consumer mis-reads. Emitting a construct
that cannot enforce a guarantee the contract makes is the mirror of what
ADR-0013 decision 2 forbids.

**The remedy is a wrapper table holding a native union.** The wrapper occupies
one slot in the parent, so `id = ordinal − 1` holds; the union's two slots live
in the wrapper's own id space, where contiguity binds nothing else. FlatBuffers
maintains the discriminant, generates the typed accessors, and its verifier
checks the pairing. Verified with three unions among five interactions: the
mapping holds throughout. Wire cost is identical to the hand-rolled form —
measured at 80 bytes for the same payload either way.

```fbs
table PayloadBox { value: PayloadUnion (id: 1); }   // type at 0, value at 1

table VehicleStatusState {
  current_speed: double     (id: 0);   // ordinal 1
  payload:       PayloadBox (id: 1);   // ordinal 2 — one slot
  engine_temp:   double     (id: 2);   // ordinal 3 — mapping intact
}
```

**Amendment from execution (Task 7) — a union arm that is not itself a table is
wrapped in a generated table.** This note as first written did not discuss arm
kinds at all, and the plan built from it instructed the backend to refuse an arm
typed by a named scalar, an enum or an enum set — a FlatBuffers union member
must be a table, and none of the three has one. That instruction is
**reversed**: such an arm becomes
`table <Union><Arm>Box { value: <resolved
type> (id: 0); }`, the same idiom this
section already uses to isolate the union itself, with the wrapped field
resolved exactly as an ordinary field would be — scalar or qualified name, the
`= null` default of §3.6 when it applies, the constraint comment. A struct or
union arm references its own table directly and gets no box.

The reversal was right because refusing made a large class of **legal** typl
unprojectable: typl §10 permits any named type as an arm, a named scalar is a
named type — the cruise-control acceptance fixture itself declares
`disengage : Percent` — and the target represents the arm fine with one more
table. Verified in both `flatc` 25.12.19 and `planus` 1.3.0. Recorded as
ADR-0018 decision 2.

### 3.2 A struct is always emitted as a table, never as a FlatBuffers struct

typl Appendix D permits a struct whose fields are all fixed-width and
non-optional to be emitted as a FlatBuffers `struct` — "inline, zero
indirection" — and the IR carries `fixed_layout` for it. **That allowance is
withdrawn for this projection.**

A FlatBuffers `struct` has a fixed inline layout and no vtable, so a reader with
a newer schema reads past what the writer wrote. Appending a struct field is a
**compatible** change in typl — it is in the mutation set E9.8's stability
property already exercises — and after such an append:

| Emitted as | v1 data read with the v2 schema     |
| ---------- | ----------------------------------- |
| `struct`   | `{a:1, b:2, c:0}` — `c` fabricated  |
| `table`    | `{a:1, b:2}` — `c` correctly absent |

The `struct` form does not reject the buffer; it invents a value. That makes
ADR-0016 decision 6 property 3 unsatisfiable on the construct, and the failure
is silent rather than loud. The `fixed_layout` flag stays in the IR for targets
where a fixed layout is safe.

The cost is the vtable and one indirection on every payload. That is the price
of remaining evolvable, and evolvability is what the projection contract exists
to protect.

### 3.3 A map is a vector of entry tables, with no `(key)`

FlatBuffers has no map type. The idiom is a vector of two-field entry tables,
optionally with one field marked `(key)` to generate `LookupByKey` and enable a
binary search.

**`(key)` is not emitted.** It obliges the producer to write the vector sorted
by that field, and nothing checks the obligation at read time — an unsorted
vector with `(key)` makes `LookupByKey` return the wrong element silently. typl
§12.2 gives a map no ordering at all: keys "must be a named string type or
primitive", and the declaration is a bounded set of associations. So `(key)`
would have the schema assert an ordering the contract never states, discharged
by a producer that does not exist until E9.11. That is the construct ADR-0013
decision 2 forbids — one that implies a guarantee the contract does not make.

It is reopenable in E9.11, where a generated producer could hold the obligation,
and it would want a contract-level statement that the map is ordered, which typl
does not have.

### 3.4 The rest of the mapping

| typl                      | FlatBuffers                                         |
| ------------------------- | --------------------------------------------------- |
| `TypeDef` (named scalar)  | inlines to its backing scalar (ADR-0017 d1)         |
| inline scalar (typl §5.2) | its resolved scalar, no comment (Task 7 note below) |
| `StructDef`               | `table`, ids `= ordinal − 1` (§3.2)                 |
| `EnumDef`                 | `enum` with an explicit underlying type             |
| `EnumSetDef`              | the resolved integer width, bits in a comment       |
| `UnionDef`                | a wrapper table holding a native union (§3.1)       |
| `ConstDef`                | not emitted (ADR-0013 decision 5)                   |
| array                     | `[T]` vector                                        |
| map                       | `[Entry]` vector of entry tables, no `(key)` (§3.3) |
| tuple                     | a generated table with positional fields            |
| `?` field                 | table field absence — tables are natively sparse    |

**Amendment from execution (Task 7) — the inline-scalar row was missing, in the
code as well as here.** `resolve_field_type` as first written had no arm for the
IR's `InlineScalar` kind, so a field written `radius : integer
[0..100]`
directly — the anonymous counterpart of a named scalar — was refused as
unprojectable, though every other backend handles the kind. Task 7 found the gap
while implementing the union-arm wrapper and fixed it in the same commit: the
field resolves to its scalar with no constraint comment, mirroring
`ridl-backend-proto`'s `InlineScalar` arm — an inline scalar names no type to
hang a comment on, which is ADR-0017's open item 1, unchanged by this backend.

### 3.5 Name scopes differ from proto3, so the proto backend's guard is not reusable

ADR-0017 decision 4 requires a backend projecting onto a namespaced target to
model that target's name scopes and refuse a collision. FlatBuffers' scopes are
**not** proto3's, verified against `flatc`:

| Scope     | Holds                                                  |
| --------- | ------------------------------------------------------ |
| namespace | table, struct, enum and union names — one shared space |
| table     | that table's field names                               |
| enum      | that enum's value names                                |

The third row is the difference that matters. proto3 scopes enum values as
siblings of their enum, which is why E9.8 prefixes every value with its enum's
name and synthesizes an `UNSPECIFIED`. **FlatBuffers scopes them inside the
enum** — two enums may each declare `OK`, verified — so **no prefixing is
emitted here, and none is needed**. Copying E9.8's prefixing would mangle every
value name for no reason.

Consequently `ridl-backend-proto`'s `SymbolScope` must not be lifted as-is: its
package scope includes enum values, which would over-refuse on this target. This
backend gets its own scope model over the three rows above. What it must still
catch is a **generated** name colliding with a declared one — the union wrapper
tables and map entry tables of §3.1 and §3.3 mint names into the namespace
scope, and `flatc` rejects a duplicate with "datatype already exists".

### 3.6 An enum-typed field, and the one case that forces a choice

**A FlatBuffers enum declaration needs no zero member**, unlike proto3, so
nothing is synthesized into the enum itself. But every FlatBuffers table field
carries a default, and `flatc` refuses a field whose implicit default of 0 is
not a member of its enum:

```text
error: default value of `0` for field `g` is not part of enum `Gear`.
```

So four cases, and only the last needs a decision:

| Enum declares 0 | typl field   | Emitted               |
| --------------- | ------------ | --------------------- |
| yes             | required     | `g: Gear;`            |
| yes             | `?`          | `g: Gear = null;`     |
| no              | `?`          | `g: Gear = null;`     |
| **no**          | **required** | **`g: Gear = null;`** |

The last row is forced rather than chosen. The alternative — defaulting to some
declared value, say the lowest — would make a truncated or malformed buffer read
silently as that value, fabricating a reading the contract never stated.
`= null` never fabricates: a missing value reads as missing.

It does render a required field as optional in generated code, which is a
faithfulness loss. But **FlatBuffers cannot express a required scalar or enum in
any case** — `flatc` refuses the attribute outright, "only non-scalar fields in
tables may be 'required'" — so the requiredness was already not representable on
this target. Given that, surfacing absence beats inventing a value. All four
rows verified against `flatc` and `planus`.

## 4. The width floor — ADR-0013 decision 6 closed by decision

ADR-0013 decision 6 makes typl §17.11's deferred `wire` clause a precondition:
because the width derives from the declared range, widening `[0..255]` to
`[0..300]` flips `uint8` to `uint16` with no edit to any width declaration, and
on FlatBuffers that is a hard wire break.

**The closure is that `ridl-diff` remains the sole guard for v0.1**, recorded
here rather than left open. The gate does classify the flip as breaking and
stops it.

The alternative that would remove the hazard from the language — emitting the
widest width in each signedness class, so a widening can never flip anything —
was measured and rejected:

| Case                        | Narrow (derived) | Always-widest | Cost |
| --------------------------- | ---------------- | ------------- | ---- |
| 8-signal status table       | 44 bytes         | 96 bytes      | 2.2× |
| 7-field fixed-layout struct | 28 bytes         | 72 bytes      | 2.6× |

FlatBuffers has no varint: a declared width is bytes on the wire, every message,
forever. Always-widest gives up the property Appendix D calls FlatBuffers'
cleanest advantage, on the target chosen precisely for byte efficiency. It
closes the hazard by removing the reason to use the target.

**The forcing case that reopens typl §17.11:** a deployment that needs to widen
a range on a FlatBuffers-bound contract without a coordinated flag day. At that
point the `wire` clause can be designed against a real case rather than an
imagined one, and its constraints are already settled in §17.11.

## 5. Verification

**`planus-translation` 1.3.0 compiles every emitted schema in the test suite.**
It is pure Rust, a dev-dependency only, so this costs no bootstrap change, no CI
install and no new gate recipe — the same shape as `protox` for the proto3
backend.

It was chosen after measuring its coverage against `flatc`, not assumed:

| Construct                   | `flatc` | `planus` |
| --------------------------- | ------- | -------- |
| `(id: N)` explicit ids      | ✓       | ✓        |
| `(deprecated)` placeholders | ✓       | ✓        |
| `[T]` vectors               | ✓       | ✓        |
| union in a wrapper table    | ✓       | ✓        |
| enum with underlying type   | ✓       | ✓        |
| `(key)`                     | ✓       | ✗        |
| `[T:N]` fixed arrays        | ✓       | ✗        |

Both gaps fall outside what this projection emits — `(key)` by §3.3 and fixed
arrays because they are legal only inside a FlatBuffers `struct`, which §3.2
never emits. **So `planus` validates every emitted construct**, which is the
property that matters: E9.8's costliest defect lived in the one path a test was
told not to validate. The coverage is per-construct, not per-schema: `planus`
also reserves nine words that `flatc` treats as contextual identifiers (`table`,
`root_type`, `attribute`, and six more), so an emitted name that reaches one —
including a name the pinned transform manufactures, `rootType` → `root_type` —
is valid for `flatc` but not checked by the oracle. Found at the branch's final
review; recorded in ADR-0018 decision 7.

Also: golden snapshots pin the emitted text; the stability property is driven
from `ridl-diff`'s classifier as E9.8's is, and here it additionally guards the
`(deprecated)` slot filling; and the acceptance fixture is the cruise-control
package emitting a valid FlatBuffers schema.

## 6. Blast radius

- **New crate** `crates/ridl-backend-flatbuffers/`, with ADR-0017 decision 1's
  API. Adds its own scope to `.git-std.toml`.
- **New emit value** `Emit::Flatbuffers`, flag `flatbuffers`, artifact
  `<base>.fbs`; a code emit, so `ir_dump_suffix` returns `None`. `ridlc`'s
  wildcard-free `Emit` matches name every site to update.
- **`wasm-check`** must pass, so the crate builds under `--no-default-features`;
  `planus-translation` stays a dev-dependency.
- **typl Appendix D** is amended by §3.2.
- **The note §4.4** is superseded by §3.1.
- **ADR-0013 decision 6** is closed by §4.

## 7. Out of scope

- **`rpc_service`, reply carriers and the store** — E9.11, which also owes the
  ADR-0013 decision 2 versus ADR-0016 decision 10 resolution.
- **The schema hash** — E9.10, over the IR rather than any emitted schema.
- **The `wire` clause itself** — typl §17.11, with the forcing case in §4.
- **`(key)` and sorted-vector lookup** — reopenable in E9.11 (§3.3).
