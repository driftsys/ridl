# @driftsys/ridl-transport-ws

The client side of the WebSocket binding of the ridl frame. The package opens a
WebSocket that offers the subprotocols `ridl-ws-v2` and `ridl-ws-v1`, in that
order, sends and receives frames, and carries each payload as bytes without
reading them. The caller encodes and decodes payloads.

The session speaks the frame version of the subprotocol the server selects: 2 on
`ridl-ws-v2` and 1 on `ridl-ws-v1`, so the package also attaches to a server
that serves frame version 1 only. Frame version 2 is frame version 1 plus the
`block` form, which carries a `fixed` payload that does not fit one message in
parts (driftsys/ridl#1129). On a session of frame version 1, a `block` is a
protocol violation.

The package runs on Deno and in a browser. It uses only the platform
`WebSocket`, so it needs no other runtime. It is not yet published.

The wire format is in the
[frame specification](../../docs/specification/frame-specification.md), section
11.1.

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
- a `publish` that answers a `read` rejects the read with `corrupt`;
- a `publish` of a subscribed signal is delivered as a sample with provenance
  `invalid`, `payload` `null` and `detected: "corrupt"`, and becomes the held
  sample; the next `publish` within the limit returns the signal to `live`;
- an `occurrence` is delivered with an empty payload and `detected: "corrupt"`,
  and counts in the event's sequence.

The arguments of a command or a query longer than the member's limit reject the
call with `SessionError` `corrupt` before anything is sent, the answer the
provider gives a `request` above the limit.

## A large `fixed`

On a session of frame version 2, the provider sends the value of a `fixed` that
does not fit one message as `block` frames of 4,096 bytes each, except the last
one, which holds the rest of the value. The session reassembles them and settles
the reads that were waiting when the first block arrived with one sample, as for
a value that arrived in one `publish`. A read sent after the first block, while
the value is still arriving, waits for the next value. The limit of a `fixed` is
the one its interface table gives, else `FIXED_CEILING`, and never more than
65,280 bytes: a table limit above 65,280 counts as 65,280, and one that is not a
finite number counts as 0. Each reassembled value goes into a new buffer of its
own length, which the session does not write to again.

A value of a `fixed` longer than its limit, whether it arrived in one `publish`
or in blocks, and on either frame version:

- rejects every read waiting for it with `SessionError` `corrupt`;
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

## Limits

- On a protocol violation, the client closes the socket without a close code,
  because the platform cannot send the codes 1002, 1003 or 1009.
- The client has no liveness timer, because a page cannot see the pings of the
  server. `connect`, `subscribe` and `read` have no time limit: each waits until
  its answer arrives or the session ends.
- When the server selects no subprotocol, `connect` rejects with
  `ProtocolError('no-subprotocol')` in Deno. In a browser, the platform fails
  the handshake itself, so `connect` rejects with `SessionError` `down` (close
  1006).
- When the server refuses the `attach`, the client closes the connection with
  code 1000. `AttachRefused.closed` resolves on the close event.
- The default request stamp is UTC from `Date.now()`, not TAI. To use a TAI
  clock, pass it with the `clock` option. The clock returns microseconds as a
  `bigint`.
