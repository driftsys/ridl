# The TypeScript wasm codec and the runtime package — design

Status: draft for review. Written 2026-10-21 for story E12.1
(driftsys/ridl#287). Decisions marked **(ruling)** were taken by the author
under the delegation to run this work unattended; Sebastien reviews them.

## Intent

A web page must run a ridl contract over the WebSocket transport without a
TypeScript codec. The page loads the generated TypeScript for a package and the
package's wasm codec, connects with `@driftsys/ridl-transport-ws`, and checks or
reads each payload through the wasm module.

Done when a Deno program and a browser page each receive a live signal from a
Rust server, check its payload with the wasm codec, and reject a payload that
violates a typl range. That is the validation half of the E12.1 exit criterion
("constructs and validates a payload without a TypeScript codec"). The
construction half depends on the open question at the end. E12.11, the transport
package, has landed and is this work's input.

## Goals, non-goals

Goals:

- One generated unit: the wasm codec and the generated TypeScript come from one
  `ridl build` run, so they cannot differ (ADR-0020 decision 7).
- No TypeScript codec: encoding and verification run in the generated Rust.
  TypeScript only reads fields of a buffer the wasm module has verified, through
  generated read accessors (ADR-0020 decision 2).
- The same artifacts run in Deno, Bun, Node 22 and a browser.

Non-goals:

- Publishing a generated package. The user who runs `ridl build` publishes it.
- Contract-generic decoding, which stays parked (E12.3).
- A proto3 codec at the wasm boundary. The boundary is FlatBuffers (ADR-0020
  decision 2).
- Generated faces over the ports. E12.1 includes them, and they build on the
  codec table of D-4. They are a later slice that this note does not design. The
  emulator (E12.4) is a separate story.

## What exists

- The generated Rust crate has `impl Payload<FlatBuffers> for T` with `encode`,
  `verify` and `decode`, and a descriptor per interface that implements
  `ridl_rt::contract::Interface` with `CATALOG` (name and 32-byte hash) and the
  member table (`ordinal`, `kind`, `payloads[].type_name`). All of it is generic
  Rust. It exists only when the manifest's `[backend.rust] encodings` names
  `flatbuffers`, and a type with no finite FlatBuffers bound has no impl. There
  is no `extern "C"` item and no `#[no_mangle]` item.
- The generated crate is not on the `-p` list of `just wasm-check`. A test,
  `the_generated_codec_checks_for_wasm32` in
  `crates/ridl-backend-rust/tests/flatbuffers_conformance.rs`, type-checks one
  fixture for `wasm32-unknown-unknown` under `just test`. Nothing builds a
  `.wasm` file.
- `ridl-backend-ts` writes one `<base>.ts` file of types and `init` functions.
  It emits no codec call.
- The TypeScript transport hands the caller `Uint8Array` payloads, the interface
  number and the ordinal. The caller needs the encoding of the session, a map
  from (interface, ordinal) to a payload type, and a codec that matches the
  catalog hash.

## Decisions

**D-1. The wasm boundary is raw exports, with no `wasm-bindgen` (ruling).** The
codec moves byte buffers and integers only, and the loader is about 100 lines. A
generator such as `wasm-bindgen` adds a build tool, a JavaScript glue file and a
version coupling for no gain.

**D-2. The generated crate gets an optional `wasm` module (ruling).** An option
`[backend.rust] wasm = true` makes the build write a module `wasm`, once per
crate rather than once per package, because its exports are unmangled symbols.
`ridl_backend_rust::generate_wasm_module` renders it over every package of the
crate, and `ridlc` writes it to `__ridl_wasm.rs` and names it in `lib.rs`. The
exports are compiled only for `target_arch = "wasm32"` with the generated
crate's feature `wasm-exports` on, which `render_cargo_toml` declares as
`wasm-exports = ["std"]`; the manifest declares no crate type and no profile (as
amended in the slice 1 review). The exports are:

| Export                                  | Behaviour                                                                                   |
| --------------------------------------- | ------------------------------------------------------------------------------------------- |
| `ridl_alloc(len) -> ptr`                | allocate `len` bytes in the module's memory                                                 |
| `ridl_free(ptr, len)`                   | free a buffer from `ridl_alloc`                                                             |
| `ridl_catalog_hash(out_ptr)`            | write the 32-byte catalog hash of the unit to `out_ptr`                                     |
| `ridl_verify(type_id, ptr, len) -> i32` | run `Payload<FlatBuffers>::verify` for payload type `type_id`; return 0, or a positive code |

The catalog hash is per unit: every interface of a unit shares the value of its
`Interface::CATALOG`. The module writes the hash of the root package's unit, or
of the one unit of the workspace's own packages when the manifest names no
package; any other build is refused. `ridl_alloc` returns 0 for a length of 0
and when the allocation fails, so the loader (slice 2) treats 0 with a length
above 0 as out of memory. In `ridl_verify`, code 1 means a malformed buffer
(`VerifyError::Structure`), a code of 2 or more is a typl rule, and an unknown
`type_id` returns -1.

The option requires `encodings` to name `flatbuffers`, and the backend reports
an error otherwise. A `type_id` is the index of the payload type in a sorted
list of the types that have a `Payload<FlatBuffers>` impl, `WASM_PAYLOAD_TYPES`.
The sort key is the qualified name, `<package>.<name>`, compared byte by byte;
an induced tuple is named by its Rust spelling. The rule codes index
`WASM_RULES`, the lower-case names of `ridl_rt::payload::Rule` sorted (`length`,
`pattern`, `range`, `step`, `unique`, `variant`); code 2 is `length`. Both lists
are those of one build, so the TypeScript table of slice 3 must come from the
same run as the module. The same run also writes that list into the TypeScript
(D-4). A session must attach with the encoding `flatbuffers`, because the
transport defaults to proto3. One `ridl_verify` with a table replaces one export
per type, so the export set does not grow with the package.

**D-3. Reading is in place, and construction is a later slice (ruling).** The
first slice checks bytes. Reading a field of a verified buffer is done by
generated TypeScript accessors over the buffer, as ADR-0020 decision 2 says, and
a read allocates no object. Building a payload from a TypeScript value needs a
value format across the boundary (ADR-0020 open item 8 names JSON). That format
is the open question for Sebastien at the end of this note.

**D-4. The TypeScript backend writes a codec table (ruling).** Each backend
reads its own manifest table, and `ridl-backend-ts` takes no option today. A new
option `[backend.typescript] codec = true` makes it write `<base>.codec.ts`
beside `<base>.ts`. `ridlc` reports an error when `codec` is set without
`[backend.rust] wasm = true`, or the reverse, because the table and the module
must come from one run. The table holds the catalog hash of the unit as a
constant, the sorted payload type list with the `type_id` of each type, the
(interface, ordinal) to `type_id` map, and the table from a rule code to its
rule text. It imports only `@driftsys/ridl-runtime`. A payload type with no
`type_id` has no entry, and the runtime reports it as unsupported.

**D-5. `@driftsys/ridl-runtime` holds the loader (ruling).** It lives at
`packages/ridl-runtime/` and is built for npm with dnt, like the transport.
`loadCodec(source, { catalogs })` takes a `URL`, a `Response`, or bytes, calls
`WebAssembly.instantiateStreaming` where a `Response` is given and
`WebAssembly.instantiate` otherwise, and compares `ridl_catalog_hash` with the
constant in the generated table. A difference throws `CatalogMismatch` at load.
This settles ADR-0020 open item 6: the loader checks the coupling by the catalog
hash that the IR already defines, and no new hash is added. The loader returns
`{ verify(typeId, bytes): VerifyResult }`.

**D-6. The build is two commands (ruling).** `ridl build` with the Rust and
TypeScript targets writes the crate and the TypeScript, and
`cargo rustc --release --target wasm32-unknown-unknown --crate-type cdylib --features wasm-exports`
compiles it. The size settings `opt-level = "z"`, `lto = true` and
`panic = "abort"` come from the caller's environment
(`CARGO_PROFILE_RELEASE_OPT_LEVEL=z`, `CARGO_PROFILE_RELEASE_LTO=true`,
`CARGO_PROFILE_RELEASE_PANIC=abort`), not from the generated `Cargo.toml` (as
amended in the slice 1 review). `ridlc` does not run `cargo`: ADR-0020 open item
4 leaves open who compiles, and a tool that spawns a build in a user's
environment is a larger decision than this slice needs. A recipe
`just wasm-codec-demo` does both steps for `examples/cabin`.

**D-7. One end-to-end proof (ruling).** The loopback server of
`ridl-transport-ws` serves a test catalog (`test.ws`, hash `[7; 32]`), not
cabin, so a cabin wasm would fail the catalog check by design. The proof uses a
new server example in `examples/cabin`, which is its own cargo workspace and can
depend on the generated cabin crate and on `ridl-transport-ws` by path. It
serves a `temperature` signal. A Deno test and a browser test load the cabin
wasm, connect with the encoding `flatbuffers`, receive the signal, verify its
payload through the wasm module, and verify that a payload with a value outside
`[-40..85]` is rejected with a rule code.

## Slices

Each slice is one pull request, reviewed on its own.

1. **Rust exports.** The `wasm` option in the manifest parser and `ridlc`, the
   module, the `Cargo.toml` change, a new line in the `freeze/` manifest dump,
   and a test that builds the cabin crate for wasm and runs `ridl_verify` in a
   wasm host. That test is the guard for the `wasm` module, because
   `just wasm-check` does not reach generated code. Done when the test fails if
   the range check is removed. As built, the module is one per crate, not one
   per package, because its exports are unmangled symbols:
   `ridl_backend_rust::generate_wasm_module` renders it over every package of
   the crate, and `ridlc` writes it to `__ridl_wasm.rs` and names it in
   `lib.rs`. The per-package backend reads the `wasm` option only to refuse it
   without `flatbuffers`.
2. **Runtime package.** `packages/ridl-runtime/`, `loadCodec`,
   `CatalogMismatch`, a Deno test against the built cabin wasm, the dnt build
   and `ts-npm-check` style verification.
3. **Codec table.** The `codec` option and `<base>.codec.ts`. Done when a
   snapshot test pins the table and a Deno test imports it.
4. **Session.** The cabin server example and the end-to-end proof of D-7 in
   Deno, then in a browser, wired into `just` and CI.
5. **Construction.** Gated on the answer to the open question below.

## Alternatives considered

| Alternative                                     | Rejected because                                                                              |
| ----------------------------------------------- | --------------------------------------------------------------------------------------------- |
| `wasm-bindgen` glue                             | adds a tool and a generated glue file; the boundary is bytes and integers                     |
| One export per payload type                     | the export set grows with the package; a table in the TypeScript needs only one export        |
| `ridlc` runs `cargo` for the wasm               | spawns a build in the user's environment; ADR-0020 open item 4 is not settled in favour of it |
| A TypeScript FlatBuffers verifier or writer     | duplicates the check; ADR-0020 decision 7 requires it to run in the generated Rust            |
| A new coupling hash for the codec               | the catalog hash already identifies the contract the codec was generated from                 |
| Publishing a `@driftsys/<package>` wasm package | the codec is per user package (ADR-0020 decision 7)                                           |

## Constraints

- `ridl-rt` stays free of per-package types (ADR-0020 decision 7) and of a
  dependency (ADR-0021 decision 8).
- The generated crate and the manifest are frozen surfaces (ADR-0029). The new
  keys only add a line to the manifest dump, which the policy allows. No fixture
  turns the option on, so the generated dumps do not change.
- `just wasm-check` and the `thumbv7em-none-eabihf` build of the generated crate
  keep passing with the option off.

## Open question for Sebastien

How does a TypeScript caller build a payload without a TypeScript codec? Three
choices, with the author's lean first:

1. A per-type `ridl_encode(type_id, json_ptr, json_len, out_ptr, out_cap)` that
   takes JSON text and writes FlatBuffers. It needs a small JSON reader in the
   generated Rust (no dependency, `no_std`). It matches ADR-0020 open item 8.
2. Generated TypeScript setters that write scalars into a buffer the wasm module
   allocated, then `ridl_finish(type_id)` verifies it. No parser, but a writer
   in TypeScript for scalars only.
3. No construction. A TypeScript consumer only receives and reads, and the
   emulator (E12.4) builds payloads in Rust.
