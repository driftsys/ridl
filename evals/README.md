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
`rubric.md`. The manifest records a `review`, `evolve` or `design` task and its
compilation, lint or compatibility expectations. The prompt states the
designer's request without rubric hints. The rubric has numbered, one-sentence
items marked **must**, **should** or **must not**. Adding a task requires an
independent review and approval of the task set; review rubrics are written
before candidate checks run on the corpus.

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
