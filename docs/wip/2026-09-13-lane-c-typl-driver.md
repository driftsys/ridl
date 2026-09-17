# Lane C driver — the typl debt

Transient working memory for `2026-09-13-step1-lanes-plan.md`. Start a fresh
session in `.claude/worktrees/lane-c-typl` and paste the block below as the
first message, once per stage: each stage is one session, and the session ends
when the stage's pull requests have merged. C1 and C2 may run as two sessions at
the same time. Archive this file with the lanes plan.

## Model routing

**Fable is unavailable until 2026-09-19.** Read every "Fable" below as Opus at
high effort, and every "Opus" as Opus at medium effort; Sonnet is unchanged. The
escalation on a failed fix loop becomes Opus at high effort. Recorded 2026-09-16
at Sebastien's direction; the authoritative statement is §4 of
`2026-09-13-step1-lanes-plan.md`.

- Driver: Opus.
- C1 E14.1: Fable drafts the disposition table; Sebastien decides.
- C2 plan refresh: Sonnet checks every file, line and code reference in the plan
  against `origin/main` and reports what no longer matches; Opus applies the
  changes. Fable designs the added task for #237.
- C3 defects: Sonnet for #244; Opus for #203 and #245.
- C4 Epic 10: Fable for Tasks 3 and 6 and the naming task; Opus for Tasks 1, 7,
  8 and 10; Sonnet for Tasks 2, 4 and 5. Opus for the second-stage review. Move
  an implementer to Fable after one failed fix loop.

## Stages already run

