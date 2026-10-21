# ADR-0030 — Transfer and bound limits

## Status

Proposed — 2026-10-21. Scope: the largest payload any runtime encodes, verifies
or carries, the largest `string` and `bytes` bound an author is advised to
declare, and how both codecs and the WebSocket transport apply them. It binds
the FlatBuffers backend, the proto3 backend, `ridl-rt`, `ridl-transport-ws`, the
typl checker and every later runtime or transport.

Written from driftsys/ridl#996 and the owner's decisions of 2026-10-21, which
replace the payload ceiling of 4 MiB + 64 KiB that the owner set on 2026-10-20.

When accepted, these records gain a dated note: ADR-0019 (decision 8, the
`max_size` bound), ADR-0017 (the proto3 size state), ADR-0024 (one new lint),
and the frame specification §11.1 ("The sizes").

## Context

- Each generated payload type has `MAX_SIZE`, the largest encoded size its type
  allows, and a zeroed `BUFFER` of that size. The FlatBuffers bound is
  `ridl_ir::projection::flatbuffers::max_size`; the proto3 bound is the
  declaration's proto3 size state.
- A declaration whose size has no finite bound, or whose bound does not fit a
  `u32`, has no codec today. Both backends refuse it with a generate error and
  no diagnostic code. The ros2 corpus declares
  `RosString : string [0..18446744073709551615]`, so neither codec generates it.
- A typl `string [N]` bound counts Unicode scalar values. Every encoding carries
  a string as UTF-8, so its byte capacity is 4·N. A `bytes [N]` bound counts
  bytes. A `string` or `bytes` with no bound written gets `[0..256]` and the
  warning TYPL-103.
- `ridl-transport-ws` limits one WebSocket message to 1 MiB by default. A frame
  is at most 36 bytes of header plus the payload.
