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
the member that is unbounded.

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
  make `ridl build` emit the codec — that is the row K0 adds as E11.14 — and it
  does not remove `#[repr(C)]` from the domain structs, which ADR-0020 decision
  3 gives to E11.12.

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
