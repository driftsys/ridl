# `@driftsys/ridl-transport-ws` — the WebSocket binding of the frame in TypeScript

`@driftsys/ridl-transport-ws`, at `packages/ridl-transport-ws/`, is the
TypeScript side of the WebSocket binding that
[the frame specification](../specification/frame-specification.md) §11.1
defines. It is the **consuming side** of a frame session, for Deno and for the
browser: it opens one WebSocket offering the subprotocols `ridl-ws-v2` and
`ridl-ws-v1`, sends `attach`, `subscribe`, `unsubscribe`, `read` and `request`,
and applies the consumer's rules that need no payload decoding. It carries
payload bytes without reading them: a payload is a `Uint8Array` that the package
hands to its caller as it arrived, and sends as the caller gave it. The one
thing it checks of a payload is its length, against the member's limit from the
generated interface tables (D-19).

The Rust crate `ridl-transport-ws` is the providing side of the same binding;
its as-built record is [`ridl-transport-ws.md`](ridl-transport-ws.md), which
this record follows in structure. Verifying a payload is the wasm codec's, of
[ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
decision 7, loaded by `@driftsys/ridl-runtime`, and decoding and encoding one is
the generated codec module's, `<base>.codec.ts`
([`ts-codec-table.md`](ts-codec-table.md)); the generated faces over this
package are not built. ADR-0020 decision 6 places the package; its dependency
row was amended on 2026-10-20 (D-4). The wire rules are the frame specification
§3 to §9 and §11.1 (binding versions 1 and 2).

This is the architecture as built. The code and its tests are the source of
truth; where this record and the code differ, this record is corrected. The
decisions keep their numbers D-1 to D-18 from the design they were taken in,
which is archived as
[`2026-10-20-ts-transport-ws-design.md`](../archive/2026-10-20-ts-transport-ws-design.md),
with its plan beside it.

## Scope

The first consumer is the debug and test portal that the crate's record names:
its client runs in a browser or in Deno and talks to a daemon that embeds the
crate's `serve`. The observability server (driftsys/ridl#288) and the
test-harness drivers (driftsys/ridl#291) are planned on top of this package.

**What the package does.**

- Encodes and decodes every one of the eleven forms of frame specification §4
  exactly as §11.1 item 2 and "Binding version 2" lay them out, and reproduces
  every test vector of §11.1 byte for byte, read from the same table the crate's
  tests read.
- Opens a consuming session over one `WebSocket`: `attach`, `subscribe`,
  `unsubscribe`, `read` (signal and `fixed`), `command` and `query`, and the end
  of the session.
- Runs unchanged in Deno and in a browser: the package's own code uses only web
  platform APIs.

**What it does not do.** It decodes, verifies and encodes no payload. It reads
no catalog descriptor: no `kind` check against a catalog, no response bound read
from one, no compatible-hash list; the one fact of the catalog it uses, each
member's payload limit, comes from the generated interface tables the caller
passes (D-19). It is not the providing side, and it has no TLS or authentication
of its own (`wss://` works because the platform's `WebSocket` supports it). It
is not yet published to JSR (D-3).

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
`BigInt` and `globalThis.reportError`, which Deno and browsers both provide; no
`Deno.*`. The source toolchain is Deno: `deno fmt`, `deno lint`, `deno check`,
`deno test`, `deno bundle` (in the browser test) and `deno publish --dry-run`.
The source tree has no `package.json`; the build generates an `npm/` directory,
which git ignores and which holds the `package.json` of the npm package. Node 22
or later is a tested target: Node 22 has a global `WebSocket`, and
`ts-npm-check` installs the built package in a temporary directory and imports
it by name from Node. The editor extension keeps its own npm toolchain.
`deno fmt` and `deno lint` cover `.ts` files only (`deno.json` `include`), so
prim stays the formatter of the Markdown and the JSON.

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

**D-4. No dependency, at run time or in tests.** The package imports nothing
outside itself. The tests use `node:assert/strict`, which Deno provides, and
Deno's own APIs, so nothing is fetched from a registry and there is no lockfile.
The browser test drives the browser through the Chrome DevTools Protocol over
Deno's own `WebSocket` client (`e2e/browser.ts`), not through Puppeteer,
Playwright or Astral. ADR-0020 decision 6 named "the runtime package" as this
package's dependency; that package does not exist, so the row was amended on
2026-10-20 to "nothing until the runtime package exists". The vocabulary types
(`Envelope`, `Kind`, `Contract`, the error categories) are defined here until
the runtime package holds them.

## The codec (`src/frame.ts`)

**D-5. Two layers, as in the crate: a sans-IO codec and a session.**

| Module              | Holds                                                                                                                             |
| ------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| `src/frame.ts`      | the eleven forms as the discriminated union `Frame`, `encode`, `decode`, `attach`, `AttachError`, `encodingTag` and the constants |
| `src/blocks.ts`     | the block size (`BLOCK_BYTES`) and the number of blocks of a payload                                                              |
| `src/reassembly.ts` | `Reassembly`: the one payload in progress of a session, and the sequence rules of its blocks (frame specification §5.5)           |
| `src/session.ts`    | `connect` and the `Session` class: the consumer's state machine over one `WebSocket`                                              |
| `src/calls.ts`      | the calls in flight of one session (`Calls`, not exported from `mod.ts`): the caller counter, the bounds and the correlation      |
| `src/errors.ts`     | the errors a session rejects with: `AttachRefused`, `ProtocolError`, `SessionError` and its `CallFailure`                         |
| `mod.ts`            | the public exports: everything exported by `frame.ts`, `errors.ts` and `session.ts`                                               |

`src/calls.ts` was split out of `session.ts` during implementation, to keep the
session file at a readable size.

The codec defines `FRAME_VERSION` (2), `SUBPROTOCOL` (`ridl-ws-v2`),
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
into the message, and the session keeps no reference to it.

## The session (`src/session.ts`, `src/calls.ts`)

**D-8. The public API.**

```ts
const session = await connect("ws://127.0.0.1:9000", {
  catalog: { name: "veh.cabin", hash }, // hash: Uint8Array, 32 bytes
  encoding: "flatbuffers", // or "proto3" or "podbuf"; the default is "proto3"
  interfaces: [Cabin, Horn], // the generated tables, for each member's limit (D-19)
  onPublish(target, sample) {}, // each accepted publish of a subscribed signal
  onOccurrence(occurrence) {}, // each accepted occurrence, in arrival order
  onNote(note) {}, // violations, discarded frames and losses
  clock: () => taiMicros(), // optional; the request stamp (D-12)
});
```

`ConnectOptions` also takes `webSocket`, a factory for the socket, which the
unit tests use to pass a fake one; the default is
`new WebSocket(url, protocols)`.

- `connect` validates the name, the encoding and the hash with `attach` before
  any socket is created, and rejects with that `AttachError` or `RangeError`. It
  then opens the connection offering `ridl-ws-v2` and then `ridl-ws-v1`, sends
  the `attach` with the frame version of the subprotocol the server selected (2
  or 1) once the connection is open, and resolves with a `Session` on
  `attached(accepted)`. It rejects with `AttachRefused` (carrying the `reason`
  and a `closed` promise) on `attached(refused)`; with `ProtocolError` when the
  server broke the binding before `attached`; and with `SessionError` `down`
  when the connection closed or failed first. A server that selected no
  subprotocol is seen differently on the two platforms: in Deno, `connect`
  rejects with `ProtocolError('no-subprotocol')`; in a browser, the platform
  fails the handshake itself, so `connect` rejects with `SessionError` `down`
  (close 1006). A URL the platform refuses rejects the promise; `connect` does
  not throw.
- `subscribe(target)` resolves on `answer(accepted)` and rejects with a
  `contract` `SessionError` on a refusing `answer`. `unsubscribe(target)` sends
  the frame and drops what the session held for the interaction. `read(target)`
  resolves with the sample of the next `publish` of that interaction.
  `sample(iface, ord)` returns the held sample of a subscribed signal. An
  interface number or an ordinal that is not an integer from 1 to 4294967295
  makes `subscribe`, `read`, `command` and `query` reject with a `RangeError`,
  and `unsubscribe` throw one, and nothing is sent.
- A `Sample` is
  `{ provenance: "init" | "live" | "invalid", envelope, payload, detected? }`,
  with `payload` a `Uint8Array` under `live` and `null` otherwise. `invalid`
  with no `detected` is `Invalid(Declared)`, as the provider published it;
  `invalid` with `detected: "corrupt"` is the session's own refusal of a `live`
  payload longer than the member's limit (D-19), the one `Invalid(Detected)` the
  package produces, because it verifies no payload (§9.6). The last good value
  of §5.1 step 5 is kept by the caller, who sees each sample in `onPublish`. An
  `Occurrence` carries `detected: "corrupt"` and an empty `payload` in the same
  case.
- `command(iface, ord, args, { bound })` resolves on `accepted`;
  `query(iface, ord, args, { bound })` resolves with the reply's payload, an
  empty `Uint8Array` when the reply has no bytes. Both reject with a
  `SessionError` whose `failure.category` is `contract` (with the `Contract`,
  and the `Rule` under `invalid-value`), `corrupt` or `busy` as the provider
  answered, or, detected by the client, `corrupt` (a reply, or the arguments
  before they are sent, longer than the member's limit, D-19), `undelivered` (a
  command's bound passed), `timeout` (a query's bound passed) or `down` (the
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
  and a `publish` settles every `read` at the head. This is exact for the server
  as built, which handles each message as soon as it is read (the crate's D-8),
  and §6.2 puts a signal's `answer` before its first `publish`. An `answer` with
  nothing waiting, or an accepted `answer` whose head is a `read`, is discarded
  and recorded (`unexpected-answer`). A refused `read` rejects with
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
timer would end healthy sessions. The session therefore ends on the platform's
`close` and `error` events only, and the server's liveness check (the crate's
D-22) is the one that runs. A server that is gone is detected when the
platform's connection fails; a server host that vanished without a TCP reset is
detected only at the operating system's timeout, and every call still ends at
its bound. `connect`, `subscribe` and `read` have no bound: each waits until its
answer arrives or the session ends. driftsys/ridl#991 also asks the
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
`tests/ts_ceilings.rs` reads `frame.ts` and compares them.

The check runs on receive, before a payload reaches the application, and
compares the payload's length with the limit: a payload of exactly the limit
passes. What is refused is reported as the frame specification §9.1 and §9.2
have a detected invalid value reported, so that nothing fails silently:

- a `reply` longer than the query's limit rejects the query with `SessionError`
  `corrupt`, the category the provider uses for a request it refuses for size;
  the limit is the one of the member the query named when it was sent, not one
  read from the response's `target`, which a `contract` echo may carry under
  another kind (§6.4);
- a `publish` that answers a `read`, of a signal or of a `fixed`, rejects the
  read with `corrupt`;
- a `publish` of a subscribed signal is delivered through `onPublish` as a
  sample with provenance `invalid`, `payload` `null` and `detected: "corrupt"`,
  with the envelope of the frame (§9.1), and becomes the held sample; the
  sequence rules of D-9 apply to it as to any publish, and the next `publish`
  within the limit returns the signal to `live`;
- an `occurrence` is delivered through `onOccurrence` with an empty payload and
  `detected: "corrupt"`, with the envelope of the frame (§9.2); it counts in the
  event's sequence, so no loss is recorded for it.

Nothing is sent back, and the session does not end. The one payload the client
sends, the arguments of a `request`, is checked before it is sent: `command` and
`query` reject with `corrupt` when the arguments are longer than the member's
limit, and send nothing. That is the answer the provider gives a `request` above
the limit (the crate's D-26), but only to arguments that one message carries; a
larger message is above the provider's message limit, which closes the session
with 1009 and rejects every call in flight with `down`. A `fixed` whose value is
longer than one message arrives in `block` frames of at most 4,096 bytes, which
the session reassembles.

**Blocks.** `src/reassembly.ts` holds the one payload in progress. The rules it
applies, and the answers to each sequence error, are frame specification §5.5;
this record does not restate them. What the session adds is the following. A
payload goes into one buffer of `total` bytes, allocated at the first block, and
is handled as a `live` `publish` that arrived whole after its last block. The
reads that the payload answers are the reads waiting for the identity when its
first block arrived; a `read` sent while the payload is being reassembled waits
for the next payload. The limit used at the first block is the member's limit
held to 65,280 bytes. A message whose form is 11 on a connection that selected
`ridl-ws-v1` ends the session, because that form is outside the table of frame
version 1.

## The end-to-end tests

**D-13. The test server is a cargo example in the crate.**
`crates/ridl-transport-ws/examples/loopback_server.rs` serves the crate's test
catalog (`test.ws`, the provider of `tests/support/provider.rs`, included by
path so the catalog is defined once) on `127.0.0.1:0`, prints `port <n>`, serves
each connection on its own thread with its own
`Described::new(Scanning(loopback.attach()), table)` backend, where `table` is
the hand-built member table of the test catalog, steps the provider every 5 ms
on its own thread, and exits with status 0 when its standard input ends. It
declares `required-features = ["server"]`, and the crate's `exclude` leaves out
`examples/`, as it leaves out `tests/`, because the example includes a file from
`tests/` and uses the dev-dependency `ridl-loopback`. `serve` is unchanged; the
example only wraps its backend in a member table. The crate's record lists the
example under its tests.

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
  sequence of the crate's scenario, so the two clients are checked against one
  provider behaviour.
- **The browser** (`e2e/browser_test.ts`, `e2e/page.ts`, `e2e/browser.ts`). The
  test bundles `e2e/page.ts` with `deno bundle --platform=browser`, serves the
  bundle and a page from `Deno.serve` on `127.0.0.1`, starts a headless
  Chromium-family browser with a temporary profile and a DevTools port, opens
  the page through the DevTools Protocol, and evaluates the promise the page
  exposes. The page runs the signal half of the sequence (attach, subscribe,
  command, `live` publish with `2a 07`) and resolves with what it received. The
  browser is `RIDL_BROWSER`, then `google-chrome`, `google-chrome-stable`,
  `chromium` and `chromium-browser` on `PATH`, then the Google Chrome
  application on macOS. On Linux, `RIDL_BROWSER_NO_SANDBOX=1` adds
  `--no-sandbox`.
- **The encoding.** The tests attach with `flatbuffers`, because the example
  serves `ridl-loopback`, whose encoding is FlatBuffers, and the server refuses
  any other tag with `encoding-unsupported` (the crate's D-11). The client's
  default stays `proto3`.
- **One server per test.** The provider's signal and event are one shared store,
  and a command changes the signal for every later connection, so each
  end-to-end test starts its own server process and stops it at its end; both
  tests can then expect `init` first, in any order.
- **Time limits.** A wait (`e2e/waiter.ts`) is registered with the listener
  before the action that produces its value, and its time limit counts from that
  action, so a slow server start does not use up the limit.

**D-15. The unit tests.** All under `packages/ridl-transport-ws/tests/`.

- **The vectors** (`vectors_test.ts`). Reads the §11.1 vector table out of
  `docs/specification/frame-specification.md`, as the crate's `tests/vectors.rs`
  does, decodes each row to the frame its `Vector` cell describes, and encodes
  that frame back to the same bytes. It fails on an unknown `Id`, so a new
  vector cannot be skipped.
- **Every code** (`codes_test.ts`). Every code of §4 and §9.6, written as bytes
  by hand, in both directions; signedness of extreme integers, a message at a
  nonzero byte offset, and payloads as views.
- **The codec's failures** (`decode_errors_test.ts`). One test per violation and
  discard case of §11.1 item 4 that the codec decides, the precedence between
  two faults in one message, and every refusal of `attach`.
- **The session** (`session_test.ts`). The rules of D-8 to D-10 over a fake
  `WebSocket` (`fake_socket.ts`) that the test drives frame by frame, scripting
  only the orders that §6.2, §6.3 and the crate's D-8 and D-13 produce.
- **The calls** (`calls_test.ts`). The call rules of D-9, with Deno's op
  sanitizer turned on in every test, because Deno 2.9 leaves it off by default;
  a call's timer left running fails the test.
- **The version** (`version_test.ts`). `deno.json`'s version equals the
  `[workspace.package]` version.

## Gates

**D-16. Four recipes.**

| Recipe           | Runs                                                                                                                                                               | Needs                            | In `build` |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------- | ---------- |
| `ts-check`       | `deno fmt --check`, `deno lint`, `deno check mod.ts` for Deno and against the DOM library (`browser.deno.json`), `deno test` of `tests/`, `deno publish --dry-run` | Deno                             | yes        |
| `ts-e2e`         | `cargo build` of the example, then `e2e/deno_test.ts` against it                                                                                                   | Deno, the pinned Rust            | yes        |
| `ts-browser-e2e` | `cargo build` of the example, then `e2e/browser_test.ts` against it                                                                                                | Deno, the pinned Rust, a browser | no         |
| `ts-npm-check`   | dnt build of the package into `npm/`, check of the built manifest and tarball, install of the tarball in a temporary directory, import by name from Node           | Deno, Node, npm, network access  | yes        |

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
toolchain, `just`, and Deno from the release zip of the pinned version. The job
reads the `rust` path filter, which runs for any path outside the skip list,
`packages/` included.

**The Deno pin.** The version is in one file, `.deno-version` at the root. CI
reads it. `./bootstrap` downloads that release's zip from GitHub into
`~/.deno/bin` when no `deno` is on `PATH`, edits no shell profile (the
contributor adds `~/.deno/bin` to `PATH`), and prints a note, without replacing
it, when another version is installed, for the reason ADR-0009 decision 8 gives
for not forcing a global change. `just story-id-check` scans `packages/`.

## Records

**D-18.** This record and the package's README state the two limits of D-10 and
D-11, the UTC default stamp of D-12 with the `clock` option, and the size checks
of D-19 with the `interfaces` option. The frame specification's "As built" note
and §11 and §11.1 name the package and its path. ADR-0020 carries the dated
amendment of decision 6 (D-4) and a note on open item 2 (D-1). The book has no
chapter on the package: it is not published, and no generated code uses it yet.

## What is not built

- Generated faces over this package (driftsys/ridl#287). The wasm codec of
  ADR-0020 decision 7, the runtime package `@driftsys/ridl-runtime` that loads
  it, and the generated codec module that decodes and encodes a payload are
  built ([`ts-codec-table.md`](ts-codec-table.md)); the cabin demo joins them to
  this package by hand.
- The providing side in TypeScript, an emulator (driftsys/ridl#290), and the
  stand-in roles of the crate's "Adding the stand-in ports later".
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
