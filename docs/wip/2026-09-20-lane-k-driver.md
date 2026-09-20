# Lane K driver — the FlatBuffers payload codec

Status: driver prompt, 2026-09-20. One lane. Each stage is a fresh session. Set
the `THIS SESSION RUNS` line below before starting a session, and do only that
stage.

Where this document and an ADR disagree, the ADR wins. This document summarizes;
it does not decide.

---

You drive lane K: roadmap story E11.7, the FlatBuffers payload codec — the first
of the three codecs ADR-0020 sanctions, and the first thing that lets a
generated package carry a payload without a hand-written `Payload`
implementation.

Read `AGENTS.md`, then
[ADR-0019](../decisions/ADR-0019-flatbuffers-projection-rules.md) (the
FlatBuffers projection, which this codec must agree with byte for byte),
[ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
decisions 1, 2, 5 and 7 (the three encodings, the codec-in-wasm boundary, one
cargo feature per encoding, the codec as generated Rust),
[ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) (the `ridl-rt`
0.1 API and the 0.x breaking-change rule),
[ADR-0023](../decisions/ADR-0023-interaction-face-generation.md) (the face's
entry point and call signatures), and the two design records
[`ridl-rt`](../design/ridl-rt.md) and
[the interaction face](../design/interaction-face.md). Those records bind this
lane.

Then read the code the codec has to fit: `crates/ridl-rt/src/payload.rs`,
`crates/ridl-rt/src/encoding.rs`, `crates/ridl-backend-flatbuffers/src/lib.rs`
(the `.fbs` projection as built), and
`crates/ridl-backend-rust/src/descriptors.rs` and `src/face.rs` (the two places
the generated face names an encoding).

**THIS SESSION RUNS: K1**

## Stages

### K0 — the two rows and their issues. Done.

E11.7 is a story about a codec, but two things stand between a working codec and
a consumer who can use one, and neither had a row. K0 filed both, in the pull
request that added this document:

- **E11.14** (driftsys/ridl#444) — the face and the codec reach `ridl build`.
  `ridlc::run_build` calls `ridl_backend_rust::generate`, and the face is
  emitted by the companion `generate_face`, so a package built through the CLI
  carries its domain types and nothing else: no descriptors, no face, no codec,
  and a `Cargo.toml` naming `ridl-rt` with no encoding feature.
- **E11.15** (driftsys/ridl#445) — the `ridl-loopback` crate, split out of E11.9
  so it does not wait on the frame specification. E11.9 keeps
  `ridl-transport-ws`; the handle clause of its `Done when` moved to E11.15
  unchanged. The loopback speaks no frame and opens no socket, and it is what a
  package emitted by E11.14 is exercised over.

K0 also moved every reference that named E11.9 as the replacement for the
test-only ports onto E11.15: `docs/design/interaction-face.md`,
`docs/design/ridl-rt.md`, `docs/technotes/ridl-rt-by-example.md` and the module
documentation of `crates/ridl-backend-rust/tests/support/loopback.rs`. It
retitled driftsys/ridl#265 and recorded the split in a comment on it, rather
than rewriting its body.

Neither new story is lane K's to execute. E11.14 is named here because four of
K1's decisions are what it executes, and E11.15 because the codec's end-to-end
demonstration runs over it.

### K1 — the design note, the gate, then the plan.

Write `docs/wip/2026-09-2x-flatbuffers-codec-design.md`: the design note for
E11.7, taking the twelve decisions below. Open it as a pull request.

**The gate is Sebastien's disposition comment on that pull request**, the way
D-1 to D-6 of the face and port ergonomics note were disposed on
driftsys/ridl#429. Write each decision so it can be disposed of by reading:
state the decision, the reason, and the alternative it rejects with the reason
that alternative was rejected. Number them K-1 to K-12 so a comment can name
one.

Then, **in the same session**, once the disposition is in hand, write the plan:
`docs/wip/2026-09-2x-flatbuffers-codec-plan.md`, in the shape of
`typl-value-objects-plan.md` — one task per landable change, each with the files
it touches, the test that proves it, and what it must not break. The plan names
the stages K2 onward, and the plan's pull request amends this document's
**Stages** section with them.

If the disposition does not arrive before the session runs low on context, stop
at that boundary and post the handoff; the plan is a fresh session with the same
prompt and the `THIS SESSION RUNS` line set to K1, continuing from the approved
note.

## The twelve decisions K1 must take

**K-1. What reads and writes the buffer.** Emitted Rust that encodes and
verifies the FlatBuffers wire format directly, `flatc`-generated accessors, or a
third-party crate (`flatbuffers`, `planus`). ADR-0020 decision 7 makes a
package's codec the generated Rust for that package, which rules out a runtime
schema interpreter but does not by itself choose between the other three. State
what a consumer must have installed to build an emitted crate; a `flatc`
invocation in a `build.rs` is a build-time dependency on a C++ toolchain, and
the note must say whether that is accepted. `planus-translation` is already a
workspace dev-dependency, as the schema validity oracle for
`ridl-backend-flatbuffers`'s tests — the note must not confuse an oracle in this
repository's tests with a dependency of emitted code.

**K-2. What the `ridl-rt` `flatbuffers` feature enables.** Today it enables
nothing, and the crate has no dependency in any feature combination
(`crates/ridl-rt/Cargo.toml`). If shared codec machinery belongs in `ridl-rt`,
say exactly what, and check it against the three things that bind that crate:
`no_std`, the `wasm32` build with `--no-default-features` (`just wasm-check`),
and the edition 2021 build at rust-version 1.83 (`just compat-check`, ADR-0021
decision 10). A new public item in `ridl-rt` is an ADR-0021 decision, not an
implementation detail.

**K-3. What `Payload::<FlatBuffers>::View<'a>` is.** An accessor over the
buffer, or the bytes. ADR-0020 decision 2 chose FlatBuffers at the codec-in-wasm
boundary precisely because the host reads the buffer in place and no
materialized object crosses on a read, so a `&[u8]` view that forces a `decode`
before any field can be read would give that reason away. Say what `verify`
leaves behind and what a reader gets without calling `decode`.

**K-4. Where the implementations are emitted, and from which entry point.**
`generate` is the pipeline entry point and `generate_face` the companion
(ADR-0023 decision 2). A codec is not a face: it is needed by a package whose
consumer never dispatches. Decide whether the `Payload` implementations go in
`generate`'s output, in `generate_face`'s, in a third entry point, or in a
module of the generated unit behind a cargo feature of the emitted crate — and
say which module of `crates/ridl-backend-rust/src/` emits them.

**K-5. How the generated face stops being bound to `ReprC`.** `src/face.rs`
names `::ridl_rt::encoding::ReprC` in the encode and verify paths, and
`src/descriptors.rs` computes `MAX_BUFFER_SIZE` and `EVENT_SOURCE_BUFFER_SIZE`
as a maximum over `<T as Payload<ReprC>>::MAX_SIZE`. Decide what replaces that
literal: a type parameter on the face, a per-package encoding chosen at build
time, or something else. Say what happens to the buffer constants when a package
carries more than one encoding, and whether ADR-0023 is amended.

**K-6. How `decode` crosses Epic 10's validating seam.** `Payload::verify`
checks the structure of the bytes and the typl constraints of the value in one
pass, and `Payload::decode` takes a `Ref` and cannot fail. Epic 10 emits a
constrained named scalar with a private inner, a fallible `new`, and a safe
`const fn new_unchecked`; a vacuous type emits **no** `new_unchecked` (the
value-objects design, decisions 1 and 2). Decide which constructor a generated
`decode` calls, for each of those two shapes and for a nested struct, and state
precisely which constraint each of `verify` and `decode` is responsible for. A
`decode` that re-checks makes `verify`'s one-pass promise a lie; a `decode` that
does not, while `verify` missed a leaf, constructs an invalid value through a
safe path.

**K-7. Who owns the `MAX_SIZE` computation.** `Payload::MAX_SIZE` is the largest
encoded size of any legal value, as a const. E16.3 and E16.4 compute the same
bound per payload for the catalog descriptor, over the same IR
(`docs/ROADMAP.md`, Epic 16). Decide whether one computation serves both and
where it lives, or whether there are two and what keeps them equal. Say what the
bound does with an unsizable type, and how that reaches a `const`.

**K-8. How `encode` writes into a caller's buffer with no allocator.**
`Payload::encode` takes `&'o mut [u8]` and returns `Encoded`, whose `bytes`
field is documented as "a prefix of the output buffer"; a FlatBuffers builder
writes back to front, so its result is naturally a suffix. Decide how the two
are reconciled, what `EncodeError::Capacity` means for this encoding, and
whether the `Encoded` documentation in `ridl-rt` changes — which would be an
ADR-0021 decision.

**K-9. How the codec and the `.fbs` projection are kept in agreement.** ADR-0019
fixes the projection: a union isolated in a wrapper table, a non-table union arm
boxed, every struct a `table` (never a FlatBuffers `struct`), a map as a vector
of entry tables with no `(key)`, the target's own name scopes, and `= null` on a
field whose enum declares no zero member. `ridl-backend-flatbuffers` emits that
schema today. Decide whether the codec and the schema emitter share one lowering
or are two implementations of one ADR, and name the test that fails when they
diverge.

**K-10. `(key)` and sorted-vector lookup.** ADR-0019's open item 1 defers this
decision to E11.7 by name. Take it, or state the reason for deferring it again
and the story that inherits it.

**K-11. Presence, defaults and the absent field on decode.** A FlatBuffers
scalar equal to its declared default is absent from the buffer, and a table
field may be absent entirely. Decide what an absent field decodes to for each
typl shape — a required field, an optional field, a collection, an enum-typed
field carrying ADR-0019 decision 6's `= null` — and whether an absent required
field is a `verify` failure. This is where a round trip that looks correct on a
non-default value silently loses information on a default one.

**K-12. What proves the codec correct.** E11.7's `Done when` today is "a payload
round-trips through the library", while E11.8's requires byte-level conformance
against a `protoc`-generated implementation. Decide whether E11.7 owes the same
obligation against an independent FlatBuffers implementation, what that
implementation is, and whether the roadmap row changes. A round trip through one
implementation proves self-consistency, not that the bytes are FlatBuffers.

## What lane K does not decide

- **The projection.** ADR-0019 is merged and binding. A projection rule the
  codec cannot implement is a finding to escalate, not a rule to reinterpret.
- **The other two codecs.** E11.8 (proto3) and E11.12 (`repr(C)`) are separate
  stories. Where a decision above would bind them too — K-4, K-5, K-7 and K-8
  all might — say so explicitly and give the reason it generalizes, but do not
  design them.
- **The frame.** E11.1 is the frame specification: what wraps a payload on the
  wire. A payload codec is what the frame carries. The note must not specify a
  frame, an envelope binding or a wire tag.
- **The engine and the transport.** Outside this repository (the 2026-09-12
  re-scope §3.7) and E11.9 respectively.
- **The plugin system.** ADR-0020 decision 8 makes a backend an executable over
  `generate(CodegenRequest)`; the Rust backend is ported onto that contract in
  step 2, not here.

## Four facts about the tree, easy to get wrong

- **`ridl-rt` ships no codec and no runtime.** Its three encoding markers are
  marker types with no cargo feature attached to them; the features exist and
  enable nothing. Do not write that the crate implements an encoding.
- **Every struct is a FlatBuffers `table`** (ADR-0019 decision 3), so there is
  no fixed-layout struct to read in place and every field access is a vtable
  lookup. A `MAX_SIZE` that forgets vtables and alignment padding is wrong, and
  the face sizes real buffers from it.
- **The hand-written `Payload<ReprC>` implementations in
  `crates/ridl-backend-rust/tests/interaction_face.rs` are a throwaway**, marked
  as one in their own module documentation. They are not a reference
  implementation and not a specification of what a codec emits.
- **`ridl --emit rust` emits no face today.** That is E11.14
  (driftsys/ridl#444), not a defect to fix in passing while implementing a
  codec.

## Order against other lanes

The Rust backend is shared. Lane C's Epic 10 reshapes the domain types this
codec encodes, and K-6 depends on the constructors Epic 10 emits, so the note
must read `crates/ridl-backend-rust/src/lib.rs` as it stands on `origin/main` on
the day it is written, and name the Epic 10 task each assumption rests on.
Anything that regenerates
`crates/ridl-backend-rust/tests/generated/interaction_face.rs` is ordered
against that lane's open work; check for an open pull request touching it before
branching.

Never enter another lane's worktree.

## Mechanics

Work in a worktree under `.claude/worktrees/`, created with `git worktree add`,
and run `./bootstrap` there. Branch fresh from `origin/main`.
`git branch --show-current` before every commit and every push. **Stage explicit
paths, never `git add -A`.** `just fmt` before every Markdown commit,
`cargo fmt --all` before every Rust commit. Conventional Commits with a type and
a scope from `.git-std.toml`; a new crate adds its own scope.

Gate before a pull request: `just verify`. Review per the lanes plan §7
(`2026-09-13-step1-lanes-plan.md`), two passes, ledger on the pull request, then
merge. One pull request open at a time. Never push a tag, publish to a registry,
add a secret, or push to `main`.

**Two container facts to check before trusting a gate.** `git-std` and `prim`
are installed in some containers and not others; when the release installer
answers 403 through the agent proxy, install `git-std` with the fallback
`ci.yml` already carries,
`cargo install --git https://github.com/driftsys/git-std git-std --locked`. And
driftsys/ridl#430 and driftsys/ridl#434 fail `just test` in a container that
runs as root, because both assert on permission bits that do not apply to uid 0;
if those are the only failures, push with `GIT_STD_SKIP_HOOKS=1`, name the
issues in the pull request, and run `lint`, `wasm-check`, `compat-check` and
`check` yourself, because `just build` stops at `test`.

## After each stage

Post one comment on driftsys/ridl#328: the stage, the pull request, and what
another lane must know. After K1: the twelve dispositions in one line each, and
whether any of them binds E11.8, E11.12 or E11.14.

When the last stage of this lane lands, run `sdd-gardening` in that pull
request: the note and the plan archive to `docs/archive/`, and the decisions
live in the ADR the note names and in a design record under `docs/design/`.

## When to stop

Stop and ask Sebastien when a decision and a merged record disagree in a way the
note did not foresee, or when a gate check does not hold. That case is real: it
happened once in lane R, where two ratified decisions did not compose, and the
answer was a decision rather than a patch.

A session low on context stops at a stage boundary and posts the handoff.

Plain, literal prose everywhere. Never name a private or consumer project.
