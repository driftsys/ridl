# Streams stage 6 Implementation Plan

> **For agentic workers:** Implement one task per run using
> `superpowers:executing-plans`; the driver reviews its tests and mutations
> before the next task. Leave changes uncommitted. Read the questions at the end
> before implementation: recommendations below are proposals requiring the
> driver's answer, not assumed approvals. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Generate typed TypeScript stream functions in each package's codec
module, with wasm verification of named payloads, platform streams, and a
docstore program that exercises the generated functions against a Rust server.

**Architecture:** Keep the codec module independent of the WebSocket package.
Pass a runtime-owned context containing a structural byte transport and the
loaded wasm codec. Generate argument encoding and member addresses from the
codegen model; runtime helpers convert stream elements and preserve
cancellation, backpressure, and failures. Keep stream containers out of the typl
surface.

**Tech Stack:** Rust code generators, FlatBuffers object API and wasm verifier,
TypeScript platform streams, Deno at `.deno-version`, existing WebSocket v3
client and Rust server.

**Spec:** `docs/wip/2026-10-31-streams-design.md:759` (section 8, R-23) and
`:1011` (section 9, Stage 6), ADR-0032 decisions 3, 4, 6, 12–14; issue #1247 as
supplied by the driver. Follow the task/test/mutation format of
`docs/archive/2026-11-02-streams-stage-5a-plan.md:339` and
`docs/archive/2026-11-04-streams-stage-5b-plan.md:621`.

## Verified starting point

These are source observations from this checkout, not live issue status. Each
reference below is a repository-relative `file:line`; subsequent tasks refer to
these observations rather than repeating their implementation details.

| ID  | Current code or record                                                                                                                                                                                                                                                                                                     | Evidence                                                                                                                                                                                                                                  |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| C1  | Both codec discovery and interface emission skip streamed members. The existing test requires that omission.                                                                                                                                                                                                               | `crates/ridl-backend-ts/src/codec.rs:287`, `crates/ridl-backend-ts/src/codec.rs:1782`, `crates/ridl-backend-ts/tests/codec.rs:1244`                                                                                                       |
| C2  | The typl generator refuses stream field types; codec converters also refuse a stream as an ordinary type. These are separate from C1.                                                                                                                                                                                      | `crates/ridl-backend-ts/src/lib.rs:631`, `crates/ridl-backend-ts/src/codec.rs:965`, `crates/ridl-backend-ts/tests/codec.rs:378`                                                                                                           |
| C3  | The codec module conditionally imports only the runtime's `PayloadCodec` type. Its constants contain encode/decode and a qualified type name, without verification or a session.                                                                                                                                           | `crates/ridl-backend-ts/src/codec.rs:1101`, `crates/ridl-backend-ts/src/codec.rs:1135`, `packages/ridl-runtime/src/codec.ts:19`                                                                                                           |
| C4  | Verification belongs to a separately loaded `Codec`: resolve a qualified name through `typeId`, then `verify`. Its result distinguishes malformed, rule and unknown-type failures.                                                                                                                                         | `packages/ridl-runtime/src/codec.ts:3`, `packages/ridl-runtime/src/codec.ts:33`, `packages/ridl-runtime/src/codec.ts:255`                                                                                                                 |
| C5  | TypeScript and wasm payload rule tables both contain six sorted texts, without utf8. Generated `RuleError.rule` is the union derived from `RULES`. A compiler integration test compares the two tables.                                                                                                                    | `crates/ridl-backend-ts/src/codec.rs:38`, `crates/ridl-backend-ts/src/codec.rs:1182`, `crates/ridl-backend-rust/src/wasm.rs:35`, `crates/ridlc/tests/codec_table.rs:140`                                                                  |
| C6  | The frame rule union already includes utf8, as detail 7. Its order differs from the payload ABI table; wasm codes are 2 plus the payload-table index.                                                                                                                                                                      | `packages/ridl-transport-ws/src/frame.ts:140`, `crates/ridl-backend-rust/src/wasm.rs:92`                                                                                                                                                  |
| C7  | `Session.queryStream` returns byte chunks; `commandStream` returns upload/accepted; `queryWithUpload` returns upload/plain reply. Each takes `{ bound: number }`. Upload methods address parameter stream 1.                                                                                                               | `packages/ridl-transport-ws/src/session.ts:573`, `packages/ridl-transport-ws/src/session.ts:614`, `packages/ridl-transport-ws/src/session.ts:642`, `packages/ridl-transport-ws/src/session.ts:684`                                        |
| C8  | Reply streams use credit 4 and cancel by sending interrupted. Upload writes await credit, close sends end, and abort sends interrupted. Stream frames are checked against the call's interface, ordinal and kind before use.                                                                                               | `packages/ridl-transport-ws/src/calls.ts:224`, `packages/ridl-transport-ws/src/calls.ts:235`, `packages/ridl-transport-ws/src/calls.ts:383`, `packages/ridl-transport-ws/src/calls.ts:392`, `packages/ridl-transport-ws/src/calls.ts:428` |
| C9  | An upload close confirms local transmission, not provider consumption. A command's acceptance is separate from its upload.                                                                                                                                                                                                 | `packages/ridl-transport-ws/src/session.ts:605`, `docs/design/ridl-transport-ws-ts.md:662`                                                                                                                                                |
| C10 | The model gives stream numbers/forms/elements/sizes, non-stream request roots or induced tuples, original parameters with docs/rules, and timing in decimal microsecond text. Fallible inline replies have no codec root.                                                                                                  | `crates/ridl-ir/proto/ridl/codegen/v1/model.proto:594`, `:619`, `:750`, `:783`, `:819`, `:849`                                                                                                                                            |
| C11 | Codec generation currently needs a supported local/foreign payload root. Name registration already refuses collisions. Foreign struct/union/enum payloads are refused; supported foreign scalar/set codecs use their own modules.                                                                                          | `crates/ridl-backend-ts/src/codec.rs:304`, `:322`, `:1926`                                                                                                                                                                                |
| C12 | Docstore declares fetch/put byte streams and a named-union describe reply. Its manifest enables Rust FlatBuffers only. Its consumer is a separate workspace and proves a 204,800-byte transfer over loopback, with an explicit provider-consumption wait.                                                                  | `examples/docstore/docstore.ridl:31`, `examples/docstore/ridl.toml:5`, `examples/docstore/consumer/Cargo.toml:9`, `examples/docstore/consumer/src/main.rs:124`, `:152`                                                                    |
| C13 | The shared `loopback_server` uses a hand-written test catalog. Its named-stream fixture produces arbitrary 1,024-byte arrays, not generated FlatBuffers elements.                                                                                                                                                          | `crates/ridl-transport-ws/examples/loopback_server.rs:4`, `crates/ridl-transport-ws/tests/support/provider.rs:52`                                                                                                                         |
| C14 | `ts-check` checks both packages, including DOM typing and publish dry runs; runtime tests include a local HTTP server. `ts-e2e` currently runs the transport's two executable fixtures. `_codec-ts-check` generates/lints/checks/tests four codec fixtures and is reached by `cabin-ws-demo`. Browser e2e runs separately. | `justfile:2912`, `:2920`, `:2964`, `:3038`, `:3159`, `:3260`                                                                                                                                                                              |
| C15 | Generated packages depend on the runtime and never on a transport. The current book's TypeScript description is in generated-code, with backend-option prose in cli-reference; codegen-plugins also states the stream omission.                                                                                            | `docs/decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md:48`, `docs/book/generated-code.md:455`, `docs/book/cli-reference.md:1074`, `docs/book/codegen-plugins.md:318`                                               |
| C16 | ADR-0032 is Accepted and its amendment records the missing TypeScript duplex method and multiple-upload limit. The codec-table record describes itself as built. The working design still calls stages 3–6 additive.                                                                                                       | `docs/decisions/ADR-0032-byte-streams.md:3`, `:30`, `docs/design/ts-codec-table.md:15`, `docs/wip/2026-10-31-streams-design.md:1058`                                                                                                      |
| C17 | ADR-0028 requires model-derived interaction docs/rules on public callable items, neutral rule wording, and no repository URLs/issues in generated docs.                                                                                                                                                                    | `docs/decisions/ADR-0028-docs-in-generated-code.md:63`, `:78`, `:152`, `:170`, `:205`                                                                                                                                                     |

