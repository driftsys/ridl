# Documentation in generated code — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Generated Rust and TypeScript code carries the documented contract the
editor shows on hover — doc text, links, `@see`, `@since`, `@deprecated` and the
extracted rules — on the public facade only, from one source in the codegen
`Model`.

**Architecture:** `ridl_ir::codegen::lower` copies `links`, `see` and `since`
from IR v2 into the `Model` and fills a new `rules` list on every doc carrier by
calling `ridl_ir::rules::rules`, with `rules::render`'s line as each rule's
`text`. The Rust backend renders these as rustdoc (types, then the face); the
TypeScript backend calls `lower` and renders TSDoc. Generated internals lose
their maintainer notes and issue citations.

**Tech Stack:** Rust workspace (toolchain pinned in `rust-toolchain.toml`);
`prost` + `pbjson` from `crates/ridl-ir/build.rs`; `quote` in the Rust backend;
`insta` snapshots (accept with `cargo insta review`, or
`INSTA_UPDATE=always cargo test -p <crate>`); `just` recipes as the gate.

**Spec:** `docs/wip/2026-10-10-docs-in-generated-code-design.md` (approved
2026-10-11). The spec's decisions D-1 to D-4, sections 1 to 3 and choices S-1 to
S-7 are binding; this plan does not restate their rationale.

## Before Task 1

Do not start until the on-request encodings stage 1
(`docs/wip/2026-10-10-on-request-encodings-plan.md`) has merged to `main`. Then
rebase `docs/2b-generated-docs` onto `main`, run `./bootstrap` in the worktree,
and run `just build` once to record a green baseline. After that merge, the face
is emitted only when the manifest names the `flatbuffers` encoding; every "faced
interface" below means an interface for which the face is emitted.

## Global Constraints

- IR v2 (`crates/ridl-ir/proto/ridl/ir/v2/`) does not change. Every `.ir.json`
  snapshot and every `catalog_hash` stays byte-identical.
- Every change to `crates/ridl-ir/proto/ridl/codegen/v1/model.proto` is
  additive: new fields and messages only, no renumbering, no rename.
- A backend reads doc text, links, `see`, `since`, `rules` and `deprecated` from
  the `Model` only, never from the `v2::Package`: the parity test
  (`crates/ridlc-gen-rust/tests/parity.rs`) runs the Rust backend as a plugin,
  which receives only the `Model`.
- A rule's rendered text is `v1::Rule.text`, verbatim. No backend rewords it.
- No generated doc comment contains an issue reference (`#` followed by a digit,
  or `driftsys/ridl`) or a URL.
- A link whose target is not an item in the same generated output, or is an item
  of internal visibility, renders as a code span of the qualified name:
  `` `veh.common.Speed` ``.
- Labels are not rendered.
- Shipped docs and rustdoc name no story, stage or epic (`just story-id-check`).
  Prose is plain and literal.
- `just build` passes at the end of every task.

## Review Focus

1. A doc whose text has a multibyte character (`°C`) before a link: `offset` and
   `len` are byte offsets, so the replacement must land on the link and not
   panic on a char boundary. Test in Task 2.
2. A pattern rule whose regex contains `*/`: TSDoc must still close where it
   should, so the rule line is escaped like the doc body. Test in Task 5.
3. Doc prose with brackets that are not a link (`a value in [0, 1]`): rustdoc
   must not report a broken intra-doc link. Covered by the cabin fixture in
   Task 6.
4. A link from a public item to an internal or unemitted item: it renders as a
   code span, never as a link that rustdoc reports. Test in Task 2.
5. A deprecated interaction: the generated crate itself must not draw a
   `deprecated` warning where its own dispatch code calls the deprecated method.
   Test in Task 3.

---

### Task 1: The `Model` carries links, `see`, `since` and rules

**Files:**

- Modify: `crates/ridl-ir/proto/ridl/codegen/v1/model.proto`
- Create: `crates/ridl-ir/src/codegen/docs.rs` (the doc-field lowering)
- Modify: `crates/ridl-ir/src/codegen.rs` (declare the module),
  `crates/ridl-ir/src/codegen/lower.rs` (the eight carrier builders)
- Modify: `crates/ridl-ir/src/rules.rs` (module doc: the shape is no longer
  provisional; the `Model` carries it)
