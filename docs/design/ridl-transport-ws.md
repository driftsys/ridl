# `ridl-transport-ws` — the WebSocket binding of the frame

`ridl-transport-ws` is the Rust side of the WebSocket binding that
[the frame specification](../specification/frame-specification.md) §11.1
defines. It holds two things: a codec for the §11.1 header layout that needs
`core` only, and a server that acts as the **providing side** of a frame session
on one connection the embedder accepted, over a `ridl-rt` port set the embedder
chose. The server forwards payload bytes without decoding them, and it maps each
client request's `seq` to the correlation its backend `Caller` returned.

[ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
decision 6 names the crate and fixes `ridl-rt` as its only unconditional
dependency, with `ridl-descriptor` under the `descriptor` feature (amended
2026-10-19: the row said "`ridl-rt` and a WebSocket crate"; amended 2026-10-24:
the `descriptor` feature). ADR-0020 decision 7 and
[ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md) decision
7 place the transport;
[ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) fixes the port
contracts the server reads and calls. The wire rules are the frame specification
§4 to §9 and §11.1 (binding version 1).

This is the architecture as built. The code and its tests are the source of
truth; where this record and the code differ, this record is corrected. The
decisions below keep their numbers D-1 to D-25, because comments in
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

- Encodes and decodes every one of the ten forms of frame specification §4
  exactly as §11.1 item 2 lays them out, and reproduces every test vector of
  §11.1 byte for byte.
- Serves the observe-and-call ports — `SignalReader`, `EventSource`, `Caller` —
  plus `FixedReader` for a `read` of a `fixed`, over WebSocket.
- Detects changed signals with `ScannableSignals` when the backend has it, and
  by reading each subscribed signal when it does not.

**What it does not do.** It serves no stand-in port (`SignalWriter`,
`EventSink`, `Handler`; see "Adding the stand-in ports later"). It ships no
public Rust client. It has no TLS and authenticates no one. It reads a catalog
descriptor only under the `descriptor` feature, and does no I/O: the embedder
reads or embeds the file.

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

| Module                              | Needs                   | Feature           | Holds                                                         |
| ----------------------------------- | ----------------------- | ----------------- | ------------------------------------------------------------- |
| `frame`                             | `core` only, no `alloc` | always            | the §11.1 header codec: `Frame`, `decode`, `encode`           |
| `ws` (private)                      | `std`                   | `server`, default | the server side of RFC 6455: handshake, message framing, ping |
| `server`                            | `std`                   | `server`, default | the session core, the backend trait, `Config` and `serve`     |
| `descriptor` (private, in `server`) | `std`                   | `descriptor`, off | `MemberTable::from_descriptor` over `ridl-descriptor`         |

`cargo add ridl-transport-ws --no-default-features` gives the codec alone, with
`ridl-rt` at its default features (`no_std`). The `server` feature turns on
`std` for this crate and nothing else; it turns on no `ridl-rt` feature. The
crate root re-exports `serve`, `Backend`, `Config`, `Connection`, `Described`,
`Ended`, `LimitTooSmall`, `MemberTable`, `Note`, `Polling`, `RetainedTooSmall`,
`Scanning` and `TableMismatch` under that feature, and `DescriptorError` under
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
constants `FRAME_VERSION` (1), `SUBPROTOCOL` (`ridl-ws-v1`), `MIN_MESSAGE_LIMIT`
(294, the largest `attach`) and `ENVELOPE_BYTES` (16), and `encoding_tag`, which
maps an encoding name to the tag of §3: `flatbuffers` 1, `proto3` 2, `repr-c` 3.

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
  message; a `name` that is not valid UTF-8.
- `DecodeError::Discard(_)`: a `publish` or a `response` whose header says it
  carries no payload and which has bytes after the header. A consumer records it
  and drops it, and its session continues; the server, which receives neither
  form, closes 1002 (D-12).

An `attach` whose encoding tag is outside §3's table (0, or 4 and above) is a
code outside its table, so it is `Violation(UnknownCode)`, as §11.1 item 2 reads
literally. A tag in the table that the backend does not serve is a session
matter, answered `refused(encoding_unsupported)` (D-11). A future fourth
encoding needs an ADR and a frame version anyway.

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

**D-6. Every form in both directions.** The codec encodes and decodes all ten
forms, whichever side calls it. The server uses half of each, and the test
client uses the other half. "Adding the stand-in ports later" depends on this.

## The server

The server is `crates/ridl-transport-ws/src/server.rs` and its modules
`backend.rs`, `config.rs`, `session.rs` and `calls.rs`.

**D-7. The embedder owns sockets and threads.** The public entry point is one
blocking function, `serve(conn, backend, &config) -> Ended`. The embedder binds
a listener, applies its access policy to each accepted connection, chooses a
thread, builds one `Backend` value for the connection, and calls `serve`.
`serve` returns when the session ends, with why. The crate rustdoc carries an
embedding example. Nothing in the crate calls `bind`, `listen`, `spawn` or
`connect`.

`Connection` is `Read + Write` plus three methods: `set_read_timeout`,
`set_write_timeout`, and `shutdown_write`, whose default does nothing. It is
implemented for `std::net::TcpStream` and, on Unix,
`std::os::unix::net::UnixStream`. `set_write_timeout` is required, not provided,
so that no embedder's connection is silently unbounded: `serve` sets the write
timeout to `liveness_timeout`, and a write that fails with `WouldBlock` or
`TimedOut` ends the session as `Ended::LivenessTimeout`, because the client has
stopped taking messages for the liveness period. The timeout bounds each write
call, not a whole message: a client that keeps reading, however slowly, is not
ended. A zero `poll_interval` or `liveness_timeout` is refused by the stream's
setter, and `serve` returns `Ended::Io(InvalidInput)`.

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
`(InterfaceNo, Ordinal)` to `(Kind, Option<Duration>)`, the duration being the
member's `max`. A retired interface and a reserved ordinal have no entry (§6.4).
The embedder builds it through `new` and `insert`, or, with the `descriptor`
feature, with `MemberTable::from_descriptor` from the bytes of a
`<unit>.catalog.binfb` file. `from_descriptor` verifies the buffer with
`ridl_descriptor::verify`, adds one entry for each member, with its `max_us` as
the bound, and adds each hash of the descriptor's `compatible` list; it returns
`DescriptorError` for a buffer that fails verification, a hash that is not 32
bytes, an interface number or an ordinal of 0, and a `max_us` that is not a
decimal `u64`.

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

- `frame_version` not 1: `refused(frame_version_unsupported)`;
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
  `needed` bytes, up to `max_message_bytes` minus the 27-byte `publish` header;
  a larger value is not sent, and is recorded as `Note::ValueTooLarge`. The
  signal stays subscribed and is tried again on its next change.

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

An occurrence larger than `max_message_bytes` is dropped and noted as
`ValueTooLarge`, and the session `seq` of its channel is advanced, so the client
sees a gap and records a loss (§5.2) instead of a silent drop. Because the
backend subscribes before the `answer` is sent and nothing is drained until the
next tick, no occurrence reaches the client before its `answer`.

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

- A settled reply larger than `max_message_bytes` is recorded and forgotten with
  no frame, so the client's own bound reports `Timeout`; `corrupt` would blame
  the arguments. A `request` with `seq` 0 is recorded and dropped.
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
  member's bound from the member table (D-10), taken only when the table holds
  the request's identity as a command or query under the request's `kind`: a
  request the table refuses (D-12) takes no bound from a member of another kind
  and is kept as one of the last 64; the `min` keeps §7's "measured from the
  request's `stamp`" when the client's clock is behind, and caps the retention
  when a client stamps ahead, so stamping requests in the future holds no
  response. Expired responses go on each tick. The kept responses are indexed by
  `seq` and, for those with a bound, by the bound, so a repeat and an expiry
  pass cost no scan of the kept set (up to the cap, a session may keep hundreds
  of thousands of small responses). For a member the table gives no bound (a
  catalog built before the default response bound), or with no table, the server
  keeps the last 64 as before: a stated deviation from §11.1 item 6
  (driftsys/ridl#986, ruled a deviation of this crate on 2026-10-23). The
  retained bytes of a session are capped by `Config::max_retained_bytes` (16 MiB
  by default), and a session never applies a cap below `max_message_bytes`; when
  a response makes the kept bytes pass the cap, the response is sent, then the
  session closes with 1008, which `serve` returns as
  `Ended::ServerClosed(1008)`. The cap applies to the last 64 as well, so a
  server with no table, or one whose members have no bound, closes with 1008
  when its replies are large enough to pass the cap before the ring holds 64 (at
  the default 1 MiB message limit, 64 replies would be 64 MiB against the 16 MiB
  default cap); an embedder with such replies sets `max_retained_bytes` to 64
  times its message limit, which bounds the ring on its own. Expired responses
  also go when a `request` arrives, before its `seq` is placed. Closing is
  observable, where dropping the oldest response would silently break item 6.

**D-17. `fixed`.** A `read` of a `fixed` calls `Backend::read_fixed` and sends
one `publish` with `kind` `Fixed`, state `Live` with the value, `seq` 0, and
`stamp` = `Clock::now()`. An error `ReadError::Contract(c)` is answered
`contract(c)`. A `subscribe` of a `fixed` is answered
`contract(UnknownInteraction)` (D-12). Through `Polling`, every `read` of a
`fixed` is answered `contract(UnknownInteraction)`.

### The end of a session

**D-18. The session's end.** At every end, in-flight backend calls are
forgotten, and every backend subscription is dropped with the `Backend` value;
then the connection is dropped. A backend port that reports `Detached` from any
call ends the session with close 1001, because the provider is going away.
`serve` returns an `Ended`:

| `Ended`              | When                                                                                                                                                   |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `ClientClosed(code)` | the client sent a close frame; the server answered with the same code, or 1000 when the client's carried none                                          |
| `ServerClosed(code)` | the server sent a close frame (D-23)                                                                                                                   |
| `Rejected`           | the handshake was answered `400` or `403` (D-19, D-20)                                                                                                 |
| `LivenessTimeout`    | the handshake did not finish within `liveness_timeout`, no message arrived for that long after it, or a write waited that long; no close frame is sent |
| `Io(kind)`           | a read or a write failed; `UnexpectedEof` is a client that closed the connection without a close frame                                                 |

The embedder may also receive `Note` values through `Config::with_record`; with
no hook nothing is recorded and recording costs nothing. A `Note` is one of:
`Violation` (closes 1002), `MessageBeforeAttach` (closes 1002), `SecondAttach`
(closes 1002), `UnexpectedDirection` (closes 1002), `KindNotAdmitted` (an
`unsubscribe` of a kind the form does not admit; dropped), `ValueTooLarge`,
`DuplicateDropped`, `CallAbandoned`, `RuleNotCarried` (a violation's rule the
frame has no code for; the outcome is replaced) and `UnknownInteraction` (an
interaction the member table does not hold; dropped or answered as D-12 says).

### `Config`

`Config` (`server/config.rs`) has four public durations — `poll_interval` (20
ms), `ping_interval` (5 s), `liveness_timeout` (15 s) and `call_abandon_after`
(60 s) — and four settings behind methods: `with_max_message_bytes` (1 MiB; a
value below `MIN_MESSAGE_LIMIT` is refused with `LimitTooSmall`),
`with_max_retained_bytes` (16 MiB; a value below `max_message_bytes` is refused
with `RetainedTooSmall`, and a session applies the larger of the cap and the
message limit, so a limit raised later also raises the cap), `with_record` and
`with_origin_check`. A `ping_interval` at or above `liveness_timeout` ends an
idle client's session before the first ping, because a client that is not pinged
sends nothing; the rustdoc says so.

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
the version the server speaks (RFC 6455 §4.2.2 item 4). The path, the query and
other headers are ignored (§11.1 item 1: the binding places nothing there; the
embedder's access policy runs before `serve`).

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

**D-20. The subprotocol.** The server selects `ridl-ws-v1` when the client
offers it, in any position of `Sec-WebSocket-Protocol`. When the client does not
offer it, the server answers `400 Bad Request` and does not upgrade. §11.1 says
that the client fails a connection on which the server selected none; answering
400 makes the failure visible at once, also to a client that does not check. The
selection is a single comparison with `ridl-ws-v1`: a table from subprotocol
name to session role would have one row, so it is left until the reversed role
of "Adding the stand-in ports later" adds the second.

**D-21. Messages.** Binary messages only; a text message closes with 1003.
Fragmented messages are reassembled, also when a frame is split across socket
reads. A data message whose total size passes `max_message_bytes` closes with
1009 as soon as the length is known, before it is buffered; the limit applies to
data messages, not to control frames, which RFC 6455 caps at 125 bytes.
`max_message_bytes` is at least 294 (§11.1 "The sizes"). Client frames that are
not masked close with 1002 (RFC 6455 §5.1). A ping is answered with a pong
carrying its payload.

Each frame the server sends, header and payload, is assembled in one buffer the
session reuses and written with one write call, so that with `TCP_NODELAY` off
the payload does not wait for the peer's delayed acknowledgement of the header.
The buffer keeps the capacity of the largest frame sent, which
`max_message_bytes` bounds (1 MiB by default per idle session after a large
message).

**D-22. Liveness.** The server sends a ping every `ping_interval` and ends the
session when no message of any kind has arrived for `liveness_timeout`. A
browser answers a ping automatically, so 5 s pings detect a dead client in 15 s
at a cost of one small frame per 5 s.

**D-23. Close codes.** Exactly §11.1 item 4's: 1000 on purpose and after
`attached(refused)`, 1001 when the backend detaches, 1002 on a protocol
violation, 1003 on a text message, 1009 on an oversized message, and 1008 when
the retained responses pass `max_retained_bytes` (D-16). The server answers a
client's close frame with a close frame that carries the same code. After every
close frame it sends, the answer to the client's included, and after a handshake
`400` or `403`, the server shuts down its write side and lingers for up to 100
ms, reading and discarding what the client still sends, so that the close frame
or the answer reaches the client instead of being lost to a reset of the
connection. It waits no longer than that for the client to close the connection
(RFC 6455 §7.1.1 has the client wait for the server to close it).

## Tests

All under `crates/ridl-transport-ws/`.

- **The vectors** (`tests/vectors.rs`). The test reads the §11.1 test-vector
  table out of `docs/specification/frame-specification.md`, decodes each `Hex`
  cell, checks that it decodes to the frame its `Vector` cell describes, and
  that encoding that frame gives the same bytes. It fails on a row whose id it
  does not know, so a new vector cannot be skipped. Reading the table from the
  specification, instead of copying it, means the test cannot drift from it.
- **Every code** (`tests/codes.rs`). Every code of §4 and §9.6, written out as
  bytes by hand and checked in both directions, so a wrong code constant cannot
  cancel itself out in a round trip.
- **The binding sizes** (`tests/sizes.rs`, driftsys/ridl#718). For payload
  lengths 0, 1 and 255, a `response` under `reply` encodes to exactly
  `frame_header_max_bytes + envelope_bytes + len` bytes, read from the
  `websocket` row of `ridl_ir::codegen::known_bindings()`, and the row's version
  is the binding version 1; every other data-plane form encodes to no more; an
  `attach` with a 255-byte name is 294 bytes. `known_bindings()` is a public
  function added to `ridl-ir` for this test, because the only other public route
  to the row needs a whole lowered system.
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

## Adding the stand-in ports later

The portal will later need to stand in for a component: publish signals, raise
events and answer calls into the system, through `SignalWriter`, `EventSink` and
`Handler` (driftsys/ridl#988). On the frame, those are the **providing** side of
a session, and §11.1 item 1 makes the WebSocket client the consumer. So a
stand-in client is a session in the other direction: the WebSocket client
provides, and the server consumes and forwards what it receives into the
backend's `SignalWriter`, `EventSink` and `Handler`. The server sends `attach`,
`subscribe` and `request`; the client sends `attached`, `publish`, `occurrence`
and `response`. All ten forms exist already, so the frame does not change. What
changes is the binding's item 1, by a new subprotocol name that selects the
reversed roles, offered beside `ridl-ws-v1`, in an amendment of §11.1. The name
is chosen with that amendment.

What the crate already does so that the addition is local:

- the codec encodes and decodes every form in both directions (D-6);
- the subprotocol selection is one comparison, which the reversed role replaces
  with a table from name to session role (D-20);
- the `Backend` trait is the providing role's alone; the reversed role gets its
  own trait over `SignalWriter + EventSink + Handler`, so neither role's
  embedder implements ports it does not serve.

## What is not built

- The stand-in ports (driftsys/ridl#988).
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
| reading the catalog descriptor to check `kind` and bounds               | built    | under the `descriptor` feature, off by default, so an embedder that passes no table links no planus (driftsys/ridl#985)                                                                                                              |
| a public Rust client                                                    | deferred | not needed by the first consumer, whose client is TypeScript; the test client covers the done-when (driftsys/ridl#989)                                                                                                               |
| accepting the upgrade without a subprotocol and letting the client fail | rejected | a client that does not check would send `attach` on an unnegotiated connection (D-20)                                                                                                                                                |
