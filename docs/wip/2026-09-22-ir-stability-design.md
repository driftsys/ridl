# IR stability — the canonical encoding and the compatibility rule

Status: pre-ADR design note, written 2026-09-22 as stage P1a of the
[lane P driver](2026-09-22-lane-p-driver.md) (§4, "P1 — E4.5a"). It proposes the
disposition of the driver's open item O-P1 and the rules the driver's P1a
bullets ask for. **The disposition of O-P1 belongs to Sebastien**; this note is
a recommendation he can overturn on the pull request, written so that stage P1b
can implement it as it stands. Nothing here is implemented, and no ADR is
amended by this note: P1b amends
[ADR-0014](../decisions/ADR-0014-ir-encodings.md) decision 9 in place with a
dated note, writes the policy into `docs/specification/`, and adds the tests
named in §6.

Every number in this note was measured on this branch on 2026-09-22 (§2). The
issue this note resolves is driftsys/ridl#231, which records the numbers as
"roughly"; the measured values replace them.

## 1. What the note decides, and for which schemas

ADR-0014 decision 9 fixes the canonical-form policy that roadmap story E4.5a is
to cite: binary is canonical, JSON is derived and conformance-obliged, prototext
is for inspection.
[ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
decision 9 makes that canonical encoding the encoding of the plugin contract —
the `CodegenRequest` a plugin reads on stdin and the `CodegenResponse` it writes
on stdout — and decision 12 says the policy lands before the contract for that
reason. Driver decision D-P2 makes the request carry the lowered model, not the
raw IR, in its own package `ridl.codegen.v1` (stage P2).

So the policy binds three schemas, and this note writes it for all three at
once: `ridl.ir.v2` (`ir.proto`, the package), `ridl.ir.v2.System`
(`system.proto`, the lowered system,
[ADR-0022](../decisions/ADR-0022-rsdl-system-in-the-ir.md)), and
`ridl.codegen.v1` (the lowered model and the contract messages, which P2 and P3
define). The word "IR" below means all three unless a sentence says otherwise.

## 2. The measurement (driftsys/ridl#231 reproduced)

### 2.1 Method

A throwaway integration test in `crates/ridlc` (not committed; the P1b test
supersedes it) generated one-field `.typl` packages of the form

```typl,ignore
package veh.deep

struct Deep {
  payload : [[…[integer; 1]…; 1]; 1]       // arrays
  payload : [string : [string : … ; 0..1]; 0..1]   // maps
  payload : (f0: (f1: (… integer …)))       // tuples
}
```

at source nesting depth 1 to 140, compiled each through `ridlc::compile`, and
for every package the front end admitted: walked the IR to count message levels
below `Package`; rendered `to_json_pretty` and counted bracket nesting;
round-tripped through `from_json`; rendered `to_binary` and round-tripped
through `from_binary`. A second probe built the `FieldType`/`ArrayType` chain by
hand under a `FixedDef`, to pin the threshold in message levels independently of
the front end. A third compiled the `veh-cluster` corpus workspace in a release
build and timed the four functions over 2,000 iterations per package.

The workspace is at `prost` 0.14.4, whose `RECURSION_LIMIT` is 100
(`src/lib.rs`), decremented once per nested message on decode
(`DecodeContext::enter_recursion`) and not consulted on encode. The parser's
`MAX_TYPE_DEPTH` is 128 (`crates/ridl-syntax/src/parser.rs`), and the JSON
reader's cap is 1,000 bracket levels (`MAX_JSON_NESTING`,
`crates/ridl-ir/src/lib.rs`).

### 2.2 The front end's ceiling

For all three shapes the front end admits source depth 127 and refuses 128 with
`FORM-102 type nesting deeper than 128 levels`. The deepest admitted package,
per shape:

| Shape  | Source depth | Message levels below `Package` | JSON bracket depth | JSON bytes | Binary bytes |
| ------ | -----------: | -----------------------------: | -----------------: | ---------: | -----------: |
| arrays |          127 |                            259 |                262 |    250,022 |        1,291 |
| maps   |          127 |                            261 |                264 |    685,506 |        2,959 |
| tuples |          127 |                            386 |                516 |    676,759 |        1,810 |

