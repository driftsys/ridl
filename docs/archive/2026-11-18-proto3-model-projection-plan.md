# The proto3 projection in the codegen model — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development to implement this plan task by task.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `ridl.codegen.v1.Model` carries the proto3 layout and the proto3
refusals of a package, and the Rust proto3 codec reads them from the model.

**Architecture:** One walk over the lowered model, in
`crates/ridl-ir/src/codegen/proto3.rs`, fills `Model.proto3`. Pull request 1
adds the walk and proves it equal to the codec's own layout over the corpus.
Pull request 2 makes `codec_proto3.rs` read the projection and removes the
codec's own proto3 rules.

**Tech stack:** Rust, prost and pbjson (generated from `model.proto` by
`crates/ridl-ir/build.rs`), `cargo xtask freeze`.

**Spec:** `docs/archive/2026-11-18-proto3-model-projection-design.md`. Read it
before any task. The `.proto` text of every new message is in its section "The
model messages" and is copied from there, not from this plan.

## Global constraints

- Additive only in `model.proto`: `Model.proto3 = 12`, new messages, new enums.
  No `map<>` field, no new value in an existing enum, no renumbering. `SCHEMA`
  stays `ridl.codegen.v1`.
- The bytes the Rust proto3 codec writes and accepts do not change. In pull
  request 2 the generated crates are byte-identical before and after.
- The text of every existing refusal is unchanged.
- Field number 0, `19000..=19999` and any number above `536870911` are refused.
- No change to `ridl-backend-proto`, to `projection/size/proto3.rs` beyond its
  module doc, to the size states, or to `AbsentCause`.
- Comments, rustdoc and docs are plain literal English, name no story id, and
  describe the system as built at that commit.
- Implementers run the focused commands each task names. CI runs the full gate.
- An implementer that cannot commit (a sandboxed Codex run) stops after the
  verification step and reports; the driver commits.

## Process

- Pull request 1 is tasks 1 to 4, on branch `feat/1413-proto3-projection`. Pull
  request 2 is tasks 5 and 6, on branch `refactor/1413-codec-reads-proto3`, cut
  from `main` after pull request 1 merges.
- Record the base commit before each task. After each task, a Claude reviewer
  reads the diff from that base and gives two verdicts (meets the brief; the
  code is sound) before the next task starts. The reviewer checks every doc
  statement the diff adds, changes or makes false, and tries the mutation the
  task names.
- `/review` passes 1 and 2 run once per pull request, after its last task.

## Review focus

Each line names an input the design implies and no happy-path test reaches. The
test for each is in the task named.

1. A struct whose field type is its own name inside an array or an optional: the
   walk and the dependency propagation terminate, and the message refers to
   itself. (Task 1, task 2.)
2. A member typed by a named scalar, an enum or an enum set of another package,
   as a field, an array element and a map key: the wire is the one the local
   equivalent gets, and `Proto3Enum.type` names the other package. (Task 1.)
3. A struct with a retired slot and fields declared out of ordinal order:
   `fields` is in number order and holds no entry for the retired number. (Task
   1.)
4. A declaration that uses a refused declaration declared after it: the user is
   refused with `REFUSED_DEPENDENCY`, and the entry the Rust backend reports is
   the root cause. (Task 2, task 5.)
5. A package with no declaration and no tuple: `Model.proto3` is set, with two
   empty lists. (Task 1.)

---

## Pull request 1

### Task 1: the schema and the walk for representable types

**Files:**

- Modify: `crates/ridl-ir/proto/ridl/codegen/v1/model.proto` (add
  `Proto3Projection proto3 = 12;` to `Model`, and a section "The proto3
  projection" after the podbuf section holding every message and enum of the
  spec)
- Create: `crates/ridl-ir/src/codegen/proto3.rs`
- Modify: `crates/ridl-ir/src/codegen.rs` (declare the module),
  `crates/ridl-ir/src/codegen/lower.rs` (set the field)
- Modify: `freeze/codegen-protocol.txt` (through `cargo xtask freeze update`)
- Test: `crates/ridlc/tests/codegen_proto3.rs` (new)

**Interfaces:**

- Produces: `pub(crate) fn project(model: &v1::Model) -> v1::Proto3Projection`
  in `codegen/proto3.rs`. `lower` sets `model.proto3 = Some(project(&model))` as
  its last statement before it returns.
