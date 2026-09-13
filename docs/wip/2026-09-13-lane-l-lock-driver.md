# Lane L driver — the lock block

Transient working memory for `2026-09-13-step1-lanes-plan.md`. Start a fresh
session in `.claude/worktrees/lane-l-lock` and paste the block below as the
first message, once per stage: each stage is one session, and the session ends
when the stage's pull requests have merged. Archive this file with the lanes
plan.

## Model routing

- Driver: Opus.
- L1 design and L2 plan: Fable for the analysis and the drafting. Identity,
  numbering, merge semantics and diff classification are all subtle.
- L3 roadmap: Opus for the roadmap text; Sonnet runs the `gh` operations from a
  list the driver has checked.
- L4 implementation: Fable for numbering, `ridl lock merge` and the diff
  changes; Opus for the command surface; Sonnet for sweeping the retired
  diagnostic codes and their references. Opus for the second-stage review.
- L5 ridl §17: Fable drafts, Sebastien decides.

## The prompt

```text
You drive lane L of docs/wip/2026-09-13-step1-lanes-plan.md: the lock block
(D-7 of docs/wip/2026-09-12-rsdl-rewrite-decisions.md), then E14.2 (#319).
Read AGENTS.md, then that plan's §2 (P-2 to P-6), §4 lane L, §5, §6 and §7.
The plan's rules bind this session.

Find the current stage first. Read the comments on #328. The stage is the
first of L1-L5 whose pull request has not merged. Do only that stage, then
end the session. L3 and L4 may run in either order once both of their
gates hold.

Worktree: .claude/worktrees/lane-l-lock. If it does not exist:
  git fetch origin
  git worktree add .claude/worktrees/lane-l-lock -b <branch> origin/main
and run ./bootstrap in it. At a later stage, create that stage's branch from
origin/main inside the same worktree. Never enter another session's worktree.
Run `git branch --show-current` before every commit and every push.

Read for every stage:
- docs/wip/2026-09-12-rsdl-rewrite-decisions.md — D-7 (:286-399), and the
  §4 amendment items (:460-501) about identity and the lock
- docs/wip/2026-09-12-interface-id-study.md and -2.md
- `git show b1c43fa` — the commit that settled D-7's two internal tensions
- docs/decisions/ADR-0002-module-system.md — ridl.lock is per workspace; the
  D-7 lock file is a different file, one per package
- docs/decisions/ADR-0015-qos-absorption-and-rpc-bounds.md — decisions 15,
  17, 18, 19 and 24, which D-7 amends; decision 10 is not affected
- docs/decisions/ADR-0010-cli-conventions.md — every subcommand follows it
- the baseline gate session's design and plan, read without checking out
  its branch:
    git show baseline-tombstone-gate:docs/archive/2026-09-13-baseline-gate-design.md
    git show baseline-tombstone-gate:docs/archive/2026-09-13-baseline-gate-plan.md
  (on main under docs/archive/ once that session has merged)
  Its D-5 limits that gate to the interaction level; the interface level
  is this lane's
- docs/wip/2026-09-13-catalog-descriptor-plan.md — Tasks 3 and 4 (the
  numbering contract and the catalog hash) — and issue #326
- docs/wip/2026-09-13-runtime-descriptors-design.md — D-4 (:112)
- issues #315, #314, #302

== L1 — the design (branch docs/lock-design) ==
Use superpowers:brainstorming with Sebastien, one question at a time.
Write docs/wip/<today>-lock-design.md.
Its FIRST section fixes the identity widths, and Sebastien approves that
section before you write the rest. The records disagree today:
docs/wip/2026-09-08-ridl-rt-design.md:205-208 proposes u16 for Ordinal,
InterfaceId and ServiceId; crates/ridl-ir/proto/ridl/ir/v2/ir.proto:87
carries every ordinal as uint32;
docs/wip/2026-09-13-catalog-descriptor-plan.md:296, :306 and :319 already
write uint32 for the ordinal and the interface number, so that plan's
Task 1 schema changes if the width does; docs/wip/2026-09-12-interface-id-study.md
:114-115 reports the vocabulary note's slot u8 and hash u64; D-7 and the
runtime descriptors design state no width. Decide the interface number
width, the member ordinal width, and whether a service number exists in
the runtime identity at all (D-7: service numbers only as a backend
attribute for a tag-based transport). As soon as Sebastien approves the
section, post the widths on #328 with the words "GW holds", and post the
same comment on #316. Lane A is waiting for it.
The rest of the design: the lock file (name, location, format, the `next`
counter, checked in and generated); provisional numbers (after the frozen
ones, never in source); `ridl lock` (its ADR-0010 row and exit codes);
`ridl lock merge` as a three-way merge driver (study 2 measured a union
driver bringing back a renamed entry without any message); `ridl diff`
matching by number; `ridl baseline` refusing a provisional number and an
untombstoned interface removal (say how this relates to the baseline gate's
RIDL-408); rename by shape against the baseline; the retirement of
RIDL-146, RIDL-147, RIDL-148 and ADR-0015 decision 19's five ServiceShape*
diff categories (count the references on origin/main); the ADR-0015
amendments; what changes for #324 Tasks 3 and 4. #314 and #302 are not
widened: read the carried-debt comment above `diff_composite` first.
Include "Alternatives considered" and trace links (#315 and the D-7 section).
Docs-only pull request, `docs(docs): design the lock file and ridl lock`.
Run /review <PR> (the docs-only lane), fix, pass 2, `just verify`, merge.

== L2 — the plan (branch docs/lock-plan) ==
Use superpowers:writing-plans on the merged design. Write
docs/wip/<today>-lock-plan.md. Every task is test first, names its files and
its implementer model. The plan lists the §6 shared files it changes.
Docs-only pull request, /review, merge.

== L3 — the roadmap (branch docs/step1-roadmap) ==
Gates: L2 merged, G2. One pull request to docs/ROADMAP.md that records:
(1) P-3: E11.0 runs beside Epic 6 and the lock, not after Epic 6;
(2) rows for the lock block, from the L2 plan, and rows for the twelve
tasks of the catalog descriptor plan (#324) — neither has a place in the
roadmap today; ask Sebastien which epic they sit under, and check
docs/archive/roadmap-landed-record.md so no row reuses a parked identifier;
(3) P-5: #243 and #237 move to Epic 10's carried defects;
(4) P-6: E14.2 after the lock; E14.3 after E14.1, Epic 10 Task 10 and
E14.2; the Rust codegen, finalized, still follows the typl debt.
Then prepare the story issues for the new rows, check the list, and give it
to a Sonnet subagent to file. `docs(roadmap): ...`. /review, merge.

== L4 — the implementation (branch feat/ridl-lock) ==
Gates: L2 merged, G1 (both baseline gate pull requests merged), G2 (#327
merged; it also changes diag.rs, main.rs, cli-reference.md and ADR-0010). Check with
the plan's §5 commands; if a gate does not hold, report which and stop.
superpowers:subagent-driven-development over the L2 plan. `just build`
green after every task; `cargo fmt --all` before every Rust commit. /review
<PR> (four seats), fix, pass 2, `just verify`, merge. Then post on #328 with
the words "G4 holds": lane B's lowering and #324 are waiting for it.

== L5 — E14.2, the ridl §17 pass (branch docs/ridl-open-questions) ==
Gate: L4 merged. Draft a table with one row per open question in the ridl
reference's §17: what it asks, the options, a recommendation, and what it
blocks. Walk it with Sebastien one row at a time. Each question is resolved
(its text moves into the section it changes) or deferred to a named
version. The baseline gate session added §17.12 and §17.13; include them.
The QoS and bound terms must agree with ADR-0015. Docs-only pull
request, `docs(ridl): ...`. Run sdd-gardening over lane L's design and plan
in this pull request. /review, merge.
E14.3 (#320: both references drop "Draft", and the rxdl reference gains
its status line) is its own small pull request, opened by the lane that
merges the last of lane C's C1, lane C's C4 Task 10 and this stage.
The lane then hands over to executing #324, after Sebastien confirms the
seven dispositions that plan takes.

At every stage boundary: post one comment on #328 (stage, pull request,
anything another lane must know). Then end the session.

Stop and ask Sebastien when a record contradicts D-7, or when a gate check
does not hold. Plain, literal prose everywhere. Never name a private or
consumer project.
```