- Test: `crates/ridl-ir/src/codegen/tests.rs`
- Snapshots: `crates/ridlc/tests/snapshots/corpus__codegen@*.snap` and any other
  insta snapshot of model JSON that the new empty fields change
- Create: `docs/decisions/ADR-0028-docs-in-generated-code.md`
- Modify: `docs/decisions/ADR-0026-doc-comments.md` (decisions 4 and 9 amended
  in place, with a dated amendment line in `## Status`),
  `docs/decisions/README.md` (one summary line),
  `docs/wip/family-general-form.md` (§4.7 note, §7 item 2, R4: the promotion is
  declined by ADR-0028; `@labels` and `@deprecated` stay doc tags)

**Interfaces:**

- Produces, in `ridl.codegen.v1` (Rust: `ridl_ir::codegen::v1`):
  - `message DocLink { string text = 1; uint32 offset = 2; uint32 len = 3; string target = 4; }`
  - `message Rule` exactly as the spec's section 1 gives it: nested `Range`,
    `Length`, `CollectionBound`, `Timing`, `ResponseBound`, `Errors`,
    `Contract`; `oneof kind` members 10–19; `string text = 1`.
  - On each carrier, `repeated DocLink links`, `repeated DocLink see`,
    `repeated string since`, `repeated Rule rules`, at these numbers:

    | Message       | `doc` | `links` | `see` | `since` | `rules` |
    | ------------- | ----- | ------- | ----- | ------- | ------- |
    | `Declaration` | 4     | 20      | 21    | 22      | 23      |
    | `Field`       | 5     | 8       | 9     | 10      | 11      |
    | `EnumValue`   | 3     | 4       | 5     | 6       | 7       |
    | `Arm`         | 4     | 7       | 8     | 9       | 10      |
    | `Interface`   | 4     | 20      | 21    | 22      | 23      |
    | `Interaction` | 4     | 20      | 21    | 22      | 23      |
    | `Service`     | 3     | 8       | 9     | 10      | 11      |
    | `Param`       | 3 new | 4       | 5     | 6       | 7       |

    The three messages with a `oneof` at 10 and above take 20–23, which leaves
    16–19 open for their `oneof`.
  - In `crates/ridl-ir/src/codegen/docs.rs`:
    `pub(crate) fn doc_links(links: &[v2::DocLink]) -> Vec<v1::DocLink>` and
    `pub(crate) fn model_rule(rule: &rules::Rule) -> v1::Rule` (sets `text` to
    `rules::render(rule)`).

- [ ] **Step 1: Write the failing tests** in
      `crates/ridl-ir/src/codegen/tests.rs`:
  - `doc_carrier_field_numbers`: add rows, in the style of the existing
    descriptor-based field-number test (~line 2425), for every number in the
    table above, plus `DocLink` and `Rule.text = 1`, `Rule.range = 10` …
    `Rule.contract = 19`.
  - `lowering_copies_links_see_and_since`: extend the hand-built `package()`
    fixture (or a new helper) so that one declaration, one field, one enum
    value, one union arm, one interface, one interaction, one parameter and one
    service each carry one link, one `see` and one `since`. Assert each `Model`
    carrier's `links`, `see` and `since` equal the IR's, field by field, and
    that `Param.doc` equals the IR parameter's `doc`.
  - `model_rules_text_matches_render` (spec test 3): for a fixture with a ranged
    unit type with a step, a string with a length and a pattern, an array field
    with a collection bound, a signal with timing, a fallible query with a
    response bound and a `require` and an `ensure`, assert that for every
    `Declaration`, `Field`, `Interaction` and `Param`, the list of `rule.text`
    equals `rules::rules(..).iter().map(rules::render)` for the matching IR
    item, in order, and that each rule's `oneof` member is the one matching its
    `rules::Rule` variant.
  - `rules_are_empty_where_no_item_ref_exists`: `EnumValue`, `Arm`, `Interface`
    and `Service` carry an empty `rules`.
  - `foreign_declaration_carries_rules`: a field typed by a ranged type of
    another package passed in `others` carries that type's range rule, and the
    `ForeignDeclaration`'s `Declaration` carries it too.

