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
`EncodeError::TooLarge` and the WebSocket default follow in later changes.

Written from driftsys/ridl#996 and the owner's decisions of 2026-10-21, which
replace the payload ceiling of 4 MiB + 64 KiB that the owner set on 2026-10-20.

Amended 2026-10-24 from driftsys/ridl#1045 and the owner's decisions of that
day: decision 10 is changed in place, and decisions 12 to 15 are added. The
limits now have one source, the compiled workspace, and neither an embedder nor
a client can raise them. The amendment lands as a decision first; the code
follows in separate issues, and the change to `with_max_message_bytes` is a
breaking change of the published `ridl-transport-ws` API, released in a version
marked breaking.

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
- From the amendment of 2026-10-24: ADR-0002 (the `[limits]` table of the
  manifest), ADR-0014 (the limit in the IR), ADR-0024 (RIDL-151), the ridl
  reference's diagnostics table, `docs/design/catalog-descriptor.md`,
  `docs/design/ridl-transport-ws.md`, the frame specification §11.1, and the
  records of the `ridl diff` categories (`docs/book/cli-reference.md`,
  `docs/book/evolving-an-interface.md`).

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
     an encoding that a `[backend.<name>]` table of the workspace manifest
     enables, is larger than the ceiling or has no finite bound. It catches
     structs, arrays and maps whose members are each within the advised bounds.
     It runs in `ridlc` after lowering, reads the encodings from the `encodings`
     of every `[backend.<name>]` table of the workspace manifest, as one set,
     and checks FlatBuffers when none is named. It is reported on the
     interaction that carries the payload, so a type no interaction carries is
     not reported. Its row goes in the ridl reference's diagnostics table, and
     in the profile codes that the corpus test lists.
   - Both messages say that the payload ceiling is 16,256 bytes and that the
     codecs do not yet enforce it; once the codecs refuse a larger value, as
     decision 4 will make them, the messages will say that. A package can raise
     either to `deny` in `[lints]`. The limits are constants in
     `ridl_core::limits`, read by both checks, not `[lints]` settings, as
     ADR-0027 decision 4 does for design lints; neither is a design lint in the
     sense of ADR-0027, because both check a platform rule, not a quality
     measured on a corpus.

8. **The default stays `[0..256]`.** A `string` or `bytes` with no bound written
   keeps the TYPL-103 default and warning.

9. **ADR-0029 decision 2 is amended for these two lints.** A new lint starts at
   `allow` or `info` under that decision. These two start at `warn`, because a
   payload above the ceiling will be refused at run time once decision 4 lands,
   and a lower level would hide that failure until then. A `warn` diagnostic
   does not fail `ridl check`, so no source that compiled before stops
   compiling.

10. **The WebSocket limit is at most one transfer.** `ridl-transport-ws` changes
    its `max_message_bytes` default from 1 MiB to 36 + 16,256 = 16,292 bytes,
    the largest data-plane message, within one transfer. Amended 2026-10-24:
    `with_max_message_bytes` may only lower the limit; a value above 16,292 is
    refused, as `with_max_retained_bytes` refuses a value outside its range
    today. The earlier text, which let a deployment set a larger value, is
    withdrawn, because the limit sizes the platform memory. A deployment that
    lowers the limit below what its catalog's limits allow (decision 15) closes
    the session of a peer that stays within them, so a lower value is for a
    deployment that knows its peers.

