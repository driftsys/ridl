# ADR-0035 — The authoring skill, its gates, its distribution and its evals

## Status

Accepted — 2026-11-22. The record was written on 2026-11-19, when the design and
plan of 2026-10-11 were gardened. The maintainer made four rulings before the
design was written (because [ADR-0005](ADR-0005-agent-enablement.md) conflicted
with the work) and agreed decisions 1 to 9 of the design in the brainstorming
session of 2026-10-11. The skill, its gates, its distribution and the eval
runner were built in driftsys/ridl#893 and simplified in #963.

Amended 2026-11-22, when the maintainer accepted this record: decisions 11 (the
judge check) and 12 (the release record or gate) are deferred. The tooling
exists: `cargo xtask eval sample` and `cargo xtask eval agreement` (in
`xtask/src/eval.rs`), and the gate that `cargo xtask eval` writes in
`summary.md` (`xtask/src/eval/report.rs`). Neither is to be run until the
maintainer confirms the model ids in `evals/runner.toml` and approves the runs:
about 144 agent runs and up to 144 judge calls. driftsys/ridl#1440 tracks them.
The owner had dropped the optional plan task for them on 2026-11-19, and
driftsys/ridl#1401, which held that task, was closed that day. At that date
nothing in this repository stated a measured gain from the skill.

Amended 2026-11-22, the same day: the Anthropic-side judge of a Codex run may
run on the Copilot CLI with the pinned Copilot model (an Anthropic model)
instead of the Claude CLI, because the maintainer signs in to the CLIs only and
holds no Claude token. The `[judge]` table of `evals/runner.toml` names it, and
`--anthropic-judge` overrides it. Decision 10's rule that a judge is of the
other vendor still holds: Copilot judges only Codex runs, never a Claude or a
Copilot run. The two hosts of a record may therefore be Copilot and Codex, each
judged by the other. The pin of the Codex model changed to `gpt-6.1-sol`.

Amended 2026-11-22, after the record run: the release record of decision 12 was
made on Copilot (`claude-sonnet-5.5`) and Codex (`gpt-6.1-sol`), with three runs
per combination, 144 agent runs in all. It is committed as
[`evals/results/record-2026-11-22/`](../../evals/results/record-2026-11-22/README.md).
The gate holds on both hosts. The maintainer waived the judge check of decision
11 for this record: no labels were made, so the rubric scores rest on a judge
that was not checked against the maintainer. The mechanical pass rates do not
depend on the judge. Decision 11 still applies to a later record.

Scope: what the `ridl-authoring` skill is made of, how it is kept consistent
with the compiler, how it reaches each agent host, and how its effect is
measured. It binds every change to `skills/`, to the editor packaging of the
skill, and to `cargo xtask eval`. The shipped shape is described in
[the authoring skill design record](../design/authoring-skill.md).

Amends [ADR-0005](ADR-0005-agent-enablement.md) §2 (the provenance discipline),
§5 (the plane the evals run in) and §7 (the unit an agent writes), and adds a
dated amendment to [ADR-0027](ADR-0027-design-lints-calibrated-on-a-corpus.md).

## Context

ADR-0005 asked for a skill "generated from the reference docs", a rules file,
and an eval corpus that "runs in the same CI plane" as the snapshot tests. When
the work started, three of its premises no longer held:

- No `ridl doc` command exists, so there is nothing to generate the skill from.
- ADR-0012 retired `uxdl`, and `.rxdl` has no implementation, so the "total
  profile" file cannot be the unit an agent writes or an eval runs on.
- An eval run calls a model. It costs money, needs a login, and gives a
  different result on every run, so it cannot be a required CI check.

The decisions below replace those premises. The design and the plan numbered
them R-S-1 to R-S-4 (the rulings) and D-1 to D-9; the archived design keeps that
numbering.

## Decisions

1. **The skill is written by hand and gates keep it from drifting.** It replaces
   "generated from the reference docs" and the consequence "a hand-forked skill
   is explicitly disallowed" of ADR-0005. The protection against drift is
   decision 4, not generation (design R-S-1).
2. **The unit an agent writes, and an eval runs on, is a workspace**: a
   `ridl.toml` and its `.typl`, `.ridl` and `.rsdl` files. This replaces the
   `.rxdl` bullet of ADR-0005 §7 (design R-S-2).
