# Streams — the port, the descriptor, the frame and the generated surfaces

Working design for driftsys/ridl#1116 and driftsys/ridl#336. It settles what
[ADR-0032](../decisions/ADR-0032-byte-streams.md) left to the build: the stream
port of `ridl-rt` and how its `std` wrapper is woken (decision 9), the stream
fields of the catalog descriptor (catalog descriptor D-4), the frame version
that carries a stream (decision 11), and the generated Rust and TypeScript
surfaces (decision 6). ADR-0032 decisions 9 to 14 are binding here; every other
choice is a ruling, numbered R-1 to R-35, each with a one-line reason, so that
the owner can check them while reading. "Alternatives considered" follows every
ruling that had a credible alternative. The owner's rulings of the first builder
check (2026-10-31) are recorded as R-11 (replaced), R-31 to R-35, and the
amendments marked in R-5, R-7 to R-10 and R-14.

The document defines the end state first, then states what each of the six
stages delivers, in the order the owner fixed. Each stage is independently
mergeable: a stage that lands alone leaves the workspace green and every earlier
stage's behaviour unchanged.

## 1. The driving use case

`query getFaultPage(filter: DiagFilter): FaultPage | DiagError @[..100ms]` in
`crates/ridlc/tests/corpus/veh-cluster/cluster/appendix-a.ridl` is the ridl
reference's Appendix A, compiled. The appendix declares
`FaultPage.faults: [FaultEvent; 0..64]`, whose largest encoding is about 70,000
bytes in FlatBuffers and 67,000 in proto3 and in podbuf (`NOTES` of the corpus),
above the 16,128-byte default limit of a query. The corpus narrowed the page to
`0..14` to compile. A page is a workaround for the absence of streams: the
author wants every fault that matches the filter, and the bound `0..64` is the
size of one transfer, not a fact about faults.

The same interface already declares
`query streamFaults(filter: DiagFilter):
<FaultEvent> @[..1s]`. So the first
consumer of streams is a **query with a streamed reply of named elements**, and
the first stage of the Rust face (stage 3) serves exactly that shape.

**The result-union element.** ADR-0032 decision 10 and ridl §12.4 tell an author
who needs a failure inside the stream to declare a named element type that is a
result union, and both write the example as `<DocChunk | DocError>`. That
spelling is **not legal today**: the grammar
(`crates/ridl-syntax/family.ungram`) defines
`StreamType = '<' (element_type:PathType | 'string' | 'bytes') '>'`, so the
element is one path, and `ridl check` reports FORM-101 "expected `>`" at the
`|`. The inline `T | E` form exists only as a whole return type
(`FallibleType`). The author writes the union as a declaration and names it:

```ridl,ignore
union FaultResult {              // result union, typl §10.2: one success arm, one error arm
  fault   : FaultEvent
  failure : DiagError
}

query faults(filter: DiagFilter): <FaultResult> @[..1s]
```

`ridl check` accepts `<FaultResult>` with no diagnostic. This document adds no
syntax (the owner's constraint), so the inline spelling stays illegal; the two
records that show it are corrected to the named form in stage 1 (section 6). The
arm cannot be named `error`, which is a keyword.

## 2. Vocabulary

- **Stream**: the ordered sequence of chunks that one call carries in one
  direction, from its start to its end. A call has at most one **reply stream**
  (a `query` whose return type is `<T>`) and zero or more **parameter streams**
  (each parameter whose type is `<T>`).
- **Chunk**: the unit the port and the frame carry. For a stream of named
  elements a chunk is one encoded element, whole. For a byte stream (`<bytes>`)
  and a text stream (`<string>`) a chunk is a run of bytes whose length the
  producer's side chose (ADR-0032 decision 2); a text chunk holds whole code
  points (decision 3). Every chunk is at most the interaction's limit (decision
  14).
- **Stream number** (`StreamNo`, a `u8`): 0 names the reply stream; `k` from 1
  names the `k`-th stream parameter, counting only the stream parameters, in
  declaration order.
- **End**: the explicit end of a stream, sent by its producer (decision 11).
  **Interruption**: a stream that stops without its end.

**R-1. Streams are numbered on the call, so a call may declare several stream
parameters.** Reason: ridl §12 allows any parameter to be a stream, and a `u8`
on the port key and on the frame costs less than a new restriction on sources
that compile today. _Alternative considered_: one stream parameter per call,
enforced by a new error (RIDL-203). Rejected: it is a break of the "set of
sources that compile without an error" surface (ADR-0029 decision 1) for no
gain, since the index is cheap.

## 3. The stream port of `ridl-rt`

### 3.1 The traits

Two traits, one per side, each extending the call trait of its side. Every
method returns without waiting and copies into or out of a buffer the caller
supplies (ADR-0032 decision 9; ADR-0018 decision 2).

```rust
// port:: (`StreamNo` is declared in `contract`, R-7, and re-exported here)
pub const REPLY_STREAM: StreamNo = StreamNo(0);

/// The outcome of one pull.
#[must_use]
pub enum Pull {
    /// One chunk was copied into `out`; its length.
    Chunk(usize),
    /// The producer ended the stream. Pulling again answers `End` again.
    End,
    /// Nothing yet.
    Pending,
    /// The stream stopped without its end, or was refused: the call's outcome.
    Failed(CallError),
}

pub trait StreamCaller: Caller {
    /// Copies the next chunk of stream `s` of call `c` into `out`.
    fn pull(&mut self, c: Correlation, s: StreamNo, out: &mut [u8]) -> Result<Pull, ReadError>;
    /// Offers one chunk to stream `s` of call `c`. Answers `chunk.len()` when the
    /// chunk was taken, 0 when the window is full and nothing was taken.
    fn push(&mut self, c: Correlation, s: StreamNo, chunk: &[u8]) -> Result<usize, PushError>;
    /// Ends stream `s` of call `c`.
    fn end(&mut self, c: Correlation, s: StreamNo) -> Result<(), PushError>;
    /// Stops stream `s` of call `c` with outcome `e`, from either side of it:
    /// the producer interrupts, or the receiver refuses what it received.
    fn abort(&mut self, c: Correlation, s: StreamNo, e: CallError) -> Result<(), PushError>;
}

pub trait StreamHandler: Handler {
    fn pull_claim(&mut self, claim: ClaimId, s: StreamNo, out: &mut [u8]) -> Result<Pull, ReadError>;
    fn push_claim(&mut self, claim: ClaimId, s: StreamNo, chunk: &[u8]) -> Result<usize, PushError>;
    /// Ends stream `s` of the claim. For `REPLY_STREAM` this is the claim's
    /// settlement: it settles `Ok(&[])` and no `settle` follows.
    fn end_claim(&mut self, claim: ClaimId, s: StreamNo) -> Result<(), PushError>;
    fn abort_claim(&mut self, claim: ClaimId, s: StreamNo, e: CallError) -> Result<(), PushError>;
}

#[non_exhaustive]
pub enum PushError { TooLarge { cap: usize }, Ended, Failed(CallError), Contract(Contract), Detached }
```

`ReadError` keeps its meaning from `Caller::reply`: the outer error is the port
call itself (`Short { needed }` when the chunk does not fit `out`, which does
not consume it; `Contract(UnknownInteraction)` when the call or the stream
number is unknown; `Detached`), and `Pull::Failed` is the outcome from the peer
or the transport. `PushError` has the same two layers: `Contract(Contract)` and
`Detached` are the port call itself; `Ended` is a push, an `end` or an `abort`
after the stream ended; `Failed(e)` is the other side's verdict on the stream:
`Contract(c)` when the receiver refused a chunk at its boundary (R-33),
`Transport(Corrupt)` when it could not decode one, `Transport(Interrupted)` when
it cancelled the stream or forgot the call; `TooLarge` is a chunk longer than
the member's limit, refused on send as `SendError::TooLarge` refuses arguments
today (R-11).

**R-2. The port's unit is the chunk, and a port never splits or merges one.**
Reason: a port names no payload type, so it cannot know whether a stream is a
byte stream or a stream of elements; keeping each chunk whole is correct for
both forms, and a `Short` on a chunk follows the rule `next`, `next_claim` and
`reply` already have. _Alternative_: byte-granular reads and writes, where a
pull fills `out` from several chunks and a push may take part of a chunk.
Rejected: it is wrong for an element stream, and the port would need the stream
form to choose.

