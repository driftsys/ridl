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

Built 2026-10-28: the two lints of decisions 6 to 9 (driftsys/ridl#1013), the
`[limits]` table and RIDL-151 of decisions 12 and 14 (driftsys/ridl#1063), the
limit in the IR and the catalog hash of decision 13 (driftsys/ridl#1065), and
the WebSocket limits of decisions 10 and 15 with the descriptor's limit column
(driftsys/ridl#1098). Decisions 3 and 4 are not built: `MAX_SIZE` is not capped
at the ceiling, and `ridl-rt` has no `PAYLOAD_CEILING` and no
`EncodeError::TooLarge` (driftsys/ridl#996). The amendment of 2026-10-28 is not
built (driftsys/ridl#1108): the code still uses 16,256 as the ceiling, the
WebSocket limits of decisions 10 and 15 are built at 16,292, and decisions 16 to
18 are not built.

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
- From the amendment of 2026-10-28, when its code lands: ADR-0002 (the note of
  2026-10-24: the range of a `[limits]` value and MANI-022), ADR-0024 (the note
  of 2026-10-24: the kinds RIDL-150 checks, and the error RIDL-157), the ridl
  reference's diagnostics table (the RIDL-150 and RIDL-151 rows, and a new
  RIDL-157 row), the family overview §7 (the MANI-022 row), the typl reference
  §4.4 and §4.5 (the ceiling of 16,128 bytes, and the smaller default ceiling of
  a signal and an event), `docs/book/lints.md` (the paragraphs on
  `payload-above-ceiling` and on `[limits]`, including the sentence that a
  `fixed` is not checked), `docs/book/cli-reference.md` (the `[limits]` section:
  the order that resolves a limit, the raise by a `[limits.interaction]` entry,
  MANI-022, RIDL-150, RIDL-151 and RIDL-157), `docs/design/interaction-face.md`,
  `docs/design/ridl-rt.md` and `docs/design/flatbuffers-codec.md` (no `BUFFER`
  above 16,128 bytes on the stack, and a fixed above that read through a view
  and encoded through the caller-buffer API),
  `docs/design/catalog-descriptor.md` (an absent `limit` means the default
  ceiling of the member's kind, and a raised limit of a signal or an event is
  above 3,840 bytes), `docs/design/ridl-transport-ws.md` (the message limit of
  16,164 bytes, the smallest retained cap, the arithmetic of the retained bytes,
  and the refusal at attach of a catalog whose fixed is larger than the message
  limit), the frame specification §11.1 "The sizes" and item 4 (the ceilings,
  the largest message of 16,164 bytes, that the WebSocket binding carries one
  payload per message, and the refusal at attach), and the freeze dumps
  `freeze/diagnostics.txt` and the `runtime-crates` dump, and
  `freeze/accepted-sources.txt` and `freeze/manifest.txt` where a frozen source
  or manifest is affected.

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
   catalog, the descriptor or `ridl-rt`.

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
   is also the largest `bytes` bound that decision 6 advises.

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
    deployment that knows its peers. Amended 2026-10-28: the largest data-plane
    message is 36 + 16,128 = 16,164 bytes, so the default and the largest value
    are 16,164, not 16,292.

11. **Data larger than one transfer is a stream.** A firmware image, a file or
    an image larger than 16 KiB is modelled as a stream of bounded elements
    (ridl reference §12). Until streams are built (driftsys/ridl#336) the
    platform carries no single value larger than one transfer. Amended
    2026-10-28: a value larger than 16,128 bytes, or 65,280 bytes for a fixed,
    is a stream; no `[limits]` entry raises a member above it. A separate
    record, ADR-0032, drafted in parallel, covers byte streams.

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
    kind is a platform constant, not recorded in the IR, so a unit whose entries
    are unchanged keeps its catalog hash. A limit that an entry gives is
    recorded, covered by the hash and carried in the descriptor's limit column,
    as before, including a limit that an entry raises above the default of its
    kind (decision 18). An absent limit in the IR and in the descriptor means
    the default ceiling of the member's kind.

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
    three.

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
    transport.

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
    `u16` length. A byte stream (ADR-0032, drafted in parallel) bounds one chunk
    the same way.

    The numbers come from memory pages of 4,096 bytes with 256 bytes of each
    cell reserved, once per cell: a ceiling is the cell's pages × 4,096 − 256. A
    signal and an event have one page, a command and a query four, and a fixed
    sixteen. The 256 bytes leave room for a header before the payload (a
    sequence counter and the envelope) and for a trailer after it (end markers
    such as an end version, a checksum or a commit marker), 128 bytes each, so
    that the payload starts on a line boundary on a target with 128-byte cache
    lines. This record does not bind an engine to that layout: it fixes the
    ceilings, and an engine lays out its cells as it chooses. 3,840 is 30 × 128,
    and 16,128 equals the largest `bytes` bound that decision 6 advises.

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
- From the amendment of 2026-10-28, once its code lands: the ridl reference's
  diagnostics table, `docs/book/lints.md` and `docs/book/cli-reference.md`: the
  default ceilings, the raise, MANI-022 and RIDL-157 (decisions 12 and 16 to
  18). The frame specification §11.1 and `docs/design/ridl-transport-ws.md`: the
  ceilings of decision 16, the message limit of 16,164 bytes, and one payload in
  one message as a rule of the WebSocket binding (decisions 1, 2, 10 and 15).

## Alternatives considered

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
  entries are unchanged keeps its catalog hash, and the IR and the descriptor
  schema do not change.

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
