# The ridl-authoring skill and its evals — design

**Status:** draft for review. Lane S of release 0.10
([release design](2026-10-11-release-0.10-design.md) section 5.7, R-6); piece 1c
of the
[devex and agent tracks brief](2026-10-03-devex-and-agent-tracks-brief.md).

**Satisfies:** driftsys/ridl#73 (rules file), #74 (typl skill), #75 (ridl
skill), #87 (rsdl skill profile); ADR-0005 §2 (Layer A) and §5 (evals).

**Related:** [ADR-0005](../decisions/ADR-0005-agent-enablement.md),
[ADR-0025](../decisions/ADR-0025-workspace-aware-mcp-tools.md),
[ADR-0027](../decisions/ADR-0027-design-lints-calibrated-on-a-corpus.md),
[ADR-0029](../decisions/ADR-0029-stability-policy.md),
[the skill outline](skill-ridl-authoring-outline.md), `evals/`.

## 1. Goal

An agent with no training data on RIDL writes a workspace that compiles, uses
the design lints and the MCP tools as evidence, and makes the design decisions
that need judgement (which interaction kind, where an interface boundary goes,
how to name a concept). The skill gives the agent that knowledge; the evals
measure whether it does, on Claude and on GPT.

Success for release 0.10: the skill ships in the release assets and in the VS
Code extension, every RIDL fence in it compiles, and the release eval shows it
is not worse than no skill on either host (D-9).

## 2. Decisions

Rulings R-S-1 to R-S-4 were made by the maintainer before the spec started,
because ADR-0005 conflicted with lane S. D-1 to D-9 were agreed in the
brainstorm on 2026-10-11.

| Id    | Decision                                                                                                                                                                                                                                                                                                                            |
| ----- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| R-S-1 | The skill is written by hand. Gates keep it from drifting from the specifications (D-3). This replaces ADR-0005 §2 "generated from the reference docs" and the consequence "a hand-forked skill is explicitly disallowed", and resolves the open question "Skill generation pipeline". No `ridl doc` command exists to generate it. |
| R-S-2 | The unit an agent writes, and the unit an eval runs on, is a workspace: a `ridl.toml` and its `.typl`, `.ridl` and `.rsdl` files. This replaces the `.rxdl` bullet of ADR-0005 §7; `.rxdl` has no implementation.                                                                                                                   |
| R-S-3 | CI runs only the parts of the evals that give the same result on every run (D-8). The comparison with and without the skill runs on demand, and its results are committed as a record, as `evals/calibration/` is. This replaces "runs in the same CI plane" in ADR-0005 §5.                                                        |
| R-S-4 | ADR-0005 moves from Proposed to Accepted, with R-S-1 to R-S-3 as amendments. Its §8 phase table gets a note that ADR-0012 retired `uxdl`; the table is not rewritten.                                                                                                                                                               |
| D-1   | One skill, `ridl-authoring`, in `skills/ridl-authoring/` (section 3).                                                                                                                                                                                                                                                               |
| D-2   | The skill's verify loop uses the MCP tools: `ridl_check`, then `ridl_explain`, then `ridl_diff` for an edit to a published contract; the lookup tools ground new work in existing symbols.                                                                                                                                          |
| D-3   | Three gates in `just build` keep the skill consistent with the compiler (section 4).                                                                                                                                                                                                                                                |
| D-4   | Distribution: a release asset for Claude Code and Codex, and the VS Code extension for Copilot. The skill's version is the ridl version (section 5).                                                                                                                                                                                |
| D-5   | Two rsdl tasks are added to `evals/tasks/`, built on `examples/cabin` (section 6.1).                                                                                                                                                                                                                                                |
| D-6   | Scoring is mechanical checks first, then the rubric marked by a judge model from the other vendor. The judge is checked once against maintainer labels (section 6.3).                                                                                                                                                               |
| D-7   | A new subcommand, `cargo xtask eval`, runs the hosts' CLIs without a user, with and without the skill (section 6.2).                                                                                                                                                                                                                |
| D-8   | CI checks the task format and never calls a model.                                                                                                                                                                                                                                                                                  |
| D-9   | Release gate: one run per combination while the skill is written, three for the release record. The skill ships when, on each of Claude and GPT, neither score is lower with the skill than without it. Copilot gets a smoke check by hand.                                                                                         |

Durable home: a new ADR (the next free number, ADR-0030 at the time of writing)
holds D-1 to D-9, and ADR-0005 records R-S-1 to R-S-4 as amendments that link to
it, the same pattern ADR-0025 used for piece 1a.