3. **There is one skill, `ridl-authoring`, in `skills/ridl-authoring/`, and its
   files are `SKILL.md` and `rules.md`.** `SKILL.md` holds the family in a few
   lines, the verify loop, three short compiled samples and links to the
   published book. `rules.md` holds the always-on constraints, each citing a
   diagnostic code. The per-language references of the first design (`typl.md`,
   `ridl.md`, `rsdl.md`) were built and removed; ADR-0005's amendment of
   2026-11-11 records why and what reopens them (design D-1).
4. **The verify loop is the contract the skill teaches.** After every edit the
   agent checks the workspace with the `ridl_check` MCP tool, or
   `ridl check --format json` when the host has no MCP, and looks a code up with
   `ridl_explain`. Before it edits an existing workspace it copies the workspace
   outside itself, and afterwards it compares the copy with the result with
   `ridl_diff` or `ridl diff`. It ends with the result of the check and of the
   diff (design D-2).
5. **Three tests in `crates/ridl/tests/` keep the skill consistent with the
   compiler, and all three run in `just build` through the existing test
   recipes** (design D-3):
   - every `typl`, `ridl` and `rsdl` fence under `skills/ridl-authoring/`
     compiles by the book's fence rules, as a workspace separate from the book
     (`skill_examples_compile` in `book_examples.rs`);
   - every `TYPL-`, `RIDL-`, `RSDL-`, `FORM-` and `MANI-` code written in the
     skill is a line of `freeze/diagnostics.txt` (`skill_gates.rs`), which also
     covers the skill's files for the gap recorded on driftsys/ridl#191;
   - every section of `SKILL.md` links a heading of the published book and every
     link of every skill file resolves (`skill_gates.rs`). A link names the
     published book (`https://driftsys.github.io/ridl/<page>.html#<anchor>`)
     because a relative link resolves nowhere the skill is installed; the test
     stays offline by mapping the page to its source under `docs/book/`.
6. **The skill reaches each host as a release asset or as a VS Code
   contribution, and its version is the ridl version** (design D-4). The editor
   release attaches `ridl-authoring-<version>.zip`, built by
   `just package-skill`, for Claude Code (`~/.claude/skills/` or a project's
   `.claude/skills/`) and Codex (`~/.codex/skills/`). The VS Code extension
   packages the directory and contributes `SKILL.md` as a `chatSkills` entry and
   `rules.md` as a `chatInstructions` entry, for GitHub Copilot.
   `metadata.version` in the frontmatter of `SKILL.md` is a `[[version_files]]`
   entry of `.git-std.toml`, so `just release` writes it.
7. **The extension needs VS Code 1.109.** `chatSkills` is not in the published
   contribution-points reference. VS Code 1.109.0 is the first release whose
   workbench registers it; 1.108.2 does not. `engines.vscode` is `^1.109.0` and
   the pinned `@types/vscode` is `1.109.0`. A VS Code fork older than 1.109 can
   no longer install the extension at all, which is the kind of cost ADR-0005
   accepted for the MCP registration API. A report from a user on an older fork
   reopens this decision.
8. **CI runs only the parts of the evals that give the same result on every run,
   and never calls a model.** CI checks the task format (the three files of a
   task exist, the manifest parses, the rubric items are numbered and carry a
   marker) and the scoring and reporting code's unit tests. The comparison with
   and without the skill runs on demand with `cargo xtask eval`, and its results
   are committed as a record under `evals/results/`, as `evals/calibration/` is.
   This replaces "runs in the same CI plane" in ADR-0005 §5 (design R-S-3 and
   D-8).
9. **The task set is the ten tasks of ADR-0027 and two rsdl tasks on
   `examples/cabin`**: `design-0004` adds a component and wires it into the
   cabin system, and `evolve-0003` changes the system compatibly. A task may
   carry a `start/` workspace to start from (design D-5).
10. **A run is scored mechanically first, and a run that passes is then marked
    against its rubric by a judge model of the other vendor** (design D-6). The
    mechanical checks are: `ridl check` reports no error; the shipped design
    lints reported equal `expect.lints`; for an evolve task, `ridl diff` against
    the starting workspace gives `expect.diff`; and the agent checked its work.
    A run that fails a check scores zero on the rubric. **must** and **must
    not** items weigh 2 and **should** items weigh 1. The judge sees the prompt,
    the rubric, the final workspace and the agent's final message, and nothing
    that says whether the skill was loaded. Claude and Copilot runs are judged
    by Codex, and Codex runs by Claude or, since the amendment of 2026-11-22, by
    the Copilot CLI running an Anthropic model.
