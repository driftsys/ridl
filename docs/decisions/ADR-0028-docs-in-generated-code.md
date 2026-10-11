# ADR-0028 — Documentation in generated code: the codegen model carries the doc links and the rules, and every backend renders one contract

## Status

Accepted — 2026-10-11. Scope: what the codegen `Model` carries of an item's
documentation (doc links, `@see`, `@since` and the rules of the item), how each
backend renders it in generated code, which generated items carry documentation,
and the fate of the general form §4.7 promotion of `labels` and `deprecated`. It
binds `ridl.codegen.v1`, every in-tree backend, every codegen plugin that
renders documentation, and `ridl_ir::rules`, whose `Rule` shape the `Model` now
carries.

Written from spec 2b of the devex and agent tracks brief, "documentation in
generated code". Sebastien approved decisions D-1 to D-4 and the design's
sections 1 to 3 in the brainstorming session of 2026-10-10. Writing the design
required seven more choices, S-1 to S-7, which it lists for review. In this
record decisions 1 to 4 are D-1 to D-4, decision 5 is section 1 with S-1 to S-4,
decision 6 is section 2 with S-5 to S-7, and decision 7 is section 3. The design
is
[`docs/wip/2026-10-10-docs-in-generated-code-design.md`](../wip/2026-10-10-docs-in-generated-code-design.md).

The `Model` fields of decision 5 are implemented. The rendering of decisions 6
and 7 is implemented in the backends by later changes of the same plan.

It amends three records in place, in the same change:

- [ADR-0026](ADR-0026-doc-comments.md) decision 4: the §4.7 promotion is
  declined (decision 4 below). Its decision 6: the codegen model now carries the
  doc links, `see` and `since`. Its decision 9: the shape of `Rule` is fixed,
  and the `Model` carries it (decision 5 below).
- [The general form](../wip/family-general-form.md) §4.1, §4.7, §7 item 2, the
  R4 invariant and the decisions ledger: `@labels` and `@deprecated` stay doc
  tags.

## Context

ADR-0026 made every named declaration and member a doc comment carrier, and IR
v2 carries each carrier's `doc`, its resolved `links`, its `see` targets and its
`since` versions. `ridl_ir::rules::rules(pkg, deps, item)` lists the rules of a
declaration, an interaction, a struct field or a parameter (range, step, unit,
length, pattern, collection bound, timing, response bound, errors, `require` and
`ensure`), and `rules::render` gives one line of text for each. Only the
language server's hover called either function, and ADR-0026 decision 9 left the
shape of `Rule` provisional until a consumer in generated code was chosen.

The codegen `Model` (`crates/ridl-ir/proto/ridl/codegen/v1/model.proto`) is what
the in-tree backends and the out-of-process plugins read. It carried `doc` on
seven messages, and `labels` and `deprecated` on five of them, but no links, no
`see`, no `since` and no rules. `Model.Param` had no `doc`. The Rust backend
emitted `///` docs and `#[deprecated]`, and no rules or links; the TypeScript
backend read only IR v2 and emitted JSDoc with `@deprecated`, `@unit`, `@range`
and `@bounds`. Some generated docs described internals of the generated code,
and some cited an issue of this repository.

The general form §4.7 proposed to move `labels` and `deprecated` from doc tags
to attribute keys. ADR-0026 decision 4 deferred that choice to this record,
because the first consumer that would need it is generated deprecation metadata.

## Decision

1. **The rules live in the codegen `Model`** (design D-1). `lower` calls
   `ridl_ir::rules::rules` once for each item that has an `ItemRef`, and writes
   the result into the `Model`. `pkg` is the package that declares the item;
   `deps` is the lowered package followed by the other packages the lowering was
   handed. IR v2 does not change, so `catalog_hash` and `ridl diff` are
   unaffected, and the `Model` is the one source for every backend and plugin.

2. **Each rule carries structured fields and one canonical text line** (design
   D-2). The text is `rules::render`'s line for that rule, the same text the
   hover shows, and a backend places it verbatim. The structured fields stay for
   tooling and for the TypeScript tags. The wording is `render`'s current one,
   for example "range 0.0 ..= 250.0" and a separate "unit km/h" line (design
   S-7). A change to the wording is one change to `render`, and it changes the
   hover and every backend together.

3. **The docs state the contract, not what a backend enforces** (design D-3).
   Every backend renders every rule that the `Model` declares, in neutral
   wording ("range 0.0 ..= 250.0", never "this function rejects"). Which rules
   the generated code of a backend checks is recorded once per backend in the
   book, not per item.

4. **`deprecated` and `labels` stay doc tags** (design D-4). The general form
   §4.7 promotion of `labels` and `deprecated` to attribute keys is declined,
   not deferred again. Deprecation metadata in generated code comes from the
   `@deprecated` doc tag, as it does today. There is no grammar change. An
   attribute block exists only on `signal`, `event`, `command` and `query` and
   on the rsdl declarations; `type`, `const`, `struct`, `enum`, `enumset`,
   `union`, `interface`, `service`, fields, enum values and `fixed` have none.
   The promotion would add the attribute block to the grammar on these eleven
   carriers to move two facts that the doc tags already deliver to every
   consumer.

