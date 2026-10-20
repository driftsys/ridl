# The proto3 payload codec — design

Satisfies: driftsys/ridl#264 (roadmap story E11.8). Related: driftsys/ridl#472
(the absence rule), driftsys/ridl#598 (the append rule of `ridl diff`),
driftsys/ridl#690 (sharing the proto3 refusal rules), driftsys/ridl#317 (the
`repr(C)` codec, which this design does not touch).

Binding records: ADR-0017 (the proto3 projection, decisions 6 to 8 especially),
ADR-0018 decisions 3 to 5 (proto3 on the network, the codec generated rather
than delegated, byte-level conformance), ADR-0019 decision 9 and its 2026-10-12
review rulings 6, 7 and 18, ADR-0020 decisions 5 and 9, ADR-0021 decisions 7, 8
and its open item 3, ADR-0023, ADR-0029 (the frozen surfaces). The pattern this
design follows is the FlatBuffers codec, `docs/design/flatbuffers-codec.md`.

No human answered questions for this design: the request is the roadmap row, the
issue, and the records above. Every decision below is the driver's call unless
it cites a record, and each one says what it costs if it is wrong.

## 1. Intent

A package whose `[backend.rust] encodings` names `proto3` gets a generated
`Payload<Proto3>` implementation for every type that can be a payload, and a
`Sizes<Proto3>` table for every interface, so that a face bound to a port whose
encoding is `Proto3` compiles and runs. The bytes this codec writes are the
bytes of the `.proto` that `ridl build --emit proto` writes for the same
package, so a third party that generates a reader from that `.proto` reads them,
and this codec reads what that third party writes.

**Why now.** proto3 is the encoding for every boundary that serializes into a
stream (ADR-0018 decision 3, ADR-0020 decision 2): a WebSocket, a link to a
different machine, an off-board link. `ridl-transport-ws` already carries an
`attach` with the proto3 encoding tag, the deployment rule already derives
`proto3` for every cross-machine link (`ridl_ir::codegen::deployment`), and
RSDL-807 warns when a link carries `proto3` and the manifest does not name it.
Today the Rust backend refuses `proto3` in `encodings` with an error naming
#264, so none of that can be exercised by a generated program.

**What success looks like.** "Our bytes parse there and its bytes parse here",
measured against `prost` code generated through `protox` from the `.proto` this
toolchain emits, with no `protoc` anywhere, and with `prost` reachable from
neither `ridl-rt` nor a generated package.

## 2. Goals

- G-1. `encodings = ["proto3"]` (alone or beside `flatbuffers`) builds; the
  generated crate carries `Payload<Proto3>` for every declaration that projects
  a type, `Sizes<Proto3>` for every interface, and enables the `ridl-rt` feature
  `proto3`.
- G-2. The bytes agree with the emitted `.proto` and with `prost`: for every
  value of every shape the fixture holds, this codec's bytes equal `prost`'s
  bytes for the same value (one listed exception, D-6), `prost` decodes this
  codec's bytes to that value, and this codec decodes `prost`'s bytes to it.
- G-3. The reader rule of #472 in proto3 terms: an absent field reads as the
  proto3 default when that default is a legal value of the field's type, and is
  refused with `MissingRequired` when it is not.
- G-4. The emitted code holds to everything a generated crate already holds to:
  `no_std` with no allocator except where the domain type allocates, Rust 1.83
  and edition 2021, `wasm32` and `thumbv7em-none-eabihf`, clippy and rustdoc
  clean under `just demo`.
- G-5. The guard: `ridl-rt` and every generated package reach no `prost`,
  `prost-build`, `prost-reflect`, `prost-types` or `protox` crate, at any edge
  kind.

## 3. Non-goals

- N-1. No change to the `.proto` the proto backend emits. The `wire` dump is
  frozen (ADR-0029); this codec follows the schema, not the other way round. If
  the codec needs a schema change, that is an escalation, not a task.
