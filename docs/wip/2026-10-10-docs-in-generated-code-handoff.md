# Spec 2b, documentation in generated code — brainstorm handoff

Brainstormed with the maintainer on 2026-10-10. The design is approved in
conversation, section by section. The next step is to write the spec at
`docs/wip/2026-10-10-docs-in-generated-code-design.md` from this note, with the
`superpowers:brainstorming` skill's "After the Design" steps (spec self-review,
then the maintainer reviews the written spec, then `superpowers:writing-plans`).
**Do not ask the decisions below again.** They are settled.

Inputs: the scope brief
[`2026-10-03-devex-and-agent-tracks-brief.md`](2026-10-03-devex-and-agent-tracks-brief.md)
(section "Spec 2b"), [ADR-0026](../decisions/ADR-0026-doc-comments.md) (decision
9 and the deferral of the general form §4.7 promotion), and
`crates/ridl-ir/src/rules.rs`.

## Facts found before the design

- `ridl_ir::rules::rules(pkg, deps, item) -> Vec<Rule>` is a pure function over
  `v2::Package`; `rules::render` gives one Markdown line per rule. Only the
  language server's hover calls it. Nothing about rules is stored in the IR.
- The codegen `Model` (`crates/ridl-ir/proto/ridl/codegen/v1/model.proto`) is
  what in-tree backends and out-of-process plugins receive. It carries `doc`,
  `labels` and `deprecated` on `Declaration`, `Field`, `Interface` and
  `Interaction`, and `doc` on `EnumValue`, `Arm` and `Param`. It carries no
  links, `@see`, `@since` or rules.
- The Rust backend emits `///` docs and `#[deprecated]`, and no rules or links.
  The TypeScript backend emits JSDoc with `@deprecated` and the tags `@unit`,
  `@range` and `@bounds`, read from constraints.
