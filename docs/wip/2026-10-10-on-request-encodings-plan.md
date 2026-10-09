# On-request encodings, sub-stage 1a — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** A generated Rust package carries a payload codec only when its
manifest names it, every emitted codec carries one size table per interface that
the reservation and the table budget read, and `ridl build` warns when a
selected deployment needs an encoding the package does not carry.

**Architecture:** `ridl-core` parses `[backend.<name>] encodings`; `ridlc` sends
it to that backend as the `encodings` request option and enables the matching
`ridl-rt` features in the generated `Cargo.toml`; the Rust backend emits types
only when the option is absent, and the codec, the descriptors, the `Sizes<E>`
table and the face when it names `flatbuffers`; `ridl-rt` 0.8 replaces
`EncodedSizes` by `Sizes<E>`; `ridlc` raises RSDL-807 from the checked system.
The face stays monomorphic over FlatBuffers in this sub-stage but reads every
size through `Sizes<FlatBuffers>`, so sub-stage 1b only adds `E`.

**Tech Stack:** Rust workspace (pinned toolchain in `rust-toolchain.toml`),
`toml` + `serde` in `ridl-core`, `quote` + `prettyplease` in the backend,
`insta` snapshots, `just` recipes as the gate.

**Spec:** `docs/wip/2026-10-10-on-request-encodings-design.md` (sections 3 to 6,
7.4, 8 to 13; section 7 is sub-stage 1b and is out of scope here).

## Global Constraints

- Every change to `crates/ridl-rt` and to the emitted code compiles under the
  codegen build matrix of ADR-0021 decision 10: edition 2021 with Rust 1.83,
  edition 2021 with the pin, edition 2024 with the pin (`just compat-check`).
- `ridl-rt` keeps `rust-version = "1.83"`, no dependency, default features off.
- Encoding names are exactly `flatbuffers`, `proto3`, `repr-c`, the values of
  `ridl_rt::encoding::Encoding::NAME`.
- The request schema stays `ridl.codegen.v1`; `BackendOption` is unchanged; keys
  unique; the `encodings` value is the names joined by `,` with no spaces.
- The `ridl-rt = "0.7"` literal in `render_cargo_toml` and its guard test are
  bumped by the release procedure, not by this plan.
- Prose (rustdoc, comments, commit messages, records) is plain literal English.
  No story id, stage or lane name in any file under `crates/`, `examples/`,
  `docs/book/`, `docs/design/` or `docs/technotes/` (`just story-id-check`).
- Commits follow `.git-std.toml`: `<type>(<scope>): <summary>`, scopes used here
  are `ridl-rt`, `ridl-core`, `ridlc`, `ridl-backend-rust`, `docs`, `adr`,
  `roadmap`. End each commit message with the attribution line the harness gives
  the implementer.
- Each task's implementer runs `just compile`, `just test`, `just lint` and
  `just fmt-check` before handing back; the task lists the other gate members it
  touches. The reviewer gets the task's diff as a file and returns two verdicts
  (meets its brief; code is sound), per the task review gate.
- Model tier per task: **fable** for a task marked subtle, **sonnet** for a task
  marked mechanical.
- Task 3 starts only after PR #787 (lane H3) has merged, or rebases onto it
  first: both edit `write_emits` in `crates/ridlc/src/lib.rs`.

## Review Focus

1. `encodings = []` and an absent key must behave the same (types only) and draw
   no diagnostic; `encodings = [""]` is MANI-015. Pinned in Task 2.
2. `encodings = ["FlatBuffers"]` (wrong case) is MANI-015 with the known names
   in the message; a user who reads it corrects the case. Pinned in Task 2.
3. A manifest with `[backend.kotlin] encodings = ["flatbuffers"]` and a build
   `--emit rust` sends the Rust backend nothing and emits types only, and the
   generated `Cargo.toml` enables no `ridl-rt` encoding feature. Pinned in Task
   3 and Task 4.
4. A types-only crate compiles for a target with no standard library with its
   default features off (the second build of `just demo` does this for the
   corpus with a codec; a types-only crate must not regress it). Pinned in
   Task 4.
5. A build that selects a deployment with a different-machine link and names
   only `flatbuffers` warns once per link, and
   `[lints] link-encoding-not-emitted = "deny"` turns that into exit code 1 with
   no file written. Pinned in Task 7.

---

### Task 1: `Sizes<E>` and the two budget functions in `ridl-rt` (additive) — fable

**Files:**

- Modify: `crates/ridl-rt/src/contract.rs` (after the `Interaction` trait, lines
  72-75; the `Member` impl at 179-224 is untouched in this task)
- Modify: `crates/ridl-rt/src/lib.rs` (crate docs listing `contract`'s items, if
  they enumerate them)
- Test: `crates/ridl-rt/tests/sizes.rs` (new)
- Modify: `docs/design/ridl-rt.md` (the `contract` item list around line 36 and
  the reservation section around lines 289-331 gain the new items; the old items
  stay listed until Task 5 removes them)

**Interfaces:**

- Consumes: `contract::Interface`, `contract::Interaction`,
  `encoding::Encoding`, `payload::Payload<E>::MAX_SIZE`.
- Produces (spec section 6.1 and 6.2, verbatim):

```rust
pub trait Sizes<E: Encoding>: Interface {
    const RESERVATIONS: &'static [u64];
    const MAX_BUFFER_SIZE: usize;
    const EVENT_SOURCE_BUFFER_SIZE: usize;
    type CallBuffer: AsRef<[u8]> + AsMut<[u8]> + Copy;
    type EventBuffer: AsRef<[u8]> + AsMut<[u8]> + Copy;
    const CALL_BUFFER: Self::CallBuffer;
    const EVENT_BUFFER: Self::EventBuffer;
}
pub const fn reservation<X, E>() -> u64
where X: Interaction, X::Iface: Sizes<E>, E: Encoding;   // X::Iface::RESERVATIONS[X::ROW]
pub const fn table_budget<I, E>() -> u64
where I: Sizes<E>, E: Encoding;                           // saturating sum of I::RESERVATIONS
```

