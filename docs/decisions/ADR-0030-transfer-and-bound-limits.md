# ADR-0030 — Transfer and bound limits

## Status

Accepted — 2026-10-21. Scope: the largest payload any runtime encodes, verifies
or carries, the largest `string` and `bytes` bound an author is advised to
declare, and how both codecs, the generated interaction face and the Rust
WebSocket transport apply them. It binds the FlatBuffers backend, the proto3
backend, `ridl-rt`, `ridl-transport-ws`, the typl and ridl checkers, and every
later runtime or transport. The TypeScript package `ridl-transport-ws` checks no
message size today (`docs/design/ridl-transport-ws-ts.md`) and does not change
here; with the new Rust default, a TypeScript peer that sends a message above
16,384 bytes has its session closed with 1009, as the frame specification §11.1
says for any message above the limit.

The two lints (decisions 6 to 9) land first. The codec cap, `PAYLOAD_CEILING`,
`EncodeError::TooLarge` and the WebSocket default follow in later changes.

Written from driftsys/ridl#996 and the owner's decisions of 2026-10-21, which
replace the payload ceiling of 4 MiB + 64 KiB that the owner set on 2026-10-20.

When accepted, these records gain a dated note or are changed in place:

- ADR-0021: decision 17 and the note of 2026-10-20 say a payload with no finite
  bound is refused at generation; decision 3 of this record reverses that, and
  `ridl-rt` gains `PAYLOAD_CEILING` and `EncodeError::TooLarge`.
- ADR-0024: two new lints.
- ADR-0029 decision 2 and `docs/book/stability.md`: amended for the two lints of
  this record (decision 9).
- `docs/design/flatbuffers-codec.md`, `docs/design/proto3-codec.md`,
  `docs/design/ridl-rt.md`, `docs/design/interaction-face.md`,
  `docs/design/catalog-descriptor.md`, `docs/book/codegen-plugins.md` and
  `docs/design/ridl-transport-ws.md`.
- The frame specification §9 and §11.1 ("The sizes"), and the typl reference
  §4.4, §4.5 and §16.2.

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
- `ridl-transport-ws` limits one WebSocket message to 1 MiB by default. A
  data-plane frame has at most 36 bytes of header before the payload; an
  `attach` frame is up to 294 bytes.
