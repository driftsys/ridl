# Session prompt: the WebSocket transport and the payload formats — recap, then steer

Status: spent. This is the session prompt of 2026-10-06 that produced
`docs/wip/2026-10-06-ws-and-payload-formats-steering.md`. It records what was
known and leaned toward on that date. The steering note supersedes it: where the
two differ, the note holds (for example, the default is types only, not
FlatBuffers, by decision A1; #265, #264 and #317 are now in the E11 milestone;
#350 has seventeen questions).

Paste this into a fresh session at the root of `driftsys/ridl`. This session
does not implement anything. It produces a recap of where two topics stand, and
then a steering discussion with Sebastien. The discussion ends in decisions he
approves, which are recorded in a dated note under `docs/wip/`.

## The two topics

1. **The WebSocket transport.** This is `ridl-transport-ws`, story E11.9,
   driftsys/ridl#265. It binds the frame onto a WebSocket.
2. **The payload formats.** The three payload encodings are proto3 for the
   network, FlatBuffers for memory, and `repr(C)` as the third (ADR-0020). Each
   encoding has a codec, a projection record and a size state.

The two topics are coupled. A WebSocket carries a framed payload, and the frame
specification defines one binding per transport.

## What is known at handoff (2026-10-06)

Verify each fact before you rely on it.

- The frame specification has landed: E11.1, #257, closed 2026-09-23. It is in
  `docs/specification/frame-specification.md`, and the book chapter is
  `docs/book/reference/frame.md`.
- E11.9 `ridl-transport-ws` (#265) is open and has no milestone. It blocks the
  one open item of the layout-inputs epic. That item is #718: the WebSocket row
  of the binding-overhead table, which holds the frame header size and the
  envelope size, each with its version. The table lives in
  `crates/ridl-ir/src/codegen/bindings.rs`. Until the row exists:
  - a plugin's `max_message_bytes` is null;
  - the layout test plugin, `crates/ridlc/tests/layout.rs`, tests the sum with a
    synthetic row of 14 + 2 bytes that no binding defines (DD-58 in
    `docs/archive/2026-10-05-layout-inputs-design.md`).
- FlatBuffers: the projection rules are ADR-0019. The codec, E11.7, has landed
  (`docs/design/flatbuffers-codec.md`).
- proto3: the projection rules are ADR-0017. The codec, E11.8, is #264, which is
  open and has no milestone. It adds byte-level conformance against a
  `protoc`-generated implementation.
- `repr(C)`: the codec, E11.12, is #317, which is open and has no milestone.
  - Its layout rules are not decided yet. They will be decided in a projection
    record of the same shape as ADR-0017 and ADR-0019, written when the backend
    is.
  - #726 waits on that record: a `repr(C)` size row and a per-link encoding key.
  - Today no channel gets the `repr(C)` encoding, and no size state exists for
    it.
- Which encoding a link gets depends on its crossing. A link on the same machine
  gets FlatBuffers, and the two other crossings get proto3 (design D-4).
- #350 lists thirteen `ridl-rt` API questions, to be settled with evidence from
  E11.1, E11.9 and the codecs.
- `docs/ROADMAP.md` gives the sequence. It runs E11.0, then E11.1, then E11.9.
  The codecs run E11.7, then E4.5a and E4.5b (the plugin protocol), then E11.8
  and E11.12. The TypeScript framework, E12 (#287), reaches its codec through
  the wasm build of the runtime library.
- Related open issues to triage:
  - #398, the string projection sentence owed by ADR-0017 and ADR-0019;
  - #690, shared proto3 refusal rules;
  - #486 and #487, generated-crate observations about payload types and face
    buffers;
  - #725, a codegen-request emit for plugin authors;
  - #727, per-link sizing grain.

## Decided by Sebastien before this session (2026-10-06)

**A codec is emitted only when it is asked for**, one encoding at a time:
FlatBuffers when asked, proto3 when asked, the C struct (`repr(C)`) when asked.
This replaces the idea of choosing codecs from a deployment's links.

Today, the Rust backend always emits the FlatBuffers codec.

- `WireEncoding` in `crates/ridl-backend-rust/src/lib.rs:399` has one value,
  `FlatBuffers`, and that value is the default.
- The `wire-encoding` option in `contract.rs:31-44` accepts only `flatbuffers`.
- The face, `Bind` and `serve`, is written for that one encoding.

The `.fbs` and `.proto` schema files are a separate matter: they are already
opt-in, through `--emit flatbuffers` and `--emit proto`.

The steering settles these questions under the decision:

1. **When nothing is asked for.** Either a build that asks for nothing gets
   types only, with no codec and no face, or it gets FlatBuffers, as today. The
   controller leans towards keeping FlatBuffers as the default for now, with
   "types only" as an explicit request, so that the cabin demo and existing
   users keep working. A strict rule with no default is a breaking change, which
   is acceptable before 1.0.
2. **How a build asks.** No command-line flag sets a backend option today. The
   controller leans towards the manifest, for example a `[backend.rust]` table
   with `encodings = [...]`, because an encoding choice should be the same in
   every build. A command-line flag, if one is wanted as well, is governed by
   ADR-0010.
3. **Several values.** `wire-encoding` takes a single value, so it becomes a
   list. The face then has to choose the encoding per port. `ridl-rt` is already
   generic over the encoding (`Payload<E: Encoding>`,
   `Member::reservation::<E>()`), but the face code is not.
4. **A mismatch with the deployment.** When a selected deployment has a link
   whose encoding was not asked for, should the toolchain warn at build time?
5. **The size column, and a table per encoding (Sebastien's proposal).** Today
   each payload carries one combined row in the generated member table,
   `PayloadInfo.max_size: EncodedSizes { proto3, flatbuffers, repr_c }`.
   `Encoding::max_size` picks a column, and `Member::reservation::<E>()` reads
   it (`crates/ridl-rt/src/encoding.rs`, `crates/ridl-rt/src/contract.rs:192`).
   Each codec also emits `<T as Payload<E>>::MAX_SIZE`, which is the value the
   face uses. So sizes live in two places, and the row carries `None` for
   encodings that were never emitted.

   The proposal: each emitted codec also emits its own size table per interface,
   for example `impl Sizes<FlatBuffers> for Cabin`, and the reservation and the
   table budget read that table.

   Gains:
   - the table matches the on-request rule;
   - an encoding not asked for is a compile error instead of a run-time
     `Unsized`;
   - `None` keeps one meaning: no bound in this encoding;
   - `repr(C)` arrives as a new table instead of changing `EncodedSizes`, which
     is not `#[non_exhaustive]`;
   - the table could be derived from the codec's own `MAX_SIZE`, which makes one
     source.

   Costs:
   - a breaking change to `ridl-rt` 0.1's public API, under ADR-0021:
     `EncodedSizes`, `PayloadInfo.max_size`, `Encoding::max_size`, and the
     signatures of `reservation` and `table_budget`; this means `ridl-rt` 0.2;
   - a consumer not compiled against the package loses the view of all encodings
     in one row. It reads the catalog descriptor instead, which keeps every
     column. So does the codegen model.

   The controller's leaning: adopt it in the same change as the on-request rule.

Records this amends: ADR-0018, ADR-0020 decision 5, `interaction-face.md`,
`flatbuffers-codec.md`, and the manifest's specification if the request lives
there.

## Step 1: the recap

Dispatch one Sonnet subagent (`Agent`, `model: "sonnet"`) to build the recap. It
writes the recap to the scratchpad and returns only the file path. For each
topic, it covers:

- What is built: crates, records, tests. Each fact is cited as `path:line`, or
  as an issue or PR number.
- What is decided but not built: the ADR decisions and the ROADMAP rows, each
  with its status as written in the record itself.
- What is open: the issues, with their state, milestone and blockers, and what
  each one waits on.
- The dependency edges between the two topics, and with E12 (TypeScript), E17
  (#718), the Kotlin plugin, and #350.
- The drift: anywhere the ROADMAP, an ADR, an issue body and the code disagree.

The subagent reads, in this order:

1. `docs/ROADMAP.md`, the E11 section and the sequence;
2. ADR-0013, ADR-0017, ADR-0018, ADR-0019 and ADR-0020, the `## Status` and the
   decisions on encodings, framing and transports;
3. `docs/specification/frame-specification.md`;
4. `docs/design/flatbuffers-codec.md`, `docs/design/ridl-rt.md` and
   `docs/design/codegen-plugins.md`;
5. the issues above, with `gh issue view <n>`;
6. a grep of `crates/` for what exists of the transport and the codecs.

Read the recap. Then present it to Sebastien as a short status table per topic,
with the open questions listed under it. Ask him whether it matches his
understanding before you go further.

## Step 2: the steering discussion

Use `superpowers:brainstorming`. Ask one question at a time, and give a
recommendation with each question. These are candidate questions. Sebastien
decides which ones matter and in what order.

- **Order.** Does E11.9 (WebSocket) wait for E11.8 (the proto3 codec)? Or can
  E11.9 land carrying opaque payload bytes, so that the WebSocket row of #718
  can land sooner?
- **The WebSocket row's numbers.** The frame header size and the envelope size
  per frame version: does the frame specification already fix them, or does
  E11.9 decide them?
- **`repr(C)`.** Is it still in step 1's release scope? When is its projection
  record written, and what does that record have to decide first: the
  C-representable subset, alignment, and the slot layout?
- **The proto3 conformance strategy.** E11.8 plans a byte-level comparison
  against `protoc`. What does that require of the gate and the toolchain
  (ADR-0009)?
- **Milestones.** #265, #264 and #317 have none. Should they get one, and which
  stage or lane runs them?
- **#350.** Which of the thirteen questions does this work settle, and which
  stay open?
- **A catalog-descriptor reader in the runtime.** Today the only reader of
  `<base>.catalog.binfb` is `ridl-descriptor`. That crate cannot be a dependency
  of `ridl-rt` or of a generated package, because:
  - it depends on planus, which ADR-0020's decision 5 amendment of 2026-10-03
    forbids `ridl-rt` and every generated package to depend on;
  - it also depends on `ridl-ir`.

  `ridl-rt` has no dependencies at all. Does the WebSocket transport need to
  read a catalog at run time? For example, a handshake that checks a peer
  against a catalog loaded from a file, or a bridge with no generated code. Or
  is the compiled-in `CATALOG` hash enough?

  The options are:
  1. wait for a consumer (following the #715 rule: build it when a runtime reads
     it);
  2. a hand-written reader with no dependencies, behind a `catalog` feature of
     `ridl-rt`, following the precedent of ADR-0020's 2026-09-20 "module under a
     feature";
  3. the same reader as a separate runtime-side crate named after its content
     (for example `ridl-catalog`), when it must be released on its own schedule;
  4. a light reader feature of `ridl-descriptor`, without `lower`, for a Rust
     runtime outside `ridl-rt`.

  Options 2 and 3 should replace the planus accessors as the only reader, with
  `ridl-descriptor` keeping planus for writing only, so that two readers of
  `catalog.fbs` cannot drift apart. Either option amends ADR-0020 decision 5 and
  ADR-0021. The controller's leaning before the discussion: option 2, but only
  once a consumer exists.

Keep to the project's discipline:

- An asymmetry between backends justifies a backend strategy, not new syntax.
- Write nothing in a shipped doc that is not built yet.
- Ask Sebastien only for decisions that are his. Record every other decision,
  with what it costs if it is wrong.

## Output

- A dated note, `docs/wip/2026-10-0X-ws-and-payload-formats-steering.md`. It
  holds:
  - the recap table;
  - the decisions taken, each with its alternatives considered and its trace
    links (issue numbers, ADR decisions);
  - the questions left open;
  - the proposed next stage or stages, each with a one-line scope and its
    starting condition.
- A list of tracker changes for Sebastien to approve: milestones, issue edits,
  new issues. Do not apply any change without his approval.
- No code change. No ADR edit. ADR amendments come later, with the work that
  needs them.

Then stop. Planning and implementation happen in later, separate sessions.
