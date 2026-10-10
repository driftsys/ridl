# The WebSocket transport and the payload formats — steering note (2026-10-06)

Status: **decided 2026-10-09.** Sections 2 to 5 record the decisions; the
tracker changes of section 5 are approved but not yet applied. Section 1 and the
appendix are the recap built at the start of the session from `origin/main` at
`7fe24437` and the live tracker. The recap was refreshed on 2026-10-09 against
`main` at `376aa927` (205 commits after `7fe24437`) and the live tracker;
citations were re-anchored and the changes listed in section 1.4 were added.

Source prompt: `docs/wip/ws-and-payload-formats-handoff.md`.

## 1. Recap

### 1.1 The WebSocket transport (`ridl-transport-ws`, E11.9, #265)

| Question                       | Answer                                                                                                                                              | Citation                                                                               |
| ------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| Crate                          | Does not exist. No source names a WebSocket library.                                                                                                | `Cargo.toml:15`; grep over `crates/`                                                   |
| Frame specification            | Landed, version 0.1.0 Draft, 946 lines (943 at `7fe24437`) (#257, closed 2026-09-23, commit 7fc417e).                                               | `docs/specification/frame-specification.md:9`                                          |
| Book chapter                   | `docs/book/reference/frame.md` exists.                                                                                                              | `git ls-tree origin/main`                                                              |
| The WebSocket binding          | §11.1 names it, says it is not built, fixes its encoding to proto3 (tag 2), and says it writes the nine items of §10 and adds nothing to the frame. | `frame-specification.md:788-808`                                                       |
| Header and envelope byte sizes | Not fixed by the spec. "This document places no bound on a header's size". Byte layout is binding item 2.                                           | `frame-specification.md:226, 764-766`                                                  |
| Binding-overhead table         | `KNOWN` is empty; the comment points at #265. The layout proof test adds a synthetic `websocket` row of 14 + 2 bytes.                               | `crates/ridl-ir/src/codegen/bindings.rs:26-28`; `crates/ridlc/tests/layout.rs:357-389` |
| Done when                      | ROADMAP: a contract reaches a second process over the transport, and the loopback runs the same tests with no socket. Names no codec.               | `docs/ROADMAP.md:476-490`                                                              |
| Blocks                         | #718 (E17.3, the WebSocket row). #287 (E12.1) names "E11.9's Deno client". Kotlin does not depend on it.                                            | #718 body; #287 body; `ROADMAP.md:1033-1050`                                           |

### 1.2 The payload formats

| Encoding    | Projection record                                       | Codec                                                                           | Size state    | Open story                                          |
| ----------- | ------------------------------------------------------- | ------------------------------------------------------------------------------- | ------------- | --------------------------------------------------- |
| FlatBuffers | ADR-0019, Accepted 2026-08-09                           | Built (E11.7); conformance against `planus`                                     | Built         | none                                                |
| proto3      | ADR-0017, Accepted 2026-08-08                           | Not built; schema emitter and sizer exist; Rust backend "emits no proto3 codec" | Built (sizer) | #264 (E11.8), open, no milestone                    |
| `repr(C)`   | None; ADR-0020 decision 4 says written with the backend | Not built; marker type and feature only; sizer answers `EncodingUndefined`      | None          | #317 (E11.12), open, no milestone; #726 waits on it |

Citations: `docs/design/flatbuffers-codec.md:1-40`;
`docs/design/interaction-face.md:95`;
`crates/ridl-ir/src/projection/size.rs:253-266`;
`crates/ridl-rt/src/encoding.rs:65-73`.

Encoding per link: same machine = FlatBuffers; different machine and off-board =
proto3; no link gets `repr(C)` (`docs/design/codegen-plugins.md`, "The encoding
rule"; archived D-4 at `docs/archive/2026-10-05-layout-inputs-design.md:171`).

ROADMAP: `repr(C)` is in step 1 ("the three payload codecs, byte-conformant",
`docs/ROADMAP.md:1104`, `:216-218`). Sequence: E11.7 → E4.5a → E4.5b → E11.8 ·
E11.12 (`:262-267`). E4.5a and E4.5b have landed.

`protoc` appears nowhere in `justfile`, `.github/`, `bootstrap` or ADR-0009.
ADR-0006 decision 3 chose `protox` over a system `protoc` for hermetic builds.
`prost` and `prost-build` are already pinned in the workspace
`Cargo.toml:106-107`.

### 1.3 Handoff facts refuted or corrected

- #350 lists 17 questions, not thirteen. Five are unchecked: 5, 12, 13, 14, 17.
  Item 17 is recorded closed by ADR-0021 decision 13 (`Wakeable`), so the body
  is stale.
- E12 reaches the Rust codec through the package's own generated Rust compiled
  to `wasm32` against `ridl-rt`, not through a wasm build of `ridl-rt` alone
  (ADR-0020 decision 7: a `ridl-rt`-only build "could encode nothing").

### 1.4 Drift found

See appendix section 4 for the full table. The items that matter here:

- ROADMAP row E11.1 and #257's title say "one binding per encoding"; the frame
  spec says one binding per transport. The #257 comment records the change; the
  row was not edited.
- #265's body is the 2026-08 filing. The 2026-09-20 comment says the ROADMAP row
  is the current scope.
- #264 and #317 name epic E11 in their bodies but carry no milestone; the "E11 —
  Runtime Library" milestone (#10) exists with 7 open issues.
- #690 names `crates/ridl-descriptor/src/size/proto3.rs`; the sizer is at
  `crates/ridl-ir/src/projection/size/proto3.rs`. Still true on 2026-10-09.

Added in the 2026-10-09 refresh:

- Catalog per unit (#777): frame specification §4, the `catalog` row, now says
  "the unit name and the 32-byte hash" where it said "package name". The
  `CatalogRef` row of §2 says "one unit's interfaces". No other header field,
  and no encoding tag, changed.
- Frame specification lines 128-130: `TraceContext` is not on the frame, so a
  frame transport delivers `None`.
- ADR-0020 decision 6 was amended on 2026-10-08: `ridl-rt-conformance` is
  published. The `ridl-transport-ws` row of the decision is unchanged.
- #726 is now labelled P3. The E17 milestone has 2 open issues, #737 and #718.
  #350 is in the "E11 — Runtime Library" milestone, as section 7.5 lists it.
- #486 and #487 were ruled by lane G on 2026-10-09 (R-7 and R-8) and labelled
  `ready`; see the appendix, section 2.3.
- Lane H driver (`docs/wip/2026-10-09-lane-h-driver.md`, item 2) questions frame
  specification §6.1: §6.1 refuses `attach` with `catalog_mismatch` whenever the
  two catalog hashes differ, while `ridl diff` classifies an appended
  interaction as a compatible change. WebSocket steering touches this, because
  every binding, the WebSocket one included, carries the catalog reference at
  `attach`.
- #265, #264, #317, #718, #726, #350 and #690 are not scheduled or changed by
  lane H or by the lane plan.

## 2. Decisions taken

Decided by Sebastien on 2026-10-09, under the 2026-10-06 rule that a codec is
emitted only when it is asked for. Each decision names its cost if it is wrong.

### A. The on-request encodings

- **A1. Types only by default.** A build that names no encoding gets the domain
  types and nothing else: no codec, no face. A codec is emitted only when it is
  named. Reason: once `repr(C)` exists, a crate with no serialization codec may
  be the common case, so FlatBuffers by default would soon be the wrong default;
  and an implicit default contradicts the rule. Breaking before 1.0. Migration:
  the cabin demo, `just demo`, and the Kotlin plugin's fixtures each gain one
  manifest line; one Kotlin heads-up.
- **A2. The manifest asks, per backend.** `[backend.rust]` with
  `encodings = [...]`. No command-line flag; one can be added later under
  ADR-0010 without a break. The `wire-encoding` plugin option, its single value
  and its default are replaced by the list.
- **A3. The face is generic over the encoding.** `Bind<E>` and `serve::<E>`; the
  build emits every codec named; the program that links the package picks the
  encoding per port, where it also picks the transport. `ridl-rt` is already
  generic (`Payload<E>`, `reservation::<E>()`). Fallback if the generic face is
  more than one stage of work: allow one entry in the list, lift the limit
  later.
- **A4. A mismatch warning.** `ridl build` raises a warning under ADR-0024 when
  a selected deployment has a link whose encoding the package's `encodings` list
  lacks; a user can lower it in `[lints]`. Cost if wrong: one noisy warning set
  to `allow`.
- **A5. One size table per emitted codec**, in the same change as A1 to A3. Each
  codec emits its own table per interface, derived from its `MAX_SIZE`, and the
  reservation and the table budget read that table. `EncodedSizes`,
  `PayloadInfo.max_size`, `Encoding::max_size`, `reservation` and `table_budget`
  change, so `ridl-rt` 0.2, in the one breaking release A1 already needs. The
  catalog descriptor keeps every column. Settles #350 item 13.

### B. The WebSocket transport does not wait for proto3

Frame spec §11.1 is amended so the WebSocket binding carries the encoding the
session names at `attach` and fixes none. #265 lands over FlatBuffers, the codec
that exists; when proto3 lands, a session names tag 2 and the binding does not
change. Reason: A3 makes the face encoding-agnostic, the transport carries
opaque bytes, and the spec already says one binding per transport. Cost if
wrong: a consumer that assumed WebSocket means proto3 names the tag, which
`attach` already requires.

**Refined 2026-10-10, during the stage 1 spec review.** #265 lands with
`connect_with::<FlatBuffers>` only, the codec that exists; plain `connect`,
whose default is proto3 (decision I), is added when #264 lands, additively.

### I. The encoding comes from the port type

Decided 2026-10-10 during the stage 1 spec review. A port type states the
encoding of the bytes it carries (`ridl_rt::port::Encoded`), and the generated
face reads it from its port type parameter: application code names no encoding,
and `cabin::Client::new(rt.attach())` stays as it is today. Each transport picks
a default from how coupled its two ends are — WebSocket proto3, shared memory
`repr(C)`, the in-process loopback FlatBuffers — and the construction can
override it (`connect_with::<E>(url)`, `Loopback::<E>::with_encoding(..)`). A
cargo feature for the default was rejected: features must be additive. The
manifest says which codecs a package carries, not which one a port uses. One
session has one encoding (frame specification rule); a transport serving several
encodings hands out one port per session, each typed with its own. This refines
A3 (`Bind<E>`, `serve::<E>` become `Client<P>` with `P: Encoded`) and B (above).
Cost if wrong: a port type per transport that implements one trait;
`ridl-loopback` typed by `E`.

### C. `repr(C)` stays in step 1, record first

Its projection record is written before any code, as ADR-0020 decision 4 says.
ADR-0020 decisions 1 and 3 already fix its shape: a codec between a generated
`#[repr(C)]` layout struct and the domain type; the domain types do not carry
`#[repr(C)]`. With A5 it arrives as a new size table and no longer changes
`ridl-rt`'s API. The questions the record must settle are in section 3.

### D. `prost` is the proto3 oracle

Conformance is checked the way E11.7 checked FlatBuffers against `planus`: a
test crate compiles the emitted `.proto` with `prost-build` through `protox`
(both pinned already), encodes the same values with both codecs and compares
bytes, and decodes each side's bytes with the other; hand-written byte vectors
from the wire spec cover packed repeated scalars, field order, unknown fields
and malformed input. The oracle-boundary guard is extended so `prost` cannot be
reached from `ridl-rt` or a generated package. `protoc` stays out of the gate
(ADR-0006 decision 3 holds). A check against a system `protoc` can be run by
hand outside the gate.

### E. Order and milestones

Stage 1 (group A) first; #265, #264 and #317 are independent of each other and
run as parallel lanes after it. #265, #264 and #317 go into the E11 milestone;
stage 1 gets a tracking issue there. See section 4.

### F. #350

Item 13 is settled by stage 1 (A5). Item 17 is already closed by ADR-0021
decision 13; its box is ticked. Items 5 and 12 go to stage 2's binding document
(what a lost call becomes on a dropped connection; whether the frame carries a
type identifier for a remote `Violation`). Item 14 (`Rule::Step` on a float
needs a tolerance) is a constraint-check question, not a codec one, and is filed
as its own issue.

### G. No catalog reader in the runtime yet

Option 1 of the handoff: wait for a consumer. Stage 2 checks a peer by the
compiled-in `CATALOG` hash; a tool that renders a live signal links the
generated package. A tool with no generated code needs a reflective decoder as
well as a reader, and neither exists for any encoding; when one is wanted, its
design picks between a `catalog` feature of `ridl-rt` and a separate crate,
starting from the handoff's leaning (the feature). Cost if wrong: none now.

### H. Recorded, not asked

The frame header and envelope byte sizes per frame version are stated by the
binding document, as frame spec §10 item 2 already says; #265 writes it and
fills the `KNOWN` row and #718's WebSocket row.

## 3. Questions left open

- **The `repr(C)` projection record** (stage 4) must settle: the C-representable
  subset (bounded strings, byte arrays, lists and maps: fixed capacity from the
  typl bound, or refused); alignment and padding (whose ABI, pinned by the IR or
  left to the target compiler); the slot layout of an optional field, an enum's
  integer repr, a union's tag and largest member; byte order (native makes it a
  same-machine format, not a wire format, and the record says so); what
  byte-conformance is checked against (the `cc` crate, `bindgen`, or a
  hand-written oracle); and ADR-0020's open item 1, whether `Inline` becomes
  `repr(C)` read in place.
- **Stage 2's binding document** must settle #350 items 5 and 12 (section 2 F).
- **#350 item 14** stays open as its own issue.
- **Frame spec §6.1 versus the catalog hash at `attach`** (lane H driver, item
  2): `ridl diff` calls an appended interaction compatible, while `attach`
  refuses on a hash mismatch. Lane H owns it; stage 2 reads lane H's answer
  before writing the binding's `attach` rules.
- **A generic inspector** with no generated code: not designed; it reopens G.
- **FlatBuffers' place among the three encodings** (added 2026-10-10): after
  #264 lands, measure proto3 against FlatBuffers on real payloads — encode time,
  decode time, encoded size, native and wasm. Deprecate FlatBuffers if it never
  wins on in-place reads of large messages. ADR-0020 open item 1 (`Inline`
  versus `repr(C)` read in place) is part of the same measurement.
- **JSON at the edges of JavaScript applications** (added 2026-10-10, for the
  step 2 TypeScript design): generated TypeScript types as plain data, so that
  `JSON.stringify` works on them, and a `fromJSON` that re-runs the typl
  constraint checks through the wasm constructor; whether JSON, in the form of
  the protobuf JSON mapping, becomes a fourth payload encoding; 64-bit integers
  mapped by their typl range (`number` when the range fits within ±2^53,
  `bigint` otherwise, written as decimal strings in JSON per the protobuf JSON
  mapping; `JSON.rawJSON` source-text access is an option to check); `bytes` as
  `Uint8Array` in memory and base64 only at the JSON edge (native `toBase64` and
  `fromBase64` support to check), with large binary data kept out of JSON.
- **The encoding rule versus the transport defaults** (added 2026-10-10):
  `docs/design/codegen-plugins.md` "The encoding rule" derives a link's encoding
  from its crossing (same machine FlatBuffers, otherwise proto3) and the
  RSDL-807 lint of stage 1 uses it; decision I gives a shared-memory transport
  `repr(C)`. Align the rule with the transport defaults when `repr(C)` or a
  shared-memory transport lands.

## 4. Proposed next stages

| Stage | Work                                                                                                                                                                                                                                    | Depends on |
| ----- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------- |
| 1     | The on-request encodings (A1 to A5): manifest `encodings` list, types-only default, generic face, size table per codec, mismatch warning; `ridl-rt` 0.2; one breaking release; Kotlin heads-up. Its own design session (spec and plan). | none       |
| 2     | #265: the WebSocket transport over FlatBuffers; frame spec §11.1 amendment; the binding document, which fills the `KNOWN` row, #718's row, and #350 items 5 and 12.                                                                     | 1          |
| 3     | #264: the proto3 codec with the `prost` oracle (D).                                                                                                                                                                                     | 1          |
| 4     | #317: the `repr(C)` projection record (section 3), then the codec, the layout struct and the C header; removes `#[repr(C)]` from the domain types (ADR-0020 decision 3).                                                                | 1          |

Stages 2, 3 and 4 are independent of each other.

Records stage 1 amends: ADR-0018, ADR-0020 decision 5, ADR-0024's lint table,
`docs/design/interaction-face.md`, `docs/design/flatbuffers-codec.md`,
`docs/design/ridl-rt.md`, and the manifest's record (ADR-0002). Stage 2 amends
the frame specification (§11.1) and ADR-0020 decision 6's "proto3-framed".

## 5. Tracker changes for approval

Approved by Sebastien on 2026-10-09 in the discussion; not yet applied.

1. File a tracking issue for stage 1, in milestone "E11 — Runtime Library".
2. Add #265, #264 and #317 to milestone "E11 — Runtime Library"; note on each
   that it is blocked by the stage 1 issue.
3. #350: tick item 17 with a pointer to ADR-0021 decision 13; note on item 13
   that stage 1 settles it; note on items 5 and 12 that #265 settles them; fix
   the title ("thirteen" to "seventeen").
4. File #350 item 14 as its own issue (a tolerance for `Rule::Step` on a float
   wire form).
5. #690: correct the path to `crates/ridl-ir/src/projection/size/proto3.rs`.
6. #265: comment that it lands over FlatBuffers and amends frame spec §11.1 (B);
   #264: comment that the oracle is `prost` (D).
7. ROADMAP: fix row E11.1 ("one binding per encoding" to "one binding per
   transport"), and record the stage order of section 4.

---

## Appendix: the full recap

### Recap: the WebSocket transport and the payload formats (2026-10-06)

### Reading basis

- Refreshed on 2026-10-09: every claim was re-checked against `main` at
  `376aa927` (205 commits after `7fe24437`) and the live tracker. Citations
  below carry the line numbers on `376aa927`; the text that follows describes
  the original 2026-10-06 reading.
- The local checkout at `/Users/sebastientasson/Workspace/driftsys/ridl` is on
  `main` at `fac5abce`. It is **52 commits behind `origin/main`** (`7fe24437`,
  the merge of #736). The local tree lacks
  `crates/ridl-ir/src/codegen/bindings.rs`, `crates/ridlc/tests/layout.rs` and
  `docs/archive/2026-10-05-layout-inputs-design.md`. These exist on origin/main
  and in worktrees under `.claude/worktrees/`.
- Every `path:line` below is read from `git archive origin/main` extracted to
  the scratchpad (`.../scratchpad/om/`). The repository itself was not changed.
  `git fetch` updated remote refs only.
- Issue facts come from `gh issue view` (live, 2026-10-06; re-read 2026-10-09).
  Raw issue extract: `.../scratchpad/issues.txt`.
- `docs/ROADMAP.md` is cited as origin/main's copy.

### 0. Common ground (both topics)

| Fact                                                                                                                                                                                                                                                                        | Citation                                                                                                                                     |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| Encoding matrix: a stream (CAN, SOME/IP, DDS, a WebSocket) = proto3; mapped or passed within a node (shm, local socket, Binder) = FlatBuffers; wasm codec with its host = FlatBuffers; independently updated wasm guest = proto3; C reader or no reader library = `repr(C)` | `docs/decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md:136-167` (decision 2)                                          |
| Encoding tags on the frame: 1 FlatBuffers, 2 proto3, 3 `repr(C)`                                                                                                                                                                                                            | `docs/specification/frame-specification.md` §3 table (lines 155-159)                                                                         |
| `ridl-rt` has one cargo feature per encoding: `flatbuffers`, `proto3`, `repr-c`; marker types `FlatBuffers`, `Proto3`, `ReprC` exist                                                                                                                                        | `crates/ridl-rt/Cargo.toml:28-30`; `crates/ridl-rt/src/encoding.rs:65-73`                                                                    |
| D-4 (encoding of a consumer link derives from its crossing alone) is in the archived layout-inputs design, not in a `docs/design/` file                                                                                                                                     | `docs/archive/2026-10-05-layout-inputs-design.md:171` ("### D-4"); also `docs/design/codegen-plugins.md` "The encoding rule" (lines 442-458) |

D-4 table (`layout-inputs-design.md:173-179`): `same machine` = FlatBuffers;
`different machine` = proto3; `off-board` = proto3. Its text: "No consumer link
gets `repr(C)` in this epic: that encoding has no layout until
driftsys/ridl#317".

#### ADR status lines, exact

| ADR                                                                          | Status line as written                                                                                                                      |
| ---------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| ADR-0013 (`docs/decisions/ADR-0013-codegen-backend-scope.md:5`)              | "Proposed — 2026-08-03." Then amendments dated 2026-08-08, 2026-09-12, 2026-08-09 (the file's status block continues beyond what was read). |
| ADR-0017 (`ADR-0017-proto3-projection-rules.md:5`)                           | "Accepted — 2026-08-08."                                                                                                                    |
| ADR-0018 (`ADR-0018-runtime-core-and-generated-surface.md:5`)                | "Proposed — 2026-08-09."                                                                                                                    |
| ADR-0019 (`ADR-0019-flatbuffers-projection-rules.md:5`)                      | "Accepted — 2026-08-09." Amended 2026-09-21 (decision 8, the root rule).                                                                    |
| ADR-0020 (`ADR-0020-third-encoding-runtime-layering-and-plugin-system.md:5`) | "Proposed — 2026-09-12." It amends ADR-0018 decision 3 and extends decision 6.                                                              |

---

### 1. Topic 1: the WebSocket transport (`ridl-transport-ws`, E11.9, #265)

### 1.1 What is built

| Item                                                                                                                                                                                                                                                                                                                                                            | Evidence                                                                                                                                                                                                                                                    |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **No `ridl-transport-ws` crate exists.** The 20 crates in `crates/` are: ridl, ridl-backend-{flatbuffers,proto,rust,ts}, ridl-core, ridl-descriptor, ridl-diff, ridl-fmt, ridl-ir, ridl-loopback, ridl-lsp, ridl-mcp, ridl-rt, ridl-rt-conformance, ridl-sem, ridl-syntax, ridlc, ridlc-gen-model, ridlc-gen-rust. Workspace `members = ["crates/*", "xtask"]`. | `Cargo.toml:15`; `ls crates`                                                                                                                                                                                                                                |
| No source file under `crates/`, `justfile` or `.github` names a WebSocket crate or a socket library. The only hits for `websocket`/`ridl-transport` are the binding-table code, its tests, the codegen proto and `ridl-rt/README.md`.                                                                                                                           | grep over origin/main: `crates/ridl-ir/src/codegen/{bindings,deployment,tests}.rs`, `crates/ridl-ir/proto/ridl/codegen/v1/deployment.proto`, `crates/ridlc/tests/layout.rs`, `crates/ridlc/tests/fixtures/cabin-layout.json`, `crates/ridl-rt/README.md:63` |
| The frame specification, version 0.1.0 Draft, 946 lines (943 at `7fe24437`), is the document a binding is written from. It closed #257 on 2026-09-23 (landed 7fc417e, #496).                                                                                                                                                                                    | `docs/specification/frame-specification.md:9` ("Version: 0.1.0 — Draft"); `gh issue view 257` closedAt 2026-09-23T19:33:36Z; #257 comment: "Landed in 7fc417e (#496), lane P stage P5"                                                                      |
| The book chapter exists.                                                                                                                                                                                                                                                                                                                                        | `docs/book/reference/frame.md` (named in the handoff; not opened here, see section 6)                                                                                                                                                                       |
| The in-process runtime `ridl-loopback` (E11.15, #445) landed: it is "not a binding of this frame and never becomes one". It speaks no frame and opens no socket.                                                                                                                                                                                                | `frame-specification.md:806-808` ("the loopback is not a binding of this frame"); `docs/ROADMAP.md:506-513`                                                                                                                                                 |
| The port vocabulary the transport will implement is in `ridl-rt` (sans-IO): `port.rs`, `sample.rs`, `contract.rs`, `error.rs`, `correlate.rs`, `task.rs`, `encoding.rs`, `payload.rs`.                                                                                                                                                                          | `ls crates/ridl-rt/src`                                                                                                                                                                                                                                     |
| The WebSocket row of the binding-overhead table is **absent**. The table is `pub const KNOWN: &[Known] = &[];`, with the comment "The table is empty because no binding document states a frame layout yet (driftsys/ridl#265)".                                                                                                                                | `crates/ridl-ir/src/codegen/bindings.rs:26-28` (struct `Known` has `frame_header_max_bytes: Option<u32>` and `envelope_bytes: Option<u32>`, plus `version`, lines 10-21)                                                                                    |
| The proof test uses a synthetic row: name `websocket`, version `"1"`, `frame_header_max_bytes: Some(14)`, `envelope_bytes: Some(2)`; asserts `warning` = 24 (14 + 2 + 8) and `temperature` = null. A separate test pins `deployment.bindings.is_empty()` and that every `max_message_bytes` in the fixture is null.                                             | `crates/ridlc/tests/layout.rs:357-389` and `:334-337` ("No binding has a row yet (driftsys/ridl#265), so every `max_message_bytes` in the fixture is null.")                                                                                                |
| DD-58 (design note): the fixture's `max_message_bytes` is null; "a number invented for the test would state an overhead no binding document defines". A 2026-10-06 note (#736) records that a test now adds a `websocket` row.                                                                                                                                  | `docs/archive/2026-10-05-layout-inputs-design.md:946-956`                                                                                                                                                                                                   |

### 1.2 Decided but not built

| Decision                                                                                                                                                                                                                                                                                                                                | Status as written                       | Citation                                 |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------- | ---------------------------------------- |
| E11.9 is `ridl-transport-ws`: "binds E11.1's frame onto a socket"; Done when "a contract reaches a second process over the transport, and E11.15's loopback runs the same tests with no socket". Split from the loopback on 2026-09-20.                                                                                                 | ROADMAP row, no "landed" note for E11.9 | `docs/ROADMAP.md:476-490`                |
| ADR-0020 decision 6: `ridl-transport-ws` = "the ports over a WebSocket, proto3-framed; the default of the getting-started path"; depends on `ridl-rt` and a WebSocket crate. Generated Rust "never names a transport".                                                                                                                  | ADR-0020 Proposed — 2026-09-12          | `ADR-0020-...md:294-309`                 |
| ADR-0020 decision 7: the TypeScript WebSocket package is "the same binding for Deno and the browser"; the transport is "mandatory nowhere".                                                                                                                                                                                             | same                                    | `ADR-0020-...md:305, 310-334`            |
| ADR-0018 decision 7: one frame, several bindings; control plane uniform across Unix socket, Binder, WebSocket and wasm host; "an opaque payload in the encoding for that path".                                                                                                                                                         | ADR-0018 Proposed — 2026-08-09          | `ADR-0018-...md:275-284`                 |
| Frame spec §11.1: `ridl-transport-ws` "carries this frame; its path is 'serialized into a stream', so its payload encoding is **proto3** (encoding tag 2)". It "writes the nine items of §10 for a WebSocket and adds nothing to the frame". §11 opening: "The WebSocket transport is planned in this repository and is not built yet". | frame spec 0.1.0 Draft                  | `frame-specification.md:788-808`, `:792` |
| Frame spec §10: a binding states nine items (transport and session; header encoding; encoding tag; framing; caller identity; retention; grouping; mapped vs framed; rate-floor answer). "A fact a binding needs that this document does not state is a defect in this document, fixed here and not in the binding."                     | same                                    | `frame-specification.md:745-786`         |
| ADR-0021 decision 12 (one handle per port role; reader handles `Send + Sync`, others `Send`) is carried into E11.9's `Done when`.                                                                                                                                                                                                       | per #350 item 15                        | `gh issue view 350` body item 15         |

### 1.3 Open

| Issue                                                  | State | Milestone                                 | Labels                  | Waits on / blocks                                                                                                                                                                                                                                                                                                                                                                         |
| ------------------------------------------------------ | ----- | ----------------------------------------- | ----------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| #265 E11.9 ridl-transport-ws                           | OPEN  | none                                      | enhancement, P1, size/M | Waits on E11.1 (done). Blocks #718 (E17.3), which "Depends on E17.0 and E11.9 (#265)". Body is the 2026-08 filing ("Done when: a tool subscribes and renders a live signal") left as written; one comment (2026-09-20) records the split and says "the roadmap row ... is the current statement of scope". Body also says "until a Deno client can attach, every tool is a Rust program". |
| #718 E17.3 frame header and envelope sizes per binding | OPEN  | "E17 — layout inputs for backend plugins" | enhancement, P1, size/S | Depends on #265. No comments. Done when: "a socket message's maximum size is the frame header plus the envelope plus the payload bound, and a test checks that sum against a frame the WebSocket binding writes".                                                                                                                                                                         |
| #350 ridl-rt API questions                             | OPEN  | "E11 — Runtime Library"                   | question, P3, size/M    | Open items 5 and 17 name E11.9 (see section 8).                                                                                                                                                                                                                                                                                                                                           |
| #287 E12.1 TypeScript surface                          | OPEN  | none                                      | enhancement, P2, size/L | Body: "Builds on E10.9's TypeScript vocabulary and factories, and on E11.9's Deno client."                                                                                                                                                                                                                                                                                                |

### 1.4 Dependencies

- E11.9 depends on E11.1 (done) and, through frame spec §11.1, on the proto3
  encoding being the one its path carries (see section 8).
- #718 / E17.3 depends on E11.9.
- E12 (#287): body names E11.9's Deno client. ROADMAP's TypeScript row says the
  codec is reached through wasm (`ROADMAP.md:947`).
- Kotlin: ROADMAP says Kotlin depends on E4.5a, E4.5b and E11.1, "not on E11.9,
  the WebSocket transport"; "E11.9's WebSocket transport is not on Kotlin's
  path" (`docs/ROADMAP.md:1033-1050`). The Kotlin runtime owns its Binder
  binding (frame spec §11.2; #516).

---

### 2. Topic 2: the payload formats

### 2.1 What is built

| Encoding                               | Built                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | Evidence                                                                                                                                                                                                                                             |
| -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **FlatBuffers**                        | Projection: ADR-0019 (Accepted) with `crates/ridl-ir/src/projection/flatbuffers.rs` and `projection/size/flatbuffers.rs`; schema emitter `crates/ridl-backend-flatbuffers/src/lib.rs`; generated codec `crates/ridl-backend-rust/src/codec.rs`; runtime reader/builder `crates/ridl-rt/src/flatbuffers.rs`. E11.7 "Done when is met, and every decision of its design note is built". Conformance round trip against `planus`: `crates/ridl-backend-rust/tests/flatbuffers_conformance.rs`. The generated face runs over this codec (D-11). | `docs/design/flatbuffers-codec.md:1-40` ("Every decision of that note is built"); `docs/ROADMAP.md:755-760`                                                                                                                                          |
| **proto3 (schema only)**               | Projection: ADR-0017 (Accepted) with `crates/ridl-ir/src/projection/proto3.rs`; schema emitter `crates/ridl-backend-proto/src/lib.rs` (E9.8). Size sizer `projection/size/proto3.rs`. Validity oracle `protox` (dev-dependency of the proto backend). **No generated proto3 codec**: the Rust backend "emits no proto3 codec".                                                                                                                                                                                                              | `docs/design/interaction-face.md:95` ("emits no proto3 codec, and the `repr_c` column is `None`"); `crates/ridl-backend-rust/src/descriptors.rs:128`; `crates/ridl-backend-proto/Cargo.toml:12,20`                                                   |
| **`repr(C)` (marker only)**            | `Encoding::ReprC`, `ReprC` marker type and the `repr-c` feature exist. The sizer answers `Absent(EncodingUndefined)` for `ReprC`. The descriptor's `repr_c` column is `None`. The IR enum value is 3. No layout, no codec, no header emitter. The Rust backend still emits `#[repr(C)]` on `fixed_layout` structs (ADR-0020 decision 3 retires that in E11.12).                                                                                                                                                                             | `crates/ridl-ir/src/projection/size.rs:183-189, 253-266` ("`ReprC` is absent with [`AbsentCause::EncodingUndefined`]"); `crates/ridl-ir/src/codegen/tests.rs:2591`; `crates/ridl-rt/src/encoding.rs:73,96-99`; `docs/design/interaction-face.md:897` |
| Size states                            | `PayloadSizes` carries proto3 and FlatBuffers states, none for `repr(C)` (landed in #717, per #726 body). Descriptor-side: `EncodedSizes.proto3` is `None` in the Rust backend's output.                                                                                                                                                                                                                                                                                                                                                    | `gh issue view 726` body; `docs/design/catalog-descriptor.md:257`                                                                                                                                                                                    |
| Per-link encoding in the codegen model | Derived from crossing; test `a_same_machine_link_is_flatbuffers_and_a_different_machine_link_is_proto3`                                                                                                                                                                                                                                                                                                                                                                                                                                     | `crates/ridl-ir/src/codegen/deployment.rs:853`; `docs/design/codegen-plugins.md:379-393` ("No link carries the `repr(C)` encoding")                                                                                                                  |

### 2.2 Decided but not built

| Decision                                                                                                                                                                                                                                                                                                                                       | Status as written                | Citation                              |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------- | ------------------------------------- |
| ADR-0018 decision 3: proto3 on the network, FlatBuffers in memory. Amended in place by ADR-0020 decisions 1-2 (three encodings, wasm boundary divided).                                                                                                                                                                                        | ADR-0018 Proposed — 2026-08-09   | `ADR-0018-...md:134-161`              |
| ADR-0018 decision 4: the codec is generated, "`--emit rust --wire proto` produces code with no external crates"; one obligation: "byte-level conformance against a `protoc`-generated implementation, covering packed repeated scalars, canonical field ordering and malformed-input robustness".                                              | same                             | `ADR-0018-...md:162-179`              |
| ADR-0020 decision 1: `repr(C)` is a third encoding, three artifacts from one IR (layout struct, C header, codec). Decision 3: domain types do not carry `#[repr(C)]`; "the removal lands with roadmap story E11.12". Decision 4: the layout rules "belong in a projection record, written when the backend is", in the shape of ADR-0017/0019. | ADR-0020 Proposed — 2026-09-12   | `ADR-0020-...md:110-135, 168-198`     |
| E11.8: proto3 payload codec plus byte-level conformance; reads an absent non-optional scalar as 0 (#472).                                                                                                                                                                                                                                      | ROADMAP row, not landed          | `docs/ROADMAP.md:752`                 |
| E11.12: `repr(C)` payload codec; "a payload round-trips through the layout struct, the emitted header compiles as C, and no generated domain struct" carries `#[repr(C)]`. "E11.12 is a new identifier ... Its layout rules ... are its prerequisite".                                                                                         | ROADMAP row, not landed          | `docs/ROADMAP.md:740-753`             |
| `--wire` flag belongs with the proto3 codec (#264).                                                                                                                                                                                                                                                                                            | open question in the face record | `docs/design/interaction-face.md:804` |
| The Rust codegen section's exit criteria name all three codecs and "byte-level conformance against a `protoc`-generated implementation for proto3".                                                                                                                                                                                            | milestone text                   | `docs/ROADMAP.md:732-738`             |
| Sequence: "E11.7 FlatBuffers → E4.5a → E4.5b → E11.8 proto3 · E11.12 repr(C)"; changed 2026-09-22 so the plugin protocol runs ahead of E11.8, E11.12 and E12 (lane P driver D-P1).                                                                                                                                                             | ROADMAP                          | `docs/ROADMAP.md:234-235, 262-267`    |

### 2.3 Open

| Issue                                                         | State | Milestone                      | Labels                                | What it waits on                                                                                                                                                                                                                                                                                                                                                               |
| ------------------------------------------------------------- | ----- | ------------------------------ | ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| #264 E11.8 proto3 codec + conformance                         | OPEN  | none                           | enhancement, P1, size/L               | No comments. Sequenced after E4.5a/E4.5b (both landed per ROADMAP 977, 999). Body: "The codec being generated rather than delegated to `prost` is the decision that makes the conformance test mandatory".                                                                                                                                                                     |
| #317 E11.12 `repr(C)` codec                                   | OPEN  | none                           | enhancement, P1, size/L               | No comments. Prerequisite: "the `repr(C)` projection record — the fixed-capacity layout of a bounded string, optional and collection; string capacity and terminator; alignment; endianness — written when this backend is". Also removes shipped `#[repr(C)]` on domain structs. Epic text in body reads "E11 — the runtime library and the frame (Rust codegen, finalized)". |
| #726 `repr(C)` size row and per-link encoding key             | OPEN  | none                           | enhancement, P3, size/M               | Waits on #317. Body: "no link can select `repr(C)`"; needs a `PayloadSizes` `repr(C)` row and a backend key per link.                                                                                                                                                                                                                                                          |
| #727 per-member / per-link sizing grain                       | OPEN  | none                           | none                                  | Placeholder: "should be acted on when a real deployment records the requirement, not before". Not specific to a codec.                                                                                                                                                                                                                                                         |
| #725 `codegen-request` emit                                   | OPEN  | none                           | none                                  | Plugin-author tooling; no payload-format content beyond the request.                                                                                                                                                                                                                                                                                                           |
| #398 string-projection sentence owed by ADR-0017 and ADR-0019 | OPEN  | none                           | documentation, P3, size/S             | Docs only. The `repr(C)` half "belongs to the `repr(C)` projection record" (E11.12).                                                                                                                                                                                                                                                                                           |
| #690 shared proto3 refusal rules                              | OPEN  | "E16 — the catalog descriptor" | debt                                  | Move refusal rules into `ridl_ir::projection::proto3` so backend and sizer share them; add proto3 agreement test. Comment (2026-10-04) adds a rustdoc item about `size_state`. Note: it names `crates/ridl-descriptor/src/size/proto3.rs`, which on origin/main is `crates/ridl-ir/src/projection/size/proto3.rs` (path drift in the issue body).                              |
| #486 `Event` derives nothing; stale `Provider` doc            | OPEN  | none                           | documentation, enhancement, P3, ready | Generated-surface items in `crates/ridl-backend-rust/src/face.rs`; touches payload types' derives, no codec dependency. Ruled by lane G on 2026-10-09 (R-7, delegated by the maintainer): derive `Debug` on `Event` always, and `Clone, Copy, PartialEq, Eq` when every payload derives them.                                                                                  |
| #487 package path; face buffers sized by `MAX_SIZE`           | OPEN  | none                           | documentation, question, P3, ready    | Item 2: stack buffers `[0u8; <T as Payload<Wire>>::MAX_SIZE]`; scales with the payload bound of the active codec. Ruled by lane G on 2026-10-09 (R-8): no crate-root re-export for a single-package workspace; the `MAX_SIZE` stack buffers are recorded as a known property.                                                                                                  |
| #350 ridl-rt API questions                                    | OPEN  | "E11 — Runtime Library"        | question, P3                          | Open items 8-14 touching codecs: see section 8.                                                                                                                                                                                                                                                                                                                                |

### 2.4 Dependencies

- E11.8 and E11.12 follow E4.5a/E4.5b (landed) by the 2026-09-22 reorder.
- #726 waits on #317. E17 `repr(C)` column "stays empty until E11.12"
  (`ROADMAP.md:351-353, 385-387`).
- E12.1 (#287): "there is no second codec ... reaches the Rust codec through
  wasm for the bytes". Under ADR-0020 decision 2 that wasm codec is FlatBuffers;
  E12 does not need E11.8 for the wasm codec by that decision, but a TypeScript
  client on a WebSocket carries proto3 (frame spec §11.1, "so the two ends of
  one socket may be a Rust runtime and a TypeScript one").
- Kotlin plugin (`driftsys/ridlc-gen-kotlin`): the ridl repo says only that it
  depends on E4.5a, E4.5b and E11.1, that its Binder binding is its own, and
  that it "mirrors every item one wave behind" lane F (`ROADMAP.md:620-622`,
  `:1033-1053`). Kotlin heads-ups #9/#10/#12/#13 on that repo are outside this
  recap.

---

### 3. Dependency edges between the two topics

| Edge                                                                                                                                                                                                                                                       | Evidence                                                                                                                                       |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| E11.9's payload encoding is proto3 (tag 2), by the frame spec and ADR-0020 decision 6 ("proto3-framed").                                                                                                                                                   | `frame-specification.md:801-802`; `ADR-0020-...md:303`                                                                                         |
| E11.9's Done when does not mention a codec: "a contract reaches a second process over the transport, and E11.15's loopback runs the same tests". The frame carries "bytes in the session's encoding" and a port "carries bytes and names no payload type". | `ROADMAP.md:490`; `frame-specification.md` §4 table (`payload` row), `:665-667`                                                                |
| #718 needs the proto3 _size state_, not the proto3 _codec_: `max_message_bytes` = frame header + envelope + the proto3 bound; "Temperature has no proto3 bound, so the sum has no value".                                                                  | `crates/ridlc/tests/layout.rs:357-389`; ROADMAP E17.3/E17.5 notes ("the layout proof's messages carry the proto3 bound only", commit caba1732) |
| The frame header/envelope sizes come from the binding document, which E11.9 writes.                                                                                                                                                                        | `frame-specification.md:764-766` (§10 item 2); `bindings.rs:26-28`                                                                             |
| `repr(C)` is on no WebSocket path (stream = proto3), and no link can select it today.                                                                                                                                                                      | `ADR-0020-...md:142`; D-4                                                                                                                      |
| E11.14 left a codec gap: the face runs over FlatBuffers only; `--wire` waits for proto3.                                                                                                                                                                   | `interaction-face.md:95, 804`                                                                                                                  |

---

### 4. Drift

| #  | Left side                                                                                                                                                | Right side                                                                                                                                                                                                                                                                             |
| -- | -------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1  | Local `main` (`fac5abce`) lacks `bindings.rs`, `layout.rs`, the layout-inputs archive and the E17 landed text.                                           | `origin/main` (`7fe24437`) has them. A reader of the local checkout sees a different repository state.                                                                                                                                                                                 |
| 2  | ROADMAP row E11.1: "a logical frame with one binding per encoding" (`ROADMAP.md:489`).                                                                   | Frame spec: "there is one binding per transport" (`frame-specification.md:63`); #257 comment: "the document has one binding per transport ... not one binding per encoding; the row predates that decision". The comment records it; the ROADMAP row and issue title were not changed. |
| 3  | #350 title: "thirteen API questions".                                                                                                                    | Body numbers 17 questions (1-17; 14 is the last in the original list, 15-17 added 2026-09-20). Unchecked in the body now: 5, 12, 13, 14, 17 (five).                                                                                                                                    |
| 4  | #350 item 17 unchecked: "Recorded open on 2026-09-20 ... a frame and transport question".                                                                | ADR-0021 line 1115: "Closed 2026-09-26 by decision 13, the `Wakeable` port extension"; frame spec §13: "`port::Wakeable` (ADR-0021 decision 13, which closed its open item 6)". ROADMAP E11.16 is the keyed `Wakeable` row.                                                            |
| 5  | #350 comment of 2026-09-20 lists items 15 and 16 as unchecked "Proposed in #429".                                                                        | The body marks 15 and 16 checked ("Decided on 2026-09-20 ... ADR-0021 decision 12 / 11"). The comment is a dated record.                                                                                                                                                               |
| 6  | #265 body: "Done when: a tool subscribes and renders a live signal"; links ADR-0018 as "Proposed, in PR #241".                                           | ROADMAP row 490 and the split comment: Done when is the second-process round trip plus the loopback running the same tests. The comment says the roadmap row is current.                                                                                                               |
| 7  | #317 body header: "Epic: E11 — the runtime library and the frame (Rust codegen, finalized)". #264 body header: "E11 — `ridl-rt`, the runtime core (V1)". | Milestone list names the milestone "E11 — Runtime Library" (#10). Neither issue carries it.                                                                                                                                                                                            |
| 8  | `bindings.rs:26-27` says no binding document "states a frame layout yet (driftsys/ridl#265)".                                                            | Frame spec §4 states the logical fields (e.g. `Envelope`: `stamp: i64`, `seq: u64`) and §4 line 226 "places no bound on a header's size"; byte layout is §10 item 2, left to each binding. Consistent, but the table's "frame header" has no source until E11.9 writes one.            |
| 9  | #690 names `crates/ridl-descriptor/src/size/proto3.rs`.                                                                                                  | On origin/main the sizers are `crates/ridl-ir/src/projection/size/proto3.rs` and `flatbuffers.rs` (no `size` dir under `ridl-descriptor/src`).                                                                                                                                         |
| 10 | ADR-0018 decision 4: `--emit rust --wire proto` "produces code with no external crates".                                                                 | No `--wire` flag exists; `interaction-face.md:804` places it with #264. Not a contradiction, a pending item.                                                                                                                                                                           |
| 11 | AGENTS.md (origin/main) describes 20 crates and says epic E3 is "sequenced in the roadmap".                                                              | Not verified further; outside both topics.                                                                                                                                                                                                                                             |

---

### 5. Which design doc holds D-4

D-4 appears in the archived
`docs/archive/2026-10-05-layout-inputs-design.md:171` ("The encoding of a
consumer link derives from its crossing"). The same rule is in the as-built
record `docs/design/codegen-plugins.md` ("The encoding rule", lines 442-458).
`docs/design/catalog-descriptor.md:363` also has a "D-4" but it is a different
decision ("What the catalog descriptor contains").

---

### 6. Verification of handoff facts

| #  | Handoff fact                                                                                                                                     | Verdict                                                                                                                                                                                                                                                               | Evidence                                                                            |
| -- | ------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| 1  | Frame spec landed: E11.1, #257, closed 2026-09-23; file `docs/specification/frame-specification.md`; book chapter `docs/book/reference/frame.md` | **Confirmed** for date, issue and spec. Book chapter path **unverifiable** here (not opened).                                                                                                                                                                         | `gh issue view 257` closedAt 2026-09-23T19:33:36Z; spec 943 lines, `:9` Draft 0.1.0 |
| 2a | E11.9 (#265) open, no milestone                                                                                                                  | **Confirmed**                                                                                                                                                                                                                                                         | `gh issue view 265`: OPEN, milestone null, labels enhancement/P1/size/M             |
| 2b | It blocks #718                                                                                                                                   | **Confirmed**                                                                                                                                                                                                                                                         | #718 body: "Depends on: E17.0 and E11.9 (#265...)"                                  |
| 2c | Table lives in `crates/ridl-ir/src/codegen/bindings.rs`                                                                                          | **Confirmed on origin/main** (absent on local main)                                                                                                                                                                                                                   | `bindings.rs:26-28`                                                                 |
| 2d | Until the row exists `max_message_bytes` is null                                                                                                 | **Confirmed**                                                                                                                                                                                                                                                         | `layout.rs:334-337`                                                                 |
| 2e | `layout.rs` tests the sum with a synthetic 14 + 2 row                                                                                            | **Confirmed** (test adds the row; fixture keeps null)                                                                                                                                                                                                                 | `layout.rs:357-389`                                                                 |
| 2f | DD-58 in `docs/archive/2026-10-05-layout-inputs-design.md`                                                                                       | **Confirmed**, with a 2026-10-06 (#736) note that a test now adds a `websocket` row                                                                                                                                                                                   | `:946-956`                                                                          |
| 3a | FlatBuffers projection rules are ADR-0019                                                                                                        | **Confirmed** (Accepted — 2026-08-09)                                                                                                                                                                                                                                 | `ADR-0019-...md:5`                                                                  |
| 3b | FlatBuffers codec E11.7 landed (`docs/design/flatbuffers-codec.md`)                                                                              | **Confirmed**                                                                                                                                                                                                                                                         | `flatbuffers-codec.md:1-40`; `ROADMAP.md:755`                                       |
| 4a | proto3 projection rules are ADR-0017                                                                                                             | **Confirmed** (Accepted — 2026-08-08)                                                                                                                                                                                                                                 | `ADR-0017-...md:5`                                                                  |
| 4b | proto3 codec E11.8 is #264, open, no milestone                                                                                                   | **Confirmed**                                                                                                                                                                                                                                                         | `gh issue view 264`                                                                 |
| 4c | It adds byte-level conformance against a `protoc`-generated implementation                                                                       | **Confirmed**                                                                                                                                                                                                                                                         | `ROADMAP.md:752`; ADR-0018 decision 4                                               |
| 5a | `repr(C)` codec E11.12 is #317, open, no milestone                                                                                               | **Confirmed**                                                                                                                                                                                                                                                         | `gh issue view 317`                                                                 |
| 5b | Layout rules not decided; projection record of the shape of ADR-0017/0019, written with the backend                                              | **Confirmed**                                                                                                                                                                                                                                                         | ADR-0020 decision 4 (`:189-198`); #317 body                                         |
| 5c | #726 waits on that record; today no channel gets `repr(C)` and no size state exists for it                                                       | **Confirmed**                                                                                                                                                                                                                                                         | #726 body; `size.rs:253-266`; `codegen-plugins.md:379-393`                          |
| 6  | Encoding per link depends on its crossing: same machine = FlatBuffers; the other two = proto3 (D-4)                                              | **Confirmed**; D-4 is in the archived layout-inputs design (not a live design doc)                                                                                                                                                                                    | `layout-inputs-design.md:171-179`                                                   |
| 7  | #350 lists thirteen `ridl-rt` API questions                                                                                                      | **Refuted**: the title says thirteen; the body lists 17 questions, five unchecked (5, 12, 13, 14, 17)                                                                                                                                                                 | `gh issue view 350`                                                                 |
| 8  | ROADMAP sequence: E11.0, E11.1, E11.9; codecs E11.7, then E4.5a, E4.5b, then E11.8 and E11.12; E12 reaches its codec through the wasm build      | **Confirmed** as two chains; E12 reaches the codec through "the package's own generated Rust compiled to `wasm32` against `ridl-rt`" (not "the wasm build of the runtime library" alone; ADR-0020 decision 7 says a `ridl-rt`-only wasm build "could encode nothing") | `ROADMAP.md:234-241, 262-267, 929`                                                  |
| 9a | #398 string projection sentence owed by ADR-0017/0019                                                                                            | **Confirmed**                                                                                                                                                                                                                                                         | `gh issue view 398`                                                                 |
| 9b | #690 shared proto3 refusal rules                                                                                                                 | **Confirmed**                                                                                                                                                                                                                                                         | `gh issue view 690`                                                                 |
| 9c | #486 and #487 generated-crate observations about payload types and face buffers                                                                  | **Confirmed** (#486: `Event` derives and `Provider` doc about payload `Copy`/`Clone`; #487: package path and `MAX_SIZE` stack buffers; both ruled by lane G on 2026-10-09, R-7 and R-8, and labelled `ready`)                                                         | `gh issue view 486 / 487`                                                           |
| 9d | #725 codegen-request emit for plugin authors                                                                                                     | **Confirmed**                                                                                                                                                                                                                                                         | `gh issue view 725`                                                                 |
| 9e | #727 per-link sizing grain                                                                                                                       | **Confirmed** (per-member and per-link)                                                                                                                                                                                                                               | `gh issue view 727`                                                                 |

---

### 7. Inputs to the steering questions (evidence only)

### 7.1 Does E11.9 need E11.8, or can the WebSocket carry opaque payload bytes?

- Frame spec: the frame's `payload` is "bytes in the session's encoding" (§4
  table). A port "carries bytes and names no payload type, so verification is
  the generated binding's on both sides" (`frame-specification.md:665-667`).
  ADR-0018 decision 7: "an opaque payload in the encoding for that path"
  (`:275-277`).
- `ridl-rt` is sans-IO: "bytes in via ..." (ADR-0018 decision 2, `:119`);
  `port.rs` takes `bytes: &[u8]` (`docs/design/ridl-rt.md:505, 522`).
- A session fixes one encoding named at `attach`; `attach` can be refused
  `encoding_unsupported` (`frame-specification.md:506, 147-161`).
- The WebSocket's path is "proto3 (encoding tag 2)" (`:801-802`); ADR-0020
  decision 6 says "proto3-framed" (`:303`).
- E11.9's Done when names no codec (`ROADMAP.md:490`). The only existing
  generated codec is FlatBuffers (tag 1), and the face is bound to it
  (`interaction-face.md:95`).
- #265's body: "a tool subscribes and renders a live signal" needs a payload
  encoding to render a value.
- Frame spec §3: the `attach` frame carries the encoding tag, so a binding could
  in principle name tag 1 on a stream; the matrix puts a WebSocket on proto3,
  which the spec calls fixed "before the session exists" (`:151`).

### 7.2 Are header size and envelope size per version fixed by the frame spec, or left to E11.9?

- Not fixed. "This document places no bound on a header's size"
  (`frame-specification.md:226`).
- Byte layout, field order and width of every header field are item 2 of what a
  binding states (`:764-766`). A fact a binding needs that the spec lacks "is a
  defect in this document, fixed here and not in the binding" (`:751-753`).
- The spec fixes the logical fields, e.g. `envelope`: `stamp: i64`, `seq: u64`
  (§4 table), and `frame_version` = `u32`, "**1** here" (`:495`). No per-version
  byte size appears.
- The binding table's schema has `frame_header_max_bytes` and `envelope_bytes`
  plus `version` (`bindings.rs:10-21`); no row. `bindings.rs:26-27` assigns the
  source to a binding document (#265).

### 7.3 Is `repr(C)` in step 1's release scope per the ROADMAP?

- Yes. Milestone summary row: "none | Rust codegen finalized | **step 1** — the
  three payload codecs, byte-conformant" (`ROADMAP.md:1104`). Epic 11 is step 1
  (`:1101`). The "Rust codegen, finalized" milestone lists the `repr(C)` round
  trip in its exit criteria (`:732-738`). The step-1 narrative: "the Rust
  codegen finalized with its three payload codecs, proto3, FlatBuffers and
  `repr(C)`" (`:216-218`).
- Sequence note: E11.8 and E11.12 run after E4.5a/E4.5b (`:262-267`).

### 7.4 What does E11.8's protoc conformance require of the gate?

- `protoc` appears nowhere in `justfile`, `.github/`, `bootstrap`,
  `rust-toolchain.toml` or ADR-0009 (grep over origin/main returned no hit).
  `./bootstrap` checks `just`, `rustup`, `mdbook` and installs git-std and prim
  and the pinned Rust toolchain (`bootstrap:21-60`; ADR-0009 decision 12,
  `:251-262`).
- ADR-0006 decision 3: "`protox` instead of a system `protoc`" with the stated
  gain "hermetic builds (no protoc on contributor machines or CI images)"
  (`docs/decisions/ADR-0006-*.md:46-49, 90`). `crates/ridl-ir/build.rs:5-8`:
  "the build needs no system `protoc` binary".
- Existing oracles are pure Rust: `protox` is a dev-dependency of
  `ridl-backend-proto` (`crates/ridl-backend-proto/Cargo.toml:12,20`);
  `planus-translation` for FlatBuffers, guarded by
  `xtask/tests/oracle_boundary.rs` ("fails when a planus crate is reachable from
  `ridl-rt` with every feature on", ADR-0020 decision 5). `prost`/`prost-build`
  pins are already in `Cargo.toml:106-107`.
- ADR-0018 decision 4: "our codec and a `protoc`-generated consumer interoperate
  without sharing a library"; the test covers "packed repeated scalars,
  canonical field ordering and malformed-input robustness" (`:162-179`). #264
  body: "nothing but a byte-level test against one proves it".
- The FlatBuffers analogue (E11.7) ran conformance against `planus`, a round
  trip through an independent implementation (`ROADMAP.md:755-760`).
- Gate rule: every gate command is a `just` recipe, CI only installs tools and
  calls recipes (`AGENTS.md` "The justfile is the single definition of every
  gate command"; ADR-0009). A new tool requirement would enter `bootstrap`'s
  named tools and the CI workflow's install steps (ADR-0009 decision 12).

### 7.5 Milestones, and which of #265/#264/#317 could belong to one

Milestones (`gh api repos/driftsys/ridl/milestones`):

| #  | Title                                   | State | Open |
| -- | --------------------------------------- | ----- | ---- |
| 1  | E0 — Walking Skeleton                   | open  | 0    |
| 2  | E1 — typl + Tooling Spine               | open  | 6    |
| 3  | E2 — ridl (Interface Layer)             | open  | 12   |
| 4  | E3 — uxdl (User Interface)              | open  | 0    |
| 5  | E4 — Ecosystem & Adoption (V1)          | open  | 2    |
| 7  | E6 — rsdl, rewritten as a language      | open  | 0    |
| 8  | E7 — rxdl & V2 Ecosystem                | open  | 1    |
| 9  | E8 — Agent Enablement                   | open  | 7    |
| 10 | E11 — Runtime Library                   | open  | 7    |
| 11 | E15 — interface identity and the lock   | open  | 0    |
| 12 | E16 — the catalog descriptor            | open  | 8    |
| 13 | E17 — layout inputs for backend plugins | open  | 2    |

- #265, #264 and #317 have no milestone. The E11 milestone's 7 open issues are
  #704, #607, #571, #526, #354, #350, #349, none of the three stories. On
  2026-10-09 the E17 milestone's 2 open issues are #737 and #718.
- Of the three, #265 is the one #718 (in E17) depends on, so it could belong to
  E17 or E11. #264 and #317 are E11 stories by their identifiers (E11.8,
  E11.12); ROADMAP line 162 says E11.16-E11.21 (#510-#515) are filed "under the
  E11 milestone". There is no milestone named for the Rust codegen finalization;
  the ROADMAP gives that section no epic number (`ROADMAP.md:1113`: "Two rows
  have no epic").

### 7.6 #350 questions that touch the transport or the codecs

State from the issue body (live). Items marked [x] are decided and recorded; [ ]
are open.

| #    | State                      | Touches                       | One line                                                                                                                                              |
| ---- | -------------------------- | ----------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1    | [x]                        | transport (E11.9)             | `read_coherent` with a short `samples` slice; decided #351: `ReadError::TooFewSamples { needed }`.                                                    |
| 3    | [x]                        | transport (E11.1, E11.9)      | `Transport` made `#[non_exhaustive]`.                                                                                                                 |
| 5    | **[ ] open**               | transport (E11.9)             | A call lost in transport with no response bound has no outcome (ridl §9.3 allows no bound).                                                           |
| 6    | [x]                        | transport (E11.9)             | `ScannableSignals::scan` with fewer slots than changes: written all together or not at all.                                                           |
| 7    | [x]                        | transport (E11.9)             | `invalidate` and `touch` return `Result<(), WriteError>`.                                                                                             |
| 8    | [x]                        | codecs (E11.7, E11.8, E11.12) | `Ref::encode` does not check typl constraints; documented on `Ref::encode` and spec R-8.                                                              |
| 9    | [x]                        | transport (E11.1, E11.9)      | `Handler::settle` takes `Result<&[u8], CallError>`; `Caller::ack` lists `Transport::Corrupt`.                                                         |
| 10   | [x]                        | codecs (E11.7, E11.8, E11.12) | `Rule` gains `Step`, loses `Invariant`.                                                                                                               |
| 11   | [x]                        | transport (E11.9)             | `Caller::reply` documents its two error layers.                                                                                                       |
| 12   | **[ ] open**               | transport/frame (E11.1)       | `Violation.type_name` is `&'static str`, so a `no_std` runtime cannot fill it from a remote peer's error, unless the frame carries a type identifier. |
| 13   | **[ ] open**               | codecs (Rust codegen, #324)   | `PayloadInfo.max_size` repeats `Payload::MAX_SIZE`; nothing checks they agree.                                                                        |
| 14   | **[ ] open**               | codecs (E11.7, E11.8)         | `Rule::Step` on a float wire form (proto3, FlatBuffers) needs a tolerance; exact `(value - min) / step` rejects 25.3 on step 0.1.                     |
| 15   | [x]                        | transport (E11.9)             | One handle per port role; reader handles `Send + Sync`, others `Send` (ADR-0021 decision 12).                                                         |
| 16   | [x]                        | transport (E11.9)             | Port traits forwarded through `&mut P` and `&P` (ADR-0021 decision 11).                                                                               |
| 17   | **[ ] open** (see drift 4) | transport (E11.1, E11.9)      | No port notifies that a reply, occurrence or claim arrived; ADR-0021 line 1115 says `Wakeable` closed it on 2026-09-26.                               |
| 2, 4 | [x]                        | codegen / crate README        | not transport or codec (read vs read_fixed naming; struct-literal breakage).                                                                          |

Open items touching transport or codecs: 5, 12, 13, 14, 17.