**R-3. A push takes the whole chunk or nothing; the count it returns is
`chunk.len()` or 0.** Reason: ADR-0032 decision 9 says a write "reports how many
bytes the port accepted" and that fewer than offered is the backpressure of ridl
§12.2; 0 is fewer, and partial acceptance would split a chunk (R-2). The window
that makes a push answer 0 is the runtime's: `ridl-loopback` holds
`Loopback::STREAM_WINDOW` chunks (4) per stream and direction. _Alternative_: a
`bool`. Rejected: it reads against decision 9's words.

**R-4. A reply stream is settled by its end; a plain settlement is not a reply
stream.** `end_claim(claim, REPLY_STREAM)` settles the claim `Ok(&[])`, and
`settle(claim, Err(e))` at any time ends the reply stream with `Failed(e)`,
after the chunks already pushed. On the calling side `Caller::reply` reports the
settlement as it reports a query whose reply has no bytes; the generated face of
a streamed query never calls it and reads `pull` instead. Reason: a runtime with
no member table (`ridl-loopback`) cannot know a member's reply is a stream, so
the provider's actions must define it, and the existing settlement and
correlation machinery is reused unchanged. _Alternative_: an `open_stream`
method on the handler. Rejected: a stream with no chunk and no end is a stream
that has not started, which the first `push_claim` already says.

**R-5 (amended). A side that stops a stream on purpose aborts it; `Transport`
gains `Interrupted`.** A producer interrupts with
`abort(c, s, CallError::Transport(Transport::Interrupted))` or `abort_claim`;
`settle(claim, Err(e))` on a streamed-reply claim is the same as
`abort_claim(claim, REPLY_STREAM, e)`. A caller's `forget(c)` aborts every
stream of the call with `Interrupted`, after which the provider's `pull_claim`
answers `Failed(Transport(Interrupted))` and its `push_claim`
`PushError::Failed(Transport(Interrupted))`. The variant is new; `Transport` is
`#[non_exhaustive]` (ADR-0021 decision 9), so adding it is not a break. Reason:
ADR-0032 decisions 10 and 11 make a mid-transfer failure of a byte stream reach
the consumer only as an interruption, and without an explicit way to interrupt,
a provider's only option would be to end the session. `Timeout`, `Undelivered`
and `Down` keep their meanings.

**R-6. `Interest` gains two keys, `Stream(Correlation, StreamNo)` and
`ClaimStream(ClaimId, StreamNo)`.** A `Stream` waker is stored with its call, as
an `Outcome` waker is, and is woken when a chunk or the end arrives on that
stream, when the window of that stream frees, or when the call is forgotten or
settled; `ClaimStream` is the same on the handler's side. Reason: a face that
waits registers interest and pulls, and the existing keys do not name a stream.
`Interest` is exhaustive, so this is a breaking change of `ridl-rt` (ADR-0021
decisions 10 and 13), released as a 0.x minor (R-35).

**R-32. The stream ports are optional extensions, as `Wakeable`,
`ScannableSignals` and `CoherentSignals` are.** No runtime is required to
implement `StreamCaller` or `StreamHandler`; neither is in the
`Factory::Runtime` bound of `ridl-rt-conformance` nor in `Backend::Ports` of
`ridl-transport-ws`. A consumer that may or may not have them asks through a
capability accessor, as `Backend::scannable()` does: `Backend` gains
`fn streams(&mut self) -> Option<&mut dyn StreamCaller>` with a default of
`None`, and the conformance suite gains a `streams` extension —
`suite!(F; wakeable, streams)` — whose tests are bounded
`where F::Runtime: StreamCaller + StreamHandler + Wakeable` and need no new
`Factory` item: the window test pushes until a push answers 0 or until 1,024
chunks, and a runtime whose window is never full passes that test with the cap.
The generated `Client<P>` adds `StreamCaller` to its bounds only when the
interface declares a member with a stream, and `serve`'s `H` adds
`StreamHandler` under the same condition: the emitter already computes the bound
set from the interface's interaction kinds (RA-19 of
`docs/design/interaction-face.md`), and a stream is one more fact read from the
same slots, so an interface with no stream is bound as today. Reason: a runtime
without streams must stay a complete runtime, and the project's rule for a
mechanism some runtimes have is an extension trait plus an accessor.
_Alternative_: supertraits on `Caller` and `Handler`. Rejected: every runtime
and every test double would have to implement them.

The breaks this leaves (restated after R-32): `ridl-rt` — `Member` gains a field
(R-7) and `Interest` gains two variants (R-6); nothing else, since the new
traits, `Transport::Interrupted`, `Rule::Utf8` and the `face` traits are
additive. `ridl-rt-conformance` — additive (a new module and extension, no new
required item). `ridl-loopback` — additive. `ridl-transport-ws` — additive:
`Frame` and `Note` are `#[non_exhaustive]`, `Backend::streams` has a default,
and the new constants are new items. `ridl-descriptor` — breaks, in the same
release: `ridl_descriptor::Member` gains a public field (`streams`), and its
accessors are additive.

### 3.2 Flow

Reply stream: the provider takes the claim through `next_claim` as today, then
`push_claim(claim, REPLY_STREAM, chunk)` for each chunk until a push answers 0,
waits on `ClaimStream`, pushes again, and `end_claim`s. The caller sends the
query, then `pull(c, REPLY_STREAM, buf)` until `End` or `Failed`, then
`forget(c)`.

Parameter stream: the caller sends the call, then `push(c, k, chunk)` and
`end(c, k)`; a push may precede the claim, and the runtime holds up to the
window. The provider pulls with `pull_claim(claim, k, buf)` until `End`. A
command's claim settles `Ok(&[])` after its arguments and `require` pass, before
any chunk is read, as ridl §6.1 and ADR-0032 note (a) state; a constraint
failure in a chunk after the acknowledgment ends the stream with the Stratum 2
category and reaches the producer through its next `push`, not through the
contract (R-33).

### 3.3 How the `std` wrapper is woken

The wrapper follows the pattern every generated future already uses
(`docs/design/ridl-rt.md`, "`Wakeable` is the extension a face that waits is
built on"): on every poll it calls
`wake_on(Interest::Stream(c, s), cx.waker())`, then pulls; `Pull::Pending`
returns `Poll::Pending`, anything else resolves the poll. The same on the
provider's side with `ClaimStream`. The push side registers the same key before
a push and returns `Poll::Pending` when the push answered 0. No new mechanism: a
runtime that implements `Wakeable` and the two new keys serves the wrapper.

### 3.4 Descriptors in `ridl-rt`

**R-7 (amended). `Member` gains `streams: &'static [StreamInfo]`, one entry per
stream of the member in ascending `StreamNo`; `PayloadInfo` is unchanged.**

```rust
// contract::
pub struct StreamNo(pub u8);                       // moved here from port::, which re-exports it
pub enum StreamForm { Elements, Bytes, Text }
pub struct StreamInfo { pub no: StreamNo, pub form: StreamForm, pub element: &'static str } // element "" under Bytes and Text
```

A member with no stream has an empty slice. Reason: R-31 gives every carrier one
home for a stream's number, form and element, and `Member` is the carrier the
generated `MEMBERS` table builds as a struct literal (ADR-0021 decision 10), so
this is the one breaking field of `ridl-rt` in stage 1. _Alternative_: leave the
runtime descriptor unchanged, because no shipped runtime reads the form.
Rejected: a runtime that carries generated descriptors but no catalog file (a
shared-memory runtime, a Rust client of the WebSocket binding) would have no way
to know which streams a member has.

## 4. The catalog descriptor and the codegen model

The IR does not change: `StreamType { named | primitive }` already tells a byte
stream from a stream of named elements (ADR-0032 consequences). Because the
catalog hash is SHA-256 over the reduced IR, **no catalog hash changes** in any
stage, and the `catalog-hash` line of the `wire` dump holds.

**R-31. One home for a member's streams in every carrier: a list of stream
entries, one per stream, each with its `StreamNo`, its form and its element; the
request stays the induced struct of the non-stream parameters (R-10).** The
owner's ruling of 2026-10-31, decided in stage 1. Today the request row spells
the whole parameter list, stream included
(`crates/ridl-descriptor/src/lower.rs`, `request`), and after R-10 nothing would
record how many streams a member has or which number each one carries. Reason:
the frame (section 5), the port (section 3) and the faces (sections 7 and 8) all
key a stream by `(call,
StreamNo)`, so the descriptor has to say which numbers
exist and what each one carries, in one place a reader need not infer from a
spelling.

**R-8 (amended). `Member` of `catalog.fbs` gains `streams: [StreamPayload]`,
appended; `Payload` is unchanged; `SCHEMA_VERSION` stays 1.**

```text
enum StreamForm : ubyte { Elements = 0, Bytes = 1, Text = 2 }
table StreamPayload {
  no: ubyte;                      // StreamNo: 0 the reply stream, k the k-th stream parameter
  form: StreamForm;
  element: string;                // canonical element type name under Elements; absent otherwise
  max_sizes: [MaxSize] (required);
}
table Member { ...; limit: uint32 = null; streams: [StreamPayload]; }
```

Entries are in ascending `no`. The `request` payload's `type_name` becomes the
spelling of the non-stream parameters (R-10), so
`replayTrace(samples:
<FilterState>)` has the request `()` and one entry
`{ no: 1, form: Elements,
element: "veh.cluster.FilterState" }`; the `response`
payload of a streamed-reply query keeps `type_name` `<T>`, as today, beside its
entry `no: 0`. A file written before the field verifies and reads `streams` as
absent, as `limit` and `compatible` did (D-3 of the catalog descriptor record).
`ridl describe` prints `streams`. Reason: an appended table list is the one
schema change the append-only rule allows, and it is R-31's home.

**R-9 (amended). Sizes: the request's sizes are the induced struct's; a stream
entry's `max_sizes` are its element's size states under `Elements` and absent
under `Bytes` and `Text`; the `response` payload of a streamed reply keeps no
row.** In the codegen model, `CommandShape.request_sizes` and
`QueryShape.request_sizes` are therefore the induced struct's for every call (as
`model.proto` already states for `request_tuple`), `reply_sizes` keeps
`ABSENT_CAUSE_ENCODING_UNDEFINED` for a streamed reply, and a new
`Interaction.streams: repeated StreamShape { uint32 no; StreamForm form; TypeRef
element; PayloadSizes sizes; }`
carries each stream's element and its states. Reason: the per-element bound is
what sizes a face's chunk buffer and what the codecs already compute for the
element's declaration; a byte chunk's only bound is the member's `limit`. **What
the Kotlin plugin reads**: `Interaction.streams` for the number, form, element
and sizes of each stream; `Param.type` and `Reply.value` keep their `StreamType`
for the declared shape; `request` or `request_tuple` for the non-stream
arguments, present for every call. An older plugin that reads neither `streams`
nor the new comment sees a request struct it did not see before and no stream,
so it emits a plain call that sends the request without its stream: by
`docs/book/stability.md` that is a break of the plugin protocol, not an
addition, and the release states it (ruling U-7). A streamed reply adds nothing
to `Interaction.reservation`, so the model's reservation agrees with the
backend's `RESERVATIONS` row (ruling U-6).