- Produces, for the tests of tasks 1 to 3, in `codegen_proto3.rs`:
  `fn projection(source: &str) -> (v1::Model, v1::Proto3Projection)` (compiles
  one typl or ridl source with `ridlc::compile`, asserts no error diagnostic,
  lowers, returns the model and its `proto3`), and
  `fn message<'a>(p: &'a v1::Proto3Projection, name: &str) -> &'a v1::Proto3Message`.
- In this task `refused` is always empty: a member the codec refuses is out of
  scope until task 2, and the walk may `todo!()` there only if no test of this
  task and no corpus package reaches it. Prefer returning the message without
  the member's owner listed; task 2 replaces that path.

**The rules the walk implements** (each is the behaviour of `Codec::message_of`,
`declaration_wire` and `scalar_wire` in
`crates/ridl-backend-rust/src/codec_proto3.rs`; read them and keep their
decisions):

- `messages` order: one per struct, union, named scalar, enum and enum set, in
  `Model.declarations` order; then one per entry of `Model.tuples`, in order. A
  constant has no message.
- Sources and names: a struct is `struct_declaration` under its declared name; a
  union is `union_declaration`; a named scalar, an enum and an enum set are
  `root_box` under `<Name>Box` with one field, number 1, source `box_value`; a
  tuple is `tuple` under its wire name.
- Numbers: a struct field's ordinal; a union arm's ordinal; tuple position plus
  1. A reserved or retired slot has no field. `fields` is sorted by number.
- Wires: `pb::scalar`, `pb::int_width_scalar`, `pb::float_width_scalar`,
  `pb::enum_set_scalar` and `pb::primitive` choose the scalar; a `String` result
  is `text`, a `Bytes` result is `bytes`, any other is `Proto3Scalar` with
  `wire_type` from `Scalar::wire_type`. An enum is `Proto3Enum`. A struct, a
  union or a tuple is `Proto3MessageRef`: `message` with its index in `messages`
  when local, `foreign` with its index in `Model.foreign` otherwise. A named
  scalar is the wire of what it wraps.
- `Proto3Array.packed` is true exactly when the element is `scalar` or
  `enumeration`.

- [ ] **Step 1: Write the failing tests** in `codegen_proto3.rs`, each on a
      source written for it, modelled on
      `crates/ridl-backend-rust/tests/fixtures/proto3_conformance.ridl`:

  - `a_struct_field_takes_its_ordinal_and_its_scalar`: one field of each width
    `u8 u16 u32 u64 i8 i16 i32 i64 f32 f64`, a `bool`, an unwidthed integer, a
    string and a bytes. Asserts each number, and per field
    `UINT32, UINT32,
    UINT32, UINT64, SINT32, SINT32, SINT32, SINT64, FLOAT, DOUBLE, BOOL, SINT64`
    with wire types `VARINT` (the integers and the bool), `I32` (`f32`), `I64`
    (`f64`), then `text` and `bytes`.
  - `fields_are_in_number_order_and_a_retired_slot_has_none` (review focus 3).
  - `a_union_lists_its_arms_by_ordinal`: source `arm`, numbers the ordinals.
  - `a_named_scalar_an_enum_and_an_enum_set_get_a_box`: names `<Name>Box`, one
    field numbered 1 with `box_value`; the enum's wire is `enumeration`, the
    enum set's is `scalar` of its width.
  - `a_tuple_numbers_its_positions_from_one`: `tuple_position` 0 and 1, numbers
    1 and 2.
  - `an_array_of_scalars_or_enums_is_packed_and_any_other_is_not`: elements
    `u32`, an enum, an enum set (packed true); a string, a bytes, a struct
    (packed false).
  - `a_map_carries_its_key_and_value_wires`: `map<string, u32>` and
    `map<u32, Struct>`.
  - `a_struct_that_holds_itself_refers_to_its_own_index` (review focus 1).
  - `a_member_typed_in_another_package_has_the_local_wire` (review focus 2): two
    packages compiled as a workspace the way `corpus_packages` in
    `crates/ridlc/tests/codegen_model.rs` does; the struct reference is
    `foreign`, the enum's `TypeRef.package` is the other package.
  - `an_empty_package_has_an_empty_projection` (review focus 5):
    `model.proto3 == Some(Proto3Projection::default())`.

- [ ] **Step 2: Run them and see them fail to compile on
      `v1::Proto3Projection`**

  Run: `cargo test -p ridlc --test codegen_proto3 --locked`

- [ ] **Step 3: Add the schema, then the walk, then the call in `lower`**

