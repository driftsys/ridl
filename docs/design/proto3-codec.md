# The proto3 payload codec

The generated `Payload<Proto3>` implementations a package carries, as built. The
binding choices are [ADR-0017](../decisions/ADR-0017-proto3-projection-rules.md)
(the projection this codec must agree with byte for byte, decisions 6 to 8
especially),
[ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md)
decisions 3 and 4 (proto3 on the network; the codec generated rather than
delegated, and its byte-level conformance) and its open item 3,
[ADR-0019](../decisions/ADR-0019-flatbuffers-projection-rules.md)'s 2026-10-12
review rulings 6, 7 and 18,
[ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
decisions 5 and 9 (one feature per encoding; a backend is an executable, not a
library), and [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md)
decisions 7, 8 and 10 and its open item 3 (the proof type, the `proto3` feature,
the additive `ridl-rt` items, the view). It follows
[the FlatBuffers codec](flatbuffers-codec.md) wherever the two formats allow,
and this record says where they differ. The reasoning behind each decision, with
the cost the design note gave each one if it were wrong, is the archived design
note
[`../archive/2026-10-20-proto3-codec-design.md`](../archive/2026-10-20-proto3-codec-design.md)
and its plan
[`../archive/2026-10-20-proto3-codec-plan.md`](../archive/2026-10-20-proto3-codec-plan.md);
the alternatives they rejected are [below](#alternatives-considered).

**The codec is emitted on request.** `ridl build` emits it, with the descriptors
and the face, when the manifest's `[backend.rust] encodings` names `proto3`,
alone or beside `flatbuffers`. With both names the two codecs are emitted side
by side, and the face, which reads its encoding from its port, runs over either.
The bytes this codec writes are the bytes of the `.proto` that
`ridl build --emit proto` writes for the same package, so a third party that
generates a reader from that `.proto` reads them, and this codec reads what that
third party writes.

## Where the code is

| What                                                                                 | Where                                                                                                     |
| ------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------- |
| The scalar a width projects to, and its wire type, which both emitters read          | `crates/ridl-ir/src/projection/proto3.rs` (`int_width_scalar`, `float_width_scalar`, `Scalar::wire_type`) |
| The proto3 size bound of a root message                                              | `crates/ridl-ir/src/projection/size/proto3.rs`                                                            |
| `Declaration.proto3` in the codegen model                                            | `crates/ridl-ir/proto/ridl/codegen/v1/model.proto`, filled by `crates/ridl-ir/src/codegen/lower.rs`       |
| The `.proto` schema emitter                                                          | `crates/ridl-backend-proto/src/lib.rs`                                                                    |
| The codec emitter                                                                    | `crates/ridl-backend-rust/src/codec_proto3.rs`                                                            |
| The `Sizes<Proto3>` table                                                            | `crates/ridl-backend-rust/src/descriptors.rs`                                                             |
| The `encodings` list (`WireEncoding::Proto3`)                                        | `crates/ridl-backend-rust/src/lib.rs`                                                                     |
| The varint, tag, reader and writer helpers the emitted code calls, and `Verified`    | `crates/ridl-rt/src/proto3.rs`                                                                            |
| The three `Malformed` variants                                                       | `crates/ridl-rt/src/payload.rs`                                                                           |
| The round trip, the generation refusals and the `wasm32` check                       | `crates/ridl-backend-rust/tests/proto3_roundtrip.rs`                                                      |
| The reader rules, the refusals and the absence rule, each beside `prost`'s verdict   | `crates/ridl-backend-rust/tests/proto3_reader.rs`                                                         |
| Conformance against `prost`, the drift test, the property, robustness and the corpus | `crates/ridl-backend-rust/tests/proto3_conformance.rs`                                                    |
| The checked-in `prost` oracle                                                        | `crates/ridl-backend-rust/tests/generated/proto3_conformance_prost.rs`                                    |
| The fixtures                                                                         | `crates/ridl-backend-rust/tests/fixtures/proto3_*.ridl`                                                   |
| The face over a proto3 port, run rather than compiled                                | `crates/ridl-backend-rust/tests/interaction_face.rs`                                                      |
| The guard that keeps `prost` out of `ridl-rt` and of a generated crate               | `xtask/tests/oracle_boundary.rs`                                                                          |

## What is emitted, and when

`generate_face_with(package, encodings)` and `generate_pipeline` emit the codec
when `encodings` names `proto3`; `generate` and `generate_with` emit the domain
types only. `ridlc` renders one `ridl-rt` feature per name in the list, so the
generated manifest enables `proto3`.

A codec is emitted for every declaration that projects a type: a struct over its
own message, a union over its message with `oneof value`, and a named scalar, an
enum and an enum set over the box message `<Name>Box { <type> value = 1; }`
(ADR-0017 decision 6). The box's `value` is not `optional`. A constant gets
nothing. An induced tuple is a root too when it crosses the wire on its own
(driftsys/ridl#713): the argument message of a call with zero or several
parameters, and the reply message of a query whose reply is a tuple. Its message
is the one the schema emitter appends, named by the model's `InducedName.wire`,
with positional fields numbered from 1. A tuple reached only from a field is
encoded inside its owner. A tuple that only a `fixed` reaches gets no codec, as
it gets no FlatBuffers codec, because a fixed value is never in flight: the
`.proto` of `veh-cluster` carries `WheelDiagnosticsTickBoundsElement`, and no
Rust function encodes it.

Per message, at the generated package's module scope beside the FlatBuffers
functions, four `pub(crate)` functions with `#[allow(non_snake_case)]`, the tail
being the declared name for the reasons the FlatBuffers record gives:

- `__ridl_pb_len_<Name>(&T) -> usize`, the exact encoded length;
- `__ridl_pb_encode_<Name>(&T, &mut ridl_rt::proto3::Writer)`, which writes
  forwards;
- `__ridl_pb_verify_<Name>(&[u8])`, which walks the fields and checks every typl
  constraint;
- `__ridl_pb_decode_<Name>(&[u8]) -> T`, which is total.

For a root whose proto3 size state is `Bounded`, the implementation is
**`Payload<Proto3>`**. `MAX_SIZE` is a literal, the bound the shared sizer
(`ridl_ir::projection::size`) answers under `Encoding::Proto3`, read from the
codegen model: `Declaration.proto3` for a declaration, and the interaction's
`request_sizes.proto3` or `reply_sizes.proto3` for an induced tuple. The plugin
path has the model and no IR, which is why `Declaration.proto3` (field 24) was
added to the codegen protocol; it is additive under
[ADR-0029](../decisions/ADR-0029-stability-policy.md). `type Buffer` and
`BUFFER` come from the same literal. The catalog descriptor's proto3 row and the
`Sizes<Proto3>` row are the same number, because all three read one sizer. Every
`MAX_SIZE` is finite, and `ridl-rt`'s `Payload` and `Sizes` API is the one the
FlatBuffers codec implements. The lowering sizes each declaration with a walk of
its own, so a nested type that several declarations share is sized once per
declaration; the cost is paid once, at generation time.

**A type whose size state is not `Bounded` is refused**, for a declaration
exactly as the FlatBuffers codec refuses a declaration with no finite bound.
Every state but `Bounded` is refused: an absent state (`NoBound`, `Overflow`,
`RefusedMember` and `Unresolved` are the causes the sizer gives a type),
`Unbounded`, and a state the model does not carry. The refusal is a
`GenerateError` that names the declaration and the cause, by its name in
parentheses for an absent state (`bound_state` in `codec_proto3.rs`). The codec
emitter stops at the first refused declaration, so one refusal fails the
package's proto3 generation, and `ridl build` fails with it; the same package
still generates with no encoding. `an_unbounded_type_is_refused_under_proto3`
pins `NoBound` for a string with no length bound and for a struct that holds
one. An induced interaction payload — a call's argument message, a query's tuple
reply — whose state is not `Bounded` is handled as the FlatBuffers codec handles
one with no finite FlatBuffers bound: the tuple gets no `Payload<Proto3>`, and
the pipeline skips its interface, with the interface's descriptors and face,
under the rule and the marker [the interaction-face record](interaction-face.md)
states (design rule 2 and "The consumer face"), so the package's other
interfaces and its types still generate. The descriptor emitter judges the tuple
in each encoding the list names (`descriptors::induced_name`), and the note
names the tuple and the sizer's cause.
`an_unbounded_type_is_refused_under_proto3` pins the skip for `Overflow`, and
`tests/induced_call_refusals.rs` pins it under `proto3` alone and beside
`flatbuffers`, with the other interface faced; `generate_face_with`, which
refuses rather than skips, refuses with the same message
(`an_interface_with_an_unbounded_payload_is_refused`). One difference from the
FlatBuffers codec remains: that codec withholds the codec of a type whose bound
it cannot judge, while the proto3 sizer resolves cross-package references
through `others`, so no proto3 type is withheld silently.

The sizer names the cause. `NoBound` is a member with no finite bound: a
`string` or `bytes` with no length bound, such as the ros2 corpus's
`RosString : string [0..18446744073709551615]`, whose bound overflows the
string's byte count; a collection's count is always a bounded `u64`, so a
collection reaches `NoBound` only through its element, and `Overflow` through
its count. `Overflow` is a bound past `u32::MAX`, a sum or product of bounds
that does not fit, or a nesting past the sizer's depth limit. `RefusedMember` is
a member the `.proto` emitter refuses: an optional array or map field (ADR-0013
decision 7), an array of arrays or of maps, a map whose value is an array or a
map, a map key outside the integral and string scalars, an enum value or a
retired enum value outside int32, a field number proto3 reserves (19000 to
19999) or does not admit (0, or above 2^29 − 1), a stream, and a name that does
not resolve. The sizer checks a field number before the bound of the field's
type, so a field with a refused number is `RefusedMember` whatever its type. The
codec's own walk runs before the size state is read, and refuses an optional
array or map field, a refused field number and an enum value outside int32 with
a message that names the field or the value (`a_refused_member_is_still_refused`
pins the last). The codec alone refuses an optional array element and an
optional map value, which the schema emits without presence (ADR-0019 review
ruling 6), as the FlatBuffers codec does. An integer with no width, an enum with
no value and a union with no arm are refused as malformed IR; the compiler
produces none of them.

**The ros2 corpus does not generate with proto3.** Every ros2 package that
declares or reaches `RosString` is refused under `[Proto3]` with `NoBound`, and
only `builtininterfaces`, which declares no string, generates;
`the_ros2_corpus_is_refused_under_proto3` pins it. It is refused under
`[FlatBuffers]` too, for the same type. This is a known limit until the
platform-wide payload ceiling of driftsys/ridl#996, which is to cap such bounds
for both codecs, lands.

`descriptors.rs` emits `Sizes<Proto3>` for every faced interface of a package
that carries proto3, with each row a const expression over the payloads'
`<T as Payload<Proto3>>::MAX_SIZE`; a declaration with no bound refuses the
package before a descriptor is rendered, and an interface whose induced payload
has no bound is skipped, so no table names a `MAX_SIZE` that does not exist. The
face is generic over its port's encoding and changed in no line.

## The wire mapping

Every row is what `crates/ridl-backend-proto/src/lib.rs` emits for the same IR.

| typl / ridl shape                        | `.proto` emitted                                      | Wire                                                                        |
| ---------------------------------------- | ----------------------------------------------------- | --------------------------------------------------------------------------- |
| integer of width u8, u16, u32            | `uint32`                                              | varint                                                                      |
| integer of width u64                     | `uint64`                                              | varint                                                                      |
| integer of width i8, i16, i32            | `sint32`                                              | zigzag varint                                                               |
| integer of width i64, bare `integer`     | `sint64`                                              | zigzag varint                                                               |
| float f32 / f64, bare `float`            | `float` / `double`                                    | fixed32 / fixed64, little-endian                                            |
| boolean                                  | `bool`                                                | varint 0 or 1                                                               |
| string / bytes                           | `string` / `bytes`                                    | length-delimited, UTF-8 for a string                                        |
| named scalar at a field                  | its backing scalar, inlined                           | as its backing                                                              |
| enum                                     | `enum`, values prefixed, zero first or `_UNSPECIFIED` | varint of the value as int32 (a negative value is ten bytes, sign-extended) |
| enum set of width W                      | the scalar of width W, inlined                        | as that scalar                                                              |
| struct                                   | `message`, field number = ordinal                     | length-delimited                                                            |
| `?` on a scalar, enum, string, struct    | `optional`                                            | written when present, whatever its value                                    |
| array of a numeric, bool, enum, enum set | `repeated`                                            | packed: one record, a length-delimited run of values                        |
| array of string, bytes, struct, union    | `repeated`                                            | one record per element                                                      |
| map                                      | `map<K, V>`                                           | one entry message per pair, key = 1, value = 2                              |
| tuple                                    | induced message, fields `field_1`…, numbered from 1   | length-delimited                                                            |
| union                                    | `message` with `oneof value`, arm number = ordinal    | the one arm's record                                                        |
| retired field, arm or enum value         | `reserved`                                            | never written; read as an unknown field                                     |
| cross-package struct, enum, union        | `pkg.Name` and an import                              | as the local kind                                                           |

**What the two emitters share, and how agreement is kept.** A field number is
the ordinal both emitters read, a tuple field's number is its position from 1, a
map entry's key and value are 1 and 2, and a box's value is 1. The one fact two
emitters could pick differently is the scalar a width projects to, and that is
one table in `ridl_ir::projection::proto3`, which the `.proto` emitter, the
proto3 sizer and this codec all read. The codec's own walk of a message is not
shared with the `.proto` emitter: the emitter reads the IR and the codec reads
the codegen model, and porting the emitter onto the model is driftsys/ridl#690's
territory. What holds the two walks together is the drift test
`proto3_layout_matches_the_emitted_schema`: it compiles the emitted `.proto` of
every corpus package (`examples/cabin`, `crates/ridlc/tests/corpus/veh-cluster`,
`evals/corpus/mavlink`, `ros2` and `vss`) and of the conformance fixture with
`protox`, and compares each message field by field — number, scalar or message
name, label — with the layout the codec emitted from
(`ridl_backend_rust::proto3_layout`), keyed by the `.proto` message name. A
field-number or scalar drift fails it without running any generated code. Each
corpus entry lists the messages the codec does not lay out, with the reason; the
only one is the `fixed` tuple above.

## The writer

The writer writes the canonical form, which is what `prost` writes:

- Fields in field-number order, whatever their declaration order.
- A field with implicit presence (no `?`: a scalar, an enum, an enum set, a
  string, bytes, an array, a map) is omitted at its proto3 default — 0, `false`,
  the empty string or bytes, enum value 0, the empty set, the empty array or
  map.
- A `?` field is written when present, even at 0 or empty, and omitted when
  absent.
- A non-optional struct or tuple field is always written, as a record that may
  be empty, which is what `prost` writes for `Some(default)`.
- A union writes exactly its one arm, even at the arm's default.
- A map writes one entry per pair in the order of the domain value's
  `Vec<(K, V)>`, so nothing sorts. Inside an entry a key or a value at its
  default is omitted, and a message value is omitted when its own encoding is
  empty (`{"": 0}` is `a2 01 00`, as `prost` writes it).
- A box writes `value` under the implicit-presence rule, so a box of 0 is the
  empty message.

**A float at `-0.0` is written, unlike `prost`.** The default is compared by its
bits, as the protobuf reference implementations (C++, Java, Go) compare it, so
the sign of zero survives a round trip through this codec and through any of
those. `prost` 0.14 compares by value and omits `-0.0`, so a `-0.0` sent through
`prost` arrives as `+0.0`; `prost` reads the written `-0.0` correctly. This is
an owner ruling of 2026-10-20, and it is the one listed exception to byte
equality with `prost`: `minus_zero_differs_from_prost_as_designed` states it as
a case, and asserts that `prost` still omits it, so the exception is seen if it
can be retired.

**`encode` sizes first and writes forwards.** It calls the length function once,
returns `EncodeError::Capacity { needed, available }` with the exact `needed`
when `out` is too small, and otherwise writes, so `Encoded.bytes` is a
**prefix** of `out` (a FlatBuffers encoding is a suffix). A nested message's
length is computed by its own length function before its prefix is written,
which costs one recomputation per nesting level, so `encode` costs the encoded
size times the nesting depth rather than the size alone, the cost `prost` pays
the same way; the depth is fixed at generation time. The length arithmetic
saturates, so a value past its declared bound that is too large for a `usize`
asks for `usize::MAX` rather than wrapping. `encode` checks what the FlatBuffers
`encode` checks — an anonymous string's or byte sequence's length, a
collection's count and a map's key uniqueness — and returns
`EncodeError::Contract` on the same rules. It allocates nothing beyond what the
domain type owns. `ridl_rt::proto3::Writer` never panics: a write past the end
records the overflow and `finish` reports it, which only an undercounting length
function can reach.

## Presence, defaults and absence

proto3 forces the reader rule of driftsys/ridl#472 on every implicit-presence
field, not only on scalars: a conforming writer omits a non-optional field at
its default, so a reader must read an absent one as that default. Where the
default is not a legal value of the field's type, the absent field is
`Malformed::MissingRequired`. Legality is decided at generation time from the
declaration, so no check runs in the generated code:

| Field type                      | Absent reads as                     | Legal when                                                                                                          |
| ------------------------------- | ----------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| integer, float, named or inline | 0                                   | `ridl_ir::zero::range_holds_zero` over its range and step, the predicate the FlatBuffers codec and `ridl diff` read |
| boolean                         | `false`                             | always                                                                                                              |
| enum                            | value 0                             | the enum declares a zero member; a synthesized `_UNSPECIFIED` is not one                                            |
| enum set                        | the empty set                       | always                                                                                                              |
| string, bytes                   | empty                               | the length lower bound is 0; a declared pattern is then checked on the empty value                                  |
| array, map                      | empty                               | the count lower bound is 0                                                                                          |
| struct or tuple (not optional)  | the message decoded from zero bytes | each of its fields is legal when absent, by this table                                                              |
| union (not optional)            | —                                   | never: its empty message holds no arm                                                                               |
| any `?` field                   | `None`                              | always                                                                                                              |

The rule holds in every message: a struct, a tuple, a map entry's key and value,
a union arm's message and a box root, so an empty payload is a box of 0, or is
refused when the box's type excludes 0. A field whose default is not legal is
never at its default, so the writer writes it without testing it.

**proto3 reads five appends that `ridl diff` calls breaking.** The string, bytes
and collection rows go further than the FlatBuffers codec, which refuses any
absent offset. So an appended non-optional struct, string, array, map or tuple
field whose empty value is legal reads an old payload under proto3 and is
refused under FlatBuffers.
`every_append_ridl_diff_calls_compatible_reads_an_old_payload` in
`tests/proto3_reader.rs` runs the evolution fixture both codecs share: every
append `ridl diff` calls compatible reads an old payload, so the diff is never
more lenient than this codec, and the five it calls breaking that this codec
reads — `WithInner`, `WithLabel`, `WithList`, `WithMap` and `WithTuple` — are
pinned as a literal, so a change in either the diff or the codec is seen there.
`ridl diff` keeps its classification (ADR-0008 decision 14 as amended): it is
right for the stricter of the two codecs.

## The reader: what it accepts and what it refuses

`verify` accepts every conforming encoding: fields in any order; a repeated
numeric field packed, unpacked or both mixed, the elements concatenated in
order; a non-minimal varint (`80 00` for 0); a `bool` varint other than 0 or 1,
read as `true`, as `prost` reads it; and an unknown field of wire type 0, 1, 2
or 5 at any position, which is skipped. A field number protobuf reserves, or a
retired ordinal, is an unknown field like any other.

**Unknown fields are skipped and dropped** (ADR-0018 open item 3, as answered
for this codec). This is what lets a newer writer append a field and an older
reader still read the payload, which is why proto3 is the network encoding. The
decoded value has no place to keep a field it does not describe, and keeping it
would need an allocator in `no_std`. A gateway that must forward fields it does
not know forwards the bytes without decoding them, as `ridl-transport-ws` does.
For the same reason **`verify` applies no `Malformed::TooLarge`** to a proto3
payload: a buffer from a newer writer may exceed this version's `MAX_SIZE`
because of the fields this version skips, and
`a_newer_writers_payload_above_max_size_verifies` pins it. What protects a
receiver is the buffer its port reads into. `verify` is linear in the bytes it
reads, except map key uniqueness, which compares each entry's key with the keys
of the entries before it and so is quadratic in the map's entry count, as the
FlatBuffers codec's is; the count is checked before each entry's key is
compared, so the walk is bounded by the declared maximum and not by the buffer.
`verify` needs no run-time depth limit: a type is not recursive, so the
generated functions nest to a depth fixed at generation time, and skipping an
unknown length-delimited field does not recurse.

What `verify` refuses as malformed:

| Fault                                                                                                                                                              | Error                                                       |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------- |
| a length or a fixed-width value past the end, a truncated varint, a packed fixed-width run whose length is not a multiple of the width                             | `Malformed::OutOfBounds`                                    |
| a varint longer than ten bytes, or whose tenth byte is above 1; a tag wider than `u32`                                                                             | `Malformed::Varint`                                         |
| a varint wider than its field's scalar: a `uint32` above `u32::MAX`, a `sint32` or an enum outside `i32`                                                           | `Malformed::Varint` — `prost` truncates                     |
| a negative `int32` (an enum value) in the 5-byte form (`ff ff ff ff 0f`), which is not the protobuf encoding: a writer sign-extends to 64 bits and emits ten bytes | `Malformed::Varint` — `protoc` parsers and `prost` truncate |
| field number 0; wire type 3, 4, 6 or 7; a known field under a wire type its type does not take                                                                     | `Malformed::WireType` — `prost` skips a group               |
| a field that is not repeated written twice; a union message with two arms, or one arm twice                                                                        | `Malformed::Duplicate` — `prost` merges or keeps the last   |
| a string that is not UTF-8                                                                                                                                         | `Malformed::Utf8`                                           |
| an absent field whose default is not legal                                                                                                                         | `Malformed::MissingRequired`                                |

**The duplicate refusal is a deliberate deviation from protobuf** (an owner
ruling of 2026-10-20, stated as well in the rustdoc of `Malformed::Duplicate`).
protobuf's merge rule reads a repeated singular scalar as "last one wins", a
repeated message field as a merge of every occurrence, and two `oneof` arms as
"last one wins", and `prost` implements all three. This codec refuses each: a
payload is one value written once, no writer this toolchain or `prost` generates
writes a field twice, and the merge rule exists for concatenating serialized
messages, which is not how a frame carries a payload. Refusing keeps `verify`
and `decode` simple: `verify` tracks a fixed-size table of seen fields per
message, with no allocation, and `decode` reads the single occurrence. A foreign
writer that concatenates two encodings of one message, or a proxy that appends
an override field, is refused where `prost` accepts. Widening the reader later
is not a break. A map key repeated across entries is not a wire duplicate: it is
`Contract(Rule::Unique)`, as in the FlatBuffers codec.

Groups are refused rather than skipped because proto3 has no groups and no
schema this toolchain emits can produce one.

**Constraint checks.** `verify` checks every typl constraint the FlatBuffers
`verify` checks, at the same positions, through the same generated `check`
functions and the same inline checks: range, step, string length in characters,
bytes length, collection count, map key uniqueness, the pattern under
`validate-pattern`, an enum value that is declared, and an enum set with no
undeclared bit, each of the last two `Contract(Rule::Variant)` — the schema
leaves enums open and the codec closes them (ADR-0019 review ruling 7). A varint
that fits the wire scalar but not the declared width (300 in a `u8` field) is
`Contract(Rule::Range)`, because the width is derived from the range.

**`decode` is total.** It takes a `Ref`, and every read that could fail is
discharged with the neutral value of its type, which `verify` makes unreachable.
Each non-optional nested field starts as the message decoded from zero bytes, so
an absent field needs no second path; a present one replaces that default, which
costs one default sub-tree per nested field on each decode. **The view is the
verified bytes** (ADR-0021 open item 3, as answered):
`ridl_rt::proto3::Verified<'a>` implements `payload::View` and has no field
accessors. proto3 is not a random-access format, so an accessor would be a scan,
and the face reads every payload through `decode`. Accessors can be added later
without a break, as inherent methods of a generated type that does not exist
yet.

## Determinism

The same value encodes to the same bytes on every run and every target. Field
order is the field-number order the emitter fixes, a map's entries keep the
order of the domain `Vec<(K, V)>`, and every fixed-width read and write is
little-endian. `the_encoding_of_one_value_is_pinned_across_runs` pins the bytes
of one value — every scalar kind at its maximum, a negative enum value as a
ten-byte varint and the wide enum set — as a literal, and
`the_encoding_is_deterministic` encodes every named value of the value table
into a zeroed buffer and into one filled with `0xAB` and compares the two, so a
byte the writer leaves untouched cannot make two encodes agree by accident.

## Conformance

**The oracle is `prost` code generated from the emitted `.proto`, with no
`protoc`.** The conformance fixture `proto3_conformance.ridl`, with
`proto3_conformance_other.ridl` for a cross-package reference, is compiled;
`ridl_backend_proto::generate_with` emits its `.proto`; `protox` compiles that
into a `FileDescriptorSet`; and `prost_build::Config::compile_fds` with
`btree_map(["."])` writes Rust, so a map's entries are in key order. That Rust
is checked in as `tests/generated/proto3_conformance_prost.rs`, and
`the_checked_in_prost_code_is_what_prost_build_writes` regenerates it and
compares bytes (`RIDL_UPDATE_GENERATED=1` rewrites it). There is no `build.rs`,
and `protoc` stays out of every gate
([ADR-0006](../decisions/ADR-0006-walking-skeleton-execution.md) decision 3).
The oracle is generated from the emitted schema, never from a schema written for
the test, so a codec that agrees with `prost` agrees with the `.proto` a third
party reads. Like the FlatBuffers suite, this does not check the schema against
ADR-0017: that rests on the proto backend's own snapshots and the frozen `wire`
dump.

The generated codec runs as a program: each suite generates the fixture with
`encodings = [Proto3]`, appends a `main` that reads hex lines and writes hex
lines, and compiles it against `ridl-rt` with the `proto3` feature through the
bare-`rustc` mechanism every compile proof in the crate uses.

**The dev-dependencies, each test-only.** `ridl-backend-proto` emits the
`.proto`; `protox` compiles it; `prost-build` generates the oracle, in the
regeneration test only; `prost` is the runtime the oracle links; and `proptest`
draws the seeded values. `prost-reflect` is not a dev-dependency of its own: it
is reached as `protox::prost_reflect`, and reads a message of any corpus package
as a `DynamicMessage` without generating code per package.
`xtask/tests/oracle_boundary.rs` holds the line:
`the_runtime_reaches_no_prost_crate` resolves `ridl-rt` with every feature and
finds none of `prost`, `prost-derive`, `prost-build`, `prost-types`,
`prost-reflect`, `protox` or `protox-parse` at any edge kind;
`the_generated_crate_reaches_no_prost_crate` does the same for the two crates
`just demo` writes under `examples/cabin/` (`generated/` and
`generated-corpus/`, the latter from `veh-cluster`), ignored for a plain
`cargo test` and run by `just demo`; and
`the_rust_backend_takes_its_oracles_as_dev_dependencies_only` reads the
backend's own direct edges and requires every edge to `prost`, `prost-build`,
`prost-reflect`, `protox` or `ridl-backend-proto` to be a dev edge. That last
check is a direct-edge check because `ridl-ir` already depends on `prost` and
`prost-reflect` normally, so a closure walk could not see a promotion of either.

**The cases**, in the three suites:

1. **Byte equality** (`bytes_this_codec_writes_equal_prost_bytes`): for a table
   of hand-chosen values — every scalar kind at 0, at 1, at its minimum and at
   its maximum, NaN, a negative enum value, an empty and a multi-byte UTF-8
   string, every collection empty and full — this codec's bytes equal `prost`'s,
   and `prost` decodes them to the same value.
2. **The other direction** (`prost_bytes_decode_here`): `prost`'s bytes verify
   and decode here to the same value and re-encode to the same bytes.
3. **The `-0.0` exception** (`minus_zero_differs_from_prost_as_designed`),
   above.
4. **The property**
   (`prost_values_round_trip_through_this_codec_byte_for_byte`): 256 seeded
   draws of one value per message of the fixture, within its typl constraints,
   with a fixed seed so a failure reproduces; `prost` encodes, this codec
   decodes and re-encodes, and the bytes are equal.
5. **Coverage** (`the_fixture_holds_every_shape`,
   `the_inventory_reaches_every_shape`): the fixture holds each scalar width,
   each enum kind (with a zero member, without one, with a negative value), an
   enum set of width u8 and u64, nested structs, a tuple, a union with a scalar,
   string, enum, enum-set and struct arm, each `?` kind, packed arrays of every
   numeric, bool and enum, arrays of string, bytes, struct and union, a map for
   each admitted key kind, each box kind, a reference into the second package,
   and the induced roots of a call with several parameters, a query of none and
   a query with a tuple reply. A test lists the fixture against that inventory,
   so a shape cannot drop out unnoticed.
6. **The reader rules** (`tests/proto3_reader.rs`): unpacked and mixed runs,
   fields out of order, a `bool` of 2, unknown fields of each wire type at the
   start, the middle and the end, and a payload above `MAX_SIZE` from a newer
   writer.
7. **The refusals**: one hand-built buffer per row of the refusal table, each
   asserting the exact `VerifyError` and checked against `prost`, so the suite
   records where this codec is stricter.
8. **The absence rule**: one buffer per row of its table, for a field of a
   struct, of a tuple, of a map entry and of a box root;
   `whether_absence_reads_as_zero_follows_the_type`, one buffer read as structs
   that differ only in the field's type; and the evolution case above.
9. **Robustness** (`verify_never_panics_and_is_no_looser_than_prost`, ADR-0018
   decision 4): every truncation and every `0x00`, `0xff` and `0x80` replacement
   of every buffer of the value table, and 512 seeded random buffers, go to
   `verify` and `decode`. The program never aborts, and whenever this codec
   accepts a buffer `prost` accepts it too and the two read the same value, so
   this codec is only ever stricter.
10. **The corpus** (`corpus_defaults_round_trip_through_prost_reflect`): for
    every message of every corpus package whose generated type has a `Default`,
    the bytes this codec writes are decoded by `prost-reflect` over the `protox`
    pool of the emitted `.proto` and re-encoded to the same bytes; a default
    with a map of more than one entry is left out, because `prost-reflect`
    writes map entries in hash order. One program is built per corpus entry,
    over `examples/cabin`, the veh-cluster corpus, mavlink and vss; the ros2
    corpus is refused (above).
11. **The drift test** (`proto3_layout_matches_the_emitted_schema`), above.
12. **Determinism**, above.
13. **The bound** (`max_size_is_the_shared_bound`,
    `the_worst_case_value_fits_max_size`): each emitted `MAX_SIZE` is the
    sizer's number, and a worst-case value per fixture message — every
    collection at its maximum count, every string at its maximum length of
    four-byte characters, every varint at its widest — encodes within it. A
    worst case that does not fit is a sizer defect, and the fix goes there.
14. **The face over a proto3 port**
    (`signal_event_command_query_round_trip_over_a_proto3_loopback`): a signal,
    an event, a command and a query over `Loopback::<Proto3>::with_encoding`,
    against the fixture generated with both codecs; the bytes on the port are
    the proto3 bytes and not the FlatBuffers ones.

No fixture field holds a legal 32-bit NaN: the checker derives an `f32` width
only from a declared range, and a declared range excludes NaN.

**The mutations measured.** Each codec task's review mutated the codec and its
helpers in a worktree of its own and ran the suites. Every mutation below was
caught; the ones in the first group are the reason the oracle exists, because a
round trip through this codec alone passes under each of them:

- swapping two field numbers, numbering tuple fields from 0, writing a scalar at
  its default, omitting a union arm at its default, writing an empty map entry
  value, writing map entries in reverse order, and reading fixed-width values
  big-endian: each fails byte equality with `prost` or the decode of `prost`'s
  bytes, and the field-number and tuple mutations also fail the drift test;
- turning the bool read into `== 1`, accepting wire type 6, clamping a truncated
  field instead of refusing it, and planting a panic in the varint reader: each
  fails the robustness case, which compares this codec's verdict with `prost`'s
  on non-canonical bytes and reports an abort with its input;
- writing the output buffer with an OR rather than a copy: only the determinism
  case, with its `0xAB` buffer, fails;
- making a scalar leaf of the sizer one byte smaller: the worst-case case fails.

The reader's refusals were measured the same way: removing the field-number-0
check, the duplicate check for a nested message or for a second union arm, the
wrong-wire-type check, the declared-enum-value check, the `narrow_i32` refusal
(truncating as `prost` does), the absence legality of an enum with no zero
member, the step grid of the zero predicate, the verification of an absent
nested message, the no-arm refusal of a union, map key uniqueness and the
collection count — each fails the case written for it. So do dropping the
`int32` refusal of an enum value, a wrong `needed` in the capacity error, the
size tables (swapped proto3 rows, a proto3 table built from the FlatBuffers
`MAX_SIZE`, a face site that names `FlatBuffers` instead of its port's
encoding), and the refusal of a type without a bound (a codec given to a
`NoBound` or an `Overflow` state, a field number checked after the bound of the
field's type, a stream or an unresolved name attributed to `NoBound` instead of
`RefusedMember`). Three mutations first survived, and each review's fix added
the case that now catches it: the UTF-8 check of a string with no constraint
(the two branches were merged into one, which every UTF-8 case reaches), the
field-number refusal (pinned at the model level, because a checked source
reaches ordinal 19000 only through 19000 slots), and a packed fixed-width run
whose last chunk is one byte short
(`a_packed_run_cut_inside_a_varint_is_out_of_bounds` gained a seven-byte float
run and a fifteen-byte double run).

## `wasm32` and `no_std`

`ridl_rt::proto3` takes no dependency, allocates nothing and has no `unsafe`
code; `just wasm-check` and `just compat-check` cover it through their
`--all-features` invocations of `ridl-rt`. The emitted code names every
primitive and prelude type by its `::core::` or `::std::` path, and allocates
only where a domain type owns a `String` or a `Vec`, which the generated
`lib.rs` aliases to `alloc`. `the_generated_proto3_codec_checks_for_wasm32` runs
`rustc --target wasm32-unknown-unknown --emit=metadata` over the emitted source
under `just test`, as the FlatBuffers suite does for its codec. `examples/cabin`
and `crates/ridlc/tests/corpus/veh-cluster` carry `proto3` in their `encodings`,
so `just demo` builds, lints with clippy, documents and checks for
`thumbv7em-none-eabihf` a crate that carries this codec, and `just compat-check`
builds it at Rust 1.83 and edition 2021. The frozen copies under
`xtask/tests/freeze-fixtures/generated/` name `flatbuffers` only, and they
regenerate byte for byte.

## Known gaps

| Gap                                                                                                                                                                                 | Issue             |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------- |
| Neither codec generates a type with no finite bound, so the ros2 corpus packages do not generate with proto3; a platform-wide payload ceiling for both codecs is to cap such bounds | driftsys/ridl#996 |
| The codec does not reproduce the proto backend's name-collision refusals: a package the `.proto` emitter refuses for a name reason still gets a Rust codec                          | driftsys/ridl#997 |
| The `.proto` emitter and the codec walk a message separately, held together by the drift test rather than by one shared walk                                                        | driftsys/ridl#690 |

## Alternatives considered

The design note weighed each of these and rejected it; the reason is the one it
gave, checked against what was built.

- **Delegating the codec to `prost` in the generated crate.** ADR-0018 decision
  4 generates the codec, `prost` needs `alloc` and its own message types beside
  the domain types, and the guard exists to keep it out.
- **`protoc`-generated code as the oracle.** `protoc` stays out of every gate
  (ADR-0006 decision 3); `prost` through `protox` needs no external tool.
- **`prost-reflect` alone as the oracle.** It writes map entries in hash order,
  and a dynamic message is a second reading of the descriptor rather than the
  generated code a third party uses. It is kept for the corpus case.
- **A `build.rs` that generates the oracle at test time.** It would run `protox`
  on every build of the crate; the oracle is checked in with a regeneration
  test, as the FlatBuffers suite checks in its own.
- **A round trip only, with no byte equality.** proto3 has a canonical encoding
  for every case a writer controls, so byte equality is available and is the
  stronger check.
- **protobuf's merge rule for a field written twice**, and **skipping groups as
  `prost` does**: refused instead, for the reasons in the reader section.
- **Keeping unknown fields.** It needs an allocator and a place in the domain
  type; a gateway forwards the bytes.
- **Writing backwards, as the FlatBuffers builder does.** A varint length prefix
  has a size that depends on the length, so a backwards writer has to shift or
  reserve bytes; writing forwards gives an exact `needed`.
- **Accessors on the proto3 view.** Each would be a scan; they can be added
  later without a break.
- **A second proto3 sizer in the Rust backend instead of `Declaration.proto3`.**
  It would be two sources of one bound, and the plugin path has no IR.
- **Moving the `.proto` emitter's walk into `ridl-ir` now.** It needs the
  emitter ported onto the codegen model first (driftsys/ridl#690); the drift
  test covers the risk.
- **Omitting a float at `-0.0`, as `prost` does.** The sign of zero would be
  lost on every round trip, through this codec too.
- **Refusing an absent string or collection, as FlatBuffers refuses an absent
  offset.** Every proto3 writer omits an empty one, so the codec would refuse
  `prost`'s canonical bytes.
- **`Malformed::TooLarge` above `MAX_SIZE`.** A newer writer's skipped fields
  can take a payload past this version's bound.
- **Accepting a type with no finite bound through a size-free codec trait and a
  bounds table in `ridl-rt`.** This was the owner's first ruling of 2026-10-20,
  and it was built and then removed before the codec landed. The final ruling of
  the same day replaced it: a size ceiling binds typl, both codecs and the
  transport, so it is not a proto3-only change and belongs to driftsys/ridl#996;
  the trait also added two public `ridl-rt` traits and left the face unable to
  serve such a payload without an allocator.
- **Giving the ros2 corpus finite bounds in its source.** This was the owner's
  second ruling of 2026-10-20, also replaced by the final one: the ceiling of
  driftsys/ridl#996 caps such bounds for every package at once, instead of one
  corpus being edited by hand.
- **Accepting a type with no finite bound with `MAX_SIZE = usize::MAX` and an
  empty buffer.** `MAX_SIZE` would stop being the largest encoded size, and code
  that holds `BUFFER` would fail at run time.

## Trace

- Roadmap: [`../ROADMAP.md`](../ROADMAP.md); the issue is driftsys/ridl#264
- Decisions: [ADR-0017](../decisions/ADR-0017-proto3-projection-rules.md),
  [ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md)
  decisions 3 and 4 and open item 3,
  [ADR-0019](../decisions/ADR-0019-flatbuffers-projection-rules.md) review
  rulings 6, 7, 14 and 18,
  [ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
  decisions 5 and 9,
  [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decisions 7,
  8 and 10 and open item 3,
  [ADR-0023](../decisions/ADR-0023-interaction-face-generation.md),
  [ADR-0029](../decisions/ADR-0029-stability-policy.md)
- Related: driftsys/ridl#472 (the absence rule), driftsys/ridl#598 (the append
  rule of `ridl diff`), driftsys/ridl#690 (sharing the proto3 refusal rules),
  driftsys/ridl#713 (induced interaction tuples), driftsys/ridl#996 (the
  platform-wide payload ceiling), driftsys/ridl#997 (the name-collision
  refusals)
- Design records: [`flatbuffers-codec.md`](flatbuffers-codec.md),
  [`ridl-rt.md`](ridl-rt.md), [`interaction-face.md`](interaction-face.md),
  [`catalog-descriptor.md`](catalog-descriptor.md),
  [`codegen-plugins.md`](codegen-plugins.md)
- Archived design and plan:
  [`../archive/2026-10-20-proto3-codec-design.md`](../archive/2026-10-20-proto3-codec-design.md),
  [`../archive/2026-10-20-proto3-codec-plan.md`](../archive/2026-10-20-proto3-codec-plan.md)
