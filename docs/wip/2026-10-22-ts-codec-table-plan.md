# The generated TypeScript codec table — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** `ridl build` writes `<base>.codec.ts`, a plain-object codec and a
member table over `flatc`'s generated code, so that `examples/cabin/web` and a
user's package address an interaction and encode, verify and decode its payload
without a hand-written map.

**Architecture:** `ridl-backend-ts` gains a module `codec.rs` that reads the
request's model (interfaces, induced tuples, FlatBuffers roots, catalog) and
writes the file when `[backend.typescript] codec = true`. The wasm module gains
one export, `ridl_type_id`, and `@driftsys/ridl-runtime` resolves each payload
type's id by name through it, so the TypeScript never states an id. The cabin
demos switch to the generated table last.

**Tech Stack:** Rust (the backends, `ridlc`, `ridl-core`), TypeScript under Deno
(`packages/ridl-runtime`, `examples/cabin/web`), `flatc` at the version in
`.flatc-version`, `insta` snapshots, Node for the wasm host test.

**Spec:**
[2026-10-22-ts-codec-table-design.md](2026-10-22-ts-codec-table-design.md), over
[2026-10-21-ts-wasm-codec-design.md](2026-10-21-ts-wasm-codec-design.md).

## Global Constraints

Copied from the design note; every task's requirements include them.

- "The public API is plain objects (owner's rule). For every payload type of the
  package — every FlatBuffers root of the package that has a finite bound, which
  is exactly the set with a `Payload<FlatBuffers>` impl and an entry in
  `WASM_PAYLOAD_TYPES` — the file exports
  `encode<Name>(value: Name): Uint8Array` and
  `decode<Name>(bytes: Uint8Array): Name`, where `Name` is the TypeScript type
  of `<base>.ts`. The `flatc` classes, builders and `XBox` tables never appear
  in a signature." (D-2)
- "The wrappers go through the `flatc` object API, with one private converter
  pair per declaration." (D-3)
- "A wasm type id is resolved by name at load, not written as a number into the
  TypeScript." The export is `ridl_type_id(name_ptr, name_len) -> i32`, "the
  index in `WASM_PAYLOAD_TYPES` of the UTF-8 name at `name_ptr`, or -1 when the
  list does not hold it". "The export is additive: the four existing exports and
  their codes do not change." (D-5)
- "The TypeScript backend spells a payload type as the wasm module does:
  `<Scope.package>.<declared name>` for a declaration, and
  `<Scope.package>.<InducedName.rust>` for an induced tuple." (D-6)
- "`decode<Name>` does not verify, and `encode<Name>` does not check a
  constraint." (D-11)
- "`ridlc` reports an error when `[backend.typescript] codec = true` is set
  without `[backend.rust] wasm = true`"; "The reverse is not required." (D-12)
- "The refusal covers a foreign declaration that needs a converter or `flatc`'s
  output for its package: a struct, a union and an enum. A foreign named scalar
  and a foreign enum set need neither". (D-14)
- "Every import of the codec file carries the `.js` suffix". (D-15)
- "`ridlc` does not run `flatc`". (D-10)
- `ridl-rt` stays free of per-package types and of a dependency; the generated
  crate and the manifest are frozen surfaces (ADR-0029): an added line passes, a
  removed line is a break.
- Prose in code comments, commit messages and docs is plain and literal; no
  story id in a shipped file (`just story-id-check`).
- Commits are Conventional Commits with the scopes `.git-std.toml` lists, and
  end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. A name the module does not hold, passed to `Codec.typeId` after a load that
   gave no `payloadTypes`: the user expects an `UnknownPayloadType` naming it,
   not a `verify` of type id -1 that reports `unknown-type` later. Pinned in
   Task 2, `typeId throws UnknownPayloadType with the name`.
2. A `ridl_type_id` that returns an id for a prefix of a name
   (`veh.cabin.Temp`): the user expects -1. Pinned in Task 1, the Node host's
   `prefix` case.
3. A struct field whose enum has no zero member and is absent in the buffer
   (`= null` in the `.fbs`): the user expects `decode<Name>` to throw, not to
   return `NaN` as the enum. Pinned in Task 5,
   `an absent required enum field throws on decode`.
4. A call whose parameter list induces a tuple: the user expects
   `encode<InducedName.rust>` over a plain object with the parameter names, not
   `field_1`. Pinned in Task 5, `a two-parameter call round-trips by name`.
5. A manifest with `codec = true` on the `rust` table: the user expects a
   refusal that names the key, not a silent no-op. Pinned in Task 3,
   `codec_on_the_rust_table_is_refused_by_the_rust_backend`.

---

### Task 1: The `ridl_type_id` export of the wasm module

**Files:**

- Modify: `crates/ridl-backend-rust/src/wasm.rs` (the module doc, the render in
  `generate_wasm_module`)
- Test: `crates/ridl-backend-rust/tests/descriptor_generation.rs`
  (`wasm_module_lists_the_payload_types_sorted`)
- Test: `crates/ridlc/tests/wasm_module.rs` (`NATIVE_PROGRAM`, `NODE_HOST`,
  `the_cabin_wasm_module_verifies_payloads_in_a_node_host`)
- Modify: `docs/book/generated-code.md` (the `wasm-exports` section lists the
  exports)

**Interfaces:**

- Consumes: `WASM_PAYLOAD_TYPES: &[&str]`, `verify_by_id`, the four exports as
  built.
- Produces, in the generated module:
  - `#[doc(hidden)] pub fn type_id_of(name: &str) -> i32`: the position of
    `name` in `WASM_PAYLOAD_TYPES` compared byte by byte and whole, or -1.
    Compiled for every target.
  - `#[unsafe(no_mangle)] pub unsafe extern "C" fn ridl_type_id(name_ptr: *const u8, name_len: usize) -> i32`
    under `#[cfg(all(target_arch = "wasm32", feature = "wasm-exports"))]`:
    returns -1, without reading, when `name_len > 0` and `name_ptr` is null, or
    `name_len > isize::MAX as usize`, or `name_ptr as usize + name_len`
    overflows; otherwise reads the `name_len` bytes and returns `type_id_of` of
    them, or -1 when they are not UTF-8. A `name_len` of 0 is the empty name,
    which no list holds: -1.

- [ ] **Step 1: Extend the unit test**

In `wasm_module_lists_the_payload_types_sorted`, change the expected export list
to
`["ridl_alloc", "ridl_catalog_hash", "ridl_free", "ridl_type_id", "ridl_verify"]`
and the `no_mangle` count to 5, and add
`assert!(d.contains("pubfntype_id_of(name:&str)->::core::primitive::i32"), "got:\n{source}");`.

- [ ] **Step 2: Run it to see it fail**

Run:
`cargo test -p ridl-backend-rust --test descriptor_generation wasm_module_lists_the_payload_types_sorted`
Expected: FAIL on the export list.

- [ ] **Step 3: Render `type_id_of` and `ridl_type_id` in
      `generate_wasm_module`**

