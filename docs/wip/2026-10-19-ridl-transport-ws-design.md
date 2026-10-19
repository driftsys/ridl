# `ridl-transport-ws` — design

Story: E11.9, driftsys/ridl#265. Closes the open half of driftsys/ridl#718.
Status: design, written 2026-10-19. Sebastien delegated every decision of this
story to the agent; each decision below marked **(agent's call)** is for
Sebastien's review, and none of them is approved until Sebastien approves it.

Satisfies: the E11.9 row of `docs/ROADMAP.md` — "a contract reaches a second
process over the transport, and E11.15's loopback runs the same tests with no
socket". Binds: the frame specification §4 to §9 and §11.1
(`docs/specification/frame-specification.md`, version 0.2.0, binding version 1);
ADR-0020 decisions 6 and 7; ADR-0018 decision 7; ADR-0021 (the `ridl-rt` 0.1
port contracts).

## 1. Intent

The frame specification §11.1 defines a WebSocket binding of the logical frame,
and nothing in this workspace speaks it. This story builds the Rust side of that
binding in a new crate, `crates/ridl-transport-ws`:

1. a codec for the §11.1 header layout, usable with no standard library, which a
   later Rust or wasm consumer can link alone;
2. a server that takes a connection the embedder accepted and a `ridl-rt` port
   implementation the embedder chose, and acts as the **providing side** of the
   frame session for that connection. It forwards payload bytes without decoding
   them, and it maps each client request's `seq` to the correlation its backend
   `Caller` returned.

The first consumer is an external debug and test portal: a small daemon embeds
the server, and a browser or Deno client (E12.11, driftsys/ridl#982) observes
signals, receives events and sends calls through it. The later stories E12.11,
E12.1, E12.2 and E12.5 build on this crate's wire behaviour, so the wire
behaviour is the part that must be right.

## 2. Goals, non-goals, anti-goals

**Goals.**

- G-1. Encode and decode every one of the ten forms of §4 exactly as §11.1 item
  2 lays them out, and reproduce every test vector of §11.1 byte for byte.
- G-2. Serve the observe-and-call ports — `SignalReader`, `EventSource`,
  `Caller` — plus `FixedReader` for a `read` of a `fixed`, over WebSocket.
- G-3. Detect changed signals with `ScannableSignals` when the backend has it,
  and by reading each subscribed signal when it does not.
- G-4. The roadmap's done-when: one scenario test, generic over the consumer
  ports, passes over a WebSocket connection to a server in a second process, and
  passes over `ridl-loopback` in process with no socket.
- G-5. Close driftsys/ridl#718: a test checks the `websocket` row of
  `ridl_ir::codegen::bindings::KNOWN` (header 20, envelope 16) against frames
  this crate writes.

**Non-goals.** The TypeScript client (E12.11), the wasm codec (E12.1), the
observability server (E12.2), the test-harness drivers (E12.5). The stand-in
ports `SignalWriter`, `EventSink` and `Handler` over WebSocket (section 9 says
how they are added later). A public Rust client. TLS and authentication. Reading
a catalog descriptor.

**Anti-goals.** Each of these makes the story a failure even if the tests pass:

- the crate binds a socket, spawns a thread or starts a task on its own;
- the crate pulls a dependency tree into an embedder's build that is not
  justified line by line in section 4;
- the server decodes, verifies or re-encodes a payload;
- the codec needs `std` or `alloc`;
- a wire behaviour differs from §11.1, so that a TypeScript client written from
  the specification alone does not interoperate with this server.

## 3. Constraints

- C-1. The embedding daemon runs on a target that copies every dependency into
  its own build tree: the dependency set is as small as possible, and each
  dependency is justified (section 4).
- C-2. No TLS and no authentication in the crate. The embedder chooses the bind
  address and the access policy. The crate binds nothing, by default or
  otherwise.
- C-3. `just wasm-check` and `just compat-check` stay green.
- C-4. Edition 2021 and `rust-version = "1.83"`, the same literals as `ridl-rt`
  and `ridl-loopback`, so the crate builds with the toolchain `ridl-rt`
  supports. **(agent's call)**: the embedder's toolchain is unknown, and the
  runtime crates already set this floor.
- C-5. The transport does not link `ridl-loopback` (roadmap, the 2026-09-20
  split). The loopback is a dev-dependency only.

## 4. Crate shape and dependencies

**D-1. One crate, two layers.** **(agent's call)**

| Module         | Needs                   | Feature           | Holds                                                         |
| -------------- | ----------------------- | ----------------- | ------------------------------------------------------------- |
| `frame`        | `core` only, no `alloc` | always            | the §11.1 header codec: `Frame`, `decode`, `encode`           |
| `ws` (private) | `std`                   | `server`, default | the server side of RFC 6455: handshake, message framing, ping |
| `server`       | `std`                   | `server`, default | the session state machine and `serve`                         |

`cargo add ridl-transport-ws --no-default-features` gives the codec alone, with
`ridl-rt` at its default features (`no_std`). The `server` feature turns on
`std` for this crate and nothing else; it does not turn on any `ridl-rt`
feature.

**D-2. No dependency but `ridl-rt`.** **(agent's call)** The WebSocket layer is
written in this crate: the server side of RFC 6455 only — parse one HTTP/1.1
upgrade request, answer it with `Sec-WebSocket-Accept` (SHA-1 and base64,
written here, about 150 lines with their test vectors), read masked client
frames, reassemble fragmented messages, answer pings, write unmasked frames, and
run the close handshake. That is roughly 600 lines, and it is the whole of what
the binding needs. The server never writes a masked frame, so it needs no random
numbers.

| Dependency                   | Kind   | Why                                                                                                                                |
| ---------------------------- | ------ | ---------------------------------------------------------------------------------------------------------------------------------- |
| `ridl-rt`                    | normal | the port traits and the vocabulary of frame specification §2; no feature turned on                                                 |
| `ridl-loopback`              | dev    | the backend of every server test, and the in-process half of the done-when                                                         |
| `tungstenite` 0.29, defaults | dev    | an independent WebSocket client, so the hand-written server side is tested against an implementation this repository did not write |
| `ridl-ir`                    | dev    | the `websocket` binding row, for driftsys/ridl#718's check (T-2)                                                                   |

(As built: `tungstenite` 0.29 with its default `handshake` feature, because the
client must run the handshake that T-4 checks; 0.29 is the newest release whose
`rust-version` is not above this crate's. 0.30 needs Rust 1.85.)

A dev-dependency does not reach an embedder's build tree. This changes ADR-0020
decision 6's row for `ridl-transport-ws`, whose "Depends on" column reads
"`ridl-rt` and a WebSocket crate": the implementation PR amends that row in
place, with a dated amendment note, to "`ridl-rt`".

## 5. The codec (`frame`)

**D-3. One borrowed `Frame` enum, with the payload rules in the types.**
**(agent's call)** A decoded frame borrows its payload and its unit name from
the message. The presence rules of §11.1 "Whether a frame carries a payload" are
made unrepresentable when broken:

- `Publish` carries a `state`: `Init`, `Live(payload)` or `InvalidDeclared`;
  only `Live` has a payload, and the provenance code is derived from the state.
- `Response` carries an `outcome`: `Accepted`, `Reply(payload)`,
  `Contract(Contract)`, `Corrupt` or `Busy`; only `Reply` has a payload.
- `Occurrence` and `Request` always carry a payload, which may be empty.
- `Attached` carries `Accepted` or `Refused(Reason)`; `Answer` carries
  `Accepted` or `Contract(Contract)`.

`Contract` is `ridl_rt::error::Contract`. A decoded `InvalidValue` carries the
`Rule` from `detail` and `type_name: None`, as §9.6 states; an encoded one
writes the rule code and drops the name. The common header fields are one
`Target { kind, iface, ord }` with `ridl-rt`'s `Kind`, `InterfaceNo` and
`Ordinal`; the envelope is `ridl-rt`'s `Envelope`.

**D-4. Decode reports two kinds of failure.** §11.1 item 4 treats them
differently, so the error says which:

- `DecodeError::Violation(_)`: the receiver records the frame and closes with
  1002. Every case item 4 lists: shorter than its header; a control-plane form
  other than `attach` longer than its header; a code outside its table; an
  outcome that is in the table but not allowed on that form (for example
  `refused` on a `response`); a nonzero `detail` where §11.1 requires 0;
  `interface` or `ordinal` 0; a `name_len` of 0 or one that does not match the
  message; a `name` that is not valid UTF-8.
- `DecodeError::Discard(_)`: a `publish` or a `response` whose header says it
  carries no payload and which has bytes after the header. The receiver records
  it and drops it; the session continues.

The ordering rules of item 4 (a message before `attach`, a second `attach`, a
first provider message that is not `attached`) are the session's, not the
codec's: `decode` reads one message without context.

**D-5. Encode writes into a caller's buffer.** `Frame::encoded_len()` and
`Frame::encode(&mut [u8]) -> Result<usize, Short>`. An `attach` whose name is
empty or longer than 255 bytes cannot be built: the constructor returns an
error, which is the "reports an error when the session is opened and sends
nothing" of §11.1 item 2.

**D-6. Every form in both directions.** The codec encodes and decodes all ten
forms, whichever side calls it. The server uses half of each, and the test
client uses the other half. Section 9 depends on this.

## 6. The server

**D-7. The embedder owns sockets and threads.** **(agent's call)** The public
entry point is one blocking function:

```rust,ignore
pub fn serve<C: Connection, P: Backend>(conn: C, ports: P, config: &Config) -> Ended;
```

The embedder binds a listener, applies its access policy to each accepted
connection, chooses a thread, builds one `Backend` value for the connection, and
calls `serve`. `serve` returns when the session ends, with why. (As built:
`serve` sets the write timeout to `liveness_timeout`, and a write that fails
with `WouldBlock` or `TimedOut` ends the session as `Ended::LivenessTimeout`,
because the client has stopped taking messages for the liveness period. The
method is required, not provided, so that no embedder's connection is silently
unbounded.) `Connection` is `Read + Write` plus `set_read_timeout`, a required
`set_write_timeout` and a provided `shutdown_write` (as built; the default of
`shutdown_write` does nothing), implemented here for `std::net::TcpStream` and,
on Unix, `std::os::unix::net::UnixStream`. Nothing in the crate calls `bind`,
`listen`, `spawn` or `connect`.

**D-8. The tick.** `serve` reads the socket with a read timeout of
`config.poll_interval` (default 20 ms). Each pass of its loop: feed the bytes
read to the WebSocket parser and handle every complete message; then poll the
backend once (signal changes, waiting occurrences, call outcomes); then send
what that produced; then ping if `ping_interval` has passed since the last ping.
The latency of a change is at most one `poll_interval` plus the write. A
runtime's `Wakeable` cannot wake a thread blocked in a socket read, so it is not
used. **(agent's call)**

**D-9. A sans-IO session core.** **(agent's call)** The frame-level logic is a
private `Session` type that takes decoded frames and the backend and returns
frames to send. It does no IO, so the session rules are tested without a socket.
`serve` is the IO around it. It stays private until an embedder needs to drive
it from its own event loop.

**D-10. The backend trait.** **(agent's call)**

```rust,ignore
pub trait Backend {
    type Ports: Attached + Encoded + Clock + SignalReader + EventSource + Caller;
    fn ports(&mut self) -> &mut Self::Ports;
    fn scannable(&self) -> Option<&dyn ScannableSignals> { None }
    fn read_fixed(&self, iface: InterfaceNo, ord: Ordinal, out: &mut [u8])
        -> Result<usize, ReadError> { Err(ReadError::Contract(Contract::UnknownInteraction)) }
}

pub struct Polling<P>(pub P);  // P: the six bounds; no scan, no fixed
pub struct Scanning<P>(pub P); // P: the six bounds + ScannableSignals + FixedReader
```

The ports are an associated type rather than supertraits so that an embedder, or
a test crate, can serve a runtime type it does not own: the orphan rule forbids
implementing this crate's trait on another crate's type, and the two adapters
cover the two common cases with no code. A backend with one optional capability
and not the other implements the three methods itself.

One value per connection: its ports have their own event queue (their
`EventSource`) and their own sequence counter (their `Caller`), which is what
§11.1 item 5 needs, because the caller's identity is the connection. For
`ridl-loopback`, the embedder passes `Scanning(loopback.attach())`. The crate
ships no loopback-specific code, because it does not link the loopback (C-5).

**D-11. Attach.** The first message must be an `attach`; anything else is a
protocol violation (close 1002). The server answers:

- `frame_version` not 1: `refused(frame_version_unsupported)`;
- the unit name differs from `Attached::catalog().name`:
  `refused(unknown_catalog)`;
- the name matches and the hash differs: `refused(catalog_mismatch)`. This is
  the exact-match rule, because the server holds no descriptor, as the frame
  specification's as-built note says of the first WebSocket provider;
- the encoding tag is not the tag of `<P::Encoding as Encoding>::NAME` (from the
  table of §3: `flatbuffers` 1, `proto3` 2, `repr-c` 3):
  `refused(encoding_unsupported)`. A backend whose encoding is not in that table
  refuses every `attach` with this reason;
- otherwise `attached(accepted)`.

After a refusal the server closes with 1000. The checks run in the order above,
so one reason is given. **(agent's call)** on the order.

**D-12. The server routes on the frame's `kind`.** It holds no catalog
descriptor, so it cannot check a `kind` against the catalog (§4 "Why `kind` is
on the frame"); that check is the backend's when it has a member table. What it
does check is that the form admits the kind:

| Form          | `kind` admitted   | Any other `kind`                               |
| ------------- | ----------------- | ---------------------------------------------- |
| `subscribe`   | `Signal`,`Event`  | `answer` with `contract(UnknownInteraction)`   |
| `read`        | `Signal`,`Fixed`  | `answer` with `contract(UnknownInteraction)`   |
| `request`     | `Command`,`Query` | `response` with `contract(UnknownInteraction)` |
| `unsubscribe` | `Signal`,`Event`  | discarded and recorded                         |

A provider-to-consumer form (`attached`, `answer`, `publish`, `occurrence`,
`response`) received by the server is a protocol violation, close 1002.
**(agent's call)**: §11.1 item 4 names "a first provider-to-consumer message
that is not `attached`" for the consumer; the server applies the mirror rule,
because a client that sends one has the roles wrong.

**D-13. Signals.**

- `subscribe` of a signal: read it from the backend, answer `accepted`, then
  send one `publish` of its current state (§5.1, §6.2). A backend read that
  fails with `ReadError::Contract(c)` is answered `contract(c)` instead and
  nothing is subscribed. A second `subscribe` of a subscribed signal is answered
  `accepted` and sends nothing more ("changes nothing", §6.2).
- `read` of a signal: one `publish` of its current state; nothing is subscribed.
  A failing read is answered as above.
- `unsubscribe`: the server forgets the signal and sends nothing.
- The state sent is the backend's `RawSample`, mapped:

  | Backend provenance     | `publish` state   | Payload                   |
  | ---------------------- | ----------------- | ------------------------- |
  | `Init`                 | `Init`            | none                      |
  | `Live`                 | `Live`            | the bytes read            |
  | `Invalid(Declared)`    | `InvalidDeclared` | none                      |
  | `Invalid(Detected(_))` | `InvalidDeclared` | none — **(agent's call)** |

  The `stamp` is the backend's, unchanged. The `seq` is the session's, by D-24:
  the server is the providing runtime of this session, and §3 and §7 make every
  channel counter per session, from 1. `Detected(_)` never crosses (§9.6). A
  backend that reports it is itself a consumer of some earlier binding that
  rejected a payload; sending its last good value as `Live` would hide the
  invalidity, so the server sends the invalidity and loses only the cause. A
  `ridl-rt` runtime does not verify payloads (§9), so `ridl-loopback` never
  reports `Detected`.
- A read that returns `ReadError::Short { needed }` is retried with a buffer of
  `needed` bytes, up to `config.max_message_bytes` minus the 27-byte `publish`
  header; a value larger than that is not sent, and is recorded. The signal
  stays subscribed and is tried again on its next change.

**D-14. Detecting a changed signal.** This is the constraint "use
`ScannableSignals` when the backend has it, and say what happens when it does
not".

- **With `ScannableSignals`** (`Backend::scannable` returns `Some`): the server
  keeps one `Watermark` per interface that has a subscribed signal, set to
  `generation(iface)` when the first signal of that interface is subscribed.
  Each tick it calls `scan` with those marks and a buffer of `Changed` entries,
  grows the buffer when `scan` returns 0 and a mark is still behind its
  generation (the loop the port documentation describes), and reads and sends
  each changed signal that is subscribed. Entries for signals that are not
  subscribed are ignored. (As built: every `Changed` entry of a subscribed
  signal is read and published, with no `seq` filter, because the mark set at
  subscribe excludes earlier commits; so a writer that restarts and reuses a
  `seq` is still published.) The cost of a tick with no change is one `scan`
  call.
- **Without it**: each tick the server reads every subscribed signal and sends a
  `publish` when the sample's backend `envelope.seq` or its provenance differs
  from the one it last saw for that signal (as built: it compares the backend
  `seq`, the provenance, the `stamp` and the value bytes with the last state
  sent, which costs one stored copy per subscribed signal; a republish of
  identical bytes under an identical `seq`, provenance and `stamp` is
  indistinguishable and harmless) ("differs", not "is greater": a backend's
  counter may restart or interleave, D-24). The port contract makes `seq` count
  every publication of a channel, an invalidation and a re-affirmation included
  (the conformance test
  `a_channel_sequence_number_counts_that_channel_publications`), so a changed
  `seq` is a change. The cost of a tick is one `read` per subscribed signal, and
  a backend with many subscribed signals and no `ScannableSignals` pays that on
  every tick.
- In both modes, publications between two ticks are coalesced into the newest,
  which a signal permits (a gap on a signal channel is not a loss, §7). Each
  `publish` the server sends for a channel carries a session `seq` greater than
  the one before it (D-24), so the consumer's ordering rule (§7) holds whatever
  the backend's counter does.

**D-15. Events.** `subscribe` of an event calls
`EventSource::subscribe(iface,
&[ord])`; `Ok` is answered `accepted`,
`Err(Contract(c))` is answered `contract(c)`. `unsubscribe` calls
`EventSource::unsubscribe`. Each tick the server drains `EventSource::next` and
sends one `occurrence` per occurrence, with the backend's `stamp` and the next
session `seq` of that event channel (D-24), so a gap the client sees is a gap on
this session. A `Short` read is retried with a larger buffer, as for signals.
The occurrence's trace context is dropped: the frame does not carry one (§2).
(As built: an occurrence larger than `max_message_bytes` is dropped and noted as
`ValueTooLarge`, and the session `seq` of its channel is advanced, so the client
sees a gap and records a loss (§5.2) instead of a silent drop.) Because the
backend subscribes before the `answer` is sent and nothing is drained until the
next tick, no occurrence reaches the client before its `answer`.

**D-16. Calls and the correlation map.**

- A `request` of a command calls `Caller::command`, of a query `Caller::query`,
  with the payload bytes and `trace: None`. The server records
  `backend Correlation → (client seq, target)`.
- A send that fails is answered at once: `SendError::Busy` is `busy`,
  `SendError::Contract(c)` is `contract(c)`, `SendError::TooLarge` is `corrupt`
  **(agent's call)**: arguments larger than the member's capacity are not a
  well-formed encoding of its type, the same reasoning ADR-0021 decision 5 gives
  for `ShortClaim`.
- Each tick the server polls `ack` for each command in flight and `reply` for
  each query in flight. A known outcome becomes one `response` with
  `correlation` = the client's `seq`, `seq` = the next value of the server's
  response counter for this session (from 1, §7), and `stamp` = `Clock::now()`
  of the backend at that tick. Then the server calls `forget`.

  | Backend outcome                                           | `response` outcome     |
  | --------------------------------------------------------- | ---------------------- |
  | `ack` → `Ok(())`                                          | `accepted`             |
  | `reply` → `Ok(len)`                                       | `reply` with the bytes |
  | `Err(CallError::Contract(c))`                             | `contract(c)`          |
  | `Err(CallError::Transport(Corrupt))`                      | `corrupt`              |
  | `Err(CallError::Transport(Busy))`                         | `busy`                 |
  | `Err(CallError::Transport(Timeout / Undelivered / Down))` | no frame; forgotten    |

  The last row follows §9.6: those three are the absence of a frame and are
  never sent. The client's own bound reports them.

  (As built: a settled reply larger than `max_message_bytes` is recorded and
  forgotten with no frame, so the client's own bound reports `Timeout`;
  `corrupt` would blame the arguments. The last 64 settled responses that the
  duplicate rule below keeps include the responses sent at once, such as a
  `busy` or a `contract`. A `request` with `seq` 0 is recorded and dropped.)

- **Abandoned calls.** **(agent's call)** A call whose outcome is still unknown
  `config.call_abandon_after` (default 60 s) after it was sent is forgotten on
  the backend, and nothing is sent: the client's own response bound reports
  `Undelivered` or `Timeout` (§9.6). Without this, a client that calls a member
  no handler serves holds backend call slots until it disconnects; on
  `ridl-loopback` the call table is shared by the whole runtime (16 slots), so
  sixteen such calls would make every caller of that runtime `Busy`.
- **Duplicates.** **(agent's call)** §11.1 item 5 says the binding never
  retransmits, so a duplicate `request` is a peer defect. The server still
  follows §7: a `request` whose `seq` is in flight is not sent to the backend
  again; one whose `seq` is among the last 64 settled responses of the session
  gets that response again; any other `seq` not greater than the highest seen is
  recorded and dropped. §11.1 item 6 asks a provider to keep a settled response
  until the request's bound has passed; with no descriptor the server does not
  know the bound, so it keeps a fixed number instead. This is a deviation,
  recorded in the as-built note of the frame specification, and tracked by a
  follow-up issue together with reading a descriptor.

**D-17. `fixed`.** A `read` of a `fixed` calls `Backend::read_fixed` and sends
one `publish` with `kind` `Fixed`, state `Live` with the value, `seq` 0, and
`stamp` = `Clock::now()`. An error `ReadError::Contract(c)` is answered
`contract(c)`. A `subscribe` of a `fixed` is answered
`contract(UnknownInteraction)` (D-12).

**D-18. The session's end.** In-flight backend calls are forgotten and every
backend subscription is dropped with the `Backend` value. A backend port that
reports `Detached` from any call ends the session with close 1001, because the
provider is going away. `serve` returns `Ended` with one of: the client closed
(with its code), the server closed (with its code and the reason recorded), the
liveness timeout passed, or an IO error.

(As built: the embedder may also receive `Note` values, the records of what the
session dropped or closed on: `Violation`, `Discarded`, `MessageBeforeAttach` (a
message other than `attach` first; closes 1002), `SecondAttach` (closes 1002),
`UnexpectedDirection` (closes 1002), `KindNotAdmitted` (an `unsubscribe` of a
kind the form does not admit; dropped), `ValueTooLarge`, `DuplicateDropped` and
`CallAbandoned`.)

**D-24. Sequence numbers are the session's.** **(agent's call)** §3 and §7 make
each channel counter per session, starting at 1, and the backend's counters are
not: on `ridl-loopback` they live on each writer and sink handle, so two writers
of one channel interleave and a replaced writer starts again at 1. Forwarding
them would break the consumer's ordering rule (§7): after a restart, every later
`publish` would be discarded as stale. So the server keeps, per signal and per
event channel of the session, a counter from 1, and stamps each `publish` and
`occurrence` it sends with the next value; `Init` keeps `seq` 0. The backend's
`stamp` is kept unchanged, because the stamp is the sender's (ridl §3.1) and
every timing rule of §8 is evaluated on it. A `publish` answering a `read` of a
signal whose backend `seq` and provenance have not changed since the server last
sent that channel reuses the session `seq` it sent then; otherwise it takes the
next one. §13's relay rule is for a relay that forwards frames between two
sessions; this server forwards samples from ports, which carry no session
counter.

**D-25. A reply to a form that does not admit the kind echoes the identity.**
**(agent's call)** The `answer` or `response` that D-12 sends carries the
`kind`, `interface` and `ordinal` of the frame it answers, as received, the way
§6.3 has a refused `read`'s `answer` carry the `read`'s identity. So a
`response` may carry a `kind` other than `Command` or `Query` when its outcome
is `contract(UnknownInteraction)`, and a decoder accepts that. The E12.11 client
is written to this rule.

## 7. The WebSocket layer (`ws`)

**D-19. The handshake.** One `GET` request with `Upgrade: websocket`,
`Connection: Upgrade`, `Sec-WebSocket-Version: 13` and a `Sec-WebSocket-Key`.
The request line and headers are limited to 8 KiB; above that, or on any
malformed request, the server answers `400 Bad Request` and closes. The path,
the query and other headers are ignored (§11.1 item 1: the binding places
nothing there; the embedder's access policy runs before `serve`).

**D-20. The subprotocol.** The server selects `ridl-ws-v1` when the client
offers it, in any position of `Sec-WebSocket-Protocol`. When the client does not
offer it, the server answers `400 Bad Request` and does not upgrade. **(agent's
call)**: §11.1 says that the client fails a connection on which the server
selected none; answering 400 makes the failure visible at once, also to a client
that does not check. The selection is a table from subprotocol name to session
role with one row, so section 9's later row is a table entry. (As built: a
single comparison with `ridl-ws-v1`, because a table of one row is speculative;
the reversed role replaces the comparison with the table when it is added.)

**D-21. Messages.** Binary messages only; a text message closes with 1003.
Fragmented messages are reassembled; a message whose total size passes
`config.max_message_bytes` closes with 1009 as soon as the length is known.
`max_message_bytes` is at least 294 (§11.1 "The sizes"); `Config` rejects a
smaller value. Default 1 MiB. **(agent's call)** Client frames that are not
masked close with 1002 (RFC 6455 §5.1). A ping is answered with a pong carrying
its payload.

**D-22. Liveness.** The server sends a ping every `config.ping_interval`
(default 5 s) and ends the session when no message of any kind has arrived for
`config.liveness_timeout` (default 15 s). **(agent's call)** on both defaults: a
browser answers a ping automatically, so 5 s pings detect a dead client in 15 s
at a cost of one small frame per 5 s.

**D-23. Close codes.** Exactly §11.1 item 4's: 1000 on purpose and after
`attached(refused)`, 1001 when the backend detaches, 1002 on a protocol
violation, 1003 on a text message, 1009 on an oversized message. The server
answers a client's close frame with a close frame and returns. (As built: after
every close frame it sends, the server shuts down its write side and lingers for
up to 100 ms, reading and discarding what the client still sends, so that the
close frame reaches the client instead of being lost to a reset of the
connection. A rejected handshake gets the same treatment, so the client sees the
`400`.)

## 8. Tests

**T-1. The vectors.** A test reads the §11.1 test-vector table out of
`docs/specification/frame-specification.md`, decodes each `Hex` cell, checks
that it decodes to the frame its `Vector` cell describes, and that encoding that
frame gives the same bytes. Reading the table from the specification, instead of
copying it, means the test cannot drift from it.

**T-2. driftsys/ridl#718.** For payload lengths 0, 1 and 255, a `response` under
`reply` with that payload encodes to exactly
`frame_header_max_bytes + envelope_bytes + len` bytes, read from the `websocket`
binding row. `ridl_ir::codegen::bindings` is a private module, so the test reads
the row through `ridl-ir`'s public API: the deployment section of a lowered
system, which carries the bindings. **(agent's call)**: when that route costs
more than about 40 lines of test setup, the implementer adds a public
`ridl_ir::codegen::known_bindings()` returning the rendered rows instead, an
additive item; every other data-plane form encodes to no more; an `attach` with
a 255-byte name is 294 bytes. The test names the binding version it checks
against the row's `version`.

**T-3. The codec's failures.** One test per case of D-4, each asserting
`Violation` or `Discard`.

**T-4. The WebSocket layer**, against `tungstenite` as the client, with its
default `handshake` feature on, so that the handshake and `Sec-WebSocket-Accept`
are checked by an implementation this repository did not write (the RFC 6455
sample `dGhlIHNhbXBsZSBub25jZQ==` gives `s3pPLMBiTxaQ9kYGzzhZRbK+xOo=`). The
dev-dependency's tree size does not reach an embedder. The version is the newest
that resolves under `--locked` with the pinned toolchain; tests do not run at
1.83 (C-4 is checked by building the packaged crate, which drops
dev-dependencies). Covered: the handshake and `Sec-WebSocket-Accept` (the RFC
6455 sample key), the subprotocol rule of D-20, fragmentation, a text message
(1003), an unmasked frame (1002), an oversized message (1009), ping and pong,
the liveness timeout, the close handshake. SHA-1 and base64 against their
published test vectors.

**T-5. The session rules**, on the sans-IO core over `ridl-loopback`, one test
per rule of D-11 to D-18, including both modes of D-14: the loopback with
`scannable` returning `Some`, and the same loopback wrapped so it returns
`None`.

**T-6. The done-when.** A scenario function, generic over a rig that gives
consumer ports (`SignalReader + EventSource + Caller + FixedReader`) and a way
to let time pass, runs this sequence over a test catalog of one interface with a
signal, an event, a command, a query and a `fixed`:

1. read the `fixed`; it is the provisioned value;
2. read the signal; it is `Init` with no bytes;
3. subscribe the event;
4. send the command with argument `x`; its acknowledgment is `accepted`;
5. the provider, on accepting the command, sets the signal to `x`, commits, and
   raises the event with `x`; the consumer then reads the signal as `Live` `x`,
   and receives one occurrence `x`;
6. send the query with argument `y`; the reply is the provider's function of
   `y`.

It runs twice:

- **in process**, over `ridl-loopback`, with the provider's claim loop pumped
  between steps, and no socket;
- **across processes**: the test re-runs its own executable as a child process,
  with an environment variable that makes the child run the provider: a
  loopback, the same provider loop on a thread, and `serve` on one connection of
  a `TcpListener` bound to `127.0.0.1:0`, whose port the child prints. The
  parent connects with a test-only client in `tests/support/`, which implements
  the consumer ports over `tungstenite` and this crate's codec, and runs the
  scenario over it.

The test-only client is the consuming runtime of §5 written once for the test: a
held sample per subscribed signal, a queue of occurrences, a map of calls in
flight. It is not published. **(agent's call)**: "the same tests" is read as
this one scenario, not the `ridl-rt-conformance` suite, whose `Factory` requires
`SignalWriter`, `EventSink` and `Handler` on the consumer's value and cannot run
over a consumer-only port. The roadmap's "a contract" is met by a hand-written
test catalog, not by a generated face, because a generated face over a WebSocket
client needs a public Rust client, which this story does not ship. Running
`examples/cabin` over the transport is a follow-up.

## 9. Adding the stand-in ports later

The portal will later need to stand in for a component: publish signals, raise
events and answer calls into the system, through `SignalWriter`, `EventSink` and
`Handler`. On the frame, those are the **providing** side of a session, and
§11.1 item 1 makes the WebSocket client the consumer. So a stand-in client is a
session in the other direction: the WebSocket client provides, and the server
consumes and forwards what it receives into the backend's `SignalWriter`,
`EventSink` and `Handler`. The server sends `attach`, `subscribe` and `request`;
the client sends `attached`, `publish`, `occurrence` and `response`. All ten
forms exist already, so the frame does not change. What changes is the binding's
item 1, by a new subprotocol name that selects the reversed roles, offered
beside `ridl-ws-v1`, in an amendment of §11.1. **(agent's call)**: the name is
chosen with that amendment, not here.

What this story does so that the addition is local:

- the codec encodes and decodes every form in both directions (D-6);
- the subprotocol table maps a name to a session role (D-20), so the reversed
  role is one new row and one new session type;
- the `Backend` trait is the providing role's alone; the reversed role gets its
  own trait over `SignalWriter + EventSink + Handler`, so neither role's
  embedder implements ports it does not serve.

## 10. Documents the implementation changes

- `docs/specification/frame-specification.md`: the two as-built notes, §11 and
  §11.1's "its transport crate is not built yet" now name the crate, its
  exact-match rule (already stated) and the count-based retention of D-16.
- ADR-0020 decision 6: the dependency column of the `ridl-transport-ws` row
  (D-2), with an amendment note.
- `docs/ROADMAP.md`: E11.9's status note.
- `AGENTS.md`: the crate count and list; `.git-std.toml`: the scope;
  `docs/technotes/walking-skeleton-architecture.md`: the crate's row.
- `justfile` `compat-check`: the packaged crate is built at the `rust-version`
  of `crates/ridl-rt/Cargo.toml`, as `ridl-loopback` is, with its default
  features and with them off, which is what checks C-4. Path dev-dependencies
  carry no version and their tests are excluded from the package, as
  `ridl-loopback` does. The crate does not opt into the `cargo-semver-checks`
  lint metadata: `xtask/src/freeze/rust_api.rs` asserts that the crates opting
  in are exactly `RUNTIME_CRATES`.
- `justfile` `wasm-check`: the codec checked with `--no-default-features` for
  `wasm32-unknown-unknown` and for `thumbv7em-none-eabihf`, which proves C-3's
  "usable without std". The recipe's members do not change, so `gate-parity` is
  unaffected.
- A design record `docs/design/ridl-transport-ws.md`, gardened from this file
  when the work lands.

The crate is not added to the `runtime-crates` freeze surface in this story: it
has no release to compare against. Adding it after its first release is a
follow-up issue.

## 11. Alternatives considered

| Alternative                                                             | Verdict  | Why                                                                                                                                                                                                     |
| ----------------------------------------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `tungstenite` as the WebSocket layer                                    | rejected | about fifteen crates in the embedder's tree (`http`, `httparse`, `sha1`, `rand` and its tree, `thiserror`, `log`, `bytes`, `utf-8`, `data-encoding`), against C-1, for a server side of about 600 lines |
| `tokio` with `tokio-tungstenite`                                        | rejected | forces an async runtime on a small daemon, and a larger tree still                                                                                                                                      |
| a WebSocket trait the embedder implements                               | rejected | moves the RFC 6455 work to every embedder, and the crate could not test the binding end to end                                                                                                          |
| the crate binds and accepts itself (`Server::bind(addr)`)               | rejected | against C-2: the embedder decides the address and the access policy                                                                                                                                     |
| the codec in its own crate                                              | rejected | two crates to publish for one binding; a feature gives the same `no_std` build                                                                                                                          |
| one shared backend fanned out to every connection                       | rejected | one `EventSource` queue and one `Caller` counter would be shared by every client, so a caller's identity could not be the connection (§11.1 item 5); one `Backend` per connection gives that for free   |
| waking on `Wakeable` instead of a polling tick                          | rejected | a waker cannot interrupt a blocking socket read without a second thread or a platform poller, both of which grow the crate; the tick bounds the latency to `poll_interval`                              |
| `Invalid(Detected)` sent as `Live` with the last good value             | rejected | hides an invalidity from the client (D-13)                                                                                                                                                              |
| reading the catalog descriptor to check `kind` and bounds               | deferred | adds `ridl-descriptor` to the embedder's tree and a descriptor to its deployment; the specification's as-built note already states the first provider holds none                                        |
| a public Rust client in this story                                      | deferred | not needed by the first consumer, whose client is TypeScript (E12.11); the test client covers the done-when                                                                                             |
| accepting the upgrade without a subprotocol and letting the client fail | rejected | a client that does not check would send `attach` on an unnegotiated connection (D-20)                                                                                                                   |

## 12. Left to the builder

The exact names of `Ended`, `Config`'s fields beyond those named here, the
recording hook through which the server reports discarded frames and protocol
violations (a callback on `Config` is expected; it must cost nothing when
unset), the internal buffer management, and the module layout of the tests.
