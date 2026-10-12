# Evaluation corpus and tasks

`evals/` holds public evidence for RIDL design evaluation, outside the published
crates and the book. Each directory under `corpus/` is a workspace translated
from pinned upstream definitions, with its own `PROVENANCE.md` and byte-exact
upstream `LICENSE`. The corpus guard runs `ridl check`, requires no Error
diagnostic, checks provenance headers, and counts all source lines, including
comments and blank lines. `tasks/` and `calibration/` hold the evaluation tasks
and calibration records as those stages are completed.

The
[porting rules](../docs/decisions/ADR-0027-design-lints-calibrated-on-a-corpus.md)
require upstream names, units, grouping and comments to survive translation.
Units and interface boundaries need upstream evidence; each set records its
fixed interaction-kind rule and every required representation deviation. Omit
out-of-subset declarations without stubbing them, and include required
dependency types. An independent review checks completeness, compares 20
declarations field by field and justifies every deviation before a port is
committed. Source budgets are 2,500 lines each for `ros2` and `mavlink`, 1,000
for `vss`, and 6,000 in total; expected warnings are preserved.

The
[task format](../docs/decisions/ADR-0027-design-lints-calibrated-on-a-corpus.md)
uses one directory per stable task ID, with `task.toml`, `prompt.md` and
`rubric.md`. A task may also hold `start/`, a workspace the agent starts from; a
design task without it starts from an empty directory, and a review or evolve
task names exactly one of a `corpus` workspace and `start/`. The manifest
records a `review`, `evolve` or `design` task and its compilation, lint or
compatibility expectations. The prompt states the designer's request without
rubric hints. The rubric has numbered, one-sentence items marked **must**,
**should** or **must not**. Adding a task requires an independent review and
approval of the task set; review rubrics are written before candidate checks run
on the corpus.

The
[calibration procedure](../docs/design/design-lints.md#the-corpus-and-the-calibration)
starts with committed ports and independently written review rubrics, then
implements candidate checks and dumps their findings. Claude and Sol label
findings independently; the maintainer adjudicates disagreements and reviews a
sample of agreements. Committed labels determine fixed thresholds and default
levels: precision of at least 80% permits Warning, 50% to below 80% permits
Info, and below 50% excludes a lint; fewer than ten findings permits Info at
most. Recall against the reviewed issue inventory is reported without a gate.
The calibration records are the labels (`calibration/<check>.toml`), the
reviewed recall mapping (`calibration/recall.toml`), the derivation tables
(`calibration/summary.md`), the hand-written notes (`calibration/notes.md`) and
the expected finding counts (`calibration/expected-counts.toml`), so later
changes are visible in review.

## Results

The calibration ran over 820 findings dumped at revision `8ef2f28d` (a dump at
`200185f6` was byte-identical) and labelled blind by Claude (`fable`) and Sol
(`gpt-6.1-sol`); the maintainer adjudicated the 8 disagreements and reviewed 31
sampled agreements. Three of the five candidate checks ship as lints, all at
Info:

| Check                       | Precision    | Threshold                        | Level                          | Recall         |
| --------------------------- | ------------ | -------------------------------- | ------------------------------ | -------------- |
| `inconsistent-unit`         | 7/7          | none                             | Info (fewer than ten findings) | not applicable |
| `duplicate-shape`           | 8/16         | fields >= 2, variants >= 2       | Info                           | 3/3            |
| `low-cohesion-interface`    | 1/2 retained | groups >= 7, min group size >= 1 | Info (fewer than ten findings) | 1/1            |
| `inconsistent-abbreviation` | 0/780        | none                             | not a lint                     | not applicable |
| `package-fan-out`           | 0/1          | fan-out > 3                      | not a lint                     | not applicable |

The `duplicate-shape` recall of 3/3 counts `review-0003:1` on a broad reading of
that rubric item; `calibration/notes.md` gives the stricter readings, 2/2 and
1/1.

`calibration/summary.md` is the unchanged output of the xtask `calibrate`
subcommand `derive` with `--write` (the xtask test
`committed_summary_is_the_derivation_of_the_committed_labels` compares the two
byte for byte, and compares the selected thresholds and levels with the
constants and catalogue levels in the compiler sources): the precision at every
candidate threshold and the level each one gives. `calibration/notes.md` is
written by hand and records the labelling method, the adjudication, the outcome
applied to the compiler, and the recall mapping with its judgement calls. Each
review task's `expect.lints` names the shipped lints whose accepted findings
fall in that task's workspace and match a rubric item.
`calibration/expected-counts.toml` pins the per-workspace counts of the shipped
lints; the test `design_lint_counts_on_the_corpus_are_pinned` compares them with
`ridl check` and prints the actual table on a mismatch.

## Running the eval tasks

`cargo xtask eval` runs each task on Claude Code and on Codex, with and without
the `ridl-authoring` skill from `skills/`, scores each run, and writes the
results:

```sh
cargo xtask eval [--task <id>]... [--host claude|codex]... [--runs <n>] \
  [--skill with|without|both] [--out <dir>] [--append] [--claude-handoff]
```

With no option, every task runs once on both hosts, with and without the skill.
Each run starts in a new directory under the system temporary directory, outside
any git repository, with an empty host home, so no user skill, rule, memory,
hook or `AGENTS.md` reaches the agent. The directory is removed when the run's
result is written. A run is scored mechanically (`ridl check` reports no error,
the design lints match `expect.lints`, an evolve task's `ridl diff` verdict
matches `expect.diff`, and the agent checked its work with the `ridl_check` MCP
tool or a shell `ridl check`); a run that passes is then judged against its
rubric by the other host's model. `runner.toml` pins the model of each host.

Each host signs in with its own CLI login, and both are needed, because the
judge of a run on one host is the other host:

- Codex uses the stored login that `codex login` writes, `auth.json` in
  `$CODEX_HOME` or in `~/.codex`. Each Codex run gets a copy of that file only,
  and a token that the run refreshed is copied back.
- Claude reads `CLAUDE_CODE_OAUTH_TOKEN`, which `claude setup-token` prints, or
  `ANTHROPIC_API_KEY`.

With `--claude-handoff`, no `claude` command runs and no Claude token is needed.
For each Claude agent run and each Claude judgement, the eval writes a
`*.request.toml` file in the run's directory, prints its path after
`eval: handoff:`, and waits. The request holds the working directory, the
arguments, the environment and the prompt; the operator runs the prompt with an
agent of their own, writes its output to the request's `output` path, and then
creates the file its `done` path names. The result of a handed-off run or
judgement records `handoff = true` and `model = "handoff"` (`judge_model` for a
judgement), because the pinned model did not necessarily run.

The results go to `--out`, by default `evals/results/<YYYY-MM-DD>/`: one
`<task>/<host>-<with|without>-<n>.toml` per run, the run's transcript under
`transcripts/` (not committed), and `summary.md` with the pass rates, the rubric
means and the release gate for each host. The summary and the gate read every
result in the directory, so the eval refuses an `--out` that already holds
results; `--append` adds the new runs to them on purpose.

`cargo xtask eval sample --from <dir> [--size <n>]` writes
`<dir>/judge-check/labels.toml`, a deterministic sample of the judged runs for
the maintainer to label, and `cargo xtask eval agreement <labels.toml>` writes
`agreement.md` next to it and exits 1 when the judge agrees with the labels on
fewer than 80% of the rubric items.
