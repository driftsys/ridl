# Transcode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a reference `transcode` function to `ridl-rt` and an opt-in
manifest key that makes the Rust backend emit `transcode_<s>_to_<d>` functions
on each type with two payload encodings.

**Architecture:** `ridl_rt::payload::transcode` verifies under S, decodes to
`T`, and encodes under D. The manifest key `[backend.rust] transcode = true` is
checked in `ridl-core`, sent by `ridlc` as a backend option, and read by
`ridl-backend-rust`, which emits one inherent function for each ordered pair of
encodings that a type implements `Payload` under. Each generated body calls the
reference function.

**Tech Stack:** Rust (edition 2024, toolchain pin 1.98.1; `ridl-rt` MSRV 1.83
and edition 2021 compatibility), `quote`, `cargo semver-checks` via
`cargo xtask freeze`.

**Spec:** `docs/archive/2026-10-23-transcode-design.md`

## Global Constraints

- Prose in comments, docs and commits is plain and literal (AGENTS.md); no story
  ids or plan names in `crates/`, `docs/book/`, `docs/design/`
  (`just story-id-check`).
- `ridl-rt` stays `no_std`-capable and allocates nothing in `transcode`; it must
  build at its `rust-version` (`just compat-check`).
- `runtime-crates` baseline stays `v0.12.0`; only additive changes.
- Type parameters are `T`, `S`, `D` — never `From`/`To`.
- Generated function name: `transcode_<s>_to_<d>`, `<s>`/`<d>` the manifest
  encoding names with `-` replaced by `_`.
- No function when S equals D; no function for a pair unless the type implements
  `Payload` under both; no `cfg` attribute on the function.
- Default off; `transcode = true` with fewer than two encodings (absent and `[]`
  count as zero) is MANI-020, an error.
- Implementers run focused crate tests; `just build` runs in CI on the PR.

## Review Focus

1. A type that implements `Payload<FlatBuffers>` but not `Payload<Proto3>` (for
   example a plain struct field type that proto3 does not root) gets no function
   and the crate still compiles — tested in Task 3.
2. `out` too small for the target encoding returns
   `TranscodeError::Encode(EncodeError::Capacity { .. })`, not a panic — tested
   in Task 1.
3. Input bytes that are valid under D but not under S are refused as
   `TranscodeError::Verify(_)` — tested in Task 1.
4. `transcode = true` with `encodings = ["flatbuffers", "flatbuffers"]` is the
   existing MANI-015 duplicate, not a silent single encoding that escapes
   MANI-020 — tested in Task 2.
5. A union value transcodes to the same bytes as a direct encode (the union arm
   tag must survive) — the corpus in Task 4 includes a union.

---

### Task 1: `ridl-rt` reference function

**Files:**

- Modify: `crates/ridl-rt/src/payload.rs`
- Modify: `crates/ridl-rt/src/lib.rs` (re-export only if the crate re-exports
  `Ref`/`EncodeError` at the root; follow what it does)
- Modify: `docs/design/ridl-rt.md` (a short section on `transcode`)

**Interfaces:**

- Produces: `ridl_rt::payload::TranscodeError` (`#[non_exhaustive]`,
  `Clone, Copy, Debug,
  PartialEq, Eq`, variants `Verify(VerifyError)`,
  `Encode(EncodeError)`, `From` for both) and
  `pub fn transcode<'o, T, S, D>(bytes: &[u8], out: &'o mut [u8]) -> Result<Ref<'o, T, D>, TranscodeError> where T: Payload<S> + Payload<D>, S: Encoding, D: Encoding`.

- [ ] **Step 1: Write the failing tests** in the `#[cfg(test)]` module of
      `payload.rs`, with a test type `Speed(u16)` that implements
      `Payload<ReprC>` (2 bytes little-endian at the front of `out`) and
      `Payload<Proto3>` (a different layout written at the end of `out`, with a
      range rule 0..=300 so a contract failure is reachable). Tests:
      `transcode_matches_direct_encode` (S→D bytes equal `Ref::<_, D>::encode`
      bytes, and decode back to the value), `transcode_reverse_direction`,
      `transcode_same_encoding_is_identity`, `transcode_refuses_bad_input`
      (`Err(TranscodeError::Verify(_))`), `transcode_reports_contract`
      (`Err(TranscodeError::Encode(EncodeError::Contract(_)))`, with S accepting
      301 and D refusing it), `transcode_reports_capacity` (match the variant
      only, not `needed`). Compare with `.map(|r| r.bytes())` because `Ref` has
      no `Debug`.
