# The FlatBuffers payload codec — implementation plan

**Status:** plan, 2026-09-20. It implements
[`2026-09-20-flatbuffers-codec-design.md`](2026-09-20-flatbuffers-codec-design.md)
as that note's pull request disposed of it: D-2, D-3, D-5, D-8, D-9 and D-10 as
proposed, D-4, D-7, D-11 and D-12 with an addition each, and D-1 and D-6 amended
together. Where this plan and the note disagree, the note has been amended in
place and both now say the same thing; where this plan and the disposition
comment disagree, the disposition wins and this plan is wrong.

**Story:** roadmap E11.7, the FlatBuffers payload codec. **Done when** a payload
round-trips through the library.

## The two things to read first

**The shared facts live outside any backend.** D-1 and D-6, as amended, put a
type's table layout, its field slots, a union arm's discriminant and its size
bound in `ridl-ir` or a small projection crate — never in
`ridl-backend-flatbuffers`. `ridl-backend-rust` and, later, `ridl-descriptor`
read them from there. A dependency of one backend on another would make a
backend a library other crates link, which is the opposite of what ADR-0020
decision 9 makes a backend in step 2. Task 1 chooses between the two homes and
states the ground for the choice.

**The codec is `generate`'s output.** D-1, as amended, puts the
`Payload<FlatBuffers>` implementations in `generate`'s own output rather than
behind a third entry point beside `generate` and `generate_face`. A consumer of
a generated package needs the codec whether or not it ever dispatches, and
`generate` already names `ridl-rt`. What made a separate entry point look
necessary was the proof mechanism, and Task 2 settles that instead.

## Ordering, and what this plan waits on

**Epic 10 Task 4 has no session.** The codec's code stages depend on the
generated value-object surface that Task 4 settles, so Tasks 4 to 7 below cannot
start until it does. Tasks 1 to 3 do not touch generated domain types and can
run now. This is the plan's one external blocker and it is not inside lane K's
control; if Task 4 is still unstarted when Task 3 lands, say so rather than
starting Task 4 of this plan against a surface that is about to change.

**Epic 10 Task 6 is driftsys/ridl#443**, and it was told about D-4's `check`
while still open. If it carries `check`, Task 5 below uses it; if it declines,
Task 5 adds it. Either way Task 5 does not block on the answer, because it can
see which happened by then.

## Task 1 — the shared projection facts and the size bound

**Implements:** D-1 (amended), D-6 (amended), D-10, D-7's bound half.

Choose the home first and record the choice in the task's pull request:
`ridl-ir`, or a new `ridl-projection` crate. `ridl-ir` is the lighter change and
is already a dependency of every crate that needs the facts; a new crate is
cleaner if the facts grow past what an IR crate should carry. Whichever is
chosen, no backend may end up a dependency of another crate.

Export, as functions over the IR:

- a type's table layout and its field slots, as ADR-0019 fixes them;
- a union arm's discriminant, which is `UnionArm.ordinal` (D-10);
- `fn max_size(...) -> Option<u64>`, `None` meaning no finite bound (D-6).

Move `ridl-backend-flatbuffers`'s existing derivation of these onto the shared
functions rather than leaving a second copy. **The drift test of D-1 and D-10
lands here**: the `.fbs` schema the FlatBuffers backend emits and the facts this
module exports must agree on every corpus fixture, and driftsys/ridl#302 becomes
a failing drift test rather than a silent wire break — without closing it.

**Done when:** the facts have one implementation, `ridl-backend-flatbuffers`
calls it, the drift test passes on every fixture and fails when either side is
perturbed, and no crate depends on a backend.

## Task 2 — the `ridl-rt` helpers, and the proof mechanism

**Implements:** D-12's first, fourth and fifth bullets; D-2's `Encoded.bytes`
amendment.

