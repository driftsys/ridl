# The podbuf payload codec

The generated `Payload<Podbuf>` implementation a package carries, the read-in-
place C header, and the layout both read, as built. The binding choices are
[ADR-0031](../decisions/ADR-0031-podbuf-projection-rules.md) (the projection:
what the bytes are, what `verify` checks, how a type evolves),
[ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
decisions 1, 3, 5, 8 and 9 (the encoding's place in the matrix, the domain types
that carry no layout, the cargo feature, the codegen model) and
[ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decisions 7 and
8 (`Encoded.bytes`, the encoding features). The reasoning and the measurements
behind the encoding's shape are the archived design
[`../archive/2026-10-24-podbuf-design.md`](../archive/2026-10-24-podbuf-design.md).

**The codec is emitted on request.** `ridl build` emits the Rust codec, with the
descriptors and the face, only when the manifest's `[backend.rust] encodings`
names `podbuf`; the C header is written by `ridl build --emit podbuf`
(`<base>.h`). Every payload also carries its own zeroed buffer,
`Payload<Podbuf>::BUFFER`, an `Aligned` array of `MAX_SIZE` bytes, because a
podbuf buffer starts on an 8-byte boundary.

## Where the code is

| What                                                                     | Where                                          |
| ------------------------------------------------------------------------ | ---------------------------------------------- |
| The layout law: placements, flags, descriptors, `elem_size`, enum widths | `crates/ridl-ir/src/projection/podbuf.rs`      |
| The size bound (`max_size`)                                              | `crates/ridl-ir/src/projection/podbuf.rs`      |
| The size state of a payload, as the sizer reports it                     | `crates/ridl-ir/src/projection/size/podbuf.rs` |
| The layout carried in the codegen model (`Model.podbuf`)                 | `crates/ridl-ir/src/codegen/podbuf.rs`         |
| The reading and writing the emitted code calls                           | `crates/ridl-rt/src/podbuf.rs`                 |
| The Rust codec emitter                                                   | `crates/ridl-backend-rust/src/codec_podbuf.rs` |
| The `Sizes<Podbuf>` table and the aligned buffers                        | `crates/ridl-backend-rust/src/descriptors.rs`  |
| The claim table entries for the generated names                          | `crates/ridl-backend-rust/src/claims.rs`       |
| The C header emitter                                                     | `crates/ridl-backend-podbuf/src/lib.rs`        |
| RIDL-153 to RIDL-155, the build-time errors                              | `crates/ridlc/src/design_lints/podbuf.rs`      |
| RIDL-156 `podbuf-padding-order`                                          | `crates/ridl/src/podbuf_padding.rs`            |
| `podbuf_inline_append`, the inline-use rule of `ridl diff`               | `crates/ridl-diff/src/walk.rs`                 |
| The `encodings` list (`WireEncoding`) the entry points take              | `crates/ridl-backend-rust/src/lib.rs`          |
| The marker `Podbuf` and the `Payload`, `View` and `Malformed` it shares  | `crates/ridl-rt/src/encoding.rs`, `payload.rs` |

## The layout is computed once, outside the emitters

A type's head, its placements, its flags fields, its pads, the form of each
field and the size bound are computed in `ridl_ir::projection::podbuf` and read
by every consumer: the Rust codec, the C header, the sizer, the
`podbuf_inline_append` rule of `ridl diff`, the padding lint and the build-time
errors. They read it from the codegen `Model` (`Model.podbuf`,
`PodbufProjection`), not from the IR, so that an out-of-tree plugin reads the
same numbers (ADR-0020 decisions 8 and 9; the model's layout is described in
[`codegen-plugins.md`](codegen-plugins.md), "The podbuf projection"). Nothing in
the projection spells a Rust or C name.

**The model carries two additions beyond the heads.** `Declaration.podbuf` is
the size state of a declaration as a payload root, which becomes `MAX_SIZE`.
`PodbufProjection.boxes` holds the one-field head of each named scalar, enum or
enum set that is a payload root, because interfaces use such types as payloads
and a plugin may not compute a layout or a bound itself. A box is in no index of
`heads`.

`ridl_rt::podbuf` decides no layout. It is handed sizes, offsets and alignments.

## What is emitted, and where

For each struct, union and named induced tuple, the Rust codec emits:

- `TPodbufHead`, the `#[repr(C, align(8))]` struct, with every pad written as a
  `_pad<offset>` field, and `const` assertions on `size_of` and `offset_of!` for
  the head and each field. A toolchain that lays the struct out differently
  fails the build, on any target.
- `TPodbufView<'a>`, the read-in-place view, with one accessor per field. A
  union has `TPodbufArm<'a>` beside it, one variant per arm.
- `impl T { __ridl_podbuf_encode, __ridl_podbuf_encode_at,
  __ridl_podbuf_verify, __ridl_podbuf_verify_at, __ridl_podbuf_decode }`.
  The `_at` forms work at a given head offset, which is how a nested struct, an
  array element and a union arm are written and checked in place.
- `impl Payload<Podbuf> for T`, with `MAX_SIZE`, an `Aligned` `Buffer` and
  `BUFFER`, `View<'a> = TPodbufView<'a>`, and `encode`, `verify` and `decode`
  over the functions above.

A named scalar, an enum and an enum set used as a payload root has a view with
one accessor, `value`, the `Payload<Podbuf>` implementation, and no head struct.
The claim table reserves every generated name, so a ridl name cannot collide
with one.

The generated code holds no `unsafe`. The only access to the buffer is through
`ridl_rt::podbuf::Region`, which reads a little-endian value over a
bounds-checked slice and returns 0 past the size the producer wrote.

## `ridl_rt::podbuf`

The module is behind the `podbuf` feature, takes no dependency, allocates
nothing and has no `unsafe`. On a big-endian target it does not compile.

| Item                   | What it does                                                                                               |
| ---------------------- | ---------------------------------------------------------------------------------------------------------- |
| `Preamble`             | reads the 8-byte preamble; refuses an unaligned buffer or a bad `head_size`                                |
| `Region`               | a window over a head, an element or an arm; every load returns 0 at or past the size the producer wrote    |
| `Desc`, `ArrayDesc`    | the two descriptor layouts, with `is_zero`                                                                 |
| `tail_range`           | the bounds and alignment check of a descriptor's data, by checked arithmetic                               |
| `text`, `scalar_count` | the string checks: the closing NUL, no inner NUL, valid UTF-8, and the count of scalar values              |
| `Writer`               | fills the caller's slice and zeroes every byte it reserves; the tail blocks are written in placement order |
| `Aligned<T>`           | gives a byte array the 8-byte alignment of a buffer                                                        |

The shared error variants are `Malformed::Preamble`, `Stride`, `Terminator` and
`KeyOrder`, and `EncodeError::Unaligned` and `EncodeError::Layout { at }`
([the `ridl-rt` record](ridl-rt.md)). A string that is not UTF-8 reuses
`Malformed::Utf8`.

## `encode`, `verify`, `decode`, `MAX_SIZE`

**`encode`** reserves the head with `Writer::reserve_head`, stores each field,
and appends each tail block in the order of the head's placements, depth first.
The writer zero-fills every byte it reserves, so pads, the preamble's `reserved`
word and an absent optional's bytes are zero without a separate write. The
encoder sorts the entries of a map by key; it refuses with
`EncodeError::Contract` a string that holds U+0000 (`Pattern`), a duplicate map
key (`Unique`) and a NaN map key (`Range`), a collection outside its bounds
(`Length`) and a value outside its typl constraints. A deprecated field is
written as its init value. A buffer that does not start on an 8-byte boundary
gives `EncodeError::Unaligned`; a store outside the bytes written gives
`EncodeError::Layout`.

**`verify`** is a call tree, not a worklist: each head type has its own
`__ridl_podbuf_verify_at(buf, at, size, tail)`, and a nested head, an array
element, a union arm or a map entry is checked by the function of its type. typl
has no recursive shape, so the depth is bounded by the type. It allocates
nothing. A boolean above 1 is `Contract` with the rule `Range`; an absent
optional whose bytes are not zero is `Contract` with the rule `Variant`; the
structure faults are `Malformed` variants. `Payload<Podbuf>::verify` first
refuses a buffer longer than the reader's own `MAX_SIZE` with
`Malformed::TooLarge`
([ADR-0031](../decisions/ADR-0031-podbuf-projection-rules.md), open item 2). The
full list of checks, and what is not checked, is ADR-0031 decision 8.

**`decode`** builds the owned domain value from a view that `verify` returned,
and allocates for `String`, `Vec` and maps as the other codecs do.

**The view.** The accessors read through `Region`. A scalar read is one bounds
check and one load. A `string` accessor returns a `&str` built by
`core::str::from_utf8`, so each read scans the string once: the check cannot
fail after `verify`, and it is the price of generated code with no `unsafe`.

**`MAX_SIZE`** is the preamble, the head and the largest tail, computed by
`ridl_ir::projection::podbuf::max_size`, with every optional present and every
bound at its maximum, and with the worst-case alignment gap before each aligned
tail block. ADR-0030 decision 3 caps it at the payload ceiling.

## The face names this codec through `Payload<Podbuf>` and `Sizes<Podbuf>`

The generated interaction face is generic over its port's encoding. With
`encodings` naming `podbuf`, the descriptors module emits a `Sizes<Podbuf>`
table for each interface, built from each payload's
`<T as Payload<Podbuf>>::MAX_SIZE`; the claim and event buffers of the table are
`Aligned`. A payload with no finite podbuf bound is refused or skipped the same
way as under the other encodings, so the table has no `None`
([the interaction face record](interaction-face.md)). A port whose encoding the
package did not generate has no `Sizes<Podbuf>` table, and binding a face over
it is refused at the binding by the `Sizes` bound.

## The C header

`ridl-backend-podbuf` writes one header per package. It holds the preamble and
descriptor structs and the helper functions, each behind its own guard so that
the headers of two packages can be included together; one `struct` per head,
with `_Alignas(8)` on its first member and every gap as a `_padN` member;
`_Static_assert` checks on `sizeof` and `offsetof` for every head and field; the
constants of the enums, enum sets and union tags; and inline accessor functions.
`<owner>_<field>_at` reads a field from a head of a size the caller gives and
answers 0 (or `NULL`) for a field past it; the head of a declaration also has
`<owner>_<field>`, which reads a whole buffer through the preamble's
`head_size`. A foreign struct, tuple or union that a field lays out inline is
defined again behind a `<PKG>_<TYPE>_HEAD_DEFINED` guard. The header carries
`#error` on a big-endian target, and has no `verify`: the gate is Rust.

## Determinism

The bytes of a value are fixed by the layout law, the sorted map entries and the
zero fill. The same value always encodes to the same bytes, and the tail order
is the order of the head's placements, depth first.

## Conformance

The encoding has no second implementation, so the oracle is the law itself.

- `crates/ridl-backend-rust/tests/podbuf_conformance.rs` holds a table of values
  with the buffers written by hand from the law, field by field, and the codec
  is held to those bytes. It also runs every truncation and every single-byte
  replacement of valid buffers, and a seeded random mutation pass, through
  `verify` and the accessors, and requires no panic and no out-of-bounds read.
  It compiles two versions of the same type and reads each version's bytes with
  the other's reader, in both directions, for an appended field, a longer and a
  shorter stride, and a head that inlines a struct of another package. It
  compares the numbers in the emitted `const` assertions with
  `ridl_ir::projection::podbuf` computed directly from the IR, and checks the
  generated module for `thumbv7em-none-eabihf`.
- `podbuf_encode.rs`, `podbuf_verify.rs` and `podbuf_roundtrip.rs` cover the
  encoder duty, each rule of `verify`, the `Payload<Podbuf>` implementation and
  the size tables.
- `crates/ridl-backend-podbuf/tests/c_compile.rs` compiles the headers as C11
  with the host compiler and no warning, runs the `_Static_assert` checks with
  `clang` for i686, AArch64 and ARMv8-R, and reads a hand-written buffer through
  the accessors. A skip with no compiler becomes a failure under
  `RIDL_REQUIRE_CC=1`.
- `just demo` checks the generated cabin crates for `i686-unknown-linux-gnu` and
  `aarch64-unknown-linux-gnu`, so the layout assertions are evaluated for a
  32-bit and a 64-bit target, and runs one value through the cabin crate's
  codec. `just compat-check` tests `ridl-rt` with every feature at the minimum
  Rust version, and `just wasm-check` checks `ridl-rt` with every feature, and
  `ridl-backend-podbuf`, for `wasm32`.
- `ridl-rt-conformance` holds the port contract tests, which do not depend on an
  encoding, so it has no podbuf case.

## Known gaps

| Gap                                                                                                                              | Issue                         |
| -------------------------------------------------------------------------------------------------------------------------------- | ----------------------------- |
| `ridl build` and `ridl check --baseline` do not pass the manifest's `[backend.<name>] encodings` to the diff                     | driftsys/ridl#1095            |
| `open` enums and enum sets are not built; every enum is strict (ADR-0031 decision 12)                                            | the typl change it depends on |
| `Payload<Podbuf>::verify` refuses a buffer longer than its own `MAX_SIZE`, which a compatible append can raise (ADR-0031 item 2) | open item of the record       |
| The C header has no reader for a payload root that is a named scalar, an enum or an enum set                                     | driftsys/ridl#1096            |
| A string that holds U+0000 cannot be encoded                                                                                     | a limit of the encoding       |
| TypeScript and wasm have no podbuf codec; those boundaries keep FlatBuffers                                                      | ADR-0020 decision 2           |

## Trace

- Roadmap: [`../ROADMAP.md`](../ROADMAP.md)
- Decisions: [ADR-0031](../decisions/ADR-0031-podbuf-projection-rules.md),
  [ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
  decisions 1, 3, 5, 8 and 9,
  [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decisions 7
  and 8, [ADR-0030](../decisions/ADR-0030-transfer-and-bound-limits.md) decision
  3
- Design records: [`ridl-rt.md`](ridl-rt.md),
  [`interaction-face.md`](interaction-face.md),
  [`codegen-plugins.md`](codegen-plugins.md),
  [`flatbuffers-codec.md`](flatbuffers-codec.md)
- The reasoning and the measurements:
  [`../archive/2026-10-24-podbuf-design.md`](../archive/2026-10-24-podbuf-design.md)
