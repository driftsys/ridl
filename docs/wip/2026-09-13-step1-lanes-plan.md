# Step 1 lanes — ridl-rt, rsdl, the lock, and the typl debt

Status: working memory, 2026-09-13. Coordinates the first part of roadmap step 1
as four lanes that run in parallel, each driven by its own session. Each lane
has a driver prompt beside this file. Archive this file and the four driver
prompts to `docs/archive/` when the last lane lands
(`sdd-working-memory-lifecycle`).

This file does not decide language or runtime design. Each lane writes its own
spec under `docs/wip/`, and the spec decides. This file decides the order, the
parallelism, the model used for each stage, and which lane may change a shared
file when.

## 1. Scope

In scope, in the order Sebastien asked for it:

1. **`ridl-rt` 0.1.0 built and published** — roadmap story E11.0 (#316).
2. **rsdl finalized, with its reference** — roadmap Epic 6.
3. **The typl debt** — E14.1 (#318), the Rust half of Epic 10, and the typl
   defects listed in §4.

Added, because items 2 and 3 depend on it: **the lock block** — the per-package
lock file of `docs/wip/2026-09-12-rsdl-rewrite-decisions.md` D-7.

Not in scope: E11.1 (the frame specification), E11.9 (the transport), the three
payload codecs E11.7, E11.8 and E11.12, the execution of the catalog descriptor
plan (#324), the Epic 3 thread, E9.10, E9.12, Epic 8, and all of step 2. Lane A
and lane L each name the next story they hand over to.

## 2. Decisions taken in the planning session

Agreed with Sebastien on 2026-09-13.

- **P-1 "Publish `ridl-rt`" means a crates.io release of 0.1.0.** The agent
  builds E11.0 and prepares the release. Sebastien runs `cargo publish`, because
  release, tagging and publishing are maintainer acts
  (`docs/decisions/ADR-0007-e1-execution.md:156`). The release waits for two
  things: E11.0's done-when (a hand-written program links the crate and reads a
  sample with its provenance), and lane L's approved identity widths (P-2). 0.1
  leaves out `Inline` payloads (the note's RA-X7, which depends on typl §17.11)
  and streams (RA-X1). Every other crate stays at version 0.0.0. `ridl-rt` gets
  its own version and its own tag.
- **P-2 Lane L decides the identity widths, and lane A uses them.** The widths
  are open and the records disagree: the `ridl-rt` note proposes `u16` for
  `Ordinal`, `InterfaceId` and `ServiceId`
  (`docs/wip/2026-09-08-ridl-rt-design.md:205-208`), the IR carries every
  ordinal as `uint32` (`crates/ridl-ir/proto/ridl/ir/v2/ir.proto:87`), and D-7
  and the runtime descriptors design state no width. D-7 owns numbering, so the
  first section of lane L's design fixes the widths. Lane A writes the rest of
  its spec in the meantime and does not fix its identity types until Sebastien
  has approved that section.
- **P-3 `ridl-rt` runs beside rsdl and the lock, not after rsdl.** The roadmap's
  step 1 sequence puts E6 before E11.0. E11.0 needs only the identity widths
  from D-7, and nothing from the rsdl language. The roadmap pull request (lane
  L, stage L3) records the new order.
- **P-4 The lock keeps its priority over the rsdl lowering and #324.** Sebastien
  decided this on 2026-09-13, and the reason still holds: the catalog hash
  includes each number and its provisional flag, and the rsdl lowering carries
  that hash. Anything that writes a hash or a golden file before the lock lands
  would be written twice.
- **P-5 Lane C takes #243 and #237 from "Rust codegen, finalized".** Epic 10
  Tasks 3 and 6 rewrite the same struct and union emission, so the snapshots
  change once instead of twice. #302 stays where it is, because it is a wire
  discriminant question, not a naming question. The roadmap pull request records
  the move.
- **P-6 E14.2 is the last stage of lane L, and E14.3 is the last item of step
  1.** The lock retires RIDL-146, RIDL-147 and RIDL-148 and amends ADR-0015, and
  several ridl §17 questions rest on those. E14.3 is one small edit once E14.1
  and E14.2 have both merged.
- **P-7 One driver session per lane, at most three lanes waiting for Sebastien
  at a time.** Lanes A, B and L start now. Lane C starts when gate G1 or G2
  holds (§5), which is when one of the two sessions already running finishes.

### Alternatives considered

- **One driver session for all four lanes.** Rejected. Every request re-sends
  the whole conversation, so the cost grows with the number of turns times the
  context size, and the four lanes wait for Sebastien at different times. A
  driver per lane keeps each context to one subject and makes each stage
  boundary a place where a session can end.
- **The roadmap's serial order, rsdl then `ridl-rt`.** Rejected by P-3: the only
  dependency is the identity widths.
- **Publish `ridl-rt` only after the Rust codegen links it.** Rejected. A 0.x
  version permits a breaking 0.2, which the codegen work is expected to cause.
  Step 2 needs a published crate (ADR-0020 decision 7, at
  `docs/decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md:191`),
  and an early release gets feedback from outside the repository earlier.
- **E14.2 in lane C.** Rejected by P-6.

## 3. Sessions already running

Two sessions were running when this plan was written. No lane enters their
worktrees, checks out their branches, or runs a formatter in their directories.

| Session          | Branch and worktree                                                                                         | Files it changes                                                                                                                                                                                                                 |
| ---------------- | ----------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| S1 tooling       | `feat/ridl-mcp-v0`, `.claude/worktrees/feat+ridl-mcp-v0`, PR #327                                           | `crates/ridl`, `ridl-core`, `ridl-lsp`, the new `ridl-mcp`, `ridlc`, `editors/vscode`, `Cargo.toml`, `Cargo.lock`, `.git-std.toml`, `justfile`, `.github/`, `AGENTS.md`, `README.md`, `CONTRIBUTING.md`, `docs/ROADMAP.md`, ADRs |
| S2 baseline gate | `baseline-tombstone-gate`, `.claude/worktrees/baseline-tombstone-gate`; two pull requests (its design, D-6) | `crates/ridl/src/main.rs`, `ridl-core/src/diag.rs` (RIDL-408), `ridl-diff` (`MemberReordered`), ADR-0010, the ridl reference, `docs/book/cli-reference.md`                                                                       |

S2's design and plan are on its local branch and not on `main`. A lane reads
them with
`git show baseline-tombstone-gate:docs/wip/2026-09-13-baseline-gate-design.md`,
never by checking the branch out.

## 4. The lanes

Each stage ends with a pull request. The model named is the one that does the
stage's main work. The driver of every lane is Opus.

### Lane A — `ridl-rt` 0.1.0

Driver prompt: `2026-09-13-lane-a-ridl-rt-driver.md`. Story: #316.

| Stage | Work                                                                                                                                                                                                                                                           | Model                                                                 | Starts when       |
| ----- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------- | ----------------- |
| A1    | Spec for `ridl-rt` 0.1: the six modules, the identity types (widths from P-2), the envelope with #308, provenance with #309, `Payload<E>`, the interaction descriptors, the ports, a disposition for each RA-X item, `no_std` and `wasm32`, the release policy | Fable                                                                 | now               |
| A2    | Implementation plan from the approved spec                                                                                                                                                                                                                     | Opus                                                                  | A1 merged         |
| A3    | The crate `crates/ridl-rt`, test first                                                                                                                                                                                                                         | Sonnet for plain types; Fable for descriptors, ports and verification | A2 merged, G2, GW |
| A4    | Release preparation: metadata, a `cargo publish --dry-run` check, the tag scheme, a maintainer checklist                                                                                                                                                       | Sonnet                                                                | A3 merged         |

Hands over to E11.1 (#257).

### Lane B — rsdl and its reference

Driver prompt: `2026-09-13-lane-b-rsdl-driver.md`.

| Stage | Work                                                                                                                                                                             | Model                                                               | Starts when   |
| ----- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------- | ------------- |
| B1    | Rewrite `docs/specification/rsdl-language-reference.md` from the decisions note D-1 to D-11, and apply the note's §4 amendments that are not about identity (those go to lane L) | Fable                                                               | now           |
| B2    | Tracker pass: close the old Epic 6 story issues and file the new ones; the Epic 6 roadmap table; the implementation plan                                                         | Sonnet for the tracker, Opus for the plan                           | B1 merged, G2 |
| B3    | Parse and check `.rsdl`, with editor support                                                                                                                                     | Fable for grammar and semantics, Opus for the LSP and the extension | B2 merged, G2 |
| B4    | Lowering to the IR: region map, link set, routing table, permission list, surface set, catalog hash; the book chapter, written as built                                          | Fable                                                               | B3 merged, G4 |

The book chapter waits for B4, because the book describes the system as built
(`AGENTS.md`). The reference reaches the book through its `{{#include}}` wrapper
and needs no chapter of its own.

### Lane L — the lock block

Driver prompt: `2026-09-13-lane-l-lock-driver.md`.

| Stage | Work                                                                                                                                                                                                                                 | Model                                                                                            | Starts when   |
| ----- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------ | ------------- |
| L1    | Design: the identity widths first (P-2), then the lock file, `ridl lock`, provisional numbering, `ridl lock merge`, the baseline refusal at the interface level, the retirement of RIDL-146 to RIDL-148, and the ADR-0015 amendments | Fable                                                                                            | now           |
| L2    | Implementation plan                                                                                                                                                                                                                  | Fable                                                                                            | L1 merged     |
| L3    | Roadmap pull request: P-3 and P-5, rows for the lock block and the catalog descriptor, and their story issues                                                                                                                        | Opus for the roadmap, Sonnet for the issues                                                      | L2 merged, G2 |
| L4    | Implementation                                                                                                                                                                                                                       | Fable for numbering, merge and diff; Opus for the command; Sonnet for sweeping the retired codes | L2 merged, G1 |
| L5    | E14.2 (#319), the ridl §17 disposition pass                                                                                                                                                                                          | Fable drafts, Sebastien decides                                                                  | L4 merged     |

Hands over to the execution of #324, after Sebastien confirms the seven
dispositions that plan takes.

### Lane C — the typl debt

Driver prompt: `2026-09-13-lane-c-typl-driver.md`. Stories: #318, #246 to #255.

| Stage | Work                                                                                                                                                                                         | Model                                                                                       | Starts when   |
| ----- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- | ------------- |
| C1    | E14.1: one disposition per typl §17 question (13 items) and for the two rows the re-scope added (§3.10, §3.11 of the release-scope note), §17.11 first, including the decision #245 asks for | Fable drafts, Sebastien decides                                                             | G1 or G2      |
| C2    | Bring `docs/wip/typl-value-objects-plan.md` up to date with the code: Task 9 (TypeScript) moves to step 2, and a task is added for #243 and #237                                             | Sonnet checks each reference, Opus edits                                                    | G1 or G2      |
| C3    | Defects #244 and #203; #245 once C1 has decided it                                                                                                                                           | Sonnet for #244, Opus for #203                                                              | G1            |
| C4    | Epic 10 Tasks 1 to 8 and 10, plus the #243 and #237 task                                                                                                                                     | Fable for Tasks 3 and 6 and the naming task; Opus for 1, 7, 8 and 10; Sonnet for 2, 4 and 5 | C2 merged, G2 |

E14.3 (#320) follows C1 and L5.

## 5. Gates

A gate is a fact a driver checks with a command before starting the stage that
waits on it.

| Gate | Holds when                                      | Check                                                                                                                                                 |
| ---- | ----------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| G1   | Both S2 pull requests have merged               | `git fetch origin && git grep -q RIDL-408 origin/main -- crates/ridl-core/src/diag.rs && git grep -q MemberReordered origin/main -- crates/ridl-diff` |
| G2   | S1 has merged                                   | `gh pr view 327 --json state --jq .state` prints `MERGED`                                                                                             |
| GW   | Sebastien has approved lane L's identity widths | a comment from lane L on #328 that states the widths and says they are approved                                                                       |
| G4   | Lane L's implementation has merged              | the comment from lane L on #328 that closes stage L4                                                                                                  |

**The coordination issue, #328,** holds one comment per stage boundary from each
lane: the stage, the pull request, and anything another lane must know. Its body
is never edited after it is created, because the GitHub API replaces a body
wholesale and two sessions editing it would overwrite each other.

## 6. Which lane may change a shared file

Only one open pull request at a time changes each of these files. The order
below is the expected merge order, not a queue: a lane may go before its turn
when no other open pull request changes the file, and it says so on the
coordination issue before it pushes.

| File                                                     | Order                                                 |
| -------------------------------------------------------- | ----------------------------------------------------- |
| `crates/ridlc/src/lib.rs` (the `Emit` enum)              | S1 → C4 Task 7 → #324 → B4                            |
| `crates/ridl-core/src/diag.rs`                           | S2 → L4 → C3 → B3                                     |
| `crates/ridl-diff/`                                      | S2 → L4                                               |
| `crates/ridl/src/main.rs`                                | S1 and S2 → L4 (`ridl lock`) → #324 (`ridl describe`) |
| `Cargo.toml`, `Cargo.lock`, `.git-std.toml`, `AGENTS.md` | S1 → A3 (the new crate) → the next new crate          |
| `docs/ROADMAP.md`                                        | S1 → L3 → B2                                          |
| `docs/specification/ridl-language-reference.md`          | S2 → B1 (§4 census items) → L4 → L5                   |
| `docs/specification/typl-language-reference.md`          | C1 → C4 Task 10                                       |
| `docs/book/cli-reference.md`                             | S2 → L4 → #324                                        |

## 7. Rules every lane follows

The driver prompts repeat these, because a session started from a prompt does
not see this planning conversation.

- **Isolate first.** The lane works in its own worktree under
  `.claude/worktrees/` and runs `./bootstrap` there. Run
  `git branch --show-current` before every commit and every push. Before a
  rebase, compare `git ls-remote origin <branch>` with the local head and read
  `git reflog -8`.
- **Review, then merge.** Open the pull request, run `/review <PR>` (the
  docs-only lane when no executable line changes), post the ledger as a pull
  request comment, fix what is kept, run pass 2, run `just verify`. After that
  the driver may squash-merge; Sebastien has given that permission for reviewed
  and verified work. Then update the local `main` to `origin/main`.
- **Never** push a tag, publish to a registry, add a secret, or push to `main`.
- **Formatting and commits.** `just fmt` before a Markdown commit,
  `cargo fmt --all` before a Rust commit, Conventional Commits with a type and a
  scope from `.git-std.toml`. A new crate adds its own scope.
- **Prose** is plain and literal. Never name a private or consumer project in a
  repository file.
- **The tracker mirrors the roadmap.** Closing an issue never rewrites its body.
  Explanations go in a comment. Before retitling a story issue, check
  `docs/archive/roadmap-landed-record.md` for a parked row with that identifier.
- **End the session at each stage boundary.** The handoff is the spec or plan
  under `docs/wip/` plus the comment on the coordination issue, not the
  conversation.
- **Garden when the work lands.** The lane archives its own spec and plan with
  `sdd-gardening` in the pull request that completes the last of its stages.
