# The TypeScript WebSocket transport package — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** a Deno and browser package, `@driftsys/ridl-transport-ws`, that is the
consuming side of the frame's WebSocket binding (frame specification §11.1) and
carries payload bytes unread, proven by a Deno program and a headless browser
page that each receive a live signal from the Rust server.

**Architecture:** a sans-IO codec (`src/frame.ts`) and a session over the
platform `WebSocket` (`src/session.ts`), with no dependency. The proof runs a
cargo example of `ridl-transport-ws` serving a `ridl-loopback` provider, and
drives Deno and Chrome against it. Two new gate recipes, `ts-check` and
`ts-e2e`, join `just build` and CI.

**Tech Stack:** Deno 2.9 (`deno fmt`, `lint`, `check`, `test`, `bundle`,
`publish --dry-run`), TypeScript, `node:assert/strict`, the Chrome DevTools
Protocol over Deno's `WebSocket`, Rust (one cargo example), just, GitHub
Actions.

**Spec:** `docs/wip/2026-10-20-ts-transport-ws-design.md` (decisions D-1 to
D-18). The wire authority is `docs/specification/frame-specification.md` §4 to
§9 and §11.1. The reference implementation of the same codec is
`crates/ridl-transport-ws/src/frame.rs`: where this plan says "as the crate
does", read that file.

## Global Constraints

- Plain, literal English in every comment, doc and commit message; no story
  identifier (`E12.11`) and no plan name in any file under `packages/`,
  `crates/`, `docs/design/` or `docs/technotes/`.
- No import outside the package in `packages/ridl-transport-ws/src/` and
  `mod.ts`. Tests import only the package, `node:assert/strict`, and Deno's
  built-in APIs. No `jsr:`, `npm:` or `https:` specifier anywhere.
- The package's own code uses only `WebSocket`, `Uint8Array`, `DataView`,
  `ArrayBuffer`, `TextEncoder`, `TextDecoder` (with `fatal: true`),
  `setTimeout`, `clearTimeout`, `Promise`, `BigInt`. No `Deno.*` in `src/`.
- Integers on the wire are little-endian; `stamp` is `i64`, `seq` and
  `correlation` `u64`, both `bigint` in TypeScript; `interface`, `ordinal`,
  codes `number`.
- Subprotocol `ridl-ws-v1`; frame version 1; default encoding `proto3` (tag 2).
- `deno fmt` and `deno lint` cover `.ts` files only; prim formats the Markdown
  and the JSON.
- Commit scope for the package: `ts-transport-ws` (added in Task 1). The example
  in the crate takes `ridl-transport-ws`. Every commit message ends with:

  ```text
  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_015aJSagNDGRrDEUqr3mEqpA
  ```

- Implementers run the focused commands each task names; the full `just build`
  is CI's.

## Review Focus

1. A `publish` of a signal that is subscribed _and_ has a `read` waiting, and a
   `read` of a signal that is not subscribed: the `read` settles with the
   `publish`, and the publish updates the held sample only when the signal is
   subscribed. Pinned in Task 3.
2. A `response` that arrives after its call's bound has passed: discarded and
   recorded as unknown correlation, never resolving a forgotten promise. Pinned
   in Task 4.
3. The session ending while a `subscribe`, a `read` and a call are all waiting:
   each rejects with `down`, and `closed` resolves once. Pinned in Task 4.
4. A unit name of exactly 255 UTF-8 bytes made of multi-byte characters:
   accepted; 256 bytes: refused before the socket is opened. Pinned in Tasks 2
   and 3.
5. A zero-length payload on a `request`, a `Live` `publish`, an `occurrence` and
   a `reply`: carried as an empty `Uint8Array`, not as `null`. Pinned in Task 2.

---

### Task 1: The package skeleton, the scope and the version test

**Files:**

- Create: `packages/ridl-transport-ws/deno.json`
- Create: `packages/ridl-transport-ws/mod.ts`
- Create: `packages/ridl-transport-ws/README.md`
- Create: `packages/ridl-transport-ws/LICENSE` (copy of the root `LICENSE`)
- Create: `packages/ridl-transport-ws/tests/version_test.ts`
- Modify: `.git-std.toml` (add the scope `ts-transport-ws` after
  `ridl-transport-ws`, with a one-line comment naming
  `packages/ridl-transport-ws/`)

**Interfaces:**

- Produces: `deno.json` with `"name": "@driftsys/ridl-transport-ws"`,
  `"version"` equal to `[workspace.package] version` of the root `Cargo.toml`,
  `"license": "MIT"`, `"exports": "./mod.ts"`,
  `"fmt": { "include": ["**/*.ts"] }`, `"lint": { "include": ["**/*.ts"] }`,
  `"publish": { "include": ["mod.ts",
  "src/", "README.md", "LICENSE"] }`.
  `mod.ts` exports nothing yet beyond a comment header; later tasks add
  `export *` lines.

