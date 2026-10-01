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

## D-H6 — Task 5 full-review coverage corrections

- Date and timezone: 2026-10-01, Europe/Paris.
- Status: implementation choice.
- Question or observed case: full pass 1 retained four caller-test gaps. A
  constant LSP width, ignored LSP `off`, a shifted CLI limit and direct-file
  defaults all survived the initial suite in isolated mutations.
- Chosen action: add LSP width 60/100/off cases in one session and CLI
  direct-file cases at configured 60/61 boundaries, including second formatting
  passes.
- Reason and alternatives considered: caller-boundary tests pin the externally
  promised behavior; testing only the reader would not detect caller mutations.
  Existing tests remain to retain the approved fixtures and indentation
  coverage.
- Authority: design sections 6 and 11; handoff task-loop review corrections;
  routine test placement judgment. No rendering or production behavior changes.
- Affected files and behavior: CLI facade and LSP server integration tests.
- Verification: both acceptance targets passed; four isolated mutations failed
  the new tests, then production files were restored. See
  [Task 5 evidence](fmt-ridl-rsdl-evidence/task-05.md). Tested base is
  `f887c780`.
- Commit and PR: correction commit pending; PR #632, base PR #630.
- Maintainer action: review the stacked PR; no approval or merge requested here.

## D-H7 — Task 5 review cap and final checkpoint

- Date and timezone: 2026-10-01, Europe/Paris.
- Status: approved existing rule and implementation choice.
- Question or observed case: full pass 2 retained two further LSP coverage gaps;
  no production defect was identified. The checkpoint still named the correction
  commit as pending when written before that commit.
- Chosen action: record `d31f4b9` as the completed correction, its passing gates
  and CI, and defer both remaining coverage findings to debt #633, milestone E1
  — typl + Tooling Spine. Preserve D-H6 as the historical decision.
- Reason and alternatives considered: the active review workflow caps full
  review at two passes and requires noncritical second-pass findings to be
  recorded as debt. A third implementation/review cycle would violate the cap.
  The docs finding about committed logs was independently refuted: the handoff
  permits local logs, which substantiate the committed summaries.
- Authority: handoff review instructions and the active review command's “Pass
  2, and the stop” rule; routine checkpoint placement.
- Affected files and behavior: decision, review and evidence records only.
- Verification: `just verify`, enabled pre-push hook and all CI checks passed on
  `d31f4b9062bf77139c8d4053d9f91156ba12d588`; four mutations fail the new tests.
  See [Task 5 evidence](fmt-ridl-rsdl-evidence/task-05.md).
- Commit and PR: corrections `d31f4b9`, PR #632 based on PR #630. This metadata
  checkpoint follows the tested correction; its required gates run before push.
- Maintainer action: review PR #632 and debt #633. No merge is authorized.

## D-CI1 — PR #630 provider-delay synchronization

- Date and timezone: 2026-10-02, Europe/Paris.
- Status: implementation choice; explicitly requested CI correction.
- Question or observed case: PR #630's Rust CI failed the minimum-wait assertion
  in `a_blocking_client_with_no_timeout_set_waits_for_the_provider` after PR
  #632 merged into its branch. The provider's sleep could begin before the
  caller's timer, so the elapsed time did not cover the entire delay.
- Chosen action: make the provider wait for the client's outcome-interest
  registration before sleeping, using the existing test double's shared waker
  slot. Apply the shared helper to its three existing callers.
- Reason and alternatives considered: simply timing thread launch would count
  setup time as call wait; lowering the threshold or retrying CI would preserve
  the scheduling defect. Synchronization preserves the success and minimum-wait
  assertions and changes only the test setup.
- Authority: maintainer request "please fix 630"; interaction-face design F-11
  and ADR-0023 decision 6 keep unbounded blocking-call behavior unchanged.
- Affected files and behavior: interaction-face test helper and its three test
  callers; no production code or formatter decision changes. The separate Tasks
  6–7 branch and its D-H13 approval checkpoint remain untouched.
- Verification: a temporary 20 ms caller delay reproduced exit 101 before the
  correction and passed after it, exit 0. All 77 interaction-face tests pass.
  See [the CI correction evidence](fmt-ridl-rsdl-evidence/pr-630-ci.md).
- Commit and PR: correction follows `dce6aa60` on PR #630's existing branch.
- Maintainer action: review the correction; no merge is authorized.

## D-CI2 — PR #630 QUICK findings and regression controls