- [ ] **Step 2: Run them to see them fail**

  Run: `cargo test -p ridl-ir --locked codegen::tests` Expected: compile errors
  on the missing fields (the red is the missing fields; after Step 3 the
  assertions, not the compiler, must be what fails if the lowering is wrong).

- [ ] **Step 3: Add the proto messages and fields; implement `docs.rs`; call it
      from each carrier builder in `lower.rs`.** The builders already hold the
      IR item (`declaration`, `field`, the arm closure in `union`, `interface`,
      `interaction`, `params`, `services`, `enum_value`). For
      `rules(pkg, deps, item)`: `pkg` is the declaring package (`home`); `deps`
      is the lowered package followed by `others`, as `lower` received them —
      add a field to `Lowering` if no existing one holds the `&v2::Package`
      values. `enum_value` is a free function and gets only the copied links,
      `see` and `since`. Reuse the existing v2-to-v1 conversions of `TimingMode`
      and `ContractKind`. If clippy reports `large_enum_variant` on a `oneof`,
      add a `.boxed(...)` line in `crates/ridl-ir/build.rs` beside the existing
      ones.

- [ ] **Step 4: Run the tests and accept the snapshots**

  Run: `cargo test -p ridl-ir --locked`, then
  `INSTA_UPDATE=always cargo test --workspace --locked`, then
  `git diff --stat -- '*.snap'`. Expected: `ridl-ir` passes. The snapshot diff
  adds only the new keys (`links`, `see`, `since`, `rules`, and `doc` on
  params); no existing value changes, and no `.ir.json` snapshot changes. Read
  the diff before you accept it.

- [ ] **Step 5: Write ADR-0028 and the amendments.** ADR-0028 records D-1 to
      D-4, sections 1 to 3 and S-1 to S-7 of the spec, with `## Status` Accepted
      2026-10-11, `## Context`, `## Decision`, `## Alternatives considered`,
      `## Consequences`, `## References`, in the layout of ADR-0026. ADR-0026
      decision 9 says the `Model` carries `Rule` and that a change to `Rule` is
      a change to `ridl.codegen.v1`; decision 4 says ADR-0028 declines the §4.7
      promotion.

- [ ] **Step 6: Run the gate and commit**

  Run: `just build` Expected: exit 0.

  ```bash
  git add -A crates/ridl-ir crates/ridlc/tests/snapshots docs/decisions docs/wip/family-general-form.md
  git commit -m "feat(ridl-ir): carry doc links, see, since and rules in the codegen model"
  ```

### Task 2: The Rust backend renders docs on declarations, fields, enum values and union arms

**Files:**

- Create: `crates/ridl-backend-rust/src/docs.rs`
- Modify: `crates/ridl-backend-rust/src/lib.rs` (the `doc_attrs` call sites at
  ~979, ~1097, ~1611, ~1774, `decl_attrs` ~2016 and `field_attrs` ~2022)
- Test: `crates/ridl-backend-rust/src/tests.rs`, new snapshots under
  `crates/ridl-backend-rust/src/snapshots/`

**Interfaces:**

- Consumes: Task 1's `v1::DocLink`, `v1::Rule` and the four fields on each
  carrier.
- Produces, in `docs.rs`:
  - `pub(crate) struct DocParts<'a> { pub doc: &'a str, pub links: &'a [v1::DocLink], pub see: &'a [v1::DocLink], pub since: &'a [String], pub rules: &'a [v1::Rule] }`
    with constructors `DocParts::declaration(&v1::Declaration)`, `::field`,
    `::enum_value`, `::arm`, `::interface`, `::interaction`, `::param`.
  - `pub(crate) struct Links<'a>` built by
    `Links::new(model: &'a v1::Model, faced: &'a BTreeSet<u32>)`, with
    `fn path(&self, target: &str) -> Option<String>`: the rustdoc path of the
    target, or `None` when the target is not an item in this output or is
    internal. Mapping: a declaration in `model.declarations` or `model.foreign`
    → `type_path` of it (`lib.rs:1874`); `Type.member` → `<path>::<member>` with
    the member's Rust spelling (a field: `snake`; an enum value or a union arm:
    `pascal`); an interface whose index is in `faced` → its face module path;
    `Interface.member` of a faced interface → the async client method,
    `<module>::Client::<snake>`. Everything else → `None`. `faced` is empty in
    `generate`, which emits no face.
  - `pub(crate) fn doc_block(parts: &DocParts<'_>, links: &Links<'_>) -> TokenStream`:
    `#[doc = "..."]` lines, in this order, each part omitted when empty:
    1. `doc`, with each link's `len` bytes at `offset` replaced, from the last
       link to the first, by `[<text>](<path>)`, or by `` `<target>` `` when
       `path` is `None`;
    2. a blank line, `# Rules`, a blank line, then `- <text>` per rule;
    3. a blank line, `# See also`, a blank line, then `- [<text>](<path>)` or
       `` - `<target>` `` per `see`;
    4. a blank line, then `Available since <v>.` per `since` entry.

  `doc_attrs` stays for the face's fixed strings; `decl_attrs` and `field_attrs`
  call `doc_block`.