- [ ] **Step 1: Write `tests/version_test.ts`**: test
      `"the package version is the workspace version"` reads `../../Cargo.toml`
      (relative to the test file, through `new URL(..., import.meta.url)`),
      extracts the `version = "x.y.z"` line under `[workspace.package]`, reads
      `../deno.json`, and asserts equality with `assert.equal`.
- [ ] **Step 2: Run it before `deno.json` exists**:
      `cd packages/ridl-transport-ws && deno test --allow-read tests/version_test.ts`
      — expected: FAIL (file not found).
- [ ] **Step 3: Create `deno.json`, `mod.ts`, `README.md` (the README also
      states three limits: on a protocol violation the client closes with no
      code, because the platform cannot send 1002, 1003 or 1009; there is no
      client liveness timer, because a page cannot see the server's pings; the
      default request stamp is UTC from `Date.now()`, not TAI, and a TAI clock
      is passed with the `clock` option, in microseconds as a `bigint`. Then a
      short description: what the package is, that it carries payload bytes
      unread, the Deno and browser targets, that it is not yet published, and a
      link to `docs/specification/frame-specification.md` §11.1), `LICENSE`.**
- [ ] **Step 4: Run** the test again — expected: PASS. Run
      `deno fmt --check && deno lint` in the package — expected: clean.
- [ ] **Step 5: Commit** `feat(ts-transport-ws): add the package skeleton` (the
      `.git-std.toml` change in the same commit, so the scope exists when the
      hook lints it).

### Task 2: The codec

**Files:**

- Create: `packages/ridl-transport-ws/src/frame.ts`
- Modify: `packages/ridl-transport-ws/mod.ts` (add
  `export * from "./src/frame.ts";`)
- Test: `packages/ridl-transport-ws/tests/vectors_test.ts`,
  `tests/codes_test.ts`, `tests/decode_errors_test.ts`

**Interfaces:**

- Produces (exact names; later tasks use them):

  ```ts
  export const FRAME_VERSION = 1;
  export const SUBPROTOCOL = "ridl-ws-v1";
  export const MIN_MESSAGE_LIMIT = 294;
  export const ENVELOPE_BYTES = 16;
  export const Kind = { Signal: 1, Event: 2, Command: 3, Query: 4, Fixed: 5 } as const;
  export type Kind = (typeof Kind)[keyof typeof Kind];
  export type EncodingName = "flatbuffers" | "proto3" | "repr-c";
  export function encodingTag(name: EncodingName): 1 | 2 | 3;
  export interface Target { kind: Kind; iface: number; ord: number }
  export interface Envelope { stamp: bigint; seq: bigint }
  export type Rule = "range" | "step" | "length" | "pattern" | "variant" | "unique";
  export type Contract =
    | { category: "invalid-value"; rule: Rule }
    | { category: "precondition-failed" }
    | { category: "contract-broken" }
    | { category: "unknown-interaction" };
  export type Reason = "unknown-catalog" | "catalog-mismatch" | "encoding-unsupported"
    | "frame-version-unsupported" | "range";
  export type PublishState =
    | { provenance: "init" } | { provenance: "live"; payload: Uint8Array } | { provenance: "invalid" };
  export type ResponseOutcome =
    | { outcome: "accepted" } | { outcome: "reply"; payload: Uint8Array }
    | { outcome: "contract"; contract: Contract } | { outcome: "corrupt" } | { outcome: "busy" };
  export type Frame =
    | { form: "attach"; frameVersion: number; encoding: number; hash: Uint8Array; name: string }
    | { form: "attached"; outcome: { outcome: "accepted" } | { outcome: "refused"; reason: Reason } }
    | { form: "subscribe" | "unsubscribe" | "read"; target: Target }
    | { form: "answer"; target: Target; outcome: { outcome: "accepted" } | { outcome: "contract"; contract: Contract } }
    | { form: "publish"; target: Target; envelope: Envelope; state: PublishState }
    | { form: "occurrence" | "request"; target: Target; envelope: Envelope; payload: Uint8Array }
    | { form: "response"; target: Target; envelope: Envelope; correlation: bigint; outcome: ResponseOutcome };
  export type Violation = "too-short" | "control-too-long" | "unknown-code" | "outcome-not-allowed"
    | "nonzero-detail" | "zero-number" | "bad-name-length" | "bad-name";
  export type Decoded =
    | { ok: true; frame: Frame }
    | { ok: false; class: "violation"; violation: Violation }
    | { ok: false; class: "discard"; discard: "unexpected-payload" };
  export function decode(message: Uint8Array): Decoded;
  export function encode(frame: Frame): Uint8Array;
  export class AttachError extends Error { readonly reason: "empty" | "too-long" | "unknown-encoding" }
  export function attach(frameVersion: number, encoding: number, hash: Uint8Array, name: string): Frame; // throws AttachError
  ```

  The violation names are the crate's `ProtocolViolation` variants in kebab
  case; the classification of each case is the crate's `decode` (read
  `crates/ridl-transport-ws/src/frame.rs`). `decode` returns payloads as
  `subarray` views of `message` (D-7). `encode` writes `detail` as §11.1 item 2
  states. `attach` refuses an empty name, a name over 255 UTF-8 bytes, an
  encoding outside 1 to 3, and a hash not 32 bytes long (`too-long` is for the
  name only; a wrong hash length throws a `RangeError`).

