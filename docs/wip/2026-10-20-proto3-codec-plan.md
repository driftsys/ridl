# The proto3 payload codec — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** A package whose `[backend.rust] encodings` names `proto3` carries a
generated `Payload<Proto3>` for every type that can be a payload and a
`Sizes<Proto3>` per interface, byte-conformant with `prost` code generated
through `protox` from the `.proto` this toolchain emits.

**Architecture:** `ridl-rt` gains a dependency-free `proto3` module (varint,
tag, forward writer, reader); `ridl-ir` gains one shared width-to-scalar table
and a per-declaration proto3 bound in the codegen `Model`; `ridl-backend-rust`
gains a second codec emitter, `codec_proto3.rs`, beside `codec.rs`, selected by
`WireEncoding::Proto3`. The oracle is checked-in `prost` code generated from the
emitted `.proto`, driven by a bare-`rustc` program built from the generated
codec.

**Tech Stack:** Rust (generated code: 1.83, edition 2021, `no_std`), `quote`,
`prost` 0.14.4, `prost-build` 0.14.4, `protox` 0.9.1, `prost-reflect` 0.16.5,
`proptest` 1.11.0 (all test-only for this crate).

**Spec:** `docs/wip/2026-10-20-proto3-codec-design.md` (decisions D-1 to D-21).
Read it first; every task cites the decisions it implements, and the decision
text is binding where this plan is silent.

## Global Constraints

- Emitted code: Rust 1.83, edition 2021, no `std`, no `alloc` except where a
  domain type owns a `Vec` or a `String` (written `::std::…`, which the
  generated `lib.rs` aliases to `alloc`); every primitive and prelude type by
  its `::core::` or `::std::` path.
- `ridl-rt`: no dependency in any feature combination,
  `#![forbid(unsafe_code)]`, `rust-version = "1.83"`, edition 2021.
- `prost`, `prost-build`, `prost-reflect`, `protox`, `ridl-backend-proto`: dev-
  dependencies of `ridl-backend-rust` only. Never in `ridl-rt` or a generated
  crate.
- No change to the `.proto` text `ridl-backend-proto` emits (`freeze/wire.txt`
  must not change).
- No change to the FlatBuffers codec's emitted text: `freeze/generated/*` must
  regenerate byte-identical, and every existing snapshot and checked-in
  generated file stays unchanged unless a task says otherwise.
- Shipped docs, rustdoc and comments name no story id or plan name
  (`just story-id-check`); plain literal English.
- Commits: Conventional Commits with scopes from `.git-std.toml`, each ending
  with the two trailer lines the driver gives.
- Implementers run the focused tests named in their task; the driver runs the
  full gates before the pull request.

## Review Focus

1. A struct whose field ordinals are not in declaration order (`b = 2` declared
   before `a = 1`): the writer must emit field-number order, or byte equality
   with `prost` breaks — Task 3 adds `fields_are_written_in_number_order`.
2. A map whose domain `Vec` holds a key twice: `verify` must answer
   `Contract(Unique)` and `encode` must not write a payload `verify` refuses —
   Task 5 adds `a_map_with_a_repeated_key_is_refused_on_both_sides`.
3. A `u8`-width field receiving 300 on the wire: `Contract(Range)`, not a
   truncation and not a panic — Task 6 adds
   `a_value_wider_than_its_width_is_a_range_violation`.
4. A payload from a newer writer with an unknown length-delimited field longer
   than this version's `MAX_SIZE` minus the known fields: accepted (D-8) — Task
   6 adds `a_newer_writers_payload_above_max_size_verifies`.
5. A packed array whose run ends in the middle of a varint: `OutOfBounds`, never
   a partial array — Task 6 adds
   `a_packed_run_cut_inside_a_varint_is_out_of_bounds`.

---

## File structure

