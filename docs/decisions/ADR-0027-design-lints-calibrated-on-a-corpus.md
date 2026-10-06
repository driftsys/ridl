# ADR-0027 — Workspace design lints

## Status

Accepted — 2026-10-06. Scope: how a check over the whole checked workspace (a
design lint) is built, how its default level and its threshold are chosen, the
evidence that choice rests on, the MCP tool that reports the underlying metrics,
and the format of the eval tasks. It binds every later design lint, every change
to the level or threshold of a shipped design lint, the corpus and the
calibration records under `evals/`, and every eval task.

Written from piece 1b of the devex and agent tracks brief, "design lints and
metrics", implemented in four pull requests: the design and plan
(driftsys/ridl#694), the corpus and the eval seed (#707), the checks and the
tool (#712), and the labelling and calibration (#738). Sebastien agreed
decisions D-1 to D-8 of the design in the brainstorming session of 2026-10-04
and approved the written design, D-9 included, the same day. He approved the
eval task set on 2026-10-04, decided every labelling disagreement and reviewed a
sample of agreed labels on 2026-10-06, and approved the derived calibration
summary the same day. The plan's controller took the rulings recorded in
decisions 2, 3, 7 and 11 while it implemented the design.

The design numbers its decisions D-1 to D-9. In this record decision 1 is design
§5 with plan ruling 2, decision 2 is design §5 with plan ruling 1, decision 3 is
D-4, decision 4 is D-5 with design §11, decision 5 is D-1 to D-3, decision 6 is
D-6, decision 7 is D-9 with plan rulings 4 and 96 to 99, decision 8 is design
§7.5, decision 9 is D-7, decision 10 is D-8, decision 11 is the outcome of the
calibration (plan rulings 100, 104 and 109), and decision 12 is design §5,
"Codes". The original design and plan are
[`docs/archive/2026-10-04-design-lints-design.md`](../archive/2026-10-04-design-lints-design.md)
and
[`docs/archive/2026-10-04-design-lints-plan.md`](../archive/2026-10-04-design-lints-plan.md);
the design's §10 holds the alternatives, restated below.

It amends two records in place, in the same change:
[ADR-0024](ADR-0024-lint-registry-and-levels.md) decision 1 states that a design
lint's catalogue severity comes from decision 4 below, and
[ADR-0025](ADR-0025-workspace-aware-mcp-tools.md) records, in its status and its
decision 9, the tool `ridl_metrics` (decision 9 below) and the move of the
dependency edges into `ridlc` (decision 2 below).

The as-built pass, checks, tool and calibration tooling are described in
[the design lints design record](../design/design-lints.md). The user-facing
description, with the lint table, is
[the lints page of the book](../book/lints.md#how-the-design-lints-levels-were-set).
The calibration records are
[`evals/calibration/summary.md`](../../evals/calibration/summary.md), the output
of the derivation, and
[`evals/calibration/notes.md`](../../evals/calibration/notes.md), the
hand-written account of the method and the outcome.

## Context

The design metrics a reviewer cites — a unit used inconsistently, two types with
the same shape, an interface that does several unrelated things, a package with
many dependencies — are easy to compute. A lint that reports them by default, at
a level chosen by hand, risks reporting findings a designer would dismiss, and a
designer who dismisses lint findings learns to ignore all of them. The existing
lints of `ridl-sem/src/lint.rs` see one package at a time, and these checks need
the whole checked workspace. ADR-0024 made every Warning and Info catalogue code
a configurable lint, but said nothing about how the default level of a new lint
is chosen.

The work therefore had to answer two questions with evidence: which checks
become lints, at which default level, and, for a check with a numeric threshold,
which threshold. The evidence had to be something other than workspaces written
by the people who set the thresholds. The same evidence was to seed the eval set
of the authoring skill (piece 1c), as ADR-0005 §5 requires.

## Decision

1. **The design lints run in one shared pass in `ridlc`, after the per-package
   checks and before `apply_lint_levels`.** `ridlc::check_design_lints` takes
   the checked packages, their resolutions, the standard IR, the lowered system
   and the source map, loads no file and applies no level. It emits each code at
   its catalogue severity, so ADR-0024's level handling applies with no change.
   It runs wherever the shared compile runs, including `check_source`, where the
   workspace is one package. The language server calls the same pass from its
   analysis path with its current inputs, unsaved buffers included: the loaded
   workspace as one set, and each standalone overlay as its own one-package set.
   Types of `ridl.std` are excluded from every check.

2. **The package dependency edges are computed once, in `ridlc::deps`.**
   `package_edges` returns the complete graph, external package qualifiers
   included, and `ridl_dependencies` reports it unchanged.
   `workspace_package_edges` keeps only the edges whose target is a workspace
   package; every metric (fan-in, fan-out, instability and `dependsOn`) uses
   that view. The function moved from `ridl-mcp` into `ridlc` so that the
   compiler and the MCP tools share it.

3. **A finding and a metric are different things.** A finding is a yes-or-no
   fact about one place. A metric is a number about a package or an interface,
   and becomes a finding only through a threshold. Package import cycles are not
   a design check, because a cycle is already an Error (TYPL-004). Naming
   consistency was limited to abbreviations; a synonym vocabulary (`speed`
   against `velocity`) is left to the authoring skill, because it would be a
   curated list the project has to maintain.

4. **A design lint's default level comes from its precision on the corpus.**
   Precision is the share of a check's corpus findings that the adjudicated
   labels accept.
   - At least 80 %: the catalogue row is a Warning, default level `warn`.
   - From 50 % up to 80 %: the catalogue row is an Info, default level `info`.
   - Below 50 %: the check does not ship as a lint. A metric stays available
     through `ridl_metrics` (decision 9).
   - A check with fewer than ten findings at a candidate threshold ships at Info
     at most, because the sample is too small for a Warning.

   A threshold is a constant beside its check (`DUPLICATE_SHAPE_MIN_FIELDS`,
   `LOW_COHESION_MIN_GROUPS`, as `SHARED_ERROR_INTERFACE_THRESHOLD` already was
   for RIDL-405). Its value is the least strict candidate whose precision still
   meets the level it ships at. Ties between candidates with equal retained
   counts are broken deterministically: for cohesion, fewer groups and then a
   smaller minimum group size; for shapes, fewer fields and then fewer variants.
   A threshold is not configurable in `ridl.toml`; ADR-0024 decision 2 sets
   levels only. The 80 % bar, the 50 % floor and the ten-finding minimum are
   parameters of this procedure: changing one re-runs the derivation over the
   committed labels, with no new labelling.

5. **The evidence is a public corpus of interface sets designed by other people,
   committed under `evals/`.** Three sets were ported into RIDL from pinned
   upstream revisions: a ROS 2 subset (robotics), a MAVLink `common` subset
   (drones and autopilots) and the COVESA VSS `Vehicle.Cabin.HVAC` branch
   (automotive), with a budget of 2,500 lines each for the first two, 1,000 for
   the third and 6,000 in total, so that automotive is at most one third. The
   porting rules keep the upstream design as it is, defects included, because
   the defects are the evidence: every upstream name, unit, grouping and comment
   survives; a unit is written only where upstream states one; an interface
   boundary is drawn only where upstream documents one; and each set fixes its
   rule for interaction kinds before the port starts. Each set has a
   `PROVENANCE.md` with its upstream, revision, licence, subset, kind rule and
   every deviation, and its byte-exact upstream `LICENSE`.
   `THIRD-PARTY-NOTICES.txt` has one entry per set. `evals/` is outside
   `crates/`, so no published crate and not the book ships it. Private
   workspaces are not used.

6. **Labels are double-blind and adjudicated by the maintainer.** Two labellers
   each label every finding `accept` or `dismiss` with a one-line reason,
   without seeing the other's labels or the design's expectations. The question
   is "would a designer reviewing this workspace change the design because of
   this finding?" Where they agree the label stands; where they disagree the
   maintainer decides; and the maintainer also reviews a sample of agreed
   labels: up to ten per check, 31 in the 1b calibration (ten each for
   `duplicate-shape`, `inconsistent-abbreviation` and `low-cohesion-interface`,
   the one agreed `package-fan-out` label, and none for `inconsistent-unit`,
   which had no agreed label). All three columns are committed in
   `evals/calibration/<lint-name>.toml`. In the 1b calibration one labeller
   labelled through a rule-based script of its own; its labels were kept as
   produced, and the method is stated in the notes.

7. **Recall is measured against review rubrics written before the checks ran,
   and is reported, never gated.** The review tasks' rubrics list the design
   issues of a corpus workspace and were committed before any check ran on the
   corpus, so they are an independent reference and do not anchor the labels on
   the checks' own output. After adjudication, `evals/calibration/recall.toml`
   records the join: each numbered rubric item has the stable ID
   `<task-id>:<item-number>`; a **must** or **should** item that names a
   property of the design is an issue, one that recommends the remedy for
   exactly one issue is an alias of it, and a **must not** item is excluded. A
   check is applicable to an issue only when the issue's subject is the property
   the check computes; each applicable issue lists the findings that match it,
   whatever their label, because labels measure precision and matches measure
   recall. A zero denominator is reported as `not applicable`. No level or
   threshold depends on recall.

8. **A test pins the number of findings each design lint reports on each corpus
   workspace.** `design_lint_counts_on_the_corpus_are_pinned` in
   `crates/ridl/tests/eval_corpus.rs` compares the counts at default levels with
   `evals/calibration/expected-counts.toml`; a change that moves a count fails
   until its author updates the file, so the change is visible in review. A
   missing file reads as an empty table and a file that does not parse is an
   error. The same test file checks that the corpus holds exactly the three
   selected workspaces, that each checks with no Error diagnostic, and that the
   line budgets hold.

9. **`ridl_metrics` reports the metrics, whatever the lint levels.** The
   read-only, offline MCP tool takes `{path}` and returns, for every workspace
   package, fan-in, fan-out, instability (fan-out divided by fan-in plus
   fan-out, `null` for a package with neither) and `dependsOn`, and for every
   declared interface its member count and its cohesion groups, with no
   threshold, plus the `workspace` object the other path tools return. The
   values are those the lints compute. It is an addition to the tool surface
   under ADR-0025 decision 9 and is pinned in
   `crates/ridl-mcp/tests/tools.json`. It lets the authoring skill cite a number
   for every check, shipped or not.

10. **An eval task is a directory of three files.** `task.toml` holds the
    machine-checkable expectations (`id`, `kind` = `review`, `evolve` or
    `design`, `corpus` for review and evolve tasks, `title`, and an `[expect]`
    table with `compiles`, `lints` and the `ridl_diff` verdict `diff`);
    `prompt.md` is what the designer asks, with no hint of the rubric; and
    `rubric.md` lists numbered one-sentence items, each marked **must**,
    **should** or **must not**. Item numbers are stable IDs and are never
    renumbered. A review task's `expect.lints` names a shipped lint only where
    an accepted finding retained at the shipped threshold matches one of its
    rubric items. The seed is ten tasks — five review, two evolve and three
    design — weighted toward review and evolution, with at most one third on the
    VSS set. A test checks that each task parses, names an existing corpus
    workspace and catalogue lints only, and has non-empty prompt and rubric
    files. No harness and no scoring are built; ADR-0005's open question on
    scoring beyond "compiles" stays open.

11. **Three design lints ship at Info; two candidates did not ship.** The
    derivation over the 820 adjudicated findings gave:
    - `inconsistent-unit` (TYPL-222): 7 of 7 accepted, fewer than ten findings,
      so Info, with no threshold.
    - `duplicate-shape` (TYPL-224): at least 2 fields and at least 2 variants, 8
      of 16 accepted (50 %, exactly the floor), so Info. No candidate that
      reached 80 % retained ten findings.
    - `low-cohesion-interface` (RIDL-414): at least 7 groups with a minimum
      group size of at least 1, 1 of 2 retained findings accepted, so Info.
    - `inconsistent-abbreviation` (0 of 780 accepted) and `package-fan-out` (0
      of 1) fell below the floor. Their emitters, catalogue rows, book rows and
      tests were removed before release. The package coupling numbers stay in
      `ridl_metrics`.

    Sebastien was shown the stricter alternative for `duplicate-shape` (at least
    4 fields, 8 of 8, still Info because fewer than ten findings remain) and
    chose the threshold decision 4 selects.

12. **A design lint about the type vocabulary takes a TYPL-2xx code, and one
    about interfaces or coupling takes a RIDL-4xx code.** Every candidate was an
    Info catalogue row during calibration, so that it ran and reported. A check
    that does not ship loses its row before release, so its name was never
    released and nothing is reserved; its code number is not reused, and a
    comment in the catalogue says so. TYPL-223 and RIDL-415 are those two
    numbers. ADR-0024 decision 7 applies to each shipped lint.

## Alternatives considered

From the design's §10. The numbers are the decisions that reject them.

- **Private workspaces as evidence.** The most realistic, but they cannot be
  committed or named, so nobody else can rerun the evidence. Rejected for
  decision 5.
- **A private repository for the corpus and the tasks.** Protects the rubrics
  from training data, but moves the count guard out of this repository's CI, so
  a lint change that misfires on real designs would not fail its own pull
  request. Rejected for decision 5; a private hold-out set stays open to the
  authoring skill's evals, and adding one later removes nothing from `evals/`.
- **Workspaces written by hand for the purpose.** Fully controlled, but designed
  by the same people who set the thresholds, which is the bias the corpus exists
  to avoid. Rejected for decision 5.
- **Configurable thresholds in `[lints]`** (`{ level = "warn", max = 8 }`). More
  flexible, but it changes the manifest schema under ADR-0002 and makes every
  parameter a released contract. Left as a follow-up for a project that asks for
  it; rejected for decision 4.
- **Metrics only through a tool, never as lints.** Matches the split between
  findings and metrics most literally, but then no CI ever reports a coupling
  problem. Kept as the outcome for a metric below the floor, not as the rule
  (decisions 4 and 9).
- **An off-by-default catalogue level** (Clippy's pedantic group). Would let a
  noisy check ship and be opted into, but needs an amendment to ADR-0024 and its
  catalogue guard. The Info level serves the 50 % to 80 % band instead (decision
  4).
- **A synonym vocabulary for naming.** Catches `speed` against `velocity`, at
  the cost of a curated list. Left to the skill (decision 3).
- **Package cycles as a design check.** Already an Error (decision 3).

## Consequences

- A new design lint, or a change to the level or threshold of a shipped one, is
  justified by labelled findings on the corpus, not by judgement alone. Adding a
  candidate means adding an Info row, dumping its findings with
  `cargo xtask calibrate dump`, labelling and adjudicating them, and running
  `cargo xtask calibrate derive --write`. A dump at the shipped constants does
  not reproduce the labelled `low-cohesion-interface` findings with 2 to 6
  groups, because the shipped minimum is 7 groups; the labelled dump used the
  search-start constants.
- The corpus is small. Every shipped design lint has fewer than ten findings at
  some candidate threshold, or a precision under 80 %, so all three ship at
  Info, and none can reach `warn` until the corpus grows. The calibration notes
  state, per check, which sets its findings come from.
- The porter's choices shape the evidence: `low-cohesion-interface` in
  particular measures interfaces the porter grouped. The fixed kind and boundary
  rules limit this, and every remaining choice is listed in a `PROVENANCE.md`.
- Two labellers can share a bias. The maintainer's review of the disagreements
  and of a sample of agreed labels is the only check on it.
- A language or checker change that moves a design-lint count on the corpus
  fails the count guard; its author updates `expected-counts.toml` and states
  the reason in the commit message.
- The two dropped checks' numbers, TYPL-223 and RIDL-415, are not reused.
- The language server reports the design lints for unsaved text, through the
  same pass as the command line.

## References

- [ADR-0024](ADR-0024-lint-registry-and-levels.md) — the lint registry and
  levels; decision 7 governs each shipped lint, and decision 4 here amends it.
- [ADR-0025](ADR-0025-workspace-aware-mcp-tools.md) — the MCP tools; decision 9
  admits `ridl_metrics` as an addition.
- [ADR-0005](ADR-0005-agent-enablement.md) — agent enablement; §5 makes evals
  part of the deliverable.
- [The design lints design record](../design/design-lints.md) — the pass, the
  checks, the tool and the calibration tooling as built.
- [The MCP workspace tools design record](../design/mcp-workspace-tools.md) —
  `ridl_metrics` as built.
- [The lints page of the book](../book/lints.md) — the lint table and how the
  design lints' levels were set.
- [`evals/README.md`](../../evals/README.md) — the corpus, the task format and
  the calibration procedure, for a contributor.
- [`xtask/README.md`](../../xtask/README.md) — `cargo xtask calibrate`.