- Date and timezone: 2026-10-02, Europe/Paris.
- Status: implementation choice following advisory review.
- Question or observed case: QUICK tests requested committed scheduling
  coverage; native bugs found that a provider-thread registration assertion
  could leave the parent blocked. Docs reported no falsified statement.
- Chosen action: add a provider-start rendezvous and an explicit caller delay
  parameter to the test helper, with a new delayed-caller regression. Finish
  serving and unpark the caller before reporting missing registration in the
  parent. Keep the three original callers' delay zero and timeouts unchanged.
- Reason and alternatives considered: removing the timing assertion would weaken
  the original requirement. Setting a client timeout would alter the
  unbounded-call cases being tested. A committed delayed-caller case detects
  restoration of the old schedule, while settling and unparking preserves the
  intended result and makes the registration failure observable.
- Authority: user-requested CI correction and the QUICK review workflow; no
  design or production changes. No additional QUICK cycle over the fixes.
- Affected files and behavior: interaction-face tests only, plus evidence.
- Verification: original-schedule mutation fails the new test, exit 101.
  Missing-registration mutation hangs the old guard, but the corrected guard
  reports exit 101. All mutations are restored; all 78 target tests pass. First
  correction head `d011d56` passed `just verify`; the final head is verified
  again before push. See [CI evidence](fmt-ridl-rsdl-evidence/pr-630-ci.md).
- Commit and PR: review corrections follow `d011d56` on PR #630's branch.
- Maintainer action: inspect the focused correction; no merge is authorized.
## D-H8 — Tasks 6–8 branch and PR grouping

- Date and timezone: 2026-10-01, Europe/Paris.
- Status: implementation choice.
- Question or observed case: interface members, attributes, and inline service
  bodies use the same member and width layout; each later task depends on the
  preceding one.
- Chosen action: implement Tasks 6, 7 and 8 sequentially, with separate task
  commits, evidence and quick reviews, in one stacked PR on `feat/387-fmt-ridl`,
  based on Task 5's `feat/387-fmt-callers` at
  `8b8587106e164a8b2b7ffb5a42fbe2463f0da42f`. Run the full PR reviews after the
  group is implemented. Fetch confirmed main remains `9f0b953c`.
- Reason and alternatives considered: the small consecutive group provides the
  complete ridl member and service layout in one reviewable change while keeping
  each task's tests and implementation distinct. Separate PRs would expose the
  intentionally temporary attribute-member fallback as a PR boundary.
- Authority: handoff task-loop step 9 permits a justified small consecutive
  group; design sections 3.1–3.3 and the existing task order remain unchanged.
- Affected files and behavior: `ridl-fmt` layout, tests, goldens and records.
- Verification: the reused worktree completed bootstrap before Task 5; Task 5's
  final checkpoint passed all eight push gates, pre-push and all CI.
- Commit and PR: Task 6 commit and grouped PR pending; base PR #632.
- Maintainer action: review the later stacked PR after PR #632; no merge is
  authorized.

## D-H9 — Task 6 shared interaction and stream rendering

- Date and timezone: 2026-10-01, Europe/Paris.
- Status: implementation choice.
- Question or observed case: the old formatter drops a direct struct stream and
  an array stream, and emits interface declarations without canonical member or
  width layout.
- Chosen action: route interfaces through `format_block_def`, exclude them from
  single-line comment routing, reuse `Layout::Tuple` for parameters and tuple
  returns, and add stream recognition to the shared field-type renderer. Keep
  attribute members verbatim until Task 7. Share value/callable slot assembly,
  preserving the parser's lenient slots and spaced fallible returns.
- Reason and alternatives considered: shared type recognition preserves streams
  in direct, optional, tuple, array and map positions as well as interaction
  payloads, parameters and returns. A payload-only patch would leave issue #625.
  A separate breaking engine would duplicate the approved layout algorithm.
- Authority: Task 6; design sections 3.1, 3.2, 5 and 6. No parser, checker,
  grammar or diagnostic changes.
- Affected files and behavior: `ridl-fmt` module and the new interface golden.
  Generalize the existing test assertion helper to a profile parameter so each
  new fixture checks its second pass, structure and comment/content streams.
- Verification: direct/array stream regression failed with the documented
  dropped text; all 17 new ridl tests produced one passing malformed-input
  control and 16 expected layout/preservation failures. All fixtures parsed.
  `cargo test -p ridl-fmt --locked` then passed, including 48 unit tests and
  corpus invariants at widths 100, 60 and 40. Tested base `8b858710` plus the
  local Task 6 diff; committed-head checks follow this task commit.
