# Attribute registry, fail-closed diff, and the interaction family

Status: design, written 2026-10-11 after a brainstorming session with Sebastien.
Every decision in section 2 is his. The plan that executes this design is
written after this document is approved.

Satisfies: release 0.10 design R-4
([`2026-10-11-release-0.10-design.md`](./2026-10-11-release-0.10-design.md));
roadmap stories E3.1, E3.2 and E3.3; ADR-0012 decisions 4, 6, 8 and 9.

Issues: #268 (E3.1), #269 (E3.2), #270 (E3.3), #370. Lane L of the
[lanes plan](./2026-10-11-release-0.10-lanes-plan.md).

## 1. Goal

Settle every part of the language surface that E3.1 to E3.3 touch before lane L
switches on the `keywords` and `diagnostics` freeze rows, so that no later
0.10.x release has to add a reserved word, remove an attribute key, or add a
codegen-protocol enum value for these features.

Success criteria:

1. Every attribute key that any profile accepts is a row of one table, and that
   table is the only allow-list the checkers read.
2. A change to any attribute value that reaches the IR is reported by
   `ridl diff`, unless the table records a reason not to report it.
3. The IR and the codegen model carry `family` and `shape` with their complete
   value sets, and the IR reader rejects an invalid (kind, family, shape)
   combination.
4. The catalog hash of every existing source, and so the `wire` dump, is
   unchanged.

## 2. Decisions

| Id  | Decision                                                                                                                                                                                                                                                                                                  |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D-1 | E3.3 lands in 0.10: the `family` and `shape` enums are added to the IR now with their full value sets. Every existing keyword lowers to `dispatch` and no shape. A later domain spelling (E7) then adds no enum value and can ship in a 0.10.x release.                                                   |
| D-2 | The ADR-0012 spellings are not reserved words. A spelling is a keyword only in the first position of an interface member, in a profile that implements it. This replaces the family-wide reservation that typl §1.4 states, and closes #370.                                                              |
| D-3 | The retired uxdl words leave `FAMILY_RESERVED`, except `states` and `during`, which general form §4.8 names as ridl clauses. The five intent verbs follow D-2. The rmdl words stay reserved.                                                                                                              |
| D-4 | One attribute table in `ridl-core` is the registry (approach A in section 7). The six keys of `GF_ATTRIBUTE_KEYS` are removed, because none has a machine consumer (general form §4.1). `require` and `ensure` are registered. Backend keys are one pattern row. No `user:` owner is added yet.           |
| D-5 | For the rsdl system diff, which gives no verdict, fail-closed means "reported". `tier`, `labels`, `deprecated` and backend keys become reported changes. `depth`, `slots` and `budget` stay unreported, with the recorded reason "sizing".                                                                |
| D-6 | The zero value of `Family` is `FAMILY_DISPATCH` and the zero value of `Shape` is `SHAPE_NONE`. This departs from the `_UNSPECIFIED = 0` convention of `ir.proto` so that an existing interaction encodes to the same bytes, its catalog hash does not change, and an IR written by 0.9 decodes correctly. |

## 3. Reserved words (#370)

### 3.1 What changes

These 16 words leave `FAMILY_RESERVED` in `crates/ridl-syntax/src/keywords.rs`
and become ordinary identifiers in every profile:

`view`, `display`, `input`, `action`, `activate`, `toggle`, `select`, `adjust`,
`dismiss`, `fetch`, `navigate`, `scroll`, `drag`, `observe`, `surface`, `agent`.

These stay reserved: every keyword that a live profile uses, `fixed`, `states`,
`during`, and the rmdl block.

No profile keyword table gains a word. 0.10 implements no domain spelling.

### 3.2 The member-start rule

The ADR-0012 spellings are `present`, `notify`, `measure`, `detect`, `actuate`,
`trigger`, the five intent verbs, and the open candidates `supply` and `read`. A
spelling is a keyword only as the first token of an interface member, in a
profile that implements it. Everywhere else it is an identifier.

An interface member line never starts with an identifier, so a profile that
gains a spelling invalidates no source that compiled before. The `keywords` dump
gains a line, and ADR-0029 accepts an added line.

Evidence that a family-wide reservation would cost users: the ROS 2 corpus maps
a published interface set and uses `present` as a field
(`evals/corpus/ros2/sensor_msgs/messages.typl`) and `trigger` as a query
(`evals/corpus/ros2/std_srvs/interactions.ridl`).

### 3.3 Documents

- typl §1.4 states the member-start rule in place of the family-wide
  reservation, and drops the removed words from its list.
- ridl §2.3 lists the reserved set as built.
- ADR-0012 gets an amendment note under decision 4 that records D-2 and D-3.
- `freeze/keywords.txt` is regenerated.

## 4. The attribute registry (#268, #269)

### 4.1 The table

A new module `ridl_core::attributes` holds `pub const ATTRIBUTES: &[AttrKey]`.
One row per key:

