# Lane B driver — rsdl and its reference

Transient working memory for `2026-09-13-step1-lanes-plan.md`. Start a fresh
session in `.claude/worktrees/lane-b-rsdl` and paste the block below as the
first message, once per stage: each stage is one session, and the session ends
when the stage's pull request has merged. Archive this file with the lanes plan.

## Model routing

- Driver: Opus.
- B1 reference: the driver writes with Sebastien. Dispatch Fable to draft any
  section whose wording decides grammar or semantics (placement, instances, the
  implicit component, crossing kinds, the diagnostic catalogue).
- B2 tracker pass: Sonnet runs the `gh` operations from a list the driver has
  checked. The implementation plan: Opus, with Fable for the lowering tasks.
- B3 parse and check: Fable for grammar and semantics; Opus for the LSP and the
  VS Code extension; Sonnet for corpus fixtures and snapshots.
- B4 lowering: Fable. Opus for the second-stage review of every task.

## The prompt

```text
You drive lane B of docs/wip/2026-09-13-step1-lanes-plan.md: rsdl finalized,
with its reference (roadmap Epic 6). Read AGENTS.md, then that plan's §2,
§4 lane B, §5, §6 and §7. The plan's rules bind this session.

Find the current stage first. Read the comments on #328. The stage is the
first of B1-B4 whose pull request has not merged. Do only that stage, then
end the session.

Worktree: .claude/worktrees/lane-b-rsdl. If it does not exist:
  git fetch origin
  git worktree add .claude/worktrees/lane-b-rsdl -b <branch> origin/main
and run ./bootstrap in it. At a later stage, create that stage's branch from
origin/main inside the same worktree. Never enter another session's worktree.
Run `git branch --show-current` before every commit and every push.

Read for every stage:
- docs/wip/2026-09-12-rsdl-rewrite-decisions.md — all 523 lines: §2 the
  language (:47), §3 decisions (:133), D-7 identity (:286-330, owned by lane
  L), D-11 (:443-459), §4 amendments (:460-492), §5 open (:503)
- docs/wip/2026-09-08-topology-vocabulary.md
- docs/specification/rsdl-language-reference.md — v0.1.0, 815 lines, to be
  rewritten
- docs/wip/family-general-form.md — §4.8 attributes, §6.3 numbers
- docs/specification/ridl-family-overview.md — doctrine 18
- docs/specification/ridl-language-reference.md — §14.5 and §14.6
- docs/decisions/ADR-0002-module-system.md — one system per workspace
- docs/wip/2026-09-13-runtime-descriptors-design.md — what the system
  descriptor needs from the lowering
- docs/ROADMAP.md — Epic 6

== B1 — the reference (branch docs/rsdl-reference) ==
Before writing, confirm five points with Sebastien, one at a time. They were
written during reviews of the decisions note and not discussed with him:
 (a) V-X3 in D-11 (:454): a distribution has no member-kind keyword; a
     member line is a bare reference whose kind is its declaration's.
 (b) D-4: `Unit` is an IR and diagnostic name, never a source spelling, so
     the casing rule R7 does not apply to it.
 (c) D-1: rejecting an implicit service named after a component rests on
     V-02 and ridl §14.5 (the service is the addressing unit).
 (d) D-5: overview doctrine 18 marks posture derivation as deferred inline,
     in the overview's own form, instead of keeping its text unchanged.
 (e) The §4 amendment census covers every shipped `provides` and posture
     site the reviews found.
Then write an outline: the reference's sections, each mapped to the
decisions it states. Get Sebastien's approval of the outline. Then rewrite
docs/specification/rsdl-language-reference.md: the five declarations, the
body lines and clause, the keys, the attribute principle, the implicit
component, instances, placement, crossing kinds, the reserved posture
section, grants, the diagnostic catalogue (the old RSDL codes kept, retired
or new — codes in Markdown are not checked against the catalogue, #191, so
check them by hand), and the facts the lowering produces, stated as facts
and not as a protobuf schema. Cite the lock once, as an input to the
lowering; lane L specifies it.
Apply the §4 amendments in the same pull request, split this way: items
about identity, the lock, RIDL-146 to RIDL-148 or ADR-0015 belong to lane
L — leave them. Roadmap edits belong to B2. Edits to the ridl reference and
to ADR-0010 wait for G1, because the baseline gate session changes both; if
G1 does not hold yet, list them on #328 for B2. Apply everything else.
Docs-only pull request, `docs(rsdl): rewrite the rsdl reference`. Run
/review <PR> (the docs-only lane), fix, pass 2, `just verify`, merge.

== B2 — tracker and plan (branch docs/rsdl-plan) ==
Gate: G2. Read the Epic 6 rows in docs/archive/roadmap-landed-record.md
first: identifiers are identity, so a new story never reuses the number of a
parked one. Write the new Epic 6 rows in docs/ROADMAP.md (if lane L's roadmap
pull request is open, wait for it or coordinate on #328). Prepare a list:
each open Epic 6 issue (#63, #64, #66, #67, #278, #279, #280, #281, #282)
closed as not planned with a comment that names the new story, and each new
story issue titled `E6.<n> — <story>` with its done-when and size. Closing
never rewrites a body. Check the list, then give it to a Sonnet subagent to
run. Apply any §4 amendment items B1 left for this stage.
Then use superpowers:writing-plans for docs/wip/<today>-rsdl-plan.md in two
parts: B3 (parse, check, diagnostics, LSP and extension support for .rsdl)
and B4 (lowering and the book chapter). Every task is test first and names
its model. Decide in the plan whether the book-example harness
(crates/ridl/tests/book_examples.rs) learns `rsdl` fences.
One pull request, `docs(roadmap): ...` or `docs(rsdl): ...`. /review, merge.

== B3 — parse and check (branch feat/rsdl-check) ==
Gates: B2 merged, G2. superpowers:subagent-driven-development over the B3
part of the plan. The diagnostic catalogue in crates/ridl-core/src/diag.rs
follows the plan's §6 order. `just build` green after every task. /review
<PR> (four seats), fix, pass 2, `just verify`, merge.

== B4 — lowering and the book chapter (branch feat/rsdl-lower) ==
Gates: B3 merged, G4 (lane L's lock has merged: the lowering reads its
numbers and the catalog hash). superpowers:subagent-driven-development over
the B4 part. The Epic 6 exit criteria: a system compiles from .rsdl, and the
IR carries the region map, the link set, the routing table, the permission
list, the surface set and the catalog hash. Write the book chapter as built,
in the same pull request. Run sdd-gardening over lane B's plan in this pull
request. /review, merge. The system descriptor emitter is not in this lane.

At every stage boundary: post one comment on #328 (stage, pull request,
anything another lane must know). Then end the session.

Stop and ask Sebastien when the decisions note and a shipped record
disagree, or when a gate check does not hold. Plain, literal prose
everywhere. Never name a private or consumer project.
```
