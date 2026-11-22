# The authoring skill and its eval runner

This record describes the `ridl-authoring` agent skill, the tests that keep it
consistent with the compiler, the way it reaches each agent host, and the eval
runner that compares an agent with and without it, as built. The code and its
tests are the source of truth. The decisions are in
[ADR-0035](../decisions/ADR-0035-authoring-skill-and-evals.md), and the layer of
agent enablement that the skill belongs to is in
[ADR-0005](../decisions/ADR-0005-agent-enablement.md). The command-line options
of the runner are in
[the evals README](../../evals/README.md#running-the-eval-tasks) and are not
repeated here.

## What is not built

- The per-language references (`typl.md`, `ridl.md`, `rsdl.md`) do not exist.
  They were written and removed, and ADR-0005 records the reason and what
  reopens them.
- The judge check has not been run, so neither `<dir>/judge-check/labels.toml`
  (written by `cargo xtask eval sample --from <dir>`) nor the `agreement.md`
  next to it (written by `cargo xtask eval agreement <labels.toml>`) exists, and
  the judge is not known to agree with the maintainer. The maintainer deferred
  the check on 2026-11-22 (ADR-0035 status, driftsys/ridl#1401).
- No release record exists: `evals/results/` is absent, and the skill's effect
  on an agent's result is not measured.
- The model ids in `evals/runner.toml` are pinned but not confirmed for a
  release record.

## The skill

```text
skills/ridl-authoring/
  SKILL.md   frontmatter, the family, the verify loop, three samples, links
  rules.md   19 always-on rules, each citing a diagnostic code
```

`SKILL.md` has the frontmatter that Claude Code, Codex and Copilot read: `name`
(equal to the directory name), `description`, and `metadata.version`. Its body
has four sections: what RIDL is, "Check your work" (the verify loop of ADR-0035
decision 4), "A first look" (a `typl` sample, a `ridl` sample and a `ridl` with
`rsdl` sample, all compiled) and "Where to find the docs" (links to the
published book). Every link names the published book at
`https://driftsys.github.io/ridl/`, because a relative link resolves nowhere the
skill is installed.

`rules.md` has `applyTo` frontmatter that limits it to `.typl`, `.ridl`, `.rsdl`
and `ridl.toml` files, so a host with an always-on instructions file loads it
for those files only.

## The gates

Three tests keep the skill consistent with the compiler. Each runs in
`just build`, through the test recipes.

| Test                                                                                               | File                                 | What fails it                                                                                      |
| -------------------------------------------------------------------------------------------------- | ------------------------------------ | -------------------------------------------------------------------------------------------------- |
| `skill_examples_compile`                                                                           | `crates/ridl/tests/book_examples.rs` | a fence that draws a diagnostic its fence does not name, or names one it does not draw             |
| `every_cited_code_is_in_the_catalogue`                                                             | `crates/ridl/tests/skill_gates.rs`   | a `TYPL-`, `RIDL-`, `RSDL-`, `FORM-` or `MANI-` code that `freeze/diagnostics.txt` lacks           |
| `every_link_of_every_skill_file_resolves` and `every_skill_section_links_an_existing_book_heading` | `crates/ridl/tests/skill_gates.rs`   | a link whose page or anchor is missing in `docs/book/`, a relative link, or a section with no link |

The skill's fences are verified as a workspace of their own, so a package name
in the skill need not differ from one in the book. A retired diagnostic code is
absent from `freeze/diagnostics.txt`, so the second test also rejects it.

## Distribution

| Host        | How the skill arrives                                                                                        |
| ----------- | ------------------------------------------------------------------------------------------------------------ |
| Claude Code | `ridl-authoring-<version>.zip`, attached to each editor release, unpacked into a `.claude/skills/` directory |
| Codex       | the same archive, unpacked into `~/.codex/skills/`                                                           |
| Copilot     | the VS Code extension, which contributes `SKILL.md` as `chatSkills` and `rules.md` as `chatInstructions`     |

- `just package-skill <out>` writes the archive. Its top-level directory is
  `ridl-authoring/`, and its name takes the version from `metadata.version` of
  `SKILL.md`. The `editor-v*` release workflow runs it and attaches the archive.
- `just package-vsix` copies `skills/ridl-authoring/` into
  `editors/vscode/skills/`, which git ignores. `just vscode-verify` checks that
  each VSIX holds both files, that every `chatSkills` and `chatInstructions`
  path of the manifest is in the archive, and that the skill archive holds only
  `ridl-authoring/`.
- `metadata.version` is a `[[version_files]]` entry of `.git-std.toml`, so
  `just release` sets it to the workspace version.
- The extension needs VS Code 1.109
  ([ADR-0035](../decisions/ADR-0035-authoring-skill-and-evals.md) decision 7).
  The book's getting-started chapter and the `ridl-mcp` README tell the user
  where to unpack the archive.

## The eval runner

`cargo xtask eval` is the `xtask/src/eval/` module: `task.rs` loads a task,
`host.rs` starts a host, `score.rs` scores the transcript, `judge.rs` marks the
rubric, and `report.rs` writes the summary and the gate.

A run goes through these steps:

1. `task.rs` loads a task directory (`task.toml`, `prompt.md`, `rubric.md`, and
   an optional `start/` workspace). A task starts from its corpus workspace, its
   `start/` directory or an empty directory.
2. `host.rs` copies the starting workspace into a new directory under the system
   temporary directory, outside any git repository, and gives the host a new
   home that holds only the `ridl` MCP configuration, the login the host needs
   and, for a "with" run, the skill. The skill is copied into the host home for
   Codex, and into the workspace for Claude (`.claude/skills/`) and Copilot
   (`.github/skills/`).
3. The host runs the task's prompt with no user, and its transcript is saved.
4. `score.rs` checks the run mechanically and, for a "with" run, checks that the
   transcript shows the skill loaded. A "with" run that shows no load, a host
   that does not start, and a run that exceeds the timeout are recorded with a
   `setup_error`, and `report.rs` leaves them out of both sides.
5. A run that passes the mechanical checks goes to `judge.rs`, which asks a host
   of the other vendor to mark each rubric item met or not met. A judge reply
   that skips an item, repeats one, or is not JSON is an error, not a partial
   score.
6. `report.rs` writes one result file per run and `summary.md`, which holds the
   pass rates, the rubric means and the gate of each host.

Three hosts exist: Claude Code, Codex and the Copilot CLI. A run with no
`--host` option uses Claude Code and Codex; the Copilot CLI runs only when
selected, and it is never a judge. `--claude-handoff` replaces each Claude
command with a request file that an operator runs by hand.

`cargo xtask eval sample --from <dir>` writes the sample of 20 judged runs for
the maintainer to label, and `cargo xtask eval agreement <labels.toml>` writes
the per-item agreement and exits 1 below 80%.

## The task set

`evals/tasks/` holds 12 tasks: five `review`, three `evolve` and four `design`.
`design-0004` and `evolve-0003` start from `examples/cabin`. The other ten are
the tasks of the design-lint calibration
([ADR-0027](../decisions/ADR-0027-design-lints-calibrated-on-a-corpus.md)
decision 10). `crates/ridl/tests/eval_corpus.rs` checks the task format in CI;
no CI step calls a model.

## Related

- [ADR-0027](../decisions/ADR-0027-design-lints-calibrated-on-a-corpus.md) — the
  task format and the calibration that the task set comes from.
- [The MCP workspace tools design record](mcp-workspace-tools.md) — the tools
  that the verify loop calls.
