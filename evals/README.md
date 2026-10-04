# Evaluation corpus and tasks

`evals/` holds public evidence for RIDL design evaluation, outside the published
crates and the book. Each directory under `corpus/` is a workspace translated
from pinned upstream definitions, with its own `PROVENANCE.md` and byte-exact
upstream `LICENSE`. The corpus guard runs `ridl check`, requires no Error
diagnostic, checks provenance headers, and counts all source lines, including
comments and blank lines. `tasks/` and `calibration/` hold the evaluation tasks
and calibration records as those stages are completed.

The
[porting rules](../docs/wip/2026-10-04-design-lints-design.md#33-porting-rules)
require upstream names, units, grouping and comments to survive translation.
Units and interface boundaries need upstream evidence; each set records its
fixed interaction-kind rule and every required representation deviation. Omit
out-of-subset declarations without stubbing them, and include required
dependency types. An independent review checks completeness, compares 20
declarations field by field and justifies every deviation before a port is
committed. Source budgets are 2,500 lines each for `ros2` and `mavlink`, 1,000
for `vss`, and 6,000 in total; expected warnings are preserved.

The [task format](../docs/wip/2026-10-04-design-lints-design.md#81-task-format)
uses one directory per stable task ID, with `task.toml`, `prompt.md` and
`rubric.md`. The manifest records a `review`, `evolve` or `design` task and its
compilation, lint or compatibility expectations. The prompt states the
designer's request without rubric hints. The rubric has numbered, one-sentence
items marked **must**, **should** or **must not**. Adding a task requires an
independent review and approval of the task set; review rubrics are written
before candidate checks run on the corpus.

The
[calibration procedure](../docs/wip/2026-10-04-design-lints-design.md#7-calibration)
starts with committed ports and independently written review rubrics, then
implements candidate checks and dumps their findings. Claude and Sol label
findings independently; the maintainer adjudicates disagreements and reviews a
sample of agreements. Committed labels determine fixed thresholds and default
levels: precision of at least 80% permits Warning, 50% to below 80% permits
Info, and below 50% excludes a lint; fewer than ten findings permits Info at
most. Recall against the reviewed issue inventory is reported without a gate.
The planned calibration records include labels, the reviewed recall mapping, the
summary and expected finding counts, so later changes are visible in review.
