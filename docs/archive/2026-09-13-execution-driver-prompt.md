# Driver prompt — executing the 2026-09-13 plans

Transient working memory: paste the block below into a fresh session as the
first message. Archive this file with the specs and plans when the branch is
gardened (`sdd-working-memory-lifecycle`).

## Model routing

- Driver (this session): Opus. It reviews each task's result and decides what
  "done" means; a cheaper driver lowers decision quality, not cost.
- Implementer subagents: Sonnet by default — every task is written as a bounded,
  test-first unit with the code in the plan.
- Escalate the implementer to Opus for plan A Task 4 (the `rmcp` macro API must
  be fitted to the version `cargo add` picks) and plan A Task 5 (the `rmcp`
  client integration test and the Tokio runtime). Use Fable only if a task fails
  twice for a reason the plan did not anticipate.
- Reviewer subagents (the second stage of subagent-driven development): Opus.

## The prompt

```text
Execute two implementation plans in this repository, in order, on one
branch. Read AGENTS.md first.

Plans (each names its spec; read the spec before the plan):
1. docs/wip/2026-09-13-ridl-mcp-v0-plan.md
2. docs/wip/2026-09-13-vscode-extension-distribution-plan.md

Setup:
- Use the superpowers:using-git-worktrees skill: worktree on a new branch
  `feat/ridl-mcp-v0` from `main`. Run `./bootstrap` in the worktree.
- First commit on the branch: the five untracked files
  docs/wip/2026-09-13-*.md (two specs, two plans, this driver prompt), as
  `docs(docs): add the ridl-mcp v0 and distribution specs and plans`.
- Then use superpowers:subagent-driven-development: one fresh implementer
  subagent per task, Sonnet by default, Opus for plan 1 Tasks 4 and 5;
  the plan's own Step text is the subagent's brief, plus the spec path and
  the Global Constraints section. Two-stage review after each task.

Rules that bind every task:
- `just build` green after every task; `cargo fmt --all` before every Rust
  commit; `just fmt` before every Markdown/JSON/YAML commit; Conventional
  Commits with the scopes each task names.
- Never push a tag, publish to a registry, add a secret, or push to main.
  Those are maintainer acts (ADR-0007 decision 14); plan 2 Task 9 lists
  them for the PR description.
- Do not delete crates/ridl-lsp/src/main.rs during plan 1; plan 2 Task 5
  does it.
- Plain, literal prose everywhere (no idioms).

Stop points:
- After plan 1 Task 7: run `just verify`, report, and continue to plan 2
  without opening a PR (one PR covers both plans).
- After plan 2 Task 9: run the sdd-gardening skill over the five
  docs/wip/2026-09-13-* files, run `just verify`, then open one PR against
  main with the maintainer checklist from plan 2 Task 9 in its
  description. Run /review on the branch before opening the PR. Do not
  merge.

If a step's command or API does not match the repository or the crate
version cargo add picked, adjust the implementation to the plan's stated
interface (the **Interfaces** block of that task) and say so in the task
report; do not change the interface other tasks consume.
```
