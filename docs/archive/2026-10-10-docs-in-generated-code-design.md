# Documentation in generated code — design

Status: draft for review, 2026-10-10. Spec 2b of the
[devex and agent tracks brief](../wip/2026-10-03-devex-and-agent-tracks-brief.md).
Written from the brainstorm handoff
[`2026-10-10-docs-in-generated-code-handoff.md`](2026-10-10-docs-in-generated-code-handoff.md),
whose decisions D-1 to D-4 and sections 1 to 3 the maintainer approved on
2026-10-10. This design adds the facts and the choices that writing the spec
required; the section "Choices this spec adds" lists them for review.

## Goal

A user of generated code reads, on the items they call or implement, the same
documented contract the editor shows on hover: the source doc text, its links,
`@see`, `@since`, `@deprecated`, and the rules extracted from the model (range,
step, unit, length, pattern, collection bounds, timing, response bound, errors,
`require` and `ensure`). The Rust backend renders this as rustdoc, the
TypeScript backend as TSDoc, and the out-of-repo Kotlin plugin as KDoc, all from
one source. Generated internals are hidden and carry no maintainer notes.

## What the brief asked for

From the brief, section "Spec 2b":

- a documented contract that is extracted once from the model, plus the source
  docs;
- rustdoc and TSDoc on the public facade only, and KDoc from the same source in
  the Kotlin plugin;
- hidden internals with no maintainer notes.

The brief left three questions open. This design answers them:

| Open question in the brief                                              | Answer                                                                                                                                                 |
| ----------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| The IR shape for the documented contract, and the stability promise     | The rules go in the codegen `Model`, not in IR v2 (D-1). Every change to the `Model` is additive. IR v2, `catalog_hash` and `ridl diff` do not change. |
| How rules are worded so that they read the same in each language        | Each rule carries one canonical text line, the line the hover already shows; every backend places it verbatim (D-2).                                   |
| The relation to the validators that the typl value objects design plans | The docs state the contract, not what a backend checks (D-3). The book records, per backend, which rules the generated code checks.                    |

## Facts this design rests on

- `ridl_ir::rules::rules(pkg, deps, item) -> Vec<Rule>`
  (`crates/ridl-ir/src/rules.rs`) is a pure function over `v2::Package`. `item`
  is an `ItemRef`: a package-level declaration or an interaction (`Decl`), a
  struct field (`Field`), or a command or query parameter (`Param`). For a field
  or a parameter whose type is a named declaration, it returns that
  declaration's rules, looked up in `pkg` or in `deps`. `rules::render` gives
  one line of text per rule. Only the language server's hover calls either
  function. Nothing about rules is stored in the IR.
- `Rule` has ten variants: `Range { min, max }`, `Step`, `Unit`,
  `Length { min, max }`, `Pattern`, `CollectionBound { min, max }`,
  `Timing { mode, min_us, max_us }`, `ResponseBound { min_us, max_us }`,
  `Errors` and `Contract { kind, source }`. ADR-0026 decision 9 calls its shape
  provisional and leaves the choice of consumer to this spec.
- IR v2 carries `doc`, `links`, `see` and `since` on `Decl`, `Field`,
  `EnumValue`, `UnionArm`, `Interface`, `Service` and `Param`
  (`crates/ridl-ir/proto/ridl/ir/v2/ir.proto`), with a `DocLink` of `text`,
  `offset`, `len` and `target`. `target` is the canonical qualified name,
  `pkg.Name` with an optional `.member`.
- The codegen `Model` (`crates/ridl-ir/proto/ridl/codegen/v1/model.proto`) is
  what in-tree backends and out-of-process plugins receive. It is built by
  `ridl_ir::codegen::lower(package, others)` in
  `crates/ridl-ir/src/codegen/lower.rs`. It carries `doc` on `Declaration`,
  `Field`, `EnumValue`, `Arm`, `Interface`, `Interaction` and `Service`, and
  `labels` and `deprecated` on all of these except `EnumValue` and `Arm`.
  `Param` carries only `name` and `type`: no `doc`. No `Model` message carries
  links, `see`, `since` or rules.