The message-level cost per source level is what ADR-0014 decision 12 reasoned:
two for an array (`FieldType`, `ArrayType`), two for a map (`FieldType`,
`MapType`), three for a tuple (`FieldType`, `TupleType`, `TupleField`). A struct
field sits four levels below the package (`Decl`, `StructDef`, `StructMember`,
`Field`). The map figure carries three extra levels on the key side, because the
`string` key lowers to an inline `TypeDef` with a `Backing`. The JSON cost per
source level is two brackets for an array or map and four for a tuple (the
`fields` array and the `TupleField` object are two more).

The "262 brackets" issue #231 cites is the array shape. **The tuple shape is the
deepest the front end admits: 386 message levels and 516 JSON levels**, nearly
twice the array figure. The JSON reader's cap of 1,000 leaves a factor of 1.9
over it, not the 3.8 ADR-0014 decision 14 states for the array shape.

### 2.3 Where binary stops round-tripping

Every package the front end admits round-trips through JSON: `from_json` returns
a package equal to the one written, for every shape at every depth up to 127.
Binary does not:

| Shape  | Last source depth that round-trips | Message levels | First source depth that fails | Message levels |
| ------ | ---------------------------------: | -------------: | ----------------------------: | -------------: |
| arrays |                                 47 |             99 |                            48 |            101 |
| maps   |                                 46 |             99 |                            47 |            101 |
| tuples |                                 31 |             98 |                            32 |            101 |

The hand-built chain confirms the threshold in the schema's own unit: **100
message levels below the root decode, 101 do not.** `to_binary` writes every one
of these packages without error; it is `from_binary` that refuses. The exact
error, for the array shape at source depth 48 (the middle of the stack elided;
it repeats `ArrayType.element: FieldType.kind:` 48 times):

    failed to decode Protobuf message: ArrayType.element: FieldType.kind:
    ArrayType.element: FieldType.kind: … Field.type: StructMember.member:
    StructDef.members: Decl.kind: Package.decls: recursion limit reached

So the window issue #231 calls "roughly 50 to 128" is, measured: **source depth
48 to 127 for arrays, 47 to 127 for maps, and 32 to 127 for tuples.** In that
window the source is legal, the checker accepts it, every language backend emits
it, `--emit ir-json` writes it and `ridl diff` reads it back, and
`--emit ir-binary` writes an artifact the toolchain itself cannot read.

### 2.4 Size and speed, on the plugin IPC path

The `veh-cluster` corpus, release build, per package:

| Package       | JSON bytes | JSON encode | JSON decode | Binary bytes | Binary encode | Binary decode | Size ratio |
| ------------- | ---------: | ----------: | ----------: | -----------: | ------------: | ------------: | ---------: |
| `veh.common`  |     15,924 |       21 µs |       93 µs |        1,506 |          4 µs |         10 µs |       10.6 |
| `veh.cluster` |     55,164 |       64 µs |      239 µs |        8,529 |         12 µs |         34 µs |        6.5 |

Binary is 6 to 11 times smaller and 5 to 9 times faster on these packages. The
absolute figures are what matter for the decision: the largest corpus package
costs 64 µs to write and 239 µs to read as JSON, and crosses a pipe in 55 kB.
The process host of ADR-0020 decision 10 spawns an executable per build — a Rust
binary starts in about a millisecond, a JVM launcher (the Kotlin plugin, driver
D-P5) in hundreds — so the encoding's share of the IPC path is below one percent
for a native plugin and below one per mille for a JVM one. The lowered model P2
defines is larger than the IR it is lowered from, plausibly by a small integer
factor; that does not change the order of magnitude. Pretty-printing dominates
the JSON size at pathological depth (the array shape at depth 127 is 250 kB of
mostly indentation against 1.3 kB of binary), and is irrelevant at the
single-digit nesting real packages have.

### 2.5 A fact the records state wrongly

ADR-0014 decision 14 and issue #231 say prost's limit is "non-configurable" and
that prost "does not expose it for configuration". `prost` 0.14.4 exposes a
cargo feature, `no-recursion-limit`, that removes the limit from the crate
(`Cargo.toml` line 45; `DecodeContext` is compiled without its counter). It is
not configurable per call or per message, which is what the records meant, but
it is configurable, and the correction matters because it makes a fourth option
(§3.4) real. P1b's amendment of ADR-0014 should carry the correction.

## 3. O-P1 — the canonical encoding

Four options were weighed. The driver names the first two; the third is the one
the P1a brief asked this note to consider; the fourth follows from §2.5.

### 3.1 Option A — canonical protobuf JSON becomes canonical, binary derived

