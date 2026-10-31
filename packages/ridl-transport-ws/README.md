# @driftsys/ridl-transport-ws

The client side of the WebSocket binding of the ridl frame. The package opens a
WebSocket with the subprotocol `ridl-ws-v1`, sends and receives frames, and
carries each payload as bytes without reading them. The caller encodes and
decodes payloads.

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