## 3. The skill

```text
skills/ridl-authoring/
  SKILL.md   entry: frontmatter, the 60-second model, the verify loop, routing
  rules.md   the always-on constraints (#73)
  typl.md    types, ranges, units, constants, composites, errors as data (#74)
  ridl.md    the interaction kinds, timing, contracts, evolution (#75)
  rsdl.md    components, instances, wiring, the system (#87)
```

- **`SKILL.md`** has the frontmatter that all three hosts read (`name`,
  `description`; the directory name equals `name`). Its body is short: the
  family in five lines, the dependency lattice, the verify loop of D-2, and one
  line per reference file saying when to read it. A host loads the reference
  files only when the task needs them.
- **`rules.md`** holds 10 to 20 "never" and "always" lines, each citing a
  diagnostic code or a doctrine of the family overview. It is also shipped as an
  always-on instructions file where the host supports one (section 5).
- **Each reference file** holds decision tables, a table of common mistakes
  keyed to diagnostic codes, and worked examples. Each section opens with a link
  to the section of the language reference it summarises.
- **Starting point:** `docs/wip/skill-ridl-authoring-outline.md`. Its `uxdl` and
  `.rxdl` content is dropped (ADR-0012, R-S-2); its codes are re-checked against
  the catalogue (several, such as `RIDL-303`, are placeholders); its skill name
  becomes `ridl-authoring`.
- **Judgement, not syntax, is the skill's main content.** Syntax errors are
  caught by `ridl_check`. The tables that the compiler cannot replace are: state
  versus occurrence (`signal` versus `event`), command versus query, where to
  split an interface (cohesion, with `ridl_metrics` and RIDL-414 as evidence),
  and naming and units (TYPL-222, TYPL-224 as evidence).
- **Timing.** The structure, the gates and the evals are built now. The
  reference files' content is written after lane L's gates are on, so it teaches
  only the frozen language.

## 4. Gates (D-3)

All three run inside `just build`. No new recipe is needed if they are tests in
existing crates; if one becomes a recipe, it is added to `build`, to the
`gate-parity` expected list and to the workflow (AGENTS.md).

1. **Fences compile.** `crates/ridl/tests/book_examples.rs` also reads
   `skills/ridl-authoring/*.md`, with the same rules as the book: a fence is a
   whole file with its own `package`, `,ignore` marks a fragment,
   `,allow=<CODE>` marks a deliberate diagnostic. The skill is a separate
   workspace from the book, so package names need not be unique across the two.
2. **Cited codes exist.** Every `TYPL-`, `RIDL-`, `RSDL-`, `FORM-` and `MANI-`
   code written in `skills/` is listed in `freeze/diagnostics.txt`, and is not
   retired. This also covers the gap recorded on driftsys/ridl#191 for the
   skill's files.
3. **Reference links resolve.** Each reference-file section links a heading of a
   language reference; `just link-check` already fails an unresolved link, and a
   test fails a section with no such link.

## 5. Distribution (D-4)

