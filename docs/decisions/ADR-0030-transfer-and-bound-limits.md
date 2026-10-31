# ADR-0030 — Transfer and bound limits

## Status

Accepted — 2026-10-21. Scope: the largest payload any runtime encodes, verifies
or carries, the largest `string` and `bytes` bound an author is advised to
declare, and how both codecs, the generated interaction face and the Rust
WebSocket transport apply them. It binds the FlatBuffers backend, the proto3
backend, `ridl-rt`, `ridl-transport-ws`, the typl and ridl checkers, and every
later runtime or transport. The TypeScript package `ridl-transport-ws` checks no
message size today (`docs/design/ridl-transport-ws-ts.md`) and does not change
here; with the new Rust limit (decision 10), a TypeScript peer that sends a
message above that limit has its session closed with 1009, as the frame
specification §11.1 says for any message above the limit.

The two lints (decisions 6 to 9) land first. The codec cap, `PAYLOAD_CEILING`,
`EncodeError::TooLarge` and the WebSocket default follow in later changes. The
codec cap, `PAYLOAD_CEILING` and `EncodeError::TooLarge` were withdrawn on
2026-10-30, unbuilt (decisions 3 and 4).

Written from driftsys/ridl#996 and the owner's decisions of 2026-10-21, which
replace the payload ceiling of 4 MiB + 64 KiB that the owner set on 2026-10-20.

Amended 2026-10-24 from driftsys/ridl#1045 and the owner's decisions of that
day: decision 10 is changed in place, and decisions 12 to 15 are added. The
limits now have one source, the compiled workspace, and neither an embedder nor
a client can raise them. The amendment lands as a decision first; the code
follows in separate issues, and the change to `with_max_message_bytes` is a
breaking change of the published `ridl-transport-ws` API, released in a version
marked breaking.

Amended 2026-10-28 from the owner's decisions of that day: decisions 1, 2, 5, 7,
10, 11, 12, 13, 14 and 15 are changed in place, and decisions 16, 17 and 18 are
added. Each kind has a default payload ceiling: 3,840 bytes for a signal and an
event, 16,128 bytes for a command and a query, and 65,280 bytes for a fixed.
Each ceiling bounds the longest copy that nothing can interrupt (decision 16).
The largest payload becomes 65,280 bytes for a fixed and 16,128 bytes for every
other kind, where it was 16,256, and the WebSocket message limit becomes 36 +
16,128 = 16,164 bytes, lowered from 16,292. A `[limits.interaction]` entry may
raise one member above its kind's default, up to 16,128 bytes, or 65,280 for a
fixed. How a transport moves a payload, whole or split, is a rule of each
transport binding, not of the language, the catalog, the descriptor or
`ridl-rt`. The amendment lands as a decision first; the code follows in
driftsys/ridl#1108. It is a breaking change: a signal or an event whose payload
is larger than 3,840 bytes with no entry for it, and a `[limits]` entry above
what decision 12 allows, stop compiling, and the WebSocket binding refuses at
attach a catalog whose fixed is larger than its message limit. The values 16,256
and 16,292 are on `main` from driftsys/ridl#1098 and not yet released, so the
amendment is released in the same version marked breaking as the transport
limits of the amendment of 2026-10-24.

Amended 2026-10-30 from the owner's decisions of that day: decisions 1, 2, 3, 4,
6, 7, 9, 10, 14, 15, 16 and 17 are changed in place, decisions 19, 20 and 21 are
added, and ADR-0031 open item 2 is closed (decision 19). Size is a property of a
payload in its interaction kind, so one rule applies to every kind: a payload
above the default ceiling of its kind, or with no finite bound, where no
`[limits.interaction]` entry gives the interaction its limit, is the error
RIDL-157 for a signal, an event, a command, a query and a fixed (decision 17).
The lint RIDL-150 `payload-above-ceiling` is retired (decision 7). TYPL-116
`bound-above-transfer` warns only on a bound that no interaction kind can carry:
a `string` bound above 16,256 characters or a `bytes` bound above 65,280 bytes
(decisions 6 and 7). The language server runs the size checks, so RIDL-151 and
RIDL-157 appear in the editor (decision 21). A type's `verify` checks structure
only; the transport refuses a payload longer than its interaction's limit, once
(decision 19). A transport binding carries every payload up to the limit that
the catalog gives its interaction, a raised limit included (decision 20). The
WebSocket binding keeps its message size of 16,164 bytes and carries a larger
payload in blocks, in a new frame version whose layout is tracked in
driftsys/ridl#1129 (decision 15). The codec guard of decision 4,
`EncodeError::TooLarge` and `PAYLOAD_CEILING`, is withdrawn: the transport
refuses a size, against the catalog's limit, on send and on receive (decision
4). The amendment lands as a decision first. Its code is added to
driftsys/ridl#1108 and ships in the same version marked breaking, except the
change of `verify` and the transport-side refusal of decision 19, which are
tracked in driftsys/ridl#1128. It is a breaking change: every command, query and
fixed whose payload is above its kind's default ceiling or has no finite bound,
with no entry for it, stops compiling (see Consequences), and a `[lints]` key
that names `payload-above-ceiling` now draws MANI-010.

Amended 2026-10-31 from the acceptance of [ADR-0032](ADR-0032-byte-streams.md):
decisions 11 and 16 are changed in place. Data larger than one transfer is a
byte stream or a stream of bounded elements, and the interaction's limit bounds
each chunk of a byte stream. No number changes.