- [ ] **Step 1: Write `tests/vectors_test.ts`.** It reads
      `docs/specification/frame-specification.md` (four directories up from the
      test, through `import.meta.url`), takes the rows of the table whose header
      is `| Id | Vector | Hex |` under `### 11.1`, and for each row decodes the
      `Hex` cell (backticks and spaces dropped) and asserts with
      `assert.deepEqual` that it equals the frame the test expects for that
      `Id`, then that `encode` of the expected frame equals the bytes. The
      expected frames are the nine of
      `crates/ridl-transport-ws/tests/vectors.rs` `expected()`, written in this
      package's types (`stamp` `0x0123456789ABn`, `0x0123456789ACn` on
      responses; hash bytes 0 to 31; name `veh.cabin`). An `Id` the test does
      not know fails the test; fewer than nine rows fails the test.
- [ ] **Step 2: Write `tests/codes_test.ts`.** One test per code table, each
      writing the message bytes by hand and asserting both `decode` and
      `encode`: every `form` 1 to 10; every `kind` 1 to 5; `provenance` 1 to 3;
      `response` outcomes 1, 2, 3 (with each `Rule` 1 to 6 as `detail`), 4 to 8;
      `answer` outcomes 1 and 3 to 6; `attached` 1 and 9 with each reason 1 to
      5; encodings 1 to 3 on `attach`. A zero-length payload on `request`,
      `occurrence`, `publish` `live` and `response` `reply` decodes to
      `new Uint8Array(0)`, not `null` (Review Focus 5).
- [ ] **Step 3: Write `tests/decode_errors_test.ts`**, one test per case of the
      crate's `tests/decode_errors.rs`, with the same names in sentence form and
      the same expected classification, plus: `attach("x".repeat(256))` throws
      `AttachError` `too-long`; a name of 85 three-byte characters (255 bytes)
      is accepted and one of 86 is `too-long` (Review Focus 4).
- [ ] **Step 4: Run** `deno test --allow-read tests/` — expected: FAIL (module
      not found).
- [ ] **Step 5: Implement `src/frame.ts`** with the interfaces above, a
      `DataView` over each message, `TextDecoder("utf-8", { fatal: true })` for
      the name.
- [ ] **Step 6: Run** `deno test --allow-read tests/` and
      `deno fmt --check && deno lint && deno check mod.ts` — expected: PASS,
      clean.
- [ ] **Step 7: Mutate to prove the oracle** (do not commit): change the `busy`
      outcome code to 9, run the tests, see `codes_test.ts` fail; change the
      byte order of `seq` on encode, see `vectors_test.ts` fail; restore both.
- [ ] **Step 8: Commit** `feat(ts-transport-ws): add the frame codec`.

### Task 3: The session — connect, attach, signals, events, notes

**Files:**

- Create: `packages/ridl-transport-ws/src/session.ts`
- Create: `packages/ridl-transport-ws/src/errors.ts`
- Modify: `packages/ridl-transport-ws/mod.ts` (export both)
- Test: `packages/ridl-transport-ws/tests/session_test.ts`,
  `packages/ridl-transport-ws/tests/fake_socket.ts` (test helper)

**Interfaces:**

- Consumes: Task 2's `encode`, `decode`, `attach`, `Frame`, `Target`,
  `Envelope`, `Contract`, `Reason`, `SUBPROTOCOL`, `FRAME_VERSION`,
  `encodingTag`.