| Host        | How the skill reaches it                                                                                                                                                                                   |
| ----------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Claude Code | Release asset `ridl-authoring-<version>.zip`; the user unpacks it into `~/.claude/skills/` or a project's `.claude/skills/`.                                                                               |
| Codex       | The same asset, unpacked into `~/.codex/skills/` ([Codex skills](https://developers.openai.com/codex/skills)).                                                                                             |
| Copilot     | The VS Code extension packages the directory and lists `SKILL.md` under `contributes.chatSkills` and `rules.md` under `contributes.chatInstructions`, next to its existing `mcpServerDefinitionProviders`. |

- `chatSkills` is not in the published contribution-points reference. It is
  registered by VS Code 1.141's workbench code, with entries of the form
  `{ "path": "./…/SKILL.md" }`. The plan finds the first VS Code version that
  registers it; if that is newer than 1.101, `engines.vscode` and the pinned
  `@types/vscode` move to it, which ADR-0005's resolved open question on host
  coverage records as an accepted kind of cost.
- The release workflow builds the zip; the book's install page and
  `crates/ridl-mcp/README.md` say where to unpack it.
- The skill's version is the ridl version, written into `SKILL.md`'s frontmatter
  by `just release`.

## 6. Evals

### 6.1 The task set (D-5)

The ten tasks under `evals/tasks/` are typl and ridl only. Two tasks are added
on `examples/cabin`, which already has an rsdl system and is public:

- `design-0004`: add a component and wire it into the cabin system.
- `evolve-0003`: change the system compatibly (`expect.diff = "compatible"`).

They use the ADR-0027 task format (`task.toml`, `prompt.md`, `rubric.md`) and go
through its independent review and the maintainer's approval before any run.

### 6.2 The runner (D-7)

`cargo xtask eval [--task <id>]... [--host claude|codex]... [--runs <n>]
[--skill with|without|both]`:

1. Copies the task's workspace to a temporary directory.
2. Gives each run an isolated configuration (its own `HOME` or the host's
   configuration-directory variable), so no user skill, rule or memory reaches
   the agent. The only configuration is the `ridl` MCP server and, for a "with"
   run, the skill at the host's project skill path.
3. Runs `claude -p` or `codex exec` with the task's `prompt.md`, and saves the
   transcript in the host's JSON output form.
4. Scores the run (6.3) and writes
   `evals/results/<date>/<task>/<host>-<with|without>-<n>.toml` and a
   `summary.md` table per host.

The plan pins the exact CLI flags of each host and records the host versions and
model ids in each result file.

### 6.3 Scoring (D-6)

1. **Mechanical checks**, all required:
   - `ridl check` on the final workspace reports no Error;
   - the shipped design lints reported match `expect.lints`;
   - for an `evolve` task, `ridl diff` against the starting workspace gives
     `expect.diff`;
   - the transcript contains at least one `ridl_check` call (the verify loop).
2. **Rubric**, only for a run that passes step 1 (a failed run scores zero): a
   judge model marks each numbered rubric item met or not met. **must** and
   **must not** items count 2, **should** items count 1; the rubric score is the
   weighted fraction met.
3. **Judge.** Claude runs are judged by the GPT model and GPT runs by the Claude
   model. The judge sees the prompt, the rubric, the final workspace and the
   agent's final message, not the transcript or whether the skill was loaded.
4. **Judge check, once.** The maintainer labels the rubric items of a sample of
   20 runs drawn across tasks and hosts. Agreement per item is recorded in
   `evals/results/judge-check.md`. If agreement is below 80%, the judge prompt
   is revised and the check repeated before a release record is made.

### 6.4 What CI runs (D-8)

A test checks every task directory: the three files exist, `task.toml` parses
with a known `kind`, and the rubric items are numbered and each carries one of
the three markers. CI never calls a model and needs no API key.

### 6.5 The release gate (D-9)

- While the skill is written: `--runs 1`, filtered by task or host.
- For the release record: `--runs 3` over 12 tasks × 2 hosts × with and without,
  that is 144 agent runs and up to 144 judge calls, committed under
  `evals/results/<date>/`.
- The skill ships in 0.10.0 when, for each of Claude and GPT, the mechanical
  pass rate with the skill is at least the rate without it, and the mean rubric
  score with the skill is at least the score without it. The measured gains are
  reported in the results summary and in the book. If either host is worse with
  the skill, the release waits for a fix to the skill.
- Copilot: by hand, confirm that the extension loads the skill and that
  `design-0001` completes with the verify loop. Recorded in the results summary.

## 7. Alternatives considered

- **Generate the skill from the references** (ADR-0005 as written). Rejected
  (R-S-1): no generator exists, and a curated skill with compiled fences and
  checked codes gives the same protection against drift.
- **Scoring by mechanical checks only.** Rejected: five of the ten tasks are
  review tasks, whose output is prose that the compiler cannot score.
- **Scoring by two blind labellers with maintainer adjudication**, as in 1b.
  Rejected for repeated use: a labelling round per skill change is too slow to
  catch regressions. It is kept once, as the judge check.
- **`ridl skill install --host …`.** Deferred: it adds a CLI subcommand, and the
  CLI surface freezes in 0.10. It can be added later by addition.
- **Runs by hand, scored by the xtask.** Rejected: 144 manual sessions per
  release record would not be repeated.
- **A fixed improvement threshold** (for example 20 percentage points).
  Rejected: no evidence supports a number before the first run.
- **Ship the rsdl part without measuring it.** Rejected (D-5).

## 8. Out of scope

- A `ridl-architect` subagent (ADR-0005 §6).
- Copilot in the measured comparison.
- rmdl content (no implementation).
- The long-form error index for `ridl_explain` (story E4.2).

## 9. Open points for the plan

- The first VS Code version that registers `chatSkills` (section 5).
- The project-level skill path and the isolation variable of each host's CLI
  (section 6.2).
- The pinned judge models and the judge prompt (section 6.3).
