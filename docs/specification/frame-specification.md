# Frame Specification

**The logical frame of the ridl runtime** — what crosses the boundary between a
provider's runtime and a consumer's runtime, for each of the five interaction
kinds, stated once and independently of any transport. A **binding** carries
this frame over one transport; a **runtime** presents what crosses through the
ports of `ridl-rt`. This document is what both are written from.

Version: 0.2.0

> **Provenance.** This document is the frame specification (driftsys/ridl#257).
> It writes down what
> [ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md)
> decision 7 decided — one frame, several bindings — with the vocabulary
> [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) fixed for
> `ridl-rt` 0.1, over the interaction semantics of the
> [ridl language reference](ridl-language-reference.md) and the RPC bounds of
> [ADR-0015](../decisions/ADR-0015-qos-absorption-and-rpc-bounds.md). Its exit
> criterion is that a second implementation — a runtime, or a binding — can be
> written from it alone, without reading the code in this repository. Where this
> document and an ADR disagree, the ADR wins and this document is corrected.

> **As built.** One crate in this workspace, `ridl-transport-ws`, speaks this
> frame over WebSocket (§11.1). It serves the observe and call ports of a
> provider runtime, applies the exact-match rule of §6.1 at `attach`, and keeps
> the last 64 settled responses of a session for duplicate suppression instead
> of keeping each until its response bound has passed (§7). The runtime here,
> `ridl-loopback`, runs in process: it carries bytes from a provider handle to a
> consumer handle over one in-memory store, opens no socket, and has no wire
> format ([its design record](../design/ridl-loopback.md)). Neither enforces a
> timing rule or a contract clause on the provider side, so every sentence below
> about those describes the specification, not behaviour a shipped runtime
> performs. The first binding is the WebSocket transport (driftsys/ridl#265);
> the second is a runtime's own binder contract on Android, outside this
> repository (§11.2, which reverses an earlier AIDL binding generated per
> interface).
>
> The catalog descriptor carries the list of compatible earlier catalog hashes
> that §6.1 names, and no runtime in this workspace reads it at `attach`.
> `ridl-transport-ws` holds no descriptor, so it applies only the exact-match
> rule of §6.1 and does not meet conformance item 1 of §12 until it reads one.

---

## Table of Contents

1. [Scope and Position in the Family](#1-scope-and-position-in-the-family)
2. [Vocabulary](#2-vocabulary)
3. [The Session](#3-the-session)
4. [The Frame](#4-the-frame)
5. [What Crosses per Interaction Kind](#5-what-crosses-per-interaction-kind)
6. [The Control Plane](#6-the-control-plane)
7. [Sequence Numbers, Loss and Duplicates](#7-sequence-numbers-loss-and-duplicates)
8. [Timing](#8-timing)
9. [Invalid Payloads](#9-invalid-payloads)
10. [What a Binding Decides](#10-what-a-binding-decides)
11. [The Bindings](#11-the-bindings)
12. [Conformance](#12-conformance)
13. [Not in This Version](#13-not-in-this-version)

---

## 1. Scope and Position in the Family

A ridl contract describes interactions between a provider and a consumer (ridl
§3). When the two run in different processes, on different machines, or in
different languages, what one runtime hands to the other has to be agreed on.
This document agrees on it at the **logical** level: which facts cross, in which
direction, when, and what each side does with them. It fixes the fields, their
domains and their meaning. It does not fix a byte layout, a message format, a
socket, or a session-establishment handshake in transport terms; a binding does
that (§10), and there is one binding per transport.

Three layers are involved, and each is defined once:

| Layer             | Defined by                                                   | What it fixes                                                                                                                           |
| ----------------- | ------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------- |
| the contract      | the `.ridl` source, lowered to the IR                        | the interfaces, their numbers, the interactions, their ordinals, kinds, payload types, timing and clauses (§11)                         |
| the logical frame | this document                                                | what crosses a boundary per interaction kind, and what each runtime does with it                                                        |
| a binding         | one document per transport, written from this one (§10, §11) | how the frame's fields are carried over that transport, and whether the binding carries any of the three payload encodings or fixes one |

The payload's encoding is not this document's either. A payload crosses as bytes
in one of the three core encodings — FlatBuffers, proto3, `repr(C)` — and which
encoding a session uses is named at `attach` (§3). A binding either carries any
of the three, the `attach` tag naming the session's, or fixes one; the
[ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
decision 2 matrix — proto3 serialized into a stream, FlatBuffers mapped or
passed within a node — gives the default a transport offers when it carries any,
and the one encoding when it fixes one. §11.1 carries any; §11.2 fixes
FlatBuffers. A projection record fixes what the bytes of each encoding are
([ADR-0017](../decisions/ADR-0017-proto3-projection-rules.md),
[ADR-0019](../decisions/ADR-0019-flatbuffers-projection-rules.md)); this
document treats the payload as opaque and says only when it is present and what
its receiver does with it.

**The frame is the thing a third party implements.** A runtime in another
language implements the receiving and sending sides of §5 and §6 and presents
them through ports shaped like `ridl-rt`'s; a binding for another transport
carries §4's fields. Neither reads `ridl-loopback` or the Rust backend to do so,
and §10 makes that a rule rather than a hope.

## 2. Vocabulary

Every name below is `ridl-rt`'s, and it is defined once, in the module named.
This document reuses the names; it does not redefine them, and a reader who
needs a type's exact shape reads
[the `ridl-rt` design record](../design/ridl-rt.md), which carries every
definition, or the crate's own source under `crates/ridl-rt/src/`.

| Name          | Defined in          | Meaning here                                                                                                                                                                                                                                                                                                                                  |
| ------------- | ------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `CatalogRef`  | `ridl_rt::contract` | one unit's interfaces: the unit name and a 32-byte SHA-256 `CatalogHash`. Two references are equal only when both the name and the hash are equal                                                                                                                                                                                             |
| `InterfaceNo` | `ridl_rt::contract` | an interface's number in its catalog (ridl §11, `interfaces.lock`), a `u32`, never 0                                                                                                                                                                                                                                                          |
| `Ordinal`     | `ridl_rt::contract` | an interaction's position in its interface body, counted from 1 (ridl §11), a `u32`, never 0                                                                                                                                                                                                                                                  |
| `Kind`        | `ridl_rt::contract` | the five interaction kinds, with the values `Signal = 1`, `Event = 2`, `Command = 3`, `Query = 4`, `Fixed = 5`                                                                                                                                                                                                                                |
| `Timestamp`   | `ridl_rt::sample`   | `i64` microseconds since the PTP epoch, 1970-01-01 00:00:00 TAI (ridl §3.1)                                                                                                                                                                                                                                                                   |
| `Duration`    | `ridl_rt::sample`   | `i64` microseconds                                                                                                                                                                                                                                                                                                                            |
| `Envelope`    | `ridl_rt::sample`   | the sender's `stamp: Timestamp` and `seq: u64` (ridl §3.1). Stamped once, at the sender, and changed by nobody after that                                                                                                                                                                                                                     |
| `Provenance`  | `ridl_rt::sample`   | where a signal's value comes from: `Init`, `Live`, or `Invalid(Cause)` (ridl §4.4, §4.5)                                                                                                                                                                                                                                                      |
| `Cause`       | `ridl_rt::sample`   | why a channel is invalid: `Declared` by the provider, or `Detected(Detection)` by the consumer's binding                                                                                                                                                                                                                                      |
| `Detection`   | `ridl_rt::sample`   | what a consumer's binding found: `InvalidValue(Violation)` (a typl constraint, ridl §10.2) or `Corrupt` (not a well-formed encoding, ridl §10.3)                                                                                                                                                                                              |
| `Freshness`   | `ridl_rt::sample`   | `Fresh`, `Stale { by }` or `Unbounded`, measured by the consumer's runtime (§8)                                                                                                                                                                                                                                                               |
| `Correlation` | `ridl_rt::port`     | a `u64` identifying one sent call to its caller                                                                                                                                                                                                                                                                                               |
| `Contract`    | `ridl_rt::error`    | the four contract-error categories of ridl §10.2: `InvalidValue(Violation)`, `PreconditionFailed`, `ContractBroken`, `UnknownInteraction`                                                                                                                                                                                                     |
| `Transport`   | `ridl_rt::error`    | the detected infrastructure failures of ridl §10.3: `Timeout`, `Undelivered`, `Down`, `Corrupt`, `Busy`                                                                                                                                                                                                                                       |
| `Violation`   | `ridl_rt::payload`  | the `Rule` that failed and, for a violation the generated code detected, the name of the typl type whose constraint failed; a `contract(InvalidValue)` outcome decoded from a frame carries the rule alone (§9.6), whereas a consumer's own `Detected(InvalidValue(..))` on received bytes and a violation delivered in process keep the name |
| `Encoding`    | `ridl_rt::encoding` | the closed set of payload encodings: `FlatBuffers`, `Proto3`, `ReprC`, each named as its cargo feature is                                                                                                                                                                                                                                     |

Two words this document adds, because `ridl-rt` has no need of them:

- A **session** is one attachment of a consuming runtime to a providing runtime
  over one binding, for one catalog (§3).
- A **frame** is one unit that crosses a session, in either direction (§4).

Two `ridl-rt` names deliberately do **not** appear on the frame. `ClaimId` is
the provider runtime's local name for a call it has presented to application
code, and `Correlation` is the caller runtime's local name for a call it has
sent; neither crosses (§5.3, §5.4). `Family` — which of the five boundaries of
ridl §3.2 an interaction sits on — is not on the frame in this version (§13).
`TraceContext`, the optional trace context that a call or an event carries
across a port (`ridl_rt::trace`), is not on the frame either, so a frame
transport delivers `None`.

## 3. The Session

**A session is asymmetric.** One side **consumes** and the other **provides**.
The consuming side opens the session, subscribes, reads and calls; the providing
side publishes, raises, answers and settles. Two components that each provide an
interface to the other open two sessions, one in each direction. Which side
provides what is the wiring's (rsdl), not the frame's.

**A session is bound to one catalog.** It is opened by an `attach` naming a
`CatalogRef` (§6.1), and every `InterfaceNo` and `Ordinal` on every frame of the
session is scoped by that catalog. This is
[ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decision 3 one
layer down: a port is bound to one catalog, and the session under it is too. A
boundary carrying several catalogs carries several sessions.

**A session fixes the payload encoding.** Every payload that crosses the session
is in one encoding, named at `attach` (§6.1) and never changed. A binding either
carries any of the three encodings, in which case the consuming program names
one at construction and the transport offers a default taken from the
[ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
decision 2 matrix, or fixes one (§1, §10 item 3); §11.1 carries any and §11.2
fixes FlatBuffers. `attach` carries the tag in both cases, so that a mismatch is
a refusal rather than a stream of corrupt payloads. The **encoding tag** values
are:

| Tag | Encoding    | `Encoding::NAME` |
| --- | ----------- | ---------------- |
| 1   | FlatBuffers | `flatbuffers`    |
| 2   | proto3      | `proto3`         |
| 3   | `repr(C)`   | `repr-c`         |

These are the values the `ridl-rt` design record lists as "the frame
specification's wire tag values" against a constant `Encoding::FORMAT` that
`ridl-rt` 0.1 does not carry. A binding carries the tag as a value in this
table; a runtime maps it to the encoding marker type. Adding a fourth encoding
is an ADR, never a fourth row written here alone
([ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decision 7).

**A session has one sequence counter per publishing channel, one per caller and
one per provider's responses** (§7), all of which start over when the session is
opened. State a consuming runtime holds about a session — the current sample of
each subscribed signal, the sequence numbers it has seen, the calls in flight —
is the session's, and a session that ends takes it with it: after the session
ends, every port of the consuming runtime that reaches through it reports
`Detached`, and every call in flight reports `Transport::Down`.

## 4. The Frame

A frame has a **header** and, on some forms, a **payload**. The header's field
set is closed: a binding carries exactly these fields, adds none, and omits none
that the form requires. Header fields have no default: a field the form requires
is present on every frame of that form.

| Field         | Domain                                                        | Present on                                     |
| ------------- | ------------------------------------------------------------- | ---------------------------------------------- |
| `form`        | one of the ten forms below                                    | every frame                                    |
| `interface`   | `InterfaceNo`, `u32`, never 0                                 | every frame but `attach` and `attached`        |
| `ordinal`     | `Ordinal`, `u32`, never 0                                     | every frame but `attach` and `attached`        |
| `kind`        | `Kind`, values 1 to 5                                         | every frame but `attach` and `attached`        |
| `envelope`    | `Envelope`: `stamp: i64`, `seq: u64`                          | `publish`, `occurrence`, `request`, `response` |
| `provenance`  | `Init`, `Live` or `Invalid(Declared)`                         | `publish`                                      |
| `correlation` | `u64`: the `seq` of the request answered                      | `response`                                     |
| `outcome`     | §5.3 and §5.4; §9.6 for what `contract(InvalidValue)` carries | `response`, `attached`, `answer`               |
| `payload`     | bytes in the session's encoding                               | as §5 states per form and per kind             |

**The ten forms**, by plane and direction:

| Form          | Plane   | Direction           | Meaning                                                                      |
| ------------- | ------- | ------------------- | ---------------------------------------------------------------------------- |
| `attach`      | control | consumer → provider | opens the session for one catalog and one encoding (§6.1)                    |
| `attached`    | control | provider → consumer | accepts or refuses the `attach` (§6.1)                                       |
| `subscribe`   | control | consumer → provider | starts delivery of one signal or one event (§6.2)                            |
| `answer`      | control | provider → consumer | accepts or refuses a `subscribe`; refuses a `read` (§6.2, §6.3)              |
| `unsubscribe` | control | consumer → provider | stops delivery of one signal or one event (§6.2)                             |
| `read`        | control | consumer → provider | asks for the current state of one signal, or the value of one `fixed` (§6.3) |
| `publish`     | data    | provider → consumer | one publication of a signal's state, or the value of a `fixed` (§5.1, §5.5)  |
| `occurrence`  | data    | provider → consumer | one occurrence of an event (§5.2)                                            |
| `request`     | data    | consumer → provider | one call of a command or a query (§5.3, §5.4)                                |
| `response`    | data    | provider → consumer | the acknowledgment of a command, or the reply of a query (§5.3, §5.4)        |

**The codes.** A binding that carries a field as a numeric code uses these
values. `form`: `attach` 1, `attached` 2, `subscribe` 3, `answer` 4,
`unsubscribe` 5, `read` 6, `publish` 7, `occurrence` 8, `request` 9, `response`
10 — the order of the table above. `provenance`: `Init` 1, `Live` 2,
`Invalid(Declared)` 3. `outcome`: `accepted` 1, `reply` 2,
`contract(InvalidValue)` 3, `contract(PreconditionFailed)` 4,
`contract(ContractBroken)` 5, `contract(UnknownInteraction)` 6, `corrupt` 7,
`busy` 8, `refused` 9. A `refused` reason: `unknown_catalog` 1,
`catalog_mismatch` 2, `encoding_unsupported` 3, `frame_version_unsupported` 4,
`range` 5. The `Rule` of an `InvalidValue`: §9.6. A new value in any of these
tables is a new frame version.

**Why `kind` is on the frame at all.** Both sides hold the catalog, so either
could look the kind up from the ordinal. It crosses because a relay, a recorder
or an observer that holds no catalog can route and classify a frame by it, and
because a frame whose `kind` disagrees with the receiver's catalog is a
detectable disagreement between peers — `Contract::UnknownInteraction` (ridl
§10.2, §11) — where an ordinal alone would be silently misread. The receiver
checks it: a frame whose `kind` is not the receiver's catalog's kind for that
`(interface, ordinal)` is treated exactly as a frame naming an unknown ordinal
(§6.4).

**The header is canonical.** Because the field set is closed and has no optional
or defaulted field, the header of a given frame has one encoding under a given
binding. That is the part of typl §17.10's canonical-encoding rule this document
owns; the payload's part belongs to each projection record.

**Widths.** `InterfaceNo` and `Ordinal` are `u32` on the frame, as they are in
`ridl-rt` and in the IR. This document places no bound on a header's size, so
the `u16` fallback
[ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decision 2
records for this specification is **not triggered**. A binding whose transport
carries one of these in a native field narrower than 32 bits — an AIDL
transaction code is one — checks the catalog's numbers against that field's
range when the session is attached, and refuses the `attach` when a number does
not fit (§6.1); it does not truncate.

## 5. What Crosses per Interaction Kind

For each kind: which frames it uses, what each field means on them, what the
sender puts in and what the receiver does with it. The five subsections are the
normative core of this document. The facts come from the ridl reference sections
cited, and from `ridl-rt`'s port contracts where the reference leaves the
runtime's side unstated.

### 5.1 Signal

A signal is continuous state: the latest sample is the truth (ridl §4). It
crosses as `publish` frames, provider to consumer, after a `subscribe` or in
answer to a `read`.

| Field        | On a `publish` of a signal                                                                                                        |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------- |
| `kind`       | `Signal`                                                                                                                          |
| `envelope`   | the provider's `stamp` and the channel's `seq` at this publication; under `Init`, `seq` 0 and the stamp of the channel's creation |
| `provenance` | `Init` before the first publication; `Live` for a publication; `Invalid(Declared)` for an invalidation                            |
| `payload`    | **present under `Live`**, the value; **absent under `Init` and `Invalid(Declared)`**                                              |

**What the provider sends.** A `SignalWriter::commit` publishes everything the
provider staged — a `set`, an `invalidate` or a `touch` per channel — under one
stamp per interface, taken from the provider runtime's clock. Each staged
channel becomes one `publish` frame:

- a **`set`** is a `publish` with `Live` and the value;
- an **`invalidate`** is a `publish` with `Invalid(Declared)` and no payload
  (ridl §4.5: the invalidity is delivered, the value is not);
- a **`touch`** is a `publish` with `Live` and the **current value again**. It
  carries the value, although nothing changed, so that a consumer which missed
  the earlier publication, or which subscribed between the two, holds the value
  after this frame. A form that re-affirms without a payload was rejected for
  that reason.

Every `publish` of a channel increments that channel's `seq` (§7), an
invalidation and a re-affirmation included: each is a publication.

**What the consumer does.** The consuming runtime holds, per subscribed signal,
one **current sample**: the last `publish` frame it accepted for that channel,
plus the last good value. On receiving a `publish` for a subscribed signal it:

1. discards the frame when it already holds a sample for that channel and the
   frame's `seq` is not greater than that sample's (a duplicate, or a
   reordering; §7); an `unsubscribe` drops the held sample, so a later
   `subscribe` starts again from the first frame it receives;
2. replaces the sample's envelope and provenance with the frame's;
3. under `Live`, hands the payload bytes to the generated binding, which runs
   `Payload::verify`. On success the sample's value is the decoded value and its
   provenance stays `Live`. On failure the value stays the **last good value**
   and the provenance becomes `Invalid(Detected(_))` (§9.1);
4. under `Init`, sets the value to the **init value**, which the generated
   binding supplies from the catalog — the init value is a fact both sides hold,
   so it never crosses;
5. under `Invalid(Declared)`, keeps the last good value — or the init value,
   when there is none — and sets the provenance to `Invalid(Declared)`.

The envelope a consumer reads on a sample is therefore **the envelope of the
frame that put the channel in its current state**, including a frame whose
payload the consumer rejected. That is the reading ridl §4.5 implies, and it is
this document's answer to
[ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md) open
item 4 for the frame; the ridl finalization pass (driftsys/ridl#319) confirms it
in the reference or corrects both.

`SignalReader::read` returns the current sample with its `Freshness`, which the
consuming runtime computes from the sample's stamp and its own clock (§8).
Nothing crosses for a read of a subscribed signal: the sample is local.
`ScannableSignals` and `CoherentSignals` are local views over the same samples
and cross nothing either.

**A commit is a set of frames with one stamp.** The frames of one
`SignalWriter::commit` carry the same `stamp`. A binding may carry them as one
unit and may deliver them as one unit; when it does, the consuming runtime may
present `CoherentSignals` over them. A binding that delivers each frame
separately does not present `CoherentSignals`, which `ridl-rt` permits a runtime
to omit. The frame requires no grouping: production coherence is the provider's
(one commit, one stamp) and delivery coherence is the binding's
([ADR-0015](../decisions/ADR-0015-qos-absorption-and-rpc-bounds.md) decision
10).

**Subscribing delivers the current state immediately.** The first `publish` a
consumer receives after the `answer` is the channel's current state — `Init`
with no payload before any publication, or the last publication (ridl §4.4).
That first frame is an ordinary `publish`; nothing marks it, because the
consumer treats a solicited and an unsolicited `publish` identically.

### 5.2 Event

An event is a discrete occurrence: every occurrence matters and occurrences are
queued, not coalesced (ridl §5). It crosses as `occurrence` frames, provider to
consumer, after a `subscribe`.

| Field        | On an `occurrence`                                                                    |
| ------------ | ------------------------------------------------------------------------------------- |
| `kind`       | `Event`                                                                               |
| `envelope`   | the provider's `stamp` when it raised the occurrence, and the channel's `seq`         |
| `provenance` | absent — an occurrence is not state and has no provenance (ridl §4.4, last paragraph) |
| `payload`    | always present: the occurrence's value as the provider encoded it                     |

**What the provider sends.** `EventSink::raise` sends one `occurrence` to every
consumer subscribed to that event at the moment it is raised, with the channel's
next `seq` (§7). There is no staging and no commit. An occurrence raised while a
consumer is not subscribed is not sent to it, and is never sent later: there is
no cache and no replay (ridl §5.1).

**What the consumer does.** The consuming runtime queues occurrences per session
in the order received and presents them through `EventSource::next`. It discards
an occurrence older than the event's time to live, measured from the frame's
`stamp` against its own clock (§8), inside `next`. It never verifies the
payload: `next` returns the bytes, and the generated binding runs
`Payload::verify` and delivers the occurrence either as the decoded value or as
`Err(Detection)` with its envelope — **an occurrence whose payload fails its
check is delivered with that marker, not withheld**
([ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decision 6;
§9.2). A gap in `seq` between two accepted occurrences of one channel is a loss
(ridl §3.1); the runtime records it and continues.

### 5.3 Command

A command is a fire-and-forget action request in the contract, with a delivery
acknowledgment beneath it in the runtime (ridl §6). It crosses as one `request`,
consumer to provider, and one `response`, provider to consumer.

| Field         | On the `request`                                                                           | On the `response`                                                                                  |
| ------------- | ------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------- |
| `kind`        | `Command`                                                                                  | `Command`                                                                                          |
| `envelope`    | the caller's `stamp` when it sent the call, and the caller's `seq` (unique per caller, §7) | the provider's `stamp` when it settled, and the provider's `seq` for its responses on this session |
| `correlation` | absent — the request's own `seq` is what the response echoes                               | the request's `seq`                                                                                |
| `outcome`     | absent                                                                                     | `accepted`, `contract(Contract)`, `corrupt`, or `busy`                                             |
| `payload`     | always present: the argument value                                                         | absent — an acknowledgment carries no functional payload (ridl §6.1)                               |

**What the caller sends.** `Caller::command` encodes the argument, evaluates the
command's `require` clauses **before sending** — a failing precondition costs no
round trip and is reported locally as `SendError::Contract(PreconditionFailed)`
— and sends one `request`. The `Correlation` the port returns is the caller
runtime's local name for the outcome it will read back through `Caller::ack`; a
runtime may use the request's `seq` as its value, and nothing on the frame
depends on that choice.

**What the provider does.** A providing runtime that cannot admit the call — no
slot, no budget, or a call faster than the member's `min` (§8) — does not
present it and answers with a `response` whose outcome is `busy`. Otherwise, on
a `request` naming a command, the providing runtime presents it once to
application code as a claim (`Handler::next_claim`, with a `ClaimId` that is the
provider's local name and never crosses), and the generated `dispatch`:

1. runs `Payload::verify` over the argument bytes; a structure failure settles
   `corrupt`, a typl violation settles `contract(InvalidValue(violation))`;
2. evaluates the `require` clauses; a false clause settles
   `contract(PreconditionFailed)`;
3. settles **`accepted`**, and only then calls the provider's method.

The `response` is sent when the claim settles. A command's acknowledgment is a
delivery acknowledgment — received and accepted for execution — and never an
execution outcome (ridl §6.1): the provider's method runs after the response is
sent, and what it does is observed as state, never reported on this frame. A
command is rejected whole or accepted whole; it is never partially executed
(ridl §6.2).

**What the caller does with the response.** `Caller::ack` reports `Ok(())` for
`accepted`, `Err(CallError::Contract(c))` for `contract(c)`,
`Err(CallError::Transport(Corrupt))` for `corrupt`, and
`Err(CallError::Transport(Busy))` for `busy`. A `response` whose `correlation`
names no call in flight is discarded. No `response` within the command's
response bound is `Transport::Undelivered` (§8), detected by the caller's
runtime; the response bound covers acceptance, not execution
([ADR-0015](../decisions/ADR-0015-qos-absorption-and-rpc-bounds.md) decision 3).

### 5.4 Query

A query is request/response with a mandatory reply (ridl §7). It crosses as one
`request` and one `response`, exactly as a command does, with two differences:
the response carries the reply, and the settlement happens after the provider's
method returns.

| Field         | On the `request`                   | On the `response`                                                                                  |
| ------------- | ---------------------------------- | -------------------------------------------------------------------------------------------------- |
| `kind`        | `Query`                            | `Query`                                                                                            |
| `envelope`    | as for a command                   | as for a command                                                                                   |
| `correlation` | absent                             | the request's `seq`                                                                                |
| `outcome`     | absent                             | `reply`, `contract(Contract)`, `corrupt`, or `busy`                                                |
| `payload`     | always present: the argument value | **present under `reply`**, the declared return type; absent under `contract`, `corrupt` and `busy` |

**What the caller sends** is as for a command; the outcome is read back through
`Caller::reply`, never through `Caller::ack`, which answers `None` for a query's
correlation always.

**What the provider does.** Admission is the command's: a query the providing
runtime cannot admit is answered `busy`. Steps 1 and 2 are the command's. Then
the generated `dispatch` calls the provider's method, evaluates the query's
`ensure` clauses over the arguments and the reply, and settles
`contract(ContractBroken)` when a clause is false — an `ensure` failure is a
provider defect (ridl §10.2) and the reply it would have carried does not cross
— or `reply` with the encoded reply otherwise. **A query settles after the
provider's method returns**, because its settlement carries the reply.

**The error arm is inside the payload.** A fallible query's return type is an
inline `T | E` (ridl §10.1), and the reply payload is that union: a Stratum 1
failure is a `reply` whose payload holds the error arm. It is never an `outcome`
value, because it is data the contract declares, and this frame's `outcome`
carries only what the contract does not declare — Stratum 2 and the two Stratum
3 failures a provider can report (§9.6).

**What the caller does with the response.** `Caller::reply` returns the reply
bytes for `reply`, `Err(CallError::Contract(c))` for `contract(c)`,
`Err(CallError::Transport(Corrupt))` for `corrupt` and
`Err(CallError::Transport(Busy))` for `busy`. The generated binding verifies the
reply bytes (§9.3). No `response` within the query's response bound is
`Transport::Timeout` (§8), detected by the caller's runtime.

### 5.5 Fixed

A `fixed` is provisioned outside the contract and immutable for the lifetime of
the software instance (ridl §8). It is not published and not called; it crosses
**at most once per read**, provider to consumer, as a `publish` frame in answer
to a `read` (§6.3).

| Field        | On a `publish` of a `fixed`                                                                        |
| ------------ | -------------------------------------------------------------------------------------------------- |
| `kind`       | `Fixed`                                                                                            |
| `envelope`   | the provider's `stamp` when it answered, and `seq` 0 — a `fixed` has no sequence: it never changes |
| `provenance` | `Live`                                                                                             |
| `payload`    | always present: the provisioned value                                                              |

A consuming runtime may cache the value for the session's lifetime and answer
every later `FixedReader::read_fixed` from the cache, because the contract
promises the value does not change while the software instance runs; a new
session reads it again, because the provider may be a new instance. There is no
`subscribe` for a `fixed`: a `subscribe` naming one is refused
`UnknownInteraction` (§6.2). There is no provider-side port: the value is
provisioned into the providing runtime, not published by application code.

### 5.6 The field table, by kind

The same facts in one table. "—" is absent; the parenthesised form is the frame
the column describes.

| Kind      | Frames                 | `envelope`                              | `provenance`                          | `correlation`     | `outcome`                                     | `payload`                   |
| --------- | ---------------------- | --------------------------------------- | ------------------------------------- | ----------------- | --------------------------------------------- | --------------------------- |
| `signal`  | `publish`              | provider stamp, channel `seq`           | `Init` / `Live` / `Invalid(Declared)` | —                 | —                                             | under `Live` only           |
| `event`   | `occurrence`           | provider stamp, channel `seq`           | —                                     | —                 | —                                             | always                      |
| `command` | `request` / `response` | caller stamp, caller `seq` / provider's | —                                     | — / request `seq` | — / `accepted`, `contract`, `corrupt`, `busy` | always / —                  |
| `query`   | `request` / `response` | caller stamp, caller `seq` / provider's | —                                     | — / request `seq` | — / `reply`, `contract`, `corrupt`, `busy`    | always / under `reply` only |
| `fixed`   | `publish` (on `read`)  | provider stamp, `seq` 0                 | `Live`                                | —                 | —                                             | always                      |

## 6. The Control Plane

The control plane is uniform across every binding
([ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md)
decision 7): the same five operations over a Unix socket, Binder, a WebSocket
and the wasm host boundary. Only the data plane varies between a path that maps
a buffer and one that frames it.

### 6.1 `attach` and `attached`

`attach` opens the session. It carries:

| Field           | Domain                                                  |
| --------------- | ------------------------------------------------------- |
| `frame_version` | `u32`: the version of this document's frame; **1** here |
| `catalog`       | `CatalogRef`: the unit name and the 32-byte hash        |
| `encoding`      | the encoding tag (§3)                                   |

`attached` answers with an `outcome`:

- **`accepted`**: the provider serves that catalog name; the hash is the
  provider's own catalog hash or one of the earlier hashes its catalog
  descriptor lists as compatible (`Catalog.compatible`,
  [ADR-0014](../decisions/ADR-0014-ir-encodings.md) decision 15 as amended on
  2026-10-09); the encoding is one the provider serves; and every interface
  number and ordinal of the catalog the provider serves fits the binding's
  native fields (§4). The session is open.
- **`refused(reason)`**, with `reason` one of `unknown_catalog` (the name is not
  served), `catalog_mismatch` (the name is served, and the hash is neither the
  provider's own nor one its descriptor lists), `encoding_unsupported`,
  `frame_version_unsupported`, or `range` (a number does not fit a native
  field). The session is not open; nothing else is exchanged.

**The compatible catalogs list.** The toolchain writes the list at the
provider's build: `ridl baseline` records, per unit, the chain of published
catalog hashes whose successive changes `ridl diff` judged compatible, and
`ridl build` emits that chain when the change from the published baseline to the
tree it builds is compatible too. A breaking change restarts the chain when it
is published: `ridl baseline` records the new hash alone. A tree that is
breaking relative to the published baseline is built with an empty list, and the
recorded chain is left as it is until the next publication. The one exception is
a catalog that did not change: when the catalog hash equals the newest hash of
the published chain, the chain is carried at publication and at the build
whatever the verdict, because every hash it lists is still compatible with that
catalog. [ADR-0014](../decisions/ADR-0014-ir-encodings.md) decision 15, as
amended on 2026-10-09 and 2026-10-10, states the rule. The direction is fixed:
an **older consumer** attaches to a **newer provider**. A newer consumer
attaching to an older provider is refused `catalog_mismatch`, because no tool
judges that direction and the provider is the authority on what it serves. The
descriptor field, the chain file and the emit are built (driftsys/ridl#787).
With an empty list the rule above is the exact-match rule.

The catalog check is the peers' agreement on the contract, taken once. On a
session whose `attach` named the provider's own hash, the two runtimes hold the
same interfaces, the same ordinals and the same kinds, so a frame that names an
unknown one is one side's defect (§6.4). On a session whose `attach` named an
earlier hash, the agreement is this: every interaction the consumer's catalog
holds, the provider's catalog holds under the same interface number, ordinal and
kind, or has retired since; the provider's catalog may hold interactions the
consumer's does not; the consumer never names an interaction its own catalog
does not hold; and the provider never sends a frame for an interaction the
consumer did not name on the session. The last clause costs the provider nothing
to keep: every data-plane frame it sends answers a `subscribe`, a `read` or a
`request` of the session (§5, §6.2, §6.3), each of which names an
`(interface, ordinal)` the consumer chose from its own catalog, so an
interaction appended after the consumer's hash — an event, say — is never
subscribed by that consumer and its occurrences are never sent to it. A
consuming runtime that subscribes at `attach` subscribes the signals of its own
catalog. What an older consumer can meet is an interaction a later catalog
retired, which the provider answers as §6.4 states.

**The hash is computed, and the face checks it.** Since driftsys/ridl#378, a
package generated by the Rust backend carries a computed catalog hash
([ADR-0014](../decisions/ADR-0014-ir-encodings.md) decision 15) in place of the
`CatalogHash([0u8; 32])` placeholder it carried before, so the comparison above
tells two catalogs apart. Since driftsys/ridl#381, the generated face's
`Bind::new` and `serve` compare the port's `CatalogRef` with the interface's
`CATALOG`, name and hash, once, and panic on a mismatch
([ADR-0023](../decisions/ADR-0023-interaction-face-generation.md) decision 8;
[the interaction-face record](../design/interaction-face.md), "The catalog
check"). That check is local to one program, and it compares the two references
for equality, list or no list; this section's `attach` is the agreement between
two peers.

A consuming runtime whose `attach` was refused reports the refusal through its
own API — how is the runtime's — and every port of it reports `Detached`.

### 6.2 `subscribe`, `answer` and `unsubscribe`

`subscribe` names one `(interface, ordinal)` whose `kind` is `Signal` or
`Event`. The granularity is the interaction (ridl §5.1): one subscription per
signal or event, never per interface and never per group. An `answer` carrying
the same identity replies with `accepted` or `contract(UnknownInteraction)` — a
subscription to an unknown interaction fails at bind time (ridl §10.2). A
`subscribe` naming a `fixed`, a `command` or a `query` is refused the same way.
`answer` is the one control-plane reply form: it carries `interface`, `ordinal`,
`kind` and an `outcome`, and no envelope, no correlation and no payload.

For a signal, `accepted` is followed by the channel's current state as a
`publish` (§5.1). For an event, nothing follows until the provider raises an
occurrence, and only occurrences raised after the `answer` are delivered. A
second `subscribe` to an interaction already subscribed on the session is
accepted and changes nothing.

`unsubscribe` names one `(interface, ordinal)` and has no answer. Frames of that
interaction already in flight may still arrive; the consuming runtime discards
them.

The port pair `EventSource::subscribe` / `unsubscribe` sends these frames; a
runtime's `SignalReader` needs the signal subscribed too, which the generated
binding or the runtime arranges — how is the runtime's, and a runtime may
subscribe every signal of an interface at attach.

### 6.3 `read`

`read` names one `(interface, ordinal)` whose `kind` is `Signal` or `Fixed`, and
is answered with one `publish` (§5.1, §5.5) carrying the current state or the
provisioned value. A `read` of a signal does not subscribe it: nothing follows
the one `publish`. A `read` of any other kind is refused with an `answer`
carrying the `read`'s identity and `contract(UnknownInteraction)` — a `publish`
cannot carry an `outcome` — which the consuming runtime maps to
`ReadError::Contract(UnknownInteraction)`. A fulfilled `read` has no `answer`:
the `publish` is its answer.

Because two reads of one `fixed` return one value, and two reads of one signal
are ordered by `seq` (§7), a `read` needs no correlation of its own: its answer
is identified by `(interface, ordinal)` and applied by the rules of §5.1.

### 6.4 An unknown interaction

A frame naming an `(interface, ordinal)` the receiver's catalog does not hold,
or naming it with a `kind` the catalog does not give it, is a disagreement
between peers, `Contract::UnknownInteraction`:

- a `request` is answered with a `response` carrying
  `contract(UnknownInteraction)` and the request's `seq` as `correlation`;
- a `subscribe` is answered with an `answer` carrying
  `contract(UnknownInteraction)`;
- a `read` is answered as §6.3 states;
- a `publish`, an `occurrence`, a `response` or an `unsubscribe` is discarded
  and recorded; nothing is sent back.

An `answer` or a `response` that refuses a frame carries the `interface`,
`ordinal` and `kind` of that frame as received. So a `response` carrying
`contract(UnknownInteraction)` may carry a `kind` other than `Command` or
`Query`, and a receiver accepts it.

An interface the catalog retires, and a `reserved` ordinal, are unknown for this
purpose: the descriptors hold no row for either. On a session attached under an
earlier hash (§6.1), a `subscribe`, a `read` or a `request` naming an
interaction that the provider's catalog retired after that hash is answered the
same way, and that answer is the designed one, not a defect: the consuming
runtime reports it as the contract error of ridl §10.2. A frame the consumer
cannot place is the provider's defect on either kind of session. Whether a
runtime can tell an unknown ordinal from one it has no value for yet depends on
its holding the member table. `ridl-loopback` holds none: giving the loopback
the catalog descriptor is not yet assigned to an issue. Until it holds one, it
cannot report `UnknownInteraction` at all.

## 7. Sequence Numbers, Loss and Duplicates

`Envelope.seq` is assigned by the sender and never rewritten (ridl §3.1). Its
scope depends on the kind:

- **A signal channel and an event channel each have one counter**, per provider
  instance, per session, starting at 1 for the first publication or occurrence;
  `seq` 0 is the envelope of a signal channel with no publication. A gap between
  two accepted occurrences of one event channel is a **loss** (ridl §3.1); a gap
  on a signal channel is not — intermediate samples may be missed (ridl §4).
- **A caller has one counter**, per session, over every call it sends on every
  channel, starting at 1. So a call's `seq` is unique per caller, not per
  channel ([ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md)
  decision 5), and it is what a `response` echoes as `correlation`.
- **A provider's responses have one counter** per session, starting at 1. It
  orders the responses; nothing depends on it beyond that.

**Ordering on a signal.** A consuming runtime accepts a `publish` for a channel
only when its `seq` is greater than the one it holds for that channel (§5.1). So
a reordered or duplicated `publish` is discarded, and the newest publication
wins whatever order the frames arrive in.

**Duplicate suppression on a call.** A binding may retransmit a `request` — the
same frame, the same envelope — while no `response` has arrived and the response
bound has not passed. The providing runtime keys each call it has presented on
**the caller's identity plus the request's `seq`**, where the caller's identity
is the session's (the binding says what it derives it from; §10). A `request`
under a key already presented is **not presented again**: the runtime re-sends
the `response` it gave, when the claim has settled, and sends nothing when it
has not. Two callers are never merged, even under one `seq`. A provider keeps a
settled call's response for a call with a response bound until that bound has
passed, measured from the request's `stamp`; for a call with no response bound,
the binding fixes the retention and states it.

The retransmission budget is bounded by the response bound
([ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md)
decision 18): a caller's runtime retrying with backoff exhausts its attempts
within the declared `max`, and reaching it is the Stratum 3 detection (§8).

No shipped runtime retransmits. `ridl-loopback` delivers each call once because
nothing in a process is lost. `ridl-transport-ws` suppresses a duplicate
`request`: it keeps the last 64 settled responses of a session, and sends the
kept response again for a `request` whose `seq` is among them. It does not know
the response bound, because it holds no descriptor, so it fixes a count instead
of retaining each response until the bound has passed.

## 8. Timing

**Nothing about timing crosses the boundary.** A member's `min` and `max` are
facts of the catalog (ridl §9), held by both sides in their descriptors; what
crosses is the envelope's `stamp`, and every timing rule is evaluated on it. The
stamp is in the platform's synchronized time base (ridl §3.1), so a receiver's
clock and a sender's stamp are comparable. Loss of synchronization is a system
failure outside this document (ridl §10.4).

| Bound                  | On a signal                                                                                                                                                        | On an event                                                                                          | On a command or a query                                                                                                                                                                                                                                                                   |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `max`, staleness bound | the **consuming** runtime computes `Freshness` on every read: `Fresh` while `now − stamp ≤ max`, else `Stale { by: now − stamp − max }`; `Unbounded` with no `max` | the **consuming** runtime discards an occurrence with `now − stamp > max` inside `EventSource::next` | the **calling** runtime reports `Transport::Undelivered` (command) or `Transport::Timeout` (query) through `ack` or `reply` once `now − request stamp > max` with no `response`; the **providing** runtime presents `Claim.remaining = max − (now − request stamp)`, `None` with no `max` |
| `min`, rate floor      | the **providing** runtime coalesces a faster `set` into the next publication                                                                                       | the **providing** runtime refuses a faster `raise` with `RaiseError::Busy`, or delays it             | the **providing** runtime may refuse a faster call at admission, and answers with `busy`                                                                                                                                                                                                  |

Under `@Xms`, the strict period, the providing runtime publishes the signal
every `X` whether or not it changed, which is a `touch` when nothing changed
(§5.1).

**None of this is built.** `ridl-loopback` holds no member table, so it reports
every sample `Unbounded`, every claim `remaining: None`, discards no occurrence,
and enforces no rate floor ([its design record](../design/ridl-loopback.md),
"What it cannot report"). The one timing fact a shipped runtime does produce is
the stamp, from a clock a test advances by hand.

## 9. Invalid Payloads

An invalid payload is one whose bytes fail `Payload::verify`: either the bytes
are **not a well-formed encoding** — `VerifyError::Structure(Malformed)`, a
Stratum 3 serialization failure (ridl §10.3) — or they are well formed and hold
a **value that breaks a typl constraint** — `VerifyError::Contract(Violation)`,
`INVALID_VALUE`, Stratum 2 (ridl §10.2). The two are never collapsed into one,
because they sit in different strata. A runtime never verifies: a port carries
bytes and names no payload type, so verification is the generated binding's on
both sides, and a runtime that verified would need the generated types below the
port, which the layering forbids
([ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
decision 6).

**A sender does not verify its encoded bytes.** `Ref::encode` calls
`Payload::encode` without running `Payload::verify` on the result
([ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decision 7).
Sender-side constraint checks belong to the encoder; the
[FlatBuffers codec record](../design/flatbuffers-codec.md#encode-verify-decode-max_size)
describes those checks. An invalid value that is published is a provider defect
that surfaces at every consumer, through the rules below, and through the
provider's own observability, never through a frame sent back on pub/sub (ridl
§4.5).

### 9.1 A signal publication

The channel transitions to the invalid state (ridl §4.5). The consuming runtime
keeps the last good value — or the init value, when there is none — sets the
sample's provenance to `Invalid(Detected(InvalidValue(violation)))` or
`Invalid(Detected(Corrupt))`, and takes the envelope of the rejected frame
(§5.1). The malformed value is never delivered; its invalidity always is, on
every read until a later `publish` with `Live` passes its check. Nothing is sent
back. The runtime records the transition.

**Nothing withholds a subsequent good publication**: a `publish` that passes its
check after one that failed returns the channel to `Live`.

### 9.2 An event occurrence

The occurrence is **delivered, marked invalid**: `Occurrence.payload` is
`Err(Detection)` and the envelope is the frame's
([ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decision 6).
It is not withheld, because nothing fails silently (ridl §10.3, §10.4), and it
is not delivered as a value, because the value is not one the contract admits.
Nothing is sent back.

### 9.3 A call's arguments and a query's reply

The **providing** runtime's generated `dispatch` verifies the argument bytes
before any application code runs, and settles `corrupt` for a structure failure
or `contract(InvalidValue(violation))` for a typl violation (§5.3). The
provider's method is not called. The `response` crosses, and the caller reads it
back as `Transport::Corrupt` or `Contract::InvalidValue(_)`.

The **calling** runtime's generated binding verifies a `reply` payload and
reports a structure failure as `CallError::Transport(Corrupt)` and a typl
violation as `CallError::Contract(InvalidValue(violation))` through the query's
reply method. Nothing is sent back to the provider: a provider that replies with
an invalid value is a provider defect, reported at the caller and through the
provider's observability.

### 9.4 A `fixed`

The consuming runtime's generated binding verifies the value on the read that
fetched it. A value that fails its check is not cached and the read reports the
detection to the application; a provisioned constant that fails its own type's
constraints is a provisioning defect, and the runtime records it.

### 9.5 A frame the binding cannot read

A frame whose header the binding cannot decode — an unknown `form`, a `kind`
outside 1 to 5, an `interface` or an `ordinal` of 0, a field the form requires
absent — is **discarded and recorded**, never partly applied. A `request` whose
header is readable but whose payload is absent where the form requires it is
answered `corrupt`. Whether a stream whose framing has been lost is torn down,
and how it is re-synchronized, is the binding's (§10); a torn-down session ends
as §3 states.

### 9.6 Which failures cross

Of the five `Transport` variants, exactly two cross a boundary, each as a
`response` outcome: **`Corrupt`**, because the provider detected it, and
**`Busy`**, because the provider refused the call at admission. `Timeout`,
`Undelivered` and `Down` are the **absence** of a frame, detected by the
caller's runtime against its own clock and its binding's session state, and
never sent by anyone. Of the four `Contract` categories, every one may cross as
a `response` or `answer` outcome. `Detected(_)` never crosses: it is the
consumer's own finding about bytes it received.

**What a violation carries.** `contract(InvalidValue(violation))` crosses as the
violation's `Rule` alone; the name of the typl type whose constraint failed does
not cross. The `Rule` codes: `Range` 1, `Step` 2, `Length` 3, `Pattern` 4,
`Variant` 5, `Unique` 6; a new `Rule` variant is added only together with a new
frame version, because a receiver cannot construct a variant it does not know. A
runtime that decodes the violation from a frame reports it with no type name,
which `ridl_rt::payload::Violation` represents as `type_name: None`; a violation
the generated binding detects locally keeps its name, and an in-process
transport delivers a violation with its name. A type identifier was rejected
because the catalog descriptor carries no type table, and the name is
diagnostic, not contractual.

## 10. What a Binding Decides

**A binding is written from this document alone.** A binding's author reads this
document, the `ridl-rt` design record for the exact shape of each name in §2,
and the projection record of the encoding the path carries. They do not read
`ridl-loopback`, the Rust backend, or another binding to learn what crosses. A
fact a binding needs that this document does not state is a defect in this
document, fixed here and not in the binding; a behaviour a binding adds that
this document does not state is not part of the frame, and two bindings that
differ on it do not interoperate through a relay.

A binding's own document states, and only states, the following. Each item is a
decision this document leaves to the transport on purpose, and a binding that
leaves one unstated is incomplete.

1. **The transport and the session.** How a consuming side reaches a providing
   side; what opens and what ends the session under §3; what "the session ended"
   is on that transport, so that `Detached` and `Transport::Down` have a
   trigger.
2. **The header encoding.** The byte layout, field order and width of every
   header field in §4, and how the `form` is carried. `InterfaceNo` and
   `Ordinal` keep their 32-bit range or the binding refuses at `attach` (§4).
3. **The encoding tag.** Whether the binding carries any of the three encodings,
   the `attach` tag naming the session's, or fixes one; when it fixes one,
   which, under the ADR-0020 decision 2 matrix, as a value of the table in §3;
   when it carries any, which default its transport offers.
4. **Framing.** Where one frame ends and the next begins on a stream transport;
   how a payload's length is carried; what the binding does when framing is lost
   (§9.5).
5. **The caller's identity** for duplicate suppression (§7): what the providing
   side derives it from — a connection, a process, a Binder identity — and
   whether it survives a reconnect.
6. **Retention of a settled response** for a call with no response bound (§7).
7. **Grouping.** Whether the frames of one commit cross as one unit (§5.1), and
   therefore whether a runtime over this binding presents `CoherentSignals`.
8. **What is mapped and what is framed.** On a path that maps a buffer rather
   than sending a frame — a shared-memory store, a Binder parcel read in place —
   which fields of §4 are carried in the mapped region's own layout and which in
   a frame beside it. The control plane is framed on every binding.
9. **The rate-floor answer** on a call (§8), when the binding enforces one.

A binding does not decide: the field set, the meaning of any field, what a
receiver does with a frame, the per-kind presence rules of §5, the invalid
payload rules of §9, or the control-plane operations and their answers.

## 11. The Bindings

Two bindings are named: one by the transport that carries it, and one by the
platform whose runtimes write their own. The WebSocket binding is specified in
§11.1 and its two transports are built: the Rust crate `ridl-transport-ws` and
the TypeScript package `@driftsys/ridl-transport-ws` at
`packages/ridl-transport-ws/`. A binder contract is not written in this
repository.

### 11.1 WebSocket

The WebSocket binding carries this frame between a consuming runtime that opens
a WebSocket connection and a providing runtime that accepts it.
`ridl-transport-ws` (driftsys/ridl#265,
[ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
decision 6) is its Rust crate, and `@driftsys/ridl-transport-ws`
(`packages/ridl-transport-ws/`, decision 7) is the same binding for Deno and the
browser; it carries payload bytes without decoding them, so the two ends of one
connection may be a Rust runtime and a TypeScript one. This section is the
binding document: it states the nine items of §10 and adds nothing to the frame.
The binding is **version 1** and carries frame version 1. The reasoning behind
each rule, with the alternatives considered, is in
[the binding rationale technote](../technotes/websocket-binding-rationale.md).

**1. The transport and the session.** One WebSocket connection carries one
session. The client is the consumer and sends `attach` as its first message; the
server is the provider and sends `attached` as its first message. The URL is the
deployment's; the binding places nothing in the path, the query or a header;
`ws://` and `wss://` are both allowed, and what `wss://` provides is the
security the deployment relies on (§13). The session ends when either side sends
a close frame, when the connection fails, or when the liveness check fails: the
provider sends WebSocket pings, and either end treats no message of any kind,
data or control, within its liveness timeout as the end of the session. The
provider pings because a browser client cannot. A consumer that needs two
catalogs opens two connections.

**The subprotocol.** The client offers `ridl-ws-v1` in `Sec-WebSocket-Protocol`;
a client offering several versions lists the newest first, and the server
selects the newest it serves. When the server selects none, the client fails the
connection; a client that can choose a close code uses 1002. On a `ridl-ws-v1`
connection, `attach.frame_version` is 1 or the `attach` is refused
`frame_version_unsupported`.

**2. The header encoding.** Every integer is fixed-width little-endian; `stamp`
is a two's-complement `i64`. There is no padding and no alignment, and no field
carries the payload's length: the header is the prefix below, and on a frame
that carries a payload, the payload is every byte after it. Every code is the
value §4 and §9.6 give it (`form`, `provenance`, `outcome`, a refusal reason, a
`Rule`), `kind` is the `Kind` value of §2 and `encoding` the tag of §3; this
section restates none. Fields are in this order, and a form carries exactly the
fields §4 gives it:

| Form                               | Bytes                                                                                                                     | Header size |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------- | ----------- |
| `attach`                           | `form` u8, `frame_version` u32, `encoding` u8, `hash` 32 bytes, `name_len` u8 (1 to 255), `name` UTF-8 (`name_len` bytes) | 39 + name   |
| `attached`                         | `form` u8, `outcome` u8, `detail` u8                                                                                      | 3           |
| `subscribe`, `unsubscribe`, `read` | `form` u8, `kind` u8, `interface` u32, `ordinal` u32                                                                      | 10          |
| `answer`                           | the common 10, then `outcome` u8, `detail` u8                                                                             | 12          |
| `publish`                          | the common 10, then `stamp` i64, `seq` u64, `provenance` u8                                                               | 27          |
| `occurrence`, `request`            | the common 10, then `stamp` i64, `seq` u64                                                                                | 26          |
| `response`                         | the common 10, then `stamp` i64, `seq` u64, `correlation` u64, `outcome` u8, `detail` u8                                  | 36          |

`detail` is the `Rule` code (§9.6) with `outcome` 3, the refusal reason code
(§4) with `outcome` 9, and 0 with every other outcome. The `name` is at least
one byte and valid UTF-8. A consuming runtime whose unit name is longer than 255
bytes of UTF-8 cannot attach over this binding: it reports an error when the
session is opened and sends nothing, so no refusal reason exists for that case.
A value outside its table is a protocol violation (item 4). `InterfaceNo` and
`Ordinal` keep their 32-bit range, so §4's range check at `attach` always passes
on this binding.

**Whether a frame carries a payload** is decided by its header, never by whether
bytes follow the header: by its `form`, by the `provenance` of a `publish` and
by the `outcome` of a `response`. From §5: an `occurrence` and a `request`
always carry one; a `publish` carries one under `Live` (a signal's publication,
and every `publish` of a `fixed`) and none under `Init` or `Invalid(Declared)`;
a `response` carries one under `reply` and none under any other outcome;
`attach`, `attached`, `subscribe`, `answer`, `unsubscribe` and `read` never
carry one. `kind` does not change the answer. A payload that is present may be
zero bytes long — a proto3 value whose fields all hold their defaults encodes to
no bytes — and is handed to the generated binding as it is.

**The sizes.** The envelope is 16 bytes. The largest data-plane header without
its envelope is 20 bytes (`response`), so a data-plane message is at most 36
bytes plus the payload bound. The largest header of any message is the `attach`
at 39 + 255 = 294 bytes; it carries no payload, so a receiver's message size
limit admits at least 294 bytes. The codegen model carries the data-plane
numbers as the `websocket` row, binding version `"1"`, `frame_header_max_bytes`
20, `envelope_bytes` 16 (`crates/ridl-ir/src/codegen/bindings.rs`). The row is
derived from this layout: the layout is frozen through the vectors below, and
the row is a derived value, corrected in a patch release if it is wrong
([ADR-0029](../decisions/ADR-0029-stability-policy.md) decision 2).

**3. The encoding tag.** The binding fixes no encoding: a session uses the
encoding its `attach` names, and a provider that does not serve it refuses
`encoding_unsupported`. The default a program gets when it names none is the
transport's, proto3, and the construction can name another.

**4. Framing.** One frame is one WebSocket message with the binary opcode;
WebSocket fragmentation is below this binding. A text message is closed with
1003. A message the receiver cannot read — shorter than its header, a
control-plane message other than `attach` that is longer than its header, a code
outside its table, `interface` or `ordinal` 0, a message before `attach`, a
second `attach`, a `name_len` that does not match the message, a `name` that is
empty or not valid UTF-8, a first provider-to-consumer message that is not
`attached` — is a protocol violation: the receiver records the frame (§9.5) and
closes with 1002, and the session ends as §3 states. A `publish` or a `response`
whose header says it carries no payload and which has bytes after its header is
discarded and recorded (§9.5). A frame whose header says it carries a payload is
never answered or discarded for the payload's length: the payload, zero bytes
included, goes to `Payload::verify`, and a payload that fails to decode follows
§9 — a `request`'s is answered `corrupt` (§5.3). So the `request` of §9.5 whose
payload is absent does not occur on this binding. A message above the receiver's
configured size limit, which admits at least 294 bytes, is closed with 1009.
Either side ends the session on purpose with 1000, the provider with 1001 when
it shuts down; after `attached(refused)` the provider closes with 1000, and the
reason is in the frame. The binding defines no close code of its own.

**5. The caller's identity** for §7 is the connection. It does not survive a
reconnect: a reconnect is a new session, its counters start over, and the calls
of the old session were reported `Transport::Down` when it ended. The binding
never retransmits a `request`: the transport is reliable and ordered, so a
message is lost only when the connection fails, which ends the session. Every
call has an outcome, because every call has a response bound
([ADR-0015](../decisions/ADR-0015-qos-absorption-and-rpc-bounds.md) decision 4):
the `response`, `Undelivered` or `Timeout` at the bound (§8), or `Down` at the
session's end (§3).

**6. Retention.** The provider keeps a settled response until the request's
bound has passed, as §7 states; no call is without a bound, so the binding
states no further retention.

**7. Grouping.** None. The frames of one commit cross as separate messages, and
a runtime over this binding does not present `CoherentSignals`.

**8. Mapped and framed.** Everything is framed; nothing is mapped.

**9. The rate-floor answer** is §8's: a call faster than the member's `min` may
be refused at admission and answered `busy`. The binding adds nothing.

**Later versions.** An addition to the frame — a form, a field, a code — is a
new frame version carried by a new binding version, offered beside this one in
the subprotocol list. Version 1's layout, codes and vectors are never edited.
Streams (ridl §12, driftsys/ridl#336) arrive this way.

**Test vectors.** Each line is one complete message in hexadecimal, spaces for
reading only; `xtask` reads this table into the `wire` freeze dump by its `Id`
column, which is stable and never renamed. The stamp in every data-plane vector
is `0x0123456789AB` microseconds (and `0x0123456789AC` on the responses), chosen
to show the byte order; the hash in `attach` is the bytes `00` to `1f`; the unit
name is `veh.cabin`. `xtask` also reads the payload a `Vector` cell names as
`` payload `<hex>` `` and fails a row whose bytes after the header differ from
it, or from no bytes when the cell names none; it decides from the header
whether a frame carries a payload, by the rule "Whether a frame carries a
payload" above.

| Id                   | Vector                                                          | Hex                                                                                                     |
| -------------------- | --------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| `attach-1`           | `attach` frame 1, FlatBuffers, `veh.cabin`                      | `01 01000000 01 000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f 09 7665682e636162696e` |
| `attached-accepted`  | `attached` accepted                                             | `02 01 00`                                                                                              |
| `attached-mismatch`  | `attached` refused `catalog_mismatch`                           | `02 09 02`                                                                                              |
| `subscribe-signal`   | `subscribe` signal, interface 1, ordinal 2                      | `03 01 01000000 02000000`                                                                               |
| `answer-accepted`    | `answer` accepted, signal, interface 1, ordinal 2               | `04 01 01000000 02000000 01 00`                                                                         |
| `publish-live`       | `publish` `Live`, seq 7, payload `ff01`                         | `07 01 01000000 02000000 ab89674523010000 0700000000000000 02 ff01`                                     |
| `request-command`    | `request` command, interface 1, ordinal 3, seq 1, payload `01`  | `09 03 01000000 03000000 ab89674523010000 0100000000000000 01`                                          |
| `response-accepted`  | `response` command accepted, correlation 1, provider seq 1      | `0a 03 01000000 03000000 ac89674523010000 0100000000000000 0100000000000000 01 00`                      |
| `response-violation` | `response` query `contract(InvalidValue(Range))`, correlation 2 | `0a 04 01000000 04000000 ac89674523010000 0200000000000000 0200000000000000 03 01`                      |

### 11.2 Binder on Android

On Android, a runtime binds the ports over its own binder contract, which may be
one generic, versioned AIDL interface serving every catalog. ridl specifies no
Binder layout and no transaction code: the contract's methods, and whether it
carries an ordinal as a transaction code or as a field, are the runtime's. Such
a binding is written from this document like any other: it states the nine items
of §10 for Binder, adds nothing to the frame, and follows §4 where it carries an
interface number or an ordinal in a field narrower than 32 bits. Its path is
"passed within a node", so its payload encoding is **FlatBuffers** (encoding tag
1). A binding is the runtime's and not a codegen backend's: the Kotlin backend's
generated faces run over a Kotlin runtime that binds the ports this way.

> **Reversed decision.** Until 2026-09-25 this section fixed an AIDL binding
> generated per interface by the Kotlin backend, with the ordinal as the
> transaction code, taken 2026-09-22. driftsys/ridl#516 reversed it.

### 11.3 Bindings this document does not name

The access service of
[ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md) decision
18 — `Read`, `Subscribe`, `Call`, `Send`, kind-blind and ordinal-keyed over gRPC
— is a binding of this frame in that record's own words; it is a published
schema a consumer opts into, not a story here. A shared-memory store and queue
are the engine's, outside this repository. Both, when written, take the form of
§10.

## 12. Conformance

A second implementation — a runtime, a binding, or both — conforms when every
statement below holds and can be shown by a test that does not read this
repository's code. The list is the observable consequences of §3 to §9, chosen
so that each catches one way of being wrong.

1. An `attach` naming a catalog the provider serves under a hash that is neither
   the provider's own nor one its descriptor lists as compatible is refused
   `catalog_mismatch`, and no other frame is exchanged; one naming a listed
   earlier hash is accepted (§6.1; the toolchain writes the list, and no runtime
   in this workspace accepts a listed hash, per the As-built note).
2. A `subscribe` to a signal is followed by exactly one `publish` before any
   provider action: `Init`, `seq` 0, no payload, before a first publication; the
   last publication otherwise (§5.1, §6.2).
3. A `subscribe` to an event is followed by nothing until the provider raises
   one, and an occurrence raised before the `answer` never arrives (§5.2).
4. A `set` and an `invalidate` of one channel, committed, each arrive as one
   `publish` whose `seq` is one greater than the previous; the invalidation
   carries no payload, and the consumer's sample keeps the last good value under
   `Invalid(Declared)` with the invalidation's envelope (§5.1).
5. Two `publish` frames of one channel delivered in the wrong order leave the
   consumer holding the one with the greater `seq` (§7).
6. A `publish` whose payload breaks a typl constraint leaves the consumer
   holding the last good value under `Invalid(Detected(InvalidValue(_)))` with
   the rejected frame's envelope, and the next valid `publish` returns the
   channel to `Live` (§9.1).
7. An `occurrence` whose payload is not a well-formed encoding is delivered as
   `Err(Corrupt)` with its envelope, not dropped (§9.2).
8. A command whose `require` clause is false at the caller sends no `request`
   (§5.3).
9. A command `request` whose argument breaks a typl constraint is answered
   `contract(InvalidValue(_))` and the provider's method is not called; one
   whose `require` clause is false at the provider is answered
   `contract(PreconditionFailed)` likewise; one that passes both is answered
   `accepted` **before** the provider's method runs (§5.3).
10. A query whose `ensure` clause is false is answered
    `contract(ContractBroken)` with no payload; one that passes is answered
    `reply` with the reply payload **after** the provider's method returns
    (§5.4).
11. A `request` retransmitted with the same envelope after its `response` was
    sent is not presented to application code again and receives the same
    `response` (§7).
12. Two callers whose first calls both carry `seq` 1 are presented as two claims
    (§7).
13. A `request` naming an ordinal the catalog does not hold, or holding it under
    another kind, is answered `contract(UnknownInteraction)` (§6.4).
14. A `read` of a `fixed` is answered with one `publish` of `kind` `Fixed`,
    `seq` 0 and the value; a `subscribe` to it is refused (§5.5, §6.2).
15. The frames of one commit carry one `stamp` (§5.1).
16. No frame carries a timing bound, an init value, a `ClaimId`, a `Correlation`
    as the port hands it out, a `Detected(_)` provenance, or a `Timeout`,
    `Undelivered` or `Down` outcome (§8, §5.1, §2, §9.6).

## 13. Not in This Version

Each item is left out on purpose, with the record that takes it.

- **Streams.** `<T>` on a call's parameter or return (ridl §12) has no frame in
  version 1: `ridl-rt` 0.1 leaves streams out (driftsys/ridl#336). They arrive
  as frame version 2, negotiated as §11.1 "Later versions" states, so that a
  version 1 peer is unchanged.
- **The family.** Which of ridl §3.2's five boundaries an interaction sits on is
  not on the frame, because `ridl-rt` 0.1 carries no `Family` and the IR has no
  family field yet; it returns as a `Member` field, a breaking change under
  [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decision 10,
  and the frame gains it then if a binding needs it.
- **The canonical encoding of a payload** (typl §17.10): the header's part is
  §4; the bytes of each encoding are each projection record's.
- **A wake hook.** The frame is push: a runtime learns of a frame from its
  binding. How it wakes an application that is waiting on a reply, an occurrence
  or a claim is the runtime's, not the frame's: `port::Wakeable`
  ([ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decision 13,
  which closed its open item 6).
- **Unknown fields on decode**
  ([ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md) open
  item 3) is a payload question, the proto3 projection's.
- **A relay or a bridge** that forwards frames between sessions keeps every
  envelope it forwards and adds nothing to it; anything more is
  [ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md)
  decision 13's, outside this version.
- **Security** — who may attach, and whether a session is authenticated or
  encrypted — is the transport's and the deployment's, and a binding states what
  its transport provides.

---

## Trace

- Roadmap: [`docs/ROADMAP.md`](../ROADMAP.md) — the frame specification, the
  WebSocket binding, and "After step 2 — Kotlin, the first external plugin"
  states the Binder statement of §11.2
- Tracking issue: driftsys/ridl#257
- Binds: [ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md)
  decisions 1, 7 and 18;
  [ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
  decisions 2, 6 and 7;
  [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decisions 2,
  3, 5, 6, 7 and 14 (the last, `busy` as a `response` outcome,
  driftsys/ridl#510);
  [ADR-0015](../decisions/ADR-0015-qos-absorption-and-rpc-bounds.md) decisions
  3, 4 (every call has a response bound, §11.1 item 5), 10 and 17
- Vocabulary: [the `ridl-rt` design record](../design/ridl-rt.md), and
  `crates/ridl-rt/src/` — `contract.rs`, `sample.rs`, `port.rs`, `error.rs`,
  `payload.rs`, `encoding.rs`
- Semantics: the [ridl language reference](ridl-language-reference.md) — §3.1
  the envelope, §4 signal, §5 event, §6 command, §7 query, §8 fixed, §9 timing,
  §10 the error strata, §11 identity, Appendix B the AIDL column
- The one runtime as built:
  [the `ridl-loopback` design record](../design/ridl-loopback.md); the generated
  face over it:
  [the interaction-face design record](../design/interaction-face.md)
- The Binder statement: driftsys/ridl#516, which reverses the earlier AIDL
  binding (§11.2)