- The ridl reference §12 defines streams of bounded elements for transfers with
  no total bound. They are not built (driftsys/ridl#336).
- The target transport model is shared memory with 4 KiB pages, a seqlock and a
  header of one or two cache lines.

## Decision

1. **The largest single transfer is 16,384 bytes, header included.** That is
   four 4 KiB pages. A transport reserves at most 128 bytes of it for the header
   of a message that carries a payload. The WebSocket data-plane header is at
   most 36 bytes, so it fits; the `attach` frame carries no payload and is
   limited by the frame specification alone.

2. **The payload ceiling is 16,256 bytes** (16,384 − 128). It is a platform
   rule, stated in the specifications, and it is the same for every encoding,
   runtime and transport. It counts the encoded payload only. The ceiling is
   changed only by amending this record, because a peer with a lower ceiling
   refuses what a peer with a higher one sends; ADR-0029's rule that size values
   are not frozen is otherwise unchanged.

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

5. **`BUFFER` stays.** Each use creates an array of at most 16,256 bytes on the
   stack. A call holds an argument buffer and a reply buffer; a client also
   holds its next-event buffer, and a server one interface-wide claim buffer.
   The owner judged that size acceptable. The encode API that takes a caller's
   buffer stays as the alternative for a target with a smaller stack.

6. **The largest advised bound is 4,032 characters for `string` and 16,128 bytes
   for `bytes`.** A string at 4,032 characters is at most 16,128 bytes of UTF-8.
   With the 128-byte header and about 32 bytes of FlatBuffers overhead it fits
   in 16,384 bytes: (16,384 − 128 − 32) / 4 = 4,056, rounded down to a multiple
   of 64. `bytes` takes the same byte size, 4,032 × 4.

7. **Two lints, both at level `warn`.**
   - TYPL-116 `bound-above-transfer`: a `string` bound above 4,032 or a `bytes`
     bound above 16,128. It is reported on the bound, in the typl checker.
   - RIDL-150 `payload-above-ceiling`: a payload type whose computed bound, in
     an encoding the package enables, is larger than the ceiling or has no
     finite bound. It catches structs, arrays and maps whose members are each
     within the advised bounds. It runs in `ridlc` after lowering, reads the
     encodings from the package's `[backend.<name>] encodings`, and checks
     FlatBuffers when none is named. It is reported on the interaction that
     carries the payload, so a type no interaction carries is not reported. Its
     row goes in the ridl reference's diagnostics table, and in the profile
     codes that the corpus test lists.
   - Both messages say the payload is capped at the ceiling and refused above it
     at run time. A package can raise either to `deny` in `[lints]`. The limits
     are constants beside the check, as ADR-0027 decision 4 does for design
     lints; neither is a design lint in the sense of ADR-0027, because both
     check a platform rule, not a quality measured on a corpus.

8. **The default stays `[0..256]`.** A `string` or `bytes` with no bound written
   keeps the TYPL-103 default and warning.

9. **ADR-0029 decision 2 is amended for these two lints.** A new lint starts at
   `allow` or `info` under that decision. These two start at `warn`, because a
   payload above the ceiling is refused at run time, and a lower level would
   hide that failure until then. A `warn` diagnostic does not fail `ridl check`,
   so no source that compiled before stops compiling.

10. **The WebSocket default limit becomes 16,384 bytes.** `ridl-transport-ws`
    changes its `max_message_bytes` default from 1 MiB to one transfer, which is
    at least 36 + `PAYLOAD_CEILING`. `with_max_message_bytes` still allows a
    larger value for a deployment that needs one.

11. **Data larger than one transfer is a stream.** A firmware image, a file or
    an image larger than 16 KiB is modelled as a stream of bounded elements
    (ridl reference §12). Until streams are built (driftsys/ridl#336) the
    platform carries no single value larger than one transfer.

## Where the rules are stated

- typl reference §4.4 and §4.5: the advised bounds and TYPL-116 (decisions 6 to
  8); §16.2: the TYPL-116 row. ridl reference: the RIDL-150 row.
- frame specification §11.1 "The sizes": the transfer and the ceiling (decisions
  1 and 2); §9: the `TooLarge` refusal (decision 4).
- `docs/design/flatbuffers-codec.md`, `docs/design/proto3-codec.md` and
  `docs/design/ridl-rt.md`: the cap on `MAX_SIZE`, the run-time refusals and
  `BUFFER` (decisions 3 to 5).
- `docs/design/interaction-face.md`: the mapping of `TooLarge` (decision 4).
- `docs/design/ridl-transport-ws.md`: the new default (decision 10).
- `docs/book/lints.md`: the two lint rows.

## Alternatives considered

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
  deployment that sent larger WebSocket messages must set
  `with_max_message_bytes`. Both are breaking changes, released in a minor
  version before 1.0.
- The diagnostics surface gains TYPL-116 and RIDL-150. `ridl-rt` gains
  `PAYLOAD_CEILING` and the `EncodeError::TooLarge` variant; `EncodeError` is
  `#[non_exhaustive]`, so the variant is an addition.

## References

- driftsys/ridl#996, driftsys/ridl#336, driftsys/ridl#264
- [ADR-0021](ADR-0021-ridl-rt-0.1-api-and-release.md),
  [ADR-0024](ADR-0024-lint-registry-and-levels.md),
  [ADR-0027](ADR-0027-design-lints-calibrated-on-a-corpus.md),
  [ADR-0029](ADR-0029-stability-policy.md)
- `crates/ridl-ir/src/projection/flatbuffers.rs`,
  `crates/ridl-ir/src/projection/size/proto3.rs`,
  `crates/ridl-backend-rust/src/codec.rs`,
  `crates/ridl-backend-rust/src/codec_proto3.rs`,
  `crates/ridl-backend-rust/src/face.rs`, `crates/ridl-rt/src/payload.rs`
