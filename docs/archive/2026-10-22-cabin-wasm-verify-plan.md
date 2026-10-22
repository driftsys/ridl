# Cabin Wasm Verify Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Deno demo and the browser demo of `examples/cabin/web` verify each
`temperature` payload through the cabin wasm codec before they decode it, and
both show that a payload with a value outside `[-40..85]` is rejected with a
rule code.

**Architecture:** `examples/cabin/ridl.toml` turns on
`[backend.rust] wasm =
true`. A private recipe builds the cabin crate as a wasm
`cdylib` into `examples/cabin/web/generated/veh_cabin.wasm`. `check.ts` gains a
verify step that takes a `Codec` from `@driftsys/ridl-runtime` (imported by
relative path, as the demos import `ridl-transport-ws`). Both runners load the
codec with the catalog hash the server announces, verify every payload, and run
a self-test on two payloads built with the `flatc` builder (-41 and 86). The
loader in `packages/ridl-runtime` treats `ridl_alloc` returning 0 for a length
above 0 as out of memory.

**Tech Stack:** Rust
(`cargo rustc --target wasm32-unknown-unknown
--crate-type cdylib --features wasm-exports`),
Deno, `flatc` 25.12.19, the `flatbuffers` npm package, `packages/ridl-runtime`.

**Spec:** [the design note](../wip/2026-10-21-ts-wasm-codec-design.md), D-2
(export contract, return codes, the out-of-memory rule), D-6 (size settings come
from the environment), D-7 (the end-to-end proof), and "Plan change
(2026-10-22)" step 4.

## Global Constraints

- `ridl_verify` codes (D-2): 0 ok, 1 malformed, -1 unknown type id, 2 or more a
  typl rule; the rule order is
  `length, pattern, range, step, unique,
  variant`, so a range violation is
  code 4.
- `ridl_alloc` returns 0 for a length of 0 and when the allocation fails; the
  loader treats 0 with a length above 0 as out of memory (D-2).
- The wasm build takes `opt-level = "z"`, `lto = true` and `panic = "abort"`
  from `CARGO_PROFILE_RELEASE_OPT_LEVEL=z`, `CARGO_PROFILE_RELEASE_LTO=true` and
  `CARGO_PROFILE_RELEASE_PANIC=abort`, not from a `Cargo.toml` (D-6).
- The public TypeScript API is plain objects only; `flatc` accessors and
  builders stay inside `check.ts` functions.
- `check.ts` and `page.ts` use no Deno API.
- The pass condition of #1022 and #1023 is unchanged; verification adds a
  failure cause, it does not replace the range check in `TemperatureCheck`.
- A gate command lives only in the `justfile` (ADR-0009). Prose is plain and
  literal; no story ids in `examples/` or `packages/`.

## Review Focus

1. A wasm file built from another catalog: `loadCodec` throws `CatalogMismatch`,
   and the runner fails naming both hashes. Task 2 runs it once by hand with a
   wrong hash.
2. An empty payload: `verify(typeId, new Uint8Array(0))` must not be treated as
   out of memory (alloc(0) is 0 by design). Task 1 tests it.
3. The type id table changes order when cabin gains a payload type: the hand map
   must fail loudly, not verify against the wrong type. Task 2 checks that a
   correct 20 verifies ok and a -41 gives code 4, which would not both hold for
   a wrong type id.
4. The browser cannot fetch the wasm (wrong path or MIME type): the page rejects
   with the fetch error, not a deadline. Task 2 serves it with
   `application/wasm`.
5. Turning on `wasm = true` adds `__ridl_wasm.rs` to the generated cabin crate;
   the `demo` recipe's no-std check, clippy under `-D warnings` and `cargo doc`
   must stay green. Task 2 runs `just demo`.

## Delegated decisions

- P-1. The rejection is shown on payloads built in TypeScript, not sent by the
  server. The generated publisher refuses to encode an out-of-range
  `Temperature`, even one from `new_unchecked`, so a server that sends one would
  need a raw-bytes path that exists only for the demo. D-7 asks that "a payload
  with a value outside `[-40..85]` is rejected", which this meets.
- P-2. `wasm = true` goes in `examples/cabin/ridl.toml` rather than in a copy of
  the manifest, so the example is what a user would write.
- P-3. The type id of `TemperatureBox` is a hand-written constant in `check.ts`,
  until the generated codec table (step 5) replaces it.
- P-4. No `wasm-codec-demo` recipe: the build is a private recipe that both demo
  recipes run, so D-6's "a recipe does both steps" is met by `cabin-ws-demo` and
  `cabin-browser-demo`.

---

### Task 1: The loader rejects a failed allocation

**Files:**

- Modify: `packages/ridl-runtime/src/codec.ts` (the `alloc` calls at the load
  path and in `verify`)
- Test: the existing loader tests under `packages/ridl-runtime/tests/` (add a
  WAT fixture whose `ridl_alloc` always returns 0)