- [ ] **Step 1: Write the failing snapshot tests** in `src/tests.rs`, with
      hand-built v2 as the existing `rust_for` tests do. The fixture must
      exercise every rule kind that a declaration or a field can carry (range,
      step, unit, length, pattern, collection bound), a resolved link to a local
      type, a link to a type member, a link to a type of a package outside the
      output, a link to an internal type, `@see`, two `@since`, and
      `@deprecated`:
  - `doc_block_on_declarations_and_fields` (snapshot);
  - `doc_block_on_enum_values_and_union_arms` (snapshot);
  - `doc_link_after_multibyte_text` (Review Focus 1): doc
    `"Measured in °C, see [Speed]."`; assert the output contains
    `see [Speed](crate::veh::common::Speed).` exactly;
  - `doc_link_to_internal_or_absent_target_is_a_code_span` (Review Focus 4):
    assert the output contains `` `veh.common.Hidden` `` and
    `` `other.pkg.Thing` `` and no `](` for either.

- [ ] **Step 2: Run them to see them fail**

  Run: `cargo test -p ridl-backend-rust --locked --lib doc_` Expected: FAIL —
  the snapshots are new and the asserted strings are absent.

- [ ] **Step 3: Implement `docs.rs` and switch the six call sites.**

- [ ] **Step 4: Run the tests; review and accept the snapshots**

  Run: `INSTA_UPDATE=always cargo test -p ridl-backend-rust --locked`, then
  `cargo test --workspace --locked`. Expected: PASS. Existing snapshots change
  only where a fixture item has links, `see`, `since` or rules. Review every
  changed `.snap`, including the corpus `corpus__rust@*` and
  `golden__generated_rust.snap`.

- [ ] **Step 5: Gate and commit**

  Run: `just build` — Expected: exit 0.

  ```bash
  git commit -am "feat(ridl-backend-rust): render doc links, rules, see and since on generated types"
  ```

### Task 3: The Rust face carries interface, interaction and parameter docs

**Files:**

- Modify: `crates/ridl-backend-rust/src/face.rs` (`one_interface` ~165, the
  module doc ~266, the async client's signal method ~383, `call_method` ~743,
  `provider` ~949, `Publisher` ~926),
  `crates/ridl-backend-rust/src/face/blocking.rs` (`client` ~102, `call_method`
  ~312)
- Modify: `crates/ridl-backend-rust/tests/fixtures/interaction_face.ridl` and
  the checked-in `crates/ridl-backend-rust/tests/generated/interaction_face.rs`
  (regenerated as `tests/interaction_face_regeneration.rs` says)
- Test: `crates/ridl-backend-rust/tests/` (the test that `include!`s the
  generated face)

**Interfaces:**

- Consumes: Task 2's `DocParts`, `Links` and `doc_block`.
- Produces: doc text on the face, by this rule. The interface module's doc is
  `doc_block` of the `Interface`, then a blank line, then the existing generated
  sentence. Each method a user calls or implements for an interaction — the
  async client method, the blocking client method, the `Provider` trait method,
  and the `Publisher` method of a signal or event — carries `doc_block` of the
  `Interaction`, then the existing generated sentence as its own paragraph,
  then, for a call with a parameter, a `# Parameters` section: a blank line,
  `# Parameters`, a blank line, then ``- `<snake name>`: <param doc>`` with each
  of the parameter's rule texts as a nested `- <text>` bullet. An interaction
  with `deprecated` puts `#[deprecated = "<reason>"]` (or a bare `#[deprecated]`
  for an empty reason) on the client methods and the `Provider` method, and
  every generated item that calls one of them carries `#[allow(deprecated)]`.

