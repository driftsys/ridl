# Formatter implementation decisions

This dated log records execution of [the plan](fmt-ridl-rsdl-plan.md).
[The design note](fmt-ridl-rsdl-layout.md) remains normative. Entries are
append-only. Implementation status and verification are in
[the review report](fmt-ridl-rsdl-review.md).

## D-H1 — existing approved rules, Tasks 5–13

- Date and timezone: 2026-10-01, Europe/Paris.
- Status: approved existing rule.
- Question or observed case: which decisions constrain the remaining tasks?
- Chosen action: preserve D-1 through D-13. Keep MIT and the ec4rs licence
  distribution; canonical two-space indentation; default width 100 Unicode
  scalar values, including indentation; trailing comments excluded from width;
  unbreakable text allowed to exceed it; `off` disables breaking. Keep `format`
  pure and configuration behind the default `editorconfig` feature.
- Reason and alternatives considered: these are maintainer decisions, rather
  than choices to reconsider during implementation. Configurable indentation
  remains debt [#631](https://github.com/driftsys/ridl/issues/631).
- Authority: design sections 6 and 8 and the maintainer handoff.
- Affected files and behavior: formatter, CLI, LSP, packaging and documentation.
  The book's five-extension example configures editors; implemented compiler
  profiles remain typl, ridl and rsdl. Keep `docs/wip/` in place. New PRs and
  [PR #630](https://github.com/driftsys/ridl/pull/630) require maintainer
  merging.
- Verification: initial remote inspection confirms Tasks 1–4 merged; PR #630 is
  open at `769b541d49ac93b0ecb6dd04a08d59518b04e64e`, with successful CI.
- Commit and PR: no new implementation commit yet.
- Maintainer action: review the eventual task PRs; no merge authorization given.

## D-H2 — Task 5 brace-glob test sequencing

- Date and timezone: 2026-10-01, Europe/Paris.
- Status: approved existing rule.
- Question or observed case: the proposed `.rsdl` tuple fixture
  `package p\nstruct S { pair: (a: integer, b: boolean) }` draws RSDL-604. Task
  9 introduces breakable rsdl declarations.
- Chosen action: use a `.typl` tuple in Task 5's CLI brace-glob width test;
  retain Task 4's `.rsdl` reader test; add a valid `.rsdl` CLI width test in
  Task 9, using attributes or value lists at different widths and a second
  `--check` run.
- Reason and alternatives considered: this tests the Task 5 caller plumbing with
  a currently breakable construct without changing the parser or moving Task 9
  ahead of Task 5.
- Authority: explicit maintainer answer in this session, “Approve the proposed
  test sequencing”, to the three-part proposal in the handoff.
- Affected files and behavior: plan Task 5 and Task 9 test paragraphs, design
  section 11 override test paragraph, CLI tests. No layout decision changes.
- Verification: stash inspected by object ID; its `.rsdl` CLI fixture is the
  tuple described above. No stash applied or removed at this point.
- Commit and PR: pending Task 5 implementation.
- Maintainer action: none for this proposal; approval received.

## D-H3 — Task 5 branch and test recovery

- Date and timezone: 2026-10-01, Europe/Paris.
- Status: implementation choice.
- Question or observed case: Task 5 must amend PR #630's fixed-width book claim;
  PR #630 is still open. The current checkout also holds the untracked handoff.
- Chosen action: use a sibling worktree on `feat/387-fmt-callers`, initially
  rooted at fetched `origin/main`, `9f0b953caa98294513cd2577f06896a46233119d`,
  then fast-forward to PR #630's `769b541d49ac93b0ecb6dd04a08d59518b04e64e`.
  Task 5's PR base will be `docs/387-fmt-book-width`; merge order is PR #630,
  then Task 5. Recover only the test-file diff first; preserve stash
  `8bd2716627d07aa342d9a98b17a8dba539ed6523` and every other stash.
- Reason and alternatives considered: stacking avoids copying the book change
  into an unrelated base or merging an unapproved PR. A sibling worktree follows
  the existing local placement and preserves the original checkout.
- Authority: handoff branch/dependency and stash instructions; routine judgment.
- Affected files and behavior: branch dependency and verification scope.
- Verification: `git fetch origin main`, `git worktree add`, and `./bootstrap`
  exited 0; bootstrap evidence is
  [Task 5 evidence](fmt-ridl-rsdl-evidence/task-05.md), with the full log
  retained locally.
- Commit and PR: pending; baseline is `769b541`.
- Maintainer action: merge PR #630 before Task 5 after reviewing both.

## D-H4 — Task 5 overlay and default controls

- Date and timezone: 2026-10-01, Europe/Paris.
- Status: implementation choice.
- Question or observed case: the saved LSP test used the same text on disk and
  in the buffer, so it could not detect reading the disk source by mistake.
- Chosen action: keep different disk text, format the overlay, apply the result
  to that overlay and require no edit on a second request. CLI cases all run a
  second `--check`; retain the default-width control even though it passes
  before implementation.
- Reason and alternatives considered: this verifies the specified buffer
  contract and fixed point directly. A failure is required for new behavior,
  while the unchanged default remains a control.
- Authority: Task 5 buffer requirement and task-loop verification requirements.
- Affected files and behavior: LSP integration test and CLI width tests.
- Verification: four CLI cases and the LSP width test failed on the unmodified
  callers for default-width rendering; all acceptance and formatter invariant
  tests then passed. See [Task 5 evidence](fmt-ridl-rsdl-evidence/task-05.md).
- Commit and PR: pending; tested working diff on `769b541`.
- Maintainer action: none.

## D-H5 — Task 5 quick documentation findings

- Date and timezone: 2026-10-01, Europe/Paris.
- Status: implementation choice.
- Question or observed case: the independent docs review found unconditional
  100-character tuple thresholds in both book chapters, falsified by caller
  configuration support. The initial committed review summary incorrectly
  reported no docs findings.
- Chosen action: describe the configurable width and its default of 100 in both
  places; correct the review and evidence summaries explicitly.
- Reason and alternatives considered: changing only the EditorConfig paragraph
  left nearby behavior descriptions inconsistent. The review record must report
  the actual finding and disposition.
- Authority: Task 5 book update requirement and independent quick review.
- Affected files and behavior: book CLI reference and getting-started chapter;
  review report and evidence. No executable changes.
- Verification: acceptance passed on the implementation tree; documentation
  gates will be rerun after this correction. The commit-triggered QUICK pass
  reviews `e515b2e` alone, not this advisory correction.
- Commit and PR: implementation `e515b2e`; correction commit pending.
- Maintainer action: none.
