# The WebSocket binding — why it is shaped this way

This note records the reasoning behind
[frame specification](../specification/frame-specification.md) §11.1: the
questions the binding had to answer, what was decided, the alternatives that
were considered, and what a wrong choice would cost. The frame specification
states the rules and wins wherever the two differ; this note explains them so
that a later binding version, or a second implementation, does not reopen a
choice without knowing why it was made. Nothing here binds: change a rule in the
specification, and change this note to match.

Two related records own parts of the reasoning and are linked, not restated:
[ADR-0029](../decisions/ADR-0029-stability-policy.md) (what is frozen and how it
is checked) and [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md)
decision 9 (why `Violation.type_name` is an `Option`).

## What is frozen and what is not

From release 0.10.0 the wire surface is frozen: the byte layout of §11.1 for
frame version 1 and binding version 1, the code tables of §4 and §9.6, and the
test vectors. The `wire` freeze dump reads the two vector tables (binding
version 1 and binding version 2) from the specification by its `Id` column and
checks each vector's length, its codes against their tables and its payload
bytes (`xtask/src/freeze/wire.rs`), so an edit to a vector is a break. A wire
format with a promise and no check is only a policy, and the promise is made
before the transport crate exists so that a WebSocket user gets it at 0.10.0.

Not frozen, because they are outside the surfaces ADR-0029 decision 1 lists: the
transport crate `ridl-transport-ws`, the TypeScript WebSocket package, the
`websocket` row of the codegen model (a value derived from the frozen layout, so
a wrong value is corrected in a patch release), the crate's configuration (ping
interval, liveness timeout, message size limit), and the transport's default
encoding.

## Decisions

### One connection is one session; the client consumes

A boundary that carries several catalogs carries several sessions (§3), and a
connection is the cheapest session boundary a WebSocket offers: its close is
unambiguous and every library reports it. Fixing client = consumer removes a
role negotiation from version 1.

The provider sends the pings, and either end treats silence of any kind within
the liveness timeout as the end of the session, because the browser WebSocket
API cannot send a ping and the TypeScript package for the browser is an end of
this binding.

Alternatives: several sessions over one connection with a session id in the
header (adds a field to the closed set of §4 and solves nothing needed now);
either side may consume, so that a device behind NAT can provide to a cloud
consumer (real, but it needs a role handshake). Both can arrive as a later
binding version, additively. Cost if wrong: a provider behind NAT cannot be
reached by this binding.

### `attach` travels in-band, never in the URL

ADR-0018 decision 7 makes the control plane uniform across bindings. Moving
`attach` into the HTTP handshake would make the WebSocket the one binding whose
refusal is not a frame, and the refusal reasons of §6.1 would need a second
mapping to HTTP status codes. §6.1 is therefore the rule of this binding word
for word. Where the provider reads its list of compatible earlier hashes is
already decided by [ADR-0014](../decisions/ADR-0014-ir-encodings.md) decision 15
(the catalog descriptor), so the binding states the source and does not decide
it again.

Alternative: catalog name and hash in the URL, refusal as HTTP 404 or 409. It
fails earlier and needs no frame parsing, but it needs that second mapping. Cost
if wrong: a proxy cannot route by catalog; it can still route by host and path,
which the deployment owns.

### The subprotocol names the binding version; `frame_version` names the frame

The subprotocol is the one place a WebSocket negotiates before any message, so a
peer that does not know the binding fails before `attach` and the failure is
visible to proxies and logs. Two version numbers exist because the frame (what
crosses) and the binding (how the bytes are laid out) can change independently.
Alternatives: no subprotocol, so that a server must parse bytes it cannot read
to refuse them; one version for both, so that every layout change bumps the
frame version and the reverse. Cost if wrong: a second version field that nobody
increments.

### One frame is one binary message, with no length field

