# IR Specification

**The encodings of the intermediate representation, its canonical form, and the
stability it promises** — what a consumer outside this repository reads, what
byte-identical means for it, how deep it nests, which schema changes it may
expect without being rebuilt, and how a change it cannot absorb is versioned.

Version: 0.1.0 — Draft

> **Provenance.** This document is roadmap story E4.5a (driftsys/ridl#321). It
> writes down the canonical-form policy of
> [ADR-0014](../decisions/ADR-0014-ir-encodings.md) decision 9, as that decision
> was amended on 2026-09-22: the canonical encoding is canonical protobuf JSON,
> and the binary and prototext encodings are derived. The reasoning trail,
> including the measurement that moved the decision, is
> [the IR stability design note](../wip/2026-09-22-ir-stability-design.md).
> Where this document and an ADR disagree, the ADR wins and this document is
> corrected.

> **Partial.** The document inventory
> ([family overview](ridl-family-overview.md) §2) gives the IR specification
> four subjects: serialization, the plugin protocol, the diff categories, and
> the canonical encoding. This version owns the first and the last, which is
> what story E4.5a asked for. The plugin protocol is story E4.5b and is
> specified where that story lands; the diff categories are stated today by the
> [typl language reference](typl-language-reference.md) §7.4, the
> [ridl language reference](ridl-language-reference.md) §11 and the
> [rsdl language reference](rsdl-language-reference.md) §14, and move here when
> a story moves them.

---

## Table of Contents

1. [Scope](#1-scope)
2. [The Encodings](#2-the-encodings)
3. [What Canonical Fixes](#3-what-canonical-fixes)
4. [The Nesting Bound](#4-the-nesting-bound)
5. [The Derived Encodings](#5-the-derived-encodings)
6. [The Compatibility Rule](#6-the-compatibility-rule)
7. [Versioning](#7-versioning)
8. [Reading the IR from Another Language](#8-reading-the-ir-from-another-language)
9. [Conformance](#9-conformance)

---

## 1. Scope

The schema of record is the protobuf in `crates/ridl-ir/proto/`, compiled with
`prost` (ADR-0004 §4). This document does not restate it; it states how that
schema is encoded on a surface a consumer reads, and what may change under a
consumer without breaking it.

It binds three schema packages:

| Package           | Messages                                        | Artifacts                                                |
| ----------------- | ----------------------------------------------- | -------------------------------------------------------- |
| `ridl.ir.v2`      | `Package` (`ir.proto`)                          | `<base>.ir.{json,txtpb,binpb}`, every committed baseline |
| `ridl.ir.v2`      | `System` (`system.proto`, ADR-0022)             | `<pkg.Name>.system.{json,txtpb,binpb}`                   |
| `ridl.codegen.v1` | the lowered codegen model and its two envelopes | the request and response of a codegen plugin             |

The word **IR** below means all three unless a sentence says otherwise.
`ridl.codegen.v1` does not exist yet: it is defined by roadmap stories E4.5a's
successors (the lowered model, then the plugin contract). The rules here are
written for it in advance, because the point of stating them is that a plugin
built outside this repository can rely on them before it is written.

## 2. The Encodings

Three encodings of the same schema are emittable (ADR-0014 decision 4). One is
canonical; the other two are derived from it in the sense that they carry the
same message and are produced by the same writer, and that where they disagree
with the canonical encoding about what this toolchain can read back, the
canonical encoding is the one that holds.

| Encoding                | Flag value  | Extension | Status    | Use                                                 |
| ----------------------- | ----------- | --------- | --------- | --------------------------------------------------- |
| canonical protobuf JSON | `ir-json`   | `.json`   | canonical | interchange, baselines, goldens, a plugin's request |
| protobuf binary         | `ir-binary` | `.binpb`  | derived   | a compact form for a consumer that wants one (§5)   |
| prototext               | `ir-text`   | `.txtpb`  | derived   | inspection (§5)                                     |

**Canonical protobuf JSON is the canonical encoding.** It is the protobuf JSON
mapping applied to the schema of record, as §3 fixes it. Two facts decided this,
both measured on 2026-09-22 and recorded in the design note:

- It round-trips every IR this toolchain's front end admits. The binary encoding
  does not: its reader stops at 100 message levels below the root, which legal
  source reaches (§5).
- Every consumer named by any record of this repository — Rust, Kotlin,
  TypeScript, and the Deno, Python and JVM plugin hosts ADR-0020 decision 10
  lists — parses JSON with its standard library, so reading the IR needs no
  protobuf toolchain. A typed reader exists where one is wanted: protobuf-java's
  `JsonFormat`, protobuf-es and protobuf-python's `json_format` all implement
  the same mapping.

The cost is size and speed: on the two `veh-cluster` corpus packages, binary is
6 to 11 times smaller and 5 to 9 times faster than JSON, and the largest of them
is 55 kB of JSON, 64 µs to write and 239 µs to read. No path in this toolchain
carries that cost measurably — a build writes each artifact once, and a plugin
process costs a millisecond to start before it reads anything.

## 3. What Canonical Fixes

A canonical artifact is **byte-identical for equal IR across runs, hosts and
platforms**. That claim rests on six properties. Each is part of the contract,
and a change to any of them is a breaking change under §6.

1. **The mapping.** The protobuf JSON mapping over the schema of record: field
   keys in `lowerCamelCase` as the mapping derives them from the `.proto` field
   names (`json_name` is never set), enum values as their names, a `oneof`
   flattened to its member's key, 64-bit integers (`int64`, `uint64`) as decimal
   strings (ADR-0014 decision 8), `bytes` as base64, and `string` as UTF-8 with
   only the escapes RFC 8259 requires. The schema declares no `float` and no
   `double` field — every numeric value in the IR is an exact decimal string
   (ADR-0007 decision 9) — so no float formatting rule is needed and none is
   stated.
2. **Presence.** A non-`optional` field holding its default is emitted; an unset
   proto3 `optional` field, an unset message field and an unset `oneof` produce
   no key. `null` never appears (ADR-0014 decision 2).
3. **Order.** Fields appear in the order the `.proto` declares them within each
   message. A repeated field's elements appear in the order the writer holds
   them, which for every list in the IR is source order or an order the schema's
   own comments name per field. The schema declares no `map<>` field; adding one
   is a breaking change (§6), because a map's key order would need a rule this
   list does not have.
4. **Layout.** Pretty-printed: two-space indentation, one field per line, `\n`
   line endings on every host, no trailing newline, and no byte-order mark.
5. **No host input.** Nothing in the writer reads the locale, the clock, an
   environment variable, a hash seed or the platform's line ending.
6. **One form, not two.** The bytes a plugin reads on its standard input are the
   bytes in the file: there is no compact variant for inter-process transport
   and no second dialect. A plugin's test fixture is therefore a file this
   toolchain wrote.

The reader side carries two different obligations, because the two readers are
in different positions.

- **This toolchain's own reader is strict.** It refuses an unknown key, an
  out-of-range numeric enum value, `null` for a repeated field, a duplicate key
  and a float-form integer (ADR-0014 decisions 11 and 14). Its input is its own
  output or a committed baseline, and the strictness is what makes the
  conformance claim measurable.
- **A consumer's reader is lenient on unknown keys.** A key it does not know is
  ignored, an absent key is the field's default, and a 64-bit value is parsed
  from its string form. This is proto3's rule for unknown fields, and it is what
  the additive changes of §6 rely on. A consumer may refuse a request whose
  schema package it does not know (§7); it must not refuse one that carries a
  key it does not know.

## 4. The Nesting Bound

The IR is a tree, and a type nests. A reader has to provision for the deepest
tree this toolchain can write, so that number is stated rather than left to be
discovered.

| Quantity                                                     | Value |
| ------------------------------------------------------------ | ----: |
| type nesting levels the front end admits                     |   127 |
| message levels below the `Package` root, deepest admitted IR |   386 |
| JSON levels of its canonical artifact                        |   516 |
| JSON levels this toolchain's own reader accepts              | 1,000 |

The front end refuses type nesting at 128 levels with FORM-102
(`MAX_TYPE_DEPTH`, `crates/ridl-syntax/src/parser.rs`), so 127 is the deepest
that compiles. The other two numbers are the per-level cost of the schema
multiplied by that limit, for the most expensive shape the language has. The
cost per source level is two message levels for an array (`FieldType`,
`ArrayType`), two for a map (`FieldType`, `MapType`) and three for a tuple
(`FieldType`, `TupleType`, `TupleField`); in JSON a level costs two brackets for
an array or a map and four for a tuple. The tuple is therefore the shape that
binds, at 386 message levels and 516 JSON levels; the array shape at the same
source depth reaches 259 and 262. A contract expression is bounded by the same
limit — the front end builds an expression tree at most 128 nodes high, and each
binary operator, member access, group and prefix raises that height by one level
— but it reaches the IR as canonical text in `Contract.source`, so it adds no
message level.

**A reader of the canonical encoding must accept at least 516 JSON levels.** A
reader built on a schema-typed protobuf JSON parser must in addition accept at
least 386 message levels. Provisioning 1,000 is recommended, because that is
this toolchain's own figure and it makes the two readers equal.

The obligation is real in more than one language. `serde_json` stops at 128 JSON
levels by default, protobuf-java at 100 message levels, Jackson at 1,000, and
JavaScript's `JSON.parse` has no limit of its own. A reader in the first two has
one configuration step to make before it can read what this toolchain legally
writes.

The three numbers move together, and only with an edit to `MAX_TYPE_DEPTH` or to
the schema's per-level cost. Either edit restates this section and moves the
test that pins it (`crates/ridlc/tests/ir_canonical.rs`).

## 5. The Derived Encodings

**Binary.** `--emit ir-binary` writes `<base>.ir.binpb` and
`<pkg.Name>.system.binpb`. Its writer is unrestricted, and its reader is not:
**a binary artifact is readable by this toolchain up to 100 message levels below
the root**, and one message level past that the decode fails with a recursion
error. The limit is prost's `RECURSION_LIMIT`, decremented once per nested
message on decode and not consulted on encode; protobuf-java and the C++ runtime
default to the same 100, so it is not one implementation's quirk.

That bound is below what the front end admits. A struct field nesting 48 arrays,
47 maps or 32 tuples — all legal source, which the checker accepts and every
backend emits — produces an artifact `to_binary` writes correctly and
`from_binary` refuses (driftsys/ridl#231). The emit does not refuse to write
such a package: the bytes are a correct encoding of it, and a consumer that
raises its own limit reads them. This asymmetry is why binary is derived and not
canonical.

`ridl diff` and `ridl check --baseline` read `.ir.json` only and refuse the
other two encodings by name (ADR-0014 decision 5). A committed baseline has to
be reviewable in a pull request.

**Prototext.** `--emit ir-text` writes `<base>.ir.txtpb`. It is for inspection.
It is not a recommended interchange form, because TypeScript — a named target —
has no usable text-format parser. This toolchain writes prototext and does not
read it on any command path.

## 6. The Compatibility Rule

Within one schema package, a change is **additive** when a reader built against
the schema before the change reads the writer's output after it and sees the
meaning it saw before, and a reader built after the change reads output from
before it and sees the field's default where the writer wrote nothing. An
additive change is made in place.

| Change                                                | Rule                                                                                                                                                                       |
| ----------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| a new field, with a number never used in that message | additive, if its default carries the meaning the old writer implied by omitting it; a new field a reader cannot ignore is breaking                                         |
| a new `oneof` member                                  | additive on the same condition; an old reader sees the `oneof` unset                                                                                                       |
| a new enum value                                      | additive; the IR's enums are closed and the checker never writes a value outside the schema, so the obligation falls on a reader's schema version rather than on its input |
| a new message, reached only through a new field       | additive                                                                                                                                                                   |
| a comment, a doc string, a `deprecated` option        | additive                                                                                                                                                                   |
| retiring a field's meaning                            | additive only as `[deprecated = true]` with the field kept, still written and still read; the field itself leaves in the next major package (§7)                           |

Everything else is **breaking**, and is never made in place:

- removing a field, an enum value or a `oneof` member;
- renaming a field. In the canonical encoding the key is the field's name
  through the JSON mapping, so a rename changes the bytes even though the wire
  number does not move. The `json_name` option is not an escape from this,
  because it would make the schema and the bytes disagree;
- renumbering a field or an enum value;
- changing a field's type, its cardinality (`repeated` or not) or its presence
  (`optional` or not);
- moving a field into or out of a `oneof`;
- changing the meaning of an existing value, of a default, or of the order of a
  repeated field;
- adding a `map<>` field (§3 item 3);
- changing any item of §3, or lowering any number of §4.

The rename entry is the one that differs from protobuf's own guidance, which
reasons about the binary wire format where a rename is invisible. The canonical
encoding is the contract here, and in it a rename is visible.

## 7. Versioning

A breaking change is a new schema package, never an edit to the old one.

A breaking change to `ridl.ir.v2` is made by adding `ridl.ir.v3` as
`crates/ridl-ir/proto/ridl/ir/v3/{ir,system}.proto`, compiled beside `v2`, with
this toolchain's writers and readers moved onto it. `v2` is not edited for it,
keeps compiling, and is removed when its last consumer has moved — which is how
`v1` was removed. The same rule governs `ridl.codegen.v1`, whose successor is
`ridl.codegen.v2`.

The artifact file names do not change with the package. `<base>.ir.json` stays
`<base>.ir.json`: the name drives snapshot detection and is cited across the
records, and the schema a reader is built against is what tells the versions
apart.

The IR artifacts carry no version field of their own. The proto3 JSON mapping
has no envelope, `ridl diff` reads a baseline with the toolchain that wrote it
or one of the same major version, and adding a field to `Package` would move
every golden for a fact the file name and the reader's schema already fix.

A codegen request is different, because the process that reads it is not this
toolchain and may be older or newer than it. It carries the version as its first
two fields, so that a reader meets them before anything it might not understand:

```proto
string schema = 1;     // "ridl.codegen.v1" — the package this request is encoded from
string toolchain = 2;  // the ridlc version that wrote it, for example "0.2.0"
```

A consumer refuses a `schema` it does not know, with a diagnostic naming both
values. It does not refuse on `toolchain`, which is informational: a consumer
that needs a fact only a later toolchain writes finds the field absent and
reports that, under the lenient-reader rule of §3.

## 8. Reading the IR from Another Language

For an implementer outside this repository, the whole obligation is:

- Parse the artifact, or the request on standard input, as JSON. Any parser will
  do that accepts at least 516 levels of nesting (1,000 recommended, §4),
  ignores keys it does not know, and reads 64-bit integers from their decimal
  string form (§3).
- Read the `schema` field first on a request, and refuse a package you do not
  know (§7).
- Expect new keys over time and ignore them; expect no key to be removed or
  renamed within a schema package (§6).
- Use a file this toolchain wrote as a fixture. The bytes on the pipe and the
  bytes in the file are the same (§3 item 6).

## 9. Conformance

Three claims of this document are measured by the test suite rather than
asserted here.

| Claim                                                                                                               | Where                                                                                    |
| ------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| every IR the front end admits round-trips through the canonical encoding, and writing it twice gives the same bytes | `crates/ridlc/tests/ir_canonical.rs`, over every corpus package and every lowered system |
| the numbers of §4                                                                                                   | the same file, over the deepest package the front end admits, in all three shapes        |
| the binary bound of §5                                                                                              | `crates/ridl-ir/src/lib.rs`, at 100 message levels and at 101                            |

The strictness of this toolchain's reader (§3) is pinned separately, one test
per narrowing, in `crates/ridl-ir/src/lib.rs`.

There is no cross-language conformance test. The strongest available evidence
would be a second implementation reading an artifact this one wrote; it does not
exist yet, and this document is written so that it can be built.
