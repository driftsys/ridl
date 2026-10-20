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
