# The FlatBuffers payload codec

The generated `Payload<FlatBuffers>` implementation a package carries, story
E11.7, as built. The binding choices are
[ADR-0019](../decisions/ADR-0019-flatbuffers-projection-rules.md) (the
projection this codec must agree with byte for byte),
[ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
decisions 2, 5, 6 and 7 (the encoding matrix, no third-party implementation in
the shipped path, one cargo feature per encoding, the codec as generated Rust)
and [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decisions 7
and 8 (`Encoded.bytes`, the `flatbuffers` feature). The reasoning behind each
decision, and the stage-by-stage record of what each round of work found, is the
archived design note
[`../archive/2026-09-20-flatbuffers-codec-design.md`](../archive/2026-09-20-flatbuffers-codec-design.md)
and its plan
[`../archive/2026-09-20-flatbuffers-codec-plan.md`](../archive/2026-09-20-flatbuffers-codec-plan.md).

**One decision of that note is not built.** D-11, the generated interaction face
moving off its `ReprC` placeholder and onto the codec, is blocked on a
projection decision nobody has taken: **driftsys/ridl#470**. The section
["What is not built"](#what-is-not-built) below is the live statement of it, and
is what whoever takes that issue should read first.

## Where the code is

| What                                               | Where                                                       |
| -------------------------------------------------- | ----------------------------------------------------------- |
| The projection facts both emitters read            | `crates/ridl-ir/src/projection/flatbuffers.rs`              |
| The `.fbs` schema emitter                          | `crates/ridl-backend-flatbuffers/src/lib.rs`                |
| The codec emitter                                  | `crates/ridl-backend-rust/src/codec.rs`                     |
| The per-type refusal (`check_flatbuffers_bound`)   | `crates/ridl-backend-rust/src/lib.rs`                       |
| The reader and builder the emitted code calls      | `crates/ridl-rt/src/flatbuffers.rs`                         |
| The round trip, run rather than compiled           | `crates/ridl-backend-rust/tests/flatbuffers_roundtrip.rs`   |
| Conformance against planus, and the `wasm32` check | `crates/ridl-backend-rust/tests/flatbuffers_conformance.rs` |

## The shared facts live outside both backends

A type's table layout, its field slots, a union arm's discriminant and its size
bound are computed once, in `ridl-ir`, and read by the schema emitter and the
codec emitter alike. They are not a public module of either backend: ADR-0020
decision 9 makes a backend an executable rather than a library other crates
link, so a dependency of one backend on another would be the wrong shape for
step 2, and `ridl-descriptor` needs the same bound for the catalog (Epic 16).

**What is shared and what is not.** The projection owns the slot ids, the union
discriminant and the size bound, because two emitters must agree on them. Which
byte of a table a field starts at, and how large the table is, are observable
only by the codec — a `.fbs` schema states no offsets, and no other emitter
reads one — so the inline layout is computed in `codec.rs`, in declaration order
with each field aligned to its own width. The bound stays sound whatever order
is chosen, because it charges `ALIGN_SLACK` once per vtable slot and once more
for the table's own `soffset`, which is the worst case any order can reach.

The drift test that fails when the schema emitter and the shared facts diverge
is in `crates/ridl-backend-flatbuffers/src/tests.rs`, over the corpus fixtures.

## What is emitted, and where

The `Payload<FlatBuffers>` implementations are in `generate`'s own output, not
behind a third entry point beside `generate_face`. A consumer of a generated
package needs the codec whether or not it ever dispatches, and `generate`
already names `ridl-rt`. `ridlc::run_build` calls `generate`, so
`ridl build --emit rust` carries the codec, and `crates/ridlc/src/lib.rs`
renders `ridl-rt = { version = "0.1", features = ["flatbuffers"] }` in the
manifest it writes.

Per table: three free functions — `__ridl_fb_{encode,verify,decode}_<snake>` —
and one view struct `<T>FbView<'a>`, at the generated package's module scope
rather than in a submodule. At module scope a same-package reference is spelled
as `type_path` spells it everywhere else, and the functions can read a generated
type's private inner value the way any other item of that module can, which is
what lets `decode` build an enum set that publishes no constructor. The
`__ridl_fb_` prefix collides with no typl name: typl §15.1 gives a declaration a
CamelCase name and a constant a SCREAMING_SNAKE one.

A `Payload<FlatBuffers>` implementation is written for a declaration
`ridl_ir::projection::flatbuffers::mints_root_table` accepts, which is a
`struct` or a `union` and nothing else.

## `encode`, `verify`, `decode`, `MAX_SIZE`

**`encode` builds at the tail.** A FlatBuffers builder writes back to front, so
what it produces is a suffix of the caller's buffer. `Encoded.bytes` is
therefore a **subslice** of the output buffer, not a prefix of it — ADR-0021
decision 7 carries that wording since its 2026-09-20 amendment. Every consumer
must pass on the slice it was handed rather than re-slicing `&buf[..len]`; the
generated face did the latter until stage K7 fixed it at all four send sites.
`EncodeError::Capacity` is what a buffer smaller than the value produces, with
no panic and no truncated write.

`encode` allocates only where the domain type already does. A vector of strings
or of tables needs each element's position before the vector can be written, and
those are exactly the shapes whose domain type is a `Vec` or a `String`, so a
generated package over types that own neither still encodes with no allocator.

**`verify` is one pass, and it is total over the typl constraints it can
reach.** It walks the structure, and beside it checks: an enum and an enum-set
discriminant, through the `TryFrom` the value-object emission already provides;
a collection's declared element count; and a named scalar's own range, length
and pattern, through a `fn check(value: &T)` emitted beside `new` in every
constrained named scalar's `impl` block. `check` carries no visibility modifier:
every caller is a function generated into the same module, or a child `pub mod`
of it, and neither needs it more visible. The named-scalar check hangs off the
one `Codec::wire` site every position reaches, so it covers every position the
projection admits a named scalar at — an inline field, a string's or a byte
sequence's bytes, an array element, a map key, a map value, a tuple field, a
nested table, a union's table arm and its boxed arm, and any of those behind an
optional.

What `verify` does **not** check is named in its own doc comment and tracked as
**driftsys/ridl#469**: an anonymous inline constraint, which carries no named
scalar and so has no `check` to call; `step`, which the IR's own vacuity test
excludes; and a map key's uniqueness.

**`decode` is total and never panics.** It takes a `Ref` — a proof `verify`
produced — and cannot fail. Every read that could fail is discharged with the
neutral value of its own type, and `verify` is what makes those branches
unreachable. `unreachable!` was rejected: a panic in a consumer's build is worse
than a value no run can reach. A generated enum with no value and a union with
no arm have no neutral value at all, so the emitter refuses both with a
`GenerateError`.

**`MAX_SIZE` is a literal with a doc comment naming the rule.** It is the number
`ridl_ir::projection::flatbuffers::max_size` returned — each table charged its
`soffset`, its inline fields, its vtable and one alignment event per slot; a
string four bytes per declared character plus a terminator; a collection its
declared maximum. It is a literal rather than a const-evaluable expression
because the slack the projection charges is not expressible in Rust's type
system. It is **pinned, not proven tight**: the round-trip suite fails on a
bound one lower and on one higher, so the number cannot drift unnoticed, but
nothing demonstrates a legal value reaches it. The measured gap is real — the
largest `Report` a probe could build is 1688 bytes against a bound of 2388 — and
it is the alignment and vtable slack that keeps the bound sound under any write
order.

**A type with no finite bound carries no codec**, per type rather than per
package: `check_flatbuffers_bound` is called as each type's implementation is
about to be emitted. Its refusal names the member, not only the declaration, by
handing a synthetic one-member declaration to the same `max_size`. Three
attributions are distinguished — a declaration-wide layout error, a member with
no type at all, and an aggregate that overflows with every member bounded on its
own. A leaf this backend cannot judge is replaced by a one-byte stand-in and the
whole position probed once, so a count or a product of counts is still charged
even where a foreign reference sits inside the same position; the stand-in can
prove a position unbounded and never prove one bounded.

## Presence, defaults, and what an absent field means

- An optional field is written when present and omitted when absent. A present
  value is written **even when it equals the field's FlatBuffers default**, so
  that a reader can tell a present default from an absent optional, which typl
  distinguishes.
- A non-optional field is always written, and its absence in a buffer is
  `Malformed::MissingRequired`. FlatBuffers cannot mark a scalar or an enum
  field required, which ADR-0019 records, so the requirement is the verifier's
  and not the schema's.
- A field whose enum declares no zero member carries `= null` in the schema
  (ADR-0019 decision 6), and an absent such field is `MissingRequired` when the
  typl field is not optional.

An optional marker outside a table field is a `GenerateError`: a FlatBuffers
vector has no absent element and a map entry no absent half. No typl source
reaches it.

## Determinism

The same value encodes to the same bytes, on every run and every target. Field
write order, vtable layout and vtable sharing are fixed by the emitter, and a
map's entries are written in the order of the generated `Vec<(K, V)>`, so
nothing sorts and nothing depends on a hash order. Every read and write in the
codec and in `crates/ridl-rt/src/flatbuffers.rs` is little-endian by
construction.

Two cases assert it rather than describing it. `the_encoding_is_deterministic`
encodes twice in one process, the second time into a buffer filled with `0xAB`
rather than zeros, so a byte the encoder leaves untouched cannot make the two
agree by accident. `the_encoding_of_one_value_is_pinned_across_runs` pins the
bytes of one value as a literal, which is the half the first cannot reach: it
holds under anything stable within a process and unstable across processes, and
Rust randomizes a `HashMap`'s iteration order per process.

## Conformance

**Conformance is a round trip through a second implementation, not byte equality
with one.** FlatBuffers fixes no canonical encoding: vtable sharing, field
ordering and alignment slack are all writer choices, and two conforming writers
differ. This is a weaker claim than E11.8 makes for proto3, where byte-level
conformance against `protoc` is the story's own `Done when`; the difference is
in the formats, not in the rigor of the two stories.

The second implementation is `planus`, already this repository's FlatBuffers
oracle. `planus-codegen` turns the `.fbs` the schema backend emits for the
round-trip fixture into a Rust reader and writer, and that reader is **checked
in** under `crates/ridl-backend-rust/tests/generated/`, with a test that
regenerates it and asserts byte equality. There is no `build.rs` and no `flatc`:
`planus`, `planus-codegen` and `planus-translation` are dev-dependencies of
`ridl-backend-rust` only, and `xtask/tests/oracle_boundary.rs` fails if any of
them, or `ridl-backend-flatbuffers`, is promoted to a normal dependency. The
`.fbs` is emitted from the same IR as the codec, so the schema and the codec
cannot drift apart.

Four cases, in `crates/ridl-backend-rust/tests/flatbuffers_conformance.rs`:

1. bytes this codec writes are read by planus and compare equal field by field;
2. bytes planus writes are accepted by `verify` and decode to the same value;
3. a planus buffer whose vtable is **truncated** — a shape this codec's own
   writer never produces, since it writes a slot for every field — decodes with
   the missing slots read as absent;
4. a planus buffer that omits a default-valued non-optional field is refused,
   which is the divergence below.

Two mutations were applied and run, and each is what says the suite is not
decorative. Shifting the union discriminant by one in `codec.rs` leaves every
case of the self-consistent round-trip suite passing and fails both conformance
directions — a codec that disagrees with its own schema is exactly what a round
trip through itself cannot see. Turning a short vtable into an error in
`ridl-rt` leaves the round-trip suite and the three other conformance cases
passing, and fails only case 3.

### The one disagreement: an omitted default (driftsys/ridl#472)

A conforming FlatBuffers writer omits a table field whose value equals its
declared default. This codec reads an omitted non-optional field as
`MissingRequired`, so **a buffer another conforming writer produced, for a value
whose non-optional scalar happens to equal its default, is a buffer this codec
refuses.** It is a decided divergence rather than a defect — the decision is
above, under presence and defaults, and it was taken so that a present default
stays distinguishable from an absent optional — but it means interoperation with
a foreign writer is not unconditional. The reader half of that decision is what
driftsys/ridl#472 reopens, and the same question binds E11.8, where a
default-valued field's absence is the format's own normal state.

## `wasm32`

ADR-0020 decision 2 makes the generated Rust compiled to `wasm32` the codec a
TypeScript consumer loads, so the `wasm32` obligation reaches what the backend
emits and not only the crates that emit it. `just wasm-check` cannot reach it:
the recipe runs `cargo check` over a `-p` list of packages, and a generated
package is text with no manifest. `the_generated_codec_checks_for_wasm32` runs
the same check — `rustc --target wasm32-unknown-unknown --emit=metadata`, the
unit of work `cargo check` performs — over the emitted source, through the
bare-`rustc` proof mechanism every other compile proof in this crate uses. It
runs under `just test`, so no recipe was added and `just gate-parity` is
unaffected. The alternative, an example crate under `crates/` holding a
checked-in generated fixture so it could join the `-p` list, was rejected: a
workspace member and a second copy of the fixture to keep in step, for a check
the test already performs over the emitter's live output.

## What is not built

### D-11 — the generated face still names `ReprC` (driftsys/ridl#470)

The generated interaction face names `::ridl_rt::encoding::ReprC` in
`crates/ridl-backend-rust/src/face.rs`, and
`crates/ridl-backend-rust/src/descriptors.rs` computes its buffer constants over
`<T as Payload<ReprC>>::MAX_SIZE`. The hand-written `Payload<ReprC>`
implementations in `crates/ridl-backend-rust/tests/interaction_face.rs` are
still there, and they are a throwaway rather than a reference implementation.

**Why it did not land.** Moving the face onto the codec needs an
`impl Payload<FlatBuffers>` for every type an interaction carries. The emitter
writes one only where `mints_root_table` holds, which is a `struct` or a
`union`. The interaction-face fixture's payloads are four named scalars and one
enum, plus one struct; measured by adding the codec to `generate_face` and
regenerating the fixture, exactly one implementation appears. The face does not
compile on the codec.

That is not the fixture's doing. A named scalar or an enum as an interaction
payload is idiomatic ridl and is what most of the corpus writes —
`signal target: Speed`, `command setLever(cmd: LeverCmd)`,
`query getLevel(limit: Level): Level`.

**Why it is a decision and not a patch.** A FlatBuffers root is a table, so a
scalar payload needs a generated wrapper table — the shape ADR-0019 decision 1
already gives a union — and that is a projection rule. ADR-0019 states no root
rule at all: the `.fbs` emitter writes no `root_type` and emits nothing for a
named scalar. A wrapper only the Rust codec knew about would break the
codec/schema agreement this whole design rests on, and putting it in the schema
moves every FlatBuffers snapshot. The same question binds E11.8 and E11.12, and
whatever is chosen fixes `MAX_SIZE` for such a root and therefore the bound
E16.4 advertises.

**What a taker needs.** A projection decision, in ADR-0019 or a record beside
it, answering: what a root table is for a payload that is not a struct or a
union; whether the `.fbs` emitter writes it; and what the bound of such a root
is. D-11's own text in the archived design note states the face-side shape that
follows — one encoding alias, emitted once per package — and stage K7's section
of that note is the measurement above.

## Known gaps

| Gap                                                                                             | Issue             |
| ----------------------------------------------------------------------------------------------- | ----------------- |
| A type reaching a cross-package reference carries no codec; the emitted source says so per type | driftsys/ridl#467 |
| An anonymous inline constraint, `step`, and a map key's uniqueness are unchecked by `verify`    | driftsys/ridl#469 |
| No `Payload` for a named-scalar or enum payload, which blocks D-11                              | driftsys/ridl#470 |
| A conforming writer's omitted default is refused                                                | driftsys/ridl#472 |
| A union-arm retirement would shift wire discriminants silently                                  | driftsys/ridl#302 |

driftsys/ridl#467 is the widest of these: ten of the corpus's fifteen payload
types are withheld a codec, every type touching the prelude or an import. Each
withheld type gets a `const __RIDL_FB_NO_CODEC_<NAME>: () = ()` in the emitted
source, carrying a doc comment that names the type, the member that could not be
judged, the reason and the issue, so a consumer meets a reason rather than an
unsatisfied trait bound far from the cause. The fix it states: hand `generate`
the other packages, which `fb_projection::Packages` already takes as `others`,
in E11.14 or in the plugin system's `CodegenRequest`.

## Trace

- Roadmap: [`../ROADMAP.md`](../ROADMAP.md) — E11.7
- Decisions: [ADR-0019](../decisions/ADR-0019-flatbuffers-projection-rules.md),
  [ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
  decisions 2, 5, 6, 7 and 9,
  [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decisions 3,
  7, 8 and 10, [ADR-0023](../decisions/ADR-0023-interaction-face-generation.md)
  decisions 2 and 5
- Design records: [`ridl-rt.md`](ridl-rt.md),
  [`interaction-face.md`](interaction-face.md)
- The reasoning and the stage-by-stage findings:
  [`../archive/2026-09-20-flatbuffers-codec-design.md`](../archive/2026-09-20-flatbuffers-codec-design.md)
  and
  [`../archive/2026-09-20-flatbuffers-codec-plan.md`](../archive/2026-09-20-flatbuffers-codec-plan.md)
