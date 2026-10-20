# ADR-0017 — The proto3 projection: foreign references, constraints, and name totality

## Status

Accepted — 2026-08-08. Scope: three rules the first wire backend needed and no
earlier record supplied — how a reference into another package projects, where
the constraint information proto3 cannot represent goes, and what totality means
for a name rather than a number. The first and third bind every backend that
projects onto a namespaced target, so they are not proto3-scoped; the second is.

Written from roadmap story E9.8, which built `crates/ridl-backend-proto`. The
reasoning trail is
[`docs/archive/2026-08-08-proto3-projection-design.md`](../archive/2026-08-08-proto3-projection-design.md)
and its plan; read the design note as a design, not as a description of what
shipped — decision 2 below reverses a mapping the note specified and execution
implemented.

**Amended 2026-10-12 — decisions 6 to 8, from the review of the emitted mapping
before 0.10 freezes it.** The review is recorded in
[ADR-0019](ADR-0019-flatbuffers-projection-rules.md)'s status, with every ruling
it made. Three changes to this projection came out of it, each made because the
proto3 payload codec (driftsys/ridl#264) would otherwise have had to change a
schema that 0.10 freezes: a root message for a named scalar, an enum and an enum
set (decision 6), the `optional` keyword on an optional tuple field (decision
7), and `sint64` for a bare `integer` (decision 8). Decisions 1 to 5 are
unchanged, except that decision 1's statement that a named scalar emits no
declaration is corrected in place by decision 6. Decision 2's rejection of a
wrapper message still holds: it is about a field position, and decision 6's
message is never referenced from one.

**Amended 2026-10-20 — decision 7: the proto3 payload codec is built.** The
proto3 codec of the Rust backend (driftsys/ridl#264) now refuses what decision 7
said it would refuse, and it changed no emitted line. Decision 7 carries the
amendment in place.

This ADR was accepted under the delegated authority recorded in
[ADR-0005](ADR-0005-agent-enablement.md)'s working model.

It does not supersede [ADR-0013](ADR-0013-codegen-backend-scope.md) or
[ADR-0016](ADR-0016-schema-projection-and-the-name-transform.md). ADR-0013 fixes
what a backend may emit, ADR-0016 fixes how identity projects, and this record
fills three gaps both leave open.

## Context

ADR-0016 decision 6 ratified four properties every projection must satisfy, all
stated over **numbers**: deterministic, total, stable under compatible change,
injective in scope. E9.8 built the first backend to project onto a target with a
real namespace, and three questions arose that no record answered.

**A foreign reference has no rule.** ADR-0016 decision 2 places the name
transform in `ridl-ir` so every backend shares it, but says nothing about a
reference that crosses a package boundary. The design note assumed such a
reference emits a proto `import`, and that `ridl.std.Duration` and
`ridl.std.Timestamp` map onto the protobuf well-known types. Execution disproved
both.

**Constraint information has no home.** A named scalar inlines to its backing
scalar, so `type Speed : km/h [0.0..250.0 step 0.5]` reaches the schema as
`float`. The unit, the range and the step have no proto3 construct to occupy.

**Totality was stated over numbers only.** ADR-0016 decision 6 says a projection
is total or "the backend fails with a diagnostic rather than emitting a number
the target rejects". A schema can also be rejected for a **name**, and the first
implementation returned success carrying schemas `protoc` rejects in eight
distinct reachable cases.

## Decision

1. **A named scalar and an enum set inline whether local or foreign, and are
   never imported.** An import is emitted only for a foreign `struct`, `enum` or
   `union` — the kinds that emit a declaration another file can reference. A
   named scalar emits no declaration at all, so importing one names a type that
   does not exist in the imported file, which `protoc` rejects.

   **Corrected 2026-10-12 by decision 6.** A named scalar and an enum set now
   each emit one declaration, the root box message `<Name>Box`. No field names a
   box, so the rule above is unchanged: a field of one of the two kinds still
   inlines and still needs no import, and importing the file for the scalar's
   own name still names a type the file does not declare.

   This requires the backend to see the referenced packages, so `generate` gains
   a companion: `generate_with(package, others)`, with `generate(package)`
   retained as `generate_with(package, &[])` for the single-package case. A
   foreign reference the resolver cannot find fails with a diagnostic rather
   than emitting an unresolvable name. E9.9 and E9.11 inherit this API.

2. **`ridl.std` gets no special casing, and the protobuf well-known types are
   not used.** Every one of `ridl.std`'s members is a `type` alias over a
   primitive (`crates/ridl-core/assets/ridl_std.typl`), so each inlines through
   decision 1's ordinary path. This **reverses** the note's mapping of
   `ridl.std.Duration` and `ridl.std.Timestamp` onto `google.protobuf.Duration`
   and `google.protobuf.Timestamp`: those are seconds-and-nanos messages, while
   the typl declarations are an `ms` float and an integer, so the mapping
   changed the wire encoding relative to what the contract declares. The
   implemented version also refused every other `ridl.std` member, which would
   have rejected ordinary source.

   A wrapper message per named scalar was considered as the alternative that
   would make every reference importable, and rejected on two grounds. It costs
   a nested tag and length prefix on every value — measured at +22 % for a
   `double` and +100 % for a small varint, where varint encoding exists to make
   small values cheap. And in proto3 a message-typed field always carries
   explicit presence, so wrapping would give every scalar a presence bit and
   realise absence for fields that never declared `?` — which
   [ADR-0013](ADR-0013-codegen-backend-scope.md) decision 7 reserves for a `?`
   the contract actually states.

3. **Constraint information is carried as generated comments, and comments are
   its only home.** The unit, the range and the step appear as a comment at each
   use site. A published options extension over `google.protobuf.FieldOptions`,
   which would make the constraint readable through descriptor reflection, was
   considered and rejected for v0.1: it requires allocated extension field
   numbers and a published schema of our own, it pulls `descriptor.proto` into
   every emitted file, and it serves a consumer that does not exist. The IR is
   already the machine-readable contract, and E9.10 hashes the IR rather than
   the emitted schema.

   The forcing case that reopens this: a consumer that must validate payloads
   against the contract without access to the IR.

4. **ADR-0016 decision 6's totality property extends from numbers to names.** A
   projection is total when it is defined for every input **or the backend fails
   with a diagnostic** — and a target rejects a schema for a duplicate name as
   readily as for an out-of-range number. A backend projecting onto a namespaced
   target must therefore model that target's name scopes and refuse a collision,
   rather than emitting a schema the target rejects.

   For proto3 the scopes are three: **package scope** holds top-level message
   and enum names and, because proto3 scopes enum values as siblings of their
   enum rather than inside it, every enum's value names; **message scope** holds
   a message's field names together with its `oneof` names; and numbers, which
   decision 6 already covered.

   This is a **backend** obligation, not a language one. The collision is a
   property of one target's namespace, so it is refused where it arises rather
   than minting a diagnostic that would reject source for every target. That
   distinguishes it from ADR-0016 decision 3, where RIDL-149 rejects the package
   because the transform is fixed by the family and the collision is a property
   of the package under it.

5. **The name transform reached two namespaces without the paired check, and
   they are covered by decision 4 rather than by RIDL-149.** ADR-0016 decision 4
   requires a rule and its application to change in the same commit. E9.8 met
   that for struct fields, which joined RIDL-149. Enum value names and union arm
   names also began projecting through the transform and are guarded by decision
   4's backend check instead. Extending RIDL-149 to them would carry its own
   churn measurement and belongs to its own story.

   **Amended 2026-09-20 by driftsys/ridl#451.** The union-arm half is done.
   RIDL-149 now covers a union's arms, keyed on both pinned transforms, because
   the two are incomparable: `XY` and `x_y` collide under `camel_case` only, and
   `HTTPServer` and `httpServer` under `snake_case` only, so keying on either
   alone lets the other class through. The churn was one word added to
   RIDL-149's existing message on four lines of one corpus snapshot. Enum value
   names still rest on decision 4's backend check; that half is unchanged.

   **Amended 2026-09-26 by driftsys/ridl#506.** The enum-value half is done
   within one enum. RIDL-149 now covers an enum's values, keyed on `pascal_case`
   alone — the Rust backend's variant spelling since the same change. Proto's
   prefixed value is `snake_case` upper-cased, and every pair that collides
   under `snake_case` collides under `pascal_case`, so within one enum the one
   key covers this backend as well. Proto's namespace is wider than one enum:
   `enum A_B { C }` beside `enum A { B_C }` both give `A_B_C`, and a declared
   `UNSPECIFIED` value in an enum with no zero meets the synthesized
   `<PREFIX>_UNSPECIFIED`. Those stay with decision 4's backend check, which is
   unchanged.

6. **Every declaration that projects a type has a root message, and a named
   scalar, an enum and an enum set are rooted in a box** — added 2026-10-12. A
   struct's root is its message; a union's is its message with the `oneof`; a
   named scalar, an enum and an enum set are rooted in
   `message <Name>Box { <type> value = 1; }`, where `<type>` and the constraint
   comment are exactly what a field of type `<Name>` gets in the same package.
   The emitter writes the box for every declaration of the three kinds, in
   declaration order after the declared messages and enums, whether or not an
   interaction carries it, and claims `<Name>Box` in the package scope of
   decision 4, so a declared type of that name is refused. The `value` field is
   not `optional`: an absent value reads as any absent non-optional field does,
   as 0 when 0 is a legal value of the type and as missing when it is not. The
   proto3 size state of the three kinds is the bound of the box
   (`crates/ridl-ir/src/projection/size/proto3.rs`), so `NoMessage` is no longer
   produced; it stays defined, because the codegen protocol is frozen.

   This answers for proto3 the question ADR-0019 decision 8 answered for
   FlatBuffers, in the same shape and with the same name. Before it, no
   interaction whose payload is a named scalar or an enum had a proto3 message —
   every interaction of the VSS corpus, `signal currentSpeed : Speed`,
   `command setGear(position: GearPosition)` — so a codec had nothing to encode
   and a `prost` peer nothing to decode. Adding the box after the freeze would
   add a name to every package scope and refuse a package that declares it,
   which is why it is decided now. A box's bytes are those of the protobuf
   wrapper types (`google.protobuf.FloatValue` and its siblings), but those are
   not used, for decision 2's reason: an enum and a constrained scalar have
   none, and the wrapper names a different contract.

7. **An optional tuple field takes the `optional` keyword** — added 2026-10-12.
   A tuple field may carry `?` (typl §7.1, §11), and the FlatBuffers projection
   states it with `= null` (ADR-0019 decision 9). The induced tuple message
   emitted the field without `optional`, so a conforming writer omitted a
   present 0 and a `prost` peer read it as absent. The field is now emitted the
   way a struct field is, and an optional tuple field whose type is an array or
   a map is refused as the struct field is, because proto3 cannot mark a
   repeated or map field absent ([ADR-0013](ADR-0013-codegen-backend-scope.md)
   decision 7); the message names the tuple field's source path. One helper
   holds the refusal for both positions, and the rule it reads,
   `ridl_ir::projection::proto3::optional_repeated`, is the one the proto3 sizer
   reads, so a payload that holds such a field has no proto3 bound.

   An optional **array element** and an optional **map value** are not changed.
   proto3 has no construct for an absent element or an absent map value, so the
   schema emits the element type without presence, as the FlatBuffers schema
   does; the codec refuses the shape (the FlatBuffers codec already does, and
   the proto3 codec is to do the same), so no program this toolchain builds
   writes it.

   **Amended 2026-10-20 — the proto3 codec is built, and it refuses the shape.**
   An optional array element and an optional map value are a `GenerateError`
   from the Rust backend's proto3 codec, as from its FlatBuffers codec, so the
   sentence above holds for both codecs. The codec follows this record's mapping
   without changing it: the `wire` dump is byte-identical. The codec's wire
   mapping, and where it must agree with this projection, is
   [the proto3 codec design record](../design/proto3-codec.md). An optional
   array or map **field** of a struct or a tuple is refused by the codec with
   the refusal the `.proto` emitter gives (ADR-0013 decision 7 and the paragraph
   above).

8. **A bare `integer` projects to `sint64`** — added 2026-10-12. A field written
   `a : integer`, with no constraint, projected to `int64`, while
   `type Big : integer` — the same typl type — is given width `i64` by the
   checker and projected to `sint64`. One type had two wire encodings depending
   on its spelling, and `int64` costs ten bytes for every negative value, which
   ADR-0013 decision 4 rules out for a range that contains negatives. An
   unconstrained integer's range does. The largest encoding is ten bytes either
   way, so no size bound changes. The fallback for an integer width the IR does
   not define stays `int64`; the compiler produces no such width.

## Alternatives considered

| Candidate                                                       | Verdict                                                                                                            | Reason                                                                                                                                                                                                                           |
| --------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A wrapper message per named scalar, making every ref importable | rejected                                                                                                           | +22 % on a `double` and +100 % on a small varint, and proto3 gives every message-typed field explicit presence, realising absence for fields that never declared `?` — the thing ADR-0013 decision 7 reserves for a declared `?` |
| Mapping `ridl.std` onto the protobuf well-known types           | rejected                                                                                                           | `Duration` and `Timestamp` are seconds-and-nanos messages while the typl declarations are an `ms` float and an integer, so the mapping changes the wire encoding                                                                 |
| Refusing every cross-package reference in v0.1                  | rejected                                                                                                           | the repository's own corpus is multi-package, so the backend would refuse real source                                                                                                                                            |
| An options extension carrying constraints structurally          | deferred                                                                                                           | serves a consumer that does not exist; reopened by a consumer that must validate without the IR (decision 3)                                                                                                                     |
| Extending RIDL-149 to enum values and union arms                | union arms taken 2026-09-20 (driftsys/ridl#451); enum values taken 2026-09-26 (driftsys/ridl#506), within one enum | the collision is a property of one target's namespace, and the extension carries a churn measurement of its own (decisions 4 and 5) — for union arms that churn turned out to be one word on four snapshot lines                 |
| A proto3 root only for the declarations an interaction carries  | rejected                                                                                                           | the projection would depend on which interactions exist, against ADR-0016 decision 6's totality — the reason ADR-0019 decision 8 gives for FlatBuffers (decision 6)                                                              |
| Leaving the proto3 root to the payload codec, after the freeze  | rejected                                                                                                           | the box adds a name to each package scope, so adding it later refuses a package 0.10 accepts and changes a frozen schema (decision 6)                                                                                            |
| Refusing an optional array element or map value in the schema   | rejected                                                                                                           | no program this toolchain builds writes the shape, because the codecs refuse it; a refusal in the schema emitter would reject a package for a schema-only consumer and gain nothing (decision 7)                                 |

## Consequences

- **Positive — a foreign reference has one rule, and it is the same rule as a
  local one.** A named scalar inlines everywhere, so a type has one wire
  encoding regardless of which package uses it.
- **Positive — the totality property is now true for names.** Eight reachable
  inputs that returned success carrying a schema `protoc` rejects are refused
  with a diagnostic instead.
- **Negative — `generate`'s contract changed mid-story**, and E9.9 and E9.11
  inherit `generate_with`. Stated here so the next backend starts from the API
  rather than discovering it.
- **Negative — constraint information is not machine-readable on this target.**
  A consumer reading only the emitted schema cannot distinguish km/h from m/s.
  Decision 3 names the case that reopens it.
- **Negative — an inline scalar carries no constraint comment at all.** A field
  written `pressure : integer [0..100]` directly, rather than through a named
  type, has no name to attach a comment to, so its constraint is absent from the
  schema. Recorded as open below.
- **Positive — every declaration a payload can name has a proto3 message.** A
  proto3 codec and a `prost` peer agree on the root of every payload the face
  admits (decision 6).
- **Negative — every emitted schema gained a message per named scalar, enum and
  enum set**, and a package that declares `<Name>Box` beside `<Name>` is refused
  by this backend, as the FlatBuffers backend already refused it.
- **Negative — a package with an optional array or map tuple field is refused**,
  where the schema lost the `?` silently before (decision 7).

## Open

1. **An inline scalar's constraint has no home.** Decision 3 makes comments the
   only home for constraint information, and an inline scalar has no named type
   to hang one on. Resolving it means either naming the anonymous type or
   admitting a second home.
2. ~~**Whether `ridl-backend-rust` should transform struct field names.**~~
   **Answered 2026-09-20 by driftsys/ridl#451: yes.** The question read: it
   emitted them verbatim, so a typl `currentSpeed` reached generated Rust as
   `currentSpeed` and drew `non_snake_case` at every consumer; adopting the
   pinned transform would rename a field on every generated struct — a breaking
   change to a shipped API, which is why E9.8 did not take it. The rename was
   taken (driftsys/ridl#243), and it is breaking, which is why both commits
   carry `!`. Tuple field names went with them, for the same reason: typl §15.1
   gives a tuple field the same camelCase spelling as a struct field.

## References

- [`docs/archive/2026-08-08-proto3-projection-design.md`](../archive/2026-08-08-proto3-projection-design.md)
  — the reasoning trail; read as a design, not as what shipped
- [ADR-0013](ADR-0013-codegen-backend-scope.md) — the emit ceiling this record
  works within; decision 7 (field absence) is what decision 2's wrapper
  rejection turns on
- [ADR-0016](ADR-0016-schema-projection-and-the-name-transform.md) — decision 6
  (the four properties, whose totality clause decision 4 extends), decision 3
  (RIDL-149, which decision 5 distinguishes from), decision 4 (rule and
  application in one commit)
- [`docs/ROADMAP.md`](../ROADMAP.md) — E9.8 (the story this record comes from),
  E9.9 and E9.11 (which inherit decision 1's API)
- `crates/ridl-backend-proto/src/lib.rs` — the implementation