- Produces:

  ```ts
  // errors.ts
  export class AttachRefused extends Error { readonly reason: Reason }
  export class ProtocolError extends Error { readonly cause: string } // no subprotocol, violation, text message
  export type CallFailure =
    | { category: "contract"; contract: Contract }
    | { category: "corrupt" } | { category: "busy" }
    | { category: "undelivered" } | { category: "timeout" } | { category: "down" };
  export class SessionError extends Error { readonly failure: CallFailure }

  // session.ts
  export interface Sample { provenance: "init" | "live" | "invalid"; envelope: Envelope; payload: Uint8Array | null }
  export interface Occurrence { iface: number; ord: number; envelope: Envelope; payload: Uint8Array }
  export type Note =
    | { note: "violation"; violation: Violation | "text-message" | "no-subprotocol" | "unexpected-form" }
    | { note: "discarded"; reason: "unexpected-payload" | "stale" | "not-subscribed" | "unknown-correlation" | "unexpected-answer"; frame?: Frame }
    | { note: "loss"; iface: number; ord: number; expected: bigint; received: bigint };
  export interface ConnectOptions {
    catalog: { name: string; hash: Uint8Array };
    encoding?: EncodingName;                     // default "proto3"
    onPublish?: (target: Target, sample: Sample) => void;
    onOccurrence?: (occurrence: Occurrence) => void;
    onNote?: (note: Note) => void;
    clock?: () => bigint;                        // microseconds; default BigInt(Date.now()) * 1000n
    webSocket?: (url: string, protocols: string[]) => WebSocket; // test seam; default new WebSocket
  }
  export interface Ended { code: number; reason: string; refused?: Reason; violation?: string }
  export function connect(url: string, options: ConnectOptions): Promise<Session>;
  export class Session {
    readonly closed: Promise<Ended>;
    subscribe(target: Target): Promise<void>;   // kind Signal or Event
    unsubscribe(target: Target): void;
    read(target: Target): Promise<Sample>;      // kind Signal or Fixed
    sample(iface: number, ord: number): Sample | undefined;
    command(iface: number, ord: number, args: Uint8Array, opts: { bound: number }): Promise<void>; // Task 4
    query(iface: number, ord: number, args: Uint8Array, opts: { bound: number }): Promise<Uint8Array>; // Task 4
    close(): void;
  }
  ```

  The fake socket (`tests/fake_socket.ts`) implements the parts of `WebSocket`
  the session uses — `binaryType`, `protocol`, `readyState`, `send`, `close`,
  `onopen`, `onmessage`, `onclose`, `onerror` — records what was sent, and gives
  the test `open(protocol)`, `deliver(frame | bytes |
  string)` and
  `serverClose(code, reason)`.

Behaviour to implement (design D-8 to D-11):

- `connect` validates the name and hash with `attach` before creating the socket
  and rejects with the `AttachError` (Review Focus 4). On `open`, a `protocol`
  other than `ridl-ws-v1` is a `ProtocolError("no-subprotocol")`, the socket is
  closed with `close()` and no code. Otherwise it sends the `attach` and waits;
  the first message must be `attached`, else a violation. `refused` rejects with
  `AttachRefused` and `closed` resolves with `refused` set once the socket
  closes.
- Every violation (a `decode` violation, a text message, a provider message
  before or other than `attached` first, a second `attached`, a
  consumer-to-provider form received) records a `violation` note, calls
  `close()` with no code, and ends the session (D-10). A discard records a
  `discarded` note and the session continues.
- `subscribe`, `read`: per identity `(kind, iface, ord)`, a first-in first-out
  queue of waiting operations. `answer` accepted settles the head `subscribe`;
  `answer` contract rejects the head operation (a `subscribe` or a `read`) with
  `SessionError({ category: "contract", ... })`; an `answer` with no waiting
  operation is `discarded`/`unexpected-answer`. A `publish` settles every `read`
  at the head of its identity's queue with the sample. A `subscribe` of a kind
  other than Signal or Event, and a `read` of a kind other than Signal or Fixed,
  are sent as asked: the server answers them (D-12 of the crate).
- Signals: a held sample per subscribed signal (keyed `iface:ord`). On a
  `publish` of a subscribed signal: discard as `stale` when a sample is held and
  the frame's `seq` is not greater; otherwise replace the sample and call
  `onPublish`. A `publish` of a signal neither subscribed nor read is
  `not-subscribed`. A `publish` that settles a `read` of an unsubscribed signal
  does not create a held sample and does not call `onPublish`. `unsubscribe`
  sends the frame and drops the held sample. `sample` returns the held sample. A
  `publish` of a `fixed` settles the `read` only.
- Events: subscribed events are a set; an `occurrence` of an event not in it is
  `not-subscribed`. Per event channel, the last accepted `seq`; an occurrence
  whose `seq` is greater than last + 1 records `loss` and is delivered; one not
  greater than last is `stale`. `unsubscribe` of an event drops its last `seq`.
