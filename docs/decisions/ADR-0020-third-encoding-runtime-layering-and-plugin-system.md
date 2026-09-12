# ADR-0020 — The third payload encoding, the runtime layering, and the codegen plugin system

## Status

Proposed — 2026-09-12. Scope: three subjects the 2026-09-12 re-scope settled
that no earlier record covers — a third payload encoding beside proto3 and
FlatBuffers, how the runtime material is layered in each language, and the
contract an out-of-tree codegen backend implements. It is not epic-scoped: it
binds every backend this workspace or the ecosystem grows and the runtime
material in every language, in the way
[ADR-0018](ADR-0018-runtime-core-and-generated-surface.md) binds what a backend
emits and [ADR-0016](ADR-0016-schema-projection-and-the-name-transform.md) binds
how identity projects.

It **amends [ADR-0018](ADR-0018-runtime-core-and-generated-surface.md) decision
3** — two encodings become three, and the codec-in-wasm boundary is settled
(decisions 1 and 2) — and **extends its decision 6's layering** into named
crates and packages per language (decisions 5 to 7). It **restores the plugin
protocol to the critical path** (decisions 8 to 12), which the
roadmap-simplification note's S-27 had proposed and the earlier sequencing had
placed behind a browser playground. The in-place amendments this record causes
are listed in **Documents amended**; each is written into the record it belongs
to, not here.

Its reasoning trail is
[`docs/wip/2026-09-12-release-scope-and-plugin-system-design.md`](../wip/2026-09-12-release-scope-and-plugin-system-design.md),
whose §3.3, §3.4, §3.5 and §3.8 carry the alternatives this record summarises,
and
[`docs/wip/2026-09-08-ridl-rt-design.md`](../wip/2026-09-08-ridl-rt-design.md),
which is the library decisions 5 and 6 place.

## Context

Three problems came out of one session, and each has the same shape: a record
that fixed a number or a name before there was a case to fix it against.

**Two encodings do not cover every consumer.** ADR-0018 decision 3 fixed proto3
for a stream and FlatBuffers for a mapped or within-node path, and its decision
6 dropped C as a language target because the header that backend produced was
the whole output — layout only, omitting every type carrying a string, an
optional or a collection, omitting every interface, with no `extern "C"`
functions to match it. Both findings stand. What neither covers is a consumer
that reads a struct at a fixed offset and links no reader library: a legacy C
program, and a Rust consumer that wants a payload encoding with no FlatBuffers
dependency. That consumer needs an encoding, not a language backend.

**The runtime material had no layering.** ADR-0018 decision 16 made the runtime
an epic and the ridl-rt design note's §0 moved the engine out of this repository
and took the name `ridl-rt` for the library that remains. The note left three
questions unanswered: whether the port traits are a crate of their own, where a
transport lives, and what the TypeScript side is made of. Until those are
answered, "generated Rust links `ridl-rt`" does not say what a consumer takes on
when it links it, and two people writing two emulators would share no transport.

**Each backend re-derives the same semantics from the raw IR.**
`crates/ridl-backend-ts/src/lib.rs` records in its module header that its init
derivation mirrors the Rust backend's `Default` derivation, and its
init-function derivation records the same for the leaf-recursion rule of typl
§5.8, naming `crates/ridl-backend-rust/src/defaults.rs`. The name transform, the
width derivation, the tombstone handling and the width-to-`bigint` decisions are
duplicated the same way. A third language written against the raw IR implements
all of it a third time, and the plugin protocol as previously sequenced —
roadmap E4.5, behind a browser playground — would have shipped that duplication
as its public contract.

## Decision