5. **What the `Model` carries** (design section 1, S-1 to S-4). Every `Model`
   message that carries `doc` gains `repeated DocLink links`,
   `repeated DocLink see`, `repeated string since` and `repeated Rule rules`.
   The carriers are `Declaration`, `Field`, `EnumValue`, `Arm`, `Interface`,
   `Interaction`, `Service` and `Param`. `Service` is a carrier like the others
   (S-2). `Param` first gains `string doc`, copied from `v2::Param.doc` (S-1).
   The field numbers are:

   | Message       | `doc` | `links` | `see` | `since` | `rules` |
   | ------------- | ----- | ------- | ----- | ------- | ------- |
   | `Declaration` | 4     | 20      | 21    | 22      | 23      |
   | `Field`       | 5     | 8       | 9     | 10      | 11      |
   | `EnumValue`   | 3     | 4       | 5     | 6       | 7       |
   | `Arm`         | 4     | 7       | 8     | 9       | 10      |
   | `Interface`   | 4     | 20      | 21    | 22      | 23      |
   | `Interaction` | 4     | 20      | 21    | 22      | 23      |
   | `Service`     | 3     | 8       | 9     | 10      | 11      |
   | `Param`       | 3     | 4       | 5     | 6       | 7       |

   The three messages with a `oneof` at 10 and above take 20 to 23, which leaves
   16 to 19 open for their `oneof`.

   `lower` copies `links`, `see` and `since` from the IR item that the carrier
   is lowered from, and fills `rules` as follows:

   | Carrier                                    | `rules` holds                                    |
   | ------------------------------------------ | ------------------------------------------------ |
   | `Declaration`, `Interaction`               | `rules(pkg, deps, ItemRef::Decl(decl))`          |
   | `Field`                                    | `rules(pkg, deps, ItemRef::Field(field))`        |
   | `Param`                                    | `rules(pkg, deps, ItemRef::Param(param))`        |
   | `EnumValue`, `Arm`, `Interface`, `Service` | always empty: `rules()` has no `ItemRef` for one |

   The four carriers that are always empty keep the field, so that every doc
   carrier has one layout and a later `ItemRef` can fill it additively (S-3). A
   `ForeignDeclaration` holds a `Declaration`, so a foreign declaration carries
   its rules and doc links too. The field of an induced tuple carries none of
   them, because the IR's tuple field has no doc and `rules()` has no `ItemRef`
   for it.

   `DocLink` has the fields of `ridl.ir.v2.DocLink`: `text = 1`, `offset = 2`,
   `len = 3` and `target = 4`. `target` is the canonical qualified name,
   `pkg.Name` with an optional `.member`, not a language spelling; each backend
   maps it to its own path with its name transform (ADR-0016). `offset` and
   `len` index the carrier's `doc` in the `Model`, which `lower` copies from the
   IR byte for byte.

   `Rule` has `string text = 1` and a `oneof kind` with the members `range`
   (10), `step` (11), `unit` (12), `length` (13), `pattern` (14),
   `collection_bound` (15), `timing` (16), `response_bound` (17), `errors` (18)
   and `contract` (19). Their messages, `Range`, `Length`, `CollectionBound`,
   `Timing`, `ResponseBound`, `Errors` and `Contract`, are nested in `Rule`, so
   that `Rule.Timing` does not collide with the `Model`'s top-level `Timing`
   (S-4). `Rule.Timing.mode` and `Rule.Contract.kind` use the `Model`'s existing
   `TimingMode` and `ContractKind`. A new rule kind later is a new `oneof`
   member, which is additive under the IR specification §6; a plugin built
   against an older `Model` renders `text` for a member it does not know.

