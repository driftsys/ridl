# @driftsys/ridl-transport-ws

The client side of the WebSocket binding of the ridl frame. The package opens a
WebSocket that offers the subprotocols `ridl-ws-v3`, `ridl-ws-v2` and
`ridl-ws-v1`, in that order, sends and receives frames, and carries each payload
as bytes without reading them. The caller encodes and decodes payloads, or binds
a generated client class that does (see "Using a generated client"). `provide`
opens the reversed role, in which the program stands in for a component and the
server consumes (see "A providing session").

The session speaks the frame version of the subprotocol the server selects: 3 on
`ridl-ws-v3`, 2 on `ridl-ws-v2` and 1 on `ridl-ws-v1`, so the package also
attaches to a server that serves frame version 1 only. Frame version 2 is frame
version 1 plus the `block` form, which carries a `fixed` payload that does not
fit one message in parts (frame specification §11.1, binding version 2). Frame
version 3 is frame version 2 plus the forms `chunk`, `stream_end` and
`stream_credit`, which carry the reply stream of a query and the parameter
stream of a command or a query (binding version 3). On a session of frame
version 1, a `block` is a protocol violation.

The package runs on Deno and in a browser. It uses only the platform `WebSocket`
and depends on `@driftsys/ridl-runtime` for the shared types (`Envelope`,
`Contract`, `CallFailure`, the raw `Sample` and `Occurrence`) and for
`CallError`, which `SessionError` extends. The npm release workflow
(`.github/workflows/npm-release.yml`) publishes it as
`@driftsys/ridl-transport-ws` on a `v*` tag.

The wire format is in the
[frame specification](../../docs/specification/frame-specification.md), section
11.1.

## Using a generated client

A `Session` implements the consumer ports of `@driftsys/ridl-runtime`: `Caller`
(`command`, `query`), `SignalReader` (`subscribe`, `unsubscribe`, `sample`,
`read`, `watch`, `checkWith`), `EventSource` (`subscribe`, `unsubscribe`,
`listen`) and `StreamCaller` (the four stream methods), and each has
`catalogHash`, the hash the session attached with. The generated client class of
an interface binds to them with `<Interface>Client.bind(session, codec)`. The
client takes the signal checks, the catalog comparison, and the encoding,
verification and decoding of each payload on itself, so the program does not
pass `signalChecks(codec)` to `connect` and does not address a member by number.
For the cabin example:

```ts
import { loadCodec } from "@driftsys/ridl-runtime";
import { connect } from "@driftsys/ridl-transport-ws";
import {
  Cabin,
  CabinClient,
  CATALOG_HASH,
  CATALOG_NAME,
  PAYLOAD_TYPES,
  RULES,
} from "./generated/veh.cabin.codec.ts";
import { makeLevel, makeWindow } from "./generated/veh.cabin.ts";

const codec = await loadCodec(new URL("./generated/veh_cabin.wasm", import.meta.url), {
  catalogHash: CATALOG_HASH,
  payloadTypes: PAYLOAD_TYPES,
  ruleText: RULES,
});
const session = await connect("ws://127.0.0.1:9000", {
  catalog: { name: CATALOG_NAME, hash: CATALOG_HASH },
  encoding: "flatbuffers",
  interfaces: [Cabin],
});
const cabin = CabinClient.bind(session, codec);

await cabin.setLevel(makeLevel(42)); // the bound defaults to the declared 50 ms
const average = await cabin.average(makeWindow(10), { bound: 1000 });

await cabin.subscribeTemperature({ bound: 1000 });
const stop = cabin.watchTemperature((sample) => console.log(sample.value));
```

The ports address a member by interface number and ordinal. On a `Session`,
`subscribe`, `unsubscribe` and `read` take either a `Target` or `(iface, ord)`,
with the kind from the `interfaces` tables of `connect`; a pair that the tables
do not hold is a `RangeError`.

- `watch(iface, ord, listener)` calls `listener` with the `Sample` of every
  accepted publish of the signal, after the session updated the held sample and
  called `onPublish`, and returns the function that removes the listener.
- `listen(iface, listener)` calls `listener` with every accepted `Occurrence` of
  any subscribed event of the interface, after `onOccurrence`, and returns the
  function that removes the listener. `onPublish` and `onOccurrence` stay in the
  options of `connect` for a tool that wants every publish and occurrence.
