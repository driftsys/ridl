# Task 5 evidence

Date: 2026-10-01, Europe/Paris.

Base: `769b541d49ac93b0ecb6dd04a08d59518b04e64e` (PR #630). Implementation
commit: `e515b2e`. Initial results below are from the working diff on this base.
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
execution. The tests seat reported no findings. The docs seat reported two stale
fixed-width claims in the book, corrected in the working diff after `e515b2e`.
The first committed evidence summary incorrectly recorded the docs review as
clean; this paragraph corrects that record. The commit-triggered QUICK pass
completed over `e515b2e` alone. All three startup headers confirm
`gpt-5.6-terra`: tests and docs at effort `high`, native built-in code review at
effort `medium`. Tests reported no findings; docs and built-in review reported
the same two fixed-width book claims. Both claims are corrected. Native Codex
review was invoked directly from the unrestricted parent because Terra is absent
from the sub-agent allowlist; no nested native client ran inside a restricted
worker sandbox. Full pass 1 completed; results and correction evidence are
below.

Documentation correction checks: `just book-check`, `just link-check`,
`just doc-path-check`, and `just check` each exited 0. The initial combined
invocation `just book-check link-check doc-path-check check` exited 1 because
`book-check` interpreted `link-check` as its root argument; it was replaced with
the four separate recipe invocations above. This was an invocation error, not a
repository behavior or environment failure.

## Committed-head gate and PR

Tested head: `f887c780c0d4b58fdcddd3a94f82117b6be8616e`. `just verify` exited 0
before pushing and opening [PR #632](https://github.com/driftsys/ridl/pull/632),
based on `docs/387-fmt-book-width`. Its full build invokes `toolchain-check`,
`gate-parity`, `install-check`, `fmt-check`, `book-check`, `link-check`,
`doc-path-check`, `compile`, `test`, `lint`, `wasm-check`, `compat-check`,
`demo`, and `check`; commit lint also passed. Thus all eight required push
recipes passed on this exact head. The cabin demo produced all six matched round
trips: signal 21, event 5, command 42, query 7, blocking command 43, blocking
query 9.

`git push -u origin feat/387-fmt-callers` exited 0 with hooks enabled;
`just pre-push` passed.
`gh pr create --base docs/387-fmt-book-width --head
feat/387-fmt-callers --title <title> --body-file <body-file>`
exited 0. No PR has been merged. All CI checks passed on this head: book, ci,
commit-lint, markdown, rust and wasm; Pages was skipped.

Local logs normalize user-specific tool, checkout and temporary-directory paths;
no credentials or unrelated native-client startup output is included.

## Full pass 1 corrections

Full pass 1 reviewed `769b541..f887c780` in fresh contexts. Compliance, tests
and docs used actual `gpt-6.1-sol/high`; the compliance shadow and one refuter
per finding used actual `gpt-5.6-terra/high`. The fresh native bugs wrapper used
Sol/high and its built-in worker used Sol/xhigh. The native fallback is not a
literal `/code-review max` invocation; that slash command is unavailable. All
seats and refuters confirmed the exact 13 changed paths. Compliance, docs, bugs
and shadow reported no actionable findings. Tests reported four gaps,
independently confirmed at confidence 97, 94, 96 and 90:

| Finding                                                   | Correction                                                                  |
| --------------------------------------------------------- | --------------------------------------------------------------------------- |
| F1, constant LSP width 60 survives                        | Format the same 80-column tuple under 60 and 100 in one server session.     |
| F2, CLI width 60 resolved as 61 survives                  | Check configured widths 60 and 61 at their exact limit and one column over. |
| F3, explicit-file arguments use default options unnoticed | Run the new boundary cases through direct file arguments.                   |
| F4, LSP `off` has no caller coverage                      | Format a 200-column tuple under `off`.                                      |

Both new tests check their second formatting pass. The LSP test uses overlays
that differ from disk and verifies disk is unchanged. No production behavior
changed. An initial LSP test setup run exited 101 because its temporary helper
does not create subdirectories; explicitly creating each configuration directory
repaired this fixture error before mutation testing.

Correction checks on the working diff:

- `cargo test -p ridl-cli --test facade --locked`: exit 0, 18 passed.
- `cargo test -p ridl-lsp --locked`: exit 0, 17 library and 65 server tests.
- Four isolated mutations were applied and restored sequentially. Each targeted
  test exited 101 with an assertion failure: LSP fixed width 60; LSP `off`
  replaced with 60; CLI 60 replaced with 61; CLI explicit-file default options.
  Commands were
  `cargo test -p ridl-lsp --test server
  formatting_resolves_distinct_widths_and_off_for_document_paths --locked`
  for the two LSP mutations and
  `cargo test -p ridl-cli --test facade
  fmt_editorconfig_exact_width_applies_to_explicit_file_arguments --locked`
  for the two CLI mutations. Local `task-05-mutation-*.log` files retain the
  actual failures. Production files are restored and the acceptance targets
  passed.

Full pass 2 reviewed `f887c780..d31f4b9062bf77139c8d4053d9f91156ba12d588`.
Actual models: tests/docs Sol/high; compliance and each independent refuter
Terra/high. Every seat/refuter verified the five changed paths. Compliance
reported no findings. The docs claim that logs must be committed was refuted at
confidence 5: the handoff permits local logs and the reviewer inspected them.
Two independently confirmed LSP test gaps (confidence 92 each) are deferred to
[debt #633](https://github.com/driftsys/ridl/issues/633), milestone E1 — typl +
Tooling Spine: exact finite-width boundaries and formatting an earlier opened
document after opening documents with other widths. The active workflow's
second-pass rule requires noncritical findings to be filed as debt. The two-pass
cap is reached; no third full review will run.

`just verify` exited 0 on `d31f4b9062bf77139c8d4053d9f91156ba12d588` before
`git push`, which exited 0 with `just pre-push` and hooks enabled. This includes
all eight required push gates and the full build. All CI checks passed on this
head: book, ci, commit-lint, markdown, rust and wasm; Pages was skipped. Local
logs: `task-05-correction-verify.log` and `task-05-correction-push.log`.

The commit-triggered QUICK review over `f887c780..d31f4b9` used actual
Terra/high for tests, docs and the fresh general-purpose bugs wrapper; the
native built-in bugs worker used Terra/medium. Docs and bugs found no issues;
tests found no coverage defect and noted pending checkpoint metadata, which this
update resolves. Startup headers are the model/effort evidence; reviewers'
generic self-reported model names were not used. No quick review of these
metadata fixes runs. This metadata checkpoint follows the tested correction
head; its required push-gate logs remain locally available.
