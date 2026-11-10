# K2 lane driver — fix the major defects from the 2026-11-09 triage

Status: driver, 2026-11-09. This document is written for an agent with no prior
context. Where it says "stop", stop and report to the main session; do not guess
past it. Where this document and an ADR disagree, the ADR wins. This document
sequences work; it does not decide product questions.

K2 means: wrong behaviour in a reachable case, or a gate that misses defects.

## 1. Tasks

One driver per issue, one pull request each. T1, T3, T5 and T6 run in parallel.
T2 starts after T1 merges, because both change clause translation. T4 and T7
start after T3 merges, because they touch the same diff and history code. T7 is
#926: the main session ruled option 2 on the owner's behalf. A widened
constraint is breaking when the type reaches a consumer as a received value, and
compatible where the consumer only sends it.

| Task | Issue | Size | Driver | What                                                                                        | Notes                                                                        |
| ---- | ----- | ---- | ------ | ------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| T1   | #1221 | S    | Sonnet | A `require` beside a parameter stream must not remove the interface's face                  | `crates/ridl-ir/src/codegen/clauses.rs`                                      |
| T2   | #1092 | S    | Sonnet | Round a float clause literal toward the side that refuses, at generation                    | Breaking: commit type carries `!`. ROADMAP Epic 18 states the direction      |
| T3   | #1229 | M    | Sonnet | Report a strict enum append in a remote package reached through a pin change                | Read ADR-0029 if a frozen dump changes                                       |
| T4   | #1228 | M    | Sonnet | Treat missing remote evidence as "cannot judge" for FlatBuffers and proto3 catalog history  | Rulings R-2 and R-3 of #1211 bound `ridl diff`; only catalog history changes |
| T5   | #1144 | M    | Sonnet | Item 1 only: totality tests for the FlatBuffers codec, descriptor verifier, `frame::decode` | Item 2 (cargo-fuzz) stays open. A panic found here is fixed in the same task |
| T6   | #1170 | M    | Opus   | A stream port signal for a forgotten reply stream                                           | Changes the `ridl-rt` port. Read ADR-0021 and `docs/design/ridl-rt.md`       |

Not in this lane:

- #1280: another session owns it (PR #1285).

## 2. How a task runs

Each task has one driver agent (model in the table). The driver does not write
the code.

1. **Worktree.** `git worktree add -b <branch> ../ridl-k2-t<N> origin/main`,
   then `./bootstrap` there. Use a separate `CARGO_TARGET_DIR` for the task.
2. **Pre-flight.** Read the issue with its comments, the code, and the records
   `AGENTS.md` names for that area. Reproduce the defect. If the issue is
   already fixed, or the fix needs a decision no record makes, stop.
3. **Implement with Codex.** One `codex exec` run per task, in the background:

   ```sh
   CARGO_TARGET_DIR=<target> RUSTC_WRAPPER= codex exec -m gpt-6.1-sol \
     -c model_reasoning_effort='"high"' -s workspace-write -C <worktree> \
     --add-dir <target> -o <out> - < <prompt>
   ```

   The prompt names the base commit, the issue, the required test-first order
   (red by an assertion, not by a compile error), the mutation each test must
   catch, and the prose rules of `AGENTS.md`. Codex writes its report to
   `.superpowers/codex/k2-t<N>-report.md`. The sandbox cannot commit and cannot
   bind TCP: Codex proposes the commit message, and the driver commits and runs
   TCP-dependent tests outside the sandbox.
4. **Focused gates.** The driver runs tests of the touched crates with
   `--no-fail-fast`, `xtask` included when a generated output or a rule table
   changes, plus `just fmt-check`, crate-scoped clippy and
   `just story-id-check`. Update a freeze dump with `cargo xtask freeze update`
   when it changes. CI runs the full gates.
5. **Per-task gate, before the pull request.** Give the seats the diff as a file
   computed from the recorded base.
   - A Claude Sonnet task reviewer (`Agent`, `model: "sonnet"`). Two verdicts:
     the task meets its brief; the code is sound. It checks every doc statement
     the diff adds, changes or makes false against the code, and mutates.
   - A Codex seat, medium effort, with
     `~/.claude/review-prompts/codex-seat-b.md`.
   - Every mutating seat works in **its own detached worktree with its own
     `CARGO_TARGET_DIR`**, and never mutates the reviewed tree.
   - Fix rounds are Codex runs, followed by a scoped re-review.
6. **Pull request.** Push, open a non-draft PR with `Closes #<issue>`. Grep the
   body for closing keywords on any other number first. Check that the PR is
   mergeable and that the full CI set ran: a conflicting PR shows only CodeQL.
