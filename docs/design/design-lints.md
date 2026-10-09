# Workspace design lints, `ridl_metrics` and the calibration corpus

This record describes, as built, the design lints — checks over the whole
checked workspace rather than one package — the dependency graph and the
cohesion groups they share with the MCP tool `ridl_metrics`, and the corpus and
the `xtask` command that set their levels and thresholds. The decisions behind
these choices are
[ADR-0027](../decisions/ADR-0027-design-lints-calibrated-on-a-corpus.md). The
lint table and the user-facing account of how the levels were set are on
[the lints page of the book](../book/lints.md); the inputs and outputs of
`ridl_metrics` are in [the `ridl-mcp` README](../../crates/ridl-mcp/README.md)
and its implementation in
[the MCP workspace tools design record](mcp-workspace-tools.md).

## Components

| Component                                                                    | Role                                                                                               |
| ---------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| [`crates/ridlc/src/design_lints/`](../../crates/ridlc/src/design_lints/)     | The shared pass, `check_design_lints`, and one module per shipped check                            |
| [`crates/ridlc/src/deps.rs`](../../crates/ridlc/src/deps.rs)                 | The package dependency graph, complete and workspace-only, and the rsdl component uses             |
| [`crates/ridl-mcp/src/metrics.rs`](../../crates/ridl-mcp/src/metrics.rs)     | `ridl_metrics`, over the graph and `cohesion_groups`                                               |
| [`crates/ridl-core/src/diag.rs`](../../crates/ridl-core/src/diag.rs)         | The catalogue rows TYPL-222, TYPL-224 and RIDL-414, and the retired-code lists for the two numbers |
| [`xtask/src/calibrate.rs`](../../xtask/src/calibrate.rs)                     | `cargo xtask calibrate dump` and `cargo xtask calibrate derive`                                    |
| [`evals/`](../../evals/)                                                     | The corpus, the calibration records and the eval tasks                                             |
| [`crates/ridl/tests/eval_corpus.rs`](../../crates/ridl/tests/eval_corpus.rs) | The corpus guard, the count guard and the task validator                                           |

## The pass

`ridlc::check_design_lints` takes one source set: the packages, the checked
packages, their resolutions, and the source map used to render every other
diagnostic. It loads no file and applies no lint level. It builds a `SiteIndex`
once ([`sites.rs`](../../crates/ridlc/src/design_lints/sites.rs)), which maps a
declaration, a struct field, an interaction member and a parameter to its source
span from the current syntax trees, then runs the three checks in a fixed order
(units, shapes, cohesion) and returns their diagnostics at the catalogue
severity.

Two callers run it:

- the shared compile in `ridlc`, after the per-package checks and the system
  checks, and before system lowering and `apply_lint_levels`, so `ridl check`,
  `ridl build`, `ridlc check`, `ridlc build`, `check_source` and the MCP tool
  `ridl_check` all report the same findings and apply the same levels;
- the language server's analysis path
  ([`crates/ridl-lsp/src/server.rs`](../../crates/ridl-lsp/src/server.rs)), with
  its current database inputs, unsaved buffers included: once for the loaded
  workspace, and once for each standalone overlay as a one-package set.

Every check skips the package `ridl.std`. A finding with no source span in the
site index is not reported.

## The checks

The thresholds are `pub(crate)` constants beside their check. Their values come
from the calibration and are recorded in
[`evals/calibration/summary.md`](../../evals/calibration/summary.md).

- **`inconsistent-unit`, TYPL-222**
  ([`units.rs`](../../crates/ridlc/src/design_lints/units.rs)). It collects a
  (name, unit) pair at each struct field, each `command` and `query` parameter,
  and each `signal`, `event` and fixed member, whose name is the member's name
  and whose unit is that of its payload type. The unit is the canonical UCUM
  form of the unit type the declared type resolves to, through aliases and
  `optional`; a site with no unit is skipped. When one exact name has two or
  more units, every site whose unit is not the most frequent one for that name
  is reported, and on a tie every site is. The check does not tell a different
  scale from a different dimension. No threshold.
- **`duplicate-shape`, TYPL-224**
  ([`shapes.rs`](../../crates/ridlc/src/design_lints/shapes.rs)). Two structs
  whose fields are equal as sorted lists of (name, qualified type), with at
  least `DUPLICATE_SHAPE_MIN_FIELDS` (2) fields, or two enums whose variant
  names are equal, with at least `DUPLICATE_SHAPE_MIN_VARIANTS` (2) variants.
  Types compare by their package-qualified name, so two types with the same
  simple name in different packages differ. Packages are visited in name order
  and declarations in source order; the first declaration with a shape is kept,
  and each later one is reported with a label on the first.