- The ridl reference §12 defines streams of bounded elements for transfers with
  no total bound. They are not built (driftsys/ridl#943).
- The target transport model is shared memory with 4 KiB pages, a seqlock and a
  header of one or two cache lines.

## Decision

1. **The largest single transfer is 16,384 bytes, header included.** That is
   four 4 KiB pages. A transport reserves at most 128 bytes of it for its own
   header. The WebSocket frame header is 36 bytes, so it fits.

2. **The payload ceiling is 16,256 bytes** (16,384 − 128). It is a platform
   rule, stated in the specifications, and it is the same for every encoding,
   runtime and transport. It counts the encoded payload only.

3. **`MAX_SIZE` is capped at the ceiling.** A payload's `MAX_SIZE` is the
   smaller of the size computed from its type's bounds and the ceiling. A type
   whose computed size is larger, has no finite bound, or overflows a `u32` now
   generates in both codecs, with `MAX_SIZE` = 16,256. The two generate errors
   for "no finite bound" and "does not fit a u32" are removed. The errors for a
   declaration a codec cannot represent at all (an untyped position, a stream)
   stay.

4. **A value larger than the ceiling is refused at run time, the same way in
   both codecs.**
   - `encode` returns `EncodeError::Capacity { needed, available }` with
     `available` = `MAX_SIZE`. The proto3 encoder computes the exact length
     before it writes; the FlatBuffers builder stops when the buffer is full.
   - `verify` returns `VerifyError::Structure(Malformed::TooLarge)` when the
     input is longer than the ceiling. FlatBuffers already refuses an input
     longer than `MAX_SIZE`; proto3 gains the ceiling check only, so a payload
     from a newer writer whose type has larger bounds still verifies when it is
     within the ceiling.
   - `ridl-rt` exports the ceiling as `PAYLOAD_CEILING: usize = 16_256`, so a
     runtime and a transport read one constant.

5. **The per-type `BUFFER` stays.** With `MAX_SIZE` at most 16,256 bytes, a
   static zeroed buffer per type is acceptable on the embedded targets the
   platform supports. The caller-supplied buffer of the encode API stays as the
   alternative.

6. **The largest advised bound is 4,032 characters for `string` and 16,128 bytes
   for `bytes`.** A string at 4,032 characters is at most 16,128 bytes of UTF-8.
   With the 128-byte header and about 32 bytes of FlatBuffers overhead it fits
   in 16,384 bytes: (16,384 − 128 − 32) / 4 = 4,056, rounded down to a multiple
   of 64. `bytes` takes the same byte size, 4,032 × 4.

7. **A bound above that is a warning, not an error.** A new lint, TYPL-116
   `bound-above-transfer`, default level `warn`, is reported on a `string` bound
   above 4,032 or a `bytes` bound above 16,128. Its message says that the
   payload is capped at the ceiling. A package can raise it to `deny` in
   `[lints]`. The limits are constants beside the check, as ADR-0027 decision 1
   requires for a design threshold, not `[lints]` settings.

8. **The default stays `[0..256]`.** A `string` or `bytes` with no bound written
   keeps the TYPL-103 default and warning.

9. **The WebSocket default limit becomes 16,384 bytes.** `ridl-transport-ws`
   changes its `max_message_bytes` default from 1 MiB to one transfer, which is
   above 36 + 16,256. `with_max_message_bytes` still allows a larger value for a
   deployment that needs one.

10. **Data larger than one transfer is a stream.** A firmware image, a file or
    an image larger than 16 KiB is modelled as a stream of bounded elements
    (ridl reference §12). Until streams are built (driftsys/ridl#943) the
    platform carries no single value larger than one transfer.

## Where the rules are stated

- typl reference §4.4 and §4.5: the advised bounds and the lint (decisions 6 to
  8); §16.2: the TYPL-116 row.
- frame specification §11.1 "The sizes": the transfer and the ceiling (decisions
  1 and 2); §9: the `TooLarge` refusal (decision 4).
- `docs/design/flatbuffers-codec.md` and `docs/design/proto3-codec.md`: the cap
  on `MAX_SIZE` and the run-time refusals (decisions 3 to 5).
- `docs/design/ridl-transport-ws.md`: the new default (decision 9).
- `docs/book/lints.md`: the lint row.

## Alternatives considered

- **A ceiling of 4 MiB + 64 KiB**, sized for an uncompressed 1024×1024 RGBA PNG.
  Rejected on 2026-10-21: one transfer of 4 MiB is too large for a seqlock
  reader, which copies the payload and retries when a writer overwrites it, and
  a 4 MiB static buffer per type is too large for an embedded target.
- **An error for a bound above the advised limit.** Deferred: it breaks the ros2
  corpus and two test fixtures, and is a break of the `accepted-sources`
  surface. The lint can be raised to `deny` by a package now and by default
  later.
- **Refusing a type with no finite bound, as today.** Rejected: imported schemas
  such as ROS 2 use unbounded strings, and the ceiling already gives a receiver
  a limit it can check.
- **No `BUFFER` for a large bound, and a caller-supplied buffer only.** Not
  needed: the ceiling keeps every `BUFFER` at or below 16,256 bytes.

## Consequences

- The ros2 corpus generates with both codecs. The `wire` dump gains its `.fbs`
  lines, which is an addition. The known FlatBuffers rejection of ros2 for a
  nested vector is a separate cause and is checked again.
- The lint warns on six `RosString` declarations in `evals/corpus/ros2/` and
  their frozen copies, and on two fixtures in
  `crates/ridl-backend-rust/tests/fixtures/`. The book example
  `FwBlock : bytes [1..65536]` and the same bound in the typl reference warn,
  and are changed to a bound within the limit or to a stream element within it.
- A value larger than 16,256 bytes that encoded before is now refused. A
  deployment that sent larger WebSocket messages must set
  `with_max_message_bytes`. Both are breaking changes, released in a minor
  version before 1.0.
- The diagnostics surface gains TYPL-116, which is an addition under ADR-0029.

## References

- driftsys/ridl#996, driftsys/ridl#943, driftsys/ridl#264
- [ADR-0017](ADR-0017-proto3-projection-rules.md),
  [ADR-0019](ADR-0019-flatbuffers-projection-rules.md),
  [ADR-0024](ADR-0024-lint-registry-and-levels.md),
  [ADR-0027](ADR-0027-design-lints-calibrated-on-a-corpus.md),
  [ADR-0029](ADR-0029-stability-policy.md)
- `crates/ridl-ir/src/projection/flatbuffers.rs`,
  `crates/ridl-ir/src/projection/size/proto3.rs`,
  `crates/ridl-backend-rust/src/codec.rs`,
  `crates/ridl-backend-rust/src/codec_proto3.rs`,
  `crates/ridl-rt/src/payload.rs`