1. **A third payload encoding: `repr(C)`.** The encoding is three artifacts from
   one IR: a generated `#[repr(C)]` layout struct per type the encoding can
   carry, a C header describing the same layout, and a generated codec between
   the layout struct and the domain type. It serves two uses — a wire payload
   where FlatBuffers is not wanted, and a C-readable layout for interoperation
   with a legacy consumer.

   **This is not C restored as a language target.** ADR-0018 decision 6 rejected
   that because the header was the entire output and its omissions were silent.
   Here the header is one artifact of a payload encoding whose Rust side carries
   the codec, the constraint checks and the domain types. And the encoding is
   **total**, which is what makes that difference checkable rather than a matter
   of intent: it is total over shape by
   [ADR-0016](ADR-0016-schema-projection-and-the-name-transform.md) decision 6
   and total over names by [ADR-0017](ADR-0017-proto3-projection-rules.md)
   decision 4, as every projection before it is. So a typl construct the
   encoding cannot carry is a diagnostic naming it, never a declaration left out
   of the header.

   This amends ADR-0018 decision 3's "two encodings and no more". It does not
   reopen that record's deferred `repr(C)` **store**: the store layout stays
   deferred on its own reasoning — verification is boundary-scoped,
   build-then-flip survives a writer crash better than a seqlock, and a vtable
   absorbs the width flips of typl §17.11 — and this decision is about a
   payload.

2. **The encoding matrix, with the codec-in-wasm boundary settled as
   FlatBuffers.** ADR-0018 decision 3's rule is kept and one classification
   under it is corrected.

   | Boundary                                                                                | Encoding    | Why                                                                                                             |
   | --------------------------------------------------------------------------------------- | ----------- | --------------------------------------------------------------------------------------------------------------- |
   | Serialized into a stream — CAN, SOME/IP, DDS, a WebSocket                               | proto3      | compact and schema-evolvable at once, so version skew between independently updated peers is survivable         |
   | Mapped or passed within a node — a shared-memory store or queue, a local socket, Binder | FlatBuffers | the receiver is handed a buffer it reads in place, with one copy or none                                        |
   | A wasm codec and the host it was generated with                                         | FlatBuffers | both sides come from one IR at one time, so there is no skew to survive, and the host reads the buffer in place |
   | A wasm guest that updates independently of its host                                     | proto3      | decision 3's original case: skew is possible, so the encoding has to survive it                                 |
   | A C reader, or any consumer that links no reader library                                | `repr(C)`   | a field at a fixed offset needs no reader library at all                                                        |

   ADR-0018 decision 3 put proto3 on "the wasm guest boundary" without dividing
   the two wasm cases, and its stated reason was that decision 14 makes the
   guest updatable independently of the host. That reason holds for a behaviour
   guest — an rmdl component compiled to wasm — and does not hold for the codec
   of decision 7, which `ridlc` generates from the same IR as the TypeScript
   that calls it and which ships with it. Under decision 3's own rule that
   boundary is within a node, so it carries FlatBuffers, and the consequence is
   that a TypeScript consumer reads the buffer in place through generated
   accessors and no materialized object crosses the wasm boundary on a read.

   This closes the open item the roadmap carries against ADR-0018 decision 3 and
   answers the first bullet of the design note's §4.

3. **The domain types do not carry `#[repr(C)]`.** They stay the value objects
   of roadmap epic E10 — a private field and a checked constructor. A scalar
   newtype may additionally carry `#[repr(transparent)]`, which constrains its
   content not at all. The layout struct of decision 1 is a separate generated
   type, and the generated codec is the only thing that converts between the
   two.

   Putting `#[repr(C)]` on the domain types themselves would confine every
   domain type to the C-representable subset — no `String`, no `Vec`, no
   `Option<T>`, no enum carrying data — charge that representation to every Rust
   consumer including those that speak only proto3, and make a typl §17.11 width
   change move every field after it in every consumer's struct.

   **This changes shipped behaviour.** ADR-0007 decision 13 took that path, and
   the Rust backend still emits `#[repr(C)]` on every struct whose IR
   `fixed_layout` flag holds. ADR-0018 decision 6 retired that decision's C
   header and its extern-C face and left the attribute in place, so the
   attribute outlived its only stated purpose. This decision retires it too:
   ADR-0007 decision 13 carries the amendment, and the removal lands with
   roadmap story E11.12, the backend that replaces it.

