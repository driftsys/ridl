# Task 11 evidence

Branch: `feat/387-fmt-completion`, based on main `0bc48da`. D-H33 records the
recovery of Tasks 9–10 and the final three-task grouping. The existing sibling
worktree remains bootstrapped; all original branches, stashes and WIP records
are preserved.

The root `.editorconfig` gains exactly `[*.{typl,ridl,rsdl}]`, `indent_size = 2`
and `max_line_length = 100`, matching design section 6.6. The existing general
indentation remains four spaces. The wider five-extension book example remains
an editor example. Formatter indentation stays two spaces; only the width is
read. No parser, profile or formatter behavior changes.

No failing test is added: Task 11 explicitly requires configuration checks, with
an unchanged default width of 100. Acceptance commands are `just check` and
`cargo test --workspace --locked`; logs remain local and ignored.

Acceptance passed: `just check`, exit 0, and `cargo test --workspace --locked`,
exit 0 (`task-11-workspace-test.log`). Existing formatter invariants and all
workspace unit, integration and documentation tests remain green. This task adds
no new executable behavior and requires no QUICK behavior review.
