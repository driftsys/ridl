# Design: the proto3 projection in the codegen model

Status: draft for approval, 2026-11-18. Tracks driftsys/ridl#1413. Related:
driftsys/ridl#690 (the proto3 rules are implemented in three places),
driftsys/ridlc-gen-kotlin#133 (the consumer that is blocked).

## Intent

A codegen plugin reads `ridl.codegen.v1.Model` and, by the rule in the header of
`model.proto`, derives no typl rule itself. The model carries the FlatBuffers
layout (`Model.flatbuffers`, field 8) and the podbuf layout (`Model.podbuf`,
field 11). For proto3 it carries only size states. A plugin therefore cannot
emit a proto3 codec that is byte-for-byte equal to the one the Rust backend
emits, because the field numbers, the scalar of each member, the packing of each
array and the refusals are computed inside `ridl-backend-rust`.

This design adds `Model.proto3`, a `Proto3Projection`, and makes the Rust proto3
codec read it. After the change, the Rust backend and every plugin read the same
facts, so they cannot differ on them.

## Goals

1. `Model.proto3` carries, for every message the proto3 codec writes, each
   field's number, its wire form (scalar, wire type, message, array, map), and
   whether an array is packed.
2. `Model.proto3` carries every declaration and tuple that proto3 refuses, with
   a cause a plugin can branch on and the diagnostic text the Rust backend
   reports.
3. One function in `ridl-ir` computes the projection. `codec_proto3.rs` reads
   the projection and holds no rule of its own for a field number, for the
   proto3 scalar of a member, for packing, or for what proto3 can represent. It
   keeps what is specific to the Rust output; see "What stays in the Rust
   codec".
4. The change is additive under the IR specification §6: `SCHEMA` stays
   `ridl.codegen.v1`, and `just freeze-check` passes with added lines only.

## Non-goals

- Moving `ridl-backend-proto` (the `.proto` emitter) or
  `ridl-ir/src/projection/size/proto3.rs` (the sizer) onto the new function.
  That is the rest of driftsys/ridl#690. This design reduces three copies of the
  rules to two plus the new single source; the existing drift tests keep the
  remaining two in agreement.
- Any change to the bytes the Rust proto3 codec writes or accepts.
- Any change to the size states (`Declaration.proto3`, `PayloadSizes.proto3`,
  `Reservation.proto3`) or a new value in `AbsentCause`. A new value in an
  existing enum is a break outside a minor release (`freeze/README.md`).
- Facts a plugin already has from the model: presence (a `?` field), typl
  constraints, and the count bounds of collections.
- The TypeScript backend.

## Anti-goals

- A projection that the Rust codec does not read. It would be a fourth copy of
  the rules, equal only while a test says so.
- A projection gated on the manifest's `encodings`. `Model.flatbuffers` and
  `Model.podbuf` are filled for every package; `Model.proto3` is too. The
  `encodings` option controls only what a backend emits.

## Current state

- `crates/ridl-ir/src/projection/proto3.rs` holds helpers only: the `Scalar` and
  `WireType` enums, `scalar`, `int_width_scalar`, `float_width_scalar`,
  `enum_set_scalar`, `primitive`, `Scalar::admitted_as_map_key`,
  `optional_repeated`, `PROTO_RESERVED` and `PROTO_MAX_FIELD_NUMBER`. It has no
  per-declaration result and no refusal type.
- `crates/ridl-backend-rust/src/codec_proto3.rs` walks the model's `Struct`,
  `Union`, `InducedTuple` and `Type` in `Codec::message_of` and builds its own
  `Wire` tree. It assigns numbers (a struct field's and a union arm's ordinal, a
  tuple position plus 1, map key 1 and value 2, box `value = 1`), packs an array
  whose element is a scalar, an enum or an enum set, and refuses with a
  `GenerateError` that has free text and no code.
- `ridl-backend-proto` and `projection/size/proto3.rs` each implement the same
  rules again, over the raw IR.
- The copies differ in one place that is known: the Rust codec refuses field
  number 0, and the `.proto` emitter does not.
