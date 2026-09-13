# Lane A driver — `ridl-rt` 0.1.0

Transient working memory for `2026-09-13-step1-lanes-plan.md`. Start a fresh
session in `.claude/worktrees/lane-a-ridl-rt` and paste the block below as the
first message, once per stage: each stage is one session, and the session ends
when the stage's pull request has merged. Archive this file with the lanes plan.

## Model routing

- Driver: Opus. It holds the stage, reviews every subagent result, and decides
  what "done" means.
- A1 spec: the driver runs `superpowers:brainstorming` with Sebastien. Dispatch
  Fable for any analysis whose answer becomes a decision in the spec (identity,
  verification, the ports, the RA-X dispositions).
- A2 plan: Opus.
- A3 implementation: Sonnet for plain data types and crate wiring; Fable for the
  interaction descriptors, the ports and the verification layer; Opus for the
  second-stage review of each task. Move an implementer to Fable after one
  failed fix loop.
- A4 release preparation: Sonnet.
- Mechanical work above about four tool calls (issue queries, reading a large
  file for one answer) goes to a Sonnet subagent.

## The prompt

```text
You drive lane A of docs/wip/2026-09-13-step1-lanes-plan.md: ridl-rt 0.1.0,
roadmap story E11.0 (#316). Read AGENTS.md, then that plan's §2 (P-1, P-2),
§4 lane A, §5, §6 and §7. The plan's rules bind this session.

Find the current stage first. Read the comments on #328 and run
`gh pr list --state all --search "ridl-rt"`. The stage is the first of A1-A4
whose pull request has not merged. Do only that stage, then end the session.

Worktree: .claude/worktrees/lane-a-ridl-rt. If it does not exist:
  git fetch origin
  git worktree add .claude/worktrees/lane-a-ridl-rt -b <branch> origin/main
and run ./bootstrap in it. At a later stage, create that stage's branch from
origin/main inside the same worktree. Never enter another session's worktree.
Run `git branch --show-current` before every commit and every push.

Read for every stage:
- docs/wip/2026-09-08-ridl-rt-design.md — all of it; §1.1 module layout
  (:151), §2 identity and envelope (:200), §6 ports, §11 open items (:935)
- docs/decisions/ADR-0018-runtime-core-and-generated-surface.md — the
  2026-09-12 amendment and the record-wide note on the name ridl-rt
- docs/decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md
  — decision 5 (:157, one cargo feature per encoding), decision 7 (:191),
  and the Documents amended table
- docs/wip/2026-09-12-release-scope-and-plugin-system-design.md — §3.7
  (:192) and §3.12 (:351)
- docs/wip/2026-09-13-runtime-descriptors-design.md — D-4 (:112) and the
  ridl-rt sentence (:260)
- issues #316, #308, #309

== A1 — the spec (branch docs/ridl-rt-design) ==
Use superpowers:brainstorming with Sebastien, one question at a time, and
section-by-section approval. Write the spec to
docs/wip/<today>-ridl-rt-v0.1-design.md. It must settle:
 1. The module layout: keep, merge or drop each of the note's six modules
    (encoding, payload, sample, contract, port, strata).
 2. Identity. Which identity types 0.1 has. D-7 of
    docs/wip/2026-09-12-rsdl-rewrite-decisions.md keeps service numbers out
    of the registry, so decide whether ServiceId belongs in ridl-rt at all.
    The WIDTHS come from lane L (gate GW on #328). Until GW holds, write
    this section with the widths marked pending and do not merge the spec.
 3. The envelope, and a disposition for #308 (the sequence number has no
    caller scope when several consumers send commands to one provider).
 4. Provenance, Freshness and Sample, and a disposition for #309 (an
    invalid event payload has no defined behaviour).
 5. Payload<E: Encoding>, the verify/decode split, and which encoding
    features 0.1 carries, given that the codecs themselves are E11.7,
    E11.8 and E11.12.
 6. The interaction descriptors: the per-member form of ADR-0013 decision
    3's ordinal table. Their fields agree with the catalog descriptor's D-4
    fields; ridl-rt does not depend on a descriptor crate.
 7. The ports and the three extensions of the note's §6.1.
 8. One disposition for each open item in §11 — RA-X1, X2, X4, X5, X6, X7,
    X8, X9, X10: in 0.1, out of 0.1 with the version or story that takes it,
    or already settled by a record (cite it). X1 and X7 are out (P-1).
 9. no_std, the alloc question, and a wasm32 build covered by
    `just wasm-check`.
10. The release policy: version 0.1.0 while the workspace stays 0.0.0; the
    tag name; whether git-std can bump one crate or the version is set by
    hand; crates.io metadata including rust-version equal to the pinned
    toolchain; a README that says 0.x and lists what 0.1 leaves out; whether
    the name ridl-rt is free on crates.io.
11. An "Alternatives considered" section, and trace links to #316.
Open a docs-only pull request, `docs(docs): design ridl-rt 0.1`. Run
/review <PR>, which takes the docs-only lane. Merge after pass 2, GW, and
Sebastien's approval of the written spec.

== A2 — the plan (branch docs/ridl-rt-plan) ==
Use superpowers:writing-plans on the merged spec. Write
docs/wip/<today>-ridl-rt-v0.1-plan.md. Every task is test first, names its
files, and names its implementer model (see Model routing in
docs/wip/2026-09-13-lane-a-ridl-rt-driver.md). One task registers the crate:
workspace members, a `ridl-rt` scope in .git-std.toml, the crate list and
count in AGENTS.md. The last task meets E11.0's done-when: a hand-written
program links ridl-rt and reads a sample with its provenance (the
E11.0 row in docs/ROADMAP.md). Docs-only pull request, /review, merge.

== A3 — the crate (branch feat/ridl-rt) ==
Gates: A2 merged, G2 (#327 merged), GW. Check them with the commands in the
plan's §5; if one does not hold, report which and stop.
Use superpowers:subagent-driven-development over the A2 plan: one
implementer subagent per task at the model the task names, then the
two-stage review. `just build` green after every task; `cargo fmt --all`
before every Rust commit. Open the pull request `feat(ridl-rt): ...`, run
/review <PR> (four seats), fix, run pass 2, `just verify`, merge.

== A4 — release preparation (branch chore/ridl-rt-release) ==
Crate metadata; `cargo publish --dry-run -p ridl-rt` passes. If that check
becomes a just recipe, keep it out of `build`, as `release` is: a recipe in
`build` must also run in CI (ADR-0009). The pull request description carries
a maintainer checklist with the exact commands Sebastien runs to publish and
tag. Never run them yourself. In the same pull request, run the
sdd-gardening skill over lane A's spec and plan. Merge after /review.
The lane then hands over to E11.1 (#257).

At every stage boundary: post one comment on #328 (stage, pull request,
anything another lane must know) and one on #316. Then end the session.

Stop and ask Sebastien when a record contradicts the spec, when a decision
would change ADR-0018 or ADR-0020, or when a gate check does not hold.
Plain, literal prose everywhere. Never name a private or consumer project.
```