- [ ] **Step 1: Extend the fixture and write the failing tests.** Give
      `interaction_face.ridl` doc comments on its interface, a signal with
      timing, a command with a documented parameter whose type is ranged, a
      fallible query with a response bound, a `require` and an `ensure`, a link
      to a type, an `@see`, an `@since`, and one `@deprecated` command. Tests:
  - `face_methods_carry_interaction_rules`: the regenerated file contains, on
    the async client method, the blocking client method and the `Provider`
    method of the query, the lines `# Rules`, the response-bound text, the
    errors text and the `require` text exactly as `rules::render` gives them;
  - `face_call_carries_parameter_section`: the command's three methods contain
    `# Parameters` and the parameter's range text;
  - `deprecated_interaction_does_not_warn_inside_the_generated_crate` (Review
    Focus 5): the test crate that `include!`s the generated face is compiled
    with `#![deny(deprecated)]` at the include site's module, and builds.

- [ ] **Step 2: Run them to see them fail**

  Run: `cargo test -p ridl-backend-rust --locked --tests` Expected: FAIL on the
  missing doc lines. The deprecation test passes now, because nothing is
  deprecated yet; it guards Step 3, which emits `#[deprecated]`.

- [ ] **Step 3: Implement.** Build one `Links` per generated output from the
      `Model` and the set of faced interface indices that `generate_face_with`
      and `generate_pipeline` already compute.

- [ ] **Step 4: Regenerate, run, review**

  Run: the regeneration command named in
  `tests/interaction_face_regeneration.rs`, then
  `INSTA_UPDATE=always cargo test --workspace --locked`. Expected: PASS; review
  the diff of `tests/generated/interaction_face.rs` and of every changed
  snapshot.

- [ ] **Step 5: Gate and commit**

  Run: `just build` — Expected: exit 0.

  ```bash
  git commit -am "feat(ridl-backend-rust): document the face from the interface and interaction docs"
  ```

### Task 4: Rust internals carry no maintainer notes

**Files:**

- Modify: `crates/ridl-backend-rust/src/descriptors.rs` (`clause_doc` ~541),
  `crates/ridl-backend-rust/src/lib.rs` (`skipped_interface_note` ~272, the
  `FaceGap` text ~290–296), `crates/ridl-backend-rust/src/codec.rs` (the view
  and accessor docs ~915–935, ~1470, ~1537, ~1634, ~1834, ~2082, ~2308–2345,
  ~2635–2752), `crates/ridlc/src/lib.rs` (~1552, `__ridl_package`)
- Test: `crates/ridl-backend-rust/tests/generated_docs.rs` (new)

**Interfaces:**

- Consumes: the outputs of Tasks 2 and 3.
- Produces: the facade/internal split, decided item by item by this rule. It
  reads the spec's facade list ("the interface module; the two clients, `serve`,
  `prelude` and `blocking`") as including the `pub` items inside the interface
  module that a user names. An item is **facade** when a user's code names it: a
  declared type, its fields, enum values and union arms; the interface module
  and every `pub` item a facade signature or `prelude` names (`Client`,
  `blocking`, `serve`, `Provider`, `Publisher`, `Event`, `Subscribe`,
  `Invalidate`, the call futures, `Serve`); and every item
  `docs/book/generated-code.md` or `docs/book/writing-a-port.md` tells a user to
  name (check the descriptors against the port chapter). Everything else is
  **internal**: `pub(crate)` if the generated crate still compiles and the
  corpus and cabin consumers still build, otherwise `#[doc(hidden)]`. An
  internal carries no `#[doc]`; the explanation moves to a `//` comment beside
  the generator code that emits it. A facade item keeps a doc that states what
  the item is for its user, with no issue reference and no description of the
  encoding internals. Record each item's disposition in the pull request
  description.