- `command` and `query` take `signal` in their options. An already aborted
  signal rejects with its reason and sends nothing. An abort after the send
  rejects with the reason at once, and the session keeps the call pending until
  its response or its bound and consumes a late answer. A provider that received
  a command may still run it.
- A listener that throws is reported to the platform and does not stop the
  handling of the frame.

`SessionError` extends `CallError` of the runtime, so a program can test
`instanceof CallError` for any failed call, a client call included.

## Payload sizes

The catalog gives every interaction a payload limit: the default ceiling of its
kind (`SIGNAL_EVENT_CEILING` 3,840 bytes, `CALL_CEILING` 16,128 bytes,
`FIXED_CEILING` 65,280 bytes, exported by the package) unless a `[limits]` entry
of the manifest gives it another: the member's `[limits.interaction]` entry,
else the entry of its kind in `[limits.kind]`, else `default` clamped to the
default ceiling of its kind. The generated `<base>.codec.ts` module carries each
limit an entry gives in its interface tables, and `connect` takes those tables
in the `interfaces` option; without them, every member has the default ceiling
of its kind.

A received payload longer than its member's limit never reaches the application:

- a `reply` longer than the limit rejects the query with `SessionError`
  `corrupt`;
- a `publish` that answers a `read` resolves the read with the invalid sample of
  the next bullet;
- a `publish` of a subscribed signal is delivered as a sample with provenance
  `invalid`, `payload` `null`, `detected: "corrupt"` and `lastGood` holding the
  last bytes that passed, and becomes the held sample; the next `publish` within
  the limit that passes its check returns the signal to `live`;
- an `occurrence` is delivered with an empty payload and `detected: "corrupt"`,
  and counts in the event's sequence.

The arguments of a command or a query longer than the member's limit reject the
call with `SessionError` `corrupt` before anything is sent, the answer the
provider gives a `request` above the limit.

## Checking a signal

The package reads no payload, so it verifies none. The caller registers a check
for each signal it wants verified, and the session runs it on every `live`
payload of that signal that is within its limit, before the payload becomes the
held value:

- `connect` takes `checks`, a list of `{ iface, ord, check }` entries, in its
  options. `Session.checkWith(iface, ord, check)` registers or replaces one
  check later. An interface number or an ordinal outside 1 to 4,294,967,295
  makes `connect` reject, before it creates a socket, and `checkWith` throw,
  with `RangeError`. A check receives the payload bytes and returns `undefined`
  when they pass, else `"corrupt"` or `{ rule }`. The generated
  `<base>.codec.ts` module exports `signalChecks(codec)` when a signal has a
  table entry. It returns one entry per such signal for the `checks` option.
- A payload that fails its check is not delivered as a value. The sample has
  provenance `invalid`, `payload` `null` and `detected` set to the result of the
  check, and it becomes the held sample. `Sample.lastGood` holds the last bytes
  that passed since the last `subscribe`, and `null` when the init value
  applies. An `init` publish sets `lastGood` to `null`, and a publish that the
  provider declares invalid keeps the held `lastGood`.
- A `read` answered by a refused or over-limit publish resolves with the invalid
  sample. It does not reject.
- `checkWith` also checks a held `live` sample at once. When the check refuses
  it, the held sample becomes `invalid` with `detected`, `payload` `null` and
  `lastGood` `null`. Registering a check calls no `onPublish`.
- A check that throws counts as `"corrupt"`, and `onNote` receives
  `{ note: "check-failed", iface, ord, error }` with the thrown error.

A frame that a duplicate or stale sequence number drops is not checked unless it
answers a waiting read. `unsubscribe` drops the held sample and its `lastGood`.

## A large `fixed`

On a session of frame version 2, the provider sends the value of a `fixed` that
does not fit one message as `block` frames of 4,096 bytes each, except the last
one, which holds the rest of the value. The session reassembles them and settles
the consecutive live reads at the head of the queue when the first block arrived
with one sample, as for a value that arrived in one `publish`. A cancelled read
ahead in the queue first consumes one reply without settling the reads behind
it. A read sent after the first block, while the value is still arriving, waits
for the next value. The limit of a `fixed` is the one its interface table gives,
else `FIXED_CEILING`, and never more than 65,280 bytes: a table limit above
65,280 counts as 65,280, and one that is not a finite number counts as 0. Each
reassembled value goes into a new buffer of its own length, which the session
does not write to again.

