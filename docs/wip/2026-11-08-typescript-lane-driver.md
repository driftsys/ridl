# TypeScript lane driver — finish the TypeScript surface of step 2

Status: driver, 2026-11-08. Three parallel tracks, one task at a time inside
each track. This document is written for an agent with no prior context, of any
vendor. Where it says "stop", stop and report to Sebastien; do not guess past
it. Where this document and an ADR disagree, the ADR wins. This document
summarizes; it does not decide.

**THIS SESSION RUNS:** set this line to one task id (for example `A1`) before a
session starts, and do only that task.

## 0. Why this lane exists

The exit criteria of step 2 (`docs/ROADMAP.md`, "The TypeScript framework") are:
a Deno program constructs and validates a payload without a TypeScript codec,
and an emulator written in TypeScript stands in for a provider without the
consumer distinguishing it.

The first criterion is met. #287 (E12.1) was closed on 2026-11-08 after an
audit: `encode<Name>`, the wasm codec and `verify` construct and validate a
payload, and the cabin and docstore demos run it. The audit found three things
that are still missing:

- The generated module does not check a typl constraint at construction
  (ADR-0013 decision 8, #923).
- Commands, queries and signals have a codec table but no generated typed face,
  and the runtime package holds none of the TypeScript port interfaces that
  ADR-0020 decision 7 names (#1262).
- A single-component package imports its domain module and the flatc output from
  the same path (#1252).

The emulator (#290), the test-harness drivers (#291) and the observability
server (#288) would each rewrite the same call-and-decode code without the
faces, so the faces come first.

## 1. Tracks and tasks

| Track     | Task | Issue | What                                                                     | Depends on |
| --------- | ---- | ----- | ------------------------------------------------------------------------ | ---------- |
| Backend   | A1   | #1252 | Repair the imports of a single-component package                         | none       |
| Backend   | A2   | #923  | A checking constructor and an unchecked one; the decoder uses the latter | A1         |
| Backend   | A3   | #1197 | Export the instance constants in the TypeScript codec module             | A2         |
| Design    | B0   | #1262 | Settle how a generated face reaches a transport; amend ADR-0020 d7       | none       |
| Design    | B1   | #399  | The runtime package rejects a lone surrogate before the codec sees it    | none       |
| Faces     | B2   | #1262 | The typed client face for commands, queries and signals                  | A1, A2, B0 |
| Transport | C1   | #994  | A bound or an `AbortSignal` on connect, subscribe and read               | none       |
| Transport | C2   | #1236 | A signal check, and keep the last good value                             | C1         |
| Transport | C3   | #1242 | Test gaps in the parameter stream paths                                  | none       |
| Transport | C4   | #1246 | The live-signal browser test that timed out once                         | none       |
| Transport | C5   | #989  | Run `examples/cabin` over the transport                                  | none       |
| Transport | C6   | #992  | Publish `@driftsys/ridl-transport-ws` to JSR                             | none       |
| Transport | C7   | #995  | Verify the checksum of the Deno release zip in bootstrap and CI          | none       |
| Consumers | D1   | #290  | The emulator                                                             | B2, C1     |
| Consumers | D2   | #291  | Test-harness drivers                                                     | D1         |
| Consumers | D3   | #288  | The observability server                                                 | B2, C1     |

Debt that fits the backend track and is taken after A3 if time allows: #1028
(keep the wasm rule list in one place) and #1029 (round-trip the codec fixture
through its own wasm build).

**Why the tracks can run in parallel.** The backend track edits
`crates/ridl-backend-ts` and regenerates its fixtures and the freeze dumps, so
its tasks are sequential and no other track touches those files. The transport
track edits `packages/ridl-transport-ws` only. The design track edits documents,
and #399 edits `packages/ridl-runtime`.

**Where the tracks conflict.** B2 generates code into the module that A1 and A2
change, so it waits for both. If B0 decides that the generated faces use shared
port interfaces in `@driftsys/ridl-runtime`, B2 also changes
`packages/ridl-transport-ws`, which overlaps with C1 and C2 in `session.ts`. In
that case C1 and C2 land before B2 starts. If B0 decides on a small structural
interface, as `StreamTransport` is for stream functions, there is no overlap.

## 2. Open decisions

These belong to Sebastien. A task that needs one stops and reports.

1. **B0, the transport boundary of a face.** Option 1 defines port interfaces in
   `@driftsys/ridl-runtime` (call, wait for an outcome, subscribe, settle) and
   makes the faces and the transport use them; it also allows an in-process
   TypeScript loopback for tests and for the emulator. Option 2 gives the faces
   a small structural interface. Option 2 is smaller and matches what stream
   functions do today.
2. **A2, the constructor shape.** The design note settles the shape, how it
   relates to the range check of the wasm codec, and whether the pattern check
   is optional. ADR-0013 decision 8 already says a pattern check may be
   optional.
3. **The release.** A1 to A3 and B2 are probably breaking for generated
   TypeScript. The lane assumes one breaking release for A1 to A3, and a second
   one for B2. Sebastien confirms before a release.
4. **The scope of the lane.** The lane assumes D1 is in scope. D2 and D3 stay in
   scope only if Sebastien says so.

## 3. Roles and models

| Role                                      | Model                                                       |
| ----------------------------------------- | ----------------------------------------------------------- |
| Driver of a track                         | Sonnet; Opus (high) for A2 and B2                           |
| Brainstorm and specification (A2, B0, B2) | Opus (high); Sonnet for the small specifications of track C |
| Implementation                            | Codex `gpt-6.1-sol`, high, through `codex exec`             |
| Task reviewer, one per task               | A Claude subagent on Sonnet                                 |
| Refuter of the task reviewer's findings   | Codex `gpt-6.1-sol`                                         |
| Branch review, once after the last task   | The `/review` passes                                        |

No model runs at xhigh. Nothing in this lane needs Fable.

## 4. How to work

- Read `AGENTS.md` first. Each task names the records it touches: for the
  backend, `docs/design/ts-codec-table.md`, ADR-0013 decision 8 and ADR-0020
  decision 7; for the transport, `docs/design/ridl-transport-ws-ts.md`.
- One git worktree per track, branched from `origin/main`, with its own
  `CARGO_TARGET_DIR`. Run `./bootstrap` in it. Never use `git stash`: the stash
  is shared by every worktree.
- One pull request per task. Stage explicit paths. Run
  `git branch
  --show-current` before every commit and push. Run
  `cargo fmt --all` before a Rust commit and `just fmt` before a Markdown, TOML
  or YAML commit.
- Conventional Commits, with scopes from `.git-std.toml`.
- An implementer runs the focused tests of its crate or package only. The driver
  runs the workspace tests before a push. CI runs the full gates.
- A Codex implementer cannot commit and cannot bind a TCP port. The driver
  commits for it and runs `just ts-e2e` and the other socket tests itself.
- Tests that change generated output: set `RIDL_UPDATE_GENERATED=1` for the
  regenerated fixtures, `INSTA_UPDATE=always` for the `ridlc` corpus snapshots,
  and run `cargo xtask freeze update` when a freeze dump changes. Read the diff
  of each regenerated file before accepting it.
- Shipped docs describe the system as built. A task that changes behaviour
  updates the record that owns it in the same pull request, and does not name a
  story or an issue number in a book, design or rustdoc text unless it links a
  tracking issue for a real gap.

## 5. The per-task review gate

The review of task N finishes, and its verdicts are read, before the implementer
of task N+1 starts.

1. The driver records the base commit before the implementer starts.
2. The driver gives the reviewer the diff as a file, computed from that base.
3. The Claude reviewer returns two verdicts: the task meets its brief, and the
   code is sound. It checks every documentation statement that the diff adds,
   changes or makes false against the code, and it mutates the production code
   to see whether each new test fails. Its mutation work runs in its own
   worktree with its own `CARGO_TARGET_DIR`.
4. The Codex refuter tries to disprove each finding and scores it.
5. The implementer's own report is evidence, not the review.

The Codex review does not replace the Claude reviewer. In a seeded check on
2026-11-04, Codex missed an older sentence that a change had made false, and the
Claude reviewer found it.

After the last task of a track, the driver runs the branch review once.

## 6. Exit criteria

- A1, A2 and A3 are merged, and the decisions of section 2 are recorded in the
  records that own them.
- B2 is merged, and a Deno program sends a command, calls a query and reads a
  signal of the cabin package through generated typed functions, with no
  hand-written `encode`, `decode` or `Kind` handling.
- D1 is merged, and a consumer cannot tell the emulator from the real provider.
- `just build` passes on main.
- This document is moved to `docs/archive/`, and the durable records are
  gardened, before the last pull request lands.