The WebSocket message boundary is already a reliable, ordered frame boundary; a
length prefix would duplicate it and give a second place for the two to
disagree. Binary messages keep the payload opaque, as §4 requires. A batch
container would be the first form a binding adds to the frame, and §11 says a
binding adds nothing, so the frames of one commit cross separately and a runtime
over this binding does not present `CoherentSignals`. Alternatives: length
prefixed frames, several per message (a later binding version, where grouping is
a framing rule); text messages with a JSON header (a second header encoding to
keep canonical, and base64 payloads). Cost if wrong: 2 to 14 WebSocket bytes of
overhead per message, recoverable additively.

### Fixed-width little-endian header, `u8` name length

Little-endian is what the FlatBuffers payload already uses on every target in
scope, so one message has one byte order; fixed widths make the header canonical
(§4) with no encoder choices, and a TypeScript end reads it with `DataView` at
fixed offsets. A `u8` form and a `u8` kind are enough for eleven forms and five
kinds; the frame version bumps if either table needs more. `InterfaceNo` and
`Ordinal` keep 32 bits, so the 16-bit fallback of ADR-0021 decision 2 stays
untriggered.

The unit name on `attach` has a `u8` length, at most 255 bytes, so the largest
header has a fixed maximum, stated in
[the frame specification](../specification/frame-specification.md) §11.1, and no
message size limit can refuse a legitimate `attach`. A 16-bit length would
remove that guarantee. A unit name longer than 255 bytes cannot attach; none in
the corpus or the examples comes near.

