# Open enums and enum sets — design

Issue: driftsys/ridl#1121. Decision source:
[ADR-0031](../decisions/ADR-0031-podbuf-projection-rules.md) decision 12
(Sebastien, 2026-10-24). Records that apply to a language surface change:
ADR-0011, ADR-0012, ADR-0015. Downstream: Tasks 13 and 14 of
`2026-10-24-podbuf-plan.md`, which start only after this change merges.

## 1. Intent

A typl `enum` or `enumset` is strict today in every backend: a value its
declaration does not name is refused. That is correct for a closed set, such as
a gear position, and wrong for a set that producers extend over time, such as a
fault code or a vehicle mode. In a chain ECU → gateway → cluster, the gateway is
often built against an older schema than the producer. With a strict enum, the
gateway refuses the new value, and the newer consumer at the end of the chain
never sees it.

This change adds the contextual keyword `open` before `enum` and `enumset`. An
open declaration accepts any value that fits its integer, gives the program a
typed "unknown" case to handle, and keeps the raw value so that re-encoding
writes it back unchanged.

## 2. Goals, non-goals, anti-goals

Goals:

- G-1. `open enum` and `open enumset` parse, format, check, and lower to the IR
  and to the codegen `Model`.
- G-2. Every backend states what an open declaration emits. The Rust and
  TypeScript faces expose an unknown case. The proto3 and FlatBuffers codecs
  accept an undeclared value of an open declaration.
- G-3. Pass-through: decoding and then re-encoding an unknown value or an
  undeclared bit writes the same raw integer, in every codec that this change
  touches.
- G-4. `ridl diff` classifies an append to an open declaration as compatible,
  narrows the `podbuf_strict_enum_append` rule to strict declarations, and
  classifies a change between strict and open.
- G-5. Before landing, an audit records how each backend and `ridl diff` treat a
  new value of a strict enum today (ADR-0031 decision 12), and the result is
  written into this design's successor records.

Non-goals:

- N-1. The podbuf codec and C header behaviour for open declarations. That is
  Task 13 of the podbuf plan. This change makes the podbuf codec compile against
  the new Rust enum shape and keeps every podbuf enum strict in behaviour (see
  §5.4).
- N-2. The `ridl diff` rule for an open value that changes the podbuf width or
  signedness. That is Task 14.
- N-3. String-backed enums (typl §17.1, v0.2).
- N-4. A verify step for strict enums in the TypeScript codec, which has none
  today (§6, finding A-3). This change records the gap as an issue; it does not
  fill it.

Anti-goals (a result that would be a failure even if it compiled):

- X-1. A value with two representations: for example `Gear::Unknown(1)` and
  `Gear::Drive` both meaning 1. Equality, hashing and `match` would then
  disagree with the wire.
- X-2. Any change in behaviour for a declaration that does not write `open`.
  Strict stays the default, and the generated code for a strict declaration is
  byte-identical to today's, except where §6's audit result requires a change
  that Sebastien approves.
- X-3. Making `open` a reserved word. A field or package named `open` must keep
  working.

## 3. Language surface

### 3.1 Grammar

Appendix E of the typl reference becomes:

```ebnf
definition  = [ "internal" ] [ "error" ] ( type_def | const_def | struct_def
            | enum_def | enumset_def | union_def ) ;
enum_def    = doc_comment? [ "open" ] "enum" CamelCase_id "{" { ( enum_value | reserved ) sep? } "}" ;
enumset_def = doc_comment? [ "open" ] "enumset" CamelCase_id "{" { enumset_bit sep? } "}"
            | doc_comment? [ "open" ] "enumset" CamelCase_id ":" type_ref ;
```

- The order is fixed: `internal`, then `error`, then `open`, then the keyword.
  `ridl-fmt` prints that order.
- `open` is recognised only when the next token is `enum` or `enumset`. In every
  other position it is an ordinary identifier. The lexer keeps producing `Ident`
  for it; the parser recognises it by text, the way the lexer already treats
  `match` (`crates/ridl-syntax/src/lexer.rs`).
- `open` is not added to `TYPL_KEYWORDS` or to the family reserved registry
  (typl §1.4), so the `reserved-set` line of `freeze/keywords.txt` does not
  change. The keywords dump gains a line that names `open` as a contextual
  keyword, if the dumper has a form for one; adding a line passes the freeze
  check.
- `error open enum` is allowed: an error enum may be open.
- A derived enum set (`open enumset Flags : Warning`) has its own `open` flag,
  independent of the backing enum's flag (author's call). The bits are derived;
  the acceptance rule is the set's own.