**Interfaces:**

- Produces: `CodecError` with message containing `out of memory` and
  `exportName: "ridl_alloc"`, thrown from `loadCodec` (hash buffer) and from
  `Codec.verify` (payload buffer) when `ridl_alloc(n)` returns 0 for `n > 0`.

- [ ] **Step 1:** Write the tests: `loadCodec` on the always-0 fixture rejects
      with `CodecError` (`exportName === "ridl_alloc"`, message contains
      `out of memory`); on a fixture whose alloc fails only above 32 bytes,
      `verify(0, new Uint8Array(64))` throws the same;
      `verify(0, new
  Uint8Array(0))` on a normal fixture does not throw.
- [ ] **Step 2:** Run `deno test` in `packages/ridl-runtime`. Expected: the two
      out-of-memory tests fail.
- [ ] **Step 3:** Implement the check in `codec.ts`.
- [ ] **Step 4:** `just ts-check` passes.
- [ ] **Step 5:** Mutate: remove each check; a test fails. Restore.
- [ ] **Step 6: Commit**
      `fix(ts-runtime): treat a failed wasm allocation as
  out of memory`

### Task 2: Verify every payload in both demos

**Files:**

- Modify: `examples/cabin/ridl.toml` (`[backend.rust]` gains `wasm = true`)
- Modify: `justfile` — a private recipe `_cabin-wasm` that builds the wasm with
  the D-6 environment and copies it to
  `examples/cabin/web/generated/veh_cabin.wasm`; `_cabin-web-generate` (or both
  demo recipes) runs it; the recipe comments say what is built.
- Modify: `examples/cabin/web/check.ts` — `TEMPERATURE_TYPE_ID` (the index of
  `veh.cabin.TemperatureBox` in `WASM_PAYLOAD_TYPES`; read it from the generated
  `__ridl_wasm.rs` and say so in the comment), `RANGE_RULE = 4`,
  `encodeTemperature(value: number): Uint8Array` (the `flatc` builder, no range
  check), and
  `verifyPayload(codec: Codec, payload: Uint8Array):
  string | undefined`
  returning a cause such as `the codec rejected the
  payload: rule 4 (range)`
  or `undefined` when ok.
- Modify: `examples/cabin/web/check_test.ts` — tests that need the built wasm.
- Modify: `main.ts`, `page.ts`, `browser.ts` — load the codec (`main.ts` reads
  the file; `page.ts` fetches `/veh_cabin.wasm`, which `browser.ts` serves as
  `application/wasm`) with the announced hash; before `TemperatureCheck` sees a
  value, run the self-test (-41 and 86 must give `kind: "rule"`,
  `code: RANGE_RULE`; 20 must give `ok`), then verify every received payload and
  fail with the cause on a rejection.
- Modify: `AGENTS.md`, `CONTRIBUTING.md`, `README.md` recipe rows; the design
  note's step 4 line marked landed.
- Check: `crates/ridlc/tests/wasm_module.rs` still passes (it appends
  `wasm = true` to the cabin manifest; if a duplicate key now breaks it, make it
  not append when the key is present).

**Interfaces:**

- Consumes: `loadCodec(source, { catalogHash })`,
  `Codec.verify(typeId,
  bytes): VerifyResult` from
  `packages/ridl-runtime/mod.ts`; Task 1's out-of-memory `CodecError`; `isPass`,
  `TemperatureCheck`, `hashFromHex` from `check.ts`.

- [ ] **Step 1:** Write the `check_test.ts` tests against the built wasm:
      `encodeTemperature(20)` verifies ok; -41 and 86 give
      `{ ok: false, kind:
  "rule", code: 4 }`; a truncated payload (first 4
      bytes) gives `kind:
  "malformed"`; `verifyPayload` returns `undefined`
      for 20 and a cause naming rule 4 for -41.
- [ ] **Step 2:** Run them. Expected: fail (no wasm, no functions).
- [ ] **Step 3:** Turn on `wasm = true`, add `_cabin-wasm`, write the `check.ts`
      functions; the tests pass.
- [ ] **Step 4:** Wire `main.ts`, `page.ts`, `browser.ts`;
      `just
  cabin-ws-demo` and `just cabin-browser-demo` pass and each prints
      that the self-test rejected -41 and 86 with rule 4.
- [ ] **Step 5: Failure paths**, each exits non-zero naming the cause; restore
      after each: a wrong catalog hash passed to `loadCodec`
      (`CatalogMismatch`); `TEMPERATURE_TYPE_ID` off by one; `RANGE_RULE` set to
      3; the wasm file missing.
- [ ] **Step 6:** `just demo`, `cargo test -p ridlc --test wasm_module`,
      `just
  freeze-check`, `just gate-parity`, `just check`,
      `just link-check`, `just
  story-id-check`.
- [ ] **Step 7: Commit**
      `feat(repo): verify the cabin temperature through the
  wasm codec in both demos`