- [ ] **Step 1: Write the failing test** `generated_docs.rs` (spec test 4): for
      `tests/generated/interaction_face.rs`, for the output of `generate` and
      `generate_pipeline` over every fixture in `tests/fixtures/`, and for the
      generated skip note of an interface whose clause the translator refuses,
      collect every `#[doc = ...]` string and every `///` line and assert none
      matches `#[0-9]`, `driftsys/ridl` or `://`.

- [ ] **Step 2: Run it to see it fail**

  Run: `cargo test -p ridl-backend-rust --locked --test generated_docs`
  Expected: FAIL on the `#704` and `#713` text in `clause_doc` and the skip
  note.

- [ ] **Step 3: Apply the split.** Remove the issue citations from `clause_doc`
      and from the skip note's lines, keeping what the refusal is and what the
      user can do. Make each internal item `pub(crate)` or `#[doc(hidden)]` with
      no doc. Confirm that `ridlc` already writes `#[doc(hidden)]` on
      `pub mod __ridl_package;` and add it if not. Keep `tests.rs:6235-6252`
      (the note carries neither #713 nor #704) passing.

- [ ] **Step 4: Run**

  Run: `INSTA_UPDATE=always cargo test --workspace --locked`, then `just demo`.
  Expected: PASS; the demo's consumer still builds, so no item a consumer names
  went `pub(crate)`.

- [ ] **Step 5: Gate and commit**

  Run: `just build` — Expected: exit 0.

  ```bash
  git commit -am "fix(ridl-backend-rust): hide generated internals and drop maintainer notes from generated docs"
  ```

### Task 5: The TypeScript backend renders TSDoc from the `Model`

**Files:**

- Modify: `crates/ridl-backend-ts/src/lib.rs` (`generate` ~82 calls `lower`;
  `jsdoc` ~981 gains the new parts; the call sites at ~220, 240, 307, 352, 366,
  380, 404, 428, 444, 454)
- Modify: `crates/ridl-backend-ts/tests/model_drift.rs` (its comment that no
  backend reads the model)
- Test: `crates/ridl-backend-ts/src/tests.rs`, new snapshots

**Interfaces:**

- Consumes: Task 1's `Model` fields.
- Produces:
  `fn jsdoc(indent: &str, parts: &TsDocParts<'_>, tags: &[String]) -> String`
  where `TsDocParts` holds `doc`, `links`, `see`, `since`, `rules`, and the link
  resolver `fn ts_link(model: &v1::Model, target: &str) -> Option<String>`: a
  local declaration → its TypeScript name; a foreign declaration → the import
  alias form `<DottedName.underscored>.<Name>`; `Type.member` →
  `<name>.<member>` with the member's TypeScript spelling; an interface or an
  interaction, or any other target → `None`. The comment body, in order: the doc
  with each link replaced by `{@link <name>}` or `` `<target>` ``; when rules
  exist, a blank line, `@remarks`, `Rules:`, then `- <text>` per rule; then the
  existing tags (`@unit`, `@range`, `@bounds`) unchanged; `@see {@link <name>}`
  or `` @see `<target>` `` per `see`; `@since <v>` per entry; `@deprecated` as
  today. Every line, rules included, goes through the existing `*/` escape. The
  TypeScript output exports no internal helper today; if Step 3 finds one, it is
  not exported or carries `@internal`.

- [ ] **Step 1: Write the failing snapshot tests** with hand-built v2, as the
      existing `ts_for` tests do: one per carrier kind with every rule kind a
      type or field carries, a local link, a foreign link, an unresolvable link,
      `@see`, `@since`, `@deprecated`; and
      `pattern_rule_with_comment_close_is_escaped` (Review Focus 2): a string
      type with pattern `a*/b`; assert the output has exactly one `*/` per
      comment block, at its end; and `ts_docs_carry_no_issue_reference` (spec
      test 4): over every snapshot fixture's output, no comment line matches
      `#[0-9]`, `driftsys/ridl` or `://`.

- [ ] **Step 2: Run them to see them fail**

  Run: `cargo test -p ridl-backend-ts --locked` Expected: FAIL — new snapshots,
  missing lines.

- [ ] **Step 3: Implement**: `generate` calls
      `ridl_ir::codegen::lower(package, &[])` (the `contract.rs` `Backend` path
      passes the request's model instead) and reads every doc fact from the
      `Model`.

- [ ] **Step 4: Run, review and accept snapshots**

  Run: `INSTA_UPDATE=always cargo test --workspace --locked`. Expected: PASS;
  existing snapshots change only where a fixture item has links, `see`, `since`
  or rules.

- [ ] **Step 5: Gate and commit**

  Run: `just build` — Expected: exit 0.

  ```bash
  git commit -am "feat(ridl-backend-ts): render doc links, rules, see and since as TSDoc"
  ```

### Task 6: `just demo` checks the generated rustdoc

**Files:**

- Modify: `justfile` (`demo`, ~492–611; its comment block above)
- Modify: `examples/cabin/cabin.ridl` (doc comments that exercise the gate)
- Modify: `AGENTS.md` (the `just demo` line in "Commands")

**Interfaces:**

- Consumes: the outputs of Tasks 2 to 4.
- Produces: in `demo`, after the clippy runs, for each of `-p veh_cabin` and
  `-p ridl_generated`:
  `RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links -D rustdoc::private_intra_doc_links" cargo doc --locked --no-deps --manifest-path examples/cabin/Cargo.toml -p <crate>`.

- [ ] **Step 1: Make the failing case.** Add to `cabin.ridl` a doc link to a
      type, a link to a type member, an `@see`, an `@since`, and the prose
      `a value in [0, 1]` (Review Focus 3). Temporarily make the Rust backend
      render one link as `[x](crate::does_not_exist)` and run the new
      `cargo doc` line by hand. Expected: FAIL with `unresolved link`. Revert
      that change.

- [ ] **Step 2: Add the two `cargo doc` lines to `demo`.**

- [ ] **Step 3: Run**

  Run: `just demo` Expected: exit 0, with the round-trip lines unchanged.

- [ ] **Step 4: Gate and commit**

  Run: `just build` — Expected: exit 0.

  ```bash
  git commit -am "build(just): check the generated rustdoc links in just demo"
  ```

  Use the commit type and scope that `.git-std.toml` allows for a justfile
  change; `just lint-commits` must pass.

### Task 7: The book states what the generated docs carry and what each backend checks

**Files:**

- Modify: `docs/book/generated-code.md` (a new
  `## Documentation in the
  generated code` section before
  `## Further reading`)
- Modify: `docs/book/codegen-plugins.md` (the new `Model` doc fields and the
  `text` fallback)
- Modify: `docs/design/codegen-plugins.md` §2 (one paragraph: the doc fields on
  every carrier, and that a plugin renders `Rule.text` for an unknown `oneof`
  member; link to the book page for the details)

**Interfaces:**

- Consumes: everything above.
- Produces: in `generated-code.md`, what a generated item's doc carries and in
  what order (Tasks 2, 3, 5); that the docs state the contract and do not say
  whether the code checks it (D-3); that internals are hidden; and a table with
  one row per rule kind (range, step, unit, length, pattern, collection bound,
  timing, response bound, errors, `require`, `ensure`) and one column per
  backend (Rust, TypeScript), each cell "checked by <generated item>" or "not
  checked". Fill each cell from the code, and cite the generated item that
  checks it (for example a named scalar's constructor, the face's `require`
  method); a cell you cannot prove from the code is "not checked". In
  `codegen-plugins.md`: the fields `links`, `see`, `since`, `rules` on every doc
  carrier, `DocLink.target` as a qualified name to map with the plugin's own
  name transform, and that a plugin renders `Rule.text` for a `oneof` member it
  does not know.

- [ ] **Step 1: Write the two sections.**

- [ ] **Step 2: Run the doc gates**

  Run:
  `just book-check && just link-check && just doc-path-check && just story-id-check && just check`
  Expected: exit 0 for each.

- [ ] **Step 3: Gate and commit**

  Run: `just build` — Expected: exit 0.

  ```bash
  git commit -am "docs(book): describe the docs in generated code and what each backend checks"
  ```

## After the last task

- Run the branch-level review (`/review` passes 1 and 2), then `sdd-gardening`
  before the pull request: the spec and this plan are archived, and ADR-0028 is
  already the durable record.
- After the merge, ask the maintainer before filing the heads-up issue in the
  Kotlin plugin repository (the spec's KDoc column, the new `Model` fields, the
  `text` fallback).