**R-10 (amended). A call's request payload is its non-stream parameters.** The
lowering induces the argument struct over the non-stream parameters, zero
parameters included, so `command put(info: DocInfo, data: <bytes>)` has the
request `DocInfo` and one stream parameter, and
`command upload(data:
<FwBlock>)` has an induced struct with no field. `request`
or `request_tuple` is therefore present for every call, and the comment "no
induced argument struct, because a parameter is a stream" goes. This changes the
frozen veh-cluster copy's output in stage 1: `replayTrace` and `rebalance`
(`xtask/tests/freeze-fixtures/generated/veh-cluster/cluster/services.ridl`) gain
induced params structs (`VehHvacCabinReplayTraceParams`,
`VehHvacCabinRebalanceParams`, on the model of `VehHvacCabinGetBoundsParams`),
which adds `.fbs` and `.proto` lines to the `wire` dump and items to the
generated crate — additive, but `cargo xtask freeze update` is needed for both
rows. Reason: the `request` frame and `Claim.len` carry the non-stream
arguments, and the face needs one named type for them, as it does today.
_Alternative_: refuse a stream parameter beside other parameters. Rejected:
ADR-0032 decision 5's own example needs both.

**R-11 (replaced; the first form is withdrawn). No diagnostic for a named
element whose largest encoding is above the interaction's limit; the sending
side refuses such an element at push, and it is never sent.** ADR-0032 decision
14 is binding: the sizer gives a stream no size and RIDL-157 does not apply to a
stream, and every source keeps the diagnostics it draws today. At run time the
generated face encodes the element into its `BUFFER`; when the encoded length is
above the member's `limit`, `push` or `push_claim` answers
`PushError::TooLarge { cap }`, nothing crosses, and the face aborts the stream
with `Transport::Interrupted` (R-5), which the receiver reads as an
interruption; the application's `Source` is dropped, or its `Consumer` is told
`failed`. Reason: the owner's ruling, and decision 14's statement that the
checker does not change. A possible follow-up, not a ruling: a design lint
(ADR-0027) could warn when an element type's bound is above its interaction's
limit, since such a stream can never carry that element.

## 5. Frame version 2

Frame version 2 carries every form of version 1 unchanged, byte for byte, and
adds three forms, two outcome codes and one `Rule` code. The WebSocket binding
offers it as `ridl-ws-v2`, beside `ridl-ws-v1`; a client lists `ridl-ws-v2`
first and the server selects the newest it serves (frame specification §11.1,
"Later versions"). Version 1's layout, codes and vectors are not edited.

### 5.1 The forms

| Form            | Code | Plane | Direction | Meaning                                                                                  |
| --------------- | ---- | ----- | --------- | ---------------------------------------------------------------------------------------- |
| `chunk`         | 11   | data  | both      | one chunk of one stream of one call                                                      |
| `stream_end`    | 12   | data  | both      | the end of one stream, normal or not; also the consumer's cancel of a stream it receives |
| `stream_credit` | 13   | data  | both      | the receiver of a stream grants the sender `credit` more chunks                          |

Header layouts, after the version 1 rules (fixed-width little-endian, no
padding, no length field):

| Form            | Bytes                                                                                                 | Header size |
| --------------- | ----------------------------------------------------------------------------------------------------- | ----------- |
| `chunk`         | the common 10, then `stamp` i64, `seq` u64, `correlation` u64, `stream` u8                            | 35          |
| `stream_end`    | the common 10, then `stamp` i64, `seq` u64, `correlation` u64, `stream` u8, `outcome` u8, `detail` u8 | 37          |
| `stream_credit` | the common 10, then `correlation` u64, `stream` u8, `credit` u32                                      | 23          |

`correlation` is the request's `seq`, the caller's name for the call, on every
stream frame in both directions, as it is on a `response`. `stream` is the
`StreamNo` of section 2. `chunk` always carries a payload, which may be zero
bytes; `stream_end` and `stream_credit` never do. `seq` on `chunk` and
`stream_end` is one counter per stream and direction, from 1 for the first
chunk; the `stream_end` takes the next value, so a receiver that sees a gap
treats the stream as interrupted (ADR-0032 decision 11) without waiting.

**R-12. `chunk` and `stream_end` carry the envelope; `stream_credit` does not.**
Reason: every data-plane form of version 1 carries one, the stamp of the first
chunk is what a recorder reads for decision 13's bound, and the `seq` gives the
gap rule above; a credit is a control fact of the receiver and has no sequence
of its own. The cost is 16 bytes on a chunk of up to 16,128. _Alternative_: no
envelope on stream frames, so `stream_end` would fit the 20-byte header maximum
of version 1. Rejected: the binding row `frame_header_max_bytes` is a derived
value, not a frozen one (ADR-0029 decision 2), and uniformity is worth 16 bytes.

**The sizes.** `chunk` is 35 + at most 16,128 = 16,163 bytes, `stream_end` 37,
`stream_credit` 23, all under `MAX_MESSAGE_LIMIT` (16,164), which does not
change. The `websocket` binding row of the codegen model gets a version `"2"`
entry with `frame_header_max_bytes` 21 (the `stream_end` header without its
envelope) and `envelope_bytes` 16.

### 5.2 The codes