- [ ] **Step 4: Run the tests and update the freeze dump**

  Run: `cargo test -p ridlc --test codegen_proto3 --locked` — all pass. Run:
  `cargo test -p ridl-ir --locked` and
  `cargo test -p ridlc-gen-model -p ridlc --locked` — all pass (snapshots of the
  model JSON gain the `proto3` key; accept them only after reading that the one
  change is the added key). Run: `cargo xtask freeze update` then
  `just freeze-check` — passes, and `git diff freeze/` shows added lines only.

- [ ] **Step 5: Commit** —
      `feat(ridl-ir): carry the proto3 layout in the codegen model`

**Mutation the reviewer tries:** number a tuple field by its position without
the plus 1; mark an array of strings packed. Each must fail a test.

### Task 2: the refusals

**Files:**

- Modify: `crates/ridl-ir/src/codegen/proto3.rs`
- Test: `crates/ridlc/tests/codegen_proto3.rs`

**Interfaces:**

- Consumes: `project`, `projection`, `message` from task 1.
- Produces: `Proto3Projection.refused` filled. A refused declaration or tuple
  has no entry in `messages`, and every `Proto3MessageRef.message` index points
  at a message that exists.
- Produces, for the tests:
  `fn refusal<'a>(p: &'a v1::Proto3Projection, model: &v1::Model, name: &str) -> &'a v1::Proto3Refused`.

**The rules:**

- Each refusal of `codec_proto3.rs` becomes one `Proto3Refused` with the same
  text, built by the same format string moved into `codegen/proto3.rs`:
  `refuse_optional_repeated` → `OPTIONAL_COLLECTION`; `refuse_optional` →
  `OPTIONAL_ELEMENT`; the two nested-collection texts → `NESTED_COLLECTION`; the
  map-key text → `MAP_KEY`; the int32 text, live or retired, local or of another
  package → `ENUM_VALUE`; `field_number` → `FIELD_NUMBER`; every other
  `GenerateError` of `message_of`, `declaration_wire` and `scalar_wire` →
  `OTHER`. Do not move `bound_state` or the tuple-without-a-Rust-name refusal:
  the first is a size-state rule and the second is a Rust naming rule.
- `member` is `Owner.field` when the text names a member, absent otherwise.
- One declaration with several refused members gets one entry per member, in the
  order the codec's walk meets them.
- After the direct refusals: any message with a `Proto3MessageRef` to a refused
  local declaration or tuple is removed from `messages` and refused with
  `REFUSED_DEPENDENCY`, repeated until none is left. The text is
  `` `<name>` needs the refused message `<other>`: <message of other's first entry> ``.
  Indices in the remaining `Proto3MessageRef.message` are recomputed after the
  removal.
- `refused` order: `Model.declarations` order, then `Model.tuples` order.

- [ ] **Step 1: Write the failing tests**, one per cause, each on the smallest
      source that draws it, asserting `cause`, `member`, and `message` equal to
      the text `ridl_backend_rust` produces for the same source today (call the
      Rust backend's generation with `encodings = ["proto3"]` as
      `crates/ridl-backend-rust/tests/proto3_roundtrip.rs` does in
      `an_unbounded_type_is_refused_under_proto3`, and compare the strings; do
      not paste the expected text):

  - `an_optional_array_field_is_refused` and `an_optional_map_field_is_refused`
  - `an_optional_array_element_is_refused`
  - `an_array_of_arrays_is_refused`, and the same through a named array type
  - `a_map_with_a_float_key_is_refused`
  - `a_retired_enum_value_outside_int32_is_refused`
  - `a_field_number_in_the_reserved_range_is_refused` (19000), and
    `a_field_number_above_the_limit_is_refused` (536870912)
  - `a_user_of_a_refused_struct_is_refused_for_the_dependency` (review focus 4):
    the user is declared first; asserts the cause, the text form above, that
    neither is in `messages`, and that a third, unrelated struct still is.
  - `a_refused_struct_that_holds_itself_terminates` (review focus 1).
  - `every_message_index_points_inside_the_list`, over the sources above.

- [ ] **Step 2: Run them and see each fail on an empty `refused`**

  Run: `cargo test -p ridlc --test codegen_proto3 --locked`

- [ ] **Step 3: Implement the refusals and the dependency propagation**

- [ ] **Step 4: Run** `cargo test -p ridlc --test codegen_proto3 --locked` and
      `cargo test -p ridl-ir --locked` — all pass.

