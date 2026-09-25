# Bug lanes F and Q — driver

Transient working memory for the two bug lanes coordinated on driftsys/ridl#507.
Start a fresh session in the lane's worktree and paste the lane's block below as
the first message, once per stage. Each stage is one session, one branch and one
pull request, and the session ends when that pull request has merged. Lane F and
lane Q may run at the same time as two sessions. Archive this file when both
lanes have closed.

Where this document and an ADR disagree, the ADR wins. This document summarizes;
it does not decide.

## Where the lanes come from

On 2026-09-25 the open bug backlog was triaged. An agent then tried to disprove
each defect claim against `main` at 36241a3, and every claim listed below
survived: each was reproduced end to end or confirmed by reading the code path.
No defect was rated critical after that check. #346 was downgraded to major
because it needs an unusual, generated input and the process it aborts is local.

Lane F takes the major defects. Lane Q takes the defects that are small,
localized and safe to fix in under an hour each.

Left out of both lanes, on purpose:

- #196 (an unreadable path is reported with the wrong cause) — minor, medium
  effort; stays in the backlog.
- #276 — roadmap story E9.12, sequenced in `docs/ROADMAP.md`.
- #308 and #309 — the ridl language reference does not define the envelope
  sequence number's caller scope or an invalid event payload. Both need a
  decision from the maintainer, not a mechanical edit.

## Model routing

