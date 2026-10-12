# The WebSocket binding — design (2026-10-12)

Status: **draft for review.** Lane W, step 1 of the release 0.10 lanes plan
([`2026-10-11-release-0.10-lanes-plan.md`](2026-10-11-release-0.10-lanes-plan.md)
section 4, "Lane W"), under release spec decisions R-1 and R-3
([`2026-10-11-release-0.10-design.md`](2026-10-11-release-0.10-design.md)). The
maintainer delegated every decision of this lane; each ruling below records its
reason, the alternatives considered, and its cost if wrong. Rulings are numbered
from R-W-7, because R-W-1 to R-W-6 are taken.

Nothing in this note is applied yet. Section 3 holds the exact text to put into
`docs/specification/frame-specification.md` and the other records; section 4
says what the `wire` freeze dump gains.

## 0. Reading basis

Read from the worktree at `0ea3d971` (`origin/main`, 2026-10-12):
`docs/specification/frame-specification.md` (0.1.0 Draft, §3, §4, §6.1, §7,
§9.5, §10, §11.1, §13); `docs/design/catalog-descriptor.md` ("The compatible
catalogs", "No runtime reads the list"); the archived lane H design
`docs/archive/2026-10-09-catalog-compat-design.md` (questions 3 to 5);
`docs/wip/2026-10-06-ws-and-payload-formats-steering.md` (decisions B, F, G, H,
I); ADR-0014 decision 15 as amended 2026-10-09 and 2026-10-10; ADR-0015 decision
4 as amended (every call has a response bound); ADR-0020 decision 6; ADR-0021
decisions 2 and 5; ADR-0029 (the wire surface, decisions 1 to 3 and 6);
`crates/ridl-ir/src/codegen/bindings.rs` (`Known`, `KNOWN` empty);
`crates/ridlc/tests/layout.rs` (the synthetic 14 + 2 row);
`xtask/src/freeze/wire.rs` and `freeze/wire.txt`; issues #350 (items 5 and 12),
#718, #336, #802.

## 1. Goal and non-goals

**Goal.** Write the WebSocket binding so that the wire surface can freeze at
0.10.0 with a WebSocket promise in it, and so that `ridl-transport-ws` (#265)
ships in a 0.10.x release as an addition that changes no frozen surface.
Concretely: amend frame specification §11.1 into a complete binding (the nine
items of §10), rule on §6.1 for `attach` over this binding, settle #350 items 5
and 12, state the header and envelope sizes #718 asks for, and leave the frame
open to streams (#336) in 0.11.

**What 0.10 freezes** (ADR-0029 decision 1, "Wire format"): the frame envelope
and the `attach` handshake, which after this note means the byte layout of
section 3.1 for frame version 1 and binding version 1, the code tables, and the
test vectors of section 3.1. The inputs of the catalog hash, the `.proto` and
`.fbs` mapping and the encoding names are already in the `wire` dump and are not
changed here.

**What 0.10.x adds** (not frozen, ADR-0029 decision 2): the crate
`ridl-transport-ws`, the TypeScript WebSocket package, the `websocket` row's
numbers in the codegen model (a value derived from the frozen layout, R-W-16),
the crate's configuration (ping interval, liveness timeout, message size limit),
and plain `connect` (proto3 default) when #264 lands.

**Non-goals.** The transport code; the proto3 codec; a relay or a bridge; a
session multiplexed over one connection; authentication beyond what TLS and the
deployment provide (frame specification §13); streams themselves (only the path
to them is ruled, R-W-18).

## 2. Rulings

### R-W-7 — The transport and the session (§10 item 1)

**Question.** What is a session on a WebSocket; who is the consumer; what ends
the session.

**Decision.** One WebSocket connection carries exactly one session. The side
that opens the connection (the WebSocket client) is the consumer and sends
`attach` as its first message; the side that accepts (the server) is the
provider. The URL is the deployment's: the binding puts nothing in the path, the
query or a header, and `ws://` and `wss://` are both allowed. The session ends
when either side sends a close frame, when the TCP connection fails, or when the
liveness check fails: the provider (the server end) sends WebSocket pings, and
either end treats "no message of any kind, data or control, within the liveness
timeout" as the end of the session. The provider pings because the browser
WebSocket API cannot send a ping, and the TypeScript package for the browser is
a named end of this binding. At the end of the session every port of the
consuming runtime reports `Detached` and every call in flight reports
`Transport::Down` (§3). A consumer that needs two catalogs opens two
connections.

**Reason.** §3 already says a boundary carrying several catalogs carries several
sessions, and a connection is the cheapest session boundary a WebSocket offers:
its close is unambiguous and every library reports it. Fixing client = consumer
removes a role negotiation from version 1.

**Alternatives.** (a) Several sessions over one connection with a session id in
the header: adds a header field to the closed set of §4 and solves nothing
needed now; it can arrive as binding version 2. (b) Either side may consume (a
device behind NAT that provides to a cloud consumer must dial out): real, but it
needs a role handshake; left to a later binding version, additive. (c) The
catalog name and hash in the URL so that a refusal is an HTTP 4xx: rejected,
R-W-12.

**Cost if wrong.** A provider behind NAT cannot be reached in 0.10.x; the fix is
an additive binding version, not a break.

### R-W-8 — The subprotocol and its version (§10 item 2, versioning)

**Question.** How do the two ends agree on the binding, and how does the binding
version relate to `frame_version`.

**Decision.** The client offers `ridl-ws-v1` in `Sec-WebSocket-Protocol`; the
server selects it, or the client fails the connection and the session never
opens (a client that can choose a close code uses 1002; a browser client
cannot). The number in the subprotocol name is the binding version: the byte
layout of section 3.1. `attach.frame_version` stays the frame specification's
version of the logical frame, `1`. Binding version 1 carries frame version 1
only, so on a `ridl-ws-v1` connection an `attach` whose `frame_version` is not 1
is refused `frame_version_unsupported`. A later binding version is offered as a
list, newest first (`ridl-ws-v2, ridl-ws-v1`), and the server picks the newest
it serves; see R-W-18 for how that carries frame version 2. The codegen model's
`Binding.version` is the binding version, `"1"`.

**Reason.** The subprotocol is the one place a WebSocket negotiates before any
message, so a peer that does not know the binding fails before `attach` and the
failure is visible to proxies and logs. Two version numbers exist because the
frame (what crosses) and the binding (how the bytes are laid out) can change
independently; naming both keeps each change small.

**Alternatives.** (a) No subprotocol, version only in `attach`: a server that
does not know the binding must then parse bytes it cannot read to refuse them.
(b) One version for both: every byte-layout change would bump the frame version
and every frame change the layout; rejected because `frame_version` already
exists in §6.1 and is transport-neutral.

**Cost if wrong.** A second version field nobody increments; no wire break.

### R-W-9 — Framing: one frame per binary message (§10 item 4, item 7)

**Question.** Where does one frame end on a WebSocket; binary or text; how is
the payload length carried; what about a commit's frames.

**Decision.** One frame of §4 is one WebSocket message with the binary opcode.
The header is the fixed-order prefix of section 3.1 and the payload is every
byte after it, so no length field is carried: the payload length is the message
length minus the header length. WebSocket fragmentation is below the binding and
invisible to it. A text message is a protocol violation (close 1003). No
grouping: the frames of one `SignalWriter::commit` cross as separate messages,
so a runtime over this binding does not present `CoherentSignals` (§5.1 permits
that). The rate-floor answer on a call (§10 item 9) is §8's `busy`, and the
binding adds nothing.

**Reason.** The WebSocket message boundary is already a reliable, ordered frame
boundary; adding a length prefix would duplicate it and give a second place for
the two to disagree. Binary messages keep the payload opaque, as §4 requires. A
batch container would be the first form this binding adds to the frame, and §11
says a binding adds nothing.

**Alternatives.** (a) Length-prefixed frames, several per message (batching, and
a `CoherentSignals` unit): deferred to a later binding version where the
grouping is a framing rule, not a frame change. (b) Text messages with a JSON
header: a second header encoding to keep canonical, and base64 payloads.

**Cost if wrong.** Per-message overhead of 2 to 14 WebSocket bytes on every
signal publication; recoverable additively.

### R-W-10 — The header byte layout (§10 item 2)

**Question.** Byte order, widths, field order and the code tables.

**Decision.** Every header integer is fixed-width little-endian; `stamp` is a
two's-complement `i64`; no varint, no padding, no alignment. The field order and
the sizes are in section 3.1. The numeric codes (form, provenance, outcome,
detail, `Rule`, refusal reason) are frame-level facts, stated once in §4 and
§9.6 beside the `Kind` values and the encoding tags that §2 and §3 already fix,
as the values every binding that carries codes uses; §11.1 references them and
restates none. `InterfaceNo` and `Ordinal` keep 32 bits, so the `u16` fallback
of ADR-0021 decision 2 stays untriggered and §4's range check at `attach` is
vacuous for this binding. The unit name on `attach` is UTF-8 with a `u8` byte
length: at most 255 bytes, never empty, valid UTF-8, so the largest `attach`
header is 39 + 255 = 294 bytes and the handshake has a fixed maximum (R-W-16).

**Reason.** Little-endian is what the FlatBuffers payload and a `repr(C)`
payload already use on every target in scope, so one message has one byte order;
fixed widths make the header canonical (§4) with no encoder choices, and a
TypeScript end reads it with `DataView` at fixed offsets. A `u8` form and a `u8`
kind are enough for ten forms and five kinds, and the frame version bumps if
either table ever needs more.

**Alternatives.** (a) Network byte order: the convention for protocols, but it
would make the header the only big-endian part of the message. (b) A proto3
message as the header: ties the control plane to one encoding, which R-W-11
forbids, and loses canonical form (field order is the encoder's). (c) A varint
header: smaller by a few bytes, not fixed-size, more code in a `no_std` runtime.

**Cost if wrong.** A layout defect found after 0.10.0 is a break of the wire
surface and needs 0.11. Mitigation: the vectors of section 3.1 are frozen now
and #265 must reproduce them; a defect found before 0.10.0 is free.

### R-W-11 — The encoding tag (§10 item 3)

**Question.** Does the WebSocket binding fix the payload encoding, as §11.1 does
today (proto3, tag 2)?

**Decision.** No. The binding carries the encoding the session names at `attach`
and fixes none (steering decision B). A provider that does not serve the named
tag refuses `encoding_unsupported`. The default a program gets without naming
one is the transport's API default, proto3 (steering decision I: `connect(url)`
is proto3, `connect_with::<E>(url)` overrides), and that default is not frozen
(ADR-0029 decision 2, "transport default"). The first crate ships
`connect_with::<FlatBuffers>` only, the codec that exists.

**Reason.** The transport carries opaque bytes and the face reads its encoding
from its port type (`ridl_rt::port::Encoded`), so nothing in the binding needs
the tag fixed; fixing it would make #265 wait for #264.

**Alternatives.** Keep proto3 fixed: a WebSocket user has no transport until the
proto3 codec lands.

**Cost if wrong.** None on the wire: `attach` already carries the tag.

### R-W-12 — §6.1 and the `attach` hash over this binding

**Question.** Lane H replaced §6.1's exact-match rule with "the provider's own
hash or one of the hashes its descriptor lists as compatible". What does the
WebSocket binding do with it, where does the provider get the list, and where
does `attach` travel?

**Decision.** §6.1 as amended by lane H is the `attach` rule of this binding,
word for word; the binding adds no check and removes none. The provider's source
of the list is the unit's catalog descriptor (`Catalog.compatible`,
`docs/design/catalog-descriptor.md`), read once at start; a provider that holds
no descriptor applies the exact-match rule, which §6.1 already states is the
rule with an empty list. The first `ridl-transport-ws` provider holds only the
generated `CATALOG` and is in that case: it applies exact match, and so it does
not meet conformance item 1 of §12 (an `attach` naming a listed earlier hash is
accepted) until it reads a descriptor; the As-built note of the frame
specification says so. `attach` travels in-band as the first binary message
after the HTTP upgrade, never in the URL or a header, and `attached` is the
first message the provider sends; a provider that receives any other first
message, or a second `attach`, closes with 1002. After `attached(refused)` the
provider closes with 1000; the reason is in the frame.

**Reason.** ADR-0018 decision 7 makes the control plane uniform across bindings;
moving `attach` into the HTTP handshake would make the WebSocket the one binding
whose refusal is not a frame. The list's location is already decided (ADR-0014
decision 15 rejected a constant on the face), so the binding states where a
provider reads it rather than deciding anew.

**Alternatives.** (a) Catalog name and hash in the URL path, refusal as HTTP
404/409: earlier failure and no frame parsing, but the reason set of §6.1 would
need an HTTP mapping and a second specification. (b) The consumer sends its
compatible list too: rejected by lane H (question 4), the direction is wrong.

**Cost if wrong.** A proxy that routes by URL cannot route by catalog; it can
route by host and path, which the deployment owns.

### R-W-13 — #350 item 5

**Item, verbatim.** "A call lost in transport with no response bound has no
outcome (ridl §9.3 allows no bound). — E11.9"

**Decision.** The premise is retired: since ADR-0015 decision 4 as amended
(#749), every command and every query has a response bound, declared or
defaulted (`[..1s]` for a command, `[..3s]` for a query), and both sides read it
from the catalog. On this binding a call therefore always has an outcome: the
`response`; `Transport::Undelivered` (command) or `Transport::Timeout` (query)
when the bound passes with no response (§8); or `Transport::Down` when the
session ends first (§3). The binding never retransmits a `request`, because a
WebSocket is reliable and ordered and a message is lost only when the connection
fails, which ends the session. The caller's identity for §7 is the connection,
and it does not survive a reconnect: a reconnect is a new session with new
counters, and the calls of the old session were already reported `Down`. The
provider keeps a settled response as §7 states, until the request's bound has
passed; §7's "for a call with no response bound, the binding fixes the
retention" is vacuous and the binding states no further retention. A second
`request` with a `seq` already presented on the same connection is a consumer
defect; the provider treats it as §7 states (not presented again; the kept
response re-sent).

**Reason.** Everything here follows from §3, §7 and §8 once the bound exists;
the binding's only contribution is the liveness check of R-W-7, which turns a
silently dead connection into a session end within a bounded time.

**Alternatives.** An application-level acknowledgment or retransmission in the
binding: duplicates what TCP and the WebSocket close already give.

**Cost if wrong.** A connection that is dead but not detected delays `Down` by
the liveness timeout; that value is the crate's configuration.

### R-W-14 — #350 item 12

**Item, verbatim.** "`Violation.type_name` is `&'static str`, so a `no_std`
runtime cannot fill it from an error a remote peer sends, unless the frame
carries a type identifier the receiver looks up in the shared catalog. — E11.1"

**Decision.** The type name does not cross. On a `response` whose outcome is
`contract(InvalidValue(violation))` (the one form that carries it: an `answer`
never does, §6.2 and §6.3), the frame carries the `Rule` as a one-byte code
(§9.6, section 3.1) and nothing else of the violation. The receiving runtime
reports `Contract::InvalidValue(Violation { type_name: None,
rule })`:
`Violation.type_name` becomes `Option<&'static str>`, `Some` when the violation
was detected locally by the generated binding and `None` when it was reported by
a peer. This is a breaking `ridl-rt` change, made in lane W step 2 with the
other #350 rulings, before the `runtime-crates` gate is switched on; the Kotlin
plugin receives a heads-up. A new `Rule` variant adds a code to the table and is
a frame version bump (R-W-18), because a receiver cannot construct a variant it
does not know; the ADR-0021 amendment states that a `Rule` variant is added only
together with a frame version, although `Rule` is `#[non_exhaustive]`
(`crates/ridl-rt/src/payload.rs`).

**Reason.** The type name is diagnostic: the contract fact is that the argument
broke a constraint, and the caller already knows which interaction and which
argument type from the correlation. A type identifier would need a type table in
the catalog descriptor, which carries none (D-7: no payload layouts), for a
string nobody acts on. Carrying the name as UTF-8 would not help a Rust runtime,
which cannot make a `&'static str` from received bytes without `alloc` and a
leak.

**Alternatives.** (a) A type identifier into a new descriptor table: a schema
change and a lookup on every error for a diagnostic. (b) The name as a bounded
string in a fixed buffer inside `Violation`: makes a `Copy` error type large.
(c) The empty string as a sentinel: a documented lie in a `&'static str`.

**Cost if wrong.** A caller that wants the type name must read the provider's
log; adding the name later is a frame version bump.

### R-W-15 — Close codes and unreadable frames (§9.5)

**Decision.** The binding uses the standard close codes and defines no code in
4000 to 4999: 1000 when either side ends the session on purpose (detach, or the
provider after `attached(refused)`); 1001 when the provider shuts down; 1002 for
a protocol violation (a message shorter than its header, an unknown form, kind,
provenance or outcome code, a field out of its domain such as `interface` 0, a
message before `attach`, a second `attach`, an `attach` whose `name_len` does
not match the message length or whose name is empty or not valid UTF-8, a first
provider-to-consumer message that is not `attached`); 1003 for a text message;
1009 for a message larger than the receiver's configured limit, which admits at
least the largest `attach` (294 bytes). §9.5 says an unreadable header is
"discarded and recorded, never partly applied" and leaves teardown to the
binding: here the receiver records the frame and closes with 1002, so the
recording obligation is kept. A frame with a readable header and an absent
payload where the form requires one is answered `corrupt` when it is a
`request`, as §9.5 states, and discarded and recorded otherwise; the session
continues.

**Reason.** On a reliable ordered transport a header the receiver cannot read
means the peers disagree on the binding, and nothing after it can be trusted;
§9.5 leaves teardown to the binding, and teardown is the honest answer. The
refusal reasons already live in `attached`, so a second encoding of them as
close codes would be two places for one fact.

**Alternatives.** Keep the session open and skip the unreadable message: with no
length prefix inside the message nothing is lost by skipping, but the next
message is as likely to be unreadable; rejected for clarity.

**Cost if wrong.** A receiver tears down a session a more lenient one would have
kept; the peer reconnects.

### R-W-16 — The header and envelope sizes (#718)

**Decision.** The numbers, from section 3.1: `envelope_bytes` = 16 (`stamp` 8 +
`seq` 8); `frame_header_max_bytes` = 20, the largest data-plane header without
its envelope (`response`: 10 common + 8 `correlation` + 2 `outcome`). A socket
message's maximum size is therefore 36 + the payload bound (a `publish` is 27 +
bound, a `request` 26 + bound; the single row is the conservative maximum, as
the `Binding` message shape requires). The largest header of any message is the
`attach` at 39 + 255 = 294 bytes; it carries no payload and enters no size sum,
so the `Known` row stays the data-plane maximum, and the doc comment of
`Known.frame_header_max_bytes` in `crates/ridl-ir/src/codegen/bindings.rs`
(today "the largest frame header the binding adds to a message") is amended in
the same pull request to say "data-plane message". §11.1 states that a
receiver's message size limit admits at least 294 bytes, so no limit can refuse
a legitimate `attach`. The numbers live in two places: the frame specification
§11.1 states them (the source), and `KNOWN` gains the row
`{ name: "websocket", version: "1", frame_header_max_bytes: Some(20),
envelope_bytes: Some(16) }`
in the same pull request as the §11.1 amendment, so every deployment section
carries it (#718's first half). This changes steering decision H, which had #265
fill the `KNOWN` row and #718's row: the row moves from #265 to the amendment
pull request, because the document that defines the numbers exists before the
crate. The layout test's synthetic 14 + 2 row is replaced by the real row and
the cabin layout fixture's `max_message_bytes` becomes non-null for the
proto3-bounded members. The test that checks the sum against a frame the binding
writes lands with #265 (#718's second half; #718 stays open until then). The row
is a value derived from the layout: if it is wrong it is corrected in a patch
release, while the layout it is derived from is frozen through the vectors
(R-W-19).

**Alternatives.** Add the row with #265 only (steering decision H as written):
the deployment section stays without a binding for the whole 0.10.0 cycle
although the document that defines the numbers exists. A `u16` name length: the
handshake would have no fixed maximum, and a receiver's 1009 limit could refuse
a legitimate `attach`.

**Cost if wrong.** A plugin sizes a socket buffer from a wrong constant; the row
is a derived value and is corrected in a patch release. A unit name longer than
255 bytes of UTF-8 cannot attach; no unit name in the corpus or the examples is
near that length.

### R-W-17 — The frame specification's version

**Decision.** The amendment of section 3 makes the document version 0.2.0 and
keeps "Draft" (lane W step 5 drops it). `attach.frame_version` stays 1: no peer
has ever sent a frame, so version 1 is the first that exists on a wire.

**Cost if wrong.** None; the number is the document's, not the wire's.

### R-W-18 — Streams (#336) arrive in 0.11 without a break

**Question.** Streams need new forms (an item, an end, flow-control credit) and
a per-element bound. How do they arrive in 0.11 while a 0.10 peer keeps working?

**Decision.** Streams arrive as frame version 2 and binding version 2. The 0.11
client offers `ridl-ws-v2, ridl-ws-v1`; a 0.10 server selects `ridl-ws-v1` and
the 0.11 client sends `attach` with `frame_version` 1 and uses no stream form on
that session; a 0.11 server selects `ridl-ws-v2` and a 0.10 client, which
offered `ridl-ws-v1` only, gets `ridl-ws-v1` as before. The version 1 layout,
code tables and vectors are never edited; version 2 is a new section beside
them. A contract that declares a stream (`<T>`) therefore needs frame version 2
on both ends, and a 0.11 consumer of such a contract that is given version 1
reports a call that uses a stream through a new `Transport` variant (additive:
`Transport` is `#[non_exhaustive]`, `crates/ridl-rt/src/error.rs`), not as
`UnknownInteraction`, which would misname a disagreement about the catalog, and
uses the rest of the contract. The same path serves any later addition to the
frame (a new `Rule` code, a new form, a new field).

**Reason.** The subprotocol list plus `frame_version` is a negotiation the
binding already has (R-W-8), so additions need no new mechanism; freezing
version 1 by never editing it is what ADR-0029 asks of a frozen surface.

**Alternatives.** Add stream forms to version 1 as "unknown forms a 0.10 peer
discards": R-W-15 makes an unknown form a protocol violation, and a 0.10
provider cannot know that a 0.11 consumer will never subscribe a stream.

**Cost if wrong.** A 0.11 peer carries two layouts for a while; no 0.10 break.

### R-W-19 — Frame vectors in the `wire` dump now

**Decision.** The `wire` dump gains one line per vector of section 3.1's vector
table, `frame-vector <id> <hex>`, where `<id>` is the table's stable `Id` column
(`attach-1`, `publish-live`, ...), never the caption, read by
`xtask/src/freeze/wire.rs` from the frame specification itself (the table is the
source; no encoder exists), with a structural check that each vector's length
equals the header size of its form plus its payload. The dump gains them in the
pull request that applies section 3, so they are frozen at 0.10.0 and ADR-0029's
"frozen by policy with no automated check" sentence is retired for the envelope
and the handshake. #265's crate must encode and decode every vector in its
tests.

**Reason.** R-3 wants a WebSocket user to get a wire promise in 0.10; a promise
with no check is what ADR-0029 calls policy only. Reading the table from the
document costs about forty lines of `xtask` and catches an edit of a frozen
vector before 0.10.0 at no cost and after it as a break.

**Alternatives.** Wait for #265's encoder to produce the vectors: the frozen
surface would then be checked only in 0.10.x, after the version that promised
it.

**Cost if wrong.** A hand-written vector with a defect becomes the frozen truth;
#265's round trip finds it, and before 0.10.0 the fix is one line.

## 3. The text changes

### 3.1 `docs/specification/frame-specification.md`

**The version line** becomes `Version: 0.2.0 — Draft`.

**The As-built note** gains, after "and no runtime in this workspace reads it at
`attach`": "The first WebSocket provider (driftsys/ridl#265) holds no descriptor
and applies the exact-match rule of §6.1, so it does not meet conformance item 1
of §12 until it reads one."

**§1, the table's binding row.** The "What it fixes" cell becomes: "how the
frame's fields are carried over that transport, and whether the binding carries
any of the three payload encodings or fixes one".

**§1, the paragraph after the table.** Replace "and which encoding a path
carries is the encoding matrix of ADR-0020 decision 2: proto3 serialized into a
stream, FlatBuffers mapped or passed within a node." with:

> and which encoding a session uses is named at `attach` (§3). A binding either
> carries any of the three, the `attach` tag naming the session's, or fixes one;
> the
> [ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
> decision 2 matrix — proto3 serialized into a stream, FlatBuffers mapped or
> passed within a node — gives the default a transport offers when it carries
> any, and the one encoding when it fixes one. §11.1 carries any; §11.2 fixes
> FlatBuffers.

**§2, the `Violation` row.** The meaning becomes: "the `Rule` that failed and,
for a violation detected locally, the name of the typl type whose constraint
failed; a violation a peer reports carries the rule alone (§9.6)".

**§3, the encoding paragraph.** Replace the sentence "The encoding is the one
the path takes under the ADR-0020 decision 2 matrix, so a binding knows it
before the session exists; `attach` carries it so that a mismatch is a refusal
rather than a stream of corrupt payloads." with:

> A binding either carries any of the three encodings, in which case the
> consuming program names one at construction and the transport offers a default
> taken from the
> [ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
> decision 2 matrix, or fixes one (§1, §10 item 3); §11.1 carries any and §11.2
> fixes FlatBuffers. `attach` carries the tag in both cases, so that a mismatch
> is a refusal rather than a stream of corrupt payloads.

**§4, after "The ten forms" table, a new paragraph.** The numeric codes every
binding that carries codes uses; `Kind` (§2) and the encoding tag (§3) are
already fixed:

> **The codes.** A binding that carries a field as a numeric code uses these
> values. `form`: `attach` 1, `attached` 2, `subscribe` 3, `answer` 4,
> `unsubscribe` 5, `read` 6, `publish` 7, `occurrence` 8, `request` 9,
> `response` 10 — the order of the table above. `provenance`: `Init` 1, `Live`
> 2, `Invalid(Declared)` 3. `outcome`: `accepted` 1, `reply` 2,
> `contract(InvalidValue)` 3, `contract(PreconditionFailed)` 4,
> `contract(ContractBroken)` 5, `contract(UnknownInteraction)` 6, `corrupt` 7,
> `busy` 8, `refused` 9. A `refused` reason: `unknown_catalog` 1,
> `catalog_mismatch` 2, `encoding_unsupported` 3, `frame_version_unsupported` 4,
> `range` 5. The `Rule` of an `InvalidValue`: §9.6. A new value in any of these
> tables is a new frame version.

**§4, the `outcome` row.** Append to the row's domain: "§9.6 for what
`contract(InvalidValue)` carries".

**§9.6, a new last paragraph:**

> **What a violation carries.** `contract(InvalidValue(violation))` crosses as
> the violation's `Rule` alone; the name of the typl type whose constraint
> failed does not cross. The `Rule` codes: `Range` 1, `Step` 2, `Length` 3,
> `Pattern` 4, `Variant` 5, `Unique` 6; a new `Rule` variant is added only
> together with a new frame version, because a receiver cannot construct a
> variant it does not know. The receiving runtime reports the violation with no
> type name, which `ridl_rt::payload::Violation` represents as
> `type_name: None`; a violation the generated binding detects locally keeps its
> name. A type identifier was rejected because the catalog descriptor carries no
> type table, and the name is diagnostic, not contractual.

**§10 item 3** becomes:

> 3. **The encoding tag.** Whether the binding carries any of the three
>    encodings, the `attach` tag naming the session's, or fixes one; when it
>    fixes one, which, under the ADR-0020 decision 2 matrix, as a value of the
>    table in §3; when it carries any, which default its transport offers.

**§11, the opening paragraph.** Replace "The WebSocket transport is planned in
this repository and is not built yet" with "The WebSocket binding is specified
in §11.1 and its transport crate is not built yet".

**§11.1, replaced whole:**

> ### 11.1 WebSocket
>
> The WebSocket binding carries this frame between a consuming runtime that
> opens a WebSocket connection and a providing runtime that accepts it.
> `ridl-transport-ws` (driftsys/ridl#265,
> [ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
> decision 6) is its Rust crate, and the TypeScript WebSocket package of
> decision 7 is the same binding for Deno and the browser, so the two ends of
> one connection may be a Rust runtime and a TypeScript one. This section is the
> binding document: it states the nine items of §10 and adds nothing to the
> frame. The binding is **version 1** and carries frame version 1.
>
> **1. The transport and the session.** One WebSocket connection carries one
> session. The client is the consumer and sends `attach` as its first message;
> the server is the provider and sends `attached` as its first message. The URL
> is the deployment's; the binding places nothing in the path, the query or a
> header; `ws://` and `wss://` are both allowed, and what `wss://` provides is
> the security the deployment relies on (§13). The session ends when either side
> sends a close frame, when the connection fails, or when the liveness check
> fails: the provider sends WebSocket pings, and either end treats no message of
> any kind, data or control, within its liveness timeout as the end of the
> session. The provider pings because a browser client cannot. A consumer that
> needs two catalogs opens two connections.
>
> **The subprotocol.** The client offers `ridl-ws-v1` in
> `Sec-WebSocket-Protocol`; a client offering several versions lists the newest
> first, and the server selects the newest it serves. When the server selects
> none, the client fails the connection; a client that can choose a close code
> uses 1002. On a `ridl-ws-v1` connection, `attach.frame_version` is 1 or the
> `attach` is refused `frame_version_unsupported`.
>
> **2. The header encoding.** Every integer is fixed-width little-endian;
> `stamp` is a two's-complement `i64`. There is no padding, no alignment and no
> length field: the header is the prefix below and the payload is every byte
> after it. Every code is the value §4 and §9.6 give it (`form`, `provenance`,
> `outcome`, a refusal reason, a `Rule`), `kind` is the `Kind` value of §2 and
> `encoding` the tag of §3; this section restates none. Fields are in this
> order, and a form carries exactly the fields §4 gives it:
>
> | Form                               | Bytes                                                                                                                     | Header size |
> | ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------- | ----------- |
> | `attach`                           | `form` u8, `frame_version` u32, `encoding` u8, `hash` 32 bytes, `name_len` u8 (1 to 255), `name` UTF-8 (`name_len` bytes) | 39 + name   |
> | `attached`                         | `form` u8, `outcome` u8, `detail` u8                                                                                      | 3           |
> | `subscribe`, `unsubscribe`, `read` | `form` u8, `kind` u8, `interface` u32, `ordinal` u32                                                                      | 10          |
> | `answer`                           | the common 10, then `outcome` u8, `detail` u8                                                                             | 12          |
> | `publish`                          | the common 10, then `stamp` i64, `seq` u64, `provenance` u8                                                               | 27          |
> | `occurrence`, `request`            | the common 10, then `stamp` i64, `seq` u64                                                                                | 26          |
> | `response`                         | the common 10, then `stamp` i64, `seq` u64, `correlation` u64, `outcome` u8, `detail` u8                                  | 36          |
>
> `detail` is the `Rule` code (§9.6) with `outcome` 3, the refusal reason code
> (§4) with `outcome` 9, and 0 with every other outcome. The `name` is at least
> one byte and valid UTF-8. A value outside its table is a protocol violation
> (item 4). `InterfaceNo` and `Ordinal` keep their 32-bit range, so §4's range
> check at `attach` always passes on this binding.
>
> **The sizes.** The envelope is 16 bytes. The largest data-plane header without
> its envelope is 20 bytes (`response`), so a data-plane message is at most 36
> bytes plus the payload bound. The largest header of any message is the
> `attach` at 39 + 255 = 294 bytes; it carries no payload, so a receiver's
> message size limit admits at least 294 bytes. The codegen model carries the
> data-plane numbers as the `websocket` row, binding version `"1"`,
> `frame_header_max_bytes` 20, `envelope_bytes` 16
> (`crates/ridl-ir/src/codegen/bindings.rs`). The row is derived from this
> layout: the layout is frozen through the vectors below, and the row is a
> derived value, corrected in a patch release if it is wrong
> ([ADR-0029](../decisions/ADR-0029-stability-policy.md) decision 2).
>
> **3. The encoding tag.** The binding fixes no encoding: a session uses the
> encoding its `attach` names, and a provider that does not serve it refuses
> `encoding_unsupported`. The default a program gets when it names none is the
> transport's, proto3, and the construction can name another.
>
> **4. Framing.** One frame is one WebSocket message with the binary opcode;
> WebSocket fragmentation is below this binding. A text message is closed
> with 1003. A message the receiver cannot read — shorter than its header, a
> code outside its table, `interface` or `ordinal` 0, a message before `attach`,
> a second `attach`, a `name_len` that does not match the message, a `name` that
> is empty or not valid UTF-8, a first provider-to-consumer message that is not
> `attached` — is a protocol violation: the receiver records the frame (§9.5)
> and closes with 1002, and the session ends as §3 states. A `request` with a
> readable header and no payload is answered `corrupt` (§9.5); any other frame
> missing a required payload is discarded and recorded. A message above the
> receiver's configured size limit, which admits at least 294 bytes, is closed
> with 1009. Either side ends the session on purpose with 1000, the provider
> with 1001 when it shuts down; after `attached(refused)` the provider closes
> with 1000, and the reason is in the frame. The binding defines no close code
> of its own.
>
> **5. The caller's identity** for §7 is the connection. It does not survive a
> reconnect: a reconnect is a new session, its counters start over, and the
> calls of the old session were reported `Transport::Down` when it ended. The
> binding never retransmits a `request`: the transport is reliable and ordered,
> so a message is lost only when the connection fails, which ends the session.
> Every call has an outcome, because every call has a response bound
> ([ADR-0015](../decisions/ADR-0015-qos-absorption-and-rpc-bounds.md) decision
> 4): the `response`, `Undelivered` or `Timeout` at the bound (§8), or `Down` at
> the session's end (§3).
>
> **6. Retention.** The provider keeps a settled response until the request's
> bound has passed, as §7 states; no call is without a bound, so the binding
> states no further retention.
>
> **7. Grouping.** None. The frames of one commit cross as separate messages,
> and a runtime over this binding does not present `CoherentSignals`.
>
> **8. Mapped and framed.** Everything is framed; nothing is mapped.
>
> **9. The rate-floor answer** is §8's: a call faster than the member's `min`
> may be refused at admission and answered `busy`. The binding adds nothing.
>
> **Later versions.** An addition to the frame — a form, a field, a code — is a
> new frame version carried by a new binding version, offered beside this one in
> the subprotocol list. Version 1's layout, codes and vectors are never edited.
> Streams (ridl §12, driftsys/ridl#336) arrive this way.
>
> **Test vectors.** Each line is one complete message in hexadecimal, spaces for
> reading only; `xtask` reads this table into the `wire` freeze dump by its `Id`
> column, which is stable and never renamed. The stamp in every data-plane
> vector is `0x0123456789AB` microseconds (and `0x0123456789AC` on the
> responses), chosen to show the byte order; the hash in `attach` is the bytes
> `00` to `1f`; the unit name is `veh.cabin`.
>
> | Id                   | Vector                                                          | Hex                                                                                                     |
> | -------------------- | --------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
> | `attach-1`           | `attach` frame 1, FlatBuffers, `veh.cabin`                      | `01 01000000 01 000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f 09 7665682e636162696e` |
> | `attached-accepted`  | `attached` accepted                                             | `02 01 00`                                                                                              |
> | `attached-mismatch`  | `attached` refused `catalog_mismatch`                           | `02 09 02`                                                                                              |
> | `subscribe-signal`   | `subscribe` signal, interface 1, ordinal 2                      | `03 01 01000000 02000000`                                                                               |
> | `answer-accepted`    | `answer` accepted, signal, interface 1, ordinal 2               | `04 01 01000000 02000000 01 00`                                                                         |
> | `publish-live`       | `publish` `Live`, seq 7, payload `ff01`                         | `07 01 01000000 02000000 ab89674523010000 0700000000000000 02 ff01`                                     |
> | `request-command`    | `request` command, interface 1, ordinal 3, seq 1, payload `01`  | `09 03 01000000 03000000 ab89674523010000 0100000000000000 01`                                          |
> | `response-accepted`  | `response` command accepted, correlation 1, provider seq 1      | `0a 03 01000000 03000000 ac89674523010000 0100000000000000 0100000000000000 01 00`                      |
> | `response-violation` | `response` query `contract(InvalidValue(Range))`, correlation 2 | `0a 04 01000000 04000000 ac89674523010000 0200000000000000 0200000000000000 03 01`                      |

**§13, the Streams bullet** becomes:

> - **Streams.** `<T>` on a call's parameter or return (ridl §12) has no frame
>   in version 1: `ridl-rt` 0.1 leaves streams out (driftsys/ridl#336). They
>   arrive as frame version 2, negotiated as §11.1 "Later versions" states, so
>   that a version 1 peer is unchanged.

**The Trace section** gains "Binds: ADR-0015 decision 4 (every call has a
response bound, §11.1 item 5)" and "The WebSocket binding's rulings:
`docs/archive/2026-10-12-ws-binding-design.md`" once this note is archived.

### 3.2 Other records

- **ADR-0020 decision 6**, the `ridl-transport-ws` row: "the ports over a
  WebSocket, proto3-framed" becomes "the ports over a WebSocket, carrying the
  encoding the session names, proto3 by default (frame specification §11.1)". A
  dated note in its Status records the change.
- **ADR-0021**: a dated amendment under the decision that fixes `Violation` (the
  payload module): `Violation.type_name` becomes `Option<&'static str>`, `None`
  for a violation a peer reported (frame specification §9.6), and a `Rule`
  variant is added only together with a frame version, although `Rule` is
  `#[non_exhaustive]`; it lands with lane W step 2's `ridl-rt` change, and the
  Kotlin plugin gets a heads-up. Lane C's 1b rewrite of ADR-0021 starts after
  this note is merged (lanes plan section 5) and carries the amendment through.
- **`docs/design/ridl-rt.md`**, the `payload` item list
  (`pub struct Violation
  { pub type_name: &'static str, pub rule: Rule }`):
  the field becomes `Option<&'static str>` with the step 2 change, one line.
- **Frame specification §2, the `Violation` row**: section 3.1 above.
- **Steering decision H**
  (`docs/wip/2026-10-06-ws-and-payload-formats-steering.md`, "Recorded, not
  asked"): a dated note that the `KNOWN` row and #718's first half move from
  #265 to the amendment pull request (R-W-16); #265 keeps the sum test.
- **`crates/ridl-ir/src/codegen/bindings.rs`**: the `KNOWN` row (R-W-16) and the
  doc comment of `Known.frame_header_max_bytes`, "the largest frame header the
  binding adds to a data-plane message, in bytes".
- **`docs/design/codegen-plugins.md`**, "Bindings, and what is not built": "The
  `bindings` list is empty today ... because no binding's frame layout is
  specified" becomes "The `bindings` list holds the `websocket` row (binding
  version 1, frame header at most 20 bytes, envelope 16), from the frame
  specification §11.1"; and the test paragraph "A second test adds a `websocket`
  row ... 14 + 2 + 8 = 24" becomes the real row's sum, 20 + 16 + 8 = 44 for
  `Cabin.warning`, with the fixture no longer `null` for the proto3-bounded
  members.
- **`docs/book/codegen-plugins.md`**, "What is not available yet": the
  "Transport binding overheads" bullet is removed, or reduced to "the sum is not
  yet checked against a frame a transport writes (driftsys/ridl#718)".
- **`freeze/README.md`**, "The frozen source copies": one sentence that the
  `wire` dump also holds the frame vectors of the frame specification §11.1,
  read from the document by their `Id`.
- **`CONTRIBUTING.md`**, "Changing a frozen surface": "the frame envelope and
  the `attach` handshake" leave the policy-only list when the vectors land.
- **`docs/specification/ridl-family-overview.md`**, the Frame Specification row:
  "0.1 draft" becomes "0.2 draft" and "no binding is built" becomes "the
  WebSocket binding is specified (§11.1), none is built".
- **ADR-0029**: the Context bullet "It holds no frame vectors and no `attach`
  handshake vectors" and decision 3's list are updated when the vectors land
  (R-W-19), and decision 6's `wire` row loses "frames are policy only now".
  `docs/book/stability.md` loses "the frame envelope, the `attach` handshake"
  from its policy-only list at the same time.
- **`docs/design/catalog-descriptor.md`**, "No runtime reads the list": add one
  sentence: "The frame specification §11.1 names the descriptor as the source a
  providing runtime reads the list from; the first WebSocket provider holds no
  descriptor and applies the exact-match rule."
- **`docs/ROADMAP.md`**, the 2026-10-09 paragraph: "section 11.1 ... is amended
  when E11.9 lands; the specification is unchanged until then" becomes "section
  11.1 was amended before 0.10.0 by the binding document; E11.9 implements it".
- **#350**: items 5 and 12 are ticked with a pointer to this note's R-W-13 and
  R-W-14. **#718**: a comment that the row lands with the amendment and the sum
  test with #265. **#336**: a comment with R-W-18. **#802** is not touched by
  this note (lane W step 2).

## 4. The `wire` freeze dump

Today `freeze/wire.txt` holds `encoding <name>`, `proto`/`fbs` schema lines and
`catalog-hash veh.cabin <hex>`. After section 3 is applied it also holds one
`frame-vector <id> <hex>` line per row of the §11.1 vector table, where `<id>`
is the row's `Id` cell (stable, never the caption) and `<hex>` is the `Hex` cell
with spaces removed; `xtask/src/freeze/wire.rs` reads the table from
`docs/specification/frame-specification.md` and fails the dump when a vector's
length does not equal its form's header size plus its payload, or when a code is
outside its table. Both frame vectors and the `attach` handshake vectors are
therefore added now (R-W-19). No vector for the catalog hash is added beyond
today's line: the hash's inputs are already frozen through it. The `wire` gate's
row is added once this amendment, the `.proto`/`.fbs` review and #858 have
merged (lanes plan section 3).

## 5. Open questions

None that block the amendment. Two items are deliberately left to the stories
that build them: the ping interval, liveness timeout and message size limit are
`ridl-transport-ws`'s configuration with defaults the crate documents (R-W-7,
R-W-15), and the `Violation.type_name` change is lane W step 2's (R-W-14).

## 6. Trace

- Release spec R-1, R-3:
  [`2026-10-11-release-0.10-design.md`](2026-10-11-release-0.10-design.md)
- Lane brief:
  [`2026-10-11-release-0.10-lanes-plan.md`](2026-10-11-release-0.10-lanes-plan.md)
  section 4, "Lane W"
- Steering:
  [`2026-10-06-ws-and-payload-formats-steering.md`](2026-10-06-ws-and-payload-formats-steering.md)
  decisions B, F, G, H, I
- Lane H: `docs/archive/2026-10-09-catalog-compat-design.md`; ADR-0014 decision
  15; `docs/design/catalog-descriptor.md`
- Issues: driftsys/ridl#350 (items 5, 12), #718, #336, #265, #264

## 7. Review

An adversarial review of the first draft (commit 69163400) raised sixteen
points; all are applied in this revision: (1) the provider pings and either end
times out on silence of any kind; (2) the encoding generalisation scoped to
§11.1, with §1 and §10 item 3 replacement text; (3) the `attach` name length as
`u8`, the 294-byte handshake maximum, the `Known` doc comment; (4) the code
tables moved to §4 and §9.6; (5) the records list completed (§2 `Violation` row,
`codegen-plugins.md`, the book chapter, `freeze/README.md`, `CONTRIBUTING.md`,
the family overview, `ridl-rt.md`); (6) a `Transport` variant, not
`UnknownInteraction`, for a stream on a version 1 session; (7) "or an `answer`"
dropped from R-W-14; (8) the `Rule`-with-frame-version clause in the ADR-0021
amendment; (9) §9.5's recording obligation kept in R-W-15; (10) a browser client
fails the connection without choosing a code; (11) steering decision H named as
changed; (12) the `Id` column of the vector table; (13) the first provider's
exact match and conformance item 1 stated, in the As-built note too; (14) the
two additions to the 1002 list; (15) the section 3.2 → 3.1 cross-references;
(16) the derived-value wording for the row.