- Commit and PR: this Task 6 change is on `feat/387-fmt-ridl`; the grouped PR
  follows Task 8.
- Maintainer action: review the stream regression and the grouped PR when open.

## D-H10 — Task 6 initializer comments

- Date and timezone: 2026-10-01, Europe/Paris.
- Status: implementation choice.
- Question or observed case: the first interaction renderer extracted only an
  initializer's literal, dropping a comment between `=` and the literal. Input
  `signal s: T = /* initializer */ DEFAULT` lost that comment.
- Chosen action: add a failing fixture first, then retain the entire `InitValue`
  text when it contains a comment, with one space before it. Keep normal
  initializer spacing for comment-free values.
- Reason and alternatives considered: verbatim initializer rendering preserves
  its comment while allowing the rest of the member to use canonical layout.
  Returning the whole member verbatim would also suppress unrelated formatting.
- Authority: design section 5's construct-comment rule and the no-comment-loss
  invariant; formatter-only correction within Task 6.
- Affected files and behavior: interaction initializer rendering and one unit
  regression; no parser or checker changes.
- Verification:
  `cargo test -p ridl-fmt ridl_initializer_comment_is_preserved
  --locked`
  exited 101 with the dropped comment; `cargo test -p ridl-fmt
  --locked` then
  passed all 49 unit tests and the corpus/invariant suites. Local log
  `task-06-red-init-comment.log`; base `8b858710` plus Task 6 diff.
- Commit and PR: included in this Task 6 change on `feat/387-fmt-ridl`.
- Maintainer action: inspect the comment regression in the grouped PR.

## D-H11 — Task 6 quick-review documentation corrections

- Date and timezone: 2026-10-01, Europe/Paris.
- Status: implementation choice and approved existing rule.
- Question or observed case: QUICK review of `3e6e9bf` found two live claims
  that interfaces remain verbatim, and a stale pending paragraph in the review
  report. It also cited the design/plan's historical initial-state descriptions.
- Chosen action: correct the book, rsdl test module comment and review report;
  record the completed Task 6 commit and acceptance. Retain historical initial
  state in the design/plan, with later dated execution records authoritative.
- Reason and alternatives considered: current user-facing behavior must match
  the code; rewriting historical baseline text would obscure the starting point
  that the handoff explicitly preserves. No normative design rule changes.
- Authority: QUICK code-to-prose review and the handoff's state-verification
  instruction about historical wording and later dated entries.
- Affected files and behavior: documentation only; no executable change.
- Verification: all three seats used actual Terra; tests/docs and bugs wrapper
  high, built-in bugs worker medium. Six changed paths matched the commit.
  Formatter acceptance passed on `3e6e9bf`; correction documentation checks
  follow. See [Task 6 evidence](fmt-ridl-rsdl-evidence/task-06.md).
- Commit and PR: Task 6 `3e6e9bf`; documentation correction follows it; grouped
  PR after Task 8, based on PR #632.
- Maintainer action: review the grouped PR; no additional decision is needed.

## D-H12 — Task 7 shared attribute layout and invariant normalization

- Date and timezone: 2026-10-01, Europe/Paris.
- Status: implementation choice and approved existing rule.
- Question or observed case: predicate attributes require brace-body comment
  placement, while width breaking must consider attributes and their value lists
  in the same order as parameters and tuple returns.
- Chosen action: reuse the single forward container collector for attribute
  blocks, and add attribute layout to the existing rendering tree. Measure code
  before trailing attribute comments. Render expression spacing recursively from
  the CST, preserving parentheses. Normalize only the sibling timing/attribute
  pair in both test invariant comparisons.
- Reason and alternatives considered: eagerly rendering attributes with their
  own width loop would prevent last-to-first breaking across a whole member. A
  separate comment pass would duplicate the existing container rules. Broad
  token sorting would hide member or comment loss; the narrow shared test
  normalization leaves those identities and all other order intact.
- Authority: design sections 3.2, 5, 6 and 9; D-2 through D-6 and D-13.
- Affected files and behavior: formatter layouts, unit tests and shared
  test-only invariant helpers; no grammar, checker or configuration changes.
- Verification: normalization test failed before the helper and passed after it;
  comment/member removal, literal changes and member reordering remain detected.
  Layout RED: 14 failures and one malformed-input control passed; all RIDL tests
  also run before production changes. Logs under the Task 7 evidence directory.
- Commit and PR: Task 7 follows `4a65b45` in the Tasks 6–8 grouped branch.
- Maintainer action: review the grouped PR; no additional approval is needed.
