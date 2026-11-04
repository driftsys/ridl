# `ridl-transport-ws` — the WebSocket binding of the frame

`ridl-transport-ws` is the Rust side of the WebSocket binding that
[the frame specification](../specification/frame-specification.md) §11.1
defines. It holds two things: a codec for the §11.1 header layout that needs
`core` only, and a server that acts as the **providing side** of a frame session
on one connection the embedder accepted, over a `ridl-rt` port set the embedder
chose. The server forwards payload bytes without decoding them, and it maps each
client request's `seq` to the correlation its backend `Caller` returned. The
server has a second role, the **provided session**, in which the WebSocket
client provides and the server consumes: the client's signals and events are
delivered into local ports, and the calls that the local ports receive are sent
to the client (D-28).

[ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
decision 6 names the crate and fixes `ridl-rt` as its only unconditional
dependency, with `ridl-descriptor` under the `descriptor` feature (amended
2026-10-19: the row said "`ridl-rt` and a WebSocket crate"; amended 2026-10-24:
the `descriptor` feature). ADR-0020 decision 7 and
[ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md) decision
7 place the transport;
[ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) fixes the port
contracts the server reads and calls. The wire rules are the frame specification
§4 to §9 and §11.1 (binding versions 1, 2 and 3).

This is the architecture as built. The code and its tests are the source of
truth; where this record and the code differ, this record is corrected. The
decisions below keep their numbers D-1 to D-29, because comments in
`crates/ridl-transport-ws/src/server/session.rs` and in the `justfile` cite
them. Read this record together with [the `ridl-rt` design record](ridl-rt.md),
whose "The ports" section states the contract the server consumes.

## Scope

The first consumer is an external debug and test portal: a small daemon embeds
the server, and a browser or Deno client (the TypeScript WebSocket package,
[`packages/ridl-transport-ws`](../../packages/ridl-transport-ws/README.md),
whose as-built record is [`ridl-transport-ws-ts.md`](ridl-transport-ws-ts.md))
observes signals, receives events and sends calls through it. The TypeScript
client, the wasm codec, the observability server and the test-harness drivers
build on this crate's wire behaviour, so the wire behaviour is the part that
must be right.

**What the crate does.**

- Encodes and decodes every one of the fourteen forms of frame specification §4
  exactly as §11.1 item 2, "Binding version 2" and "Binding version 3" lay them
  out, and reproduces every test vector of §11.1 byte for byte.
- Serves the observe-and-call ports — `SignalReader`, `EventSource`, `Caller` —
  plus `FixedReader` for a `read` of a `fixed`, over WebSocket, and, through
  `StreamCaller`, the reply stream of a query on a session of frame version 3
  (D-27).
- Detects changed signals with `ScannableSignals` when the backend has it, and
  by reading each subscribed signal when it does not.
- Builds the member table, which checks `kind` and gives the response bounds,
  from the catalog descriptor that `ridl build --emit catalog` writes, under the
  `descriptor` feature, off by default, so an embedder that passes no table
  links no planus (driftsys/ridl#985).
- Consumes a remote provider on the subprotocol `ridl-ws-v2-provider`: the
  client publishes the signals and raises the events of an interface that a
  local instance offers, and answers the calls sent to that instance. The crate
  delivers them into the instance's local ports (`SignalWriter`, `EventSink`,
  `Handler`) and forwards the calls (D-28, D-29, and D-16 for the calls).

**What it does not do.** It ships no public Rust client. It has no TLS and
authenticates no one. It reads a catalog descriptor only under the `descriptor`
feature, and does no I/O: the embedder reads or embeds the file.

**What it must never do.** Bind a socket, spawn a thread or start a task on its
own; pull a dependency into an embedder's build that is not justified in "The
crate and its dependencies"; decode, verify or re-encode a payload; need `std`
or `alloc` in the codec; or differ from §11.1 on the wire, so that a client
written from the specification alone does not interoperate with this server.

**Constraints.**

- The embedding daemon runs on a target that copies every dependency into its
  own build tree, so the dependency set is as small as possible.
- No TLS and no authentication in the crate. The embedder chooses the bind
  address and the access policy; the crate binds nothing, by default or
  otherwise.
- Edition 2021 and `rust-version = "1.88"`, the same literals as `ridl-rt` and
  `ridl-loopback`, so the crate builds with the toolchain `ridl-rt` supports,
  with every cargo feature. `just compat-check` checks this (see "Gates").
- The transport does not link `ridl-loopback`. The loopback is a dev-dependency
  only.

## The crate and its dependencies

**D-1. One crate, two layers.**

| Module                              | Needs                   | Feature           | Holds                                                                      |
| ----------------------------------- | ----------------------- | ----------------- | -------------------------------------------------------------------------- |
| `frame`                             | `core` only, no `alloc` | always            | the §11.1 header codec: `Frame`, `decode`, `encode`                        |
| `ws` (private)                      | `std`                   | `server`, default | the server side of RFC 6455: handshake, message framing, ping              |
| `server`                            | `std`                   | `server`, default | the session cores, the backend traits, `Config`, and the four entry points |
| `descriptor` (private, in `server`) | `std`                   | `descriptor`, off | `MemberTable::from_descriptor` over `ridl-descriptor`                      |

`cargo add ridl-transport-ws --no-default-features` gives the codec alone, with
`ridl-rt` at its default features (`no_std`). The `server` feature turns on
`std` for this crate and nothing else; it turns on no `ridl-rt` feature. The
crate root re-exports `serve`, `serve_selected`, `serve_provided`,
`serve_provided_selected`, `Backend`, `ProvidedBackend`, `Providing`, `Config`,
`Connection`, `Described`, `Ended`, `LimitOutOfRange`, `MemberTable`, `Note`,
`Polling`, `RequestedCatalog`, `RetainedTooSmall`, `Scanning`, `SelectionError`,
`Streaming` and `TableMismatch` under that feature, and `DescriptorError` under
`descriptor` (`crates/ridl-transport-ws/src/lib.rs`).

**D-2. No unconditional dependency but `ridl-rt`.** The WebSocket layer is
written in this crate, and only its server side: parse one HTTP/1.1 upgrade
request, answer it with `Sec-WebSocket-Accept` (SHA-1 and base64, in
`src/ws/sha1.rs` and `src/ws/base64.rs`, with their published test vectors),
read masked client frames, reassemble fragmented messages, answer pings, write
unmasked frames, and run the close handshake. That is the whole of what the
binding needs. The server never writes a masked frame, so it needs no random
numbers.

| Dependency         | Kind                   | Why                                                                                                                                                             |
| ------------------ | ---------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ridl-rt`          | normal                 | the port traits and the vocabulary of frame specification §2; no feature turned on                                                                              |
| `ridl-descriptor`  | optional, `descriptor` | the catalog descriptor's verifier and accessors, default features off: planus and serde (`alloc`, `derive`) only, no `ridl-ir`, no `serde_json`, no `hashbrown` |
| `ridl-loopback`    | dev                    | the backend of every server test, and the in-process half of the done-when test                                                                                 |
| `tungstenite` 0.29 | dev                    | an independent WebSocket client, so the hand-written server side is tested against an implementation this repository did not write                              |
| `ridl-ir`          | dev                    | the `websocket` binding row, for the binding sizes check                                                                                                        |

The workspace entry for `ridl-descriptor` turns its default features off,
because Cargo ignores `default-features = false` on an inherited entry; `ridl`
and `ridlc` name `std` and `dedup` themselves.

`tungstenite` keeps its default `handshake` feature, because the client must run
the handshake the tests check. 0.29 was the newest release whose `rust-version`
was not above this crate's minimum of 1.83 when it was chosen. A dev-dependency
does not reach an embedder's build tree.

The package leaves out `tests/` (`exclude` in `Cargo.toml`): `tests/vectors.rs`
reads the frame specification from the repository, and the path dev-dependencies
carry no version, as in `ridl-loopback`.

## The codec (`frame`)

The codec is `crates/ridl-transport-ws/src/frame.rs`. It also defines the
constants `FRAME_VERSION` (2), `SUBPROTOCOL` (`ridl-ws-v2`), `FRAME_VERSION_3`
(3), `SUBPROTOCOL_V3` (`ridl-ws-v3`), `INITIAL_CREDIT` (4, the chunks a sender
of a stream may send before the first `stream_credit`), `SUBPROTOCOL_V1`
(`ridl-ws-v1`, selected only to refuse an `attach`, D-20),
`SUBPROTOCOL_PROVIDER` (`ridl-ws-v2-provider`, the reversed role of D-20 and
D-28), `MIN_MESSAGE_LIMIT` (294, the largest `attach`), `MAX_MESSAGE_LIMIT`,
`ENVELOPE_BYTES` (16) and `BLOCK_BYTES` (4,096, the payload bytes of every
`block` but the last), and `encoding_tag`, which maps an encoding name to the
tag of §3: `flatbuffers` 1, `proto3` 2, `podbuf` 3.

**D-3. One borrowed `Frame` enum, with the payload rules in the types.** A
decoded frame borrows its payload and its unit name from the message. The
presence rules of §11.1 "Whether a frame carries a payload" cannot be broken in
a value:

- `Publish` carries a `PublishState`: `Init`, `Live(payload)` or
  `InvalidDeclared`; only `Live` has a payload, and the provenance code is
  derived from the state.
- `Response` carries a `ResponseOutcome`: `Accepted`, `Reply(payload)`,
  `Contract(Contract)`, `Corrupt` or `Busy`; only `Reply` has a payload.
- `Occurrence` and `Request` always carry a payload, which may be empty.
- `Attached` carries `Accepted` or `Refused(Reason)`; `Answer` carries
  `Accepted` or `Contract(Contract)`.

`Contract` is `ridl_rt::error::Contract`. A decoded `InvalidValue` carries the
`Rule` from `detail` and `type_name: None`, as §9.6 states; an encoded one
writes the rule code and drops the name. `Rule` is `#[non_exhaustive]`, and a
new variant comes only with a new frame version (§9.6), so `frame::rule_tag`
gives the code of a rule, or `None` for one this frame version has no code for;
the codec writes `detail` 0 for such a rule, which no receiver accepts, and does
not panic. The server checks the rule before it builds an outcome: a `response`
with such a violation is sent as `corrupt`, an `answer` as
`contract(ContractBroken)`, and `Note::RuleNotCarried` is recorded. No test
reaches these three paths: every `Rule` of the current `ridl-rt` has a tag, so
no test can build a violation the frame cannot carry without a seam in
`ridl-rt`. The gap is known and accepted until `ridl-rt` adds a variant. The
common header fields are one `Target { kind, iface, ord }` with `ridl-rt`'s
`Kind`, `InterfaceNo` and `Ordinal`; the envelope is `ridl-rt`'s `Envelope`.

**D-4. Decode reports two kinds of failure.** §11.1 item 4 treats them
differently, so the error says which:

- `DecodeError::Violation(_)`: the receiver records the frame and closes with
  1002. Every case item 4 lists: shorter than its header; a control-plane form
  other than `attach` longer than its header; a code outside its table; an
  outcome that is in the table but not allowed on that form (for example
  `refused` on a `response`); a nonzero `detail` where §11.1 requires 0;
  `interface` or `ordinal` 0; a `name_len` of 0 or one that does not match the
  message; a `name` that is not valid UTF-8; and a block that is wrong by
  itself, `ProtocolViolation::BadBlock` (the cases are in
  [frame specification](../specification/frame-specification.md) §11.1, "Binding
  version 2", "Framing"); and a stream form that is wrong by itself,
  `ProtocolViolation::BadStream` for a stream form whose `kind` is not `command`
  or `query` and for bytes after the header of a `stream_end` or a
  `stream_credit`, which are a violation and not a discard, because discarding
  an end or a credit would leave its stream stuck (§11.1, "Binding version 3",
  "Framing"). The other cases of a stream form that is wrong by itself keep the
  violation of the same case on every form: an outcome not allowed on a
  `stream_end` is `OutcomeNotAllowed`, a `detail` where the outcome has none is
  `NonzeroDetail`, and a `credit` of 0 is `ZeroNumber`. A `kind` outside the
  table stays `UnknownCode` and an `interface` or `ordinal` 0 stays
  `ZeroNumber`, as for every form.
- `DecodeError::Discard(_)`: a `publish` or a `response` whose header says it
  carries no payload and which has bytes after the header. A consumer records it
  and drops it, and its session continues; the server, which receives neither
  form, closes 1002 (D-12).

An `attach` whose encoding tag is outside §3's table (0, or 4 and above) is a
code outside its table, so it is `Violation(UnknownCode)`, as §11.1 item 2 reads
literally. A tag in the table that the backend does not serve is a session
matter, answered `refused(encoding_unsupported)` (D-11). A future fourth
encoding needs an ADR and a frame version anyway.

The codec does not know the version of the session. It decodes form 11 on any
session, and the session decides what a `block` means: the provider never
accepts one, so a `block` it receives closes the connection with 1002. The
public API has no function that splits a payload into blocks; the encoder writes
the fields of a `Frame::Block` as given, and the session's private `push_blocks`
builds valid blocks.

The ordering rules of item 4 (a message before `attach`, a second `attach`, a
first provider message that is not `attached`) are the session's, not the
codec's: `decode` reads one message without context.

**D-5. Encode writes into a caller's buffer.** `Frame::encoded_len()` and
`Frame::encode(&mut [u8])`, which returns the length written or
`Short { needed
}`. An `attach` whose name is empty or longer than 255 bytes, or
whose encoding tag is outside the table, cannot be built: `Attach::new` returns
an `AttachError` (`Empty`, `TooLong { len }` or `UnknownEncoding(tag)`). That is
the "reports an error when the session is opened and sends nothing" of §11.1
item 2.

**D-6. Every form in both directions.** The codec encodes and decodes all
fourteen forms, whichever side calls it. The server uses half of each, and the
test client uses the other half. The provided session (D-28) depends on this: it
sends `attach`, `subscribe` and `request` and receives `attached`, `answer`,
`publish`, `occurrence` and `response`.

## The server

The server is `crates/ridl-transport-ws/src/server.rs` and its modules
`backend.rs`, `config.rs`, `session.rs` and `calls.rs` for the providing role,
and `provided.rs`, `provided_backend.rs`, `provided/signals.rs` and
`provided/calls.rs` for the provided session (D-28).

**D-7. The embedder owns sockets and threads.** The public entry points are
blocking functions, `serve(conn, backend, &config) -> Ended` and
`serve_selected(conn, max_version, select, &config) -> Ended`. The embedder
binds a listener, applies its access policy to each accepted connection, chooses
a thread, and either builds one `Backend` value for `serve` or supplies a
`select` closure to `serve_selected`. Both return when the session ends, with
why. The crate rustdoc carries an embedding example. Nothing in the crate calls
`bind`, `listen`, `spawn` or `connect`.

`serve` takes a backend that the embedder built before the client sent anything.
A server that holds several catalogs cannot do that, because the catalog is
named in the client's `attach`.
`serve_selected(conn, max_version, select,
&config) -> Ended` is the second
entry point for that case. It takes no backend; it takes `select`, a closure
that is called at most once, when the first `attach` has been parsed, with a
`RequestedCatalog` that borrows the name and the hash from that message. The
closure returns a backend matching the requested name and hash when available,
or falls back to one with the requested name so the session can refuse an
incompatible hash (D-11). It returns `None` for an unknown name. Deployment
verification permits multiple catalogs with the same name and different hashes;
the example gateway prefers an exact match. Everything else in this section
applies to both functions: the embedder owns the socket and the thread, and the
backend is dropped on every exit.

The embedder states `max_version`, 2 or 3, in advance, because the opening
handshake picks the subprotocol before the `attach` arrives, so the server
cannot ask the selected backend what it serves. Any other value returns
`Ended::ConfigRefused` without reading the connection. Version 1 is still
negotiated for a client that offers only `ridl-ws-v1`, and is refused as `serve`
refuses it (D-20). After the closure returns a backend, the session applies the
ordinary checks of D-11 in their order. Only when they all pass does the server
check the two things that the advance statement could not know: whether the
backend serves the negotiated version, and whether the `Config` message limit
carries the backend's members (D-26). Either failure is answered with an
`attached` refusal and close 1000, and `serve_selected` returns
`Ended::ConfigRefused` (D-18). The Origin check of the `Config` runs before the
selection.

Two more entry points serve the provided session (D-28):
`serve_provided(conn, backend, &config) -> Ended` and
`serve_provided_selected(conn, select, &config) -> Ended`. They follow the same
rules: the embedder owns the socket and the thread, both block until the session
ends, and the backend is dropped on every exit. They take no `max_version`,
because the provided role has the one subprotocol.

`Connection` is `Read + Write` plus three methods: `set_read_timeout`,
`set_write_timeout`, and `shutdown_write`, whose default does nothing. It is
implemented for `std::net::TcpStream` and, on Unix,
`std::os::unix::net::UnixStream`. `set_write_timeout` is required, not provided,
so that no embedder's connection is silently unbounded: `serve` sets the write
timeout to `liveness_timeout`, and a write that fails with `WouldBlock` or
`TimedOut` ends the session as `Ended::LivenessTimeout`, because the client has
stopped taking messages for the liveness period. The timeout bounds each write
call, not a whole message, and the liveness clock does not run while the server
writes (D-22): a client that keeps reading, however slowly, is not ended. A zero
`poll_interval` or `liveness_timeout` is refused by the stream's setter, and
`serve` returns `Ended::Io(InvalidInput)`.

The crate sets no socket option. Each frame the server sends is written with one
write call (D-21), and the rustdoc of `Connection` says that an embedder may set
`TCP_NODELAY`.

**D-8. The tick.** `serve` reads the socket with a read timeout that ends when
the next poll is due: `config.poll_interval` (default 20 ms) after the last
poll, so a read after a message waits only for the rest of the interval. Each
pass of its loop feeds the bytes read to the WebSocket parser and handles every
complete message; then, when a `poll_interval` has passed since the last poll,
polls the backend once (signal changes, waiting occurrences, call outcomes) and
sends what that produced; then pings if `ping_interval` has passed since the
last ping. A message is handled as soon as it is read, and the backend is polled
once per `poll_interval`, whatever the client's message rate: a client that
sends often neither multiplies the backend's work nor delays a poll. The latency
of a change is at most one `poll_interval` plus the write. A runtime's
`Wakeable` cannot wake a thread blocked in a socket read, so it is not used.

The stream must be in blocking mode. On a stream in non-blocking mode a read
with no bytes waiting fails at once with `WouldBlock`, which `serve` takes as a
read timeout: during the handshake, the first such read ends the session with
`LivenessTimeout`; after the handshake, the loop spins without waiting, polling
at each `poll_interval`, until the liveness timeout ends the session. The
rustdoc of `Connection` and `serve` says so, and that a stream accepted from a
non-blocking listener may inherit that mode on some platforms.

**D-9. A sans-IO session core.** The frame-level logic is a private `Session`
type (`server/session.rs`) that takes decoded messages and the backend and
returns encoded frames to send. It does no IO, so the session rules are tested
without a socket. `serve` is the IO around it. `Session` stays private until an
embedder needs to drive it from its own event loop. After the session has
decided to close, it produces nothing more, whatever it is given.

**D-10. The backend trait.** `Backend` (`server/backend.rs`) has an associated
type `Ports: Attached + Encoded + Clock + SignalReader + EventSource + Caller`,
a method `ports(&mut self)`, and three provided methods: `scannable()`, which
returns `None`; `read_fixed(iface, ord, out)`, which answers
`ReadError::Contract(UnknownInteraction)`; and `members()`, which returns
`None`: the member table of the catalog, when the embedder supplied one
(`server/members.rs`). Two adapters cover the common cases with no code:
`Polling<P>` over the six bounds, with no scan and no `fixed`; and `Scanning<P>`
over the six bounds plus `ScannableSignals` and `FixedReader`. A third,
`Described<B>`, wraps any backend with an `Arc<MemberTable>` and forwards the
other three methods; `Described::new` compares the table's name and hash with
`ports().catalog()` and returns `TableMismatch` on a difference, so a wrong
table is found on the embedder's thread before `serve` runs.

`MemberTable` holds the unit name, the catalog hash, and a map from
`(InterfaceNo, Ordinal)` to the member's kind, its `max` bound and its payload
limit. `insert` sets an entry with no limit; `set_limit` sets the limit of an
existing entry and `limit` reads it. A retired interface and a reserved ordinal
have no entry (§6.4). The embedder builds it through `new` and `insert`, or,
with the `descriptor` feature, with `MemberTable::from_descriptor` from the
bytes of a `<unit>.catalog.binfb` file. `from_descriptor` verifies the buffer
with `ridl_descriptor::verify`, adds one entry for each member, with its
`max_us` as the bound and its `limit` as the payload limit (a member whose
`limit` is absent has the default ceiling of its kind as its limit), and adds
each hash of the descriptor's `compatible` list; it returns `DescriptorError`
for a buffer that fails verification, a hash that is not 32 bytes, an interface
number or an ordinal of 0, and a `max_us` that is not a decimal `u64`.

The ports are an associated type rather than supertraits so that an embedder, or
a test crate, can serve a runtime type it does not own: the orphan rule forbids
implementing this crate's trait on another crate's type. A backend with one
optional capability and not the other implements the three methods itself.

One value per connection: its ports have their own event queue (their
`EventSource`) and their own sequence counter (their `Caller`), which is what
§11.1 item 5 needs, because the caller's identity is the connection. For
`ridl-loopback`, the embedder passes `Scanning(loopback.attach())`. The crate
ships no loopback-specific code, because it does not link the loopback. The
value is dropped at the end of the session; the server does not unsubscribe the
backend's event subscriptions one by one, so a backend whose subscriptions
outlive its port value releases them in its own `Drop`.

### Attach and routing

**D-11. Attach.** The first message must be an `attach`; anything else, a
discarded frame included, is a protocol violation (close 1002). The server
answers:

- `frame_version` that differs from the version of the subprotocol the server
  selected (2 on `ridl-ws-v2`, 3 on `ridl-ws-v3`), or an `attach` on a
  `ridl-ws-v1` connection: `refused(frame_version_unsupported)` (D-20);
- the unit name differs from `Attached::catalog().name`:
  `refused(unknown_catalog)`;
- the name matches and the hash is neither the catalog's own nor, with a member
  table (D-10), one of the table's compatible hashes:
  `refused(catalog_mismatch)`. Without a table this is the exact-match rule of
  §6.1;
- the encoding tag is not the tag of the backend's encoding name (§3's table):
  `refused(encoding_unsupported)`. A backend whose encoding is not in that table
  refuses every `attach` with this reason;
- otherwise `attached(accepted)`.

Under `serve_selected`, two more checks follow the four above and precede the
acceptance, because the backend is known only after the `attach` is read. A
`select` that returns `None` is answered `refused(unknown_catalog)`; when the
`attach` already carries the wrong frame version, or the connection is a
`ridl-ws-v1` one, that reason is `frame_version_unsupported` instead. A selected
backend that cannot serve the negotiated version (version 3 needs streams and a
member table) is refused `frame_version_unsupported`. A selected backend whose
member table needs a larger message limit than the `Config` allows is refused
`range` (frame specification §11.1, "The sizes"). The capability reason wins
when both checks fail. Whenever ordinary attach checks pass and the message
limit is too small, the server records `Note::MessageLimitTooSmall`, with either
`refused(range)` or `refused(frame_version_unsupported)`. In both cases no
session data has been sent.

After a refusal the server closes with 1000. The checks run in the order above,
so one reason is given. A second `attach` on an open session closes with 1002.

**D-12. The server routes on the frame's `kind`.** Without a member table it
cannot check a `kind` against the catalog (§4 "Why `kind` is on the frame") and
checks only that the form admits the kind. With one, every `subscribe`, `read`,
`request` and `unsubscribe` first looks the frame's `(interface, ordinal)` up in
the table: absent, or present under another `kind`, a `subscribe` or a `read` is
answered `answer` with `contract(UnknownInteraction)`, a `request` a `response`
carrying it and the request's `seq` as `correlation`, and an `unsubscribe` is
dropped and recorded as `Note::UnknownInteraction` (§6.4). Nothing reaches the
backend and no signal or event state is created; a refused `request` is retained
like any settled response, for duplicate handling. The form check follows in
both cases:

| Form          | `kind` admitted   | Any other `kind`                               |
| ------------- | ----------------- | ---------------------------------------------- |
| `subscribe`   | `Signal`,`Event`  | `answer` with `contract(UnknownInteraction)`   |
| `read`        | `Signal`,`Fixed`  | `answer` with `contract(UnknownInteraction)`   |
| `request`     | `Command`,`Query` | `response` with `contract(UnknownInteraction)` |
| `unsubscribe` | `Signal`,`Event`  | discarded and recorded                         |

A provider-to-consumer form (`attached`, `answer`, `publish`, `occurrence`,
`response`) received by the server is a protocol violation, close 1002, in every
shape: a `publish` or a `response` with bytes after a header that carries no
payload, which the codec reports as a discard and a consumer would drop, closes
1002 like one without them. The discard class of §11.1 item 4 is the consumer's;
on the server it never lets a client frame through. §11.1 item 4 names "a first
provider-to-consumer message that is not `attached`" for the consumer; the
server applies the mirror rule, because a client that sends one has the roles
wrong.

**D-25. A reply to a form that does not admit the kind echoes the identity.**
The `answer` or `response` that D-12 sends carries the `kind`, `interface` and
`ordinal` of the frame it answers, as received, the way §6.3 has a refused
`read`'s `answer` carry the `read`'s identity. So a `response` may carry a
`kind` other than `Command` or `Query` when its outcome is
`contract(UnknownInteraction)`, and a decoder accepts that. The TypeScript
client is written to this rule.

### Signals

**D-13. Signals.**

- `subscribe` of a signal: read it from the backend, answer `accepted`, then
  send one `publish` of its current state (§5.1, §6.2). A backend read that
  fails with `ReadError::Contract(c)` is answered `contract(c)` instead and
  nothing is subscribed. A second `subscribe` of a subscribed signal is answered
  `accepted` and sends nothing more ("changes nothing", §6.2).
- `read` of a signal: one `publish` of its current state; nothing is subscribed.
  A failing read is answered as above. A `read` whose backend sample is `Init`,
  on a signal the session holds no state for, sends `Init` with `seq` 0 and
  creates no channel state, so a client cannot grow the session by reading
  signals that were never published.
- `unsubscribe`: the server marks the signal unsubscribed and sends nothing. The
  channel's state, its session `seq` and the sample last sent, stays for the
  rest of the session (D-24; frame specification §7), so a later `subscribe` of
  the same signal continues the count.
- The state sent is the backend's `RawSample`, mapped:

  | Backend provenance     | `publish` state   | Payload        |
  | ---------------------- | ----------------- | -------------- |
  | `Init`                 | `Init`            | none           |
  | `Live`                 | `Live`            | the bytes read |
  | `Invalid(Declared)`    | `InvalidDeclared` | none           |
  | `Invalid(Detected(_))` | `InvalidDeclared` | none           |

  The `stamp` is the backend's, unchanged. The `seq` is the session's (D-24).
  `Detected(_)` never crosses (§9.6). A backend that reports it is itself a
  consumer of some earlier binding that rejected a payload; sending its last
  good value as `Live` would hide the invalidity, so the server sends the
  invalidity and loses only the cause. A `ridl-rt` runtime does not verify
  payloads (§9), so `ridl-loopback` never reports `Detected`.
- A read that returns `ReadError::Short { needed }` is retried with a buffer of
  `needed` bytes, up to the smaller of `max_message_bytes` minus the 27-byte
  `publish` header and the member's limit (D-26); a larger value is not sent,
  and is recorded as `Note::ValueTooLarge`. The signal stays subscribed and is
  tried again on its next change. A `read` of a signal whose value is larger is
  answered `contract(UnknownInteraction)` and recorded, as a `read` of a `fixed`
  is (D-17), so that the client's read ends.

The state of each signal the client subscribes, or reads after a publication,
lasts as long as the session, with no bound in the crate, because a channel's
session `seq` never restarts (§7). Per-session memory, and in polling mode the
work of each tick, grows with the distinct signals the client names. `serve`'s
rustdoc says so: the embedder's access policy must trust each client for that
growth. With a member table (D-12) the lookup bounds it: a session holds at most
one signal channel and one event channel per member of the catalog. Without one
the growth above stands, and `serve`'s rustdoc says so.

**D-14. Detecting a changed signal.**

- **With `ScannableSignals`** (`Backend::scannable` returns `Some`): the server
  keeps one `Watermark` per interface that has a subscribed signal, set to
  `generation(iface)` when the first signal of that interface is subscribed.
  Each tick it calls `scan` once with those marks and a buffer of `Changed`
  entries, grows the buffer and scans again only when `scan` returned 0 while a
  mark is still behind its generation (the loop the port documentation
  describes), and reads and sends each changed signal that is subscribed, each
  key once. One pass per tick: an interface whose changes did not fit behind the
  ones written waits for the next tick, a provider that commits continuously
  cannot hold the tick, and a commit between two scans is not sent twice.
  Entries for signals that are not subscribed are ignored. Every `Changed` entry
  of a subscribed signal is read and published, with no `seq` filter, because
  the mark set at subscribe excludes earlier commits; so a writer that restarts
  and reuses a `seq` is still published. The cost of a tick with no change is
  one `scan` call.
- **Without it**: each tick the server reads every subscribed signal and sends a
  `publish` when the sample differs from the last state sent for that signal. It
  compares the backend `seq`, the provenance, the `stamp` and the value bytes,
  which costs one stored copy per subscribed signal. A republish of identical
  bytes under an identical `seq`, provenance and `stamp` is indistinguishable
  and harmless. The test is "differs", not "is greater": a backend's counter may
  restart or interleave (D-24). The port contract makes `seq` count every
  publication of a channel, an invalidation and a re-affirmation included (the
  conformance test
  `a_channel_sequence_number_counts_that_channel_publications`), so a changed
  `seq` is a change. The cost of a tick is one `read` per subscribed signal.
- In both modes, publications between two ticks are coalesced into the newest,
  which a signal permits (a gap on a signal channel is not a loss, §7). Each
  `publish` the server sends for a channel carries a session `seq` greater than
  the one before it (D-24).

**D-24. Sequence numbers are the session's.** §3 and §7 make each channel
counter per session, starting at 1, and the backend's counters are not: on
`ridl-loopback` they live on each writer and sink handle, so two writers of one
channel interleave and a replaced writer starts again at 1. Forwarding them
would break the consumer's ordering rule (§7): after a restart, every later
`publish` would be discarded as stale. So the server keeps, per signal and per
event channel of the session, a counter from 1, and stamps each `publish` and
`occurrence` it sends with the next value; `Init` keeps `seq` 0. The backend's
`stamp` is kept unchanged, because the stamp is the sender's (ridl §3.1) and
every timing rule of §8 is evaluated on it. A `publish` answering a `read` of a
signal whose backend `seq`, provenance, `stamp` and value bytes all equal the
sample the server last sent on that channel reuses the session `seq` it sent
then; otherwise it takes the next one. The comparison is the four fields,
because a replaced writer may repeat a backend `seq` (D-14). §13's relay rule is
for a relay that forwards frames between two sessions; this server forwards
samples from ports, which carry no session counter.

### Events

**D-15. Events.** `subscribe` of an event calls
`EventSource::subscribe(iface,
&[ord])`; `Ok` is answered `accepted`,
`Err(Contract(c))` is answered `contract(c)`. `unsubscribe` calls
`EventSource::unsubscribe`. Each tick the server reads `EventSource::next` up to
a budget and sends one `occurrence` per occurrence, with the backend's `stamp`
and the next session `seq` of that event channel (D-24), so a gap the client
sees is a gap on this session. The budget is 64 occurrences per tick, the
dropped ones counted, and fewer once the occurrence bytes sent in the tick reach
`max_message_bytes`; what is left waits for the next tick, so a provider that
raises continuously cannot hold the tick, at the cost of up to one
`poll_interval` more latency per budget on a busy channel. A `Short` read is
retried with a larger buffer, as for signals. The occurrence's trace context is
dropped: the frame does not carry one (§2).

An occurrence larger than `max_message_bytes`, or longer than its member's limit
(D-26), is dropped and noted as `ValueTooLarge`, and the session `seq` of its
channel is advanced, so the client sees a gap and records a loss (§5.2) instead
of a silent drop. The occurrence's identity is known only once it is read, so
the member's limit is applied after the read. Because the backend subscribes
before the `answer` is sent and nothing is drained until the next tick, no
occurrence reaches the client before its `answer`.

### Calls

**D-16. Calls and the correlation map.** The calls in flight and the settled
responses kept for duplicates are `server/calls.rs`.

- A `request` of a command calls `Caller::command`, of a query `Caller::query`,
  with the payload bytes and `trace: None`. The server records
  `backend Correlation → (client seq, target)`.
- A send that fails is answered at once: `SendError::Busy` is `busy`,
  `SendError::Contract(c)` is `contract(c)`, and `SendError::TooLarge` is
  `corrupt`: arguments larger than the member's capacity are not a well-formed
  encoding of its type, the same reasoning ADR-0021 decision 5 gives for
  `ShortClaim`.
- A `request` for a command or a query whose arguments are longer than the
  member's limit (D-26) is answered `corrupt` without a call to the backend and
  before any `verify` (ADR-0030 decisions 15 and 19). The server places the
  `seq` first, then checks that the member is known, then checks the length, so
  a repeat gets the kept response again and an unknown member is
  `contract(UnknownInteraction)` whatever the length. No note is recorded. With
  no member table, the limit is the default ceiling of the frame's `kind`,
  16,128 bytes, which is what the default message limit carries. A `publish` is
  not checked.
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

- A settled reply larger than `max_message_bytes`, or longer than its member's
  limit (D-26), is answered `corrupt`, recorded as `ValueTooLarge` and
  forgotten: the backend holds a value the catalog does not allow, which is a
  provider defect the client reads as `Transport::Corrupt`, and the call ends at
  once instead of at the client's bound. A `request` with `seq` 0 is recorded
  and dropped.
- **A streamed reply** is not answered by a `response`; D-27 states how the
  server serves it. The table above applies to every other call.
- **Abandoned calls.** A call whose outcome is still unknown
  `config.call_abandon_after` (default 60 s) after it was sent is forgotten on
  the backend, noted as `CallAbandoned`, and nothing is sent: the client's own
  response bound reports `Undelivered` or `Timeout` (§9.6). Without this, a
  client that calls a member no handler serves holds backend call slots until it
  disconnects; on `ridl-loopback` the call table is shared by the whole runtime
  (16 slots), so sixteen such calls would make every caller of that runtime
  `Busy`. A zero duration forgets every call whose outcome is not known on the
  first poll after the request.
- **Duplicates.** §11.1 item 5 says the binding never retransmits, so a
  duplicate `request` is a peer defect. The server still follows §7: a `request`
  whose `seq` is in flight is not sent to the backend again; one whose `seq`
  names a settled response still kept gets that response again; any other `seq`
  not greater than the highest seen is recorded (`DuplicateDropped`) and
  dropped. A response is kept, with the responses sent at once (`busy`, a
  `contract`), until `min(request.stamp, receipt) + max` by the backend's clock,
  where `receipt` is `Clock::now()` when the request arrived and `max` the
  member's bound from the member table (D-10); the `min` keeps §7's "measured
  from the request's `stamp`" when the client's clock is behind, and caps the
  retention when a client stamps ahead, so stamping requests in the future holds
  no response. The bound is taken only when the table holds the request's
  identity as a command or query under the request's `kind`: a request the table
  refuses (D-12) takes no bound from a member of another kind and is kept as one
  of the last 64. Expired responses go on each tick. The kept responses are
  indexed by `seq` and, for those with a bound, by the bound, so a repeat and an
  expiry pass cost no scan of the kept set (up to the cap, a session may keep
  hundreds of thousands of small responses). For a member the table gives no
  bound (a catalog built before the default response bound), or with no table,
  the server keeps the last 64 as before: a stated deviation from §11.1 item 6
  (driftsys/ridl#986, ruled a deviation of this crate on 2026-10-23). The
  retained bytes of a session are capped by `Config::max_retained_bytes` (16 MiB
  by default, and never below 16,164 bytes, the largest message limit); when a
  response makes the kept bytes pass the cap, the response is sent, then the
  session closes with 1008, which `serve` returns as
  `Ended::ServerClosed(1008)`. The cap applies to the last 64 as well, so a
  server with no table, or one whose members have no bound, closes with 1008
  when its replies are large enough to pass the cap before the ring holds 64. At
  the default message limit, 64 replies are at most 64 × 16,164 = 1,034,496
  bytes, below the 16 MiB default cap; an embedder that lowers the cap keeps it
  at least 64 times its message limit, lowering the message limit too, so that
  the ring alone bounds it. Expired responses also go when a `request` arrives,
  before its `seq` is placed. Closing is observable, where dropping the oldest
  response would silently break item 6.

**The provided direction.** In the provided session (D-28) the calls run the
other way: the local ports receive a call, and the remote client answers it. The
code is `server/provided/calls.rs`.

- **Serving.** When `attached(accepted)` arrives, the session calls
  `Handler::serve` once for each command and query of the backend's member table
  (D-28). A refusal by the handler records `Note::PortRefused` and closes the
  session with 1011. Each tick, the session first expires overdue calls, then
  takes claims from `Handler::next_claim` into a buffer of the call ceiling
  (16,128 bytes) while fewer than 64 calls are in flight, and sends one
  `request` for each. The `seq` of the `request` is the session's counter from
  1, the `stamp` is the claim's, and the payload is the claim's bytes.
- **The cap of 64.** At most 64 calls are in flight. A claim beyond the cap is
  not taken: it stays queued on the handler, so a caller waits as it waits for
  any handler that serves slowly. The handler belongs to the embedder's ports
  and not to the session. At the end of the session the session settles the
  calls in flight as `Transport::Down` (the table below) and leaves the queued
  claims untouched, because it never took them. The local ports are dedicated to
  the session and serve no member outside its table when it starts (the rustdoc
  of `serve_provided` says so).
- **Refused at once.** A claim is settled locally, and no `request` is sent, in
  these cases: the member is not a command or a query of the table, or its reply
  is a stream (`contract(UnknownInteraction)`; a streamed reply is not served
  over version 2); the claim's remaining time is negative, or the claim is older
  than the member's bound (`Undelivered` for a command, `Timeout` for a query);
  the arguments are longer than the member's limit (D-26) or the `request` does
  not fit `max_message_bytes` (`Corrupt`); a claim longer than the buffer
  (`ShortClaim`) is settled `Corrupt`.
- **The deadline.** A call in flight is settled when its deadline passes: the
  earlier of the claim's own remaining time and the member's bound measured from
  the claim's stamp. A member with no bound and a claim with no remaining time
  have `config.call_abandon_after` as the deadline, so a catalog built before
  the default response bound still ends every call. A command that passes its
  deadline is settled `Undelivered` and a query `Timeout`, the two outcomes that
  §9.6 gives to the absence of a response.
- **The outcome.** A `response` is matched by its `correlation` and its target.
  One that matches no call in flight records `Note::UnknownCorrelation` and is
  dropped.

  | `response` outcome                              | Local claim settled as    |
  | ----------------------------------------------- | ------------------------- |
  | `accepted`, for a command                       | `Ok`, no bytes            |
  | `reply`, for a query, within the member's limit | `Ok` with the bytes       |
  | `contract(c)`                                   | `Err(Contract(c))`        |
  | `busy`                                          | `Err(Transport::Busy)`    |
  | `corrupt`                                       | `Err(Transport::Corrupt)` |
  | an outcome outside the call kind's set (R-DS-6) | `Err(Transport::Corrupt)` |
  | a `reply` longer than the member's limit        | `Err(Transport::Corrupt)` |
  | the connection ends before any response         | `Err(Transport::Down)`    |

  An outcome outside the set (`accepted` for a query, `reply` for a command;
  frame specification §5.3 and §5.4) is settled `Corrupt` at once, so that the
  call ends and no claim waits for its deadline. The two clients differ here:
  the TypeScript consumer reports `unexpected-outcome` and leaves its call in
  flight ([`ridl-transport-ws-ts.md`](ridl-transport-ws-ts.md), D-9), and the
  TypeScript provider answers a handler that returns such an outcome, or that
  fails, with no `response` at all (its D-21). Against the TypeScript provider
  the Rust claim therefore ends at its deadline.
- **No retransmit.** The session never sends a `request` twice, so it keeps no
  settled responses.

**D-17. `fixed`.** A `read` of a `fixed` calls `Backend::read_fixed` and sends
one `publish` with `kind` `Fixed`, state `Live` with the value, `seq` 0, and
`stamp` = `Clock::now()`. An error `ReadError::Contract(c)` is answered
`contract(c)`. A `subscribe` of a `fixed` is answered
`contract(UnknownInteraction)` (D-12). Through `Polling`, every `read` of a
`fixed` is answered `contract(UnknownInteraction)`.

A `fixed` that one `publish` carries — at most `max_message_bytes` minus the
27-byte `publish` header, 16,137 bytes at the default limit — is sent whole. A
longer `fixed` is sent as `block` frames of 4,096 bytes (`BLOCK_BYTES`), all in
one step of the session, so no other frame comes between two blocks of one
payload. The rules of the blocks are in frame specification §5.5 and §11.1
"Binding version 2". The value is read up to the smaller of its member's limit
and the default ceiling of a `fixed`, 65,280 bytes (D-26). A member table can
hold any `u32` as a limit; a limit above 65,280 is held to 65,280, so a larger
value is answered `ValueTooLarge` as below and the session buffer never grows
past 65,280 bytes.

When `max_message_bytes` is below one block message, 4,130 bytes, no `block` can
be sent: the value is read only up to what one `publish` carries. A `fixed`
above that cap is not sent: the session records `ValueTooLarge` and answers the
`read` with an `answer` carrying `contract(UnknownInteraction)`, the one refusal
§6.3 defines for a `read` and the same answer a `read` of a `fixed` with no
value gets, so the client's read ends with
`ReadError::Contract(UnknownInteraction)` instead of waiting until the session
ends. The catalog is still attached and every other member is served. A `fixed`
longer than its member's limit, which a runtime that refuses on send never
stores, is answered the same way, and so is a `read` of a signal whose value is
longer than the signal's limit or than one message.

**D-27. A streamed reply over frame version 3.** The code is in
`server/session.rs`. The wire rules are frame specification §5.4, §7, §8, §9.6
and §11.1 "Binding version 3"; this decision records only what the server
chooses.

- **Selection.** The server selects `ridl-ws-v3` when the client offers it, the
  backend answers `Backend::streams()` with `Some`, and `Backend::members()` is
  `Some`; otherwise it selects as D-20 states. The public wrapper `Streaming<B>`
  is how a server serves streams: for example
  `Described::new(Streaming(Scanning(rt)), table)`, where the ports of `rt`
  implement `StreamCaller`. `Polling` and `Scanning` keep the default `None`,
  because stable Rust cannot forward a method conditionally on a bound of the
  wrapped type, and `Described` forwards `streams()`. A table with no streamed
  member makes version 3 harmless. `FRAME_VERSION` and `SUBPROTOCOL` keep
  meaning version 2, because the crate is frozen and changing a constant's value
  would be a break; version 3 has its own constants. An `attach` must name the
  frame version the handshake selected (D-11).
- **Dispatch.** A `request` whose member's reply is a stream, read from the
  member table, calls `Caller::query` and then follows the stream through
  `StreamCaller`. A `SendError` of `Busy`, `Contract` or `TooLarge`, and an
  argument longer than the member's limit (`corrupt`), are answered with one
  `stream_end` of the mapped outcome, `seq` 1 and stream 0, never a `response`.
  The frame kept for a retransmit is `stream_end interrupted` with `seq` 1,
  whatever the first answer was. Any other `SendError` is `Detached`, which ends
  the session with 1001 (D-18). A retransmit while the stream is open is
  `DuplicateDropped` and sends nothing.
- **Pulling.** Each tick, for each streamed call, the server pulls chunks with
  `StreamCaller::pull` and sends each as a `chunk` with the next `seq` of the
  stream, from 1, while the client's credit lasts, for at most 64 chunks per
  stream per tick (`CHUNKS_PER_TICK`), so that a provider that pushes
  continuously cannot keep a tick from returning. With credit 0, the server
  pulls once into a one-chunk hold for the stream: an end or a failure is sent
  at once as a `stream_end`, and a held chunk waits for credit. So a stream that
  spends its last credit on its last chunk still ends without waiting for a
  credit that the client may not send; the cost is one chunk, at most the
  member's limit, held per stream. A chunk longer than the member's limit, or
  than one message carries, stops the stream with `stream_end corrupt` and is
  recorded as `ValueTooLarge`. The server grants no credit of its own: no member
  it serves has a parameter stream. After a `stream_end` the server forgets the
  call.
- **Abandonment.** The clock of `call_abandon_after` for a streamed call
  restarts at each `chunk` sent and each `stream_credit` received. When it
  passes with neither, the server sends `stream_end interrupted`, forgets the
  call and records `Note::CallAbandoned`. The clock is the host's monotonic
  clock (`Instant`), not the backend's.
- **What the client may send.** A `stream_credit` adds to the credit; one for a
  call that is not streaming, which can arrive just after the end, is ignored
  and records no note. A `stream_end` stops the stream: the server aborts it,
  forgets the call at once, records `Note::StreamStopped`, sends nothing more
  for the call, and keeps a `stream_end interrupted` for a later retransmit of
  the `request`. The server aborts the stream on the backend with the error of
  the client's outcome (`interrupted`, `contract` or `corrupt`) and then forgets
  the call, so a provider on the loopback reads `Interrupted` whatever the abort
  carried: after a forget a push answers `Interrupted`. The verdict reaches a
  provider only on a parameter stream, which is not built. A `stream_end` with
  `end` or `busy` closes 1002 with `ProtocolViolation::OutcomeNotAllowed`. A
  `stream_credit` or a `stream_end` on a stream other than 0 is ignored and
  records no note. A client `chunk` on stream 0 is `Note::UnexpectedDirection`
  and a client `chunk` on any other stream is `Note::StreamCreditExceeded`, each
  closing 1002.
- **A streamed member on a session below version 3.** A `request` for a member
  whose reply is a stream is answered `response` with
  `contract(UnknownInteraction)` and `Note::UnknownInteraction`, because version
  2 has no outcome that names a missing capability. A conforming client never
  sends it (`ridl-transport-ws-ts.md`).
- **The end of a session.** The server forgets every call, and the port's
  `forget` stops every open stream with `Transport::Interrupted`
  (`ridl_rt::error::Transport`). The provider cannot read `Transport::Down`
  through the port, so none is sent.

### The end of a session

**D-18. The session's end.** At every end, in-flight backend calls are forgotten
(the provider of a streamed reply reads `Transport::Interrupted`, D-27), and
every backend subscription is dropped with the `Backend` value; then the
connection is dropped. A backend port that reports `Detached` from any call ends
the session with close 1001, because the provider is going away. `serve` returns
an `Ended`:

| `Ended`              | When                                                                                                                                                                                                                                                                                                                                                                                                          |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ClientClosed(code)` | the client sent a close frame; the server answered with the same code, or 1000 when the client's carried none                                                                                                                                                                                                                                                                                                 |
| `ServerClosed(code)` | the server sent a close frame (D-23)                                                                                                                                                                                                                                                                                                                                                                          |
| `Rejected`           | the handshake was answered `400`, `403`, `404` or `409` (D-19, D-20)                                                                                                                                                                                                                                                                                                                                          |
| `LivenessTimeout`    | the handshake did not finish within `liveness_timeout`, the session had not attached that long after the upgrade (D-22), no message arrived for that long after it, or a write waited that long; no close frame is sent                                                                                                                                                                                       |
| `Io(kind)`           | a read or a write failed; `UnexpectedEof` is a client that closed the connection without a close frame                                                                                                                                                                                                                                                                                                        |
| `ConfigRefused`      | the message limit of the `Config` cannot carry one message of a member of the catalog (D-26); from `serve`, returned before the handshake, nothing is read or sent. From `serve_selected`, also an unsupported `max_version` (before the handshake) or a selected backend that cannot serve the negotiated version or the message limit (D-11); then the `attach` is answered `refused` and the close is 1000 |

The embedder may also receive `Note` values through `Config::with_record`; with
no hook nothing is recorded and recording costs nothing. A `Note` is one of:
`Violation` (closes 1002), `MessageBeforeAttach` (closes 1002), `SecondAttach`
(closes 1002), `UnexpectedDirection` (closes 1002), `KindNotAdmitted` (an
`unsubscribe` of a kind the form does not admit; dropped), `ValueTooLarge`,
`DuplicateDropped`, `CallAbandoned`, `StreamCreditExceeded` (a `chunk` from the
client on a stream the server granted it no credit for; closes 1002),
`StreamStopped` (the client stopped a reply stream), `RuleNotCarried` (a
violation's rule the frame has no code for; the outcome is replaced),
`UnknownInteraction` (an interaction the member table does not hold; dropped or
answered as D-12 says) and `MessageLimitTooSmall` (the member whose messages the
`Config` cannot carry, with the limit it needs; `serve` records it before the
handshake. `serve_selected` records it after selection when ordinary attach
checks pass, with `refused(range)` or, when the capability check also fails,
`refused(frame_version_unsupported)`. Both return `ConfigRefused`, D-26).

### `Config`

`Config` (`server/config.rs`) has four public durations — `poll_interval` (20
ms), `ping_interval` (5 s), `liveness_timeout` (15 s) and `call_abandon_after`
(60 s) — and four settings behind methods: `with_max_message_bytes` (16,164
bytes, `MAX_MESSAGE_LIMIT`, the largest data-plane message: the limit can only
be lowered, and a value outside the range 294 to 16,164 is refused with
`LimitOutOfRange`), `with_max_retained_bytes` (16 MiB; a value below 16,164 is
refused with `RetainedTooSmall`, whatever the message limit, so that one
response of the largest message never closes a session on its own, and a session
applies the cap as set), `with_record` and `with_origin_check`. A
`ping_interval` at or above `liveness_timeout` ends an idle client's session
before the first ping, because a client that is not pinged sends nothing; the
rustdoc says so.

### The sizes

**D-26. The limit of a member, on both sides.** The size of a payload is the
transport's to refuse, against the interaction's limit from the catalog
(ADR-0030 decision 19). The session resolves a member's limit as the limit the
member table gives it, or, when the table gives none or the backend has no
table, the default ceiling of the member's kind, which `ridl-rt` exports: 3,840
bytes for a signal and an event, 16,128 for a command and a query, 65,280 for a
`fixed` (ADR-0030 decision 16). Without a table the kind is the frame's `kind`.

- On receive, the arguments of a `request` longer than the member's limit are
  answered `corrupt` without a call to the backend and before any `verify`
  (D-16). Every `request` is checked, with or without a table.
- On send, a value longer than its member's limit is refused as a value that
  does not fit one message is refused: a signal's `publish` and an `occurrence`
  are not sent and the value is recorded as `ValueTooLarge` (D-13, D-15), a
  reply is answered `corrupt` and recorded (D-16), and a `read` of a `fixed` or
  of a signal is answered `contract(UnknownInteraction)` and recorded (D-17).
  The crate implements no sending port, so this is where its send side is. With
  a runtime that refuses a value above its member's limit on send, as
  `ridl-loopback` does, the path is reached only by a backend that does not.
- In the provided session the same limits apply to what the client sends: a live
  `publish` longer than its signal's limit is delivered as a detected-invalid
  sample, an `occurrence` longer than its event's limit is delivered as a
  corrupt occurrence, and a `reply` longer than a query's limit settles the call
  `Corrupt` (D-29, D-16). The arguments of a local call longer than the member's
  limit are refused to the local caller as `Corrupt`, and no `request` is sent.

**The message limit.** `MAX_MESSAGE_LIMIT` is 36 + 16,128 = 16,164 bytes: the
largest header with its envelope of a message that carries a payload, plus the
default ceiling of a command and a query, the largest payload one message
carries (ADR-0030 decision 10). It is the default of `max_message_bytes` and the
largest value accepted, and the smallest `max_retained_bytes` accepted.

**The `Config` check.** `serve_selected` runs the same check after selection, as
D-11 says: it answers `refused(range)` (or `frame_version_unsupported` if the
capability check also fails) with close 1000 instead of returning before the
handshake, because the backend is not known earlier. `range` is the reason for a
limit the receiving side cannot carry, here the message limit of the `Config`
(frame specification §6.1). With a member table, `serve` first checks that
`max_message_bytes` is at least 36 plus the largest limit over the members of
what one message carries: each member's limit, and for a `fixed` at most 4,130
bytes minus 36, because a `fixed` that one message cannot carry crosses in
blocks and a block message is at most 4,130 bytes (frame specification §11.1,
"Binding version 2"). So a `fixed` counts the smaller of 36 plus its limit and
4,130. A limit below that cannot carry a payload the catalog allows, which is a
configuration error, not a session event: `serve` records `MessageLimitTooSmall`
with the member and the limit it needs, returns `Ended::ConfigRefused` before
the opening handshake without reading the connection, and the connection closes
when it is dropped. This preflight sends no `attached` reply. The selected path
uses the existing `range` reason; no new wire reason is added. The default
`Config` carries every catalog, a catalog with a large `fixed` included, so a
binding never refuses an allowed interaction at attach (ADR-0030 decision 20).
Without a member table there is no `Config` check: the default ceiling of the
frame's `kind`, under the message limit, bounds what the session sends and what
a `request` carries.

The check uses one header size for every member, 36 bytes, the largest
data-plane header with its envelope. A `fixed` crosses only in a `publish`,
whose header is 27 bytes, so for a `fixed` below one block message the check
refuses a message limit from 27 plus its limit to 35 plus its limit, which would
carry every value of it whole: up to 9 bytes more than needed. The check refuses
the same 9 bytes for a signal, which also crosses in a `publish`. The rule is
not made exact for one kind; that would be a separate change of the check.

## The WebSocket layer (`ws`)

The layer is `crates/ridl-transport-ws/src/ws.rs`, private, with its tests in
the same file against `tungstenite`.

**D-19. The handshake.** One `GET` request with a `Host` header,
`Upgrade: websocket`, `Connection: Upgrade`, `Sec-WebSocket-Version: 13` and one
`Sec-WebSocket-Key` that decodes from base64 to 16 bytes (RFC 6455 §4.2.1). The
request line and headers are limited to 8 KiB; above that, or on any malformed
request, the server answers `400 Bad Request` and closes. The version is checked
before the other headers: when a well-formed request's version is not 13,
whatever its other headers hold, the `400` carries `Sec-WebSocket-Version: 13`,
the version the server speaks (RFC 6455 §4.2.2 item 4). Other headers are
ignored, and so are the path and the query for `serve` and `serve_selected`
(§11.1 item 1: the binding places nothing there; the embedder's access policy
runs before them). `serve_provided_selected` does not ignore the request target:
it gives it, unchanged, to the embedder's selector (D-28), after the version and
`Origin` checks and before the `101`. The selector returns the backend, or a
`SelectionError`: `NotFound` is answered `404 Not Found` and `Conflict` is
answered `409 Conflict`, each with `Connection: close` and `Content-Length: 0`,
and `serve_provided_selected` returns `Ended::Rejected`. The binding gives the
target no meaning; the embedder does (the debug gateway reads
`/<component>/<instance>/<catalog>`, see
[the deployment descriptor record](deployment-descriptor.md#attaching-to-a-node)).

The `Origin` header is the one exception: it is read and given to a check the
embedder sets with `Config::with_origin_check`, as `None` when the request has
none. A request the check refuses is answered `403 Forbidden`, with the same
linger as a `400` (D-23), and `serve` returns `Ended::Rejected`. Without a check
every origin is accepted, because the embedder chooses the policy. A request
with two `Origin` headers is answered `400`, so that the check and a proxy
cannot read different values. The embedder's access policy sees only the peer
address, which is the same for any web page a local browser loads, so a
browser-facing embedder should set the check; the crate rustdoc says so, and
that the crate authenticates no one.

**D-20. The subprotocol.** The server selects `ridl-ws-v3` when the client
offers it, in any position of `Sec-WebSocket-Protocol`, and, for `serve`, the
backend serves streams and holds a member table (D-27). For `serve_selected`,
version selection uses the caller's `max_version`: 3 permits `ridl-ws-v3`, while
2 permits at most `ridl-ws-v2`, before the backend is known. Otherwise it
selects `ridl-ws-v2` when the client offers it. When the client offers
`ridl-ws-v1` and not `ridl-ws-v2`, the server selects `ridl-ws-v1`, refuses
every `attach` on that connection `frame_version_unsupported` (D-11) and closes
with 1000, so that a version 1 client learns the reason instead of a failed
handshake. When the client offers no subprotocol the server serves (`ridl-ws-v3`
counts only when the applicable condition above holds), the server answers
`400 Bad Request` and does not upgrade. §11.1 says that the client fails a
connection on which the server selected none; answering 400 makes the failure
visible at once, also to a client that does not check. The selection is a table
from subprotocol name to the role of the client, and each entry point selects
only the names of its own role:

| Subprotocol           | Frame version | The client is | Entry points                                |
| --------------------- | ------------- | ------------- | ------------------------------------------- |
| `ridl-ws-v3`          | 3             | the consumer  | `serve`, `serve_selected`                   |
| `ridl-ws-v2`          | 2             | the consumer  | `serve`, `serve_selected`                   |
| `ridl-ws-v1`          | 1             | the consumer  | `serve`, `serve_selected` (refused)         |
| `ridl-ws-v2-provider` | 2             | the provider  | `serve_provided`, `serve_provided_selected` |

An entry point ignores the names of the other role, and selects, among the names
of its own role that the client offers and its version limit permits, the one
with the highest frame version. So a mixed offer selects `ridl-ws-v2` at `serve`
and `ridl-ws-v2-provider` at `serve_provided`, and an offer that holds no name
of the entry point's role is answered `400`. A listener of an embedder that
binds `serve` therefore refuses the reversed role, and a listener that binds
`serve_provided` refuses the consuming roles, each at the handshake (frame
specification §11.1, "Reversed role"). The provided role has no `attach` to
refuse on a `ridl-ws-v1` connection: the client of that role does not exist.

**D-21. Messages.** Binary messages only; a text message closes with 1003.
Fragmented messages are reassembled, also when a frame is split across socket
reads. A data message whose total size passes `max_message_bytes` closes with
1009 as soon as the length is known, before it is buffered; the limit applies to
data messages, not to control frames, which RFC 6455 caps at 125 bytes.
`max_message_bytes` is at least 294 and at most 16,164 (§11.1 "The sizes").
Client frames that are not masked close with 1002 (RFC 6455 §5.1). A ping is
answered with a pong carrying its payload.

Each frame the server sends, header and payload, is assembled in one buffer the
session reuses and written with one write call, so that with `TCP_NODELAY` off
the payload does not wait for the peer's delayed acknowledgement of the header.
The buffer keeps the capacity of the largest frame sent, which
`max_message_bytes` bounds (16,164 bytes by default per idle session after a
large message).

**D-22. Liveness.** The server sends a ping every `ping_interval` and ends the
session when no message of any kind has arrived for `liveness_timeout`. The time
the server spends writing the frames of one message's answer or of one poll is
not counted, because it cannot read while it writes; a client that stops reading
is ended by the write timeout (D-7). The time a client takes to read bytes the
kernel has already accepted is still counted.

The wait for the first message of the session is bounded by `liveness_timeout`
as a whole: the `attach` for `serve` and `serve_selected`, and the `attached`
for the provided session (D-28). While the session has not attached, the session
ends as `Ended::LivenessTimeout` once `liveness_timeout` has passed since the
upgrade, whatever control frames arrive meanwhile. Before this bound, a peer
that sent pongs or pings and no `attach` kept the connection open, because any
message reset the silence clock; the bound applies to `serve` and
`serve_selected` too, and not only to the provided entry points. The server
sends its pings from the upgrade on in either role. A browser answers a ping
automatically, so 5 s pings detect a dead client in 15 s at a cost of one small
frame per 5 s.

**D-23. Close codes.** Exactly §11.1 item 4's: 1000 on purpose and after
`attached(refused)`, 1001 when the backend detaches, 1002 on a protocol
violation, 1003 on a text message, 1009 on an oversized message, and 1008 when
the retained responses pass `max_retained_bytes` (D-16). The provided session
adds one: 1011 when the local `Handler` refuses to serve a member of the table
when the session starts (D-16, "The provided direction"). Item 4 does not list
1011. The server answers a client's close frame with a close frame that carries
the same code. After every close frame it sends, the answer to the client's
included, and after a handshake `400`, `403`, `404` or `409`, the server shuts
down its write side and lingers for up to 100 ms, reading and discarding what
the client still sends, so that the close frame or the answer reaches the client
instead of being lost to a reset of the connection. It waits no longer than that
for the client to close the connection (RFC 6455 §7.1.1 has the client wait for
the server to close it).

## Tests

All under `crates/ridl-transport-ws/`.

- **The vectors** (`tests/vectors.rs`). One test reads the binding version 1
  test-vector table, one reads the binding version 2 table, one reads the
  binding version 3 table and one reads the "Reversed role" table, all out of
  `docs/specification/frame-specification.md`. Each decodes every `Hex` cell,
  checks that it decodes to the frame its `Vector` cell describes, and that
  encoding that frame gives the same bytes. Each fails on a row whose id it does
  not know and on a known id that its table lacks, so a new vector cannot be
  skipped. Reading the tables from the specification, instead of copying them,
  means the test cannot drift from them.
- **Every code** (`tests/codes.rs`). Every code of §4 and §9.6, written out as
  bytes by hand and checked in both directions, so a wrong code constant cannot
  cancel itself out in a round trip.
- **The binding sizes** (`tests/sizes.rs`, driftsys/ridl#718). For payload
  lengths 0, 1 and 255, a `response` under `reply` encodes to exactly
  `frame_header_max_bytes + envelope_bytes + len` bytes, read from the
  `websocket` row of `ridl_ir::codegen::known_bindings()`, and the row's version
  is the binding version 3; every other form that carries a payload (`publish`,
  `occurrence`, `request`, `block`, `chunk`) encodes to no more; an `attach`
  with a 255-byte name is 294 bytes. `known_bindings()` is a public function
  added to `ridl-ir` for this test, because the only other public route to the
  row needs a whole lowered system.
- **The streamed reply** (`tests/serve.rs`, the stream group). Over a
  `Described` `Streaming` backend on a loopback with a stepped provider: the
  selection of each version for each backend (D-27); 64 elements and the end
  with the next `seq`; an empty stream; four chunks and no more without credit;
  a credit stall that does not block a signal; a retransmit after the end; a
  refusal at dispatch; a provider error that crosses as a `stream_end`; the
  abandonment clock; a client `stream_end`, a client `chunk` and a `stream_end`
  with `end`; and the stream forms on a session below version 3, which are an
  unknown code. The frame codes of the three forms are in `tests/codes.rs`.
- **The constraints** (`tests/constraints.rs`). D-1's "no `alloc`" and D-2's
  dependency set, read from the sources and the manifests (see "Gates").
- **The codec's failures** (`tests/decode_errors.rs`). One test per case of D-4
  and D-5.
- **The WebSocket layer** (unit tests in `src/ws.rs`, `src/ws/sha1.rs`,
  `src/ws/base64.rs`), against `tungstenite` as the client: the handshake and
  `Sec-WebSocket-Accept` (the RFC 6455 sample key `dGhlIHNhbXBsZSBub25jZQ==`
  gives `s3pPLMBiTxaQ9kYGzzhZRbK+xOo=`), the subprotocol rule of D-20, the
  `Origin` check, fragmentation fed one byte at a time, a text message (1003),
  an unmasked frame (1002), an oversized message (1009), ping and pong, and the
  close handshake. SHA-1 and base64 against their published test vectors. The
  tests bind `127.0.0.1:0`; the crate's own code never does.
- **The session rules** (unit tests in `src/server/session.rs`), on the sans-IO
  core over `ridl-loopback`, one group per decision D-11 to D-18 and D-24,
  including both modes of D-14: the loopback through `Scanning`, and the same
  loopback through `Polling`.
- **`serve`** (`tests/serve.rs`), with a `tungstenite` or raw TCP client: a
  signal change streamed, a rejected handshake, the client's close, pings, the
  liveness timeout of a silent client, a client that stops reading, a violation
  recorded as a `Note`, a backend that detaches (1001).
- **The done-when** (`tests/scenario.rs`, with `tests/support/`). A scenario
  function, generic over a rig that gives consumer ports
  (`SignalReader + EventSource + Caller + FixedReader`) and a way to let time
  pass, runs one sequence over a test catalog of one interface with a signal, an
  event, a command, a query and a `fixed`: read the `fixed`; read the signal as
  `Init`; subscribe the event; send the command and get `accepted`; the
  provider, on accepting the command, sets the signal, commits, and raises the
  event with the argument, and the consumer reads the signal as `Live` and
  receives one occurrence; send the query and get the provider's function of its
  argument. It runs twice: in process over `ridl-loopback` with no socket; and
  across processes, where the test re-runs its own executable with the
  environment variable `RIDL_TRANSPORT_WS_PROVIDER=1`, the child serves a
  loopback, wrapped in `Described` with the hand-built member table of the test
  catalog, on a port of `127.0.0.1` it prints, and the parent runs the scenario
  over a test-only client in `tests/support/client.rs` — the consuming runtime
  of §5 written once for the test over `tungstenite` and this crate's codec. It
  is not published.

  "The same tests" of the done-when is read as this one scenario, not the
  `ridl-rt-conformance` suite, whose `Factory` requires `SignalWriter`,
  `EventSink` and `Handler` on the consumer's value and cannot run over a
  consumer-only port. The contract is a hand-written test catalog, not a
  generated face, because a generated face over a WebSocket client needs a
  public Rust client, which the crate does not ship (driftsys/ridl#989).
- **Selection** (`tests/selected.rs`). `serve_selected` over `ridl-loopback`:
  the closure runs once and receives the name and hash of the `attach`; `None`
  answers `unknown_catalog`; the ordinary refusals keep their priority over the
  two selected-backend refusals; a backend without streams under version 3
  answers `frame_version_unsupported`, and a message limit that is too small
  answers `range` and records the note; both end as `Ended::ConfigRefused` after
  close 1000; an unsupported `max_version` reads nothing.
- **The debug gateway** (`tests/debug_gateway.rs`, over the committed identity
  deployment of `ridl-rt-conformance`). Independent `tungstenite` clients attach
  per catalog, observe the initial and later signal values and the events of
  every catalog the machine holds, including a mapped one, send a command and
  read its answer, are closed with 1002 when they publish, and release their
  attachment when they disconnect. For the injection listener the tests send raw
  requests: every malformed path, an unknown component, instance or catalog, a
  catalog the instance only maps, and a path with a query, a percent escape or
  an extra segment, is answered `404`; a path whose instance is attached as
  itself is answered `409`; the session subscribes only the members of the
  interfaces that the instance offers; and a handshake that ends in any way
  (origin refusal, wrong subprotocol, a refused `attached`, end of input, an
  accepted session) releases the twin.
- **The provided session** (`tests/provided.rs`, `tests/provided_signals.rs`,
  `tests/provided_calls.rs`, and the unit tests under `src/server/provided/`).
  Each listener accepts only its role, and a mixed offer selects the role of the
  entry point; the selector receives the exact request target, after the
  `Origin` check; the server sends `attach` first, with the port's encoding; a
  refused `attached`, a malformed one, a failed write and a failed upgrade
  release the backend; the reversed vectors reach the backend after the
  `subscribe` is answered; every consumer disposition of D-29 (the `Init` rule,
  a duplicate or stale `seq`, a payload over the limit, an expired occurrence, a
  gap, the commit groups of R-DS-4); the calls of D-16 (every outcome, the
  deadline, the cap of 64, `Down` at the end of the session); and the server's
  pings and its close of a client that falls silent.
- **The debug gateway example** (`examples/debug_gateway.rs`, with
  `examples/support/debug_gateway.rs`; the `descriptor` feature). Reads a
  deployment descriptor and a machine name, builds an `ridl-loopback` `Node`,
  binds `127.0.0.1` on the port given (0 asks for any free port), prints
  `port <n>`, and serves each connection on its own thread with
  `serve_selected`. The selector finds the catalog the client names among the
  node's catalogs and attaches to it as the listener (`InstanceNo::LISTENER`),
  with the member table of the descriptor's embedded catalog. The listener reads
  every signal and event of the machine's catalogs and may send commands and
  queries; it writes nothing. With a fourth argument, the injection port, the
  example binds a second listener on `127.0.0.1`, prints `inject-port <n>`, and
  serves each connection with `serve_provided_selected`. Its selector reads the
  request path `/<component>/<instance>/<catalog>`, finds the instance on the
  machine that offers the catalog, and attaches to the node as the twin of the
  instance (`InstanceNo::twin`). A path that names nothing, or a catalog that
  the instance only maps, is `NotFound`; an instance that is attached as itself
  makes the attach fail with `IdentityConflict`, which is `Conflict`. The member
  table of the session holds only the interfaces the instance offers. The node
  lives in the gateway's own process, so only components that run in that
  process are visible through it. The example states `max_version` 2 for the
  listener. The example has no access policy beyond the loopback address (D-7).
  A gateway that does not bind the injection port cannot inject.
- **The debug gateway fixture** (`examples/debug_gateway_fixture.rs`, the
  `descriptor` feature). The gateway of the previous item over the committed
  identity deployment, with a control channel on standard input and output: the
  Deno end-to-end test of the TypeScript `provide` drives it and reads the
  node's own view of each value through local consumer ports, independently of
  the gateway's buffers. `just ts-e2e` builds it and runs
  `packages/ridl-transport-ws/e2e/provide_test.ts` against it.
- **The loopback server** (`examples/loopback_server.rs`). Serves the scenario's
  test catalog and provider, with its member table, on `127.0.0.1:0`, one thread
  and one attachment per connection, prints `port <n>`, and exits when its
  standard input ends. It serves the TypeScript package's end-to-end tests. The
  package leaves it out.

The test targets `serve` and `scenario` require the `server` feature, so
`--no-default-features` still runs the codec's tests.

## Gates

- `just wasm-check` checks the codec with `--no-default-features` for
  `wasm32-unknown-unknown` and for `thumbv7em-none-eabihf`, a target with no
  standard library, which is the proof of the `std` half of D-1's "`core` only".
  The `alloc` half is `tests/constraints.rs`, which asserts that `src/frame.rs`
  and `src/lib.rs` name no `alloc` crate outside comments.
- `tests/constraints.rs` also asserts D-2 from the manifests: the one
  unconditional dependency is `ridl-rt`, through a workspace entry that names no
  feature; the one optional dependency is `ridl-descriptor`, through a workspace
  entry with its default features off; the `server` feature is empty, so it
  turns on nothing in `ridl-rt`, and the `descriptor` feature turns on `server`
  and `ridl-descriptor` only. The checks are textual.
- `just compat-check` requires the manifest's `rust-version` and edition to
  equal `ridl-rt`'s, and builds the packaged crate with that minimum toolchain
  with its default features, with none, and with the `descriptor` feature,
  against the packaged `ridl-descriptor` and `ridl-ir` patched in by path, a
  lockfile resolved by the fallback resolver and sources fetched by the
  minimum's cargo. The `tungstenite` dev-dependency table is removed from the
  packaged manifest first, because the offline build cannot resolve it; the
  recipe fails when the package has any other dev-dependency table. The recipe
  also fails when the crate gains a workspace dependency outside `ridl-rt`,
  `ridl-rt-conformance`, `ridl-loopback`, `ridl-descriptor` and `ridl-ir`.
- The crate is published after `ridl-rt` by `crates-io-release.yml`.
- The crate is in the `runtime-crates` and `msrv` freeze surfaces from 0.12.0,
  its first release, and opts into the `cargo-semver-checks` lint metadata
  (ADR-0029).

## The provided session

The debug socket needs a WebSocket client to stand in for a component: publish
its signals, raise its events and answer the calls that the machine's consumers
send to it (driftsys/ridl#988). On the frame, those are the **providing** side
of a session, and §11.1 item 1 makes the WebSocket client the consumer. So the
stand-in is a session in the other direction, which the subprotocol
`ridl-ws-v2-provider` selects (frame specification §11.1, "Reversed role"): the
WebSocket client provides, and the server consumes and delivers what it receives
into local ports. The frame does not change. The server sends `attach`,
`subscribe` and `request`; the client sends `attached`, `answer`, `publish`,
`occurrence` and `response`.

**D-28. The provided session.** The code is `server/provided.rs`. The rulings
R-DS-3 to R-DS-6 that this decision and D-29 and D-16 hold are in the archived
plan
([`2026-11-03-debug-socket-plan.md`](../archive/2026-11-03-debug-socket-plan.md),
"Delegated rulings"); this record states them as built.

- **Entry points.** `serve_provided(conn, backend, &config) -> Ended` and
  `serve_provided_selected(conn, select, &config) -> Ended`, where `select` is
  `FnOnce(&str) -> Result<B, SelectionError>` and is called once with the
  request target (D-19). `serve_provided` is `serve_provided_selected` with a
  selector that returns the one backend and ignores the target. The `Origin`
  check runs before the selector. After selection, the session checks the
  message limit against the backend's table as D-26 does, and answers `400` and
  returns `Ended::ConfigRefused`, before the upgrade, when the limit is too
  small (it records `Note::MessageLimitTooSmall`) or when the catalog name or
  the port's encoding cannot be put in an `attach` (no note). The backend is
  dropped on every exit, selection and handshake failures included.
- **The backend.** `ProvidedBackend` (`server/provided_backend.rs`) has
  `Ports: Attached + Encoded + Clock + Handler`, `ports()`, and `members()`, the
  `MemberTable` of the members **that this instance offers in this catalog**;
  the embedder filters it before it builds the backend, and the session
  subscribes, serves and accepts nothing outside it. It receives the deliveries
  of a consumer, not port writes, through three methods: `sample` for each
  accepted signal sample (interface, ordinal, provenance with its cause, the
  last good value, freshness, the remote envelope), `occurrence` for each
  accepted occurrence (the payload or the cause, the remote envelope, and the
  count of occurrences missed since the last accepted one), and `commit` at the
  end of a group of signal frames (D-29). `Providing<P>` adapts the local ports;
  its constructor refuses ports and a table of different catalogs
  (`TableMismatch`, as `Described` does).
- **The first messages.** The server writes `attach` right after the upgrade,
  with frame version 2, the encoding of the ports, and the catalog of the ports.
  It then waits for `attached`, for at most `liveness_timeout` (D-22). A message
  of another form before `attached` closes with 1002; a discarded frame before
  it is `Note::MessageBeforeAttach` and closes with 1002. `attached(refused)`
  closes the session with 1000, and the backend is dropped: the client checks
  the catalog and encoding of the `attach` (frame specification §6.1), as the
  server does in the other role. `attached(accepted)` starts the session: the
  handler serves the members (D-16), and the server sends one `subscribe` for
  every signal and event of the table, with no `read`. A second `attached` and
  every consumer form (`attach`, `subscribe`, `read`, `request`, `unsubscribe`),
  and `chunk`, `stream_end` and `stream_credit`, close with 1002.
- **The tick** is the one of D-8. In the provided session it closes a commit
  group that has been idle for a `poll_interval` and settles the calls of D-16.
  The WebSocket layer, the pings, the write timeout and the close codes are
  those of the providing role (D-21 to D-23).

**D-29. The provided session is a full consumer.** Writing a remote provider's
frames straight into `SignalWriter` and `EventSink` cannot follow the consumer
rules of frame specification §5 to §9, which the "Reversed role" section applies
to the server: the ports cannot restore an `Init`, cannot carry a
detected-invalid sample, and cannot carry a corrupt occurrence or a remote
envelope and sequence (R-DS-3). So the session applies the rules itself, as a
consuming client does (`server/provided/signals.rs`), and gives its backend the
result:

- **Channels.** A `publish` or an `occurrence` for a member that is not in the
  table under that kind records `Note::UnknownInteraction` and is dropped. One
  for a channel whose `subscribe` the client did not answer `accepted` records
  `Note::NotSubscribed` and is dropped. A `seq` that is not greater than the
  last one accepted on the channel records `Note::DuplicateDropped` and is
  dropped. An `answer` for a member of the table under its kind settles the
  subscription; any other records `Note::UnknownInteraction`.
- **Signals.** `Init`, which the client sends after a `subscribe` when it has no
  value yet (frame specification §5.1), clears the channel's last good value and
  is delivered as `Init`. `InvalidDeclared` is delivered as `Invalid(Declared)`.
  A live payload longer than the member's limit is delivered as
  `Invalid(Detected(Corrupt))` before any verification (ADR-0030 decisions 15
  and 19), and the last good value stays. Any other live payload is `Live` and
  becomes the last good value. Freshness is computed against the local clock and
  the member's bound from the remote stamp.
- **Events.** A gap in the `seq` is counted and delivered with the next
  occurrence, and recorded as `Note::EventGap`. An occurrence older than the
  member's bound is not delivered, and its gap is still recorded, because the
  sequence tracking sees it. A payload longer than the member's limit is
  delivered as `Err(Corrupt)`.
- **Commit groups (R-DS-4).** Only signal frames form a group: consecutive
  accepted frames with the same remote `stamp`. An occurrence never opens,
  extends or closes a group (frame specification §5.2: there is no staging and
  no commit for an event), and no group holds only occurrences. A group closes
  before a frame with another stamp, before a second accepted frame for a
  channel already in the group, and after one idle `poll_interval`, so that one
  local `commit` carries at most one frame per channel (§5.1) and a client that
  keeps one stamp still commits. The group is not closed when the session ends:
  the `Providing` adapter discards the writes it staged.

`Providing<P>` maps these deliveries onto the local ports
`P:
Attached + Encoded + Clock + SignalWriter + EventSink + Handler`. Its
rustdoc lists what the ports cannot carry, and the book states the loss for a
user ([debugging a machine](../book/debugging.md)). The loss is confined to the
debug path:

| Delivery                      | What the ports receive                                        | What is lost                                                    |
| ----------------------------- | ------------------------------------------------------------- | --------------------------------------------------------------- |
| `Init`                        | nothing                                                       | the twin has not published; the last value on the machine stays |
| live value                    | `set`; `commit` at the end of each group, stamped by the node | the remote stamp: the node stamps with its own clock            |
| declared invalid value        | `invalidate`                                                  | nothing                                                         |
| detected invalid value        | `invalidate` and `Note::DetectedInvalid`                      | the cause: the machine reports `Declared`                       |
| occurrence                    | `raise`, with a new local envelope and sequence               | the remote envelope and sequence                                |
| corrupt occurrence            | not raised, `Note::CorruptOccurrence`                         | the occurrence                                                  |
| gap in the sequence           | `Note::EventGap`                                              | the count, outside the note                                     |
| a port that refuses the write | `Note::PortRefused`                                           | the delivery                                                    |

No frozen port trait changes.

## What is not built

- Streams on the provided session: it carries frame version 2 only, a query
  whose reply is a stream is refused locally with `UnknownInteraction`, and a
  later binding version would add them. The arbitration of a redundant provider
  set (driftsys/ridl#1196): the instances of a set are all attached, and the
  node does not choose between them.
- A parameter stream on a command or a query, a byte stream, and the UTF-8 rule
  of a `<string>` stream. A chunk from the client is refused (D-27). The `Rule`
  for UTF-8 is added with the parameter streams: to version 3 if it has not been
  released, else to a later version.
- A public Rust client, and the generated face of `examples/cabin` across the
  transport (driftsys/ridl#989). `examples/cabin/server` already serves the
  `temperature` signal to a TypeScript client, with the member table read from
  the descriptor `ridl build --emit catalog` writes.

## Alternatives considered

| Alternative                                                             | Verdict  | Why                                                                                                                                                                                                                                  |
| ----------------------------------------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `tungstenite` as the WebSocket layer                                    | rejected | about twenty crates in the embedder's tree (`http`, `httparse`, `sha1`, `rand`, `thiserror`, `log`, `bytes`, `data-encoding`, and what those depend on), against the dependency constraint, for a server side of a few hundred lines |
| `tokio` with `tokio-tungstenite`                                        | rejected | forces an async runtime on a small daemon, and a larger tree still                                                                                                                                                                   |
| a WebSocket trait the embedder implements                               | rejected | moves the RFC 6455 work to every embedder, and the crate could not test the binding end to end                                                                                                                                       |
| the crate binds and accepts itself (`Server::bind(addr)`)               | rejected | the embedder decides the address and the access policy                                                                                                                                                                               |
| the codec in its own crate                                              | rejected | two crates to publish for one binding; a feature gives the same `no_std` build                                                                                                                                                       |
| one shared backend fanned out to every connection                       | rejected | one `EventSource` queue and one `Caller` counter would be shared by every client, so a caller's identity could not be the connection (§11.1 item 5); one `Backend` per connection gives that with no extra code                      |
| waking on `Wakeable` instead of a polling tick                          | rejected | a waker cannot interrupt a blocking socket read without a second thread or a platform poller, both of which grow the crate; the tick bounds the latency to `poll_interval`                                                           |
| `Invalid(Detected)` sent as `Live` with the last good value             | rejected | hides an invalidity from the client (D-13)                                                                                                                                                                                           |
| forwarding the backend's `seq`                                          | rejected | a backend counter restarts with a replaced writer and interleaves across writers, so the consumer would discard later publications as stale (D-24)                                                                                   |
| writing the remote provider's frames straight into the ports            | rejected | the ports cannot restore an `Init`, carry a detected-invalid sample, or carry a corrupt occurrence or a remote envelope; the session applies the consumer rules itself (D-29)                                                        |
| a second `Backend`-like trait that implements the ports itself          | rejected | the embedder, not the crate, owns the local ports and their grant; `ProvidedBackend` hands over the deliveries and `Providing<P>` is the default adapter                                                                             |
| a public Rust client                                                    | deferred | not needed by the first consumer, whose client is TypeScript; the test client covers the done-when (driftsys/ridl#989)                                                                                                               |
| accepting the upgrade without a subprotocol and letting the client fail | rejected | a client that does not check would send `attach` on an unnegotiated connection (D-20)                                                                                                                                                |