| File                                                                                      | Responsibility                                                 |
| ----------------------------------------------------------------------------------------- | -------------------------------------------------------------- |
| `crates/ridl-rt/src/proto3.rs` (new)                                                      | varint, zigzag, tag, `Writer`, `Reader`, `Verified`            |
| `crates/ridl-rt/src/payload.rs`                                                           | three `Malformed` variants                                     |
| `crates/ridl-rt/src/lib.rs`, `Cargo.toml` feature docs                                    | gate the module on `proto3`                                    |
| `crates/ridl-ir/src/projection/proto3.rs`                                                 | the width table over the `Model`'s widths, `Scalar::wire_type` |
| `crates/ridl-ir/proto/ridl/codegen/v1/model.proto`, `codegen/lower.rs`                    | `Declaration.proto3`                                           |
| `crates/ridl-backend-rust/src/codec_proto3.rs` (new)                                      | the proto3 codec emitter                                       |
| `crates/ridl-backend-rust/src/lib.rs`, `contract.rs`, `descriptors.rs`                    | `WireEncoding::Proto3`, per-encoding size tables               |
| `crates/ridl-backend-rust/tests/fixtures/proto3_conformance*.ridl` (new)                  | the conformance fixture and its second package                 |
| `crates/ridl-backend-rust/tests/generated/proto3_conformance_prost.rs` (new)              | the checked-in oracle                                          |
| `crates/ridl-backend-rust/tests/proto3_conformance.rs` (new)                              | the conformance suite                                          |
| `crates/ridl-backend-rust/tests/proto3_roundtrip.rs` (new)                                | self round trips, `MAX_SIZE`, wasm32                           |
| `crates/ridl-backend-rust/tests/support/proto3.rs` (new)                                  | the program harness (hex in, hex out) and the oracle loader    |
| `xtask/tests/oracle_boundary.rs`                                                          | the prost guard                                                |
| `examples/cabin/ridl.toml`, `crates/ridlc/tests/corpus/veh-cluster/ridl.toml`, `justfile` | demo and compat-check reach the codec                          |
| `docs/design/proto3-codec.md` (new) and the records of D-21                               | the as-built record                                            |

---

### Task 1: `ridl-rt`'s `proto3` module and the three `Malformed` variants

Implements D-9 (variants), D-11 (`Verified`), D-14 (writer shape), D-15.

**Files:**

- Create: `crates/ridl-rt/src/proto3.rs`
- Modify: `crates/ridl-rt/src/lib.rs` (module gate and the crate doc's feature
  list, which says `proto3` enables nothing), `crates/ridl-rt/src/payload.rs`
  (`Malformed`), `crates/ridl-rt/Cargo.toml` (feature comment only)
- Test: unit tests in `crates/ridl-rt/src/proto3.rs`;
  `crates/ridl-rt/src/payload.rs` tests

**Interfaces — Produces** (all under `#[cfg(feature = "proto3")]`,
`pub mod proto3`):

- `pub enum WireType { Varint = 0, I64 = 1, Len = 2, I32 = 5 }`
- `pub const fn varint_len(v: u64) -> usize`
- `pub const fn zigzag32(v: i32) -> u32`, `zigzag64(v: i64) -> u64`,
  `unzigzag32(v: u32) -> i32`, `unzigzag64(v: u64) -> i64`
- `pub const fn tag_len(number: u32) -> usize`
- `pub struct Writer<'a>`: `new(out: &'a mut [u8]) -> Self`,
  `tag(&mut self, number: u32, wire: WireType)`, `varint(&mut self, v: u64)`,
  `fixed32(&mut self, v: u32)`, `fixed64(&mut self, v: u64)`,
  `bytes(&mut self, b: &[u8])`, `finish(self)`. The writer is used only after
  `encode` has checked capacity against the exact length (D-14), so its methods
  return nothing and have no panic path: a write that would pass the end writes
  what fits and sets an overflow flag, and `finish` returns
  `Result<&'a [u8], EncodeError>` — `Capacity` when the flag is set, which only
  a length function that undercounts can cause.
- `pub fn check_capacity(needed: usize, out: &[u8]) -> Result<(), EncodeError>`
  returning `Capacity { needed, available }`.
- `pub struct Reader<'a>`: `new(buf: &'a [u8]) -> Self`,
  `next(&mut self) -> Option<Result<Field<'a>, Malformed>>` where
  `pub struct Field<'a> { pub number: u32, pub value: Value<'a> }` and
  `pub enum Value<'a> { Varint(u64), I64(u64), Len(&'a [u8]), I32(u32) }`.
  `next` refuses field number 0 and wire types 3, 4, 6, 7 with
  `Malformed::WireType`, an over-long varint with `Malformed::Varint`, and a
  truncation with `Malformed::OutOfBounds`.
- `pub fn read_varint(buf: &[u8], at: &mut usize) -> Result<u64, Malformed>`
  (for packed runs)