6. **Rendering per backend** (design section 2, S-5 and S-6).

   | Source        | Rust (rustdoc)                                  | TypeScript (TSDoc)                                                                    | Kotlin plugin (KDoc) |
   | ------------- | ----------------------------------------------- | ------------------------------------------------------------------------------------- | -------------------- |
   | doc text      | `///` lines                                     | `/** */`                                                                              | `/** */`             |
   | `[Name]` link | an intra-doc link to the target's Rust path     | `{@link Name}`                                                                        | `[Name]`             |
   | rules         | a `# Rules` section, one bullet per rule `text` | a `Rules:` list under `@remarks`; the existing `@unit`, `@range`, `@bounds` tags stay | a `Rules:` paragraph |
   | `@see`        | a `# See also` list of links                    | `@see {@link X}`                                                                      | `@see X`             |
   | `@since`      | one `Available since X.` line per version       | `@since X`                                                                            | `@since X`           |
   | `@deprecated` | `#[deprecated(note = ...)]`                     | `@deprecated reason`                                                                  | `@Deprecated`        |
   | `@labels`     | not rendered                                    | not rendered                                                                          | not rendered         |
   - A backend replaces the `len` bytes at `offset` in `doc` with its link
     syntax, from the last link to the first, so that each earlier offset stays
     valid.
   - A link whose target is not an item in the same generated output renders as
     a code span of the qualified name, never as a link that would not resolve.
   - Labels are metadata for tooling and are not rendered.
   - The rules of an interaction go on every public item a user calls or
     implements for that interaction: the method of each of the two clients, and
     the method of the `serve` trait.
   - In the Rust face, a call's method renders the doc and the rules of each
     parameter under a `# Parameters` section: one bullet with the parameter's
     name and doc text, and its rule lines as nested bullets (S-6). TypeScript
     emits no face, so it has no parameter to render; the Kotlin plugin uses
     `@param`.
   - The TypeScript backend calls `lower` to obtain the `Model`, as the Rust
     backend does, and reads `doc`, `links`, `see`, `since` and `rules` from it
     (S-5). Its `@unit`, `@range` and `@bounds` tags keep their current source.
   - The KDoc column is guidance for the out-of-repository Kotlin plugin, whose
     repository receives a heads-up issue when the `Model` change merges.

7. **The facade, the internals and maintainer notes** (design section 3). The
   facade is every generated item that a user names in their own code: the
   declared types, their fields, enum values and union arms; the interface
   module; the two clients, `serve`, `prelude` and `blocking`. Only facade items
   carry doc text and rules. Every other generated item is an internal: in Rust
   it is `pub(crate)` where the crate compiles with it, and `#[doc(hidden)]`
   where it must stay `pub`; in TypeScript it is not exported, or is exported
   with `@internal` where another generated module imports it. Internals carry
   no doc text; the explanation they carry today moves to `//` comments in the
   generator source. A generated doc cites no issue and no repository URL. A
   refusal's own error message is not a doc comment and is outside this rule.
   The book's chapter on generated code records what each backend checks.

## Alternatives considered

From the design. The numbers are the decisions that reject them.

- **Store the rules in IR v2 as well.** The rules are derived data that the
  catalog hash would have to exclude, and the same fact would be stated twice.
  **Each backend calls `rules()` itself.** The Kotlin plugin would have to
  re-implement the extraction, and the wording would drift between languages.
  Rejected for decision 1.
- **Structured fields only.** Each backend would word the rule itself, and the
  wording would drift. **Text only.** The TypeScript `@unit`, `@range` and
  `@bounds` tags would lose their source. Rejected for decision 2.
- **Render only the rules a backend enforces.** The TypeScript output would show
  almost nothing, and the docs would differ by backend. **Mark each rule as
  checked or not checked.** The `Model` would need a fact per backend. Rejected
  for decision 3.
- **The §4.7 promotion.** It adds the attribute block to the grammar on eleven
  carriers to move two facts that the doc tags already deliver. Rejected for
  decision 4.
- **Store rules only on the four carriers that have an `ItemRef`.** Smaller, but
  each carrier would have its own layout. Rejected for decision 5 (S-3).
- **Render parameter docs inline in the method's doc text.** Shorter, but the
  parameter's rules would have no clear owner in the rendered text. Rejected for
  decision 6 (S-6).

## Consequences

- The `Model` grows by additive fields only. A plugin's lenient reader ignores a
  key it does not know, and a plugin built against an older `Model` reads the
  rest unchanged. IR v2, `catalog_hash` and `ridl diff` do not change.
- The shape of `ridl_ir::rules::Rule` is no longer provisional. A change to
  `Rule` or to `rules::render` is a change to `ridl.codegen.v1`, and it changes
  the hover and the generated docs together.
- The `--emit codegen-model` output and the codegen request carry the new keys:
  every doc carrier has `links`, `see`, `since` and `rules`, empty or not, and
  every parameter has `doc`.
- `@labels` and `@deprecated` stay doc tags, and the grammar does not change.
  The general form §4.7 records the proposal as declined.
- The generated docs of a backend change when it renders decision 6: rules,
  links, `@see` and `@since` appear on facade items, and docs that described
  internals or cited an issue are removed.

## References

- [ADR-0026](ADR-0026-doc-comments.md) — doc comments; decisions 4, 6 and 9
  amended.
- [ADR-0014](ADR-0014-ir-encodings.md) — the encodings of the IR and the
  `Model`.
- [ADR-0016](ADR-0016-schema-projection-and-the-name-transform.md) — the name
  transform that maps a link target to a language path.
- [ADR-0020](ADR-0020-third-encoding-runtime-layering-and-plugin-system.md) —
  the codegen plugin protocol.
- [The IR specification](../specification/ir-specification.md) §6 — additive
  changes.
- [The general form](../wip/family-general-form.md) §4.7 — the declined
  promotion.
- [The design](../wip/2026-10-10-docs-in-generated-code-design.md) — spec 2b.
