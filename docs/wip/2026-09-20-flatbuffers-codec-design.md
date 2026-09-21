# The FlatBuffers payload codec — design note

**Status: disposed of, 2026-09-20.** The disposition comment on this note's pull
request took D-2, D-3, D-5, D-8, D-9 and D-10 as proposed; D-4, D-7, D-11 and
D-12 with an addition each; and amended **D-1 and D-6** together on where the
shared projection facts live and on the codec's entry point. Every amended
decision below is marked, and the amendment is written into the decision itself,
not left to the comment. Open items 1 and 3 are closed here; open item 2 stays
Epic 10's. The implementation plan is
[`2026-09-20-flatbuffers-codec-plan.md`](2026-09-20-flatbuffers-codec-plan.md),
which carries the amendments. The records of §5 still move with the stage that
needs each one.

**Story:** roadmap E11.7, the FlatBuffers payload codec. **Done when** a payload
round-trips through the library.

## 1. Context

`ridl-rt` 0.1 defines what a payload is: `Payload<E>` with `MAX_SIZE`, `View`,
`encode`, `verify` and `decode`, reached only through the proof type `Ref`
([the ridl-rt design record](../design/ridl-rt.md), "The payload encodings and
the proof type";
[ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decision 7).
Nothing implements it. The generated interaction face of E11.13 compiles only
because its fixture carries hand-written `Payload<ReprC>` implementations, which
[the interaction-face design record](../design/interaction-face.md) lists as its
first placeholder, to be retired by E11.7, E11.8 or E11.12.

`ridl-backend-flatbuffers` emits a `.fbs` schema and nothing else. It is a
schema projection for a foreign toolchain, and its rules are
[ADR-0019](../decisions/ADR-0019-flatbuffers-projection-rules.md): a union
isolated in a wrapper table, a non-table union arm boxed, every struct a
`table`, a map a vector of entry tables with no `(key)`, the target's own name
scopes, and `= null` on a field whose enum declares no zero member. No Rust
reads or writes those bytes today.

E11.7 is the first of the three codecs, and it is the one that decides the shape
the other two inherit. It is also the encoding
[ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
decision 2 puts on two boundaries at once — within a node, and between a wasm
codec and the host generated with it — so it is the encoding a within-process
face should be built over.

Three couplings are live while this is written:

- **Epic 10**, the typl value objects, owns the checked constructors every
  decoded value must pass through. Task 3 landed in driftsys/ridl#420; the rest
  is in flight.
- **Epic 16**, the catalog descriptor, computes a FlatBuffers upper bound per
  payload in its own crate (`2026-09-13-catalog-descriptor-plan.md`, Tasks 5 and
  7). That is the same number as `MAX_SIZE`.
- **E11.13's face**, which names `Payload<ReprC>` in every buffer it sizes.

## 2. The decisions

### D-1 — the codec is Rust output, emitted by the Rust backend

**Amended by the disposition, 2026-09-20**, on both halves: the entry point and
where the shared facts live. The text below is the amended decision.

`ridl-backend-rust` emits the `Payload<FlatBuffers>` implementations **into
`generate`'s own output**, not behind a companion entry point. The codec is
needed by a package whose consumer never dispatches, and `generate` already
names `ridl-rt`, so a third entry point beside `generate` and `generate_face`
would split the output a plain consumer needs across two calls. What made a
separate entry point look necessary was the proof mechanism — every rustc proof
compiles `ridl-rt` bare — and D-12 settles that instead.
`ridl-backend-flatbuffers` keeps emitting the `.fbs` schema and gains no Rust
emission.

The two emitters share no emission code. What they share is ADR-0019, and one
place that reads it: the projection facts both need — a type's table layout, its
field slots, a union arm's discriminant, a type's size bound. **Those facts live
outside any backend**, in `ridl-ir` or in a small projection crate, where
`ridl-backend-flatbuffers`, `ridl-backend-rust` and later `ridl-descriptor` all
reach them without one crate depending on a backend. A dependency of the Rust
backend on the FlatBuffers backend would make a backend a library other crates
link, which is the opposite of what
[ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
decision 9 makes a backend in step 2. A drift test asserts the two emitters
agree on every fixture.

**Rejected:** the FlatBuffers backend emitting Rust. A backend is named for the
language it writes; the codec links `ridl-rt` and names the domain types the
Rust backend emits, so it belongs to that backend's output or it duplicates it.

### D-2 — `encode` builds at the tail, and `Encoded.bytes` is a subslice

A FlatBuffers buffer is built back to front: the root offset is written last and
sits at the low end of the finished buffer. `Payload::encode` is handed one
`&mut [u8]` and no allocator. The encoder therefore builds from the end of the
slice downwards and returns the finished buffer as a **subslice of `out` ending
at its end**, not as a prefix of it.

This contradicts one line of `crates/ridl-rt/src/payload.rs` — "Writes `self`
into the front of `out`", and `Encoded.bytes`'s "a prefix of the output buffer".
It is a change to the documented contract, not to any signature: `Encoded`
already carries `bytes: &[u8]`, and every consumer in the tree reads
`Ref::bytes()` and passes it on. The amendment is ADR-0021 decision 7's
paragraph plus those two doc comments, made in E11.7's own records task.

**Rejected:** building at the tail and then moving the result to the front with
one `copy_within`. It keeps the prefix wording at the cost of a memmove of up to
`MAX_SIZE` bytes on every encode, on the path that exists to avoid copies.

**Rejected:** a forward-writing encoder that back-patches offsets. FlatBuffers
offsets are backward-relative; a forward writer has to buffer every table until
its children are placed, which is the allocator this design does not have.

### D-3 — `View<'a>` is a generated accessor over the verified bytes

**Amended 2026-09-21 (stage K5), on which fields the accessor reads in place.**
A scalar, a string, a byte sequence and a nested table are read in place, which
is the property ADR-0020 decision 2 rests on. **A union and a collection decode
on access**: a union's arms have no one view type, and an indexable vector view
needs a view type per element shape and per nesting level. That is a scope
decision, not a rejection of the fuller form — the trait does not change when a
later stage adds an element-wise view — and §4b records it. The text below is
otherwise as disposed.

For a type `T`, `<T as Payload<FlatBuffers>>::View<'a>` is a generated
`TFbView<'a>` holding the verified buffer and the root offset, with one accessor
method per field returning the field's own view — a scalar by value, a string as
`&'a str`, a table as a nested view, a vector as an indexable view. The view
allocates nothing and borrows the buffer.

This is what ADR-0020 decision 2 buys: FlatBuffers is the within-node and
codec-to-host encoding because the receiver reads the buffer in place. A view
that is only the bytes would make every read go through `decode`, and a view
that is the parsed value would make `verify` allocate.

**Rejected:** `View<'a> = &'a [u8]`, the minimum that compiles.

### D-4 — the constraint check runs in `verify`, over a borrowed value

**Taken, 2026-09-20.** The `check` beside `new` is a request on lane C, and Epic
10 Task 6 is driftsys/ridl#443, which was told while still open rather than
after it merged. Whether `check` is public stays Epic 10's call — open item 2,
left open.

`Payload::decode` returns `Self` and cannot fail, and every generated domain
type is an Epic 10 value object whose constructor rejects an out-of-constraint
value. The two meet in `verify`: the generated `verify` walks the buffer and
checks the typl constraint of every leaf it reaches, so by the time a `Ref`
exists the constraints hold, and `decode` builds each value with the unchecked
constructor and neither fails nor panics. This is what
`crates/ridl-rt/src/payload.rs` already states for the trait — structure and
constraints in one pass — and E11.7 is the first implementation of it.

Checking without constructing needs something Epic 10 does not emit yet. Its
constructors take the inner value by value (`new(inner: String)`), so calling
`new` per leaf inside `verify` would materialize and drop a value per field.
**Epic 10's emission gains one associated function per value object**,
`fn check(inner: &Inner) -> Result<(), ::ridl_rt::payload::Violation>`, and
`new` becomes the composition of `check` and `new_unchecked`. The rule then has
one definition, `verify` checks over a borrow, and `decode` constructs without
re-checking.

That is an additive change to a story in flight, so it is named here and made
there: it belongs to Epic 10's emission, not to the codec.

**Rejected:** a fallible `decode`. It changes the `ridl-rt` trait, and it moves
the failure past the proof type, which exists to make exactly this unreachable.

**Rejected:** `verify` checking structure only, with `decode` calling `new` and
panicking on a violation. A malformed-but-structural buffer would then be a
panic in a consumer rather than a `VerifyError::Contract`, which is the error
ridl §10.2 requires.

### D-5 — what `verify` checks, and the `Malformed` mapping

The generated `verify` is a total walk of the type's own shape, in one pass, in
this order: buffer length against `MAX_SIZE`, then the root offset, then each
table's vtable and each field the type declares, recursing.

| Condition                                                        | Reported as                        |
| ---------------------------------------------------------------- | ---------------------------------- |
| `buf.len() > <T as Payload<FlatBuffers>>::MAX_SIZE`              | `Malformed::TooLarge`              |
| an offset, length or vtable entry outside the buffer             | `Malformed::OutOfBounds`           |
| a field the projection makes required is absent                  | `Malformed::MissingRequired`       |
| a string that is not UTF-8                                       | `Malformed::Utf8`                  |
| a union discriminant with no arm, or a present tag with no value | `Malformed::Union`                 |
| a leaf outside its typl constraint                               | `VerifyError::Contract(Violation)` |

**Amended 2026-09-21 (stage K5), on the last row of that table, closed
2026-09-21 (stage K6).** A leaf outside its typl constraint is
`VerifyError::Contract` for an enum and an enum-set discriminant and for a
collection's declared element count, all three as written. Stage K5 left a named
scalar's own range, length and pattern unchecked; stage K6 adds `check` beside
`new` (D-4) and calls it from here, over a borrow, so the row now reads as
written for every leaf the table names. §4c records what closed it. Every other
row of the table is as disposed.

**Every scalar is read with `from_le_bytes` over a copied byte array**, so the
walk never dereferences an unaligned pointer and a generated `verify` never
produces `Malformed::Unaligned`. The buffer may therefore arrive at any
alignment, which is what a transport hands over.

`Malformed::TooDeep` and `Malformed::TooManyTables` are likewise never produced
by a generated `verify`: the walk recurses over the declared type, whose depth
is finite and known at generation time, and it visits each declared field once.
Both variants stay in `ridl-rt` for a hand-written implementation over a dynamic
schema. The length check against `MAX_SIZE` is what bounds work on a hostile
buffer, in place of the verifier limits another implementation would carry.

### D-6 — the size bound has one implementation, and it is the projection's

**Amended by the disposition, 2026-09-20**, on where the implementation lives,
for D-1's reason.

`MAX_SIZE` is computed **in the shared home D-1 names** — `ridl-ir` or a small
projection crate — beside the projection facts that determine it, and exported
as a function over the IR returning `Option<u64>`, `None` meaning no finite
bound. The Rust emitter calls it and writes the value into the generated
`const MAX_SIZE: usize`. It is not `ridl-backend-flatbuffers`'s to own, because
`ridl-descriptor` needs the same number and must not depend on a backend to get
it.

Epic 16 needs the same number for `EncodedSizes.flatbuffers`. Its plan
(`2026-09-13-catalog-descriptor-plan.md`, Task 7) writes an independent
implementation in `ridl-descriptor::size::flatbuffers`. **That task is amended
to call this one** rather than derive the bound a second time; the note records
the amendment and Epic 16's plan carries it when Epic 16 starts. Two
implementations of one bound is the defect the catalog descriptor exists to
avoid: a codec whose buffer is larger than the size the descriptor advertises is
not detectable by any test either story would write.

The emitted constant is a literal with a comment naming the rule that produced
it, not a const-evaluable expression over the field types. The face's
`MAX_BUFFER_SIZE` stays the const-evaluable maximum it already is, over these
literals.

**Rejected:** the codec computing its own bound from the emitted Rust types.
That is a second implementation with the same divergence risk, and it cannot see
the padding and vtable slack the projection charges.

### D-7 — a type with no finite bound is refused, with a diagnostic

If the bound of D-6 is `None`, the emitter writes no `Payload<FlatBuffers>`
implementation for that type and returns a `GenerateError` naming the type and
the member that is unbounded. **§4a records stage K4's implementation of this
refusal**: the function exists (`check_flatbuffers_bounds`) and names the
member, but nothing calls it yet — K5 does, once it has a per-type
implementation to withhold.

**Taken with an addition, 2026-09-20.** typl makes every array and map bound
mandatory, defaults a `string` and a `bytes` to `[0..256]` when unspecified
(TYPL-103, a warning), and rejects a recursive composite reference, direct or
transitive (TYPL-206, an error). So a type with no finite bound **is not
reachable from typl source**. This diagnostic is totality over the IR, not a
case a user will meet: it must exist and be tested, and it does not need to be
loud. That closes open item 3, and the plan's first task needs no fixture work
to discover the answer.

This keeps the encoding total in the sense ADR-0016 decision 6 and ADR-0017
decision 4 fix for every projection: a construct the target cannot carry is a
diagnostic naming it, never a silent omission and never a wrong answer. A
`MAX_SIZE` of `usize::MAX` would compile, would size a stack buffer nothing can
allocate, and would turn a generation-time fact into a runtime failure in a
consumer.

**Rejected:** `usize::MAX`, and a saturating bound.

### D-8 — deterministic output; conformance is round-trip, not byte equality

The encoder is deterministic: the same value encodes to the same bytes, on every
run and every target. Field write order, vtable layout and vtable sharing are
fixed by the emitter, and a map's entries are written in the order of the
generated `Vec<(K, V)>`, which is the domain type the Rust backend already emits
for a typl map — so nothing sorts and nothing depends on a hash order. A test
asserts byte equality across two encodes of one value.

**Conformance is not byte equality with another implementation.** FlatBuffers
fixes no canonical encoding: vtable sharing, field ordering and alignment slack
are all writer choices, and two conforming writers differ. The conformance test
is a round trip through a second implementation over the emitted `.fbs`: bytes
this codec writes are read by `planus` and compare equal field by field, and
bytes `planus` writes are accepted by this codec's `verify` and decode to the
same value. `planus` is already this repository's FlatBuffers oracle.

This is a weaker claim than E11.8 makes for proto3, where byte-level conformance
against `protoc` is the story's own `Done when`. The difference is in the
formats, not in the rigor of the two stories.

### D-9 — absent, default and optional

- A typl optional field is written when present and omitted when absent. Absence
  is `None`; a present value is written **even when it equals the field's
  FlatBuffers default**, so that the reader can tell the two apart.
- A non-optional field is always written, and its absence in a buffer is
  `Malformed::MissingRequired` — FlatBuffers cannot mark a scalar or enum field
  required, which ADR-0019 records, so the requirement is the verifier's, not
  the schema's.
- A field whose enum declares no zero member carries `= null` in the schema
  (ADR-0019), and the codec treats an absent such field as
  `Malformed::MissingRequired` when the typl field is not optional.

**Rejected:** omitting a present default-valued field, which is what a
FlatBuffers writer does by default. It makes a present-and-default value
indistinguishable from an absent optional, which typl distinguishes.

### D-10 — the discriminant is `UnionArm.ordinal`, and #302 becomes visible

The codec writes a union arm's discriminant from the IR's `UnionArm.ordinal`,
which is 1-based, in declaration order, and which a tombstone keeps occupied.
The value is read through D-1's one place, so the codec and the `.fbs` emitter
compute it from the same source, and the drift test compares them on every
fixture — including a fixture with a retired arm.

That test is what driftsys/ridl#302 needs and does not have. The `.fbs` emitter
renders arms in declaration order with no explicit values, so after a tombstoned
retirement its implicit numbering and `UnionArm.ordinal` disagree, and the drift
test fails and names the arm. **E11.7 does not close #302**: the fix is explicit
member values in the schema, and #302 records that `planus` 1.3.0 rejects that
form, so the schema side waits on the oracle. What E11.7 removes is the silence
— the defect stops being a wire break nobody sees and becomes a failing test
with the arm named in it.

A non-table arm is boxed in a generated wrapper table (ADR-0019 decision 2), and
the codec encodes and decodes through that wrapper. The union itself sits in its
own wrapper table (decision 1).

### D-11 — the face names one encoding alias, emitted once per package

The generated package gains one alias, written from a backend option:

```rust
pub type Wire = ::ridl_rt::encoding::FlatBuffers;
```

Every place the face today names `ReprC` — `MAX_BUFFER_SIZE`,
`EVENT_SOURCE_BUFFER_SIZE`, each `Ref::encode` and `Ref::verify`, and each
`unreachable!` message — names `Wire` instead. The default for the option is
`FlatBuffers`, which is the encoding ADR-0020 decision 2 puts on the boundary an
in-process face sits at.

The face gains no type parameter. A `Client<'a, E: Encoding, P: ...>` would put
`E` on every descriptor, every provider trait and every caller, for a choice
that is made once per generated package, and it would need a `where` bound per
payload type on every impl.

With this, the fixture's hand-written `Payload<ReprC>` implementations are
deleted and the generated codec takes their place, retiring the first row of the
interaction-face record's "What is provisional" table. That record's other rows
are untouched.

**Taken with an addition, 2026-09-20: this stage settles driftsys/ridl#448.**
The `Wire` rebinding is the one place every generated constructor is rewritten,
so the change that does it either emits the catalog check
[ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decision 3
describes, or records in the same change that the check waits for E16.2 and
amends the two sentences #448 quotes. Today ADR-0021 decision 3 and ADR-0023
decision 5 both promise a check no constructor performs — no `.catalog()` call
exists under `crates/ridl-backend-rust/`. Touching every constructor and leaving
that promise false is the one outcome ruled out.

**Rejected:** a type parameter on the face. **Rejected:** leaving the face on
`ReprC` until E11.12. That would keep a hand-written placeholder in the tree
through two more stories, and it would mean the first real codec is never
exercised by the face's round trip.

### D-12 — features, the generated crate, `wasm32`, and the boundary

- **`ridl-rt`'s `flatbuffers` feature stops being empty.** It gates the shared
  reading and writing helpers the emitted code calls — the byte-order reads of
  D-5, the tail builder of D-2, the vtable walk — which belong in the library
  rather than in every generated package. The crate still takes **no external
  dependency**: the FlatBuffers runtime crate that ADR-0020 decision 5 permits
  under this feature is not used, so RA-01's ceiling stays unspent.
- **The generated `Cargo.toml`** that Epic 10 emits declares `ridl-rt` with the
  feature of each encoding the run emitted.
- **Allocation.** `verify` allocates nothing. `decode` allocates only where the
  domain type owns a `String` or a `Vec`, so a generated package over types that
  own neither is `no_std` with no allocator, and one over types that do is
  `no_std` with `alloc`.
- **`wasm32`.** The fixture package joins `just wasm-check`, because ADR-0020
  decision 2 makes the generated Rust compiled to `wasm32` the codec a
  TypeScript consumer loads.
- **The proof mechanism (added 2026-09-20).** Every rustc proof in the Rust
  backend compiles `ridl-rt` bare, with no feature, so a proof over
  `Payload<FlatBuffers>` output needs `--cfg feature="flatbuffers"` or a cargo
  build; the `--extern` mechanism alone will not compile the emitted code. This
  is what D-1's amendment relies on, and it lands with the emitter.
- **The release coupling (added 2026-09-20).** A consumer outside this
  repository cannot build the emitted crate until a `ridl-rt` release carries
  the feature's contents, because the emitted manifest names crates.io. The
  feature gating and the release are one decision, not two.
- **The minimum toolchain (added 2026-09-20).** `just compat-check` runs
  `cargo +1.83 test --all-features` against the packaged crate, so the helpers
  of the first bullet must build at `rust-version = "1.83"`, not only at the
  toolchain pin.
- **Out of scope.** E11.7 emits no frame (E11.1), no transport (E11.9), no
  proto3 (E11.8), no `repr(C)` layout (E11.12) and no TypeScript. It does not
  remove `#[repr(C)]` from the domain structs, which ADR-0020 decision 3 gives
  to E11.12.
- **Corrected 2026-09-20, after the disposition.** This bullet said E11.7 "does
  not make `ridl build` emit the codec — that is the row K0 adds as E11.14".
  **The D-1 amendment made that false**: `ridlc` calls `generate`, and the
  amendment puts the `Payload<FlatBuffers>` impls in `generate`'s output, so
  `ridl build --emit rust` carries the codec as soon as the emitter stage lands.
  E11.14 keeps the descriptors, the face and the manifest's encoding feature,
  none of which `generate` emits today. The plan's "What the D-1 amendment moved
  out of E11.14" states it, and E11.14's roadmap row is corrected by the plan's
  last task rather than here.

## 3. What this note does not decide

- The `repr(C)` projection rules. ADR-0020 decision 4 places them in their own
  record, written with E11.12.
- The frame: how an encoded payload is framed with its ordinal, kind, envelope,
  provenance and correlation is E11.1, and no decision here anticipates it.
- Which encoding a transport uses. D-11 fixes a default for a generated package,
  not a rule for a boundary; ADR-0020 decision 2's matrix is unchanged.
- Whether `ridl-diff`'s tombstone debt is closed. See D-10.

## 4. Open items

1. ~~**Where the shared projection facts of D-1 live in the crate's API.**~~
   **Closed by the disposition, 2026-09-20.** Not a public module of
   `ridl-backend-flatbuffers`: the facts live outside any backend, in `ridl-ir`
   or a small projection crate, so that `ridl-backend-rust` and later
   `ridl-descriptor` reach them without depending on a backend. The ground is
   ADR-0020 decision 9 — a backend is not a library other crates link — rather
   than the step-2 convenience this note offered as the second option.
2. **Open — Epic 10's.** **Whether `check` of D-4 is public in the generated
   package.** It has a use beyond the codec — a consumer validating a value it
   did not build — and making it public is a surface commitment Epic 10 should
   take, not this note.
3. ~~**A vector of tables and the `MAX_SIZE` of an unbounded collection.**~~
   **Closed by the disposition, 2026-09-20.** No such type is reachable from
   typl source: every array and map bound is mandatory, a `string` and a `bytes`
   default to `[0..256]` (TYPL-103), and recursion is an error (TYPL-206). D-7's
   diagnostic is totality over the IR, tested but not loud, and the plan's first
   task needs no fixture work to settle it.

## 4a. Stage K4, 2026-09-20: D-7's refusal exists, uncalled, and names its member

Stage K4 (plan Task 3) added D-7's refusal —
`ridl_backend_rust::check_flatbuffers_bounds`, in
`crates/ridl-backend-rust/src/lib.rs` — as a function nothing in the pipeline
calls yet. It does not implement D-7 in the sense of the compiler refusing
anything today; it is the refusal, ready for K5 to call once K5 has something to
withhold. The review round of 2026-09-20 corrected two things this addition
first got wrong, recorded here in place of what it said before.

**The ground for not wiring it in is per-type, not "this breaks existing
tests."** D-7's own text is "the emitter writes no `Payload<FlatBuffers>`
implementation _for that type_" — a per-type withholding, not a per-package
refusal. Before K5 there is no per-type `Payload<FlatBuffers>` implementation to
withhold, so `generate`/`generate_face` (which emit only domain types) have no
correct call site for this function at all, independent of what wiring it in
anyway would break. Gating all of `domain_items` — the shape the plan's Task 3
text and this note's first draft both took — would be a wholesale package
refusal D-7 does not authorise, since it would withhold every type's domain code
over one type's unbounded codec.

**The first draft's measurement was wrong, and its two named tests were never
the failures.** It said wiring `check_flatbuffers_bounds(package)?` into
`domain_items` breaks eight of this crate's own tests, naming
`recursive_struct_default_terminates`,
`a_cyclic_struct_takes_no_conditional_derives`, a cross-package case, and a
`Stream` case. Measured directly (`cargo test -p ridl-backend-rust --locked`
with the check wired in), the count is **five**, and **none** of the four named
tests is among them — those four pass, because the function's own cross-package,
cycle, and `Stream` guards already shield exactly the shapes they build. The
five that do fail — `struct_with_optional_and_reserved`,
`an_unspecified_field_primitive_takes_no_conditional_derives`,
`an_induced_tuple_struct_carries_its_derives`,
`a_tuple_under_an_internal_declaration_is_package_private`,
`leaf_recursion_denies_default_through_a_composite_field` — are unrelated
hand-built fixtures in `src/tests.rs` that carry a `string` with no `len_max`
incidentally, not on purpose: they predate this stage and were never written
with a FlatBuffers bound in mind. `cargo test -p ridlc --locked` passes in full
with the check wired in, so the corpus claim ("`ClimateReport` already depends
on generating across a cross-package reference") was also not what made the
wiring fail — it is true as a fact about the corpus, but it was not the failure
the wiring produced. Repairing those five fixtures' bounds is K5's, when it
wires the check in per type.

**The cross-package and cycle guards are per-member, not per-declaration.** The
first draft's guard answered for the whole declaration: if any member's type
reached an unresolved reference or a cycle, the _entire_ declaration was left
alone, silently, even when a different member of the same declaration was
genuinely unbounded — a struct with both a bare `string` map key and one foreign
field returned `Ok(())`, which is the silent omission ADR-0016 decision 6 and
ADR-0017 decision 4 forbid. The guard is now scoped to the one member being
examined: a member this backend cannot judge is exempted, and every other member
of the same declaration is still probed on its own — `unbounded_member` in
`crates/ridl-backend-rust/src/lib.rs`, pinned by two tests in `src/tests.rs`
over the two declaration orders (the exempt member first, and the exempt member
last).

**The refusal names the member, not only the declaration.** D-7's own text asks
for "the type and the member that is unbounded", and this needed no change to
`ridl_ir::projection::flatbuffers::max_size`'s API: a synthetic one-member
`v2::Decl` in the same package, handed to the existing public `max_size`,
charges exactly that member the way the real declaration's own computation would
— `struct_table_bound` sums each member's `field_charge` independently, and
`union_wrapper_bound` takes the largest of its arms' `union_arm_bound` — so
probing one member in isolation reproduces its share of the real bound with
nothing else able to answer `None` in its place (`probe_struct_field`,
`probe_union_arm`). Only the aggregate causes — the summed size overflows `u64`,
or exceeds `MAX_ENCODABLE` while every member is individually bounded — have no
single member to name, and the refusal falls back to the declaration alone for
exactly those
(`flatbuffers_bound_names_the_declaration_when_the_cause_is_aggregate`).

**Consequence for K5.** K5 calls `check_flatbuffers_bounds` once per type, as it
is about to emit that type's `Payload<FlatBuffers>` implementation, not once for
the whole package ahead of every other emit. It inherits the per-member
cross-package and cycle exemptions as built here, and repairs the five test
fixtures named above when it wires the check into the live pipeline.

**Two gaps carried forward from the 2026-09-20 review's second pass, for K5 to
close.**

1. **The exemption is per-member but still whole-member: an anonymous composite
   can still hide an unbounded leaf beside an unjudgeable one.** A struct field
   typed `map<veh.other.Speed, string>` — a cross-package (unjudgeable) key and
   a bare unbounded `string` value in the same map — returns `Ok(())`.
   `member_resolves_locally` answers `false` for the whole field the moment it
   reaches the unresolved key, so the member is exempted in full and the
   unbounded value inside the same map rides along unexamined;
   `unbounded_member` never gets to probe the value on its own, because probing
   happens per struct field or per union arm, not per leaf inside an array, a
   map, or a tuple. A _named_ local declaration does not have this hole: a local
   struct with one cross-package field and one unbounded field is still refused,
   because each is a separate member of the enclosing struct and each is probed
   independently — the gap is specific to an anonymous composite carrying both
   kinds of leaf inline. Verified directly (`check_flatbuffers_bounds` over the
   fixture above answers `Ok(())`). K5 closes this when it wires the check per
   type; until then, a struct or a union with an anonymous composite member is
   not fully covered by this refusal.
2. **`Attribution::Declaration` is reached for more than aggregate overflow.**
   Its own doc comment said the cause is aggregate — the summed size overflows
   `u64`, or the total exceeds `MAX_ENCODABLE` — but two other causes land on
   the same variant and the same declaration-only message, and neither is
   disambiguated: a member with no `r#type` at all (skipped by
   `unbounded_member` rather than attributed, the same as a reserved tombstone),
   and a `fb_projection::struct_table` layout error over the _whole_ declaration
   — two fields sharing one ordinal, for example — that only shows up across
   members and that no single-field probe can reproduce. The doc comment on
   `Attribution::Declaration` is corrected to say so; the message
   `check_flatbuffers_bounds` writes for this variant still does not distinguish
   the three causes, which is K5's to do if a reader needs to.

**A third gap, noted rather than tested.** A same-package cycle beside a
genuinely unbounded member (a cyclic field and a bare unbounded `string` field
in one struct) is not covered by any test in this module —
`flatbuffers_bound_leaves_a_cycle_alone_beside_a_bounded_member` pins a cycle
beside a _bounded_ member only. A probe confirms the untested path refuses
correctly, naming the unbounded field, but the coverage gap is real and is left
for K5 to close alongside the two above.

## 4b. Stage K5, 2026-09-21: the codec is emitted, and what this stage decided

Stage K5 (plan Task 4) emitted the view, `encode`, `verify`, `decode` and
`MAX_SIZE` into `generate`'s output, from a new module
`crates/ridl-backend-rust/src/codec.rs`, and wired D-7's refusal in per type. A
payload round-trips: `crates/ridl-backend-rust/tests/flatbuffers_roundtrip.rs`
compiles the generated codec as a program and runs it, over a fixture carrying
one declaration per shape the codec reaches. What follows is what this stage
decided that the note above did not already carry.

**The two gaps §4a carried forward are closed, and the third is covered.**

1. **An unjudgeable leaf is replaced by a one-byte stand-in, and the whole
   position is probed once.** The exemption was per member but whole-member, so
   `map<veh.other.Speed, string>` answered `Ok(())`: the unresolved key made the
   whole field unjudgeable and the unbounded value rode along. `judge` in
   `crates/ridl-backend-rust/src/lib.rs` now probes a position it can resolve in
   full as it is, and a position it cannot over `lower_bound_stand_in`'s copy of
   it: every leaf this backend cannot judge — a named reference that does not
   resolve in the package or reaches a cycle, a stream, an unspecified primitive
   — is replaced by a `boolean`, the smallest thing the projection charges
   anything for, at one inline byte and nothing out of line, and the copy is
   handed to the same `max_size` as one member. A `boolean` charges no more than
   any leaf it stands in for, and a vector's charge and a table's bound are both
   monotone in what they hold, so a copy that is unbounded proves the real
   position is, and a copy that fits proves nothing, which is what `Unjudgeable`
   means. `probe_struct_field` and `probe_union_arm` are one function again,
   `probe_field_type`, which writes ordinal 1 rather than copying the member's,
   so a malformed ordinal is attributed as a layout error instead of being
   mistaken for an unbounded member.

   **What the substitution charges, and what it does not.** It charges every
   count in the position, every product of nested counts, and every locally
   known leaf, together: `[[veh.other.Speed; 0..2^20]; 0..2^20]` is refused over
   the product of its two counts,
   `map<veh.other.Key, [boolean; 0..2^20];
   0..2^20>` over its entry count
   times its value's count, and `[(veh.other.Speed, [boolean; 0..2^31]); 0..4]`
   over four of a local inner array that fits alone. What it does not charge is
   anything at or below an unjudgeable leaf, and the leaf is the **whole named
   reference**, not only the foreign part of it: a local composite that reaches
   a foreign reference anywhere inside it is one leaf to
   `member_resolves_locally`, and the stand-in collapses all of it — its offset,
   its table, its vtable, its other members — to one byte.
   `[veh.other.Speed;
   0..2^31]` stays exempt because at one byte an element
   the count fits; and `[Mid; 0..2_000_000_000]` with a local
   `Mid { s: veh.other.Speed }` stays exempt too, although its real charge is
   roughly ten times the ceiling, because a local table worth about 26 bytes
   becomes one. That is not unsound — an exempt type carries no codec, so no
   wrong `MAX_SIZE` is published — but it is a wider blind spot than a foreign
   scalar's width, and K6, K7 and K8 should read it as such. The collapse stops
   at the reference: a local composite sitting beside one, rather than
   containing one, is charged in full, so `map<veh.other.Key, Big>` with a local
   `Big` over the ceiling is still refused. The stand-in can prove a position
   unbounded and never prove one bounded. The first two fix rounds of this stage
   got this wrong twice — the first by dropping a collection's count altogether,
   the second by probing each nesting level's count alone with the immediate
   element replaced, which charged neither a product of counts nor a count over
   a locally known element — and the review of 2026-09-21's second pass found
   all four shapes still exempt. Each is now a test in `src/tests.rs`, beside
   two controls: a nested collection whose counts multiply to something that
   fits, and the `[veh.other.Speed; 0..2^31]` limit above.

   **A member this backend cannot judge no longer shields the aggregate.** Two
   local `[boolean; 0..2^31]` members are each under the ceiling and refused
   together as `Aggregate`; with one foreign field beside them the struct
   answered `Ok(())`, because `unbounded_member` took any exempt member as the
   explanation for the declaration's `None`. It now charges the whole struct
   once more over the stand-in of each member before answering `Exempt`, and a
   `None` there is `Aggregate`, since the stand-ins are lower bounds. A union is
   its largest arm rather than a sum, so it has no aggregate to charge. This was
   pre-existing rather than introduced by a fix round, and it is fixed rather
   than recorded as a hole.
2. **`Attribution::Declaration` is split into three.** `Layout(String)` carries
   `fb_projection::struct_table`'s own message for a declaration-wide layout
   error — two members on one ordinal, or an ordinal of 0 — and is checked
   first, because with two members on one ordinal every member probes as bounded
   and only the aggregate answers `None`. `Untyped(String)` names a member with
   no `r#type` at all. `Aggregate` is what is left: every member bounded on its
   own, the total not. Each writes its own message.
3. **The untested path §4a noted is now tested.** A same-package cycle beside a
   genuinely unbounded member refuses over the unbounded one and names it
   (`flatbuffers_bound_names_the_unbounded_member_beside_a_cycle`).

Each of the three has a control beside it, so none of them would pass for an
attribution that simply stopped exempting: the same anonymous composite with
nothing unbounded beside the unjudgeable leaf is still exempt, and so is a
collection whose count fits at one byte an element.

**An `Unspecified` field primitive is exempted, not refused.** It emits `()` and
is charged nothing, exactly as a `Stream` is, and `derives` already lists the
two side by side among its refusing positions. It is malformed IR rather than an
unbounded shape, so `member_resolves_locally` answers `false` for it and the
type carries no codec.

**`check_flatbuffers_bounds` became `check_flatbuffers_bound`, per type.** D-7's
own wording is per type, and §4a's "Consequence for K5" says K5 calls it once
per type as it is about to emit that type's implementation. It is now called
exactly there, and `Ok(())` means one of two things the caller already knows
apart: the type has a bound, or its missing bound has a cause this backend
cannot judge and that one type carries no codec. The five hand-built fixtures
§4a named are repaired: four gave their `string`-backed named scalar typl §4.4's
default `[0..256]` bound, which the checker always materializes, and the fifth
is the `Unspecified` primitive above.

**What the round trip runs, and what is only compiled.** One round trip over
`Report` runs every scalar width, a string, bytes, an enum, an enum set, a
nested struct, a union's boxed arm, an induced tuple, a fixed array, a bounded
array of scalars, a map, a vector of strings, a vector of tables, a vector of
unions, a vector of tuples, a reserved tombstone, an optional composite, an
optional string and an optional scalar. A second case runs the union as a root
payload and decodes its **table** arm, which the `Report` round trip does not
reach, and runs a struct as a root payload. A third runs the D-9 case: every
field at the value a FlatBuffers writer would omit, with the optionals present
at those values.

The first pass of this stage claimed "one declaration per shape the codec
reaches", which the review of 2026-09-21 showed was overstated — an optional
composite, a vector of strings, of tables, of unions and of tuples, a union as a
payload and the union's table arm were none of them run, so
`Builder::push_offset_vector` was reached at run time only through map entry
tables. The fixture carries all of them now. What is still compile-only rather
than run: a tuple nested inside an array, a map or another tuple, and a
cross-package reference, which has no codec to run at all (driftsys/ridl#467).

**Four behavioural tests, each pinned by a mutation.** The review found three
defences that no test isolated — a present default-valued field, the union
discriminant, and the tightness of `MAX_SIZE` — and each now has a case that
fails alone when that one defence is broken, checked by applying the mutation
and running the suite:

- skipping a present optional whose value equals the FlatBuffers default fails
  only `a_present_default_valued_field_survives_the_round_trip`;
- accepting any discriminant in the union `verify`'s wildcard arm fails only
  `verify_refuses_a_union_discriminant_that_names_no_arm`, which writes one byte
  at the position the wrapper table puts its discriminant rather than flipping
  every byte in the buffer;
- `bound.saturating_sub(1)` fails only
  `max_size_is_pinned_and_holds_the_largest_legal_value`.

The third pins the two bounds as numbers, which an inequality cannot do: the
bound charges seven bytes of slack per vtable slot, so a bound short by one
still holds every value a fixture can build. The constant is wire-visible — a
consumer sizes a buffer from it — so the literal is what it deserves, and a
deliberate change to the projection's charges changes it in the same commit. The
literal is **pinned, not proven tight**: the test fails on a bound one lower and
on one higher, so the number cannot drift unnoticed, but nothing demonstrates
that a legal value reaches it. The same case encodes the largest `Inner` a
fixture admits, a sixteen-character label at four UTF-8 bytes a character, and
that value takes 96 bytes against a bound of 133; the largest `Report` is
encoded by no test, and a probe puts it at 1688 bytes against a bound of 2388.
The gap is the slack the bound charges for alignment and for the vtable, and it
is what keeps the bound sound under any write order; a tighter bound would be a
change to the projection's charges, not to this test.

**`MAX_SIZE` is a literal with a doc comment naming the rule.** Task 4 rules out
a const-evaluable expression over the field types. The constant is the number
`ridl_ir::projection::flatbuffers::max_size` returned, and the doc comment on it
names the rule that produced it — each table charged its `soffset`, its inline
fields, its vtable and one alignment event per slot; a string four bytes per
declared character plus a terminator; a collection its declared maximum — and
says why it is a literal: the slack the projection charges is not expressible in
Rust's type system.

**The inline layout is the codec's, not the projection's.** The projection owns
the slot ids, the union discriminant and the size bound, because two emitters
must agree on them. Which byte of a table a field starts at, and how large the
table is, are observable only by the codec: a `.fbs` schema states no offsets,
and no other emitter reads one. They are therefore computed in `codec.rs`, in
declaration order with each field aligned to its own width, which makes the
encoding deterministic (D-8). The bound stays sound whatever order is chosen,
because it charges `ALIGN_SLACK` once per vtable slot and once more for the
table's own `soffset`, which is the worst case any order can reach.
`crates/ridl-rt/src/flatbuffers.rs`'s module documentation said the offsets were
the projection's; it is corrected in place, as is the `ridl-rt` design record's
paragraph on the feature.

**The codec is emitted at the generated package's module scope, not in a
submodule.** Three free functions per table —
`__ridl_fb_{encode,verify,decode}_<snake>` — plus one view struct
`<T>FbView<'a>` per table. At module scope a same-package reference is spelled
exactly as `type_path` spells it everywhere else, and the functions can read a
generated type's private inner value the way any other item of that module can,
which is what lets `decode` build an enum set that publishes no constructor. The
`__ridl_fb_` prefix collides with no typl name: typl §15.1 gives a declaration a
CamelCase name and a constant a SCREAMING_SNAKE one.

**What the view offers, and what it decodes.** D-3 asks for one accessor per
field returning the field's own view. A scalar, a string, a byte sequence and a
nested table are read in place, which is the zero-copy property ADR-0020
decision 2 rests on. **A union and a collection decode on access**: a union's
arms have no one view type, and an indexable vector view would need one view
type per element shape and one per nesting level. That is this stage's scope
decision, not a rejection of the fuller form; a later stage can add an
element-wise view without changing the trait.

**`decode` is total and never panics.** Every read that could fail is discharged
with the neutral value of its own type — zero for a number, `false`, the empty
string, the empty collection, the first declared enum variant, the enum set with
no bit set, the first declared union arm — and `verify` is what makes those
branches unreachable. The alternative, `unreachable!`, was rejected: D-4 says
`decode` neither fails nor panics, and a panic in a consumer's build is worse
than a value no run can reach. A generated enum with no value and a union with
no arm have no neutral value at all, so the emitter refuses both with a
`GenerateError` rather than emitting a `decode` it cannot complete.

**What `verify` checked at this stage, and the hazard the rest left — closed
2026-09-21, stage K6; §4c records how.** The structural walk of D-5 in full,
plus two constraints: an enum and an enum-set discriminant, through the
`TryFrom` Epic 10 already emits, reported as `VerifyError::Contract`; and a
collection's declared element count, reported as `Rule::Length`. Both are what
keeps `decode`'s neutral discharge unreachable. A named scalar's own range,
length and pattern were **not** checked at K5: that was stage K6, which adds
`check` beside `new` (D-4) and calls it from `verify`.

Stated as what a hostile buffer produced at K5: a buffer carrying a `Label` of
four hundred characters, or a `Speed` of 9000, passed `verify`, and
`Ref::decode()` — a safe call, over a proof type whose whole purpose is to make
this unreachable — returned a value outside its declared typl bound. It was
memory-safe and contract-broken, and it was exactly the hazard D-4 names when it
rules out a `verify` that checks structure only. The window was narrow and
closed by design rather than left open: nothing in the tree consumed the codec
between K5 and K6, and K7 is what moves the face onto `Wire`. It was not
implicit either — the generated `verify` and `decode` each carried a doc comment
saying the typl constraints were not checked yet, and K6 removed both comments
with the same change that removed the hazard.

**One inconsistency in the attribution, noted rather than fixed.** A `FieldType`
whose `kind` is `None` probes to `Unbounded` and is attributed as an unbounded
member, while its neighbours among the malformed-IR cases are handled
differently: a member with no `r#type` at all is `Attribution::Untyped` and a
stream or an unspecified primitive is exempt. All three are malformed IR that no
typl source reaches, and the messages differ only in which of them a reader is
told about, so this is recorded rather than smoothed over.

**An optional marker outside a table field is refused.** A FlatBuffers vector
has no absent element and a map entry no absent half, so `T?` in one of those
positions is a `GenerateError` rather than a value silently written as present.
No typl source reaches it.

**`encode` allocates only where the domain type already does.** A vector of
strings or of tables needs each element's position before the vector can be
written, and there are as many positions as elements. Those are exactly the
shapes whose domain type is a `Vec` or a `String`, so a generated package over
types that own neither still encodes with no allocator, which is what D-12
claims. `Builder::push_offset_vector` is the one helper this needed, and K3's
module documentation had already said such a helper would arrive with the
emitter.

**The codec is `generate`'s output and not `generate_face`'s.** D-1 as amended
says the codec is `generate`'s output; it says nothing about the companion.
Adding it to `generate_face` as well would regenerate
`crates/ridl-backend-rust/tests/generated/interaction_face.rs`, which the plan
assigns to Task 6 (stage K7) and which is ordered against another lane's work.
So `generate_face` is unchanged here, and K7 adds the codec to it with the
`Wire` rebinding, in the stage that owns that file.
`the_pipeline_generate_stays_clean_of_the_face` is rewritten rather than
deleted: it used to assert that `generate` names no runtime path outside a
constructor, which the D-1 amendment made false, and it now asserts that every
runtime path `generate` names belongs to the constructors or the codec and that
none belongs to the face.

**The emitted manifest names the feature.** `crates/ridlc/src/lib.rs` now
renders `ridl-rt = { version = "0.1", features = ["flatbuffers"] }`, because
`generate`'s output calls the helpers that feature gates. This makes the
generated manifest unbuildable outside this repository until a `ridl-rt` release
carries the feature's contents — the release coupling D-12 records — and nothing
here can test it, because every proof links `ridl-rt`'s source rather than a
release. E11.14 owns the manifest.

**What a cross-package reference gets: no codec, and a note saying why
(driftsys/ridl#467).** `ridl-backend-rust` resolves no cross-package reference,
so it can neither size nor encode a type that reaches one — it cannot even learn
a foreign named scalar's width. Such a type is exempt, not refused.

**This is a silent omission in the sense ADR-0016 decision 6 and ADR-0017
decision 4 rule out, and it is the majority of the corpus.** Ten types are
withheld across the corpora, nine of them in veh-cluster — `DriverProfile`,
`SensorBounds`, `SensorResult`, `SensorReading`, `SensorFault`, `DiagFilter`,
`FaultEvent`, `FaultPage` and `ClimateReport`, every type touching the prelude
or an import — and the tenth is workspace-two-members' `ClusterReading`. Five
types in the veh-cluster corpus get a codec: `SpeedLimitPayload` and
`RawWheelFrame` in `veh.common`, and `DoorPayload`, `RawWheelSpan` and
`FilterState` in `veh.cluster`. The deferral is deliberate, but a doctrine
deviation this wide cannot be tracked only in a note that archives at the end of
the lane, so it has its own issue, **driftsys/ridl#467**, linked from
driftsys/ridl#263. The fix it states: `generate` handed the other packages,
which `fb_projection::Packages` already takes as `others`, in E11.14 or in the
plugin system's `CodegenRequest`. It binds E11.8 and E11.12 the moment either
emits a codec.

Until then the emitted source says so where it happens. Each withheld type gets
a `const __RIDL_FB_NO_CODEC_<NAME>: () = ()` carrying a doc comment that names
the type, the member that could not be judged, the reason, and the issue, so a
consumer meets a reason rather than an unsatisfied trait bound in their own
crate far from the cause.

## 4c. Stage K6, 2026-09-21: `check` beside `new`, and what it closes

Stage K6 (plan Task 5) added the constraint check D-4 asks Epic 10 for, called
it from `verify`, and removed the hazard §4b and D-5's amendment recorded. What
follows is what this stage decided that the note above did not already carry.

**`check` is not public — Sebastien's decision, not this stage's.** Every named
scalar's `impl` block gains `fn check(value: T) -> Result<(), Violation>` beside
`new`, `new_unchecked` and the getter, with no `pub` and no `#vis`: it carries
no visibility modifier regardless of the type's own declared visibility.
`verify` (`crates/ridl-backend-rust/src/codec.rs`) is the one caller outside
`new`, and it is emitted into the same generated module as the domain types
(design note, "The codec is emitted at the generated package's module scope, not
in a submodule"), so a private function is visible to it the way any other item
of that module is. Open item 2 of §4, whether `check` is public in the generated
package, stays open — this closes only whether the codec needs it public, and it
does not.

**`new` becomes the composition of `check` and `new_unchecked`, exactly as D-4
states,** rather than keeping its own inline checks beside a duplicate copy in
`check`. `new`'s signature and its externally observable behaviour (which values
it accepts, which `Violation` it returns, in the same order) are unchanged; only
its body is now `Self::check(&value)?; Ok(Self::new_unchecked(value))`. This is
why the constrained-scalar snapshot and unit tests written before this stage
needed no assertions changed beyond the snapshot regeneration itself — `new`'s
contract held, and the range/length/pattern check text simply moved into the new
function.

**`check`'s parameter type is the natural borrow, not always `&Inner`.** A
`float`/`integer`/`boolean` backing takes `&f64`/`&i64`/`&bool`; a `string`
backing takes `&str` and a `bytes` backing takes `&[u8]`, not `&String` or
`&Vec<u8>`. This is what lets the codec's `verify` call `check` directly on the
bytes it already has — a FlatBuffers-backed `&str` or `&[u8]` slice, read in
place — with no allocation, which is what Task 2 (K3) promised of `verify`. A
`let value = *value;` reborrow at the top of `check` is emitted only for the
three `Copy` backings, so the existing range/length/pattern check bodies
(`constraint_checks`) are reused unchanged for every backing rather than
duplicated for a reference form.

**What `verify` now checks, closing the gap §4b and D-5's amendment named.** The
generated `verify` walks a named scalar's own range, length and pattern over a
borrow, through `check`, beside the structural walk, the enum and enum-set
discriminant, and the collection element count K5 already checked. This applies
at every position a named scalar can occupy on the wire: an inline scalar field,
a `string`-backed field's bytes, and a `bytes`-backed field's bytes — the last
two read the buffer's verified slice directly and pass it to `check` with no
copy. A **vacuous** named scalar (`ctor ==
"new"`, `crate::NamedScalar::ctor`)
emits no `check` at all (`emit_vacuous_type_def`), so `verify` calls nothing for
one and the structural read is unchanged for that position, which is correct: a
vacuous type has no constraint for `check` to hold. `decode` is unchanged by
this stage — it already built a named scalar with its unchecked constructor over
bytes `verify` accepted; what changed is that `verify` now accepts fewer
buffers, so that constructor is now always called over a value inside the type's
typl bound. The generated `verify` and `decode` doc comments that announced the
hazard are removed with this change, as the driver instructed.

**`crates/ridl-backend-rust/tests/flatbuffers_roundtrip.rs` gained
`verify_refuses_a_named_scalar_outside_its_declared_bound`**, which builds a
`Report` with `Speed::new_unchecked(9000)` (outside `[0..300]`) and a
`Label::new_unchecked("x".repeat(20))` (outside `[0..16]`), encodes each with
`encode` (which still never rechecks a constraint), and asserts `verify` refuses
both with `VerifyError::Contract` naming the right type and rule; a value within
every bound still verifies, so the assertion is not vacuous. Removing the
`check` call from `verify` fails only this test among the crate's full suite —
checked by disabling it and re-running. The same edit also widened
`verify_refuses_an_undeclared_discriminant`'s byte-flip fuzz test to accept
`Rule::Range` and `Rule::Pattern` beside `Rule::Variant` and `Rule::Length`: a
random byte flip can now land inside a named scalar's own inline bytes and read
as a value outside its typl bound, which is a real `Contract` outcome the fuzz
loop must not treat as a failure.

**`crates/ridl-backend-rust/src/tests.rs` gained
`step_only_scalar_is_vacuous_and_still_names_the_gap`**, pinning
driftsys/ridl#463 finding (a): `constraint_is_vacuous` excludes `step`
(`crates/ridl-ir/src/lib.rs`), so a `step`-only constraint takes the infallible
`emit_vacuous_type_def` path while `unchecked_doc` still emits its quantization
note on the type. Adding `step` to the vacuity check — the mutation the issue
named — fails this test alone.

**#463 findings (b) and (c) are fixed in the same file this stage already edits,
not filed onward.** (b): `emit_vacuous_type_def`'s doc claimed `From<Inner>` was
correct "because there is no invariant to bypass", which is false of a
`step`-only constraint reaching that same function; the doc now states the true
reason — `new` checks nothing on this path, for every input that reaches it,
`step` included. (c):
`same_package_scalar_ctor(ctx, type_ref).unwrap_or_else(|| quote! { new })` in
`emit_const` was unreachable, since `same_package_scalar_backing` and
`same_package_scalar_ctor` resolve the same declaration through the same lookup;
the fallback also named the wrong constructor for a constrained type (`new`
returns `Result`, which does not type-check in a `const` item). It is replaced
with `.expect(...)`, which documents the invariant instead of papering over it
with a wrong default.

## 5. Records this changes, if the disposition takes it

None of these moves in the note's own pull request.

| Record                                                                          | What changes                                             | Decision |
| ------------------------------------------------------------------------------- | -------------------------------------------------------- | -------- |
| ADR-0021 decision 7, `crates/ridl-rt/src/payload.rs`, the ridl-rt design record | `Encoded.bytes` is a subslice, not a prefix              | D-2      |
| ADR-0021 decision 8, the ridl-rt design record                                  | the `flatbuffers` feature is no longer empty             | D-12     |
| ADR-0023, a new decision                                                        | the codec entry point beside `generate_face`             | D-1      |
| the interaction-face design record                                              | the `Payload<ReprC>` placeholder row is retired; `Wire`  | D-11     |
| the typl value-objects design and plan                                          | `check` beside `new`                                     | D-4      |
| `2026-09-13-catalog-descriptor-plan.md` Task 7                                  | calls the bound of D-6 instead of deriving one           | D-6      |
| ADR-0019, or a new projection record                                            | nothing, if the disposition takes D-1 to D-10 as written | —        |

## 6. Execution

Proposed stages, one pull request each, written up task by task in the plan that
follows the disposition:

1. **The projection facts and the bound** in `ridl-backend-flatbuffers`: the
   shared layout, the discriminant, the size function, and the drift test of D-1
   and D-10.
2. **The `ridl-rt` helpers** under the `flatbuffers` feature, plus the
   `Encoded.bytes` amendment of D-2. Additive except for that wording.
3. **Epic 10's `check`** (D-4) — in Epic 10's own pull request if that epic is
   still in flight when this runs, in this stage if it is not.
4. **The emitter**: views, `encode`, `verify`, `decode`, `MAX_SIZE`, and the
   refusal of D-7.
5. **The face on `Wire`** (D-11), deleting the hand-written implementations.
6. **Conformance and determinism** (D-8), and the records of §5.

## 7. Trace

- Roadmap: `docs/ROADMAP.md` — E11.7
- Binds nothing on its own; it proposes.
- Reads: [ADR-0019](../decisions/ADR-0019-flatbuffers-projection-rules.md),
  [ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
  decisions 2, 5, 6, 7 and 8,
  [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decisions 7
  and 8, [ADR-0023](../decisions/ADR-0023-interaction-face-generation.md)
  decision 2
- Design records: [`../design/ridl-rt.md`](../design/ridl-rt.md),
  [`../design/interaction-face.md`](../design/interaction-face.md)
- Adjacent plans:
  [`2026-09-13-catalog-descriptor-plan.md`](2026-09-13-catalog-descriptor-plan.md),
  [`typl-value-objects-plan.md`](typl-value-objects-plan.md)
- Defect: driftsys/ridl#302
