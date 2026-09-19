# Lane M driver — the generated interaction face (MVP)

Transient working memory for a lane proposed 2026-09-15, alongside
`2026-09-13-step1-lanes-plan.md` but not part of it: that plan's four lanes (A,
B, C, L) do not cover generating a RIDL interface's Rust API over `ridl-rt`.
Lane A built the full `ridl-rt` library (E11.0) — identity, the envelope, the
payload and encoding scaffolding (traits, not a working codec — see
Dependencies), and the ports — but no backend generates code against it yet.
ADR-0018 decision 15 and ADR-0020 decisions 5-7 sequence that generated
interaction face after the frame specification (E11.1) and the transport
(E11.9); neither is in scope for the current lanes, and `docs/ROADMAP.md`
carries no story for the face at all yet (this is not an edge the roadmap's own
sequence diagram records — that dependency lives in the ADRs). Lane M is a
deliberate exception: an in-process-only MVP so the team has something to write
against soon, ahead of that full sequence. Start a fresh session in
`.claude/worktrees/lane-m-interaction-face` and paste the block below as the
first message, once per stage: each stage is one session, and the session ends
when the stage's pull request has merged. Archive this file with whatever plan
or spec it produces, per `sdd-working-memory-lifecycle`.

This lane is not yet listed in `2026-09-13-step1-lanes-plan.md`. M1's driver
posts its existence to #328 and proposes whether that plan should gain a Lane M
row (§4). §6's shared-file table is also missing the one file this lane collides
on: `crates/ridl-backend-rust/src/lib.rs` is the file Lane C's Epic 10 is
already reshaping (see Dependencies) and the same file this lane's codegen
extends. M1 proposes an order for it in §6, rather than leaving the collision
uncoordinated.

## What this lane targets

ADR-0018 decision 15 restores the generated `Client`/`Provider`/`dispatch` face
as "phase 2" — the interaction layer the Rust backend once shipped and then
retracted. The ADR's Alternatives-considered table gives the reason: "it cannot
be connected to a runtime at all, and has never been compiled" (:501). `ridl-rt`
0.1.0 (Lane A, E11.0, merged and closed) ships the ports phase 2 binds to, but
it is a library, not a runtime — `docs/design/ridl-rt.md:3-9` is explicit that
no runtime exists yet; a runtime is a separate crate that implements the port
traits (ADR-0020 decision 6), and the first one, the in-process loopback, is
E11.9, out of this lane's scope. Lane M therefore needs its own throwaway,
test-only implementation of the port traits — just enough to drive one example
package's round trip; M1 must assign this explicitly (see Dependencies). The
target shape for the generated face is already drafted, unbuilt, in
`docs/wip/2026-09-08-ridl-rt-design.md` §8 (:770-839): a `Client<'a, P: ...>`
generic over exactly the ports an interface needs, a `Provider` trait the
application implements, and a generated `dispatch` over `Handler`.

## Dependencies — read before scoping M1

- **Hard, satisfied:** `crates/ridl-rt`'s ports and identity types (Lane A,
  merged). Nothing else from Lane A is needed; the crate need not be published
  to crates.io first (that's a maintainer act, tracked separately, and this lane
  depends only on the in-tree crate).
- **No dependency:** Lane B (rsdl describes systems and deployment, a layer
  above the single ridl interface this lane targets).