- [ ] **Step 2: Run** `cargo test -p ridl-rt transcode` — expected: FAIL, not
      found.
- [ ] **Step 3: Implement** the enum, the two `From` impls and the function
      (body as in the spec), with doc comments that say it is the
      domain-mediated reference path and that a generated function may replace
      it.
- [ ] **Step 4: Run** `cargo test -p ridl-rt` and
      `cargo test -p ridl-rt --no-default-features` — expected: PASS.
- [ ] **Step 5: Run** `cargo xtask freeze check` — expected: `runtime-crates`
      passes (additive).
- [ ] **Step 6: Commit**
      `feat(ridl-rt): add a reference transcode between payload encodings`.

### Task 2: the manifest key and MANI-020

**Files:**

- Modify: `crates/ridl-core/src/manifest.rs` (`TRANSCODE_KEY`, field
  `transcode: bool` on `BackendTable`, parsing beside `wasm` at ~line 523/602,
  the MANI-020 check after the MANI-019 loop at ~line 679, the key list at
  ~line 820)
- Modify: `crates/ridl-core/src/diag.rs` (`MANI_020` after `MANI_019`)
- Modify: `crates/ridlc/src/lib.rs` (send the option, beside `WASM_KEY` at
  ~line 3033)
- Modify: `freeze/manifest.txt`, `freeze/diagnostics.txt` (regenerate with
  `cargo xtask freeze update`)
- Modify: `docs/book/cli-reference.md` (the key and MANI-020, beside the `wasm`
  text at ~line 877)

**Interfaces:**

- Produces: `ridl_core::manifest::TRANSCODE_KEY: &str = "transcode"`;
  `BackendTable::transcode: bool`; `DiagCode::MANI_020`, Error, summary "a
  backend table sets `transcode = true` with fewer than two encodings"; backend
  option `transcode` with value `true`, sent only when the key is `true`.

- [ ] **Step 1: Write the failing tests** in `manifest.rs`'s test module:
      `backend_transcode_parses_as_a_boolean`,
      `backend_transcode_not_a_boolean_is_mani_015`,
      `backend_transcode_with_no_encodings_is_mani_020` (key absent, and `[]`),
      `backend_transcode_with_one_encoding_is_mani_020`,
      `backend_transcode_with_two_encodings_has_no_diagnostic`,
      `backend_transcode_with_a_repeated_encoding_is_mani_015` (no MANI-020 on
      top). MANI-020 is reported on the `transcode` value's span. In `ridlc`, a
      test that the option list carries `transcode=true` for a manifest that
      sets it and carries no `transcode` option otherwise (follow the existing
      `wasm` option tests near line 3457).
- [ ] **Step 2: Run** `cargo test -p ridl-core backend_transcode` and the
      `ridlc` test — expected: FAIL.
- [ ] **Step 3: Implement** the key, the check, the catalogue entry with a doc
      comment in the MANI-019 style, and the option.
- [ ] **Step 4: Run** `cargo test -p ridl-core -p ridlc` — expected: PASS (the
      catalogue drift test included).
- [ ] **Step 5: Run** `cargo xtask freeze update`, then
      `cargo xtask freeze check` — expected: only `manifest.txt` and
      `diagnostics.txt` change, by added lines.
- [ ] **Step 6: Update** `docs/book/cli-reference.md`.
- [ ] **Step 7: Commit**
      `feat(manifest): add the transcode key of the Rust backend table`.

### Task 3: the backend emits the functions

**Files:**

- Modify: `crates/ridl-backend-rust/src/contract.rs` (`TRANSCODE_OPTION`,
  parsing as for `WASM_OPTION`, the unknown-option message lists it)
- Modify: `crates/ridl-backend-rust/src/lib.rs` (`generate_pipeline` and
  `generate_pipeline_over` take `transcode: bool`; update their callers)
- Create: `crates/ridl-backend-rust/src/transcode.rs` (the emitter)
- Modify: `crates/ridl-backend-rust/src/tests.rs`
- Modify: `docs/book/generated-code.md` (a section on the functions and the key)

**Interfaces:**