## How to work and global constraints

- Work only in
  `/Users/sebastientasson/Workspace/driftsys/ridl-streams-6-duplex`, on the
  driver-managed `feat/streams-6-duplex` branch. Read `AGENTS.md` first. Do not
  commit, push, stash, checkout, reset, or otherwise change git state.
- Stay offline. Set `RUSTC_WRAPPER=` and retain the supplied `CARGO_TARGET_DIR`;
  use `CARGO_NET_OFFLINE=true` for Cargo and `DENO_NO_UPDATE_CHECK=1` for Deno.
  Direct Deno test/check commands use `--cached-only`; do not invoke recipe
  subprocesses that could fetch dependencies in this sandbox. Missing cached
  dependencies are a reported limitation, not permission to fetch.
- Before each task, verify its base and the accepted answers to the questions.
  Stop and report if either differs. Do not implement an unapproved alternative.
- Every new or strengthened behavioral assertion gets a recorded RED run before
  its production change, then GREEN. For a new API, add only its signature and
  an explicit throwing placeholder if needed to let the test run; report that
  exception. A missing import, compile error or missing executable is not the
  behavioral RED. Static type proofs supplement runtime assertions.
- Apply every listed production mutation individually after GREEN, run its named
  test, require an assertion failure, and restore the exact original bytes
  without git operations. Re-run the focused test after restoration. The driver
  reviews evidence before committing; never call a compilation failure a killed
  mutation.
- Each task is one reviewable driver commit, at most two only after the driver
  explicitly splits it. Every commit must leave `just ts-check`,
  `cargo test -p ridl-backend-ts --locked`, `just ts-e2e`, and
  `just cabin-ws-demo` green. From Task 2, `_codec-ts-check` also covers the new
  generated fixture. Run recipes separately, not as arguments to another recipe.
- In the sandbox, TCP tests and recipes are **not run in sandbox**.
  Compile/check their sources and run socket-free unit tests. The driver must
  execute the full recipes before accepting that commit. This applies to the
  HTTP test inside `ts-check` too (C14); do not report its partial execution as
  a green gate.
- Run focused Rust formatting/clippy for changed crates, Deno format/lint/check
  including DOM libraries, and the existing non-stream fixture regressions. The
  driver runs `just verify` before the PR. No release or freeze-baseline
  operation is authorized by this plan.
- Shipped documentation, generated comments and identifiers use literal English;
  no story, stage or epic names in shipped records. Keep those names here and in
  the archived design only. Add no diagnostic, grammar, IR or wire-layout
  change.
- After each implementation task, write its gitignored task report with files,
  RED/GREEN and mutation results, unavailable tests, rulings, questions and the
  proposed commit. The separate report requested for this planning run conflicts
  with its one-file limit; the planning-run report is included below instead.

## Rulings proposed by this plan

Ruling: S6-1 — The owner has already classified widening the TypeScript rule
union with utf8 as breaking and assigned it to the next breaking release. Task 1
therefore uses `!`. Add utf8 to both sorted payload tables in one commit (C5):
`length, pattern, range, step, unique, utf8, variant`. This gives wasm payload
codes 2–8, moves variant from 7 to 8, and gives utf8 code 7. These are payload
ABI codes, not the frame's detail-code order (C6). Regenerate the wasm codec and
TypeScript module together. This corrects the design's additive claim (C16). The
sorted insertion, rather than appending, needs the driver's Q2 answer.