- **`low-cohesion-interface`, RIDL-414**
  ([`cohesion.rs`](../../crates/ridlc/src/design_lints/cohesion.rs)).
  `cohesion_groups` gives each interface member the set of named types it
  references directly — the payload of a `signal`, `event` or fixed member, the
  parameters of a `command`, and the parameters, return type and error type of a
  `query` — without expanding named type definitions, and without primitives or
  `ridl.std` types. A member with no type left is omitted. Members that share a
  type are joined (union-find), which gives the LCOM4 groups; names within a
  group are sorted, and groups follow their earliest member. An interface is
  reported at its declaration when it has at least `LOW_COHESION_MIN_GROUPS` (7)
  groups and its smallest group has at least `LOW_COHESION_MIN_GROUP_SIZE` (1)
  members; the message lists the groups.

The two candidates that did not ship, `inconsistent-abbreviation` and
`package-fan-out`, have no code left in the pass. The dependency graph and the
fan-out metric they used remain, for `ridl_metrics`.

## The dependency graph

[`deps.rs`](../../crates/ridlc/src/deps.rs) computes the package edges once.
`package_edges` returns, for each workspace package, the package qualifiers of a
walk over its IR references (the IR records no imports, so an import that no
declaration uses gives no edge), and the rsdl edges of `component_requires` and
the system's member lines; a qualifier that names no workspace package is kept
as written. `ridl_dependencies` reports that complete graph.
`workspace_package_edges` keeps only edges whose target is a workspace package;
`ridl_metrics` computes fan-in, fan-out, instability and `dependsOn` from it.

## The corpus and the calibration

```text
evals/
  README.md                  the corpus, the task format, the calibration procedure
  corpus/<set>/              ros2, mavlink, vss: one RIDL workspace each,
                             with PROVENANCE.md and the upstream LICENSE
  calibration/
    <lint-name>.toml         labelled findings of one candidate check
    recall.toml              the join between rubric items and findings
    summary.md               the output of `calibrate derive --write`, unchanged
    notes.md                 the method, the outcome and the recall mapping
    expected-counts.toml     findings per lint per workspace at default levels
  tasks/<id>/                task.toml, prompt.md, rubric.md
```

`summary.md` is in `.primignore`, because `prim fmt` would rewrap the tables the
derivation writes; `notes.md` is formatted and linted like any other Markdown.

[`cargo xtask calibrate`](../../xtask/README.md) has two subcommands:

- `dump <out-dir>` copies each corpus workspace into a temporary directory under
  `<out-dir>`, probes the built `ridl` for the candidate checks it ships, sets
  only those checks to `warn` in a `[lints]` table, runs
  `ridl check --format json`, and writes one findings file per candidate. The
  checks run at the thresholds compiled into them: the labelled dump was taken
  while each constant was at its search start (2 fields, 2 variants, 2 groups, a
  group size of 1), so a dump at the shipped constants no longer reports the
  cohesion findings below 7 groups, and the two dropped candidates' files are
  empty (the dump does not write them to `[lints]`, and it fails on any
  MANI-010). A finding has a stable ID built from its check, workspace, relative
  path, byte range and occurrence index, and a typed `metric` for a thresholded
  check, parsed from the diagnostic message; a missing or malformed value fails
  the dump.
- `derive` (also `--derive`) reads the five label files and `recall.toml`, and
  prints, per check, the precision at every candidate threshold, the recall
  against the rubric issues, and the level and threshold ADR-0027 decision 4
  gives; `--write` writes `summary.md`. Missing or invalid input exits with code
  2.

The levels and thresholds are copied from the summary into the catalogue and the
constants by hand, in the same change.

## Tests

- [`crates/ridlc/tests/design_lints.rs`](../../crates/ridlc/tests/design_lints.rs)
  — each check's rule, its sites, its boundaries at the threshold, and the
  `deps` edges, on small fixture workspaces.
- [`crates/ridl/tests/eval_corpus.rs`](../../crates/ridl/tests/eval_corpus.rs) —
  the corpus holds exactly the three selected workspaces, each checks with no
  Error diagnostic, and the line budgets hold; every task in `evals/tasks/` is
  well formed and names catalogue lints only; and
  `design_lint_counts_on_the_corpus_are_pinned` compares the findings per lint
  per workspace with `evals/calibration/expected-counts.toml`.
- [`crates/ridl-core/src/diag.rs`](../../crates/ridl-core/src/diag.rs) —
  `retired_typl_codes_are_never_redeclared` and
  `retired_ridl_codes_are_never_redeclared` keep TYPL-223 and RIDL-415 out of
  the catalogues.
- [`xtask/src/calibrate.rs`](../../xtask/src/calibrate.rs) —
  `committed_summary_is_the_derivation_of_the_committed_labels` compares
  `evals/calibration/summary.md` with the derivation of the committed labels,
  and its selected thresholds and levels with the compiler sources.
- [`crates/ridl-lsp/tests/server.rs`](../../crates/ridl-lsp/tests/server.rs) —
  the language server reports the design lints for unsaved text.
- [`xtask/tests/calibrate_cli.rs`](../../xtask/tests/calibrate_cli.rs) — the
  dump and derive command lines, their inputs and their failures.
- The MCP tool's tests and `tools.json` are listed in
  [the MCP workspace tools design record](mcp-workspace-tools.md#tests).
