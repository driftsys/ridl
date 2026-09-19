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

Added on 2026-09-16, after the fact: **the generated interaction face**, roadmap
story E11.13 (#393), as Lane M. It is not one of the four items Sebastien asked
for and it is out of ADR-0018 decision 15's sequence; it is here because the
team needs a face to write against before E11.1 and E11.9 land, and because the
face binds only the `ridl-rt` ports that item 1 shipped.

Not in scope: E11.1 (the frame specification), E11.9 (the transport), the three
payload codecs E11.7, E11.8 and E11.12, the execution of the catalog descriptor
plan (#324), the Epic 3 thread, E9.10, E9.12, Epic 8, and all of step 2. Lane M
stands in for the codecs and the transport with placeholders it marks as
throwaway, rather than pulling any of them in. Lane A and lane L each name the
next story they hand over to.

## 2. Decisions taken in the planning session

Agreed with Sebastien on 2026-09-13.

- **P-1 "Publish `ridl-rt`" means a crates.io release of 0.1.0.** The agent
  builds E11.0 and prepares the release. Sebastien runs `cargo publish`, because
  release, tagging and publishing are maintainer acts
  (`docs/decisions/ADR-0007-e1-execution.md`, decision 14). The release waits
  for two things: E11.0's done-when (a hand-written program links the crate and
  reads a sample with its provenance), and lane L's approved identity widths
  (P-2). 0.1 leaves out `Inline` payloads (the note's RA-X7, which depends on
  typl §17.11) and streams (RA-X1). Every other crate stays at version 0.0.0.
  `ridl-rt` gets its own version and its own tag.
- **P-2 Lane L decides the identity widths, and lane A uses them.** The widths
  are open and the records disagree: the `ridl-rt` note proposes `u16` for
  `Ordinal`, `InterfaceId` and `ServiceId`
  (`docs/wip/2026-09-08-ridl-rt-design.md:205-208`), the IR carries every
  ordinal as `uint32` (`crates/ridl-ir/proto/ridl/ir/v2/ir.proto:105`, and again
  at `:269`, `:288` and `:345`), the catalog descriptor plan's schema already
  writes `uint32` for the ordinal and the interface number
  (`docs/wip/2026-09-13-catalog-descriptor-plan.md:296`, `:306`, `:319`), and
  D-7 and the runtime descriptors design state no width. If lane L chooses a
  different width, that plan's Task 1 schema changes with it. _Settled since
  this was written:_ Sebastien approved the widths on 2026-09-13, recorded in §1
  of `docs/archive/2026-09-13-lock-design.md` and in the lane L comment on #328
  that holds gate GW. The decision above stands as taken; the widths it left
  open are no longer open. D-7 owns numbering, so the first section of lane L's
  design fixes the widths. Lane A writes the rest of its spec in the meantime
  and does not fix its identity types until Sebastien has approved that section.
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
  Task 6 (sound derives) changes the struct and union declarations that #243 and
  #237 are about, and Task 3 changes the named-scalar emission in the same
  emitter, so the snapshots change once instead of twice. #302 stays where it
  is, because it is a wire discriminant question, not a naming question. The
  roadmap pull request records the move.
- **P-6 E14.2 is the last stage of lane L, and E14.3 is its own small pull
  request.** The lock retires RIDL-146, RIDL-147 and RIDL-148 and amends
  ADR-0015, and ridl §17 questions that cite ADR-0015 may change with it. E14.3
  changes the typl and ridl references, so it waits for every earlier change to
  them: C1 (E14.1), C4 Task 10 and L5 (E14.2). The lane that merges the last of
  those three opens it. The Rust codegen, finalized, still follows the typl debt
  in step 1, as the roadmap's sequence says.
- **P-7 One driver session per stage, at most three lanes waiting for Sebastien
  at a time.** A lane runs one stage at a time, with one exception: lane C's C1
  and C2 are independent and may run as two sessions at once, each in its own
  worktree. Lanes A, B and L start now. Lane C starts when gate G1 or G2 holds
  (§5), which is when one of the two sessions already running finishes.

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
  In step 2, the generated Rust of each package is compiled to `wasm32` and
  links `ridl-rt` (ADR-0020 decision 7, at
  `docs/decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md:191`).
  A consumer that builds that outside this repository needs the crate from a
  registry. That is this plan's inference, not a statement of ADR-0020. An early
  release also gets feedback from outside the repository earlier.
- **E14.2 in lane C.** Rejected by P-6.

## 3. Sessions already running

Two sessions were running when this plan was written. No lane enters their
worktrees, checks out their branches, or runs a formatter in their directories.
**Both have since merged** — S1 as #327, and S2 as #330
(`baseline-tombstone-gate`) and #331 (`member-reordered-category`) — so the
table below is a record of what each one changed, which is what the §6 orders
are built on, not a live warning. Gates G1 and G2 both hold. #331 also appended
items 14 and 15 to typl §17, which is why stage C1's table below has fifteen §17
rows and not thirteen.

| Session          | Branch and worktree                                                                                         | Files it changes                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| ---------------- | ----------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| S1 tooling       | `feat/ridl-mcp-v0`, `.claude/worktrees/feat+ridl-mcp-v0`, PR #327                                           | `crates/ridl` (including `src/main.rs` and `tests/`), `crates/ridl-core/src/diag.rs` and its snapshot, `ridl-lsp`, the new `ridl-mcp`, `crates/ridlc/src/lib.rs` and `tests/`, `editors/vscode`, `Cargo.toml`, `Cargo.lock`, `.git-std.toml`, `justfile`, `.github/workflows/`, `.gitignore`, `install.sh`, `install.ps1`, `AGENTS.md`, `README.md`, `CONTRIBUTING.md`, `docs/ROADMAP.md`, `docs/book/cli-reference.md`, `docs/technotes/`, `docs/archive/`, ADR-0005, ADR-0007, ADR-0010 |
| S2 baseline gate | `baseline-tombstone-gate`, `.claude/worktrees/baseline-tombstone-gate`; two pull requests (its design, D-6) | `crates/ridl/src/main.rs`, `crates/ridl/tests/` (`baseline_gate.rs` new, `baseline_desk.rs`), `crates/ridl-core/src/diag.rs` (RIDL-408), `crates/ridlc/tests/corpus.rs` (the RIDL-408 catalogue row), `ridl-diff` (`MemberReordered`), ADR-0010, the ridl reference (including new §17.12 and §17.13), `docs/specification/ridl-family-overview.md` (the open-question index), `docs/book/cli-reference.md`, `docs/wip/README.md`, `docs/archive/` (its own design and plan)              |

S2's design and plan reached `main` when S2 merged, and its branch,
`baseline-tombstone-gate`, has been deleted. Read them at
`docs/archive/2026-09-13-baseline-gate-design.md` (and `-plan.md`); the
`git show baseline-tombstone-gate:...` this section used to give no longer
resolves.

## 4. The lanes

Each stage ends with at least one pull request: C3 opens one per defect, and C4
one per Epic 10 story. The model named is the one that does the stage's main
work. The driver of every lane is Opus.

**Fable is unavailable until 2026-09-19, and every row below that names it reads
as Opus at high effort.** Where a row names Opus, that is Opus at medium effort.
Sonnet rows are unchanged. Recorded here on 2026-09-16, at Sebastien's
direction, because this table and the four driver prompts beside it were written
on 2026-09-13, before the outage, and a session reading them would otherwise ask
for a model it cannot run. `2026-09-15-lane-m-driver.md` §"Model routing" is
where the outage was first written down; this is the same fact, with the effort
levels named.

The distinction the effort levels keep is the one the table already draws: Fable
holds the design-sensitive half of a stage and Opus the rest — L4 reads "Fable
for numbering, merge and diff; Opus for the command", B3 "Fable for grammar and
semantics, Opus for the LSP". Collapsing both to one model without the effort
split would lose that.

The stages already run under the outage — A1, A3, B1, B3, B4, L1, L2, L4, L5 and
C1 — substituted without recording what they used. That is not recoverable from
the record, and it is not worth reconstructing; it is noted so a later reader
does not take those rows as evidence of what Fable produces.

If a stage has not started by 2026-09-19, prefer Fable for its touchy portion
instead — check with Sebastien before assuming the outage has lifted.

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

| Stage | Work                                                                                                                                                                                                                                 | Model                                                                                            | Starts when       |
| ----- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------ | ----------------- |
| L1    | Design: the identity widths first (P-2), then the lock file, `ridl lock`, provisional numbering, `ridl lock merge`, the baseline refusal at the interface level, the retirement of RIDL-146 to RIDL-148, and the ADR-0015 amendments | Fable                                                                                            | now               |
| L2    | Implementation plan                                                                                                                                                                                                                  | Fable                                                                                            | L1 merged         |
| L3    | Roadmap pull request: P-3 and P-5, rows for the lock block and the catalog descriptor, and their story issues                                                                                                                        | Opus for the roadmap, Sonnet for the issues                                                      | L2 merged, G2     |
| L4    | Implementation                                                                                                                                                                                                                       | Fable for numbering, merge and diff; Opus for the command; Sonnet for sweeping the retired codes | L2 merged, G1, G2 |
| L5    | E14.2 (#319), the ridl §17 disposition pass                                                                                                                                                                                          | Fable drafts, Sebastien decides                                                                  | L4 merged         |

Hands over to the execution of #324, after Sebastien confirms the seven
dispositions that plan takes.

### Lane C — the typl debt

Driver prompt: `2026-09-13-lane-c-typl-driver.md`. Stories: #318, #246 to #255.

| Stage | Work                                                                                                                                                                                                                                                              | Model                                                                                       | Starts when   |
| ----- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- | ------------- |
| C1    | E14.1: one disposition per typl §17 question (15 items; items 12 and 13 are the two rows the re-scope added, from §3.10 and §3.11 of the release-scope note, and items 14 and 15 are the two #331 added), §17.11 first, plus a row for the decision #245 asks for | Fable drafts, Sebastien decides                                                             | G1 or G2      |
| C2    | Bring `docs/wip/typl-value-objects-plan.md` up to date with the code: Task 9 (TypeScript) moves to step 2, and a task is added for #243 and #237                                                                                                                  | Sonnet checks each reference, Opus edits                                                    | G1 or G2      |
| C3    | Defects #244 and #203; #245 once C1 has decided it                                                                                                                                                                                                                | Sonnet for #244, Opus for #203 and #245                                                     | G1, G2        |
| C4    | Epic 10 Tasks 1 to 8 and 10, plus the #243 and #237 task                                                                                                                                                                                                          | Fable for Tasks 3 and 6 and the naming task; Opus for 1, 7, 8 and 10; Sonnet for 2, 4 and 5 | C2 merged, G2 |

E14.3 (#320: both references drop "Draft", and the rxdl reference gains its
status line) follows C1, C4 Task 10 and L5 (P-6).

### Lane M — the generated interaction face

Driver prompt: `2026-09-15-lane-m-driver.md`. Story: E11.13, #393.

**Added 2026-09-16, after the other four.** This lane was proposed on 2026-09-15
alongside this plan rather than in it, and M1 landed before it was listed here.
It is a deliberate exception to ADR-0018 decision 15's sequence: the generated
face is specified to follow E11.1 and E11.9, and this lane builds an
in-process-only MVP of it before either, so the team has a face to write
against. It can do that because the face binds the **ports** of `ridl-rt`, which
shipped with E11.0, and E11.1 and E11.9 sit below those ports.

| Stage | Work                                                                                                                                | Model                                                            | Starts when |
| ----- | ----------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------- | ----------- |
| M1    | The spec: `docs/archive/2026-09-16-interaction-face-v0-design.md` — scope, payload encoding, numbering, the placeholders, the shape | Opus                                                             | landed      |
| M2    | Implementation plan from the approved spec                                                                                          | Opus                                                             | M1 merged   |
| M3    | The emitter's `descriptors.rs` and `face.rs`, the example package, the round trip                                                   | Opus for `Client`/`Provider`/`dispatch`; Sonnet for the plumbing | M2 merged   |
| M4    | Progressive documentation of the `ridl-rt` traits, written against what M3 generates                                                | Opus                                                             | M3 merged   |

**No gate.** The lane's one hard dependency, `ridl-rt` 0.1.0 (E11.0), is merged,
and the IR numbering it needs landed with L4. It waits on nothing and blocks
nothing.

**One sequencing preference, not a gate.** M3 extends
`crates/ridl-backend-rust/src/lib.rs`, which Lane C's Epic 10 is reshaping
(P-5). The spec's §8 accepts the resulting rework rather than blocking on it,
but if Epic 10 is close to landing when M3 is ready, let it land first: the
rework then disappears and nothing in the spec changes.

M1 carries three placeholders that later stories retire — a hand-written payload
implementation (E11.7, E11.8 or E11.12), test-only ports (E11.9), and a zero
catalog hash (E16.2).

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
| `crates/ridl-core/src/diag.rs`                           | S1 and S2 → L4 → C3 → B3                              |
| `crates/ridl-diff/`                                      | S2 → L4                                               |
| `crates/ridl/src/main.rs`                                | S1 and S2 → L4 (`ridl lock`) → #324 (`ridl describe`) |
| `crates/ridl-backend-rust/src/lib.rs`                    | C4 → M3                                               |
| `Cargo.toml`, `Cargo.lock`, `.git-std.toml`, `AGENTS.md` | S1 → A3 (the new crate) → the next new crate          |
| `docs/ROADMAP.md`                                        | S1 → L3 → M1 → B2                                     |
| `docs/specification/ridl-language-reference.md`          | S2 → B1 (§4 census items) → L4 → L5 → E14.3           |
| `docs/specification/ridl-family-overview.md`             | S2 → B1 (§4 census items) → L5                        |
| `docs/specification/typl-language-reference.md`          | C1 → C4 Task 10 → E14.3                               |
| `docs/book/cli-reference.md`                             | S1 and S2 → L4 → #324                                 |
| `docs/decisions/ADR-0010-cli-conventions.md`             | S1 and S2 → L4 (`ridl lock`) → #324 (`ridl describe`) |

**`crates/ridl-backend-rust/src/lib.rs` was missing from this table** until
2026-09-16. It is the file Lane C's Epic 10 reshapes (Tasks 3 and 6, P-5) and
the one Lane M's codegen extends, so it was a collision no row covered. M3 goes
after C4 because Epic 10 rewrites the file's emit functions while M3 adds two
module declarations and one call, and the smaller change rebases onto the larger
one more easily than the reverse.

`docs/ROADMAP.md` records M1 where it actually went: it added the E11.13 row
ahead of B2, with no other pull request holding the file open, by this section's
own rule.

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
