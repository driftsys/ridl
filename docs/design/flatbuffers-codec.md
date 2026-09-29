# The FlatBuffers payload codec

The generated `Payload<FlatBuffers>` implementation a package carries, story
E11.7, as built. The binding choices are
[ADR-0019](../decisions/ADR-0019-flatbuffers-projection-rules.md) (the
projection this codec must agree with byte for byte),
[ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
decisions 2, 5, 6 and 7 (the encoding matrix, the one FlatBuffers runtime crate
a codec is permitted, one cargo feature per encoding, the codec as generated
Rust) and [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md)
decisions 7 and 8 (`Encoded.bytes`, the `flatbuffers` feature). The reasoning
behind each decision, and the stage-by-stage record of what each round of work
found, is the archived design note
[`../archive/2026-09-20-flatbuffers-codec-design.md`](../archive/2026-09-20-flatbuffers-codec-design.md)
and its plan
[`../archive/2026-09-20-flatbuffers-codec-plan.md`](../archive/2026-09-20-flatbuffers-codec-plan.md).

**Every decision of that note is built.** The last was D-11, the generated
interaction face moving off its `ReprC` placeholder and onto this codec, landed
by stage K9b: the face names `::ridl_rt::encoding::FlatBuffers` at every buffer
it sizes and every `Ref` it builds (through a per-package `pub type Wire` alias
until 2026-09-29, when the generated-name collision design removed it,
driftsys/ridl#588), and the hand-written `Payload<ReprC>` implementations its
fixture carried are deleted. The projection decision that had blocked it,
**driftsys/ridl#470**, is
[ADR-0019 decision 8](../decisions/ADR-0019-flatbuffers-projection-rules.md):
every declaration has a root table, and a named scalar, an enum and an enum set
are rooted in a box. What the face does with this codec is
[the interaction-face design record](interaction-face.md).

## Where the code is

| What                                                                 | Where                                                       |
| -------------------------------------------------------------------- | ----------------------------------------------------------- |
| The projection facts both emitters read, and each declaration's root | `crates/ridl-ir/src/projection/flatbuffers.rs`              |
| The `.fbs` schema emitter                                            | `crates/ridl-backend-flatbuffers/src/lib.rs`                |
| The codec emitter                                                    | `crates/ridl-backend-rust/src/codec.rs`                     |
| The per-type refusal (`check_flatbuffers_bound`)                     | `crates/ridl-backend-rust/src/lib.rs`                       |
| The reader and builder the emitted code calls                        | `crates/ridl-rt/src/flatbuffers.rs`                         |
| The round trip, run rather than compiled                             | `crates/ridl-backend-rust/tests/flatbuffers_roundtrip.rs`   |
| Conformance against planus, and the `wasm32` check                   | `crates/ridl-backend-rust/tests/flatbuffers_conformance.rs` |
| The encoding option (`WireEncoding`) the entry points check          | `crates/ridl-backend-rust/src/lib.rs`                       |
| The `View` trait every emitted view implements                       | `crates/ridl-rt/src/payload.rs`                             |
| The face over this codec, run rather than compiled                   | `crates/ridl-backend-rust/tests/interaction_face.rs`        |

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
already names `ridl-rt`. `ridlc::run_build` calls `generate_pipeline`, whose
output is `generate`'s plus the descriptors and the face (E11.14), so
`ridl build --emit rust` carries the codec, and `crates/ridlc/src/lib.rs`
renders `ridl-rt = { version = "0.4", features = ["flatbuffers"] }` in the
manifest it writes (the version is a literal there, and moves with each
release).

Per table: three free functions — `__ridl_fb_{encode,verify,decode}_<Name>` —
and one view struct `<T>FbView<'a>`, at the generated package's module scope
rather than in a submodule. At module scope a same-package reference is spelled
as `type_path` spells it everywhere else, and the functions can read a generated
type's private inner value the way any other item of that module can, which is
what lets `decode` build an enum set that publishes no constructor. The
`__ridl_fb_` prefix collides with no typl name, because no typl name begins with
an underscore, and the tail is the declared name rather than its `snake_case`
(since 2026-09-29, the generated-name collision design): `snake_case` is not
injective over the names TYPL-009 accepts, so `type HTTPServer` beside
`type HttpServer` gave one function name twice (E0428). Each function carries
`#[allow(non_snake_case)]` for the CamelCase tail. The view's `bytes`, which
hands back the verified buffer, is a method of `ridl_rt::payload::View<'a>`
(ADR-0021 decision 20) and not an inherent method, so a field whose accessor is
named `bytes` does not meet it: the accessor is inherent and wins the dot call,
and a consumer reaches the buffer through `View::bytes(&view)`. Every primitive
and prelude type the codec writes at package scope is written by its `::core::`
or `::std::` path, because a declaration may carry any of those names.

A `Payload<FlatBuffers>` implementation is written for every declaration
`ridl_ir::projection::flatbuffers::root_table` names a root for, which is every
declaration that projects a type at all: a struct over its own table, a union
over its wrapper, and a named scalar, an enum and an enum set over the box table
ADR-0019 decision 8 gives them. A constant projects no type and gets none.

**The box root is decision 2's box read at the root.**
`table <Name>Box { value:
<resolved type> (id: 0); }` is the same table a
non-table union arm is isolated in, and the two share one implementation rather
than two that agree: `Codec::box_bodies` writes the encode, the verify and the
decode once, and decision 2's arm and decision 8's root both call it. The slot
comes from the layout the projection hands over, so the table the codec writes
and the table `max_size` charges cannot be two different tables, and the `.fbs`
emitter reads the same layout for the same reason. The one difference between
the two call sites is that a root table is already followed by the time
`Payload::verify` reaches it, where an arm's sits behind the union's value
offset and is followed first. The box's `value` field is not optional, and a
buffer with no slot for it reads as any absent non-optional field does (see
"Presence, defaults, and what an absent field means"): as the FlatBuffers
default when 0 is a legal value of the box's type, and as
`Malformed::MissingRequired` when it is not. The view a box hands back is the
value rather than a borrow, and what that costs depends on the backing: a scalar
or an enum is one read, and nothing a nested view would save, while a string or
a bytes backing **allocates**, where a struct field of the same type is borrowed
in place as `&'a str` or `&'a [u8]`. The generated doc comment on `value()` says
which of the two a given box is, since a caller in a hot path needs to know.
Handing back a borrow instead would mean a second view type for those two
backings alone, which is not worth the surface; a caller that wants the bytes
without the allocation reads them off `bytes()`.

Decision 8 narrowed what `generate` accepts, in one direction worth naming. A
declaration whose box cannot be charged is now refused in its own right, over
the box's own field: a named scalar with no width, a `string` or `bytes` one
with no length bound, and an enum set with an unspecified width all answer
`` `pkg.Name.value` has no finite FlatBuffers bound ``. A named scalar with **no
backing at all** — what the front end leaves after a parse error such as
`type X:` — is told apart from those and answers
`` `pkg.Name.value` carries no type ``, because it is malformed IR rather than
an unbounded shape. (The `.fbs` emitter is more tolerant of that one: it
defaults a backing-less named scalar to `string` and emits the box without
complaint. That predates decision 8 and is left alone.) The compiler produces no
such IR (typl §4.4–§4.5 default a length to `[0..256]` with TYPL-103, and the
checker derives a width for every numeric named scalar), so this is the same
totality-over-IR-handed-in-directly standing every other case of that refusal
has. It did change the hand-built fixtures in
`crates/ridl-backend-rust/src/tests.rs`, which now carry what the compiler
emits.

One consequence of that is worth stating rather than leaving a reader to infer
it from a missing fixture. The **vacuous** constructor path — `new` rather than
`new_unchecked` — is still live and still tested for a `boolean`, `integer` or
`float` backing, but for a `string` or a `bytes` one it is now unreachable: such
a type is vacuous only if it carries no length bound, and a type with no length
bound has no box bound, so no IR that `generate` accepts can reach it. It is
dead code rather than an undertested path, and there is no fixture for it
because there can be none.

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
constrained named scalar's `impl` block.

**`check` is `pub(crate)` since E11.14 (2026-09-21); whether it becomes `pub` is
still Epic 10's call.** It carried no visibility modifier while every caller was
a function generated into the same module, or a child `pub mod` of it. That
stopped being true when E11.14 closed driftsys/ridl#467: the codec of one
package now resolves a reference into another package of the same build and
calls that type's `check` through the emitted module tree, where a private item
is unreachable. The emitted crate is one crate per build, so `pub(crate)`
reaches every generated caller and adds nothing to the crate's public surface —
which leaves open item 2 of the 2026-09-20 design note exactly where it was:
making `check` `pub`, for a consumer validating a value it did not build, is a
surface commitment Epic 10 takes, and `pub(crate)` does not take it. Every
`__ridl_fb_*` function moved from a bare `fn` to `pub(crate)` in the same change
and for the same reason. The named-scalar check hangs off the one `Codec::wire`
site every position reaches, so it covers every position the projection admits a
named scalar at — an inline field, a string's or a byte sequence's bytes, an
array element, a map key, a map value, a tuple field, a nested table, a union's
table arm and its boxed arm, and any of those behind an optional.

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

## The face names this codec through one alias

`generate` emits the codec, and `generate_face` appends the interaction face to
exactly the items `generate` emits. There is one codec emitter and one call to
it, so a face compiles over the same implementations a consumer of `generate`
gets rather than over a second set written for it.

The face names the encoding by its full path at each site: `MAX_BUFFER_SIZE`,
`EVENT_SOURCE_BUFFER_SIZE`, every `Ref::encode` and `Ref::verify` the face
builds name `::ridl_rt::encoding::FlatBuffers`, as the codec's own `Payload`
implementations do. From stage K9b to 2026-09-29 they named it through a
per-package alias, `pub type Wire`, which a declaration or an interface named
`Wire` collided with; the generated-name collision design removed the alias
(driftsys/ridl#588). The encoding is stated by
`ridl_backend_rust::WireEncoding`, which defaults to `FlatBuffers` and reaches
the entry points through `generate_face_with(package, wire)` and
`generate_pipeline`; `generate_face(package)` is the defaulted form. The entry
points check the option against the one variant, so `generate`'s output is
unchanged by it. The reasoning, and why the face takes no `E: Encoding` type
parameter, is design note D-11 and
[the interaction-face design record](interaction-face.md).

**One consequence of the codec building at the tail reaches every consumer.**
`Encoded.bytes` is a subslice of the output buffer, so a caller that
reconstructs `&buf[..len]` sends leading bytes the encoder never wrote. The face
passes the returned slice on unchanged at all four send sites (stage K7), and
`the_encoded_bytes_are_not_a_prefix_of_the_buffer` in
`crates/ridl-backend-rust/tests/interaction_face.rs` states the property
directly: the equally long prefix of the same buffer fails `verify`.

## Presence, defaults, and what an absent field means

- An optional field is written when present and omitted when absent. A present
  value is written **even when it equals the field's FlatBuffers default**. An
  optional scalar or enum field projects with `= null` (ADR-0019 decision 9), so
  the schema states its presence too, and a reader that follows the schema tells
  a present default from an absent optional. A `Some(0)` survives a codec →
  planus → codec round trip, measured by
  `an_optional_scalar_at_its_default_survives_a_foreign_round_trip`. An optional
  string or table needed no marker: an absent offset and a present one are
  already distinct in the format.
- A non-optional field is always written. That writer rule is kept although a
  conforming writer omits a field at its default: this encoder reserves inline
  space for every field, so omitting one saves no bytes, and a reader generated
  before driftsys/ridl#472 refuses an absent non-optional field.
  `this_codec_writes_a_non_optional_field_at_its_default` pins it through the
  vtable, because no round trip through this codec can see the difference.
- An absent non-optional **scalar or enum** field reads as its FlatBuffers
  default: 0, or the enum's zero member. That is how a conforming writer writes
  a field at its default, and it holds in `verify`, in `decode` and in a view's
  accessor, in every table — a struct's, a tuple's, a map entry's, a union arm's
  box — and in a box root. When 0 is not a legal value of the field's type, the
  absent field is `Malformed::MissingRequired`. Legality is decided at
  generation time from the declaration, so no check runs in the generated code:
  an enum is legal at 0 when it declares a zero member, an enum set always is (0
  is the empty set), and a numeric scalar — named or with an inline constraint —
  is when 0 lies in its range and, if it declares a `step`, on its grid
  `min + n·step`. `[-1.5..1.5 step 1.0]` holds -1.5, -0.5, 0.5 and 1.5, so an
  absent field of that type is refused. The generated `check` is not used for
  this, because it ignores `step` (driftsys/ridl#469) and an inline constraint
  has none. What cannot be decided — a `step` with no lower bound, or a bound
  that is not plain decimal text — is refused.
- An absent non-optional string, bytes, table, union or collection field is
  `Malformed::MissingRequired`: an offset has no default.
- A field whose enum declares no zero member carries `= null` in the schema
  (ADR-0019 decision 6). When the typl field is not optional, an absent one is
  `MissingRequired`, which is the rule above for a type that excludes 0.

What this gives up is telling a missing non-optional scalar from a present 0
where 0 is legal. FlatBuffers cannot state that difference in any case — no
scalar field can be `required` (ADR-0019 decision 6) — and a field whose
presence matters is declared optional.

`decode` and the accessors changed together with `verify`. Before
driftsys/ridl#472 they read an absent non-optional field at offset 0, which is
the buffer's root offset, so relaxing `verify` alone would have let `decode`
build a value out of the buffer header.

An optional marker outside a table field — an array element, or a map entry's
value — is a `GenerateError` from this codec: a FlatBuffers vector has no absent
element, and this codec gives a map entry no absent half. `ridl check` accepts
both, so the refusal comes from the backend. The schema backend still states an
optional map value, with `= null`. An optional map key is refused earlier, by
`ridl check` (TYPL-209); the codec's own refusal of one is kept as a second
guard that no checked source reaches.

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
them, or `ridl-backend-flatbuffers`, is promoted to a normal dependency at any
distance — the guard walks the normal-edge closure, not just the direct edges.
The `.fbs` is emitted from the same IR as the codec.

**What "from the same IR" does and does not buy.** It rules out the schema and
the codec being generated from two different inputs. It does not make the suite
a check against ADR-0019: both emitters read the same shared projection, so a
wrong slot id **in the projection** moves both together and planus faithfully
follows the wrong `.fbs`. What this suite proves is that the codec's bytes are
FlatBuffers and agree with the emitted schema; agreement between the emitted
schema and ADR-0019 rests on the schema backend's own snapshots.

Twelve cases, in `crates/ridl-backend-rust/tests/flatbuffers_conformance.rs`:

1. bytes this codec writes are read by planus and compare equal field by field;
2. bytes planus writes are accepted by `verify` and decode to the same value;
3. a planus buffer whose vtable is **truncated** — a shape this codec's own
   writer never produces, since it writes a slot for every field — decodes with
   the missing slots read as absent;
4. a planus buffer that omits a default-valued non-optional field verifies and
   reads as the default through `verify`, `decode` and the view's accessors —
   one buffer per position (the root table, a **nested** table, a tuple's table,
   a map entry, a union arm's box) and one per scalar kind (an integer, a float,
   a boolean, an enum set, and an enum at its zero member);
5. an optional scalar present at its default survives a codec → planus → codec
   round trip, because it projects with `= null`;
6. a **scalar root** — a named scalar in its box table (ADR-0019 decision 8) —
   is read by planus in the one direction and written by planus in the other;
7. a box root **with no value slot** — which is what planus writes for a box at
   its default — reads as the default, for a named scalar and for an enum;
8. an **empty string box**, which a conforming writer writes as a present
   zero-length slot rather than eliding, round-trips;
9. an absent field whose type **excludes 0** — the range `[1..10]`, or an enum
   with no zero member — is refused with `MissingRequired`, in a buffer this
   codec wrote from the first version of an evolution fixture and read with the
   second, which appends the field, and in an empty planus box; the appended
   field whose type admits 0 reads as 0;
10. this codec writes a non-optional field at its default: its buffer carries a
    slot for every non-optional field of `Report` at its default, for the
    scalars of a nested table, a tuple, a map entry and a union arm's box, and
    for a box root;
11. whether an absent field reads as 0 **follows its type**: one planus buffer
    that omits a field is read as five structs that differ only in that field's
    type — `integer [0..10]` and `[-1.0..1.0 step 1.0]` read as 0, an enum whose
    zero member is declared second reads as that member, and `integer [1..10]`
    and `[-1.5..1.5 step 1.0]` (a grid without 0) are refused; an empty planus
    box is read the same way;
12. `ridl diff` calls an append compatible **exactly** when this codec reads a
    buffer of the old version: over the evolution fixture, whose sixteen structs
    each gain one field — optional, legal at 0, excluding 0, or with no default
    at all — every append `ridl_diff::diff_packages` calls compatible verifies,
    and every one it calls breaking is refused with `MissingRequired`
    (driftsys/ridl#598).

Cases 4, 5, 7, 9 and 11 are the default rule of driftsys/ridl#472, below: case 4
is the reader rule at each table position and for each scalar kind, case 7 the
same rule at a root, cases 9 and 11 its limit, and case 5 the optional half.
Case 10 is the writer rule the issue kept, and case 8 is the bound on how far
the rule reaches. Case 7 was first written because the rule it then measured, a
refusal, was otherwise pinned only as generated text: deleting the branch that
enforced it turned every snapshot carrying a box root red — fourteen under
`--lib` and six more across `ridlc` — and left every round trip and every other
conformance case passing, since nothing built such a buffer.

Two mutations were applied and run, and each is what says the suite is not
decorative; the counts below were measured on 2026-09-29. Shifting the union
discriminant by one in `codec.rs` fails five conformance cases — 1 to 5, each of
which carries a union field through a buffer another implementation writes or
reads — and, of the round-trip suite, only the case that pins one value's bytes:
every other round trip through this codec alone passes. A codec that disagrees
with its own schema is exactly what a round trip through itself cannot see.
Turning a short vtable into an error in `ridl-rt` leaves the round-trip suite
passing and fails five conformance cases — 3, 4, 7, 9 and 11, each of which
reads a buffer that planus or an earlier version of a type wrote without a
trailing slot.

**What the cases do not reach.** An empty vector, a multi-byte UTF-8 string, and
any assertion about alignment. A default-valued scalar inside a nested table was
the fourth, and the sharpest; case 4 now reaches it.

### A default and presence (driftsys/ridl#472)

A conforming FlatBuffers writer omits a table field whose value equals its
declared default. Until driftsys/ridl#472 was decided on 2026-09-29, that met
this codec's presence rules from both sides: it refused a buffer that omitted a
non-optional default, and a present default-valued optional scalar was lost by a
foreign re-encode, because the schema stated no presence for it. Both halves are
now closed, by the rules in "Presence, defaults, and what an absent field
means":

- **Non-optional.** An absent scalar or enum reads as the default, unless 0 is
  not a legal value of its type (cases 4, 7 and 9).
- **Optional.** An optional scalar or enum projects with `= null` (ADR-0019
  decision 9), so a foreign re-encode keeps a present 0 (case 5).

**At a root.** A box's `value` is a field like any other, so a box of 0, or of
an enum's zero member, that planus writes with no slot is read as that value.
Before #472 this was the rule's widest reach — at a root an omitted field is the
whole payload — and ADR-0019's consequence on decision 8 recorded it. What is
left is a box whose type excludes 0, which has no slot only when its writer
wrote no value, and is refused.

**It reaches only the kinds a FlatBuffers default applies to.** A string and a
bytes field have no default: an offset is present or absent, and a conforming
writer writes an empty string as a present zero-length one, so an empty `Label`
survives a foreign round trip. Measured rather than reasoned, by
`an_empty_string_box_round_trips_through_planus`: planus writing
`LabelBox { value: Some("") }` produces a present slot, and this codec verifies
it and decodes `Label("")`. What a non-optional string box refuses is an
**absent** offset, which is a null string, and typl gives a non-optional field
no way to state one.

**Wire compatibility.** The writer did not change, so a reader generated before
#472 reads what this one writes: the bytes are identical. A reader generated
after it reads everything an earlier writer wrote, which carried every field. A
foreign consumer that regenerates from the schema sees each optional scalar or
enum field become an optional value (`planus`: `Option<T>`), and reads a buffer
a foreign writer produced from the earlier schema, with such a field omitted at
0, as absent where it read 0; this codec already read that field as absent.

**Appending a field (driftsys/ridl#598, decided 2026-09-30).** A reader of a new
version meets a buffer of the old one with every appended field absent, so
`ridl diff` classifies an append by what this reader does with that absence
(ADR-0008 decision 14 as amended, typl §7.4). An appended optional field is
compatible. An appended non-optional scalar, enum or enum set is compatible when
0 is a legal value of its type, which is when this codec reads it as 0. Every
other appended non-optional field — a string, bytes, table, union or collection,
or a scalar or enum whose type excludes 0, such as `c : Level [1..10]` — is
breaking, because this codec refuses the old buffer with `MissingRequired` (case
9). The codec and the diff read one definition of "0 is legal", `ridl_ir::zero`:
`zero_is_legal` in `codec.rs` calls `ridl_ir::zero::range_holds_zero`, and the
lowering's enum zero member comes from `ridl_ir::zero::enum_zero_member`. Case
12 ties the two together over one fixture. E11.8 meets the same reader rule in
proto3 terms, where it is forced: proto3 gives a non-optional scalar no
presence, so an absent one is 0.

**A foreign reader does not refuse.** The emitted `.fbs` gives a field such as
`c : Level [1..10]` no default annotation — FlatBuffers has no way to mark a
scalar field required (ADR-0019 decision 6) — so a foreign reader generated from
it, planus or `flatc`, reads an absent `c` as 0, a value outside the field's
range, where this codec refuses the buffer. This codec writes every non-optional
field, so under the rule above such an absence reaches a reader only from an
append `ridl diff` reports as breaking, or from a foreign writer that left the
field out; a foreign consumer that must reject it checks the range itself. This
is recorded rather than tested: a test would need a second checked-in planus
reader, generated from the evolution fixture, only to measure what the
FlatBuffers format already specifies for an absent scalar.

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

## Known gaps

| Gap                                                                                          | Issue             |
| -------------------------------------------------------------------------------------------- | ----------------- |
| An anonymous inline constraint, `step`, and a map key's uniqueness are unchecked by `verify` | driftsys/ridl#469 |
| A union-arm retirement would shift wire discriminants silently                               | driftsys/ridl#302 |

**driftsys/ridl#467 is closed by E11.14 (2026-09-21).** It was the widest of
these: ten of the corpus's fifteen payload types were withheld a codec, every
type touching the prelude or an import, each getting a
`const __RIDL_FB_NO_CODEC_<NAME>: () = ()` in the emitted source whose doc
comment named the type, the member that could not be judged, the reason and the
issue. The fix is the one the gap stated — `generate` is handed the other
packages of the build, which `fb_projection::Packages` already took as `others`
— plus the naming half it did not: a reference the codec resolves in another
package is written as a path through the module tree `ridlc` writes, rather than
as a bare identifier, and the items such a path names are `pub(crate)`. The
withheld note remains for the causes that are not a cross-package reference,
which is what it now says.

**driftsys/ridl#472 was decided on 2026-09-29.** It was a divergence rather than
a gap — a conforming writer's omitted default was refused, and a present
default-valued optional scalar was lost by a foreign round trip — and it is
closed by the reader rule and ADR-0019 decision 9, as "A default and presence"
above records. Its one remainder, driftsys/ridl#598, was decided on 2026-09-30:
`ridl diff` now calls an append breaking where this codec refuses the old buffer
("Appending a field" above).

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
