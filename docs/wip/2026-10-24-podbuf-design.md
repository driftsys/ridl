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
- Total over typl: every typl construct has a rule, and a construct the encoding
  cannot carry is a diagnostic that names it (ADR-0016 decision 6, ADR-0017
  decision 4).
- Compatible evolution within one type: appended fields, new enum values,
  deprecated fields.

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
- The generator emits `_Static_assert` checks on `sizeof` and `offsetof` in the
  C header and `const` assertions on `size_of` and `offset_of!` in Rust, for
  every type and field (author's call). A toolchain that disagrees fails the
  build.

## 5. Layout

### 5.1 Preamble, head and tail

A podbuf buffer is `[preamble][head][tail]` (Sebastien).

- The **preamble** is 8 bytes and records the head size (§5.4).
- The **head** is one `#[repr(C)]` struct per type. It holds every fixed-size
  field: scalars, enums, enum sets, exact-length arrays (`[T; N]`,
  `bytes [N..N]`), nested structs and tuples.
- Every **variable-length** field — a bounded `string`, `bytes` or array, a map,
  and the active arm of a union — is an 8-byte **descriptor**
  `{ off: u32, len: u32 }` in the head. `off` is relative to the start of the
  buffer, which is the start of the preamble; `len` counts bytes for
  `string`/`bytes` and elements for an array or a map.
- The **tail** follows the head and holds only the data actually present: string
  and bytes contents, array elements, map entries, union arms.
- A descriptor has its natural alignment, 4 (author's call). Data in the tail is
  placed at the alignment of its element type.

A considered threshold rule (bounded fields stored inline when their capacity is
at most 256 bytes) was dropped (Sebastien); see §10.

### 5.2 The first-fit law

(Sebastien)

1. Take the fields in **declaration order**.
2. Place each field at the **lowest offset** where it fits at its natural
   alignment (1, 2, 4 or 8) without overlapping a field already placed.
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

| Type            | Fields in declaration order                      | Placement                                                                                                     | Head | Buffer without tail |
| --------------- | ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------- | ---- | ------------------- |
| `DoorPayload`   | `sensor_id: u8`, `is_open: bool`                 | 0, 1; pad 2..8                                                                                                | 8    | 16                  |
| `ClimateReport` | `cabin`, `setpoint` (16-bit raw), `fan: u8`      | 0, 2, 4; pad 5..8                                                                                             | 8    | 16                  |
| `DiagFilter`    | `severity: u8`, `category?: string`              | 0; flag byte 1; descriptor 4..12; pad 2..4, 12..16                                                            | 16   | 24                  |
| `FaultEvent`    | `code: u16`, `message: string`, `timestamp: i64` | 0; descriptor 4..12; `timestamp` cannot use 8..16, which overlaps the descriptor, so 16..24; pad 2..4, 12..16 | 24   | 32                  |
| `FaultPage`     | `faults: [FaultEvent; 0..64]`                    | descriptor 0..8; elements are 24-byte `FaultEvent` heads, 8-aligned in the tail, strings after them           | 8    | 16                  |

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

- Both fields are little-endian. `reserved` must be zero.
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
| enum                           | the narrowest integer that holds the declared values                                                  | author's call |
| enum set                       | the integer width typl infers (typl §9.3); undeclared bits are zero                                   | author's call |
| optional field `?`             | one bit in a flags integer placed by the first-fit law; an absent field's bytes are zero              | author's call |
| tuple                          | an inline struct, same law                                                                            | author's call |
| exact-length array `[T; N]`    | inline `T[N]`, no length                                                                              | Sebastien     |
| bounded `string`               | descriptor; tail holds the UTF-8 bytes followed by one NUL that `len` does not count                  | Sebastien     |
| bounded `bytes`, bounded array | descriptor; tail holds the bytes or the element heads, contiguous                                     | Sebastien     |
| union                          | a narrow integer tag (the arm's ordinal) in the head, plus a descriptor to the active arm in the tail | Sebastien     |
| map `[K : V; min..max]`        | descriptor to an array of `{ key, value }` entries sorted by key                                      | Sebastien     |

- typl's `string [N]` bounds scalar values, not bytes (typl §4.4). Because the
  tail stores only the bytes present, no worst-case byte capacity is reserved.
- A union descriptor's `len` equals the head size of the active arm.
- Map key order: bytewise for strings and bytes; numeric for integers and
  floats. `+0.0` and `-0.0` are equal, so both present is a duplicate; a NaN key
  is rejected (Sebastien).
- Every typl string, bytes and collection carries a bound, explicit or the
  `[0..256]` default (typl §4.4, §4.5, §12), so podbuf has no unbounded case.

## 7. Verify

`verify(buf) -> Result` is generated per type. It checks, in one pass over the
data present (Sebastien for the scope, author's call for the exact list):

1. The buffer start is 8-aligned and the buffer length is at least 8. In the
   preamble, `reserved` is zero, `head_size` is a multiple of 8, `8 + head_size`
   is at most the buffer length, and `head_size` covers every field of the
   reader's type that is neither optional nor has a declared default (author's
   call; such fields are in the first release, so every producer's head holds
   them).
2. Every pad byte of the reader's head is zero, when `head_size` equals the
   reader's own head size. When `head_size` is larger, the producer's head may
   place an appended field in a byte that the reader's head holds as pad, so
   `verify` does not check pad bytes (author's call).
3. Each boolean is 0 or 1; each enum value is declared; each enum set has no
   undeclared bit; each flags integer is checked only at the bits of the
   optional fields the reader knows, and the other bits are ignored; an absent
   optional's bytes are zero. A field of the reader's type whose offset plus
   size exceeds `head_size` is absent: it is not checked, and the accessor
   returns absent or the declared default.
4. The typl constraints of each value: integer ranges, string and bytes length
   bounds (`min` to `max`, counted as typl counts them), and the other generated
   typl checks, the same code the other encodings call.
5. For each descriptor: `off` is aligned for its data, `off` is at least
   `8 + head_size`, `off + len·size` (plus 1 for a string's NUL) is within the
   buffer with no overflow (checked arithmetic), and `min ≤ len ≤ max`.
6. For each string: the byte after the data is NUL, there is no NUL inside the
   data, and the data is valid UTF-8.
7. For each union: the tag names a declared arm, and `len` equals that arm's
   head size; then the arm is verified.
8. For each map: keys are strictly increasing in the order of §6.
9. Each array element, union arm and map entry is verified with the same rules.

**Not checked (Sebastien), by design:** tail bytes that no descriptor covers,
and overlap between descriptors. The buffer is read-only after the gate's copy,
so two descriptors over the same bytes cannot corrupt each other. The trade-off
is that a producer can leave unreferenced bytes in the tail; no consumer API
reads them.

**Encoder duty (Sebastien):** the encoder writes every pad byte and every byte
of an absent optional as zero, and writes no unreferenced bytes into the tail.
`verify` checks pads and absent optionals, so it costs nothing to check unused
tail capacity.

## 8. Evolution

### 8.1 Compatible changes (same type version)

typl assigns field identity by declaration order and is append-only (typl §7.4).
That is the first-fit law's precondition: an appended field never moves an
existing one.

- Appending a field at the end of a struct, when the field is optional or has a
  declared default (Sebastien). The preamble (§5.4) makes both directions work:
  - **Older reader, newer (longer) head.** The reader reads only the fields it
    knows. It ignores the head bytes past its own head size, including flag bits
    and whole flags fields that it does not know.
  - **Newer reader, older (shorter) head.** A field whose offset plus size
    exceeds the producer's `head_size` is absent, if optional, or takes its
    declared default. The generated accessors and `verify` handle this case.
- Flag bits of appended optional fields follow the first-fit law like any other
  field: the bit goes in an existing flags field if that field has a free bit,
  otherwise in a new flags field. A reader checks only the flag bits of the
  fields it knows and ignores every other bit (Sebastien). This is the single
  rule for flags in `verify` (§7); a flags integer is never rejected for a bit
  that belongs to a later field.
- Adding an enum value that declares a fallback to an existing value. A producer
  writes the fallback to a consumer that predates the value (author's call;
  agreed with the group of mechanical rules). The producer must know the
  consumer's version for this. The gate or the transport provides that with an
  attach-time version or layout-hash check, as the WebSocket transport's
  compatible-hash check at attach does (driftsys/ridl#1037). Where the fallback
  is declared is open (§15.2).
- Retiring a field with typl's existing `@deprecated` doc tag. The field and its
  type stay in the source and in the bytes; the encoder writes zero or the
  default (Sebastien).

### 8.2 Breaking changes (new type version)

Removing a field with a `reserved` tombstone (the type is gone from the source,
so its bytes cannot be kept), reordering, widening an integer across a width
boundary, changing a bound, growing a nested struct or an element type, and
removing an arm or an enum value (Sebastien).

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
- `verify`, which reads the preamble first;
- read-in-place accessors (a field read is one load; a string read returns a
  slice built from its descriptor);
- a C header with the same structs, assertions and accessor macros or inline
  functions.

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
| FlatBuffers on this path                                                                                             | kept elsewhere | more tolerant evolution and many languages, but slower to encode (4–14×) and to read, larger on the R52 (about 23 KB against 3.6 KB), offsets everywhere |
| proto3 on this path                                                                                                  | kept elsewhere | smallest bytes and best evolution, but a full parse on every read (4–10× slower verify plus decode)                                                      |
| SOME/IP serialization                                                                                                | rejected       | big-endian, packed and length-prefixed: a stream format, handled by connectors                                                                           |
| Sort all fields by size                                                                                              | rejected       | an appended field would sort into the middle and move existing fields                                                                                    |
| Sort fields by size per release segment                                                                              | rejected       | needs release history at build time; first-fit packs as well with no history                                                                             |
| Every field a 64-bit cell                                                                                            | rejected       | 2–8× larger heads for scalars, two-register compares on the R52                                                                                          |
| Typed tombstone (`reserved x : T`)                                                                                   | rejected       | a language change for one backend; `@deprecated` already keeps the field and its type                                                                    |
| Version attribute on a type, or the generator reading previous IR                                                    | rejected       | new syntax, or a build that depends on stored history; versions stay a convention                                                                        |
| 8-aligned descriptors                                                                                                | rejected       | a special case in the law with no packing gain                                                                                                           |
| Inline C union sized for the largest arm                                                                             | rejected       | every message pays for the largest arm                                                                                                                   |
| Unsorted map entries                                                                                                 | rejected       | duplicate detection becomes O(n²); sorting gives one pass and reproducible bytes                                                                         |
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

## 14. Settled after the builder check (author's calls)

- **Tail order.** The encoder writes tail data in field order, depth first (a
  string inside an array element follows that element's array block). `verify`
  does not depend on the order.
- **Languages.** podbuf generates Rust and C. TypeScript and wasm are not in
  scope; those boundaries keep FlatBuffers (ADR-0020 decision 2).
- **Floats in `verify`.** A float is checked only by the typl constraints its
  type declares; a float type with no range accepts any bit pattern, NaN
  included.
- **Empty union arm.** An arm whose type has no fields is the tag alone, with
  the descriptor `{ 0, 0 }`.
- **ridl-rt integration.** podbuf implements the same encoding contract in
  `ridl-rt` as proto3 and FlatBuffers (ADR-0020 decision 5), under the `podbuf`
  feature.
- **Verify depth.** typl has no recursive shapes (§7.3), so the nesting depth of
  a buffer is bounded by the type. The generated `verify` can be non-recursive.
- **Decode.** podbuf also generates decode into the owned domain type. It
  therefore implements the `ridl-rt` encoding contract like proto3 and
  FlatBuffers, and `ridl-rt-conformance` gets podbuf cases.
- **Generated Rust.** It reads fields with safe little-endian loads and no
  `unsafe` pointer casts. It is `no_std`, and `verify` and the in-place reads
  allocate nothing. The minimum Rust version follows `ridl-rt`'s.
- **Verify work bound.** Overlapping descriptors can make `verify` do more work
  than the buffer length suggests, because two descriptors may cover the same
  bytes (§7). The work is bounded by the maxima that the type declares, which is
  the same worst case as a buffer of maximum size. This bound is accepted.

## 15. Open questions for Sebastien

1. **How does a reader learn the producer's head size?** Decided (Sebastien,
   2026-10-24): every buffer carries a preamble with its head size (§5.4). The
   gate does not pass the producer's layout to `verify`.
2. **Where is an enum value's fallback declared?** §8.1 requires a new enum
   value to name a fallback, but typl has no syntax for it. A doc tag (like
   `@deprecated`), a new attribute (a language change), or a rule in the
   producer with no declaration — each produces different generated code.

## 16. Left to the builder

- The Rust and C names of generated items, and the accessor API shape.
- The diagnostic codes for a typl construct podbuf cannot carry, and for each
  break `ridl diff` reports.
- How the padding lint decides that a type has not been released yet.
- How the computed layout is represented in the codegen `Model`.
- The ADR-0020 amendment text and the feature rename in `ridl-rt`.
- Test strategy, including a fuzz test of `verify` and cross-target layout tests
  (AArch64, AArch32, x86-64, i686).