The `flatbuffers` feature stops being empty. It gates the shared reading and
writing helpers the emitted code calls: the byte-order reads of D-5, the tail
builder of D-2, the vtable walk. The crate still takes **no external
dependency** — the FlatBuffers runtime crate ADR-0020 decision 5 permits stays
unused, so RA-01's ceiling stays unspent.

Three constraints, all verified against the tree rather than assumed:

1. **The helpers must build at `rust-version = "1.83"`.** `just compat-check`
   runs `cargo +1.83 test --all-features` against the packaged crate, so a
   helper that needs a newer language feature fails the gate, not merely the
   pin.
2. **Every rustc proof compiles `ridl-rt` bare, with no feature.** A proof over
   `Payload<FlatBuffers>` output therefore needs `--cfg feature="flatbuffers"`
   or a cargo build. Settle which, here, before any emitter code exists — Task 4
   depends on it and D-1's amendment rests on it.
3. **`verify` allocates nothing**, and `decode` allocates only where the domain
   type owns a `String` or a `Vec` (D-12).

`Encoded.bytes` becomes a subslice rather than a prefix (D-2). That is a wording
change to ADR-0021 decision 7, `crates/ridl-rt/src/payload.rs` and the `ridl-rt`
design record; it is the one part of this task that is not additive, and
ADR-0021 decision 10 makes it a 0.x minor, not a break to avoid.

**Done when:** the helpers exist under the feature, `just compat-check` passes,
`just wasm-check` passes, the proof mechanism is chosen and demonstrated on one
throwaway example, and `Encoded.bytes`'s three records agree.

## Task 3 — `MAX_SIZE`, and the refusal of D-7

**Implements:** D-7 (with its addition).

The emitter writes `max_size` of Task 1 into the generated
`const MAX_SIZE: usize` as a literal with a comment naming the rule that
produced it, never a const-evaluable expression over the field types. The face's
`MAX_BUFFER_SIZE` stays the const-evaluable maximum over those literals.

When the bound is `None`, the emitter writes no `Payload<FlatBuffers>`
implementation for that type and returns a `GenerateError` naming the type and
the unbounded member.

**This diagnostic is totality over the IR, not a case a user will meet.** typl
makes every array and map bound mandatory, defaults a `string` and a `bytes` to
`[0..256]` when unspecified (TYPL-103, a warning), and rejects a recursive
composite reference, direct or transitive (TYPL-206, an error). So it must exist
and be tested — construct the IR directly in the test rather than trying to
write typl source that produces it — and it does not need to be loud.

**Done when:** the constant is emitted for every corpus fixture, the refusal is
covered by a test over a hand-built IR, and no test tries to reach it from typl
source.

## Task 4 — the view, `encode`, `verify`, `decode`

**Implements:** D-2, D-3, D-5, D-9. **Waits on Epic 10 Task 4.**

The emitter writes, into `generate`'s output, per payload type:

- `View<'a>`, a generated zero-copy accessor over verified bytes (D-3);
- `encode`, building at the tail into the caller's slice, with `Encoded.bytes`
  the subslice actually written (D-2);
- `verify`, which checks structure and then the typl constraints over a borrowed
  value (D-4, D-5), so `decode` stays infallible;
- `decode`, infallible over verified bytes.

The `Malformed` mapping is D-5's, and `Unaligned`, `TooDeep` and `TooManyTables`
are never produced by generated code — assert that, rather than leaving it as
prose. A present default-valued field is written explicitly (D-9).

**Done when:** a payload round-trips — the story's own `Done when` — for every
corpus fixture, under the proof mechanism Task 2 chose.

## Task 5 — the constraint check beside `new`

**Implements:** D-4. **Coordinates with driftsys/ridl#443.**

`verify` needs to run a type's constraint check against a value the caller
already holds, without moving it and without rebuilding it through `new`. If
Epic 10 Task 6 landed `check`, use it. If it declined, add it here, to the same
design the value-objects records describe.

**Whether `check` is public in the generated package is Epic 10's call**, not
this plan's — open item 2 of the note, deliberately left open. The codec does
not need it public.