- Driver: Opus.
- F1 (#339) and F4 (#346): Fable. The first changes a publishing gate, the
  second the parser's recursion.
- F2, F3: Opus. F5: Sonnet.
- Every Q stage: Sonnet. Move an implementer up one tier after one failed fix
  loop.

## Shared files

- F1, Q1 and Q2 edit `crates/ridl/src/main.rs`.
- F2 and F3 edit `crates/ridl-lsp/src/server.rs`; run F2 before F3.

Rebase on `origin/main` before starting any of these stages, and again before
merging.

## Lane F — the major defects

Each line gives where the refutation found the defect at 36241a3. If a line
number no longer matches, find the item by its function name.

- **F1 — #339.** `published_interaction` (`crates/ridl/src/main.rs:868-890`)
  resolves the package and container with a first-match `find`, and the
  `ReservedNameRedeclared` refusal is an `is_some_and(...)`, so a lookup that
  does not resolve lets the baseline publish. Reproduced: with a duplicate
  published snapshot, `ridl baseline` exits 0, reports no RIDL-408, drops the
  interaction `doorClosed` and deletes the stray snapshot. The other two cases
  of the issue (an interface and a service sharing a name, a snapshot that
  cannot be stat'ed through `is_ir_json` at `:547`) were confirmed by reading
  only; the stage reproduces them first. The fix resolves with the same
  tie-break rule as `ridl_diff::diff_sets`, or carries the resolved container on
  `Change` as the issue proposes, and makes a failed stat an error instead of a
  silent exclusion.
- **F2 — #384.** `crates/ridl-lsp/src/server.rs:231` discards the
  `load_workspace` error with `.ok()`, and `ridl-lsp` sends no `showMessage` or
  `logMessage` anywhere. The minimum fix sends `window/showMessage` with the
  error before entering the main loop. Whether to resolve the root per `didOpen`
  instead of once at `initialize` is the stage's decision; record it on the
  issue.
- **F3 — #345 and #386, one pull request.** `ridlc::check_source`
  (`crates/ridlc/src/lib.rs:146`) calls `front_end` only, and the language
  server's `analyze` (`crates/ridl-lsp/src/server.rs:507`) never calls
  `service_catalog`, while `load_and_check` (`crates/ridlc/src/lib.rs:1094`,
  catalog at `:1157`) does. Reproduced: the `ridl_check` MCP tool returns no
  diagnostics for a file that `ridl check` reports RIDL-140 on. Prefer one path
  that both faces call over a second copy of the pass.
- **F4 — #346.** The `while` loops of `or_expr`, `and_expr`, `add_expr` and
  `mul_expr` (`crates/ridl-syntax/src/parser.rs:1758-1840`) do not increment
  `depth`, `infer` and `walk_refs` in the checker have no guard, and `run_mcp`
  (`crates/ridl/src/main.rs:327-330`) sets no `thread_stack_size`. Reproduced:
  `ridl mcp` aborts at a 1000-term chain; the CLI survives to about 3000. The
  fix reports a diagnostic for a chain over the limit, in the CLI and in
  `ridl mcp`, instead of aborting.
- **F5 — #344.** `editors/vscode/src/extension.ts:89` sets `clientStarted`
  before `await client.start()`, and `restartClient` (`:125-131`) calls
  `client.stop()` without checking the client state, which throws in
  vscode-languageclient 10.1.0 for any state other than running. Validate with
  `just vscode-verify`.

## Lane Q — quick-win debt

- **Q1 — #335.** Add `ridl_diff::Category::MemberReordered` to
  `ORDINAL_CATEGORIES` (`crates/ridl/src/main.rs:611-616`) and give
  `drift_message` an arm that names the member, its containing body, and the
  ordinal it left and took. Reproduced: `ridl diff` reports two breaking
  `member_reordered` changes on a struct field swap, and `ridl check --baseline`
  exits 0 with no output.
- **Q2 — #340.** `refuse_empty_baseline` (`crates/ridl/src/main.rs:1479-1489`)
  suggests `--out <location>` without consulting `is_source_dir`. Reproduced:
  following the suggestion writes the IR file next to `ridl.toml`. Omit the
  suggestion when the location is the source tree, and name the directory
  instead of "there".
- **Q3 — #356, both defects in one pull request.** (a) `parse_default_timing`
  (`crates/ridl-sem/src/timing.rs`, beside the `is_zero` check) accepts a
  negative bound; reproduced with a `[defaults].timing` of `-100ms`. Correct the
  doc comment that says otherwise. (b) `lower_query_return`
  (`crates/ridl-sem/src/check.rs:4367`) does not call `primitive_path_keyword`
  as the parameter and stream-element paths do; reproduced:
  `query name(): string` draws a diagnostic with an empty code. Mint the code
  through the catalogue, not by hand.
- **Q4 — #201.** The `book-check` recipe in the `justfile` does not notice that
  mdBook created a chapter file SUMMARY.md names but the tree does not have.
  Reproduced: `just book-check` exits 0. Fail when the scratch copy's `docs/`
  holds a `.md` file the real `docs/` does not. Add a fixture that makes the
  check fail, in the shape `just doc-path-check` uses.
- **Q5 — #506.** Waits for the maintainer to choose A (emit
  `#[allow(non_camel_case_types)]`) or B (a pinned PascalCase transform with an
  ADR-0016 amendment) on the issue. Only A is a quick win; if B is chosen, the
  stage leaves this lane and gets its own design note.

## The prompt — lane F

```text
You drive lane F of docs/wip/2026-09-25-bug-lanes-driver.md: the major
defects #339, #384, #345 with #386, #346 and #344. Read AGENTS.md, then that
document in full. Its routing and shared-file rules bind this session.

Find the current stage. Read the comments on #507. The stages are F1 to F5,
in that order. Do only one stage in this session, then end it.

Worktree: .claude/worktrees/lane-f-bugs. If it does not exist:
  git fetch origin
  git worktree add .claude/worktrees/lane-f-bugs -b fix/<stage-branch> origin/main
and run ./bootstrap in it. At a later stage, create that stage's branch from
origin/main inside the same worktree. Never enter another session's worktree.
Run `git branch --show-current` before every commit and every push.

For the stage:
1. Read the issue and its comments (`gh issue view <N> --comments`).
2. Write the test that reproduces the defect and watch it fail, before any
   fix (superpowers:test-driven-development).
3. Fix, with the model the routing names for the stage.
4. `fix(<scope>): ...`, one pull request whose body says "Closes #<N>".
   Run /review <PR>, fix, pass 2, `just verify`, merge.
5. Post the stage, the pull request and anything lane Q must know as a
   comment on #507. Never edit #507's body.
```

## The prompt — lane Q

```text
You drive lane Q of docs/wip/2026-09-25-bug-lanes-driver.md: the quick-win
debt #335, #340, #356, #201 and #506. Read AGENTS.md, then that document in
full. Its routing and shared-file rules bind this session.

Find the current stage. Read the comments on #507. The stages are Q1 to Q5
and are independent; take the lowest one not yet posted on #507. Q5 runs only
once the maintainer has chosen option A on #506; if B was chosen, post that
Q5 left the lane and stop. Do only one stage in this session, then end it.

Worktree: .claude/worktrees/lane-q-debt. If it does not exist:
  git fetch origin
  git worktree add .claude/worktrees/lane-q-debt -b fix/<stage-branch> origin/main
and run ./bootstrap in it. At a later stage, create that stage's branch from
origin/main inside the same worktree. Never enter another session's worktree.
Run `git branch --show-current` before every commit and every push.

For the stage:
1. Read the issue and its comments (`gh issue view <N> --comments`).
2. Write the test that reproduces the defect and watch it fail, before any
   fix (superpowers:test-driven-development).
3. Fix, with a Sonnet implementer.
4. `fix(<scope>): ...`, one pull request whose body says "Closes #<N>".
   Run /review <PR> (the quick pass), fix, `just verify`, merge.
5. Post the stage, the pull request and anything lane F must know as a
   comment on #507. Never edit #507's body.
```