### 3.2 Semantics (typl §8 and §9)

- **Strict** (the default). A value that the declaration does not name is
  invalid. For an enum set, a bit that the declaration does not name is invalid.
- **Open.** Any value that fits the declaration's integer is valid. For an enum,
  a value the declaration does not name is the unknown case and carries the raw
  integer. For an enum set, undeclared bits are kept; the named flags are the
  declared bits that are set.
- **The integer an open enum accepts.** On the wire it is whatever the encoding
  carries: int32 for proto3, `long` for FlatBuffers, the inferred width for
  podbuf (Task 13). In the typl language layer it is `int64`, as for every enum
  (typl §9.3 for sets).
- **Init value.** Unchanged: the 0 member, else the lowest declared value. An
  unknown value is never an init value.
- **Evolution rules** (typl §8 and §9, and the book's
  `evolving-an-interface.md`):
  - Appending a value or bit to an open declaration is compatible, except where
    it changes the podbuf width or signedness (Task 14).
  - Appending a value or bit to a strict declaration is breaking for every
    encoding (§6, Q-1, decided).
  - Removing or renumbering a value is breaking, open or strict, as today.
  - Strict → open is compatible on the wire, because every value that was valid
    stays valid. It is a source break for Rust consumers, because the generated
    enum gains a variant and `#[non_exhaustive]`. `ridl diff` reports it as the
    new category `enum_opened`, verdict compatible, and the book says that Rust
    callers must add a wildcard arm.
  - Open → strict is breaking: a producer may already send a value that the
    narrowed reader refuses. New category `enum_closed`, verdict breaking.

### 3.3 Checks