- `pub fn narrow_u32(v: u64) -> Result<u32, Malformed>`,
  `narrow_i32(v: u64) -> Result<i32, Malformed>` (an enum's int32, sign-extended
  ten-byte form accepted),
  `unzigzag32_checked(v: u64) -> Result<i32, Malformed>` — each
  `Malformed::Varint` when the value does not fit
- `pub fn utf8(b: &[u8]) -> Result<&str, Malformed>`
- `pub struct Verified<'a> { bytes: &'a [u8] }` with `pub fn new(bytes) -> Self`
  and `impl<'a> View<'a> for Verified<'a>`
- `Malformed::Varint`, `Malformed::WireType`, `Malformed::Duplicate`, each with
  a doc line from the D-9 table

- [ ] **Step 1: Write the failing tests** in `proto3.rs`'s `mod tests`:
      `varint_round_trips_at_every_length` (0, 1, 127, 128, 16383, 16384,
      `u32::MAX`, `u64::MAX` — lengths 1,1,1,2,2,3,5,10);
      `zigzag_matches_the_protobuf_table` (0→0, -1→1, 1→2, -2→3,
      `i32::MIN`→`u32::MAX`); `an_eleven_byte_varint_is_refused`
      (`[0x80;10] ++ [0x00]` → `Malformed::Varint`);
      `a_tenth_byte_above_one_is_refused` (`[0xff;9] ++ [0x02]`);
      `a_non_minimal_varint_reads` (`[0x80, 0x00]` → 0);
      `the_reader_refuses_field_zero_and_groups_and_wire_types_six_and_seven`
      (tags `0x00`, `0x0b`, `0x0c`, `0x0e`, `0x0f` → `WireType`);
      `the_reader_reports_a_truncated_length_as_out_of_bounds`
      (`0x12 0x05 0x61`); `the_reader_yields_each_wire_type` (one field of each
      of 0, 1, 2, 5); `narrowing_refuses_what_does_not_fit`
      (`narrow_u32(1 << 32)`, `unzigzag32_checked(u64::from(u32::MAX) + 1)`,
      `narrow_i32` of the ten-byte form of -3 is `Ok(-3)`, of `1 << 40` is
      `Err`); `the_writer_writes_a_tag_and_a_packed_run` (field 15, `[1, 2]` →
      `7a 02 01 02`, the probe's bytes); a payload-tests case
      `malformed_has_the_three_proto3_variants` constructing each.
- [ ] **Step 2: Run** `cargo test -p ridl-rt --all-features proto3` — FAIL
      (module missing).
- [ ] **Step 3: Implement** the module and the variants. Varint decode is the
      loop over at most ten bytes; the tenth byte may carry only bit 0.
- [ ] **Step 4: Run** `cargo test -p ridl-rt --all-features` and
      `cargo test -p ridl-rt` (default features: the module is absent and
      nothing else changed) and
      `cargo check -p ridl-rt --all-features --target thumbv7em-none-eabihf` and
      `cargo +1.83 check -p ridl-rt --all-features` if 1.83 is installed (else
      note it for the driver's compat-check) — PASS.
- [ ] **Step 5: Commit** `feat(ridl-rt): add the proto3 wire helpers`.

### Task 2: the shared width table and `Declaration.proto3` in the codegen `Model`

Implements D-3 (table) and D-4 (the `Model` field).

**Files:**

- Modify: `crates/ridl-ir/src/projection/proto3.rs`,
  `crates/ridl-ir/proto/ridl/codegen/v1/model.proto` (field 24 on `Declaration`
  with a comment stating the rule), `crates/ridl-ir/src/codegen/lower.rs`,
  `freeze/codegen-protocol.txt` (regenerated)
- Test: `crates/ridl-ir/src/projection/proto3.rs` tests,
  `crates/ridl-ir/src/codegen/tests.rs`

**Interfaces — Produces:**

- `pub fn int_width_scalar(width: v1::IntWidth) -> Option<Scalar>` and
  `pub fn float_width_scalar(width: v1::FloatWidth) -> Option<Scalar>` — `None`
  for `Unspecified` (the codec refuses it, D-2); `scalar(&v2::TypeDef)` calls
  the same match arms so there is one table.
- `pub enum WireType { Varint, I64, Len, I32 }` and `Scalar::wire_type(self)`;
  `Scalar::packable(self) -> bool` (every scalar but `String` and `Bytes`).
- `v1::Declaration.proto3: Option<v1::SizeState>`, `None` on a constant.

- [ ] **Step 1: Failing tests:** `the_model_width_table_is_the_ir_width_table`
      (for every `IntWidth` and `FloatWidth` value but `Unspecified`, the scalar
      of the `Model` width equals `scalar()` of a `TypeDef` with the IR width);
      `every_scalar_has_its_wire_type` (bool/ints varint, float I32, double I64,
      string/bytes Len); in `codegen/tests.rs`,
      `a_declaration_carries_its_proto3_root_bound` — over a fixture with a
      struct, a union, a named scalar, an enum, an enum set and a constant: each
      kind's `proto3` equals
      `size_state(home, &PayloadShape::Named(name), &ctx,
  Encoding::Proto3)`
      lowered to `v1::SizeState`, and the constant's is `None`;
      `a_declaration_with_an_optional_array_has_no_proto3_bound` (absent,
      `RefusedMember`).
- [ ] **Step 2: Run** `cargo test -p ridl-ir proto3` — FAIL.
- [ ] **Step 3: Implement.** Reuse the existing lowering of a `SizeState` into
      `v1::SizeState` that payload sizes already use in `lower.rs`.
- [ ] **Step 4: Run** `cargo test -p ridl-ir`, then `cargo xtask freeze update`
      and `git diff --stat freeze/` — only `freeze/codegen-protocol.txt`
      changes, and only by added lines (`git diff freeze/ | grep '^-[^-]'`
      prints nothing). Then
      `cargo test -p ridl-backend-rust -p ridl-backend-proto -p ridlc` — PASS
      (no other output changes).
- [ ] **Step 5: Commit**
      `feat(ridl-ir): carry each declaration's proto3 bound in the codegen model`.

### Task 3: the codec emitter for scalars, enums, enum sets, boxes and structs

Implements D-1, D-2 (the rows for these kinds), D-4 (refusal, `MAX_SIZE`), D-5,
D-6, D-13 (checks at these positions), D-14, D-16. Self round trips only; the
oracle arrives in Task 4.

**Files:**

- Create: `crates/ridl-backend-rust/src/codec_proto3.rs`,
  `crates/ridl-backend-rust/tests/proto3_roundtrip.rs`,
  `crates/ridl-backend-rust/tests/fixtures/proto3_roundtrip.ridl`
- Modify: `crates/ridl-backend-rust/src/lib.rs` (`WireEncoding::Proto3`,
  `parse`, `name`, `check_emitted`, `package_items` calls
  `codec_proto3::package_items` when the list holds `Proto3`; FlatBuffers codec
  only when it holds `FlatBuffers`), `src/contract.rs` (`parse_encoding` accepts
  `proto3`; the `repr-c` refusal stays), `src/descriptors.rs` (until Task 8 a
  package naming `proto3` without `flatbuffers` emits no `Sizes` table — see
  Step 3)
- Test: `tests/proto3_roundtrip.rs`, `src/tests.rs` (the contract option)

**Interfaces — Produces:**

- `pub(crate) fn package_items(ctx: &Ctx) -> Result<Vec<TokenStream>, GenerateError>`
  in `codec_proto3.rs`
- Emitted per declaration `T`: `__ridl_pb_len_T(&T) -> usize`,
  `__ridl_pb_encode_T(&T, &mut ::ridl_rt::proto3::Writer<'_>)`,
  `__ridl_pb_verify_T(&[u8]) -> Result<(), VerifyError>`,
  `__ridl_pb_decode_T(&[u8]) -> T`, and
  `impl Payload<::ridl_rt::encoding::Proto3> for T` with
  `type View<'a> = ::ridl_rt::proto3::Verified<'a>`.
- `pub(crate) fn layout(ctx: &Ctx) -> Vec<MessageLayout>` with
  `pub(crate) struct MessageLayout { name: String, fields: Vec<(u32, String, Label)> }`
  (`Label::{Implicit, Optional, Repeated}`), exposed to tests through a
  `#[doc(hidden)] pub fn proto3_layout(package: &v2::Package, others: &[&v2::Package])`
  in `lib.rs` — the drift test of Task 4 reads it.

- [ ] **Step 1: Failing tests** in `tests/proto3_roundtrip.rs`, generating with
      `generate_pipeline(&package, &[WireEncoding::Proto3], &[])` and running a
      program (`support::rustc::run_program_capturing_stdout`) that encodes,
      prints hex, verifies, decodes and asserts equality:
      `a_struct_of_every_scalar_round_trips` (u8, u16, u32, u64, i8, i16, i32,
      i64, bare `integer`, f32, f64, bare `float`, bool, bounded string with a
      multi-byte character, bounded bytes, a named scalar, an enum with a zero
      member, an enum without one, an enum with a negative value, an enum set);
      `optional_fields_round_trip_present_at_zero_and_absent`;
      `a_nested_struct_round_trips`; `each_box_root_round_trips` (named scalar,
      enum, enum set); `fields_are_written_in_number_order` (fixture struct
      declares ordinal 2 before 1; the hex starts with tag `08`);
      `a_field_at_its_default_is_not_written` (all-zero struct whose types admit
      0 encodes to the empty buffer; a box of 0 is empty);
      `minus_zero_is_written` (an f64 field at `-0.0` encodes to
      `11 00 00 00 00 00 00 00 80`); `encode_reports_the_exact_capacity_needed`
      (a buffer one byte short gives
      `Capacity { needed: len, available: len - 1 }`);
      `the_encoded_bytes_are_a_prefix_of_the_buffer`;
      `max_size_is_the_shared_bound` (each emitted `MAX_SIZE` equals the
      `Declaration.proto3` bound of Task 2);
      `a_declaration_without_a_proto3_bound_is_refused` (optional array field →
      `GenerateError` naming `pkg.T.field`);
      `the_generated_proto3_codec_checks_for_wasm32`; in `src/tests.rs`,
      `the_contract_accepts_proto3` (`encodings=proto3` and
      `encodings=flatbuffers,proto3` build; `repr-c` still names #317).
- [ ] **Step 2: Run** `cargo test -p ridl-backend-rust --test proto3_roundtrip`
      — FAIL.
- [ ] **Step 3: Implement.** Classify each field type into the proto3 wire shape
      from the `Model` (`int_width_scalar`/`float_width_scalar` for widths,
      `ridl_ir::projection::proto3::primitive` for bare primitives); sort fields
      by ordinal at generation time; legality of absence from D-12
      (`zero_is_legal` shared with `codec.rs` — move it to a common place rather
      than copying it); reuse `codec.rs`'s named-scalar `check` and
      inline-constraint check helpers through `pub(crate)` rather than
      duplicating them. A field with absent legality false whose value can never
      be the default is always written (it is never the default).
      `descriptors.rs`: keep emitting the FlatBuffers table only when the list
      holds `FlatBuffers`; when it holds only `Proto3`, emit no table and no
      face for now — `generate_face_with(&p, &[Proto3])` returns the types and
      the proto3 codec; Task 8 adds the table and the face.
- [ ] **Step 4: Run** the task's tests, then `cargo test -p ridl-backend-rust`
      (every FlatBuffers test and snapshot unchanged) and
      `cargo xtask freeze check` — PASS.
- [ ] **Step 5: Commit**
      `feat(ridl-backend-rust): emit the proto3 codec for scalars, enums and structs`.

### Task 4: the `prost` oracle, the drift test and the runtime guard

Implements D-17's oracle, cases 1, 2 and 10 for Task 3's shapes, and the
`ridl-rt` half of D-18.

**Files:**

- Create: `crates/ridl-backend-rust/tests/fixtures/proto3_conformance.ridl` and
  `proto3_conformance_other.ridl` (the full shape inventory of D-17 case 4, plus
  the cross-package reference), `tests/generated/proto3_conformance_prost.rs`,
  `tests/proto3_conformance.rs`, `tests/support/proto3.rs`
- Modify: `crates/ridl-backend-rust/Cargo.toml` (dev-dependencies with a comment
  block like the planus one), `xtask/tests/oracle_boundary.rs`
- Test: those files

**Interfaces — Produces:**

- `support::proto3::fixture() -> (v2::Package, Vec<v2::Package>)`
- `support::proto3::emitted_proto() -> Vec<(String, String)>` (file name, text)
  for both packages, from `ridl_backend_proto::generate_with`
- `support::proto3::descriptor_pool() -> prost_reflect::DescriptorPool` via
  `protox::compile`
- `support::proto3::Program` — builds once per test binary (a `OnceLock`) the
  generated codec plus a `main` that reads lines `<Type> <op> <hex>` from stdin
  (`op` is `decode-encode`, `verify`, `encode-default`) and answers one line
  each: the re-encoded hex, `ok`, or the `VerifyError` `Debug` text.
  `Program::run(&self, lines: &[String]) -> Vec<String>`.
- `mod prost_oracle { include!("generated/proto3_conformance_prost.rs"); }`

- [ ] **Step 1: Failing tests:**
      `the_checked_in_prost_code_is_what_prost_build_writes` (regenerates with
      `protox::compile` +
      `prost_build::Config::new()
  .btree_map(["."]).out_dir(tmp).compile_fds(..)`
      and compares bytes; `RIDL_UPDATE_GENERATED=1` rewrites);
      `bytes_this_codec_writes_equal_prost_bytes` and `prost_bytes_decode_here`
      over a value table for the Task 3 shapes (every scalar at 0, 1, min, max;
      NaN; a negative enum value; optional at 0 and absent; nested) — equality
      of bytes both ways, and `prost` decoding the program's bytes to the
      table's `prost` value; `minus_zero_differs_from_prost_as_designed`
      (program writes `11 00…80`, `prost` writes nothing, and `prost` decodes
      the program's bytes to `-0.0` with the sign bit set);
      `proto3_layout_matches_the_emitted_schema` (for the fixture and every
      corpus package — `examples/cabin`,
      `crates/ridlc/tests/corpus/veh-cluster`, `evals/corpus/*`, each compiled
      with `ridlc` — every `MessageLayout` from `proto3_layout` equals the
      protox descriptor's field numbers, scalar types and labels; a message the
      codec does not cover yet is skipped with its name listed in an
      `expected_uncovered` array that Task 5 empties); in `oracle_boundary.rs`,
      `the_runtime_reaches_no_prost_crate` and the four new `BOUNDARIES` rows
      (`ridl-backend-rust` → `ridl-backend-proto`, `protox`, `prost-build`,
      `prost-reflect`).
- [ ] **Step 2: Run**
      `cargo test -p ridl-backend-rust --test proto3_conformance` and
      `cargo test -p xtask --test oracle_boundary` — FAIL.
- [ ] **Step 3: Implement** the support module and generate the oracle file.
- [ ] **Step 4: Run** both again — PASS. Mutation check for the reviewer to
      repeat: swap the ordinal of one field in `codec_proto3.rs` → both equality
      tests and the drift test fail.
- [ ] **Step 5: Commit**
      `test(ridl-backend-rust): check the proto3 codec against prost`.

### Task 5: arrays, maps, tuples, unions, cross-package references and the induced interaction roots

Implements the remaining D-2 rows, D-1's induced interaction tuple roots, D-5's
packed and map rules, D-13's count and uniqueness checks.

**Files:**

- Modify: `crates/ridl-backend-rust/src/codec_proto3.rs`,
  `tests/proto3_conformance.rs`, `tests/proto3_roundtrip.rs`
- Test: those

- [ ] **Step 1: Failing tests:** extend the value table of Task 4 to every shape
      of the inventory (packed arrays of each numeric, bool and enum type;
      arrays of string, bytes, struct and union; maps for each admitted key kind
      with scalar, enum, string and struct values, entries in key order, the
      induced argument message of a call of two parameters, of a query of no
      parameter, and the induced reply message of a tuple reply — each with
      `MAX_SIZE` equal to the interaction's `request_sizes.proto3` or
      `reply_sizes.proto3` bound, asserted by
      `an_induced_interaction_root_takes_the_interactions_bound`; including a
      `{"": 0}` entry whose bytes are `<tag> 00`; a tuple; a union with a scalar
      arm at 0, a string arm at `""`, an enum arm, an enum-set arm and a struct
      arm; the cross-package reference; each `?` kind) with byte equality both
      ways; `the_inventory_reaches_every_shape` (walks the fixture's `Model` and
      asserts each D-2 row occurs at least once);
      `an_empty_array_is_not_written`;
      `a_map_with_a_repeated_key_is_refused_on_both_sides` (`encode` →
      `EncodeError::Contract(Unique)`; a hand-built buffer with the key twice →
      `VerifyError::Contract(Unique)`);
      `an_optional_array_element_is_refused_at_generation` and the same for an
      optional map value; `a_collection_over_its_count_is_refused` (encode and
      verify, `Rule::Length`); `expected_uncovered` in the drift test is now
      empty.
- [ ] **Step 2: Run** the conformance and round-trip suites — FAIL.
- [ ] **Step 3: Implement.** The union is a message with one record at the arm's
      ordinal; a non-optional union field absent is `MissingRequired`. A map
      entry is a message with key 1 and value 2 under D-5's entry rule.
- [ ] **Step 4: Run** — PASS; `cargo test -p ridl-backend-rust` — PASS.
- [ ] **Step 5: Commit**
      `feat(ridl-backend-rust): emit the proto3 codec for collections and unions`.

### Task 6: the reader rules, the refusals and absence

Implements D-7, D-8, D-9, D-10, D-12; D-17 cases 5, 6 and 7.

**Files:**

- Modify: `crates/ridl-backend-rust/src/codec_proto3.rs`,
  `tests/proto3_conformance.rs`; create
  `tests/fixtures/proto3_zero_legality.ridl` and reuse
  `tests/fixtures/flatbuffers_evolution_v{1,2}.ridl`

- [ ] **Step 1: Failing tests** (each a hand-built buffer, each also fed to
      `prost` and the `prost` verdict asserted, so the suite records where this
      codec is stricter): `unpacked_and_mixed_repeated_numerics_read`;
      `fields_out_of_order_read`; `a_bool_of_two_reads_as_true`;
      `unknown_fields_of_each_wire_type_are_skipped_anywhere`;
      `a_newer_writers_payload_above_max_size_verifies`;
      `a_group_is_refused_where_prost_skips_it` (`WireType`);
      `a_known_field_with_the_wrong_wire_type_is_refused` (`WireType`);
      `a_varint_wider_than_its_scalar_is_refused` (`Varint`; `prost` truncates);
      `a_value_wider_than_its_width_is_a_range_violation` (300 in a `u8` field →
      `Contract(Range)`); `a_packed_run_cut_inside_a_varint_is_out_of_bounds`;
      `a_singular_field_twice_is_a_duplicate` (scalar, string, nested message;
      `prost` accepts each); `two_union_arms_are_a_duplicate`;
      `an_undeclared_enum_value_is_a_variant_violation`;
      `an_undeclared_enum_set_bit_is_a_variant_violation`;
      `invalid_utf8_is_refused`; `absence_reads_as_the_default_when_it_is_legal`
      (one buffer per D-12 row, at a struct field, a tuple field, a map entry
      key and value, and a box root);
      `absence_is_missing_required_when_the_default_is_illegal`
      (`integer [1..10]`, `[-1.5..1.5 step 1.0]`, an enum with no zero member, a
      string `[1..8]`, an array `[T; 1..4]`, a union field, an empty payload of
      a box whose type excludes 0);
      `whether_absence_reads_as_zero_follows_the_type` (one buffer, five structs
      differing in one field's type);
      `every_append_ridl_diff_calls_compatible_reads_an_old_payload` (the
      evolution fixture: for each struct, encode with v1's codec, verify with
      v2's; every append `ridl_diff::diff_packages` calls compatible verifies).
- [ ] **Step 2: Run** — FAIL.
- [ ] **Step 3: Implement** in the emitted verify: one `[bool; N]` seen-array
      per message; legality of absence decided at generation time.
- [ ] **Step 4: Run** — PASS.
- [ ] **Step 5: Commit**
      `feat(ridl-backend-rust): apply the proto3 reader rules and refusals`.

### Task 7: the property, robustness, corpus and determinism cases

Implements D-17 cases 3, 8, 9, 11 and the worst-case half of 12.

**Files:**

- Modify: `crates/ridl-backend-rust/tests/proto3_conformance.rs`,
  `tests/support/proto3.rs`, `Cargo.toml` (`proptest` dev-dependency with the
  `std` feature)

- [ ] **Step 1: Tests:**
      `prost_values_round_trip_through_this_codec_byte_for_byte` (`proptest`
      with a fixed seed and `cases = 256`, strategies per fixture type that
      respect each typl range, step, length and count; maps generated as
      `BTreeMap`; floats exclude `-0.0`; `prost` encodes, the program decodes
      and re-encodes, the bytes are equal);
      `verify_never_panics_and_is_no_looser_than_prost` (each case-1 buffer
      truncated at every length and with every single byte replaced by `0x00`,
      `0xff` and `0x80`, plus 512 seeded random buffers: the program never
      aborts, and every buffer it accepts `prost` also accepts with an equal
      re-encoding); `corpus_defaults_round_trip_through_prost_reflect` (D-17
      case 9); `the_encoding_of_one_value_is_pinned_across_runs` (a literal) and
      `the_encoding_is_deterministic` (second encode into `[0xAB; N]`);
      `the_worst_case_value_fits_max_size` (per fixture type: maximum counts,
      maximum lengths of four-byte characters, widest varints, negative enum
      values).
- [ ] **Step 2: Run** — fix the codec (or the sizer, per D-4) if anything fails.
- [ ] **Step 3: Run**
      `cargo test -p ridl-backend-rust --test proto3_conformance` and record its
      wall time in the commit body; keep it under 30 seconds on the driver's
      machine.
- [ ] **Step 4: Commit**
      `test(ridl-backend-rust): add the proto3 property, robustness and corpus cases`.

### Task 8: `Sizes<Proto3>`, the face over a proto3 port, and the generated crates

Implements D-1 (size table), D-18 (generated-crate half), D-19.

**Files:**

- Modify: `crates/ridl-backend-rust/src/descriptors.rs` (one `Sizes<E>` impl per
  encoding in the list, the size refusals per encoding, the interface doc naming
  each table), `crates/ridl-backend-rust/src/lib.rs` (face emitted when the list
  is non-empty), `tests/interaction_face.rs` (the reversed codec and its
  hand-written `Sizes<Proto3>` deleted; the four round trips run also over
  `Loopback::<Proto3>::with_encoding`), `tests/fixtures/interaction_face.ridl`
  consumers and `tests/generated/interaction_face.rs` (regenerated with
  `[FlatBuffers, Proto3]`), `examples/cabin/ridl.toml`,
  `crates/ridlc/tests/corpus/veh-cluster/ridl.toml`, `justfile` (`compat-check`
  cabin cell gains `--cfg 'feature="proto3"'` for `ridl-rt` and the crate;
  `demo` runs `the_generated_crate_reaches_no_prost_crate` beside the planus
  check), `xtask/tests/oracle_boundary.rs`, `xtask/src/freeze/rust_api.rs`
  (stale comment), `crates/ridlc` snapshots or expectations that read those two
  manifests
- Test: `tests/interaction_face.rs`, `tests/descriptor_generation.rs`,
  `crates/ridlc/tests/cabin_example.rs`

- [ ] **Step 1: Failing tests:**
      `the_proto3_size_table_is_built_from_proto3_bounds` (rows and maxima are
      expressions over `Payload<Proto3>::MAX_SIZE`);
      `a_proto3_only_package_has_a_face`;
      `signal_event_command_query_round_trip_over_a_proto3_loopback`;
      `the_generated_crate_reaches_no_prost_crate` (`#[ignore]`d).
- [ ] **Step 2: Run** — FAIL.
- [ ] **Step 3: Implement.** The FlatBuffers table's emitted text must not
      change for a FlatBuffers-only list.
- [ ] **Step 4: Run** `cargo test -p ridl-backend-rust -p ridlc`,
      `cargo xtask freeze check`, then `just demo` and `just compat-check` —
      PASS.
- [ ] **Step 5: Commit**
      `feat(ridl-backend-rust): emit the proto3 size table and run the face over it`.

### Task 9: the records

Implements D-21. No code.

**Files:**

- Create: `docs/design/proto3-codec.md` (as-built: where the code is, what is
  emitted, the wire mapping, presence and absence, the reader rules and
  refusals, conformance with the cases and the mutations the reviews measured,
  `wasm32`, known gaps), linked from `docs/design/README.md`
- Modify: the records D-21 lists; `docs/book/SUMMARY.md` only if a book page is
  added (none planned)
- [ ] **Step 1:** write and amend; each amendment dated 2026-10-20 in the
      record's own style.
- [ ] **Step 2: Run** `just link-check`, `just doc-path-check`,
      `just story-id-check`, `just book-check`, `just check` — PASS.
- [ ] **Step 3: Commit** `docs(docs): record the proto3 payload codec`.