7. **Review.** Run `/review` passes 1 and 2 through `review-driver` agents, as
   `~/.claude/commands/review.md` describes. Seat B and the refuters run on
   Codex. The compliance, tests and docs seats run on Sonnet. Fix Critical and
   Important findings with Codex. File the surviving Minors in one debt issue.
8. **Merge.** The owner delegated this on 2026-11-09. Load the `driftsys-admin`
   skill first. Merge with a squash when CI is green on the final head, both
   review passes are recorded, every Critical and Important finding is fixed,
   and every other surviving finding is fixed or filed in a debt issue. The
   driver decides on the owner's behalf and records each ruling in section 4. In
   case of doubt, stop and report the question with a recommendation to the main
   session. Doubt includes a change to a frozen surface that no record permits,
   or a reversal of an accepted ADR.

## 3. Rules

- Never name this lane, its tasks or the issues' story ids in shipped docs,
  rustdoc or source comments. Link `driftsys/ridl#N` when a gap is real.
- Conventional Commits, scopes from `.git-std.toml`, plain literal English.
- Never `git stash` in this repository: the stash is shared across worktrees.
- Before rebasing, check the remote and the reflog for another session's work.
- Do not touch the worktrees `ridl-1280` or `ridl-ts-*`.
- The disk is at 87 %. Delete a task's target directories when the task ends.
- Record every ruling you make in section 4, with its reason.

## 4. Log