- `close()` sends a close with code 1000. On the socket's `close` or `error`
  events, everything waiting rejects with `SessionError({ category: "down" })`,
  every later call rejects the same way, and `closed` resolves once.

- [ ] **Step 1: Write `tests/session_test.ts`** with these tests, each over the
      fake socket and asserting the bytes sent with `decode` and the results
      with `assert`:
      `connect sends attach with the catalog, frame version 1 and proto3`;
      `connect rejects with no-subprotocol and closes without a code when the server selected none`;
      `connect rejects with AttachRefused catalog-mismatch and closed reports the refusal`;
      `connect rejects a 256-byte name and opens no socket`;
      `a first message other than attached is a violation that closes without a code`;
      `a text message is a violation`;
      `subscribe of a signal resolves on answer accepted, and the first publish is held`;
      `subscribe rejects with contract unknown-interaction on a refusing answer`;
      `a publish whose seq is not greater than the held one is discarded as stale`;
      `unsubscribe drops the held sample, and a later subscribe starts from the next publish`;
      `read of an unsubscribed signal settles with the publish and holds nothing`;
      `read of a subscribed signal settles with the publish and updates the held sample`
      (Review Focus 1);
      `read of a fixed settles with a live publish whose seq is 0`;
      `a refused read rejects with contract unknown-interaction`;
      `two reads of one signal settle with one publish`;
      `an occurrence of a subscribed event is delivered with its payload`;
      `a gap in an event's seq is recorded as a loss and the occurrence is delivered`;
      `an occurrence of an unsubscribed event is discarded`;
      `a publish with bytes after a header that carries no payload is discarded and the session continues`;
      `close sends 1000 and closed resolves`;
      `a server close rejects a waiting subscribe and read with down`.
- [ ] **Step 2: Run** `deno test --allow-read tests/session_test.ts` — expected:
      FAIL (module not found).
- [ ] **Step 3: Implement `src/errors.ts` and `src/session.ts`** (calls stub
      `command`/`query` throwing `Error("not implemented")` until Task 4).
- [ ] **Step 4: Run** the session tests and the whole suite, then
      `deno fmt --check && deno lint && deno check mod.ts` — expected: PASS,
      clean.
- [ ] **Step 5: Check the browser typing**: create
      `packages/ridl-transport-ws/browser.deno.json`
      (`{"compilerOptions":
      {"lib": ["dom", "esnext"]}}`) and run
      `deno check --config browser.deno.json mod.ts` — expected: clean (no
      `Deno` global used in `src/`).
- [ ] **Step 6: Commit** `feat(ts-transport-ws): add the consuming session`.

### Task 4: The session — calls and the end of a session

**Files:**

- Modify: `packages/ridl-transport-ws/src/session.ts`
- Test: `packages/ridl-transport-ws/tests/calls_test.ts`

**Interfaces:**

- Consumes: Task 3's `Session`, `SessionError`, `CallFailure`, the fake socket.
- Produces: `Session.command` and `Session.query` as typed in Task 3.

Behaviour (design D-9):

- One caller counter per session, starting at 1n; each `request` takes the next
  value as `seq`, the `clock()` value as `stamp`, and `kind` Command or Query.
  The payload is copied into the message.
- A `response` is matched on `correlation` to a call in flight. `accepted`
  resolves a command; `reply` resolves a query with the payload view;
  `contract`, `corrupt`, `busy` reject with the matching `SessionError`. The
  call is then forgotten. A `response` whose correlation names no call in flight
  is `discarded`/`unknown-correlation`.
- `bound` is milliseconds, required, a finite number greater than 0 (a
  `RangeError` otherwise, before sending). When it passes with no `response`, a
  command rejects `undelivered` and a query `timeout`, and the call is
  forgotten, so a later `response` is `unknown-correlation` (Review Focus 2).
- On the end of the session every call in flight rejects `down` and its timer is
  cleared; a call made after the end rejects `down` without sending.

- [ ] **Step 1: Write `tests/calls_test.ts`**:
      `a command sends a request with seq 1 and the caller stamp, and resolves on accepted`;
      `a second call takes seq 2`; `a query resolves with the reply payload`;
      `a zero-length reply resolves with an empty Uint8Array` (Review Focus 5);
      `contract invalid-value with rule range rejects with that contract`;
      `corrupt and busy reject with their categories`;
      `a command with no response within its bound rejects undelivered`;
      `a query with no response within its bound rejects timeout`;
      `a response after the bound is discarded as unknown-correlation`;
      `a response with an unknown correlation is discarded and the session continues`;
      `the session ending rejects a call in flight, a waiting subscribe and a waiting read with down, and closed resolves once`
      (Review Focus 3);
      `a call after the session ended rejects down and sends nothing`;
      `a bound of 0 or NaN throws RangeError and sends nothing`. Use bounds of
      20 ms or less and the clock option fixed at `0x0123456789ABn`.