- Consumes: `ridl_rt::payload::{transcode, TranscodeError, Ref}` (Task 1); the
  `transcode` option (Task 2).
- Produces: `ridl_backend_rust::contract::TRANSCODE_OPTION: &str = "transcode"`;
  `generate_pipeline(package, encodings, transcode: bool, others)`; emitted
  items of the form shown in the spec, section `ridl-backend-rust`, paths fully
  qualified (`::ridl_rt::…`), each with a doc comment naming the source and
  target encodings and linking `::ridl_rt::payload::transcode`.

- [ ] **Step 1: Write the failing tests** in `tests.rs`, over a package with a
      struct `Speed` (both impls) and a type that only FlatBuffers roots:
      `transcode_emits_both_directions` (source contains
      `fn transcode_flatbuffers_to_proto3` and
      `fn transcode_proto3_to_flatbuffers` inside `impl Speed`),
      `transcode_skips_a_type_with_one_encoding`, `transcode_off_emits_nothing`
      (no `transcode_` in the source),
      `transcode_with_one_encoding_emits_nothing`, and in `contract.rs` tests
      `transcode_option_value_not_boolean_is_refused`.
- [ ] **Step 2: Run** `cargo test -p ridl-backend-rust transcode` — expected:
      FAIL.
- [ ] **Step 3: Implement** the option and the emitter. Decide "implements
      `Payload` under E" with the same predicate the FlatBuffers and proto3
      codec emitters use to emit their `impl Payload` (`codec.rs`,
      `codec_proto3.rs` ~line 2853); do not write a second rule.
- [ ] **Step 4: Run** `cargo test -p ridl-backend-rust` — expected: PASS, and
      the checked-in fixtures under `tests/generated/` unchanged (the
      regeneration guards pass, because they do not set the key).
- [ ] **Step 5: Update** `docs/book/generated-code.md`.
- [ ] **Step 6: Commit**
      `feat(backend-rust): emit transcode functions when the manifest asks`.

### Task 4: the equivalence test and the records

**Files:**

- Create: `crates/ridl-backend-rust/tests/fixtures/transcode/` (a package with a
  struct, an enum and a union, each rooted by both encodings, and one type
  rooted only by FlatBuffers)
- Create: `crates/ridl-backend-rust/tests/generated/transcode.rs` (generated
  output, with `flatbuffers`, `proto3`, `transcode = true`)
- Create: `crates/ridl-backend-rust/tests/transcode_equivalence.rs`
- Modify: `crates/ridl-backend-rust/tests/interaction_face_regeneration.rs` or
  create a sibling guard, following its pattern, so the fixture is regenerated
  and compared
- Modify: `docs/decisions/ADR-0018-runtime-core-and-generated-surface.md` (a
  dated note under decision 13)

**Interfaces:**

- Consumes: Tasks 1 and 3.

- [ ] **Step 1: Generate** the fixture with
      `generate_pipeline(…, transcode: true, …)` and add the regeneration guard;
      run it — expected: PASS.
- [ ] **Step 2: Write the tests** in `transcode_equivalence.rs`, `include!`ing
      the fixture the way `interaction_face.rs` does: for each of at least three
      values per type (including each union arm and the bounds of each range),
      `T::transcode_proto3_to_flatbuffers(Ref::<T, Proto3>::encode(&v, a)?.bytes(), b)?.bytes()`
      equals `Ref::<T, FlatBuffers>::encode(&v, c)?.bytes()`, and the reverse
      direction likewise. Also assert that the FlatBuffers-only type has no
      `transcode_` function (a `compile_fail` doc test or a text check on the
      fixture).
- [ ] **Step 3: Run**
      `cargo test -p ridl-backend-rust --test transcode_equivalence` — expected:
      PASS. A byte difference is a defect in an encoder: stop and report it, do
      not change the test to accept it.
- [ ] **Step 4: Write** the ADR-0018 decision 13 note (dated 2026-10-23): the
      domain-mediated path ships as `ridl_rt::payload::transcode`, and
      `[backend.rust] transcode = true` generates `transcode_<s>_to_<d>`
      functions whose bodies a later streaming transcoder replaces; this test is
      the equivalence check that transcoder must pass.
- [ ] **Step 5: Commit**
      `test(backend-rust): check transcode against direct encoding`.

After Task 4: garden the design and this plan with the `sdd-gardening` skill
before the PR merges.