The canonical form is the JSON `ridlc` already writes to `<base>.ir.json`, and
the request on a plugin's stdin is those same bytes for the lowered model.

For it:

- It round-trips every IR the front end admits (§2.3), which is E4.5a's
  `Done when` verbatim, with no change to the language and no change to the
  writer or the reader.
- Every consumer named by any record — Rust, Kotlin, TypeScript, and the Deno,
  Python and JVM plugins ADR-0020 decision 10 lists — has a JSON parser in its
  standard library, so a plugin needs no protobuf toolchain to read the request.
  A typed reader exists as well where one is wanted: protobuf-java's
  `JsonFormat` (which protobuf-kotlin uses), protobuf-es, and protobuf-python's
  `json_format` all implement the same mapping.
- It is the form the repository already treats as the artifact: `ridl diff` and
  `ridl check --baseline` read it and refuse the others (ADR-0014 decision 5),
  every IR golden holds it (decision 3), and a committed baseline must be
  reviewable, which binary is not. Making the reviewed form the canonical one
  removes the case where the canonical bytes and the reviewed bytes differ.
- A plugin can be tested by feeding it a file from disk, because the file and
  the pipe carry identical bytes; a failing plugin run can be diagnosed by
  reading its input.
- No consumer reads binary today (driver §2), so nothing moves.

Against it: the size and speed cost of §2.4, which is real and, on the one path
that carries it, immaterial; and the JSON mapping's own conventions (64-bit
integers as strings, enum values as names, `lowerCamelCase` keys) that a plugin
author must know. Those conventions are the protobuf JSON mapping's, fixed by
the schema and the mapping, not by this repository, and §5 states them.

### 3.2 Option B — binary stays canonical, with the nesting limit stated

Keep ADR-0014 decision 9 and add to it: the canonical encoding round-trips up to
100 message levels below the root, and a package deeper than that is
representable in binary but not readable by this toolchain.

Rejected. A canonical encoding the reference toolchain cannot read back for
legal input fails E4.5a's `Done when` as the roadmap states it, and the roadmap
would have to be weakened to admit it. The limit is also not this toolchain's
alone: protobuf-java and the C++ runtime default to the same 100, so the Kotlin
plugin would inherit the defect on its side and a plugin author would have to
raise `setRecursionLimit` to read what `ridlc` legally writes. Binary's
advantages, size and speed, do not bear on any path that exists (§2.4).

### 3.3 Option C — lower `MAX_TYPE_DEPTH` so every admitted IR fits binary

Lower the parser's limit from 128 to the depth at which the deepest shape stays
within 100 message levels: 31 for tuples (§2.3), the tightest shape, since the
limit is one budget shared by every shape. Then every IR the front end admits
round-trips through binary, and E4.5a's `Done when` holds with binary canonical.

Rejected, on three grounds:

- ADR-0014 decision 12 rejected a checker-level nesting limit in terms that
  still hold: it is a language change made to work around a library limit,
  restricting input every other consumer of the IR handles. Since that record
  the count of consumers that handle it went up (JSON now round-trips it, and
  four language backends emit it), so the reason grew stronger, not weaker.
- It changes what FORM-102 is. Today it is a stack guard "far beyond any real
  schema" (the constant's comment); at 31 it becomes a semantic limit a schema
  author can meet — a 32-level tuple is unusual but not absurd — and the
  language references would have to state it as a rule of the language rather
  than of the parser.
- It fixes one reader's default. Readers of any encoding carry defaults of their
  own — protobuf-java 100 message levels, `serde_json` 128 bracket levels,
  Jackson 1,000, JavaScript's `JSON.parse` none — and no single parser depth
  satisfies all of them except one so low that it binds. The policy has to state
  a bound that readers provision for (D-3) whatever the encoding; once it does,
  lowering the language limit buys nothing.

Kept in reserve: `MAX_TYPE_DEPTH` is one constant, and if a stated bound ever
proves a burden on a plugin author, lowering it is a one-line change with a
policy edit. That is the reason to state the bound in the front end's units
(D-3), so that the two stay visibly coupled.

### 3.4 Option D — enable prost's `no-recursion-limit` feature

Enable the feature on the workspace's `prost` dependency. `from_binary` then
reads every package `to_binary` writes; E4.5a's `Done when` holds with binary
canonical and no language change.