- `ridl_backend_rust::proto3_layout` (`#[doc(hidden)]`) exposes the codec's
  message layout to `tests/proto3_conformance.rs`, which compares it with the
  emitted `.proto` over the `CORPUS` list of that test.

## Approaches considered

**A. By construction, in two pull requests (chosen by the owner, 2026-11-18).**
Pull request 1 adds the walk to `ridl-ir`, the model messages, and a test that
compares the projection with `proto3_layout`. Pull request 2 makes
`codec_proto3.rs` read `Model.proto3` and deletes its own rules. The first pull
request alone unblocks the plugin; the second meets "the two cannot differ".
This is the form podbuf already has: `codec_podbuf.rs` and `ridl-backend-podbuf`
both read `Model.podbuf`.

**B. By test only.** Pull request 1 alone. Cheaper, but the projection is a
fourth implementation of the rules and agrees with the codec only on the inputs
the corpus holds. Rejected: it adds to the debt #690 records.

**C. Port all three consumers at once** (codec, `.proto` emitter, sizer). This
closes #690 fully but triples the size and the review surface of a change whose
purpose is to unblock a plugin. Rejected for this work; left to #690.

## Design

### Where the rules live

The walk is a new file, `crates/ridl-ir/src/codegen/proto3.rs`, with one entry
point, `project(model: &v1::Model) -> v1::Proto3Projection`. It reads the
lowered model and nothing else: `Model.foreign` holds the full declaration of
every referenced declaration of another package, so the model is enough. It
calls the helpers of `crates/ridl-ir/src/projection/proto3.rs` for the scalar
table, the map-key set and the field-number limits. `codegen/lower.rs` sets
`model.proto3` last, after `model.podbuf`.

