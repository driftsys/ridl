# podbuf — the third payload encoding

Status: draft design, 2026-10-24. Working memory under `docs/wip/`; it becomes
the projection record that
[ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
decision 4 asks for, and an amendment to ADR-0020, when it is gardened.

Each decision below is marked **(Sebastien)** when the owner took it, or
**(author's call)** when the author of this document settled it and the owner
has not confirmed it yet.

## 1. Intent

ADR-0020 decision 1 adds a third payload encoding next to proto3 and
FlatBuffers, for a consumer that reads a field at a fixed offset and links no
reader library. Decision 4 leaves the byte-level rules to a projection record
written with the backend. This document is that record's design.

The case that drives it: messages shared in memory inside one SoC that has
AArch64 application cores and a Cortex-R52-class safety core. A producer may be
untrusted. A gate copies each message into a trust zone and calls a generated
`verify` before any consumer sees it.

**Goal (Sebastien).** On that path, verify and decode cost close to zero, encode
cost is reasonable, and the encoding is more efficient than proto3 — and, as
measured, than FlatBuffers. Floating-point cost is out of scope for this goal.

### 1.1 Goals

- One deterministic byte layout, computed from the typl declaration alone.
- A consumer reads fields in place, with no decode step and no library; a C
  reader needs only the generated header.
- A generated `verify` per type whose cost grows with the data present, never
  with the declared capacity.
- Total over typl: every typl construct has a rule (§6.1), and a type that
  exceeds podbuf's size limits is a diagnostic that names it (ADR-0016 decision
  6, ADR-0017 decision 4).
- Compatible evolution within one type: appended fields, new values of an open
  enum or enum set, deprecated fields.

### 1.2 Non-goals

- A wire or stream format. Anything that leaves the SoC goes through a SOME/IP
  or proto3 connector (Sebastien).
- Big-endian targets (Sebastien).
- A fused copy-and-verify function. The gate owns its copy; ridl generates only
  `verify` (Sebastien). A fused variant can be added later without a format
  change.
- Quantized floats as scaled integers. Set aside (Sebastien); see §9.
- The AUTOSAR RTE adapter and the ARXML export. They come from an ARXML or
  Franca IDL codegen plugin, not from podbuf (Sebastien). An ARXML importer is a
  separate, later project (Sebastien).
- Specifying the `open` keyword. It is a separate typl change (§8.5).
- Tracking type versions. A new type version is a convention, not a ridl concept
  (Sebastien).
- Cache-line alignment and sizing of message buffers. That belongs to the gate
  and the shared-memory transport (Sebastien).

### 1.3 Anti-goals

- A consumer that has to check anything after `verify` has passed.
- A layout that differs between the AArch64 cores and the R52 core.
- A `verify` whose cost depends on unused capacity.
- A layout rule that moves an existing field when a field is appended.

## 2. Name

The encoding is named **podbuf** (Sebastien), for "plain old data buffer": the
head is a plain-old-data `#[repr(C)]` struct, and the whole buffer is plain
bytes that are valid to read in place. ADR-0020 calls the encoding `repr(C)` and
its decision 5 reserves the cargo feature `repr-c`. The amendment renames both:
the encoding is `podbuf` and the `ridl-rt` feature is `podbuf`. `#[repr(C)]`
stays as the description of how the head is laid out.

## 3. Trust model

- A producer may be untrusted (Sebastien).
- The gate copies each message into a trust zone, then calls the generated
  `verify` (Sebastien). The copy closes the time-of-check to time-of-use gap:
  the producer cannot change the bytes after the check.
- After `verify` succeeds, the consumer reads the buffer without checks
  (Sebastien). The buffer is read-only from then on.
- podbuf requires the buffer to start on an 8-byte boundary; `verify` checks it
  (Sebastien). Cache-line alignment is the gate's and the transport's concern.
- **Deployment rule (author's call).** The gate's `verify` for a type must be
  generated from a version of that type at least as new as the version every
  consumer behind the gate was built from. A gate built from an older version
  ignores the bytes of fields it does not know, so it cannot check a value that
  a newer consumer then reads.

## 4. Target rule

- podbuf supports **any little-endian target whose C ABI aligns each fixed-width
  type to at most its size** (Sebastien). This covers AArch64 (AAPCS64), AArch32
  (AAPCS32, the R52 core), x86-64 (SysV and Windows) and i686.
- The generated Rust and C code refuse to compile on a big-endian target
  (author's call).
- Allowed field types in the head: fixed-width integers (`u8` to `u64`, `i8` to
  `i64`), `f32`, `f64`, fixed arrays of these, and nested podbuf structs. Never
  a pointer, `usize`/`size_t`, `long`, a C `enum`, a C bitfield, or a 128-bit
  type (author's call). Every gap is an explicit pad field, so i686 (which
  aligns `u64` to 4 inside a struct) reproduces the same offsets.
- Every head struct is declared with alignment 8: `_Alignas(8)` on its first
  member in C and `#[repr(C, align(8))]` in Rust (author's call). Without it,
  i686 gives a head that holds a `u64` an alignment of 4, which changes where a
  head can sit inside another head or in the tail.
- The generator emits `_Static_assert` checks on `sizeof` and `offsetof` in the
  C header and `const` assertions on `size_of` and `offset_of!` in Rust, for
  every type and field (author's call). A toolchain that disagrees fails the
  build.

## 5. Layout

### 5.1 Preamble, head and tail

A podbuf buffer is `[preamble][head][tail]` (Sebastien).

- The **preamble** is 8 bytes and records the head size (§5.4).
- The **head** is one `#[repr(C, align(8))]` struct per type (§4). It holds
  every fixed-size field: scalars, enums, enum sets, exact-length arrays
  (`[T; N]`, `bytes [N..N]`), nested structs and tuples.
- Every **variable-length** field — a bounded `string`, `bytes` or array, a map,
  and the active arm of a union — is an 8-byte **descriptor** descriptor in the
  head. `off: u32` is relative to the start of the buffer, which is the start of
  the preamble. A `string`, `bytes` or union descriptor is
  `{ off: u32, len: u32 }`; `len` counts bytes for `string`/`bytes` and bytes of
  the arm head for a union. An array or map descriptor is
  `{ off: u32, len: u16, elem_size: u16 }` (Sebastien, decision of 2026-10-24);
  `len` counts elements or entries.
- A descriptor whose `len` is 0 is all zero bytes, and so is the descriptor of
  an absent optional field (author's call). Such a descriptor points at no tail
  data: an empty string has no NUL in the tail.
- The **tail** follows the head and holds only the data actually present: string
  and bytes contents, array elements, map entries, union arms.
- A descriptor has its natural alignment, 4 (author's call). Data in the tail is
  placed at the alignment of its element type.
- **Stride.** `elem_size` is the size in bytes of one element (or map entry) as
  the producer laid it out. A reader steps from one element to the next by
  `elem_size` and reads only the prefix of each element that it knows; a field
  of the element that lies past the producer's `elem_size` reads as 0, by the
  same rule as an appended field (§8.1). The stride costs no bytes: it uses the
  upper half of the descriptor's length word. The limits are 65,535 elements per
  array or map and 65,535 bytes per element (§6.1).
- `elem_size` rule (author's call): for an element or entry that is a struct
  head, `elem_size` is a multiple of 8, which is every head's alignment; for any
  other element type (a scalar, an enum, an enum set, a descriptor, a tuple or
  an exact-length array), `elem_size` equals the element size, because such an
  element cannot grow compatibly.
- A union arm has no separate stride (author's call). The arm's descriptor `len`
  already states the arm head size the producer used, so a reader reads the
  prefix it knows and a field past `len` reads as 0.

A considered threshold rule (bounded fields stored inline when their capacity is
at most 256 bytes) was dropped (Sebastien); see §10.

### 5.2 The first-fit law

(Sebastien)

1. Take the fields in **declaration order**.
2. Place each field at the **lowest offset** where it fits at its natural
   alignment (1, 2, 4 or 8) without overlapping a field already placed.
   **Optional flags** (author's call): the flags are `u8` fields, one per eight
   optional fields. When the placement reaches the optional field number
   `8k + 1` (counting optional fields from 1 in declaration order), it first
   places flags field `k` as a `u8` by this same rule, then places the optional
   field. Bit `i` (from the least significant bit) of flags field `k` is set
   when optional field number `8k + i + 1` is present.
3. Round the struct size up to a multiple of 8.
4. Every unused byte is an explicit pad field (`_padN`) whose value is zero.

The offset of a field depends only on the fields declared before it, so
appending a field never moves an existing one, and a later small field fills a
gap that an earlier alignment left. The generated struct lists the fields sorted
by offset, with the pad fields written out, so a C compiler produces the same
layout. Cap'n Proto assigns struct offsets in the same way.

### 5.3 Worked examples

Measured layouts from the spike (§12), types from the `veh-cluster` corpus.
Placements are offsets inside the head. The buffer size without a tail is the
head size plus the 8-byte preamble.

| Type            | Fields in declaration order                      | Placement                                                                                                               | Head | Buffer without tail |
| --------------- | ------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------- | ---- | ------------------- |
| `DoorPayload`   | `sensor_id: u8`, `is_open: bool`                 | 0, 1; pad 2..8                                                                                                          | 8    | 16                  |
| `ClimateReport` | `cabin`, `setpoint` (16-bit raw), `fan: u8`      | 0, 2, 4; pad 5..8                                                                                                       | 8    | 16                  |
| `DiagFilter`    | `severity: u8`, `category?: string`              | 0; flag byte 1; descriptor 4..12; pad 2..4, 12..16                                                                      | 16   | 24                  |
| `FaultEvent`    | `code: u16`, `message: string`, `timestamp: i64` | 0; descriptor 4..12; `timestamp` cannot use 8..16, which overlaps the descriptor, so 16..24; pad 2..4, 12..16           | 24   | 32                  |
| `FaultPage`     | `faults: [FaultEvent; 0..64]`                    | descriptor 0..8 with `elem_size` 24; elements are 24-byte `FaultEvent` heads, 8-aligned in the tail, strings after them | 8    | 16                  |

Only the buffer start carries a preamble. An element, a union arm or a map entry
in the tail is a head without a preamble.

`FaultEvent` shows that declaration order still affects packing for a type's
first release: declaring `timestamp` before `message` gives 16 bytes instead of
24. §8.4 adds a lint for that.

(`ClimateReport`'s temperatures were measured as scaled 16-bit integers. With
scaled integers set aside, a quantized float is an `f32` or `f64` as typl
infers, and the layout changes accordingly.)

### 5.4 Preamble

(Sebastien, decision of 2026-10-24.) Every podbuf buffer starts with an 8-byte
preamble, so the buffer states its own head size and is self-describing:

    preamble = { head_size: u32, reserved: u32 }

- Both fields are little-endian. The encoder writes `reserved` as zero and a
  reader ignores its value (author's call). A later preamble use of the word
  then needs no change to older readers, which is the same forward-compatibility
  reason that `verify` does not check pad bytes (§7).
- `head_size` is the size in bytes of the head the producer wrote. It is a
  multiple of 8.
- The head starts at offset 8. A descriptor's `off` stays relative to the start
  of the buffer, so the first byte of the tail is at offset `8 + head_size`.
- The preamble does not carry a type id or a version. A reader knows the type
  from the gate or the transport, as before.

Cost: 8 bytes per message. A head that holds only scalars is no longer
byte-identical to an RTE struct, because the preamble precedes it. The adapter
that the ARXML or Franca IDL plugin generates (§10) covers the difference.

## 6. Value rules

| typl construct                 | podbuf representation                                                                                 | Decided by    |
| ------------------------------ | ----------------------------------------------------------------------------------------------------- | ------------- |
| integer                        | the wire width typl infers from the range (typl §4.2)                                                 | Sebastien     |
| float                          | `f32` or `f64` as typl infers (typl §4.3)                                                             | author's call |
| boolean                        | `u8`, 0 or 1                                                                                          | author's call |
| enum                           | the narrowest integer that holds the declared values; strict unless declared `open` (§8.5)            | Sebastien     |
| enum set                       | the integer width typl infers (typl §9.3); strict unless declared `open` (§8.5)                       | Sebastien     |
| optional field `?`             | one bit in a `u8` flags field placed by the first-fit law (§5.2); an absent field's bytes are zero    | author's call |
| tuple                          | an inline struct, same law                                                                            | author's call |
| exact-length array `[T; N]`    | inline `T[N]`, no length                                                                              | Sebastien     |
| bounded `string`               | descriptor; tail holds the UTF-8 bytes followed by one NUL that `len` does not count                  | Sebastien     |
| bounded `bytes`, bounded array | descriptor; tail holds the bytes, or the elements at stride `elem_size`, contiguous                   | Sebastien     |
| union                          | a narrow integer tag (the arm's ordinal) in the head, plus a descriptor to the active arm in the tail | Sebastien     |
| map `[K : V; min..max]`        | descriptor to an array of `{ key, value }` entries sorted by key                                      | Sebastien     |

- typl's `string [N]` bounds scalar values, not bytes (typl §4.4). Because the
  tail stores only the bytes present, no worst-case byte capacity is reserved.
- A string that contains U+0000 cannot be encoded, because the string carries a
  NUL terminator and `verify` rejects an interior NUL (author's call). The
  encoder returns an error for such a string. This is a known limit of podbuf:
  the conformance round trips between podbuf and the other encodings exclude
  strings that contain U+0000.
- A union descriptor's `len` is the head size of the active arm as the producer
  laid it out, a multiple of 8, or 0 for an arm with no fields.
- An enum's integer is signed when any declared value is negative, and unsigned
  otherwise; its width is the narrowest that holds every declared value. For an
  open enum or enum set, a new value that does not fit the inferred width or
  signedness changes the width, which is a break (§8.2), as for an integer range
  (typl §4.2).
- Map key order: bytewise for strings and bytes; numeric for integers and
  floats. `+0.0` and `-0.0` are equal, so both present is a duplicate; a NaN key
  is rejected (Sebastien).
- Every typl string, bytes and collection carries a bound, explicit or the
  `[0..256]` default (typl §4.4, §4.5, §12), so podbuf has no unbounded case.

### 6.1 Totality

Every typl construct has a podbuf rule (author's call):

| typl construct                  | Rule                                                            |
| ------------------------------- | --------------------------------------------------------------- |
| boolean, integer, float         | a fixed-width scalar in the head (§6)                           |
| enum, enum set                  | an integer in the head, strict or open (§8.5)                   |
| string, bytes                   | a descriptor and tail bytes (§5.1, §6)                          |
| exact-length array `[T; N]`     | inline `T[N]` in the head                                       |
| bounded array                   | a descriptor with a stride, and the elements (§5.1)             |
| map                             | a descriptor with a stride, and the sorted entries (§5.1, §6)   |
| union                           | a tag in the head and a descriptor to the active arm head (§6)  |
| struct, tuple                   | a head laid out by the first-fit law (§5.2), inline when nested |
| optional `?`                    | a flag bit, and zeroed bytes when absent (§5.2, §6)             |
| units, constraints, init values | no bytes of their own; constraints are checked by `verify` (§7) |
| recursive shapes                | impossible: typl rejects them (typl §7.3)                       |

podbuf adds build-time errors, each naming the type, for:

- a type whose maximum podbuf size, computed from its declared bounds, exceeds
  `u32::MAX` bytes, because a `u32` offset cannot address all of it;
- an array or map whose maximum count exceeds 65,535, or whose element or entry
  size exceeds 65,535 bytes, because the descriptor's `len` and `elem_size` are
  `u16`.

The exact codes are left to the builder (§16). A payload that exceeds the
payload ceiling of ADR-0030 is reported by the existing lint RIDL-150, not by
podbuf.

## 7. Verify

`verify(buf) -> Result` is generated per type. It checks, in one pass over the
data present (Sebastien for the scope, author's call for the exact list):

1. The buffer start is 8-aligned and the buffer length is at least 8. In the
   preamble, `head_size` is a multiple of 8 and `8 + head_size` is at most the
   buffer length (checked arithmetic). The `reserved` word is not checked
   (§5.4).
2. Each boolean is 0 or 1; each value of a strict enum is declared; each strict
   enum set has no undeclared bit; an open enum accepts any value that fits its
   integer width, and an open enum set accepts and keeps every bit (§8.5); each
   flags integer is checked only at the bits of the optional fields the reader
   knows, and the other bits are ignored; an absent optional's bytes are zero. A
   byte of the reader's head past `head_size`, of an element past the producer's
   `elem_size`, or of an arm past its descriptor `len`, reads as 0 (§8.1), and
   the field that holds it is checked with that value like any other.
3. The typl constraints of each value: integer ranges, the string length bound
   counted in Unicode scalar values (`min` to `max`, typl §4.4), and the other
   generated typl checks, the same code the other encodings call.
4. For each descriptor with `len` = 0: every other descriptor byte is 0. Such a
   descriptor has no data, and the rest of this rule does not apply to it. For
   each other descriptor: `off` is aligned for its data, `off` is at least
   `8 + head_size`, and the data is within the buffer with no overflow (checked
   arithmetic). The data is `len` bytes plus 1 for a string's NUL, `len` bytes
   for `bytes` and for a union arm, and `len·elem_size` bytes for an array or a
   map. For `bytes`, an array and a map, `min ≤ len ≤ max`. For a string,
   `len ≤ 4·max`, the largest UTF-8 byte length of `max` scalar values; this
   bounds the work, and rule 3 checks the scalar-value count.
5. For each non-empty array or map: `elem_size` follows the rule of §5.1 (a
   multiple of 8 for a struct element, the exact element size otherwise).
6. For each non-empty string: the byte after the data is NUL, there is no NUL
   inside the data, and the data is valid UTF-8.
7. For each union: the tag names a declared arm, and `len` is a multiple of 8;
   then the arm is verified.
8. For each map: keys are strictly increasing in the order of §6.
9. Each array element, union arm and map entry is verified with the same rules.

**Not checked, by design:** pad bytes anywhere, in the head or in a nested head
(author's call). First-fit backfill can place a field appended in a later
release in the pad bytes of an older layout without changing `head_size`, so an
older reader that checked pads would reject a valid newer buffer. Also not
checked (Sebastien): tail bytes that no descriptor covers, and overlap between
descriptors. The buffer is read-only after the gate's copy, so two descriptors
over the same bytes cannot corrupt each other. The trade-off is that a producer
can leave unreferenced bytes in the tail and non-zero bytes in the pads; no
consumer API reads either.

**Encoder duty (Sebastien):** the encoder writes every pad byte, the preamble
`reserved` word and every byte of an absent optional as zero (so its descriptor
is all zero), and writes no unreferenced bytes into the tail. Zeroed pads close
the leak of stale memory to the consumer. `verify` checks absent optionals but
neither pads nor unused tail capacity, so its cost does not grow with capacity.

## 8. Evolution

### 8.1 Compatible changes (same type version)

typl assigns field identity by declaration order and is append-only (typl §7.4).
That is the first-fit law's precondition: an appended field never moves an
existing one.

- Appending a field at the end of a struct (Sebastien), under the rule of typl
  §7.4 (author's call on how podbuf applies it). An appended field reads as 0
  wherever its bytes are when the producer did not write it. First-fit backfill
  can place the field in an older layout's pad bytes, inside the older
  `head_size`, so `head_size` alone cannot tell that the field is missing;
  instead, the encoder's zeroed pads make a missing field and a field with the
  value 0 the same bytes. There is no declared default for a missing field. So
  `ridl diff` accepts an append for podbuf only when 0 is a valid value of the
  field: an optional field, whose flag bit reads 0 and so means absent, or a
  scalar, enum or enum set whose type allows 0 (typl §7.4). Any other appended
  non-optional field is a podbuf break.
  - **Older reader, newer (longer) head.** The reader reads only the fields it
    knows. It ignores the head bytes it does not know, including flag bits and
    whole flags fields of later fields.
  - **Newer reader, older (shorter) head.** A field whose bytes lie past the
    producer's `head_size`, or in the older layout's pads, reads as 0. The
    generated accessors and `verify` read past `head_size` as 0.
- Appending a field to the element type of an array, to the key or value type of
  a map entry, or to a union arm type, under the same rule. The descriptor's
  `elem_size` (§5.1) or the arm descriptor's `len` gives the producer's size,
  and the fields past it read as 0.
- An append to a struct type that is nested **inline** in a head anywhere in the
  workspace — a struct field, a tuple member, an element of an exact-length
  array, or a struct inside a map entry — is a podbuf break (author's call). The
  inline copy cannot grow without moving the fields after it. `ridl diff`
  reports it, naming each inline use. Moving such a field behind a descriptor is
  out of scope.
- Flag bits of appended optional fields follow the first-fit law like any other
  field: the bit goes in an existing flags field if that field has a free bit,
  otherwise in a new flags field. A reader checks only the flag bits of the
  fields it knows and ignores every other bit (Sebastien). This is the single
  rule for flags in `verify` (§7); a flags integer is never rejected for a bit
  that belongs to a later field.
- Adding a value to an `open` enum, or a flag to an `open` enum set (Sebastien;
  §8.5). An older reader sees the new value as unknown. With the preamble, the
  buffer is fully self-describing, so no compatible change requires the producer
  to know the consumer's version.
- Retiring a field with typl's existing `@deprecated` doc tag (Sebastien). The
  field and its type stay in the source and in the bytes; the encoder writes the
  field's init value (typl §5.8, §7.2), not 0, so a reader that still reads the
  field sees a valid value (author's call). A deprecated non-optional field
  whose init value is neither declared nor derivable, and whose type excludes 0,
  is a podbuf diagnostic: the encoder has no valid value to write.

### 8.2 Breaking changes (new type version)

Removing a field with a `reserved` tombstone (the type is gone from the source,
so its bytes cannot be kept), reordering, widening an integer across a width
boundary, changing a bound, appending to a struct that is nested inline (§8.1),
appending a field that cannot read as 0 (§8.1), removing an arm or an enum
value, adding a value to a strict enum or a flag to a strict enum set
(Sebastien), and adding a value to an open enum or enum set that changes its
inferred width or signedness (§6; author's call). Changing a strict enum to an
open one, or the reverse, also breaks: it changes which values the type
contains.

### 8.3 Type versions are a convention

ridl does not track type versions (Sebastien). After a breaking change, the
author declares a new type with a new name (for example `FaultEventV2`) and
keeps the old declaration while consumers still need it. The producer serves the
current version and the two before it (N-2); choosing which version to write for
a consumer is the producer's and the gate's logic, not ridl's (Sebastien).

### 8.4 Checks

- `ridl diff` reports each change of §8.2 as a podbuf break and each change of
  §8.1 as compatible (author's call on the reporting form).
- A lint reports a declaration order whose first-fit layout wastes padding that
  a different order would save, for example on `FaultEvent`. It applies only
  before a type's first release, because order is frozen afterwards; how the
  lint knows a type is unreleased is left to the builder (§12).

### 8.5 Strict and open enums

(Sebastien, decision of 2026-10-24.)

- An enum is **strict** by default. `verify` rejects any value that the
  declaration does not name. Adding a value is a break, so it is a new type
  version by the convention of §8.3.
- An enum declared with the contextual keyword `open`
  (`open enum DtcStatus
  { ... }`) is **open**. `verify` accepts any value that
  fits the integer width. The read-in-place accessor and the decode return a
  known value or `unknown(raw)`. Every consumer must handle `unknown`. Adding a
  value is compatible; an older reader sees it as unknown. The generated Rust
  for an open enum carries an unknown variant, with its exact shape left to the
  builder (§16), and `#[non_exhaustive]`.
- The same rule applies to `enumset`. A strict enum set rejects undeclared bits.
  An `open enumset` accepts and keeps them, so adding a flag is compatible.
- `open` is a contextual keyword. It is recognised only before `enum` or
  `enumset` and is not reserved.
- `open` is a typl language change, recorded separately (typl §8, §9, the
  backends and `ridl diff`). This design depends on it and does not specify it.
  Before that change lands, it must check how today's backends and `ridl diff`
  treat a new enum value, so that defaulting every enum to strict breaks nothing
  silently.

## 9. Floats

Floats are out of scope for the efficiency goal (Sebastien). A float field is an
`f32` or `f64` in the head. The typl reference already defines a quantized wire
form, a float with a range and a step sent as a scaled integer (typl §4.3), and
the spike showed it removes a large software floating-point cost on the R52; it
is recorded here as a later option, not part of this design.

## 10. What podbuf generates, and what it exposes

podbuf generates, per type (Sebastien):

- the `#[repr(C)]` head struct, with pad fields and assertions, and the preamble
  struct;
- an encoder from the domain type, which writes the preamble;
- `verify`, which reads the preamble first, generated in Rust only, because the
  gate is Rust (author's call);
- read-in-place accessors (a field read is a bounds-checked load after `verify`,
  §14; a string read returns a slice built from its descriptor);
- a C header with the same structs, assertions and accessor macros or inline
  functions, for consumers that read a buffer the gate has verified.

podbuf exposes its computed layout — each field's offset, size, alignment and
form (in place or descriptor), each descriptor's element type, the tag and arm
layout of a union — to codegen plugins through the plugin interface (Sebastien).
An ARXML or Franca IDL plugin uses it to generate the RTE adapter and the
interface description without computing the first-fit law a second time.

## 11. Alternatives considered

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
| Absence detected by `head_size` alone (`offset + size > head_size` means absent or default)                          | rejected       | first-fit backfill puts an appended field in the older layout's pads, inside `head_size`; fields read as 0 instead (§8.1)                                |
| `ridl diff` break for an append to any nested type, element types included (option (a))                              | rejected       | every array element type and map entry type would be frozen; the stride in the descriptor keeps element appends compatible at no byte cost               |
| 8-byte stride record `{ elem_size, reserved }` at the start of each array or map region in the tail                  | rejected       | 8 bytes per array or map for no gain within the `u16` limits of the descriptor stride (§6.1)                                                             |
| Separate stride for a union arm                                                                                      | rejected       | the arm descriptor's `len` already gives the arm head size                                                                                               |
| Zeroed-tail check in `verify`                                                                                        | rejected       | made verify cost grow with capacity; moved to the encoder                                                                                                |
| Names `reprbuf`, `ridlbuf`, `hptbuf`, `typlbuf`, `wirebuf`, `membuf`, `cbuf`, `rxbuf`, `rpbuf`, `tailbuf`, `spanbuf` | rejected       | Rust-only, toolchain-bound, unexplained, misleading (wire, receive buffer, RPMsg), or taken on crates.io                                                 |

## 12. Evidence

A throwaway spike, run outside the repository on 2026-10-24, measured ridl's
generated proto3 and FlatBuffers code against a hand-written podbuf (called
"head plus tail" there) on five `veh-cluster` types. Instruction counts come
from QEMU emulating a Cortex-R52 (`mps3-an536`); they are not cycles, and no
cache or shared-memory latency is modelled. The podbuf code was hand-written for
five types, so a generated version may differ.

R52 instructions, verify with the typl checks, and verify plus reading every
field:

| Payload                | FlatBuffers verify | podbuf verify | FlatBuffers verify + read | podbuf verify + read | proto3 verify + decode |
| ---------------------- | ------------------ | ------------- | ------------------------- | -------------------- | ---------------------- |
| `DoorPayload`          | 92                 | 26            | 176                       | 47                   | 521                    |
| `DiagFilter` (10/64)   | 314                | 246           | 483                       | 267                  | 874                    |
| `FaultEvent` (41/256)  | 565                | 413           | 825                       | 437                  | 1,630                  |
| `FaultEvent` (256/256) | 1,496              | 1,531         | 1,993                     | 1,555                | 3,046                  |
| `FaultPage` (4/64)     | 2,208              | 1,594         | not measured              | 1,668                | 7,237                  |
| `FaultPage` (64/64)    | 33,783             | 25,214        | not measured              | 26,008               | 114,695                |

R52 encode: FlatBuffers 418 / 589 / 735 / 1,459 / 3,310 / 45,863 against podbuf
30 / 100 / 119 / 681 / 696 / 10,530 for the same rows.

Bytes (proto3 / FlatBuffers / podbuf): 4/24/16, 14/40/35, 53/88/74, 269/304/289,
186/344/246, 2,976/5,024/3,696. The podbuf figures include the 8-byte preamble
of §5.4, which the spike did not have: 8 is added to each measured total.
Instruction counts and `.text` sizes were not measured again with the preamble.

Ratios: FlatBuffers encode is 2–14× podbuf encode; proto3 verify plus decode is
2–11× podbuf verify plus read. The spike's podbuf verify also checked pad bytes,
which this design does not check (§7), so its podbuf verify counts are higher
than the design's would be. The FlatBuffers and podbuf columns are not the same
checks: each verifier checks its own encoding's structure (vtables and offsets
for FlatBuffers, descriptors for podbuf) plus the same typl checks, and the
FlatBuffers read column lacks the two `FaultPage` rows.

R52 `.text` for reader and verifier, all five types: podbuf about 3.6 KB;
FlatBuffers about 22.8 KB and proto3 about 16.6 KB, of which about 10 KB is the
typl float step check in software floating point.

A fuzz test changed each byte of valid podbuf buffers and tried every
truncation; it found no panic and no out-of-bounds access in verify or the
accessors.

## 13. Trace

- Satisfies: ADR-0020 decision 1 (the third encoding) and decision 4 (its
  projection record).
- Related: ADR-0020 decision 3 (domain types do not carry `#[repr(C)]`; the head
  struct is separate) and decision 5 (one feature per encoding); ADR-0016
  decision 6 and ADR-0017 decision 4 (totality); ADR-0029 (the generated surface
  that the new backend adds to); typl reference §4.2–§4.6, §7.1–§7.4, §8–§12,
  §14.2 (`@deprecated`).
- Depends on: the typl `open` keyword for `enum` and `enumset` (§8.5), a
  separate typl change that is not yet written.

## 14. Settled after the builder check (author's calls)

- **Tail order.** The encoder writes tail data in field order, depth first (a
  string inside an array element follows that element's array block). `verify`
  does not depend on the order.
- **Languages.** podbuf generates Rust and C. TypeScript and wasm are not in
  scope; those boundaries keep FlatBuffers (ADR-0020 decision 2).
- **Floats in `verify`.** A float is checked only by the typl constraints its
  type declares; a float type with no range accepts any bit pattern, NaN
  included.
- **Empty union arm.** An arm whose type has no fields has head size 0, so its
  descriptor has `len` = 0 and is all zero (§5.1); `verify` rejects any other
  `off` (§7 rule 4).
- **ridl-rt integration.** podbuf implements the same encoding contract in
  `ridl-rt` as proto3 and FlatBuffers (ADR-0020 decision 5), under the `podbuf`
  feature.
- **Verify depth.** typl has no recursive shapes (§7.3), so the nesting depth of
  a buffer is bounded by the type. The generated `verify` can be non-recursive.
- **Decode.** podbuf also generates decode into the owned domain type. It
  therefore implements the `ridl-rt` encoding contract like proto3 and
  FlatBuffers, and `ridl-rt-conformance` gets podbuf cases.
- **Generated Rust.** It reads fields with safe little-endian loads and no
  `unsafe` pointer casts. A field read after `verify` is a bounds-checked load:
  its cost is one compare and one load, which is what §10 means by a field read.
  It is `no_std`, and `verify` and the in-place reads allocate nothing. The
  minimum Rust version follows `ridl-rt`'s.
- **Verify work bound.** Overlapping descriptors can make `verify` do more work
  than the buffer length suggests, because two descriptors may cover the same
  bytes (§7). The work is bounded by the maxima that the type declares, which is
  the same worst case as a buffer of maximum size. This bound is accepted.

## 15. Open questions for Sebastien

1. **How does a reader learn the producer's head size?** Decided (Sebastien,
   2026-10-24): every buffer carries a preamble with its head size (§5.4). The
   gate does not pass the producer's layout to `verify`.
2. **How does an enum accept new values?** Decided (Sebastien, 2026-10-24): an
   enum is strict by default and an `open` enum accepts unknown values (§8.5).
   No fallback value is declared. A doc tag is rejected because removing it
   would change tool output, and an attribute is rejected because strict or open
   is part of the type's shape (§11).

## 16. Left to the builder

- The Rust and C names of generated items, and the accessor API shape, including
  the shape of the unknown variant of an open enum.
- The diagnostic codes of the size-limit errors (§6.1) and of the deprecated
  field with no valid value (§8.1), and the codes for each break `ridl diff`
  reports.
- How the padding lint decides that a type has not been released yet.
- How the computed layout is represented in the codegen `Model`.
- The ADR-0020 amendment text and the feature rename in `ridl-rt`.
- Test strategy, including a fuzz test of `verify` and cross-target layout tests
  (AArch64, AArch32, x86-64, i686).
