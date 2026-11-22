# Release record of 2026-11-22

The release record of ADR-0035 decision 12 for the `ridl-authoring` skill, made
with `cargo xtask eval --host copilot --host codex --skill both --runs 3` at
commit `36790a04`.

- Hosts: Copilot CLI with `claude-sonnet-5.5`, and Codex CLI with `gpt-6.1-sol`.
- Judges: Codex judges the Copilot runs; Copilot with `claude-sonnet-5.5` judges
  the Codex runs (`[judge] anthropic = "copilot"` in `evals/runner.toml`).
- Runs: 12 tasks × 2 hosts × with and without the skill × 3 runs = 144. One
  judgement failed (`review-0002/codex-without-2`): its file records
  `error = "the judge reply is not JSON"`. That run has no rubric score, so it
  is left out of the rubric mean of codex without the skill. Its mechanical
  result passed and counts in the pass rate.
- Two runs exited with status 1: `review-0005/copilot-without-2` and
  `review-0005/copilot-without-3`. Both failed the mechanical checks (the agent
  neither called `ridl_check` nor ran `ridl check`), so they count as mechanical
  failures with a rubric score of 0.0.

**These numbers are provisional.** The runs used `ridl` 0.20.0 from `PATH` as
the agents' MCP server and as the scorer, while the workspace was at 0.21.0. The
result files do not record the version; the evidence is that the binary on
`PATH` was 0.20.0, and that its errors on the two start workspaces below match
the failures the result files record. ridl 0.20.0 rejects the start workspaces
of evolve-0003 and design-0004 with 5 errors. As a result:

- evolve-0003 fails `diff_matches` on all 12 runs, whatever the agent did.
- design-0004 fails `no_error` on all 6 Codex runs. The 6 Copilot runs pass only
  because they renamed the existing `Control` service, which the task prohibits,
  so their scores are not valid either.

The record is to be re-run with ridl 0.21.0 (driftsys/ridl#1460).

The gate holds on both hosts:

| Host    | Pass rate with | Pass rate without | Rubric with | Rubric without |
| ------- | -------------- | ----------------- | ----------- | -------------- |
| codex   | 0.81           | 0.67              | 0.64        | 0.50           |
| copilot | 0.81           | 0.58              | 0.64        | 0.48           |

`summary.md` has the scores per task. Each `<task>/<host>-<arm>-<n>.toml` holds
the mechanical result and the judge's verdict per rubric item of one run. The
transcripts are not committed.

**The judge was not checked.** The maintainer waived the judge check of decision
11 for this record, so the rubric scores rest on a judge that was not compared
with the maintainer's labels. The mechanical pass rates do not depend on the
judge. Decision 11 still applies to a later record.

**The measured skill is the one at commit `36790a04`.**
`skills/ridl-authoring/rules.md` changed after the run (driftsys/ridl#1450), so
the file on `main` is not the one that these runs used.
