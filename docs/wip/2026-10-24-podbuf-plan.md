# podbuf Implementation Plan

> **Status, 2026-10-27.** Tasks 1 to 12 and 15 are built, and Task 16 is built
> except its Step 5 (three freeze rows: `wire`, `codegen-protocol` and
> `runtime-crates`; the `cli` row, whose dump only gained lines, is restored in
> this pull request), which is a follow-up pull request. **Tasks 13 and 14 are
> open:** both wait for the typl `open` keyword, which is not built, so this
> plan stays in `docs/wip/`. The design it implements is archived at
> `docs/archive/2026-10-24-podbuf-design.md`, and its decisions are
> [ADR-0031](../decisions/ADR-0031-podbuf-projection-rules.md); the as-built
> codec is [`../design/podbuf-codec.md`](../design/podbuf-codec.md). The
> `docs/wip/2026-10-24-podbuf-design.md` path that the text below cites is now
> that archived file.

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build podbuf, the third payload encoding of ADR-0020 decision 1, as
the design specifies it: one deterministic layout computed by the first-fit law,
a generated `verify` whose cost grows with the data present, read-in-place
accessors, a C header a consumer reads with no library, the `podbuf` feature of
`ridl-rt`, the podbuf rules of `ridl diff`, and the computed layout exposed to
codegen plugins.

**Architecture:** The layout law lives once, in `ridl-ir`
(`projection::podbuf`), and four consumers read it: the Rust codec in
`ridl-backend-rust`, the C header in a new wire backend crate
`ridl-backend-podbuf`, the codegen `Model` (a `PodbufProjection` message), and
`ridl-diff` with the padding lint. `ridl-rt` gains a `podbuf` module behind the
`podbuf` feature, which replaces `repr-c`, with the shared preamble, descriptor,
little-endian load and writer helpers a generated `Payload<Podbuf>`
implementation calls.

**Tech Stack:** Rust 2024 for the toolchain crates, Rust 2021 for `ridl-rt` and
the generated crate; `quote` + `prettyplease` for the Rust emitter; text
emission for the C header; `prost`/`protox` for the `ridl.codegen.v1` schema;
`insta` snapshots and `proptest` for tests; plain `rustc` and the host C
compiler as proof oracles.

**Spec:** `docs/wip/2026-10-24-podbuf-design.md` (the design, approved by the
owner; the sections cited below are its sections).

## Global Constraints

- **Minimum Rust version and editions.** `ridl-rt` keeps `edition = "2021"` and
  `rust-version = "1.88"`; the generated crate declares edition 2021 and
  `rust-version = "1.88"` and compiles with Rust 1.88 with every feature on and
  with default features off (ADR-0029 decision 1, last row). Nothing emitted may
  need a later Rust: `core::mem::offset_of!` (stable since 1.77) is allowed.
- **`no_std`, no allocation in `verify`, no `unsafe`.** The `podbuf` module of
  `ridl-rt` and the generated `verify`, head struct and read-in-place accessors
  are `no_std` and allocate nothing; `decode` allocates for `String`, `Vec` and
  maps as the other codecs do. No generated line and no line of `ridl-rt` holds
  `unsafe` (§14, "Generated Rust"): every read is a bounds-checked little-endian
  load from a `&[u8]`. `ridl-rt` keeps no dependency in any feature combination
  (ADR-0020 decision 5, ADR-0021 decision 8).
- **Little-endian only.** `ridl-rt`'s `podbuf` module carries
  `#[cfg(not(target_endian = "little"))] compile_error!(...)`, and the C header
  carries `#error` under `__BYTE_ORDER__ != __ORDER_LITTLE_ENDIAN__` (§4).
- **Preamble and alignment numbers are fixed.** Preamble 8 bytes
  `{ head_size: u32, reserved: u32 }`; head alignment 8; descriptor 8 bytes at
  alignment 4; `elem_size` and `len` of an array or map descriptor are `u16`;
  limits 65,535 elements and 65,535 bytes per element; `off` is `u32` relative
  to the buffer start (§5.1, §5.4, §6.1).
- **Justfile rule.** The justfile is the single definition of every gate
  command. A new check is a recipe, a member of `build`, a name in
  `gate-parity`'s `expected` list, and one `run: just <recipe>` line in
  `.github/workflows/ci.yml`; a tool install may be added to the workflow, a
  gate command may not (AGENTS.md, ADR-0009).
- **Gates are CI's job.** An implementer runs the focused tests of the crate it
  changed (`cargo test -p <crate>`), and the one `rustc` or `cc` proof its task
  names. `just build`, `just freeze-check`, `just demo` and `just compat-check`
  run in CI on the pull request.
- **Prose.** Comments, commit messages, rustdoc, the C header's comments and
  every document are plain, literal English. No story id, stage or lane name in
  a shipped file (`just story-id-check`); a gap links its issue.
- **Freeze rules (ADR-0029, `freeze/README.md`).** A line added to a dump
  passes; a line removed or changed is a break, approved by deleting the
  surface's row in `freeze/surfaces.toml` in the breaking pull request (the name
  stays in `required`) and adding the row back in a follow-up with the merge
  commit as `baseline`. Task 1 is the one task that removes lines; the rows it
  deletes are added back in Task 16. Every other task only adds lines. A
  regenerated dump is committed with the change that changes it
  (`cargo xtask freeze update`). A `.ridl` or `.toml` under `examples/` or
  `evals/corpus/` changes no dump; the frozen copies under
  `xtask/tests/freeze-fixtures/` are refreshed on purpose only.
- **Codes are allocated once, here.** The codes of this plan are RIDL-151,
  RIDL-152 and RIDL-153 (errors) and RIDL-154 (`podbuf-padding-order`, a lint at
  `info`). The design leaves them to the builder (§16); Task 16 records them in
  the projection record. A new lint starts at `info` or `allow` (ADR-0029
  decision 2) and adds its row to `docs/book/lints.md` in the same change
  (ADR-0024 decision 7).
- **Names of generated items (builder's call, §16).** Rust: `<T>PodbufHead` (the
  `#[repr(C, align(8))]` struct), `<T>PodbufView<'a>` (the view `verify`
  returns, with one accessor per field), the internal helpers `__ridl_podbuf_*`.
  C: `ridl_podbuf_preamble`, `<pkg>_<type>_head` and accessor functions
  `<pkg>_<type>_<field>`, spelled with the pinned transform of `ridl_ir::name`
  (ADR-0016). A claim table entry per generated name
  (`ridl-backend-rust/src/claims.rs`), so a ridl name cannot collide with one.

## Review Focus

Five inputs the design implies that no ordinary round-trip test exercises, with
the task that owns each one's test:

1. **An appended field backfilled into an older layout's pad bytes, inside the
   older `head_size`.** `FaultEvent` plus an appended `priority: u8` places at
   offset 2 (§5.3, §8.1): a v2 reader over a v1 buffer reads 0 there and
   `verify` accepts it; a v1 reader over a v2 buffer ignores the byte because no
   pad is checked (§7). The pair of fixtures and both directions: Task 10.
2. **Descriptor arithmetic at the edges.** `off` inside the preamble or the head
   (`off < 8 + head_size`), `off + len` or `off + len·elem_size` past `u32::MAX`
   or past the buffer, a `len = 0` descriptor whose other bytes are not 0, and
   two descriptors over the same bytes (accepted by design). Checked arithmetic,
   no panic, each refusal named: Task 8.
3. **A string bounded by scalars, checked by bytes first.** `len ≤ 4·max` bounds
   the work; the scalar-value count is checked after UTF-8 (rules 3, 4, 6): a
   3-byte ASCII string under `max = 2` is refused, a string of `max` four-byte
   scalars at exactly `4·max` bytes is accepted, an interior NUL and a missing
   terminator are refused, and the encoder refuses a string holding U+0000:
   Tasks 7 and 8.
4. **A stride that differs from the reader's element head in either direction.**
   `elem_size` larger than the reader's head (newer producer): the reader steps
   by `elem_size` and reads its prefix; smaller (older producer): the bytes past
   `elem_size` read as 0 and the optional-flag checks still apply; not a
   multiple of 8 for a struct element, or not the exact size for a scalar
   element: refused (§5.1, rule 5). Tasks 8 and 10.
5. **i686 alignment of a `u64` after a descriptor.** `FaultEvent.timestamp` sits
   at 16 only because the head is declared with alignment 8; without it i686
   places it at 12 (§4). The `_Static_assert`s of the C header under
   `clang --target=i686-linux-gnu` and the `const` assertions of the generated
   crate under `cargo check --target i686-unknown-linux-gnu`: Tasks 5 and 15.

## Prerequisites

**The contextual keyword `open` on `enum` and `enumset` (design §8.5).** A
separate typl change, not written yet: the grammar and the typed AST, the IR v2
fields (this plan assumes `EnumDef.open: bool` and `EnumSetDef.open: bool`), the
codegen `Model` fields (`v1::Enum.open`, `v1::EnumSet.open`), the keyword in
`freeze/keywords.txt` (`open` is contextual, so the `reserved-set` line does not
change), the unknown variant with `#[non_exhaustive]` on the generated Rust
domain enum, and how today's backends and `ridl diff` classify a new value of a
strict enum. Every enum is strict until it lands, which is today's behaviour.

Tasks that depend on it: **Task 13** (open enums and enum sets in podbuf) and
**Task 14** (the `ridl diff` rule for an open enum whose new value changes its
width or signedness). Those two tasks start only after the prerequisite has
merged to `main`; every other task does not read the `open` flag and proceeds
before it.

## Questions for Sebastien

Each question names the default this plan takes until it is answered; the task
that depends on the answer is marked.

**Answered by Sebastien on 2026-10-24:** every default below is accepted.
Question 1: the `--encoding podbuf` flag is approved. Question 2: one break
window for the four frozen surfaces and the catalog descriptor value is
approved, shipped in a minor release of `ridl-rt`. podbuf stays in this
repository.

1. **How does `ridl diff` know a package carries podbuf?** `ridl diff` reads IR
   snapshots only, and the IR carries no `[backend.<name>] encodings`. An
   inline-nested append and a tombstoned removal are compatible for proto3 and
   FlatBuffers and breaking for podbuf (§8.2), so a verdict that does not know
   the encoding is wrong for one side or the other. Default (Task 11): the walk
   reports them as the categories `PodbufInlineAppend` and `PodbufFieldRetired`;
   their verdict is `Breaking` when `ridl diff` is given the new repeatable flag
   `--encoding podbuf` (and the MCP `ridl_diff` tool the optional input
   `encodings`), and `Compatible` under a new text-report heading "breaking
   under podbuf" otherwise. The flag and the input are additive.
2. **The rename breaks four frozen surfaces.** `runtime-crates` (the feature
   `repr-c` and the marker type `ridl_rt::encoding::ReprC`), `wire` (the line
   `encoding repr-c`), `codegen-protocol` (`ENCODING_REPR_C` becomes
   `ENCODING_PODBUF`, same number 3) and `cli` (the help text that names
   `repr-c`). The catalog descriptor's `Encoding` enum value `ReprC` (value 2 in
   `catalog.fbs`) and the `ridl describe` JSON text that prints it are renamed
   too. Default (Task 1): one approved break window for all four rows, the proto
   enum value renamed rather than kept. Confirm the list, and that the release
   that ships it is a minor release of `ridl-rt`.
3. **Where do the podbuf conformance cases live?** `ridl-rt-conformance` holds
   the port contract tests, generic over a runtime factory, and no encoding case
   for proto3 or FlatBuffers either. Default (Task 10): the podbuf conformance
   cases go beside the proto3 and FlatBuffers ones in
   `crates/ridl-backend-rust/tests/`, plus one `Loopback::<Podbuf>` case in
   `crates/ridl-loopback/tests/encoding.rs`.
4. **Alignment gaps in the tail.** Tail data is placed at the alignment of its
   element type (§5.1), so a 24-byte element block after a 5-byte string leaves
   3 bytes no descriptor covers. The encoder duty says "writes no unreferenced
   bytes into the tail" (§7). Default (Task 7): alignment gaps are written as
   zero and are the one unreferenced kind of tail byte the encoder may leave.