A value of a `fixed` longer than its limit, whether it arrived in one `publish`
or in blocks, and on either frame version:

- resolves the consecutive live reads at the head of the queue with an invalid
  sample (`detected: "corrupt"`, `lastGood: null`, the envelope of the `publish`
  or of the first block), unless a cancelled read ahead first consumes the reply
  without settling the reads behind it;
- is recorded once, through `onNote`, as
  `{ note: "discarded", reason: "over-limit", frame }`, where `frame` is the
  `publish` or the first block, also when no read waits for it. In blocks, it is
  refused at the first block, before a byte is copied, and its other blocks are
  skipped;
- leaves the session open.

The blocks of one value come in order, from index 0, with no blocks of another
value between them. Each of these ends the session with the violation
`block-sequence`, and every waiting operation then rejects with `down`:

- a missing, a repeated or a reordered block;
- a block of index above 0 with no value in progress, or a block of index 0
  while a value is in progress;
- a block whose interface, ordinal, envelope or total length differs from the
  value in progress;
- a whole `publish` or an `answer` of the `fixed` whose blocks are in progress.

Frames of other interactions between two blocks are handled as usual, and so is
a `publish` or an `answer` with the same interface and ordinal under another
kind.

Against a provider that serves frame version 1 only (`ridl-ws-v1` selected), a
`fixed` larger than one message is not sent in blocks: the provider answers the
read with `contract(UnknownInteraction)`, so the read rejects with
`SessionError` `contract` `unknown-interaction`, the same error an unknown
member gives.

## A streamed reply