- The `Model` already defines the enums `TimingMode` and `ContractKind`, and the
  messages `Timing` and `Clause`.
- The Rust backend calls `lower` and reads both the `Model` and the
  `v2::Package`. It emits `///` docs and `#[deprecated]`, and no rules or links.
- The TypeScript backend reads only the `v2::Package`; it does not call `lower`.
  It emits JSDoc with `@deprecated` and the tags `@unit`, `@range` and
  `@bounds`, which it reads from the constraints.
- Some generated docs describe internals: the codec emits "A zero-copy accessor
  over FlatBuffers bytes ..." (`crates/ridl-backend-rust/src/codec.rs`). Some
  cite an issue: the refusal notes that cite driftsys/ridl#704
  (`crates/ridl-backend-rust/src/lib.rs`,
  `crates/ridl-backend-rust/src/descriptors.rs`).
- Under `docs/specification/ir-specification.md` §6, a new optional field, a new
  message and a new `oneof` member are additive. A plugin's lenient reader
  ignores a key it does not know.

## Decisions

### D-1. Rules live in the codegen `Model`

`lower` calls `ridl_ir::rules::rules` once for each item that has an `ItemRef`,
with `others` as `deps`, and writes the result into the `Model`. IR v2 does not
change, so `catalog_hash` and `ridl diff` are unaffected, and the `Model` is the
one source for every backend and plugin.

Rejected: storing rules in IR v2 as well, because they are derived data that the
hash would have to exclude, and the same fact would be stated twice. Rejected:
each backend calling `rules()` itself, because the Kotlin plugin would have to
re-implement the extraction, and the wording would drift between languages.

### D-2. Each rule carries structured fields and one canonical text line

The text is `rules::render`'s line for that rule, the same text the hover shows.
Backends place it verbatim. The structured fields stay for tooling and for the
TypeScript tags.

Rejected: structured fields only, because each backend would word the rule
itself and the wording would drift. Rejected: text only, because the TypeScript
`@unit`, `@range` and `@bounds` tags would lose their source.

### D-3. Docs state the contract, not enforcement

Every backend renders every rule that the `Model` declares, in neutral wording
("range 0.0 ..= 250.0", never "this function rejects"). Which rules a backend's
generated code checks is recorded in the book, per backend, not per item.

Rejected: rendering only the rules that the backend enforces, because the
TypeScript output would show almost nothing and the docs would differ by
backend. Rejected: marking each rule as checked or not checked, because the
`Model` would need a per-backend fact.

### D-4. `deprecated` and `labels` stay doc tags

The general form §4.7 promotion of `labels` and `deprecated` to attribute keys
is declined, not deferred again. Deprecation metadata in generated code comes
from the `@deprecated` doc tag, as it does today. There is no grammar change.
ADR-0028 records this, ADR-0026 decision 4 is amended to point to it, and the
general form §4.7 and its §7 item 2 are amended.

Rejected: the promotion. An attribute block exists only on `signal`, `event`,
`command` and `query` and on rsdl declarations; `type`, `const`, `struct`,
`enum`, `enumset`, `union`, `interface`, `service`, fields, enum values and
`fixed` have none. The promotion would add the attribute block to the grammar on
eleven carriers to move two facts that the doc tags already deliver to every
consumer.

## Section 1 — what the `Model` carries

### Doc carriers

Every `Model` message that carries `doc` gains, next to it, the layout IR v2
uses:

```proto
// Resolved links in `doc`, in source order (ADR-0026).
repeated DocLink links = <n>;
// Resolved @see targets, in source order (ADR-0026).
repeated DocLink see = <n>;
// @since versions, in source order (ADR-0026).
repeated string since = <n>;
// The rules of the item, in the order `ridl_ir::rules::rules` returns them.
repeated Rule rules = <n>;
```