5. **Who owns the buffer alignment on the Rust side?** `verify` checks that the
   buffer starts on an 8-byte boundary (§3, rule 1), but `Payload::Buffer` and
   the `Sizes<E>` buffers are `[u8; N]` today, with alignment 1, and a `Vec<u8>`
   gives no alignment guarantee. Default (Task 9): `Payload<Podbuf>::Buffer`,
   `ClaimBuffer` and `EventBuffer` are `ridl_rt::podbuf::Aligned<[u8; N]>`, a
   `#[repr(C, align(8))]` wrapper that satisfies the traits' bounds; `verify`
   checks the pointer as designed; the loopback and transcode tests pass aligned
   buffers. A consumer that hands `verify` an unaligned slice gets
   `Malformed::Unaligned`.
6. **The unknown variant of an open enum is the prerequisite's.** The design
   leaves its shape to the builder (§8.5, §16) and it is a domain-type item that
   every encoding reads. Default: the prerequisite emits it; Task 13 maps a raw
   value onto it.
7. **The padding lint's "released" test and level.** Default (Task 12): a struct
   is released when a snapshot under the baseline location
   (`.ridl/baseline/<pkg>.ir.json`, or `--baseline`) declares it; with no
   baseline every struct is unreleased. The lint runs in `ridl check` only
   (`ridlc` does not read a baseline), only when a `[backend.<name>] encodings`
   list names `podbuf`, at level `info`.
8. **Union tag width.** §6 says "a narrow integer tag (the arm's ordinal)" and
   fixes no width. Default (Task 2): the narrowest unsigned width that holds the
   largest arm ordinal, so `u8` for up to 255 arms.
9. **A union arm whose type is a named scalar, an enum or an enum set.** typl
   lets an arm name any named type (TYPL-204), and §6 states the arm `len` as
   "the head size of the active arm". Default (Task 2): such an arm is a
   one-field head, the value at offset 0, size rounded up to 8.
10. **`ClimateReport` in §5.3.** The worked example measures the temperatures as
    16-bit raw integers, and `veh.common.Temperature` is a float type, so the
    corpus type lays out differently. Task 2 tests the law with integer fixtures
    that reproduce the table's numbers, not with the corpus type.
11. **Which codes.** RIDL-151 (type above `u32::MAX` bytes), RIDL-152 (array or
    map above the descriptor limits), RIDL-153 (deprecated field with no valid
    value) and RIDL-154 (`podbuf-padding-order`) are the builder's picks in the
    free 15x band beside RIDL-150. Confirm the band.

---

## Task 1: Rename the encoding to `podbuf` and its feature with it

The name and the feature rename of design §2, with the ADR-0020 amendment that
records them. Every site that spells `repr-c`, `repr(C)` or `ReprC` for the
encoding changes; `#[repr(C)]` stays where it describes a Rust attribute.

**Files:**

- Modify: `crates/ridl-rt/Cargo.toml` (feature `podbuf` replaces `repr-c`),
  `crates/ridl-rt/src/encoding.rs`, `crates/ridl-rt/src/lib.rs`,
  `crates/ridl-rt/src/contract.rs`, `crates/ridl-rt/src/payload.rs` (doc
  examples), `crates/ridl-rt/examples/read_sample.rs`,
  `crates/ridl-rt/tests/payload.rs`
- Modify: `crates/ridl-core/src/manifest.rs` (`ENCODING_NAMES`),
  `crates/ridl-ir/proto/ridl/codegen/v1/deployment.proto`
  (`ENCODING_PODBUF = 3`), `crates/ridl-ir/src/projection/size.rs`
  (`Encoding::Podbuf`), `crates/ridl-ir/src/codegen/tests.rs`,
  `crates/ridl-descriptor/schema/catalog.fbs` (`Podbuf = 2`),
  `crates/ridl-descriptor/src/{lower.rs,describe.rs,generated.rs}` (regenerate
  with `cargo xtask descriptor-codegen`),
  `crates/ridl-descriptor/tests/lower.rs`
- Modify: `crates/ridl-transport-ws/src/frame.rs` (`"podbuf" => Some(3)`),
  `crates/ridl-backend-rust/src/{contract.rs,lib.rs,codec.rs}` (messages and
  doc), `crates/ridlc/src/link_encodings.rs`,
  `crates/ridlc/src/design_lints/ceiling.rs`,
  `crates/ridlc/tests/{cabin_example.rs,support/mod.rs}`,
  `crates/ridl-loopback/tests/encoding.rs`
- Modify: `freeze/surfaces.toml` (delete the rows `runtime-crates`, `wire`,
  `codegen-protocol`, `cli`; keep the names in `required`), `freeze/wire.txt`,
  `freeze/codegen-protocol.txt`, `freeze/cli.txt` (regenerated)
- Modify:
  `docs/decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md`
  (an amendment paragraph in `## Status`: decision 1's encoding is named
  `podbuf`, decision 5's feature is `podbuf`, decision 9's option name is
  `podbuf`; decisions 1, 5 and 9 changed in place),
  `docs/specification/frame-specification.md` (encoding tag row 3),
  `docs/specification/ridl-family-overview.md` (ledger row 35),
  `docs/specification/typl-language-reference.md` (§4.4 and §17.13 pointers),
  `docs/design/ridl-rt.md`, `docs/design/catalog-descriptor.md`,
  `docs/design/interaction-face.md`, `docs/design/codegen-plugins.md`,
  `docs/book/{generated-code.md,cli-reference.md,introduction.md,codegen-plugins.md,catalog-descriptor.md}`,
  `docs/ROADMAP.md`, `docs/technotes/walking-skeleton-architecture.md`
- Test: `crates/ridl-rt/src/encoding.rs` (unit tests),
  `crates/ridl-ir/src/codegen/tests.rs`, `crates/ridl-transport-ws/src/frame.rs`
  (unit tests), `xtask/src/freeze/wire.rs` (unit tests)

**Interfaces:**

- Produces: `pub struct ridl_rt::encoding::Podbuf;` with
  `impl Encoding for Podbuf { const NAME: &'static str = "podbuf"; }`; the
  feature `podbuf = []` in `ridl-rt`;
  `ENCODING_NAMES: [&str; 3] =
  ["flatbuffers", "proto3", "podbuf"]`;
  `v1::Encoding::Podbuf` (JSON name `ENCODING_PODBUF`, value 3);
  `ridl_ir::projection::size::Encoding::Podbuf`; the catalog descriptor enum
  value `Podbuf = 2`.
- Consumes: nothing new.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/ridl-rt/src/encoding.rs, in `mod tests`
#[test]
fn each_name_is_the_cargo_feature_name() {
    assert_eq!(FlatBuffers::NAME, "flatbuffers");
    assert_eq!(Proto3::NAME, "proto3");
    assert_eq!(Podbuf::NAME, "podbuf");
}

// crates/ridl-transport-ws/src/frame.rs, in its tests
#[test]
fn the_encoding_tag_of_podbuf_is_3() {
    assert_eq!(encoding_tag("podbuf"), Some(3));
    assert_eq!(encoding_tag("repr-c"), None);
}

