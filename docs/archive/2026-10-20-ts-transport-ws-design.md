# The TypeScript WebSocket transport package — design

- Satisfies: driftsys/ridl#982 (roadmap story E12.11)
- Depends on: driftsys/ridl#265 (the Rust crate `ridl-transport-ws`, merged as
  0c032554)
- Depended on by: driftsys/ridl#288 (observability server), driftsys/ridl#291
  (test-harness drivers)
- Binds: frame specification §3 to §9 and §11.1;
  [ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
  decisions 6 and 7 and its open item 2;
  [ADR-0009](../decisions/ADR-0009-toolchain-and-gate-parity.md) decisions 5, 6,
  10 and 12
- Status: proposed, 2026-10-20

## Intent

The Rust crate `ridl-transport-ws` is the providing side of the frame's
WebSocket binding (frame specification §11.1). Nothing consumes it outside a
Rust test. The first consumer named for it is a debug and test portal whose
client runs in a browser or in Deno, and the TypeScript framework's later
stories — the observability server and the test-harness drivers — build on that
client. This package is that client: the consuming side of §11.1, in TypeScript,
for Deno and for the browser.

It carries payload bytes and never reads them. A payload is a `Uint8Array` that
the package hands to its caller as it arrived, and that it sends as the caller
gave it. Decoding is the generated binding's, through the wasm codec of ADR-0020
decision 7, which is a separate story.

**Done when:** a Deno program and a browser page each receive a live signal from
the Rust server (`ridl_transport_ws::serve`). "Receive a live signal" is read
as: the client attaches, subscribes a signal, and receives a `publish` whose
provenance is `Live` and whose payload bytes are the value the provider set.

## Goals

1. A codec for the ten forms of §4, laid out exactly as §11.1 item 2 states,
   that reproduces every test vector of §11.1 byte for byte, read from the same
   table the Rust tests read.
2. A consuming session over one WebSocket: open, `attach`, `subscribe`,
   `unsubscribe`, `read`, `request` (command and query), and the end of the
   session, applying the consumer's rules of §5, §6, §7 and §9.6 that need no
   catalog and no payload decoding.
3. The same package, unchanged, runs in Deno and in a browser: it uses only web
   platform APIs (`WebSocket`, `Uint8Array`, `DataView`, `TextEncoder`,
   `TextDecoder`, `setTimeout`).
4. The done-when is proven by tests that run in `just build` and in CI, against
   the Rust server, in Deno and in a headless browser.

## Non-goals

- Decoding, verifying or encoding a payload, and the wasm codec (ADR-0020
  decision 7, roadmap story E12.1). The init value of a signal is not known to
  this package, so a sample under `Init` has no value, only its provenance and
  envelope.
- The TypeScript runtime package of ADR-0020 decision 6 (the port interfaces and
  the codec loader). It is story E12.1's.
- Generated faces over this package (story E12.1).
- The observability server (E12.2) and the test-harness drivers (E12.5).
- The providing side in TypeScript (an emulator, E12.4), and the stand-in ports
  of the Rust design record's "Adding the stand-in ports later".
- Any change to the Rust server's behaviour. A defect found in it is filed as an
  issue; if it blocks the done-when, the lane stops and reports it.
- Publishing the package to JSR or npm. The package is made publishable
  (`deno publish --dry-run` passes), and nothing is uploaded.
- TLS and authentication: `wss://` works because the platform's `WebSocket`
  supports it, and the package adds nothing.
- Reading a catalog descriptor: no `kind` check against a catalog, no response
  bound read from one, no compatible-hash list.

## Anti-goals

The package fails its purpose, even if its tests pass, when:

- it differs from §11.1 on the wire, so that a client written from the
  specification alone and this one behave differently against the same server;
- its codec is tested only against itself, so that a wrong constant cancels out
  in a round trip;
- it reaches for a Deno-only or Node-only API in the package's own code, so that
  the browser build needs a shim;
- it pulls a runtime dependency into a consumer's build;
- it hides a failure: a refused `attach`, a protocol violation, a lost
  occurrence or a call with no outcome is reported, never dropped silently.

## Constraints

- Plain, literal English in every file; no story identifier in shipped source or
  in `docs/design/` (`just story-id-check`).
- Every gate is a justfile recipe, a member of `build`, named in the
  `gate-parity` expected list, and run by `.github/workflows/ci.yml` with
  `run: just <recipe>` (ADR-0009 decisions 5 and 6).
- A recipe that needs a tool checks for it and fails with the command that
  installs it; it never skips (ADR-0009 decision 10). `./bootstrap` names every
  tool the gate requires (decision 12).
- The `wire` freeze dump and the §11.1 vectors are never edited by this work.
- The other lane running in parallel (the proto3 codec, driftsys/ridl#264) owns
  its own files; this lane touches none of them. Both lanes may edit shared
  files (`justfile` `build` and `gate-parity`, `ci.yml`, `bootstrap`,
  `.git-std.toml`); this lane rebases onto `main` before it pushes and resolves
  any conflict on its side (builder check, settled as the author's call).

## Decisions

**D-1. The package lives at `packages/ridl-transport-ws/`, and is named
`@driftsys/ridl-transport-ws`.** A new top-level `packages/` holds the
TypeScript packages of the framework, as `crates/` holds the Rust crates; the
directory name equals the package's bare name, as a crate's does. The package
has the same name as the crate because it is the same binding on the other side
of the connection. This settles the location half of ADR-0020 open item 2; the
runtime package's name stays open. _Cost if wrong:_ a directory move and a
rename before the first publication; nothing outside this repository refers to
either yet.

**D-2. Deno first, and web-standard code only.** The package is TypeScript
source with explicit `.ts` import specifiers, a `deno.json` and no build step,
which is what Deno and JSR consume directly. Its own code uses only APIs that
Deno and browsers both provide, so the browser runs the same source, bundled.
Node is not a target of this story: Node 22 has a global `WebSocket` client, so
the package probably runs there too, but nothing tests it and the README does
not claim it. The toolchain is Deno alone: `deno fmt`, `deno lint`,
`deno check`, `deno test`, `deno bundle`, `deno publish --dry-run`. No
`package.json` and no `node_modules`. The editor extension keeps its own npm
toolchain; the two do not share one. _Cost if wrong:_ a Node consumer later
needs an npm build (JSR generates one on publication) and a Node test job.

**D-3. Publication is to JSR, later, and not in this story.** `deno.json`
carries `name`, `version` and `exports`, so `deno publish --dry-run` checks the
package as JSR would; the gate runs the dry run. The version is the workspace
version (`Cargo.toml` `[workspace.package]`), and a test in the gate fails when
they differ, so the release procedure's bump has one more file to carry. A
follow-up issue records the publication workflow (the JSR scope, the token, the
order after the crate). _Cost if wrong:_ an npm-first consumer needs the npm
registry; JSR serves npm through its compatibility layer, so the cost is a
configuration change, not code.

**D-4. No dependency, at run time or in tests.** The package imports nothing
outside itself. The tests use Deno's built-in `node:assert/strict` and Deno's
own APIs, so nothing is fetched from a registry and there is no lockfile. The
browser test drives the browser through the Chrome DevTools Protocol over Deno's
own `WebSocket` client, in about a hundred lines, rather than through Puppeteer,
Playwright or Astral. ADR-0020 decision 6 names "the runtime package" as this
package's dependency; that package does not exist, and this story depends on the
Rust crate alone. The record is amended: the dependency on the runtime package
arrives with the runtime package, which then holds the vocabulary types
(`Envelope`, `Kind`, the error categories) that this package defines for now.
_Cost if wrong:_ when the runtime package lands, its types and this package's
must be reconciled, and that is a breaking change of this package's exported
types if done after its first publication. Doing it before publication costs one
move.

**D-5. Two layers, as in the crate: a sans-IO codec and a session.**

| Module           | Holds                                                                                                                    |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------ |
| `src/frame.ts`   | the ten forms as a discriminated union, `encode(frame): Uint8Array`, `decode(bytes): Frame` or a decode error, constants |
| `src/session.ts` | `connect(url, options)` and the `Session` class: the consumer's state machine over one `WebSocket`                       |
| `src/errors.ts`  | the error classes the session rejects with                                                                               |
| `mod.ts`         | the public exports                                                                                                       |

The codec encodes and decodes every form in both directions, as the crate's D-6
does, so a later providing side in TypeScript reuses it. A decode error is one
of two classes, as the crate's D-4: a **violation** (the session ends) or a
**discard** (a `publish` or a `response` that says it has no payload and has
bytes after its header; the frame is recorded and dropped). _Cost if wrong:_
low; the split mirrors the specification's two failure classes.

**D-6. The numeric types.** `interface`, `ordinal`, `kind` and the codes are
`number`; `stamp`, `seq` and `correlation` are `bigint`, because an `i64` or a
`u64` past 2^53 has no exact `number` form (the TypeScript backend makes the
same choice for 64-bit widths). The catalog hash is a 32-byte `Uint8Array`.
_Cost if wrong:_ a consumer that wants `number` converts; changing the type
later is a breaking change.

**D-7. A payload is handed out as a view, not a copy.** The session sets
`binaryType = "arraybuffer"`; each message arrives as its own `ArrayBuffer`,
which nothing else holds or reuses, so a payload is a `Uint8Array` view over the
bytes after the header and costs no copy. A payload the caller passes to a call
is copied once into the message buffer. _Cost if wrong:_ none on the platforms
in scope; a platform that reused message buffers would need a copy.

**D-8. The public API.** The shape, to be fixed in the plan's types:

```ts
const session = await connect("ws://127.0.0.1:9000", {
  catalog: { name: "veh.cabin", hash }, // hash: Uint8Array, 32 bytes
  encoding: "proto3", // or "flatbuffers" or "repr-c"; default "proto3"
  onPublish(target, sample) {}, // every accepted publish, solicited or not
  onOccurrence(occurrence) {}, // every accepted occurrence, in arrival order
  onNote(note) {}, // what the consumer records: violations, discards, losses
});

await session.subscribe({ kind: "signal", iface: 1, ord: 2 });
session.sample(1, 2); // the held sample of a signal: provenance, envelope, payload
const s = await session.read({ kind: "signal", iface: 1, ord: 2 });
const f = await session.read({ kind: "fixed", iface: 1, ord: 5 });
session.unsubscribe({ kind: "event", iface: 1, ord: 3 });
await session.command(1, 3, args, { bound: 500 }); // ms; resolves on `accepted`
const reply = await session.query(1, 4, args, { bound: 500 }); // Uint8Array
session.close(); // ends the session on purpose
const ended = await session.closed; // why the session ended
```

- `connect` resolves once `attached(accepted)` arrives and rejects with the
  refusal reason on `attached(refused)`, or on a connection whose server
  selected no subprotocol (§11.1 "The subprotocol"), or with a unit name that is
  empty or longer than 255 bytes of UTF-8 before anything is sent (§11.1 item
  2).
- A sample is
  `{ provenance: "init" | "live" | "invalid", envelope: { stamp,
  seq }, payload: Uint8Array | null }`.
  `payload` is present under `live` only. `invalid` is `Invalid(Declared)`: the
  package does not verify, so it never produces `Invalid(Detected)` (§9.6), and
  the last good value of §5.1 step 5 is kept by the caller, who sees each sample
  in `onPublish`.
- The calls reject with one error class carrying the `Contract` or `Transport`
  category of ridl §10: `contract` with its kind (and the `Rule` for
  `InvalidValue`), `corrupt`, `busy`, and, detected by the client, `undelivered`
  (a command's bound passed), `timeout` (a query's bound passed) and `down` (the
  session ended).
- Listeners rather than a pull `next()`: a JavaScript program waits on a
  callback or a promise, and the port's pull shape is the runtime package's to
  spell over this one. _Cost if wrong:_ the API is reshaped before its first
  publication; E12.2 and E12.5 are its first users and may ask for an async
  iterator, which can be added over the listeners without a break.

**D-9. The consumer's rules the package applies.** Those of §5 to §9 that need
no catalog and no payload decoding:

- **Signals (§5.1, §7).** One held sample per subscribed signal; a `publish`
  whose `seq` is not greater than the held sample's is discarded (recorded);
  `unsubscribe` drops the held sample; a `publish` of an interaction not
  subscribed and not being read is discarded and recorded, which covers frames
  in flight after an `unsubscribe` (§6.2).
- **Events (§5.2, §7).** Occurrences are delivered in arrival order; a gap in
  `seq` between two accepted occurrences of one channel is recorded as a loss
  and delivery continues. The time-to-live discard needs the catalog's `max` and
  is not done.
- **Calls (§5.3, §5.4, §7).** One caller counter per session from 1, used as the
  `request`'s `seq`; a `response` is matched on `correlation`; one whose
  correlation names no call in flight is discarded and recorded. Each call has a
  bound given by the caller in milliseconds (no descriptor holds it, and §11.1
  item 5 says every call has one), after which it rejects `undelivered` or
  `timeout` and is forgotten. The `request` `stamp` is the caller's clock, an
  option defaulting to `Date.now()` in microseconds (D-12).
- **Answers (§6.2, §6.3, §6.4).** `subscribe` and `read` carry no correlation,
  and their answers are identified by `(kind, interface, ordinal)`. The session
  keeps, per identity, a first-in first-out queue of the `subscribe` and `read`
  operations waiting: an `answer` settles the operation at the head of the
  queue; a `publish` settles the `read` operations at the head. The server
  answers in the order it receives (the crate's D-8 handles each message as soon
  as it is read), and §6.2 puts a signal's `answer` before its first `publish`,
  so this is exact for the server as built. The fake-socket tests script only
  orders that §6.2, §6.3 and the crate's D-8 and D-13 produce (builder check,
  settled as the author's call). A refused `read` rejects with
  `contract(UnknownInteraction)`.
- **The end of a session (§3).** On a close or an error, every waiting operation
  and every call in flight rejects with `down`, every later operation rejects
  with `down`, and `closed` resolves with the close code and reason. A refused
  `attach` resolves `closed` after the server's close 1000.

_Cost if wrong:_ a rule applied wrongly shows as a wrong sample or a missed loss
at the observability server, which is the first consumer that relies on them.
The unit tests cover each rule with a fake socket.

**D-10. A protocol violation closes without a code, because the platform cannot
send 1002.** §11.1 item 4 has the receiver close with 1002, and 1003 for a text
message. The browser's `WebSocket.close` and Deno's accept only 1000 and 3000 to
4999, and throw `InvalidAccessError` for 1002 (checked on Deno 2.9.5); §11.1
"The subprotocol" already allows for "a client that can choose a close code".
The binding defines no code of its own, so a private 4000-range code is not
used. On a violation, on a text message and on a missing subprotocol the session
calls `close()` with no code, which sends a close frame with no status (the
server sees 1005), records the violation through `onNote`, and ends with the
reason. A message larger than a limit is not checked: the platform sets the
limit, and the client cannot close with 1009. A follow-up issue asks the frame
specification to state the client's close for a platform that cannot choose a
code. _Cost if wrong:_ the server records a close with no status where it would
have recorded 1002; nothing else changes.

**D-11. No client liveness timer.** §11.1 item 1 has either end treat no message
of any kind within its liveness timeout as the end of the session. A browser's
`WebSocket` answers the server's pings and does not show them to the page, so on
an idle session the page sees no message while the connection is healthy, and a
timer would end healthy sessions. The package therefore ends a session on the
platform's `close` and `error` events only, and the server's liveness check
(D-22 of the crate) is the one that runs. A dead server is detected when the
platform's connection fails. The same follow-up issue as D-10 asks the
specification to state what a client that cannot see pings does. _Cost if
wrong:_ a page whose server host vanished without a TCP reset waits for the
operating system's timeout; every call still ends at its bound.

**D-12. The default clock is wall-clock UTC in microseconds.** The `request`
`stamp` is in the platform's synchronized time base, TAI (ridl §3.1). A browser
has `Date.now()`, which is UTC in milliseconds, and no TAI. The option
`clock:
() => bigint` lets a program supply a TAI clock; the default is
`Date.now()` times 1000, and the README says it differs from TAI by the leap
seconds. The server as built evaluates no timing rule on a request stamp. _Cost
if wrong:_ a future provider that enforces a call's bound on the request stamp
sees calls 37 s older than they are; the fix is the default, not the API.

**D-13. The test server is a cargo example in the crate.**
`crates/ridl-transport-ws/examples/loopback_server.rs` serves the crate's test
catalog (`test.ws`, the provider of `tests/support/provider.rs`, included by
path so the catalog is defined once) on `127.0.0.1` with an OS-chosen port it
prints as `port <n>`, one thread per connection, each with its own
`Scanning(loopback.attach())` backend, and the provider stepped on its own
thread. It exits when its standard input closes. `examples/` is added to the
crate's `exclude`, as `tests/` is, because it includes a file from `tests/` and
uses the dev-dependency `ridl-loopback`. The server's code is unchanged. _Cost
if wrong:_ the crate's package and `just compat-check` are checked to stay green
with the example present; if the exclusion breaks either, the example moves into
a test target instead.

**D-14. How the done-when is proven.**

- **Deno.** A Deno test starts the example binary, connects, attaches to
  `test.ws`, subscribes the signal and receives its `Init` publish, sends the
  command with the argument `2a 07`, receives `accepted`, and receives a
  `publish` under `Live` whose payload is `2a 07`. It also reads the `fixed`,
  subscribes the event and receives its occurrence, and sends the query and
  checks the reply — the Rust scenario's sequence, so the two clients are
  checked against one provider behaviour.
- **The browser.** The test bundles the package with
  `deno bundle
  --platform=browser`, serves the bundle and a page from
  `Deno.serve` on `127.0.0.1`, starts a headless Chromium-family browser with
  `--remote-debugging-port=0` and a temporary profile, opens the page through
  the Chrome DevTools Protocol, and evaluates a promise the page exposes. The
  page runs the signal half of the sequence (attach, subscribe, command, `Live`
  publish with `2a 07`) and resolves with what it received. The browser is found
  from `RIDL_BROWSER`, then `google-chrome`, `chromium`, `chromium-browser` on
  `PATH`, then the macOS application path. A spike on this machine (Chrome, Deno
  2.9.5) received a binary message from a Deno WebSocket server in 7.4 s of wall
  time end to end, including the browser's start.
- **One server per test.** The provider's signal and event are one shared
  `Loopback` store, and a command changes the signal for every later connection,
  so each end-to-end test starts its own server process and stops it at its end;
  both tests can then assert `Init` first, in any order (builder check, settled
  as the author's call).
- **Cost.** GitHub's `ubuntu-latest` image ships Google Chrome, so CI downloads
  no browser. The Deno test needs Deno and the pinned Rust and adds a few
  seconds plus the example's build to `just build`. The browser test runs in CI
  only (D-16), so no contributor needs a browser to pass `just build`.

A browser proof that runs no browser — type-checking the package against the DOM
library and bundling it — was considered and is kept as a second check, but it
cannot show that a page receives a signal, so it does not replace the browser.

**D-15. The codec is tested against the specification's table, and against
hand-written bytes.**

- `tests/vectors_test.ts` reads the §11.1 vector table out of
  `docs/specification/frame-specification.md`, as the crate's `tests/vectors.rs`
  does, decodes each row to the frame its `Vector` cell describes, and encodes
  it back to the same bytes. It fails on an unknown `Id`, so a new vector cannot
  be skipped.
- `tests/codes_test.ts`: every code of §4 and §9.6, written as bytes by hand, in
  both directions, so a wrong constant cannot cancel itself in a round trip.
- `tests/decode_errors_test.ts`: one test per violation and discard case of
  §11.1 item 4 that the codec decides.
- `tests/session_test.ts`: the rules of D-9 and D-10 over a fake `WebSocket`
  that the test drives frame by frame.
- `tests/version_test.ts`: `deno.json`'s version equals the workspace version.

**D-16. The gates.** Three recipes. Two are members of `build`, in the
`gate-parity` expected list, and run by CI; the third runs in CI only:

| Recipe           | Runs                                                                                                                                                       | Needs                            | In `build` |
| ---------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------- | ---------- |
| `ts-check`       | `deno fmt --check`, `deno lint`, `deno check` of the package for Deno and against the DOM library, `deno test` of the unit tests, `deno publish --dry-run` | Deno                             | yes        |
| `ts-e2e`         | `cargo build` of the example, then the Deno end-to-end test against it                                                                                     | Deno, the pinned Rust            | yes        |
| `ts-browser-e2e` | `cargo build` of the example, then the headless-browser test against it                                                                                    | Deno, the pinned Rust, a browser | no         |

Ruling (main session, 2026-10-20): the browser proof runs in CI only — a
contributor's `just build` must not need a browser — at the cost that a
browser-only regression is first seen in CI, not locally. `ts-browser-e2e` is
outside `build`, as `vscode-verify` is, so `gate-parity` does not cover it; the
CI step that runs it is checked by reading, and the recipe's comment says so.
AGENTS.md's command list and CONTRIBUTING's description of the gates state it.

`ts-check` is a static check and joins `pre-push`; `ts-e2e` and `ts-browser-e2e`
do not, as `compile` and `test` do not. In CI the three run in a new
`typescript` job that installs Deno at the pinned version, the pinned Rust
toolchain with the cache, and `just`. The job is path-filtered with the `rust`
flag, which already runs for any path outside the skip list, `packages/`
included.

**The Deno pin.** The version is in one file, `.deno-version` at the root
(`2.9.5`). The CI job reads it to download that release. `./bootstrap` installs
that release with Deno's official installer when `deno` is not on `PATH`, and
prints a note, without replacing it, when a different version is installed — the
reasoning of ADR-0009 decision 8 for not forcing a global downgrade.

`deno fmt` and `deno lint` are limited to `.ts` files in `deno.json`, so prim
stays the formatter of the Markdown and JSON (AGENTS.md "prim owns the
connective tissue"). `just story-id-check` gains `packages` as a scanned tree.
_Cost if wrong:_ see the ruling above.

**D-17. The commit scope is `ts-transport-ws`.** The crate already holds the
scope `ridl-transport-ws`, and `.git-std.toml` lists one scope per crate or
package. _Cost if wrong:_ one line in `.git-std.toml`.

**D-18. Records.** The design record and the package README state the two limits
of D-10 and D-11 (no close code on a violation, no client liveness timer), and
the README states D-12's UTC default and the `clock` option (main session
ruling, 2026-10-20). The as-built description goes to a new design record,
`docs/design/ridl-transport-ws-ts.md`, at gardening, beside the crate's. The
frame specification's §11.1 opening, which names "the TypeScript WebSocket
package of decision 7", is updated to name the package and its path. ADR-0020
gets a dated amendment for D-4 (decision 6's dependency row) and a note on open
item 2 (the location settled by D-1). The book gains no chapter in this story:
the package is not published and no generated code uses it yet.

## What is left to the builder

- The exact exported type names and their TSDoc, within D-8's shape.
- The internal structure of `session.ts`.
- The DevTools Protocol helper's structure, within D-4 and D-14.
- How the Deno test passes the example binary's path (an environment variable
  set by the recipe is expected).

## Alternatives considered

| Alternative                                                                                 | Verdict  | Why                                                                                                                                                                                      |
| ------------------------------------------------------------------------------------------- | -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| the package under `editors/` or inside `crates/ridl-transport-ws/`                          | rejected | `editors/` holds editor integrations; a TypeScript package inside a Rust crate's directory would be packaged by cargo or need an exclusion, and the framework will hold several packages |
| Node-first, an npm package with `package.json` and `tsc`                                    | rejected | the roadmap names Deno and the browser; a Node toolchain adds `node_modules` and a lockfile to the gate for no target in scope; JSR produces an npm build when one is wanted             |
| Puppeteer, Playwright or Astral to drive the browser                                        | rejected | each fetches a dependency tree, and Playwright and Astral download a browser; the DevTools Protocol over Deno's `WebSocket` needs none, and CI's image already has Chrome                |
| the browser proof as type-checking and bundling only                                        | rejected | shows that the package compiles for a browser, not that a page receives a signal (kept as part of `ts-check`)                                                                            |
| the browser test in `build`, with a browser required on every contributor's machine         | rejected | ruled by the main session: a contributor's `just build` must not need a browser (D-16)                                                                                                   |
| reuse the crate's `provider_process` test as the server                                     | rejected | it serves one connection and runs only as an ignored test through `cargo test`; an example serves the Deno and the browser test from one process and is a plain binary                   |
| a new Rust crate or an `xtask` subcommand as the test server                                | rejected | a new crate is a new publication question and scope; `xtask` does not depend on the loopback or the transport                                                                            |
| `stamp` and `seq` as `number`                                                               | rejected | not exact past 2^53 (D-6)                                                                                                                                                                |
| a pull `next()` for occurrences, as the port has                                            | rejected | JavaScript has no wake for a pull loop without polling; listeners and promises are the platform's form (D-8)                                                                             |
| a client liveness timer                                                                     | rejected | a browser page cannot see pings and would end healthy idle sessions (D-11)                                                                                                               |
| a private close code in 4000 to 4999 for a violation                                        | rejected | the binding defines no close code of its own (§11.1 item 4)                                                                                                                              |
| a minimal TypeScript runtime package created now, for the dependency of ADR-0020 decision 6 | rejected | that package is story E12.1's, with the codec loader; creating its types here would fix its API before its design                                                                        |
| `jsr:@std/assert` in the tests                                                              | rejected | a registry fetch and a lockfile for what `node:assert/strict`, built into Deno, already gives                                                                                            |
