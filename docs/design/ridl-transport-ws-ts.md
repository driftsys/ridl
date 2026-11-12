# `@driftsys/ridl-transport-ws` — the WebSocket binding of the frame in TypeScript

`@driftsys/ridl-transport-ws`, at `packages/ridl-transport-ws/`, is the
TypeScript side of the WebSocket binding that
[the frame specification](../specification/frame-specification.md) §11.1
defines. It is the **consuming side** of a frame session, for Deno and for the
browser: it opens one WebSocket offering the subprotocols `ridl-ws-v3`,
`ridl-ws-v2` and `ridl-ws-v1`, sends `attach`, `subscribe`, `unsubscribe`,
`read`, `request` and the stream forms of a reply or an upload, and applies the
consumer's rules that need no payload decoding. It is also the **providing
side** of the reversed role, `provide`, in which the browser or Deno program
stands in for a component (D-21). It carries payload bytes without reading them:
a payload is a `Uint8Array` that the package hands to its caller as it arrived,
and sends as the caller gave it. It checks each payload's length against the
member's limit from the generated interface tables (D-19), and runs a caller's
registered check on live signal payloads (D-24).

The Rust crate `ridl-transport-ws` is the providing side of the same binding,
and the consuming side of the reversed role; its as-built record is
[`ridl-transport-ws.md`](ridl-transport-ws.md), which this record follows in
structure. Verifying a payload is the wasm codec's, of
[ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
decision 7, loaded by `@driftsys/ridl-runtime`, and decoding and encoding one is
the generated codec module's, `<base>.codec.ts`
([`ts-codec-table.md`](ts-codec-table.md)). The generated stream methods of a
client class use this package's `Session` (see
[What is not built](#what-is-not-built)). Generated plain-call and provider
faces remain unbuilt (driftsys/ridl#287). ADR-0020 decision 6 places the
package; its dependency row was amended on 2026-10-20 (D-4). The wire rules are
the frame specification §3 to §9 and §11.1 (binding versions 1, 2 and 3).

This is the architecture as built. The code and its tests are the source of
truth; where this record and the code differ, this record is corrected. The
decisions are numbered D-1 to D-24. D-1 to D-18 retain their numbers from the
original design, archived as
[`2026-10-20-ts-transport-ws-design.md`](../archive/2026-10-20-ts-transport-ws-design.md),
with its plan beside it.

## Scope

The first consumer is the debug and test portal that the crate's record names:
its client runs in a browser or in Deno and talks to a daemon that embeds the
crate's `serve`. The observability server (driftsys/ridl#288) and the
test-harness drivers (driftsys/ridl#291) are planned on top of this package.

**What the package does.**

- Encodes and decodes every one of the fourteen forms of frame specification §4
  exactly as §11.1 item 2, "Binding version 2" and "Binding version 3" lay them
  out, and reproduces every test vector of §11.1 byte for byte, read from the
  same table the crate's tests read.
- Opens a consuming session over one `WebSocket`: `attach`, `subscribe`,
  `unsubscribe`, `read` (signal and `fixed`), `command`, `query`, `queryStream`
  (a query whose reply is a stream), `commandStream`, `queryWithUpload` and
  `queryDuplex` (calls with parameter stream 1), and the end of the session.
- Opens a providing session over one `WebSocket` with `provide`: it offers only
  `ridl-ws-v2-provider`, answers the server's `attach`, publishes signals,
  raises events and answers requests (D-21).
- Runs unchanged in Deno and in a browser: the package's own code uses only web
  platform APIs.

**What it does not do.** It decodes, verifies and encodes no payload itself. It
runs the signal check the caller registers (D-24). It reads no catalog
descriptor: no `kind` check against a catalog, no response bound read from one,
no compatible-hash list; the one fact of the catalog it uses, each member's
payload limit, comes from the generated interface tables the caller passes
(D-19). It has no TLS or authentication of its own (`wss://` works because the
platform's `WebSocket` supports it). It is not yet published to JSR (D-3).

**What it must never do.** Differ from §11.1 on the wire, so that a client
written from the specification alone and this one behave differently against the
same server; test its codec only against itself, so that a wrong constant
cancels out in a round trip; reach for a Deno-only or Node-only API in its own
code; pull a runtime dependency into a consumer's build; or hide a failure: a
refused `attach`, a protocol violation, a lost occurrence or a call with no
outcome is reported, never dropped silently.

## The package and its dependencies

**D-1. The location and the name.** The package lives at
`packages/ridl-transport-ws/` and is named `@driftsys/ridl-transport-ws`. The
top-level `packages/` holds the TypeScript packages, as `crates/` holds the Rust
crates, and the directory name equals the package's bare name. The package has
the crate's name because it is the same binding on the other side of the
connection. This settles the location half of ADR-0020 open item 2; the runtime
package's name stays open.

**D-17. The commit scope is `ts-transport-ws`.** The crate already holds the
scope `ridl-transport-ws`, and `.git-std.toml` lists one scope per crate or
package.

**D-2. Deno first, and web-standard code only.** The package is TypeScript
source with explicit `.ts` import specifiers and a `deno.json`; the source needs
no build step, and a dnt build step produces the npm package (D-3). Its own code
(`mod.ts` and `src/`) uses only `WebSocket`, `Uint8Array`, `DataView`,
`ArrayBuffer`, `TextEncoder`, `TextDecoder`, `setTimeout`, `clearTimeout`,
`BigInt`, `ReadableStream`, `WritableStream`, `CountQueuingStrategy` and
`globalThis.reportError`, which Deno and browsers both provide; no `Deno.*`. The
source toolchain is Deno: `deno fmt`, `deno lint`, `deno check`, `deno test`,
`deno bundle` (in the browser test) and `deno publish --dry-run`. The source
tree has no `package.json`; the build generates an `npm/` directory, which git
ignores and which holds the `package.json` of the npm package. Node 22 or later
is a tested target: Node 22 has a global `WebSocket`, and `ts-npm-check`
installs the built package in a temporary directory and imports it by name from
Node. The editor extension keeps its own npm toolchain. `deno fmt` and
`deno lint` cover `.ts` files only (`deno.json` `include`), so prim stays the
formatter of the Markdown and the JSON.

**D-3. Publication is to npm; JSR is later.** `deno.json` carries `name`,
`version`, `license`, `exports` and the `publish` file list, so
`deno publish --dry-run` checks the package as JSR would; `just ts-check` runs
the dry run. Nothing is uploaded to JSR. The workflow
`.github/workflows/npm-release.yml` publishes `@driftsys/ridl-transport-ws` to
npm on a `v*` tag, with provenance and npm trusted publishing (no stored token).
It checks that the tag version equals the `deno.json` version and that the
tagged commit is on `main`, skips a version already on npm, and uses
`--tag next` for a prerelease; ADR-0020's 2026-10-21 amendment records npm. The
version is the workspace version (`Cargo.toml` `[workspace.package]`), and
`tests/version_test.ts` fails when the two differ. The JSR publication is
driftsys/ridl#992.

**D-4. One dependency: `@driftsys/ridl-runtime`.** The package imports the
runtime for the vocabulary types it shares (`Envelope`, `Contract`,
`CallFailure`, `WaitOptions`, `Detection`, `SignalCheck`, `SignalCheckEntry`,
and `RawSample` and `RawOccurrence`, which it exports as `Sample` and
`Occurrence`) and for `CallError`, the superclass of `SessionError`. It imports
nothing else. The tests use `node:assert/strict`, which Deno provides, and
Deno's own APIs, so nothing else is fetched from a registry and there is no
lockfile. The browser test drives the browser through the Chrome DevTools
Protocol over Deno's own `WebSocket` client (`e2e/browser.ts`), not through
Puppeteer, Playwright or Astral. In the source tree `deno.json` maps the runtime
to the sibling directory; the npm build maps it to the npm package, as the
dependency `^<runtime version>`, and skips `npm install`, so the build does not
need that version on the registry.

## The codec (`src/frame.ts`)

**D-5. Two layers, as in the crate: a sans-IO codec and a session.**

| Module              | Holds                                                                                                                               |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| `src/frame.ts`      | the fourteen forms as the discriminated union `Frame`, `encode`, `decode`, `attach`, `AttachError`, `encodingTag` and the constants |
| `src/blocks.ts`     | the block size (`BLOCK_BYTES`) and the number of blocks of a payload                                                                |
| `src/reassembly.ts` | `Reassembly`: the one payload in progress of a session, and the sequence rules of its blocks (frame specification §5.5)             |
| `src/session.ts`    | `connect` and the `Session` class: the consumer's state machine over one `WebSocket`                                                |
| `src/calls.ts`      | the calls in flight of one session (`Calls`, not exported from `mod.ts`): the caller counter, the bounds and the correlation        |
| `src/duplex.ts`     | `DuplexCall`: the two stream states of one duplex query, its credit and sequence counters and its first-reply bound                 |
| `src/errors.ts`     | the errors a session rejects with: `AttachRefused`, `ProtocolError`, `SessionError` and its `CallFailure`                           |
| `mod.ts`            | the public exports: everything exported by `frame.ts`, `errors.ts` and `session.ts`                                                 |

`src/calls.ts` was split out of `session.ts` during implementation, to keep the
session file at a readable size.

The codec defines `FRAME_VERSION` (2), `SUBPROTOCOL` (`ridl-ws-v2`),
`FRAME_VERSION_3` (3), `SUBPROTOCOL_V3` (`ridl-ws-v3`), `INITIAL_CREDIT` (4),
`SUBPROTOCOL_V1` (`ridl-ws-v1`), `MIN_MESSAGE_LIMIT` (294) and `ENVELOPE_BYTES`
(16), the `Kind` codes, `encodingTag`, which maps `flatbuffers`, `proto3` and
`podbuf` to 1, 2 and 3, and the three default ceilings of a payload with
`defaultCeiling(kind)` (D-19). As in the crate's D-3, the payload rules are in
the types: a `publish` carries a `PublishState` (`init`, `live` with a payload,
or `invalid`), a `response` a `ResponseOutcome` (`accepted`, `reply` with a
payload, `contract`, `corrupt` or `busy`), and an `occurrence` or a `request`
always carries a payload, which may be empty.

The codec encodes and decodes every form in both directions, as the crate's D-6
does, so a later providing side in TypeScript can reuse it. `decode` returns a
`Decoded` value and does not throw. A failure is one of two classes, as in the
crate's D-4: a **violation** (`too-short`, `control-too-long`, `unknown-code`,
`outcome-not-allowed`, `nonzero-detail`, `zero-number`, `bad-name-length`,
`bad-name`; the session ends) or a **discard** (`unexpected-payload`: a
`publish` or a `response` whose header says it carries no payload and which has
bytes after the header; the frame is recorded and dropped). `encode` allocates
the message and copies a payload into it once. `attach` refuses to build an
`attach` frame with an empty name or one longer than 255 UTF-8 bytes
(`AttachError` `empty` or `too-long`), an encoding tag outside 1 to 3 or not an
integer (`unknown-encoding`), or a hash that is not 32 bytes long
(`RangeError`).

**D-6. The numeric types.** `iface`, `ord`, `kind` and the codes are `number`;
`stamp`, `seq` and `correlation` are `bigint`, because an `i64` or a `u64` past
2^53 has no exact `number` form. The catalog hash is a 32-byte `Uint8Array`. On
encode, a value out of range is written as `DataView` writes it, wrapped; the
caller is trusted, as the crate's newtypes trust theirs.

**D-7. A payload is handed out as a view, not a copy.** The session sets
`binaryType = "arraybuffer"`; each message arrives as its own `ArrayBuffer`,
which nothing else holds, so a decoded payload is a `Uint8Array` view over the
bytes after the header. A payload the caller passes to a call is copied once
into the message. An upload's send callbacks can retain its original arguments
while the call remains in flight.

## The session (`src/session.ts`, `src/calls.ts`)

**D-8. The public API.**

```ts
const session = await connect("ws://127.0.0.1:9000", {
  catalog: { name: "veh.cabin", hash }, // hash: Uint8Array, 32 bytes
  encoding: "flatbuffers", // or "proto3" or "podbuf"; the default is "proto3"
  interfaces: [Cabin, Horn], // the generated tables, for each member's limit (D-19)
  onPublish(target, sample) {}, // each accepted publish of a subscribed signal
  onOccurrence(occurrence) {}, // each accepted occurrence, in arrival order
  onNote(note) {}, // violations, discarded frames, losses and throwing checks
  clock: () => taiMicros(), // optional; the request stamp (D-12)
});
```

`ConnectOptions` also takes `webSocket`, a factory for the socket, which the
unit tests use to pass a fake one; the default is
`new WebSocket(url, protocols)`.

- `connect` validates the name, the encoding and the hash with `attach` before
  any socket is created, and rejects with that `AttachError` or `RangeError`. It
  then opens the connection offering `ridl-ws-v3`, `ridl-ws-v2` and then
  `ridl-ws-v1`, sends the `attach` with the frame version of the subprotocol the
  server selected (3, 2 or 1) once the connection is open, and resolves with a
  `Session` on `attached(accepted)`. It rejects with `AttachRefused` (carrying
  the `reason` and a `closed` promise) on `attached(refused)`; with
  `ProtocolError` when the server broke the binding before `attached`; and with
  `SessionError` `down` when the connection closed or failed first. A server
  that selected no subprotocol is seen differently on the two platforms: in
  Deno, `connect` rejects with `ProtocolError('no-subprotocol')`; in a browser,
  the platform fails the handshake itself, so `connect` rejects with
  `SessionError` `down` (close 1006). A URL the platform refuses rejects the
  promise; `connect` does not throw.
- `subscribe(target, opts?)` resolves on `answer(accepted)` and rejects with a
  `contract` `SessionError` on a refusing `answer`. `unsubscribe(target)` sends
  the frame and drops what the session held for the interaction.
  `read(target, opts?)` resolves with the sample of the next `publish` of that
  interaction, unless a cancelled read ahead in the queue consumes that reply
  without settling the reads behind it. `sample(iface, ord)` returns the held
  sample of a subscribed signal. An interface number or an ordinal that is not
  an integer from 1 to 4294967295 makes `subscribe`, `read`, `command` and
  `query` reject with a `RangeError`, and `unsubscribe` throw one, and nothing
  is sent.
- A `Sample` is
  `{ provenance: "init" | "live" | "invalid", envelope, payload, lastGood, detected? }`,
  with `payload` a `Uint8Array` under `live` and `null` otherwise. `lastGood`
  holds the last passing bytes, or `null` when the init value applies (D-24).
  `invalid` with no `detected` is `Invalid(Declared)`, as the provider published
  it; `invalid` with `detected: "corrupt"` or `{ rule: Rule }` is a locally
  detected invalid value. An `Occurrence` carries `detected: "corrupt"` and an
  empty `payload` when it exceeds its limit.
- `command(iface, ord, args, { bound })` resolves on `accepted`;
  `query(iface, ord, args, { bound })` resolves with the reply's payload, an
  empty `Uint8Array` when the reply has no bytes. Both reject with a
  `SessionError` whose `failure.category` is `contract` (with the `Contract`,
  and the `Rule` under `invalid-value`), `corrupt` or `busy` as the provider
  answered, or, detected by the client, `corrupt` (a reply, or the arguments
  before they are sent, longer than the member's limit, D-19), `undelivered` (a
  command's bound passed), `timeout` (a query's bound passed; also the category
  when a bound passes on `connect`, `subscribe` or `read`) or `down` (the
  session ended).
- `close()` sends a close with code 1000 and rejects every waiting operation
  with `down` at once. `closed` resolves once, with an `Ended`: the close code
  and reason the platform reported (1006 when the connection failed), and
  `refused` or `violation` when either ended the session.

Listeners are used rather than a pull `next()`, because a JavaScript program
waits on a callback or a promise. An async iterator can be added over the
listeners later without a break.

**D-9. The consumer's rules the session applies.** Those of §5 to §9 that need
no payload decoding, with the one fact of the catalog they need, each member's
limit, read from the generated tables (D-19):

- **Signals (§5.1, §7).** One held sample per subscribed signal. A `publish`
  whose `seq` is less than the held sample's is discarded and recorded
  (`stale`), except that one that arrives while a `read` of the signal is
  waiting settles the read and records nothing. A `publish` whose `seq` equals
  the held sample's is dropped and not recorded: the server reuses a session
  `seq` only for an identical sample (the crate's D-24), except `Init`, which
  always carries `seq` 0, so such a `publish` is the reply to a `read` that an
  earlier `publish` already settled, a duplicate, or an `Init` while an `Init`
  is held. `unsubscribe` drops the held sample, and a `publish` of an
  interaction that is not subscribed and has no `read` waiting is discarded and
  recorded (`not-subscribed`), which covers frames in flight after an
  `unsubscribe` (§6.2). `onPublish` is called for a `publish` that becomes the
  held sample of a subscribed signal; a `publish` that only answers a `read` of
  an unsubscribed signal settles the read and is not passed to `onPublish`.
- **Events (§5.2, §7).** Occurrences of a subscribed event are delivered in
  arrival order. A `seq` not greater than the last accepted one is discarded
  (`stale`); a gap between two accepted occurrences of one channel is recorded
  as a `loss`, with the expected and the received `seq`, and delivery continues.
  The time-to-live discard needs the catalog's `max` and is not done.
- **Calls (§5.3, §5.4, §7).** One caller counter per session, from 1, used as
  the `request`'s `seq`. A `response` is matched on `correlation`. One whose
  correlation names no call in flight is discarded and recorded
  (`unknown-correlation`), which includes a response that arrives after its
  call's bound. One whose outcome does not fit its call's kind (`accepted` for a
  query, `reply` for a command) is discarded and recorded
  (`unexpected-outcome`), and the call stays in flight until its bound: the
  specification gives no outcome for this provider defect. The outcome is
  compared with the call's kind, not with the frame's `kind`, because §6.4 lets
  a `contract(UnknownInteraction)` response echo another kind. Each call has a
  bound in milliseconds, given by the caller because no descriptor is read and
  §11.1 item 5 says every call has one. A bound that is not a finite number
  greater than 0 and at most 2^31 - 1 (the longest `setTimeout` delay) rejects
  the returned promise with a `RangeError` and sends nothing.
- **Answers (§6.2, §6.3, §6.4).** `subscribe` and `read` carry no correlation,
  so their answers are identified by `(kind, interface, ordinal)`. The session
  keeps, per identity, a first-in first-out queue of the waiting `subscribe` and
  `read` operations: an `answer` settles the operation at the head of the queue,
  and a `publish` settles consecutive live reads at the head, or consumes one
  cancelled placeholder without settling later reads (D-11). Without
  cancellation this is exact for the server as built, which handles each message
  as soon as it is read (the crate's D-8), and §6.2 puts a signal's `answer`
  before its first `publish`. An `answer` with nothing waiting, or an accepted
  `answer` whose head is a `read`, is discarded and recorded
  (`unexpected-answer`). A refused `read` rejects with
  `contract(unknown-interaction)`. A `publish` that answers a `read` of a
  subscribed signal with a `seq` equal to the held one settles the read and is
  not recorded as stale, because the crate reuses the session `seq` for a read
  (the crate's D-24). When a live `publish` of the subscription settles the
  `read` first, the read's own reply then arrives with the same `seq` and is
  dropped, as the signals rule above says.
- **An `unsubscribe` before its `subscribe`'s answer.** The accepted answer
  settles the `subscribe`, and the interaction is not subscribed, for a signal
  and for an event: frames in flight after an `unsubscribe` are discarded
  (§6.2). A caller that unsubscribes and subscribes again waits for the second
  answer.
- **Listeners.** The session updates its state before it calls a listener, and
  calls each listener in a `try`/`catch`. An exception a listener throws is
  passed to `globalThis.reportError`, as an uncaught exception in an event
  handler would be, and does not stop the handling of the frame, the close on a
  violation, or the loss tracking.
- **The end of a session (§3).** On a close or an error, every waiting operation
  and every call in flight rejects with `down`, every later operation rejects
  with `down`, and `closed` resolves. On a refused `attach` the client records
  the refusal and closes with 1000 at once, so a provider that does not close
  leaves no connection open; `AttachRefused.closed` resolves on the close event.
  When the platform reports an `error` event, the session ends with 1006, and
  the code and reason of the `close` event that follows are not reported.

**D-10. A protocol violation closes without a code, because the platform cannot
send 1002.** §11.1 item 4 has the receiver close with 1002, and 1003 for a text
message. The browser's `WebSocket.close` and Deno's accept only 1000 and 3000 to
4999, and throw `InvalidAccessError` for 1002. The binding defines no code of
its own, so a private code in 4000 to 4999 is not used. On a violation of the
codec, a text message (`text-message`), a server that selected no subprotocol
(`no-subprotocol`, which Deno reports; a browser fails the handshake itself and
the session ends with 1006, see D-8), or a form the provider must not send at
that point (`unexpected-form`: a first message other than `attached`, a second
`attached`, a consumer form), or a frame that breaks the reassembly rules of
frame specification §5.5, a `block` out of sequence or a whole `publish` or
`answer` of the `fixed` being reassembled (`block-sequence`), or a message whose
form is 11 on a `ridl-ws-v1` connection (`unknown-code`) the session calls
`close()` with no code, which sends a close frame with no status (the server
sees 1005), records the violation through `onNote`, and ends with `violation`
set in `Ended`. A message is not checked against a message limit: the platform
sets the limit of the connection, and the client cannot close with 1009. What is
checked is the length of each received payload against its member's limit
(D-19), which refuses the payload and does not end the session; a payload in
blocks is refused at its first block, from `total`. driftsys/ridl#991 asks the
frame specification to state the close of a client that cannot choose a close
code.

**D-11. No client liveness timer.** §11.1 item 1 has either end treat no message
of any kind within its liveness timeout as the end of the session. A browser's
`WebSocket` answers the server's pings and does not show them to the page, so on
an idle session the page sees no message while the connection is healthy, and a
timer would end healthy sessions. The client uses the platform's `close` and
`error` events and the server's liveness check (the crate's D-22) to detect a
lost connection. A server that is gone is detected when the platform's
connection fails; a server host that vanished without a TCP reset is detected
only at the operating system's timeout. A call ends at its bound if no answer
arrives while its timer runs; a streamed reply clears that timer at its first
chunk and then ends only with its stream end or the session. A `queryWithUpload`
with an open upload has no reply timer until the upload closes (D-23). `connect`
accepts optional `bound` and `signal` fields; `subscribe(target, opts?)` and
`read(target, opts?)` accept the same fields through the exported `WaitOptions`
interface. A bound is a finite number of milliseconds greater than 0 and at most
2147483647; an invalid bound rejects with `RangeError` before a socket is
created or a frame is sent. A passed bound rejects with `SessionError` category
`timeout`, and an aborted signal rejects with `signal.reason`. An already
aborted signal creates no socket or sends no frame. Connect's bound starts at
the call and ends at `attached(accepted)`; timeout or abort closes the socket,
ends the session and ignores late events. With neither option, each waits until
its answer arrives or the session ends. Every settlement removes its timer and
abort listener.

A cancelled subscribe or read remains as a settled placeholder in the FIFO queue
until a reply consumes it or the session ends: answers carry no correlation id,
so the client cannot distinguish its late reply from a later operation's reply,
and cancelling many operations whose replies never arrive retains all their
placeholders until the session ends. A cancelled read also continues to absorb a
payload whose blocks are being reassembled, without settling a later read. A
cancelled subscribe is marked dropped, so late acceptance does not establish a
subscription, but its first publish can still produce a `not-subscribed` note.
When no subscription is established and no other live subscribe waits,
cancellation sends `unsubscribe` without dropping a previously established
subscription's held sample or event sequence. driftsys/ridl#991 also asks the
specification to state what a client that cannot see pings does.

**D-12. The default request stamp is wall-clock UTC in microseconds.** A
`request`'s `stamp` is in the platform's synchronized time base, TAI (ridl
§3.1). A browser has `Date.now()`, which is UTC in milliseconds, and no TAI. The
`clock` option, `() => bigint` in microseconds, lets a program supply a TAI
clock; the default is `Date.now()` times 1000, which differs from TAI by the
leap seconds. The README states this. The server as built evaluates no timing
rule on a request stamp.

**D-19. The size of a received payload is checked against the member's limit
from the generated tables.** The catalog is the one source of a payload limit
([ADR-0030](../decisions/ADR-0030-transfer-and-bound-limits.md) decisions 13 and
19): every interaction has the default ceiling of its kind unless a `[limits]`
entry of the manifest gives it another (the member's `[limits.interaction]`
entry, else the entry of its kind in `[limits.kind]`, else `default` clamped to
the default ceiling of its kind), and the generated `<base>.codec.ts` module
writes the resolved value as `limit` on the member's table entry
([`ts-codec-table.md`](ts-codec-table.md)). So `connect` takes the generated
tables in the `interfaces` option and reads each member's limit from them, keyed
by `(kind, interface, ordinal)`; a member no table names, and every member when
no table is given, has the default ceiling of its kind, which `frame.ts` exports
as `SIGNAL_EVENT_CEILING` (3,840 bytes), `CALL_CEILING` (16,128) and
`FIXED_CEILING` (65,280) with `defaultCeiling`. No hand-written limit option
exists, so that a limit cannot be stated twice. The Rust crate `ridl-rt` holds
the same three values in `ridl_rt::contract`, and the crate's test
`tests/ts_ceilings.rs` reads `frame.ts` and `blocks.ts` and compares them, with
the block size, the frame version and the two subprotocol names of the crate.

The check runs on receive, before a payload reaches the application, and
compares the payload's length with the limit: a payload of exactly the limit
passes. What is refused is reported as the frame specification §9.1 and §9.2
have a detected invalid value reported, so that nothing fails silently:

- a `reply` longer than the query's limit rejects the query with `SessionError`
  `corrupt`, the category the provider uses for a request it refuses for size;
  the limit is the one of the member the query named when it was sent, not one
  read from the response's `target`, which a `contract` echo may carry under
  another kind (§6.4);
- a `publish` that answers a `read`, of a signal or of a `fixed`, resolves the
  read with an invalid sample as defined in D-24;
- a `publish` of a subscribed signal is delivered through `onPublish` as a
  sample with provenance `invalid`, `payload` `null` and `detected: "corrupt"`,
  with the envelope of the frame (§9.1) and `lastGood` as defined in D-24, and
  becomes the held sample; the sequence rules of D-9 apply to it as to any
  publish. A later payload within the limit that passes its check returns the
  signal to `live`;
- an `occurrence` is delivered through `onOccurrence` with an empty payload and
  `detected: "corrupt"`, with the envelope of the frame (§9.2); it counts in the
  event's sequence, so no loss is recorded for it.

Nothing is sent back, and the session does not end. The client checks request
arguments before sending: `command` and `query` reject with `corrupt` when the
arguments are longer than the member's limit, and send nothing. That is the
answer the provider gives a `request` above the limit (the crate's D-26), but
only to arguments that one message carries; a larger message is above the
provider's message limit, which closes the session with 1009 and rejects every
call in flight with `down`. A `fixed` whose value is longer than one message
arrives in `block` frames of at most 4,096 bytes, which the session reassembles.
The client also sends upload chunk payloads through `commandStream`,
`queryWithUpload` and `queryDuplex`; their separate size refusal sends no chunk
and interrupts the upload (D-23).

**Blocks.** `src/reassembly.ts` holds the one payload in progress. The rules it
applies, and the answers to each sequence error, are frame specification §5.5;
this record does not restate them. What the session adds is the following. A
payload goes into one buffer of `total` bytes, allocated at the first block, and
is handled as a `live` `publish` that arrived whole after its last block. The
reads that the payload answers are the consecutive live reads at the head of the
identity's queue when its first block arrived; a cancelled read ahead first
consumes one reply without settling the reads behind it. A `read` sent while the
payload is being reassembled waits for the next payload. The limit used at the
first block is the member's limit held to 65,280 bytes. A message whose form is
11 on a connection that selected `ridl-ws-v1` ends the session, because that
form is outside the table of frame version 1. The session refuses it before it
decodes it, and before `attached` too, so a malformed block on that session gets
the same violation code, `unknown-code`, as a well-formed one; the decoder
itself reads form 11 on any session.

The codes of the failures. A block that is wrong by itself is the violation
`bad-block`; a `kind` outside the table stays `unknown-code` and an `interface`
or `ordinal` 0 stays `zero-number`. A sequence error is the session violation
`block-sequence`. A `fixed` payload refused for its size, whether it crossed
whole or in blocks, resolves consecutive live reads at the head of its
identity's queue with an invalid sample (`detected: "corrupt"`,
`lastGood: null`), using the whole publish's or first block's envelope, unless a
cancelled read ahead first consumes the reply without settling the reads behind
it. It is recorded once as the note
`{ note: "discarded", reason: "over-limit", frame }`, with the `publish` or the
first block as `frame`, in place of the `not-subscribed` note that a payload no
`read` waits for gets. The consumer does not know the provider's message limit,
so the report of a `fixed` must not depend on whether the provider split it. A
signal's or an event's payload above its limit keeps the `detected` report of
D-19 and records no `over-limit` note; an unsolicited one is still recorded
`not-subscribed`, as any unsolicited frame is.

The session treats every identity as having no member table. The `interfaces`
option lists only the members of the generated tables, and the generated module
leaves out a member whose payload has no finite FlatBuffers bound, so the table
is partial. Each listed entry carries its `kind`, which would let the session
tell that a block does not name a `fixed`; it does not use that, on purpose. It
reassembles any identity within that identity's limit (the entry's, else 65,280,
and never more than 65,280) and hands the result to the existing `publish` path,
which discards it as `not-subscribed` when no `read` waits. The cost is that a
block payload that names an identity the tables list under another kind, which
only a provider defect can send under a matching catalog hash, is copied, up to
65,280 bytes, before it is discarded. The gain is one rule, no second set of
identities and no second discard path in the session.

The two exceptions of frame specification §5.5 (a whole `publish` and an
`answer` of the `fixed` being reassembled) compare `(kind, interface, ordinal)`:
`Reassembly.holds` is true when the frame's `kind` is `Fixed` and its
`interface` and `ordinal` are the slot's. The session keys every queue of
waiting operations by an identity that holds the kind, so a frame of another
kind settles or shifts another queue and cannot change the reads that the blocks
answer. A frame the codec discards for bytes after its header is recorded and
the reassembly goes on.

**D-20. A streamed reply.** `queryStream(iface, ord, args, { bound })` returns a
`ReadableStream<Uint8Array>` whose items are the payloads of the `chunk` frames
in order; the stream machinery is in `src/calls.ts`. The rules of the wire are
frame specification §5.4 and §11.1 "Binding version 3". What the client decides
is the following.

- **Stream targets (§6.4).** Each call retains its target. Before applying a
  `chunk`, `stream_end` or `stream_credit`, the client compares the interface,
  ordinal and kind with the call named by `correlation`. A mismatch is discarded
  and recorded through `onNote` as `unknown-correlation`, without changing
  credit, sequence, timers or streams. This check applies to plain calls,
  streamed replies and uploads, before checking the frame's direction. A
  matching stream frame for a plain call with neither an upload nor a streamed
  reply is `unexpected-form`.
- **The credit is the stream's `pull`.** The stream has a high-water mark of 4
  chunks, the initial credit. The session tracks `granted`, the chunks it has
  granted, and `received`. On each `pull` it computes
  `room = desiredSize − (granted − received)`, and when `room` is above 0 it
  adds `room` to `granted` and sends `stream_credit` for `room`. So the server
  may send what the reader's queue can still hold and no more, and the
  platform's backpressure and the frame's credit are one mechanism, and the
  session needs no second queue. The technote
  [`websocket-binding-rationale.md`](../technotes/websocket-binding-rationale.md)
  gives the alternatives.
- **`unsupported`.** On a session of frame version 1 or 2, `queryStream` sends
  nothing and returns a stream that errors with a `SessionError` of the category
  `unsupported`. A missing capability is not a defect of the peer, so it does
  not end the session. `CallFailure` has the categories `interrupted` and
  `unsupported`.
- **Cancel.** Cancelling the stream sends `stream_end interrupted` and forgets
  the call.
- **Errors.** The stream errors with the category of the server's `stream_end`:
  `contract`, `corrupt`, `busy` or `interrupted`. A gap in the stream's `seq`, a
  `chunk` or a `stream_end` that skips a value, errors it `interrupted`. After a
  gap in the `seq` of a `chunk` the client sends `stream_end interrupted`; after
  a gap in the `seq` of a `stream_end` it sends nothing, because the stream has
  ended. A chunk longer than the member's limit errors it `corrupt` and sends
  `stream_end corrupt`. When `bound` milliseconds pass with neither a `chunk`
  nor a `stream_end`, it errors `timeout` and the client sends
  `stream_end interrupted`; the bound ends at the first frame, as
  [ADR-0032](../decisions/ADR-0032-byte-streams.md) decision 13 states. When the
  session ends, an open stream errors `down`. An argument longer than the
  member's limit errors the stream `corrupt`, and a bound or a number that
  `command` refuses errors it with a `RangeError`, in both cases with nothing
  sent. The client's `stream_end` carries `seq` 1.
- **A refusing `response`.** A `response` for the call whose outcome is
  `contract`, `corrupt` or `busy`, received before the stream's first frame, is
  that stream's refusal: the stream errors with the category and the session
  stays open. A server that does not know a member's stream cannot answer with a
  `stream_end`, and answers `contract(UnknownInteraction)` (frame specification
  §5.4), so one wrong ordinal must not end a session. After a `chunk`, the same
  `response` is the violation `unexpected-form`, because the reason holds only
  before the reply starts. A `response` with `reply` or `accepted`, a
  `stream_credit` from the server for this reply stream, a chunk beyond the
  credit granted, and a stream frame on a stream other than 0 are the violation
  `unexpected-form` as well, and a violation errors every open stream with
  `down`.
- **The frame version is private.** The session holds the version the server
  selected and exposes no accessor for it; a caller learns that the server does
  not stream from `unsupported`.

**D-21. The providing side, `provide`.** The code is `src/provision.ts` and
`src/provision_calls.ts`. It is the client of the subprotocol
`ridl-ws-v2-provider` (frame specification §11.1, "Reversed role"), additive to
`connect`.

```ts
const provision = await provide(
  "ws://127.0.0.1:9001/alpha.First/Unit/alpha", // the path is the gateway's
  {
    catalog: { name: "alpha", hash }, // hash: Uint8Array, 32 bytes
    encoding: "flatbuffers", // required: there is no default
    members: [{ iface: 1, ord: 1, kind: "signal" } /* ... */],
    onRequest(request) {
      return { outcome: "accepted" };
    },
    onNote(note) {},
    clock: () => taiMicros(), // optional; the stamp of a response and the initial publish
  },
);
// Update the initial state before the gateway subscribes (R-DS-10).
provision.publish(1, 1, payload, stamp); // stamp: bigint microseconds
```

- **The options.** `encoding` is explicit, because the encoding that `connect`
  defaults to is not the one the server necessarily serves: the debug gateway
  serves FlatBuffers. `members` lists what the instance provides, as the
  `MemberEntry` values of `connect` (kind, interface, ordinal and an optional
  limit; D-19). `onRequest` answers each command and query. `provide` validates
  the catalog, the encoding and the members before it creates a socket, and
  rejects with the `AttachError` or `RangeError`, as `connect` does.
- **The attach.** The package offers `ridl-ws-v2-provider` only and waits for
  the server's `attach`. It accepts the `attach` when its frame version is 2,
  its name equals the catalog name, its hash equals the catalog hash **exactly**
  (the package reads no list of compatible hashes), and its encoding is the one
  given; otherwise it answers `attached(refused)` with the first reason that
  fails, in that order, closes with 1000 and rejects with `AttachRefused`. When
  it accepts, it answers `attached(accepted)` and resolves the promise with a
  `Provision`.
- **The session.** `publish`, `invalidate` and `raise` take the stamp as an
  argument, a `bigint` in microseconds that fits an `i64`, because the package
  has no clock for the value's time. Under R-DS-10, `publish` and `invalidate`
  update the current signal state even before subscription; the initial value is
  sent after the `answer` when the subscription arrives (§5.1). A `raise`
  without a subscription is not delivered and records a `discarded` note with
  reason `not-subscribed`; it does not throw. It still advances the session's
  event channel sequence counter (frame §7). After resubscription, the next
  delivered occurrence can therefore show a gap, which the Rust consumer records
  as loss (§5.2). Each operation is refused with a `SessionError` `contract`
  `unknown-interaction` for an unlisted member, with `corrupt` when the payload
  exceeds the member's limit, and with `down` after the session ended; an
  interface or ordinal that is not a nonzero `u32`, or a stamp outside `i64`,
  throws a `RangeError`. The session counts `seq` per channel from 1. A signal
  keeps its last frame, so the first `subscribe` of a signal and each `read` of
  it send that frame, or `init` (`seq` 0, stamped at the attach) when nothing
  was published; an event is not kept. A `subscribe` of a member that the
  options do not list, or of another kind, is answered
  `contract(UnknownInteraction)`.
- **Requests.** A `request` for a member that is not a listed command or query
  is answered `contract(UnknownInteraction)`; arguments longer than the member's
  limit, `corrupt`; and a request that arrives while 64 are in flight, `busy`.
  Otherwise `onRequest` receives a copy of the payload, may be asynchronous, and
  its outcome becomes the `response`: a reply longer than the limit is sent as
  `corrupt`. An `invalid-value` violation with rule `utf8` is also sent as
  `corrupt`, because this providing session uses frame version 2 (§9.6). The
  session keeps the last 64 settled responses, because it reads no descriptor
  and so knows no bound, and a repeated `seq` is answered with the kept response
  and not served again.
- **The difference from the Rust provided session.** The frame has no outcome
  for a defect of the provider, so the TypeScript provider sends **no
  `response`** when `onRequest` returns an outcome outside the call kind's set
  (`accepted` for a query, `reply` for a command), returns nothing, throws or
  rejects; it records
  `{ note: "discarded", reason: "unexpected-outcome", frame }` through `onNote`
  and releases the slot. A clock or encoding failure while settling a request is
  recorded in the same way and never escapes the message callback. A synchronous
  refusal advances neither the replay high-water mark nor the response counter
  until it is sent successfully, so a retry can settle it. The Rust provided
  session, which is the consumer of these responses, settles an outcome outside
  the set as `Corrupt` at once (the crate's D-16), so a call to a TypeScript
  provider that failed ends at the Rust call's own deadline. A handler that
  never settles holds its slot among the 64 until the session ends, and the
  server's deadline reports the call.
- **The limits of a browser (R-DS-7).** The platform's `WebSocket` closes with
  no code except 1000 and 3000 to 4999, and shows no pings to the page, so the
  providing side follows D-10 and D-11 exactly. On a violation (a text message,
  a server that selected no subprotocol, a form the consumer must not send at
  that point, and `chunk`, `stream_end` and `stream_credit`, which are not part
  of the reversed role) it calls `close()` with no code, records the violation
  through `onNote`, and ends the session. It has no liveness timer; the server's
  check is the one that runs. `Provision.close()` closes with 1000, and
  `Provision.closed` resolves on the platform's close or error event.
  driftsys/ridl#991 asks the frame specification to state what such a client
  does for `connect` and the consumer role. The reversed role has the same
  specification gap; the issue should be widened to cover it.

**D-23. An upload.** The upload methods return platform streams and, for
non-streamed outcomes, promises:

```ts
commandStream(iface: number, ord: number, args: Uint8Array, opts: { bound: number }):
  { upload: WritableStream<Uint8Array>; accepted: Promise<void> };
queryWithUpload(iface: number, ord: number, args: Uint8Array, opts: { bound: number }):
  { upload: WritableStream<Uint8Array>; reply: Promise<Uint8Array> };
queryDuplex(iface: number, ord: number, args: Uint8Array, opts: { bound: number }):
  { upload: WritableStream<Uint8Array>; stream: ReadableStream<Uint8Array> };
```

All three send a `request` that opens parameter stream 1. Its chunks and end
carry the request's caller number as `correlation`, the same target and a stream
`seq` starting at 1. Upload state belongs to `src/calls.ts` for `commandStream`
and `queryWithUpload`, and to `src/duplex.ts` for `queryDuplex`;
`src/session.ts` encodes and sends their frames. The server's receiving rules
are in [the server record](ridl-transport-ws.md), D-30. Each method carries one
upload. `commandStream` returns an acknowledgment, `queryWithUpload` a plain
reply, and `queryDuplex` a concurrent reply stream (added 2026-11-04). No method
carries two or more parameter streams (driftsys/ridl#1224).

- **Writes and credit.** One `write` sends one `chunk`, including an empty
  write, and consumes one credit. The initial credit is 4 chunks; each server
  `stream_credit` on stream 1 adds its grant. A write waits when the credit is
  spent and resolves when its chunk is sent. The sink uses the platform's
  default queuing strategy, with high-water mark 1, and has at most one active
  write waiting for credit. Queued writes remain in the platform stream. A
  resumed write checks for failure again before sending, so a credit followed by
  refusal in the same turn sends no extra chunk. A write longer than the
  member's limit sends no chunk: it sends `stream_end interrupted`, errors the
  upload, rejects any pending result with `corrupt` and forgets the call. For
  `queryDuplex`, it also stops stream 0 after stream 1, as described in **Duplex
  queries** below. A write exactly at the limit sends one whole chunk. The
  [binding rationale](../technotes/websocket-binding-rationale.md#streams-arrive-as-frame-version-3-and-binding-version-3)
  explains why the client preserves write boundaries.
- **Close.** After the queued writes complete, `close` sends `stream_end end`
  with the next sequence and resolves. There is no acknowledgment of provider
  consumption of the upload end. Close therefore confirms local sending only; it
  cannot confirm acceptance of the final chunks or detect every late refusal. A
  refusal the client has already processed errors the upload and rejects a
  pending write or close. An acknowledged command is forgotten when its upload
  closes, and a `stream_credit` that arrives for it afterwards is ignored
  without a note (frame specification §11.1); a command still waiting for
  acknowledgment remains until its response or bound. A query remains for its
  plain or streamed reply.
- **Acknowledgment and reply.** `accepted` resolves on the command's
  `response accepted`, leaving its upload open. A plain reply within the
  member's limit resolves `reply` with its payload and forgets the query. If the
  upload is still open, that reply errors it and releases a write waiting for
  credit with `interrupted`. After an early plain reply, the server sends
  `stream_end interrupted` on the upload. The client has already forgotten the
  call and records that end as a discard with reason `unknown-correlation`. A
  reply above the limit rejects both with `corrupt`. An outcome that does not
  fit the call's kind is discarded and recorded as `unexpected-outcome` without
  settling the call (D-9).
- **Abort.** Aborting sends `stream_end interrupted` on stream 1 with the next
  sequence and releases a write waiting for credit. For `queryWithUpload`,
  `reply` rejects `interrupted` and the query is forgotten. For `queryDuplex`,
  abort also stops stream 0 as described below. A command retains a pending
  acknowledgment until its response or bound; an already acknowledged command is
  forgotten. A resolved `accepted` stays resolved.
- **Refusals and direction.** A server `stream_end` on stream 1 with `seq` 1
  errors the upload with its category (`contract`, including the rule under
  `invalid-value`, `corrupt`, `busy` or `interrupted`), rejects a pending
  `accepted` or `reply` with the same error and forgets the call. A server end
  with another sequence, or with outcome `end`, is treated as `interrupted`: it
  is not a successful upload acknowledgment. A refusing `response` also errors
  the upload and rejects its pending result with the same failure. A resolved
  `accepted` stays resolved. Refusal rejects a write waiting for credit at once.
  For `commandStream` and `queryWithUpload`, an inbound `chunk` on any stream or
  any stream frame on a stream other than 1 is `unexpected-form`. `queryDuplex`
  also accepts reply chunks and ends on stream 0 as described below. The target
  check above precedes these rules. D-20's prohibition on inbound credit still
  applies to a streamed reply.
- **Send exceptions and session end.** If sending the request, a chunk or an end
  throws, the client forgets the call, clears its timer and rejects its pending
  `accepted` or `reply` with that exact error object. A failed chunk or end send
  also rejects the writer operation with that error. This includes the
  interruption sent for an oversized write: the send exception takes precedence
  over the local `corrupt` error. The send failure alone does not close the
  session. When the session ends, an upload still in flight errors with `down`
  and its pending result rejects with `down`.
- **Bounds.** A command's acknowledgment timer starts after its request is sent;
  expiry rejects `accepted` and errors its upload with `undelivered`.
  `queryWithUpload` starts its reply timer only when `close` sends
  `stream_end end`; expiry rejects `reply` and errors the upload with `timeout`.
  Before close, `queryWithUpload` has no client timer; the server bounds
  inactivity through abandonment (D-30). Both timer expiries forget the call.
  `queryDuplex` starts its first-reply-frame bound at the request, as described
  below.
- **Preflight.** On a session below version 3 all three methods return an
  errored upload with `unsupported`, sending nothing. `commandStream` and
  `queryWithUpload` also return a rejected result promise; `queryDuplex` returns
  an errored reply `ReadableStream`. Invalid interface numbers, ordinals or
  bounds fail both returned operations with the same `RangeError` as D-9.
  Arguments above the member's limit fail both with the same `SessionError`
  `corrupt`. An ended session fails both with `down`.

**Duplex queries.** `queryDuplex` combines stream 1's upload rules above with
D-20's reply stream 0 under one correlation. Each direction has its own initial
credit of 4, credit counter and sequence counter. The target check precedes both
directions' processing. Reply chunks can arrive before upload close; normal
upload close leaves the reply open. A normal reply end closes the reply and
errors an upload still open with `interrupted`. Reply cancel sends one stream-0
interruption and errors the upload. An abort of an upload that is still open
sends `stream_end interrupted` on stream 1 and then on stream 0, errors the
reply and forgets the call; an abort after upload close sends nothing, as the
platform defines for a closed `WritableStream`. Abort resolves once both
interruptions are sent. An oversized upload write sends the same interruptions
in that order, fails both local streams with `corrupt` and forgets the call.
Either cancellation rejects a write waiting for credit with `interrupted`,
including one the sink has not yet processed. A refusal of either stream, a
refusing response before the first reply frame, or session end fails both
streams with the same error object. A plain response, a response after the reply
begins, inbound reply credit or inbound upload chunks violates the protocol as
in D-20. Send exceptions release the call and fail both directions with the
original exception.

The duplex first-reply-frame bound runs from the request and stops at the first
reply chunk or end; upload progress and close neither stop nor restart it. The
server's shared abandonment rule (D-30 in
[the server record](ridl-transport-ws.md)) abandons only when neither direction
has progressed within `call_abandon_after`. This client adds no inactivity
timer. These rules follow frame specification §5.4 and §11.1; upload close does
not confirm provider consumption. `src/duplex.ts` owns the two stream states;
`src/calls.ts` retains the shared call and checks its target. The socket-free
`tests/duplex_test.ts` covers credit, close, cancellation, abort, refusal,
bounds, preflight, malformed replies and send exceptions. The loopback e2e cases
`queryDuplex_echoes_64_chunks` and `queryDuplex_cancel_and_abort` cover
concurrent echo and interruption followed by an ordinary call on the same
session. Their TCP execution status is recorded in D-14 below.

**D-24. The signal check.** `ConnectOptions.checks` registers a readonly list of
`SignalCheckEntry` values (`iface`, `ord`, `check`); a later entry for the same
signal replaces an earlier one. `connect` validates every interface number and
ordinal with `checkNumbers` in its preflight, rejecting with `RangeError` before
creating a socket. `Session.checkWith(iface, ord, check)` registers or replaces
one check and validates the numbers even after the session ends; valid
registration after the end does nothing. The exported types `Detection`
(`"corrupt" | { rule: Rule }`), `SignalCheck` and `SignalCheckEntry` are the
types of `@driftsys/ridl-runtime`, re-exported. A check returns `undefined` when
its bytes pass.

In `#onPublish`, the size limit and sequence tests precede the check, and the
check precedes holding the sample. Only live signal payloads within the limit
are checked. Duplicate and stale frames are not checked unless they answer at
least one unsettled read. The session computes the held sample and drop decision
first, builds a sample only for a frame it will hold or an unsettled read it
will answer, checks at most once, resolves the reads, and then applies the drop
decision and holds the sample. Existing `not-subscribed`, `stale` and
`over-limit` notes retain their rules. Each held sample still reaches
`onPublish`, including a refused one.

The sample carries the frame's envelope. Its bytes follow frame specification
§5.1:

| Publish state                     | `payload` | `lastGood`                       | `detected`    |
| --------------------------------- | --------- | -------------------------------- | ------------- |
| Live, no check or check passes    | the bytes | the same bytes                   | absent        |
| Live, check refuses or over limit | `null`    | the held `lastGood`, else `null` | the detection |
| Init                              | `null`    | `null`                           | absent        |
| Invalid (Declared)                | `null`    | the held `lastGood`, else `null` | absent        |

A refused live sample has provenance `invalid`. Only a subscribed signal has
held bytes: an unsubscribed signal read or any non-signal uses `null` for the
held `lastGood`. `unsubscribe` drops the held sample and its good bytes; a later
subscription starts with none. A read answered by a refused or over-limit
payload resolves with the invalid sample, including its envelope, detection and
`lastGood`. This also applies to an over-limit fixed payload in blocks: the
sample uses its first block's envelope and `lastGood: null`.

After registration, `checkWith` immediately checks a held live payload. Refusal
replaces it with provenance `invalid`, `payload: null`, `lastGood: null` and the
detection, preserving the envelope. A passing check leaves the held sample
unchanged. Registration is not a publication: it calls no `onPublish` and
changes no sequence. A check that throws, during publication or rechecking,
counts as `"corrupt"` and emits one
`{ note: "check-failed", iface, ord, error }` note with the thrown error. The
note follows state updates and read resolution, and follows `onPublish` when a
publication is held, so a listener can close or unsubscribe without a later
state update restoring the sample.

## The end-to-end tests

**D-13. The test server is a cargo example in the crate.**
`crates/ridl-transport-ws/examples/loopback_server.rs` serves the crate's test
catalog (`test.ws`, the provider of `tests/support/provider.rs`, included by
path so the catalog is defined once) on `127.0.0.1:0`, prints `port <n>`, serves
each connection on its own thread with its own
`Described::new(Streaming(Scanning(loopback.attach())), table)` backend, where
`table` is the hand-built member table of the test catalog, steps the provider
every 5 ms on its own thread, and exits with status 0 when its standard input
ends. It declares `required-features = ["server"]`, and the crate's `exclude`
leaves out `examples/`, as it leaves out `tests/`, because the example includes
a file from `tests/` and uses the dev-dependency `ridl-loopback`. `serve` is
unchanged; the example only wraps its backend in a member table and in
`Streaming`. The crate's record lists the example under its tests.

**D-22. The providing proof is a Deno test against the debug gateway.**
`e2e/provide_test.ts` starts the cargo example `debug_gateway_fixture` of the
crate from the path in `RIDL_DEBUG_GATEWAY_FIXTURE`, which serves the committed
identity deployment of `ridl-rt-conformance` on a listening port and an
injection port. The test connects a listener with `connect` and a provider with
`provide` (`/alpha.First/Unit/alpha`, FlatBuffers), and, through the fixture's
control channel on standard input, checks what a local consumer on the machine
sees: a published value, a declared-invalid value, an event, and a command and a
query that the provider answers; that the instance attached as itself is refused
while the twin is attached; and that it is released after `close()`. The
listener also receives each value. `just ts-e2e` runs it after
`e2e/deno_test.ts`. It runs in Deno only: the browser proof of D-14 covers the
consuming side.

**D-14. How the proof runs.** "A Deno program and a browser page each receive a
live signal from the Rust server" is read as: the client attaches, subscribes a
signal, and receives a `publish` whose provenance is `live` and whose payload
bytes are the value the provider set.

- **Deno** (`e2e/deno_test.ts`). The test starts the example from the path in
  `RIDL_TRANSPORT_WS_SERVER` (`e2e/server.ts`), connects with the encoding
  `flatbuffers`, reads the `fixed`, subscribes the signal and receives its
  `init` publish, subscribes the event, sends the command with the argument
  `2a 07` and gets `accepted`, receives the `live` publish whose payload is
  `2a 07` and the occurrence with the same payload, sends the query and checks
  the reply, and closes with 1000 with no violation recorded. This is the
  sequence of the crate's scenario without its last step, which the signal check
  test covers, so the two clients are checked against one provider behaviour. A
  separate test,
  `a signal check
  refuses a live publish, keeps the last good bytes and recovers`,
  registers `SIGNAL_CHECKS` of `e2e/catalog.ts` in `checks`, which refuses one
  payload with the rule `range`. It sends a good command argument and sees a
  `live` sample with that `lastGood`, sends the refused argument and sees an
  `invalid` sample with `detected` `{ rule: "range" }` and the earlier
  `lastGood`, reads the signal and gets the same invalid sample, and sends a
  good argument and sees `live` again, with no `violation` or `check-failed`
  note. Between the `fixed` and the signal, the first test also reads the three
  large `fixed` values (ordinals 7 to 9 of `e2e/catalog.ts`, 65,280, 16,137 and
  16,138 bytes) and compares every byte with the pattern. A second test opens a
  raw socket without the package's `Session`, sends `attach` and waits for
  `attached`, reads the `fixed` of 16,137 bytes and gets one `publish`, reads
  the `fixed` of 16,138 bytes and gets four `block` frames; the crate's scenario
  has no such test. A third test,
  `queryStream_receives_64_elements_over_loopback_server`, sends the streamed
  query (ordinal 10 of `e2e/catalog.ts`, an argument `n` and a reply of `n`
  chunks of 1,024 bytes of the value `i`) with `n` 64, which is more than the
  initial credit of 4, so the reply arrives in full only if the credit the
  reader grants reaches the server. It checks the count and the bytes of each
  chunk, with a time limit of 5 s, and that no note was recorded. Four upload
  scenarios additionally assert the following, each with a 5 s limit and session
  and server cleanup:
  - `commandStream` is acknowledged before writing 200,000 bytes in pieces of
    16,128 bytes to key 1; `FETCH` must return exactly those bytes.
  - `queryWithUpload` consumes 200,000 bytes, then returns exactly 12 bytes:
    little-endian `u64` count 200,000 followed by the independently computed
    wrapping `u32` byte sum.
  - Refusing the third 1,024-byte command chunk with `InvalidValue(Range)` must
    reject a later write as `contract / invalid-value / range`, while `accepted`
    remains resolved.
  - A query upload refused with `InvalidValue(Utf8)` must reject both a later
    write and `reply` as `contract / invalid-value / utf8`.

  The refusal tests keep writing, at most sixteen chunks, without closing, so
  closing cannot hide a refusal of the last chunks. The upload harness permits
  discard notes but asserts no violation: after a refusal settles and forgets a
  query, its later response may be discarded as `unknown-correlation`. These
  four upload scenarios are written and type-checked. Two duplex scenarios also
  run against the loopback server:
  - `queryDuplex_echoes_64_chunks` requires the first echo while the upload
    remains open, checks 64 exact indexed echoes in total, closes the upload and
    requires normal reply end.
  - `queryDuplex_cancel_and_abort` checks reply cancellation and upload abort,
    then completes an ordinary query on the same session after each action.

  The four upload scenarios and both duplex scenarios require TCP execution to
  establish interoperability; that execution is not verified by the available
  local results.
- **The browser** (`e2e/browser_test.ts`, `e2e/page.ts`, `e2e/browser.ts`). The
  test bundles `e2e/page.ts` with `deno bundle --platform=browser`, serves the
  bundle and a page from `Deno.serve` on `127.0.0.1`, starts a headless
  Chromium-family browser with a temporary profile and a DevTools port, opens
  the page through the DevTools Protocol, and evaluates the promise the page
  exposes. The page runs the signal half of the sequence (attach, subscribe,
  command, `live` publish with `2a 07`), the signal check scenario of the Deno
  test, the large fixed values and the streamed reply of 64 elements. It then
  uploads 200,000 bytes to key 3 through `commandStream` and reads them back
  through `FETCH`. The test requires the page's `upload` result to be
  `"200000:true"` in addition to its earlier results. The upload extension is
  written and type-checked against DOM types; its execution in a browser is not
  verified by the available local results. The browser is `RIDL_BROWSER`, then
  `google-chrome`, `google-chrome-stable`, `chromium` and `chromium-browser` on
  `PATH`, then the Google Chrome application on macOS. On Linux,
  `RIDL_BROWSER_NO_SANDBOX=1` adds `--no-sandbox`.
- **The encoding.** The tests attach with `flatbuffers`, because the example
  serves `ridl-loopback`, whose encoding is FlatBuffers, and the server refuses
  any other tag with `encoding-unsupported` (the crate's D-11). The client's
  default stays `proto3`.
- **One server per test.** The provider's signal and event are one shared store,
  and a command changes the signal for every later connection, so each
  end-to-end test starts its own server process and stops it at its end; both
  signal scenarios can then expect `init` first, in any order.
- **Time limits.** A wait (`e2e/waiter.ts`) is registered with the listener
  before the action that produces its value, and its time limit counts from that
  action, so a slow server start does not use up the limit.

**D-15. The unit tests.** All under `packages/ridl-transport-ws/tests/`.

- **The vectors** (`vectors_test.ts`). Reads the four vector tables (binding
  versions 1, 2 and 3, and the reversed role) out of
  `docs/specification/frame-specification.md`, as the crate's `tests/vectors.rs`
  does, decodes each row to the frame its `Vector` cell describes, and encodes
  that frame back to the same bytes. It fails on an unknown `Id` and on a known
  `Id` that its table lacks, so a new vector cannot be skipped.
- **Every code** (`codes_test.ts`). Every code of §4 and §9.6, written as bytes
  by hand, in both directions; signedness of extreme integers, a message at a
  nonzero byte offset, and payloads as views.
- **The codec's failures** (`decode_errors_test.ts`). One test per violation and
  discard case of §11.1 item 4 that the codec decides, the precedence between
  two faults in one message, and every refusal of `attach`.
- **The session** (`session_test.ts`). The rules of D-8 to D-10 over a fake
  `WebSocket` (`fake_socket.ts`) that the test drives frame by frame, scripting
  only the orders that §6.2, §6.3 and the crate's D-8 and D-13 produce. They
  also cover D-24's checks, retained good bytes, read refusal, rechecking and
  check registration validation.
- **The calls** (`calls_test.ts`). The call rules of D-9, with Deno's op
  sanitizer turned on in every test, because Deno 2.9 leaves it off by default;
  a call's timer left running fails the test.
- **The streamed reply** (`calls_test.ts`, `session_test.ts`). The credit
  formula of D-20, the outcomes and categories, a refusing `response` before and
  after a chunk, the bound, the cancel, `unsupported` below version 3, and the
  offer of the three subprotocols. The vectors of binding version 3 are in
  `vectors_test.ts`.
- **The providing side** (`provision_test.ts`). `provide` over the fake
  `WebSocket`: the offer of the one subprotocol, the validation before any
  socket, each refusal reason of the `attach` and the exact hash, the answers to
  `subscribe`, `read` and `unsubscribe`, the initial and the kept `publish`, the
  per-channel `seq`, every refusal of `publish`, `invalidate` and `raise`, the
  requests (unknown member, over limit, the cap of 64, the kept responses, a
  wrong-kind, empty, throwing and rejecting handler, and a handler that never
  settles), each violation of D-21 through `onNote`, a clock that fails before
  the attach is answered or while a response is built, and the absence of a
  liveness timer. The vectors of the reversed role are in `vectors_test.ts`.
- **Uploads** (`calls_test.ts`, `session_test.ts`). D-23's one chunk per write,
  empty-chunk credit, credit waits, close and abort sequences, acknowledgment
  lifetime, request and close timers, refusals, early replies, invalid
  directions and preflight errors. Send-exception tests cover chunk, close,
  abort and over-limit interruption for all three methods, checking exact error
  identity, prompt result rejection and forgetting through a late frame's
  `unknown-correlation` note. Exact-limit writes and close or abort after an
  acknowledgment have separate tests. Stream-target tests vary interface,
  ordinal and kind for all three forms and assert no delivery or credit change.
- **The version** (`version_test.ts`). `deno.json`'s version equals the
  `[workspace.package]` version.

## Gates

**D-16. Four recipes.**

| Recipe           | Runs                                                                                                                                                                 | Needs                                          | In `build` |
| ---------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------- | ---------- |
| `ts-check`       | `deno fmt --check`, `deno lint`, `deno check mod.ts` for Deno and against the DOM library (`browser.deno.json`), `deno test` of `tests/`, `deno publish --dry-run`   | Deno                                           | yes        |
| `ts-e2e`         | Build the transport examples and run `e2e/deno_test.ts` and `e2e/provide_test.ts`; generate/build docstore and run its typed byte, named-element and duplex consumer | Deno, pinned Rust, pinned flatc, wasm32 target | yes        |
| `ts-browser-e2e` | `cargo build` of the example, then `e2e/browser_test.ts` against it                                                                                                  | Deno, the pinned Rust, a browser               | no         |
| `ts-npm-check`   | dnt build of the package into `npm/`, check of the built manifest and tarball, install of the tarball in a temporary directory, import by name from Node             | Deno, Node, npm, network access                | yes        |

The browser proof runs in CI only, so a contributor's `just build` needs no
browser; a browser-only regression is first seen in CI. `ts-browser-e2e` is
outside `build`, as `vscode-verify` is, so `gate-parity` does not cover its CI
step, which is checked by reading; the recipe's comment says so.
`ts-browser-e2e` looks the browser up before it builds anything and fails, with
`RIDL_BROWSER` named, when none is found. Each recipe fails with the command
that installs Deno when `deno` is missing; none skips (ADR-0009 decision 10).
`ts-check` needs no compilation and is a member of `pre-push`; `ts-e2e` and
`ts-npm-check` are not. `ts-npm-check` is a member of `build` and runs in the
`typescript` job, so a contributor's `just build` needs Node and npm, which
`bootstrap` does not install. The browser proof that runs no browser,
type-checking against the DOM library, is part of `ts-check`; it shows that the
package compiles for a browser, not that a page receives a signal.

In CI the four run in the `typescript` job, which installs the pinned Rust
toolchain, the wasm32 target, pinned `flatc`, `just`, and Deno from the release
zip of the pinned version. Node and npm come from the runner image. The job
reads the `rust` path filter, which runs for any path outside the skip list,
`packages/` included.

**The Deno pin.** The version is in `.deno-version` at the root, and the SHA-256
of its release zips is in `.deno-sha256`, which is edited with it. CI reads it.
`./bootstrap` downloads that release's zip from GitHub into `~/.deno/bin` when
no `deno` is on `PATH`, edits no shell profile (the contributor adds
`~/.deno/bin` to `PATH`), and prints a note, without replacing it, when another
version is installed, for the reason ADR-0009 decision 8 gives for not forcing a
global change. `just story-id-check` scans `packages/`.

## Records

**D-18.** This record and the package's README state the two limits of D-10 and
D-11 (and, for `provide`, D-21), the UTC default stamp of D-12 with the `clock`
option, and the size checks of D-19 with the `interfaces` option. The frame
specification's "As built" note and §11 and §11.1 name the package and its path.
ADR-0020 carries the dated amendment of decision 6 (D-4) and a note on open item
2 (D-1). The book has no chapter on the package: `npm-release.yml` publishes it
on a `v*` tag.

## What is not built

- Generated plain-call and provider faces over this package (driftsys/ridl#287).
  Generated stream methods are built and drive `Session` through `StreamCaller`
  without a transport import; the
  [codec table record](ts-codec-table.md#stream-functions) owns their supported
  forms. The cabin signal demo still joins its generated payload codecs to this
  package by hand.
- The upload method limits are recorded in D-23. The upload methods check no
  named-element payload or UTF-8 text boundary. Rule code 7 decodes as `"utf8"`
  and reaches callers in a refusal.
- An emulator (driftsys/ridl#290). The providing side offers no stream, and no
  generated face over it.
- Reading a catalog descriptor: the `kind` check, the response bound, the
  time-to-live discard of an occurrence, and compatible earlier hashes. The
  payload limits come from the generated tables instead (D-19).
- A close code on a protocol violation and a client liveness timer (D-10, D-11;
  driftsys/ridl#991).
- Publication to JSR (driftsys/ridl#992).
- A test of the source under Node; `ts-npm-check` tests the built package.

## Alternatives considered

| Alternative                                                                                 | Verdict  | Why                                                                                                                                                                                      |
| ------------------------------------------------------------------------------------------- | -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| the package under `editors/` or inside `crates/ridl-transport-ws/`                          | rejected | `editors/` holds editor integrations; a TypeScript package inside a Rust crate's directory would be packaged by cargo or need an exclusion, and the framework will hold several packages |
| Node first, an npm package with `package.json` and `tsc`                                    | rejected | the roadmap names Deno and the browser; a Node-first source toolchain adds `node_modules` and a lockfile to the gate; the source stays Deno-first and dnt generates the npm package      |
| Puppeteer, Playwright or Astral to drive the browser                                        | rejected | each fetches a dependency tree, and Playwright and Astral download a browser; the DevTools Protocol over Deno's `WebSocket` needs none, and CI's image already has Chrome                |
| the browser proof as type-checking and bundling only                                        | rejected | shows that the package compiles for a browser, not that a page receives a signal (the DOM type check is kept in `ts-check`)                                                              |
| the browser test in `build`, with a browser required on every contributor's machine         | rejected | a contributor's `just build` must not need a browser (D-16)                                                                                                                              |
| reuse the crate's cross-process scenario test as the server                                 | rejected | it serves one connection and runs only as an ignored test through `cargo test`; an example serves both end-to-end tests and is a plain binary                                            |
| a new Rust crate or an `xtask` subcommand as the test server                                | rejected | a new crate is a new publication question and scope; `xtask` does not depend on the loopback or the transport                                                                            |
| `stamp` and `seq` as `number`                                                               | rejected | not exact past 2^53 (D-6)                                                                                                                                                                |
| a pull `next()` for occurrences, as the port has                                            | rejected | JavaScript has no wake for a pull loop without polling; listeners and promises are the platform's form (D-8)                                                                             |
| a client liveness timer                                                                     | rejected | a browser page cannot see pings and would end healthy idle sessions (D-11)                                                                                                               |
| a private close code in 4000 to 4999 for a violation                                        | rejected | the binding defines no close code of its own (§11.1 item 4)                                                                                                                              |
| a minimal TypeScript runtime package created now, for the dependency of ADR-0020 decision 6 | rejected | that package holds the codec loader and has its own design; creating its types here would fix its API before that design                                                                 |
| `jsr:@std/assert` in the tests                                                              | rejected | a registry fetch and a lockfile for what `node:assert/strict`, built into Deno, already gives                                                                                            |
| an invalid call bound thrown synchronously                                                  | rejected | an async method that sometimes throws is a trap; the returned promise rejects with a `RangeError`, as `connect` rejects (D-9)                                                            |
| a listener exception left to end the frame's handling                                       | rejected | it would skip the close on a violation or the loss tracking; it is passed to `reportError` instead (D-9)                                                                                 |
| `deno.land/install.sh` in `./bootstrap`                                                     | rejected | it edits the contributor's shell profile and fetches code from JSR; the release zip is the file CI downloads (D-16)                                                                      |
| caller-managed signal checks and retained good bytes                                        | rejected | Reads need the same invalid sample as subscriptions; the session retains good bytes while the caller supplies the check (D-24)                                                           |
