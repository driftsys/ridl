# ADR-0031 — The podbuf projection: the byte layout, verify, and evolution

## Status

Accepted — 2026-10-27. Scope: the rules that fix the bytes of the third payload
encoding, `podbuf` — the target rule, the preamble, the first-fit placement law
and the optional flags, the descriptors and the stride, the value rules, the
totality table, what `verify` checks and what it does not, the encoder's duty,
the evolution rules, strict and open enums, and the names and diagnostic codes
of what the backend generates. It binds the podbuf layout in `ridl-ir`, the Rust
codec in `ridl-backend-rust`, the C header backend `ridl-backend-podbuf`, the
`podbuf` module of `ridl-rt`, `ridl-diff`, the podbuf checks of `ridlc` and
`ridl check`, and every codegen plugin that reads the podbuf layout from the
codegen `Model`.

**Amended 2026-10-29 — decision 10, the inline-append rule, from
driftsys/ridl#1100.** The rule is narrowed to an append that grows the head of a
struct a head of the old side holds inline; decision 10 states it and carries
the dated note, decision 13 points to it, and the negative consequence on a type
held inline is reworded.

This record is the projection record that
[ADR-0020](ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
decision 4 asks for, in the shape of
[ADR-0017](ADR-0017-proto3-projection-rules.md) and
[ADR-0019](ADR-0019-flatbuffers-projection-rules.md). ADR-0020 decision 1 fixes
the encoding's place in the matrix; this record fixes what the bytes are.

It restates the design `docs/archive/2026-10-24-podbuf-design.md` as decisions,
corrected to what the code does. Where the design and the code differed, the
code won and the difference is listed under "Where this record departs from the
design". Each decision keeps the mark the design gave it: **(Sebastien)** when
the owner took it, **(author's call)** when the author of the design settled it,
and **(builder's call)** when the implementation settled a point the design left
to the builder (design §16).

The design's evidence (a spike on five `veh-cluster` types, measured in
instructions on a Cortex-R52 model) stays in the archived design, §12. It is the
reason for the encoding's shape, not a rule of it, and the built codec has not
been measured again; the figures below are the design's.

When accepted, these records are changed:

- ADR-0020: decision 4 and open item 1 point at this record, and open item 1 is
  closed.
- `docs/design/podbuf-codec.md`: new. The as-built record of the codec.
- `docs/book/lints.md` and the ridl reference's diagnostics table: RIDL-153 to
  RIDL-156 (written with the code).
- `docs/book/stability.md`: the `podbuf` feature and the podbuf lines of the
  `wire` dump.

Amended 2026-10-30 by [ADR-0030](ADR-0030-transfer-and-bound-limits.md)
decisions 17 and 19, from driftsys/ridl#1099. Decision 8 is changed in place:
`Payload<Podbuf>::verify` no longer refuses a buffer longer than its own
`MAX_SIZE`; the transport refuses a payload longer than its interaction's limit,
once. Open item 2 is closed with that answer. Decision 6 is changed in place: a
payload above the default ceiling of its kind is reported by the error RIDL-157,
which replaces the retired lint RIDL-150. Not built: the generated `verify`
still checks the length (driftsys/ridl#1128), and RIDL-150 is still raised
(driftsys/ridl#1108).

## Context

ADR-0020 decision 1 adds a third payload encoding for a consumer that reads a
field at a fixed offset and links no reader library. The case that drives it is
messages shared in memory inside one SoC that has AArch64 application cores and
a Cortex-R52-class safety core. A producer may be untrusted. A gate copies each
message into a trust zone and calls a generated `verify` before any consumer
sees it.

Goal **(Sebastien)**: on that path, verify and decode cost close to zero, encode
cost is reasonable, and the encoding is more efficient than proto3 and, as
measured, than FlatBuffers. Floating-point cost is out of scope for the goal.

Goals of the encoding:

- One deterministic byte layout, computed from the typl declaration alone.
- A consumer reads a field in place, with no decode step and no library; a C
  reader needs only the generated header.
- A generated `verify` whose cost grows with the data present, never with the
  declared capacity.
- Total over typl: every typl construct has a rule, and a type that exceeds
  podbuf's limits is a diagnostic that names it (ADR-0016 decision 6, ADR-0017
  decision 4).
- Compatible evolution within one type: appended fields, new values of an open
  enum or enum set, deprecated fields.

Non-goals **(Sebastien)**: a wire or stream format (anything that leaves the SoC
goes through a SOME/IP or proto3 connector); big-endian targets; a fused
copy-and-verify function (the gate owns its copy; ridl generates only `verify`);
quantized floats as scaled integers; the AUTOSAR RTE adapter and the ARXML
export (they come from a codegen plugin, not from podbuf); tracking type
versions (a new version is a convention, decision 11); cache-line alignment and
sizing of message buffers (the gate's and the transport's concern).

Anti-goals: a consumer that has to check anything after `verify` has passed; a
layout that differs between the AArch64 cores and the R52 core; a `verify` whose
cost depends on unused capacity; a layout rule that moves an existing field when
a field is appended.

## Decision

1. **The name and the trust model.** The encoding is named `podbuf`, for "plain
   old data buffer" **(Sebastien)**: the head is a plain-old-data `#[repr(C)]`
   struct and the buffer is plain bytes that are valid to read in place.
   ADR-0020 named the encoding `repr(C)`; its 2026-10-25 amendment renames the
   encoding, the cargo feature of `ridl-rt` and the `encodings` option to
   `podbuf`.
   - A producer may be untrusted **(Sebastien)**. The gate copies each message
     into a trust zone, then calls the generated `verify`; the copy closes the
     time-of-check to time-of-use gap. After `verify` succeeds, the consumer
     reads the buffer without further checks and the buffer is read-only.
   - A podbuf buffer starts on an 8-byte boundary, and `verify` checks it
     **(Sebastien)**. Cache-line alignment is the gate's and the transport's
     concern.
   - **Deployment rule (author's call).** The gate's `verify` for a type is
     generated from a version of that type at least as new as the version of
     every consumer behind the gate. A gate built from an older version ignores
     the bytes of fields it does not know, so it cannot check a value that a
     newer consumer then reads.

2. **The target rule.**
   - podbuf supports any little-endian target whose C ABI aligns each
     fixed-width type to at most its size **(Sebastien)**: AArch64, AArch32 (the
     R52 core), x86-64 and i686.
   - The generated Rust and C refuse to compile on a big-endian target (author's
     call): `ridl_rt::podbuf` carries `compile_error!` under
     `not(target_endian = "little")`, and the header carries `#error` under
     `__BYTE_ORDER__ != __ORDER_LITTLE_ENDIAN__`.
   - A head holds fixed-width integers (`u8` to `u64`, `i8` to `i64`), `f32`,
     `f64`, fixed arrays of these, and nested heads. Never a pointer, `usize`,
     `size_t`, `long`, a C `enum`, a C bitfield or a 128-bit type (author's
     call). Every gap is an explicit pad field, so i686, which aligns `u64` to 4
     inside a struct, reproduces the same offsets.
   - Every head is declared with alignment 8: `_Alignas(8)` on its first member
     in C and `#[repr(C, align(8))]` in Rust (author's call). Without it i686
     would give a head that holds a `u64` an alignment of 4, which changes where
     a head can sit inside another head or in the tail.
   - The generator emits `_Static_assert` checks on `sizeof` and `offsetof` in
     the C header and `const` assertions on `size_of` and `offset_of!` in Rust,
     for every head and field (author's call). A toolchain that disagrees fails
     the build. `core::mem::offset_of!` needs Rust 1.77, below `ridl-rt`'s 1.88.

3. **The buffer: preamble, head and tail.** A buffer is `[preamble][head][tail]`
   **(Sebastien)**.
   - The **preamble** is 8 bytes, `{ head_size: u32, reserved: u32 }`, both
     little-endian **(Sebastien, 2026-10-24)**. `head_size` is the size in bytes
     of the head the producer wrote, a multiple of 8. The head starts at offset
     8 and the first byte of the tail is at `8 + head_size`. The encoder writes
     `reserved` as zero and a reader ignores it (author's call), so a later use
     of the word needs no change to older readers. The preamble carries no type
     id and no version; the gate or the transport knows the type.
   - The **head** is one `#[repr(C, align(8))]` struct per type. It holds every
     fixed-size field: scalars, enums, enum sets, exact-length arrays (`[T; N]`
     and `bytes [N..N]`), and nested structs, tuples and unions, inline.
   - Every **variable-length** field — a bounded `string`, `bytes` or array, a
     map, and the active arm of a union — is an 8-byte **descriptor** in the
     head, at alignment 4 (author's call). `off: u32` is relative to the start
     of the buffer, which is the start of the preamble. A `string`, `bytes` or
     union descriptor is `{ off: u32, len: u32 }`, with `len` counting bytes for
     a `string` or `bytes` and the bytes of the arm head for a union. An array
     or map descriptor is `{ off: u32, len: u16, elem_size: u16 }` **(Sebastien,
     2026-10-24)**, with `len` counting elements or entries. A descriptor whose
     `len` is 0 is all zero bytes, and so is the descriptor of an absent
     optional field (author's call): it points at no tail data, and an empty
     string has no NUL in the tail.
   - The **tail** follows the head and holds only the data present: string and
     bytes contents, array elements, map entries, union arms. Data in the tail
     is placed at the alignment of its element type (author's call). Only the
     buffer start carries a preamble; an element, a union arm or a map entry in
     the tail is a head without one.
   - **Stride.** `elem_size` is the size in bytes of one element or map entry as
     the producer laid it out. A reader steps from one element to the next by
     `elem_size` and reads only the prefix of each element that it knows; a
     field that lies past the producer's `elem_size` reads as 0, by the rule of
     an appended field (decision 10). The stride costs no bytes: it uses the
     upper half of the descriptor's length word. The limits are 65,535 elements
     per array or map and 65,535 bytes per element.
   - **The `elem_size` rule, as built.** For an element or entry that is an
     inline head — a struct, a tuple or a union — `elem_size` is a multiple of 8
     that the producer chooses: the head size it wrote, and larger than a
     reader's own head size when the producer's type is newer. `verify` checks
     only that it is a multiple of 8, not that it equals any head size. For
     every other element (a scalar, an enum, an enum set, a descriptor, an
     exact-length array) `elem_size` is the element's exact size, because such
     an element cannot grow compatibly, and `verify` checks that equality. The
     design said a tuple's `elem_size` equals its element size; a tuple is laid
     out as an inline head, as a struct is, so its size is rounded to 8 and the
     first branch applies.
   - **`elem_size` 0 on an inline head with `len` above 0 is accepted.** 0 is a
     multiple of 8, and the data is `len·0` = 0 bytes, so the descriptor passes
     the bounds check when `off` is 8-aligned, at least `8 + head_size` and
     within the buffer. Every element then starts at `off` and has size 0, so a
     reader reads every field of every element as 0, in `verify`, the view and
     `decode` alike. `verify` checks those zeros like any other value: an
     element whose fields do not all accept 0 is refused with the `Contract`
     rule of the failing field, and `min ≤ len ≤ max` still applies. A map with
     such a stride and `len` above 1 is refused with `Malformed::KeyOrder`,
     because every key reads as 0 and keys must strictly increase. The layout of
     such a buffer is not one the encoder writes; the rule is stated so that a
     reader's behaviour is not left to the code.
   - A union arm has no separate stride (author's call): the arm descriptor's
     `len` is the arm head size the producer used, and a field past `len` reads
     as 0. An arm whose type has no field has head size 0, so the encoder writes
     an all-zero descriptor for it. `verify` checks that descriptor as it checks
     every arm descriptor — in bounds, 8-aligned, `len` a multiple of 8 — and
     reads no content; so a newer producer whose arm type gained a field
     (decision 10) is accepted, and the all-zero descriptor is still accepted.
   - A considered threshold rule (a bounded field stored inline when its
     capacity is at most 256 bytes) was dropped **(Sebastien)**; see
     "Alternatives considered".

4. **The first-fit law** **(Sebastien)**.
   1. Take the fields in declaration order.
   2. Place each at the lowest offset where it fits at its natural alignment (1,
      2, 4 or 8) without overlapping a field already placed. **Optional flags
      (author's call):** the flags are `u8` fields, one per eight optional
      fields. When the placement reaches the optional field number `8k + 1`
      (counting optional fields from 1 in declaration order), it first places
      flags field `k` as a `u8` by this same rule, then places the optional
      field. Bit `i`, from the least significant bit, of flags field `k` is set
      when optional field number `8k + i + 1` is present.
   3. Round the head size up to a multiple of 8.
   4. Every unused byte is an explicit pad field `_pad<offset>`, whose value is
      zero.

   The offset of a field depends only on the fields declared before it, so
   appending a field never moves an existing one, and a later small field fills
   a gap that an earlier alignment left. The generated struct lists the fields
   sorted by offset with the pad fields written out, so a C compiler produces
   the same layout. The law is computed once, in `ridl_ir::projection::podbuf`,
   and every emitter and check reads it from there.

   **Tail order (builder's call).** The encoder writes tail data in the order of
   the head's placements, which are sorted by offset, depth first: a string
   inside an array element follows that element's array block. The design said
   "field order"; the order of the placements is what `max_size` and the encoder
   both iterate, so it is the rule. `verify` does not depend on the order.

   Worked examples, from `veh-cluster` (the head size is a multiple of 8; the
   buffer without a tail adds the 8-byte preamble):

   | Type            | Fields in declaration order                      | Placement (offsets in the head)                                                 | Head | Buffer without tail |
   | --------------- | ------------------------------------------------ | ------------------------------------------------------------------------------- | ---- | ------------------- |
   | `DoorPayload`   | `sensor_id: u8`, `is_open: bool`                 | 0, 1; pad 2..8                                                                  | 8    | 16                  |
   | `ClimateReport` | `cabin`, `setpoint` (16-bit raw), `fan: u8`      | 0, 2, 4; pad 5..8                                                               | 8    | 16                  |
   | `DiagFilter`    | `severity: u8`, `category?: string`              | 0; flag byte 1; descriptor 4..12; pad 2..4, 12..16                              | 16   | 24                  |
   | `FaultEvent`    | `code: u16`, `message: string`, `timestamp: i64` | 0; descriptor 4..12; `timestamp` cannot use 8..16, so 16..24; pad 2..4, 12..16  | 24   | 32                  |
   | `FaultPage`     | `faults: [FaultEvent; 0..64]`                    | descriptor 0..8 with `elem_size` 24; the elements are 24-byte heads in the tail | 8    | 16                  |

5. **The value rules.**

   | typl construct                 | podbuf representation                                                                                 | Decided by    |
   | ------------------------------ | ----------------------------------------------------------------------------------------------------- | ------------- |
   | integer                        | the wire width typl infers from the range (typl §4.2)                                                 | Sebastien     |
   | float                          | `f32` or `f64` as typl infers (typl §4.3)                                                             | author's call |
   | boolean                        | `u8`, 0 or 1                                                                                          | author's call |
   | enum                           | the narrowest integer that holds the declared values; strict (decision 12)                            | Sebastien     |
   | enum set                       | the integer width typl infers (typl §9.3); strict (decision 12)                                       | Sebastien     |
   | optional field `?`             | one bit in a `u8` flags field placed by the first-fit law; an absent field's bytes are zero           | author's call |
   | tuple                          | an inline head, laid out by the same law and rounded to 8                                             | author's call |
   | exact-length array `[T; N]`    | inline `T[N]`, no length                                                                              | Sebastien     |
   | bounded `string`               | descriptor; the tail holds the UTF-8 bytes followed by one NUL that `len` does not count              | Sebastien     |
   | bounded `bytes`, bounded array | descriptor; the tail holds the bytes, or the elements at stride `elem_size`, contiguous               | Sebastien     |
   | union                          | a narrow integer tag (the arm's ordinal) in the head, plus a descriptor to the active arm in the tail | Sebastien     |
   | map `[K : V; min..max]`        | descriptor to an array of `{ key, value }` entries sorted by key                                      | Sebastien     |

   - typl's `string [N]` bounds scalar values, not bytes (typl §4.4). The tail
     stores only the bytes present, so no worst-case byte capacity is reserved.
   - A string that contains U+0000 cannot be encoded, because the string carries
     a NUL terminator and `verify` rejects an interior NUL (author's call). The
     encoder refuses it with `EncodeError::Contract` and the rule `Pattern`.
     This is a known limit of podbuf: conformance round trips between podbuf and
     the other encodings exclude strings that contain U+0000.
   - A union descriptor's `len` is the head size of the active arm as the
     producer laid it out, a multiple of 8, or 0 for an arm with no fields.
   - An enum's integer is signed when any declared value is negative and
     unsigned otherwise; its width is the narrowest that holds every declared
     value.
   - A union's tag is unsigned; its width is the narrowest that holds the
     largest arm ordinal, and a union with no arm has a `u8` tag.
   - Map key order is bytewise for strings and bytes, and numeric for integers
     and floats. `+0.0` and `-0.0` are equal, so both present is a duplicate; a
     NaN key is rejected **(Sebastien)**. The encoder sorts the entries and
     refuses a duplicate key (`Unique`) and a NaN key (`Range`) with
     `EncodeError::Contract`.
   - Every typl string, bytes and collection carries a bound, explicit or the
     `[0..256]` default, so podbuf has no unbounded case.

6. **Totality and the build-time errors** (author's call). Every typl construct
   has a podbuf rule:

   | typl construct                  | Rule                                                            |
   | ------------------------------- | --------------------------------------------------------------- |
   | boolean, integer, float         | a fixed-width scalar in the head (decision 5)                   |
   | enum, enum set                  | an integer in the head, strict (decision 12)                    |
   | string, bytes                   | a descriptor and tail bytes (decisions 3 and 5)                 |
   | exact-length array `[T; N]`     | inline `T[N]` in the head                                       |
   | bounded array                   | a descriptor with a stride, and the elements (decision 3)       |
   | map                             | a descriptor with a stride, and the sorted entries (decision 5) |
   | union                           | a tag in the head and a descriptor to the active arm head       |
   | struct, tuple                   | a head laid out by the first-fit law, inline when nested        |
   | optional `?`                    | a flag bit, and zeroed bytes when absent (decision 4)           |
   | units, constraints, init values | no bytes of their own; constraints are checked by `verify`      |
   | recursive shapes                | impossible: typl rejects them (typl §7.3)                       |

   A named scalar, an enum or an enum set used as a payload root is carried in a
   one-field head, the head a union arm of that type gets (the model calls it a
   box). The three build-time errors name the type, are raised only when a
   `[backend.<name>]` table of the workspace manifest lists `podbuf`, and are
   error codes with no lint name and no level (ADR-0024 decision 1):

   - RIDL-153 `type too large for podbuf`: a struct, tuple or union whose
     largest encoding is above `u32::MAX` bytes. A tuple is a named induced
     tuple, the argument struct of a call with several parameters or a tuple
     reply, and is reported on its interaction; a struct or union is reported on
     its declaration.
   - RIDL-154 `array or map exceeds the podbuf descriptor limits`: an array or
     map past 65,535 elements or 65,535 bytes per element; reported on the
     field.
   - RIDL-155 `deprecated field has no valid podbuf value`: a `@deprecated`
     non-optional field whose init value is neither declared nor derivable and
     whose type excludes 0.

   The ridl reference's diagnostics table
   ([`../specification/ridl-language-reference.md`](../specification/ridl-language-reference.md))
   owns the full text of each row.

   A type that has no bound only because an array or map inside it passes a
   descriptor limit draws RIDL-154 on the field and not RIDL-153. The codes
   RIDL-151 and RIDL-152 were allocated by ADR-0030 after the plan was written,
   so the podbuf codes start at 153 (builder's call). A payload above the
   payload ceiling of ADR-0030 is reported by RIDL-150, not by podbuf. Amended
   2026-10-30: by RIDL-157, which replaces the retired RIDL-150 for every kind
   (ADR-0030 decision 17).

   **The size bound (builder's call).** `MAX_SIZE` of a `Payload<Podbuf>` is the
   preamble, the head and the largest tail, with every optional field present
   and every bound at its maximum, and it includes the worst-case alignment gap
   before each aligned tail block. The encoder zero-fills those gaps, so the
   bound has to include them or the buffer would be short. The bound is computed
   once by `ridl_ir::projection::podbuf::max_size` and reaches the codegen model
   as `Declaration.podbuf`. ADR-0030 decision 3 asks each backend to cap the
   generated `MAX_SIZE` at the payload ceiling; the podbuf codec, like the other
   two, emits the computed bound uncapped (driftsys/ridl#996), and RIDL-150
   reports a payload above the ceiling at check time (RIDL-157 once ADR-0030's
   amendment of 2026-10-30 is built).

7. **Generated names (builder's call).** In Rust, per type `T`: `TPodbufHead`
   (the `#[repr(C, align(8))]` struct, for a struct, union or named tuple),
   `TPodbufView<'a>` (the view that `verify` returns, with one accessor per
   field, and one accessor `value` for a boxed root), and `TPodbufArm<'a>` (for
   a union, one variant per arm). The internal functions are
   `__ridl_podbuf_encode`, `__ridl_podbuf_encode_at`, `__ridl_podbuf_verify`,
   `__ridl_podbuf_verify_at` and `__ridl_podbuf_decode`. In C: `ridl_podbuf_*`
   for the preamble, descriptor and load helpers, `<pkg>_<type>_head` for a
   head, `<pkg>_<type>_<field>` and `<pkg>_<type>_<field>_at` for accessors, and
   `<PKG>_<TYPE>_HEAD_DEFINED` for the guard macro. Every name is spelled with
   the pinned transform of `ridl_ir::name` (ADR-0016). Each generated Rust type
   name, `TPodbufHead`, `TPodbufView` and `TPodbufArm`, has a claim table entry
   (`ridl-backend-rust/src/claims.rs`), so a ridl name cannot collide with it;
   the `__ridl_podbuf_*` functions need none, because no typl name spells a
   leading `__`, and a view accessor is the field's own name on the view type.

8. **Verify.** `verify(buf) -> Result` is generated per type, in Rust only,
   because the gate is Rust (author's call). It checks, over the data present:
   1. The buffer start is 8-aligned and the length is at least 8. In the
      preamble, `head_size` is a multiple of 8 and `8 + head_size` is at most
      the buffer length (checked arithmetic). `reserved` is not checked.
   2. Each boolean is 0 or 1; each value of a strict enum is declared; each
      strict enum set has no undeclared bit; each flags integer is checked only
      at the bits of the optional fields the reader knows; the bytes of an
      absent optional are zero. A byte of the reader's head past `head_size`, of
      an element past the producer's `elem_size`, or of an arm past its
      descriptor `len`, reads as 0 and is checked with that value like any
      other.
   3. The typl constraints of each value: integer ranges, the string length
      bound counted in Unicode scalar values (typl §4.4), and the other
      generated typl checks, the same code the other encodings call.
   4. For each descriptor with `len` 0: every other descriptor byte is 0. For
      each other descriptor: `off` is aligned for its data, at least
      `8 + head_size`, and the data is within the buffer, by checked arithmetic.
      The data is `len` bytes plus 1 for a string's NUL, `len` bytes for `bytes`
      and a union arm, and `len·elem_size` bytes for an array or map. For
      `bytes`, an array and a map, `min ≤ len ≤ max`. For a string,
      `len ≤ 4·max`, the largest UTF-8 byte length of `max` scalar values; this
      bounds the work, and rule 3 checks the scalar-value count.
   5. For each non-empty array or map, `elem_size` is a multiple of 8 when the
      element is an inline head, and the element's exact size otherwise
      (decision 3).
   6. For each non-empty string, the byte after the data is NUL, no NUL is
      inside the data, and the data is valid UTF-8.
   7. For each union, the tag names a declared arm and `len` is a multiple of 8;
      then the arm is verified. An arm whose type has no field has no content to
      verify; its descriptor is checked by rule 4 like any other.
   8. For each map, the keys are strictly increasing in the order of decision 5.
   9. Each array element, union arm and map entry is verified with the same
      rules.

   **How the refusals are reported (builder's call, rulings of the
   implementation).** Structure faults use `Malformed`: `Unaligned`, `Preamble`,
   `OutOfBounds`, `Stride`, `Terminator`, `Utf8`, `Union` and `KeyOrder`. A
   value fault uses the existing `Contract` outcomes: a boolean above 1 is
   refused as `Contract` with the rule `Range`; an absent optional whose bytes
   are not zero is refused as `Contract` with the rule `Variant`; a constraint
   is refused with the rule of that constraint. A NaN map key is refused with
   `Range`.

   **Verify is a call tree (builder's call).** The generated `verify` is not
   recursive and has no worklist. Each head type has its own
   `__ridl_podbuf_verify_at(buf, at, size, tail)` function, and a nested head,
   an array element, a union arm or a map entry is checked by the function of
   its type. typl has no recursive shape (typl §7.3), so the call depth is
   bounded by the type and the generated `verify` allocates nothing. The design
   said the verifier "can be non-recursive"; the call tree has the same bound.

   **`Payload<Podbuf>::verify` also refuses a buffer longer than `MAX_SIZE`**,
   with `Malformed::TooLarge`, before it reads the preamble (builder's call).
   See open item 2. Amended 2026-10-30: this check is withdrawn
   ([ADR-0030](ADR-0030-transfer-and-bound-limits.md) decision 19).
   `Payload<Podbuf>::verify` refuses no buffer for its length; it reads the
   preamble, the head and the tail that the descriptors reference, and ignores
   the bytes past them. The transport refuses a payload longer than its
   interaction's limit from the catalog before `verify` runs, and an embedder
   whose gate copies bytes that no binding has checked checks that length
   itself. The verify work bound below does not depend on the length check. Not
   built yet (driftsys/ridl#1128).

   **Not checked, by design** (author's call): pad bytes, anywhere. First-fit
   backfill can place a field appended in a later release in the pad bytes of an
   older layout without changing `head_size`, so an older reader that checked
   pads would reject a valid newer buffer. Also not checked **(Sebastien)**:
   tail bytes that no descriptor covers, and overlap between descriptors. The
   buffer is read-only after the gate's copy, so two descriptors over the same
   bytes cannot corrupt each other. A producer can therefore leave unreferenced
   bytes in the tail and non-zero bytes in the pads; no consumer API reads
   either.

   **Verify work bound** (author's call). Overlapping descriptors can make
   `verify` do more work than the buffer length suggests, because two
   descriptors may cover the same bytes. The work is bounded by the maxima the
   type declares: at each nesting level every descriptor can name at most its
   bound, so the bound is the product of the nested maxima along the deepest
   path, which can be larger than a buffer of maximum size when descriptors
   overlap. This bound is accepted.

9. **The encoder's duty (Sebastien).** The encoder writes every pad byte, the
   preamble's `reserved` word and every byte of an absent optional as zero (so
   its descriptor is all zero), and writes no unreferenced byte into the tail.
   Zeroed pads close the leak of stale memory to a consumer. `verify` checks
   absent optionals but neither pads nor unused tail capacity, so its cost does
   not grow with capacity. `ridl_rt::podbuf::Writer` enforces the zero fill: it
   zeroes every byte it reserves, and a store outside the bytes it wrote makes
   `finish` return `EncodeError::Layout`.

10. **Evolution within one type version.** typl assigns field identity by
    declaration order and is append-only (typl §7.4). That is the first-fit
    law's precondition.

    _Compatible:_
    - Appending a field at the end of a struct **(Sebastien)**, under the rule
      of typl §7.4 (author's call on how podbuf applies it). An appended field
      reads as 0 wherever its bytes are when the producer did not write it.
      First-fit backfill can place the field in an older layout's pad bytes,
      inside the older `head_size`, so `head_size` alone cannot tell that the
      field is missing; the encoder's zeroed pads make a missing field and a
      field with the value 0 the same bytes. There is no declared default for a
      missing field. So `ridl diff` accepts an append for podbuf only when 0 is
      a valid value of the field: an optional field, whose flag bit reads 0 and
      so means absent, or a scalar, enum or enum set whose type allows 0. Any
      other appended non-optional field is a break.
      - _Older reader, newer (longer) head:_ the reader reads only the fields it
        knows and ignores the head bytes it does not know, including flag bits
        and whole flags fields of later fields.
      - _Newer reader, older (shorter) head:_ a field whose bytes lie past the
        producer's `head_size`, or in the older layout's pads, reads as 0. The
        generated accessors and `verify` read past `head_size` as 0 through
        `ridl_rt::podbuf::Region`.
    - Appending a field to the element type of an array, to the key or value
      type of a map entry, or to a union arm type, under the same rule. The
      descriptor's `elem_size` or the arm descriptor's `len` gives the
      producer's size, and the fields past it read as 0.
    - Flag bits of appended optional fields follow the first-fit law: the bit
      goes in an existing flags field if that field has a free bit, otherwise in
      a new flags field. A reader checks only the flag bits of the fields it
      knows (**Sebastien**).
    - Adding a value to an open enum or a flag to an open enum set (decision 12;
      not built).
    - Retiring a field with typl's `@deprecated` doc tag **(Sebastien)**. The
      field and its type stay in the source and in the bytes; the encoder writes
      the field's init value (typl §5.8, §7.2), not 0, so a reader that still
      reads the field sees a valid value (author's call). RIDL-155 reports the
      field whose init value is neither declared nor derivable and whose type
      excludes 0.

    _Breaking (a new type version):_ reordering; widening an integer across a
    width boundary; changing a bound; removing an arm or an enum value; adding a
    value to a strict enum or a flag to a strict enum set **(Sebastien)**;
    appending an enum value, an enum-set flag or a union arm that changes the
    podbuf integer of the enum or the enum set or the tag of the union, by width
    or by signedness (the narrowest integer that holds every value, signed when
    a value is negative, by the rules of decision 5: `u8` to `u16` at 256, `u16`
    to `u32` at 65,536 and `u32` to `u64` at 4,294,967,296; for an enum set,
    `u8` to `u16` at bit 8, `u16` to `u32` at bit 16 and `u32` to `u64` at bit
    32, by the width typl §9.3 infers; a negative value appended to an enum that
    has values is an insert below its highest value, a generic break, so the
    integer turns signed only when an enum with no value gains values), which
    lays out a different integer in every head that holds the enum, the enum set
    or the union; appending a field that cannot read as 0; appending to a struct
    that a head of the released side holds inline, when the append grows the
    struct's head; changing a strict enum to an open one or the reverse; and
    removing a field.

    **A strict-enum append is reported (builder's call).** `ridl diff` reports
    an appended enum value or enum-set flag as the category
    `podbuf_strict_enum_append`, because an older reader's `verify` refuses a
    value or a bit its declaration does not name (decision 12).

    **An append that grows a struct nested inline is a break** (author's call,
    narrowed 2026-10-29): the inline copy cannot grow without moving the fields
    after it. The inline uses are a struct field, a tuple member and an element
    of an exact-length array; moving such a field behind a descriptor is out of
    scope. The holders are read on the old side: a head that holds the struct
    inline there is a published copy, where a holder that is new on the new side
    has none, and `ridl diff` names the old holders. The head grows when an
    appended field does not fit entirely inside the old head: the layout law
    places any of its bytes past the old head's size (every head has the
    alignment `HEAD_ALIGN`, so a change of the head is a change of its size). A
    field first-fit places entirely inside the old head, in its pad bytes,
    changes no byte an older reader checks, because `verify` does not check pads
    (decision 8, "Not checked, by design"), so it is compatible and draws the
    ordinary `decl_added` line alone; a field that starts in the pads and ends
    past the old size does not fit, and is a break. When several fields are
    appended at once, `ridl diff` draws `podbuf_inline_append` for each field
    that does not fit and for none that does. A new declaration the law refuses
    is read as grown, which keeps the conservative verdict.

    _Amended (2026-10-29, driftsys/ridl#1100)._ As first written, every append
    to a struct held inline was a break, on holders of the new side. The two
    narrowings above (the head must grow, and only an old holder counts) are the
    amendment.

    **The inline-use rule, as built (builder's call).** `ridl diff` finds an
    inline use by asking the layout law what the encoder lays out inline, so the
    rule mirrors the heads the encoder lays out: a declaration, and a named
    induced tuple (the argument struct of a call with several parameters, and a
    tuple reply). A struct that is a root of its own (a single named parameter,
    a named reply, a signal or event payload) is not an inline use, because the
    root's `head_size` gives the producer's size. A root that is an exact-length
    array, such as a fixed payload `[S; 2..2]`, has no head and contributes
    nothing; a tuple inside such a root is a head, and a struct inside that
    tuple is a use. A struct that is the element of a bounded array, the key or
    value of a map entry, or a union arm is not an inline use either, because
    the stride or the arm's `len` gives the producer's size. This settles the
    design's two bullets that disagreed about a map value that is directly a
    struct: it is not an inline use, and a field appended to it is compatible.
    The report names only the nearest inline use (`SensorBounds/range`, not the
    head that holds `SensorBounds`), because another declaration held inline is
    not entered: its own fields are read when the walk reaches its struct.

    **A field removal is breaking under every encoding (builder's call).**
    Removing a struct field with a `reserved` tombstone takes the type out of
    the source, so its bytes cannot be kept. `ridl diff` already reports every
    struct-field removal as `decl_removed`, breaking, whatever the encodings; no
    podbuf-specific category for a retired field exists, and a test pins the
    breaking line under both. The design listed the removal as a podbuf break.

11. **Type versions are a convention (Sebastien).** ridl does not track type
    versions. After a breaking change, the author declares a new type with a new
    name (for example `FaultEventV2`) and keeps the old declaration while
    consumers still need it. The producer serves the current version and the two
    before it (N-2); choosing which version to write for a consumer is the
    producer's and the gate's logic, not ridl's.

12. **Strict and open enums (Sebastien, 2026-10-24).** Decided; the `open`
    keyword is not built.
    - An enum is **strict** by default: `verify` rejects any value the
      declaration does not name, and adding a value is a break (a new type
      version, decision 11).
    - An enum declared with the contextual keyword `open` is **open**: `verify`
      accepts any value that fits its integer width, and the accessor and decode
      return a known value or `unknown(raw)`. Every consumer handles `unknown`.
      Adding a value is compatible. The same rule applies to `enumset`: an
      `open enumset` accepts and keeps undeclared bits. The generated Rust for
      an open enum carries an unknown variant and `#[non_exhaustive]`; the shape
      of that variant is left to the change that adds `open`.
    - `open` is a contextual keyword, recognised only before `enum` or
      `enumset`, and not reserved. It is a typl language change, recorded
      separately (typl §8, §9, the backends and `ridl diff`). **That change is a
      prerequisite of the open rules and is not built.** Today every enum and
      enum set is strict, and the codec and `verify` implement only the strict
      rules. `ridl diff` reports the strict-enum break of an appended value or
      flag as decision 13 says; when `open` lands, that rule narrows to strict
      enums and enum sets, and an append to an open one is compatible. Before
      the change lands it must check how today's backends and `ridl diff` treat
      a new enum value, so that defaulting every enum to strict breaks nothing
      silently. A new value that does not fit the inferred width or signedness
      of an open enum changes the width, which is a break, as for an integer
      range (typl §4.2).

13. **The checks.**
    - `ridl diff` reports each break of decision 10 as a podbuf break and each
      compatible change as compatible. Three changes are judged by encoding, all
      listed under the heading "breaking under podbuf" and breaking when
      `podbuf` is among the encodings the comparison is judged for, compatible
      otherwise: the append that grows an inline struct, the category
      `podbuf_inline_append`; the append that changes the podbuf integer of an
      enum, an enum set or a union's tag, the category `podbuf_width_change`,
      reported once per declaration with the old and the new integer; and the
      append to a strict enum or enum set, the category
      `podbuf_strict_enum_append`, reported once per declaration, except that an
      enum or an enum set whose append changes the integer draws only the
      `podbuf_width_change` line (builder's call). The three rules are stated in
      decision 10; `ridl diff --explain <category>` states each in full. The
      compatible verdict without podbuf holds under proto3; an enum-set flag
      that widens the integer also widens the enum set's FlatBuffers scalar,
      which `ridl diff` does not report yet (driftsys/ridl#1120). An appended
      union arm that keeps the tag is reported as the generic rule judges it,
      compatible. When the `open` keyword is built (open item 1), the strict
      enum rule narrows to strict enums and enum sets.
      `ridl diff --encoding <name>` and the `ridl_diff` MCP tool name the
      encodings, because a snapshot does not record them.
    - RIDL-156 `podbuf-padding-order`, a lint at `info`, reports a struct whose
      declared field order lays out in a larger head than the same fields sorted
      by descending alignment (declaration order kept among equal alignments).
      The order is wire identity once published, so the lint applies only to a
      struct that no baseline snapshot declares; with no baseline every struct
      is unreleased (this is how the lint knows, which the design left to the
      builder). `ridl check` raises it, and only when a `[backend.<name>]` table
      lists `podbuf`. The positive case is a struct whose saving order is
      strictly smaller; `FaultEvent` is the negative case, since it holds 18
      data bytes and every order rounds to 24 (builder's call; the design named
      it as a positive example).
    - **Known gap.** `ridl build` and `ridl check --baseline` do not pass the
      manifest's `[backend.<name>] encodings` to the diff, so a workspace that
      builds podbuf and gates on its baseline accepts an inline-append break
      until the follow-up lands. It is tracked as driftsys/ridl#1095.

14. **What podbuf generates, and what it exposes (Sebastien).** Per type:
    - the head struct with its pad fields and assertions, and the preamble
      struct;
    - an encoder from the domain type, which writes the preamble;
    - `verify` and `decode` in Rust, and the read-in-place view with one
      accessor per field;
    - a C header with the same structs, assertions and inline accessor
      functions, for consumers that read a buffer the gate has verified.

    The Rust codec is emitted into a package's output when the manifest's
    `[backend.rust] encodings` names `podbuf`; the C header is written by
    `ridl build --emit podbuf`. podbuf exposes its computed layout — each
    field's offset, size, alignment and form, each descriptor's element type,
    the tag and arm layout of a union — to codegen plugins in the codegen
    `Model` (`Model.podbuf`, `PodbufProjection`), so a plugin never computes the
    law again (ADR-0020 decisions 8 and 9). **As built (builder's call):** the
    model carries `Declaration.podbuf`, the size state of a declaration as a
    payload root, and `PodbufProjection.boxes`, the one-field head of each named
    scalar, enum or enum set that is a payload root. Both are required because
    interfaces use such types as payloads, and ADR-0020 decisions 8 and 9 forbid
    a plugin from computing a layout or a bound itself. The wire dump of the
    stability policy holds the offset and size of every field of the head of
    every struct and union of the corpus.

15. **Settled by the implementation (author's calls, adjusted to the code).**
    - **Languages.** podbuf generates Rust and C. TypeScript and wasm are not in
      scope; those boundaries keep FlatBuffers (ADR-0020 decision 2).
    - **Floats in `verify`.** A float is checked only by the typl constraints
      its type declares; a float type with no range accepts any bit pattern, NaN
      included.
    - **ridl-rt integration.** podbuf implements the encoding contract of
      `ridl-rt` as proto3 and FlatBuffers do (ADR-0020 decision 5), under the
      `podbuf` feature, with the marker `ridl_rt::encoding::Podbuf` and the
      shared reading and writing in `ridl_rt::podbuf`. The `ridl-rt` features
      `flatbuffers`, `proto3` and `podbuf` take no dependency.
    - **No `unsafe`.** Generated Rust reads fields with safe little-endian loads
      over a bounds-checked slice, never an `unsafe` pointer cast. It is
      `no_std`, and `verify` and the in-place reads allocate nothing; `decode`
      allocates for `String`, `Vec` and maps as the other codecs do. The minimum
      Rust version follows `ridl-rt`'s.
    - **The string accessor re-validates UTF-8 on each read (changed from the
      design).** The design said a field read after `verify` costs one compare
      and one load. That holds for a scalar. A `string` accessor returns a
      `&str`, and without `unsafe` the only way to build one is
      `core::str::from_utf8`, so each read scans the string once, at a cost
      proportional to its length. `verify` has already proved the bytes valid,
      so the check cannot fail; it is the price of having no `unsafe`.
    - **Decode.** podbuf generates `decode` into the owned domain type.
    - **ridl-rt-conformance.** The design said `ridl-rt-conformance` gets podbuf
      cases. It holds the port contract tests, which do not depend on an
      encoding, so it gains none. The conformance of the codec is in the tests
      of `ridl-backend-rust` (`docs/design/podbuf-codec.md`).

## Where this record departs from the design

The design's decisions stand except where the code differed. Each difference is
stated in the decision above; this list is the index.

- The codes are RIDL-153 to RIDL-156, not the numbers the plan proposed
  (decision 6).
- `elem_size` of a tuple element is its head size, a multiple of 8, not its
  element size (decision 3).
- Tail order is the order of the head's placements, which are sorted by offset
  (decision 4).
- `verify` is a call tree with one function per head type (decision 8).
- A boolean above 1 and an absent optional with non-zero bytes are refused with
  existing `Contract` rules; `Payload<Podbuf>::verify` refuses a buffer longer
  than `MAX_SIZE` (decision 8; withdrawn on 2026-10-30 by ADR-0030 decision 19).
- The string accessor re-validates UTF-8 on each read (decision 15).
- A field removal is breaking under every encoding, not a podbuf-specific break
  (decision 10).
- A map value that is directly a struct is not an inline use (decision 10).
- The padding lint does not cite `FaultEvent`, which no order improves (decision
  13).
- The model carries the per-declaration bound and the boxes (decision 14).
- `ridl-rt-conformance` gets no podbuf case (decision 15).
- The `open` keyword is a prerequisite that is not built (decision 12).

## Alternatives considered

| Alternative                                                                                                          | Outcome        | Reason                                                                                                                                                   |
| -------------------------------------------------------------------------------------------------------------------- | -------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Fixed-capacity `repr(C)` (all strings and lists inline at capacity)                                                  | rejected       | `FaultPage` was 17,416 bytes at any fill, and verify cost grew with capacity (11,015 R52 instructions for 4 of 64 events against 1,594)                  |
| Inline when capacity ≤ 256 bytes, tail otherwise                                                                     | rejected       | with a generated adapter, it saved almost nothing at run time, and cost two forms in the law, a threshold, and up to 4× wasted bytes                     |
| FlatBuffers on this path                                                                                             | kept elsewhere | more tolerant evolution and many languages, but slower to encode (2–14×) and to read, larger on the R52 (about 23 KB against 3.6 KB), offsets everywhere |
| proto3 on this path                                                                                                  | kept elsewhere | smallest bytes and best evolution, but a full parse on every read (2–11× slower verify plus decode)                                                      |
| SOME/IP serialization                                                                                                | rejected       | big-endian, packed and length-prefixed: a stream format, handled by connectors                                                                           |
| Sort all fields by size                                                                                              | rejected       | an appended field would sort into the middle and move existing fields                                                                                    |
| Sort fields by size per release segment                                                                              | rejected       | needs release history at build time; first-fit packs as well with no history                                                                             |
| Every field a 64-bit cell                                                                                            | rejected       | 2–8× larger heads for scalars, two-register compares on the R52                                                                                          |
| Typed tombstone (`reserved x : T`)                                                                                   | rejected       | a language change for one backend; `@deprecated` already keeps the field and its type                                                                    |
| Version attribute on a type, or the generator reading previous IR                                                    | rejected       | new syntax, or a build that depends on stored history; versions stay a convention                                                                        |
| 8-aligned descriptors                                                                                                | rejected       | a special case in the law with no packing gain                                                                                                           |
| Inline C union sized for the largest arm                                                                             | rejected       | every message pays for the largest arm                                                                                                                   |
| Unsorted map entries                                                                                                 | rejected       | duplicate detection becomes O(n²); sorting gives one pass and reproducible bytes                                                                         |
| Doc tag `@non_exhaustive` for an open enum                                                                           | rejected       | the deletion test (`family-general-form.md` §4.1) forbids a doc tag whose removal changes tool output                                                    |
| Attribute `[open]` for an open enum                                                                                  | rejected       | strict or open changes which values the type contains, which is part of its shape, not an annotation                                                     |
| Enum value fallback to an existing value                                                                             | rejected       | the producer must know the consumer's version; open enums need no such knowledge                                                                         |
| Pad-byte check in `verify`                                                                                           | rejected       | an older reader would reject a valid newer buffer whose appended field sits in the older layout's pad bytes                                              |
| Absence detected by `head_size` alone (`offset + size > head_size` means absent or default)                          | rejected       | first-fit backfill puts an appended field in the older layout's pads, inside `head_size`; fields read as 0 instead (decision 10)                         |
| `ridl diff` break for an append to any nested type, element types included                                           | rejected       | every array element type and map entry type would be frozen; the stride in the descriptor keeps element appends compatible at no byte cost               |
| 8-byte stride record `{ elem_size, reserved }` at the start of each array or map region in the tail                  | rejected       | 8 bytes per array or map for no gain within the `u16` limits of the descriptor stride                                                                    |
| Separate stride for a union arm                                                                                      | rejected       | the arm descriptor's `len` already gives the arm head size                                                                                               |
| Zeroed-tail check in `verify`                                                                                        | rejected       | made verify cost grow with capacity; moved to the encoder                                                                                                |
| A podbuf-specific category for a retired field                                                                       | rejected       | `ridl diff` already reports every field removal as breaking under every encoding, so the category could never be emitted                                 |
| `unsafe` pointer casts for a one-compare string read                                                                 | rejected       | the generated code and `ridl-rt` hold no `unsafe`; a UTF-8 scan per string read is the cost                                                              |
| Names `reprbuf`, `ridlbuf`, `hptbuf`, `typlbuf`, `wirebuf`, `membuf`, `cbuf`, `rxbuf`, `rpbuf`, `tailbuf`, `spanbuf` | rejected       | Rust-only, toolchain-bound, unexplained, misleading (wire, receive buffer, RPMsg), or taken on crates.io                                                 |

## Consequences

- **Positive.** A consumer reads a field at a fixed offset with no library, in C
  from the generated header or in Rust from the view. The layout is one
  computation, shared by the Rust codec, the C header, the sizer, `ridl diff`
  and the padding lint, and exposed to plugins in the model. The assertions fail
  a build on a toolchain that disagrees with the law.
- **Positive.** `verify` costs in proportion to the data present, and its
  refusals reuse the existing `Malformed` and `Contract` outcomes.
- **Negative.** A head that holds only scalars is not byte-identical to an RTE
  struct, because the 8-byte preamble precedes it. The adapter that an ARXML or
  Franca IDL plugin generates covers the difference.
- **Negative.** A string that contains U+0000 cannot be encoded.
- **Negative.** An append that grows a struct held inline is a break, so a type
  that other types hold inline can grow after its first release only by the pad
  bytes of its head, unless its holders change too (decision 10, as amended
  2026-10-29).
- **Negative.** Three encodings are three conformance obligations, and the
  oracle of this one is the layout law itself: no second implementation exists
  to compare against.
- **Freeze.** The `podbuf` feature name of `ridl-rt` and the podbuf lines of the
  `wire` dump join the surfaces ADR-0029 freezes. The rename from `repr(C)` was
  a break of those surfaces, approved in the pull request that made it.

## Open items

1. **The `open` keyword.** Decision 12's open enums wait for the typl change. It
   needs its own record, and a check of how every backend and `ridl diff` treat
   a new enum value under a strict default.
2. **A compatible append that raises `max_size`.** Closed 2026-10-30 by
   [ADR-0030](ADR-0030-transfer-and-bound-limits.md) decision 19, with the first
   answer: `verify` checks structure only and no longer refuses a buffer longer
   than the reader's `MAX_SIZE`, and the transport refuses a payload longer than
   its interaction's limit. An append that raises `max_size` stays compatible.
   The text below is the question as it was asked. `Payload<Podbuf>::verify`
   refuses a buffer longer than the reader's own `MAX_SIZE`. Decision 10 makes
   an append of an optional field, or of a field that reads as 0, compatible. An
   append can raise the type's `max_size`, so an older reader refuses the
   largest values a newer producer writes, although decision 10 calls the change
   compatible. The two rules are in tension, and the record does not settle
   them. The answer is one of: relax the length check of
   `Payload<Podbuf>::verify` (decision 8 already bounds the work by the maxima
   the type declares), or state that an append that raises `max_size` is a
   break. Tracked as driftsys/ridl#1099.
3. **`ridl build` and `ridl check --baseline` and the encodings** (decision 13):
   tracked as driftsys/ridl#1095.
4. **A C reader for a boxed root.** The header has no reader for a payload root
   that is a named scalar, an enum or an enum set (the box). It is listed in the
   known gaps of `docs/design/podbuf-codec.md`, as driftsys/ridl#1096.
5. **Quantized floats.** The typl reference defines a quantized wire form, a
   float with a range and a step sent as a scaled integer (typl §4.3); the spike
   showed it removes a large software floating-point cost on the R52. It is a
   later option, not part of this record.

## Documents amended

| Document                                                                                 | Change                                                                                                                                |
| ---------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| [ADR-0020](ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)                | decision 4 and open item 1 point at this record; open item 1 is closed: `Inline` stays FlatBuffers', podbuf is the in-place encoding  |
| [`docs/design/podbuf-codec.md`](../design/podbuf-codec.md)                               | new: the as-built codec                                                                                                               |
| [`docs/design/podbuf-codec.md`](../design/podbuf-codec.md)                               | amended 2026-10-30, to be updated when driftsys/ridl#1128 lands: `verify` has no length check, and the "Known gaps" row of #1099 goes |
| [`docs/book/stability.md`](../book/stability.md)                                         | the `podbuf` feature and the podbuf lines of the wire dump                                                                            |
| [`docs/specification/ridl-family-overview.md`](../specification/ridl-family-overview.md) | ledger row 42, and row 35 cites this record                                                                                           |

## Trace

- Satisfies: ADR-0020 decision 1 (the third encoding) and decision 4 (its
  projection record).
- Related: ADR-0020 decision 3 (domain types do not carry `#[repr(C)]`; the head
  struct is separate), decision 5 (one feature per encoding) and decisions 8 and
  9 (the codegen model); ADR-0016 decision 6 and ADR-0017 decision 4 (totality);
  ADR-0024 (the lint registry); ADR-0029 (the frozen surfaces); ADR-0030 (the
  payload ceiling); typl reference §4.2–§4.6, §7.1–§7.4, §8–§12 and §14.2
  (`@deprecated`).
- Depends on, for decision 12 only: the typl `open` keyword, not built.
- Design record: [`../design/podbuf-codec.md`](../design/podbuf-codec.md).
- The reasoning and the evidence:
  [`../archive/2026-10-24-podbuf-design.md`](../archive/2026-10-24-podbuf-design.md).
- Code: `crates/ridl-ir/src/projection/podbuf.rs`,
  `crates/ridl-rt/src/podbuf.rs`,
  `crates/ridl-backend-rust/src/codec_podbuf.rs`,
  `crates/ridl-backend-podbuf/src/lib.rs`.