The walk reads the model and not the raw IR because the Rust codec reads the
model today and its refusal texts name model-level things. The `.proto` emitter
already lowers a model, so it can move onto the projection later (#690).

The cost of this choice, accepted by the owner on 2026-11-19: the proto3 sizer
(`projection/size/proto3.rs`) reads the raw IR and runs while the model is being
built, so it cannot call a walk that needs the finished model. Moving the sizer
onto the projection later needs a step after lowering that corrects the size
states from `refused`, as `patch_flatbuffers` corrects the model from the
FlatBuffers projection. That step is not designed here.

### The model messages

`Model` gains `Proto3Projection proto3 = 12;`. Every name below is new, so the
freeze dump only gains lines. No `map<>` field is used (a `map<>` field is a
break under the IR specification §6).

```proto
// The proto3 layout of the package: every message the proto3 codec writes,
// and every declaration and tuple proto3 cannot represent. Set whenever the
// toolchain lowers a package, whatever `encodings` the manifest selects.
message Proto3Projection {
  // In `Model.declarations` order, then every induced tuple in
  // `Model.tuples` order, whether or not a declaration or an interaction
  // reaches it. Map entries are not listed: an entry's key is field 1 and
  // its value is field 2, and `Proto3Map` carries both wires.
  repeated Proto3Message messages = 1;
  // The declarations and tuples that have no message, in the same order.
  // A message that needs a refused one is refused too, transitively, with
  // the cause REFUSED_DEPENDENCY, and is not in `messages`.
  repeated Proto3Refused refused = 2;
}

message Proto3Message {
  // The `.proto` message name without its package, as `ridl-backend-proto`
  // emits it: the declaration's name, `<Name>Box` for a root box, the wire
  // name of a tuple.
  string name = 1;
  // In field-number order, the order the canonical writer writes. The
  // numbers are not dense: a reserved or retired slot has no field.
  repeated Proto3Field fields = 2;
  oneof source {
    uint32 struct_declaration = 10; // index into Model.declarations
    uint32 union_declaration = 11;  // its fields are the arms of `oneof value`
    uint32 root_box = 12;           // `<Name>Box { <type> value = 1; }`
    uint32 tuple = 13;              // index into Model.tuples
  }
}

message Proto3Field {
  uint32 number = 1;
  Proto3Wire wire = 2;
  oneof source {
    string field = 10;          // a struct field, by its source name
    string arm = 11;            // a union arm, by its source name
    uint32 tuple_position = 12; // 0-based; the number is position + 1
    bool box_value = 13;        // the one field of a root box
  }
}

message Proto3Wire {
  // Whether the proto3 default of this wire is a legal value of its type:
  // 0 inside the range and on the step grid of a numeric scalar, a length
  // lower bound of 0 for text and bytes, a zero member or an open enum, a
  // count lower bound of 0 for an array or a map, always for an enum set and
  // for a message. An absent field with implicit presence, map key or map
  // value reads as the default when this is true and is malformed when it is
  // false; a writer omits a value at its default only when this is true.
  bool default_is_legal = 1;
  oneof kind {
    Proto3Scalar scalar = 10;     // numeric, boolean, or an enum set's integer
    bool text = 11;               // `string`, wire type LEN
    bool bytes = 12;              // `bytes`, wire type LEN
    Proto3Enum enumeration = 13;  // a proto3 enum, an int32 varint
    Proto3MessageRef message = 14;
    Proto3Array array = 15;
    Proto3Map map = 16;
  }
}

message Proto3Scalar {
  Proto3ScalarType type = 1;
  Proto3WireType wire_type = 2;
}

// The enum may be one of another package; `TypeRef` says which.
message Proto3Enum { TypeRef type = 1; }

// A nested message: a struct, a union or an induced tuple, of this package
// or of another one.
message Proto3MessageRef {
  oneof target {
    uint32 message = 10;  // index into Proto3Projection.messages
    uint32 foreign = 11;  // index into Model.foreign
  }
}

message Proto3Array {
  Proto3Wire element = 1;
  // True when the canonical writer writes one packed record: the element is
  // a scalar or an enum. False: one record per element.
  bool packed = 2;
}

message Proto3Map {
  Proto3Wire key = 1;    // entry field 1
  Proto3Wire value = 2;  // entry field 2
}

message Proto3Refused {
  oneof source {
    uint32 declaration = 10;
    uint32 tuple = 11;
  }
  Proto3RefusalCause cause = 2;
  // The member the cause is about, when there is one: `Owner.field`.
  optional string member = 3;
  // The diagnostic text. For every cause but REFUSED_DEPENDENCY, the Rust
  // backend reports this text unchanged. For REFUSED_DEPENDENCY the text is
  // "`<name>` needs the refused message `<other>`: <the message of other>",
  // the form `PodbufRefused.cause` has.
  string message = 4;
}

enum Proto3ScalarType {
  PROTO3_SCALAR_TYPE_UNSPECIFIED = 0;
  PROTO3_SCALAR_TYPE_BOOL = 1;
  PROTO3_SCALAR_TYPE_INT64 = 2;
  PROTO3_SCALAR_TYPE_UINT32 = 3;
  PROTO3_SCALAR_TYPE_UINT64 = 4;
  PROTO3_SCALAR_TYPE_SINT32 = 5;
  PROTO3_SCALAR_TYPE_SINT64 = 6;
  PROTO3_SCALAR_TYPE_FLOAT = 7;
  PROTO3_SCALAR_TYPE_DOUBLE = 8;
}

enum Proto3WireType {
  PROTO3_WIRE_TYPE_UNSPECIFIED = 0;
  PROTO3_WIRE_TYPE_VARINT = 1;
  PROTO3_WIRE_TYPE_I64 = 2;
  PROTO3_WIRE_TYPE_LEN = 3;
  PROTO3_WIRE_TYPE_I32 = 4;
}

enum Proto3RefusalCause {
  PROTO3_REFUSAL_CAUSE_UNSPECIFIED = 0;
  PROTO3_REFUSAL_CAUSE_OPTIONAL_COLLECTION = 1;   // `T[]?` or a map field `?`
  PROTO3_REFUSAL_CAUSE_OPTIONAL_ELEMENT = 2;      // optional element, key or value
  PROTO3_REFUSAL_CAUSE_NESTED_COLLECTION = 3;
  PROTO3_REFUSAL_CAUSE_MAP_KEY = 4;               // not integral and not string
  PROTO3_REFUSAL_CAUSE_ENUM_VALUE = 5;            // outside int32, live or retired
  PROTO3_REFUSAL_CAUSE_FIELD_NUMBER = 6;          // 0, 19000..=19999, > 536870911
  PROTO3_REFUSAL_CAUSE_REFUSED_DEPENDENCY = 7;    // needs a refused message
  PROTO3_REFUSAL_CAUSE_OTHER = 8;                 // see `message`
}
```

Notes on the shape:

- `Proto3ScalarType` has no `STRING` and no `BYTES`: those are the `text` and
  `bytes` arms of `Proto3Wire`, as in `FbWire`. The projection table's `String`
  and `Bytes` results map to those arms.
- `Proto3Scalar.wire_type` is derivable from `type`, and is carried so that a
  plugin holds no table. The wire type of every other arm is fixed and stated in
  the comment of the arm (`LEN` for text, bytes, a message, a map entry and a
  packed array; `VARINT` for an enum).
- `OTHER` covers every refusal that is not a proto3 rule but that the codec
  reports today for an input it cannot walk: a union with no arm, an enum with
  no value, a field or an arm with no type, an unresolved reference. A plugin
  reports `message` for it.
- A member that uses an enum of another package whose value is outside int32
  refuses the local declaration that holds the member, with `ENUM_VALUE`,
  `member` naming the local `Owner.field`, and the text the codec reports today,
  which names the other package's enum.

### What the projection and the size state each say

The two are independent, as they are for podbuf:

- A declaration with a refused member is in `refused`, and its size state is
  `absent` with `ABSENT_CAUSE_REFUSED_MEMBER`, as today.
- A declaration that is representable but has no finite bound is in `messages`,
  and its size state says `unbounded` or `absent`, as today. A backend still
  refuses to emit a codec for it from the size state.

### Whether a default is legal

Added on 2026-11-19, after a check of the Kotlin plugin. The first draft left
this fact out, on the claim that a plugin already has it. The claim was wrong:
the model does not carry it (`FbSlot.needs_null_default` is a different
predicate, about the `.fbs` schema), the Rust backend computes it per kind
(`zero_is_legal`, `Wire::default_is_legal`), and the Kotlin plugin holds a hand
port of the rule for scalars and enums only. A proto3 codec needs it for every
kind, so `Proto3Wire.default_is_legal` carries it. The walk fills it with
`ridl_ir::zero::range_holds_zero` for a numeric scalar and with the reads the
comment of the field lists for the other kinds. It sits on the wire and not on
the field because the rule also applies to the key and the value of a map entry.
The comparison test of pull request 1 cannot check it, as it cannot check
packing; pull request 2 makes the Rust codec read it.

### What stays in the Rust codec

These are facts of the Rust output or of the size states, not proto3 layout
rules, and pull request 2 leaves them in `codec_proto3.rs`:

- The Rust representation of each field: `Repr`, `NamedScalar` and its
  constructor, the typl constraint, and the count bounds of an array or a map.
  Whether a default is legal is not in this list: the codec reads
  `Proto3Wire.default_is_legal`. The codec gets them by walking the model type
  of each field beside the projection field, joined on `Proto3Field.source` (the
  field name, the arm name, the tuple position).
- `bound_state`, which refuses a declaration whose size state is not bounded.
- Which tuples are emitted and in which order (`reachable_tuples`). The codec
  looks up the projection message of each tuple it emits by index; it does not
  iterate `Proto3Projection.messages`.
- Which refusal ends the generation. The codec still stops on one error. It
  reports the first entry of `refused` whose cause is not `REFUSED_DEPENDENCY`
  and whose source is a declaration or a tuple the codec emits. A refused tuple
  that the codec does not emit does not stop it, as today. When a package holds
  several refusals, the one reported may differ from the one reported today; the
  text of each is unchanged.

### Field number 0

The projection refuses field number 0 with `FIELD_NUMBER`, as the Rust codec
does. The `.proto` emitter keeps its current behaviour; aligning it belongs to
#690. This is the author's decision, taken because the Rust codec is the
reference for byte equality.

### Pull request 1: the projection

- The walk in `codegen/proto3.rs`, the field in `lower.rs`, and the messages in
  `model.proto`.
- `cargo xtask freeze update`, with `freeze/codegen-protocol.txt` gaining lines
  only.
- Tests:
  - For `examples/cabin`, every package under `crates/ridlc/tests/corpus`, the
    three `evals/corpus` packages and the `proto3_conformance` fixtures: every
    message `ridl_backend_rust::proto3_layout` reports is in `Model.proto3` with
    the same numbers, type and label, and every declaration message of
    `Model.proto3` is in the layout. The test derives the label from the
    projection wire and the model's `?`, and the qualified name from the package
    name. A tuple may be in the projection and not in the layout, because the
    codec emits only the tuples it reaches. Where `proto3_layout` returns an
    error that is a projection refusal, `refused` holds an entry whose `message`
    is that error's text; an error from `bound_state` is not one.
  - One unit test per refusal cause, on the smallest input that draws it,
    asserting `cause`, `member` and `message`.
  - Every index in the projection (`declaration`, `tuple`, `message`, `foreign`)
    is inside its list, over the corpus, and the projection survives the JSON
    round trip, as the podbuf tests in `crates/ridlc/tests/codegen_model.rs`
    check.
- Docs, each describing only what this pull request builds:
  - `docs/design/codegen-plugins.md`: a section "The proto3 projection", after
    "The podbuf projection" and with the same parts.
  - `docs/book/codegen-plugins.md`: a subsection "The proto3 layout", and the
    "What is not available yet" list.
  - `docs/design/proto3-codec.md`: a row for `Model.proto3`, and the section
    "What the two emitters share" stating that the codec still walks on its own
    until pull request 2.
  - The module docs of `projection/proto3.rs` and `projection/size/proto3.rs`.

### Pull request 2: the Rust codec reads the projection

- `codec_proto3.rs` builds its `Wire` tree from `Model.proto3` and reports
  `Proto3Refused.message` as its `GenerateError`. `refuse_optional_repeated`,
  `refuse_optional`, `field_number`, the nested-collection and map-key checks,
  the int32 check and the calls to `pb::primitive`, `pb::int_width_scalar` and
  `pb::float_width_scalar` are removed from the backend.
- Acceptance: the generated crates are byte-identical before and after (the
  `generated-crate` freeze dumps and the backend snapshots do not change), every
  existing refusal test passes with its message unchanged, and
  `tests/proto3_conformance.rs` passes.
- The comparison test of pull request 1 is then true by construction. It is
  replaced by a test that fails if `codec_proto3.rs` calls a numbering or
  scalar-selection helper of `projection::proto3` directly.
- Docs: `docs/design/proto3-codec.md` and the module doc of `codec_proto3.rs`
  state that the codec reads the model.

### Release

The issue is done when the change is in a tagged release. The release itself is
the owner's step and follows the release procedure; both pull requests are
additive and need no breaking note. After the release, a comment on
driftsys/ridlc-gen-kotlin#133 names the version to pin and answers open question
1 of its design note.

## Constraints

- `model.proto` is a frozen surface (ADR-0029): additive changes only.
- ADR-0020 decisions 8 and 9 define the model; the addition is recorded as a
  dated additive amendment under decision 9, as the earlier additions were.
- ADR-0017 holds the proto3 projection rules. This design changes none of them.
- Shipped docs name no story id and describe the system as built.

## Left to the builder

- The internal Rust types of the walk's result.
- The exact order of `refused` when one declaration has several refused members.
  The Rust backend reports the first one it meets; the first entry for a
  declaration must be that one.
- How a tuple that only an interaction reaches, and whose root is not bounded,
  is treated: the Rust codec does not walk it today. The projection walks it;
  pull request 2 must keep the codec's current output for it.

## Trace

- Closes: driftsys/ridl#1413 (with pull request 2 and the release).
- Reduces: driftsys/ridl#690.
- Unblocks: driftsys/ridlc-gen-kotlin#133.
- Records to amend when gardened: ADR-0020 decision 9 (dated amendment),
  `docs/design/codegen-plugins.md`, `docs/design/proto3-codec.md`,
  `docs/book/codegen-plugins.md`.