No new diagnostic is required by the decision. One is added (author's call):

- An open enum with no declared value draws a warning, because no value has a
  name and every value is unknown. Code allocated by the builder from the TYPL
  warning range under ADR-0024, with a book entry in `docs/book/lints.md`. If
  the builder finds that an empty strict enum is already diagnosed, the open
  case reuses that code instead.

## 4. IR and codegen `Model`

- `ridl/ir/v2/ir.proto`: `EnumDef.open = 3` (bool) and `EnumSetDef.open = 4`
  (bool). Append-only; an absent field reads as `false`, which is strict, so
  every existing IR snapshot keeps its meaning.
- `ridl/codegen/v1/model.proto`: `Enum.open = 5` and `EnumSet.open = 5` (bool).
  The builder confirms the next free numbers.
- The catalog hash (`crates/ridl-ir/src/catalog_hash.rs`) includes the flag,
  because it changes which payloads a reader accepts. A strict declaration
  hashes as today, so existing hashes do not change: the flag contributes only
  when it is `true` (author's call; the builder confirms that the hash
  construction allows an optional contribution without changing other hashes).
- The catalog descriptor carries no typl enum today
  (`crates/ridl-descriptor/src/lower.rs`) and is not changed.
- The ADR-0029 freeze surfaces that dump the IR and `Model` schemas gain the new
  fields; adding lines passes the freeze check.

## 5. Backends

### 5.1 Rust domain types

Open enum (the shape follows the AWS SDK for Rust / Smithy pattern):

```rust
#[derive(...)]            // the same derive set as a strict enum
#[non_exhaustive]
pub enum DtcStatus {
    Inactive,
    Active,
    Unknown(UnknownDtcStatus),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct UnknownDtcStatus(i64);   // field private: only the codec builds one

impl UnknownDtcStatus {
    pub const fn raw(self) -> i64 { self.0 }
}

impl From<i64> for DtcStatus { /* declared value → its variant, else Unknown */ }
impl From<DtcStatus> for i64 { /* variant → its value, Unknown → raw */ }
```

- An open enum has no `#[repr(i64)]` and no explicit discriminants, because a
  variant with a field and an implicit discriminant could collide with a
  declared value. Conversion goes through the two `From` impls. Any generated
  code that converts a strict enum with `as i64` stays as it is for strict
  enums; for open enums it calls `i64::from`.
- `From<i64>` is total and is the only way to construct `Unknown`. It returns
  the declared variant for every declared value, so X-1 holds by construction.
- `TryFrom<i64>` is not emitted for an open enum (`From` covers it).
- The `Unknown` type name is `Unknown<Name>`. It goes through the pinned name
  transform (ADR-0016) and the name-collision check (RIDL-149 and its
  neighbours): a user declaration named `Unknown<Name>` in the same package, or
  a value whose PascalCase spelling is `Unknown`, collides and is refused with
  the existing collision diagnostic or a new one the builder allocates.
- `Default` stays the init member (§3.2).
- `#[non_exhaustive]` makes a downstream crate keep a wildcard arm. Inside the
  generated crate, an exhaustive `match` still compiles; the generated code
  matches `Unknown` explicitly.

Open enum set: the existing `#[repr(transparent)] struct Flags(i64)` stays.
`From<i64>` replaces `TryFrom<i64>` and keeps every bit. A new method
`unknown_bits(self) -> i64` returns the bits outside `DECLARED_MASK`. The whole
integer is read, as today, with the existing `From<Flags> for i64`; there are no
named-flag accessors today (only the bit constants), and this change adds none.
`#[non_exhaustive]` does not apply to a struct with private fields and is not
emitted.

The generated documentation (ADR-0028) states on each open declaration that a
value outside the named ones is valid and is read as `Unknown`.

### 5.2 Rust codecs (proto3 and FlatBuffers)

- Verify: a strict declaration keeps the `TryFrom` check. An open declaration
  has no value check.
- Decode: an open enum decodes with `From<i64>`. A strict enum keeps today's
  path.
- Encode: an open enum encodes `i64::from(value)`; `Unknown` writes its raw
  integer (G-3).
- proto3 carries enums as int32. An `Unknown` whose raw integer does not fit
  int32 can only come from another encoding (transcoding); the encoder refuses
  it with the existing contract error for an out-of-range value (the builder
  names the rule). The same check applies to FlatBuffers only if its `long`
  cannot hold an `i64`, which it can, so FlatBuffers needs none.

### 5.3 TypeScript

- An open enum keeps its `export enum` and adds a type
  `type DtcStatusValue = DtcStatus | { readonly unknown: number }` (author's
  call on the name: `<Name>Value`). Fields of an open enum type are typed with
  `<Name>Value`.
- The codec decodes a declared value to the enum member and any other value to
  `{ unknown: raw }`, and encodes `{ unknown: raw }` back to `raw` (G-3).
- An open enum set keeps its branded type (`number` up to 32 bits, `bigint` for
  a 64-bit set); the codec already keeps every bit (an unchecked cast today), so
  only the `<Name>Bits` helper gains an `unknownBits` function.
- **Unknown values outside the safe-integer range (author's call).** The
  generator already refuses a declared value beyond `Number.MAX_SAFE_INTEGER`
  (`crates/ridl-backend-ts/src/lib.rs`), but no decode-time check exists. The
  codec refuses to decode an unknown value of an open enum outside
  `±Number.MAX_SAFE_INTEGER` with the `variant` rule, instead of rounding it.
  G-3 therefore holds in TypeScript for safe integers only. proto3 (int32) never
  reaches the limit; FlatBuffers (`long`) and podbuf can.
- **Duplicate representations in TypeScript (author's call).** X-1 is enforced
  by construction in Rust only. TypeScript cannot stop a caller from writing
  `{ unknown: 1 }` when `Active = 1`. The codec always decodes a declared value
  to the enum member, so decoded values are canonical; an encoder given
  `{ unknown: 1 }` writes 1, which decodes as `Active`. The generated
  documentation says so.

### 5.4 Schema backends and podbuf

- proto3 (`ridl-backend-proto`): no change. proto3 enums are open on the wire
  and in protoc-generated code already. A comment on the emitted enum says
  whether the typl declaration is strict or open (author's call), so a reader of
  the `.proto` file knows which rule the RIDL codecs apply.
- FlatBuffers (`ridl-backend-flatbuffers`): no change to the schema type
  (`long`). The same comment as proto3.
- podbuf: the Rust podbuf codec and the C header keep strict behaviour for every
  enum until Task 13. The podbuf codec must compile against the open enum shape
  (no `#[repr(i64)]`, `From` instead of `TryFrom`). Until Task 13 lands, an open
  declaration in a package whose encodings list `podbuf` is refused at build
  time with a diagnostic that names Task 13's issue (Q-2, decided).

### 5.5 Codegen plugins

The `Model` fields of §4 are the plugin interface. `ridlc-gen-model` passes them
through unchanged; its parity test gains a fixture with an open enum.

## 6. The audit (G-5) and the questions it raises

What the survey found today (2026-11-01, main at `e0d8abb9`):

- A-1. `ridl diff` without `--encoding podbuf` reports an appended enum value or
  enum-set bit as **compatible** (`DeclAdded` with `appended_slot`, in
  `crates/ridl-diff/src/classify.rs`). Under `--encoding podbuf` it reports
  `podbuf_strict_enum_append`, breaking.
- A-2. Every Rust codec (proto3, FlatBuffers, podbuf) refuses an undeclared
  value at verify, through `TryFrom` (`Rule::Variant`). So an older Rust reader
  refuses a payload from a producer that appended a value — the case A-1 calls
  compatible. The Rust backend's own comment on `emit_enum_set` records this
  tension.
- A-3. The TypeScript codec casts without a check (`Number(x) as t.Enum`), so a
  TypeScript reader accepts every value, strict or not. The wasm codec path
  verifies in Rust and refuses.
- A-4. Rust decode after a failed `TryFrom` falls back to the first variant
  (`unwrap_or`); this path runs only after verify, so it is not reached for a
  verified payload.

**Q-1 (Sebastien).** A-1 and A-2 disagree today for every strict enum:
`ridl
diff` says an append is compatible, and an older Rust reader refuses the
new value. Decision 12 says that for a strict enum "adding a value is a break".
The choices:

- (a) Make an append to a strict enum or enum set **breaking** for every
  encoding. This matches decision 12 and A-2. Every existing workspace that
  appends to an enum gets a breaking verdict until the author marks the enum
  `open`. That is a visible change to `ridl diff` verdicts; it fixes a verdict
  that is wrong today.
- (b) Keep it compatible outside podbuf, and document that a strict enum append
  is compatible only for readers that tolerate it. This keeps today's verdicts
  and leaves A-2's refusals unreported.

**Decided (Sebastien, 2026-11-01): (a).** The reason: a verdict of compatible
that a reader then refuses is the failure `ridl diff` exists to prevent. If (a)
is chosen, the `podbuf_strict_enum_append` category becomes redundant with the
generic rule; the builder either keeps it as the podbuf-specific heading or
retires it, and records which.

**Q-2 (Sebastien).** Before Task 13, what happens to an `open` declaration in a
package that lists `podbuf`? **Decided (Sebastien, 2026-11-01):** refuse at
build time, so that no podbuf reader silently treats an open enum as strict.

## 7. Documents amended

- typl reference §1.4 (contextual keywords), §8, §9, Appendix E.
- `docs/book/evolving-an-interface.md` (append rules, `enum_opened`,
  `enum_closed`), `docs/book/generated-code.md` (the open shapes and the
  `TryFrom` / `verify` table), `docs/book/lints.md` if §3.3's warning is added.
- ADR-0031 decision 12: a dated note that the keyword is built and the shape of
  the unknown variant. The ADR's own `## Status` gets the amendment line.
- `docs/design/podbuf-codec.md`: the note on strict enums points to Task 13.
- The podbuf plan's Task 13 tests use `DtcStatus::Unknown(7)`; with an opaque
  payload they become `DtcStatus::Unknown(u) if u.raw() == 7`. Task 13 is
  amended when it starts, not by this change.

## 8. Alternatives considered

- **`#[non_exhaustive]` alone, no `Unknown` variant.** A reader would have no
  way to hold a value it does not know, so it would have to refuse it. Rejected:
  it does not deliver the decision.
- **`Unknown(i64)` with a public payload.** Simpler, but `Unknown(1)` and
  `Drive` could both mean 1 (X-1). Rejected.
- **A newtype struct with associated constants** (FlatBuffers Rust style):
  `struct DtcStatus(i64)` with `const ACTIVE`. Every value has one
  representation and the type is open by construction. Rejected: `match` on it
  is not checked for exhaustiveness, so a reader loses the compiler's help on
  the named values, and a strict enum and an open one would look unrelated.
- **`Result<Enum, NotInSchema>`** (Cap'n Proto style) from the accessor. It does
  not round-trip: the decoded struct has to store something, and storing the
  `Result` makes every field access unwrap.
- **An attribute `[open]` or a doc tag**, recorded in the archived podbuf design
  as rejected: the openness changes the wire contract and belongs in the grammar
  (ADR-0031 decision 12).

## 9. Left to the builder

- Next free field numbers in `ir.proto` and `model.proto`.
- The diagnostic code for §3.3's warning and for the `Unknown<Name>` collision,
  if the existing collision code does not cover it.
- Whether `freeze/keywords.txt` has a form for a contextual keyword, and the
  line it adds.
- Test placement, following the survey's map of the existing enum tests.
- The exact text of the proto3 and FlatBuffers comments.
