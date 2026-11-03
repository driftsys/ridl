# The debug socket — handoff to the Codex session

**For:** a dedicated Codex session (model `gpt-6.1-sol`, reasoning effort
`medium`), started in this worktree. **From:** the Claude session that wrote the
design. **Status:** use only after the owner has approved
`2026-11-03-debug-socket-design.md`.

You write the implementation plan for the design, then build it task by task. A
Claude reviewer gates every task. You do not merge, push or open a PR.

## 1. Read first

1. `AGENTS.md` — the project rules. They bind you. In particular: plain, literal
   English; no story ids, epic names or plan names in code, rustdoc or shipped
   docs; Conventional Commits with the scopes in `.git-std.toml`; the justfile
   is the only definition of a gate.
2. `docs/wip/2026-11-03-debug-socket-design.md` — the design. Its **(call)**
   items and its open questions are decided by the owner before you start; the
   owner's answers are written into the note. Do not reopen a decision. If the
   code makes a decision impossible as written, stop and report it (see §5).
3. The records the design cites: `docs/design/deployment-descriptor.md`,
   `docs/design/ridl-transport-ws.md`, `docs/design/ridl-rt.md`, ADR-0021,
   `docs/specification/frame-specification.md` §3 and §11.1, ADR-0029 and
   `freeze/README.md` (the `runtime-crates` freeze surface).

## 2. Phase 1 — write the plan, then stop

Write `docs/wip/2026-11-03-debug-socket-plan.md`. It covers PR A, PR B and PR C
of the design's §7, each as its own section, so that each PR can start from a
clean base.

For each task:

- **Files:** the files it creates or changes.
- **Behaviour:** what is true after the task, in the design's terms.
- **Tests first:** each test, named, with the assertion that fails before the
  change. A red that is only a compile error is accepted only for a new API.
- **Review focus:** what the reviewer must check, and for each new test the
  one-line code mutation it must catch. The reviewer runs those mutations.
- **Checks:** the focused commands that pass at the end of the task
  (`cargo test -p <crate>`,
  `cargo clippy -p <crate> --all-targets -- -D warnings`,
  `cargo fmt --all --check`, and for the TypeScript package `deno fmt --check`,
  `deno lint`, `deno test` in `packages/ridl-transport-ws`). Do not run
  `just build`; CI runs the full gate.
- **Size:** about one reviewable diff. Split a task whose diff would exceed
  roughly 600 changed lines of non-test code.

Also state in the plan:

- the names the design leaves to you (traits, functions, features), once, in a
  "Names" section;
- each place where the design was not precise enough and the choice you made, as
  a line starting with `Ruling:`;
- the records to garden at the end of each PR (design §7).

Then stop. The owner and Claude review the plan before any code.

## 3. Phase 2 — build, one task at a time

For each task:

1. Record the task's base: `git rev-parse HEAD`, in the task report.
2. Write the tests, run them, and keep the failing output.
3. Make the change. Minimum code for the task; no refactor outside it; match the
   surrounding style and comment density.
4. Run the task's checks.
5. Commit, one commit per logical change, Conventional Commits, no `!` and no
   `BREAKING CHANGE` (the design is additive; a change that cannot be additive
   is a stop, see §5). No trailer line. If your environment cannot write git
   state, leave the changes unstaged and propose the messages in the report.
6. Write `.superpowers/codex/debug-socket-task-<N>-report.md` (git ignored):
   - the base commit and the commits made;
   - per behaviour: the change (file:line), the test that pins it, and its
     failing output before the change;
   - per new test: the mutation it catches;
   - the commands run and their results, and any test you could not run (a test
     that binds a TCP port may not run in a sandbox: say so, and the reviewer
     runs it);
   - each `Ruling:` taken in the task.
7. **Stop.** Do not start task N+1. A Claude reviewer reads the diff from the
   base you recorded and returns two verdicts: the task meets its brief, and the
   code is sound. You receive its confirmed findings as a fix prompt; fix them
   in the same task, report again, and stop again.

You may run your own review of a task before you stop and list what you found in
the report. It is advice to the reviewer, not the gate.

## 4. After the last task of a PR

Stop and say so in the last report. The Claude session runs the branch review
(two passes, with you as an advisory seat), the fix wave, the gardening of
`docs/wip/`, the PR and CI. The owner gives the merge word.

## 5. Stop and report, do not decide, when

- a design decision cannot be built as written;
- a change would break a published API, the wire, or a frozen surface
  (`cargo semver-checks` on the `runtime-crates` surface, `freeze/wire.txt`,
  `freeze/generated/`);
- a test outside the task's crates fails and the cause is not obviously yours;
- the task needs a new dependency.

Write the reason in the report under "Stopped", with the evidence, and stop.

## 6. Sandbox notes from earlier Codex sessions in this repository

- Set `RUSTC_WRAPPER=` when sccache is blocked; use `--offline` when there is no
  network.
- Tests that bind a local TCP port (`ridl-transport-ws` end-to-end tests) may
  fail with `Operation not permitted` in a sandbox. Do not change them to pass;
  list them in the report.
- Do not run `git stash`: stashes are shared across this repository's worktrees.