The carriers are `Declaration`, `Field`, `EnumValue`, `Arm`, `Interface`,
`Interaction`, `Service` and `Param`. `Param` first gains `string doc`, copied
from `v2::Param.doc`. The field numbers are the next free numbers in each
message that are not inside a `oneof` range or a reserved range; the plan
assigns them.

`lower` copies `links`, `see` and `since` from the IR item the carrier is
lowered from. It fills `rules` as follows:

| Carrier                                    | `rules` holds                                                                                  |
| ------------------------------------------ | ---------------------------------------------------------------------------------------------- |
| `Declaration`                              | `rules(pkg, others, ItemRef::Decl(decl))`: a type's range, step, unit, length, pattern         |
| `Interaction`                              | `rules(pkg, others, ItemRef::Decl(decl))`: timing, response bound, errors, `require`, `ensure` |
| `Field`                                    | `rules(pkg, others, ItemRef::Field(field))`: the rules of the field's named type               |
| `Param`                                    | `rules(pkg, others, ItemRef::Param(param))`: the rules of the parameter's named type           |
| `EnumValue`, `Arm`, `Interface`, `Service` | always empty: `rules()` has no `ItemRef` for these items                                       |

The four always-empty carriers keep the field so that every doc carrier has one
layout and a later `ItemRef` can fill it additively.

A `ForeignDeclaration` holds a `Declaration`, so a foreign declaration carries
its rules and doc links too.

### `DocLink`

```proto
// One resolved doc link. The same fields as `ridl.ir.v2.DocLink`.
message DocLink {
  // The link text as written, without the brackets.
  string text = 1;
  // Byte offset of the link in the carrier's `doc`; 0 for a `see` entry.
  uint32 offset = 2;
  // Byte length of the link in the carrier's `doc`; 0 for a `see` entry.
  uint32 len = 3;
  // The canonical qualified name of the target, `pkg.Name`, with an
  // optional `.member`.
  string target = 4;
}
```

`target` is a qualified name, not a language spelling. Each backend maps it to
its own path with its name transform (ADR-0016). `offset` and `len` index the
carrier's `doc` in the `Model`, which `lower` copies from the IR byte for byte.

### `Rule`

```proto
// One rule of an item (ADR-0028). `text` is the canonical line; a backend
// places it verbatim. A reader that does not know the `kind` member still
// renders `text`.
message Rule {
  message Range { optional string min = 1; optional string max = 2; }
  message Length { optional uint64 min = 1; optional uint64 max = 2; }
  message CollectionBound { uint64 min = 1; uint64 max = 2; }
  message Timing {
    TimingMode mode = 1;
    optional string min_us = 2;
    optional string max_us = 3;
  }
  message ResponseBound { optional string min_us = 1; optional string max_us = 2; }
  message Errors { repeated string names = 1; }
  message Contract { ContractKind kind = 1; string source = 2; }

  oneof kind {
    Range range = 10;
    string step = 11;
    string unit = 12;
    Length length = 13;
    string pattern = 14;
    CollectionBound collection_bound = 15;
    Timing timing = 16;
    ResponseBound response_bound = 17;
    Errors errors = 18;
    Contract contract = 19;
  }
  // `ridl_ir::rules::render` of this rule.
  string text = 1;
}
```

The fields are those of today's `ridl_ir::rules::Rule`. The members are nested
in `Rule` so that their names do not collide with the `Model`'s existing
messages: the timing member's message is `Rule.Timing`, distinct from the
top-level `Timing`. `TimingMode` and `ContractKind` are the `Model`'s existing
enums. A new rule kind later is a new `oneof` member, which is additive; a
plugin built against an older `Model` renders `text` for a member it does not
know.

The shape of `ridl_ir::rules::Rule` stops being provisional: ADR-0026 decision 9
is amended to say that the `Model` carries it and that a change to it is a
change to `ridl.codegen.v1`.

## Section 2 — rendering per backend