Alternatives: network byte order (the convention, but the only big-endian part
of a message); a proto3 message as the header (ties the control plane to one
encoding and loses canonical form, since field order is the encoder's); a varint
header (smaller by a few bytes, not fixed-size, more code in a `no_std`
runtime). Cost if wrong: a layout defect found after 0.10.0 is a break of the
wire surface. The vectors are frozen now and the transport crate must reproduce
them.

### Payload presence comes from the header

Whether a frame carries a payload is decided by its `form`, the `provenance` of
a `publish` and the `outcome` of a `response`, never by whether bytes follow the
header. A proto3 value whose fields all hold their defaults encodes to no bytes,
so "no bytes" cannot mean "no payload". The payload then goes to
`Payload::verify` as it is, and a failed decode follows §9.

### The binding fixes no encoding

The transport carries opaque bytes and the face reads its encoding from its port
type, so nothing in the binding needs the tag fixed, and fixing it would make
the transport crate wait for the proto3 codec. A session uses the encoding its
`attach` names; a provider that does not serve it refuses
`encoding_unsupported`. The default a program gets when it names none is the
transport's API default, proto3, which is not frozen. The first crate ships the
FlatBuffers codec only, the one that exists. Cost if wrong: none on the wire,
because `attach` already carries the tag.

### Calls always have an outcome; nothing is retransmitted

Since every command and every query has a response bound
([ADR-0015](../decisions/ADR-0015-qos-absorption-and-rpc-bounds.md) decision 4),
a call on this binding ends in a `response`, in `Undelivered` or `Timeout` at
the bound (§8), or in `Down` when the session ends first (§3). The binding never
retransmits a `request`: a WebSocket is reliable and ordered, and a message is
lost only when the connection fails, which ends the session. The caller's
identity for §7 is the connection and does not survive a reconnect. The binding
therefore adds no acknowledgment or retransmission, which would duplicate what
TCP and the WebSocket close already give. A dead connection that is not yet
detected delays `Down` by the liveness timeout, which is crate configuration.

### A violation crosses as its `Rule` alone

The type name is diagnostic. The contract fact is that the argument broke a
constraint, and the caller already knows the interaction and the argument type
from the correlation. A type identifier would need a type table in the catalog
descriptor, which carries none, for a string nobody acts on; carrying the name
as UTF-8 would not help a Rust runtime, which cannot make a `&'static str` from
received bytes without `alloc` and a leak. Alternatives: a type identifier into
a new descriptor table; a bounded string in a fixed buffer inside `Violation`
(makes a `Copy` error type large); the empty string as a sentinel (a documented
lie in a `&'static str`). A `Rule` variant is added only together with a frame
version, because a receiver cannot construct a variant it does not know. Cost if
wrong: adding the name later is a frame version bump.

### Close codes: standard codes, and an unreadable header ends the session

On a reliable ordered transport, a header the receiver cannot read means the
peers disagree on the binding, and nothing after it can be trusted. §9.5 leaves
teardown to the binding, so the receiver records the frame and closes with 1002.
The refusal reasons already live in `attached`; encoding them again as close
codes would be two places for one fact, so the binding defines no code in 4000
to 4999. Alternative: keep the session open and skip the unreadable message,
rejected because the next message is as likely to be unreadable. Cost if wrong:
a receiver tears down a session a more lenient one would have kept, and the peer
reconnects.

### The sizes

The envelope and header sizes are stated in
[the frame specification](../specification/frame-specification.md) §11.1. The
`attach` is not a data-plane message and enters no size sum. The numbers are
carried by the `websocket` row of the codegen model, added in the pull request
that specified the binding (driftsys/ridl#890), so every deployment section has
them before the transport crate exists. The test that checks the sum against a
frame the transport writes comes with the crate (driftsys/ridl#718).

### Blocks arrive as frame version 2 and binding version 2

A `fixed` of up to 65,280 bytes does not cross in one message: the largest
message of the Rust provider is 16,164 bytes, and a `publish` header takes 27 of
them, so a `fixed` above 16,137 bytes needs a form that carries part of a
payload. That form, `block`, is a new form, so it is a new frame version. A
binding never refuses an interaction the catalog allows (ADR-0030 decision 20),
so the answer is blocks and not a smaller limit. The rules of the form are in
the frame specification §4, §5.5 and §11.1 "Binding version 2"; this section
gives the reasons.

- **4,096 bytes per block, not a whole message.** ADR-0030 decision 16 asks a
  binding to copy at most 4,096 bytes between two scheduling points, so a
  receiver copies one block per message it handles. Block `i` lands at offset
  4,096 × `i`, one page per block. The size does not depend on the provider's
  message limit, so a lowered limit splits the same way and the consumer, which
  does not know that limit, never has to. The cost is 16 messages for the
  largest `fixed` instead of 5, paid once per session because a `fixed` is read
  once.
- **Only a `fixed`, only provider to consumer, only above one message.** Every
  signal, event, command and query fits one message, so splitting them would add
  a reassembly on the provider for no payload that needs it. The provider never
  reassembles. Alternative: blocks for every payload above 4 KiB in both
  directions, rejected and open to a later binding version if a measurement asks
  for it.
- **`total` and `index` in every block, no last-block flag.** The receiver
  learns the size at the first block, so it refuses a payload above the
  catalog's limit before it copies a byte, and a missing block is a sequence
  error and not a wait without a bound. Each block names its interaction, so the
  decoder checks it with no session state. Alternatives: a first block with the
  header and later blocks with an index only (a block that cannot be attributed
  without state); a byte offset in place of `index` (the same information, with
  the block size in two places).
- **A form, not fields on `publish`.** Fields on `publish` would sit on every
  `publish` of every signal. Alternative: blocks as binding framing that splits
  any encoded message, rejected because a fragment does not name its
  interaction, so the size could not be checked against the catalog's limit.
- **Version 1 peers are told why.** The Rust provider serves frame version 2
  only and, for a client that offers `ridl-ws-v1` alone, selects it and answers
  `attached(refused(frame_version_unsupported))`, because a browser reports a
  failed handshake as a close with 1006 and no reason. Alternative: serve both
  versions, rejected because version 1 would have to refuse a large `fixed`.
- **One reassembly per session, and a sequence error ends the session.** The
  provider answers reads one after the other, so nothing needs several
  reassemblies at once. The transport is reliable and ordered, so a missing,
  repeated or reordered block is a sender defect, and dropping the payload would
  leave the `read` waiting. A payload above the catalog's limit is different:
  decision 19 makes the refusal of a size a failure of that interaction's read,
  so the session continues and the remaining blocks are skipped, still checked
  for their sequence.
- **Other frames may come between blocks.** Forbidding it is simpler to state,
  but it would stop a sender from answering a call between two blocks, which is
  what the copy rule of decision 16 asks of a sender on a single-threaded
  executor. The receiver pays nothing, because it holds one reassembly either
  way. The Rust provider as built sends the blocks of one payload in one step.
  Two frames are the exception: a whole `publish` and an `answer` of the `fixed`
  being reassembled. The provider answers the `read`s of one interaction in the
  order it receives them and sends the whole answer of one `read` before the
  next, so such a frame can only be a sender defect, and applying an `answer`
  would settle the next `read` with the payload meant for the first. The
  exceptions compare the kind as well as the numbers, because the consumer keys
  its queues of waiting operations by the kind too, and a frame whose kind
  disagrees with the catalog is a frame of an unknown interaction (frame
  specification §4, §6.4), not a frame of the interaction being reassembled.
- **A `ridl-ws-v1` connection refuses every `attach`.** The Rust provider cannot
  serve version 1, so it refuses an `attach` of frame 1 and also an `attach` of
  frame 2 that arrives on a connection that selected `ridl-ws-v1`. The reason is
  the same `frame_version_unsupported`, and the subprotocol the connection
  selected, not the field alone, decides.
- **The consumer sizes its buffer from the catalog and treats its member table
  as partial.** The generated TypeScript module carries each member's limit, so
  no limit option exists on `connect` and the catalog stays the one source. The
  tables omit members without a finite bound, so the session does not use them
  to decide whether a block names a `fixed`; it reassembles within the limit it
  has and lets the existing `publish` path discard an unsolicited payload.
  Alternative: a second set of identities and a second discard path, rejected as
  more rules for a case only a sender defect reaches.
- **The `Config` check counts a `fixed` as at most one block message.** A
  `fixed` that does not fit one message crosses in blocks of at most 4,130
  bytes, so a message limit of 4,130 bytes carries every payload the catalog
  allows. The check keeps one header size for every member, which refuses up to
  9 bytes more than needed for a `fixed` below one block message; an exact rule
  per kind is a separate change.

Cost if wrong: the block size is a constant of binding version 2, so another
size is another binding version, and version 2 stays as it is.

### Streams arrive as frame version 3 and binding version 3

Streams need new forms (an item, an end, flow-control credit) and a per-element
bound. The subprotocol list plus `frame_version` is a negotiation the binding
already has, so additions need no new mechanism. A client that supports all
three offers `ridl-ws-v3, ridl-ws-v2, ridl-ws-v1`, a server that serves version
2 selects `ridl-ws-v2`, and the client uses no stream form on that session.
Version 1's layout, codes and vectors are never edited, which is what ADR-0029
asks of a frozen surface. A consumer given a contract that declares a stream
over a session of frame version 1 or 2 reports the call through a new
`Transport` variant (additive, because `Transport` is `#[non_exhaustive]`), not
as `UnknownInteraction`, which would misname a disagreement about the catalog.
Alternative: add stream forms to version 1 as forms an old peer discards,
rejected because an unknown form is a protocol violation and an old provider
cannot know the new consumer will never subscribe a stream. Cost if wrong: a
newer peer carries two layouts for a while; no break for the older one. The same
path serves a new `Rule` code, a new form or a new field.

### The vectors are frozen now

The vector table in the specification is the source; no encoder exists to
produce them. Reading the table catches an edit of a frozen vector before 0.10.0
at no cost and after it as a break. Waiting for the transport crate to produce
vectors would check the frozen surface only after the release that promised it.
Cost if wrong: a defect in a hand-written vector becomes the frozen truth until
the crate's round trip finds it, and before 0.10.0 the fix is one line.

## References

- [Frame specification](../specification/frame-specification.md) §10 (what a
  binding decides) and §11.1.
- [ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
  decision 6, the transport rows;
  [ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md)
  decision 7.
- [The catalog descriptor](../design/catalog-descriptor.md), "The compatible
  catalogs".
- Issues driftsys/ridl#265 (the transport crate), #264 (the proto3 codec), #336
  (streams), #350 and #718.