Built 2026-10-28: the two lints of decisions 6 to 9 (driftsys/ridl#1013), the
`[limits]` table and RIDL-151 of decisions 12 and 14 (driftsys/ridl#1063), the
limit in the IR and the catalog hash of decision 13 (driftsys/ridl#1065), and
the WebSocket limits of decisions 10 and 15 with the descriptor's limit column
(driftsys/ridl#1098). Decisions 3 and 4 were not built, and were withdrawn on
2026-10-30 (driftsys/ridl#996).

Built 2026-10-30 (driftsys/ridl#1108, driftsys/ridl#1128): the amendments of
2026-10-28 and 2026-10-30, except what waits for driftsys/ridl#1129. Built:

- `ridl_core::limits` holds the three default ceilings (3,840, 16,128 and 65,280
  bytes), the largest raise, and the TYPL-116 thresholds (16,256 characters and
  65,280 bytes). TYPL-116 warns only on a bound that no kind can carry.
- The `ridlc` size pass resolves each interaction's limit (the
  `[limits.interaction]` entry, else the `[limits.kind]` entry, else `default`
  clamped to the kind's ceiling) and raises RIDL-151 against it, and RIDL-157
  where no entry gives the limit and the payload is above the default ceiling of
  its kind or has no finite bound, for a signal, an event, a command, a query
  and a fixed. RIDL-150 is retired and its lint name is reserved, so a `[lints]`
  key that names it draws MANI-010. MANI-022 follows decisions 12 and 18.
- The language server publishes RIDL-151, RIDL-152, RIDL-157 and the MANI-022 of
  the limits pass (decision 21).
- The generated FlatBuffers and podbuf `verify` check structure only (decision
  19). Decision 3 and decision 4 stay withdrawn: `MAX_SIZE` is not capped, and
  `ridl-rt` has no `PAYLOAD_CEILING` and no `EncodeError::TooLarge`.
- `ridl-rt` exports the three default ceilings. A sending port refuses a payload
  above its member's limit with `TooLarge { cap }`, and `ridl-loopback` returns
  it. The WebSocket transport applies the limit of each member on both sides,
  carries a message of at most 16,164 bytes, and refuses a `Config` that cannot
  carry an allowed payload. The TypeScript client reads each member's limit from
  the generated codec module and rejects a received payload above it before
  `verify`, and the codegen model carries the limit.

What waits for driftsys/ridl#1129 (decisions 15 and 20): the WebSocket blocks.
Until then, a fixed whose payload is above one message (16,137 bytes at the
default message limit) and within its limit is not carried over WebSocket: the
session records `ValueTooLarge` and sends no `publish`, nothing is refused at
attach, and the WebSocket provider answers a `read` of it with `answer` and
`contract(UnknownInteraction)`; the TypeScript client rejects a payload above
the limit as `corrupt`. The fixed itself is accepted by the catalog and by the
loopback.

Departure from decision 19, on the providing side of the WebSocket binding: a
`read` of a signal or of a fixed whose value is above the member's limit is
answered `answer` with `contract(UnknownInteraction)` and recorded as
`ValueTooLarge`, with no `publish`, so that no read waits on a value the
provider does not send; the decision's "a fixed's read reports the detection" is
what the client side reports for a payload it receives above the limit. The
provider reaches that path only behind a runtime that does not refuse the value
on send, which `ridl-loopback` does.

Note 2026-10-30, on decision 19: the rationale says the FlatBuffers verifier is
bounded by its depth and table limits. No code returns `Malformed::TooDeep` or
`Malformed::TooManyTables`. The work of the FlatBuffers verifier is bounded by
the maxima that the type declares and by the nesting of the type, as
`docs/design/flatbuffers-codec.md` states.

When accepted, these records gain a dated note or are changed in place:

- ADR-0021: decision 17 and the note of 2026-10-20 say a payload with no finite
  bound is refused at generation; decision 3 of this record reverses that, and
  `ridl-rt` gains `PAYLOAD_CEILING` and `EncodeError::TooLarge`. Superseded on
  2026-10-30: decisions 3 and 4 are withdrawn, so ADR-0021 keeps the refusal at
  generation, and `ridl-rt` gains neither.
- ADR-0024: two new lints.
- ADR-0029 decision 2 and `docs/book/stability.md`: amended for the two lints of
  this record (decision 9).
- `docs/design/flatbuffers-codec.md`, `docs/design/proto3-codec.md`,
  `docs/design/ridl-rt.md`, `docs/design/interaction-face.md`,
  `docs/design/catalog-descriptor.md`, `docs/book/codegen-plugins.md` and
  `docs/design/ridl-transport-ws.md`.
- The frame specification §9 and §11.1 ("The sizes"), and the typl reference
  §4.4, §4.5 and §16.2.
- From the amendment of 2026-10-24: ADR-0002 (the `[limits]` table of the
  manifest), ADR-0014 (the limit in the IR), ADR-0024 (RIDL-151), the ridl
  reference's diagnostics table, `docs/design/catalog-descriptor.md`,
  `docs/design/ridl-transport-ws.md`, the frame specification §11.1, and the
  records of the `ridl diff` categories (`docs/book/cli-reference.md`,
  `docs/book/evolving-an-interface.md`).
- From the amendment of 2026-10-28, updated when its code landed on 2026-10-30
  (driftsys/ridl#1108): ADR-0002 (the note of 2026-10-24: the range of a
  `[limits]` value and MANI-022), ADR-0024 (the note of 2026-10-24: the kinds
  RIDL-150 checks, and the error RIDL-157), the ridl reference's diagnostics
  table (the RIDL-150 and RIDL-151 rows, and a new RIDL-157 row), the family
  overview §7 (the MANI-022 row), the typl reference §4.4 and §4.5 (the ceiling
  of 16,128 bytes, and the smaller default ceiling of a signal and an event),
  `docs/book/lints.md` (the paragraphs on `payload-above-ceiling` and on
  `[limits]`, including the sentence that a `fixed` is not checked),
  `docs/book/cli-reference.md` (the `[limits]` section: the order that resolves
  a limit, the raise by a `[limits.interaction]` entry, MANI-022, RIDL-150,
  RIDL-151 and RIDL-157), `docs/design/interaction-face.md`,
  `docs/design/ridl-rt.md` and `docs/design/flatbuffers-codec.md` (no `BUFFER`
  above 16,128 bytes on the stack, and a fixed above that read through a view
  and encoded through the caller-buffer API),
  `docs/design/catalog-descriptor.md` (an absent `limit` means the default
  ceiling of the member's kind, and a raised limit of a signal or an event is
  above 3,840 bytes), `docs/design/ridl-transport-ws.md` (the message limit of
  16,164 bytes, the smallest retained cap, the arithmetic of the retained bytes,
  and the refusal at attach of a catalog whose fixed is larger than the message
  limit, which the amendment of 2026-10-30 replaces with blocks), the frame
  specification §11.1 "The sizes" and item 4 (the ceilings, the largest message
  of 16,164 bytes, that the WebSocket binding carries one payload per message,
  and the refusal at attach; the last two are superseded by the amendment of
  2026-10-30), and the freeze dumps `freeze/diagnostics.txt` and the
  `runtime-crates` dump, and `freeze/accepted-sources.txt` and
  `freeze/manifest.txt` where a frozen source or manifest is affected.
- From the amendment of 2026-10-30, changed in this change:
  [ADR-0031](ADR-0031-podbuf-projection-rules.md) (decision 6: RIDL-157 in place
  of RIDL-150; decision 8: no length check in `verify`; open item 2 closed; the
  list of departures from the design) and `docs/decisions/README.md` (the
  summaries of ADR-0030 and ADR-0031).
- From the amendment of 2026-10-30, updated when its code landed on 2026-10-30
  (driftsys/ridl#1108, driftsys/ridl#1128): ADR-0021 (the entry of the first
  list above: `ridl-rt` gains no `PAYLOAD_CEILING` and no
  `EncodeError::TooLarge`, and gains the three default ceilings),
  `docs/design/ridl-rt.md` and `docs/design/interaction-face.md` (the port's
  `TooLarge { cap }` against the member's limit is the one refusal on send),
  ADR-0024 (a note: RIDL-150 retired, its name reserved, RIDL-157 for every
  kind, and the language server runs the size checks), ADR-0029 (the amendment
  of 2026-10-21 to decision 2: only TYPL-116 remains a lint that starts at
  `warn`), the ridl reference's diagnostics table (the RIDL-150 row retired, the
  RIDL-151 row, which names RIDL-150, and the RIDL-157 row for every kind), the
  typl reference §4.4, §4.5 and §16.2 (the TYPL-116 threshold),
  `docs/book/lints.md` (the TYPL-116 paragraph, the `payload-above-ceiling` row
  and paragraphs, and the sentence that the language server does not run it),
  `docs/book/cli-reference.md` (the `[limits]` section: RIDL-150 retired,
  RIDL-157 for every kind), `docs/book/stability.md` (the two lints that start
  at `warn`), `docs/design/flatbuffers-codec.md` and
  `docs/design/podbuf-codec.md` (no length check in `verify`; the "Known gaps"
  row of driftsys/ridl#1099), `docs/design/proto3-codec.md` (what protects a
  receiver is the transport's refusal against the interaction's limit), the
  frame specification §9 (the refusal before `verify`) and §11.1 (the WebSocket
  blocks of decision 15, a new frame version),
  `docs/design/ridl-transport-ws.md` and `docs/design/ridl-transport-ws-ts.md`
  (the size checks of each side, and the blocks of decision 15), and the freeze
  dump `freeze/diagnostics.txt` (RIDL-150 and its lint name removed), and
  `freeze/accepted-sources.txt` where a frozen source is affected.

## Context

- Each generated payload type has `MAX_SIZE`, the largest encoded size its type
  allows, and a `BUFFER` constant of that size. The FlatBuffers bound is
  computed by `ridl_ir::projection::flatbuffers::max_size`
  (`docs/design/flatbuffers-codec.md`); the proto3 bound is the declaration's
  proto3 size state (`docs/design/proto3-codec.md`). `BUFFER` is a `const`
  array, so each use creates a zeroed array on the stack, and a call can hold
  more than one: an argument buffer, a reply buffer, and in a server the claim
  and event buffers.
- A declaration whose size has no finite bound, or whose bound does not fit a
  `u32`, has no codec today. Both backends refuse it with a generate error and
  no diagnostic code. The ros2 corpus declares
  `RosString : string [0..18446744073709551615]`, so proto3 does not generate
  it. The FlatBuffers backend also rejects ros2 for a separate reason: a vector
  whose element is a vector.
- The proto3 sizer also reports a declaration nested more than 64 levels deep as
  `Overflow`, the same cause as a bound that does not fit a `u32`.
- A typl `string [N]` bound counts Unicode scalar values. Every encoding carries
  a string as UTF-8, so its byte capacity is 4·N. A `bytes [N]` bound counts
  bytes. A `string` or `bytes` with no bound written gets `[0..256]` and the
  warning TYPL-103.
- `encode` returns `EncodeError::Capacity` when the caller's buffer is too
  short; the generated face treats that as impossible, because it always passes
  a buffer of `MAX_SIZE`. The FlatBuffers builder writes from the end of the
  buffer, so when it returns `Capacity` its `needed` is a lower bound. The port
  errors `WriteError`, `RaiseError`, `SendError` and `SettleError` already have
  a `TooLarge { cap }` variant; `CallError` has none.
- `ridl-transport-ws` limited one WebSocket message to 1 MiB by default when
  this record was written (decision 10 changed it; the code default is now
  16,292 bytes). A data-plane frame has at most 36 bytes of header before the
  payload; an `attach` frame is up to 294 bytes.
- The ridl reference §12 defines streams of bounded elements for transfers with
  no total bound. They are not built (driftsys/ridl#336).
- The target transport model is shared memory with 4 KiB pages, a seqlock and a
  header of one or two cache lines.
- On 2026-10-28 the owner set the ceilings of each kind from how long a copy of
  the value may block other work (decision 16).

## Decision

1. **The largest single transfer is 16,384 bytes, header included.** That is
   four 4 KiB pages. A transport reserves at most 128 bytes of it for the header
   of a message that carries a payload. The WebSocket data-plane header is at
   most 36 bytes, so it fits; the `attach` frame carries no payload and is
   limited by the frame specification alone. Amended 2026-10-28: the payload
   ceiling is no longer this transfer minus its header; it is set for each kind
   (decision 16). The transfer does not say how a transport moves a payload.
   Whether a payload travels whole in one message, or is split and queued page
   by page, is a rule of each transport binding, not of the language, the
   catalog, the descriptor or `ridl-rt`. The rule that one payload travels in
   one message is a rule of the WebSocket binding only: its message limit of
   36 + 16,128 = 16,164 bytes (decision 10) carries every payload of a signal,
   an event, a command or a query whole. Until that binding can split a payload
   over several messages, it refuses at attach a catalog whose fixed is larger
   than its message limit, naming the member (decision 15). Another binding may
   split a payload. Nothing about pages, slots or splitting is added to the
   catalog, the descriptor or `ridl-rt`. Amended 2026-10-30: a binding never
   refuses an interaction that the catalog allows (decision 20), so the refusal
   at attach of a catalog whose fixed is larger than the message limit is
   withdrawn. The rule that one payload travels in one message is no longer a
   rule of the WebSocket binding: a payload larger than one message travels in
   blocks (decision 15, "WebSocket: blocks").

2. **The payload ceiling is 16,256 bytes** (16,384 − 128). It is a platform
   rule, stated in the specifications, and it is the same for every encoding,
   runtime and transport. It counts the encoded payload only. The ceiling is
   changed only by amending this record, because a peer with a lower ceiling
   refuses what a peer with a higher one sends; ADR-0029's rule that size values
   are not frozen is otherwise unchanged. Amended 2026-10-28: the payload
   ceiling is set for each kind (decision 16). It is 16,128 bytes for a signal,
   an event, a command and a query after any raise (decision 18), and 65,280
   bytes for a fixed. 65,280 is the largest payload of any interaction, and
   `ridl-rt`'s planned `PAYLOAD_CEILING` is 65,280. How the cap of decision 3
   and the refusal of decision 4, which are not built, apply the smaller ceiling
   of the other kinds is settled when they are built (driftsys/ridl#996). 16,128
   is also the largest `bytes` bound that decision 6 advises. Amended
   2026-10-30: the smaller ceiling of a kind, and a limit that an entry gives,
   is applied by the transport against the catalog, not by the codec (decision
   19), and the codec applies no ceiling: `ridl-rt` has no `PAYLOAD_CEILING`
   (decision 4 as amended). 65,280 stays the largest payload of any interaction,
   as a rule of the catalog. Decision 6 no longer advises 16,128 bytes for
   `bytes`.

3. **`MAX_SIZE` is capped at the ceiling.** The IR size states, the catalog
   descriptor's size rows and the codegen `Model` stay as computed, so they
   still describe the type. Each backend sets the generated `MAX_SIZE` to the
   smaller of the computed bound and the ceiling; the rule that every `MAX_SIZE`
   equals the descriptor's FlatBuffers bound becomes "equals the smaller of that
   bound and the ceiling". An out-of-tree codegen plugin applies the same rule
   from the size state and the ceiling stated in the frame specification; a
   capped value in the `Model` would be an addition, left for later. A type
   whose computed size is larger than the ceiling, has no finite bound, or
   overflows a `u32` now generates in that codec, with `MAX_SIZE` = 16,256. The
   refusals that are not about size stay: an untyped position, a stream, the
   FlatBuffers rule that a vector's element cannot be a vector, and proto3's
   nesting limit of 64 levels. So that the two proto3 causes can be told apart,
   the sizer reports the nesting limit as a new cause, `Depth`; a new enum value
   in the codegen protocol lands in a minor release (ADR-0029).

   Amended 2026-10-30: this decision is withdrawn, unbuilt. It follows from the
   withdrawn codec guard (decision 4 as amended) and from the catalog as the one
   source of a size limit: a cap needs a ceiling in the codec. `MAX_SIZE` stays
   the computed bound and equals the descriptor's bound. A type with no finite
   bound, or one whose bound overflows a `u32`, keeps today's behaviour: the
   backend refuses to generate its codec. RIDL-157 makes that refusal
   unreachable for any payload that an interaction carries (decision 17 as
   amended), so only a type that no interaction carries can meet it. The new
   proto3 cause `Depth` is not needed and is not added.

4. **A value larger than the ceiling is refused at run time.**
   - `encode` returns a new variant, `EncodeError::TooLarge { ceiling }`, when
     the encoded value would be larger than the ceiling. Both encoders write
     into at most the first `PAYLOAD_CEILING` bytes of the caller's buffer,
     whatever its length. The proto3 encoder computes the exact length first and
     returns `TooLarge` before it writes. The FlatBuffers encoder returns
     `TooLarge` when it runs out of space and the space it had was the ceiling,
     and `Capacity` when the caller's buffer was shorter than the ceiling. So
     `TooLarge` wins whenever the buffer is at least the ceiling, which the face
     always passes.
   - The generated face maps `EncodeError::TooLarge` to the port's
     `TooLarge { cap }`, with `cap` = the ceiling: `WriteError` for a signal,
     `RaiseError` for an event, `SendError` for a call's arguments, and
     `SettleError` for a provider's reply. A reply refused this way is answered
     to the consumer as the transport answers an unsendable reply today, with
     the outcome `corrupt`. No path panics.
   - `verify` returns `VerifyError::Structure(Malformed::TooLarge)` when the
     input is longer than the ceiling. FlatBuffers keeps its existing, stricter
     refusal of an input longer than `MAX_SIZE`. proto3 gains the ceiling check
     only: its verifier skips unknown fields, so a payload from a newer writer
     may be longer than this type's `MAX_SIZE` and is still valid.
   - `ridl-rt` exports the ceiling as `PAYLOAD_CEILING: usize = 16_256`, so
     every runtime and transport reads one constant.
   - Amended 2026-10-30. The codec part of this decision is withdrawn, all four
     items: `encode` gains no `EncodeError::TooLarge` and keeps only
     `EncodeError::Capacity`, `verify` refuses no input for its length, neither
     above `MAX_SIZE` nor above a ceiling (decision 19), and `ridl-rt` exports
     no `PAYLOAD_CEILING`. The decision becomes: **the size of a payload is
     refused by the transport, against the limit that the catalog gives its
     interaction**, the default ceiling of its kind or the limit an entry gives.
     On the sending side, the port is to refuse a payload longer than its
     member's limit with its `TooLarge { cap }`, `cap` being that limit, and the
     generated face returns that error. This is a rule to build: today `cap` is
     the capacity of the runtime's slot, `ridl-loopback` never returns it, and
     nothing compares a payload with the catalog's limit (driftsys/ridl#1128).
     On the receiving side, the endpoint refuses a payload longer than the limit
     before it verifies it (decision 19). Reasons: the catalog is the one source
     of truth for a size limit (decision 15); every send goes through a port,
     which is to check the interaction's limit with its `TooLarge { cap }`,
     which is never above 65,280 bytes, so a codec guard at 65,280 would never
     refuse first; RIDL-157 makes the `MAX_SIZE` of every payload type that an
     interaction carries finite and within its limit (decision 17 as amended);
     and a ceiling of a transfer inside the codec would wrongly refuse an encode
     that no transport carries, such as a value of a library type written to a
     file. `encode` returns `EncodeError::Capacity` for a value longer than the
     caller's buffer, as today; the face always passes a buffer of `MAX_SIZE`,
     which decision 3 no longer caps.

5. **`BUFFER` stays.** Each use creates an array of at most 16,256 bytes on the
   stack. A call holds an argument buffer and a reply buffer; a client also
   holds its next-event buffer, and a server one interface-wide claim buffer.
   The owner judged that size acceptable. The encode API that takes a caller's
   buffer stays as the alternative for a target with a smaller stack. Amended
   2026-10-28: generated code puts no `BUFFER` larger than 16,128 bytes on the
   stack. A signal, an event, a command and a query are at most 16,128 bytes
   (decision 2), so their `BUFFER` stays as this decision states. A fixed above
   16,128 bytes is read through a view, in place, and its provider encodes it
   through the encode API that takes a caller's buffer.

6. **The largest advised bound is 4,032 characters for `string` and 16,128 bytes
   for `bytes`.** A string at 4,032 characters is at most 16,128 bytes of UTF-8.
   With the 128-byte header and about 32 bytes of FlatBuffers overhead it fits
   in 16,384 bytes: (16,384 − 128 − 32) / 4 = 4,056, rounded down to a multiple
   of 64. `bytes` takes the same byte size, 4,032 × 4.

   Amended 2026-10-30: the bound is no longer an advised bound but the largest
   bound that some interaction kind can carry. The largest payload of any kind
   is 65,280 bytes, the default ceiling of a fixed (decision 16), so the largest
   bound is 65,280 bytes for `bytes` and 16,256 characters for `string`. The
   string bound is derived as 4,032 was: a string whose bound is N characters is
   at most 4·N bytes of UTF-8, and the largest framing of a payload that is one
   string is that of FlatBuffers, measured at 58 bytes (a query's reply or a
   fixed of type `string [0..16305]` has a FlatBuffers `MAX_SIZE` of 65,278
   bytes = 4 × 16,305 + 58; proto3 adds 4 bytes and podbuf 17). So (65,280 − 58)
   / 4 = 16,305, rounded down to a multiple of 64, is 16,256 = 254 × 64. The
   ceiling of 65,280 is a payload size with no header in it, so no header is
   subtracted, where 4,032 subtracted 128 bytes of a transfer. A
   `string [0..16256]` is at most 65,024 + 58 = 65,082 bytes in FlatBuffers, so
   it fits a fixed in every encoding with 198 bytes left for other framing. The
   number 16,256 is also the payload ceiling of decision 2 as first written; the
   two values are not related.

7. **Two lints, both at level `warn`.**
   - TYPL-116 `bound-above-transfer`: a `string` bound above 4,032 or a `bytes`
     bound above 16,128. It is reported on the bound, in the typl checker.
   - RIDL-150 `payload-above-ceiling`: a payload type whose computed bound, in
     an encoding that a `[backend.<name>]` table of the workspace manifest
     enables, is larger than the ceiling or has no finite bound. It catches
     structs, arrays and maps whose members are each within the advised bounds.
     It runs in `ridlc` after lowering, reads the encodings from the `encodings`
     of every `[backend.<name>]` table of the workspace manifest, as one set,
     and checks FlatBuffers when none is named. It is reported on the
     interaction that carries the payload, so a type no interaction carries is
     not reported. Its row goes in the ridl reference's diagnostics table, and
     in the profile codes that the corpus test lists.
   - Amended 2026-10-28: RIDL-150 reports a `command` or a `query` whose payload
     is above 16,128 bytes, and a `fixed` whose payload is above 65,280 bytes,
     or either with no finite bound, where no `[limits]` entry gives the
     interaction its limit. It now sizes a `fixed`, which it did not before. A
     signal and an event are checked by RIDL-157 (decision 17), and an
     interaction that an entry gives a limit by RIDL-151 (decision 14), so
     RIDL-150 reports neither. Its message gives the ceiling of the
     interaction's kind.
   - Both messages say that the payload ceiling is 16,256 bytes and that the
     codecs do not yet enforce it; once the codecs refuse a larger value, as
     decision 4 will make them, the messages will say that. Amended 2026-10-30:
     no codec refuses a size (decision 4 as amended); the TYPL-116 message
     changes as the next item states, and RIDL-150 is retired. A package can
     raise either to `deny` in `[lints]`. The limits are constants in
     `ridl_core::limits`, read by both checks, not `[lints]` settings, as
     ADR-0027 decision 4 does for design lints; neither is a design lint in the
     sense of ADR-0027, because both check a platform rule, not a quality
     measured on a corpus.
   - Amended 2026-10-30, TYPL-116: it reports a `string` bound above 16,256
     characters or a `bytes` bound above 65,280 bytes (decision 6 as amended), a
     bound that no interaction kind can carry. It stays a lint at `warn`, on the
     bound, in the typl checker. Size is a property of a payload in its
     interaction kind, so the interaction checks of decision 17 decide whether a
     payload fits. The threshold of 4,032 characters and 16,128 bytes was right
     only for a command and a query: it warned on a legitimate fixed, whose
     ceiling is 65,280 bytes, and said nothing useful for a signal or an event,
     whose ceiling is 3,840 bytes. The type lint still serves a type that no
     interaction carries, such as a type of a typl library package, and gives
     feedback in the editor on the bound itself. Its message names the largest
     payload of any kind, 65,280 bytes, and no longer names the payload ceiling
     of 16,256 bytes or says that the codecs do not enforce it: the transport
     refuses a size, against the catalog's limit (decision 4 as amended).
   - Amended 2026-10-30, RIDL-150: the lint `payload-above-ceiling` is retired.
     The case it reported, a command, a query or a fixed above its kind's
     default ceiling or with no finite bound, with no entry for it, is the error
     RIDL-157 (decision 17 as amended). The number 150 joins the retired `RIDL-`
     codes, and the name `payload-above-ceiling` stays reserved and is never
     reused (ADR-0024 decision 7). A `[lints]` key that names it is an unknown
     lint name, so it draws the warning MANI-010 `unknown-lint` and is ignored,
     as ADR-0024 decision 11 states for any name that is not registered; it does
     not stop a check. The earlier items of this decision that describe RIDL-150
     describe the lint until the amendment is built.

8. **The default stays `[0..256]`.** A `string` or `bytes` with no bound written
   keeps the TYPL-103 default and warning.

9. **ADR-0029 decision 2 is amended for these two lints.** A new lint starts at
   `allow` or `info` under that decision. These two start at `warn`, because a
   payload above the ceiling will be refused at run time once decision 4 lands,
   and a lower level would hide that failure until then. A `warn` diagnostic
   does not fail `ridl check`, so no source that compiled before stops
   compiling. Amended 2026-10-30: RIDL-150 is retired (decision 7 as amended),
   so TYPL-116 is the one exception that remains. RIDL-150 started at `warn` and
   not at `error` to keep every source that compiled before compiling; the
   ceiling is a hard limit at run time for every kind (decision 17 as amended),
   so that choice is withdrawn.

10. **The WebSocket limit is at most one transfer.** `ridl-transport-ws` changes
    its `max_message_bytes` default from 1 MiB to 36 + 16,256 = 16,292 bytes,
    the largest data-plane message, within one transfer. Amended 2026-10-24:
    `with_max_message_bytes` may only lower the limit; a value above 16,292 is
    refused, as `with_max_retained_bytes` refuses a value outside its range
    today. The earlier text, which let a deployment set a larger value, is
    withdrawn, because the limit sizes the platform memory. A deployment that
    lowers the limit below what its catalog's limits allow (decision 15) closes
    the session of a peer that stays within them, so a lower value is for a
    deployment that knows its peers. Amended 2026-10-28: the largest data-plane
    message is 36 + 16,128 = 16,164 bytes, so the default and the largest value
    are 16,164, not 16,292. Amended 2026-10-30: a deployment can no longer lower
    the limit below what its catalog's limits need in a way that closes the
    session of a peer that keeps them. The catalog is the one source of a size
    limit (decision 15), so a `Config` whose message limit cannot carry a
    payload that the catalog allows, whole or in blocks, is refused at attach,
    as a configuration error.

11. **Data larger than one transfer is a stream.** A firmware image, a file or
    an image larger than 16 KiB is modelled as a stream of bounded elements
    (ridl reference §12). Until streams are built (driftsys/ridl#336) the
    platform carries no single value larger than one transfer. Amended
    2026-10-28: a value larger than 16,128 bytes, or 65,280 bytes for a fixed,
    is a stream; no `[limits]` entry raises a member above it. Amended
    2026-10-31 from [ADR-0032](ADR-0032-byte-streams.md) decision 8: such data
    is a byte stream (`<bytes>` or `<string>`, ADR-0032 decisions 2 and 3) when
    its chunking is not part of the contract, or a stream of bounded elements
    (ADR-0032 decision 4) when each element carries meaning.

12. **Each interaction has its own payload limit, set in the manifest** (added
    2026-10-24). Each kind of interaction may allocate and carry its payload in
    a different way, so each interaction has its own limit, in bytes of encoded
    payload. A unit declares its limits in a `[limits]` table of its own
    `ridl.toml` (ADR-0002); a workspace root's table does not apply to its
    members:

    ```toml
    [limits]
    default = 4096                  # every kind with no entry below

    [limits.kind]
    signal = 256
    event = 2048
    query = 16128

    [limits.interaction]
    "Climate.temperature" = 64      # Interface.member, in this unit
    ```

    The limit of an interaction is, first found: its entry in
    `[limits.interaction]`, the entry of its kind (`signal`, `fixed`, `event`,
    `command` or `query`) in `[limits.kind]`, `default`, and the payload ceiling
    of decision 2. With no `[limits]` table, every interaction's limit is the
    ceiling. The table names only the unit's own interactions, by the
    interface's or inline service's name without its package, then `.`, then the
    member. An inline service `demo.svc` is keyed `svc.member`; a key with the
    package (`demo.svc.member`, `pkg.Iface.member`) names nothing. When two
    interfaces or inline services of one unit share a short name, a key that
    uses the name is ambiguous and is reported with the same code as a key that
    names nothing. A limit of 0 is valid. The table of a remote unit that the
    workspace imports is read as well, so a consumer records the same limits as
    the owner and computes the same catalog hash; a limit of a remote unit is
    recorded and is not checked again on the consumer side. An unknown kind and
    a value above 16,256 are manifest errors with a `MANI-` code (ADR-0002); a
    name that matches no interaction is a checker error with a `RIDL-` code. The
    codes are MANI-021 for an unknown kind, MANI-022 for a value above the
    ceiling, and RIDL-152 for a name that matches no interaction. The ceiling
    stays the upper bound for every interaction and is changed only by amending
    this record. Amended 2026-10-28: the last step of that order is the default
    ceiling of the interaction's kind (decision 16), and with no `[limits]`
    table every interaction's limit is that default. A `[limits.kind]` entry
    above the default of its kind (3,840 for `signal` and `event`, 16,128 for
    `command` and `query`, 65,280 for `fixed`) is MANI-022, which now means a
    value above what the entry may give. `default` applies to each kind up to
    that kind's default, with no error: a signal or an event whose limit comes
    from a `default` above 3,840 has the limit 3,840. A `[limits.interaction]`
    entry may raise its member above the default of its kind, up to 16,128, or
    65,280 for a fixed (decision 18). The manifest layer raises MANI-022 for a
    `[limits.kind]` entry and for a `[limits.interaction]` entry above 65,280.
    An entry between 16,129 and 65,280 is valid only when it names a fixed, and
    the manifest layer does not know the kind of the member a key names, so the
    pass of `ridlc` that resolves the keys raises MANI-022 for it, on the value,
    as the checker raises MANI-009.

    The limits are in the manifest, not in an attribute, because a signal, an
    event and a fixed take no attribute block (`family-general-form.md` §4.3),
    and the catalog-wide and per-kind defaults have no declaration to sit on.
    The general form and the attribute registry do not change.

13. **The resolved limits are part of the catalog** (added 2026-10-24). A pass
    of `ridlc` that runs after the checks resolves each interaction's limit and
    records it in the IR on the interaction, so the catalog hash covers it. The
    checker and lowering of `ridl-sem` leave the limit unset. An interaction
    whose limit is the ceiling because no entry applies records no limit, so a
    unit with no `[limits]` table keeps its catalog hash. `ridl diff` reports
    every change of a limit, a raise or a lowering, as breaking, so a catalog
    whose limits changed is not on the `compatible` chain of the earlier one.
    Two peers whose limits differ therefore have different hashes, neither
    accepts the other, and the attach is refused (frame specification §6.1). The
    catalog descriptor's member table gains a column with each member's limit.
    Amended 2026-10-28: an interaction whose limit is the default ceiling of its
    kind because no entry applies records no limit. The default ceiling of a
    kind is a platform constant, not recorded in the IR. A unit whose entries
    are unchanged keeps its catalog hash, except when its `default` is above the
    default ceiling of a kind: the limit of that kind is then clamped to the
    ceiling and recorded, so the hash changes. With `default = 4096`, a signal
    recorded the limit 4,096 and now records 3,840. A limit that an entry gives
    is recorded, covered by the hash and carried in the descriptor's limit
    column, as before, including a limit that an entry raises above the default
    of its kind (decision 18). An absent limit in the IR and in the descriptor
    means the default ceiling of the member's kind.

14. **A payload above a declared limit is an error** (added 2026-10-24). When a
    `[limits]` entry gives an interaction its limit, and the computed bound of
    the interaction's payload, in an encoding the workspace enables (the same
    set as RIDL-150), is larger than that limit or has no finite bound, the
    checker reports a new error, RIDL-151, on the interaction. As an error code
    it has no name and no level (ADR-0024 decision 1). It is an error, not a
    lint, because the package declared the limit and the payload breaks it.
    Where the limit is the ceiling because no entry applies, RIDL-150 stays at
    `warn` (decisions 7 and 9): no source that compiled before this amendment
    stops compiling, and the ros2 corpus is unchanged. A call checks its
    arguments and its reply against its one limit; a signal, a fixed and an
    event check their value. RIDL-150 does not size a `fixed` today, so RIDL-151
    adds that sizing. Amended 2026-10-28: RIDL-151 checks an interaction of any
    kind against the limit that an entry gives it, including a raised limit
    (decision 18). Where no entry applies, a signal and an event are checked by
    the error RIDL-157 (decision 17), and a fixed, a command and a query by
    RIDL-150 at `warn` (decision 7). One interaction draws at most one of the
    three. Amended 2026-10-30: where no entry applies, every kind is checked by
    RIDL-157 (decision 17 as amended), and RIDL-150 is retired. One interaction
    draws at most one of RIDL-151 and RIDL-157.

15. **The transport derives its limits from the catalog** (added 2026-10-24). A
    server whose backend has a member table reads each member's limit from it.
    The message limit stays the `Config` value of decision 10, never less than
    the 294-byte floor of the frame specification §11.1 (`MIN_MESSAGE_LIMIT`),
    and a message above it still closes the session with 1009 (§11.1). A
    `request` within that limit whose arguments are longer than its member's
    limit is answered with the outcome `corrupt`, as for arguments that do not
    verify (§9.3). This is a check of length against the catalog, not a
    verification of the payload, and it amends §11.1 item 4 for a `request`
    only. The server answers a repeated `seq` first, then an unknown member,
    then checks the length, and records no note for the refusal. A publish is
    not checked: the Rust server receives none, and a client receives only what
    its server sent. A backend with no member table, or a descriptor without the
    limit column, applies the message limit alone. The retained-bytes cap has
    the largest message limit, 16,292, as its smallest valid value; the clamp
    that raises a smaller cap to the message limit
    (`crates/ridl-transport-ws/src/server/session.rs`) is removed, and
    `RetainedTooSmall` states that minimum, which is the constant 16,292
    whatever the configured message limit. `with_max_message_bytes` refuses a
    value outside 294 to 16,292 with one error, `LimitOutOfRange`. 16,292 is the
    Rust provider's maximum and default; the frame specification does not make
    it a rule for every receiver of the binding. `with_max_retained_bytes` keeps
    its role of sizing the memory an embedder grants a session, above that
    minimum. Amended 2026-10-28: every 16,292 in this decision reads 16,164, the
    largest message of decision 10 as amended. A `request` whose arguments are
    longer than a limit that an entry raised (decision 18) is still answered
    against that member's limit in the member table. The binding carries one
    payload per message, so until it can split a payload over several messages,
    a provider refuses at attach a catalog that holds a fixed whose payload's
    largest encoded size, in the session's encoding, does not fit in one message
    within its message limit, and names the member. A fixed whose payload fits
    is served as before, whatever its ceiling. Splitting a fixed over several
    messages is tracked with the stream framing of ADR-0032, drafted in
    parallel.

    Amended 2026-10-30: the refusal at attach of the amendment of 2026-10-28 is
    withdrawn, because a binding never refuses an interaction that the catalog
    allows (decision 20). The check of a `request` against its member's limit
    stays; it is the transport-side refusal of decision 19 on the provider's
    side. The sentence that a publish is not checked is replaced by decision 19:
    every endpoint that receives a payload checks its length against its
    member's limit, so the client side checks a `reply` and a `publish`.

    **WebSocket: blocks** (the owner, 2026-10-30: "the catalog is the SSOT,
    transport and receiver must match, doing blocks if needed").
    - The limit that the catalog gives an interaction, the default ceiling of
      its kind or the value a `[limits.interaction]` entry allows, is the one
      source of truth for the size of its payload. The sender, the binding and
      the receiver each take their limit for the interaction from it; none of
      them has its own smaller or larger limit for a payload. A server `Config`
      cannot refuse a payload that the catalog allows: a `Config` that would
      refuse one is refused at attach, as a configuration error, and never
      causes a refusal of one message at run time.
    - The receiver sizes its receive buffer for an interaction from the
      catalog's limit, and refuses only a payload above that limit, with the
      outcome of decision 19 and the frame specification §9.
    - The binding keeps its bounded message size: 36 + 16,128 = 16,164 bytes
      (decision 10). A payload that fits one message is carried whole, as today.
      Blocks are used only for a payload larger than one message. With a message
      limit of 16,164 bytes, a signal, an event, a command and a query always
      fit one message, even when raised (decision 18), so today only a fixed
      above 16,128 bytes, of up to 65,280 bytes, is sent in blocks, in one
      direction: the provider splits the payload, and the consumer reassembles
      the blocks into a buffer sized from the catalog's limit, then verifies the
      payload. The provider never reassembles. The size of a block is settled in
      the design note of driftsys/ridl#1129.
    - Blocks need a new frame version, because the header's field set is closed
      (frame specification §4), carried by a new binding version offered beside
      version 1 (§11.1 "Later versions"). The blocks need: a part index or a
      last-block flag; a bound on reassembly for each session; a rule for a
      missing, repeated or out-of-order block, and for a total above the
      member's limit; and a rule on whether the blocks of one payload may
      interleave with other frames. The layout is left to a design note of the
      frame specification, tracked in driftsys/ridl#1129.
    - This replaces the rule of 2026-10-28 that the WebSocket binding refuses at
      attach a catalog whose fixed is larger than its message limit, and the
      rule that one payload travels in one message as a rule of the WebSocket
      binding (decision 1). It is consistent with the ruling of 2026-10-28 that
      splitting is a concern of the transport: nothing about blocks goes into
      the catalog, the descriptor or the codecs of `ridl-rt`.

16. **Each kind has a default payload ceiling** (added 2026-10-28). The default
    ceiling of each kind, in bytes of encoded payload, is:

    | Kind               | Default ceiling |
    | ------------------ | --------------- |
    | `signal`, `event`  | 3,840 bytes     |
    | `command`, `query` | 16,128 bytes    |
    | `fixed`            | 65,280 bytes    |

    The numbers are the same on every target, so a catalog that is valid on one
    target is valid on every target. The default ceilings are constants of the
    checker, in `ridl_core::limits` beside the payload ceiling; they are not
    recorded in the IR (decision 13) and are not added to `ridl-rt` or to a
    transport. Amended 2026-10-30: a transport applies the limit of each member
    (decision 19), and an absent limit in the descriptor means the default
    ceiling of the member's kind (decision 13), so a transport needs the three
    values. `ridl-rt` exports them as values of the catalog rule, so that a
    transport resolves an absent limit; they are not a guard of the codec.
    `ridl_core::limits` and `ridl-rt` are kept equal by a test. They are still
    not recorded in the IR.

    A ceiling is compared with the payload's encoded `MAX_SIZE` under each
    encoding the workspace manifest enables (decision 17), so the framing of a
    codec counts inside the ceiling. For a signal whose payload is one
    `string [0..N]`, the largest N that fits 3,840 bytes is 945 characters in
    FlatBuffers, 959 in proto3 and 955 in podbuf; the sizer counts 4 bytes per
    character.

    A transport binding should copy at most 4,096 bytes between two scheduling
    points, so that a large call does not block the transport for longer than
    the copy of 4,096 bytes.

    Rationale. A ceiling bounds the longest copy that nothing can interrupt. A
    signal or an event is held as one cell and copied whole under the seqlock,
    so the size of the cell is the time a reader or a writer blocks. A command
    or a query is decoded whole by the receiving endpoint, which on a
    single-threaded embedded executor stalls all other work while it decodes;
    16,128 bytes take about 40 to 80 microseconds on a Cortex-M7 class target. A
    fixed is usually memory-mapped and read in place, with no copy, so its
    ceiling is larger; 65,280 bytes is safe for the address space and fits a
    `u16` length. Amended 2026-10-31: the interaction's limit bounds each chunk
    of a byte stream the same way ([ADR-0032](ADR-0032-byte-streams.md) decision
    14); the sentence read "a byte stream (ADR-0032, drafted in parallel) bounds
    one chunk the same way" before.

    The numbers come from memory pages of 4,096 bytes with 256 bytes of each
    cell reserved, once per cell: a ceiling is the cell's pages × 4,096 − 256. A
    signal and an event have one page, a command and a query four, and a fixed
    sixteen. The 256 bytes leave room for a header before the payload (a
    sequence counter and the envelope) and for a trailer after it (end markers
    such as an end version, a checksum or a commit marker), 128 bytes each, so
    that the payload starts on a line boundary on a target with 128-byte cache
    lines. This record does not bind an engine to that layout: it fixes the
    ceilings, and an engine lays out its cells as it chooses. 3,840 is 30 × 128,
    and 16,128 equals the largest `bytes` bound that decision 6 advised before
    its amendment of 2026-10-30.

    Reserving more now is the safer direction: raising a ceiling later is not a
    break for sources, because every source that compiled still compiles, and
    lowering it is.

17. **A signal or an event above its default ceiling is an error** (added
    2026-10-28). When no `[limits]` entry gives a signal or an event its limit,
    and the computed bound of its payload, in an encoding the workspace enables
    (the same set as RIDL-150), is larger than 3,840 bytes or has no finite
    bound, the checker reports a new error, RIDL-157, on the interaction. As an
    error code it has no name and no level (ADR-0024 decision 1), so `[lints]`
    cannot lower it. It runs in `ridlc` after lowering, where RIDL-150 and
    RIDL-151 run, and it skips the packages that RIDL-151 skips. It is an error,
    not RIDL-150's warning, because the ceiling bounds the copy that a reader or
    a writer of the value makes without interruption (decision 16). The error is
    on the interaction, not on the type: a type that a fixed, a command or a
    query carries has the default ceiling of its kind.

    Amended 2026-10-30: **one rule for every kind.** When no
    `[limits.interaction]` entry gives an interaction its limit, and the
    computed bound of its payload, in an encoding the workspace enables, is
    larger than the default ceiling of its kind (3,840 bytes for a signal and an
    event, 16,128 bytes for a command and a query, 65,280 bytes for a fixed) or
    has no finite bound, the checker reports RIDL-157 on the interaction, for
    every kind. A call checks its arguments and its reply; a query whose reply
    is a fallible `T | E` is checked through both types. RIDL-157 replaces the
    lint RIDL-150, which is retired (decision 7 as amended). The ceiling is a
    hard limit at run time for every kind: the transport refuses a larger
    payload on both sides, against the catalog's limit (decisions 4 and 19). A
    warning allowed code to be built that then fails in production, which is not
    consistent with that; the level `warn` of RIDL-150 was a choice made to keep
    earlier sources compiling, and it is withdrawn. The way to accept a larger
    payload is a `[limits.interaction]` entry (decision 18): it is a value of
    the contract, recorded in the IR, covered by the catalog hash and carried in
    the descriptor, so each peer and each engine sees it. A comment directive
    that allows a lint in the source, such as the one that driftsys/ridl#1126
    proposes for ADR-0024, cannot replace that entry: it changes no contract
    value, and it cannot silence RIDL-157, which is an error and not a lint.
    Above what an entry may give, the payload is a stream (decision 11,
    ADR-0032). The same pass that sizes the payloads raises RIDL-151, RIDL-152
    and RIDL-157, and it skips the same packages.

18. **An entry may raise one member up to 16,128 bytes** (added 2026-10-28). A
    `[limits.interaction]` entry that names one member, of any kind, may give it
    a limit above the default ceiling of its kind, up to 16,128 bytes. A fixed
    may be given up to 65,280 bytes, which is its default, so in effect a fixed
    is not raised. The entry is the allowlist: `[limits.kind]` and `default`
    cannot raise a kind above its default (decision 12). The raised limit is
    recorded in the IR, covered by the catalog hash and carried in the
    descriptor's limit column (decision 13), so an engine sees each signal or
    event whose limit is above 3,840 bytes, and either supports it or refuses
    the catalog, naming the member. Nothing else is raised above 16,128 bytes:
    larger data is a stream (decision 11).

19. **The transport owns the size of a payload; `verify` checks structure**
    (added 2026-10-30, from driftsys/ridl#1099). A type's `verify`, in every
    codec, checks the structure and the typl constraints of what it reads and
    ignores the bytes past what it reads. It refuses no buffer for its length,
    and in particular no longer refuses a buffer longer than its own `MAX_SIZE`.
    The size refusal is made once, by the transport, against the interaction's
    limit from the catalog: the limit that an entry gives, carried in the
    descriptor's limit column, or the default ceiling of the member's kind when
    the column holds none (decision 13). Every endpoint of a binding that
    receives a payload refuses one longer than that limit before it verifies the
    payload, and reports the refusal as a structure failure of the kind (frame
    specification §9.1 to §9.4): a signal's sample is
    `Invalid(Detected(Corrupt))`, an event's occurrence carries `Err(Corrupt)`,
    a request is answered `corrupt`, a reply is `CallError::Transport(Corrupt)`,
    and a fixed's read reports the detection. This is a check of length, not a
    verification, so the rule that a runtime never verifies (frame specification
    §9) is unchanged. An embedder that calls `verify` on bytes that no binding
    has checked, such as the gate of the podbuf trust model (ADR-0031), checks
    the length against the interaction's limit itself.

    Before this decision was built, the codecs differed: the generated
    FlatBuffers and podbuf `verify` refused a buffer longer than the type's
    `MAX_SIZE` with `Malformed::TooLarge`, and the proto3 `verify` applied no
    length check, because its verifier skips unknown fields
    (`docs/design/proto3-codec.md`). FlatBuffers and podbuf were changed to the
    proto3 behaviour on 2026-10-30 (driftsys/ridl#1108). `Malformed::TooLarge`
    stays in `ridl-rt`, because removing a variant would change its published
    API; the generated code no longer returns it.

    Rationale. ADR-0031 decision 10 makes an appended field compatible under
    podbuf, and ADR-0019 makes an appended field compatible under FlatBuffers.
    An append can raise the type's `MAX_SIZE`, so an older reader that refuses a
    buffer longer than its own `MAX_SIZE` refuses the larger valid buffers of a
    newer writer. Example: v1 `Reading { value: f32 }` has a podbuf `MAX_SIZE`
    of 16 bytes, an 8-byte preamble and an 8-byte head; v2 appends
    `note: string [0..64]`; a v1 reader refuses an 80-byte v2 buffer, although
    the change is compatible. The limit of the interaction does not change with
    an append: it is recorded in the catalog, and a change of it is breaking
    (decision 13). The work of `verify` does not depend on the length check:
    podbuf bounds it by the maxima the reader's type declares (ADR-0031 decision
    8), proto3 by the bytes it reads, which the transport has already bounded,
    and FlatBuffers by the declared maxima and its depth and table limits. This
    closes ADR-0031 open item 2 with its first answer. The code is tracked in
    driftsys/ridl#1128.

20. **A transport binding never refuses an allowed interaction** (added
    2026-10-30). A binding carries every payload up to the limit that the
    catalog gives its interaction: the default ceiling of its kind, or the limit
    that a `[limits.interaction]` entry gives it, a raised limit included. A
    catalog that compiles is therefore served by every binding, whatever the
    sizes of its payloads, and a binding may not refuse it at attach for the
    size of a member. How a binding carries a payload larger than one of its
    messages, whole in a larger message or split into parts, is the binding's
    rule (decision 1); the WebSocket binding uses blocks (decision 15,
    "WebSocket: blocks"). A binding may still refuse a catalog for a reason that
    is not the size of an allowed payload, and a deployment's own configuration
    may still be refused when it cannot carry its catalog.

21. **The language server runs the size checks** (added 2026-10-30). The checks
    of decisions 12, 14 and 17, which `ridlc` runs after lowering (RIDL-151,
    RIDL-152, RIDL-157, and the MANI-022 that the pass of decision 12 raises for
    an entry between 16,129 and 65,280 that does not name a fixed), also run in
    the language server, so an author sees them in the editor, on the
    interaction, as `ridl check` reports them. Before 2026-10-30 the lint
    `payload-above-ceiling` (RIDL-150) and the errors RIDL-151 and RIDL-152 were
    raised only by `ridl check` and `ridl build`; RIDL-157, and MANI-022 for an
    entry between 16,129 and 65,280 that does not name a fixed, came with
    driftsys/ridl#1108, which also built this decision. The language server
    already read the lowered IR for its hovers and inlay hints and ran the
    design lints of `ridlc`; the cost added on each check is the lowering of the
    workspace where it is not already done, the sizing of each interaction's
    payload in each enabled encoding and the resolution of the `[limits]`
    tables. This record does not say how the server schedules that work.

## Where the rules are stated

- typl reference §4.4 and §4.5: the advised bounds and TYPL-116 (decisions 6 to
  8); §16.2: the TYPL-116 row. ridl reference: the RIDL-150 row.
- frame specification §11.1 "The sizes": the transfer and the ceiling (decisions
  1 and 2); §9: the transport's refusal of a payload above its member's limit,
  before `verify` (decisions 4 and 19 as amended).
- `docs/design/flatbuffers-codec.md`, `docs/design/proto3-codec.md` and
  `docs/design/ridl-rt.md`: `BUFFER` (decision 5), and that no codec refuses a
  size (decisions 3 and 4, withdrawn on 2026-10-30).
- `docs/design/interaction-face.md`: the port's `TooLarge { cap }` against the
  member's limit on send (decision 4 as amended).
- `docs/design/ridl-transport-ws.md`: the new default (decision 10).
- `docs/book/lints.md`: the two lint rows.
- ADR-0002 (the manifest): the `[limits]` table (decision 12). ridl reference:
  the RIDL-151 row (decision 14).
- ADR-0014 and `docs/design/catalog-descriptor.md`: the limit in the IR and in
  the member table (decision 13). `docs/book/cli-reference.md` and
  `docs/book/evolving-an-interface.md`: the `ridl diff` limit change (decision
  13). `docs/design/ridl-transport-ws.md` and the frame specification §11.1: the
  message limit and the request check (decisions 10 and 15).
- From the amendment of 2026-10-28, once its code lands: the ridl reference's
  diagnostics table, `docs/book/lints.md` and `docs/book/cli-reference.md`: the
  default ceilings, the raise, MANI-022 and RIDL-157 (decisions 12 and 16 to
  18). The frame specification §11.1 and `docs/design/ridl-transport-ws.md`: the
  ceilings of decision 16, the message limit of 16,164 bytes, and one payload in
  one message as a rule of the WebSocket binding (decisions 1, 2, 10 and 15).
- From the amendment of 2026-10-30, once its code lands: the typl reference
  §4.4, §4.5 and §16.2 and `docs/book/lints.md`: the TYPL-116 threshold
  (decisions 6 and 7). The ridl reference's diagnostics table,
  `docs/book/lints.md` and `docs/book/cli-reference.md`: RIDL-157 for every
  kind, RIDL-150 retired, and the size checks in the language server (decisions
  7, 17 and 21). The codec records and the frame specification §9: `verify`
  without a length check and the transport's refusal (decisions 4 and 19). The
  frame specification §11.1 and `docs/design/ridl-transport-ws.md`: the blocks
  of the WebSocket binding (decisions 15 and 20), once the design note of
  driftsys/ridl#1129 fixes their layout.

## Alternatives considered

- **Keep RIDL-150 at `warn` for a command, a query and a fixed** (2026-10-30).
  Rejected: the ceiling is a limit at run time for every kind, so a warning lets
  code be built that the transport or the codec refuses in production. The level
  `warn` was a choice made to keep earlier sources compiling.
- **Allow an oversized payload with a comment directive in the source**
  (2026-10-30), such as the directive that driftsys/ridl#1126 proposes.
  Rejected: RIDL-157 is an error, not a lint, and the limit a peer must accept
  is a contract value that belongs in the catalog, which a
  `[limits.interaction]` entry records (decision 18).
- **A TYPL-116 threshold for each kind** (2026-10-30). Rejected: a type does not
  know which interaction kinds carry it, and the interaction checks of decision
  17 already apply each kind's ceiling. The type lint reports only a bound that
  no kind can carry.
- **A TYPL-116 string threshold of exactly 16,305 characters** (2026-10-30), the
  largest that fits 65,280 bytes in FlatBuffers when the string is the whole
  payload. Rejected for 16,256, rounded down to a multiple of 64 as decision 6
  rounded 4,032: the exact value depends on the framing of one encoding, and the
  rounding leaves room for the framing of a struct that holds the string.
- **An append that raises `MAX_SIZE` is a break** (2026-10-30), the second
  answer that ADR-0031 open item 2 offered. Rejected: it would make most appends
  to a type with a string, `bytes`, array or map breaking under podbuf and
  FlatBuffers, while proto3 already accepts them, and a reader's length check
  against its own `MAX_SIZE` is not needed to bound the work of `verify`
  (decision 19).
- **A codec guard: `EncodeError::TooLarge` against a `PAYLOAD_CEILING` of 65,280
  in `ridl-rt`** (decision 4 as first written, kept in the first draft of the
  amendment of 2026-10-30). Withdrawn on 2026-10-30: the port's
  `TooLarge { cap }` is to check the catalog's limit, which is never above
  65,280, so the guard would never refuse first, and it would refuse an encode
  outside a transport, such as a library value written to a file.
- **`verify` checks the input against `PAYLOAD_CEILING`** (decision 4 as first
  written). Withdrawn on 2026-10-30: the transport already refuses a payload
  above its interaction's limit, which is never above the ceiling, so a second
  check in each codec adds nothing, and the size refusal is made once (decision
  19).
- **Every WebSocket payload above 4 KiB in blocks of 4 KiB, in both directions**
  (2026-10-30), with a message limit of about 4.1 KB. Rejected: it bounds every
  message to one page and lets a signal pass between the blocks of a large
  command, but it needs reassembly in the server for a command and a query,
  splitting in the TypeScript client, and rules for interleaving, and it saves
  no memory in the receiver, which reassembles a payload whole before it
  verifies it. It can be added later as a change of the WebSocket binding if a
  measurement shows that large commands delay signals.
- **A WebSocket message limit derived from the catalog** (2026-10-30), 36 + the
  largest limit in the catalog, at most 65,316 bytes, with a lower server
  `Config` refused at attach. Rejected by the owner for blocks (decision 15):
  the message size stays bounded, and the memory a message needs does not grow
  with the largest fixed of a catalog.
- **Refuse at attach a catalog that a WebSocket provider cannot carry in one
  message** (decision 15 as amended on 2026-10-28). Withdrawn on 2026-10-30: a
  catalog that compiles is valid, and a binding that refuses it moves a limit of
  the binding into the contract (decision 20).

- **RIDL-150 at level `error` for a signal and an event, or a wider RIDL-151**
  (2026-10-28). Rejected in favour of the new code RIDL-157 (decision 17): a
  lint has one default level for its code, an error code is never a lint
  (ADR-0024 decision 1), and `[lints]` could lower such a level to `allow`.
  RIDL-151 keeps its meaning, a payload above a limit that a `[limits]` entry
  declares.
- **MANI-022 for a `default` above the default ceiling of a kind** (2026-10-28).
  Rejected: a `default` applies to every kind, and a manifest that sets one
  value for every kind would stop compiling. A `default` is applied to each kind
  up to that kind's default (decision 12).
- **One memory page minus one 64-byte cache line, a ceiling of 4,032 bytes**
  (2026-10-28). Rejected: it leaves no reserved room after the payload for an
  end marker, a 64-byte header does not start the payload on a line boundary on
  a target with 128-byte cache lines, and a ceiling that is too high can only be
  lowered by a break of sources.
- **The page and the cache line of the target** (2026-10-28). Rejected: a
  catalog valid on one target would be invalid on a target with smaller pages or
  larger cache lines, and two peers would disagree on what each may send.
- **Pages, slots or splitting in the catalog, the descriptor or `ridl-rt`**
  (2026-10-28). Rejected: how a payload is moved belongs to the transport
  binding (decision 1 as amended), and the catalog states only how large a
  payload may be.
- **A fixed at 3,840 bytes, like a signal** (2026-10-28). Rejected: a fixed is
  usually read in place, so the copy that the signal ceiling bounds is not made,
  and large constant tables such as calibration maps must stay possible.
- **A 16-page maximum for a raise of a signal, an event, a command or a query**
  (2026-10-28), 65,280 bytes. Rejected: a document or a file is a byte stream
  (ADR-0032, drafted in parallel), not a large value, so a raise stops at 16,128
  bytes.
- **A fixed at 16,128 bytes, like a command** (2026-10-28). Rejected: a fixed is
  usually memory-mapped and read in place, so no copy is made for each read, and
  a constant table such as a calibration map can be larger than 16,128 bytes.
- **A ceiling of 65,280 bytes for a command and a query** (2026-10-28).
  Rejected: the receiving endpoint decodes a call whole, and on a
  single-threaded embedded executor a decode of 65,280 bytes stalls all other
  work about four times as long as one of 16,128 bytes. Raising the ceiling
  later is not a break for sources.
- **The 16,256-byte ceiling of decision 2 as first written** (2026-10-28).
  Replaced by the ceilings of decision 16, so that every ceiling follows the
  copy-time rule of that decision.
- **A ceiling of 4 MiB + 64 KiB**, sized for an uncompressed 1024×1024 RGBA PNG.
  Rejected on 2026-10-21: one transfer of 4 MiB is too large for a seqlock
  reader, which copies the payload and retries when a writer overwrites it, and
  a 4 MiB stack array per buffer is too large for an embedded target.
- **An error for a bound above the advised limit.** Deferred: it breaks the ros2
  corpus and test fixtures, and is a break of the `accepted-sources` surface.
  Each lint can be raised to `deny` by a package now and by default later.
- **Report the ceiling through `EncodeError::Capacity`.** Rejected: a caller
  with a larger buffer would bypass the ceiling, and the face treats `Capacity`
  as impossible.
- **Cap the size states in the IR.** Rejected: the size state describes the type
  and feeds the codegen protocol; capping it would hide that a type is unbounded
  and change the frozen `codegen-protocol` surface.
- **Refusing a type with no finite bound, as today.** Rejected: imported schemas
  such as ROS 2 use unbounded strings, and the ceiling already gives a receiver
  a limit it can check.
- **A transport limit an embedder may raise** (the text of decision 10 before
  2026-10-24). Rejected: the limit sizes the platform memory, and a peer with a
  lower limit would close the session of a peer that uses a higher one (frame
  specification §11.1).
- **Raise RIDL-150 to `error`.** Rejected on 2026-10-24: every unbounded string
  of an imported schema would stop compiling, and the ros2 corpus would need a
  bound on each. The error applies only where an author declared a limit
  (decision 14).
- **A `max_bytes` attribute on each interaction.** Rejected on 2026-10-24: a
  signal, an event and a fixed take no attribute block, so the attribute would
  reverse ADR-0008 decisions 2 and 3 and change the language surface, and the
  catalog-wide and per-kind defaults would still need the manifest.
- **A limit in the manifest that the catalog hash does not cover.** Rejected:
  two peers with different limits would attach and then disagree on what each
  may send.
- **Built-in per-kind defaults below the ceiling.** Rejected on 2026-10-24: no
  measurement yet supports a value for any kind. A project sets smaller values
  in `[limits.kind]`.
- **One limit for every interaction.** Rejected on 2026-10-24: interactions
  allocate and carry payloads in different ways.
- **One lint on single bounds only.** Rejected: a struct of two strings at the
  advised bound, or a large array, passes it and fails only at run time.

## Consequences

- The ros2 corpus generates a Rust proto3 codec. FlatBuffers still rejects it
  for the nested vector. The `wire` dump does not change, because it reads the
  `.proto` and `.fbs` schemas, which never depended on `MAX_SIZE`.
- TYPL-116 warns on six `RosString` declarations in `evals/corpus/ros2/`, on
  their frozen copies, and on two fixtures in
  `crates/ridl-backend-rust/tests/fixtures/`. The book example
  `FwBlock: bytes [1..65536]` in `docs/book/getting-started.md`, the same bound
  in `crates/ridl/tests/book_examples.rs` and in a code span of the typl
  reference, and the unbounded string in
  `crates/ridl-backend-rust/tests/proto3_roundtrip.rs` warn, and are changed to
  a bound within the limit. The tests that expect the "no finite bound" generate
  error change to expect a capped `MAX_SIZE`.
- RIDL-150 warns on the ros2 array fields above the ceiling, and on the frozen
  veh-cluster fixture, whose `getFaultPage` reply has a generated `MAX_SIZE` of
  70,649 bytes. The generated crates under `freeze/generated/` change to the
  capped values; ADR-0029 decision 2 does not freeze size values and the
  generated-crate check compares the public API, so `cargo xtask freeze update`
  records the change.
- A value larger than 16,256 bytes that encoded before is now refused, and a
  deployment that sent WebSocket messages above one transfer can no longer do so
  (decision 10 as amended on 2026-10-24). Both are breaking changes, released in
  a minor version before 1.0.
- The diagnostics surface gains TYPL-116 and RIDL-150. `ridl-rt` gains
  `PAYLOAD_CEILING` and the `EncodeError::TooLarge` variant; `EncodeError` is
  `#[non_exhaustive]`, so the variant is an addition.
- From the amendment of 2026-10-24: the manifest gains the `[limits]` table, the
  IR gains a limit per interaction (covered by the catalog hash), the
  descriptor's member table gains a limit column, `ridl diff` gains a limit
  change, and the diagnostics surface gains RIDL-151, RIDL-152 and the manifest
  errors MANI-021 and MANI-022 of decision 12. `with_max_message_bytes` can no
  longer raise the limit and the retained-cap clamp is removed; both change the
  published `ridl-transport-ws` API and are released in a version marked
  breaking. The new IR field and the new descriptor column are additions to the
  frozen `ir` surface and the descriptor schema under ADR-0029; the freeze dumps
  are updated when the code lands.
- From the amendment of 2026-10-28: a signal or an event whose payload is larger
  than 3,840 bytes or has no finite bound, with no `[limits]` entry for it,
  which drew RIDL-150 or nothing before, now draws RIDL-157 and fails
  `ridl check`; an entry in `[limits.interaction]` that raises it up to 16,128
  bytes makes it compile again. A `[limits.kind]` entry above its kind's
  default, and a `[limits.interaction]` entry above 16,128 for a kind other than
  `fixed` or above 65,280 for a fixed, now draw MANI-022. Both remove accepted
  sources, a breaking change of the `accepted-sources` surface (ADR-0029). A
  command or a query between 16,129 and 16,256 bytes now draws the warning
  RIDL-150, and a fixed above 65,280 bytes or with no finite bound draws it for
  the first time; a warning does not stop a source compiling. The WebSocket
  message limit falls from 16,292 to 16,164 bytes, a change of the published
  `ridl-transport-ws` API, and a WebSocket provider refuses at attach a catalog
  whose fixed is larger than that limit. The values 16,256 and 16,292 are not
  yet released, so all of this ships in the next version marked breaking.
  Measured on `main` at 6d9903db: no shipped schema has a signal or an event
  near 3,840 bytes. The sources that stop compiling are the two unbounded ROS 2
  feedback events in `evals/corpus/ros2/nav2_msgs` (`followPathFeedback` and
  `navigateToPoseFeedback`, RIDL-157) and the TypeScript codec fixture
  `crates/ridl-backend-ts/tests/fixtures/codec/shape.ridl` (the signals
  `Sensors.note` and `Sensors.extras`, and the event `Sensors.bounds`);
  driftsys/ridl#1108 bounds them. The ROS 2 queries and commands are unbounded
  and keep a RIDL-150 warning. The replies of `VehicleStatus.getFaultPage` in
  the veh-cluster corpus are 67 to 70 KB, above 16,128 bytes, and keep a warning
  until the reply becomes a stream (ADR-0032). No fixed is above 16,256 bytes or
  unbounded. The diagnostics surface gains RIDL-157. A unit whose `[limits]`
  entries are unchanged keeps its catalog hash, except a unit whose `default` is
  above the default ceiling of a kind, whose clamped limit is recorded (decision
  13). The IR and the descriptor schema do not change.
- From the amendment of 2026-10-30: every command, query and fixed whose payload
  is above the default ceiling of its kind or has no finite bound, with no
  `[limits.interaction]` entry for it, which drew the warning RIDL-150 before,
  now draws the error RIDL-157 and fails `ridl check`. This removes accepted
  sources, a breaking change of the `accepted-sources` surface, and the removal
  of RIDL-150 and its lint name is a breaking change of the `diagnostics`
  surface (ADR-0029); both ship in the same version marked breaking as the
  amendment of 2026-10-28 (driftsys/ridl#1108). A `[lints]` key that names
  `payload-above-ceiling` draws MANI-010 and is ignored. Measured on `main` at
  952ebb2a with `ridl check`, the sources that stop compiling, beyond those that
  the amendment of 2026-10-28 lists, are:
  - `evals/corpus/ros2` and its frozen copy under
    `xtask/tests/freeze-fixtures/sources/evals/corpus/ros2`: 15 commands and
    queries with no finite bound in a request or a reply (`clearEntireCostmap`,
    `followPath`, `followWaypoints`, `getCostmap`, `isPathValid`, `loadMap`,
    `navigateToPose` and `saveMap` of `nav2msgs`; `getMap`, `getPlan`, `loadMap`
    and `setMap` of `navmsgs`; `setCameraInfo` of `sensormsgs`; `setBool` and
    `trigger` of `stdsrvs`). An entry cannot help, because RIDL-151 also refuses
    a payload with no finite bound, so each needs a bound.
  - The veh-cluster corpus (`crates/ridlc/tests/corpus/veh-cluster` and
    `xtask/tests/freeze-fixtures/generated/veh-cluster`): the reply of
    `VehicleStatus.getFaultPage` is up to 70,649 bytes in FlatBuffers, 67,008 in
    proto3 and 67,152 in podbuf, above the 16,128 bytes that an entry may give a
    query. Streams are not built (ADR-0032), so the corpus lowers the bound of
    `FaultPage.faults` (`[0..64]`) until the reply fits 16,128 bytes in every
    enabled encoding, and the test snapshots and freeze dumps that read the
    corpus are updated.
  - The TypeScript codec fixture `crates/ridl-backend-ts/tests/fixtures/codec`
    draws RIDL-150 today only on `Sensors.note` and `Sensors.bounds`, which the
    amendment of 2026-10-28 already moves to RIDL-157.
  - `evals/corpus/mavlink`, `evals/corpus/vss` and `examples/cabin` draw no
    RIDL-150 and no TYPL-116, and are unchanged.

  TYPL-116 now warns on fewer bounds: the six `RosString` declarations of the
  ros2 corpus (bound 18,446,744,073,709,551,615) still warn, and a bound between
  4,033 and 16,256 characters, or between 16,129 and 65,280 bytes, no longer
  does. The language server publishes RIDL-151, RIDL-152 and RIDL-157 (decision
  21). The generated FlatBuffers and podbuf `verify` accept a buffer longer than
  their `MAX_SIZE`, a change of run-time behaviour with no change of a public
  signature (driftsys/ridl#1128). Decision 3 is withdrawn, so the consequences
  of 2026-10-21 that depend on it do not happen: the ros2 corpus does not gain a
  proto3 codec for its unbounded types, and no test changes to expect a capped
  `MAX_SIZE`. `ridl-rt` gains no `PAYLOAD_CEILING` and no
  `EncodeError::TooLarge`, which the Consequences of 2026-10-21 announced; it
  gains the three default ceilings (decision 16 as amended). A WebSocket payload
  larger than one message travels in blocks, a new frame version and binding
  version (decision 15, driftsys/ridl#1129); until it is built, such a fixed
  cannot be served over WebSocket, and the refusal at attach of 2026-10-28 is
  not built either.

## References

- driftsys/ridl#996, driftsys/ridl#336, driftsys/ridl#264, driftsys/ridl#1045,
  driftsys/ridl#1042, driftsys/ridl#1099, driftsys/ridl#1108,
  driftsys/ridl#1126, driftsys/ridl#1128, driftsys/ridl#1129
- [ADR-0021](ADR-0021-ridl-rt-0.1-api-and-release.md),
  [ADR-0024](ADR-0024-lint-registry-and-levels.md),
  [ADR-0027](ADR-0027-design-lints-calibrated-on-a-corpus.md),
  [ADR-0029](ADR-0029-stability-policy.md)
- `crates/ridl-ir/src/projection/flatbuffers.rs`,
  `crates/ridl-ir/src/projection/size/proto3.rs`,
  `crates/ridl-backend-rust/src/codec.rs`,
  `crates/ridl-backend-rust/src/codec_proto3.rs`,
  `crates/ridl-backend-rust/src/face.rs`, `crates/ridl-rt/src/payload.rs`