| Source        | Rust (rustdoc)                                  | TypeScript (TSDoc)                                                                    | Kotlin plugin (KDoc) |
| ------------- | ----------------------------------------------- | ------------------------------------------------------------------------------------- | -------------------- |
| doc text      | `///` lines, as today                           | `/** */`, as today                                                                    | `/** */`             |
| `[Name]` link | an intra-doc link to the target's Rust path     | `{@link Name}`                                                                        | `[Name]`             |
| rules         | a `# Rules` section, one bullet per rule `text` | a `Rules:` list under `@remarks`; the existing `@unit`, `@range`, `@bounds` tags stay | a `Rules:` paragraph |
| `@see`        | a `# See also` list of links                    | `@see {@link X}`                                                                      | `@see X`             |
| `@since`      | one `Available since X.` line per version       | `@since X`                                                                            | `@since X`           |
| `@deprecated` | `#[deprecated(note = ...)]`, as today           | `@deprecated reason`, as today                                                        | `@Deprecated`        |
| `@labels`     | not rendered                                    | not rendered                                                                          | not rendered         |

Rules for every backend:

- **Link replacement.** A backend replaces the `len` bytes at `offset` in `doc`
  with its link syntax. It replaces from the last link to the first, so that
  each earlier offset stays valid.
- **Unresolvable links.** A link whose target is not an item in the same
  generated output (for example a type of a dependency package that this output
  does not contain) renders as a code span of the qualified name, never as a
  link that would not resolve.
- **Labels** are metadata for tooling and are not rendered. The brief asks for
  docs and rules only.
- **Interaction rules** go on every public item a user calls or implements for
  that interaction: the method of each of the two clients, and the method of the
  `serve` trait.
- **Parameter docs.** In the Rust face, a call's method renders its parameter's
  doc and rules under a `# Parameters` section: one bullet with the parameter's
  name, its doc text, and its rule lines as nested bullets. TypeScript emits no
  face, so it has no parameter to render; the Kotlin plugin uses `@param`.
- **Kotlin.** The KDoc column is guidance for the out-of-repo plugin. Its
  repository receives a heads-up issue, as it did for earlier `Model` changes.

The TypeScript backend calls `lower` to obtain the `Model`, as the Rust backend
does, and reads `doc`, `links`, `see`, `since` and `rules` from it. Its existing
`@unit`, `@range` and `@bounds` tags keep their current source; moving them to
`Rule`'s structured fields is allowed but not required by this design.

## Section 3 — the facade, internals, maintainer notes and testing

### The facade

The facade is every generated item that a user names in their own code: the
declared types, their fields, enum values and union arms; the interface module;
the two clients, `serve`, `prelude` and `blocking`. Only facade items carry doc
text and rules.

### Internals

Every generated item outside the facade is an internal.

