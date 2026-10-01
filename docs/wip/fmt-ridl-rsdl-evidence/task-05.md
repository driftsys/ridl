# Task 5 evidence

Date: 2026-10-01, Europe/Paris.

Base: `769b541d49ac93b0ecb6dd04a08d59518b04e64e` (PR #630). Implementation
commit: pending. Initial results below are from the working diff on this base.
Full logs are retained locally in this directory and excluded from commits;
summaries contain the commands, results and relevant evidence.

## Setup and baseline

- `git fetch origin main`: exit 0; fetched main is
  `9f0b953caa98294513cd2577f06896a46233119d`.
- `git worktree add <sibling-worktree> -b feat/387-fmt-callers origin/main`:
  exit 0. `./bootstrap`: exit 0. `git merge --ff-only docs/387-fmt-book-width`:
  exit 0; Task 5 is stacked on PR #630 so its book claim can be amended.
- `cargo test -p ridl-cli --test facade --locked`: exit 0, 12 passed.
- `cargo test -p ridl-lsp --locked`: exit 0, including 63 server tests.

## RED

Recovered only the test paths from stash
`8bd2716627d07aa342d9a98b17a8dba539ed6523` using
`git diff <stash>^1 <stash>
-- crates/ridl/tests/facade.rs crates/ridl-lsp/tests/server.rs`,
then `git apply`. Caller changes remained absent. The approved brace-glob
fixture uses `.typl`.

- `cargo test -p ridl-cli --test facade fmt_editorconfig --locked`: exit 101,
  four failures and one passing default control. Override, brace glob and outer
  nested-root cases produced an inline 80-column tuple instead of the expected
  broken tuple. The `off` case broke the 200-column tuple instead of keeping it
  inline. These are configuration regressions; there was no parse failure.
- `cargo test -p ridl-lsp --test server
  formatting_reads_editorconfig_width_and_keeps_two_space_indentation --locked`:
  exit 101, one failure. The edit used inline default-width output instead of
  breaking the 80-column tuple at 60. Its source is an overlay distinct from the
  file on disk.

Expected broken line sequence for the 80-column fixture:

```text
pair: (
  aaaaaaaaaaaaaaaaaaaaaaaaa: integer,
  bbbbbbbbbbbbbbbbbbbbbbbbb: boolean
)
```

Actual old rendering:

```text
pair: (aaaaaaaaaaaaaaaaaaaaaaaaa: integer, bbbbbbbbbbbbbbbbbbbbbbbbb: boolean)
```

## GREEN and acceptance

- `cargo test -p ridl-cli --test facade --locked`: exit 0, 17 passed.
- `cargo test -p ridl-lsp --locked`: exit 0, including 64 server tests.
- `cargo test -p ridl-fmt --locked`: exit 0, including the three-profile
  structure, token/comment and idempotence harness at widths 100, 60 and 40.
- Every new CLI configuration test formats and then runs `fmt --check`. The LSP
  test applies the returned text to its overlay and requests formatting again
  under four-space client options; the result is an empty edit list. The disk
  file remains unchanged. EditorConfig indentation keys and client indentation
  options do not affect canonical two-space indentation.

## Review and gates

Quick review completed in two independent native Codex contexts. Both startup
headers confirm actual model `gpt-5.6-terra`, effort `high`, read-only
execution. The tests and docs seats reported no findings over their specified
Task 5 files. Full review and required push/PR gates pending.