11. **Data larger than one transfer is a stream.** A firmware image, a file or
    an image larger than 16 KiB is modelled as a stream of bounded elements
    (ridl reference §12). Until streams are built (driftsys/ridl#336) the
    platform carries no single value larger than one transfer.

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
    query = 16256

    [limits.interaction]
    "Climate.temperature" = 64      # Interface.member, in this unit
    ```

    The limit of an interaction is, first found: its entry in
    `[limits.interaction]`, the entry of its kind (`signal`, `fixed`, `event`,
    `command` or `query`) in `[limits.kind]`, `default`, and the payload ceiling
    of decision 2. With no `[limits]` table, every interaction's limit is the
    ceiling. The table names only the unit's own interactions, by the
    interface's name as the catalog records it, then `.`, then the member. An
    unknown kind and a value above 16,256 are manifest errors with a `MANI-`
    code (ADR-0002); a name that matches no interaction is a checker error with
    a `RIDL-` code. The codes are assigned when the checks are built. The
    ceiling stays the upper bound for every interaction and is changed only by
    amending this record.

    The limits are in the manifest, not in an attribute, because a signal, an
    event and a fixed take no attribute block (`family-general-form.md` §4.3),
    and the catalog-wide and per-kind defaults have no declaration to sit on.
    The general form and the attribute registry do not change.

13. **The resolved limits are part of the catalog** (added 2026-10-24). The
    checker resolves each interaction's limit, and lowering records it in the IR
    on the interaction, so the catalog hash covers it. An interaction whose
    limit is the ceiling because no entry applies records no limit, so a unit
    with no `[limits]` table keeps its catalog hash. `ridl diff` reports every
    change of a limit, a raise or a lowering, as breaking, so a catalog whose
    limits changed is not on the `compatible` chain of the earlier one. Two
    peers whose limits differ therefore have different hashes, neither accepts
    the other, and the attach is refused (frame specification §6.1). The catalog
    descriptor's member table gains a column with each member's limit.

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
    adds that sizing.

15. **The transport derives its limits from the catalog** (added 2026-10-24). A
    server whose backend has a member table reads each member's limit from it.
    The message limit stays the `Config` value of decision 10, never less than
    the 294-byte floor of the frame specification §11.1 (`MIN_MESSAGE_LIMIT`),
    and a message above it still closes the session with 1009 (§11.1). A
    `request` within that limit whose arguments are longer than its member's
    limit is answered with the outcome `corrupt`, as for arguments that do not
    verify (§9.3). This is a check of length against the catalog, not a
    verification of the payload, and it amends §11.1 item 4 for a `request`
    only. A publish is not checked: the Rust server receives none, and a client
    receives only what its server sent. A backend with no member table, or a
    descriptor without the limit column, applies the message limit alone. The
    retained-bytes cap has the largest message limit, 16,292, as its smallest
    valid value; the clamp that raises a smaller cap to the message limit
    (`crates/ridl-transport-ws/src/server/session.rs`) is removed, and
    `RetainedTooSmall` states that minimum. `with_max_retained_bytes` keeps its
    role of sizing the memory an embedder grants a session, above that minimum.

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
- ADR-0002 (the manifest): the `[limits]` table (decision 12). ridl reference:
  the RIDL-151 row (decision 14).
- ADR-0014 and `docs/design/catalog-descriptor.md`: the limit in the IR and in
  the member table (decision 13). `docs/book/cli-reference.md` and
  `docs/book/evolving-an-interface.md`: the `ridl diff` limit change (decision
  13). `docs/design/ridl-transport-ws.md` and the frame specification §11.1: the
  message limit and the request check (decisions 10 and 15).

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
  change, and the diagnostics surface gains RIDL-151 and the manifest errors of
  decision 12. `with_max_message_bytes` can no longer raise the limit and the
  retained-cap clamp is removed; both change the published `ridl-transport-ws`
  API and are released in a version marked breaking. The new IR field and the
  new descriptor column are additions to the frozen `ir` surface and the
  descriptor schema under ADR-0029; the freeze dumps are updated when the code
  lands.

## References

- driftsys/ridl#996, driftsys/ridl#336, driftsys/ridl#264, driftsys/ridl#1045,
  driftsys/ridl#1042
- [ADR-0021](ADR-0021-ridl-rt-0.1-api-and-release.md),
  [ADR-0024](ADR-0024-lint-registry-and-levels.md),
  [ADR-0027](ADR-0027-design-lints-calibrated-on-a-corpus.md),
  [ADR-0029](ADR-0029-stability-policy.md)
- `crates/ridl-ir/src/projection/flatbuffers.rs`,
  `crates/ridl-ir/src/projection/size/proto3.rs`,
  `crates/ridl-backend-rust/src/codec.rs`,
  `crates/ridl-backend-rust/src/codec_proto3.rs`,
  `crates/ridl-backend-rust/src/face.rs`, `crates/ridl-rt/src/payload.rs`