- [ ] **Step 2: Run** `deno test --allow-read tests/calls_test.ts` — expected:
      FAIL.
- [ ] **Step 3: Implement** the calls in `src/session.ts`.
- [ ] **Step 4: Run** the whole suite and
      `deno fmt --check && deno lint &&
      deno check mod.ts && deno check --config browser.deno.json mod.ts`
      — expected: PASS, clean, and no timer left running (Deno's test sanitizer
      reports a leaked timer as a failure).
- [ ] **Step 5: Commit** `feat(ts-transport-ws): add commands and queries`.

### Task 5: The test server, as a cargo example

**Files:**

- Create: `crates/ridl-transport-ws/examples/loopback_server.rs`
- Modify: `crates/ridl-transport-ws/Cargo.toml`
  (`exclude = ["tests/",
  "examples/"]`, and the comment above it says why
  `examples/` is left out)
- Modify: `docs/design/ridl-transport-ws.md` (the "Tests" list: one item for the
  example, saying it serves the TypeScript package's end-to-end tests)

**Interfaces:**

- Produces: a binary `target/debug/examples/loopback_server` that prints exactly
  one line `port <n>` on standard output, flushed, after it binds `127.0.0.1:0`;
  serves every connection on its own thread with its own `Scanning(rt.attach())`
  backend and `Config::default()`; steps the provider with `provider_step` every
  5 ms; and exits 0 when its standard input reaches the end. The catalog is
  `tests/support/provider.rs`'s, included with
  `#[path = "../tests/support/provider.rs"] mod provider;`: `test.ws`, hash
  thirty-two `7`s, interface 1, signal 1, event 2, command 3, query 4, fixed 5
  with value `f0 0d`.

- [ ] **Step 1: Write the example.** The provider `Loopback` is shared behind an
      `Arc<Mutex<Loopback>>`: the accept loop locks it to call `attach()` for
      each connection, and the stepping thread locks it to call `provider_step`.
      (Check `ridl_loopback::Loopback` is `Send`; if it is not, report back
      before working around it.)
- [ ] **Step 2: Run it by hand**:
      `cargo run -p ridl-transport-ws --example loopback_server < /dev/null`
      exits at once with status 0; with a held standard input
      (`sleep 3 | cargo run ...`) it prints `port <n>` and exits after 3 s.
- [ ] **Step 3: Check the crate's gates are unaffected**:
      `cargo test -p ridl-transport-ws`,
      `cargo clippy -p ridl-transport-ws --all-targets -- -D warnings`, and
      `cargo package -p ridl-transport-ws --list --allow-dirty | grep -c examples`
      prints `0`. Run `just compat-check` — expected: PASS.
- [ ] **Step 4: Commit**
      `test(ridl-transport-ws): add a loopback server example for the TypeScript client`.

### Task 6: The end-to-end tests — Deno and a headless browser

**Files:**

- Create: `packages/ridl-transport-ws/e2e/server.ts` (starts the example binary
  named by `RIDL_TRANSPORT_WS_SERVER`, reads `port <n>`, returns
  `{ port, stop() }`; `stop` closes its standard input and awaits the exit)
- Create: `packages/ridl-transport-ws/e2e/catalog.ts` (the test catalog's
  constants, with a comment naming
  `crates/ridl-transport-ws/tests/support/provider.rs` as their definition)
- Create: `packages/ridl-transport-ws/e2e/deno_test.ts`
- Create: `packages/ridl-transport-ws/e2e/browser.ts` (find the browser, launch
  it, a minimal DevTools Protocol client: `launch(): Promise<Browser>`,
  `Browser.evaluate(url: string, expression: string): Promise<unknown>`,
  `Browser.close()`)
- Create: `packages/ridl-transport-ws/e2e/page.ts` (the page's script: imports
  `../mod.ts`, runs the signal sequence, and assigns `globalThis.result` a
  promise of `{ init: string, live: string }` with the provenances and payloads
  as hex)
- Create: `packages/ridl-transport-ws/e2e/browser_test.ts`

**Interfaces:**

- Consumes: Task 3 and 4's `connect`, `Session`; Task 5's binary and its
  `port <n>` line.
- Each test starts its own server process with `e2e/server.ts` and stops it in a
  `finally`, because a command changes the shared provider's signal for every
  later connection (design D-14).
- Browser lookup order (design D-14): `RIDL_BROWSER`; then `google-chrome`,
  `google-chrome-stable`, `chromium`, `chromium-browser` on `PATH`; then
  `/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`. None found:
  the test fails with a message naming `RIDL_BROWSER` and the install pages of
  Chrome and Chromium. Launch flags: `--headless=new`,
  `--remote-debugging-port=0`, `--user-data-dir=<temp dir>`, `--no-first-run`,
  `--no-default-browser-check`, `about:blank`; on Linux also `--no-sandbox` only
  when `RIDL_BROWSER_NO_SANDBOX=1` (CI sets it if the runner needs it). The
  DevTools port is read from `<user-data-dir>/DevToolsActivePort`.

- [ ] **Step 1: Write `e2e/deno_test.ts`**, test
      `"a Deno program receives a live signal from the Rust server"`: connect to
      `ws://127.0.0.1:<port>` with the test catalog; `read` the fixed → `live`,
      payload `f0 0d`, seq `0n`; `subscribe` the signal → held sample `init`,
      seq `0n`, payload `null`; `subscribe` the event;
      `command(1, 3, [0x2a, 0x07], { bound: 2000 })` resolves; wait (≤ 2 s) for
      `onPublish` with `live` and payload `2a 07`, seq ≥ `1n`; wait for
      `onOccurrence` with payload `2a 07`;
      `query(1, 4, [0x01, 0x80, 0xff],
      { bound: 2000 })` resolves to
      `02 81 00`; `close()`; `closed` resolves with code 1000. No note of class
      `violation` was recorded.
- [ ] **Step 2: Write `e2e/browser_test.ts`**, test
      `"a browser page receives a live signal from the Rust server"`: bundle
      `e2e/page.ts` with `deno bundle --platform=browser` into a temporary file
      (run through `Deno.Command(Deno.execPath(), ...)`); serve `/` (an HTML
      page loading `/page.js` as a module) and `/page.js` from `Deno.serve` on
      `127.0.0.1:0`, passing the server's port to the page in the query string;
      `launch()`, `evaluate(pageUrl, "globalThis.result")` with a 10 s limit;
      assert `{ init: "init:", live: "live:2a07" }` (format
      `<provenance>:<hex payload>`); close the browser, the HTTP server and the
      example.
- [ ] **Step 3: Build the example and run both**:
      `cargo build -p ridl-transport-ws --example loopback_server` then, in the
      package,
      `RIDL_TRANSPORT_WS_SERVER=<abs path to target/debug/examples/loopback_server> deno test --allow-all e2e/deno_test.ts`
      and the same with `e2e/browser_test.ts` — expected: 1 passed each. The two
      test files are run separately because the browser test is a CI-only gate
      (Task 7).
- [ ] **Step 4: Prove the tests can fail** (do not commit): make the example's
      `provider_step` command branch set the signal to the argument with its
      first byte changed — both tests fail on the payload; restore.
- [ ] **Step 5: Commit**
      `test(ts-transport-ws): receive a live signal in Deno and in a browser`.

### Task 7: The gates, CI and bootstrap

Ruling (main session, 2026-10-20): the browser proof runs in CI only — a
contributor's `just build` must not need a browser. See design D-16.

**Files:**

- Create: `.deno-version` at the root, one line `2.9.5`.
- Modify: `justfile` — add recipes `ts-check`, `ts-e2e` and `ts-browser-e2e`;
  add `ts-check` and `ts-e2e` (not `ts-browser-e2e`) to `build` and to
  `gate-parity`'s `expected` list; add `ts-check` to `pre-push`; add `packages`
  to `story-id-check`'s `scanned` array, to the literal it is compared against,
  and to the comment listing the scanned trees. The comment above
  `ts-browser-e2e` says it is outside `build` on purpose (a contributor's build
  needs no browser), that CI's `typescript` job runs it, and that `gate-parity`
  does not cover it, so its CI step is checked by reading.
- Modify: `.github/workflows/ci.yml` — a new job `typescript` (`needs: changes`,
  `if: needs.changes.outputs.rust == 'true'`, ubuntu-latest): checkout,
  `rustup show`, `Swatinem/rust-cache@v2`, install `just` as the other jobs do,
  install Deno by reading `.deno-version` and downloading
  `https://github.com/denoland/deno/releases/download/v<version>/deno-x86_64-unknown-linux-gnu.zip`
  into `$HOME/.local/bin`, then `run: just ts-check`, `run: just ts-e2e` and
  `run: just ts-browser-e2e`; add `typescript` to the `ci` aggregate's `needs`;
  extend the comment on the `changes` job to say `typescript` uses the `rust`
  flag; extend the comment on the `env:` pins to say the Deno pin lives in
  `.deno-version`, because `./bootstrap` reads it too.
- Modify: `bootstrap` — when `deno` is not on `PATH`, install the release named
  by `.deno-version` with Deno's official installer
  (`curl -fsSL https://deno.land/install.sh | sh -s -- -y v<version>`, which
  installs into `~/.deno/bin`) and tell the contributor to add `~/.deno/bin` to
  `PATH`; when `deno` is present at another version, print a note naming both
  versions and do not replace it. No browser check.
- Modify: `AGENTS.md` — the Commands block gains `just ts-check`, `just ts-e2e`
  and `just ts-browser-e2e` lines (the last saying it runs in CI and needs a
  Chromium-family browser, and is not a member of `build`); the `build` line
  names the two members; the `pre-push` line names `ts-check`; the opening
  paragraph that lists the workspace's directories names `packages/` and the
  package.
- Modify: `CONTRIBUTING.md` — where the gates are described, say that
  `just build` needs Deno (installed by `./bootstrap`) and that the browser
  test, `just ts-browser-e2e`, runs in CI and can be run locally with a
  Chromium-family browser (`RIDL_BROWSER` names one).

Recipes (each guards its tools as ADR-0009 decision 10 requires — a missing tool
exits 1 with the install command, never skips):

```text
ts-check:        in packages/ridl-transport-ws:
                 deno fmt --check; deno lint; deno check mod.ts;
                 deno check --config browser.deno.json mod.ts;
                 deno test --allow-read tests/;
                 deno publish --dry-run --allow-dirty
ts-e2e:          cargo build --locked -p ridl-transport-ws --example loopback_server;
                 RIDL_TRANSPORT_WS_SERVER=$PWD/target/debug/examples/loopback_server
                 deno test --allow-all e2e/deno_test.ts   (in the package)
ts-browser-e2e:  the same build; then deno test --allow-all e2e/browser_test.ts
```

`ts-browser-e2e` checks for a browser before it builds, with the Task 6 lookup,
and fails naming `RIDL_BROWSER` when none is found.

- [ ] **Step 1: Add the recipes and run them**: `just ts-check`, `just ts-e2e`
      and `just ts-browser-e2e` — expected: all pass.
- [ ] **Step 2: Run** `just gate-parity` before the CI change — expected: FAIL
      naming `ts-check ts-e2e`; make the CI change; run again — expected: PASS.
- [ ] **Step 3: Prove the guards**: `PATH=/usr/bin:/bin just ts-check` exits 1
      naming Deno; `RIDL_BROWSER=/nonexistent just ts-browser-e2e` exits 1
      naming the browser.
- [ ] **Step 4: Run** `just story-id-check`, `just check`, `just link-check`,
      `just doc-path-check`, `./bootstrap` — expected: all pass.
- [ ] **Step 5: Commit** `ci(repo): run the TypeScript package's gates` (the
      justfile, workflow, `.deno-version`, bootstrap, AGENTS.md and
      CONTRIBUTING.md together).

### Task 8: The records

**Files:**

- Modify: `docs/specification/frame-specification.md` §11.1 opening paragraph
  and §11 opening: name the package `@driftsys/ridl-transport-ws` at
  `packages/ridl-transport-ws/` as built (the TypeScript consuming side). Do not
  touch the item list, the layout table or the vectors.
- Modify:
  `docs/decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md`
  — a dated
  `**Amended 2026-10-20 — decision 6, the TypeScript WebSocket
  package depends on nothing until the runtime package exists.**`
  paragraph in `## Status` (design D-4), the decision 6 table row's `Depends on`
  cell updated to say so, and a note on open item 2 that the location is
  `packages/` (design D-1), the runtime package's name still open.
- Modify: `docs/design/ridl-transport-ws.md` — "Scope" names the package as
  built (replacing "the TypeScript WebSocket package, driftsys/ridl#982").

- [ ] **Step 1: Edit** the three files.
- [ ] **Step 2: Run** `just check`, `just link-check`, `just doc-path-check`,
      `just story-id-check`, `just freeze-check` — expected: all pass (the
      `wire` dump unchanged).
- [ ] **Step 3: Commit** `docs(adr): record the TypeScript WebSocket package`.

The design record `docs/design/ridl-transport-ws-ts.md`, the roadmap's landed
note, `docs/BACKLOG.md` and `docs/archive/roadmap-landed-record.md` are written
at gardening, after the tasks, from this plan and the design.