11. **The judge is checked once against the maintainer's labels, and a release
    record is made only if it agrees.** The maintainer labels the rubric items
    of a sample of 20 judged runs; agreement per item below 80% blocks the
    release record until the judge prompt is revised and the check repeated.
    `cargo xtask eval sample` and `cargo xtask eval agreement` are built; the
    check has not been run (design D-6).
12. **The release gate is a comparison, not a threshold.** For each host, the
    mechanical pass rate with the skill is at least the rate without it, and the
    mean rubric score with the skill is at least the score without it; a host
    with no scored run on one side is undecided, not passing. The record is made
    with three runs per combination (12 tasks, 2 hosts, both arms: 144 agent
    runs and up to 144 judge calls). While the skill is written, one run per
    combination, filtered by task or host, is enough. `cargo xtask eval`
    computes the gate and writes it in `summary.md`; no record has been made
    (design D-9).

## Alternatives considered

- **Generate the skill from the references** (ADR-0005 as written). Rejected by
  decision 1: no generator exists, and a curated skill with compiled fences and
  checked codes gives the same protection against drift.
- **Score by mechanical checks only.** Rejected: five of the ten seed tasks are
  review tasks, whose output is prose that the compiler cannot score.
- **Score by two blind labellers with the maintainer adjudicating**, as the
  design lints did (ADR-0027 decision 6). Rejected for repeated use: a labelling
  round per skill change is too slow to catch a regression. It is kept once, as
  the judge check (decision 11).
- **`ridl skill install --host ...`.** Deferred: it adds a CLI subcommand, and
  the CLI surface froze in 0.10. It can be added later, by addition.
- **Runs by hand, scored by the xtask.** Rejected: 144 manual sessions per
  release record would not be repeated.
- **A fixed improvement threshold** (for example 20 percentage points).
  Rejected: no evidence supports a number before the first run.
- **Ship the rsdl part of the skill without measuring it.** Rejected: two rsdl
  tasks are in the task set (decision 9).
- **Keep the per-language references.** Withdrawn: a pilot with one run per task
  did not show a gain, and one run per task cannot separate a gain from noise.
  The references return only if an eval with several runs per task shows one
  (decision 3). Further alternatives to the withdrawal are not documented.
- **Hold `engines.vscode` below 1.109 and declare the `chatSkills` contribution
  by hand.** Not documented as considered; the plan states the bump as a
  requirement.

## Consequences

- `skills/` is Markdown only. A change to a diagnostic code, a book heading or a
  fenced sample that the skill cites fails a test in `just build`, not a review.
- The skill's behaviour is not measured in this repository. The gate and the
  comparison are built and unit-tested, and the judge is unchecked. Until the
  judge check and the release record are done, no document may state that the
  skill improves an agent's result.
- `evals/runner.toml` pins the model of each host; the pinned ids have not been
  confirmed by the maintainer for a release record.
- The skill's version moves with every release, so each release changes
  `SKILL.md` and the editor release attaches a new archive.
- The Copilot CLI is a third eval host that a run selects with `--host copilot`;
  it is outside the default host list, and it judges only Codex runs, never a
  Claude or a Copilot run (the amendment of 2026-11-22). The first design left
  Copilot out of the measured comparison and kept a smoke check by hand. The
  smoke check by hand is not recorded.
- A VS Code fork older than 1.109 cannot install the extension (decision 7).

## Documents amended

| Document                                                    | Change                                                                                        |
| ----------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| [ADR-0005](ADR-0005-agent-enablement.md)                    | A dated amendment records decisions 1, 2 and 8, and marks three open questions resolved.      |
| [ADR-0027](ADR-0027-design-lints-calibrated-on-a-corpus.md) | A dated amendment records the `start/` directory of a task and the three hosts of the runner. |
| [ADR-0025](ADR-0025-workspace-aware-mcp-tools.md)           | Decision 4 no longer says that the skill does not exist.                                      |

## Traceability

Traces: driftsys/ridl#73 (the rules file) is built. #74, #75 and #87 asked for
the typl, ridl and rsdl skills; their per-language references are withdrawn
(decision 3), and `SKILL.md` carries one short sample of each language. ADR-0005
§2 and §5.

Design: [the authoring skill design record](../design/authoring-skill.md).
Source material, archived verbatim:
[the design](../archive/2026-10-11-ridl-authoring-skill-design.md) and
[the plan](../archive/2026-10-11-ridl-authoring-skill-plan.md).

Tracking: driftsys/ridl#1440 (the judge check and the release record). The
earlier issue driftsys/ridl#1401 was closed on 2026-11-19.
