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

### Two seats and a refuter, not four — a cost decision with a named risk

This is worth stating accurately, because the first draft of this file stated it
inaccurately and in the direction that flattered the conclusion. The review of
this pull request caught that.

**What C3 actually produced.** Fourteen kept findings across four review passes
on #406 and #407. Nine were test precision. **Five were not**, and all five came
from the two seats this routing removes — the docs and compliance lenses. Worse
for the argument: the two findings C3's own ledgers rated highest were both
theirs. #407 pass 1's ledger calls its docs finding "the one that mattered" — a
diagnostic message asserting something untrue of some of the inputs it greets —
and pass 2's Critical, the scratch file committed at the repository root, was
raised by compliance.

So the honest summary is that **the tests lens has the best yield per finding,
and the dropped lenses raised the most consequential ones.** One stage is thin
evidence either way.

**One thing the seat tags show that cuts the other way, and it matters.** Two of
those five were not found by a dropped lens alone. #407 pass 1's docs finding is
tagged `docs+bugs`, and the Critical is tagged `compliance+docs+tests` — so a
retained seat co-found each of them, the correctness lens on the first and the
tests lens on the second. Under this routing both would plausibly still have
been caught. That is the strongest evidence for two seats being enough, and it
comes from the seat tags rather than from an argument, which is why every ledger
line carries one.

**What is well sourced.** Mixing model families beats adding a third seat of the
same family: an earlier ledger recorded nineteen multi-seat duplicate groups
across two pull requests reviewed by four Claude seats. Separately, the
2026-09-14 review-cost measurement — recorded outside this repository — found
the tests seat the best-yielding of the four, at six unique kept findings for
$2.55, with compliance producing seventeen findings of which thirteen duplicated
another seat and one survived. That measurement is real; the first draft of this
file cited it to the wrong document.

**So the reduction is taken on cost, with the co-discovery above as the reason
to expect it to hold**, and the residual risk is mitigated rather than denied:
**seat 1's brief is widened** to carry what docs and compliance were catching —
whether the change does what it claims, whether prose and code still agree, and
whether the records this change touches still agree with each other. That
widening is the reason two seats is defensible; without it the evidence above
argues for three.

**Revisit this** if a defect of the docs or compliance kind reaches `main`
through a C4 pull request. Record the seat tag on every ledger line, which is
what makes that judgement possible later.

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
Nothing. Decision D — what a colliding union arm does — was taken on 2026-09-17:
report a diagnostic, extend RIDL-149 to a union's arms over BOTH pinned
transforms, move camel_case into ridl-ir, no rename. Task 11 carries the
reasoning and the implementation steps. Every task in C4 is runnable.

Read decision D's point 3 before writing that check. A check keyed on snake_case
alone does not close #237: the two transforms are incomparable, and the arm pair
XY and x_y collides under camel_case only, which is the Rust defect itself.

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

  Seat 1 — gpt-5.6-sol. Correctness, and the records. Three questions, and the
           second and third matter as much as the first:
             (a) does the change do what it claims, and what breaks;
             (b) does every statement it makes about the code match the code —
                 a diagnostic message, a doc comment, a specification sentence;
             (c) do the records this change touches still agree with each other
                 after it.
           (b) and (c) are here because C3 dropped them from no seat and they
           still raised its two most consequential findings: a message that
           asserted something untrue of some of its inputs (b), and a scratch
           file committed at the repository root, alongside two records left
           disagreeing where they had agreed (c).
  Seat 2 — Opus 5, taking the TESTS lens explicitly: would each test fail if the
           behaviour were wrong. Not whether tests exist.
           Keep this lens by name. It had the best yield per finding in C3 and in
           the 2026-09-14 cost measurement before it.
  Refuter — Sonnet 5, high effort, one per finding (batch by file above eight).
           Keep a finding when it is not REFUTED and confidence >= 60. Record a
           dropped finding with the refuter's reasoning; never discard silently.

TAG EVERY LEDGER LINE WITH ITS SEAT. That tag is the evidence for whether two
seats was the right call. If a docs or compliance defect reaches main through a
C4 pull request, the routing goes back to three seats.

Give each seat only the work product: a two-sentence description, the
requirements, BASE and HEAD, and the rule-file paths. Never your session, your
reasoning, or the pull request body.

Do not send a seat a finding you can settle by reading the code yourself — check
it, and say in the ledger that you did.

VERIFY A KEPT TEST FINDING BY MUTANT. Apply the exact mutation the finding names,
confirm the suite passed before your fix and fails after, and record both in the
ledger. C3 did this on every kept test finding, and twice it caught a test that
passed for the wrong reason.

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

## Three rules above that came from C3, and what they cost to learn

**"Read the diff yourself."** Stated as a precaution rather than as something C3
proved: the ledgers record what the review seats found, not what the driver's
own diff-read caught before the pull request, so there is no measured case here
either way. The reason to keep it is that a report and a diff are different
artefacts — an implementer's report describes what it believes it did, and the
review seats on both C3 pull requests found real defects inside work whose
reports were accurate. The cheapest place to find those is before a seat is paid
to.

**"Stage explicit paths."** A review subagent wrote a scratch `review.diff` into
the lane worktree, and `git add -A` carried it into a commit at the repository
root. It reached a pull request and was caught as a Critical finding in pass 2.
The cause is environmental rather than a lapse of judgement: another session's
subagent, or a formatter sweep, can put a file in the worktree at any time. The
#406 implementer hit the same class from the other side when the pre-commit
`prim .` pulled an unstaged edit into a staged commit.

**"Verify a kept test finding by mutant."** C3 applied it to every kept test
finding, and twice it found a test that passed for the wrong reason — one where
an exact duplicate entering the projection map left the code set unchanged, and
one where a swapped pair of label spans left every unit test green. Both would
have shipped as green suites.