- **Resolved since this file's first draft — no placeholder needed:** every
  `ridl-rt` port is byte-oriented and scoped to a catalog-wide interface number
  (`crates/ridl-rt/src/port.rs:6,9` — "never a payload type: the generated
  binding decodes" / "the interface numbers ... scoped by that catalog"). This
  file originally found no such number anywhere in the IR and told M1 to invent
  a throwaway one. Lane L's L4 landed while that fix was in flight (`fdcf2ca`,
  PR #391, 2026-09-15): the IR now carries a real `Interface.number` and
  `Interface.provisional` (`crates/ridl-ir/proto/ridl/ir/v2/ir.proto`, added by
  #391), assigned by the compiler from the package's `interfaces.lock` when one
  exists, or provisionally at compile time when it does not (lock design §3). An
  example package with no `interfaces.lock` gets a provisional number for free —
  M1 uses that directly and does not invent a separate scheme. The only thing to
  confirm in the spec: a provisional number is fine for an in-process MVP, since
  RIDL-411 only blocks _publishing_ one, not using one locally. **Before scoping
  M1, re-check `origin/main` for further drift** — this section was already
  overtaken once by a concurrent merge; `git log` and the current IR are the
  source of truth, not this file.
- **A placeholder that is still needed:** `ridl-rt` ships no runtime — a runtime
  is a separate crate that implements the port traits, and the first real one
  (the in-process loopback) is E11.9, out of this lane's scope. M1 must assign a
  throwaway, test-only port implementation of its own, disposable the same way
  the numbering placeholder above would have been, so M3's round-trip test has
  something to run against.
- **A coupling to expect, not block on:** Epic 10 (Lane C's typl debt) is
  actively reshaping the Rust types this lane's `Client`/`Provider` methods use
  as arguments and return values, in `crates/ridl-backend-rust/src/lib.rs` — the
  same file this lane's codegen extends. The lanes plan's P-5 states Epic 10
  Task 6 changes struct and union declarations and Task 3 changes named-scalar
  emission. Starting Lane M now, before Lane C lands, means a follow-up touch-up
  pass once those shapes change. Acceptable at 0.x; name it as a known cost in
  the spec rather than being surprised by it later.
- **Not in this lane's scope:** the crates.io publish and cross-crate version
  sync Sebastien separately raised. That is a maintainer/release concern
  spanning every crate, tracked on its own, not folded into Lane M.

## Model routing

Fable is unavailable to Sebastien until Saturday 2026-09-19. Every stage below
substitutes Opus for the touchy/architectural work Fable would otherwise take.
If a stage has not started by 2026-09-19, prefer Fable for that stage's touchy
portion instead — check with Sebastien before assuming the outage has lifted.

- Driver: Opus. It holds the stage, reviews every subagent result, and decides
  what "done" means.
- M1 spec: Opus runs `superpowers:brainstorming` with Sebastien.
- M2 plan: Opus.
- M3 implementation: Opus for the `Client`/`Provider`/`dispatch` generation (the
  touchy part); Sonnet for mechanical emitter plumbing and the example package's
  boilerplate.
- M4 documentation: Opus.
- Mechanical work above about four tool calls (issue queries, reading a large
  file for one answer) goes to a Sonnet subagent.

## The prompt

```text
You drive Lane M: the generated interaction face over ridl-rt (MVP, in-
process only). Read AGENTS.md, this file in full, and
docs/wip/2026-09-13-step1-lanes-plan.md §5, §6 and §7 (the gates, the
shared-file ordering, and the rules every lane follows — Lane M follows
them even though it is not yet listed in that plan).

Find the current stage first. Read the comments on #328 and search for an
open pull request or issue naming Lane M or "interaction face". The stage
is the first of M1-M4 whose pull request has not merged. Do only that
stage, then end the session.

Worktree: .claude/worktrees/lane-m-interaction-face. If it does not exist:
  git fetch origin
  git worktree add .claude/worktrees/lane-m-interaction-face -b <branch> origin/main
and run ./bootstrap in it. At a later stage, create that stage's branch from
origin/main inside the same worktree. Never enter another session's
worktree. Run `git branch --show-current` before every commit and every
push.

Read for every stage:
- docs/wip/2026-09-08-ridl-rt-design.md §6 (ports, :492), §8 (:770-839)
- docs/decisions/ADR-0018-runtime-core-and-generated-surface.md decision 15
  (:327-358) and its 2026-09-12 amendment
- docs/decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md
  decisions 5-7
- crates/ridl-rt/src/port.rs, payload.rs, encoding.rs, sample.rs, contract.rs,
  error.rs
- crates/ridl-backend-rust/src/lib.rs (today's Rust emitter, the codegen
  this lane extends)
- docs/archive/2026-09-13-lock-design.md §3 (provisional numbering) — Lane L's L4
  landed after this file's first draft; the IR's `Interface.number` and
  `Interface.provisional` fields are real now (`crates/ridl-ir/proto/ridl/ir/v2/ir.proto`)
- This file's "Dependencies" section above

== M1 — the spec (branch docs/interaction-face-design) ==
Use superpowers:brainstorming with Sebastien, one question at a time,
section-by-section approval. Write the spec to
docs/wip/<today>-interaction-face-v0-design.md. It must settle:
 1. Scope: in-process only, no transport, no frame. State explicitly what
    this excludes (E11.1, E11.9, E11.7/E11.8/E11.12) and why that's safe for
    an MVP.
 2. The payload encoding backing the MVP. ADR-0020 sanctions exactly three
    encodings (proto3, FlatBuffers, repr(C)); do not invent a fourth. None
    is built yet (E11.7/E11.8/E11.12 are all out of scope for the current
    lanes) — decide the cheapest path to something real, or a narrowly
    scoped stand-in explicitly marked as throwaway, and say which.
 3. Confirm the interface numbering (see "Dependencies" above): the example
    package uses the provisional `Interface.number` the compiler now assigns
    when no `interfaces.lock` exists (Lane L's L4, `fdcf2ca`/#391), not an
    invented scheme. Re-check `origin/main` first — this is the one part of
    the file already overtaken once by a concurrent merge.
 4. The placeholder in-process port implementation (see "Dependencies"
    above): test-only, disposable, just enough to drive one example
    package's round trip — not E11.9's real loopback runtime.
 5. The Client/Provider/dispatch shape from the design note §8, adapted for
    the payload-encoding and numbering decisions above. Confirm RA-19 and
    RA-20 still hold (a client generic over exactly the ports it needs; a
    provider trait plus dispatch; no thread, future, socket or timer in
    generated code).
 6. Where the code is generated from: a new module in
    crates/ridl-backend-rust, or elsewhere — and where the example package
    proving a round trip lives.
 7. The coupling to Lane C's Epic 10 (P-5): name it as accepted rework, not
    a blocker.
 8. A tracking issue and a roadmap identifier. This lane is not yet in
    docs/ROADMAP.md. No single table gives the next free Epic 11 identifier —
    Epic 11 stories are split across docs/ROADMAP.md's own Epic 11 table, its
    "Rust codegen, finalized" table, and parked or landed rows in
    docs/archive/roadmap-landed-record.md. Grep both files for every `E11.`
    identifier before picking one, or just ask Sebastien — do not trust an
    enumeration written from memory, including this file's own past mistake
    here. Open a GitHub issue for the chosen identifier before merging this
    stage.
 9. An "Alternatives considered" section, and trace links to that issue.
Open a docs-only pull request, `docs(docs): design the interaction face
MVP`. Run /review <PR> (docs-only lane). Merge after pass 2 and Sebastien's
approval of the written spec. Post to #328: this lane exists, whether the
lanes plan should list it, and the roadmap identifier chosen.

== M2 — the plan (branch docs/interaction-face-plan) ==
Use superpowers:writing-plans on the merged spec. Write
docs/wip/<today>-interaction-face-v0-plan.md. Every task is test first,
names its files, and names its implementer model (see Model routing
above). Docs-only pull request, /review, merge.

== M3 — implementation (branch feat/interaction-face) ==
Gate: M2 merged. Use superpowers:subagent-driven-development over the M2
plan: one implementer subagent per task at the model the task names, then
the two-stage review. `just build` green after every task; `cargo fmt --all`
before every Rust commit. The example package must compile and a round-trip
call must pass in a test, driven by the placeholder port implementation M1
designed — that is this stage's done-when. Open the pull
request `feat(ridl-backend-rust): ...` (or the crate the spec names), run
/review <PR>, fix, run pass 2, `just verify`, merge.

== M4 — the progressive ridl-rt trait documentation ==
This answers Sebastien's observation that the ridl-rt traits are hard for
the team to pick up at first. Write it against the concrete Client/Provider/
dispatch M3 produced, not the raw port traits in isolation — build from a
single read or write, through a call, to the generated dispatch, each step
motivating the next trait rather than presenting the vocabulary at once.
Decide with Sebastien whether this lives as expanded rustdoc on
crates/ridl-rt/src/lib.rs and port.rs, a docs/technotes/ entry, or both.
Docs-only pull request, /review, merge. Run sdd-gardening over this lane's
spec and plan in the same pull request if this is the lane's last stage;
otherwise in whichever stage's pull request is last.

At every stage boundary: post one comment on #328 (stage, pull request,
anything another lane must know) and one on this lane's tracking issue
once M1 has opened it. Then end the session.

Stop and ask Sebastien when a record contradicts the spec, when a decision
would change ADR-0018 or ADR-0020, when the payload-encoding choice or the
in-process port placeholder is unclear, or when a gate check does not hold.
Plain, literal prose everywhere. Never name a private or consumer project.
```