- Some generated docs describe internals ("A zero-copy accessor over FlatBuffers
  bytes") or cite issues (refusal gap notes citing #704).
- Adding an optional field, a message or a `oneof` member is additive under
  `docs/specification/ir-specification.md` §6.

## Decisions

- **D-1 Rules live in the codegen `Model`.** ridlc's lowering calls
  `ridl_ir::rules` once per item and writes the result into the `Model`. IR v2
  does not change, so `catalog_hash` and `ridl diff` are unaffected. Rejected:
  storing rules in IR v2 as well (derived data the hash must clear, stated
  twice); each backend recomputing them (the Kotlin plugin would re-implement
  extraction, and wording would drift).
- **D-2 Each rule carries structured fields and one canonical text line.** The
  text is `rules::render`'s line, the same text the hover shows. Backends place
  it verbatim. The structured fields stay for tooling and for the TypeScript
  tags. Rejected: structured only (wording drifts per language); text only (the
  TypeScript tags lose their source).
- **D-3 Docs state the contract, not enforcement.** Every backend renders every
  rule the model declares, in neutral wording ("Range: 0.0 to 250.0 km/h"),
  never "this function rejects". Which rules a backend's generated code checks
  is recorded in the book, per backend, not per item. Rejected: only enforced
  rules (TypeScript would render almost nothing; docs would differ by backend);
  marking each rule checked or not checked (needs a per-backend fact in the
  `Model`).
- **D-4 `deprecated` and `labels` stay doc tags.** The general form §4.7
  promotion to attribute keys is declined, not deferred again. No grammar
  change. Record it in ADR-0026 and amend the general form text.

## Section 1 — what the `Model` carries (approved)

Every `Model` message that has `doc` (`Declaration`, `Field`, `EnumValue`,
`Arm`, `Interface`, `Interaction`, `Param`) gains, next to it, the same flat
layout IR v2 uses:

- `repeated DocLink links`: resolved in-text links, as an offset and a length
  into `doc`;
- `repeated DocLink see`;
- `repeated string since`;
- `repeated Rule rules`.

`Rule` holds a `oneof` with one member per rule kind (`Range`, `Step`, `Unit`,
`Length`, `Pattern`, `CollectionBound`, `Timing`, `ResponseBound`, `Errors`,
`Contract`, with today's `ridl_ir::rules::Rule` fields) and a `string text`. A
new rule kind later is a new `oneof` member (additive); a plugin built against
an older `Model` still renders `text` for a kind it does not know.

`DocLink` names its target in the `Model`'s existing qualified-name form, not a
language spelling; each backend applies its own name transform (ADR-0016).

A field or parameter whose type is a named declaration repeats that
declaration's rules, as `rules()` already returns. An interaction carries its
timing, response bound, errors and contracts. The `Rule` shape stops being
provisional: amend ADR-0026 decision 9.

## Section 2 — rendering per backend (approved)

| Source        | Rust (rustdoc)                                  | TypeScript (TSDoc)                                                                  | Kotlin plugin (KDoc) |
| ------------- | ----------------------------------------------- | ----------------------------------------------------------------------------------- | -------------------- |
| doc text      | `///` lines, as today                           | `/** */`, as today                                                                  | `/** */`             |
| `[Name]` link | intra-doc link to the target's Rust path        | `{@link Name}`                                                                      | `[Name]`             |
| rules         | a `# Rules` section, one bullet per rule `text` | a `Rules:` list under `@remarks`; the existing `@unit`/`@range`/`@bounds` tags stay | a `Rules:` paragraph |
| `@see`        | a `# See also` list of links                    | `@see {@link X}`                                                                    | `@see X`             |
| `@since`      | an `Available since X.` line                    | `@since X`                                                                          | `@since X`           |
| `@deprecated` | `#[deprecated(note = ...)]`, as today           | `@deprecated reason`                                                                | `@Deprecated`        |
| `@labels`     | not rendered                                    | not rendered                                                                        | not rendered         |

- A link whose target is not in the generated output (for example a type in a
  dependency package that this crate does not generate) renders as a code span
  with the qualified name, not a dead link.
- Labels are metadata for tooling; the brief asks only for docs and rules.
- Interaction rules go on every public item a user calls or implements for that
  interaction: the client methods and the `serve` trait method.
- The KDoc column is guidance for the out-of-repo plugin; its repository gets a
  heads-up issue, as earlier `Model` changes did.

## Section 3 — internals, maintainer notes, testing (approved)

- **Facade:** every generated item a user names in their own code — declared
  types and their fields, enum values and union arms; the interface module; the
  two clients, `serve`, `prelude` and `blocking`. Only these carry doc text and
  rules.
- **Internals:** in Rust, `pub(crate)` where possible; `#[doc(hidden)]` where an
  item must be `pub` for a technical reason (`__ridl_package`). In TypeScript,
  not exported, or exported with `@internal`. Internals carry no doc text. Their
  explanatory docs move to `//` comments in the generator source, next to the
  code that emits them.
- **Gap notes** that cite issues (for example #704) are removed from generated
  docs, following D-3; the book records what each backend checks.
- **Tests:**
  1. a snapshot per backend over one fixture that exercises every rule kind, a
     link, an unresolved link, `@see`, `@since` and `@deprecated`;
  2. `just demo` runs `cargo doc` on the cabin crate and the corpus crate with
     `-D rustdoc::broken_intra_doc_links`;
  3. a lowering test: each item's `Model.rules[].text` equals `rules::render` of
     `rules()` for that item (editor and facade parity);
  4. no doc comment in generated output contains an issue reference or a
     repository URL.
- **Records:** a new ADR-0028 records D-1 to D-4 and sections 1 to 3; amend
  ADR-0026 decision 9 and the general form §4.7; the book's generated-code
  chapter gains the per-backend "what is checked" table.

## Sequencing

Write the spec and the plan now. Start the implementation only after the
on-request encodings stage 1 (`.claude/worktrees/on-request-encodings-1a`,
`docs/wip/2026-10-10-on-request-encodings-plan.md`) has merged, because both
change `ridl-ir` and `ridl-backend-rust`.
