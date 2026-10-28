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

Amended 2026-10-28 from the owner's decisions of that day: decisions 1, 2, 7,
11, 12, 13 and 14 are changed in place, and decisions 16 and 17 are added. A
signal and an event have a payload ceiling of 3,840 bytes, one memory page of
4,096 bytes minus 256 reserved bytes; a fixed, a command and a query keep the
ceiling of 16,256 bytes. How a transport moves a payload, whole or split, is a
rule of each transport binding, not of the language, the catalog, the descriptor
or `ridl-rt`. The amendment lands as a decision first; the code follows in
driftsys/ridl#1108. It is a breaking change: a signal or an event whose payload
is larger than 3,840 bytes, and a `[limits]` entry above 3,840 for one of them,
stop compiling. It is released in the next version marked breaking, with the
transport limits of the amendment of 2026-10-24.

Built 2026-10-28: the two lints of decisions 6 to 9 (driftsys/ridl#1013), the
`[limits]` table and RIDL-151 of decisions 12 and 14 (driftsys/ridl#1063), the
limit in the IR and the catalog hash of decision 13 (driftsys/ridl#1065), and
the WebSocket limits of decisions 10 and 15 with the descriptor's limit column
(driftsys/ridl#1098). Decisions 3 and 4 are not built: `MAX_SIZE` is not capped
at the ceiling, and `ridl-rt` has no `PAYLOAD_CEILING` and no
`EncodeError::TooLarge` (driftsys/ridl#996). The amendment of 2026-10-28 is not
built: the kind ceiling of decision 16, RIDL-153 of decision 17, and what that
amendment changes in decisions 7, 12, 13 and 14 (driftsys/ridl#1108).

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
  of 2026-10-24: the kinds RIDL-150 checks, and the error RIDL-153), the ridl
  reference's diagnostics table (the RIDL-150 and RIDL-151 rows, and a new
  RIDL-153 row), the family overview §7 (the MANI-022 row, and the sentence that
  names MANI-009 as the one manifest code the manifest layer does not raise),
  the typl reference §4.4 and §4.5 (the advised bounds against the smaller
  ceiling of a signal and an event), `docs/book/lints.md` (the paragraphs on
  `payload-above-ceiling` and on `[limits]`), `docs/book/cli-reference.md` (the
  `[limits]` section: the order that resolves a limit, MANI-022, RIDL-150,
  RIDL-151 and RIDL-153), `docs/design/catalog-descriptor.md` (an absent `limit`
  means the ceiling of the member's kind), the frame specification §11.1 "The
  sizes" (the WebSocket binding carries every payload whole, in one message),
  and the freeze dumps `freeze/diagnostics.txt`, and
  `freeze/accepted-sources.txt` and `freeze/manifest.txt` where a frozen source
  or manifest is affected. `docs/design/ridl-transport-ws.md` and
  `docs/design/ridl-rt.md` do not change: the WebSocket limits stay as built,
  and `ridl-rt` gains no constant.

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
- On 2026-10-28 the owner set how that model holds a value: a signal and an
  event are each held as one cell, and a cell is one memory page of which 256
  bytes are reserved. A fixed, a command and a query are not held in a cell.

## Decision

1. **The largest single transfer is 16,384 bytes, header included.** That is
   four 4 KiB pages. A transport reserves at most 128 bytes of it for the header
   of a message that carries a payload. The WebSocket data-plane header is at
   most 36 bytes, so it fits; the `attach` frame carries no payload and is
   limited by the frame specification alone. Amended 2026-10-28: the 16,384
   bytes size the payload ceiling (decision 2); they do not say how a transport
   moves a payload. Whether a payload travels whole in one message, or is split
   and queued page by page, is a rule of each transport binding, not of the
   language, the catalog, the descriptor or `ridl-rt`. The rule that one payload
   travels in one transfer is a rule of the WebSocket binding only: its message
   limit of 36 + 16,256 = 16,292 bytes (decision 10) carries every payload
   whole, and stays as built. Another binding may split a payload. Nothing about
   pages, slots or splitting is added to the catalog, the descriptor or
   `ridl-rt`.

2. **The payload ceiling is 16,256 bytes** (16,384 − 128). It is a platform
   rule, stated in the specifications, and it is the same for every encoding,
   runtime and transport. It counts the encoded payload only. The ceiling is
   changed only by amending this record, because a peer with a lower ceiling
   refuses what a peer with a higher one sends; ADR-0029's rule that size values
   are not frozen is otherwise unchanged. Amended 2026-10-28: the payload
   ceiling is the largest payload of any interaction and the ceiling of the
   kinds `fixed`, `command` and `query`; the kinds `signal` and `event` have the
   lower ceiling of decision 16. The ceiling of a kind is a platform rule in the
   same way, changed only by amending this record. The run-time refusal of
   decision 4 and `ridl-rt`'s `PAYLOAD_CEILING` stay at 16,256 bytes.

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
   - Amended 2026-10-28: RIDL-150 checks a `fixed`, a `command` and a `query`.
     It now sizes a `fixed`, which it did not before, against the payload
     ceiling of 16,256 bytes. A signal and an event are checked against the
     ceiling of their kind by RIDL-153 (decision 17), an error, so RIDL-150 does
     not report them.
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
    platform carries no single value larger than one transfer. Amended
    2026-10-28: "one transfer" here is the ceiling of the payload's kind
    (decisions 2 and 16), not a property of a transport binding.

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
    ceiling. Amended 2026-10-28: the last step of that order is the ceiling of
    the interaction's kind (decisions 2 and 16), and with no `[limits]` table
    every interaction's limit is the ceiling of its kind. `default` applies to a
    kind up to the ceiling of that kind: a signal or an event whose limit comes
    from a `default` above 3,840 has the limit 3,840, with no error. The table
    names only the unit's own interactions, by the interface's or inline
    service's name without its package, then `.`, then the member. An inline
    service `demo.svc` is keyed `svc.member`; a key with the package
    (`demo.svc.member`, `pkg.Iface.member`) names nothing. When two interfaces
    or inline services of one unit share a short name, a key that uses the name
    is ambiguous and is reported with the same code as a key that names nothing.
    A limit of 0 is valid. The table of a remote unit that the workspace imports
    is read as well, so a consumer records the same limits as the owner and
    computes the same catalog hash; a limit of a remote unit is recorded and is
    not checked again on the consumer side. An unknown kind and a value above
    16,256 are manifest errors with a `MANI-` code (ADR-0002); a name that
    matches no interaction is a checker error with a `RIDL-` code. The codes are
    MANI-021 for an unknown kind, MANI-022 for a value above the ceiling, and
    RIDL-152 for a name that matches no interaction. The ceiling stays the upper
    bound for every interaction and is changed only by amending this record.
    Amended 2026-10-28: MANI-022 is a value above the ceiling of its kind. A
    `[limits.kind]` entry for `signal` or `event` above 3,840 is MANI-022,
    raised by the manifest layer as today; an entry for `fixed`, `command` or
    `query` keeps the range 0 to 16,256. A `[limits.interaction]` entry above
    3,840 that names a signal or an event is MANI-022 as well; the manifest
    layer does not know the kind of the member a key names, so the pass of
    `ridlc` that resolves the keys raises it, on the value, as the checker
    raises MANI-009. A `default` above 3,840 is not an error, because it applies
    to a fixed, a command and a query too.

    The limits are in the manifest, not in an attribute, because a signal, an
    event and a fixed take no attribute block (`family-general-form.md` §4.3),
    and the catalog-wide and per-kind defaults have no declaration to sit on.
    The general form and the attribute registry do not change.

13. **The resolved limits are part of the catalog** (added 2026-10-24). A pass
    of `ridlc` that runs after the checks resolves each interaction's limit and
    records it in the IR on the interaction, so the catalog hash covers it. The
    checker and lowering of `ridl-sem` leave the limit unset. An interaction
    whose limit is the ceiling because no entry applies records no limit, so a
    unit with no `[limits]` table keeps its catalog hash. Amended 2026-10-28: an
    interaction whose limit is the ceiling of its kind because no entry applies
    records no limit. The ceiling of a kind is a platform constant, not recorded
    in the IR, so no unit's catalog hash changes; only a limit that a declared
    `[limits]` entry gives is recorded, as before. An absent limit in the IR and
    in the descriptor's member table means the ceiling of the member's kind.
    `ridl diff` reports every change of a limit, a raise or a lowering, as
    breaking, so a catalog whose limits changed is not on the `compatible` chain
    of the earlier one. Two peers whose limits differ therefore have different
    hashes, neither accepts the other, and the attach is refused (frame
    specification §6.1). The catalog descriptor's member table gains a column
    with each member's limit.

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
    adds that sizing. Amended 2026-10-28: where no entry applies, a `fixed`, a
    `command` and a `query` are checked by RIDL-150 at `warn`, and a signal and
    an event by the error RIDL-153 (decision 17). Where an entry applies,
    RIDL-151 checks every kind; the entry of a signal or an event is at most
    3,840 (decision 12), so RIDL-151 also holds such an interaction within the
    ceiling of its kind.

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
    minimum.

16. **A signal and an event have a payload ceiling of 3,840 bytes** (added
    2026-10-28). Each of them is held as one cell, and a cell is one memory page
    of 4,096 bytes, of which 256 bytes are reserved: 128 bytes before the
    payload and 128 bytes after it. The 128 bytes before the payload are the
    cell header: the sequence counter and the envelope, with room for the
    envelope to grow. The 128 bytes after the payload are a trailer reserved for
    end markers: an end version that detects a torn read, a checksum, or a
    commit marker for a cell read from shared memory or by a persistence layer.
    This record reserves the bytes and does not define their contents; the
    contents are each engine's. Both reservations are 128 bytes, so that the
    payload starts on a line boundary on a target with 128-byte cache lines, and
    the header does not share an adjacent-line prefetch pair with the payload on
    x86. The page and the reservations are fixed constants, the same on every
    target, so a catalog that is valid on one target is valid on every target.
    The ceiling of the kinds `signal` and `event` is therefore 4,096 − 256 =
    3,840 bytes (30 × 128) of encoded payload. The ceiling of `fixed`, `command`
    and `query` stays 16,256 bytes (decision 2). A fixed is not held in a cell:
    it is published rarely, so the cost of copying each sample, which is the
    reason for the cell ceiling, does not apply to it, and large constant tables
    such as calibration maps must stay possible. The ceiling of a kind is a
    constant of the checker, in `ridl_core::limits` beside `PAYLOAD_CEILING`; it
    is not recorded in the IR (decision 13) and is not added to `ridl-rt` or to
    a transport.

    The ceiling is compared with the payload's encoded `MAX_SIZE` under each
    encoding the workspace manifest enables (decision 17), so the framing of a
    codec counts inside the 3,840 bytes, and the 256 reserved bytes never cover
    codec overhead. For example, a payload of one `string [0..N]` fits when 4·N
    plus the codec's framing is at most 3,840 bytes, so N is about 950; the
    exact figure for each codec is measured with the code (driftsys/ridl#1108).

    Reserving more now is the safer direction: raising a ceiling later is not a
    break for sources, because every source that compiled still compiles, and
    lowering it is.

17. **A signal or an event above the ceiling of its kind is an error** (added
    2026-10-28). When no `[limits]` entry gives a signal or an event its limit,
    and the computed bound of its payload, in an encoding the workspace enables
    (the same set as RIDL-150), is larger than 3,840 bytes or has no finite
    bound, the checker reports a new error, RIDL-153, on the interaction. As an
    error code it has no name and no level (ADR-0024 decision 1), so `[lints]`
    cannot lower it. It runs in `ridlc` after lowering, where RIDL-150 and
    RIDL-151 run, and it skips the packages that RIDL-151 skips. It is an error,
    not RIDL-150's warning, because the cell that holds the value has no room
    for a larger one. The error is on the interaction, not on the type: a type
    that a fixed, a command or a query carries keeps the ceiling of 16,256
    bytes. Where an entry gives the limit, RIDL-151 reports the same case
    instead (decision 14), so one interaction draws at most one of the two.

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
  ceiling of a kind, MANI-022 and RIDL-153 (decisions 12, 16 and 17). The frame
  specification §11.1: one payload in one message, a rule of the WebSocket
  binding (decision 1).

## Alternatives considered

- **RIDL-150 at level `error` for a signal and an event** (2026-10-28).
  Rejected: a lint has one default level for its code, and an error code is
  never a lint (ADR-0024 decision 1), so a level per kind cannot be expressed in
  the registry, and `[lints]` could lower such a level to `allow`. A separate
  error code keeps RIDL-150 a lint for a fixed, a command and a query.
- **A per-kind level on RIDL-150, or a wider RIDL-151** (2026-10-28). Rejected
  in favour of the new code RIDL-153 (decision 17). RIDL-151 keeps its meaning,
  a payload above a limit that a `[limits]` entry declares, so no existing code
  changes meaning.
- **MANI-022 for a `default` above the ceiling of a kind** (2026-10-28).
  Rejected: a `default` applies to a command and a query too, and a manifest
  that sets `default = 16256` today would stop compiling. A `default` above
  3,840 is valid and resolves to at most 3,840 for a signal and an event
  (decision 12).
- **A `RIDL-` code for a `[limits.interaction]` entry above the ceiling of its
  kind** (2026-10-28). Rejected: the error is in the manifest value, so it keeps
  the manifest code MANI-022, raised by the pass of `ridlc` that resolves the
  keys, as the checker raises MANI-009 (decision 12).
- **A fixed under the cell ceiling** (2026-10-28). Rejected: a fixed is
  published rarely, so the cost of copying each sample, which is the reason for
  the cell ceiling, does not apply to it, and large constant tables such as
  calibration maps must stay possible. A fixed keeps the payload ceiling of
  16,256 bytes (decisions 2 and 16).
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
  than 3,840 bytes or has no finite bound, which drew RIDL-150 or nothing
  before, now draws RIDL-153 and fails `ridl check`, and a `[limits]` entry
  above 3,840 for one of them now draws MANI-022. Both remove accepted sources,
  a breaking change of the `accepted-sources` surface (ADR-0029), released in
  the next version marked breaking. The code change measures which sources of
  the corpus under `evals/`, the test fixtures, the examples and the book it
  breaks, and bounds them. The diagnostics surface gains RIDL-153. A fixed whose
  payload is larger than 16,256 bytes or has no finite bound now draws the
  warning RIDL-150, which does not stop a source compiling. No catalog hash
  changes, the IR and the descriptor schema do not change, and the WebSocket
  limits stay as built.

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