4. **The `repr(C)` projection rules belong in a projection record, written when
   the backend is.** The rules — the fixed-capacity layout of a bounded string,
   an optional and a collection; the string capacity and whether a terminator is
   guaranteed for a C reader; alignment; endianness — are target-specific in
   exactly the way [ADR-0017](ADR-0017-proto3-projection-rules.md)'s and
   [ADR-0019](ADR-0019-flatbuffers-projection-rules.md)'s are, so they take that
   record's shape and are not in this one and not in the ADR-0018 amendments.
   This record fixes the encoding's place in the matrix; that record fixes what
   the bytes are.

5. **One `ridl-rt` crate, one cargo feature per encoding.** `ridl-rt` is a
   single `no_std` crate holding the six modules the ridl-rt design note's §1.1
   lists — `contract`, `sample`, `payload`, `port`, `strata`, `encoding` — with
   one feature per encoding: `flatbuffers`, `proto3`, `repr-c`. With default
   features off the crate has no dependency at all, which makes that note's
   RA-01 read as a ceiling rather than a floor: the FlatBuffers runtime is its
   only permitted dependency and is reached only under the `flatbuffers`
   feature. The crate builds natively and for `wasm32`, and joins the crates
   `just wasm-check` covers.

   Splitting the port traits into a crate beneath it (`ridl-abi`, the
   roadmap-simplification note's S-03) was rejected: the ports are not definable
   without `Sample<T>`, `Envelope` and `Provenance`, so the two crates would
   always ship together at matching versions, and the one thing the split would
   buy — an engine author depending on the traits without the codec machinery —
   the features already buy. That note's S-03 becomes roadmap story E11.0,
   `ridl-rt`, and the archived `ridl-abi` note is the first draft of this
   design.

6. **The runtimes live outside `ridl-rt`.** The note's RA-03 fixes the
   dependency graph as emitter output → `ridl-rt` ← runtime and nothing else, so
   each runtime is its own crate or package.

   | Crate or package                           | Holds                                                                              | Depends on                                      |
   | ------------------------------------------ | ---------------------------------------------------------------------------------- | ----------------------------------------------- |
   | `ridl-rt`                                  | the library of decision 5                                                          | the FlatBuffers runtime, only under its feature |
   | `ridl-loopback`                            | the in-process reference runtime — every port over a queue and a map, and no IO    | `ridl-rt`                                       |
   | `ridl-transport-ws`                        | the ports over a WebSocket, proto3-framed; the default of the getting-started path | `ridl-rt` and a WebSocket crate                 |
   | the TypeScript runtime package (name open) | the `ridl-rt` wasm module plus the port interfaces spelled in TypeScript           | the wasm artifact                               |
   | the TypeScript WebSocket package           | the same binding for Deno and the browser                                          | the runtime package                             |

   Generated Rust links `ridl-rt`, generated TypeScript imports the runtime
   package, and neither ever names a transport.

7. **TypeScript is the same three layers, and the WebSocket transport is
   mandatory nowhere.** The runtime package carries the wasm codec and the port
   interfaces; the generated faces sit on it; the transport is a separate
   package. The WebSocket transport is what the getting-started path uses and
   what a remote server or an emulator links by default, and nothing links it
   unless it asks. This confirms ADR-0018 decision 6 for TypeScript — generated
   types and faces, the codec reached through wasm — and settles the encoding at
   that boundary by decision 2.

   Shipping faces and a frame with no transport was rejected because two
   emulators written by two people would then share no transport and the
   TypeScript backend would have no end-to-end test in this repository; shipping
   faces with no frame specification was rejected because two hand-written
   transports would not interoperate.

8. **Lower once, in the compiler.** A codegen model sits between the IR and
   every backend, in-tree or plugin: names already transformed by ADR-0016's
   pinned rules, widths already derived, init values already resolved,
   descriptors already tabulated, and ordinals and constraint tables already
   flat. Every backend consumes that model, and a backend is then mostly a
   printer.

   Plugins over the raw IR with no lowering step were rejected because every
   plugin re-derives the semantics the Context section lists. Template packs —
   the IR through a template engine — were rejected for faces and codecs, where
   each existing backend carries on the order of a thousand lines of logic a
   template language cannot type-check; a template pack stays a reasonable way
   to write a types-only printer over the lowered model.

9. **One backend contract: `generate(CodegenRequest) -> CodegenResponse`.** The
   request carries the lowered model in ADR-0014's canonical encoding plus the
   backend options. The response carries files as path-and-bytes pairs, plus
   diagnostics. **The plugin never touches the filesystem**: `ridlc` writes the
   files, so `--out`, dry-run and overwrite behaviour are identical for every
   backend. A non-zero exit or a malformed response is a `ridlc` error that
   names the plugin.

10. **Two hosts for that contract; the process host is this release's.** The
    in-tree Rust and TypeScript backends implement the contract in-process. The
    process host implements it by running an executable named
    `ridlc-gen-<language>` found on `PATH` or given with a flag, writing the
    request to its stdin and reading the response from its stdout, so a plugin
    is any executable in any language — a Rust binary, a JVM launcher script, a
    Deno script, a Python file.

    **The wasm host is a second host for the same contract** — `ridlc` loads a
    module and calls its export with the same request bytes — and is not in this
    release. It adds a wasm runtime to `ridlc`, and it does not make a
    TypeScript plugin easier to write, because TypeScript to wasm means bundling
    a JavaScript engine; a TypeScript plugin author writes a Deno process
    speaking the protocol, which the process host already covers.

    Backends as dynamically loaded Rust crates were rejected: the plugin would
    be Rust-only and the dynamic-library ABI is not stable, which reduces to the
    in-tree feature-gated crate that exists today.

11. **The parity test is the in-tree TypeScript backend run as a plugin.** The
    TypeScript backend is also built as a `ridlc-gen-ts` binary, and the test
    suite runs it through the process host and asserts output byte-identical to
    the in-process path. That gives the protocol a parity test at no cost in new
    languages, and it is a different test from the one a plugin written outside
    this workspace provides — roadmap's Kotlin plugin is the first of those, and
    it is sequenced after this release.

12. **The contract is the IR encoding, so the IR stability policy lands first.**
    Roadmap story E4.5a is a prerequisite of the lowering step and the contract,
    and driftsys/ridl#231 — the canonical binary encoding cannot round-trip the
    IR — is its first item. E4.5b returns to the critical path with it, so
    roadmap issue #47 stays whole.

## Alternatives considered

| Candidate                                               | Verdict  | Reason                                                                                                                                                                              |
| ------------------------------------------------------- | -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `#[repr(C)]` on the domain types themselves             | rejected | confines every domain type to the C subset, charges the representation to every Rust consumer, and lets a typl §17.11 width change move every later field (decision 3)              |
| C restored as a language backend                        | rejected | the finding of ADR-0018 decision 6 stands — a layout-only header drops most of a realistic package silently; the need is an encoding, and decision 1 is one                         |
| proto3 at the codec-in-wasm boundary                    | rejected | ADR-0018 decision 3's reason for proto3 is survivable version skew, and a codec generated with its host has no skew; keeping proto3 there would materialize an object on every read |
| The `repr(C)` layout rules in the ADR-0018 amendments   | rejected | they are target-specific in the way ADR-0017's and ADR-0019's are, so they take a projection record's shape (decision 4); one roadmap revision had this wrong                       |
| `ridl-abi` — the port traits as a crate under `ridl-rt` | rejected | the ports need `Sample<T>`, `Envelope` and `Provenance`, so the crates would always ship together, and the features already buy what the split would (decision 5)                   |
| TypeScript faces and frame with no transport package    | rejected | two independently written emulators would share no transport, and the TypeScript backend would have no end-to-end test in this repository (decision 7)                              |
| TypeScript faces with no frame specification            | rejected | two hand-written transports over the same contract would not interoperate (decision 7)                                                                                              |
| Plugins over the raw IR, no lowering step               | rejected | every plugin re-derives the name transform, the widths, the init values and the tombstone handling that the in-tree backends already duplicate (decision 8)                         |
| Template packs for the whole backend                    | rejected | faces and codecs are on the order of a thousand lines of logic each that a template language cannot type-check; a types-only printer over the lowered model stays reasonable        |
| Backends as dynamically loaded Rust crates              | rejected | Rust-only plugins over an unstable dynamic-library ABI, which reduces to the feature-gated in-tree crate that exists today (decision 10)                                            |
| A wasm host only, no process host                       | deferred | adds a wasm runtime to `ridlc` and makes no plugin language easier; reopened by the browser playground (decision 10)                                                                |
| An in-tree Kotlin backend                               | rejected | the plugin system exists so a third language does not enter this workspace; the roadmap-simplification note's S-27 rejected it on the same ground                                   |

## Consequences

- **Positive — a third language reaches the ecosystem without reaching the
  workspace.** Decisions 8 to 11 make an out-of-tree backend a supported
  artifact rather than a fork, which is what lets the Kotlin work proceed in the
  hands of the people who maintain Kotlin.
- **Positive — a backend becomes mostly a printer.** Decision 8 moves the
  duplicated derivations into the compiler once, so the next backend's cost is
  its target's syntax rather than the semantics again.
- **Positive — a consumer pays for the encodings it uses.** Decision 5's default
  features off means a `no_std` consumer links a third-party runtime only under
  the `flatbuffers` feature; `proto3` and `repr-c` add none, because ADR-0018
  decision 4 generates those codecs rather than delegating them.
- **Positive — legacy C interoperation is served without a C language target.**
  Decisions 1 and 3 give a C reader a header and a fixed layout while the domain
  types keep their constructors.
- **Positive — a TypeScript read materializes nothing.** Decision 2 puts
  FlatBuffers at the wasm boundary, so the accessors read the buffer in place.
- **Negative — three encodings are three conformance obligations.** ADR-0018
  decision 4's byte-level conformance test now has a third target, and the
  `repr(C)` one is a C compiler rather than a reference implementation.
- **Negative — E4.5a moves onto the critical path and #231 blocks it.** Decision
  12 makes the IR stability policy a prerequisite of the lowering step, so the
  round-trip defect has to be fixed before any backend is ported.
- **Negative — both in-tree backends are ported before either gains a feature.**
  Decision 8 is a new compiler stage and a rewrite of two printers against it,
  with no user-visible change at the end of it.
- **Neutral — the wasm plugin host stays unbuilt.** Decision 10 defers it, and
  the process host covers a TypeScript plugin author through Deno in the
  meantime.

## Open

1. **`Inline` versus `repr(C)`.** The ridl-rt design note's `Inline` payload is
   a FlatBuffers struct read in place; a `repr(C)` struct read in place is the
   same operation with a simpler rule and no runtime dependency. Whether
   `Inline` becomes the `repr(C)` encoding, with FlatBuffers keeping only its
   tables, touches decision 2's matrix and is settled with the projection record
   of decision 4. That note's RA-X7 and RA-X8 are inputs to it.
2. **Where the TypeScript packages live in this workspace, and what the runtime
   package is called.** Decision 6 leaves both open.
3. **Which wasm runtime `ridlc` embeds** for the second host of decision 10, and
   whether the export signature can be identical to the process host's stdin and
   stdout framing.
4. **Whether the lowered model is a public artifact.** Decision 9 puts it in
   ADR-0014's canonical encoding on the wire to a plugin, which makes it a
   versioned surface; whether it is also emittable by a `ridlc` subcommand for a
   plugin author to inspect is not settled.

## Documents amended

| Document                                                                                       | Change                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| ---------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [ADR-0018](ADR-0018-runtime-core-and-generated-surface.md)                                     | decision 3's "two encodings and no more" amended by decisions 1 and 2, and its wasm-boundary classification settled; decision 6's layering extended by decisions 5 to 7 and its Kotlin clause superseded by decisions 8 to 11; decision 15's phase 2 given what it binds by decisions 5 to 7. That record's decisions 16 and 17 carry a 2026-09-12 amendment as well, resting on the re-scope's other decisions rather than on this one — the roadmap's two steps for decision 16, and the re-scope note's §3.1 for decision 17 |
| [ADR-0013](ADR-0013-codegen-backend-scope.md)                                                  | decision 1's target list amended: a language backend need not live in this workspace, and the C header returns as an artifact of decision 1's encoding rather than as a language target                                                                                                                                                                                                                                                                                                                                         |
| [ADR-0007](ADR-0007-e1-execution.md)                                                           | decision 13's `#[repr(C)]` on generated domain structs retired by decision 3; ADR-0018 decision 6 had retired only that decision's header and extern-C face                                                                                                                                                                                                                                                                                                                                                                     |
| [`docs/specification/ridl-family-overview.md`](../specification/ridl-family-overview.md)       | decision-ledger row 35 (decisions 1 to 4), and the open-question index's cross-cutting entry (decision 12)                                                                                                                                                                                                                                                                                                                                                                                                                      |
| [`docs/specification/typl-language-reference.md`](../specification/typl-language-reference.md) | §17.13 sends the `repr(C)` string rules to decision 4's projection record                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| [`docs/wip/2026-09-08-ridl-rt-design.md`](../wip/2026-09-08-ridl-rt-design.md)                 | the library's placement is decisions 5 and 6                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| [`docs/ROADMAP.md`](../ROADMAP.md)                                                             | E11.0 is `ridl-rt` (decision 5); E11.9 is the transport crate and the loopback runtime (decision 6), with the matching package (decision 7); E11.12 is the `repr(C)` codec (decision 1); E4.5a and E4.5b are both in scope (decisions 8 to 12)                                                                                                                                                                                                                                                                                  |

## References

- [`docs/wip/2026-09-12-release-scope-and-plugin-system-design.md`](../wip/2026-09-12-release-scope-and-plugin-system-design.md)
  — the reasoning trail: §3.3 the encoding, §3.4 and §3.5 the layering, §3.8 the
  plugin system, §3.9 the Kotlin plugin, §4 the open items
- [`docs/wip/2026-09-08-ridl-rt-design.md`](../wip/2026-09-08-ridl-rt-design.md)
  — §1.1 the module layout decision 5 names, RA-01 the dependency ceiling, RA-03
  the dependency graph decision 6 follows, RA-29 and RA-30 the reader without a
  verifier and the absent foreign-function boundary
- [ADR-0013](ADR-0013-codegen-backend-scope.md) — the backend classification and
  the target list decision 1 amends
- [ADR-0014](ADR-0014-ir-encodings.md) — the canonical encoding decision 9's
  request carries
- [ADR-0016](ADR-0016-schema-projection-and-the-name-transform.md) — the pinned
  name transform decision 8 applies once, and the totality obligation decision 1
  inherits
- [ADR-0017](ADR-0017-proto3-projection-rules.md) and
  [ADR-0019](ADR-0019-flatbuffers-projection-rules.md) — the shape decision 4's
  projection record takes
- [ADR-0018](ADR-0018-runtime-core-and-generated-surface.md) — decision 2
  sans-IO, decision 3 the encodings, decision 4 the generated codec and its
  conformance obligation, decision 6 the language scope, decision 7 one frame
  with several bindings, decisions 15 and 16 the retraction and the runtime epic
- [`docs/archive/2026-09-08-ridl-abi-design.md`](../archive/2026-09-08-ridl-abi-design.md)
  — the first draft of decision 5's library
- [`docs/ROADMAP.md`](../ROADMAP.md) — E4.5, E11.0, E11.7, E11.8, E11.9, E11.12,
  E12.1
- `crates/ridl-backend-ts/src/lib.rs` — the module header and the init-function
  derivation, the two mirrored derivations decision 8 removes
- driftsys/ridl#231 — the canonical binary encoding cannot round-trip the IR