**Done when:** `verify` rejects a value that breaks its typl constraints, with
the violation the `Malformed` mapping of D-5 names, and the check runs over a
borrow.

## Task 6 — the face on `Wire`, and driftsys/ridl#448

**Implements:** D-11 (with its addition). **Waits on Task 4.**

The generated package gains `pub type Wire = ::ridl_rt::encoding::FlatBuffers;`
from a backend option defaulting to `FlatBuffers`. Every place the face names
`ReprC` — `MAX_BUFFER_SIZE`, `EVENT_SOURCE_BUFFER_SIZE`, each `Ref::encode` and
`Ref::verify`, each `unreachable!` message — names `Wire`. The face gains no
type parameter. The fixture's hand-written `Payload<ReprC>` implementations are
deleted and the generated codec takes their place, retiring the first row of the
interaction-face record's "What is provisional" table.

**This task settles driftsys/ridl#448.** The `Wire` rebinding is the one place
every generated constructor is rewritten, so it either emits the catalog check
ADR-0021 decision 3 describes, or records in the same change that the check
waits for E16.2 and amends the two sentences #448 quotes. Today ADR-0021
decision 3 and ADR-0023 decision 5 both promise a check no constructor performs:
no `.catalog()` call exists under `crates/ridl-backend-rust/`, and decision 5's
"still performed once in the constructor" describes a check that was never
emitted. Emitting it needs one decision #448 names — what `new` does on a
mismatch — and that is an ADR-0023 amendment. **Touching every constructor and
leaving the promise false is ruled out.**

**Done when:** the face names `Wire` throughout, the hand-written
implementations are gone, the round-trip tests pass over the generated codec,
and #448 is closed by a check or by an amendment naming E16.2.

## Task 7 — conformance, determinism, and the records

**Implements:** D-8, and §5 of the note.

Conformance is a round trip through `planus`, which is already this repository's
FlatBuffers oracle, rather than byte equality against it (D-8). The encoder is
deterministic: the same value encodes to the same bytes on every run and every
target, with field write order, vtable layout and vtable sharing fixed by the
emitter and a map's entries in the order of the generated `Vec<(K, V)>`.

The fixture package joins `just wasm-check` (D-12), because ADR-0020 decision 2
makes the generated Rust compiled to `wasm32` the codec a TypeScript consumer
loads.

Then the records of §5 move, each with the stage that needed it — most are
already carried by Tasks 2, 5 and 6; what remains here is the catalog descriptor
plan's Task 7, which is amended to call the bound of D-6 rather than derive a
second implementation of it.

**Done when:** the conformance suite passes, determinism is asserted rather than
described, `just wasm-check` covers the fixture, and no record of §5 still
describes the old shape.

## Out of scope

E11.7 emits no frame (E11.1), no transport (E11.9), no proto3 (E11.8), no
`repr(C)` layout (E11.12) and no TypeScript. It does not make `ridl build` emit
the codec — that is E11.14 — and it does not remove `#[repr(C)]` from the domain
structs, which ADR-0020 decision 3 gives to E11.12.

## Trace

- Roadmap: `docs/ROADMAP.md` — E11.7
- Design note:
  [`2026-09-20-flatbuffers-codec-design.md`](2026-09-20-flatbuffers-codec-design.md),
  disposed of 2026-09-20
- Reads: [ADR-0019](../decisions/ADR-0019-flatbuffers-projection-rules.md),
  [ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
  decisions 2, 5, 6, 7, 8 and 9,
  [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decisions 3,
  7, 8 and 10, [ADR-0023](../decisions/ADR-0023-interaction-face-generation.md)
  decisions 2 and 5
- Blocked by: Epic 10 Task 4, which has no session
- Coordinates with: driftsys/ridl#443 (Epic 10 Task 6)
- Settles: driftsys/ridl#448
- Makes visible: driftsys/ridl#302