`outcome` gains `end` 10 and `interrupted` 11. On a `stream_end`, `outcome` is
one of: `end` (normal), `contract(c)` (3 to 6: an element or a chunk refused at
the receiving boundary, or a refusal before the first chunk), `corrupt` 7 (a
chunk the receiver could not decode, or longer than the member's limit), `busy`
8 (the providing runtime refused the call at admission), `interrupted` 11 (the
sender stopped without an end, or the receiver cancels). `detail` is the `Rule`
code with `outcome` 3 and 0 otherwise. `Rule` gains `Utf8` 7, for a `<string>`
chunk that is not valid UTF-8 (ADR-0032 decision 12); `Rule` is
`#[non_exhaustive]`, and a new variant arrives with a new frame version, as §9.6
requires.

**R-13. A streamed-reply query is never answered by a `response`; every outcome
of it is a `stream_end`.** The reply stream opens with its first `chunk` or its
`stream_end`; a refusal before any chunk (`busy`, `contract`, `corrupt`) is a
`stream_end` with that outcome and `seq` 1. A `response` for a member whose
reply is a stream is a protocol violation (close 1002). Reason: one answer form
per shape keeps the consumer's state machine one branch per member; the
consumer's catalog says which members stream, and the server knows from its
member table (R-18).

**R-14 (amended). `interrupted` crosses the frame, as the third `Transport`
variant that does, and a `stream_end` sent by the receiver of a stream carries
the receiver's verdict.** Frame specification §9.6 says exactly two cross;
version 2 adds this one. Reason: without it an interrupted download would wait
with no bound, since decision 13 bounds only the start of a streamed reply. A
`stream_end` travels against the stream's direction when its receiver stops it
(the consumer on a reply stream, the provider on a parameter stream), and then
carries `contract(c)` when a chunk was refused at the receiving boundary,
`corrupt` when one could not be decoded, and `interrupted` when the receiver
cancels with no reason to state. The sender stops, and the side that sent the
`request` forgets the call once every stream of it has ended or was stopped.
Section 5.5 states what each surface reports.

**R-15. Invalid UTF-8 in a `<string>` chunk is `contract(InvalidValue(Utf8))`.**
Reason: ADR-0032 decision 12 places it in Stratum 2, and no existing `Rule`
names it. The check runs where the type is known: the generated face on each end
(section 7), the generated TypeScript codec (section 8). The transport forwards
bytes and checks no text.

### 5.3 Flow control

**R-16. A window of chunks per stream and direction, granted by the receiver:
the initial credit is 4 chunks, and `stream_credit` adds to it.** The receiver
sends `stream_credit` as it hands chunks to its application and frees its
buffer. A `chunk` beyond the sender's credit is a protocol violation (1002), a
peer defect as a duplicate `request` is. Reason: one WebSocket connection
carries every signal, event and call of a session, so a slow consumer of one
stream must not stall the connection; TCP's own flow control would stall the
whole session. A chunk is the unit everywhere (R-2) and is bounded by the limit,
so a credit in chunks bounds bytes: 4 × 16,128 is about 64 KiB per stream.
_Alternatives_: rely on the transport's flow control (rejected: head-of-line
blocking of the session's signals); a credit in bytes (rejected: the sender
would have to split chunks, which it may not do for an element stream).

### 5.4 Duplicates, retention and timing

**R-17. A retransmitted `request` of a streamed-reply query is answered
`stream_end interrupted`, and the provider keeps the `seq`, not the chunks.**
The kept-response set of frame specification §7 holds, for such a call, one
37-byte `stream_end`, until the bound has passed as for any response. Reason:
ADR-0032 note (c): a provider cannot keep a streamed reply whole; a consumer
that lost chunks resumes with an `offset` parameter (decision 5).

The response bound follows ADR-0032 decision 13 on both sides. The consumer
reports `Transport::Timeout` when neither a `chunk` nor a `stream_end` of the
reply stream arrived within `max` of the request's stamp; on a query with a
parameter stream and a plain reply, within `max` of its own `stream_end end`.
The server's retention clock for such a query
(`min(request.stamp, receipt) +
max` today) runs from the receipt of the
consumer's `stream_end end`, and so does `call_abandon_after`. Nothing about
timing crosses (§8).

### 5.5 A refusal after the acknowledgment

**R-33. A provider's refusal of a parameter stream after the acknowledgment
reaches the producer through its next push, carries `contract(c)` or `corrupt`
on the wire against the stream's direction, and leaves the acknowledgment as it
was.** The owner's ruling of 2026-10-31. The chain, for
`command put(info,
data: <bytes>)` or `command upload(data: <FwBlock>)`:

1. The generated `serve` pulls a chunk, verifies it (structure, constraints,
   UTF-8 for `<string>`), and on a failure calls
   `abort_claim(claim, k, CallError::Contract(InvalidValue(v)))` (or
   `Transport(Corrupt)` for a structural failure) and tells the application's
   `Consumer` `failed(e)`. The command's claim was settled `Ok(&[])` at the
   acknowledgment and is not settled again; a query with a parameter stream and
   a plain reply settles `Err(e)` instead, since its reply is still open.
2. In process, the producer's next `push(c, k, _)` or `end(c, k)` answers
   `PushError::Failed(e)`; a `Stream(c, k)` waker is woken.
3. Over the frame, the provider sends `stream_end` on stream `k` with the
   outcome of `e` (R-14), and drops every later `chunk` of that stream without a
   note until the consumer's own `stream_end` or the end of the bound; the
   consumer's runtime answers its producer as in step 2.
4. The Rust `CSink`'s next `poll_ready`, `send` or `end` returns
   `Err(ClientError::Call(e))`; the `CCall` future, which resolved `Ok(())` at
   the acknowledgment, is unchanged. For a query with an upload and a plain
   reply, `QCall` resolves `Err(ClientError::Call(e))`.
5. In TypeScript, `commandStream`'s `accepted` promise, already resolved at the
   acknowledgment, is unchanged; the `WritableStream` errors, so its pending and
   later `write` and `close` reject with a `SessionError` carrying `e`'s
   category (`contract` with the `Contract` and the `Rule`, or `corrupt`).
   `queryWithUpload`'s `reply` rejects the same way.

Reason: ridl §6.1 makes a command's acknowledgment a delivery acknowledgment
that cannot cover chunks that arrive after it (ADR-0032 note (a)), so the
refusal has to travel on the stream, not on the call's outcome, and the one
place a producer looks after the acknowledgment is its next push. The
conformance tests of stage 2 pin steps 1 and 2.

### 5.6 The vectors

Three rows join the vector table of §11.1, read into the `wire` dump by their
`Id`: `chunk-reply` (a `chunk` on stream 0, correlation 2, seq 1, payload
`ff01`), `stream-end-end` (a `stream_end end`, stream 0, correlation 2, seq 2)
and `stream-credit` (a `stream_credit` of 4 on stream 0, correlation 2). Adding
rows adds lines to the dump and passes the freeze check; no version 1 row is
edited. The exact bytes are written with the codec in stage 4.

## 6. The server (`ridl-transport-ws`) and the TypeScript client

**R-18 (amended). A server offers `ridl-ws-v2` only when its backend answers
`streams()` with `Some` (R-32) and its member table carries the stream entries;
otherwise it offers `ridl-ws-v1` alone.** Reason: without the stream ports it
cannot carry a stream at all, and without a table it cannot tell a streamed
reply's end from an empty plain reply (R-4); refusing the version at the
subprotocol selection is the one place the frame has for that. `Polling` and
`Scanning` backends serve streams alike.

On a version 2 session the server, each tick:

- for each streamed-reply call in flight with credit left, `pull`s the reply
  stream into its message buffer and sends a `chunk` per chunk; `Pull::End`
  sends `stream_end end` and forgets the call; `Pull::Failed(e)` sends a
  `stream_end` with `e`'s outcome and forgets it. Chunks are pulled only while
  the client's credit allows, so the loopback window (R-3) is the backpressure
  to the provider;
- for each `chunk` received on a parameter stream, `push`es it to the backend;
  the server granted the credit, so it holds at most 4 chunks per stream not yet
  accepted, and grants one credit per chunk the backend takes. A chunk longer
  than the member's limit (D-26) is answered `stream_end corrupt` and the call
  is forgotten, as a `request` longer than the limit is answered `corrupt`;
- a `stream_end` from the client on a stream it receives aborts the backend
  stream with its outcome (`interrupted`, or the verdict of R-14); a
  `Pull::Failed(e)` or a `PushError::Failed(e)` from the backend on a parameter
  stream sends `stream_end` with `e`'s outcome (R-33); the end of the session
  forgets every call, as today, and every open stream of it is `Transport::Down`
  on the backend.

The `Config` check of D-26 is unchanged: 35 + 16,128 fits the default limit.
Three `Note` values are added (`Note` is `#[non_exhaustive]`, so additively):
`StreamCreditExceeded` (a violation), `ChunkTooLarge` and `StreamStopped`.

**The TypeScript client** (`packages/ridl-transport-ws`) gains, on `Session`:

```ts
// Stage 4: a streamed reply.
queryStream(iface, ord, args, { bound }): ReadableStream<Uint8Array>;
// Stage 5: a parameter stream on a command or a query.
commandStream(iface, ord, args, { bound }): { upload: WritableStream<Uint8Array>; accepted: Promise<void> };
queryWithUpload(iface, ord, args, { bound }): { upload: WritableStream<Uint8Array>; reply: Promise<Uint8Array> };
```

**R-19. The `ReadableStream`'s `pull` is the credit.** The stream is built with
`highWaterMark` 4 chunks; the session sends `stream_credit` from the `pull`
callback as the application drains the queue, and `cancel` sends
`stream_end interrupted`. The `WritableStream`'s `write` waits while the
client's credit is spent, and `close` sends `stream_end end`. Reason: the
platform's backpressure and the frame's credit are the same mechanism, so no
second queue is needed. The codec (`frame.ts`) decodes the three forms;
`connect` offers `["ridl-ws-v2", "ridl-ws-v1"]` and the session records which
was selected; `queryStream` on a version 1 session rejects with
`ProtocolError("streams-unsupported")`.

## 7. The generated Rust surface

The Rust backend today refuses a stream member
(`docs/design/interaction-face.md`, "The consumer face", driftsys/ridl#943): the
face of `veh.hvac.cabin` in the frozen veh-cluster crate covers the service's
other members and leaves `replayTrace`, `streamFilterHistory` and `rebalance`
out. (`VehicleStatus` has no face for an unrelated reason: its `setGear` clause
is outside the narrow translator,
`freeze/generated/veh-cluster/veh.cluster.rs`.) The face is extended as follows;
the descriptors, `Client`, `Provider`, `serve` and the futures keep their
shapes, and the bounds follow R-32.

### 7.1 The traits in `ridl_rt::face`

```rust
// face:: — core only, no_std
/// A producer the generated `serve` drives, one item at a time.
pub trait Source {
    type Item;
    fn poll_next(&mut self, cx: &mut Context<'_>) -> Poll<Option<Self::Item>>;
}
/// A byte producer: `Ok(0)` is the end, `Err(Interrupted)` interrupts.
pub trait ByteSource {
    fn poll_read(&mut self, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<Result<usize, Interrupted>>;
}
/// A consumer the generated `serve` feeds, one item at a time.
pub trait Consumer {
    type Item;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<()>;
    fn accept(&mut self, item: Self::Item);
    fn end(&mut self);
    /// The stream stopped without its end: interrupted, or refused by this side's own check (R-33).
    fn failed(&mut self, e: CallError);
}
pub trait ByteConsumer {
    fn poll_write(&mut self, cx: &mut Context<'_>, buf: &[u8]) -> Poll<usize>;
    fn end(&mut self);
    fn failed(&mut self, e: CallError);
}
pub struct Interrupted;
```

**R-20. The provider's side is pull-shaped: a provider method for a streamed
reply returns a `Source`, and for a stream parameter returns a `Consumer`; the
generated `serve` drives both, inside its claim budget.** Reason: `Provider`
methods are synchronous and must stay so on a target without `std`; a method
that received a writer could not wait when the window is full, while `serve`
already is the task that polls and is woken (`Interest::ClaimStream`). `Poll`
and `Context` are `core::task`, so this needs no `std` and no dependency.
_Alternatives_: a method that receives a sink handle and pushes (rejected:
cannot wait in a synchronous method); one method call per element (rejected: the
provider would hold the stream's state across calls with no type that names it).

The `Provider` trait for the use case:

```rust
pub trait Provider {
    type Faults: ridl_rt::face::Source<Item = FaultResult>;
    /// `query faults(filter: DiagFilter): <FaultResult>`
    fn faults(&mut self, filter: &DiagFilter) -> Self::Faults;
    // stage 5:
    type UploadFirmware: ridl_rt::face::Consumer<Item = FwBlock>;
    fn upload_firmware(&mut self) -> Self::UploadFirmware;
    type Fetch: ridl_rt::face::ByteSource;
    fn fetch(&mut self, id: &DocId) -> Self::Fetch;
}
```

`serve`, for a streamed reply, takes the claim, verifies the arguments and
`require` as today, calls the method, then on each poll with window left polls
the `Source`: `Ready(Some(item))` encodes into the element's `BUFFER`, verifies
(the sending side's boundary check of ridl §10.2, and for `<string>` the UTF-8
check of R-15), and `push_claim`s; `Ready(None)` `end_claim`s; `Pending` leaves
the source's registration of `cx` to wake `serve`. A value that fails its own
constraints on the sending side settles `Contract(InvalidValue)`, which ends the
stream (ridl §12.4). `ensure` is not evaluated on a streamed reply: the checker
already refuses `result` on a stream (`ExprType::Unsupported`). At most one
`Source` poll per claim per `serve` poll counts against `SERVE_BUDGET`.

### 7.2 The consumer's side

```rust
impl<P: ...> Client<P> {
    /// The call; the future resolves once the stream has started, within the bound.
    pub fn faults(&mut self, filter: &DiagFilter) -> FaultsCall<'_, P>;
}
/// The streamed reply, after `FaultsCall` resolved `Ok`. Holds the element's BUFFER inline.
pub struct FaultsStream<'a, P> { /* port, correlation, buffer */ }
impl<'a, P> FaultsStream<'a, P> {
    /// The pull API of ADR-0032 decision 9 — the whole face on a target without `std`.
    pub fn poll_next(&mut self, cx: &mut Context<'_>) -> Poll<Option<Result<FaultResult, ClientError>>>;
    pub async fn next(&mut self) -> Option<Result<FaultResult, ClientError>>;
}
// A byte stream reply (stage 5), the same shape over bytes:
impl<'a, P> FetchStream<'a, P> {
    pub fn poll_read(&mut self, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<Result<usize, ClientError>>; // Ok(0) is the end
    pub async fn read(&mut self, buf: &mut [u8]) -> Result<usize, ClientError>;
}
```

Dropping the stream value calls `forget`, which interrupts (R-5). The call's
future resolves `Ok(stream)` on the first `Chunk` or `End` (the chunk is kept in
the buffer and handed out by the first `poll_next`), `Err(Timeout)` at the bound
(decision 13), and `Err(e)` on a `Failed(e)` before any chunk.

**R-21. The byte-stream wrapper serves a short `buf` from a staging buffer of
the member's limit, inline, with no allocation.** Reason: a chunk is at most the
member's `limit` (decision 14), 16,128 bytes at most, so a `[u8; LIMIT]` field
of the stream value absorbs a chunk and `poll_read` copies from it, and `Short`
never reaches the application.

**R-22. The `std` surface is inherent `poll_*` and `async fn` methods; the
`AsyncRead`, `AsyncWrite`, `Stream` and `Sink` impls of ADR-0032 decision 6 are
behind two opt-in cargo features of the generated crate, `futures-io` and
`futures-core`.** Reason: the generated crate depends on `ridl-rt` alone and
builds for `thumbv7em-none-eabihf` and as edition 2021 (`just compat-check`,
ADR-0021 decision 10's matrix); the standard library has no async I/O trait, and
the two `futures-*` crates are the executor-neutral ones every runtime adapts
to. _Alternatives_: `tokio::io` traits (rejected: one executor's dependency in
every generated crate); a trait of `ridl-rt`'s own (rejected: a trait the
ecosystem has no adapter for; `face::Source` exists for the provider's side
only, where no ecosystem trait fits a `no_std` provider).

The `blocking` module gains `next()` and `read()` over `ridl_rt::task::block_on`
with the call's deadline, as it wraps the futures today.

### 7.3 What the face emits per stream form

| Declaration                   | Consumer value                                         | Provider return               |
| ----------------------------- | ------------------------------------------------------ | ----------------------------- |
| `query f(..): <T>`, `T` named | `FStream`: `poll_next` → `Result<T, ClientError>`      | `type F: Source<Item = T>`    |
| `query f(..): <bytes>`        | `FStream`: `poll_read`                                 | `type F: ByteSource`          |
| `query f(..): <string>`       | `FStream`: `poll_read`, each chunk checked UTF-8       | `type F: ByteSource`, checked |
| `command c(.., d: <T>)`       | `c(args) -> (CCall, CSink)`: `poll_ready`/`send`/`end` | `type C: Consumer<Item = T>`  |
| `command c(.., d: <bytes>)`   | `CSink`: `poll_write`/`end`                            | `type C: ByteConsumer`        |
| `query q(d: <T>): R`          | `(QCall, QSink)`; `QCall` resolves after `end`         | `Consumer`, then the reply    |
| `query pipe(a: <T>): <U>`     | `(PipeCall, PipeSink)`; `PipeCall` → `PipeStream`      | `Consumer` and `Source`       |

Stage 3 builds the first row; stage 5 the rest.

## 8. The generated TypeScript surface

The TypeScript backend (`crates/ridl-backend-ts`) today refuses a stream in a
codec and in the typl surface. It gains, in the generated codec module of a
package, one function per streamed interaction over the session API of section
6:

```ts
// Stage 6.
export function faults(session: Session, filter: DiagFilter, opts?): ReadableStream<FaultResult>;
export function fetch(session: Session, id: DocId, opts?): ReadableStream<Uint8Array>;      // <bytes>
export function fetchText(session: Session, id: DocId, opts?): ReadableStream<string>;      // <string>
export function uploadFirmware(session: Session): { upload: WritableStream<FwBlock>; accepted: Promise<void> };
```

**R-23. The generated TypeScript stream is a `TransformStream` over the
session's byte stream, one element per chunk; a `<string>` stream decodes each
chunk with `TextDecoder` in `fatal` mode and errors the stream on invalid
UTF-8.** Reason: ADR-0032 decision 6's rows name
`ReadableStream`/`WritableStream`, a `TransformStream` composes with them and
keeps the credit of R-19 flowing through `pipeThrough`, and `fatal` is the
platform's UTF-8 check. An element that fails `verify` errors the stream with
the `Contract` failure (ridl §12.4).

## 9. The stages

Each stage names the crates it touches, the frozen surfaces it touches, the new
diagnostics, and the tests that prove it. "Frozen" refers to the rows of
`freeze/surfaces.toml` (ADR-0029).

### Stage 1 — Descriptor, model and `ridl-rt` descriptors

Delivers: R-7 to R-11 and R-31, and the two record corrections of section 1.

- **Crates**: `ridl-descriptor` (`schema/catalog.fbs`: `StreamForm`,
  `StreamPayload`, `Member.streams`; `lower.rs`: the stream entries and the
  request spelling over the non-stream parameters; `describe.rs`; `verify` walks
  the new table list), `ridl-ir` (`model.proto`: `StreamForm`, `StreamShape`,
  `Interaction.streams`, the comment of `request_tuple`; the lowering of the
  induced argument struct over non-stream parameters, R-10), `ridl-sem` (the
  induced struct rule), `ridl-rt`
  (`contract::{StreamNo, StreamForm,
  StreamInfo}`, `Member.streams`),
  `ridl-backend-rust` (`descriptors.rs` writes `streams` and the params struct
  of every call; the face still leaves a stream member out), `ridl`
  (`ridl describe` prints `streams`).
- **Frozen surfaces**: `runtime-crates` — `Member` gains a field, a break under
  ADR-0021 decision 10: approve it by removing the row and adding it back at the
  stage's commit (ADR-0029 decision 4); the release is R-35's. `generated-crate`
  — every `Member` literal changes and the frozen veh-cluster copy gains
  `VehHvacCabinReplayTraceParams` and `VehHvacCabinRebalanceParams` (R-10):
  additive, so the baseline comparison passes after `cargo xtask freeze update`.
  `wire` — the `.fbs` and `.proto` lines of those two structs are added
  (`freeze update`); the catalog hash lines do not change, because the IR does
  not. `codegen-protocol` — lines added for the new messages and fields; passes.
  `diagnostics` and `accepted-sources` — unchanged.
- **Diagnostics**: none, and none changed (ADR-0032 decision 14; R-11).
- **Records**: ADR-0032 decision 10 and ridl §12.4 change
  `<DocChunk | DocError>` to a named union (section 1);
  `docs/design/catalog-descriptor.md` D-4, "What a catalog contains" and "Not
  built"; `docs/design/ridl-rt.md` "The interaction descriptors";
  `docs/design/codegen-plugins.md` for `Interaction.streams`.
- **Tests**: `ridl-descriptor` lowers `streamFaults` to one entry
  `{ 0,
  Elements, veh.cluster.FaultEvent }` with the element's size rows,
  `replayTrace` to `{ 1, Elements, … }` with the request `()`, `rebalance` to
  entries 0 and 1, `<bytes>` to `Bytes` and `<string>` to `Text` with no size
  row, and a file written without the field reads an empty `streams`;
  `ridl describe` prints them; the catalog hash of veh-cluster is unchanged (the
  `wire` dump); a stream whose element is `type Big: bytes [1..40000]` draws no
  diagnostic (decision 14 pinned); `ridl diff` reports a reply changed from `T`
  to `<T>` as it reports it today (a test that pins the current category); the
  induced argument struct of `command put(info: DocInfo, data: <bytes>)` is
  `DocInfo` and of `command upload(data: <FwBlock>)` has no field; the model's
  `Interaction.streams` of the veh-cluster fixture is pinned in
  `ridlc-gen-model`'s parity test; `limits_parity.rs` still passes.

### Stage 2 — The port, the loopback and the conformance suite

Delivers: R-1 to R-6, R-32, R-33's steps 1 and 2, sections 3.1 to 3.3.

- **Crates**: `ridl-rt` (`port::{Pull, PushError, StreamCaller, StreamHandler}`,
  the `&mut P` forwarding impls; `Interest::Stream` and `Interest::ClaimStream`;
  `Transport::Interrupted`; `correlate::Table` gains a per-slot `Stream` waker
  map keyed by `StreamNo`), `ridl-loopback` (stream queues per call slot and
  `StreamNo`, `Loopback::STREAM_WINDOW = 4`, both traits on `Loopback`,
  `CallerHandle` and `HandlerHandle`; the aggregate forwards),
  `ridl-rt-conformance` (`streams.rs`; the `streams` extension of `suite!`; no
  new `Factory` item, R-32).
- **Frozen surfaces**: `runtime-crates` — one break: `Interest` gains two
  variants (exhaustive). Approve as in stage 1; it ships in R-35's release.
  `ridl-rt-conformance` and `ridl-loopback` are additive.
- **Diagnostics**: none.
- **Tests** (`ridl-rt-conformance/src/streams.rs`, bounded on
  `F::Runtime: StreamCaller + StreamHandler + Wakeable`):
  `a_reply_stream_is_pulled_in_order_then_ends`;
  `a_pull_before_the_first_chunk_is_pending`;
  `a_short_buffer_reports_needed_and_does_not_consume`;
  `end_claim_on_the_reply_stream_settles_the_claim`;
  `a_settle_err_after_chunks_fails_the_stream_after_them`;
  `a_push_into_a_full_window_answers_zero_and_a_pull_frees_it` (pushes until 0
  or 1,024 chunks);
  `a_stream_waker_is_woken_by_a_chunk_an_end_and_a_freed_window`;
  `forget_fails_the_providers_pull_and_push_with_interrupted`;
  `an_abort_by_the_producer_fails_the_receivers_pull_after_the_chunks_before_it`;
  `a_receivers_abort_of_a_parameter_stream_fails_the_producers_next_push` (R-33
  step 2, with `Contract(InvalidValue)`);
  `a_refusal_after_the_acknowledgment_leaves_the_acknowledgment` (R-33 step 1:
  `ack` still answers `Ok(())`);
  `a_parameter_stream_pushed_before_the_claim_is_pulled_after_it`;
  `a_parameter_stream_ends_explicitly_and_a_second_end_is_ended`;
  `a_chunk_longer_than_the_limit_is_too_large_and_not_sent` (R-11);
  `two_stream_parameters_are_two_streams`;
  `a_bidirectional_call_carries_both_streams`;
  `a_reclaimed_slots_stream_answers_unknown`. The loopback keeps its own
  `…_drops_without_deadlock` test for a stream waker.

### Stage 3 — The Rust face for a streamed reply of named elements

Delivers: section 7 for the first row of 7.3, R-20 to R-22.

- **Crates**: `ridl-rt` (`face::Source`, `face::Interrupted`),
  `ridl-backend-rust` (`descriptors::query_reply_type` accepts
  `StreamType.named`; `face.rs` emits `<Name>Stream`, the `Provider` associated
  type, the `serve` arm; the `blocking` module; the two opt-in features in the
  emitted manifest), `ridlc` (the veh-cluster corpus: `cluster/services.ridl`
  gains
  `interface FaultLog { query faults(filter: DiagFilter): <FaultResult>
  @[..1s] }`
  and `union FaultResult`; `getFaultPage` stays, because `appendix-a.ridl` is
  the reference's text), `ridl-loopback` (dev).
- **R-24. The corpus gains a sibling, `FaultLog.faults`, in `services.ridl`, and
  `getFaultPage` stays.** Reason: `appendix-a.ridl` is Appendix A compiled with
  one adaptation, and its diagnostics are facts about the reference; a new
  interface in the authored file keeps that, and the ordinal space of
  `VehicleStatus` unchanged.
- **Frozen surfaces**: `generated-crate` — the `veh.hvac.cabin` face gains
  `stream_filter_history` (a streamed reply of named elements, in the frozen
  copy today), and the frozen corpus copy under `xtask/tests/freeze-fixtures/`
  is refreshed on purpose to add `FaultLog`, the only new face; both add items,
  so the baseline comparison passes after `cargo xtask freeze update`.
  `VehicleStatus` keeps no face (its `setGear` clause). `wire` — the refreshed
  copy adds veh-cluster `.proto` and `.fbs` lines for `FaultLog` and
  `FaultResult`; the cabin catalog hash line is untouched. `runtime-crates` —
  additive (`face::Source`).
- **Diagnostics**: none.
- **Tests**: `tests/face_generation.rs` pins the emitted `FaultsStream`,
  `Provider::Faults`, the `serve` arm and the `StreamCaller`/`StreamHandler`
  bounds on `FaultLog` only (R-32: `Horn`'s signal-only `Client` still builds
  over a port without them); `tests/interaction_face.rs` runs a loopback round
  trip of 64 `FaultResult` elements, about 68 KB in FlatBuffers, above one call
  limit, checks order and the end, and a provider that returns `Pending` twice
  before each element; a provider that interrupts after 3 elements gives the
  consumer 3 elements then `Transport::Interrupted`; a dropped `FaultsStream`
  makes the provider's next push `Failed(Transport(Interrupted))`; an element
  the provider produces outside its range ends the stream with
  `Contract(InvalidValue)` before it is sent; an element whose encoding is above
  a lowered `[limits.interaction]` entry is refused `TooLarge` at push and the
  consumer reads `Interrupted` (R-11); the `blocking` client reads the same
  stream; `face_compile.rs` compiles the crate with and without the two
  features; `just compat-check` and `just demo` stay green (cabin has no stream,
  so its crate is unchanged).

### Stage 4 — Frame version 2 and the streamed reply over WebSocket

Delivers: sections 5 and 6 for the reply stream, R-12 to R-19.

- **Crates**: `ridl-transport-ws` (`frame.rs`: `Frame::Chunk`,
  `Frame::StreamEnd`, `Frame::StreamCredit`, the codes, `FRAME_VERSION_2`,
  `SUBPROTOCOL_V2`; `server/`: `Backend::streams()` with its default (R-32), the
  version selection (R-18), the per-call stream state, the credit, the kept
  `stream_end`, the three `Note`s; `descriptor`: `MemberTable` reads
  `Member.streams`), `ridl-ir` (`codegen/bindings.rs`: the version `"2"` row),
  `packages/ridl-transport-ws` (`frame.ts`, `session.ts`, `calls.ts`:
  `queryStream`, R-19), `xtask` (the three vectors read into the `wire` dump).
- **Frozen surfaces**: `wire` — three vector lines added; no version 1 line
  changes. `runtime-crates` — additive: `Frame` and `Note` are
  `#[non_exhaustive]`, `Backend::streams` has a default, the constants are new
  items. `codegen-protocol` — unchanged.
- **Records**: frame specification §4 (forms, codes), §7 (R-17), §8 (decision
  13's rows), §9.6 (R-14), §11.1 (version 2: subprotocol, layouts, vectors);
  `docs/design/ridl-transport-ws.md` and `ridl-transport-ws-ts.md`;
  `docs/technotes/websocket-binding-rationale.md`.
- **Diagnostics**: none.
- **Tests**: `tests/vectors.rs` reproduces the three vectors byte for byte, in
  Rust and in `frame_test.ts`; a version 1 client against a version 2 server is
  served version 1 unchanged, and a version 2 client against a server without a
  table is selected `ridl-ws-v1`; the server streams 64 elements to a
  `tungstenite` client and sends `stream_end end` with the next `seq`; a client
  that grants no credit receives 4 chunks and no more until it sends
  `stream_credit`; a fifth chunk sent beyond credit on the upload direction
  closes 1002 (stage 5 sends uploads; here the test client sends an unsolicited
  `chunk`); a client's `stream_end interrupted` forgets the call on the backend;
  a provider's interruption reaches the client as `stream_end interrupted`; a
  retransmitted `request` after the end is answered `stream_end interrupted`; a
  `response` for a streamed member from a fake server is a violation on the
  TypeScript client; `just ts-e2e` adds a `queryStream` transfer of 64 elements
  over `loopback_server` and checks the count, the order and the end;
  `just ts-browser-e2e` the same in a browser.

### Stage 5 — Parameter streams, byte streams and text streams

Delivers: the remaining rows of 7.3, R-10's face side, R-15, R-21, R-23's
session half, ADR-0032 decisions 2, 3 and 12.

- **Crates**: `ridl-rt` (`face::ByteSource`, `Consumer`, `ByteConsumer`;
  `payload::Rule::Utf8`), `ridl-backend-rust` (parameter streams: the sink
  values and the `Consumer` associated types; `<bytes>` and `<string>` in reply
  and parameter position; the UTF-8 check on both ends; the staging buffer),
  `ridl-transport-ws` (the upload direction: `chunk` received, `push` to the
  backend, credit granted per accepted chunk, `ChunkTooLarge`; `rule_tag` for
  `Utf8`), `packages/ridl-transport-ws` (`commandStream`, `queryWithUpload`, the
  `WritableStream`), `ridl-loopback` (nothing: stage 2 built both directions),
  examples: `examples/cabin` gains no stream (its crate is a frozen surface and
  the demo's blocking lines are counted); a new worked example
  `examples/docstore/` with `fetch(id): <bytes>`, `put(info, data: <bytes>)` and
  `describe` from ADR-0032 decision 5, compiled and run by the test suite.
- **R-25. The byte-stream chunk size on the sending side is the member's limit,
  filled from the application's `ByteSource` or `poll_write` as bytes arrive,
  and a partial fill is sent when the source is `Pending`.** Reason: the
  transport chooses the boundaries (decision 2), a full chunk is the cheapest
  per byte, and waiting for a full chunk while the application has nothing yet
  would add latency the author did not ask for. For `<string>`, the fill is cut
  back to the last complete code point (decision 3), and a chunk that is not
  valid UTF-8 is refused before transmission (R-15).
- **Frozen surfaces**: `runtime-crates` — additive. `wire` — `Rule` code 7 is
  version 2's, already introduced as a code value in stage 4's vectors or added
  here as a vector line (`stream-end-utf8`); additive. `generated-crate` — the
  `veh.hvac.cabin` face gains `replay_trace` (a parameter stream) and
  `rebalance` (bidirectional), which the frozen copy declares: additive, after
  `cargo xtask freeze update`. No frozen copy declares a byte or text stream.
- **Diagnostics**: none new. The checker is unchanged (ADR-0032 decision 14;
  R-11): an element above the limit is refused at push, never by a diagnostic.
- **Tests**: a loopback upload of 200 KB through `put`, read by the provider's
  `ByteConsumer` in chunks of at most the limit, byte-equal to the input; a
  `<string>` reply of a text whose code points straddle every chunk boundary
  arrives with no split and decodes; a chunk with a truncated code point is
  refused on the sending side with `Contract(InvalidValue(Utf8))`, and one
  injected on the wire is refused on the receiving side before the application;
  a `query pipe(a: <T>): <U>` whose reply starts before the upload ends; a query
  with an upload and a plain reply measures its bound from the upload's end (a
  hand-driven clock advanced past `max` before the end does not time out, and
  past `max` after it does); R-33 steps 3 to 5: a provider that refuses the
  third block of `uploadFirmware` makes the Rust `CSink`'s next `send` return
  `Err(ClientError::Call(Contract(InvalidValue(_))))` while the `CCall` stays
  `Ok(())`, sends `stream_end contract` against the upload over the frame, and
  makes the TypeScript `WritableStream`'s next `write` reject with the
  `contract` failure while `accepted` stays resolved; `just ts-e2e` uploads 200
  KB from a `WritableStream` and reads it back through `fetch`; the server
  answers an oversized chunk `stream_end corrupt`; `docstore`'s example program
  runs in the test suite.

### Stage 6 — TypeScript codegen for streams

Delivers: section 8, R-23.

- **Crates**: `ridl-backend-ts` (the codec module emits the stream functions and
  the `TransformStream` over the element codec; the typl-surface refusal of a
  stream stays, since a stream is an interaction shape), `packages/ridl-runtime`
  (a helper that builds the `TransformStream` from an element `decode`/`verify`
  pair and the UTF-8 one), `examples/cabin/web` is unchanged (cabin has no
  stream); `examples/docstore/web` added to `cabin-ws-demo`'s sibling recipe or
  to `ts-e2e`.
- **Frozen surfaces**: none (`ridl-backend-ts` output is not a frozen surface;
  the `ts-codec-table` record is updated).
- **Diagnostics**: none.
- **Tests**: `_codec-ts-check` type-checks and lints the generated module of a
  fixture with every stream form; a Deno test pipes 64 elements through
  `faults()` against `loopback_server`; a `<string>` fixture errors the stream
  on an injected invalid chunk; `deno publish --dry-run` passes.

## 10. Rulings not tied to one section

- **R-26. No new form opens a stream; the reply stream opens with its first
  `chunk` or `stream_end`, and a parameter stream opens with the `request`.**
  Reason: an open frame would be one more round trip per stream and would carry
  nothing the request does not.
- **R-27. The initial credit is a constant of the binding version (4), not a
  field of `attach` or `request`.** Reason: version 1's forms stay byte for
  byte, and a field on `attach` would make the credit a session fact when it is
  a per-stream one; `stream_credit` adjusts it from the first frame on.
- **R-28. A `chunk` payload for a named element is the element encoded as the
  type is encoded in reply position today (its own root, or its `<Name>Box`).**
  Reason: the codecs already define that encoding and its size state, and a
  query returning a named union already crosses as one payload.
- **R-29. The server holds no stream across a session; a stream is interrupted
  by the end of its session.** Reason: frame specification §11.1 item 5 says the
  caller's identity is the connection and a reconnect is a new session; resuming
  is the author's `offset` parameter (ADR-0032 decision 5).
- **R-30. `ridl diff`, the MCP tools and the LSP are untouched.** Reason: the IR
  is unchanged, so every reader of it reads what it read; a diff category for
  "stream form changed" is unnecessary because changing `<T>` to `T` or the
  element type is already a change of the reply type.
- **R-34. The generated crate's async trait impls are behind two opt-in features
  named after the crates they pull, `futures-io` and `futures-core`.** The
  owner's ruling of 2026-10-31 on R-22's open point; a single `async-traits`
  feature is not added. Reason: a feature per dependency lets a consumer that
  wants `Stream` and `Sink` leave `futures-io` out, and the name says what it
  pulls.
- **R-35. One breaking `ridl-rt` release carries stages 1 and 2 together, and it
  ships in the next breaking release of the workspace, with
  driftsys/ridl#1108.** The owner's ruling of 2026-10-31. Stage 1's `Member`
  field and stage 2's `Interest` variants are the two breaks of `ridl-rt`
  (R-32), and `ridl-descriptor` breaks in the same release through its `Member`
  field; stages 3 to 6 are additive and may ship in any later release. Reason:
  one break is cheaper for every downstream runtime than two, and the workspace
  already has a breaking release scheduled.

## 11. Questions settled by the owner

The three questions the first version raised were answered on 2026-10-31 and are
recorded as rulings: RIDL-157 does not extend to a stream element (R-11, as
replaced); one breaking `ridl-rt` release carries stages 1 and 2, in the next
breaking release with driftsys/ridl#1108 (R-35); the generated crate's async
trait impls are behind `futures-io` and `futures-core` (R-34). Nothing in this
document waits on the owner.

## 12. Where the as-built code and the ADR disagree

- **The inline union element.** ADR-0032 decision 10 and ridl §12.4 write
  `<DocChunk | DocError>`; the grammar accepts one path inside `<…>`, and
  `ridl check` reports FORM-101 on the `|`. The records are corrected to the
  named union form in stage 1 (section 1); no syntax is added.
- **Invalid UTF-8 inside a payload is Stratum 3 today.** The codecs report a
  `string` field that is not valid UTF-8 as `Malformed` (structure, `Corrupt`),
  while ADR-0032 decision 12 places invalid UTF-8 in a `<string>` chunk in
  Stratum 2 (`INVALID_VALUE`). R-15 follows the ADR for the stream; the payload
  rule is unchanged and the two differ on purpose, which the frame specification
  §9.6 text should say when stage 4 edits it.
- **`docs/design/ridl-rt.md`, "What 0.1 leaves out"**, says "Streams
  (`stream`-typed fields)"; a stream is never a field (TYPL-301). Stage 2
  rewrites the sentence.
- **The induced argument struct.** `model.proto` says a request has "no induced
  argument struct, because a parameter is a stream or untyped"; ADR-0032
  decision 5's `command put(info: DocInfo, data: <bytes>)` needs the request
  `DocInfo` beside the stream. R-10 changes the lowering in stage 1.
- **The descriptor's request spelling.** `crates/ridl-descriptor/src/lower.rs`
  spells a request with a stream parameter as the whole list,
  `(samples: <FilterState>)`, so a reader today can tell a stream only by
  parsing a string the record says no reader parses. R-31 gives the streams
  their own entries in stage 1.

## As built

Rulings taken while the port was built. The source is the rustdoc of
`crates/ridl-rt/src/port.rs`; this note only lists them.

- An empty chunk is always taken.
- A runtime without a member table accepts `REPLY_STREAM` on a query, and
  `StreamNo(1)` and `StreamNo(2)` on any call.
- A forget keeps an ended stream and interrupts an open one.
- A query's settlement releases its parameter streams, and so does a command's
  settlement with an error; a command's settlement `Ok` does not.
- A dropped handler interrupts a call on which it moved a stream; a `Pending`
  pull and a registration move nothing.
- A receiver's abort drops its unread chunks; an abort of a finished stream
  answers `Ended`, and by the provider on a stream parameter it still counts as
  the provider's end of it.
- A forget before any provider took the claim withdraws the call with its
  streams.
- A claim only offered through `ShortClaim` has no streams until it is taken.
- The interaction's limit bounds every chunk in both directions.
- A streamed reply reserves nothing in the codegen model.
- No `Command` or `Query` impl is generated for a member that has a stream
  parameter, in either position of the call. A query whose reply is a stream of
  named elements and whose only stream is that reply gets a `Query` impl with
  `type Reply = ()`: the face's send evaluates `require` through it as for every
  query, its settlement carries no reply bytes, because the elements travel on
  the reply stream, so no payload type is claimed for a reply, and `()`
  implements no `Payload`, so a generic caller cannot decode one from it; its
  `ensure` is always `Ok`, because the checker refuses `result` on a stream.
- A call table slot holds `Interest::Stream` wakers for four streams
  (`ridl_rt::correlate::STREAM_WAKERS`), not one per `(slot, StreamNo)` as R-6
  read; a waiter on a fifth stream of one call is handed back and woken at once
  on every registration, a busy loop until a cell frees. Kept: `Table` is
  fixed-size and allocates nothing, and no diagnostic is added for a member with
  five streams. Every slot carries the four cells, in a runtime with no stream
  extension too.
- The codegen model numbers stream parameters as a `u32`; `StreamNo` and the
  descriptor's stream entries are a `u8`, so past 255 stream parameters the
  descriptor lowering drops the stream and the Rust backend emits a `StreamNo`
  that does not compile. Documented, not changed: no source declares that many,
  and no diagnostic is added.
- `settle(Ok(&[]))` after the caller refused the reply stream records `Ok` and
  leaves the refusal on the stream; `end_claim` there answers `Failed(e)` and
  records `Err(e)`.

## Trace

- Issues: driftsys/ridl#1116, driftsys/ridl#336, driftsys/ridl#943.
- Binding: [ADR-0032](../decisions/ADR-0032-byte-streams.md) decisions 9 to 14;
  ADR-0018 decision 2; ADR-0021 decisions 9, 10 and 13; ADR-0029; ADR-0030
  decisions 12 and 16 to 18.
- Records this design amends when built: `docs/design/ridl-rt.md`,
  `ridl-transport-ws.md`, `ridl-transport-ws-ts.md`, `catalog-descriptor.md`,
  `interaction-face.md`, `codegen-plugins.md`, the frame specification, and the
  ridl reference §9.3 and §12.