// crates/ridl-ir/src/codegen/tests.rs
#[test]
fn the_podbuf_encoding_keeps_value_3() {
    assert_eq!(v1::Encoding::Podbuf as i32, 3);
    assert_eq!(serde_json::to_value(v1::Encoding::Podbuf).unwrap(), "ENCODING_PODBUF");
}
```

- [ ] **Step 2: Run them**

`cargo test -p ridl-rt -p ridl-transport-ws -p ridl-ir` — expected: compile
errors on `Podbuf` (not declared) and the frame test fails on `Some(3)`.

- [ ] **Step 3: Implement the rename**

Rename `ReprC` to `Podbuf` and `repr-c` to `podbuf` at every file listed above;
the `Sizes<E>` and `payload` rustdoc examples name `Podbuf`. Keep the
`ENCODING_REPR_C`-shaped comment in `deployment.proto` as "`podbuf`; no link
carries it until the Rust backend emits the codec". Write the ADR-0020 amendment
paragraph and change decisions 1, 5 and 9 in place. Regenerate the descriptor
accessors (`cargo xtask descriptor-codegen`) and the dumps
(`cargo xtask freeze update`); delete the four rows of `freeze/surfaces.toml`
and state the approval in the pull request description (`freeze/README.md`,
"Approve a break").

- [ ] **Step 4: Run the focused tests**

`cargo test -p ridl-rt -p ridl-transport-ws -p ridl-ir -p ridl-core -p ridl-descriptor -p ridl-loopback`
— expected: pass.
`grep -rn "repr-c\|ReprC" crates docs --include=*.rs --include=*.md --include=*.toml --include=*.proto --include=*.fbs | grep -v docs/archive | grep -v docs/wip`
— expected: no output.

- [ ] **Step 5: Commit**

`feat(ridl-rt)!: rename the repr-c encoding and feature to podbuf` — body: the
feature `repr-c` becomes `podbuf` and `ridl_rt::encoding::ReprC` becomes
`Podbuf` (ADR-0020 decision 5 as amended); the frame tag 3 keeps its value; the
four frozen-surface rows are deleted per `freeze/README.md`.

---

## Task 2: The layout law in `ridl-ir`

The first-fit law, the flags placement, the descriptors with their stride, the
enum width rule and the union and map heads (§5.1, §5.2, §6, §6.1), computed
once over the IR v2 and consumed by every later task.

**Files:**

- Create: `crates/ridl-ir/src/projection/podbuf.rs`
- Modify: `crates/ridl-ir/src/projection.rs` (`pub mod podbuf;`)
- Test: `crates/ridl-ir/src/projection/podbuf.rs` (`mod tests`, fixtures built
  with `ridlc::compile` are not available in `ridl-ir`, so the tests build
  `v2::StructDef` values directly, as `projection::flatbuffers`'s tests do)

**Interfaces:**

- Produces (all `pub` in `ridl_ir::projection::podbuf`):
  - `pub const PREAMBLE_SIZE: u32 = 8; pub const HEAD_ALIGN: u32 = 8; pub const DESCRIPTOR_SIZE: u32 = 8; pub const DESCRIPTOR_ALIGN: u32 = 4; pub const MAX_COUNT: u64 = 65_535; pub const MAX_ELEM_SIZE: u64 = 65_535;`
  - `pub struct Head { pub size: u32, pub fields: Vec<Placement>, pub pads: Vec<Pad> }`
    — `fields` sorted by offset; `pads` are the `_padN` runs, `N` the offset.
  - `pub struct Placement { pub source: Source, pub offset: u32, pub size: u32, pub align: u32, pub form: Form, pub optional: Option<FlagBit> }`
  - `pub enum Source { Field { name: String, ordinal: u32 }, Flags { index: u32 }, Tag, Arm, Position(u32), Key, Value }`
  - `pub struct FlagBit { pub flags: u32, pub bit: u8 }`
  - `pub enum Form { Bool, Int { width: v2::IntWidth, signed: bool }, Float(v2::FloatWidth), Enum { width: v2::IntWidth, signed: bool, reference: String }, EnumSet { width: v2::IntWidth, reference: String }, Inline(Box<Head>), InlineArray { element: Box<Form>, count: u32 }, Descriptor(DescriptorKind) }`
  - `pub enum DescriptorKind { Text { min: u64, max: u64 }, Bytes { min: u64, max: u64 }, Array { element: Box<Form>, elem_size: u32, min: u64, max: u64 }, Map { entry: Box<Head>, elem_size: u32, min: u64, max: u64 }, Arm { arms: Vec<(u32, Head)> } }`
    — `Arm.arms` is one head per declared arm, by ordinal.
  - `pub struct Pad { pub offset: u32, pub size: u32 }`
  - `pub fn place(items: &[(u32, u32)]) -> Vec<u32>` — the first-fit primitive
    over `(size, align)` pairs in declaration order; returns one offset each.
  - `pub fn struct_head(packages: Packages<'_>, owner: &str, def: &v2::StructDef) -> Result<Head, ProjectionError>`
  - `pub fn tuple_head(packages: Packages<'_>, name: &str, tuple: &v2::TupleType) -> Result<Head, ProjectionError>`
  - `pub fn union_head(packages: Packages<'_>, owner: &str, def: &v2::UnionDef) -> Result<Head, ProjectionError>`
    — the tag at offset 0 (default of question 8), the arm descriptor placed by
    the law, size rounded to 8.
  - `pub fn enum_width(def: &v2::EnumDef) -> (v2::IntWidth, bool)` — the
    narrowest width that holds every value; signed when any value is negative.
  - `pub fn field_form(packages: Packages<'_>, ty: &v2::FieldType) -> Result<Form, ProjectionError>`
  - `pub fn elem_size(form: &Form) -> u32` — a multiple of 8 for `Inline`, the
    exact size otherwise (§5.1).
  - `pub fn max_size(packages: Packages<'_>, decl: &v2::Decl) -> Option<u64>` —
    preamble plus head plus the largest tail; `None` when the sum exceeds
    `u32::MAX` or a count or element size exceeds its `u16` limit.
- Consumes: `ridl_ir::projection::flatbuffers::Packages` (reused as the
  resolver), `ridl_ir::v2`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn place_follows_the_first_fit_law() {
    // FaultEvent: code u16, message descriptor (8, align 4), timestamp i64.
    assert_eq!(place(&[(2, 2), (8, 4), (8, 8)]), vec![0, 4, 16]);
    // DiagFilter: severity u8, flags u8, category descriptor.
    assert_eq!(place(&[(1, 1), (1, 1), (8, 4)]), vec![0, 1, 4]);
    // A later small field fills the gap an earlier alignment left.
    assert_eq!(place(&[(1, 1), (8, 8), (2, 2)]), vec![0, 8, 2]);
}

#[test]
fn the_worked_examples_of_the_design_lay_out_as_measured() {
    assert_eq!(head(door_payload()).size, 8);
    assert_eq!(head(climate_report()).size, 8);
    let filter = head(diag_filter());
    assert_eq!(filter.size, 16);
    assert_eq!(offsets(&filter), vec![0, 1, 4]);             // severity, flags 0, category
    assert_eq!(pads(&filter), vec![(2, 2), (12, 4)]);
    let event = head(fault_event());
    assert_eq!(event.size, 24);
    assert_eq!(offsets(&event), vec![0, 4, 16]);
    assert_eq!(pads(&event), vec![(2, 2), (12, 4)]);
    let page = head(fault_page());
    assert_eq!(page.size, 8);
    assert!(matches!(page.fields[0].form,
        Form::Descriptor(DescriptorKind::Array { elem_size: 24, min: 0, max: 64, .. })));
}

#[test]
fn flags_are_placed_before_optional_field_8k_plus_1() {
    // Nine optional u8 fields: flags 0 at 0, fields 1..8 at 1..8, flags 1 at 9, field 9 at 10.
    let h = head(nine_optionals());
    assert_eq!(h.size, 16);
    assert_eq!(flag_offsets(&h), vec![0, 9]);
    assert_eq!(h.fields.iter().find(|p| is_field(p, "f9")).unwrap().optional,
        Some(FlagBit { flags: 1, bit: 0 }));
}

#[test]
fn an_enum_takes_the_narrowest_width_and_the_signedness_of_its_values() {
    assert_eq!(enum_width(&values(&[0, 1, 2, 3])), (v2::IntWidth::U8, false));
    assert_eq!(enum_width(&values(&[-1, 200])), (v2::IntWidth::I16, true));
    assert_eq!(enum_width(&values(&[70_000])), (v2::IntWidth::U32, false));
}

#[test]
fn elem_size_is_a_multiple_of_8_for_a_struct_and_exact_otherwise() {
    assert_eq!(elem_size(&Form::Inline(Box::new(head(fault_event())))), 24);
    assert_eq!(elem_size(&Form::Int { width: v2::IntWidth::U16, signed: false }), 2);
    assert_eq!(elem_size(&Form::Descriptor(text(0, 16))), 8);
}

#[test]
fn a_union_head_is_a_tag_and_an_arm_descriptor() {
    let h = head_of_union(sensor_result());          // ok: SensorReading, err: SensorFault
    assert_eq!(h.size, 16);
    assert_eq!(offsets(&h), vec![0, 4]);             // tag u8 at 0, descriptor at 4
}

#[test]
fn max_size_sums_preamble_head_and_largest_tail() {
    // FaultPage: 8 + 8 + 64 * 24 + 64 * (4 * 256 + 1).
    assert_eq!(max_size_of(fault_page()), Some(8 + 8 + 64 * 24 + 64 * 1025));
    assert_eq!(max_size_of(array_of(65_536, u8())), None);
    assert_eq!(max_size_of(array_of(1, struct_of_size(65_536))), None);
}
```

- [ ] **Step 2: Run them**

`cargo test -p ridl-ir podbuf` — expected: compile error, `projection::podbuf`
does not exist.

- [ ] **Step 3: Implement the module**

The first-fit primitive is the one body this plan writes out:

```rust
pub fn place(items: &[(u32, u32)]) -> Vec<u32> {
    let mut placed: Vec<(u32, u32)> = Vec::new(); // (start, end)
    let mut offsets = Vec::with_capacity(items.len());
    for &(size, align) in items {
        let mut off = 0u32;
        loop {
            off = off.div_ceil(align) * align;
            match placed.iter().find(|&&(s, e)| off < e && s < off + size) {
                None => break,
                Some(&(_, end)) => off = end,
            }
        }
        placed.push((off, off + size));
        offsets.push(off);
    }
    offsets
}
```

`struct_head` walks the members in declaration order, skipping tombstones;
before optional field number `8k + 1` it places flags field `k` as a `(1, 1)`
item; a nested struct or tuple is one `(round_up(size, 8), 8)` item; a
descriptor is `(8, 4)`; the head size is the largest end rounded up to 8; pads
are the gaps. `max_size` uses `checked_*` arithmetic throughout and applies the
two `u16` limits.

- [ ] **Step 4: Run them**

`cargo test -p ridl-ir podbuf` — expected: pass.

- [ ] **Step 5: Commit**

`feat(ridl-ir): compute the podbuf layout by the first-fit law`

---

## Task 3: The podbuf size state and the build-time diagnostics

`Encoding::Podbuf` gets a state from the law (Task 2), so `Sizes<Podbuf>`, the
descriptor rows and RIDL-150 see it; and the three podbuf errors of §6.1 and
§8.1 are raised, each naming the type, when a `[backend.<name>] encodings` list
names `podbuf`.

**Files:**

- Create: `crates/ridl-ir/src/projection/size/podbuf.rs`,
  `crates/ridlc/src/podbuf_checks.rs`
- Modify: `crates/ridl-ir/src/projection/size.rs` (route `Encoding::Podbuf` to
  `podbuf::state` and `podbuf::tuple_state`), `crates/ridl-core/src/diag.rs`
  (RIDL-151, RIDL-152, RIDL-153 in the RIDL catalogue, `Error`, no lint name),
  `crates/ridlc/src/lib.rs` (call `podbuf_checks::check` where
  `check_design_lints` is called, with the same encodings list),
  `crates/ridlc/src/design_lints/ceiling.rs` (`checked_encodings` returns a
  `(bool, bool, bool)` and RIDL-150 reads the podbuf state),
  `crates/ridl-descriptor/tests/lower.rs` (a podbuf row is now present),
  `docs/specification/ridl-language-reference.md` §16 (the three rows),
  `freeze/diagnostics.txt` (added lines)
- Test: `crates/ridl-ir/src/projection/size/podbuf.rs` (`mod tests`),
  `crates/ridlc/tests/podbuf_checks.rs`, `crates/ridlc/tests/payload_ceiling.rs`

**Interfaces:**

- Produces:
  `pub(crate) fn state<'a>(home: &'a Package, type_name: &str, ctx: &Ctx<'a>) -> SizeState`
  and `pub(crate) fn tuple_state<'a>(...) -> SizeState` in
  `projection::size::podbuf` (`Bounded(u32)`; `Absent(Overflow)` for a `None`
  from `podbuf::max_size`; `Absent(Unresolved)` as the FlatBuffers sizer).
  `pub(crate) fn podbuf_checks::check(db: &RidlDatabase, packages: &[CheckedPackage], encodings: &[String], sources: &mut SourceMap) -> Vec<Diagnostic>`;
  `DiagCode::RIDL_151`, `RIDL_152`, `RIDL_153`.
- Consumes: `projection::podbuf::{max_size, struct_head, field_form}`,
  `ridl_ir::zero::{range_holds_zero, enum_zero_member}` for RIDL-153's "type
  excludes 0", the field's `Init` (`derivable`, declared) for "neither declared
  nor derivable", and the `deprecated` doc tag as the IR carries it
  (`Field.deprecated`, ADR-0026).

- [ ] **Step 1: Write the failing tests**

```rust
// crates/ridlc/tests/podbuf_checks.rs
#[test]
fn a_type_above_u32_max_bytes_is_ridl_151_naming_it() {
    let out = check_fixture("podbuf_too_large.ridl", &["podbuf"]);
    assert_eq!(codes(&out), vec!["RIDL-151"]);
    assert!(out.diagnostics[0].message.contains("`limits.Huge`"));
}

#[test]
fn an_array_above_65535_elements_or_bytes_per_element_is_ridl_152() {
    let out = check_fixture("podbuf_descriptor_limits.ridl", &["podbuf"]);
    assert_eq!(codes(&out), vec!["RIDL-152", "RIDL-152"]);   // `count` and `wide` fields
}

#[test]
fn a_deprecated_field_with_no_valid_value_is_ridl_153() {
    // `@deprecated speed : Speed` with Speed = km/h [1.0..250.0], no init.
    let out = check_fixture("podbuf_deprecated.ridl", &["podbuf"]);
    assert_eq!(codes(&out), vec!["RIDL-153"]);
    // The same field with `= 10.0` draws nothing.
    assert!(codes(&check_fixture("podbuf_deprecated_with_init.ridl", &["podbuf"])).is_empty());
}

#[test]
fn the_checks_are_silent_when_no_backend_names_podbuf() {
    assert!(codes(&check_fixture("podbuf_too_large.ridl", &["flatbuffers"])).is_empty());
}

// crates/ridl-ir/src/projection/size/podbuf.rs
#[test]
fn a_struct_is_bounded_at_its_podbuf_max_size() {
    assert_eq!(state_of("FaultPage"), SizeState::Bounded(8 + 8 + 64 * 24 + 64 * 1025));
}

// crates/ridlc/tests/payload_ceiling.rs
#[test]
fn ridl_150_reads_the_podbuf_size_when_the_manifest_names_podbuf() { /* a payload above 16,256 bytes under podbuf only */ }
```

- [ ] **Step 2: Run them**

`cargo test -p ridlc podbuf -p ridl-ir size::podbuf` — expected: compile errors
on the missing module and codes.

- [ ] **Step 3: Implement**

Add the three catalogue rows with the `diag_codes!` macro (summaries: "type too
large for podbuf", "array or map exceeds the podbuf descriptor limits",
"deprecated field has no valid podbuf value"), the sizer, the check, and the
`ceiling.rs` triple. The check's span is the declaration's name; RIDL-152's span
is the field.

- [ ] **Step 4: Run them**

`cargo test -p ridlc -p ridl-ir -p ridl-core -p ridl-descriptor` — expected:
pass; the catalogue guard tests in `ridl-core` pass with the three Error rows.

- [ ] **Step 5: Commit**

`feat(ridlc): size podbuf payloads and report the podbuf limits`

---

## Task 4: Expose the computed layout in the codegen `Model`

§10's last paragraph: each field's offset, size, alignment and form, each
descriptor's element type, and a union's tag and arm layout, carried in the
request so a plugin never computes the law again (ADR-0020 decisions 8 and 9).

**Files:**

- Modify: `crates/ridl-ir/proto/ridl/codegen/v1/model.proto`
  (`PodbufProjection podbuf = 11;` on `Model`; `SizeState podbuf = 3;` on
  `PayloadSizes`; `ReservationState podbuf = 3;` on `Reservation`; the new
  messages below), `crates/ridl-ir/src/codegen/lower.rs` (compute and set
  `model.podbuf`), `freeze/codegen-protocol.txt` (added lines),
  `docs/design/codegen-plugins.md` (a "The podbuf projection" section),
  `docs/book/codegen-plugins.md` (the `podbuf` bullet replaces the "no payload
  has a size state" sentence)
- Create: `crates/ridl-ir/src/codegen/podbuf.rs`
- Test: `crates/ridl-ir/src/codegen/tests.rs`,
  `crates/ridlc/tests/codegen_model.rs`

**Interfaces:**

- Produces (proto):
  `message PodbufProjection { repeated PodbufHead heads = 1; }`;
  `message PodbufHead { oneof source { uint32 declaration = 10; uint32 tuple = 11; MapEntry map_entry = 12; ArmHead arm = 13; } uint32 size = 2; uint32 align = 3; repeated PodbufField fields = 4; repeated PodbufPad pads = 5; }`;
  `message PodbufField { PodbufSource source = 1; uint32 offset = 2; uint32 size = 3; uint32 align = 4; PodbufForm form = 5; optional PodbufFlagBit optional = 6; }`;
  `message PodbufForm { oneof kind { bool boolean = 10; PodbufInt int = 11; FloatWidth float = 12; PodbufEnum enum = 13; PodbufEnumSet enum_set = 14; uint32 inline_head = 15; PodbufInlineArray inline_array = 16; PodbufDescriptor descriptor = 17; } }`;
  `message PodbufDescriptor { oneof kind { PodbufBound text = 10; PodbufBound bytes = 11; PodbufArray array = 12; PodbufMap map = 13; PodbufArms arms = 14; } }`
  with `elem_size`, `min`, `max` fields and head indexes into `heads`.
- Produces (Rust):
  `pub(crate) fn codegen::podbuf::project(scope: &Scope, named: &[flatbuffers::NamedTuple<'_>]) -> Result<v1::PodbufProjection, String>`
  — walks declarations in order, then induced tuples as the FlatBuffers lowering
  does, then map entries and arm heads in discovery order.
- Consumes:
  `projection::podbuf::{struct_head, tuple_head, union_head, Head, Form, DescriptorKind}`.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/ridl-ir/src/codegen/tests.rs
#[test]
fn the_model_carries_the_podbuf_head_of_every_struct() {
    let model = lower_corpus("veh-cluster", "veh.cluster");
    let event = podbuf_head(&model, "FaultEvent");
    assert_eq!(event.size, 24);
    assert_eq!(field_offsets(event), vec![0, 4, 16]);
    assert_eq!(event.pads.iter().map(|p| (p.offset, p.size)).collect::<Vec<_>>(), vec![(2, 2), (12, 4)]);
}

#[test]
fn the_podbuf_projection_round_trips_through_canonical_json() {
    let model = lower_corpus("veh-cluster", "veh.cluster");
    let json = codegen::to_json_pretty(&model).unwrap();
    assert_eq!(codegen::from_json(&json).unwrap().podbuf, model.podbuf);
}

// crates/ridlc/tests/codegen_model.rs
#[test]
fn emit_codegen_model_writes_the_podbuf_projection() {
    let json = emit_codegen_model("veh-cluster");
    assert!(json["podbuf"]["heads"].as_array().unwrap().iter().any(|h| h["size"] == 24));
}
```

- [ ] **Step 2: Run them**

`cargo test -p ridl-ir codegen -p ridlc codegen_model` — expected: compile
error, `Model` has no `podbuf` field.

- [ ] **Step 3: Implement**

Add the messages (append-only, new field numbers), the lowering, and the design
record section. `cargo xtask freeze update` adds lines to
`freeze/codegen-protocol.txt`; no line is removed (the `Encoding values` line
changed in Task 1).

- [ ] **Step 4: Run them**

`cargo test -p ridl-ir -p ridlc -p ridlc-gen-model -p ridlc-gen-rust` —
expected: pass, parity included (the model is the same on both hosts).

- [ ] **Step 5: Commit**

`feat(ridl-ir): carry the podbuf layout in the codegen model`

---

## Task 5: The `ridl-backend-podbuf` crate and the C header

A wire backend in the shape of `ridl-backend-flatbuffers`: `generate_with` per
ADR-0017 decision 1, a `Backend` behind the plugin contract,
`ridl build --emit podbuf` writing `<base>.h`. The header holds the preamble
struct, one head struct per type with `_Alignas(8)` on its first member and the
`_padN` fields written out, `_Static_assert` on `sizeof` and `offsetof` for
every type and field, the enum constants, inline accessor functions that read a
field or a string's pointer and length from a verified buffer, and `#error` on a
big-endian target (§4, §10).

**Files:**

- Create: `crates/ridl-backend-podbuf/Cargo.toml`,
  `crates/ridl-backend-podbuf/src/lib.rs`,
  `crates/ridl-backend-podbuf/src/contract.rs`,
  `crates/ridl-backend-podbuf/src/tests.rs`,
  `crates/ridl-backend-podbuf/tests/corpus.rs`,
  `crates/ridl-backend-podbuf/tests/c_compile.rs`,
  `crates/ridl-backend-podbuf/tests/snapshots/`
- Modify: `Cargo.toml` (workspace dependency entry), `.git-std.toml` (scope
  `ridl-backend-podbuf`), `crates/ridlc/Cargo.toml`, `crates/ridlc/src/lib.rs`
  (`Emit::Podbuf`, `backend_table() = Some("podbuf")`,
  `carries_codec() = false`, the `Backend::new(raw)` arm),
  `xtask/src/freeze/wire.rs` (a `podbuf` line per head field:
  `podbuf <pkg.Type> <field> <offset> <size>`, read from
  `--emit codegen-model`), `xtask/tests/oracle_boundary.rs` (the new crate takes
  no oracle dependency), `freeze/cli.txt` and `freeze/wire.txt` (added lines),
  `docs/book/cli-reference.md` (the emit), `.github/workflows/ci.yml` (install
  `clang` in the job that runs `just test`),
  `docs/technotes/walking-skeleton-architecture.md` (the crate's entry)
- Test: as created above

**Interfaces:**

- Produces: `pub struct Generated { pub header_source: String }`,
  `pub struct GenerateError { pub message: String }`,
  `pub fn generate(package: &v2::Package) -> Result<Generated, GenerateError>`,
  `pub fn generate_with(package: &v2::Package, others: &[&v2::Package]) -> Result<Generated, GenerateError>`,
  `pub struct Backend<'a>` with `impl codegen::Backend`
  (`language() = "podbuf"`, one file `<artifact_base>.h`, refuses any option).
- Consumes: `ridl_ir::projection::podbuf::*`,
  `ridl_ir::name::{snake_case, screaming_snake}`,
  `ridl_ir::codegen::{comment_preamble, RawIr}`.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/ridl-backend-podbuf/src/tests.rs
#[test]
fn fault_event_head_is_declared_with_pads_and_static_asserts() {
    let h = generate_fixture("fault_event.typl").header_source;
    assert!(h.contains("typedef struct veh_cluster_fault_event_head {"));
    assert!(h.contains("_Alignas(8) uint16_t code;"));
    assert!(h.contains("uint8_t _pad2[2];"));
    assert!(h.contains("ridl_podbuf_desc message;"));
    assert!(h.contains("uint8_t _pad12[4];"));
    assert!(h.contains("int64_t timestamp;"));
    assert!(h.contains("_Static_assert(sizeof(veh_cluster_fault_event_head) == 24"));
    assert!(h.contains("_Static_assert(offsetof(veh_cluster_fault_event_head, timestamp) == 16"));
}

#[test]
fn the_header_refuses_a_big_endian_target() {
    assert!(generate_fixture("door.typl").header_source
        .contains("#if __BYTE_ORDER__ != __ORDER_LITTLE_ENDIAN__\n#error"));
}

#[test]
fn a_type_above_the_limits_is_refused_naming_it() {
    let err = generate_fixture_err("too_large.typl");
    assert!(err.message.contains("`limits.Huge`"));
}

// crates/ridl-backend-podbuf/tests/c_compile.rs
#[test]
fn the_veh_cluster_header_compiles_as_c11_on_the_host() {
    // `cc -std=c11 -Wall -Wextra -Werror -fsyntax-only`; skip with a notice when no `cc`.
}

#[test]
fn the_static_asserts_hold_on_i686_aarch64_and_armv8r() {
    // `clang --target=<t> -std=c11 -ffreestanding -fsyntax-only -Werror` for
    // i686-linux-gnu, aarch64-linux-gnu, armv8r-none-eabihf; skip each target
    // clang refuses, with a notice; RIDL_REQUIRE_CC=1 turns a skip into a failure.
}
```

- [ ] **Step 2: Run them**

`cargo test -p ridl-backend-podbuf` — expected: the crate does not exist.

- [ ] **Step 3: Implement**

The crate, the contract adapter, the `ridlc` wiring and the dump lines. Head
struct names are `<snake pkg path>_<snake type>_head`; a nested head is a member
of its own struct type; an exact-length array is `T name[N]`; a descriptor
member is `ridl_podbuf_desc` (`uint32_t off, len`) or `ridl_podbuf_array_desc`
(`uint32_t off; uint16_t len, elem_size`). Accessors take `const uint8_t *buf`
and read through the preamble's `head_size`, returning 0 for a field past it.
`cargo xtask freeze update` adds the `cli` and `wire` lines.

- [ ] **Step 4: Run them**

`cargo test -p ridl-backend-podbuf -p ridlc` — expected: pass; the corpus
snapshot for `veh-cluster` is reviewed and accepted.

- [ ] **Step 5: Commit**

`feat(ridl-backend-podbuf): emit the podbuf C header`

---

## Task 6: The `podbuf` module of `ridl-rt`

The shared helpers a generated `Payload<Podbuf>` implementation calls: the
preamble, the descriptors, the little-endian loads that read 0 past the
producer's size, the writer that zeroes what it reserves, the text check, the
aligned buffer wrapper, and the new `Malformed` variants (§5.4, §7, question 5).

**Files:**

- Create: `crates/ridl-rt/src/podbuf.rs`
- Modify: `crates/ridl-rt/src/lib.rs`
  (`#[cfg(feature = "podbuf")] pub mod podbuf;`, the module list in the crate
  doc), `crates/ridl-rt/src/payload.rs`
  (`Malformed::{Preamble, Stride, Terminator, Utf8, KeyOrder}`),
  `crates/ridl-rt/Cargo.toml` (docs.rs `all-features` already on),
  `docs/design/ridl-rt.md` ("Module layout" and "Features" sections)
- Test: `crates/ridl-rt/src/podbuf.rs` (`mod tests`),
  `crates/ridl-rt/tests/podbuf.rs`

**Interfaces:**

- Produces (`ridl_rt::podbuf`, behind `podbuf`, `no_std`, no `unsafe`):
  - `#[repr(C, align(8))] pub struct Aligned<T>(pub T);` with
    `impl<const N: usize> AsRef<[u8]> + AsMut<[u8]> for Aligned<[u8; N]>`,
    `Copy`, and `pub const fn zeroed() -> Self`.
  - `pub const PREAMBLE_SIZE: usize = 8;`
  - `pub struct Preamble { pub head_size: u32 }` with
    `pub fn read(buf: &[u8]) -> Result<Preamble, VerifyError>` — rule 1: pointer
    8-aligned (`Malformed::Unaligned`), `len >= 8`, `head_size % 8 == 0`,
    `8 + head_size <= len` (`Malformed::Preamble`), `reserved` ignored.
  - `pub struct Region<'a> { buf: &'a [u8], start: usize, size: usize }` with
    `pub fn new(buf, start, size) -> Region` and
    `pub fn u8_at(&self, off: usize) -> u8` … `u64_at`, `i8_at` … `i64_at`,
    `f32_at`, `f64_at`, `bytes_at(off, n) -> impl Iterator<Item = u8>` — each
    reads 0 for a byte at or past `size`, so a field past the producer's head,
    element or arm reads as 0 (§8.1).
  - `pub struct Desc { pub off: u32, pub len: u32 }`,
    `pub struct ArrayDesc { pub off: u32, pub len: u16, pub elem_size: u16 }`,
    each with `pub fn read(region: &Region<'_>, at: usize) -> Self` and
    `pub fn is_zero(&self) -> bool`.
  - `pub fn tail_range(off: u32, bytes: u64, buf_len: usize, min_off: usize, align: usize) -> Result<core::ops::Range<usize>, VerifyError>`
    — rule 4 with checked arithmetic.
  - `pub fn text(bytes: &[u8]) -> Result<&str, VerifyError>` — rule 6: no
    interior NUL (`Malformed::Terminator`), valid UTF-8 (`Malformed::Utf8`); the
    caller passes `len + 1` bytes and this checks the last is NUL.
  - `pub fn scalar_count(s: &str) -> usize`.
  - `pub struct Writer<'o>` with `pub fn new(out: &'o mut [u8]) -> Writer<'o>`,
    `pub fn reserve_head(&mut self, size: u32) -> Result<usize, EncodeError>`
    (writes the preamble, zeroes the head),
    `pub fn tail(&mut self, bytes: u64, align: usize) -> Result<usize, EncodeError>`
    (zero-fills the alignment gap, default of question 4),
    `pub fn put_u8(&mut self, at: usize, v: u8)` … `put_f64`,
    `pub fn put_desc(&mut self, at: usize, d: Desc)`,
    `pub fn put_array_desc(...)`,
    `pub fn finish(self) -> Result<&'o [u8], EncodeError>`.
  - `#[cfg(not(target_endian = "little"))] compile_error!("podbuf requires a little-endian target");`
- Consumes: `payload::{VerifyError, Malformed, EncodeError}`.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/ridl-rt/tests/podbuf.rs
#[test]
fn the_preamble_check_is_rule_1() {
    let buf = Aligned([0u8; 16]);
    assert!(matches!(Preamble::read(&buf.0[1..]), Err(VerifyError::Structure(Malformed::Unaligned))));
    assert!(matches!(Preamble::read(&buf.0[..7]), Err(VerifyError::Structure(Malformed::Preamble))));
    let mut b = Aligned([0u8; 16]); b.0[0] = 12;             // head_size 12, not a multiple of 8
    assert!(matches!(Preamble::read(&b.0), Err(VerifyError::Structure(Malformed::Preamble))));
    let mut b = Aligned([0u8; 16]); b.0[0] = 16;             // 8 + 16 > 16
    assert!(matches!(Preamble::read(&b.0), Err(VerifyError::Structure(Malformed::Preamble))));
    let mut b = Aligned([0u8; 16]); b.0[0] = 8; b.0[4..8].copy_from_slice(&[0xff; 4]);
    assert_eq!(Preamble::read(&b.0).unwrap().head_size, 8);   // reserved ignored
}

#[test]
fn a_region_reads_zero_past_the_producer_size() {
    let buf = Aligned([0xffu8; 32]);
    let r = Region::new(&buf.0, 8, 8);
    assert_eq!(r.u8_at(7), 0xff);
    assert_eq!(r.u8_at(8), 0);
    assert_eq!(r.u64_at(4), 0x0000_0000_ffff_ffff);           // straddles the size
}

#[test]
fn tail_range_uses_checked_arithmetic() {
    assert!(matches!(tail_range(u32::MAX - 3, 8, 64, 16, 1), Err(VerifyError::Structure(Malformed::OutOfBounds))));
    assert!(matches!(tail_range(12, 4, 64, 16, 1), Err(VerifyError::Structure(Malformed::OutOfBounds))));   // inside the head
    assert!(matches!(tail_range(18, 8, 64, 16, 8), Err(VerifyError::Structure(Malformed::Unaligned))));
    assert_eq!(tail_range(16, 8, 64, 16, 8).unwrap(), 16..24);
}

#[test]
fn text_requires_a_terminator_and_no_interior_nul() {
    assert_eq!(text(b"ab\0").unwrap(), "ab");
    assert!(matches!(text(b"ab"), Err(VerifyError::Structure(Malformed::Terminator))));
    assert!(matches!(text(b"a\0b\0"), Err(VerifyError::Structure(Malformed::Terminator))));
    assert!(matches!(text(b"\xff\0"), Err(VerifyError::Structure(Malformed::Utf8))));
}

#[test]
fn the_writer_zeroes_the_head_and_the_alignment_gaps() {
    let mut out = Aligned([0xaau8; 64]);
    let mut w = Writer::new(&mut out.0);
    let head = w.reserve_head(16).unwrap();
    assert_eq!(head, 8);
    let s = w.tail(5, 1).unwrap();                            // 24..29
    let e = w.tail(24, 8).unwrap();                           // 32..56, gap 29..32 zeroed
    let bytes = w.finish().unwrap();
    assert_eq!(&bytes[..8], &[16, 0, 0, 0, 0, 0, 0, 0]);
    assert!(bytes[8..24].iter().all(|b| *b == 0));
    assert!(bytes[s + 5..e].iter().all(|b| *b == 0));
    assert_eq!(bytes.len(), 56);
}
```

- [ ] **Step 2: Run them**

`cargo test -p ridl-rt --features podbuf --test podbuf` — expected: compile
error, no `podbuf` module.

- [ ] **Step 3: Implement**

The module as specified; every load is `u32::from_le_bytes` over bytes the
region fetched with `get(..).copied().unwrap_or(0)`. Add the five `Malformed`
variants (the enum is `#[non_exhaustive]`, so the addition is compatible).

- [ ] **Step 4: Run them**

`cargo test -p ridl-rt --all-features` and
`cargo check -p ridl-rt --target thumbv7em-none-eabihf --features podbuf` —
expected: pass; `grep -n unsafe crates/ridl-rt/src/podbuf.rs` prints nothing.

- [ ] **Step 5: Commit**

`feat(ridl-rt): add the podbuf module behind the podbuf feature`

---

## Task 7: The generated head struct and encoder

The Rust half of §10's first two bullets: the `#[repr(C, align(8))]` head struct
with pad fields and `const` assertions, and `encode`, which writes the preamble,
the head and the tail in field order depth first, zeroes every pad, the reserved
word and every byte of an absent optional, writes a deprecated field's init
value, sorts map entries and refuses a string holding U+0000 (§5, §6, §7
"Encoder duty", §8.1, §14 "Tail order").

**Files:**

- Create: `crates/ridl-backend-rust/src/codec_podbuf.rs`,
  `crates/ridl-backend-rust/tests/fixtures/podbuf_roundtrip.ridl`,
  `crates/ridl-backend-rust/tests/podbuf_encode.rs`
- Modify: `crates/ridl-backend-rust/src/lib.rs` (`mod codec_podbuf;`,
  `WireEncoding::Podbuf`, `parse`/`name`/`check_emitted`, the `package_items`
  arm), `crates/ridl-backend-rust/src/contract.rs` (`parse_encoding` accepts
  `podbuf`), `crates/ridl-backend-rust/src/claims.rs` (claim `<T>PodbufHead`,
  `<T>PodbufView`), `crates/ridl-backend-rust/src/transcode.rs` (`encoding_path`
  arm), `crates/ridl-backend-rust/tests/support/rustc.rs`
  (`--cfg feature="podbuf"` replaces `repr-c`)
- Test: `crates/ridl-backend-rust/tests/podbuf_encode.rs`,
  `crates/ridl-backend-rust/src/tests.rs`

**Interfaces:**

- Produces:
  `pub(crate) fn codec_podbuf::package_items(ctx: &Ctx) -> Result<(Vec<TokenStream>, Vec<transcode::PayloadOwner>), GenerateError>`;
  `pub(crate) fn codec_podbuf::layout(ctx: &Ctx) -> Result<Vec<HeadLayout>, GenerateError>`
  and `pub use codec_podbuf::HeadLayout as PodbufHeadLayout;` (the drift test of
  Task 10 compares it with the model's projection). Generated per type `T`:
  `#[repr(C, align(8))] pub struct TPodbufHead { ... }` with
  `const _: () = { assert!(size_of::<TPodbufHead>() == N); assert!(offset_of!(TPodbufHead, f) == o); ... };`,
  and
  `impl T { pub(crate) fn __ridl_podbuf_encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> }`.
  The `Payload<Podbuf>` implementation itself is Task 9's.
- Consumes: `v1::PodbufProjection` from `ctx.model` (Task 4),
  `ridl_rt::podbuf::Writer`.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/ridl-backend-rust/tests/podbuf_encode.rs — the generated code runs as a
// program through support::rustc, as flatbuffers_roundtrip.rs does.
#[test]
fn door_payload_encodes_to_the_measured_16_bytes() {
    let bytes = run("DoorPayload { sensor_id: 7, is_open: true }");
    assert_eq!(bytes, [8,0,0,0, 0,0,0,0,  7, 1, 0,0,0,0,0,0]);
}

#[test]
fn fault_event_writes_descriptor_pads_and_tail_in_order() {
    let bytes = run(r#"FaultEvent { code: 0x1234, message: "ab", timestamp: -1 }"#);
    assert_eq!(bytes.len(), 8 + 24 + 3);
    assert_eq!(&bytes[8..10], &[0x34, 0x12]);
    assert_eq!(&bytes[10..12], &[0, 0]);                       // _pad2
    assert_eq!(&bytes[12..20], &[32,0,0,0, 2,0,0,0]);           // off 32, len 2
    assert_eq!(&bytes[20..24], &[0, 0, 0, 0]);                  // _pad12
    assert_eq!(&bytes[24..32], &[0xff; 8]);
    assert_eq!(&bytes[32..35], b"ab\0");
}

#[test]
fn an_absent_optional_is_all_zero_and_a_present_one_sets_its_bit() {
    assert_eq!(&run("DiagFilter { severity: 2, category: None }")[8..24],
        &[2, 0, 0,0, 0,0,0,0, 0,0,0,0, 0,0,0,0]);
    let b = run(r#"DiagFilter { severity: 2, category: Some("x".into()) }"#);
    assert_eq!(b[9], 1);
    assert_eq!(&b[12..20], &[24,0,0,0, 1,0,0,0]);
}

#[test]
fn fault_page_elements_are_24_byte_heads_with_their_strings_after_the_block() {
    let b = run("FaultPage { faults: vec![ev(1, \"a\"), ev(2, \"bc\")] }");
    assert_eq!(&b[8..16], &[16,0,0,0, 2,0, 24,0]);              // off 16, len 2, elem_size 24
    assert_eq!(b.len(), 16 + 48 + 2 + 3);
}

#[test]
fn map_entries_are_sorted_by_key() {
    let b = run(r#"Labels { by_name: map([("b", 2), ("a", 1)]) }"#);
    assert_eq!(first_key(&b), "a");
}

#[test]
fn a_string_holding_u0000_is_refused_by_the_encoder() {
    assert!(run_result(r#"FaultEvent { code: 1, message: "a\0b", timestamp: 0 }"#)
        .contains("EncodeError"));
}

#[test]
fn a_deprecated_field_is_written_as_its_init_value() {
    // `@deprecated legacy : integer [1..9] = 5` encodes 5 whatever the domain value.
    assert_eq!(run("Legacy::default()")[8], 5);
}

// crates/ridl-backend-rust/src/tests.rs
#[test]
fn the_emitted_podbuf_items_hold_no_unsafe() {
    let src = generate_podbuf_fixture();
    assert!(!src.contains("unsafe"));
}
```

- [ ] **Step 2: Run them**

`cargo test -p ridl-backend-rust --test podbuf_encode` — expected: the backend
refuses `podbuf` (`WireEncoding::parse` returns `None`).

- [ ] **Step 3: Implement**

`WireEncoding::Podbuf`, the emitter over the model's `PodbufProjection`, the
head struct with pads and assertions, the encoder. Tail order is field order,
depth first (§14): an array block is written, then each element's own tail. Map
entries are sorted with the order of §6 before writing; the encoder returns
`EncodeError::Contract` for a duplicate key (`+0.0` and `-0.0` are one key) and
for a NaN key.

- [ ] **Step 4: Run them**

`cargo test -p ridl-backend-rust` — expected: pass; existing snapshots are
unchanged because no corpus manifest names `podbuf` yet.

- [ ] **Step 5: Commit**

`feat(ridl-backend-rust): emit the podbuf head struct and encoder`

---

## Task 8: The generated `verify`, the view and `decode`

Rules 1 to 9 of §7 in one non-recursive pass over the data present, and nothing
else (no pad, no uncovered tail byte, no overlap check); the view with one
bounds-checked accessor per field; `decode` into the owned domain type (§10, §14
"Verify depth", "Decode", "Floats in verify", "Empty union arm").

**Files:**

- Modify: `crates/ridl-backend-rust/src/codec_podbuf.rs`
- Create: `crates/ridl-backend-rust/tests/podbuf_verify.rs`
- Test: `crates/ridl-backend-rust/tests/podbuf_verify.rs`

**Interfaces:**

- Produces, generated per type `T`:
  `pub struct TPodbufView<'a> { buf: &'a [u8], at: usize, size: usize }` with
  `impl ridl_rt::payload::View<'a>`, one accessor per field
  (`pub fn code(&self) -> u16`, `pub fn message(&self) -> &'a str`,
  `pub fn category(&self) -> Option<&'a str>`,
  `pub fn faults(&self) -> impl Iterator<Item = FaultEventPodbufView<'a>>`, a
  map as an iterator of `(K, V)` views, a union as an enum of arm views, an enum
  field as the domain enum);
  `pub(crate) fn __ridl_podbuf_verify(buf: &[u8]) -> Result<TPodbufView<'_>, VerifyError>`;
  `pub(crate) fn __ridl_podbuf_decode(view: TPodbufView<'_>) -> T`.
- Consumes:
  `ridl_rt::podbuf::{Preamble, Region, Desc, ArrayDesc, tail_range, text, scalar_count}`,
  the typl constraint checks the other codecs call
  (`codec::scalar_constraint_check`).

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn verify_accepts_what_encode_wrote_and_the_view_reads_it_in_place() {
    let v = verify(encode(fault_event(0x1234, "ab", -1)));
    assert_eq!((v.code(), v.message(), v.timestamp()), (0x1234, "ab", -1));
}

#[test]
fn a_boolean_above_1_a_strict_enum_value_not_declared_and_an_undeclared_bit_are_refused() {
    assert_eq!(refusal(patch(door(), 9, 2)), "Contract");            // is_open = 2
    assert_eq!(refusal(patch(gear(), 8, 7)), "Contract");            // GearPosition 7
    assert_eq!(refusal(patch(flags(), 8, 0x80)), "Contract");         // WarningFlags bit 7
}

#[test]
fn an_absent_optional_whose_bytes_are_not_zero_is_refused() {
    assert_eq!(refusal(patch(filter_absent(), 12, 24)), "Contract");  // descriptor off with len 0
}

#[test]
fn rule_4_descriptor_edges() {
    let e = encode(fault_event(1, "ab", 0));
    assert_eq!(refusal(patch_u32(e.clone(), 12, 8)), "OutOfBounds");          // off inside the head
    assert_eq!(refusal(patch_u32(e.clone(), 12, u32::MAX - 1)), "OutOfBounds"); // off + len overflows
    assert_eq!(refusal(patch_u32(e.clone(), 16, 4 * 256 + 1)), "OutOfBounds");  // len above 4·max
    let empty_with_off = patch_u32(encode(fault_event(1, "", 0)), 12, 32);
    assert_eq!(refusal(empty_with_off), "OutOfBounds");                       // len 0, off not 0
}

#[test]
fn two_descriptors_over_the_same_bytes_are_accepted() {
    let b = encode(two_strings("ab", "ab"));
    assert!(verify_result(patch_u32(b, second_desc_off(), first_desc_off_value())).is_ok());
}

#[test]
fn rule_3_counts_scalars_after_rule_6_checks_bytes() {
    assert_eq!(refusal(encode_raw(short2(), b"abc")), "Contract");             // 3 bytes, 3 scalars, max 2
    assert!(verify_result(encode_raw(short2(), "𝄞𝄞".as_bytes())).is_ok());     // 8 bytes = 4·max, 2 scalars
    assert_eq!(refusal(raw_without_nul(short2(), b"ab")), "Terminator");
    assert_eq!(refusal(raw_with_bytes(short2(), b"a\0b\0")), "Terminator");
    assert_eq!(refusal(raw_with_bytes(short2(), b"\xff\xfe\0")), "Utf8");
}

#[test]
fn rule_5_stride_mismatch_is_refused() {
    let b = encode(fault_page(2));
    assert_eq!(refusal(patch_u16(b.clone(), 14, 20)), "Stride");       // not a multiple of 8
    let a = encode(array_of_u16(3));
    assert_eq!(refusal(patch_u16(a, 14, 4)), "Stride");                // scalar element: exact size only
}

#[test]
fn rule_7_and_8_unions_and_maps() {
    assert_eq!(refusal(patch(encode(sensor_ok()), 8, 3)), "Union");          // tag names no arm
    assert_eq!(refusal(patch_u32(encode(sensor_ok()), 16, 12)), "Union");    // arm len not a multiple of 8
    assert!(verify_result(encode(empty_arm())).is_ok());                     // len 0, all zero
    assert_eq!(refusal(patch_u32(encode(empty_arm()), 12, 32)), "OutOfBounds");
    assert_eq!(refusal(swap_entries(encode(labels()))), "KeyOrder");
    assert_eq!(refusal(dup_float_keys(0.0, -0.0)), "KeyOrder");
    assert_eq!(refusal(nan_key()), "Contract");
}

#[test]
fn a_float_with_no_range_accepts_nan() {
    assert!(verify_result(encode(free_float(f32::NAN))).is_ok());
}

#[test]
fn decode_builds_the_domain_value_from_the_view() {
    let value = fault_page(3);
    assert_eq!(FaultPage::decode(Ref::<FaultPage, Podbuf>::verify(&encode(value.clone())).unwrap()), value);
}

#[test]
fn verify_allocates_nothing() {
    // The program under test installs a counting global allocator and asserts
    // zero allocations across `verify` and every accessor of the view.
}
```

- [ ] **Step 2: Run them**

`cargo test -p ridl-backend-rust --test podbuf_verify` — expected: the generated
module has no `__ridl_podbuf_verify`.

- [ ] **Step 3: Implement**

`verify` as a worklist (not recursion) over `(region, head index)` items whose
depth is bounded by the type; each rule in the order of §7; a string's
scalar-value count checked after `text()` returns. The view's accessor is a
`Region` load at the field's offset.

- [ ] **Step 4: Run them**

`cargo test -p ridl-backend-rust` — expected: pass.

- [ ] **Step 5: Commit**

`feat(ridl-backend-rust): generate the podbuf verify, view and decode`

---

## Task 9: `Payload<Podbuf>`, `Sizes<Podbuf>` and the manifest wiring

The encoding contract of `ridl-rt` (§14 "ridl-rt integration"):
`Payload<Podbuf>` per type with the aligned buffer, `Sizes<Podbuf>` per
interface, the generated `Cargo.toml` feature, the transcode functions, the
corpus manifest, and the book.

**Files:**

- Modify: `crates/ridl-backend-rust/src/codec_podbuf.rs`,
  `crates/ridl-backend-rust/src/descriptors.rs` (the `Sizes<Podbuf>` table from
  `PayloadSizes.podbuf`), `crates/ridlc/src/lib.rs` (`ridl-rt` features in the
  emitted `Cargo.toml`), `crates/ridlc/tests/corpus/veh-cluster/ridl.toml`
  (`encodings = ["flatbuffers", "proto3", "podbuf"]`),
  `crates/ridl-backend-rust/tests/fixtures/transcode.ridl` and
  `tests/generated/transcode.rs` (regenerated),
  `crates/ridl-backend-rust/tests/transcode_equivalence.rs`,
  `crates/ridl-loopback/tests/encoding.rs` (`Loopback::<Podbuf>` round trip),
  `crates/ridlc/tests/cabin_example.rs` (the `Sizes<Podbuf>` absence test
  becomes a presence test over a fixture that names `podbuf`),
  `crates/ridlc/tests/snapshots/` and `crates/ridl-backend-rust/src/snapshots/`
  (accepted), `docs/book/generated-code.md` (the feature table row `podbuf`,
  "Converting a payload between encodings"), `docs/book/cli-reference.md`
- Test: `crates/ridl-backend-rust/tests/podbuf_roundtrip.rs` (created),
  `crates/ridl-backend-rust/tests/transcode_equivalence.rs`,
  `crates/ridlc/tests/rust_crate_emit.rs`

**Interfaces:**

- Produces, generated:
  `impl Payload<Podbuf> for T { const MAX_SIZE: usize = <podbuf max_size>; type Buffer = Aligned<[u8; MAX_SIZE]>; const BUFFER: Buffer = Aligned::zeroed(); type View<'a> = TPodbufView<'a>; fn encode(...); fn verify(...); fn decode(...) }`,
  `impl Sizes<Podbuf> for <Interface>` with
  `ClaimBuffer = Aligned<[u8; MAX_BUFFER_SIZE]>`,
  `T::transcode_podbuf_to_proto3`, `T::transcode_proto3_to_podbuf`, and the
  FlatBuffers pairs; the emitted `Cargo.toml` names `features = ["podbuf"]` when
  the manifest does.
- Consumes: Tasks 6, 7, 8.

- [ ] **Step 1: Write the failing tests**

```rust
// tests/podbuf_roundtrip.rs
#[test]
fn a_report_round_trips_through_ref_verify_and_decode() {
    let mut buf = <Report as Payload<Podbuf>>::BUFFER;
    let proof = Ref::<Report, Podbuf>::encode(&sample(), buf.as_mut()).unwrap();
    assert_eq!(Ref::<Report, Podbuf>::verify(proof.bytes()).unwrap().decode(), sample());
}

#[test]
fn max_size_is_pinned_and_holds_the_largest_legal_value() {
    assert_eq!(<FaultPage as Payload<Podbuf>>::MAX_SIZE, 8 + 8 + 64 * 24 + 64 * 1025);
    let mut buf = <FaultPage as Payload<Podbuf>>::BUFFER;
    assert!(Ref::<FaultPage, Podbuf>::encode(&largest_fault_page(), buf.as_mut()).is_ok());
}

#[test]
fn the_buffer_types_are_8_aligned() {
    assert_eq!(core::mem::align_of::<<FaultPage as Payload<Podbuf>>::Buffer>(), 8);
    assert_eq!(core::mem::align_of::<<Cluster as Sizes<Podbuf>>::ClaimBuffer>(), 8);
}

#[test]
fn a_short_buffer_reports_capacity() {
    let mut out = Aligned([0u8; 8]);
    assert!(matches!(Ref::<FaultEvent, Podbuf>::encode(&ev(1, "a"), &mut out.0), Err(EncodeError::Capacity { .. })));
}

// tests/transcode_equivalence.rs
#[test]
fn podbuf_transcodes_agree_with_the_direct_encodings() {
    assert_equivalent!(Reading, reading());     // extended to the six ordered pairs
}

// crates/ridlc/tests/rust_crate_emit.rs
#[test]
fn the_emitted_manifest_enables_the_podbuf_feature_when_named() {
    assert_eq!(ridl_rt_features(build("veh-cluster")), ["flatbuffers", "proto3", "podbuf"]);
}
```

- [ ] **Step 2: Run them**

`cargo test -p ridl-backend-rust --test podbuf_roundtrip -p ridlc rust_crate_emit`
— expected: no `Payload<Podbuf>` implementation.

- [ ] **Step 3: Implement**

The implementations, the size table, the manifest feature, the corpus manifest.
Accept the snapshots after reading them: every `veh-cluster` type gains a head
struct, a view, the assertions and the three functions.

- [ ] **Step 4: Run them**

`cargo test -p ridl-backend-rust -p ridlc -p ridl-loopback` — expected: pass,
including `veh_cluster_generated_rust_compiles_with_rustc`.

- [ ] **Step 5: Commit**

`feat(ridl-backend-rust): implement Payload<Podbuf> and Sizes<Podbuf>`

---

## Task 10: Conformance, evolution and the fuzz test of `verify`

The byte oracle for podbuf is the law itself: the bytes of a value are fully
determined, so conformance is byte equality against hand-written expected
buffers for the §5.3 types, plus the evolution pairs of §8.1, the stride
mismatches of Review Focus 4, the drift test between the codec's layout and the
model's projection, and the exhaustive truncation and byte-replacement test over
`verify` and the accessors (§12's fuzz result, kept as a test).

**Files:**

- Create: `crates/ridl-backend-rust/tests/podbuf_conformance.rs`,
  `crates/ridl-backend-rust/tests/fixtures/podbuf_evolution_v1.ridl`,
  `crates/ridl-backend-rust/tests/fixtures/podbuf_evolution_v2.ridl`,
  `crates/ridl-backend-rust/tests/support/podbuf.rs`
- Modify: `crates/ridl-backend-rust/tests/support/rustc.rs` (a
  `thumbv7em-none-eabihf` check of the generated podbuf module, as
  `the_generated_codec_checks_for_wasm32` does for FlatBuffers)
- Test: `crates/ridl-backend-rust/tests/podbuf_conformance.rs`

**Interfaces:**

- Consumes: `ridl_backend_rust::PodbufHeadLayout` (Task 7),
  `ridl_ir::codegen::lower`, the generated program harness.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn the_value_table_encodes_to_the_expected_bytes() {
    for row in VALUE_TABLE {              // DoorPayload, ClimateReport, DiagFilter (absent, present), FaultEvent, FaultPage (0, 1, 4 events), SensorResult (ok, err), a map
        assert_eq!(encode(row.value), row.bytes, "{}", row.name);
        assert_eq!(decode(verify(row.bytes)), row.value, "{}", row.name);
    }
}

#[test]
fn an_older_reader_ignores_an_appended_field_in_a_longer_head() {
    // v2 appends `priority: u8` to FaultEvent; first-fit places it at offset 2, inside v1's 24 bytes.
    let v2_bytes = encode_v2(fault_event_v2(1, "a", 0, 9));
    let v1 = verify_v1(v2_bytes);                     // accepted: pads are not checked
    assert_eq!(v1.code(), 1);
}

#[test]
fn a_newer_reader_reads_zero_for_a_field_the_producer_did_not_write() {
    let v1_bytes = encode_v1(fault_event_v1(1, "a", 0));
    assert_eq!(verify_v2(v1_bytes).priority(), 0);    // 0 is a valid `integer [0..9]`
}

#[test]
fn a_newer_reader_reads_zero_past_a_shorter_head_size() {
    // v2 also appends `extra: i64`, which falls past 24, so v2's head is 32 and v1's is 24.
    assert_eq!(verify_v2(encode_v1(fault_event_v1(1, "a", 0))).extra(), 0);
}

#[test]
fn stride_differs_from_the_reader_in_both_directions() {
    // Elements written by v2 (elem_size 32) read by v1: steps by 32, reads the 24-byte prefix.
    assert_eq!(verify_v1(encode_v2(fault_page_v2(2))).faults().count(), 2);
    // Elements written by v1 (elem_size 24) read by v2: `extra` reads 0 and the flag checks still run.
    assert!(verify_v2(encode_v1(fault_page_v1(2))).faults().all(|e| e.extra() == 0));
}

#[test]
fn the_codec_layout_matches_the_model_projection() {
    // `PodbufHeadLayout` per type equals the `PodbufHead` of `--emit codegen-model`, over the
    // fixture and every corpus package (the drift test the proto3 suite has).
}

#[test]
fn every_truncation_and_byte_replacement_is_refused_or_read_in_bounds() {
    // For each buffer of VALUE_TABLE: every prefix length, and every single byte replaced by
    // 0x00, 0xff and its bitwise negation; verify must return (no panic); when it accepts,
    // every accessor of the view is called and every string is read to its end.
}

#[test]
fn a_seeded_random_mutation_pass_finds_no_panic() {
    // proptest with a fixed seed, 10_000 cases: random buffers up to MAX_SIZE and random
    // multi-byte mutations of the table's buffers, same assertion as above.
}

#[test]
fn the_generated_podbuf_module_checks_for_thumbv7em() { /* rustc --target thumbv7em-none-eabihf --emit=metadata */ }
```

- [ ] **Step 2: Run them**

`cargo test -p ridl-backend-rust --test podbuf_conformance` — expected: the
evolution fixtures and the support module do not exist.

- [ ] **Step 3: Implement**

The fixtures, the hand-written byte table (each row's bytes derived by hand from
§5.2 and §5.3 and commented field by field), the mutation loops.

- [ ] **Step 4: Run them**

`cargo test -p ridl-backend-rust --test podbuf_conformance` — expected: pass.

- [ ] **Step 5: Commit**

`test(ridl-backend-rust): podbuf conformance, evolution and mutation tests`

---

## Task 11: The podbuf break rules of `ridl diff`

§8.1, §8.2 and §8.4 without the `open` rules: an append to a struct nested
inline anywhere in the workspace is a podbuf break naming each inline use; a
field removed with a `reserved` tombstone is a podbuf break; an appended
non-optional field whose type excludes 0 stays breaking under the existing rule
(driftsys/ridl#598), and a test pins that the rule is podbuf's too. The scoping
default of question 1.

**Files:**

- Modify: `crates/ridl-diff/src/lib.rs` (`Category::PodbufInlineAppend`,
  `Category::PodbufFieldRetired`, their `--explain` text, `heading()`),
  `crates/ridl-diff/src/walk.rs`, `crates/ridl-diff/src/classify.rs`,
  `crates/ridl-diff/src/classify/classify_tests.rs`,
  `crates/ridl-diff/src/tests.rs`, `crates/ridl/src/main.rs`
  (`ridl diff --encoding <name>`, repeatable), `crates/ridl-mcp/src/tools.rs`
  and `tools.json` (optional `encodings` input), `freeze/cli.txt`,
  `freeze/mcp.txt` (added lines), `docs/book/evolving-an-interface.md` ("The
  verdicts": the podbuf rows), `docs/book/cli-reference.md`,
  `docs/design/mcp-workspace-tools.md`
- Test: `crates/ridl-diff/src/tests.rs`, `crates/ridl/tests/diff_cli.rs`

**Interfaces:**

- Produces:
  `pub fn diff_refs_with(old: &[&Package], new: &[&Package], context: &[&Package], encodings: &[&str]) -> DiffReport`;
  the two categories; `Change.detail` for `PodbufInlineAppend` names each inline
  use (`veh.common/SensorBounds/range`, …).
- Consumes: `projection::podbuf::field_form` to decide "nested inline" (a
  `Form::Inline` or `Form::InlineArray` reached from any struct, tuple or map
  entry of the new side).

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn appending_to_a_struct_nested_inline_is_a_podbuf_break_naming_the_uses() {
    let report = diff_with_encodings(INLINE_V1, INLINE_V2, &["podbuf"]);
    let change = report.changes.iter().find(|c| c.category == Category::PodbufInlineAppend).unwrap();
    assert_eq!(change.verdict, Verdict::Breaking);
    assert_eq!(change.path, "veh.common/SensorReading/quality");
    assert!(change.detail.as_deref().unwrap().contains("veh.common/SensorBounds/range"));
}

#[test]
fn the_same_append_is_compatible_without_podbuf_and_listed_under_its_heading() {
    let report = diff_with_encodings(INLINE_V1, INLINE_V2, &[]);
    assert_eq!(report.verdict, Verdict::Compatible);
    assert!(render(&report).contains("breaking under podbuf"));
}

#[test]
fn a_tombstoned_field_is_a_podbuf_break() {
    let report = diff_with_encodings(RETIRE_V1, RETIRE_V2, &["podbuf"]);
    assert!(report.changes.iter().any(|c| c.category == Category::PodbufFieldRetired && c.verdict == Verdict::Breaking));
}

#[test]
fn an_appended_field_whose_type_excludes_zero_is_breaking_under_every_encoding() {
    for encodings in [&[][..], &["podbuf"][..]] {
        assert_eq!(diff_with_encodings(APPEND_V1, APPEND_NONZERO_V2, encodings).verdict, Verdict::Breaking);
    }
}

#[test]
fn an_appended_field_to_an_array_element_type_is_compatible_under_podbuf() {
    assert_eq!(diff_with_encodings(ELEMENT_V1, ELEMENT_V2, &["podbuf"]).verdict, Verdict::Compatible);
}

// crates/ridl/tests/diff_cli.rs
#[test]
fn diff_encoding_podbuf_exits_1_on_an_inline_append() { /* exit code 1 with the flag, 0 without */ }
```

- [ ] **Step 2: Run them**

`cargo test -p ridl-diff podbuf` — expected: compile error on the categories.

- [ ] **Step 3: Implement**

The walk emits the categories; the classifier sets the verdict from `encodings`;
the three `match` arms over `Category` that deny wildcard arms gain the new
variants; `--explain` text; the CLI flag and the MCP input.

- [ ] **Step 4: Run them**

`cargo test -p ridl-diff -p ridl -p ridl-mcp` — expected: pass.

- [ ] **Step 5: Commit**

`feat(ridl-diff): report the podbuf break rules`

---

## Task 12: The padding lint RIDL-154 `podbuf-padding-order`

§8.4's second bullet: a declaration order whose first-fit layout wastes padding
that another order would save, reported before a type's first release. The
"released" test and the home are the default of question 7.

**Files:**

- Modify: `crates/ridl-core/src/diag.rs` (RIDL-154, `Info`,
  `lint = "podbuf-padding-order"`), `crates/ridl/src/main.rs` (run the lint from
  `run_check` when a `[backend.<name>] encodings` list names `podbuf`; the
  baseline location is the one `desk_check` resolves),
  `crates/ridl/src/podbuf_padding.rs` (created), `docs/book/lints.md` (the row),
  `docs/specification/ridl-language-reference.md` §16, `freeze/diagnostics.txt`
- Test: `crates/ridl/tests/lints.rs`, `crates/ridl/tests/baseline_desk.rs`

**Interfaces:**

- Produces:
  `pub(crate) fn podbuf_padding::check(packages: &[v2::Package], released: &BTreeSet<(String, String)>, sources: &mut SourceMap) -> Vec<Diagnostic>`
  — for each struct not in `released` (by `(package, name)`), compute
  `struct_head` size for the declared order and for the fields sorted by
  descending alignment (stable); report when the second is smaller, with the
  message naming both sizes and the saving order.
- Consumes: `projection::podbuf::{struct_head, place}`, the baseline reader of
  `desk_check` (`load_ir_json` over `.ridl/baseline/*.ir.json`).

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn fault_event_draws_ridl_154_with_the_saving_order_before_its_first_release() {
    let out = ridl_check(workspace_with_podbuf("fault_event.typl"), None);
    assert_eq!(codes(&out), vec!["RIDL-154"]);
    assert!(out.message(0).contains("24 bytes") && out.message(0).contains("16 bytes"));
    assert!(out.message(0).contains("timestamp, code, message"));
}

#[test]
fn a_struct_declared_in_the_baseline_draws_nothing() {
    let ws = workspace_with_podbuf("fault_event.typl");
    ridl_baseline(&ws);
    assert!(codes(&ridl_check(ws, None)).is_empty());
}

#[test]
fn the_lint_is_silent_when_no_backend_names_podbuf_and_is_info_by_default() {
    assert!(codes(&ridl_check(workspace_without_podbuf("fault_event.typl"), None)).is_empty());
    assert_eq!(default_level("podbuf-padding-order"), Some(LintLevel::Info));
}
```

- [ ] **Step 2: Run them**

`cargo test -p ridl lints` — expected: compile error, no RIDL-154.

- [ ] **Step 3: Implement**

The row, the check, the wiring; update the `ridl-core` lint-name list test and
the book page in the same change (ADR-0024 decision 7).

- [ ] **Step 4: Run them**

`cargo test -p ridl -p ridl-core` — expected: pass, `book_lints.rs` included.

- [ ] **Step 5: Commit**

`feat(ridl): lint a podbuf field order that wastes padding`

---

## Task 13: Open enums and enum sets in podbuf — after the prerequisite

§6, §7 rule 2 and §8.5 with `open` available: a strict enum rejects an
undeclared value and a strict enum set an undeclared bit (Task 8 already does
this for every enum); an open enum accepts any value of its width and the view
and `decode` return the unknown variant; an open enum set accepts and keeps
every bit; the C header writes the constants and a comment stating strict or
open.

**Files:**

- Modify: `crates/ridl-backend-rust/src/codec_podbuf.rs`,
  `crates/ridl-backend-podbuf/src/lib.rs`,
  `crates/ridl-ir/src/projection/podbuf.rs` (`Form::Enum { open }`,
  `Form::EnumSet { open }`), `crates/ridl-ir/proto/ridl/codegen/v1/model.proto`
  (`PodbufEnum.open`, `PodbufEnumSet.open`),
  `crates/ridl-backend-rust/tests/fixtures/podbuf_roundtrip.ridl`
- Test: `crates/ridl-backend-rust/tests/podbuf_verify.rs`,
  `crates/ridl-backend-podbuf/src/tests.rs`

**Interfaces:**

- Consumes: `v2::EnumDef.open`, `v2::EnumSetDef.open`, the domain enum's unknown
  variant (prerequisite).

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn an_open_enum_accepts_an_undeclared_value_and_reads_it_as_unknown() {
    let v = verify(patch(encode(dtc_status(1)), 8, 7));     // `open enum DtcStatus`, 7 undeclared
    assert!(matches!(v.status(), DtcStatus::Unknown(7)));
    assert!(matches!(decode(v).status, DtcStatus::Unknown(7)));
}

#[test]
fn an_open_enumset_keeps_an_undeclared_bit() {
    let v = verify(patch(encode(open_flags(0b01)), 8, 0b1000_0001));
    assert_eq!(v.flags().raw(), 0b1000_0001);
}

#[test]
fn a_strict_enum_still_refuses_an_undeclared_value() {
    assert_eq!(refusal(patch(encode(gear(1)), 8, 7)), "Contract");
}

// crates/ridl-backend-podbuf/src/tests.rs
#[test]
fn the_header_states_whether_an_enum_is_strict_or_open() {
    let h = generate_fixture("open_enum.typl").header_source;
    assert!(h.contains("/* open enum: a value outside the constants is valid */"));
    assert!(h.contains("#define VEH_COMMON_DTC_STATUS_ACTIVE 1"));
}
```

- [ ] **Step 2: Run them**

`cargo test -p ridl-backend-rust --test podbuf_verify -p ridl-backend-podbuf` —
expected: fail, the verify rejects 7 and the header has no comment.

- [ ] **Step 3: Implement**

Read `open` from the model; the verify's enum check branches on it; the view
maps a raw value onto the declared variant or the unknown one.

- [ ] **Step 4: Run them**

`cargo test -p ridl-backend-rust -p ridl-backend-podbuf -p ridl-ir` — expected:
pass.

- [ ] **Step 5: Commit**

`feat(ridl-backend-rust): accept unknown values of an open enum in podbuf`

---

## Task 14: The `ridl diff` rule for an open enum that changes width — after the prerequisite

§6's last rule and §8.2: a value added to an open enum or enum set that changes
the inferred width or signedness is a podbuf break; one that fits is compatible.
(Strict-to-open and the reverse are classified by the prerequisite.)

**Files:**

- Modify: `crates/ridl-diff/src/lib.rs` (`Category::PodbufEnumWidened`),
  `crates/ridl-diff/src/classify.rs`, `crates/ridl-diff/src/tests.rs`,
  `docs/book/evolving-an-interface.md`
- Test: `crates/ridl-diff/src/tests.rs`

**Interfaces:**

- Consumes: `projection::podbuf::enum_width`, `v2::EnumDef.open`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_value_that_fits_the_width_of_an_open_enum_is_compatible() {
    assert_eq!(diff_with_encodings(OPEN_V1, OPEN_V2_FITS, &["podbuf"]).verdict, Verdict::Compatible);  // 0..3 then 4
}

#[test]
fn a_value_that_crosses_the_width_or_signedness_is_a_podbuf_break() {
    assert!(has(diff_with_encodings(OPEN_V1, OPEN_V2_300, &["podbuf"]), Category::PodbufEnumWidened));   // 300 needs u16
    assert!(has(diff_with_encodings(OPEN_V1, OPEN_V2_NEG, &["podbuf"]), Category::PodbufEnumWidened));   // -1 makes it signed
}
```

- [ ] **Step 2: Run them**

`cargo test -p ridl-diff open_enum` — expected: compile error on the category.

- [ ] **Step 3: Implement**

Compare `enum_width` of both sides when the new side is open and a value was
added.

- [ ] **Step 4: Run them**

`cargo test -p ridl-diff -p ridl` — expected: pass.

- [ ] **Step 5: Commit**

`feat(ridl-diff): report an open enum value that changes the podbuf width`

---

## Task 15: Cross-target layout checks and the end-to-end demo

The targets the design names (§4, §16) that CI can check from an x86-64 runner:
`cargo check` of the generated crate for `i686-unknown-linux-gnu`,
`aarch64-unknown-linux-gnu` and `thumbv7em-none-eabihf` evaluates every `const`
layout assertion (no linker is needed); the C header's `_Static_assert`s are
Task 5's clang test. The cabin example gains podbuf so `just demo` round-trips
one value through it.

**Files:**

- Modify: `rust-toolchain.toml` (targets `i686-unknown-linux-gnu`,
  `aarch64-unknown-linux-gnu`), `justfile` (`demo`: two `cargo check` lines over
  `ridl_generated` and `veh_cabin` for the two new targets, beside the
  `thumbv7em-none-eabihf` ones; `rustup target add` for each),
  `examples/cabin/ridl.toml`
  (`encodings = ["flatbuffers", "proto3", "podbuf"]`),
  `examples/cabin/consumer/src/main.rs` (one podbuf round trip whose value the
  `demo` recipe matches),
  `xtask/tests/freeze-fixtures/generated/{cabin,veh-cluster}/ridl.toml` and
  `freeze/generated/*` (refreshed on purpose: `cargo xtask freeze update`; lines
  are added), `docs/book/running-a-program.md` (the encodings line)
- Test: `crates/ridlc/tests/cabin_example.rs`,
  `crates/ridl/tests/gate_parity.rs` (unchanged members; `demo` is already a
  member)

**Interfaces:**

- Consumes: Task 9's generated crate.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/ridlc/tests/cabin_example.rs
#[test]
fn the_cabin_crate_carries_a_podbuf_codec_and_size_table() {
    let src = build_cabin();
    assert!(src.contains("impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::Podbuf> for CabinTemperature"));
    assert!(src.contains("impl ::ridl_rt::contract::Sizes<::ridl_rt::encoding::Podbuf> for Cabin"));
}
```

And the recipe's own check: `just demo` fails unless its output holds the line
`podbuf round trip: 21.5` (the consumer's new line).

- [ ] **Step 2: Run them**

`cargo test -p ridlc cabin` — expected: fail, cabin names no podbuf.

- [ ] **Step 3: Implement**

The manifests, the consumer line, the recipe lines, the fixture refresh.

- [ ] **Step 4: Run them**

`cargo test -p ridlc cabin`, then `just demo` once locally — expected: pass; the
three `cargo check --target` lines of the generated crates succeed.

- [ ] **Step 5: Commit**

`build(repo): check the podbuf layout on i686, aarch64 and thumbv7em`

---

## Task 16: The projection record, the design record and the freeze rows

ADR-0020 decision 4 asks for a projection record written with the backend. The
design document becomes ADR-0031; the as-built codec gets a design record like
`flatbuffers-codec.md`; the design moves to `docs/archive/`; the four freeze
rows deleted in Task 1 are added back with the merge commit of Task 1's pull
request as `baseline`. This task is two pull requests: the records, then the
rows (the rows need a merge commit that already holds the changed dumps).

**Files:**

- Create: `docs/decisions/ADR-0031-podbuf-projection-rules.md` (Status Accepted;
  decisions numbered from §4 to §9 and §14 of the design: the target rule, the
  preamble, the first-fit law and flags, the descriptors and stride, the value
  rules, the totality table, verify's list and what it does not check, the
  encoder duty, the evolution rules, strict and open enums, the settled items;
  the Alternatives table of §11; the codes RIDL-151 to RIDL-154 and the
  generated names of Global Constraints; Trace), `docs/design/podbuf-codec.md`
  (where the code is, what is emitted, the writer, the reader, conformance,
  known gaps), `docs/archive/2026-10-24-podbuf-design.md` and
  `docs/archive/2026-10-24-podbuf-plan.md` (moved)
- Modify: `docs/decisions/README.md` (ADR-0031 entry),
  `docs/decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md`
  (decision 4 and open item 1 point at ADR-0031; open item 1 is closed: `Inline`
  stays FlatBuffers', podbuf is the in-place encoding),
  `docs/specification/ridl-family-overview.md` (ledger row and the projection
  records list), `docs/design/README.md`,
  `docs/technotes/walking-skeleton-architecture.md`, `docs/book/stability.md`
  (the `podbuf` feature and the new wire lines), `docs/wip/README.md`
  (dispositions), `freeze/surfaces.toml` (second pull request: the four rows)
- Test: `just link-check`, `just doc-path-check`, `just story-id-check`,
  `just book-check` run in CI; `cargo test -p ridl --test book_lints`

- [ ] **Step 1: Check the citations before moving the design**

`grep -rn "2026-10-24-podbuf-design" --include=*.rs --include=*.md --include=*.toml .`
— every hit outside `docs/wip/` and `docs/archive/` is rewritten to ADR-0031
(the archived path is a dead citation otherwise).

- [ ] **Step 2: Write the records**

ADR-0031 restates the design as decisions, each marked as the design marks it;
`podbuf-codec.md` describes the code as built, naming files and functions; the
book's Stability page lists the `podbuf` feature among the frozen feature names.

- [ ] **Step 3: Run the checks**

`prim fmt docs && prim lint docs`, `just link-check`, `just doc-path-check`,
`just story-id-check` — expected: pass. `cargo test -p ridl --test book_lints` —
expected: pass.

- [ ] **Step 4: Commit the records**

`docs(adr): record the podbuf projection rules as ADR-0031` — body: the design
is archived, ADR-0020 decision 4 and open item 1 point at it.

- [ ] **Step 5: After that pull request merges, add the rows back**

In `freeze/surfaces.toml`, add `runtime-crates`, `wire`, `codegen-protocol` and
`cli` with `baseline = "<merge commit of Task 1's pull request>"`;
`just freeze-check` passes with no notice. Commit:
`build(repo): add the frozen rows back after the podbuf rename`.

---

## Self-review

- **Coverage against the design.** §2 name and feature: Task 1. §3 trust model
  and 8-byte alignment: Tasks 6 and 9 (question 5). §4 target rule, alignment 8,
  assertions, big-endian refusal: Tasks 5, 6, 7, 15. §5.1 preamble, descriptors,
  stride, `elem_size` rule: Tasks 2, 6, 7, 8. §5.2 first-fit and flags: Task 2.
  §5.3 worked examples: Tasks 2, 7, 10. §5.4 preamble: Tasks 6, 7, 8. §6 value
  rules, map order, NUL: Tasks 2, 7, 8. §6.1 totality and limits: Tasks 2, 3. §7
  verify list, not-checked list, encoder duty: Tasks 6, 7, 8, 10. §8.1
  compatible changes: Tasks 10, 11. §8.2 breaks: Tasks 11, 14 (the prerequisite
  owns strict and open flips). §8.4 checks: Tasks 11, 12. §8.5 open enums: Task
  13. §9 floats: Task 8 (`a_float_with_no_range_accepts_nan`). §10 artifacts and
  exposure: Tasks 4, 5, 7, 8. §14 settled items: Tasks 6 to 10. §16 builder
  items: Global Constraints, Tasks 3, 4, 12, 16.
- **Step scan.** Every task has a failing test, a run with its expected failure,
  an implementation step with signatures, a passing run and a commit with a
  scope in `.git-std.toml` (`ridl-backend-podbuf` is added by Task 5).
- **Type consistency.** `Head`, `Placement`, `Form`, `DescriptorKind` (Task 2)
  feed `PodbufProjection` (Task 4), `codec_podbuf` (Tasks 7, 8, 9), the C header
  (Task 5), the diff (Tasks 11, 14) and the lint (Task 12);
  `ridl_rt::podbuf::{Preamble, Region, Desc, ArrayDesc, Writer, Aligned}` (Task
  6) are what the generated code of Tasks 7 to 9 calls.
- **Review Focus.** Each of the five has its test in the task named.
- **Proportion.** One body is written out, `place` (Task 2); every other step
  gives signatures, test names and assertions.