- **Rust:** `pub(crate)` where the crate compiles with it; `#[doc(hidden)]`
  where the item must stay `pub` for a technical reason (`__ridl_package`, which
  a consumer's path names). The plan lists each internal and which of the two it
  gets.
- **TypeScript:** not exported, or exported with `@internal` where another
  generated module imports it.
- Internals carry no doc text. The explanatory docs they carry today move to
  `//` comments in the generator source, next to the code that emits them.

### Maintainer notes

Generated docs that cite an issue (the driftsys/ridl#704 refusal notes) are
removed, following D-3. The book's chapter on generated code records what each
backend checks. A refusal's own error message, which is not a doc comment, is
outside this rule.

### Tests

1. **A snapshot per backend** over one fixture package that exercises every rule
   kind, a resolved link, an unresolvable link, `@see`, `@since` and
   `@deprecated`, on a declaration, a field, an enum value, a union arm, an
   interface and an interaction with a parameter.
2. **`just demo` runs `cargo doc`** on the cabin crate and on the corpus crate
   with `RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links"`, so a link that the
   Rust backend renders and rustdoc cannot resolve fails the gate.
3. **A lowering test** in `ridl-ir`: for every item of the fixture, each `Model`
   rule's `text` equals `rules::render` of the matching element of `rules()` for
   that item, in the same order. This keeps the editor and the generated docs in
   parity.
4. **A doc-content test** per backend: no doc comment in the generated output of
   the fixture contains an issue reference (`#` followed by digits, or
   `driftsys/ridl`) or a repository URL.

## Choices this spec adds

The handoff did not settle these. Each is the choice above; the maintainer's
review confirms or changes it.

- **S-1.** `Model.Param` gains `doc`, because it has none today. The handoff
  assumed it did.
- **S-2.** `Model.Service` is a doc carrier too, so it gains the four fields.
  The handoff listed seven carriers; there are eight.
- **S-3.** `rules` is present but always empty on `EnumValue`, `Arm`,
  `Interface` and `Service`, to keep one layout on every carrier (section 1
  approved "every message that has `doc`").
- **S-4.** The `Rule` kind messages are nested in `Rule`, so that `Rule.Timing`
  does not collide with the `Model`'s top-level `Timing`.
- **S-5.** The TypeScript backend calls `lower`, because it reads only IR v2
  today.
- **S-6.** Parameter docs render in Rust as a `# Parameters` section. The
  approved table had no row for them.
- **S-7.** The rule text is `rules::render`'s current wording: "range 0.0 ..=
  250.0" and a separate "unit km/h" line, not the handoff's illustrative "Range:
  0.0 to 250.0 km/h". Changing the wording later is one change to `render`, and
  it changes the hover and every backend together.

## Alternatives considered

D-1 to D-4 record the alternatives for each decision. Two more were considered
while writing this spec:

- **Store rules only on the four carriers that have an `ItemRef`.** Smaller, but
  each carrier would have its own layout, and section 1 was approved with one
  layout. Not taken (S-3).
- **Render parameter docs inline in the method's doc text.** Shorter, but the
  parameter's rules would have no clear owner in the rendered text. Not taken
  (S-6).

## Records to amend

| Record                                                  | Change                                                                                                                             |
| ------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| new `docs/decisions/ADR-0028-docs-in-generated-code.md` | records D-1 to D-4 and sections 1 to 3                                                                                             |
| `docs/decisions/ADR-0026-doc-comments.md`               | decision 9: the `Rule` shape is fixed and the `Model` carries it; decision 4: the §4.7 deferral is resolved by ADR-0028 (declined) |
| `docs/decisions/README.md`                              | a summary line for ADR-0028                                                                                                        |
| `docs/wip/family-general-form.md` §4.7, §7 item 2, R4   | the promotion is declined; `@labels` and `@deprecated` stay doc tags                                                               |
| `docs/design/codegen-plugins.md`                        | the new `Model` fields, and the rule that a plugin renders `text` for an unknown rule kind                                         |
| `docs/book/generated-code.md`                           | the facade, and a per-backend table of the rules that the generated code checks                                                    |
| `docs/book/codegen-plugins.md`                          | the new `Model` fields that a plugin may render                                                                                    |
| `crates/ridl-ir/proto/ridl/codegen/v1/model.proto`      | section 1                                                                                                                          |

## Dependencies and sequencing

Implementation starts only after the on-request encodings stage 1
(`docs/wip/2026-10-10-on-request-encodings-plan.md`, worktree
`.claude/worktrees/on-request-encodings-1a`) has merged, because both change
`ridl-ir` and `ridl-backend-rust`. The plan is written now and rebased onto that
merge before the first task starts.

The Kotlin heads-up issue is filed when the `Model` change merges, not before.

## Trace

- Satisfies: the brief's spec 2b
  ([`2026-10-03-devex-and-agent-tracks-brief.md`](../wip/2026-10-03-devex-and-agent-tracks-brief.md)).
- Amends: ADR-0026 decisions 4 and 9; the general form §4.7.
- Bound by: ADR-0016 (name transform, for link targets), ADR-0020 (the codegen
  plugin protocol), `docs/specification/ir-specification.md` §6 (additive
  changes).