Ruling: S6-2 — Amended by the owner on 2026-11-04: duplex queries with exactly
one parameter stream are in scope. Task 0 implements the byte transport method
for #1240; Task 3b generates typed duplex functions. Two or more parameter
streams (#1224) stay out, with an explicit generated omission. Never represent
an unsupported stream member as a plain call.

**Superseded 2026-11-04:**

Ruling: S6-2 — Recommend keeping #1240 outside this stage. The supplied driver
state says it is OPEN; no network verification is possible. Support reply-only
streams and exactly one parameter stream on commands/plain-reply queries. Leave
duplex and two-or-more-upload members out, with distinct generated comments and
links in maintained records to #1240 and #1224. Never turn one into a plain
call. Q1 and Q4 gate this scope reduction from “every stream form”.

Ruling: S6-3 — Recommend a runtime-owned `StreamSession` context, passed as the
first argument, containing `{ transport, codec }`. `transport` is structural;
the WebSocket `Session` satisfies it without an import from the transport in
generated code. Call `session.transport.queryStream(...)` as a method so its
receiver is retained. No global codec, singleton session or hidden wasm load.
This supplies the verification capability missing from C3 while preserving the
session-first intent of design section 8. Q3 gates this proposed refinement.

Ruling: S6-4 — Recommend exported names `<Interface>_<declaredMember>`, for
example `DocStore_fetch`, and required final `opts: StreamOptions` containing
`bound: number` in milliseconds. Prefixing avoids the common repeated member
name across interfaces. Register the complete generated name and imported helper
aliases in the existing collision checker; refuse remaining collisions with both
source paths. Do not silently rename them. The design's unqualified names and
optional `opts?` are illustrative but do not define collision/default-bound
rules; Q3 must approve this refinement before generation starts.

Ruling: S6-5 — A named element is encoded and verified as one whole payload per
chunk; never split its encoded bytes. Raw byte writes may be divided at the
member limit. Text writes are encoded with TextEncoder, divided only between
complete code points, and reject unpaired JavaScript surrogates rather than
silently replacing them. A single code point or named encoding that exceeds the
limit fails locally as corrupt and interrupts the upload. A zero limit fails a
generated streaming call before sending a request. Q5 gates these previously
unspecified sender choices. Named bytes/string types remain named elements.

Ruling: S6-6 — Recommend a runtime `StreamError` for locally detected failures,
with a structural `failure` property: corrupt for malformed payloads, or
contract / invalid-value / rule for constraint and utf8 failures. Keep remote
transport errors unchanged, including their identity. Do not promise
`instanceof
SessionError` for local errors. Unknown payload types and mismatched
wasm catalog hashes are setup errors, reported before a request. Transform
errors cancel the raw stream through normal pipe propagation; this currently
sends interrupted (C8), not a newly invented outbound contract frame. Q6 gates
this error contract.

Ruling: S6-7 — Recommend an additional, generated-type docstore loopback server
for typed end-to-end proof. Keep the existing raw transport fixtures as
transport regressions (C13). Test 64 named chunks on the docstore server rather
than call the arbitrary bytes of its existing fixture FlatBuffers. This
substitutes for section 9's exact `faults()` / `loopback_server` acceptance
case; Q7 must approve that substitution, or the driver must commission the
separate generated catalog integration into the existing server before Task 4.

## Review Focus

- Cancellation and local verification failure must release the underlying
  transport call and its timer; test in Tasks 1–3, including errors while credit
  is exhausted. Avoid detached `pipeTo` promises.
- Slow readers/writers must preserve bounded buffering and ordering; test 64
  elements with an idle reader, and a write waiting after four credits (Tasks
  1–3).
- Primitive text versus a named text/bytes type, malformed UTF-8, split code
  points and lone surrogates must have distinct behavior (Tasks 1–3).
- Empty requests, several non-stream arguments, optional named arguments,
  custom/zero limits, internal/foreign types and generated-name collisions must
  not produce an incomplete call or an unusable module (Tasks 2–3).
- Upload close is not provider consumption. A pending refusal must reach the
  typed writer without changing an already-resolved command acknowledgment; the
  docstore read-back must wait for provider completion (Tasks 3–4).

### Task 0: TypeScript client duplex method (#1240)

**Base and scheduling:** Owner decision of 2026-11-04 supersedes S6-2 and Q1.
This task can run in parallel with Tasks 1–3 because its production changes are
confined to the transport package. Task 3b depends on it. This run stops after
Task 0; it does not execute the remaining tasks or resolve Q2–Q7.

**Files:** `packages/ridl-transport-ws/src/{session,calls,duplex}.ts`,
`packages/ridl-transport-ws/tests/duplex_test.ts`,
`packages/ridl-transport-ws/e2e/{catalog,deno_test}.ts`, the package README,
`docs/design/ridl-transport-ws-ts.md` (D-23), and the missing-method line in
`docs/decisions/ADR-0032-byte-streams.md`. Only if necessary, extend test
support under `crates/ridl-transport-ws/`; the existing provider already has
`DUPLEX`, ordinal 14, in `tests/support/provider.rs:71`.

**Interface:** Add `Session.queryDuplex(iface, ord, args, opts)` with the same
argument types and required `{ bound: number }` as queryStream/queryWithUpload:

```ts
{ upload: WritableStream<Uint8Array>; stream: ReadableStream<Uint8Array> }
```

Use `duplex`, matching the server tests and Rust face's `call.duplex()`
(`crates/ridl-backend-rust/src/face/upload_query.rs:18`). Rust generates member
names rather than a separate generic client method. One request/correlation owns
both directions: upload stream 1 and reply stream 0, each with initial credit 4
and independent sequence/credit counters. Check the target before applying
either direction's frames. Preserve queryStream's reply limits, sequence checks,
refusal rules and credit replenishment; preserve queryWithUpload's write limits
and credit waiting. Reply cancel interrupts the whole call and releases a
waiting write; upload abort interrupts the whole call and releases a waiting
read/write, including abort during a credit-blocked write. Refusal errors both
sides with the same error. Normal upload close confirms local transmission only
and leaves the reply open; reply end interrupts an upload still open. The
first-reply-frame bound runs from the request and stops on the first reply
chunk/end, not on upload credit or upload close.

Authority: `docs/specification/frame-specification.md` §5.3, §5.4 and §11.1
binding version 3; server `src/server/session/tests/upload.rs:1087` (reply
before upload end), `:1118` (reply end interrupts upload), `:1162` (reply after
upload end). R-5b-T3-4's owner-approved abandonment rule is already implemented
at `crates/ridl-transport-ws/src/server/session.rs:1245`: abandon only when
**neither** direction progressed for `call_abandon_after`, including upload
activity after its end. Do not add a competing client inactivity timer or alter
the server. Existing server controls at `tests/upload.rs:1261`, `:1297` and
`:1352` prove each active direction and the both-idle case.

- [ ] **Write semantic RED first.** Add a signature with an explicit throwing
      placeholder only if necessary; a compile error is not RED. Run all
      socket-free rows before implementing the state machine.

| Test                                 | Required assertions                                                                                                                                                                                                         | Production mutation it must kill                                                             |
| ------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `duplex_credit_and_close`            | One request, independent stream numbers/sequences; fifth upload write waits for stream-1 credit; reply arrives before close; close leaves reply readable through a later normal end; reply pulls replenish stream-0 credit. | Remove upload credit waiting; forget on upload close; suppress reply credit.                 |
| `duplex_target_check`                | Wrong interface, ordinal or kind credit/chunk/end changes neither direction and records a discard; valid frames still work.                                                                                                 | Bypass target matching for duplex.                                                           |
| `duplex_cancel_and_abort`            | Reply cancel errors a credit-blocked writer and sends one stream-0 interruption; abort while credit-blocked errors the reply and releases the writer, sends one stream-1 interruption, clears timers.                       | Fail only the initiating side; wait for sink abort behind a blocked write.                   |
| `duplex_refusal_and_end`             | Response refusal, upload refusal and reply failure share their exact error across both sides; reply normal end interrupts open upload; session down errors both.                                                            | Route refusal only to one side; leave upload open on reply end; omit duplex session failure. |
| `duplex_reply_validation`            | Sequence gap is interrupted; oversize reply is corrupt; excess reply credit or invalid stream direction ends the session; plain response is a violation.                                                                    | Skip reply sequence, size, credit or form checks.                                            |
| `duplex_upload_validation`           | Oversize upload sends no chunk and fails both; a send exception retains its identity across both streams and leaves no live call.                                                                                           | Skip upload size check; swallow send failure.                                                |
| `duplex_bound`                       | No reply frame before request bound errors both even with upload progress; first reply frame stops the timer; close does not restart it.                                                                                    | Start bound at upload close; fail to clear first-frame timer.                                |
| `duplex_preflight`                   | Invalid numbers/bounds/args, ended session and versions 1/2 error both with the same object and send no request.                                                                                                            | Skip duplex preflight/version checks.                                                        |
| `queryDuplex_echoes_64_chunks` (e2e) | Against loopback_server DUPLEX, read the first echo before close, concurrently send/read 64 deterministic chunks with exact order/content, then normal end; finite deadline.                                                | Suppress upload/reply credit or end the reply at upload close.                               |
| `queryDuplex_cancel_and_abort` (e2e) | Cancel/abort duplex and successfully run a later ordinary query on the same session; the other local side fails interrupted.                                                                                                | Leave the other local side open or close the session when interrupting a call.               |

- [ ] **Implement** a single in-flight duplex call owning the two stream states;
      keep ordinary calls and existing upload-only methods unchanged. Observe
      failures without detached rejected promises.
- [ ] **Run GREEN and each mutation individually**, restore exact bytes and
      re-run the focused unit suite. Run transport Deno fmt/lint/check, DOM
      check and all socket-free transport tests with `--cached-only`. Check the
      e2e sources. TCP e2e and its live mutations: **not run in sandbox**; the
      driver runs `just ts-e2e` and the listed e2e mutations against
      loopback_server. Run existing server abandonment unit tests offline if
      available. Update the transport design and ADR limit line against code.
- [ ] **Driver commit proposal:**
      `feat(ridl-transport-ws): add TypeScript duplex queries`. Use
      `Closes #1240` only after the complete done-when, including driver-run TCP
      e2e, passes.

### Task 1: Runtime decoding helpers and the breaking utf8 payload rule

**Base:** Current checkout, after answers to Q2, Q3, Q5 and Q6 are accepted.

**Files:**

- Create `packages/ridl-runtime/src/streams.ts` and
  `packages/ridl-runtime/tests/streams_test.ts`.
- Modify `packages/ridl-runtime/mod.ts`, `src/errors.ts`, and `README.md` for
  the public exports and their setup/error contract.
- Modify `crates/ridl-backend-ts/src/codec.rs` (`RULES`) and its existing codec
  snapshots under `crates/ridl-backend-ts/tests/snapshots/`.
- Modify `crates/ridl-backend-rust/src/wasm.rs` (`RULES`),
  `crates/ridl-backend-rust/tests/descriptor_generation.rs`, and
  `crates/ridlc/tests/codec_table.rs`.

**Interfaces:** Export from the runtime:

```ts
export interface StreamOptions { readonly bound: number }
export interface StreamTransport {
  queryStream(iface: number, ord: number, args: Uint8Array,
    opts: StreamOptions): ReadableStream<Uint8Array>;
  commandStream(iface: number, ord: number, args: Uint8Array,
    opts: StreamOptions): { upload: WritableStream<Uint8Array>; accepted: Promise<void> };
  queryWithUpload(iface: number, ord: number, args: Uint8Array,
    opts: StreamOptions): { upload: WritableStream<Uint8Array>; reply: Promise<Uint8Array> };
  queryDuplex(iface: number, ord: number, args: Uint8Array,
    opts: StreamOptions): { upload: WritableStream<Uint8Array>; stream: ReadableStream<Uint8Array> };
}
export interface StreamSession {
  readonly transport: StreamTransport;
  readonly codec: Codec;
}
export function decodeStream<T>(decode: (bytes: Uint8Array) => T,
  verify: (bytes: Uint8Array) => VerifyResult): TransformStream<Uint8Array, T>;
export function decodeTextStream(): TransformStream<Uint8Array, string>;
```

`Codec` and `VerifyResult` are the existing runtime types (C4). `StreamError`
extends Error; its `failure` is exactly S6-6's corrupt or invalid-value shape.
Export a `PayloadRule` string union with S6-1's seven texts for the local error.
Do not narrow the existing `VerifyResult.rule?: string` API. Unknown rule texts
or an unknown-type verification result throw `CodecError`, never successful
data.

- [ ] **Write and run RED tests**, one row at a time before implementing it:

| Test                                                           | Assertion                                                                                                                                                                            | Production mutation it must catch after GREEN                                                                                  |
| -------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------ |
| `payload_rules_include_utf8_in_both_emits` (ridlc codec_table) | Both emitted lists equal the explicit seven-text array, not just each other. Pin utf8=7 and variant=8 in generated rule-code arms.                                                   | Delete utf8 from either production table; separately swap the Utf8/Variant variant-name strings in the Rust generator's table. |
| `decodes_64_verified_elements`                                 | A numbered 64-chunk source yields exactly 0–63; verify precedes each decode; end closes normally.                                                                                    | Skip verify; separately drop the final element or decode before verify.                                                        |
| `invalid_payload_never_reaches_decode`                         | Parameterize malformed/range/unknown-type results: decoder call count stays zero; errors are corrupt / contract-range / CodecError respectively. Unknown rule text also fails setup. | Treat a failed verify as success; separately map all verification errors to corrupt.                                           |
| `text_chunks_are_independently_valid_utf8`                     | Decode ASCII, a four-byte character, BOM and empty chunks; preserve BOM as content. Invalid bytes and two individually invalid halves of a code point reject with contract-utf8.     | Remove fatal decoding; separately enable streaming TextDecoder state or strip BOM.                                             |
| `transform_cancel_and_error_cancel_upstream`                   | Reader cancel and a verification failure invoke the source's cancel once; the failed chunk is never delivered.                                                                       | Swallow verification errors inside the production transformer instead of throwing.                                             |
| `idle_reader_does_not_drain_source`                            | With an upstream high-water mark of zero, 64 available values are not all pulled while the output is unread; after reads, all arrive in order.                                       | Replace the transform with an eager collection of the entire source.                                                           |

- [ ] **Implement** minimal helpers with per-chunk checking, fatal TextDecoder
      decoding without cross-chunk state (`ignoreBOM: true`), and bounded native
      stream queues. Keep framing, credit and target checking in the transport.
      Update the two tables together. Strengthen the existing six-rule assertion
      at `crates/ridl-backend-rust/tests/descriptor_generation.rs:1101`, not
      just snapshots. Keep unknown-rule fallback coverage at `:1193`.
- [ ] **Run GREEN and each mutation**, then restore. Commands: from the runtime
      directory `deno test --cached-only tests/streams_test.ts`; from the root
      `cargo test -p ridlc --locked --test codec_table` and
      `cargo test -p ridl-backend-rust --locked --test descriptor_generation`.
      Run the per-commit gates above; all must pass on the driver's machine.
- [ ] **Driver commit proposal:**
      `feat(ts-runtime)!: add verified stream decoding and utf8 payload rules`.
      Body:
      `BREAKING CHANGE: generated TypeScript rule unions include utf8;
      regenerate TypeScript and wasm codecs together. Sorted payload rule codes
      assign utf8 7 and variant 8.`
      No version bump is performed in this task.

### Task 2: Generate reply-stream functions and their metadata

**Base:** Task 1 accepted and committed by the driver; Q1, Q3 and Q4 answered.

**Files:** Modify `crates/ridl-backend-ts/src/codec.rs`,
`crates/ridl-backend-ts/tests/codec.rs`, and affected codec snapshots. Create
`crates/ridl-backend-ts/tests/fixtures/codec-streams/{ridl.toml,streams.ridl,deno.json,generated-only.deno.json,streams_test.ts,browser.deno.json}`.
Extend `justfile`'s `_codec-ts-check` with this fixture in the same commit.

**Interfaces:** A streamed reply produces
`<Interface>_<member>(session: StreamSession, ...nonStreamParams, opts: StreamOptions): ReadableStream<Element>`.
For docstore this is
`DocStore_fetch(session: StreamSession, id: t.DocId, opts: StreamOptions): ReadableStream<Uint8Array>`.
Named elements use their generated value type, raw bytes Uint8Array, raw text
string. Parameters retain declaration order and their existing TypeScript type;
an optional parameter uses `T | undefined` if a required `opts` follows it.

Generate a stream-marked interface-table entry with iface, ord, kind, resolved
limit when present, `request: PayloadCodec<Args>`, and
`streams: readonly { no: number; form: "elements" | "bytes" | "text"; element?: PayloadCodec<T> }[]`.
Keep the stream array in model stream-number order. Do not put the element codec
in a plain `reply` role. Existing non-stream entries keep their shape. The
entry's stream data is additive metadata; the transport still owns addressing.
Allocate local argument bindings against the context/options/helper names;
retain a declared parameter name when it is safe and append the first available
numeric suffix otherwise. Preserve positional order and render TSDoc with the
actual generated binding. Include source parameters named `session` and `opts`
in the fixture so that the signature cannot accidentally declare either twice.

- [ ] **Write and run RED tests** before removing either skip in C1:

| Test                                                      | Assertion                                                                                                                                                                                                                                                                                                    | Production mutation                                                                                                    |
| --------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------- |
| `stream_reply_exports_match_model` (Rust)                 | Independently enumerate supported streamed members and compare the entire exported-function set, return types, table addresses and stream numbers. Include named scalar, struct, union, open enum value, named bytes/string, raw bytes/text, zero/one/several arguments, a tombstone and an ordinary member. | Skip one stream form or only the last live member; separately use row index instead of slot ordinal.                   |
| `generated_reply_uses_codec_and_session` (Deno fixture)   | Recording transport receives the exact generated interface/ordinal, encoded non-stream arguments and bound. Record its method receiver. Verify with a fake Codec before decode, and deliver 64 typed elements. Bytes pass unchanged; text is decoded.                                                        | Call the wrong transport method, lose its receiver, omit an argument, or bypass verification.                          |
| `stream_tables_keep_limits_and_roles` (Rust/Deno fixture) | A manifest-lowered limit appears in the entry; stream entries have request/streams and no plain reply; ordinary members retain their old codecs. A raw-only package still emits a usable module and lints.                                                                                                   | Drop the limit, give a stream the plain reply role, or suppress module generation when no named stream element exists. |
| `setup_errors_send_no_request` (Deno fixture)             | Wrong codec catalog hash, unresolved element type and zero limit fail before any transport method call. Hash comparison uses all 32 bytes.                                                                                                                                                                   | Remove the setup check or compare only the first hash byte.                                                            |
| `stream_names_and_unsupported_shapes_are_explicit` (Rust) | Two interfaces with a `fetch` both get functions. A concatenation collision/helper-alias collision is refused naming both sources. Duplex remains absent until Task 3b replaces that temporary omission; multiple uploads remain absent with a specific reason comment; ordinary stream field refusal stays. | Disable name registration; separately erase the unsupported-shape guard and emit a plain call.                         |
| `generated_stream_docs_render_model_contract` (Rust)      | Function TSDoc contains model doc, resolved links, canonical rule text, since/deprecation and named parameter docs; no issue URL, generated checks claim or private-helper docs.                                                                                                                             | Omit rules or source doc metadata from the new emitter.                                                                |

- [ ] **Implement** stream classification separately from ordinary type
      conversion. Use model StreamShape and request/request_tuple (C10), not raw
      IR or an inference from a missing reply payload. Resolve and register
      every required request/element codec during discovery, before name
      checking. Reuse supported foreign scalar/set codecs and retain current
      foreign composite/internal/unbounded codec limits, with an explanatory
      omission comment where a role has no finite codec. Test these cases in the
      Rust matrix; never emit `unknown` or a partial function to hide a missing
      codec. Fail malformed stream metadata with GenerateError, not a new
      diagnostic.
- [ ] **Compose** named reply bytes with `decodeStream`; compose text with
      `decodeTextStream`; leave bytes as the raw ReadableStream. Compare the
      loaded codec's catalogHash with generated CATALOG_HASH, resolve type ids
      before opening the call, encode and verify the request before sending. Map
      local request failures through S6-6. The caller connects the transport
      with the generated catalog and `encoding: "flatbuffers"`; the structural
      context cannot inspect a transport's private attachment state. Derive
      imports from names actually used: an induced-request-only module must not
      import an unused domain namespace or a nonexistent flatc barrel.
- [ ] **Handle docs through model data** (C17). Reuse the existing doc renderer
      where possible; no new doc-rule evaluation. Render parameter tags for
      arguments exposed by this callable. Keep domain constraints on domain
      types, plus the interaction's own rules on its callable.
- [ ] **Run GREEN/mutations** using
      `cargo test -p ridl-backend-ts --locked --test codec` and
      `just _codec-ts-check`. The new recipe segment follows the existing
      fixture pattern: generate TypeScript/FBS, flatc object API, remap the
      local runtime, lint/check the emitted codec and tests, check DOM typings,
      run tests. It must have no import map entry for the WebSocket package;
      assert the emitted import list has no transport specifier. No new public
      recipe or CI job is necessary because `_codec-ts-check` is already reached
      (C14).
- [ ] **Driver commit proposal:**
      `feat(ridl-backend-ts): generate verified reply stream functions`.

### Task 3: Generate typed uploads for commands and plain-reply queries

Ruling: R-6-T3-3 — Use native writers; close after error rejects TypeError.

Ruling: R-6-T3-5 — Drop empty query replies: RIDL-105 rejects `()` returns.

**Base:** Task 2 accepted; Q4–Q6 answered. The fixture's upload cases are added
here, so Task 2 does not leave tests expecting unimplemented upload exports.

**Files:** Modify runtime `src/streams.ts`, `mod.ts`, `tests/streams_test.ts`;
TS generator `src/codec.rs`, `tests/codec.rs`; extend `codec-streams` sources,
tests and snapshots. Extend the existing table-omission test in C1 to expect the
supported upload entries and functions, retaining unsupported cases.

**Interfaces:** Runtime adds:

```ts
export function encodeUpload<T>(upload: WritableStream<Uint8Array>,
  encode: (value: T) => Uint8Array,
  verify: (bytes: Uint8Array) => VerifyResult,
  limit: number): WritableStream<T>;
export function byteUpload(upload: WritableStream<Uint8Array>,
  limit: number): WritableStream<Uint8Array>;
export function textUpload(upload: WritableStream<Uint8Array>,
  limit: number): WritableStream<string>;
```

Generate `<Interface>_<command>(session, ...nonStreamParams, opts)` returning
`{ upload: WritableStream<Element>; accepted: Promise<void> }`; an upload query
returns `{ upload: WritableStream<Element>; reply: Promise<Reply> }`. Use
named/induced plain reply codecs, including a named result union and an induced
tuple reply. An inline `T | E` with no codec remains an explicit omission; it is
not raw bytes disguised as a typed reply. These functions return a writable;
they do not take a pre-existing writable parameter. This is section 8's
`uploadFirmware` shape and needs Q4's acceptance of issue wording.

- [ ] **Write and run RED tests**:

| Test                                            | Assertion                                                                                                                                                                                                                                                                                                        | Production mutation                                                                        |
| ----------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| `named_upload_is_one_verified_chunk`            | Two values yield exactly two independently verified encoded chunks. Invalid range sends neither that chunk nor any later one. A named bytes type is not split.                                                                                                                                                   | Skip verification or pass a named encoding through the raw-byte splitter.                  |
| `raw_uploads_split_at_member_limit`             | With limit 5, 12 bytes yield 5/5/2, preserving all bytes. Empty writes preserve an empty chunk. Text containing a four-byte code point produces individually valid UTF-8 chunks and exact rejoined text.                                                                                                         | Use one raw write for the entire input; separately split text at arbitrary byte offsets.   |
| `unrepresentable_text_and_elements_abort`       | A lone surrogate is contract-utf8. A code point too large for limit 3 and a named element over limit reject corrupt and abort underlying upload once. Zero limit never opens a call.                                                                                                                             | Let TextEncoder replace surrogates; separately transmit an oversized named element.        |
| `typed_write_waits_for_credit_and_close`        | A controllable raw writer keeps the typed write pending until the raw write resolves. With FakeSocket, write 5 stays pending after four credits. Close waits for queued writes and forwards one end.                                                                                                             | Resolve typed write on encode/enqueue, without awaiting the raw writer's write.            |
| `remote_refusal_errors_typed_writer`            | After accepted resolves, inject contract-range and contract-utf8 while a typed write awaits credit. Pending/later writes, writer.closed, and a close queued before failure reject with the same remote error; late close follows the platform TypeError. Accepted stays resolved. No unhandled rejection occurs. | Ignore raw writer.closed rejection or replace the remote error with a generic local error. |
| `abort_releases_a_credit_waiter`                | Abort while a write is credit-blocked rejects it, interrupts once, releases locks/timers and does not send another chunk.                                                                                                                                                                                        | Wait for the blocked write before forwarding abort.                                        |
| `generated_upload_encodes_only_plain_arguments` | A stream between two ordinary parameters is excluded from request encoding. Correct method, target, bound and typed writable/result are used for command and query. A named scalar uses its root box, multiple arguments their induced struct.                                                                   | Include the stream parameter in request packing or call commandStream for a query.         |
| `plain_reply_is_verified_before_decode`         | A valid named-union/tuple reply decodes to its value. Malformed/rule-invalid replies reject without decoder invocation. Early provider reply errors the open typed upload while resolving the valid reply.                                                                                                       | Decode before verify or disconnect raw upload failure from typed writable.                 |

- [ ] **Implement** writable adapters over a raw writer. Named writes await the
      actual writer.write; raw/text splitting awaits each chunk, so credit is
      not bypassed. Forward close and abort, observe writer.closed without
      detached rejection, and propagate failures through the exposed controller.
      Take and release writer locks exactly once. If platform abort ordering
      makes the proposed behavior impossible with the current raw writer, stop
      with that evidence; do not alter transport semantics without a driver
      ruling.
- [ ] **Generate** upload functions using commandStream/queryWithUpload and the
      corresponding adapter. Keep the transport's accepted promise unchanged.
      Decode the query's reply through its wasm-verified codec. Extend
      discovery, collisions and TSDoc from Task 2 to upload/ordinary-result
      roles. Do not alter the transport's credit, sequence, target check or
      response timers.
- [ ] **Run GREEN/mutations**: runtime streams tests; backend codec tests;
      `_codec-ts-check` with a local FakeSocket import solely in test code. Add
      the test's import-map mapping for that test file, while separately proving
      the generated module checks under `generated-only.deno.json`, whose only
      package mappings are flatbuffers and the local runtime. Run the FakeSocket
      subset without network permissions. Run per-commit gates.
- [ ] **Driver commit proposal:**
      `feat(ridl-backend-ts): generate verified stream upload functions`.

### Task 3b: generate duplex functions

**Base:** Tasks 0–3 accepted. Changes belong to the runtime's structural
StreamTransport interface (queryDuplex signature specified in Task 1), runtime
stream adapters/tests, and the backend codec generator/tests/stream fixtures. No
transport-package import is allowed in generated code or the runtime.

**Interface:** Generate `<Interface>_<member>(session, ...plainArgs, opts)`
returning `{ upload: WritableStream<Element>; stream: ReadableStream<Reply> }`
for a query with exactly one parameter stream and a streamed reply. Use Task 3's
upload adapter and Task 1's verifying decode helpers over Task 0's queryDuplex.
Exclude the stream argument from request encoding. Preserve names, metadata,
codec-root discovery, collision checking and model-derived TSDoc from Tasks 2–3.
Both local adapter failures cancel/abort the raw call and reach the other side;
retain remote error identity. Multiple uploads (#1224) remain omitted.

- [ ] **Write RED**, then implement the composition and run GREEN:

| Test                                        | Required assertions                                                                                                                                                             | Production mutation it must kill                                   |
| ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------ |
| `generated_duplex_functions`                | Correct queryDuplex address, plain args and bound; typed named upload and verified named reply; request has no stream argument. Generated-only type check imports runtime only. | Call queryWithUpload or encode the stream argument.                |
| `duplex_adapters_propagate_failure`         | Upload verification/abort errors reply; reply verification/cancel errors upload, including a write waiting for credit; remote error identity remains unchanged.                 | Disconnect adapter cancellation/abort propagation.                 |
| `duplex_adapters_preserve_credit_and_close` | 64 named elements with bounded buffering; upload close permits later decoded replies and normal end.                                                                            | Resolve writes before raw credit; close decoded reply with upload. |

- [ ] **Run mutations**, runtime/backend tests, generated-only type proof,
      FakeSocket integration and per-commit gates; socket e2e is driver-run.
- [ ] **Driver commit proposal:**
      `feat(ridl-backend-ts): generate verified duplex stream functions`.

### Task 4: Generated docstore functions against a Rust server

**Base:** Tasks 0–3b accepted, Q7 answered. Duplex is included by the owner
decision of 2026-11-04.

**Files:** Modify `examples/docstore/ridl.toml`, `docstore.ridl`, `.gitignore`
and `justfile`. Create
`examples/docstore/server/{Cargo.toml,Cargo.lock,src/main.rs}` and
`examples/docstore/web/{deno.json,main.ts,check.ts,check_test.ts,server.ts}`.
Keep the server in its own Cargo workspace, like the existing consumer (C12),
with local path dependencies and a local ridl-rt patch. Keep generated Rust in
the already-ignored `examples/docstore/generated/`; ignore
`examples/docstore/web/generated/`. No generated sources are committed.

**Interfaces:** Enable `[backend.typescript] codec = true` for docstore. Keep
fetch/put/describe, and add a documented named element `DocChunk` with an index
(`ChunkIndex`, integer [0..65535]) and bytes (`DocBlock`, bytes [1..3200]), plus
`query chunks(id: DocId): <DocChunk> @[..1s]`. The provider yields chunks in
index order, at most 3,200 bytes each. This gives a real named-element path
without replacing docstore's raw-byte API. Add documented duplex query
`exchange(id: DocId, chunks: <DocChunk>): <DocChunk> @[..1s]`, echoing each
named chunk before upload end. Exercise `DocStore_exchange` in the typed e2e: 64
exact indexed values, concurrent write/read, first reply before upload end,
normal end after upload close, and cancellation/abort propagation. Add no text
API just for tests. Negative controls drop, duplicate, reorder or alter a duplex
reply too; mutations bypass exact content/order/count and end checks.

The server binds loopback port 0, announces port/hash and stops when stdin ends,
following `examples/cabin/server/src/main.rs:5`. It drives the generated
DocStore provider on ridl-loopback and serves it through the existing Streaming
backend/catalog member table. Uploaded contents become visible at explicit
provider end, and fetch/chunks wait for a pending put of that document to
finish. `accepted` and writer.close must not be treated as that completion
barrier.

- [ ] **Write semantic RED controls first.** `check_test.ts` checks the
      consumer's validator with 204,800 deterministic bytes (`i % 251`): exact
      byte content and count, plus exactly 64 DocChunk values with indices 0–63
      and 3,200 bytes each. A dropped, duplicated, reordered or modified chunk
      and premature end must each fail. Mutation: remove the exact-content,
      order or end/count check respectively, and require the matching negative
      control to fail its assertion.
- [ ] **Write provider unit test** `download_waits_for_upload_end`: construct
      the provider directly, poll fetch/chunks while put remains open and
      require Pending; end the upload and require the complete committed
      bytes/elements. Mutation: publish partial upload state or snapshot it
      before upload end. It runs without TCP. The initial test can use a
      provider implementation that compiles but returns an empty source,
      producing an assertion RED.
- [ ] **Build the server and program.** Reuse provider behavior from docstore's
      consumer as a reference, not a raw byte test-catalog substitute. Main
      loads the generated wasm with CATALOG_HASH/PAYLOAD_TYPES/RULES, attaches
      with the generated catalog, tables and FlatBuffers, then passes
      `{ transport, codec }` to `DocStore_put`, `DocStore_fetch` and
      `DocStore_chunks` and `DocStore_exchange`. It checks the byte, named and
      duplex transfers, normal end, server exit, and a finite overall deadline.
      Keep ordinary describe available; do not add ordinary-call codegen here.
- [ ] **Add a private generation/build helper** `_docstore-web-generate` to the
      justfile and invoke it from `ts-e2e` before the program. Generate Rust,
      catalog, TypeScript and FBS, run pinned flatc, build the wasm with the
      same cdylib/wasm-exports procedure as `_cabin-wasm` (`justfile:3099`),
      lint/check the web sources, run their unit tests and build/lint the
      server. Respect CARGO_TARGET_DIR, unique binary `docstore-server`, local
      runtime patches and an offline-created committed server lockfile. Existing
      ts-e2e tests still run. No new build member means no gate-parity list
      addition; run gate-parity to prove existing recipe wiring remains valid.
- [ ] **Run the integrated RED/GREEN on the driver.** After unit GREEN, change
      the server to omit its final named chunk, run `just ts-e2e` and require an
      assertion failure from the generated consumer, then restore and pass.
      Separately corrupt a fetched byte. Re-run existing
      `cargo test -p ridlc --locked --test docstore_example` to retain the Rust
      consumer proof, plus the per-commit gates. Socket tests: **not run in
      sandbox**; compile the server and check the Deno program here. A missing
      server before its implementation is not the semantic RED.
- [ ] **Driver commit proposal:**
      `feat(ts-runtime): exercise generated docstore streams over WebSocket`.

### Task 5: Record the generated surface and archive the working design

**Base:** Tasks 0–4 (including 3b) reviewed, mutations killed, and all
driver-run gates green. Use the actual merge date in amendments, not an assumed
publication date.

**Files and required final statements:**

| Record                                                                      | What it must state after implementation                                                                                                                                                                                                                                                                                                                                                                                                                          |
| --------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `docs/book/generated-code.md`, TypeScript and checks sections               | Distinguish the `.ts` types from `.codec.ts` stream functions; show loading wasm, connecting the transport, passing the runtime context, fetch/put/chunks/exchange, actual function names/options/result shapes and local/remote errors. Explain verification before data reaches application code, named versus raw chunking, credit/cancel propagation and the close/consumption distinction. Preserve the typl stream refusal and explicit unsupported forms. |
| `docs/book/cli-reference.md`, TypeScript backend-option prose               | codec=true now also emits stream functions and metadata; point to the generated-code chapter for use. Keep the FlatBuffers/wasm/flatc requirements accurate; do not invent a separate TypeScript backend chapter (C15).                                                                                                                                                                                                                                          |
| `docs/book/codegen-plugins.md:318`                                          | Replace the blanket TypeScript omission with the precise supported subset and remaining omissions. Keep the warning against representing stream members as plain calls.                                                                                                                                                                                                                                                                                          |
| `docs/design/ts-codec-table.md`                                             | Preserve D-1–D-15 and append numbered decisions for the accepted S6 choices. Describe discovery of request/element/result codecs, stream-table shape, names and collision checks, runtime-only imports, StreamSession setup, errors, sender splitting, verifier use and test coverage. Correct the statements that every stream is skipped and the only runtime import is PayloadCodec. Record the breaking utf8 union/table and wasm code pairing.              |
| `docs/decisions/ADR-0032-byte-streams.md`, Status amendment                 | Keep Accepted. Record the TypeScript mappings now built, the evidence recipes, and exact remaining limits (#1224 and unavailable codec roots); remove the missing-duplex limit and describe queryDuplex and generated duplex functions. Retain Rust-side limits and the upload-close qualification; do not claim all stream shapes or all target languages are implemented.                                                                                      |
| `docs/decisions/ADR-0028-docs-in-generated-code.md`, narrow dated amendment | Correct the historical “TypeScript emits no face/has no parameter to render” statements for codec stream functions. State their model-derived interaction and parameter documentation contract; keep neutral rules and private-helper policy.                                                                                                                                                                                                                    |
| Runtime `README.md` and exported TSDoc                                      | Confirm the signatures/errors delivered by Tasks 1 and 3 and give the context construction example. State no dependency on the WebSocket package. Do not duplicate the entire codec-table record.                                                                                                                                                                                                                                                                |
| Working design archive                                                      | Append “As built (stage 6)” with driver answers, accepted S6 rulings, the utf8 breaking-release correction, tests and remaining limits; then move `docs/wip/2026-10-31-streams-design.md` to `docs/archive/2026-10-31-streams-design.md`. Update active references, including this plan, to the archived path. Preserve earlier as-built sections. Do not archive the completed stage-6 implementation plan until the driver's normal gardening step.            |

- [ ] **Write records against code and accepted reports.** Inspect all
      references to the moved path with `rg`; update live references and verify
      archived relative links. No git move command is needed in the sandbox; the
      driver owns staging the eventual file move. Correct
      accepted-surface/release wording where necessary, without changing freeze
      baselines.
- [ ] **Run** `just book-check`, `just link-check`, `just doc-path-check`,
      `just story-id-check`, `just check`, and the per-commit gates. Generated
      documentation tests from Task 2 must still pass. No new mirrored prose
      test or production mutation is required for this documentation-only task;
      its verification is link/build/content review against implementation.
- [ ] **Driver commit proposal:**
      `docs(ridl-backend-ts): record generated stream functions and limits`.

## Self-review and planning-run report

- Original planning run: built this five-task plan only. The 2026-11-04
  amendment adds Tasks 0 and 3b; its execution report is
  `.superpowers/codex/6-task-0-report.md`. No production change or test was
  added; no semantic RED/GREEN run or production mutation was performed in the
  planning run. All test/mutation rows above are requirements for
  implementation, not evidence of completed testing.
- File changed: `docs/wip/2026-11-04-streams-stage-6-plan.md` only. No git state
  operations, network calls, commits or pushes were performed. The separate
  `.superpowers/codex/6-plan-report.md` was not written because the
  task-specific one-file limit takes precedence; this section supplies its
  content (Q8).
- Original planning-run coverage (superseded by the 2026-11-04 duplex
  amendment): R-23 decoding/UTF-8 is Task 1; generated replies and tables Task
  2; writable parameter streams and plain replies Task 3; generated-code
  Rust/Deno interop and 64 elements Task 4; required records/archive Task 5.
  Duplex, multiple uploads and the exact old faults/server fixture are explicit
  driver questions, not silently claimed acceptance.
- Dependency check: runtime exports precede generated imports, reply support
  precedes uploads, fixture cases are introduced with their implementation, and
  records follow executable evidence. Task 1's cross-generator table change is
  atomic to preserve the existing parity gate (C5). All proposed commit scopes
  occur in `.git-std.toml:12` and its scope list.
- Validation of this file: targeted `prim lint` and `prim fmt --check` passed
  after formatting this file only. A read-only script checked 54 explicit source
  citations: all file paths and line bounds resolved, with five tasks, seven
  rulings and the driver questions last. Full implementation gates, socket e2e,
  and browser e2e: **not run in sandbox** for this planning task. No pass is
  inferred from reading recipes.
- Proposed planning commit (driver only):
  `docs(ridl-backend-ts): plan generated TypeScript stream functions`.

## Questions for the driver

1. **Include #1240 here or defer duplex?** Answer recorded 2026-11-04: the owner
   includes duplex in stage 6. Task 0 supplies queryDuplex; Task 3b generates
   the typed functions; Task 4 exercises a duplex docstore member. Two or more
   parameter streams (#1224) remain excluded.

   **Superseded 2026-11-04 (original recommendation):**

   1. **Include #1240 here or defer duplex?** Recommendation: defer it, retain a
      specific generated omission comment, and link #1240 in the maintained
      records. This keeps five reviewable tasks and does not invent cancellation
      semantics for a third transport return shape. If duplex is required,
      complete and review #1240 first, then revise this plan with its exact API
      and tests; do not combine that transport feature into one of the codegen
      commits.
2. **Approve S6-1's sorted utf8 insertion?** The owner already decided that the
   union change is breaking and ships in the next breaking release; that part is
   settled. Recommendation: preserve sorted tables, give utf8 payload code 7 and
   move variant to 8 atomically in Task 1 with the `!` commit. Require paired
   regeneration, and do not confuse these codes with frame detail codes. If
   preserving variant's numeric payload code is required, approve append-only
   utf8 instead and amend the sorted-table records before Task 1.
3. **Approve session injection, names and options (S6-3/S6-4)?** Recommendation:
   `StreamSession = { transport, codec }` in the runtime, exported
   `DocStore_fetch`-style names and required `{ bound }`. This satisfies the
   transport dependency rule and makes verifier setup explicit. The design's
   direct Session / unqualified function / optional opts spellings otherwise
   need a separate binding/default policy; implementation must wait for this
   answer rather than choose one.
4. **Approve the parameter-stream surface and supported subset?**
   Recommendation: return `{ upload, accepted }` or `{ upload, reply }`, as
   section 8 demonstrates; do not accept an externally supplied writable.
   Support one parameter stream including duplex (Q1 answered), explicit
   omissions for multiple streams and inline fallible replies without a codec,
   and no change to typl stream refusal. This records the limit behind the
   issue's “one function per streamed interaction” wording.
5. **Approve raw/text sender chunking and zero-limit behavior (S6-5)?**
   Recommendation: split raw bytes, split text only at code-point boundaries,
   reject lone surrogates, never split named encodings, and fail a zero-limit
   call locally. These are choices the existing transport deliberately does not
   make; accepting them prevents silent text replacement and oversized writes.
6. **Approve the local error contract (S6-6)?** Recommendation: runtime
   StreamError with structural contract/corrupt failure data, unchanged remote
   SessionError objects, setup CodecError/CatalogMismatch before transmission,
   and normal upstream cancellation on a decode error. Sending a specific local
   contract outcome on the wire would require an additional transport API and
   plan change.
7. **Approve the typed e2e substitution (S6-7)?** Recommendation: exercise
   exactly 64 generated DocChunk values on the generated docstore Rust provider
   in `ts-e2e`, alongside its byte upload/read-back. Keep the existing raw
   loopback_server regressions. This tests actual FlatBuffers/wasm
   interoperability instead of treating raw fixture bytes as named payloads,
   while avoiding a second generated catalog inside the shared transport
   fixture. Run `ts-browser-e2e` as the existing browser regression; a docstore
   browser page can remain separate follow-up work because Deno e2e satisfies
   the issue's chosen recipe alternative.
8. **Planning report path versus one-file limit?** Recommendation: accept the
   planning-run report embedded above, preserving the explicit instruction that
   only this plan file changes. If a separate ignored report is required,
   authorize that second file; no implementation or git-state action depends on
   it.
