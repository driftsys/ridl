# Lane R driver — the generated face and port ergonomics

Status: driver prompt, 2026-09-20. One lane, three stages, each a fresh session.
Set the `THIS SESSION RUNS` line below before starting a session, and do only
that stage.

Where this document and an ADR disagree, the ADR wins. This document summarizes;
it does not decide.

---

You drive lane R: the generated face and port ergonomics that
`docs/wip/2026-09-20-face-and-port-ergonomics-design.md` (driftsys/ridl#429)
proposes to settle before E11.9.

Read `AGENTS.md`, then the note in full, then
[the interaction-face design record](../design/interaction-face.md),
[the `ridl-rt` design record](../design/ridl-rt.md),
[ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) and
[ADR-0023](../decisions/ADR-0023-interaction-face-generation.md). Those records
bind this session.

The note is not on `main`. It lives on driftsys/ridl#429's branch:

    git show origin/claude/ridle-dev-status-api-b4ngjk:docs/wip/2026-09-20-face-and-port-ergonomics-design.md

**THIS SESSION RUNS: R2**

## Gate — satisfied, do not re-litigate

Sebastien disposed of D-1 to D-6 on 2026-09-20, in two comments on
driftsys/ridl#429:

1. All six taken as proposed (`#429#issuecomment-5749666535`).
2. **D-3 amended** (`#429#issuecomment-5749853333`): a runtime may also offer an
   **aggregate handle** covering the port set one interface's face needs.

The second came out of R1's review, and it matters. D-3 as ratified said a
runtime exposes one handle per port trait and that a face is built over one
handle, while D-1 left the face's bounds unchanged — and those bounds are
multi-trait (`Client<'a, P: SignalReader + EventSource + Caller>`,
`crates/ridl-backend-rust/tests/generated/interaction_face.rs:490`). No
per-trait handle satisfies that, so `Client::new` had no argument a D-3 runtime
could supply. It was escalated rather than patched, which is the rule at the
bottom of this document.

**Implement against the merged ADR decisions, not against the note.** R1 landed
them as `f6b749e` (driftsys/ridl#432): ADR-0021 decisions 11 and 12, ADR-0023
decision 4 amended, decision 3 amended, and a new decision 5.

## Stages

### R1 — the records. Done.

Merged as `f6b749e` (driftsys/ridl#432). ADR-0021 decisions 11 and 12; ADR-0023
decision 4 amended and a new decision 5, plus decision 3's reason restated; the
"The ports" paragraphs of `docs/design/ridl-rt.md`; story E11.9's `Done when` in
`docs/ROADMAP.md`; the ADR summaries in `AGENTS.md` and
`docs/decisions/README.md`; and driftsys/ridl#350 items 15 to 17.

### R2 — the forwarding impls. Branch `feat/ridl-rt-forwarding-impls`.

The `ridl-rt` half of D-2, plus the two paragraphs ADR-0021 commits this change
to. Branch fresh from `origin/main`.

1. In `crates/ridl-rt/src/port.rs`, add `impl<P: T + ?Sized> T for &mut P` for
   **every** port trait. There are eleven: `Attached`, `Clock`, `SignalReader`,
   `SignalWriter`, `EventSource`, `EventSink`, `Caller`, `Handler`,
   `FixedReader`, `ScannableSignals`, `CoherentSignals`.
2. Add `impl<P: T + ?Sized> T for &P` for the six whose methods all take
   `&self`: `Attached`, `Clock`, `SignalReader`, `FixedReader`,
   `ScannableSignals`, `CoherentSignals`. Confirm that set against `port.rs`
   rather than trusting this list.
3. One test per trait under `crates/ridl-rt/tests/` that calls the trait through
   a borrow.
4. Flip the forwarding paragraph of `docs/design/ridl-rt.md`, "The ports", from
   "impls this module does not yet contain" to the present tense, and drop the
   sentence saying the paragraph describes impls `port.rs` does not contain.
5. Add one paragraph to the crate-level rustdoc in `crates/ridl-rt/src/lib.rs`
   stating the handle model of ADR-0021 decision 12. D-3's own change list names
   it, and ADR-0021's 2026-09-20 amendment preamble commits this change to it.
   R1 could not do it without ceasing to be docs-only.

Additive: no existing signature changes, no `alloc`, no `Box` impl — decision 11
defers it, and the crate brings in `alloc` under no feature combination. **Do
not bump the crate version**; that is the maintainer's call under ADR-0007
decision 14. Say in the pull request that the change is additive.

Commits: `feat(ridl-rt)`.

### R3 — the face. Branch `feat/face-by-value`. Needs R2 merged.

D-1 and D-4, in `crates/ridl-backend-rust/src/face.rs`:

- `Client<P>` and `Publisher<W>` hold the port by value, `new(port: P)`, no
  lifetime parameter, the RA-19 bounds unchanged.
- One `Copy` newtype per command and per query,
  `<Name>Correlation(Correlation)`, returned by the send method.
- `<name>_reply` takes its query's newtype; the interface-wide `ack` becomes one
  `<name>_ack` per command taking that command's newtype.
- `Correlation` in `ridl-rt` stays untyped.

Regenerate the fixture
(`RIDL_UPDATE_GENERATED=1 cargo test -p
ridl-backend-rust --test interaction_face`),
update the source-text assertions in `tests/face_generation.rs`, keep
`tests/dispatch_generation.rs` and the dispatch body unchanged, and keep every
round-trip test's shape — they still pass `&mut port`, which R2 makes compile.

Rewrite the `Client`, `Publisher` and correlation paragraphs of
`docs/design/interaction-face.md` to the shape as built.

Commits: `feat(ridl-backend-rust)`.

**R3 also owns two documents the note's §5 table does not list**, found in R1's
review: `docs/design/interaction-face.md` and
`docs/technotes/ridl-rt-by-example.md` both carry `Client<'a, P>`,
`Publisher<'a, W>` and `Result<Correlation, SendError>`, which decisions 4 and 5
supersede. ADR-0023's amendment names both so a reader is warned meanwhile.

## What does not change

Note §3: `Provider` methods take their argument by reference (ADR-0023 decision
3); a send returns `SendError`; no port waits, names a payload type, or takes
the current time; port receivers stay `&self` for reads and `&mut self` for
writes; `Correlation` and `ClaimId` stay `u64` newtypes.

The derive set is the value-objects design's decision 7 and lands in Epic 10
task 6, not here (note D-5). The wake hook (note §4) is not decided; do not add
one.

## Three things R1's review established, easy to get wrong

- **What the forwarding impls buy is a face over a _reference_ to a port, and
  nothing else.** A wrapper or a test double is already accepted by the face's
  trait bounds, with or without them, because the face is generic over `P`. Do
  not claim otherwise in a comment, a test name, or a pull request.
- **The `Send`/`Sync` split of decision 12 applies to a runtime whose handles
  cross threads.** A single-threaded `no_std` runtime whose handles use `Cell`
  or `RefCell` is held to neither, which is why no port trait carries the bound.
  Add no `Send` or `Sync` bound to any trait.
- **A port role is one port trait.** The split between a role handle and an
  aggregate is by port-trait count in a face's bound, not by interaction kind —
  `Caller` serves both commands and queries, so a calls-only interface has two
  interaction kinds and a single-trait bound.

## Order against lane C

R3 regenerates `crates/ridl-backend-rust/tests/generated/interaction_face.rs`.
Epic 10 tasks 4 and 5 regenerate the same file. Land R3 before lane C's task 4
branches; if task 4 has merged first, rebase R3 and regenerate the fixture once,
and say so in the pull request.

R2 touches nothing lane C touches — no fixture is regenerated and nothing under
`crates/ridl-backend-rust` changes. Lane C's task 4 is blocked by R3, not by R2.

Never enter another lane's worktree.

## Mechanics

Worktree `.claude/worktrees/lane-r-face`, created in R1 and left on the merged
branch `docs/face-and-port-records`. Branch fresh from `origin/main` inside it.

`git branch --show-current` before every commit and every push. **Stage explicit
paths, never `git add -A`.** Test first for R2 and R3: write the failing test,
run it, then the change. `cargo fmt --all` before every Rust commit, `just fmt`
before every Markdown commit.

Gate before a pull request: `just verify`. Review per the lanes plan §7, two
passes, ledger on the pull request, then merge. One pull request open at a time.

### Two facts about this container, found in R1

- **`git-std` is not installed**, and its release installer answers 403 through
  the agent proxy — the same failure driftsys/ridl#429 reports for prim.
  `just verify` and `just lint-commits` cannot run until you install it with the
  fallback `ci.yml` already carries:

      cargo install --git https://github.com/driftsys/git-std git-std --locked

- **driftsys/ridl#430 fails `just test` here and only here.** The container runs
  as root, permission bits do not apply to uid 0, and
  `fmt_on_an_unreadable_subdirectory_exits_two` asserts on them. If that is the
  only failure, push with `GIT_STD_SKIP_HOOKS=1` and name #430 in the pull
  request. `just build` stops at `test`, so run `lint`, `wasm-check`,
  `compat-check` and `check` yourself afterwards. CI's runner is not root, and
  its `rust` job is green.

## After each stage

Post one comment on driftsys/ridl#328: the stage, the pull request, and what
another lane must know. After R2: that R3 is unblocked, and whether the
crate-level rustdoc paragraph landed. After R3: that the fixture was regenerated
and lane C's task 4 may branch.

After R3 merges, run `sdd-gardening` over the note in that last pull request:
the note archives to `docs/archive/`, and R1's records already carry the
decisions. Then end the session.

## When to stop

Stop and ask Sebastien when a decision on driftsys/ridl#429 and the code
disagree in a way the note did not foresee, or when a gate check does not hold.
R1 hit that case once and it was real: D-1 and D-3 did not compose, and the
answer was a decision, not a patch.

A session low on context stops at a stage boundary and posts the handoff.

Plain, literal prose everywhere. Never name a private or consumer project.