On a session of frame version 3,
`session.queryStream(iface, ord, args, {
bound })` sends a query whose reply is
a stream and returns a `ReadableStream<Uint8Array>` of the payloads of the
`chunk` frames, in order. The reader's pace sets the server's: the session
grants credit as the reader reads, and cancelling the stream stops the call. The
stream errors with a `SessionError` whose category is `contract`, `corrupt`,
`busy`, `interrupted`, `timeout`, `down` or, on a session of frame version 1 or
2, `unsupported`, in which case nothing is sent. The rules are in
[the package's design record](../../docs/design/ridl-transport-ws-ts.md), D-20.

## Uploading a parameter stream

On a session of frame version 3,
`session.commandStream(iface, ord, args, { bound })` returns
`{ upload, accepted }`, and
`session.queryWithUpload(iface, ord, args, { bound })` returns
`{ upload, reply }`. The upload is a `WritableStream<Uint8Array>` on stream 1:
each write sends one chunk, up to the member's payload limit, and waits while
its credit is spent. Closing the writer sends the end; it does not confirm that
the provider accepted the last chunks.

A command's `accepted` promise resolves on acknowledgment and leaves the upload
open; its bound runs from the request. A query's reply bound starts when the
upload closes. A plain reply received while the upload is open resolves `reply`
and errors the upload with `interrupted`. Aborting sends `interrupted`: it
leaves a command's acknowledgment waiting, and rejects a query's reply. A send
exception errors the upload, forgets the call and rejects its pending `accepted`
or `reply` promise with the same error. Frame versions 1 and 2 reject both
operations with `unsupported` and send nothing.

`session.queryDuplex(iface, ord, args, { bound })` returns `{ upload, stream }`:
a `WritableStream<Uint8Array>` for parameter stream 1 and a concurrent
`ReadableStream<Uint8Array>` for reply stream 0. Each direction has its own
credit. Upload close leaves the reply open. Cancelling the reply sends
`stream_end interrupted` on stream 0. Aborting the upload while it is open sends
`stream_end interrupted` on stream 1 and then on stream 0, and resolves once
both are sent. Both forget the call, error the other stream and reject writes
waiting for credit; refusals error both sides. The bound runs from the request
until the first reply chunk or end. See D-23 in
[the design record](../../docs/design/ridl-transport-ws-ts.md) for the full
contract, including the distinction between upload close and provider
consumption.

## A providing session

`provide(url, options)` is the reversed role of the binding: the program stands
in for a component, and the server consumes (frame specification §11.1,
"Reversed role"). It offers only the subprotocol `ridl-ws-v2-provider`, waits
for the server's `attach`, answers `attached`, and resolves with a `Provision`.
The `encoding` option is required, as there is no default: the debug gateway of
`ridl-transport-ws` serves `"flatbuffers"`. The `members` option lists what the
instance provides, as `MemberEntry` values (kind, interface, ordinal and an
optional payload limit).

```ts
const provision = await provide(
  `ws://127.0.0.1:${port}/alpha.First/Unit/alpha`,
  {
    catalog: { name: "alpha", hash },
    encoding: "flatbuffers",
    members: [
      { iface: 1, ord: 1, kind: "signal" },
      { iface: 1, ord: 3, kind: "command" },
    ],
    onRequest: () => ({ outcome: "accepted" }),
  },
);
// This becomes the initial value when the gateway subscribes.
provision.publish(1, 1, new Uint8Array([42]), BigInt(Date.now()) * 1000n);
```

The path of the URL is the gateway's: the binding gives it no meaning.

- The package accepts the server's `attach` only when the catalog name and the
  catalog hash equal the options exactly; it reads no list of compatible hashes.
  Otherwise it answers `attached(refused)`, closes with 1000 and rejects with
  `AttachRefused`.
- `publish`, `invalidate` and `raise` take the stamp as a `bigint` in
  microseconds. `publish` and `invalidate` update the current signal state
  before subscription, and the server receives that state after the
  subscription's `answer`. A `raise` without a subscription sends nothing and
  records `{ note: "discarded", reason: "not-subscribed", frame }` through
  `onNote`. It still advances the session's event channel sequence counter
  (frame §7). After resubscription, the next delivered occurrence can therefore
  show a gap, which the Rust consumer records as loss (§5.2). Each throws a
  `SessionError` `contract` `unknown-interaction` for an unlisted member,
  `corrupt` when the payload exceeds its limit, and `down` after the session
  ended.
- `onRequest` answers each command and query and may be asynchronous. The
  package answers a request for an unlisted member with
  `contract(UnknownInteraction)`, arguments over the limit with `corrupt`, and a
  request that arrives while 64 are in flight with `busy`. A handler that
  returns an outcome the call kind does not allow (`accepted` for a query,
  `reply` for a command), returns nothing, throws or rejects, gets **no
  response**: the frame has no outcome for it, so the package records
  `{ note: "discarded", reason: "unexpected-outcome" }` through `onNote` and the
  server's own bound ends the call. (The Rust provided session settles the same
  mistake of the other side as `Corrupt` at once.) A handler that never settles
  holds one of the 64 slots until the session ends.
- No stream form crosses the reversed role. `provide` has the limits below.

## Limits

- On a protocol violation, the client closes the socket without a close code,
  because the platform cannot send the codes 1002, 1003 or 1009. `provide` does
  the same and records the violation through `onNote`.
- The client has no liveness timer, because a page cannot see the pings of the
  server. `provide` has no timer either, and the server's liveness check is the
  one that runs.
- `connect` accepts optional `bound` and `signal` fields in its options;
  `subscribe(target, opts?)` and `read(target, opts?)` accept the same fields
  through `WaitOptions`. The bound is in milliseconds: a finite number greater
  than 0 and at most 2147483647. An invalid bound rejects with `RangeError`
  before creating a socket or sending a frame. When it passes, the operation
  rejects with `SessionError` category `timeout`. Connect's bound runs from the
  call until `attached(accepted)`. An aborted signal rejects with
  `signal.reason`; an already aborted signal creates no socket or sends no
  frame. With neither option, each waits until its answer or the session ends.
  Timers and abort listeners are removed when the operation settles.
- A timeout or abort of `connect` closes the socket and ignores late events. A
  cancelled subscribe or read keeps a placeholder until a reply consumes it or
  the session ends;
  [D-11](../../docs/design/ridl-transport-ws-ts.md#the-session-srcsessionts-srccallsts)
  explains why unanswered cancellations accumulate. A cancelled subscribe does
  not establish a subscription; after its late acceptance, a publish can still
  produce a `not-subscribed` note. If no subscription is established and no
  other live subscribe waits, cancellation sends `unsubscribe`. Existing held
  samples and event sequences are preserved.
- When the server selects no subprotocol, `connect` rejects with
  `ProtocolError('no-subprotocol')` in Deno. In a browser, the platform fails
  the handshake itself, so `connect` rejects with `SessionError` `down` (close
  1006).
- When the server refuses the `attach`, the client closes the connection with
  code 1000. `AttachRefused.closed` resolves on the close event.
- The default request stamp is UTC from `Date.now()`, not TAI. To use a TAI
  clock, pass it with the `clock` option. The clock returns microseconds as a
  `bigint`.
