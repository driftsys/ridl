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