- [ ] **Step 5: Commit** —
      `feat(ridl-ir): list the proto3 refusals in the codegen model`

**Mutation the reviewer tries:** drop the transitive step of the propagation
(refuse direct users only); accept field number 19999. Each must fail a test.
Add a three-struct chain to the dependency test if the first survives.

### Task 3: agreement with the Rust codec over the corpus

**Files:**

- Test: `crates/ridl-backend-rust/tests/proto3_conformance.rs` (add tests and
  helpers; change no existing test)
- Test: `crates/ridlc/tests/codegen_proto3.rs`

**Interfaces:**

- Consumes: `ridl_backend_rust::proto3_layout`, the `CORPUS` list, the `Fields`
  type and `Proto3Label` of the conformance test; `Model.proto3`.
- Produces:
  `fn projection_fields(model: &v1::Model, message: &v1::Proto3Message) -> Fields`
  in `proto3_conformance.rs`. It spells each field as the layout does: the
  number; the scalar's `.proto` name (`uint32`, `sint64`, `string`, …), or the
  message or enum name, bare for the package and `pkg.Name` otherwise; the label
  `Repeated` for an array or a map, `Optional` when the model's member is `?`,
  `Implicit` otherwise.

- [ ] **Step 1: Write the tests**

  - `the_model_projection_matches_the_codec_layout`: for the fixture and every
    `CORPUS` entry, lower each package and call `proto3_layout`. When the layout
    is `Ok`: every layout message is in `Model.proto3.messages` with equal
    `Fields`; every projection message whose source is not `tuple` is in the
    layout; `refused` holds no entry whose source is a declaration. When the
    layout is `Err` (ros2): the test asserts the error text is a size-state
    refusal and compares nothing else.
  - In `codegen_proto3.rs`,
    `every_corpus_projection_survives_the_json_round_trip` and
    `every_corpus_index_points_inside_its_list` (`declaration`, `tuple`,
    `message`, `foreign`), over `crates/ridlc/tests/corpus`, following
    `every_corpus_package_that_lays_out_carries_its_podbuf_projection_through_json`
    and `every_head_index_of_the_corpus_points_inside_the_list` in
    `codegen_model.rs`.

- [ ] **Step 2: Run**
      `cargo test -p ridl-backend-rust --test proto3_conformance --locked` and
      `cargo test -p ridlc --test codegen_proto3 --locked`. A failure here is a
      defect of task 1 or 2: fix it in `codegen/proto3.rs` and add the smallest
      source that shows it to `codegen_proto3.rs`.

- [ ] **Step 3: Commit** —
      `test(ridl-backend-rust): compare the model's proto3 projection with the codec layout`

**Mutation the reviewer tries:** in `codegen/proto3.rs`, map `i32` to `INT64`;
skip the last arm of a union. The corpus test must fail on each.

### Task 4: the records

**Files:**

- Modify: `docs/design/codegen-plugins.md` (new section "The proto3 projection"
  after "The podbuf projection", with the same parts: order, source, fields and
  wires, refusals, relation to the size states)
- Modify: `docs/book/codegen-plugins.md` (subsection "The proto3 layout" after
  "The podbuf layout"; the list under "What is not available yet")
- Modify: `docs/design/proto3-codec.md` (a table row for `Model.proto3`; in
  "What the two emitters share, and how agreement is kept", state that the model
  carries the projection, that the codec still walks on its own, and that
  `the_model_projection_matches_the_codec_layout` keeps the two equal)
- Modify:
  `docs/decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md`
  (a dated additive amendment under decision 9, in the form of the 2026-10-06
  one)
- Modify: module docs of `crates/ridl-ir/src/projection/proto3.rs` and
  `crates/ridl-ir/src/projection/size/proto3.rs` (where they say which code
  holds the refusal rules)

- [ ] **Step 1: Write the sections.** Every statement is checked against the
      code of tasks 1 to 3. State nothing about pull request 2.
- [ ] **Step 2: Run** `just fmt`, `just check`, `just book-check`,
      `just link-check`, `just doc-path-check`, `just story-id-check`.
- [ ] **Step 3: Commit** —
      `docs(docs): describe the proto3 projection of the codegen model`

### Task 4b: whether a default is legal

Added on 2026-11-19; see the spec, "Whether a default is legal".

**Files:**