`type_id_of` sits beside `verify_by_id` with a doc comment of the same form
("Not a stable API."); `ridl_type_id` sits in `mod exports` after `ridl_verify`,
with a `# Safety` section: "`name_ptr` is valid for a read of `name_len` bytes,
or `name_len` is 0." Update the module doc's export sentence to name five
exports.

- [ ] **Step 4: Run the unit test to see it pass**

Run: `cargo test -p ridl-backend-rust --test descriptor_generation` Expected:
PASS.

- [ ] **Step 5: Extend the Node host test**

In `NATIVE_PROGRAM`, after the `types` line, print one line
`typeid <name> <type_id_of(name)>` per entry of `WASM_PAYLOAD_TYPES`, and
`typeid veh.cabin.Nothing <type_id_of("veh.cabin.Nothing")>`. In `NODE_HOST`,
destructure `ridl_type_id`, and for each argument of the form `name:<text>`
write the UTF-8 bytes of `<text>` through `ridl_alloc`, call
`ridl_type_id(ptr, len)`, free, and push the code into `results`; add to the
JSON `typeIdNull: ridl_type_id(0, 0)` and `typeIdEmpty: ridl_type_id(ptr, 0)`
for a `ptr` from `ridl_alloc(1)`. In the test, pass
`name:veh.cabin.Temperature`, `name:veh.cabin.Temp` and
`name:veh.cabin.Nothing`, and assert: the first equals the index of
`veh.cabin.Temperature` in `types` and equals the native `typeid` line's value;
the other two are -1; `typeIdNull` and `typeIdEmpty` are -1; the `exports` list
is the memory plus the five functions; every native `typeid` line's value equals
the index of its name, and the `Nothing` line is -1.

- [ ] **Step 6: Run the host test**

Run: `cargo test -p ridlc --test wasm_module` Expected: PASS (needs `node` and
the `wasm32-unknown-unknown` target).

- [ ] **Step 7: Add the export to the book's export list and commit**

