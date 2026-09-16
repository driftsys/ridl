# Lane C driver — stage C4, Epic 10

Transient working memory for
[`2026-09-13-step1-lanes-plan.md`](2026-09-13-step1-lanes-plan.md). Start a
fresh session in `.claude/worktrees/lane-c-typl` and paste the block below as
the first message, once per session. Archive this file with the lanes plan.

This prompt replaces the C4 row of
[`2026-09-13-lane-c-typl-driver.md`](2026-09-13-lane-c-typl-driver.md), which
covers C1 to C3 and is otherwise unchanged. C4 needed its own because it is the
largest stage of the four lanes — ten live tasks and roughly eleven commits —
and because it runs a cheaper review than the one those prompts describe.

## Why C4 is four sessions and not one

Stage C1, C2 and C3 were each one session of one or two pull requests. C4 is ten
tasks, each its own pull request with its own review. A single session runs out
of context partway through, and the failure is silent: it starts forgetting
decisions it took earlier in the same session. The session split below is along
the plan's own dependency order.

## Model routing

Set by Sebastien on 2026-09-16, and it is not the routing the lanes plan's §4
table gives for C4. It replaces that row while Fable is unavailable (§4's
substitution note) and lowers cost without dropping the lens that has been
earning its keep.

| Role                                     | Model         | Effort |
| ---------------------------------------- | ------------- | ------ |
| Driver                                   | Opus          | medium |
| Spec and plan work                       | Opus          | high   |
| Code — Tasks 1, 2, 4, 5, 7, 8, 10        | Sonnet 5      | —      |
| Code — Tasks 3, 6, 11                    | Opus 5        | high   |
| Intermediate review, on driver judgement | `gpt-5.6-sol` | —      |
| Pull request seat 1 — correctness        | `gpt-5.6-sol` | —      |
| Pull request seat 2 — the tests lens     | Opus 5        | —      |
| Refuter                                  | Sonnet 5      | high   |

Tasks 3, 6 and 11 take the stronger model because they are the three the lanes
plan gave to Fable: the breaking change to every generated named scalar, the
derive-eligibility recursion, and the naming defects that carry decision D.

**Two seats and a refuter, not four seats.** Two reasons, both measured rather
than assumed. Across C3's four review passes, nine of ten findings were test
precision and **none** was an executable-code defect, so the tests lens is where
the yield is; and the earlier cost record in the review command names
`review-tests` as the best-yielding seat at $2.55 for six unique kept findings.
Mixing model families also finds more than adding a third seat of the same
family: an earlier ledger recorded nineteen multi-seat duplicate groups across
two pull requests reviewed by four Claude seats.

## The prompt