- N-2. No payload that has no root message in the emitted `.proto`: a stream,
  and any shape the face still refuses. (A call of zero or several parameters
  and a tuple reply do have one since driftsys/ridl#713 — the induced argument
  or reply message — and are in scope, D-1. ADR-0019 review ruling 14's "the
  face refuses it today" predates that change and is amended, D-21.)
- N-3. No WebSocket client (`ws::connect`), and no change to
  `ridl-transport-ws`; its server already takes the encoding from its backend
  port.
- N-4. No zero-copy field accessors on the proto3 view (D-11).
- N-5. No preservation of unknown fields (D-8).
- N-6. No change to the FlatBuffers codec's output. The frozen `generated-crate`
  copies name `flatbuffers` only, and they must regenerate byte for byte.
- N-7. No performance comparison against FlatBuffers. ADR-0020 open item 7
  measures that after this lands; it is not this story.

**Anti-goals** (what would make this a failure even if every test passed):

- A codec that agrees with itself and not with the schema — a round trip through
  this codec alone proves nothing about the wire (the FlatBuffers record
  measured exactly this: a shifted discriminant passed every self round trip).
- A codec that agrees with `prost` only because both were fed the same mistake —
  so the oracle is generated from the emitted `.proto`, never from a schema
  written for the test, and a structural drift test compares the codec's own
  field table with the descriptor `protox` compiles from that `.proto` (D-17).
- `prost` in a generated crate's dependency graph, even as a dev-dependency.

## 4. Constraints

- C-1. The emitted code compiles with Rust 1.83 at edition 2021, without `std`
  and without `alloc` unless a domain type the code builds already owns a `Vec`
  or a `String` (the generated `lib.rs` aliases `alloc` as `std`).
- C-2. `ridl-rt` takes no dependency in any feature combination (ADR-0021
  decision 8) and stays `#![forbid(unsafe_code)]`.
- C-3. A backend is an executable, not a library other crates link (ADR-0020
  decision 9): `ridl-backend-rust` may reach `ridl-backend-proto`,
  `prost-build`, `protox` and `prost-reflect` only as dev-dependencies. (`prost`
  itself is already in its normal closure through `ridl-ir`; the boundary that
  matters is `ridl-rt` and the generated crate.)
- C-4. The plugin path (`Backend::generate` over a `CodegenRequest`) has the
  codegen `Model` and no IR package, so every fact the codec reads must be in
  the `Model`.
- C-5. `protoc` stays out of every gate (ADR-0006 decision 3).
- C-6. Shipped docs and source name no story id (`just story-id-check`).

## 5. Decisions

### D-1. What is emitted, and when

When `encodings` names `proto3`, `generate_face_with` and `generate_pipeline`
emit, for every declaration that projects a type — a struct, a union, a named
scalar, an enum and an enum set (ADR-0017 decision 6's root rule) — one
`impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3> for T`, and for
every interface one
`impl ::ridl_rt::contract::Sizes<::ridl_rt::encoding::Proto3>`. A constant gets
nothing. `WireEncoding` gains `Proto3` (it is `#[non_exhaustive]`, so this is
additive); `parse("proto3")` answers it; the backend contract stops refusing
`proto3`; `ridlc` already renders one `ridl-rt` feature per list entry, so the
manifest gains `"proto3"` with no change there. With both names, both codecs are
emitted side by side; the face is already generic over the port's encoding and
changes in no line.

A struct's root is its message, a union's its message with `oneof value`, and a
named scalar, an enum and an enum set are rooted in
`message <Name>Box { <type> value = 1; }`. The box's `value` is not `optional`
(ADR-0017 decision 6).

**An induced interaction tuple is a root too**, as it is for the FlatBuffers
codec (driftsys/ridl#713): the argument struct of a call with zero or several
parameters, and the reply struct of a query whose reply is a tuple, cross the
wire on their own, the face sends them, and the size table names their
`MAX_SIZE`. Each gets a `Payload<Proto3>`. Its message in the `.proto` is the
one `wire::with_interaction_tuples(…, Schema::Proto3)` appends, named by
`InducedName.wire`, with positional fields `field_1`, `field_2`, … numbered from
1 (`emit_induced_tuple`); the Rust struct is `InducedName.rust`. Its `MAX_SIZE`
is the bound the `Model` already carries for that payload —
`CommandShape.request_sizes.proto3`, `QueryShape.request_sizes.proto3` or
`QueryShape.reply_sizes.proto3`, which the shared sizer computes with
`tuple_state` — so no protocol field is needed for it (D-4). A tuple reached
only from a field is not a root; its message is encoded inside its owner.

Without this, `Sizes<Proto3>` would name a `Payload<Proto3>` that does not exist
for every interface with such a call, and the `veh-cluster` corpus and the
interaction-face fixture, which both have them, would not compile.

Cost if wrong: none beyond the work; this is what the roadmap row asks for.

### D-2. The wire mapping, and where it must agree with the `.proto`

Every row is what `crates/ridl-backend-proto/src/lib.rs` emits for the same IR.
The codec must agree with each row; D-3 says how agreement is kept and D-17 how
it is tested.

| typl / ridl shape                     | `.proto` emitted                                      | Wire                                                                        |
| ------------------------------------- | ----------------------------------------------------- | --------------------------------------------------------------------------- |
| integer of width u8, u16, u32         | `uint32`                                              | varint                                                                      |
| integer of width u64                  | `uint64`                                              | varint                                                                      |
| integer of width i8, i16, i32         | `sint32`                                              | zigzag varint                                                               |
| integer of width i64, bare `integer`  | `sint64`                                              | zigzag varint                                                               |
| float f32 / f64, bare `float`         | `float` / `double`                                    | fixed32 / fixed64, little-endian                                            |
| boolean                               | `bool`                                                | varint 0 or 1                                                               |
| string                                | `string`                                              | length-delimited UTF-8                                                      |
| bytes                                 | `bytes`                                               | length-delimited                                                            |
| named scalar at a field               | its backing scalar, inlined                           | as its backing                                                              |
| enum                                  | `enum`, values prefixed, zero first or `_UNSPECIFIED` | varint of the value as int32 (a negative value is ten bytes, sign-extended) |
| enum set of width W                   | the scalar of width W, inlined                        | as that scalar                                                              |
| struct                                | `message`, field number = ordinal                     | length-delimited                                                            |
| `?` on a scalar, enum, string, struct | `optional`                                            | written when present, whatever its value                                    |
| array of a numeric, bool or enum      | `repeated`                                            | packed: one record, length-delimited run of values                          |
| array of string, bytes, struct, union | `repeated`                                            | one record per element                                                      |
| map                                   | `map<K, V>`, K integral or string                     | one entry message per pair, key = 1, value = 2                              |
| tuple                                 | induced message, fields `field_1`…, numbered from 1   | length-delimited                                                            |
| union                                 | `message` with `oneof value`, arm number = ordinal    | the one arm's record                                                        |
| retired field / arm / enum value      | `reserved`                                            | never written; read as an unknown field                                     |
| cross-package struct, enum, union     | `pkg.Name` and an import                              | as the local kind                                                           |

Refused by the backend when proto3 is requested, each with a `GenerateError`
naming the declaration and the member, the same refusals the `.proto` emitter or
the shared proto3 sizer already make: an optional array or map (ADR-0013
decision 7, `proto3::optional_repeated`); an array of arrays or of maps, a map
whose value is an array or a map, a map key outside the integral and string
scalars; an enum value outside int32; a field number protobuf reserves or
exceeds. Refused by the codec alone, as the FlatBuffers codec refuses it
(ADR-0019 review ruling 6): an optional array element and an optional map value,
which the schema emits without presence. Refused because the compiler never
produces them, and the FlatBuffers codec refuses them too: an integer or float
with no width (the `int64` and `string` fallbacks of
`ridl_ir::projection::proto3::scalar`).

### D-3. The shared facts live in `ridl-ir`, and a drift test holds them together

The FlatBuffers codec shares slot ids, discriminants and bounds with its schema
emitter through `ridl_ir::projection::flatbuffers`. proto3 needs less: a field
number is the ordinal both emitters already read (the `Model`'s `Slot.ordinal`
and `Arm.ordinal`, the IR's `ordinal`), a tuple field's number is its position
from 1, a map entry's key and value are 1 and 2, and a box's value is 1. What
two emitters could disagree on is the scalar a width projects to. So:

- `ridl_ir::projection::proto3` gains one function from a `v1::IntWidth` /
  `v1::FloatWidth` (the `Model`'s spelling of a width) to `Scalar`, and
  `scalar(&v2::TypeDef)` is rewritten over the same table, so there is one
  table. A `Scalar::wire_type()` (varint, i64, len, i32) joins it.
- The codec emitter exposes, to its own tests, the table it emitted from: per
  message, `(field number, scalar or message name, label)`, keyed by the
  `.proto` message name — `InducedName.wire` for an induced tuple, not the Rust
  name. `proto3_layout_matches_the_emitted_schema` compiles the emitted `.proto`
  of every corpus package (`examples/cabin`,
  `crates/ridlc/tests/corpus/veh-cluster`, `evals/corpus/*`, the conformance
  fixture) with `protox` and compares it with that table field by field. A
  field-number or scalar drift fails it without running any generated code.

Moving the proto backend's structural walk into `ridl-ir` (as the FlatBuffers
projection did) was considered and rejected for this story: the proto backend
reads the IR and the codec reads the `Model`, so a shared walk means first
porting the proto backend onto the `Model`, which is driftsys/ridl#690's
territory, and the drift test catches the same disagreements.

Cost if wrong: a disagreement the drift test does not model (for example a
packed flag) reaches only the `prost` conformance suite, which runs over one
fixture rather than the corpus.

### D-4. `MAX_SIZE` is the shared proto3 bound, carried in the `Model`

`MAX_SIZE` is a literal, the bound `ridl_ir::projection::size::size_state`
answers for the declaration's root message under `Encoding::Proto3` — the same
number the catalog descriptor and the codegen `Model`'s payload sizes carry, so
a `Sizes<Proto3>` row and the descriptor's proto3 row cannot differ.
`type Buffer = [u8; MAX_SIZE]` and `BUFFER` come from the same literal.

Because of C-4, the bound has to be in the `Model`. **The codegen protocol gains
one field:** `Declaration.proto3 = 24`, a `SizeState` — the proto3 state of the
declaration's root message, absent on a constant. `ridl_ir::codegen::lower`
fills it from `size_state(home, PayloadShape::Named(name), ctx, Proto3)`. It is
additive under ADR-0029: the `codegen-protocol` dump gains lines and loses none,
no enum gains a value, and a plugin that does not read it is unaffected. It is
the per-declaration counterpart of `FbRoot.max_size`, and a Kotlin proto3 codec
needs the same number. An induced interaction tuple needs no such field: its
bound is already the interaction's `request_sizes.proto3` or
`reply_sizes.proto3` (D-1).

A declaration whose state is not `Bounded` is refused with a `GenerateError`
naming it and the cause (`RefusedMember`, `Unresolved`, `Overflow`, `NoBound`),
per type, when proto3 is requested. That is the FlatBuffers rule (D-7 of its
note). Unlike FlatBuffers, no type is withheld silently: the proto3 sizer
resolves cross-package references through `others`, so there is no "cannot
judge" case.

A test pins each emitted `MAX_SIZE` to the sizer's number, and a worst-case
value per fixture type (every collection at its maximum count, every string at
its maximum length of four-byte characters, every varint at its widest) is
encoded and checked to fit. If a worst case does not fit, the sizer is wrong and
the fix goes there.

Cost if wrong: one field in a frozen protocol that cannot be removed later. The
alternative, a second sizer in the Rust backend over the `Model`, is two sources
of one number.

### D-5. The writer writes the canonical form, which is what `prost` writes

- Fields in field-number order, whatever their declaration order.
- A field with implicit presence (no `?`: a scalar, an enum, an enum set, a
  string, bytes, an array, a map) is omitted when its value is the proto3
  default — 0, `false`, the empty string, empty bytes, the empty array or map,
  enum value 0, the empty set. This is what every proto3 writer does, so this
  codec's bytes equal `prost`'s.
- A `?` field is written when present, even at 0 or empty, and omitted when
  absent.
- A non-optional struct, tuple or union field is always written, as a
  length-delimited record that may have length 0 — what `prost` writes for
  `Some(default)`.
- A union writes exactly its one arm, even at the arm's default (`prost` does
  the same for a `oneof`).
- An array of numerics, booleans or enums is packed; any other array is one
  record per element. An empty array is not written.
- A map writes one entry per pair in the order of the domain value's
  `Vec<(K, V)>`, so the encoding is deterministic and nothing sorts. Inside an
  entry, a key or a value equal to its default is omitted, and a struct, tuple
  or union value is omitted when its own encoding is empty — measured: this is
  `prost`'s entry encoding (`{"": 0}` is `a2 01 00`).
- A box writes `value` under the same rule as any field with implicit presence,
  so a box of 0 is the empty message.

Measured on `prost` 0.14.4 (driver's probe, 2026-10-20): field-number order, the
default omission, packed numerics, a zero-length nested message for
`Some(default)`, a `oneof` arm written at its default, the entry encoding above,
and `BTreeMap` entries in key order. A map has a byte-equality property with
`prost` only when the domain `Vec` is in key order and has no duplicate key; the
tests build it that way.

Cost if wrong: none for interoperability (every proto3 reader reads every one of
these forms); byte equality with `prost` would be partial.

### D-6. A float at `-0.0` is written, unlike `prost`

The proto3 rule omits a field "at its default", and the protobuf reference
implementations (C++, Java, Go) compare a float by its bits, so `-0.0` is
written. `prost` 0.14.4 compares by value and omits it (measured: `-0.0` encodes
to nothing), so a `-0.0` sent through `prost` arrives as `+0.0`. This codec
writes `-0.0`, so its own round trip and a round trip through any reference
implementation keep the sign. `prost` reads the written `-0.0` correctly. This
is the one listed exception to G-2's byte equality, and the test states it as a
case rather than skipping it.

Cost if wrong: if the owner prefers total byte equality with `prost`, the writer
compares by value instead and the sign of zero is lost on every round trip, ours
included. This is a "theirs" question (§9).

### D-7. The reader accepts every conforming encoding

- Fields in any order.
- A repeated numeric field in packed form, unpacked form, or both mixed; the
  elements are concatenated in order (the protobuf rule).
- A non-minimal varint (`80 00` for 0).
- A `bool` varint other than 0 or 1 reads as `true` (the protobuf rule; `prost`
  does the same).
- An unknown field number with wire type 0, 1, 2 or 5 is skipped (D-8).
- A field number protobuf reserves (19000 to 19999) or a retired ordinal is an
  unknown field like any other.

### D-8. Unknown fields are skipped and dropped

A field this version does not know — written by a newer version of the contract,
or a retired ordinal — is skipped by its wire type and not kept. This is what
lets a newer writer append a field and an older reader still read the payload,
which is the reason proto3 is the network encoding (ADR-0018 decision 3). The
decoded value has no place to keep it, and keeping it would need an allocator in
`no_std`. This answers ADR-0018 open item 3 for the generated Rust codec:
dropped on decode. A gateway that must forward fields it does not know forwards
the bytes without decoding them, which `ridl-transport-ws` already does.

`verify` does not apply `Malformed::TooLarge` to a proto3 payload. A buffer from
a newer writer may legitimately exceed this version's `MAX_SIZE` because of
fields this version skips; the bound that protects a receiver is the buffer its
port reads into, which is `MAX_SIZE` bytes at every face site. `verify` is
linear in the bytes it reads.

Cost if wrong: a consumer that relies on a `verify` size refusal does not get
one for proto3; a consumer that needs unknown fields preserved must handle the
bytes itself.

### D-9. What the reader refuses as malformed, and the three new `Malformed` variants

`ridl_rt::payload::Malformed` is `#[non_exhaustive]`, and its variant is not
carried in a frame (the frame specification §9 carries the stratum only), so
adding a variant is additive for `ridl-rt` and for the `runtime-crates` surface.
Three are added, because no existing variant describes these faults:

| Fault                                                                                                    | Variant                                                                                                           |
| -------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| a length or a fixed-width value runs past the end of the buffer, a truncated varint                      | `OutOfBounds` (existing)                                                                                          |
| a varint longer than ten bytes, or whose tenth byte is above 1                                           | `Varint` (new)                                                                                                    |
| a varint wider than its field's scalar: a `uint32` above `u32::MAX`, a `sint32` or an enum outside `i32` | `Varint` (new) — `prost` truncates; this codec refuses, because the bytes do not hold a value of the field's type |
| field number 0; wire type 3, 4, 6 or 7; a known field with a wire type its type does not take            | `WireType` (new)                                                                                                  |
| a field that is not repeated appears twice; a union message carries two arms, or one arm twice           | `Duplicate` (new) — see D-10                                                                                      |
| a string that is not UTF-8                                                                               | `Utf8` (existing)                                                                                                 |
| an absent field whose default is not a legal value of its type (D-12)                                    | `MissingRequired` (existing)                                                                                      |

Groups (wire types 3 and 4) are refused rather than skipped: proto3 has no
groups, and no schema this toolchain emits can produce one, so a group can only
come from a writer outside the contract. (`prost` skips them.)

Nesting needs no run-time depth limit: a type with a proto3 bound is not
recursive, so the generated verify functions nest to a depth fixed at generation
time, and skipping an unknown length-delimited field does not recurse.

Cost if wrong: three public variants on a published crate, which are permanent;
a foreign writer that emits groups is refused.

### D-10. A singular field written twice is refused

protobuf's merge rule reads a repeated scalar as "last one wins", a repeated
message field as a merge of every occurrence, and two `oneof` arms as "last one
wins"; `prost` implements all three (measured). This codec refuses each with
`Malformed::Duplicate` instead. A payload is one value written once; no writer
this toolchain or `prost` generates writes a field twice; the merge rule exists
for concatenating serialized messages, which is not how a frame carries a
payload. Refusing also keeps `verify` and `decode` simple and deterministic:
`verify` tracks a fixed-size `[bool; N]` of seen fields per message (no
allocation), and `decode` reads the single occurrence. A map key repeated across
entries is not a wire duplicate; it is refused by the typl uniqueness check as
`Contract(Rule::Unique)`, as the FlatBuffers codec refuses it.

Cost if wrong: a foreign writer that concatenates two encodings of one message,
or a proxy that appends an override field, is refused where `prost` would
accept. This is a "theirs" question (§9).

### D-11. The proto3 view is the verified bytes, with no accessors

ADR-0021 open item 3 leaves the proto3 view open: the bytes, an accessor, or an
owned parsed value. The view is `ridl_rt::proto3::Verified<'a>`, which holds the
verified bytes and implements `ridl_rt::payload::View`; it has no field
accessors. proto3 is not a random-access format — a field can be anywhere in the
message, so an accessor is a scan — and the face reads every payload through
`decode`, not through a view. An owned parsed value is what `decode` already
returns. Accessors can be added later without a break, because they would be
inherent methods of a generated type that does not exist yet.

Cost if wrong: a consumer that wants to read one field without decoding the
payload cannot, for proto3.

### D-12. Absence: the default when it is legal, `MissingRequired` when it is not

This is #472's reader rule in proto3 terms. In proto3 the rule is forced for
every implicit-presence field, not only scalars: a conforming writer omits a
non-optional field at its default (D-5), so a reader must read an absent one as
that default. Legality is decided at generation time from the declaration, so no
check runs in the generated code:

| Field type                      | Absent reads as                     | Legal when                                                                                                           |
| ------------------------------- | ----------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| integer, float, named or inline | 0                                   | `ridl_ir::zero::range_holds_zero` over its range and step — the predicate the FlatBuffers codec and `ridl diff` read |
| boolean                         | `false`                             | always                                                                                                               |
| enum                            | value 0                             | the enum declares a zero member (`ridl_ir::zero::enum_zero_member`); a synthesized `_UNSPECIFIED` is not one         |
| enum set                        | the empty set                       | always                                                                                                               |
| string, bytes                   | empty                               | the length lower bound is 0 (a declared pattern is then checked on the empty value, as for a present one)            |
| array, map                      | empty                               | the count lower bound is 0                                                                                           |
| struct or tuple (not optional)  | the message decoded from zero bytes | each of its fields is legal when absent, by this table                                                               |
| union (not optional)            | —                                   | never: a union holds exactly one arm                                                                                 |
| any `?` field                   | `None`                              | always                                                                                                               |

An absent field that is not legal is `Malformed::MissingRequired`. The rule
holds in every message — a struct, a tuple, a map entry's key and value, a union
arm's message, and a box root, so an empty payload is a box of 0, or is refused
when the box's type excludes 0.

The string, bytes and collection rows go further than the FlatBuffers codec,
which refuses any absent offset: a FlatBuffers writer may omit only a scalar,
while a proto3 writer omits all of these. `ridl diff`'s append rule (ADR-0008
decision 14 as amended) stays as it is: it calls an appended non-optional string
or collection breaking, because the FlatBuffers codec refuses the old buffer; it
is right for the stricter of the two codecs.

Cost if wrong: if a missing string or collection ought to be refused even when
empty is legal, a truncated writer's payload reads as empty values. proto3
cannot tell the two apart in any case.

### D-13. Constraint checks in `verify`

`verify` checks every typl constraint the FlatBuffers `verify` checks, at the
same positions, through the same generated `check` functions and the same
inline-constraint checks: range, step, string length in characters, bytes
length, collection count, map key uniqueness, the pattern under
`validate-pattern`, an enum value that is declared, and an enum set with no
undeclared bit (each `Contract(Rule::Variant)` — ADR-0019 review ruling 7: the
schema leaves enums open and the codec closes them). A varint that fits the wire
scalar but not the declared width (300 in a `u8` field) is
`Contract(Rule::Range)`, since the width is derived from the declared range.
`encode` checks what the FlatBuffers `encode` checks (anonymous lengths and
collection counts) and returns `EncodeError::Contract` on the same rules.

### D-14. `encode` sizes first and writes forward

Each type gets `__ridl_pb_len_<Name>(&T) -> usize`, the exact encoded length,
and `__ridl_pb_encode_<Name>(&T, &mut ridl_rt::proto3::Writer)`. `encode` calls
the length function once, returns `EncodeError::Capacity { needed, available }`
with the exact `needed` when `out` is too small, and otherwise writes forward,
so `Encoded.bytes` is a prefix of `out`. A nested message's length is computed
by the same length function before its prefix is written. This costs a
recomputation per nesting level (the cost `prost` pays the same way); the depth
is fixed at generation time. No allocation.

Writing backwards, as FlatBuffers does, was rejected: a proto3 length prefix is
a varint whose size depends on the length, so a backwards writer has to shift or
reserve, and the forward writer gives an exact `needed`.

### D-15. `ridl-rt`'s `proto3` module

Under the existing `proto3` feature, a module `ridl_rt::proto3` holds what every
generated implementation shares, as `ridl_rt::flatbuffers` does: varint and
zigzag encoding and decoding, the tag, a forward `Writer` over `&mut [u8]` with
capacity checks, a `Reader` over `&[u8]` that yields
`(field number, wire type,
value)` and skips an unknown field, the length
helpers, and `Verified<'a>`. No dependency, no allocation, no `unsafe`, Rust
1.83 and edition 2021. It decides no layout: it is handed field numbers and
scalars. `just test` (`--all-features`), `just wasm-check` and
`just compat-check` already cover `ridl-rt` with every feature on.

### D-16. Emitted names and placement

Per type, at the generated package's module scope beside the FlatBuffers
functions: `__ridl_pb_len_<Name>`, `__ridl_pb_encode_<Name>`,
`__ridl_pb_verify_<Name>`, `__ridl_pb_decode_<Name>`, `pub(crate)`, each with
`#[allow(non_snake_case)]`, the tail the declared name — the FlatBuffers codec's
naming rule and its reasons. No typl name begins with an underscore, so the
claims table needs no new entry. Every primitive and prelude type is written by
its `::core::` or `::std::` path.

### D-17. The conformance oracle and how coverage is reached

**The oracle is `prost` code generated from the emitted `.proto`.** The
conformance fixture
`crates/ridl-backend-rust/tests/fixtures/proto3_conformance.ridl` (with a second
package for a cross-package reference) is compiled by `ridlc`;
`ridl_backend_proto::generate_with` emits its `.proto`; `protox::compile` turns
that into a `FileDescriptorSet`; `prost_build::Config::compile_fds` with
`btree_map(["."])` writes Rust; that Rust is **checked in** under
`crates/ridl-backend-rust/tests/generated/`, and
`the_checked_in_prost_code_is_what_prost_build_writes` regenerates it and
compares bytes (`RIDL_UPDATE_GENERATED=1` rewrites it). No `build.rs`, no
`protoc`. Two generations were measured byte-identical.

Dev-dependencies of `ridl-backend-rust`, each test-only:

| Crate                | Why                                                                                                                                                                                    |
| -------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ridl-backend-proto` | emits the `.proto` the oracle is generated from (the FlatBuffers suite uses `ridl-backend-flatbuffers` the same way)                                                                   |
| `protox`             | compiles that `.proto` without `protoc` (ADR-0006 decision 3)                                                                                                                          |
| `prost-build`        | generates the oracle's Rust, in the regeneration test only                                                                                                                             |
| `prost`              | the runtime the checked-in oracle code links (already in the lockfile through `ridl-ir`)                                                                                               |
| `prost-reflect`      | the corpus pass below: a `DynamicMessage` over the `protox` pool reads a message of any corpus package without generating code per package (already in the lockfile through `ridl-ir`) |
| `proptest`           | the seeded value generator of the property cases (already a workspace dependency)                                                                                                      |

**The generated codec runs as a program**, through the bare-`rustc` mechanism
every compile proof in this crate uses (`tests/support/rustc.rs`): the suite
generates the package with `encodings = [Proto3]`, appends a `main` that reads
hex lines from standard input and writes hex lines, and compiles it against
`ridl-rt` with the `proto3` feature.

**Cases.**

1. Bytes this codec writes for a table of hand-chosen values — every scalar kind
   at 0, at 1, at its minimum and at its maximum, `-0.0`, NaN, a negative enum
   value, an empty and a non-empty string with multi-byte UTF-8, every
   collection empty and full — are equal to the bytes `prost` writes for the
   same values, and `prost` decodes them to those values (the `-0.0` case
   asserts the D-6 difference instead of equality).
2. Bytes `prost` writes are verified and decoded by this codec to the same
   values, and re-encoded to the same bytes.
3. A seeded property: `proptest` generates `prost` values within the fixture's
   typl constraints (a few hundred per run, fixed seed, so a failure
   reproduces); `prost` encodes; the program decodes and re-encodes; the bytes
   are equal. Both directions in one property: the program's bytes are what
   `prost` would write, and `prost`'s bytes are what the program reads.
4. Every shape of D-2 is reached by cases 1 to 3: the fixture holds each scalar
   width, each enum kind (with a zero member, without one, with a negative
   value), an enum set of width u8 and u64, nested structs, a tuple, a union
   with a scalar arm, a string arm, an enum arm, an enum-set arm and a struct
   arm, each `?` kind, packed arrays of every numeric, bool and enum, arrays of
   string, bytes, struct and union, a map for each admitted key kind with
   scalar, enum, string and struct values, each root box kind, a reference into
   the second package, an interface with a call of several parameters, a query
   of no parameter and a query whose reply is a tuple (the induced roots of
   D-1). A test lists the fixture's declarations against that inventory, so a
   shape cannot drop out unnoticed.
5. The reader rules of D-7: unpacked and mixed repeated numerics, fields out of
   order, a non-minimal varint, a `bool` of 2, unknown fields of each wire type
   at the start, the middle and the end of a message.
6. The refusals of D-9 and D-10, one hand-built buffer per row, each asserting
   the exact `VerifyError`, and each checked against `prost` so the suite
   records where this codec is stricter.
7. The absence rule of D-12: one buffer per row of its table, for a field of a
   struct, of a tuple, of a map entry and of a box root, and the same field read
   as five structs that differ only in the field's type — the FlatBuffers
   suite's case 11, in proto3 terms. Case 12 of that suite has a proto3
   counterpart: over the evolution fixture, every append `ridl diff` calls
   compatible reads an old payload, so the diff is never more lenient than this
   codec.
8. Malformed-input robustness (ADR-0018 decision 4): truncations and byte
   mutations of every case-1 buffer, and seeded random byte strings, are fed to
   `verify`; it never panics; whenever it accepts, `prost` accepts the same
   bytes and decodes the same value.
9. The corpus pass: for every type of every corpus package that the codec
   covers, the generated `Default` value is encoded by this codec, decoded by
   `prost-reflect` as a `DynamicMessage` over the `protox` pool of the emitted
   `.proto`, and re-encoded by `prost-reflect` to the same bytes (maps of more
   than one entry are left out: `prost-reflect` writes map entries in hash
   order, measured).
10. The structural drift test of D-3.
11. Determinism: one value's bytes pinned as a literal, and a second encode into
    a buffer filled with `0xAB` compared with the first.
12. `MAX_SIZE` equals the sizer's number for every fixture type, and the
    worst-case value fits (D-4).
13. `the_generated_proto3_codec_checks_for_wasm32` —
    `rustc --target
    wasm32-unknown-unknown --emit=metadata` over the emitted
    source.

**Mutations required at review.** Each codec task's reviewer mutates the codec
(a field number, a zigzag, the packed flag, the default-omission comparison, a
duplicate check, an absence legality) in its own worktree and confirms that a
case fails. The design record will list the mutations measured, as the
FlatBuffers record does.

### D-18. The guard

`xtask/tests/oracle_boundary.rs` gains `PROST_CRATES` (`prost`, `prost-derive`,
`prost-build`, `prost-types`, `prost-reflect`, `protox`, `protox-parse`) beside
`PLANUS_CRATES`, and three checks:

- `the_runtime_reaches_no_prost_crate`: `ridl-rt`, resolved with
  `--all-features`, reaches none of them at any edge kind;
- `the_generated_crate_reaches_no_prost_crate`: `#[ignore]`d like its planus
  sibling, and run by `just demo` after `ridl build` writes the crates; the
  demo's `--exact` list and its expected pass count grow by one;
- `BOUNDARIES` gains `ridl-backend-rust → ridl-backend-proto`, `→ protox`,
  `→ prost-build`, `→ prost-reflect` as dev-only edges. `prost` itself cannot be
  a dev-only boundary of `ridl-backend-rust`, because `ridl-ir` already depends
  on it; the guard's header says so.

`examples/cabin/ridl.toml` and `crates/ridlc/tests/corpus/veh-cluster/ridl.toml`
gain `"proto3"`, so `just demo` builds, lints, documents and checks for
`thumbv7em-none-eabihf` a crate carrying the proto3 codec, `just compat-check`
builds it at Rust 1.83 and edition 2021 (its cabin cell gains
`--cfg feature="proto3"`), and the generated-crate guard reads a crate that
actually carries the codec. The frozen copies under
`xtask/tests/freeze-fixtures/generated/` keep `["flatbuffers"]` (N-6).

### D-19. The face over a proto3 port

One interaction-face round trip of each kind that carries a payload — a signal,
an event, a command, a query — runs over `Loopback::<Proto3>::with_encoding`,
against a package generated with `encodings = [FlatBuffers, Proto3]`. It
replaces the test-only "reversed codec" `Payload<Proto3>` and its hand-written
`Sizes<Proto3>` in `crates/ridl-backend-rust/tests/interaction_face.rs`, which
existed only to prove the face generic over a second encoding before one
existed.

### D-20. Frozen surfaces

- `wire`: unchanged (N-1).
- `codegen-protocol`: lines added for `Declaration.proto3` (D-4); none removed.
  `cargo xtask freeze update` and the commit says why.
- `runtime-crates`: additive — the `proto3` module under its feature and three
  `Malformed` variants (D-9, D-15). `cargo semver-checks` must pass.
- `generated-crate`: unchanged; the frozen copies name `flatbuffers` only, and
  their regeneration must be byte-identical. `xtask/src/freeze/rust_api.rs`'s
  comment that the Rust backend refuses `proto3` is corrected.
- `msrv`: unchanged.
- `manifest`, `cli`: unchanged (the `encodings` names already include `proto3`).

No published API breaks: `WireEncoding` and `Malformed` are `#[non_exhaustive]`,
and the protocol field is new. The change is a feature, not a breaking change.

### D-21. Records

A new as-built record `docs/design/proto3-codec.md` beside
`flatbuffers-codec.md`, with the cases and the mutations measured. Amended in
place: ADR-0018 open item 3 (answered for the Rust codec, D-8); ADR-0021 open
item 3 (answered, D-11); ADR-0019's review rulings 6, 7 and 18 (discharged) and
ruling 14 (its "the face refuses it today" is no longer true since
driftsys/ridl#713); `docs/design/flatbuffers-codec.md` (its sentences that say
the proto3 codec "is not built" and will be checked against `protoc`);
`docs/design/ridl-rt.md` (`proto3` is no longer empty; the three variants);
`docs/design/interaction-face.md` (the `Sizes<E>` gap row);
`docs/book/generated-code.md` (the `proto3` feature row);
`docs/book/cli-reference.md` (the refusal of `proto3`); the codegen protocol's
documentation of `Declaration.proto3` in `docs/design/codegen-plugins.md` and
the book's plugin chapter; `docs/ROADMAP.md`, `docs/BACKLOG.md` and
`docs/archive/roadmap-landed-record.md`.

## 6. What is left to the builder

- The exact Rust shape of the `ridl_rt::proto3` reader and writer, within D-15.
- How the emitter shares code between the FlatBuffers and proto3 codec emitters
  (the `Wire`/`Scalar` classification, the constraint checks). Sharing is
  welcome as long as the FlatBuffers output stays byte-identical (N-6).
- The fixture's exact declarations, within case 4's inventory.
- How many seeded cases a property runs, within a test-time budget of a few
  seconds.

## 7. Alternatives considered

| Candidate                                                                        | Verdict                     | Reason                                                                                                                                                                                            |
| -------------------------------------------------------------------------------- | --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Delegate the codec to `prost` in the generated crate                             | rejected                    | ADR-0018 decision 4 generates the codec; `prost` needs `alloc` and its own message types beside the domain types; and the guard exists to keep it out                                             |
| `protoc`-generated C++ as the oracle                                             | rejected                    | `protoc` stays out of the gate (ADR-0006 decision 3); `prost` through `protox` is hermetic and is what the roadmap row names                                                                      |
| `prost-reflect` alone as the oracle, no generated `prost` code                   | rejected as the main oracle | its map encoding is in hash order (measured), and a dynamic message is a second interpretation of the descriptor rather than the generated code a third party would use; kept for the corpus pass |
| A `build.rs` generating the oracle at test time                                  | rejected                    | the FlatBuffers suite checks its oracle in with a regeneration test; a `build.rs` would run `protox` on every build of the crate                                                                  |
| Round trip only, no byte equality (as for FlatBuffers)                           | rejected                    | proto3 has a canonical encoding for the cases a writer controls, so byte equality is available and is the stronger check; ADR-0018 decision 4 asks for byte-level conformance                     |
| Last-one-wins and merge for duplicate fields                                     | rejected (D-10)             | no generated writer produces duplicates; merge needs per-message state across occurrences; a theirs question                                                                                      |
| Preserve unknown fields                                                          | rejected (D-8)              | needs an allocator and a place in the domain type; a gateway forwards bytes                                                                                                                       |
| Skip groups as `prost` does                                                      | rejected (D-9)              | proto3 has no groups and no schema this toolchain emits can produce one                                                                                                                           |
| Writing backwards, as the FlatBuffers builder does                               | rejected (D-14)             | a varint length prefix needs a shift or a reserve; forward writing gives an exact `needed`                                                                                                        |
| Field accessors on a proto3 view                                                 | deferred (D-11)             | each accessor is a scan; the face decodes; additive later                                                                                                                                         |
| A second proto3 sizer in the Rust backend instead of a `Model` field             | rejected (D-4)              | two sources of one bound; the plugin path has no IR                                                                                                                                               |
| Moving the proto backend's walk into `ridl-ir` now                               | rejected (D-3)              | needs the proto backend ported onto the `Model` first (driftsys/ridl#690); the drift test covers the risk                                                                                         |
| Writing `-0.0` the way `prost` does (omitted)                                    | rejected (D-6)              | loses the sign of zero on every round trip; the reference implementations write it; a theirs question                                                                                             |
| Refusing an absent string or collection, as FlatBuffers refuses an absent offset | rejected (D-12)             | every proto3 writer omits an empty one, so the codec would refuse `prost`'s canonical bytes                                                                                                       |
| `Malformed::TooLarge` above `MAX_SIZE`, as FlatBuffers applies it                | rejected (D-8)              | a newer writer's skipped fields can exceed this version's bound; the port's buffer is the receiver's protection                                                                                   |

## 8. Trace

- Satisfies: driftsys/ridl#264.
- Related: driftsys/ridl#472, driftsys/ridl#598, driftsys/ridl#690,
  driftsys/ridl#317.
- Records: ADR-0017, ADR-0018 decisions 3 to 5 and open item 3, ADR-0019
  decision 9 and review rulings 6, 7, 14 and 18, ADR-0020 decisions 2, 5 and 9,
  ADR-0021 decisions 7 and 8 and open item 3, ADR-0023, ADR-0029,
  `docs/design/flatbuffers-codec.md`.

## 9. Questions for the owner

The builder check (2026-10-20) raised two points. The first was a claim the code
contradicted — an earlier draft left induced interaction tuples out of scope,
while driftsys/ridl#713 already makes them payloads — and is answered in D-1 by
the FlatBuffers codec's own rule. The second is below, with D-6, which only the
owner can settle. The driver's proposed answer is the one the decisions above
take.

1. **D-10: refuse a singular field written twice, or apply protobuf's merge
   rule?** Proposed: refuse (`Malformed::Duplicate`). The answer shapes `verify`
   and `decode` (a seen table and a single read, against per-message merge state
   across occurrences), so it is settled before the codec tasks start.
2. **D-6: write a float at `-0.0` (reference implementations), or omit it
   (`prost`)?** Proposed: write it. Either way is one comparison in the writer.
