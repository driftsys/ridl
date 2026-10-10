# Lane H — open items after release 0.7.0

Created 2026-10-09. The main session drives the lane. Sebastien delegated every
decision in it on 2026-10-09; each one is recorded under [Rulings](#rulings),
and the lane's final report lists them.

## Scope

Four items were left open when the book user chapters landed (#771 to #780):

1. **driftsys/ridl#782** — the Rust backend emits code that draws 26 clippy
   lints (21 `needless_borrow`, 5 `blocks_in_conditions`), so `just demo` cannot
   run clippy on the crate generated from the veh-cluster corpus.
2. **The catalog hash conflict.** The frame specification §6.1 refuses `attach`
   with `catalog_mismatch` whenever the two catalog hashes differ. `ridl diff`
   classifies an appended interaction as a compatible change. A deployed
   consumer therefore can never attach to a provider that was upgraded by a
   compatible change: the change is compatible in `ridl diff` and incompatible
   on the wire.
3. **PR B of the book** — the chapters "Writing a port" and "Evolving an
   interface". The release that publishes `ridl-rt-conformance` was a
   precondition; v0.7.0 published it on 2026-10-09, so the precondition holds.
4. **The publish list.** The release workflow publishes the crates from a list
   written by hand, which an xtask test now checks (#779).
   `cargo publish --workspace` could replace the list.

## Stages

| Stage | Work                                                                                                                                                           | Model                                                   | Starts when          |
| ----- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------- | -------------------- |
| H0    | File the issue for `cargo publish --workspace` (item 4): done, #783                                                                                            | the main session                                        | now                  |
| H1    | Fix #782 in the Rust backend's emitter; extend `just demo` to run `cargo clippy -- -D warnings` on the corpus crate                                            | Sonnet stage agent; implementer Sonnet; reviews per SDD | now                  |
| H2    | The catalog compatibility design under `docs/wip/`, option A (item 2), with the amendments of ADR-0014 decision 15 and frame specification §6.1, then its plan | Fable for the design; Opus for the plan                 | now                  |
| H3    | Implement the H2 plan, task by task                                                                                                                            | Sonnet stage agent; models per the plan                 | H2 merged, H1 merged |
| H4    | The book chapter "Writing a port"                                                                                                                              | Sonnet stage agent; Opus writer                         | now                  |
| H5    | The book chapter "Evolving an interface"; garden the H2 design and plan and this driver with `sdd-gardening`                                                   | Sonnet stage agent; Opus writer                         | H3 merged, H4 merged |

H1, H2 and H4 change disjoint files and run in parallel. H3 waits on H1 because
both may change the Rust backend. H5 waits on H3 because the book describes the
system as built: the chapter's compatibility section states the attach rule that
H3 implements. H5 waits on H4 because both change `docs/book/SUMMARY.md`.

## Stage briefs

### H1 — the emitter lints (#782)

- Fix the emitter so that the generated code draws neither lint: pass the value
  without `&` where the callee already takes a reference (`PATTERN.is_match(…)`,
  `push_string(…)`), and emit the float step check as a `let` binding followed
  by `if`, not as `if { … } {`.
- Pin each fix with a test in the backend's existing snapshot or emission tests.
  The red state of each test must be an assertion failure, not a compile error.
- Extend `just demo` so it runs `cargo clippy -- -D warnings` on the crate it
  generates from `crates/ridlc/tests/corpus/veh-cluster`. The corpus also draws
  13 `dead_code` warnings that come from the corpus itself; allow `dead_code`
  for that one run only, with a comment in the recipe that says why. Do not
  allow any other lint.
- Update the `just demo` description in `AGENTS.md` and the recipe comment so
  they name the new check.
- One pull request, `Closes #782`.

### H2 — catalog compatibility, option A

**Decision taken (ruling R-1):** option A. The catalog descriptor lists the
earlier catalog hashes that the toolchain judged compatible with the current
catalog, and a provider accepts an `attach` that names one of them.

The design must decide, each with an "Alternatives considered" note:

1. **Where the list comes from.** Which toolchain step records an earlier hash
   as compatible: the lock and baseline workflow (`ridl lock`, `ridl diff`, the
   lockfile) is the expected source. State what happens when `ridl diff` reports
   an incompatible change: the list must restart, so that a consumer built
   against a pre-break catalog is refused.
2. **Where the list lives.** The catalog descriptor
   (`docs/design/catalog-descriptor.md`, `crates/ridl-descriptor`), the
   generated face's constants, or both. Keep the descriptor's version rule and
   its lenient reader.
3. **What a provider does after it accepts an earlier hash.** The provider holds
   interactions the consumer does not know. §6.1 currently says that two
   runtimes attached under one `CatalogRef` hold the same interfaces, which
   makes `Contract::UnknownInteraction` a protocol error. State the invariant
   that replaces it: the consumer never names an interaction it does not hold,
   and the provider must not send the consumer a frame for an interaction that
   the consumer's catalog does not contain (for example an event appended after
   the consumer's hash). Decide how the provider knows which interactions that
   is.
4. **Direction.** Only an older consumer attaching to a newer provider, or also
   a newer consumer attaching to an older provider. Choose one and say why.
5. **The amendments.** The text of the ADR-0014 decision 15 amendment and the
   §6.1 change, written in the same pull request as the design.
6. **What H3 builds**, as a plan of tasks under `docs/wip/`, each with exact
   files, tests and acceptance.

Read first: frame specification §6 and §10, ADR-0014 (decision 15 and its
amendments), ADR-0022, ADR-0023 decision 8, `docs/design/catalog-descriptor.md`,
`docs/design/interaction-face.md` "The catalog check", the `ridl diff` rules in
`crates/ridl-diff`, and the archived one-catalog-per-unit design (#777).

### H3 — implementation

Execute the H2 plan with subagent-driven development: one implementer per task,
a task review with both verdicts before the next task, then `/review` on the
pull request.

### H4 — "Writing a port"

The outline agreed on 2026-10-08: the port traits by role, the conformance suite
through the published `ridl-rt-conformance` crate, the frame specification §10
and Appendix B. Every `ridl`/`typl`/`rsdl` fence follows the book harness rules
in `AGENTS.md`; a Rust example that is compiled is included from `examples/`
with an anchor, as "Using the generated Rust code" does. The chapter states the
current attach rule only by linking frame specification §6.1; it does not
restate it, so H3 does not make it stale.

### H5 — "Evolving an interface"

The outline agreed on 2026-10-08: the lock, baseline and diff workflow, and a
table of change verdicts (compatible, breaking, and what each one does at
`attach`). The compatibility section describes the attach rule as H3 built it.
The same pull request gardens the H2 design and plan and this driver into
`docs/archive/` with `sdd-gardening`, and removes lane H from `docs/wip/`.

## Rules every stage follows

The rules of the lanes plan §7 apply: worktree under `.claude/worktrees/` with
`./bootstrap`, `/review` then fix then pass 2 then `just verify` before merge,
never a tag, a publish or a push to `main`, plain prose, and an end-of-stage
comment on the coordination issue #328.

A stage agent escalates every ruling that this driver or a recorded decision
does not cover to the main session. It does not hand back until its pull request
merges or a real blocker stops it.

## Rulings

Each ruling: what was decided — why — what it costs if wrong.

- **R-1** Option A for item 2 — it keeps `ridl diff`'s verdict true on the wire
  without a negotiation protocol — if wrong, the descriptor carries a field that
  a later design replaces, and a frame specification amendment is reverted.
- **R-2** File item 4 as an issue rather than implement it now — the
  hand-written list is correct and tested today, and the change touches the
  release workflow, which only a real release exercises — if wrong, the
  hand-written list stays one release longer.
- **R-3** H5 waits for H3 — the book describes the system as built — if wrong,
  the evolution chapter lands later than it could have.
- **R-4** In H2, the `ridl` facade reads the baseline and computes the list of
  compatible earlier hashes, then passes the list to `ridlc`. `ridlc` stays a
  pure source-to-IR function — ADR-0008 decisions 9 and 14 keep baseline reads
  outside the compiler, as its tool-qualification boundary — if wrong, the build
  options carry one input field more than needed.
- **R-5** In H1, the corpus crate's clippy run rewrites the crate's `allow` to
  `expect` as the cabin run does, when both lints fire on the corpus crate;
  otherwise it keeps `allow` and records a deferred Minor — the two runs then
  check the same thing — if wrong, a stale `expect` fails `just demo` until the
  emitter stops emitting it.
- **R-6** In H2, fix both pass 2 Minors in the pull request — they were one link
  and one sentence — if wrong, nothing; the fix was small.
- **H2's own rulings** R-H2-1 to R-H2-13 are in the Rulings section of
  `2026-10-09-catalog-compat-design.md` (merged in #785, 6db0eee7). R-H2-14 was
  withdrawn and replaced by R-4. H3 is tracked on driftsys/ridl#787;
  driftsys/ridl#786 records `ridlc`'s existing dependency on `ridl-diff`.
- **H4's rulings** (merged in #790, a265dff7): the chapter does not cover an
  "Appendix B", because the frame specification has none (Appendix B is "Codegen
  Targets" in the ridl reference) — if wrong, one section to add; the Factory
  skeleton is a `rust,ignore` fence with a link to the loopback test, because
  `just book-check` copies only `docs/` and `examples/` — if wrong, the example
  is not compile-checked; the TOML example keeps the `0.7` pins — if wrong, it
  goes stale at the next release, tracked on driftsys/ridl#796; "the previous
  chapter" in `catalog-descriptor.md` now names its chapter — no cost.
- **R-7** On #794 pass 1, fix the Important and seven Minors in the pull
  request, including the `unnecessary_cast` on the integer step check, which
  predates the change but is the same class as #782. Park three: `-A dead_code`
  on the whole corpus crate (the brief asked for it), no lint of the
  `--no-default-features` path, and `Operand::receiver` duplicating state (a
  design preference) — the parked three are cheap to revisit later — if wrong,
  one more small pull request.
- **R-8** Start H3 before H1 merges. H1 changes `crates/ridl-backend-rust` and
  the `corpus__rust` snapshots; H3's plan changes neither, so the overlap is at
  most a rebase — if wrong, H3 regenerates snapshots after H1 merges.
- **R-9** On #794 pass 2, fix the Important (the integer step test let a mutant
  that casts literals survive) and the Minor (an assertion broader than the
  property) in the pull request, with a scoped re-review — two passes is the cap
  — if wrong, one more small pull request.
- **R-10** On #794's quick pass, defer two Minors about the integer step check
  (a no-cast assertion that names only the fixture's literals, and an unpinned
  `__step <= 0` guard) — typl rejects `step` on an integer type (TYPL-105), so
  no source reaches that path — if wrong, a later change that makes the path
  reachable must add the tests.
- **R-11** In H3 Task 2, the `.catalogs` parser is strict: it drops the per-line
  `trim`, so a line with surrounding whitespace or a CRLF ending is refused —
  the design says each line is exactly 64 lowercase hex characters and the
  toolchain writes the file — if wrong, a file edited by hand on Windows is
  refused until it is rewritten.
- **H1 outcome:** merged as #794 (0903f0bf), closing #782; the deferred Minors
  are on driftsys/ridl#798.
- **R-12** In H3, the `shape_walk` xtask test that Task 1 broke is fixed in its
  own reviewed commit before Task 4, and every later implementer and task review
  runs the whole workspace's tests — Task 1's implementer and reviewer ran only
  the crate's tests, and the failure passed both — if wrong, the cost is the
  longer test run per task.
- **R-13** In H3, a test pins that the `compatible` list does not feed the
  catalog hash: one catalog built with an empty list and with a non-empty list
  has the same hash — otherwise recording an earlier hash would change the
  current one — if wrong, nothing; the test only asserts the design.
- **R-14** In H3, `ridl baseline` orders its two moves (the snapshots and the
  history files) so that an interrupted publication can only drop an earlier
  hash, never carry one past a breaking change — dropping refuses an older
  consumer, which is safe; carrying lets an incompatible consumer attach — if no
  order achieves it, the residual risk goes to a follow-up issue.
- **R-15** In H3, `ridl build` reads a unit's history only when the baseline
  holds a package of that unit, the same guard `ridl baseline` applies — a stray
  history file would otherwise list hashes no published snapshot backs — if
  wrong, nothing; the guard only drops hashes.
- **R-16** H3's final review found the branch ready. Two trivial fixes and the
  transitive-reach test go into the pull request. A residual risk (a partially
  hand-edited baseline can carry hashes past a breaking change) does not block
  the merge and is filed as driftsys/ridl#808, with a hardening option that is a
  design decision — if wrong, a hand-edited baseline can let an incompatible
  consumer attach until #808 lands.
- **R-17** When a unit's current catalog hash equals the head of its published
  history, the chain is carried whatever the unit verdict — the catalog did not
  change, so every listed hash stays compatible — if wrong, a change that
  `unit_verdict` calls breaking but that leaves the hash equal keeps the chain.
- **R-18** (amends R-11) The history parser accepts exactly one trailing `\r`
  per line, so a file checked out with git's autocrlf still parses; any other
  padding is still refused — if wrong, nothing; a CR carries no meaning here.
- **R-19** A build with any error diagnostic, RSDL-7xx included, writes no
  compatible list; the prose says so — an empty list is the safe direction — if
  wrong, a build that writes artifacts despite an RSDL-7xx error lists nothing.
- **#810 pass 1 outcome:** six Important findings fixed in the pull request; the
  cost and structure Minors are on driftsys/ridl#811.
- **R-20** R-17 changes ADR-0014 decision 15 and frame specification §6.1, so
  #810 carries a dated amendment of both, and three tests pass 2 found missing —
  a decision that changes is recorded in its record — if wrong, nothing.

Lane H closed 2026-10-10: H1 #794, H2 #785, H3 #810, H4 #790, H5 #812;
follow-ups #796, #798, #808, #811.