Rejected as the disposition of O-P1, kept as the mechanism behind D-2 if a
binary consumer ever needs the full range:

- It fixes the Rust reader only. A plugin reading binary in another language
  keeps its own default (§3.3), so the policy would still have to state the
  bound and the plugin author still has to raise a limit — for JSON that
  obligation exists too (D-3), but a plugin with a plain JSON parser in a
  language without a bracket limit has nothing to raise.
- The feature is workspace-global: cargo unifies it onto the one `prost` build
  every crate here links, including the `prost-reflect` behind prototext and the
  `pbjson` runtime. Nothing in this workspace decodes untrusted protobuf today
  (`ridl-rt` does not depend on `prost`; its proto3 payload encoding is not
  built yet), so the guard it removes protects nothing at present, but a later
  runtime crate that decodes wire payloads with `prost` in this workspace would
  inherit the removal without choosing it. That is a decision to take when such
  a crate exists, not one to take here for a reader nobody uses.
- The JSON reader replaced its library limit with a measured cap and a sized
  stack (ADR-0014 decision 14). Binary has no equivalent guard to put in place
  of the limit — the wire format cannot be depth-measured without the schema —
  so removing the limit leaves an unbounded recursion on the caller's stack.

### 3.5 The recommendation

**Option A.** Canonical protobuf JSON becomes the canonical encoding of every
toolchain schema (§1); binary and prototext are derived. Binary stays emittable,
with its round-trip bound stated (D-2). The bound of the canonical form is
stated in the policy (D-3). What "canonical" fixes is listed so that
"byte-identical" is a checkable claim (D-4). The compatibility rule and the
versioning rule follow (D-5, D-6).

## 4. Decisions

### D-1. Canonical protobuf JSON is the canonical encoding; binary and prototext are derived

The canonical encoding of `ridl.ir.v2` (`Package` and `System`) and of
`ridl.codegen.v1` is canonical protobuf JSON as `ridl-ir` writes it today
(ADR-0014 decisions 1, 2, 8 and 14). It is the encoding of the `<base>.ir.json`
and `<pkg.Name>.system.json` artifacts, of every committed baseline and golden,
and of the `CodegenRequest` and `CodegenResponse` on the process host's stdin
and stdout (ADR-0020 decision 9, whose text "ADR-0014's canonical encoding" is
unchanged and now names JSON). ADR-0014 decision 9 is amended in place by P1b to
say so, with the measurement of §2 as its reason.

**Reason.** §3.1: it round-trips every IR the front end admits, every named
consumer reads it without a protobuf toolchain, and it is already the reviewed
and diffed form. The cost (§2.4) falls on no path where it is measurable.

**Rejected.** Option B (§3.2): a canonical form the reference toolchain cannot
read for legal input. Option C (§3.3): a language change for a library default.
Option D (§3.4): a Rust-only fix that removes a guard workspace-wide.

**What does not change.** ADR-0004 §4's "protobuf compiled with `prost` remains
the canonical IR" is about the schema, and stands: the `.proto` files stay the
schema of record, and the JSON is defined by the protobuf JSON mapping applied
to them. Only which of the three encodings is named canonical moves.

### D-2. Binary stays emittable and derived, and its round-trip bound is stated

`--emit ir-binary` keeps writing `<base>.ir.binpb` and
`<pkg.Name>.system.binpb`; `to_binary`/`from_binary` keep their signatures;
`ridl diff` and `ridl check --baseline` keep refusing the encoding by name
(ADR-0014 decision 5). The policy states binary's bound: **a binary artifact is
readable by this toolchain up to 100 message levels below the root** (§2.3), and
a package past that — source depth 48 or more for arrays, 47 for maps, 32 for
tuples, in a struct field — is written correctly and is not readable by
`from_binary`. The `ridlc` emit does not refuse to write such a package: the
artifact is a correct encoding, and a consumer with a raised limit reads it.

**Reason.** Binary is the smallest and fastest form (§2.4) and a consumer that
wants it should have it. Its bound is a fact about readers, stated once so that
no consumer discovers it the way issue #231 did.

**Rejected.** Enabling `no-recursion-limit` now (§3.4). Retiring the binary
emit: it costs nothing to keep, ADR-0014 decision 4 named it, and removing it
would remove the one compact form of the IR for a consumer that wants one.

### D-3. The canonical form's nesting bound is stated, in the front end's units and the encoding's

The policy states, as measured numbers with the method that produced them:

- the front end admits 127 levels of type nesting (`MAX_TYPE_DEPTH` = 128,
  FORM-102 at 128);
- the deepest IR that admits nests **386 message levels below the root** and
  **516 JSON levels** (the tuple shape; §2.2);
- the toolchain's own JSON reader accepts 1,000 JSON levels (ADR-0014 decision
  14), so it reads every canonical artifact with a factor of 1.9 in hand;
- **a reader of the canonical encoding must accept at least 516 JSON levels**,
  and one built on a schema-typed protobuf JSON parser must accept at least 386
  message levels; the policy recommends provisioning 1,000, the toolchain's own
  figure, so that the two are equal.

The three numbers move together and only by an edit to `MAX_TYPE_DEPTH` or to
the per-level message cost of the schema; either edit restates the bound in the
policy and re-runs the test of §6. Stage P2 restates the bound for
`ridl.codegen.v1` from its own schema: if the lowered model nests a type at the
same three messages per tuple level, the numbers are the IR's; if it flattens
type nesting into a table, they are smaller, and the note says which.

**Reason.** §3.3's third ground: every reader has a default, they differ, and
without a stated number each plugin author measures it alone. The default that
binds first for a Rust plugin is `serde_json`'s 128 levels, which refuses the
canonical form from source depth 61 for arrays and 31 for tuples, so even the
canonical encoding needs one configuration step in that language, and the policy
is where a plugin author learns it.

**Rejected.** Stating no bound and relying on "real packages nest single
digits": true of the corpus and unverifiable for a plugin author's input.
Stating the toolchain's cap (1,000) as the bound of the encoding: it is a
reader's guard, not a property of what the writer produces, and a bound the
front end cannot reach is not a bound a test can pin.

### D-4. What "canonical" fixes

A canonical artifact is byte-identical for equal IR across runs, hosts and
platforms. The policy lists what that rests on, so that a change to any item is
a change to the canonical form (breaking under D-5):

1. **The mapping.** The protobuf JSON mapping over the schema of record: field
   keys in `lowerCamelCase` as the mapping derives them from the `.proto` names
   (`json_name` is never set), enum values as their names, a `oneof` flattened
   to its member's key, 64-bit integers (`int64`, `uint64`) as decimal strings
   (ADR-0014 decision 8), `bytes` as base64, `string` as UTF-8 with only the
   escapes RFC 8259 requires. The schema declares no `float` or `double` field
   (every numeric value is an exact decimal string, ADR-0007 decision 9), so no
   float formatting rule is needed, and none is stated.
