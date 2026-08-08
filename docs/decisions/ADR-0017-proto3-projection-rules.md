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

## Alternatives considered

| Candidate                                                       | Verdict  | Reason                                                                                                                                                                                                                           |
| --------------------------------------------------------------- | -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A wrapper message per named scalar, making every ref importable | rejected | +22 % on a `double` and +100 % on a small varint, and proto3 gives every message-typed field explicit presence, realising absence for fields that never declared `?` — the thing ADR-0013 decision 7 reserves for a declared `?` |
| Mapping `ridl.std` onto the protobuf well-known types           | rejected | `Duration` and `Timestamp` are seconds-and-nanos messages while the typl declarations are an `ms` float and an integer, so the mapping changes the wire encoding                                                                 |
| Refusing every cross-package reference in v0.1                  | rejected | the repository's own corpus is multi-package, so the backend would refuse real source                                                                                                                                            |
| An options extension carrying constraints structurally          | deferred | serves a consumer that does not exist; reopened by a consumer that must validate without the IR (decision 3)                                                                                                                     |
| Extending RIDL-149 to enum values and union arms                | deferred | the collision is a property of one target's namespace, and the extension carries a churn measurement of its own (decisions 4 and 5)                                                                                              |

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

## Open

1. **An inline scalar's constraint has no home.** Decision 3 makes comments the
   only home for constraint information, and an inline scalar has no named type
   to hang one on. Resolving it means either naming the anonymous type or
   admitting a second home.
2. **Whether `ridl-backend-rust` should transform struct field names.** It emits
   them verbatim, so a typl `currentSpeed` reaches generated Rust as
   `currentSpeed` and draws `non_snake_case` at every consumer. Adopting the
   pinned transform would rename a field on every generated struct — a breaking
   change to a shipped API, which is why E9.8 did not take it.

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