In `docs/book/generated-code.md`, the `wasm-exports` section: add `ridl_type_id`
with one sentence ("returns the type id of a payload type named by its qualified
name, or -1").

```bash
git add crates/ridl-backend-rust/src/wasm.rs crates/ridl-backend-rust/tests/descriptor_generation.rs crates/ridlc/tests/wasm_module.rs docs/book/generated-code.md
git commit -m "feat(ridl-backend-rust): export ridl_type_id from the wasm module"
```

**Semantic mutations the tests must kill:** `position + 1` instead of `position`
(the index assertion); `starts_with` instead of equality (the `veh.cabin.Temp`
case); 0 instead of -1 for an unknown name (the `Nothing` case); the
null-pointer guard removed (`typeIdNull` traps or reads garbage).

---

### Task 2: `typeId`, `payloadTypes` and the table types in `@driftsys/ridl-runtime`

**Files:**

- Modify: `packages/ridl-runtime/src/codec.ts`
- Modify: `packages/ridl-runtime/src/errors.ts`
- Modify: `packages/ridl-runtime/mod.ts`
- Modify: `packages/ridl-runtime/tests/fixture.ts`
- Test: `packages/ridl-runtime/tests/codec_test.ts`
- Modify: `packages/ridl-runtime/README.md` (the API section, the Limits list)

**Interfaces:**

- Consumes: the export of Task 1, by its ABI only.
- Produces (all exported from `mod.ts`):

```ts
export class UnknownPayloadType extends Error {
  readonly type: string; // name = "UnknownPayloadType"
  constructor(type: string); // message: `the wasm module has no payload type "${type}"`
}
export type MemberKind = "signal" | "event" | "command" | "query" | "fixed";
export interface PayloadCodec<T> {
  readonly type: string;
  encode(value: T): Uint8Array;
  decode(bytes: Uint8Array): T;
}
export interface MemberRef {
  readonly iface: number;
  readonly ord: number;
  readonly kind: MemberKind;
}
export interface LoadOptions {
  catalogHash: Uint8Array;
  payloadTypes?: readonly string[];
  ruleText?: readonly string[];
  imports?: CodecImports;
}
export interface Codec {
  readonly catalogHash: Uint8Array;
  typeId(name: string): number;
  verify(typeId: number, bytes: Uint8Array): VerifyResult;
}
```

`loadCodec` reads the export `ridl_type_id` with `fn(exports, "ridl_type_id")`,
so a module without it throws `CodecError` with `exportName` `ridl_type_id` at
load. With `payloadTypes`, it resolves each name in order before it returns and
rejects with `UnknownPayloadType` of the first miss. `typeId(name)` returns the
cached id when the name was resolved, and otherwise encodes `name` with
`TextEncoder`, allocates `bytes.length` bytes (a length of 0 calls
`ridl_type_id(0, 0)` and allocates nothing), copies, calls, frees in `finally`,
caches a result of 0 or more in a `Map<string, number>`, throws
`UnknownPayloadType(name)` for -1, and throws `CodecError(\`ridl_type_id
returned the invalid code ${code}\`)` for a code below -1.

- [ ] **Step 1: Extend the fixture**

In `fixture.ts`, add the options `typeId?: boolean` (default true; false leaves
the export out) and `badTypeId?: boolean` (returns -5 for the name `b`), and the
function:
`(func (export "ridl_type_id") (param $p i32) (param $l i32) (result i32) ...)`
which counts in `$ops` like `ridl_free` does, returns -1 when `$l` is not 1,
returns 0 when the byte at `$p` is 0x61 (`a`), 1 when it is 0x62 (`b`), and -1
otherwise. Update the file's header comment.

- [ ] **Step 2: Write the failing tests**

In `codec_test.ts`, with the existing helpers:

- `typeId resolves a name through ridl_type_id`: `typeId("a") === 0`,
  `typeId("b") === 1`.
- `typeId throws UnknownPayloadType with the name`: `typeId("c")` throws an
  `UnknownPayloadType` whose `type === "c"` and whose message is
  `the wasm module has no payload type "c"`.
- `payloadTypes are resolved at load and a miss rejects`:
  `loadCodec(bytes, { catalogHash: HASH, payloadTypes: ["a", "c"] })` rejects
  with `UnknownPayloadType` of type `c`; with `["a", "b"]` it resolves, and
  `typeId("a") === 0`.
- `typeId is resolved once per name`: read the exported global `allocs`, call
  `typeId("a")` twice, assert `allocs` grew by 1.
- `typeId frees the name buffer`: after one `typeId("a")`, `frees` grew by 1 and
  `freeLen === 1`.
- `typeId of an empty name throws UnknownPayloadType and allocates nothing`:
  `typeId("")` throws `UnknownPayloadType` of type `""`, and `allocs` did not
  grow.
- `a module without ridl_type_id is a CodecError naming it`:
  `loadCodec(fixtureBytes({ typeId: false }), …)` rejects with `CodecError`
  whose `exportName === "ridl_type_id"`.
- `a typeId code below -1 is a CodecError`: with `badTypeId: true`,
  `typeId("b")` throws `CodecError` whose message is
  `ridl_type_id returned the invalid code -5`.

- [ ] **Step 3: Run them to see them fail**

Run:
`cd packages/ridl-runtime && deno test --allow-read --allow-write --allow-net --allow-env tests/`
Expected: the eight new tests FAIL (`typeId is not a function`, a missing
export).

- [ ] **Step 4: Implement the interfaces block in `codec.ts`, `errors.ts` and
      `mod.ts`**

- [ ] **Step 5: Run the package checks**

Run: `just ts-check` Expected: PASS (`deno fmt --check`, `deno lint`, both
`deno check`, the tests, `deno publish --dry-run`).

- [ ] **Step 6: Update the README and commit**

README: the API block shows `payloadTypes` in the options and
`codec.typeId(name)`; the bullet list gains `Codec.typeId`, `payloadTypes` and
`UnknownPayloadType`; the Limits list names `ridl_type_id` among the required
exports.

```bash
git add packages/ridl-runtime
git commit -m "feat(packages): resolve a payload type id by name in ridl-runtime"
```

**Semantic mutations the tests must kill:** the cache removed (`allocs` grows by
2); -1 mapped to 0 (`c` resolves); the `free` skipped (`frees` unchanged);
`payloadTypes` ignored at load (the `["a", "c"]` load resolves); the length-0
guard removed (`allocs` grows on `""`).

---

### Task 3: The `codec` manifest key, its validation and its delivery

**Files:**

- Modify: `crates/ridl-core/src/manifest.rs` (`CODEC_KEY`, `BackendTable.codec`,
  `RawBackend.codec`, the parse, the cross-table check, tests)
- Modify: `crates/ridl-core/src/diag.rs` (`MANI_019`)
- Modify: `crates/ridlc/src/lib.rs` (`codec_backends`, `backend_options`, tests)
- Modify: `crates/ridl-backend-ts/src/contract.rs` (`CODEC_OPTION`, the option
  loop)
- Modify: `crates/ridl-backend-ts/src/tests.rs` (option tests)
- Modify: `crates/ridl-backend-rust/tests/descriptor_generation.rs` (one refusal
  case)
- Modify: `xtask/src/freeze/manifest.rs`, `freeze/manifest.txt`,
  `freeze/diagnostics.txt`
- Modify: `docs/book/cli-reference.md` (the manifest paragraph that documents
  `wasm`, at the line that begins "`[backend.rust] wasm = true` adds a module")

**Interfaces:**

- Produces:
  - `ridl_core::manifest::CODEC_KEY: &str = "codec"`;
    `BackendTable { encodings, wasm, codec: bool }`.
  - `DiagCode::MANI_019`, code `MANI-019`, severity `Error`, catalogue text "a
    backend table sets `codec = true` without `[backend.rust] wasm = true`".
    Raised by the manifest parser on the span of the `codec` value, for every
    table whose `codec` is `true` when the `rust` table is absent or its `wasm`
    is not `true`, with the message
    `` `[backend.<name>] codec = true` needs `[backend.rust] wasm = true`: the codec table is generated beside the wasm module ``.
  - `ridlc`:
    `backend_options(backends, wasm_backends, codec_backends, language)` pushes
    `BackendOption { key: "codec", value: "true" }` after the `wasm` option when
    `codec_backends` holds `language`; `codec_backends` is computed beside
    `wasm_backends` from `table.codec`.
  - `ridl_backend_ts::contract::CODEC_OPTION: &str = "codec"`; the backend
    accepts `codec` with the value `true` or `false`, refuses another value with
    `` `codec` is `<other>`; the value is `true` or `false` ``, and refuses
    another key with
    ``the typescript backend takes the option `codec`; `<key>` is not one it knows``.
    With `codec = true` the backend records the flag; Task 4 acts on it.

- [ ] **Step 1: Write the failing manifest tests**

In `manifest.rs` `tests`, beside the `wasm` tests:

- `backend_codec_parses_as_a_boolean`:
  `[backend.rust] encodings = ["flatbuffers"]\nwasm = true\n[backend.typescript]\ncodec = true`
  parses with no diagnostic and `backends["typescript"].codec`; `codec = false`
  and an absent key give `false`.
- `backend_codec_not_a_boolean_is_mani_015`:
  `[backend.typescript]\ncodec = "yes"` gives `["MANI-015"]` with the message
  `` `[backend.typescript] codec` holds `"yes"`, which is not a boolean `` and
  the span text `"yes"`.
- `backend_codec_without_rust_wasm_is_mani_019`:
  `[backend.typescript]\ncodec = true` alone gives `["MANI-019"]`, the message
  above with `<name>` = `typescript`, and the span text `true`; the same with
  `[backend.rust]\nencodings = ["flatbuffers"]` and no `wasm` gives
  `["MANI-019"]`.
- `backend_codec_on_any_table_needs_rust_wasm`: `[backend.kotlin]\ncodec = true`
  alone gives `["MANI-019"]` naming `kotlin`.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p ridl-core manifest::tests::backend_codec` Expected: FAIL
(`codec` is not a field; no `MANI_019`).

- [ ] **Step 3: Implement the key, the parse and `MANI_019`**

The parse mirrors `wasm` in `RawBackend`, the visitor and the table loop. The
cross-table check runs after every table is read, in the function that builds
the `backends` map.

- [ ] **Step 4: Run the manifest tests to see them pass**

Run: `cargo test -p ridl-core manifest` Expected: PASS.

- [ ] **Step 5: Write the failing delivery tests**

- `ridlc` (`crates/ridlc/src/lib.rs` tests, beside the `encodings("typescript")`
  helper): `backend_options_carries_codec_for_the_table_that_sets_it`: with
  `codec_backends = {"typescript"}` and `wasm_backends = {"rust"}`,
  `backend_options(…, "typescript")` is `[("codec", "true")]` and
  `backend_options(…, "rust")` has no `codec` entry.
- `ridl-backend-ts` `tests.rs`:
  `the_codec_option_is_accepted_and_other_keys_are_refused`: a request with
  `options = [BackendOption { key: "codec", value: "true" }]` gives a response
  with no diagnostic and one file `<base>.ts`; `value: "yes"` gives one error
  diagnostic with the message
  `` `codec` is `yes`; the value is `true` or `false` ``; `key: "wasm"` gives
  ``the typescript backend takes the option `codec`; `wasm` is not one it knows``.
- `ridl-backend-rust` `descriptor_generation.rs`:
  `codec_on_the_rust_table_is_refused_by_the_rust_backend`:
  `refused_options(vec![option("encodings", "flatbuffers"), option("codec", "true")])`
  contains `` `codec` is not one it knows ``.

- [ ] **Step 6: Run them to see them fail, implement, run to see them pass**

Run:
`cargo test -p ridlc backend_options_carries_codec && cargo test -p ridl-backend-ts the_codec_option && cargo test -p ridl-backend-rust --test descriptor_generation codec_on_the_rust_table`
Expected: FAIL, then PASS after the `ridlc` and `contract.rs` changes.

- [ ] **Step 7: Update the frozen manifest dump and the diagnostics dump**

Add `"backend.*.codec boolean"` to the dumper in `xtask/src/freeze/manifest.rs`
and to its unit test's expected list, then run `cargo xtask freeze update`.
Expected: `freeze/manifest.txt` gains the line `backend.*.codec boolean` and
`freeze/diagnostics.txt` gains `MANI-019 error`; `just freeze-check` passes
(both are added lines).

- [ ] **Step 8: Document the key and commit**

In `docs/book/cli-reference.md`, after the `wasm` paragraph, one paragraph:
`[backend.typescript] codec = true` writes `<base>.codec.ts` beside the
TypeScript module (Task 4 fills in what it holds); it needs
`[backend.rust] wasm = true`, and MANI-019 reports a manifest that sets it
without.

```bash
git add crates/ridl-core crates/ridlc/src/lib.rs crates/ridl-backend-ts crates/ridl-backend-rust/tests/descriptor_generation.rs xtask/src/freeze/manifest.rs freeze docs/book/cli-reference.md
git commit -m "feat(ridlc): add the codec key of a backend table"
```

**Semantic mutations the tests must kill:** `MANI_019` raised when `wasm` is
`true` (the accepted case fails); the check reading `backends["typescript"]`
only (the `kotlin` case); `backend_options` passing `codec` to every language
(the `rust` assertion); the TypeScript backend accepting any value (the `yes`
case).

---

### Task 4: The codec file for cabin's kinds

**Files:**

- Create: `crates/ridl-backend-ts/src/codec.rs`
- Modify: `crates/ridl-backend-ts/src/lib.rs` (`mod codec;` and `pub use`)
- Modify: `crates/ridl-backend-ts/src/contract.rs` (write the file when the flag
  is on)
- Create: `crates/ridl-backend-ts/tests/codec.rs`
- Create: `crates/ridl-backend-ts/tests/snapshots/codec__cabin.snap` (written by
  `insta`)
- Modify: `docs/book/generated-code.md` (the "TypeScript" section),
  `docs/design/codegen-plugins.md` (the sentence that says the TypeScript
  backend reads only its doc comments from the model)

**Interfaces:**

- Consumes: `v1::Model` (`scope.package`, `declarations`, `tuples`,
  `interfaces`, `flatbuffers.roots`, `catalog`), the request's `artifact_base`;
  the runtime types of Task 2 by name.
- Produces:
  - `pub fn generate_codec(model: &v1::Model, artifact_base: &str) -> Result<GeneratedTs, GenerateError>`
    in `codec.rs`.
  - In `contract.rs`, with `codec = true`, a second file
    `format!("{}.codec.ts", request.artifact_base)` whose content is
    `format!("{preamble}/* eslint-disable */\n\n{}", codec.source)`, after
    `<base>.ts` in `files`.
  - The file's layout, in order (D-13 and "What is generated" in the note):
    1. `import * as flatbuffers from "flatbuffers";`,
       `import type { MemberRef, PayloadCodec } from "@driftsys/ridl-runtime";`,
       `import type * as t from "./<artifact_base>.js";`,
       `import * as fb from "./<package segments joined by '/'>.js";`, then one
       `import type * as <DottedName.underscored> from "./<package>.js";` per
       foreign package that a converted type names, in sorted order (Task 5 is
       the first to need one)
    2. `export const CATALOG_NAME = "<Catalog.package>";`,
       `export const CATALOG_HASH: Uint8Array = Uint8Array.from([<32 bytes, decimal>]);`,
       `export const PAYLOAD_TYPES = [<qualified names, sorted byte by byte, one per line>] as const;`,
       `export const RULES = ["length", "pattern", "range", "step", "unique", "variant"] as const;`
    3. Per declaration in `model.declarations` order, for the kinds of this
       task:
       - named scalar, enum, enum set, each a payload type (`<Name>Box`):
         `export function encode<Name>(value: t.<Name>): Uint8Array` builds
         `new fb.<Name>BoxT(<packed>).pack(builder)` on
         `const builder = new flatbuffers.Builder();` and returns
         `builder.asUint8Array()`;
         `export function decode<Name>(bytes: Uint8Array): t.<Name>` returns
         `<unpacked>` of
         `fb.<Name>Box.getRootAs<Name>Box(new flatbuffers.ByteBuffer(bytes)).unpack().value`.
         `<packed>` and `<unpacked>` by kind: named scalar and enum set `value`
         and `… as t.<Name>`; enum `BigInt(value)` and `Number(…) as t.<Name>`.
       - struct: `function pack<Name>(value: t.<Name>): fb.<Name>T` creates
         `new fb.<Name>T()` and assigns each field by name (`Spellings.camel` of
         the field on the `fb` side, `declared` on the `t` side) with the
         field's kind rule;
         `function unpack<Name>(object: fb.<Name>T): t.<Name>` returns an object
         literal of the fields. When the struct is a payload type:
         `encode<Name>` finishes `pack<Name>(value).pack(builder)` and
         `decode<Name>` returns
         `unpack<Name>(fb.<Name>.getRootAs<Name>(new flatbuffers.ByteBuffer(bytes)).unpack())`.
       - field rules of this task: a named scalar or enum set field is a type
         assertion both ways; an enum field is `BigInt(…)` and
         `Number(…) as t.<Enum>`; a struct field is `pack<Other>` and
         `unpack<Other>`.
    4. Per payload type,
       `const codec<Name>: PayloadCodec<t.<Name>> = { type: "<qualified>", encode: encode<Name>, decode: decode<Name> };`
    5. Per interface of `model.interfaces` whose identity is `declared`, in
       order:
       `export const <Name> = { iface: <number>, <member>: { iface: <number>, ord: <ordinal>, kind: "<kind>", <roles> }, … } as const;`
       where `<roles>` is `payload: codec<T>` for a signal, an event and a
       fixed; `request: codec<A>` for a command;
       `request: codec<A>, reply: codec<R>` for a query. The payload type of
       each role follows the Rust descriptors' rule (`request` or
       `request_tuple`, `reply_payload` or `reply_tuple`, `fixed.named`). A
       member whose payload type has no finite FlatBuffers bound gets, in its
       place, the line
       `// <member> has no entry: <qualified payload> has no finite FlatBuffers bound`.
       A retired slot gets nothing. An inline service shape gets no table.
    6. Each exported function and constant carries a one-line TSDoc, with the
       qualified name in a code span: for `encode<Name>`, "Encodes a
       `<qualified>` payload."; for `decode<Name>`, "Decodes a `<qualified>`
       payload the codec has verified."; for an interface table, "The members of
       interface `<Name>`, interface number `<n>`."; and one sentence each for
       the four constants.
  - Refusals (`GenerateError::Unrepresentable`):
    - `malformed codegen model: \`Catalog.hash\` is <n> bytes, not 32`;
    - a `tuple_collisions` entry:
      ``the induced tuple `<name>` has two shapes, (<first_fields joined by ", ">) and (<second_fields joined by ", ">); the codec module cannot name both``;
    - an induced tuple whose `InducedName.rust` equals a declared name:
      `` `<name>` is both a declaration of the package and the induced tuple of `<Interface>.<member>` ``;
    - a kind this task does not cover (union, tuple, map, array, optional field,
      bytes, foreign reference) is refused with
      `` `<qualified>` reaches a <kind>, which the codec module does not convert yet ``
      until Task 5 replaces each with its rule.

- [ ] **Step 1: Write the failing snapshot test**

`crates/ridl-backend-ts/tests/codec.rs`: a helper
`fn cabin_model() -> v1::Model` that compiles `examples/cabin` with
`ridlc::compile_workspace` (as `model_drift.rs`'s cross-package test does),
asserts no diagnostic other than `TYPL-406`, takes the `veh.cabin` package and
returns `codegen::lower(&package, &[])`. Test `the_cabin_codec_module`:
`insta::assert_snapshot!("cabin", generate_codec(&cabin_model(), "veh.cabin").unwrap().source)`.
Review the snapshot against the "What is generated" example of the note: `Cabin`
at `iface: 1` with `temperature` 1 signal, `warning` 2 event, `setLevel` 3
command with `request: codecLevel`, `average` 4 query with
`request: codecWindow, reply: codecAverage`; `Horn` at `iface: 2` with `active`
1; `PAYLOAD_TYPES` of the eight names in the order `__ridl_wasm.rs` lists them.

- [ ] **Step 2: Write the failing completeness test**

`every_bounded_root_has_a_codec_and_every_live_interaction_an_entry`, over
`cabin_model()` and over `ridlc::compile` of
`crates/ridl-backend-rust/tests/fixtures/interaction_face.ridl` lowered with
`codegen::lower(&package, &[])`: the set of `encode<Name>` function names read
from the source equals the set of `decode<Name>` names and equals the set of
roots whose bound is `MaxSize` (declared name or `InducedName.rust`); for every
interface with a `declared` identity and every slot whose occupant is an
interaction, the source holds the line
`<member>: { iface: <number>, ord: <ordinal>, kind: "<kind>"`; the source holds
no `field_1`.

- [ ] **Step 3: Write the failing refusal tests**

`a_model_without_a_catalog_hash_is_refused` (clear `catalog.hash`; the message
above with `0`), `a_tuple_collision_is_refused` (push one
`TupleCollision { name: "Pair", first_fields: ["a"], second_fields: ["b"] }`;
the message above), `an_induced_tuple_named_as_a_declaration_is_refused` (rename
one tuple's `name.rust` to `Warning` in the `interaction_face` model after
making a two-parameter call reach it: use
`crates/ridl-backend-rust/tests/fixtures/induced_params.ridl`, whose `Pump.both`
induces a tuple, and rename it to `Level`; the message names `Pump.both`).

- [ ] **Step 4: Run the test file to see it fail**

Run: `cargo test -p ridl-backend-ts --test codec` Expected: FAIL
(`generate_codec` does not exist).

- [ ] **Step 5: Implement `codec.rs` and the `contract.rs` hook**

- [ ] **Step 6: Run the tests, review and accept the snapshot**

Run: `cargo test -p ridl-backend-ts --test codec` then `cargo insta review` (or
set `INSTA_UPDATE=always` on the first run and read the file). Expected: PASS;
the snapshot matches Step 1's description.

- [ ] **Step 7: Add the contract test and the drift test's codec half**

In `tests.rs`, `the_codec_option_writes_a_second_file`: a request over the
`interaction_face` package with `codec = true` returns two files, `face.demo.ts`
and `face.demo.codec.ts`, the second starting with the generated marker line and
containing `export const Cabin = {`; with `codec = false` it returns one file.

- [ ] **Step 8: Run the crate's tests, lint and the book check**

Run:
`cargo test -p ridl-backend-ts && cargo clippy -p ridl-backend-ts --all-targets -- -D warnings && just book-check link-check doc-path-check story-id-check`
Expected: PASS.

- [ ] **Step 9: Update the two documents and commit**

`docs/book/generated-code.md`, "TypeScript": replace "It emits no face, no
codec" with the as-built sentence: with `[backend.typescript] codec = true` it
also writes `<base>.codec.ts`, the plain-object `encode<Name>` and
`decode<Name>` functions over `flatc`'s generated code, one table per interface,
and the catalog constants; name the two steps the user runs
(`ridl build --emit typescript --emit flatbuffers`, then
`flatc --ts --gen-object-api -o <out-dir> <base>.fbs`). In
`docs/design/codegen-plugins.md`, the transition sentence: the codec module
reads the model only.

```bash
git add crates/ridl-backend-ts docs/book/generated-code.md docs/design/codegen-plugins.md
git commit -m "feat(ridl-backend-ts): write the codec module for scalars, enums and structs"
```

**Semantic mutations the tests must kill:** `ord` written as the slot's position
instead of `ordinal` (the completeness test over a fixture with a retired
ordinal: Task 5's fixture; here the `interaction_face` numbering); `BigInt` and
`Number` swapped (the snapshot, and Task 7's round trip); `PAYLOAD_TYPES` in
declaration order (the snapshot, and Task 6's pin); `iface` written as the
interface's index plus one instead of `number` (the snapshot pins `Horn` at 2 by
name order, which is the model's number); the `Box` `.value` read dropped (the
snapshot, Task 7's round trip).

---

### Task 5: The converters for unions, maps, tuples, arrays, optionals and induced tuples

**Files:**

- Create: `crates/ridl-backend-ts/tests/fixtures/codec/ridl.toml`
- Create: `crates/ridl-backend-ts/tests/fixtures/codec/common.typl` (a verbatim
  copy of `crates/ridlc/tests/corpus/veh-cluster/common/common.typl`)
- Create: `crates/ridl-backend-ts/tests/fixtures/codec/shape.ridl`
- Create: `crates/ridl-backend-ts/tests/fixtures/codec/roundtrip_test.ts`
- Create: `crates/ridl-backend-ts/tests/fixtures/codec/deno.json`
- Modify: `crates/ridl-backend-ts/src/codec.rs`
- Modify: `crates/ridl-backend-ts/tests/codec.rs`
- Create: `crates/ridl-backend-ts/tests/snapshots/codec__common.snap`
- Modify: `justfile` (`_codec-ts-check`, called from `_cabin-web-generate`)

**Interfaces:**

- Consumes: Task 4's `generate_codec`; Task 2's runtime package (for
  `deno check`).
- Produces the remaining conversion rules of D-4, each emitted by `codec.rs`:
  - union `<U>` with arms `<arm>` (declared name, `Spellings.snake` on the `fb`
    side): `pack<U>(value: t.<U>): fb.<U>T` is a `switch (value.kind)` whose
    case `"<arm>"` returns `new fb.<U>T(fb.<U>Union.<snake>, <packed arm>)`,
    where an arm of a struct or union type packs with `pack<Arm>` and an arm of
    a named scalar, enum or enum set type packs into
    `new fb.<U><Arm>BoxT(<packed value>)` (`<Arm>` is the arm's
    `Spellings.pascal`); `unpack<U>(object: fb.<U>T): t.<U>` is a
    `switch (object.valueType)` whose case `fb.<U>Union.<snake>` returns
    `{ kind: "<arm>", value: <unpacked> }` reading `object.value as fb.<T>T`
    (and `.value` of a box), and whose `default` throws
    `new RangeError("union <qualified> carries no arm")`.
  - map field `<f>` of key `K` and value `V` with entry table `<E>`: pack
    `value.<f>.map(([key, value]) => { const entry = new fb.<E>T(); entry.key = <packed K>; entry.value = <packed V>; return entry; })`;
    unpack
    `object.<f>.map((entry) => [<unpacked key>, <unpacked value>] as const)`.
  - tuple field or induced tuple with table `<T>` and fields `f1 … fn`: pack
    assigns `tuple.field1 … tuple.fieldN` (the `flatc` camel of `field_<n>`)
    from the named fields; unpack returns `{ <f1>: <unpacked field1>, … }`.
  - array field: `value.<f>.map(<pack element>)` and
    `object.<f>.map(<unpack element>)`; an element that converts by assertion
    only is copied with `[...value.<f>]`.
  - `bytes` field or named scalar backed by bytes: pack `Array.from(value)`,
    unpack `Uint8Array.from(object)`.
  - optional field `<f>`: pack
    `object.<f> = value.<f> === undefined ? null : <packed>`; unpack builds the
    literal of the required fields as `const result: t.<Name> = { … };` then
    `if (object.<f> !== null) result.<f> = <unpacked>;` and returns `result`.
  - required field of an enum without a zero member (`= null` in the `.fbs`,
    `FbSlot.needs_null_default`): unpack reads
    `object.<f> === null ? absent("<qualified>", "<f>") : Number(object.<f>) as t.<E>`
    where `function absent(type: string, field: string): never` throws
    `new RangeError(\`field ${field} of ${type} is absent\`)`, emitted once at
    the top of the file when needed.
  - induced tuple of an interaction (D-9):
    `export interface <InducedName.rust> { <field>: <ts type>; … }` emitted
    after the constants, with the `t.` prefix on named types, and the
    `encode`/`decode`/`codec` items as for a struct; an interaction's `request`
    or `reply` whose tuple is this one references `codec<InducedName.rust>`.
  - foreign reference (D-14): a field, an arm, an element or a payload type
    whose `TypeRef.foreign` is true and whose foreign declaration is a struct, a
    union or an enum is refused with
    `` `<qualified payload>` reaches `<foreign package>.<Name>`, a <struct|union|enum> of another package, which the codec module does not convert yet ``;
    a foreign named scalar or enum set converts by assertion, with the `bigint`
    form when `model.foreign` states a 64-bit width.
- The fixture `shape.ridl`, package `veh.common`:

```ridl
package veh.common

/// An enum with no zero member, so a field of it is `= null` in the `.fbs`.
enum Mode {
  LOW = 1
  HIGH = 2
}

/// A struct whose one field reads as absent when the buffer leaves it out.
struct Setting {
  mode: Mode
}

interface Sensors {
  signal latest : SensorReading @100ms
  reserved legacyRaw
  event result : SensorResult @[100ms..1s]
  signal bounds : SensorBounds @1s
  command setLimit(limit: Speed, actual: Speed) @[..50ms]
  query profile(name: Name): DriverProfile @[..100ms]
  query span(): (min: Speed, max: Speed) @[..100ms]
  signal setting : Setting @1s
}
```

(`Sensors` ordinals: `latest` 1, `legacyRaw` 2 retired, `result` 3, `bounds` 4,
`setLimit` 5, `profile` 6, `span` 7, `setting` 8.)

`ridl.toml`: `[package] name = "veh.common" version = "1.0.0"`,
`[backend.rust] encodings = ["flatbuffers"] wasm = true`,
`[backend.typescript] codec = true`. `deno.json`:
`{"imports": {"flatbuffers": "npm:flatbuffers@25.9.23", "@driftsys/ridl-runtime": "../../../../../packages/ridl-runtime/mod.ts"}, "unstable": ["sloppy-imports"], "nodeModulesDir": "none"}`
(the recipe copies `deno.json` and `roundtrip_test.ts` beside the generated
files, so the path to the runtime package is written from the fixture's own
directory and rewritten by the recipe to an absolute path).

- [ ] **Step 1: Create the fixture and pin the copy**

Test `the_codec_fixture_copies_the_corpus_common_typl` in `tests/codec.rs`: the
bytes of `tests/fixtures/codec/common.typl` equal the bytes of
`../ridlc/tests/corpus/veh-cluster/common/common.typl`.

- [ ] **Step 2: Write the failing snapshot and refusal tests**

- `the_common_codec_module`: `fn common_model()` compiles the fixture directory
  with `ridlc::compile_workspace`, takes `veh.common`, lowers with the
  workspace's `ridl.std` package as the one other (so the foreign
  `ridl.std.Label`, `Name`, `Message`, `Timestamp` references resolve), and
  `insta::assert_snapshot!("common", …)`. Review: `SensorResult` packs by
  `switch (value.kind)` with cases `"ok"` and `"err"`; `SensorBounds` has the
  tuple `range` as `field1`/`field2`, the array `readings`, and the map `meta`
  through `SensorBoundsMetaEntryT`; `DriverProfile` has the optional `override`;
  `SensorsSetLimitParams` (or the `InducedName.rust` the model carries; read it
  from `model.tuples` in the test and assert the interface name appears) is an
  `export interface` with `limit` and `actual`; `Sensors` has `latest` 1,
  `result` 3, `bounds` 4, `setLimit` 5, `profile` 6, `span` 7, and no 2.
- `a_foreign_struct_is_refused`: a hand-built model (the `tests.rs` builders,
  lifted into this file or re-exported) with one struct whose field is a
  resolved foreign `TypeRef { package: "veh.other", foreign: true }` naming a
  struct in `model.foreign`; the message above with `struct`. The same for an
  enum and a union.
- `a_foreign_named_scalar_converts_by_assertion`: the `common` snapshot contains
  the line `import type * as ridl_std from "./ridl.std.js";` (one such import
  per foreign package a converted type names, as Task 4's layout item 1 states)
  and the assertion `name: object.name as ridl_std.Name`.

- [ ] **Step 3: Run to see them fail, implement the rules, run to see them
      pass**

Run: `cargo test -p ridl-backend-ts --test codec` Expected: FAIL on the refusal
message of Task 4 for `union`, then PASS.

- [ ] **Step 4: Write the round-trip test**

`roundtrip_test.ts` imports `./veh.common.codec.ts` and `./veh.common.ts` and,
with `assert.deepEqual`, checks `decode<Name>(encode<Name>(v))` equals `v` for:
`SensorResult` `{ kind: "ok", value: { value: 12.5, timestamp: 7n } }` and
`{ kind: "err", value: { code: 3, message: "fault" } }`; `SensorBounds` with
`range: { min: 1, max: 2 }`, eight `readings`, `labels: ["a"]`,
`meta: [["k", "v"]]`; `DriverProfile` with and without `override`;
`<InducedName.rust of setLimit>` `{ limit: 10, actual: 20 }`;
`<InducedName.rust of span>` `{ min: 1, max: 2 }`; `WarningFlags` `3`;
`GearPosition` `GearPosition.REVERSE`; `Setting` `{ mode: Mode.HIGH }`. Test
`an absent required enum field throws on decode`: `Mode` has no zero member, so
`Setting.mode` is `= null` in the `.fbs`; build `new fb.SettingT()` with `mode`
left `null`, pack and finish it with a `flatbuffers.Builder`, and assert
`decodeSetting` throws a `RangeError` whose message is
`field mode of veh.common.Setting is absent`. Test
`a two-parameter call round-trips by name`: the `setLimit` case above, and
`Sensors.setLimit.request.type` equals the qualified `InducedName.rust`.

- [ ] **Step 5: Add the recipe**

`_codec-ts-check` (private) in the `justfile`: builds the CLI if needed,
`rm -rf` then writes `"$tmp"` under
`${CARGO_TARGET_DIR:-target}/codec-ts-check/`, runs
`"$target/debug/ridl" build crates/ridl-backend-ts/tests/fixtures/codec --emit typescript --emit flatbuffers --out-dir "$tmp"`,
`flatc --ts --gen-object-api -I "$tmp" -o "$tmp" "$tmp/veh.common.fbs"` (a
`ridl.std` named scalar inlines at its field, so the schema needs no include),
copies `deno.json` (with the runtime path rewritten to
`$PWD/packages/ridl-runtime/mod.ts`) and `roundtrip_test.ts` into `"$tmp"`, then
runs `deno check --config "$tmp/deno.json" "$tmp/veh.common.codec.ts"` and
`deno test --config "$tmp/deno.json" "$tmp/roundtrip_test.ts"`. Call it from
`_cabin-web-generate` after the `flatc` and `deno` version checks. Document the
recipe in `AGENTS.md`'s `cabin-ws-demo` line in one clause.

- [ ] **Step 6: Run the recipe**

Run: `just _codec-ts-check` Expected: `deno check` passes and every round-trip
test passes.

- [ ] **Step 7: Commit**

```bash
git add crates/ridl-backend-ts justfile AGENTS.md
git commit -m "feat(ridl-backend-ts): convert unions, maps, tuples, arrays and optionals in the codec module"
```

**Semantic mutations the tests must kill:** tuple positions swapped (the `range`
and `span` round trips); the union `kind` written in snake case (the `ok`/`err`
round trip and the snapshot); map entries as `[value, key]` (the `meta` round
trip); `null` unpacked as `null` instead of an absent property (`DriverProfile`
without `override` deep-equals an object with no `override` key); the
absent-enum guard removed (the `RangeError` test); a foreign struct converted as
a local one (the refusal test); `ord` as the slot position (the
`reserved legacyRaw` slot makes `result` ordinal 3, not 2).

---

### Task 6: The build writes the file, and the pin test

**Files:**

- Create: `crates/ridlc/tests/codec_table.rs`
- Create: `crates/ridlc/tests/fixtures/codec-two-packages/ridl.toml`,
  `a/ridl.toml`, `a/a.typl`, `b/ridl.toml`, `b/b.typl`

**Interfaces:**

- Consumes: Task 1's module, Task 3's key, Task 4's file.
- Produces: nothing new in the product; the tests are the deliverable. The
  two-package fixture: root `ridl.toml` with `[workspace] members = ["a", "b"]`,
  `[backend.rust] encodings = ["flatbuffers"] wasm = true`,
  `[backend.typescript] codec = true`; `a/ridl.toml` names `pin.a`, `a/a.typl`
  declares `type Zed : integer [0..1]`; `b/ridl.toml` names `pin.b`, `b/b.typl`
  declares `type Alpha : integer [0..1]` (the short names sort the other way
  round from the qualified names, so a spelling without the package would
  reorder them).

- [ ] **Step 1: Write the failing tests**

`codec_table.rs`, with `tempfile` and the copy helpers of `wasm_module.rs` (lift
`cabin_with_wasm` into `tests/support/mod.rs` as
`pub fn copy_cabin(dst: &Path, manifest: &str)`):

- `a_build_with_codec_writes_the_codec_file_beside_the_wasm_module`: copy cabin
  with its manifest plus `\n[backend.typescript]\ncodec = true\n`; run
  `ridlc::run_build(&src, &out, &[Emit::Rust, Emit::TypeScript], false.into())`;
  assert no error, `out/veh.cabin.codec.ts` exists, its first line is the
  generated marker, `out/__ridl_wasm.rs` exists. With the manifest as it is (no
  `codec`), the same build writes `veh.cabin.ts` and no `veh.cabin.codec.ts`.
- `the_codec_table_names_what_the_wasm_module_lists`: from the first build, read
  `WASM_PAYLOAD_TYPES` and `WASM_RULES` from `__ridl_wasm.rs` (the `dense` and
  `head` reading of `descriptor_generation.rs`) and `PAYLOAD_TYPES` and `RULES`
  from `veh.cabin.codec.ts` (the `[` to `] as const` text, split on `,`, quotes
  trimmed); assert the two type lists are equal and the two rule lists are
  equal; assert the `CATALOG_HASH` bytes equal the `CATALOG_HASH` array of
  `__ridl_wasm.rs`.
- `each_package_lists_a_subsequence_of_the_crate_list`: build the two-package
  fixture with both emits; read the crate list and `PAYLOAD_TYPES` of
  `pin.a.codec.ts` and `pin.b.codec.ts`; assert the crate list is
  `["pin.a.Zed", "pin.b.Alpha"]`, each package's list is a subsequence of it in
  order, and their concatenation in package order equals it.
- `codec_without_wasm_writes_nothing`: copy cabin with `wasm = true` removed and
  the `codec` table added; `run_build` has an error diagnostic with code
  `MANI-019`, and `out` holds no `veh.cabin.codec.ts` and no `veh.cabin.ts`.

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p ridlc --test codec_table` Expected: the tests FAIL to
compile until the support helper and the fixture exist. Once they exist, the pin
tests pass on their first run, because Task 4 already writes the file; Step 3 is
what proves they pin anything.

- [ ] **Step 3: Mutate once by hand and restore**

In `codec.rs`, spell the qualified name without the package; run the pin test:
it must FAIL on `the_codec_table_names_what_the_wasm_module_lists`. Restore. In
`codec.rs`, sort by the declared name; run: it must FAIL on
`each_package_lists_a_subsequence_of_the_crate_list`. Restore.

- [ ] **Step 4: Run the tests to see them pass and commit**

Run: `cargo test -p ridlc --test codec_table --test wasm_module` Expected: PASS.

```bash
git add crates/ridlc/tests
git commit -m "test(ridlc): pin the codec table to the wasm module's lists"
```

**Semantic mutations the tests must kill:** the name spelled without the package
(Step 3); the sort by short name (Step 3); the hash written from `Scope.package`
instead of `Catalog.hash` (the `CATALOG_HASH` assertion); the file written when
`codec` is absent (the no-`codec` case).

---

### Task 7: `examples/cabin/web` reads the generated table

**Files:**

- Modify: `examples/cabin/ridl.toml`
- Modify: `justfile` (`_cabin-web-generate`, the `cabin-ws-demo` comment)
- Modify: `examples/cabin/web/deno.json`
- Modify: `examples/cabin/web/check.ts`, `main.ts`, `page.ts`, `check_test.ts`
- Modify: `AGENTS.md` (the `cabin-ws-demo` line), `docs/book/generated-code.md`
  only if Task 4's sentence names the demo

**Interfaces:**

- Consumes: `./generated/veh.cabin.codec.ts`'s `Cabin`, `CATALOG_NAME`,
  `CATALOG_HASH`, `PAYLOAD_TYPES`, `RULES`, `encodeTemperature`,
  `decodeTemperature`; Task 2's `Codec.typeId` and `UnknownPayloadType`.
- Produces, in `check.ts`:
  - removed: `TEMPERATURE`, `TEMPERATURE_TYPE_ID`, `encodeTemperature`,
    `decodeTemperature`, the `flatbuffers` and `TemperatureBox` imports;
  - `export const RANGE_RULE = 2 + RULES.indexOf("range");`
  - `export function verifyPayload(codec: Codec, payload: Uint8Array): string | undefined`
    and `export function selfTest(codec: Codec): string | undefined` take the id
    from `codec.typeId(Cabin.temperature.payload.type)`; the `unknown-type`
    cause reads `the codec does not know type id ${id}` with that id;
  - `check.ts` re-exports nothing from the generated file: `main.ts`, `page.ts`
    and `check_test.ts` import `Cabin`, `Horn`, the constants and the functions
    from `./generated/veh.cabin.codec.ts` directly.

- [ ] **Step 1: Switch the manifest and the recipe**

`examples/cabin/ridl.toml` gains `[backend.typescript]\ncodec = true`.
`_cabin-web-generate`: replace the two generation lines with
`"$target/debug/ridl" build examples/cabin --emit typescript --emit flatbuffers --out-dir examples/cabin/web/generated`
and
`flatc --ts --gen-object-api -I examples/cabin/web/generated -o examples/cabin/web/generated examples/cabin/web/generated/veh.cabin.fbs`;
`generated/schema/` is no longer written. `deno.json` gains the import
`"@driftsys/ridl-runtime": "../../../packages/ridl-runtime/mod.ts"`.

- [ ] **Step 2: Rewrite `check_test.ts` as the failing tests**

Keep the `TemperatureCheck` and `isPass` tests. Replace the rest:

- `the generated table addresses Cabin.temperature at 1:1`: `Cabin.iface === 1`,
  `Cabin.temperature.iface === 1`, `Cabin.temperature.ord === 1`,
  `Cabin.temperature.kind === "signal"`,
  `Cabin.temperature.payload.type === "veh.cabin.Temperature"`,
  `Cabin.average.reply.type === "veh.cabin.Average"`, `Horn.iface === 2`.
- `the codec is loaded with the table's hash and resolves every payload type`:
  `loadCodec(bytes, { catalogHash: CATALOG_HASH, payloadTypes: PAYLOAD_TYPES, ruleText: RULES })`
  resolves; `codec.typeId("veh.cabin.Temperature")` is an integer from 0 to 7;
  `codec.typeId("veh.cabin.Nothing")` throws `UnknownPayloadType`.
- `encodeTemperature round-trips through decodeTemperature` for -41, 20, 86 (the
  generated functions).
- `RANGE_RULE is 2 plus the index of range in RULES`: equals 4.
- the existing self-test, `verifyPayload` and truncation tests, with
  `TEMPERATURE_TYPE_ID` replaced by
  `codec.typeId(Cabin.temperature.payload.type)`, and `fakeCodec` given
  `typeId(name) { assert.equal(name, "veh.cabin.Temperature"); return 5; }` and
  `verify` asserting `typeId === 5`.

- [ ] **Step 3: Run the demo to see it fail**

Run: `just cabin-ws-demo` Expected: FAIL at `deno check main.ts` (the removed
exports).

- [ ] **Step 4: Rewrite `check.ts`, `main.ts` and `page.ts`**

`main.ts` and `page.ts`: load the codec with `CATALOG_HASH`, `PAYLOAD_TYPES` and
`RULES`; compare the server's announced hash with `CATALOG_HASH` and fail with
`the server announces catalog hash <hex>, the generated table has <hex>` on a
difference; attach with `{ name: CATALOG_NAME, hash: CATALOG_HASH }`; subscribe
and filter with `Cabin.temperature.iface` and `Cabin.temperature.ord`; decode
with `Cabin.temperature.payload.decode`. `page.ts` keeps the `hash` query
parameter for the cross-check.

- [ ] **Step 5: Run both demos**

Run: `just cabin-ws-demo && just cabin-browser-demo` Expected: both print their
`ok` line and exit 0; `deno test check_test.ts` passes inside the first.

- [ ] **Step 6: Update the recipe comments and `AGENTS.md`, run the static
      gates, commit**

Run: `just pre-push` Expected: PASS.

```bash
git add examples/cabin justfile AGENTS.md
git commit -m "feat(repo): read cabin's interaction table and codec from the generated TypeScript"
```

**Semantic mutations the tests must kill:** `verifyPayload` passing a constant
id instead of `typeId(...)` (the `fakeCodec` assertion on the name); `main.ts`
subscribing to `ord: 2` (the demo receives no sample and fails on its deadline);
`decodeTemperature` returning the raw `TemperatureBoxT` (the round trip's
`assert.equal`); the announced-hash cross-check removed (no test: it is a
demo-only check, named here so a reviewer reads it).

---

## Self-review

- Spec coverage: D-1 and D-13 (Task 4), D-2, D-3, D-4 (Tasks 4 and 5), D-5 (Task
  1), D-6 (Tasks 4 and 6), D-7 and D-8's runtime types (Task 2), D-8 and D-9
  (Tasks 4 and 5), D-10 and D-15 (Tasks 4 and 7), D-11 (Tasks 4 and 7), D-12
  (Task 3), D-14 (Task 5), the frozen-surface update (Task 3), the cabin changes
  (Task 7). Q-3's follow-up issue is filed with the pull request, not by a task.
- Names used across tasks: `ridl_type_id`, `type_id_of` (Tasks 1, 2, 6);
  `UnknownPayloadType`, `PayloadCodec`, `MemberRef`, `MemberKind`,
  `payloadTypes`, `typeId` (Tasks 2, 4, 7); `CODEC_KEY`, `MANI_019`,
  `CODEC_OPTION` (Tasks 3, 6); `generate_codec(model, artifact_base)` (Tasks 4,
  5); `CATALOG_NAME`, `CATALOG_HASH`, `PAYLOAD_TYPES`, `RULES`, `encode<Name>`,
  `decode<Name>`, `codec<Name>`, `<Interface>` (Tasks 4 to 7).
- Review Focus: each of the five lines names its test and task.
