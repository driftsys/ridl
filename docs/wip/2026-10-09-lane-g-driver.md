# Lane G — gate holes, user-visible defects, and the debt backlog

Created 2026-10-09. The main session drives the lane. Sebastien delegated every
decision in it on 2026-10-09; each one is recorded under [Rulings](#rulings),
and the lane's final report lists them.

## Scope

The open-issue sweep of 2026-10-09 (114 open issues) ranked these. Lane H owns
#782 and #783 and its own items; this lane does not touch them.

1. **Gate integrity** — the lock, diff and build gates let through what they
   exist to refuse:
   - #700 `ridl diff` reports `identical` after `ridl lock` freezes a
     provisional number, although the catalog hash changes.
   - #397 `ridl-diff` compares enum and enumset members by position, not by
     their explicit value.
   - #778 item D-3: deleting a whole unit's last package without retiring its
     numbers is not refused by the baseline gate (RIDL-412).
   - #426 `just gate-parity` cannot see a member removed from `just build`.
2. **User-visible defects**:
   - #770 a `.rxdl` file in a package is skipped with no diagnostic.
   - #765 false RIDL-101/RIDL-108 on an unreadable RPC annotation that keeps a
     minimum (also carries two test gaps and standing drift; read its body).
   - #643 a qualified internal type is refused in a foreign package view.
   - #657 `ridl fmt` moves a comment written after an rsdl machine separator to
     the next machine.
   - #623 a bare CR is a line break in `ridl-lsp` but not in the lexer.
   - #728 an rsdl region's interfaces are emitted in name order; the design
     fixes number order.
   - #739 `cargo xtask calibrate dump` writes `[lints]` entries for dropped
     lints.
   - #196 an unreadable path is reported with the wrong cause, or none.
3. **Release and CI reliability**: #729 (two timing-flaky plugin tests), #687
   (no test of the CI changes-job filter and `if:` gates), #734, #646, #645
   (catalogue-versus-reference drift guards that accept typos and miss removed
   clauses).
4. **Decisions waiting**: #735, #704, #486, #487, #631. (#350, #665, #718, #726,
   #727 wait on other work and stay out of this lane.)
5. **#778 close-out**: D-2 and the specification sentence (see R-2, R-3), the
   leftover minors moved to one debt issue, then #778 closed.
6. **Debt triage**: the 59 open review-debt issues (the sweep's bucket E).

## Stages

| Stage | Work                                                                                                                    | Model                                                      |
| ----- | ----------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------- |
| G0    | Rule on the five waiting decisions (item 4); write each ruling below and on its issue; fold any small code into a stage | the main session                                           |
| G1    | #700 and #397 in `ridl-diff`                                                                                            | Sonnet stage agent; Opus implementers and task reviewers   |
| G2    | #778 D-3 in the baseline gate; the specification sentence (R-3); #778 close-out (item 5)                                | Sonnet stage agent; Opus implementers and task reviewers   |
| G3    | #770, #765, #643 (loader and checker diagnostics)                                                                       | Sonnet stage agent; implementers per task                  |
| G4    | #657, #623, #728 (formatter, LSP positions, rsdl lowering order)                                                        | Sonnet stage agent; implementers per task                  |
| G5    | #426, #729, #687, #739, #196, #734, #646, #645 (gates, CI, xtask, CLI messages)                                         | Sonnet stage agent; Sonnet implementers, Opus reviewers    |
| G6    | Debt triage of bucket E: close what is moot, merge duplicates, label the rest; garden this driver with `sdd-gardening`  | Sonnet stage agent; the main session confirms each closure |

**Parallelism.** G1, G2, G3 and G4 change disjoint crates or files and run in
parallel, at most three stage agents at a time:

- G1 changes `crates/ridl-diff`. Before it starts, read lane H's driver
  (`.claude/worktrees/lane-h-driver/docs/wip/2026-10-09-lane-h-driver.md`, stage
  H3). If H3's plan touches `crates/ridl-diff`, G1 waits for H3 to merge.
- G2 changes `crates/ridl/src/main.rs`, `crates/ridl/tests/baseline_gate.rs` and
  one sentence of `docs/specification/ridl-language-reference.md`.
- G3 changes `crates/ridl-core` (loader) and `crates/ridl-sem` (`timing.rs`, the
  visibility guard).
- G4 changes `crates/ridl-fmt`, `crates/ridl-lsp`, `crates/ridl-syntax` and the
  rsdl lowering in `crates/ridl-sem/src/rsdl/`; it rebases over G3 if both touch
  `ridl-sem`.

G5 starts when one of G1 to G4 has merged, because #729 changes plugin tests the
other stages' gate runs depend on, and it touches the justfile and the CI
workflow. G6 runs last.

## Stage briefs

Each stage plans its own work: a short plan under `docs/wip/` (one task per
issue, or per tightly coupled pair), then
`superpowers:subagent-driven-development` with a task review per task. Each fix
starts with a test that reproduces the defect (red), then the fix (green). A
test that pins existing behaviour is proven by a named semantic mutation.

### G0 — decisions

Read each issue and the record it cites. Rule, write the ruling under
[Rulings](#rulings) and as a comment on the issue, and either close the issue
(no code), or hand the code to the stage whose files it touches, or leave it
with a label `ready` if it fits no stage.

### G1 — the diff gate

- #700: the diff must see a frozen provisional number and the catalog hash
  change. Read the issue for the region-map part. The verdict for a frozen
  number is a decision: record it (not `identical`; which category).
- #397: compare enum and enumset members by explicit value, as the specification
  already requires; a reorder with unchanged values is not a change, a renumber
  is breaking.

### G2 — the baseline gate

- D-3: a package or a whole unit present in the baseline and gone from the fresh
  set: every published, non-retired number of its shapes is refused with
  RIDL-412, the same message as a lost interface. The deliberate override is
  deleting that unit's snapshots from `.ridl/baseline/`; say so in the message
  and in `docs/book/` where RIDL-412 is described. Tests in
  `crates/ridl/tests/baseline_gate.rs`.
- R-3's sentence. Then #778's close-out: one new `debt` issue holding the minors
  listed in the #778 comment of 2026-10-09, D-2 closed per R-2, #778 closed with
  a comment linking both.

### G3, G4, G5

One task per issue, in the order listed in the table. Each issue's body is the
requirement. A fix that needs a new diagnostic code follows ADR-0024 (warning or
info) or the catalogue rules (error); record the code chosen as a ruling.

### G6 — debt triage

For each bucket-E issue: read it against `main`; close it with one sentence when
the code or a later change made it moot; merge it into another when they
overlap; otherwise label it `debt` and give it a milestone. Produce the list of
proposed closures first; the main session confirms them before any issue is
closed. Then garden this driver.

## Rules every stage follows

- Worktree under `.claude/worktrees/lane-g-<stage>`, created from fresh
  `origin/main`, then `./bootstrap`. One pull request per stage.
- `/review` pass 1, fix wave, pass 2, a quick pass over any commit after pass 2,
  then `just verify` before reporting the pull request ready. Every finding gets
  a disposition on its ledger line and in the pull request body.
- Never a tag, a publish or a push to `main`. Merging: the stage reports the
  pull request ready; the main session merges.
- Plain literal prose. No story or stage ids in shipped files
  (`just story-id-check`).
- Commit trailer: the attribution line the session's system reminder gives.
- An end-of-stage comment on the coordination issue #328.
- A stage agent escalates every ruling that this driver or a recorded decision
  does not cover to the main session. It does not hand back until its pull
  request is ready or a real blocker stops it; the main session resumes it with
  `SendMessage`, never with a second agent.

## Rulings

Each ruling: what was decided — why — what it costs if wrong.

- **R-1** #778 D-3 is done in this lane — a published number that escapes the
  gate because its whole unit disappeared breaks the lock's only invariant — a
  user who deletes a unit on purpose must also delete its baseline snapshots.
- **R-2** #778 D-2 (the codegen model rebuilds the unit hash per package) is
  closed as not planned — `codegen::lower(package, others)` is the per-package
  API every backend and plugin reads (ADR-0020 decision 9), and the cost is
  linear in the unit's size per package — reopen if a profile of `ridl build`
  shows it.
- **R-3** The specification follows the code on provisional order:
  `docs/specification/ridl-language-reference.md` (~1243) says byte order of the
  name, the interface before an inline shape of the same name, the `service:`
  prefix not counted — the code's rule is pinned by four tests and whole-key
  order would push every inline shape behind every interface — a reader of the
  old sentence predicted different provisional numbers before locking.
- **R-4** G1 does not start until lane H's H3 plan exists and has been read — on
  2026-10-09 H2's design was not yet written, and option A makes `ridl diff` the
  step that judges an earlier catalog hash compatible, so H3 is likely to change
  `crates/ridl-diff`; if the plan names that crate, G1 waits for H3 to merge.
  G2, G3 and G4 start now and fill the three agent slots — if wrong, G1 starts
  later than it could have.
- **R-5** #735: leave it as built and close the issue as not planned. A channel
  that no link consumes has no ring to size, and the request already records the
  depth as absent with source `UNDERIVABLE`, which tells a plugin why. A warning
  on an event nobody consumes would fire on every unused event, and dropping the
  channel from the request would change what every plugin reads — if wrong,
  RSDL-806 is widened later; adding a warning case is a compatible change under
  ADR-0024.
- **R-6** #704: approved as scoped in its 2026-10-05 comment (design note first,
  then the IR field, the lowering, the Rust backend), outside this lane, label
  `ready`. It changes the IR and the plugin protocol, which no stage of this
  lane touches — if wrong, the narrow translator stays one lane longer.
- **R-7** #486: derive `Debug` on the generated `Event` enum always, and
  `Clone, Copy, PartialEq, Eq` whenever every payload derives them; do not mark
  it `#[non_exhaustive]` (the generated crate is regenerated with its consumer,
  so the attribute would only force a wildcard arm that hides a new event);
  reword the `Provider` doc to the first reason alone. Recorded in
  `docs/design/interaction-face.md` by the change that implements it. Label
  `ready`, outside this lane, because lane H's H1 and H3 change the Rust backend
  now — if wrong, consumers keep matching into events one lane longer.
- **R-8** #487: decline the crate-root re-export for a single-package workspace
  (two spellings for one item, and the paths change when a second package is
  added); record the stack buffers sized by `MAX_SIZE` as a known property with
  the two ways out the issue names. Both are lines in
  `docs/design/interaction-face.md`, done in the same change as R-7. Label
  `ready` — if wrong, a single-package consumer keeps the longer path.
- **R-9** #631: the 2026-10-01 decision stands — fixed space/2 indentation, the
  EditorConfig indentation work stays debt for a later lane; G6 gives it a
  milestone — if wrong, a user with tab indentation keeps reformatted files.
- **R-10** #770: a `.rxdl` file the loader finds draws `RIDL-417`, a warning,
  lint name `unsupported-source-file`, emitted where the loader collects source
  files (both the disk and the overlay paths) — it follows RIDL-416, the one
  existing loader warning about a file the unit does not read, and the name is
  not tied to one extension so a later skipped kind reuses it — if wrong, the
  code is renamed before a release; a lint name is cheap to change before users
  configure it in `[lints]`.
- **R-11** #657: keep the formatter's behaviour — a comment after an rsdl
  machine's `}` is laid out on its own line between the machines, in source
  order, as the crate docs and two pinned tests already say — and correct the
  one contradicting sentence in `docs/wip/fmt-ridl-rsdl-layout.md` §5. Moving
  the comment onto the machine's line would make it trail a separator that is
  optional — if wrong, a user who wrote the comment on the `}` line sees it move
  down one line.
- **R-12** #623: a lone CR is a line break everywhere: the lexer ends a line
  comment, a string and a regex at it, `line_col` counts it, a CRLF pair stays
  one break, and the typl reference's line-ending sentence names LF, CRLF and a
  lone CR — it matches `ridl-lsp`'s `LineIndex` (which follows the LSP
  specification) and the `header-file` text, and needs no new diagnostic code.
  Rejecting a bare CR would leave the CLI and the editor disagreeing on every
  file still read — if wrong, a file that relied on a lone CR inside a comment
  or a string now lexes differently, which only a file written with classic Mac
  line endings meets.
- **R-13** #728: closed as already fixed — `regions()` sorts by interface number
  (`crates/ridl-sem/src/rsdl/lower.rs`), pinned by
  `the_routing_table_is_sorted_by_interface_number_and_not_by_name`, which G4
  saw fail under two mutations (sort by name, no sort) — if wrong, the issue is
  reopened.
- **R-14** G2: the two existing tests that move a package out of its unit
  (`baseline_drops_a_snapshot_whose_package_is_gone`,
  `a_failed_publication_keeps_the_stale_snapshot`) are adapted by giving the
  stale snapshot number 0 (not yet published) — each test is about snapshot
  housekeeping, not number loss, and with a published number the new RIDL-412
  refusal is the correct result for both — if wrong, the two tests no longer
  cover a published package that moves, which the three new D-3 tests cover.
- **R-15** G2: the R-3 sentence states a tie-break (an interface before an
  inline shape of the same name) that no test pins; G2 adds that test, proven by
  a mutation that reverses the tie-break — the specification must not state a
  rule the suite does not hold — if wrong, one extra test.
- **R-16** G2 RIDL-412 wording: the snapshot-deletion override is offered only
  when the whole unit is gone from the fresh set, and then the message does not
  lead with restoring a line in the unit's `interfaces.lock`, which no longer
  exists. A package gone from a unit that remains keeps the lost-interface
  message, whose remedy (retire the number) is enough — deleting a unit's
  snapshots drops the gate for every package of that unit for one publication,
  which is too broad for one package — if wrong, a user removing one package
  reads one remedy fewer.
- **R-17** G2: a legacy snapshot with no `unit` that loses a package is now
  refused instead of passing silently; accepted and stated in the pull request
  body, with no new test — it occurs only during the migration to per-unit
  locks, and refusing is the gate's purpose — if wrong, a user mid-migration
  retires or restores the number by hand.
- **R-18** G2: the gate's `member.retired` check is reachable only from a legacy
  per-package snapshot of a subpackage (for every other case `ridl diff` already
  classifies a retired removal as `InterfaceRetired`). Keep the check and pin it
  with a test built on the legacy per-package baseline, the subpackage's number
  retired in the fresh lock, expecting no RIDL-412; the test that claimed to pin
  it is kept as a pin of the diff's own retire rule and renamed to say so —
  removing the check would refuse a retired number during the lock migration —
  if wrong, one test covers a path that ends when legacy snapshots do. The
  legacy-only wording imprecision ("the workspace no longer has that unit" for a
  legacy subpackage) is accepted, as it predates this change.
- **R-19** #623 left-overs: `crates/ridl/src/lock.rs` `line_of` stays LF-only
  (TOML does not treat a lone CR as a line break); the doc-comment code in
  `crates/ridl-sem` (`doc_lint.rs`, `docs.rs`) that splits on `\n` only goes to
  one follow-up `debt` issue filed by G4, not into G4's pull request, because G3
  changes `ridl-sem` at the same time; the `ridl-mcp` test helper stays — if
  wrong, a lone-CR file misses the detached-doc lint until that issue lands.
- **R-20** #770: the overlay loader accepts a `.rxdl` overlay and reports it
  with RIDL-417 without compiling it; ADR-0025 decision 7 (with a dated
  amendment line in its `## Status`) and rule 2 of
  `docs/design/mcp-workspace-tools.md` §4.1 are amended in the same pull request
  — R-10 requires the warning on the overlay path, which a refused overlay
  cannot give — if wrong, a client that relied on the `LoadError` for a `.rxdl`
  overlay now gets a warning instead.
- **R-21** #770: a `.rxdl` file given alone (single-file mode, no `ridl.toml`)
  also draws RIDL-417 and is not compiled, instead of being compiled as typl —
  the documentation says a `.rxdl` file is not compiled, and compiling it as
  typl gives diagnostics about a language it is not written in — if wrong, a
  user who used `ridl check x.rxdl` as a typl check loses that.
- **R-22** G2 reverses the reference's §17.13 position ("a whole package removed
  or renamed … publication still does not refuse it"); the reversal is kept and
  recorded as a decision, not as a wording fix. Its ground: since the
  one-catalog-per-unit change the unit lock gives a package removed from a unit
  that remains a retirement in the language (retire its numbers), so the old
  premise "no package-level retirement" no longer holds for that case; for a
  whole unit gone, the refusal catches the accidental case (a moved directory, a
  changed manifest) that would otherwise drop the snapshots and lose the record
  without a word, and the deliberate case costs one deletion that the message
  names. The rewritten §17.13 text states this ground and what would reopen it
  (a unit-level retirement construct). The change is marked breaking in the
  squash commit (`!` and a `BREAKING CHANGE:` footer), because `ridl build` now
  refuses a publication it accepted before — if wrong, a user who removes a unit
  on purpose pays one extra step, and §17.13 is restored.
- **R-23** Marking rule for this lane: a change that makes the toolchain refuse
  input it accepted, or read accepted input differently, is marked breaking in
  its squash commit — consumers read the changelog to decide an upgrade — if
  wrong, the changelog lists a breaking entry that a user does not notice.
- **R-24** #793 is marked breaking under R-23, against the pass-1 deep refuter's
  verdict on the string and regex case: that verdict holds for a lone CR inside
  a string or a regex (the reference already forbade it), but a lone CR inside a
  line comment was accepted text and now ends the comment, so the rest of that
  line is read as source — if wrong, one changelog entry is marked breaking that
  affects almost no file.
- **R-25** R-4 applied: lane H's plan (merged in #785,
  `docs/wip/2026-10-09-catalog-compat-plan.md` Task 1) adds
  `crates/ridl-diff/src/unit_verdict.rs`, a per-unit verdict computed from the
  `ridl-diff` report — the report that #700 and #397 change — so G1 starts after
  H3 merges and rebases its verdict tests over H3's — if wrong, G1 lands later
  than it could have.
- **R-26** #643: the viewing package is a new public field of
  `ridl_sem::Resolution`, not a parameter threaded through nine callers — a
  struct literal outside this repository stops compiling, which the pull request
  body states under an API note; the release that carries it is already a
  breaking one (#792, #793), and a `Default` resolution with an empty package
  hides every internal declaration reached through a qualified path, the strict
  side — if wrong, a parameter replaces the field in a later change.
- **R-27** #729: the flake did not reproduce under load (slowest case about 4 s
  against the 10 s bound), so the fix (a 120 s bound for the five tests that do
  not test the timeout, pinned at or above the 60 s default) is applied on the
  issue's evidence, and the pull request says `Refs #729`, not `Closes` — the
  issue stays open with a comment saying what changed and that it closes on
  2026-11-09 if no further timing failure of those tests is seen in CI — if
  wrong, a different cause hides behind the longer bound and the issue stays
  open one month.