| Date       | Task | Event                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ---------- | ---- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 2026-11-09 | T5   | PR #1287 opened (test-only, `Refs #1144`). Codex found no panic or wrong accept; mutation checks fail each new test. Task review (Sonnet) and Seat B (Codex) ran; a fix round added the exact re-encode assertion and fixed one doc sentence. Review passes 1 and 2: Minors only, filed as #1293. CI green on head 9a17b181.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| 2026-11-09 | T5   | Ruling: Minors go to debt #1293 (no Critical or Important). Merge blocked: the PreToolUse hook of driftsys-admin refuses the admin merge in this agent context even after the skill loaded. Not bypassed. The main session must merge #1287.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| 2026-11-09 | T1   | Ruling: the issue asks that a broken `require` settle `ContractBroken`, but docs/design/interaction-face.md (consumer face, settlement table) and ADR-0023 decision 1 assign `PreconditionFailed` to a failed `require` and `ContractBroken` to a failed `ensure`. The regression asserts `ClientError::Send(SendError::Contract(PreconditionFailed))`; no record is changed. Reason: a record covers it, so it is not a product decision. Codex stopped at pre-flight and asked; fix run resumes.                                                                                                                                                                                                                                                                                                                                                                                                                   |
| 2026-11-09 | T1   | PR #1289 head 8251f121: per-task gate passed (Sonnet reviewer: meets brief, sound, 4 of 4 mutations caught; Codex Seat B: no findings). Pass 1 (HEAD 03e39a28): 2 Minors, fixed (technote sentence, stream-only test). Pass 2: 2 Minors filed as debt #1294; 1 Important dropped (refuted 20). CI failure from the shape-walk line table fixed in 8251f121; CI green. Ruling: owner message relayed by the coordinator said merge; a later coordinator message says a hook refuses merge from a subagent, so the PR is left ready for the main session.                                                                                                                                                                                                                                                                                                                                                              |
| 2026-11-09 | T3   | PR #1288 opened (head 872a57be, merged main later as b622cd14). Codex implemented in the remote pass of `remote_inline.rs`. Task gate: Sonnet reviewer (meets brief yes, code sound yes), Codex Seat B (1 Minor, fixed). Pass 1 ledger `~/.claude/review-ledgers/driftsys-ridl/pr1288-pass1.md`: 1 Important (width lines beside a remote strict append) fixed. Pass 2 (comment on the PR): 1 Important (no test of strict append beside inline growth) fixed. Debt #1291.                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| 2026-11-09 | T3   | Ruling: a remote strict append replaces the width lines for the same declaration, as the local rule does (ADR-0017 and ADR-0019 say so). Ruling: pass 2 Minors and pass 1 Minors filed in #1291, not fixed. Ruling: CI failed once on `xtask` shape_walk (two new `.interfaces` fixture lines); table bumped from 15 to 17.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| 2026-11-09 | T3   | Merge not run by the driver: the coordinator ruled that the main session merges. CI green on b622cd14; main moved by 2 more commits after that, so the main session must update the branch before merging. An auto-mode denial blocked `reset --hard` and a force push; a plain `git merge` of main was used instead.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| 2026-11-09 | T6   | Ruling: the port gets two provided methods and a three-valued enum (`claim_forget`, `wake_on_claim_forget`, `ClaimForget`), not a new `Interest` key, because `Interest` is exhaustive and a new key breaks existing runtimes. Three answers, because a command's caller forgets normally after its acknowledgment and the end of its sink. Recorded in `docs/design/ridl-rt.md`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| 2026-11-09 | T6   | PR #1292. Task review passed both verdicts; Seat B found a busy loop on a normal command forget, fixed. Pass 1: 4 Important fixed. Pass 2: Minors only, filed as debt. Ruling: the ROADMAP's "one provided method" is left to the release notes and the Kotlin heads-up. Coordinator instruction: the main session merges, not this driver.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| 2026-11-09 | T6   | #1292 replaced by #1298 (head e686a8ff, CI green): the pre-commit hook of git-std removes MERGE_HEAD, so a merge commit fails, and a force push was refused. Ready to merge by the main session; main moved one commit after the last CI. Debt #1297.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| 2026-11-09 | T4   | PR #1300 (`fix(ridl)!:`), head 78249984. Codex implemented; Sonnet task reviewer (meets brief, sound; mutations caught except two Minors, tests added) and Codex Seat B (2 Minors, fixed). Pass 1: Minors only, fixed in a fix round. Pass 2: 1 Important (stability.md claimed the ADR-0029 minor-release approval covers the new failure behavior; confirmed 88) fixed by removing the claim; Minors filed as #1304.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| 2026-11-09 | T4   | Ruling: the filter keeps only `Missing` issues for flatbuffers/proto3 without podbuf (Unlayable and KindChanged are podbuf layout facts). Ruling: PR title carries `!` because `ridl baseline` now exits 2 for flatbuffers/proto3 workspaces with missing evidence. Open question, not decided: ADR-0029 treats failure behavior as policy-frozen and approves only the #1163 changes; the owner should confirm this extension (recommendation: approve, issue #1228 is the owner's own and asks for it) and the release notes must state it.                                                                                                                                                                                                                                                                                                                                                                        |
| 2026-11-09 | T2   | PR #1301 (`Closes #1092`), branch k2-t2-float-clause-round. Rounding lives in the Rust backend printer, not the model. Rulings: (1) strict comparisons round the other way from a plain "toward refusal" reading (`<` and `>=` use the bound at or above, `<=` and `>` at or below) because that equals the exact rule of expr-core section 7 and still never accepts a refused value; (2) an inexact float literal on `==`/`!=`, an unreadable or oversized exponent, and a bound that needs infinity are refused (interface skipped with a note); (3) a local copy of the exact decimal reader in `clauses.rs`, because main moved the original into `ridl-ir` as a private function and a public one would change a frozen API; (4) the stray rewrite of the getting-started paragraph by Codex was reverted as out of scope.                                                                                     |
| 2026-11-09 | T2   | Per-task gate: Sonnet reviewer (meets brief, sound, all semantic mutations killed) and Codex Seat B (2 doc Minors, fixed). Rebase over #1288/#1290/#1295/#1296. Pass 1 (HEAD 39097db4): 1 Important (generated copies of the changed note stale, `just freeze-check` red; the refuter did not return, accepted on CI evidence), fixed by regenerating the veh-cluster crate and two snapshots; 11 Minors. Pass 2: 4 Minors, 2 doc ones fixed, 2 test gaps filed. Debt issue #1307.                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| 2026-11-09 | T7   | PR #1302 (head 2745606f), `Closes #926`. Ruling relayed on #926. Rulings: an unreached type is breaking (fail closed, as the strict enum rule); the remote pass reports widening and narrowing by the same direction rule at the consumer path, and a remote scalar width or backing change as `width_changed` (beyond the literal ruling: needed so a remote widening that flips the wire width is not silently compatible); ADR-0008 d14 and ADR-0029 amended (owner approval, minor release only, no dump row removed, freeze check clean). Task gate: Sonnet reviewer (16 mutations, 2 test gaps) + Codex Seat B (2 Important: renamed interface, remote width) fixed. Pass 1: 1 Important (remote width rows outside the recorded ruling) kept as an open question, 1 Important test gap fixed; Minors fixed or filed in #1308. Pass 2: same width Important kept, other Minors in #1308. CI green on 2745606f. |
| 2026-11-09 | T7   | Open question for the main session: the remote narrowing and remote `width_changed` rows extend the ruling (which names widening). Recommendation: keep them; if the owner objects, revert commit 73b8ca6e's ADR wording and remove the two row kinds from `remote_inline.rs`. Merge not run by the driver.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