2. **Presence.** A non-`optional` field holding its default is emitted; an unset
   proto3 `optional` field, an unset message field and an unset `oneof` produce
   no key; `null` never appears (ADR-0014 decision 2 and its open item 1,
   `pbjson-build`'s `emit_fields()`).
3. **Order.** Fields in the order the `.proto` declares them within each
   message; a repeated field's elements in the order the writer holds them,
   which for every list in the IR is source order or a stated order (the
   schema's comments name it per field). The schema declares no `map<>` field;
   adding one is a breaking change under D-5, because its key order would need a
   rule this list does not have.
4. **Layout.** Pretty-printed: two-space indentation, one field per line, `\n`
   line endings on every host, no trailing newline, no byte-order mark. This is
   `serde_json::Serializer::pretty` as `render_json` calls it, and the goldens
   hold it.
5. **No host input.** Nothing in the writer reads the locale, the clock, an
   environment variable, a hash seed or the platform's line ending; the
   generated `Serialize` impl writes fields in schema order from a `Vec`, and
   `std::fs::write` writes the string's bytes unchanged.
6. **One form, not two.** The bytes on the process host's pipe are the bytes in
   the file: no compact variant for IPC, no second dialect. The cost is §2.4's,
   and the gain is that a plugin's test fixture is a file `ridlc` wrote.

The reader side is stated as two obligations, because they differ:

- **The toolchain's own reader is strict** (ADR-0014 decisions 11 and 14, and
  the four narrowings decision 14 records): unknown keys, out-of-range enum
  numbers, `null` for a repeated field, duplicate keys and float-form integers
  are refused. Its input is its own output or a committed baseline, and
  strictness is what the conformance test measures.
- **A plugin's reader is lenient on unknown keys.** A key it does not know is
  ignored, an absent key is the field's default, and a 64-bit value is parsed
  from its string form. This is the proto3 rule for unknown fields and what
  D-5's additive changes rely on. A plugin may refuse a request whose `schema`
  (D-6) it does not know; it must not refuse one that carries a key it does not
  know.

**Reason.** "Byte-identical" is a claim tests can pin only if the list is
explicit; the six items are what the writer does today, verified against the
generated impl, the schema and the write path, and stating them turns an
implementation fact into a contract.

**Rejected.** Sorting keys alphabetically, the rule of generic JSON
canonicalizers (RFC 8785): it would move `name` off the first line of every
message, change every golden, and buy determinism the writer already has. A
compact form on the pipe: a second dialect for a saving that §2.4 shows is not
measurable on that path.

### D-5. The compatibility rule

Within one schema package (`ridl.ir.v2`, `ridl.codegen.v1`), a change is
**additive** when a reader built against the schema before the change reads the
writer's output after it and sees the meaning it saw before, and a reader built
after the change reads output from before it and sees the field's default where
the writer wrote nothing. Additive changes are made in place:

| Change                                                | Rule                                                                                                                                                                                                                        |
| ----------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| a new field, with a number never used in that message | additive, if its default carries the meaning the old writer implied by omitting it; a new field a reader cannot ignore is a breaking change                                                                                 |
| a new `oneof` member                                  | additive on the same condition; an old reader sees the `oneof` unset                                                                                                                                                        |
| a new enum value                                      | additive; the schema comments say what an old reader does with a value it does not know — the IR's enums are closed and the checker never emits an unknown one, so the obligation is on the reader's version, not its input |
| a new message, reached only through a new field       | additive                                                                                                                                                                                                                    |
| a comment, a doc string, a `deprecated` option        | additive                                                                                                                                                                                                                    |
| retiring a field's meaning                            | additive only as a `[deprecated = true]` option with the field kept, written and read; the field leaves in the next major package (D-6)                                                                                     |

Everything else is **breaking**, and is never made in place:

- removing a field, an enum value or a `oneof` member;
- renaming a field — in the canonical form the key is the field's name through
  the JSON mapping, so a rename changes the bytes even though the wire number
  does not move; the `json_name` option is not an escape from this, because it
  would make the schema and the bytes disagree;
- renumbering a field or an enum value;
- changing a field's type, its cardinality (`repeated` or not) or its presence
  (`optional` or not);
- moving a field into or out of a `oneof`;
- changing the meaning of an existing value, of a default, or of the order of a
  repeated field;
- adding a `map<>` field (D-4 item 3);
- changing any item of D-4, or lowering any number of D-3.

The rule for source order in a repeated field is part of the schema comments
already, and this table makes it part of the contract.

**Reason.** A plugin is built against a schema version and updated on its own
schedule; the rule says which changes the toolchain may make without waiting for
it. The JSON-specific entry (renaming) is the one that differs from the binary
rule and is the reason the table is written here rather than by reference to
protobuf's own guidance.

**Rejected.** Treating a rename as additive because the wire number is stable:
true of binary, false of the canonical form, and the canonical form is the
contract. Allowing removal in place with `reserved`: a plugin built against the
older schema then reads a default where it expects a value, silently; a removal
waits for the major package where every reader knows it is gone.

### D-6. Versioning: a breaking change is a new package, and the request carries the schema name

A breaking change to `ridl.ir.v2` is made by adding `ridl.ir.v3` as
`crates/ridl-ir/proto/ridl/ir/v3/{ir,system}.proto`, compiled beside `v2` into
`ridl_ir::v3`, and moving the toolchain's writers and readers onto it. `v2` is
never edited for it, keeps compiling and is removed when its last consumer moves
— the precedent is the removal of `v1` when E2 moved its last reader (`build.rs`
header comment). The same rule applies to `ridl.codegen.v1`, whose successor is
`ridl.codegen.v2`. The artifact file names do not change with the package:
`<base>.ir.json` stays `<base>.ir.json`, because the name is load-bearing for
snapshot detection and cited everywhere (ADR-0014 decision 4), and the schema a
reader is built against is what tells the versions apart.

The `CodegenRequest` (stage P3) carries the version as its first two fields, so
that a plugin reading the JSON meets them before anything it might not
understand:

    string schema = 1;     // "ridl.codegen.v1" — the package this request is encoded from
    string toolchain = 2;  // the ridlc version that wrote it, e.g. "0.2.0"

A plugin refuses a `schema` it does not know, with a diagnostic that names both
values; it does not refuse on `toolchain`, which is informational — a plugin
that needs a fact only a later toolchain writes finds the field absent and
reports that, under D-4's lenient-reader rule. The IR artifacts themselves carry
no envelope field: the proto3 JSON mapping has none, `ridl diff` reads a
baseline with the toolchain that wrote it or one of the same major, and adding a
field to `Package` would move every golden for a fact the file name and the
reader's schema already fix.

**Reason.** The driver's bullet asks for the versioning rule and the version a
request carries; protobuf's package-per-major convention is the one every
consumer's toolchain understands, and a name the plugin reads first is the
cheapest check a process on the other side of a pipe can make.

**Rejected.** A numeric `version = 1` field: a bare number does not say which
schema it counts, and the package name does. A version field in `Package`: a
golden move for no reader that needs it. Editing `v2` in place under a "pre-1.0,
no compatibility promised" rule: the workspace is at 0.2.0 and publishes to
crates.io on every tag (ADR-0007 decision 14 as amended), the Kotlin plugin is
built outside this repository against the published schema, and the point of
E4.5a is that the promise exists before that plugin does.

## 5. What the policy says to a plugin author

The paragraph P1b writes into `docs/specification/`, in substance: the request
on stdin is canonical protobuf JSON of `ridl.codegen.v1` as D-4 fixes it; read
`schema` first; parse with any JSON parser that accepts at least 516 levels of
nesting (1,000 recommended), ignores keys it does not know, and reads 64-bit
integers from strings; the same bytes are what `ridl build --emit codegen-model`
writes (stage P2), so a fixture is a file. The response on stdout is the same
encoding. Because the mapping renders `bytes` as base64, P3 should carry a
generated file's content as a `string` where it is text — every file the four
backends emit is UTF-8 text — and reserve a `bytes` member for a file that is
not; that is a P3 decision this note flags rather than takes.

## 6. What P1b implements from this note

- ADR-0014 decision 9 amended in place with a dated note: JSON canonical, binary
  and prototext derived, citing §2; decision 14's "does not expose it for
  configuration" corrected per §2.5 in the same amendment.
- The policy in `docs/specification/`. **The tree has no section that specifies
  the IR encodings today**: the family overview's document inventory (§2) lists
  "IR specification — not started" with "serialization, plugin protocol, diff
  categories, canonical encoding" as its scope, and no other file under
  `docs/specification/` names `ir-json` or the encodings. The driver's "find the
  section; do not create a second one" has no first section to find; P1b creates
  the IR specification the inventory row already names, or a section of it, and
  marks the row started.
- In `crates/ridl-ir`, one test that round-trips every corpus package through
  the canonical encoding and compares equal, and one that generates the three
  shapes of §2.2 at source depth 127 through the front end, round-trips each
  through the canonical encoding, and asserts the D-3 numbers (386 message
  levels, 516 JSON levels) so that a schema change that moves the per-level cost
  moves the policy. The binary bound of D-2 is pinned the way the existing
  `NESTING_PAST_LIMIT` test pins prototext: an error, never a panic, at 101
  message levels.
- Issue #231 closed by the pull request: the canonical encoding round-trips; the
  binary limit is stated, not fixed.
- No golden moves: D-4 fixes the form the goldens already hold.

## 7. Records touched by the recommendation, for the reviewer

| Record                               | Effect                                                                                                                                                    |
| ------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ADR-0014 decision 9                  | amended (P1b): the canonical encoding is JSON                                                                                                             |
| ADR-0014 decision 14                 | corrected (P1b): prost's limit is a cargo feature, not unconfigurable; the "factor of 3.8" is the array shape, the tuple shape leaves 1.9                 |
| ADR-0014 decisions 1, 2, 3, 5, 8, 11 | unchanged; D-4 restates them as the canonical form's content                                                                                              |
| ADR-0004 §4                          | unchanged; the schema stays canonical, the encoding label moves                                                                                           |
| ADR-0020 decisions 9 and 12          | unchanged text; "ADR-0014's canonical encoding" now names JSON                                                                                            |
| roadmap E4.5a                        | `Done when` holds as written                                                                                                                              |
| driver D-P2, stages P2 and P3        | the request's encoding is JSON; P2 restates D-3's bound for its schema; P3 carries `schema` and `toolchain` first and decides text-versus-bytes for files |