`Interaction` gains `const ROW: usize` in Task 5, not here; until then
`reservation` takes the row from a second parameter: write it as
`pub const fn reservation<I: Sizes<E>, E: Encoding>(row: usize) -> u64` in this
task and Task 5 changes it to the `X: Interaction` form when `ROW` exists. Task
5's Interfaces block repeats the final signature.

- [ ] **Step 1: Write the failing tests** in `crates/ridl-rt/tests/sizes.rs`.
      Hand-write two payload types with `impl Payload<FlatBuffers>` whose
      `MAX_SIZE` are 8 and 24 (copy the shape of `tests/budget.rs`'s fixture),
      an `Interface` `Cabin` with three members, and
      `impl Sizes<FlatBuffers> for Cabin` with `RESERVATIONS = &[8, 32, 24]`,
      `MAX_BUFFER_SIZE = 24`, `EVENT_SOURCE_BUFFER_SIZE = 8`, the array types,
      and the zeroed consts. Tests:
  - `a_reservation_is_its_row`: `reservation::<Cabin, FlatBuffers>(1) == 32` and
    `reservation::<Cabin, FlatBuffers>(0) == 8` (two rows, so a function that
    ignores `row` fails).
  - `a_table_budget_sums_every_row`:
    `table_budget::<Cabin, FlatBuffers>() == 64` (three distinct rows, so a
    maximum or a first row fails).
  - `a_table_budget_saturates`: a second interface whose rows are
    `[u64::MAX, 1]` gives `u64::MAX`.
  - `a_budget_is_a_const`:
    `const B: u64 = table_budget::<Cabin, FlatBuffers>();` then
    `assert_eq!(B, 64)`.
  - `a_call_buffer_has_the_interface_maximum`: a generic
    `fn hold<I: Sizes<E>, E: Encoding>() -> usize { let mut b = I::CALL_BUFFER; b.as_mut().len() }`
    returns 24, and the same over `EVENT_BUFFER` returns 8, and every byte of
    `CALL_BUFFER` is 0.
- [ ] **Step 2: Run** `cargo test -p ridl-rt --test sizes` — expected: fails to
      compile, `Sizes` not found.
- [ ] **Step 3: Implement** the trait and the two `const fn`s in `contract.rs`
      with the rustdoc of spec section 6.1 and 6.2 (plain English; say that
      every number derives from `MAX_SIZE` and that a missing table is a compile
      error). Add `Sizes`, `reservation` and `table_budget` to the module docs'
      list of what reads the descriptors.
- [ ] **Step 4: Run** `cargo test -p ridl-rt --test sizes` — expected: PASS.
      Then `cargo test -p ridl-rt` and `cargo doc -p ridl-rt --no-deps` with
      `RUSTDOCFLAGS=-Dwarnings` — expected: PASS, no warning.
- [ ] **Step 5: Update `docs/design/ridl-rt.md`**: add the trait and the two
      functions to the `contract` item list and a paragraph under the
      reservation section saying the table is the source and the old row-based
      functions are retired by the same release (cite nothing by story id).
- [ ] **Step 6: Gate**: `just compat-check` (the packaged `ridl-rt` builds as
      edition 2021 with 1.83), `just wasm-check`, `just check`,
      `just link-check`.
- [ ] **Step 7: Commit**:
      `feat(ridl-rt): add the per-codec size table Sizes<E> and its budget functions`.

---

### Task 2: `[backend.<name>] encodings` in the manifest — sonnet

**Files:**

- Modify: `crates/ridl-core/src/manifest.rs` (`Manifest` at line 70,
  `parse_manifest` at 124, `RawCodegen` at 262 as the model for a new
  `RawBackend`, `check_unknown_keys` at 370-402, the tests module from 523)
- Modify: `crates/ridl-core/src/diag.rs` (after MANI-014 at line 1326)
- Modify: `crates/ridl-core/src/package.rs` or `workspace.rs`, wherever MANI-012
  is raised for a member's `[codegen]`: raise MANI-016 beside it
- Modify: `docs/specification/ridl-family-overview.md` §7 (MANI table, lines
  340-346): rows for MANI-015 and MANI-016
- Modify: `docs/decisions/ADR-0002-*.md`: an
  `Amended 2026-10-xx by
  driftsys/ridl#801` line in the Status block and a
  "**The `[backend.<name>]` tables**" paragraph in §4 after the `[codegen]` one
  (line 260), in the same form, with the toml example of spec section 3.1 and
  the sentence that separates it from rsdl's `backend.key` namespace

**Interfaces:**

- Produces: `Manifest::backends: BTreeMap<String, BackendTable>` with
  `pub struct BackendTable { pub encodings: Vec<(String, Range<usize>)> }` (the
  string is the validated name, the range its span in the manifest text); an
  empty `Vec` for `encodings = []`; no entry for an absent table.
- Produces: `DiagCode::MANI_015` (Error, "`[backend.<name>] encodings` is not an
  array of encoding names, names an unknown encoding, or names one twice") and
  `DiagCode::MANI_016` (Error, "`[backend.<name>]` is set in a workspace
  member").
- Produces:
  `pub const ENCODING_NAMES: [&str; 3] = ["flatbuffers", "proto3", "repr-c"]` in
  `manifest.rs`, the one list later tasks cite.

- [ ] **Step 1: Write the failing tests** in the `manifest.rs` tests module,
      using the existing `parse` and `codes` helpers and `PACKAGE_HEAD`:
  - `backend_encodings_parse_in_order`:
    `[backend.rust]\nencodings = ["proto3", "flatbuffers"]` gives
    `backends["rust"].encodings` names `["proto3", "flatbuffers"]` and no
    diagnostic.
  - `backend_encodings_empty_is_types_only`: `encodings = []` parses to an empty
    `Vec`, no diagnostic.
  - `backend_table_for_any_name_is_accepted`: `[backend.kotlin]` parses, no
    diagnostic.
  - `unknown_encoding_is_mani_015`: `["FlatBuffers"]` gives exactly
    `["MANI-015"]`, and the message contains `FlatBuffers` and
    `flatbuffers, proto3, repr-c`.
  - `duplicate_encoding_is_mani_015`: `["flatbuffers", "flatbuffers"]`.
  - `empty_encoding_name_is_mani_015`: `[""]`.
  - `encodings_not_an_array_is_mani_015`: `encodings = "flatbuffers"`.
  - `backend_not_a_table_is_mani_015`: `backend = 1` and `[backend]\nrust = 1`.
  - `unknown_backend_key_is_mani_005`: `[backend.rust]\nfoo = 1` gives
    `["MANI-005"]` (model: `unknown_codegen_key_is_mani_005`).
  - In the loader's tests (where MANI-012 is tested):
    `backend_table_in_member_is_mani_016`, and
    `backend_table_at_root_is_not_mani_016`.
- [ ] **Step 2: Run** `cargo test -p ridl-core manifest` — expected: compile
      failure on `backends`.
- [ ] **Step 3: Implement**: a `RawBackends` map in the typed parse (model
      `RawCodegen`), the validation producing MANI-015 with the span of the
      offending element (the table's span when the shape is wrong), the
      `check_unknown_keys` arm `"backend" =>` that walks each sub-table with
      `check_section_keys(.., "backend.<name>", .., &["encodings"], ..)`, the
      two catalogue rows (rustdoc in the form of MANI-012's), and MANI-016
      beside MANI-012.
- [ ] **Step 4: Run** `cargo test -p ridl-core` — expected: PASS, including the
      catalogue drift test.
- [ ] **Step 5: Records**: the family overview rows and the ADR-0002 paragraph
      and Status line.
- [ ] **Step 6: Gate**: `just check`, `just link-check`, `just doc-path-check`,
      `just story-id-check`.
- [ ] **Step 7: Commit**:
      `feat(ridl-core): read [backend.<name>] encodings from the manifest`.

---

### Task 3: the `encodings` option reaches its backend; the generated `Cargo.toml` follows it — sonnet

Starts after PR #787 has merged (Global Constraints).

**Files:**

- Modify: `crates/ridlc/src/lib.rs` (`run_build`/`run_build_with` at 752-1010
  where the `Manifest` is loaded and `codegen_header_file` read;
  `render_cargo_toml` at 1309; `write_crate_files` at 1150-1200;
  `codegen_request` at 1785 unchanged; `write_emits` at 2054-2160)
- Modify: `crates/ridl-ir/proto/ridl/codegen/v1/plugin.proto` (the
  `BackendOption` comment at lines 73-75: replace "No flag sets one yet" by the
  `encodings` convention)
- Modify: `docs/design/codegen-plugins.md` lines 96-114 ("Options")
- Test: `crates/ridlc/tests/rust_crate_emit.rs` (existing, for the
  `Cargo.toml`), `crates/ridlc/tests/codegen_model.rs` (request contents), and a
  new fixture directory under `crates/ridlc/tests/` with a manifest that has
  both `[backend.rust]` and `[backend.kotlin]`

**Interfaces:**

- Consumes: `Manifest::backends` (Task 2).
- Produces: `write_emits(.., backends: &BTreeMap<String, Vec<String>>, ..)` —
  the map from backend language to its encoding names, in manifest order;
  `fn backend_options(backends, language: &str) -> Vec<v1::BackendOption>`
  returning `[]` or one option `{ key: "encodings", value: names.join(",") }`;
  the language of a built-in emit is `"rust"`, `"typescript"`, `"proto"`,
  `"flatbuffers"`, `"codegen-model"` and of a plugin its `PluginSpec::language`.
- Produces: `render_cargo_toml(crate_name, preamble, encodings: &[String])`
  writing `ridl-rt = { version = "0.7" }` for an empty list and
  `ridl-rt = { version = "0.7", features = ["flatbuffers", "proto3"] }` for
  names, in order.

- [ ] **Step 1: Write the failing tests**:
  - `the_rust_request_carries_the_rust_encodings_only`: build the new fixture
    with `--emit codegen-model` is not enough to see the Rust request, so
    unit-test `backend_options`: for
    `{rust: [flatbuffers, proto3], kotlin: [flatbuffers]}`,
    `backend_options(.., "rust")` is one option `encodings=flatbuffers,proto3`,
    `backend_options(.., "kotlin")` is `encodings=flatbuffers`, and
    `backend_options(.., "typescript")` is empty.
  - `a_plugin_receives_its_own_table`: run
    `ridl build <fixture> --plugin kotlin=<a shell stub that dumps the request JSON>`
    (model: the plugin tests in `crates/ridlc/tests/` that use a stub plugin)
    and assert the dumped `options` is
    `[{"key":"encodings","value":"flatbuffers"}]`.
  - In `rust_crate_emit.rs`: `cargo_toml_enables_one_feature_per_encoding`
    (fixture with `["flatbuffers"]`: the `ridl-rt` line equals the string above)
    and `cargo_toml_has_no_feature_for_types_only` (a fixture with no table: the
    line is `ridl-rt = { version = "0.7" }` — a renderer that always writes
    `features` fails).
  - The `"0.7"` guard test keeps passing unchanged.
- [ ] **Step 2: Run** `cargo test -p ridlc` — expected: failures on the new
      names.
- [ ] **Step 3: Implement**: read `backends` from the `Manifest` next to
      `codegen_header_file` and thread it to `write_emits` and
      `write_crate_files`; in `write_emits`, build the request once with
      `Vec::new()` and for each emit and each plugin clone it and set
      `options = backend_options(..)` before handing it on; `render_cargo_toml`
      takes the names. Update the proto comment and the design note's "Options"
      paragraph: one request per (package, backend), the `encodings` key, the
      joined value, `wire-encoding` retired.
- [ ] **Step 4: Run** `cargo test -p ridlc -p ridlc-gen-rust -p ridlc-gen-model`
      — expected: PASS (the two parity tests still pass `Vec::new()`).
- [ ] **Step 5: Gate**: `just compile`, `just test`, `just lint`, `just check`,
      `just link-check`.
- [ ] **Step 6: Commit**:
      `feat(ridlc): send [backend.<name>] encodings to that backend and enable its ridl-rt features`.

---

### Task 4: the Rust backend emits a codec only when `encodings` names it — fable

**Files:**

- Modify: `crates/ridl-backend-rust/src/contract.rs` (lines 12-63)
- Modify: `crates/ridl-backend-rust/src/lib.rs` (`generate` 83, `generate_face`
  107, `generate_face_with` 128, `generate_with` 154, `generate_pipeline` 197,
  `generate_pipeline_over` 212, `WireEncoding` 397-418, `package_items`)
- Modify: `crates/ridl-backend-rust/src/tests.rs` (1552, 1555 and the snapshots
  that include the codec), `src/snapshots/`
- Modify: `crates/ridl-backend-rust/tests/descriptor_generation.rs` (530-540),
  `name_collision_claims.rs` (50, 213, 496), `name_collision_compile.rs` (69),
  every test calling `generate_face`
- Modify: `examples/cabin/ridl.toml`,
  `crates/ridlc/tests/corpus/veh-cluster/ridl.toml` (each gains
  `[backend.rust]` + `encodings = ["flatbuffers"]`), and the `ridlc` snapshots
  those builds feed (`crates/ridlc/tests/corpus.rs`, `golden.rs`, re-recorded
  with `cargo insta` and inspected)
- Test: `crates/ridl-backend-rust/tests/descriptor_generation.rs`,
  `crates/ridl-backend-rust/tests/face_compile.rs`

**Interfaces:**

- Consumes: the `encodings` request option (Task 3).
- Produces:

```rust
pub const ENCODINGS_OPTION: &str = "encodings";
#[non_exhaustive] pub enum WireEncoding { FlatBuffers }          // no Default
impl WireEncoding { pub fn parse(name: &str) -> Option<Self>; pub fn name(self) -> &'static str; }
pub fn generate(package: &v2::Package) -> Result<Generated, GenerateError>;                 // types only
pub fn generate_with(package: &v2::Package, others: &[&v2::Package]) -> Result<Generated, GenerateError>;  // types only, signature unchanged
pub fn generate_face_with(package: &v2::Package, encodings: &[WireEncoding]) -> Result<Generated, GenerateError>;
pub fn generate_pipeline(package: &v2::Package, encodings: &[WireEncoding], others: &[&v2::Package]) -> Result<Generated, GenerateError>;
pub(crate) fn generate_pipeline_over(model: &v1::Model, encodings: &[WireEncoding]) -> Result<Generated, GenerateError>;
```

`generate_face` is removed. `Backend::generate` reads `ENCODINGS_OPTION`, splits
on `,`, maps each name through `parse` (an unknown name that is one of
`proto3`/`repr-c` is refused with "the Rust backend has no `proto3` codec
(driftsys/ridl#264)" / "... `repr-c` ... (driftsys/ridl#317)"; any other name,
an empty element and a repeated name with "`encodings` names ..."), refuses any
other key with "takes one option, `encodings`", and calls
`generate_pipeline_over(model, &list)`.

- [ ] **Step 1: Write the failing tests** in `descriptor_generation.rs` (replace
      `the_default_wire_encoding_is_flatbuffers`):
  - `no_encodings_emits_types_only`: `generate_face_with(&package, &[])`'s
    source contains the type `Temperature` and contains none of `Payload<`,
    `FbView`, `impl ::ridl_rt::contract::Interface`, `pub mod cabin`.
  - `flatbuffers_emits_the_codec_descriptors_and_face`: with
    `&[WireEncoding::FlatBuffers]` the source contains all four.
  - `generate_emits_types_only`: `generate(&package)` contains no `Payload<`.
  - Through `Backend::generate` with a hand-built request (model:
    `crates/ridl-ir/src/codegen/tests.rs:969`): `wire_encoding_is_refused`
    (message contains `encodings`), `proto3_is_refused_naming_its_issue`
    (message contains `driftsys/ridl#264`), `a_repeated_encoding_is_refused`,
    `an_empty_element_is_refused` (`"flatbuffers,"`),
    `an_unknown_key_is_refused`.
  - In `face_compile.rs`: `a_types_only_crate_compiles_without_a_codec`: the
    types-only output compiles with `rustc` against `ridl-rt` built with no
    features (the test file already drives `rustc`; pass no `--cfg feature` for
    `flatbuffers`).
- [ ] **Step 2: Run**
      `cargo test -p ridl-backend-rust --test descriptor_generation` — expected:
      compile failure on the new signatures.
- [ ] **Step 3: Implement** per the Interfaces block. `package_items` appends
      the codec, the descriptors and the face only when the list is non-empty;
      with `FlatBuffers` in the list the output is byte-identical to today's
      (assert it once in a test by comparing with the checked-in fixture).
- [ ] **Step 4: Run** `cargo test -p ridl-backend-rust` — expected: PASS after
      re-recording the `src/tests.rs` snapshots whose model names no encoding
      (inspect each: the diff is the codec, the descriptors and the face
      removed, nothing else).
- [ ] **Step 5: Manifests and corpus**: add the table to the two `ridl.toml`
      files; run `cargo test -p ridlc` and re-record only the snapshots whose
      diff is the new manifest lines; run `just demo` and `just compat-check`.
- [ ] **Step 6: Gate**: `just compile`, `just test`, `just lint`, `just demo`,
      `just compat-check`, `just wasm-check`.
- [ ] **Step 7: Commit**:
      `feat(ridl-backend-rust): emit the codec, the descriptors and the face only when encodings names a codec`
      — state in the body that this is a breaking change to what
      `ridl build --emit rust` emits and that the two manifests gain the table.

---

### Task 5: the size table replaces the size rows — fable

One atomic change across `ridl-rt` and the backend: the workspace compiles the
generated fixture against `ridl-rt`, so the removals and the new emission land
together.

**Files:**

- Modify: `crates/ridl-rt/src/contract.rs` (`Interaction` 72-75 gains `ROW`;
  `Member` 144-158 keeps `payloads: &'static [PayloadInfo]`; remove
  `Member::reservation` 179-203, `table_budget(&[Member])` 218-224, `Unsized`
  228-238, `EncodedSizes` 283-295 and `PayloadInfo::max_size`; `reservation`
  from Task 1 takes its final form)
- Modify: `crates/ridl-rt/src/encoding.rs` (remove `max_size` from `Encoding` at
  50-61 and from the three impls at 82-99; the `compile_fail` doctests at 24-48
  and the `Fourth` test at 105-142 lose `max_size`; drop the
  `use crate::contract::EncodedSizes`)
- Modify: `crates/ridl-rt/tests/budget.rs` (delete, its cases move to
  `tests/sizes.rs`), `tests/descriptors.rs` (`SIZES` and `max_size` fields),
  `tests/payload.rs:170`, `examples/read_sample.rs:19,104`
- Modify: `crates/ridl-backend-rust/src/descriptors.rs` (100-170 the interface
  descriptor, 380-440 `payload_info` and `max_size_path`)
- Modify: `crates/ridl-backend-rust/tests/generated/interaction_face.rs`
  (regenerated by `interaction_face_regeneration.rs`)
- Test: `crates/ridl-rt/tests/sizes.rs`,
  `crates/ridl-backend-rust/tests/descriptor_generation.rs`,
  `crates/ridl-backend-rust/tests/flatbuffers_conformance.rs`
- Modify: `docs/design/ridl-rt.md` (36, 170-171, 185-197, 274, 289-297, 323-357:
  the removed items leave the lists, the section on budgets describes the
  table), `docs/technotes/ridl-rt-by-example.md` if it names `reservation` or
  `EncodedSizes`

**Interfaces:**

- Consumes: `Sizes<E>` (Task 1), the pipeline list (Task 4).
- Produces, in `ridl-rt`:

```rust
pub trait Interaction { type Iface: Interface; const MEMBER: &'static Member; const ROW: usize; }
pub struct PayloadInfo { pub type_name: &'static str }
pub const fn reservation<X, E>() -> u64 where X: Interaction, X::Iface: Sizes<E>, E: Encoding;
pub trait Encoding: sealed::Sealed + 'static { const NAME: &'static str; }
```

- Produces, in the emitted code, per interface and per named codec:
  `impl ::ridl_rt::contract::Sizes<::ridl_rt::encoding::FlatBuffers> for <Iface>`
  with the items of spec section 6.1, every value a const expression over
  `<T as Payload<FlatBuffers>>::MAX_SIZE` (reuse `max_size_path` and
  `max_size_const`; `RESERVATIONS` rows are `a as u64 + b as u64`, a fixed
  member's row is `0u64`). The inherent `MAX_BUFFER_SIZE` and
  `EVENT_SOURCE_BUFFER_SIZE` constants on the interface descriptor **stay in
  this task**, redefined as
  `pub const MAX_BUFFER_SIZE: usize = <Self as Sizes<FlatBuffers>>::MAX_BUFFER_SIZE;`
  so the face compiles unchanged; Task 6 removes them. Each interaction
  descriptor gains `const ROW: usize = <its index in MEMBERS>;`.

- [ ] **Step 1: Write the failing tests**:
  - `tests/sizes.rs` (ridl-rt): `reservation::<CabinAverage, FlatBuffers>()`
    equals row 1 (32) and `reservation::<CabinTemperature, FlatBuffers>()` row 0
    (8), with the test's `Interaction` impls carrying `ROW`; move
    `a_table_budget_sums_every_member`, `an_empty_table_has_a_budget_of_zero`
    and `the_sum_is_wider_than_one_size` from `budget.rs` onto the table; delete
    the `Unsized` cases.
  - `descriptor_generation.rs`: `the_size_table_derives_from_max_size`: the
    generated source for the fixture contains
    `impl ::ridl_rt::contract::Sizes<::ridl_rt::encoding::FlatBuffers> for Cabin`
    and no `EncodedSizes`; `each_interaction_descriptor_has_its_row`: for every
    `impl Interaction for X` in the fixture, `X::ROW` indexes a member whose
    ordinal equals `X::MEMBER.ordinal` (a compiled check in `face_compile.rs`'s
    style, or by parsing the fixture).
  - `flatbuffers_conformance.rs`:
    `the_reservation_rows_match_the_oracle_bounds`: for the fixture's `Cabin`,
    `<Cabin as Sizes<FlatBuffers>>::RESERVATIONS` equals the rows the test
    computes from the planus-verified `MAX_SIZE`s (query row = argument + reply,
    two payloads of different size so a maximum fails);
    `the_buffer_sizes_are_the_interface_maxima`: `MAX_BUFFER_SIZE` equals the
    largest call payload's `MAX_SIZE` and differs from the largest event's (the
    fixture has an event larger than every call payload; if not, extend
    `interaction_face.ridl` with one); `CALL_BUFFER.len() == MAX_BUFFER_SIZE`;
    `EVENT_BUFFER.len() == EVENT_SOURCE_BUFFER_SIZE`.
- [ ] **Step 2: Run** `cargo test -p ridl-rt` then
      `cargo test -p ridl-backend-rust` — expected: compile failures on the
      removed and the new items.
- [ ] **Step 3: Implement** the `ridl-rt` removals and `ROW`, then the emitter;
      regenerate the fixture
      (`cargo test -p ridl-backend-rust --test interaction_face_regeneration`
      with the regeneration env the test documents); update the `read_sample`
      example and the `ridl-rt` tests.
- [ ] **Step 4: Run** the whole workspace: `just test` — expected: PASS (the
      face still compiles through the inherent aliases).
- [ ] **Step 5: Docs**: `docs/design/ridl-rt.md` sections listed above; the
      rustdoc of `PayloadInfo` no longer speaks of `None`; `Encoding`'s docs say
      the trait carries the name only.
- [ ] **Step 6: Gate**: `just compile`, `just test`, `just lint`,
      `just compat-check`, `just wasm-check`, `just demo`, `just check`,
      `just link-check`.
- [ ] **Step 7: Commit**:
      `feat(ridl-rt)!: replace the EncodedSizes rows by one size table per codec`
      — body: the full list of spec section 6.3 except the face-type row.

---

### Task 6: the face takes every size from `Sizes<FlatBuffers>` — sonnet

**Files:**

- Modify: `crates/ridl-backend-rust/src/face.rs` (`payload_buffer` 1072-1075 and
  its three callers 365, 818, 846; the doc strings at 1089 and 1107)
- Modify: `crates/ridl-backend-rust/src/face/poll.rs` (70, 142, 188),
  `face/serve.rs` (64, 94, 104), `face/dispatch.rs` (121-177)
- Modify: `crates/ridl-backend-rust/src/descriptors.rs` (remove the inherent
  `MAX_BUFFER_SIZE`/`EVENT_SOURCE_BUFFER_SIZE` aliases Task 5 kept, and the
  interface doc string at 121-128)
- Modify: `crates/ridl-backend-rust/tests/generated/interaction_face.rs`
  (regenerated), `docs/design/interaction-face.md` lines 66-76 and 440 (buffer
  sizing now names `Sizes<E>::CALL_BUFFER`; the const-evaluable rule moves to
  the table's items)
- Test: `crates/ridl-backend-rust/tests/interaction_face.rs`,
  `dispatch_generation.rs`

**Interfaces:**

- Consumes: `Sizes<FlatBuffers>` items emitted by Task 5.
- Produces: every buffer in the emitted face is
  `<super::#iface as ::ridl_rt::contract::Sizes<::ridl_rt::encoding::FlatBuffers>>::CALL_BUFFER`
  (call arguments, replies, `Serve::buf`, the dispatch claim buffer) or
  `::EVENT_BUFFER` (event polling); `Serve::buf`'s type is
  `<.. as Sizes<..>>::CallBuffer`; `payload_buffer(type_name)` becomes
  `call_buffer(iface)` and no emitted line names a payload's own `MAX_SIZE` for
  a buffer.

- [ ] **Step 1: Write the failing tests**:
  - `interaction_face.rs`: `no_buffer_is_sized_by_a_payloads_own_max_size`: the
    fixture source contains no `MAX_SIZE]` (the array-length form) and contains
    `CALL_BUFFER` and `EVENT_BUFFER`;
    `the_descriptor_has_no_inherent_buffer_constants`: the fixture contains no
    `pub const MAX_BUFFER_SIZE`.
  - `dispatch_generation.rs`: the existing short-buffer refusal test is kept and
    its buffer is built as
    `[0u8; <Cabin as Sizes<FlatBuffers>>::MAX_BUFFER_SIZE - 1]`; assert it is
    refused and that a buffer of exactly `MAX_BUFFER_SIZE` is accepted (a face
    sized from the event maximum fails one of the two on the fixture, where the
    maxima differ).
- [ ] **Step 2: Run** `cargo test -p ridl-backend-rust --test interaction_face`
      — expected: FAIL on the string assertions.
- [ ] **Step 3: Implement** the emitter changes and remove the aliases;
      regenerate the fixture.
- [ ] **Step 4: Run** `cargo test -p ridl-backend-rust` and `just demo` (the
      cabin program round-trips every value) — expected: PASS.
- [ ] **Step 5: Docs**: the two passages of `docs/design/interaction-face.md`.
- [ ] **Step 6: Gate**: `just compile`, `just test`, `just lint`, `just demo`,
      `just compat-check`, `just check`, `just link-check`.
- [ ] **Step 7: Commit**:
      `refactor(ridl-backend-rust): size every face buffer from the codec's size table`.

---

### Task 7: RSDL-807 `link-encoding-not-emitted` — fable

**Files:**

- Modify: `crates/ridl-core/src/diag.rs` (after RSDL-806 at 1200: the row; the
  name map at 2109-2113)
- Modify: `crates/ridlc/src/lib.rs` (`run_build_with` around 964-1010, after
  `select_deployment`; `unclaimed_backend_keys` at 1685-1696 is the model)
- Create: `crates/ridlc/src/link_encodings.rs` (the check;
  `pub(crate) fn link_encoding_not_emitted(..)`)
- Modify: `docs/book/lints.md` (table at line 123),
  `docs/specification/rsdl-language-reference.md` (the sentence at 759 and the
  code table at 862-864), `docs/design/codegen-plugins.md` ("The encoding rule",
  one sentence: the rule is what RSDL-807 compares against)
- Test: `crates/ridlc/tests/cli.rs` (the RSDL-804 tests at 86-120 are the
  model), a new fixture workspace under `crates/ridlc/tests/` with an rsdl
  system that declares a deployment with one same-machine and one
  different-machine link

**Interfaces:**

- Consumes: `Manifest::backends` (Task 2), the selected deployment name and the
  checked `system` in `run_build_with`, `encoding_of` in
  `crates/ridl-ir/src/codegen/deployment.rs:293` (make it `pub` or copy its
  three-arm match; it is the one rule).
- Produces:
  `pub(crate) fn link_encoding_not_emitted(db, system: &CheckedSystem, deployment: &str, backend: &str, encodings: &[String], sources: &mut ..) -> Vec<Diagnostic>`
  — one `warning(DiagCode::RSDL_807, ..)` per consumer link of the named
  deployment whose derived encoding name is not in `encodings`, at the link's
  `requires` span, message "link `<consumer> -> <provider>` of deployment
  `<name>` carries `proto3`, which `[backend.<backend>] encodings` does not
  name"; called once per emitted backend (each built-in emit's language and each
  plugin's) when a deployment is selected; the lint level is applied by the
  existing `apply_lint_levels` path like every other diagnostic of the build.

- [ ] **Step 1: Write the failing tests** in `cli.rs` with the new fixture
      (`[backend.rust] encodings = ["flatbuffers"]` at its root):
  - `a_link_whose_encoding_is_not_emitted_warns_once`:
    `ridl build <fixture> --emit rust --deployment Bench` exits 0, stderr
    contains exactly one `warning[RSDL-807]`, the message names `proto3` and
    `[backend.rust] encodings`, and the location is the rsdl file's line of the
    different-machine `requires`.
  - `a_same_machine_link_does_not_warn`: a fixture variant with only the
    same-machine link: no `RSDL-807`.
  - `a_types_only_build_warns_for_every_link`: no `[backend.rust]` table: two
    `RSDL-807`.
  - `the_lint_is_compared_against_the_emitted_backends_table`:
    `[backend.kotlin] encodings = ["proto3"]` plus
    `[backend.rust] encodings = ["flatbuffers"]`, `--emit rust`: one warning
    naming `[backend.rust]`.
  - `allow_silences_it_and_deny_fails_the_build`:
    `[lints] link-encoding-not-emitted = "allow"` gives no diagnostic; `"deny"`
    gives `error[RSDL-807]`, exit 1, and no `lib.rs` written to the out dir.
  - `check_does_not_raise_it`: `ridl check <fixture>` has no `RSDL-807`.
  - `no_deployment_selected_draws_nothing`: without `--deployment`, none.
- [ ] **Step 2: Run** `cargo test -p ridlc --test cli link_encoding` — expected:
      FAIL (no such diagnostic).
- [ ] **Step 3: Implement** the catalogue row (`Warning`, summary from spec
      section 8, `lint = "link-encoding-not-emitted"`, rustdoc naming the
      encoding rule), the name-map entry, the check module and its call site.
- [ ] **Step 4: Run** `cargo test -p ridl-core -p ridlc` — expected: PASS
      (catalogue drift test included).
- [ ] **Step 5: Docs**: the `lints.md` row
      (`link-encoding-not-emitted | RSDL-807 | warn | a link of the selected deployment carries an encoding the emitted backend's encodings list does not name`),
      the rsdl reference's sentence and table row, the one sentence in
      `codegen-plugins.md`.
- [ ] **Step 6: Gate**: `just compile`, `just test`, `just lint`, `just check`,
      `just link-check`, `just book-check`.
- [ ] **Step 7: Commit**:
      `feat(ridlc): warn when a selected deployment needs an encoding the package does not carry`.

---

### Task 8: the book, the technote and the roadmap describe the built behaviour — sonnet

**Files:**

- Modify: `docs/book/cli-reference.md` (around 618: the emitted items list says
  the codec, the descriptors and the face appear when `encodings` names a codec;
  730-734: replace "no flag for the payload encoding" by the manifest rule and
  the statement that a single-file build is types only; the manifest chapter or
  section that lists the tables gains `[backend.<name>]` with the toml example),
  `docs/book/introduction.md` (42-44), `docs/book/generated-code.md` (56: the
  codec "when the manifest names it"; 229 and the feature table at 235: the
  `flatbuffers` feature follows the list; a short paragraph on
  `Sizes<FlatBuffers>` where `MAX_BUFFER_SIZE` was described, if it is)
- Modify: `docs/technotes/walking-skeleton-architecture.md` (the `ridl-core` row
  at 63: the manifest's `[backend.<name>]` table; the `ridl-backend-rust` row at
  159: emits the codecs the request names, types only otherwise)
- Modify: `docs/ROADMAP.md` (line 525: reword the "`proto3` stays `None`"
  sentence to the per-codec table; the proto3 and `repr(C)` codec rows: each
  arrives as a `Sizes<E>` implementation)

- [ ] **Step 1: Edit** each passage; every `toml` example is the one of spec
      section 3.1; no `ridl` fence changes.
- [ ] **Step 2: Gate**: `just book-check`, `just link-check`,
      `just doc-path-check`, `just story-id-check`, `just check`, and
      `cargo test -p ridl-cli --test book_examples`.
- [ ] **Step 3: Commit**:
      `docs(docs): describe the on-request codecs and the size table`.

---

### Task 9: the records are amended — sonnet

**Files** (the line numbers are those of main on 2026-10-10; the passages are
quoted in spec section 10):

- Modify: `docs/decisions/ADR-0018-*.md`: Status amendment line
  (`**Amended 2026-10-xx — decisions 4, 5 and 15, on-request encodings.**` in
  the form of the existing ones); decision 4 (163-164) and 5 (183-197): inline
  `**Amendment (date) — ...**` saying the Rust codec is selected by
  `[backend.rust] encodings`, `--wire` is not the form a codec request takes,
  the schema emits stay `--emit`; decision 15: the face is generic over the
  encoding as well as the ports once the generic face lands, and reads its sizes
  from `Sizes<E>` now.
- Modify: `docs/decisions/ADR-0020-*.md`: decision 5 (199-293): `contract` loses
  `EncodedSizes` and `Unsized`, gains `Sizes<E>`; a generated package enables
  one feature per named encoding and none for types only; decision 9 (349): the
  `encodings` option convention; the "Documents amended" table (542): rows for
  ADR-0021, ADR-0023, `interaction-face.md`, `ridl-rt.md`.
- Modify: `docs/decisions/ADR-0021-*.md`: decision 10 (485-570): a dated note
  "ships as 0.8.0" with the list of spec section 6.3; decision 17 (767-780):
  `Encoding::max_size`, `Member::reservation` and `table_budget(&[Member])`
  replaced by `Sizes<E>` and the two functions; decision 19: `Bind` unchanged.
- Modify: `docs/decisions/ADR-0023-*.md`: decision 2's consequence note
  (180-192): `ridl build --emit rust` emits the face only when an encoding is
  named; lines 584-589: buffers are sized from `Sizes<E>`; decision 6 keeps
  `Client<P>` with a note that the encoding parameter is the next sub-stage's.
- Modify: `docs/design/interaction-face.md`: 89-103 (the `PayloadInfo.max_size`
  passage becomes the `Sizes<FlatBuffers>` table); 581-590 (`WireEncoding`
  default: replaced by the list); 768-805 rule 3 ("No flag selects the encoding,
  yet" becomes the manifest rule, with why a flag is not used); 897 (the
  provisional table's size rows); leave 572-580 ("gains no type parameter") with
  a one-line note that the generic face is specified in the design under
  `docs/wip/` and lands next.
- Modify: `docs/design/flatbuffers-codec.md`: 17-29 (emitted on request), 43
  (the `WireEncoding` row), 249-275 (retitle: the face names the codec through
  `Sizes<FlatBuffers>` and the full path; the alias is gone).
- Modify: `docs/design/catalog-descriptor.md`: 226 (cite the codegen model, not
  `EncodedSizes`), 313-323 (the two sums are `reservation` and `table_budget`
  over `Sizes<E>`; drop the `Unsized` sentence).
- Modify: `docs/specification/rsdl-language-reference.md` "backend keys"
  (432-438): one sentence separating the manifest's `[backend.<name>]` table
  from the attribute namespace.

- [ ] **Step 1: Edit** each record in its own amendment convention (spec section
      10 names them); keep superseded text and mark it, as the records do.
- [ ] **Step 2: Gate**: `just check`, `just link-check`, `just doc-path-check`,
      `just story-id-check`, `just book-check`.
- [ ] **Step 3: Commit**:
      `docs(adr): amend ADR-0018, ADR-0020, ADR-0021, ADR-0023 and ADR-0002 for the on-request encodings`
      (one commit for the ADRs, one `docs(docs): ...` for the design records).

---

### Task 10: the Kotlin heads-up — sonnet

**Files:** none in this repository.

- [ ] **Step 1: File** an issue on driftsys/ridlc-gen-kotlin with
      `gh issue create --repo driftsys/ridlc-gen-kotlin` titled "Heads-up: ridl
      0.8 sends `encodings` instead of `wire-encoding`, and ridl-rt 0.8 replaces
      EncodedSizes", whose body is the four points of spec section 9.5, each
      with the ridl commit or PR that lands it, and the statement that nothing
      changes for the plugin until a manifest gains `[backend.kotlin]`. Write
      "tracked by driftsys/ridl#801"; never a closing keyword.
- [ ] **Step 2: Record** the issue number in a comment on driftsys/ridl#801.

---

## Self-review

- Spec coverage: section 3 → Task 2; 4 → Task 3 (4.3 → Task 10); 5.1-5.3, 5.5 →
  Task 4; 5.4 → no task (unchanged by design); 6 → Tasks 1, 5; 7.4 → Task 6; 8 →
  Task 7; 9.1-9.3 → Task 4 (manifests), Tasks 5-6 (fixtures); 9.4 → Task 8; 9.5
  → Task 10; 10 → Tasks 2, 3, 5, 6, 7, 8, 9; 11 → the tests named in each task;
  12 → Global Constraints; 13 → this plan is 1a.
- Type consistency: `Sizes<E>` items, `reservation`'s two forms (Task 1 interim,
  Task 5 final) and `backend_options` are named once each and reused;
  `WireEncoding::parse` is defined in Task 4 and used by nothing else.
- Review Focus items 1-5 each name their owning task.
- Proportion: the plan carries signatures, test names and assertions, and no
  bodies.