| Field      | Meaning                                                                                                  |
| ---------- | -------------------------------------------------------------------------------------------------------- |
| `name`     | the key, or `Namespaced` for the dotted `namespace.key` pattern                                          |
| `owner`    | `Core`, or `Backend` for a key whose namespace a configured backend claims                               |
| `form`     | `Flag`, `Assignment` or `Predicate` (general form §4.2)                                                  |
| `sites`    | the declaration kinds and body lines that accept the key                                                 |
| `consumer` | a short text that names the machine consumer (general form §4.1)                                         |
| `diff`     | `Compared(heading)`, or `NotReported(reason)` where the reason is one of a closed set, today only sizing |

The rows:

| Key          | Owner   | Form       | Sites                               | Diff                        |
| ------------ | ------- | ---------- | ----------------------------------- | --------------------------- |
| `require`    | core    | predicate  | `command`, `query`                  | interface `ContractChanged` |
| `ensure`     | core    | predicate  | `query`                             | interface `ContractChanged` |
| `instances`  | core    | assignment | `component`                         | compared (as today)         |
| `external`   | core    | flag       | `component`, `machine`              | compared (as today)         |
| `tier`       | core    | assignment | `distribution`                      | compared (new)              |
| `labels`     | core    | assignment | the rsdl declarations (as today)    | compared (new)              |
| `deprecated` | core    | assignment | the rsdl declarations (as today)    | compared (new)              |
| `depth`      | core    | assignment | `deployment`, placement line        | not reported: sizing        |
| `slots`      | core    | assignment | `deployment`, placement line        | not reported: sizing        |
| `budget`     | core    | assignment | `deployment`, placement line        | not reported: sizing        |
| `ns.key`     | backend | any        | every rsdl site that takes them now | compared (new)              |

The sites in this table restate what `AttrSite::allows` in
`crates/ridl-sem/src/rsdl/attrs.rs` and the ridl checker accept today. The plan
copies them from the code, and this design does not change them.

### 4.2 Readers

- The ridl checker (`Checker::check_member_attrs`) and `rsdl/attrs.rs` look up
  the table. `GF_ATTRIBUTE_KEYS` and `RSDL_KEYS` are deleted.
- The diagnostics do not change: FORM-106 for an unknown key, FORM-107 for a key
  on a site that does not accept it, FORM-108 for a duplicate key, RSDL-804 for
  an unclaimed backend namespace. One of the six removed keys now draws FORM-106
  where it drew FORM-107; both are errors.
- The system diff in `crates/ridl-diff/src/system.rs` reports each `Compared`
  row as a before-and-after line under the heading of the site the key is on.

### 4.3 Tests

1. No two rows have the same name.
2. Every row has a non-empty consumer and at least one site.
3. For every `Compared` row, a test builds two IRs that differ only in that key
   and asserts that `ridl diff` reports a change. The test iterates the table,
   so a new row without diff support fails it.
4. For every `NotReported` row, a test asserts that the change is not reported,
   so that the recorded reason stays true.

### 4.4 Freeze

The `keywords` dump gains one line per (key, site, form), for example
`attribute tier distribution assignment`. Widening an allow-list adds lines.
Removing a key or a site removes a line and fails the check.

### 4.5 Documents

- The attribute sections of the ridl and rsdl references list the registry as
  built.
- ADR-0012 gets amendment notes: under decision 8, the owner is `core` or
  `backend`, and the `user:` owner waits for a consumer; under decision 9, D-5.

## 5. Family and shape (#270)

### 5.1 Enums

In `crates/ridl-ir/proto/ridl/ir/v2/ir.proto`, mirrored in
`crates/ridl-ir/proto/ridl/codegen/v1/model.proto`:

```proto
enum Family {
  FAMILY_DISPATCH = 0;
  FAMILY_PRESENTATION = 1;
  FAMILY_INTENT = 2;
  FAMILY_ACQUISITION = 3;
  FAMILY_CONTROL = 4;
}

enum Shape {
  SHAPE_NONE = 0;
  SHAPE_ACTIVATE = 1;
  SHAPE_TOGGLE = 2;
  SHAPE_SELECT = 3;
  SHAPE_ADJUST = 4;
  SHAPE_DISMISS = 5;
}
```

A comment in `ir.proto` records D-6 as the reason for the zero values.

### 5.2 Placement

- `family` is a field of `SignalDef`, `EventDef`, `CommandDef` and `QueryDef`.
- `FixedDef` has no `family`, because `fixed` is family-neutral (ADR-0012
  decision 4).
- `shape` is a field of `CommandDef` only, so a shaped signal, event or query
  cannot be expressed.
- The codegen model's `Interaction` gains the same two fields.

### 5.3 Valid combinations

The open cells of ADR-0012 decision 4 count as valid, so that a later spelling
for them needs no enum value:

| Kind    | Valid (family, shape)                                                                                                          |
| ------- | ------------------------------------------------------------------------------------------------------------------------------ |
| signal  | dispatch, presentation, acquisition, control                                                                                   |
| event   | dispatch, presentation, intent, acquisition                                                                                    |
| command | (dispatch, none), (intent, activate), (intent, toggle), (intent, select), (intent, adjust), (intent, dismiss), (control, none) |
| query   | dispatch, acquisition                                                                                                          |

One function in `ridl-ir`, `check_interaction_class`, holds this table. The
strict IR reader (ADR-0014) calls it on decode, and the IR writer and the
codegen request writer call it before they encode. `(command, intent, none)` is
therefore rejected when the IR is read, not by a lint that a caller can forget.

### 5.4 Lowering, hash and diff

- Every existing keyword lowers to `FAMILY_DISPATCH` and `SHAPE_NONE`, so the
  encoded IR of every existing source is unchanged.
- A test asserts that the catalog hash of `examples/cabin` and the `wire` dump
  are byte-identical before and after the change.
- `ridl-diff` gains the interface category `InteractionClassChanged`, classified
  breaking, for a family or shape change on a name-paired interaction. No source
  can produce it before E7, but the walk compares the fields from the start, so
  the diff fails closed. A unit test builds the two IRs directly.

### 5.5 Codegen protocol

Plugins read leniently, so a plugin that does not know the fields keeps working
and sees the core kind (ADR-0012 decision 7). The `codegen-protocol` dump gains
lines. A heads-up issue goes to driftsys/ridlc-gen-kotlin.

### 5.6 Documents

ADR-0012 gets an amendment note under decision 6 that records the placement in
5.2 and D-6.

## 6. Delivery

Three pull requests, in this order. Each is a group of tasks executed with
subagent-driven development and the per-task review gate, followed by `/review`
passes 1 and 2.

| PR | Content                     | Issues     | Breaking                                                                   |
| -- | --------------------------- | ---------- | -------------------------------------------------------------------------- |
| 1  | Section 3, reserved words   | #370       | no: removing a reserved word cannot invalidate a source                    |
| 2  | Section 4, the registry     | #268, #269 | no: every key that compiles today still compiles; `ridl diff` reports more |
| 3  | Section 5, family and shape | #270       | no: additive fields; Kotlin heads-up                                       |

Each pull request rebases onto `main` and runs `cargo xtask freeze update`
before it merges. The `keywords` and `diagnostics` rows wait for all three. This
design adds no diagnostic code, so `freeze/diagnostics.txt` is expected not to
change.

Review seats mutate the code to check the tests: delete a `Compared` arm in the
system diff, accept `(command, intent, none)` in `check_interaction_class`, and
confirm that a test fails each time.

Out of scope:

- The remaining half of #276 (an attribute block on `interface` and `service`).
  ADR-0028 decision 4 declined the promotion it waited for. Proposed to
  Sebastien: close it as superseded.
- Ratifying `docs/wip/family-general-form.md` into `docs/specification/`. It is
  a separate lane L item.
- The domain spellings themselves (E7.7 to E7.9).

## 7. Alternatives considered

- **Defer E3.3 to 0.11.** Adding the field later is additive, but each new enum
  value could then only ship in a minor release. Rejected for D-1.
- **Reserve the words only, add the enums with E7.** Same cost as the deferral,
  for the same reason. Rejected for D-1.
- **Reserve the spellings family-wide**, as typl §1.4 says. Breaks the ROS 2
  corpus and any user source that names a field `present` or a query `trigger`.
  Rejected for D-2.
- **Reserve the spellings in `.rxdl` only.** Still invalidates identifiers in
  that profile, and the member-start rule costs no user anything. Rejected for
  D-2.
- **Keep the uxdl block reserved**, or free only the words ADR-0012 rejected.
  Freeing a word after the freeze removes a dump line, so the decision has to be
  taken now, and no profile uses these words. Rejected for D-3.
- **Two tables with a governance test over both (approach B).** Cheaper, but
  keeps two sources of truth, where ADR-0012 decision 8 asks for one registry.
  Rejected for D-4.
- **Generate the table from a TOML file (approach C).** A build step for about a
  dozen rows. Rejected for D-4.
- **Keep the six `GF_ATTRIBUTE_KEYS` as registered keys with no site.** They
  have no consumer, so by general form §4.1 they are not attributes. Rejected
  for D-4.
- **Follow the `_UNSPECIFIED = 0` convention.** Changes every catalog hash, so
  every 0.9 runtime fails `attach` against a 0.10 peer, and the `wire` dump
  changes. Rejected for D-6.
- **One enum per kind that lists the spellings** (for example `SignalSpelling`
  with `SIGNAL`, `PRESENT`, `MEASURE`, `ACTUATE`). Makes every invalid
  combination unrepresentable, but the open cells would each need a new enum
  value, which only a minor release can add, and ADR-0012 decision 6 names two
  enums. Not taken.