C1, C2 and C3 have merged; **C4 is the only stage left.** C1 closed E14.1 (#318)
and discharged #245 with no code. C2 refreshed the value-objects plan and was
followed by #404, which moved the constraint error to
`ridl_rt::payload::Violation` and closed that plan's Open item 3. C3 closed #244
and #203, minting TYPL-215, RIDL-413 and TYPL-010.

**C4 is no longer blocked.** Decision D — what a colliding union arm does — was
taken on 2026-09-17: report a diagnostic under RIDL-149 over both pinned
transforms, move `camel_case` into `ridl-ir`, no rename. It was the last thing
gating Task 11's #237 half, and every C4 task is now runnable.

**C4 has its own driver prompt**,
[`2026-09-16-lane-c-c4-driver.md`](2026-09-16-lane-c-c4-driver.md). Use it
rather than the C4 line in the routing above and the `== C4 ==` block in the
prompt below. C4 runs as four sessions instead of one, because it is ten tasks
with a pull request each, and under the review and model routing Sebastien set
on 2026-09-16. The C1 to C3 material below stands as the record of what ran.

## The prompt

```text
You drive lane C of docs/wip/2026-09-13-step1-lanes-plan.md: the typl debt —
E14.1 (#318), the Rust half of Epic 10 (#246 to #255), and the defects #243,
#237, #244, #203 and #245. Read AGENTS.md, then that plan's §2 (P-5, P-6,
P-7), §4 lane C, §5, §6 and §7. The plan's rules bind this session.

Lane C starts when G1 or G2 holds (plan §5). Check first; if neither holds,
report that and stop.

Find the current stage. Read the comments on #328. The stages are C1 to C4.
C1 and C2 are independent and may run as two sessions at the same time. Do
only one stage in this session, then end it.

Worktree: .claude/worktrees/lane-c-typl for the first session. If it does not
exist:
  git fetch origin
  git worktree add .claude/worktrees/lane-c-typl -b <branch> origin/main
and run ./bootstrap in it. A second session running at the same time
creates its own worktree, .claude/worktrees/lane-c-typl-2. At a later
stage, create that stage's branch from origin/main inside the lane's
worktree. Never enter another session's worktree. Run
`git branch --show-current` before every commit and every push.

Read for every stage. Every line number below is where the text sat at
d2e9d41; if a number does not match, find the item by its number or its
bold lead-in rather than trusting the line.
- docs/specification/typl-language-reference.md — §17 open questions
  (:1068-1258; item 11, the wire-width floor, at :1143; item 13, strings,
  at :1185; item 14, value-aware reorder comparison, at :1221; item 15,
  a reorder alongside an addition or removal, at :1240)
- docs/wip/2026-09-12-release-scope-and-plugin-system-design.md — §3.10
  (:292, integer-backed unit types) and §3.11 (:318, strings)
- docs/wip/typl-value-objects-design.md and docs/wip/typl-value-objects-plan.md
- docs/ROADMAP.md — Epic 14, Epic 10, and the carried typl defects
- docs/decisions/ADR-0013-codegen-backend-scope.md
- issues #318, #246-#255, #243, #237, #244, #203, #245

== C1 — E14.1, the typl §17 pass (branch docs/typl-open-questions) ==
Draft a table with one row per §17 item (15 items; items 12 and 13 are the
rows the re-scope added from §3.10 and §3.11, and items 14 and 15 are the
two #331 added after this prompt was written — both are about
`ridl-diff`'s reorder comparison, and item 15 already carries #302, so do
not file a second issue for it): what it asks, the options, a
recommendation, and what it blocks. Put §17.11 first. Lane A's RA-X7
(docs/wip/2026-09-08-ridl-rt-design.md §11) and E11.12 depend on it. Add a
row for #245: is a name-based
`reserved` legal in an enum body, and if so what does it guarantee. Walk the
table with Sebastien one row at a time. Each question is resolved (its text
moves into the section of the reference it changes) or deferred to a named
version. When §17.11 is decided, post the decision on #328. File an issue
for each resolved question that needs code. Docs-only pull request,
`docs(typl): ...`. Run /review <PR> (the docs-only lane), fix, pass 2,
`just verify`, merge.
E14.3 (#320: both references drop "Draft", and the rxdl reference gains
its status line) is its own small pull request, opened by the lane that
merges the last of this stage, C4 Task 10 and lane L's L5.

== C2 — the Epic 10 plan refresh (branch docs/typl-value-objects-refresh) ==
The plan is dated 2026-08-03, before #238 (the pinned name transform), #241
(the interaction layer retracted), #242 and #303 (the proto3 and FlatBuffers
backends). Task 2 names crates/ridl-backend-rust/src/interact.rs; check
whether that file still exists. Give a Sonnet subagent the job of checking
every Files line, line reference and code snippet in every task against
origin/main, and reporting only what no longer matches. Then:
- update each task that no longer matches;
- move Task 9 (TypeScript) out, to step 2, and leave a note where it was;
  Task 1's two-backend done-when completes in step 2. The tracker half of
  this is already done: E10.9 (#254) was closed as not planned on
  2026-09-12, refiled under §3.5 and §3.8 of the release-scope note. Do not
  reopen it; the move is to the plan document only;
- add a task for #243 (struct field names emitted verbatim, which draws
  non_snake_case) and #237 (union arm names that collide under camel_case).
  Check ADR-0016's pinned name transform first; #237 needs a decision
  (rename the arm, report a diagnostic, or both), and Fable drafts it for
  Sebastien.
Every task stays test first and names its model. Docs-only pull request,
/review, merge.

== C3 — defects (one branch and one pull request each) ==
Gates: G1 and G2 (both running sessions change
crates/ridl-core/src/diag.rs).
- #244: exact-duplicate struct fields and parameters report RIDL-149, a
  name-transform collision, instead of a duplicate. Test first.
- #203: a user package named ridl.std compiles clean and its artifact is
  overwritten by the standard package. Read ADR-0002 before choosing
  between a reserved name and a diagnostic.
- #245: only after C1 has merged, apply its decision.
Each one: `fix(<scope>): ...`, /review <PR> (four seats), fix, pass 2,
`just verify`, merge.

== C4 — Epic 10 (branches feat/typl-value-objects-<task>) ==
Gates: C2 merged, G2. Task 7 changes crates/ridlc/src/lib.rs and Task 10
changes the typl reference; both follow the plan's §6 order, and Task 10
also waits for C1. superpowers:subagent-driven-development over the
refreshed plan, one pull request per story (E10.x). `just build` green
after every task; `cargo fmt --all` before every Rust commit. Each pull
request: /review <PR> (four seats), fix, pass 2, `just verify`, merge. Run
sdd-gardening over the Epic 10 design and plan in the last pull request.

At every stage boundary: post one comment on #328 (stage, pull request,
anything another lane must know). Then end the session.

Stop and ask Sebastien when the design and the code disagree in a way the
plan did not expect, or when a gate check does not hold. Plain, literal
prose everywhere. Never name a private or consumer project.
```