```text
You drive stage C4 of lane C of docs/wip/2026-09-13-step1-lanes-plan.md: Epic 10,
the typl value objects. Read AGENTS.md, then that plan's §2 (P-5, P-6, P-7), §4
lane C including the model-routing note above the tables, §5, §6 and §7. Those
rules bind this session.

THIS SESSION RUNS: <tasks>            ← set this line, see "Sessions" below.
Do only those tasks, then post the handoff and end the session.

== Sessions ==
C4 is ten live tasks and roughly eleven commits. It does not fit one session.
Run it as four, in this order, each a fresh session with this prompt:
  C4a — Tasks 1, 2, 7    (the classifier, the ridl-rt dependency, the crate)
  C4b — Tasks 3, 4, 5, 8 (the constructors)
  C4c — Tasks 6, 11      (the derives and the two naming defects)
  C4d — Task 10          (record the decision, verify ridl-diff, garden)
A session that finds itself running low on context stops at a task boundary,
posts the handoff, and says which tasks remain.

== Read first ==
- docs/wip/typl-value-objects-plan.md — the plan of record. Execute it task by
  task, in order. It was refreshed on 2026-09-16 against the code; its ## Currency
  section says what changed and why.
- docs/wip/typl-value-objects-design.md — the spec. Where plan and spec disagree
  the spec wins, with one recorded exception the plan names in its ## Open.
- docs/ROADMAP.md — Epic 10 and its carried defects.
- ADR-0013 (backend scope), ADR-0016 (the pinned name transform), ADR-0020
  decision 6 (generated Rust links ridl-rt), ADR-0021 decision 10.
- Issues #246 to #255 (E10.1-E10.10), #243 and #237 (Task 11), #252.

== Gates ==
C2 merged (#396) and G2 holds. Re-check G2 before starting:
  gh pr view 327 --json state --jq .state   → MERGED
If it does not hold, report and stop.

== Blocked work ==
Task 11's #237 half waits on decision D — Open item 4 of the plan, what a
colliding union arm does. A recommendation is drafted there; Sebastien decides.
The #243 half of Task 11 does NOT wait for it. If C4c starts before the
decision, do #243 and stop at #237.

== Worktree and branches ==
Work in .claude/worktrees/lane-c-typl. Never enter another lane's worktree:
lane-c-typl-2, lane-l-lock, lane-a-ridl-rt, lane-b-rsdl, lane-m-interaction-face.
One branch and one pull request per Epic 10 story, branched fresh from
origin/main: feat/typl-value-objects-<task>.
Run `git branch --show-current` before every commit and every push.

STAGE EXPLICIT PATHS. Never `git add -A` or `git add .`. A review subagent wrote
a scratch file into this worktree during C3 and `-A` swept it into a commit at
the repository root; it reached a pull request and was caught only as a Critical
finding in pass 2. Name every path you stage.

== How to run one task ==
1. Read the task in the plan. It is written test first and names its own model.
2. If the task's shape no longer matches the code, STOP and fix the plan first,
   in its own docs-only pull request. Do not execute a task you know is stale.
   Dispatch the spec/plan work to an Opus agent at high effort.
3. Dispatch the implementation to a code agent, test first, with a complete
   brief: the root cause or the goal, the exact files, the exact expected
   behaviour, what is out of scope, and the commands to run. The agent does the
   work itself and commits; it never pushes and never merges.
     Sonnet 5        — Tasks 1, 2, 4, 5, 7, 8, 10
     Opus 5, high    — Tasks 3, 6, 11   (the design-bearing ones)
   Move a task to Opus 5 after one failed fix loop.
4. Review the agent's work YOURSELF before it goes near a pull request. Read the
   diff. Do not accept a report in place of reading it.
5. Intermediate review, on your judgement rather than mechanically: when a task
   is large, subtle, or touches a contract, dispatch a gpt-5.6-sol review of the
   diff so far, before the pull request. Skip it for a small mechanical task.
   Do not batch these to the end of the session — the point is to catch a defect
   while the change is one edit old.
6. `cargo fmt --all`, `just test`, `just lint`. Then open the pull request.
7. Review, fix, merge (below). THEN start the next task. Never run two tasks'
   pull requests at once.

== Review of a pull request ==
Two seats in parallel, one refuter, at most two passes.

  Seat 1 — gpt-5.6-sol. Correctness and defects: does the change do what it
           claims, and what breaks.
  Seat 2 — Opus 5, taking the TESTS lens explicitly: would each test fail if the
           behaviour were wrong. Not whether tests exist.
           Keep this lens by name. Across C3's four review passes, nine of ten
           findings were test precision and none was a code defect; the measured
           cost record also names the tests seat as the best-yielding one.
  Refuter — Sonnet 5, high effort, one per finding (batch by file above eight).
           Keep a finding when it is not REFUTED and confidence >= 60. Record a
           dropped finding with the refuter's reasoning; never discard silently.

Give each seat only the work product: a two-sentence description, the
requirements, BASE and HEAD, and the rule-file paths. Never your session, your
reasoning, or the pull request body.

Do not send a seat a finding you can settle by reading the code yourself — check
it, and say in the ledger that you did.

VERIFY A KEPT TEST FINDING BY MUTANT. Apply the exact mutation the finding names,
confirm the suite passed before your fix and fails after, and record both in the
ledger. C3 did this eleven times and it caught two tests that passed for the
wrong reason.

You may decline a kept finding. Record it in the ledger with the reasoning, so
the judgement is reviewable. Do not implement a suggestion you believe is wrong.

Post the ledger as a pull request comment. Pass 2 only if pass 1 kept findings,
scoped to the fix diff. Then `just verify`, then squash-merge, then update local
main to origin/main.

== Per-task tracker ==
Close the Epic 10 story issue when its task merges. Closing never rewrites a
body; explanations go in a comment.

== Known state, so you do not re-derive it ==
- Task 9 (TypeScript) is NOT executed. It moved to step 2. E10.9 (#254) is closed
  as not planned — do not reopen it.
- Task 2 was rewritten on 2026-09-16: the generated code defines no error type
  and returns ridl_rt::payload::Violation. Open item 3 is answered, not open.
- Task 7 adds ridl-rt to the generated manifest. Settle the Task 2 / Task 7
  ordering in session C4a before implementing either: the generated code names
  ridl_rt before the manifest that declares it exists, and the rustc compile
  proofs drive a single file with no --extern. The plan's Task 2 flags this.
- TYPL-205 is NOT free; the specification reserves it for a deferred tuple-shape
  lint. TYPL-215, RIDL-413 and TYPL-010 were minted in C3; the next free TYPL-2xx
  is 216 and the next RIDL-4xx is 414.
- Union arms still have no exact-duplicate check. That is Task 11's #237 half.
- typl §7, ridl §6.1 and §7.1, and typl §3.1 gained rules in C3. Read them before
  editing those sections.

== Shared files (§6) ==
crates/ridlc/src/lib.rs (the Emit enum) — Task 7's turn.
docs/specification/typl-language-reference.md — Task 10's turn, then E14.3.
Before pushing anything that touches a shared file, run `gh pr list --state open`
and confirm no other pull request holds it. If you go before your turn, say so on
#328 before you push.

== At the stage boundary ==
Post one comment on #328: the tasks done, their pull requests, and anything
another lane must know. If tasks remain, say which and why you stopped. Then end
the session.

== Stop and ask Sebastien ==
When the design and the code disagree in a way the plan did not expect, when a
gate does not hold, or when a task needs a decision the plan does not contain.
Plain literal prose everywhere: no idioms, no figures of speech. Never name a
private or consumer project in a repository file.
```

## Two rules above that came from C3, and what they cost to learn

**"Read the diff yourself."** C3's implementing agent reported accurately and
the report was still not enough: reviewing its work turned up four findings that
only reading the diff surfaces, including a test whose assertion was decorative
— it compared against a hard-coded expression that evaluated to zero while
reading as though it derived an offset from the fixture.

**"Stage explicit paths."** A review subagent wrote a scratch `review.diff` into
the lane worktree, and `git add -A` carried it into a commit at the repository
root. It reached a pull request and was caught as a Critical finding in pass 2.
The cause is environmental rather than a lapse of judgement: another session's
subagent, or a formatter sweep, can put a file in the worktree at any time. The
#406 implementer hit the same class from the other side when the pre-commit
`prim .` pulled an unstaged edit into a staged commit.

**"Verify a kept test finding by mutant."** C3 applied eleven, and two of them
found tests that passed for the wrong reason — one where an exact duplicate
entering the projection map left the code set unchanged, and one where a swapped
pair of label spans left every unit test green.