- Modify: `crates/ridl-ir/proto/ridl/codegen/v1/model.proto`
  (`bool default_is_legal = 1;` on `Proto3Wire`, with the spec's comment)
- Modify: `crates/ridl-ir/src/codegen/proto3.rs`
- Modify: `freeze/codegen-protocol.txt` (through `cargo xtask freeze update`)
- Modify: `docs/design/codegen-plugins.md`, `docs/book/codegen-plugins.md` (the
  proto3 sections), `docs/design/proto3-codec.md` (what the comparison test does
  not cover)
- Test: `crates/ridlc/tests/codegen_proto3.rs`

**Interfaces:**

- Produces: `Proto3Wire.default_is_legal`, set on every wire the walk builds: a
  field's wire, an array's element, a map's key and value.
- The value per kind equals what `crates/ridl-backend-rust/src/codec_proto3.rs`
  computes today (`Wire::default_is_legal`, `zero_legal`,
  `crate::zero_is_legal`): a numeric scalar or a boolean,
  `ridl_ir::zero::range_holds_zero` over the constraint, true with no
  constraint; text and bytes, length lower bound 0; an enum, a zero member or
  open; an enum set, true; an array or a map, count lower bound 0; a message,
  true.

- [ ] **Step 1: Write the failing tests**, one per kind, each with a legal and
      an illegal case where the kind has one:
      `a_ranged_scalar_default_is_legal_only_when_the_range_holds_zero` (ranges
      `[0..10]`, `[1..10]`, and a step grid that excludes 0),
      `a_text_or_bytes_default_is_legal_only_without_a_length_floor`,
      `an_enum_default_is_legal_with_a_zero_member_or_when_open`,
      `an_enum_set_and_a_message_default_are_always_legal`,
      `a_collection_default_is_legal_only_without_a_count_floor`,
      `a_map_key_and_value_carry_their_own_default_rule`.
- [ ] **Step 2: Run** `cargo test -p ridlc --test codegen_proto3 --locked` and
      see them fail.
- [ ] **Step 3: Add the field and fill it.**
- [ ] **Step 4: Run** the same command,
      `cargo test -p ridl-ir -p ridlc -p ridlc-gen-model --locked` (model
      snapshots gain the key; read the change before accepting it),
      `cargo xtask freeze update`, `just freeze-check` (one added line), and the
      documentation checks of task 4 step 2.
- [ ] **Step 5: Commit** —
      `feat(ridl-ir): carry whether a proto3 default is legal in the codegen model`

**Mutation the reviewer tries:** return true for every scalar; read the value's
rule for a map key. Each must fail a test.

Then: `just verify`, push, open pull request 1 (`Refs #1413`, not `Closes`),
`/review` passes 1 and 2, and garden only the plan state; the design note and
this plan stay in `docs/wip/` until pull request 2.

---

## Pull request 2

### Task 5: the codec reads the projection

**Files:**

- Modify: `crates/ridl-backend-rust/src/codec_proto3.rs`
- Test: existing suites; `crates/ridl-backend-rust/tests/proto3_conformance.rs`

**Interfaces:**

- Consumes: `ctx.model.proto3` (`messages`, `refused`), and for a declaration of
  another package the `proto3` of that package's model, reached the way the
  codec reaches other packages today.
- Produces: no API change. `proto3_layout` keeps its signature and its output.

**The change:**

- `message_of` takes each field's number, scalar, wire type, packing and message
  reference from the `Proto3Message` of the declaration or tuple, found by its
  `source` index. It joins each `Proto3Field` to the model member by
  `Proto3Field.source` to build what stays in the codec: `Repr`, `NamedScalar`,
  the constraint, the count bounds. `zero_legal` and `Wire::default_is_legal`
  are removed: the codec reads `Proto3Wire.default_is_legal`, and no longer
  calls `zero_is_legal` from this file.
- The codec stops on the first entry of `refused` whose cause is not
  `REFUSED_DEPENDENCY` and whose source is a declaration, or a tuple in
  `reachable_tuples`, and returns its `message` as the `GenerateError`.
- Removed from the backend: `refuse_optional_repeated`, `refuse_optional`,
  `field_number`, the nested-collection checks, the map-key check, the int32
  check, and every call to `pb::primitive`, `pb::int_width_scalar`,
  `pb::float_width_scalar`, `pb::scalar` and `pb::enum_set_scalar`.
- Kept: `bound_state`, `reachable_tuples` and the emission order, the
  tuple-without-a-Rust-name refusal.

- [ ] **Step 1: Record the reference output.** Before any edit, generate the
      crates of `examples/cabin`, `crates/ridlc/tests/corpus/veh-cluster` and
      `crates/ridlc/tests/corpus/rust-shapes` with `ridl build` into a scratch
      directory outside the repository.
- [ ] **Step 2: Write the failing test**
      `the_reported_refusal_is_the_root_cause_and_not_the_dependency` in
      `proto3_conformance.rs` (review focus 4): the source of
      `a_user_of_a_refused_struct_is_refused_for_the_dependency`; asserts the
      `GenerateError` text is the refused struct's own text. It passes today
      only by declaration order, so write the source with the user first and
      confirm what the codec reports before the port; if it already passes, keep
      it as the pin.
- [ ] **Step 3: Port `message_of` and the refusal path; delete the listed
      functions.**
- [ ] **Step 4: Verify.** Run `cargo test -p ridl-backend-rust --locked` and
      `cargo test -p ridlc -p ridl-cli --locked` — all pass with no snapshot
      change. Generate the three crates again and `diff -r` against step 1 — no
      difference. Run `just freeze-check` — passes with no dump change.
- [ ] **Step 5: Commit** —
      `refactor(ridl-backend-rust): read the proto3 layout from the codegen model`

**Also in this task**, carried from the review of pull request 1 (its four Minor
findings):

- A test pins the refusal text and the member of a tuple field with no type
  (`crates/ridl-ir/src/codegen/proto3.rs`, the tuple branch of the missing-type
  refusal).
- `foreign_tuples_are_listed_only_in_the_declaring_package` also lifts a tuple
  that proto3 refuses, and asserts a refusal in the declaring package and none
  in the importing one.
- A unit test with two declarations of another package pins the index of
  `Proto3MessageRef.foreign`.
- `docs/design/proto3-codec.md` names the list it calls "the encoding corpus".

**Mutations the reviewer tries**, beyond the one below: flip `packed` for one
array, and flip `default_is_legal` for one scalar, each in `ridl-ir`'s
`codegen/proto3.rs`. Each must now fail a test of the Rust codec, because the
codec reads both.

**Mutation the reviewer tries:** in `ridl-ir`'s `codegen/proto3.rs`, change one
scalar mapping. The byte-equality tests of the Rust codec against prost
(`bytes_this_codec_writes_equal_prost_bytes`) must now fail, which shows the
codec reads the projection.

### Task 6: the guard and the records

**Files:**

- Modify: `crates/ridl-backend-rust/tests/proto3_conformance.rs` (remove
  `the_model_projection_matches_the_codec_layout` and `projection_fields`; add
  the guard)
- Modify: `docs/design/proto3-codec.md`, `docs/design/codegen-plugins.md` (the
  sentences task 4 wrote about the codec walking on its own), the module doc of
  `codec_proto3.rs` (the paragraph "The wire mapping" and the paragraph on the
  codec's own walk)

- [ ] **Step 1: Write the guard test**
      `the_codec_selects_no_proto3_scalar_itself`: reads `src/codec_proto3.rs`
      and fails if it contains `pb::primitive(`, `pb::scalar(`,
      `pb::int_width_scalar(`, `pb::float_width_scalar(`,
      `pb::enum_set_scalar(`, `PROTO_RESERVED` or `PROTO_MAX_FIELD_NUMBER`.
- [ ] **Step 2: Update the records** to state that the codec reads
      `Model.proto3`, and which facts it still derives itself (the list under
      "Kept" in task 5).
- [ ] **Step 3: Run**
      `cargo test -p ridl-backend-rust --test proto3_conformance --locked` and
      the documentation checks of task 4 step 2.
- [ ] **Step 4: Commit** —
      `test(ridl-backend-rust): guard the codec against selecting a proto3 scalar`

Then: `just verify`, push, open pull request 2 (`Closes #1413` is not used: the
issue closes with the release), `/review` passes 1 and 2, run `sdd-gardening` so
that `docs/wip/` holds neither the design note nor this plan.

## After both pull requests

These are the owner's steps or need the owner's word:

- A release that carries both pull requests, by the release procedure.
- A comment on driftsys/ridlc-gen-kotlin#133 naming the version to pin.
- Closing driftsys/ridl#1413, and a comment on driftsys/ridl#690 stating what
  remains: the `.proto` emitter, which can read `Model.proto3`, and the sizer,
  which needs a correction step after lowering (the design note, "Where the
  rules live").
