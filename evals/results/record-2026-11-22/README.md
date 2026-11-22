# Release record of 2026-11-22

The release record of ADR-0035 decision 12 for the `ridl-authoring` skill, made
with the command below at commit `3c02e11c` (`main`) and `ridl` 0.21.0. Every
result file records `ridl_version = "ridl 0.21.0"`.

```text
RIDL_EVAL_RIDL=<abs path>/target/debug/ridl \
  cargo xtask eval --host copilot --host codex --skill both --runs 3
```

- Hosts: GitHub Copilot CLI 1.0.112 with `claude-sonnet-5.5`, and codex-cli
  0.169.0 with `gpt-6.1-sol`.
- Judges: Codex judges the Copilot runs; Copilot with `claude-sonnet-5.5` judges
  the Codex runs (`[judge] anthropic = "copilot"` in `evals/runner.toml`).
- Runs: 12 tasks × 2 hosts × with and without the skill × 3 runs = 144.
- Both CLIs failed to reach their services during the first pass of
  `review-0001` (Codex reported "workspace routing discovery failed"; Copilot
  exited with status 1 after about 1.3 s, with no model call), so no agent turn
  ran in 8 of its 12 runs. All 12 `review-0001` runs were run again with
  `--task review-0001 --append`, and the new runs replaced its results. No other
  task had an agent run with a non-zero exit status.
- Two judgements failed: `design-0002/codex-without-2` and
  `design-0002/codex-without-3` record `error = "the judge reply is not JSON"`.
  They have no rubric score, so they are left out of the rubric mean of codex
  without the skill. Their mechanical results count in the pass rate.

The gate holds on both hosts:

| Host    | Pass rate with | Pass rate without | Rubric with | Rubric without |
| ------- | -------------- | ----------------- | ----------- | -------------- |
| codex   | 1.00           | 0.69              | 0.82        | 0.56           |
| copilot | 0.97           | 0.64              | 0.73        | 0.53           |

`summary.md` has the scores per task. Each `<task>/<host>-<arm>-<n>.toml` holds
the mechanical result and the judge's verdict per rubric item of one run. The
transcripts are not committed.

**The judge was not checked.** The maintainer waived the judge check of decision
11 for this record, so the rubric scores rest on a judge that was not compared
with the maintainer's labels. The mechanical pass rates do not depend on the
judge. Decision 11 still applies to a later record.

**The measured skill is the one at commit `3c02e11c`.** No change to
`skills/ridl-authoring/` has landed on `main` since.

**This record replaces an earlier one of the same day.** The first record was
run with `ridl` 0.20.0 from `PATH` as the agents' MCP server, as the `ridl` an
agent ran from its shell, and as the scorer. That version rejects the start
workspaces of evolve-0003 and design-0004, so those results were not valid. The
runner now checks the binary before any run and puts it first on the agent's
`PATH`. The first record stays in git history, at commit `3c02e11c8` (the last
commit on `main` that touched this directory before this one).
