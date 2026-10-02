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

## D-H13 — Task 7 intervening annotation comment: approval checkpoint

- Date and timezone: 2026-10-01, Europe/Paris.
- Status: awaiting approval.
- Question or observed case: QUICK tests and bugs seats independently found
  `query q(): T [persist] /* note */ @10ms` remains attribute-first. The
  existing direct-comment guard emits the whole member verbatim. D-4 states
  timing first regardless of source order; section 5 preserves a commented
  one-line construct verbatim, without specifying this comment's attachment when
  the annotation pair moves.
- Chosen action or proposal: pause Tasks 8–13 and request an explicit choice:
  retain the commented member verbatim, or put timing first with the comment
  either after the attribute block or between timing and attributes. Retaining
  the whole member is the recommended proposal, not an approved exception.
- Reason and alternatives considered: automatically relocating the comment or
  inventing an exception would settle a rendering that the handoff reserves for
  the maintainer. The reviewer finding does not itself authorize a design
  change.
- Authority: handoff stop conditions and plan section 7: "the note does not
  settle a case — an input whose rendering or break position the note does not
  determine". Design sections 3.2 and 5 are the rules needing precedence here.
- Affected files and behavior: no correction applied pending the answer. Task 7
  production remains at `ae3ddbf`; the probe was temporary and restored.
- Verification: the minimal whole-file probe parses without errors, preserves
  structure and comments, and is a fixed point while retaining attribute-first
  order.
  `cargo test -p ridl-fmt --lib
  pending_intervening_annotation_comment_probe --locked -- --nocapture`
  exited 0 on `ae3ddbf` plus the temporary probe. Log:
  `task-07-intervening-comment-probe.log`. All three QUICK contexts completed,
  with matching nine-file scope. See Task 7 evidence for actual model metadata.
- Commit and PR: Task 7 `ae3ddbf405301a7126e0355ae799a3c713408062`, stacked on
  Task 6 in `feat/387-fmt-ridl`; grouped Tasks 6–8 PR is not open.
- Maintainer action: answer the pending rendering question. Then add the
  approved exact-output regression, rerun acceptance, record the disposition and
  resume Task 8. No parser or grammar change is proposed.

## D-H14 — approved annotation comment attachment and updated branch base

- Date and timezone: 2026-10-02, Europe/Paris.
- Status: explicit maintainer approval, superseding the pending D-H13
  checkpoint.
- Question: how `query q(): T [persist] /* note */ @10ms` renders when timing
  moves before attributes.
- Decision: timing first; an inline comment between annotations stays with the
  annotation immediately preceding it in source. The approved output is
  `query q(): T @10ms [ persist ] /* note */`. Line comments keep their newline.
  Other direct comments, standalone comments and multiline block comments retain
  the whole-member verbatim path.
- Authority: the maintainer reconsidered the earlier tentative verbatim choice,
  requested pros and cons, then answered "ok" to the timing-first recommendation
  with preceding-annotation comment attachment. No further approval is pending.
- Reason: canonical annotation order remains predictable and the comment retains
  its attachment. Moving the comment next to timing would change that
  attachment.
- Changes: exact-output regressions for both source orders, line comments,
  predicate blocks, trailing-comment width and verbatim controls; a dropped
  intervening comment remains visible in the invariant stream. The design note
  records this approved amendment.
- Branch: the maintainer merged PR #632 into #630 and #630 into main. Tasks 6–7
  were restacked onto `037256d5068a6222599030bef96105f831640a3a`, preserving
  D-CI1 and D-CI2. Commit mapping: `3e6e9bf` to `faab6bb`, `4a65b45` to
  `bd6c6d7`, `ae3ddbf` to `e27a3dd`, and `336cd2f` to `8f5681c`. The grouped
  Tasks 6–8 PR will now target main.
- Verification: the exact approved output failed with exit 101 before
  correction. Formatter acceptance and the comment-loss mutation follow in Task
  7 evidence.
- Review: this corrects the existing Task 7 QUICK finding; the correction gets
  no recursive QUICK review. The group receives full review after Task 8.
- Maintainer action: review the grouped PR. Tasks 8–13 resume in order.

## D-H15 — Task 8 service forms reuse the shared renderer

- Date and timezone: 2026-10-02, Europe/Paris.
- Status: implementation choice within the approved design.
- Decision: dispatch inline services through the existing brace formatter and
  use their `DottedName` for the header. Add a shape-list layout variant to the
  existing width renderer for named services, with a break after the colon and
  no closer. Keep required commas between shapes and remove the trailing comma.
- Reason: a separate width loop would duplicate the established last-to-first
  breaking rules. Named and inline service bodies need different termination,
  already represented by the CST. A commented named shape list retains its
  source text; inline body comments follow the container rules.
- Authority: design sections 3.3, 5 and 6.3; plan Task 8; ADR-0015 decision 13.
- Tests: both forms with and without trailing commas, the section 3.3 golden,
  width 40 broken shapes with a trailing comment, exact boundary and off,
  between-member/header/brace comments, commented shape lists and dotted paths.
  Every exact-output case checks reparsing, structure, comment identity and a
  second formatting pass. Appendix A runs at widths 100, 60 and 40.
- Verification: five layout regressions failed before implementation; one
  comment preservation control passed. Formatter acceptance passed afterward.
  Evidence is in [Task 8](fmt-ridl-rsdl-evidence/task-08.md).
- Commit and PR: separate Task 8 commit on `feat/387-fmt-ridl`; grouped Tasks
  6–8 PR targets main `037256d` and includes the issue #625 correction.
- Maintainer action: review the grouped PR. No rendering approval is needed.

## D-H16 — annotation line-comment collision requires approval

- Date and timezone: 2026-10-02, Europe/Paris.
- Status: awaiting approval; D-H14 remains approved for its settled cases.
- Observed case: `query q(): T [persist] // attribute`, followed by
  `@10ms // member`. Timing-first rendering currently combines the two comment
  tokens as `query q(): T @10ms [ persist ] // attribute // member`. A multiline
  block comment after timing can also have its opener consumed by the moved line
  comment, producing invalid output.
- Proposal: preserve this member verbatim when the moved annotation line comment
  would collide with a trailing member comment. The approved `/* note */` case
  would still use timing first. The alternative is timing first with
  `// attribute` after attributes and `// member` on its own following line.
- Reason and alternatives considered: both comments cannot remain inline on the
  same final member line as separate tokens. Choosing the alternative would
  change the second comment's placement; choosing the proposal creates a narrow
  exception to canonical annotation order. Neither choice is assumed approved.
- Authority: handoff stop conditions and plan section 7 reserve unsettled
  renderings for the maintainer. The handoff remains local and untracked in the
  original worktree.
- Affected files and behavior: no collision correction applied. Independent
  header, separator, coverage and prose corrections can proceed; full review
  pass 2 and Tasks 9–13 await this answer.
- Verification: full review pass 1 retained the reproduction at confidence 99 on
  `700d116`; the formatter changes comment content and may fail reparsing. See
  [PR #634 evidence](fmt-ridl-rsdl-evidence/pr-634.md).
- Commit and PR: [PR #634](https://github.com/driftsys/ridl/pull/634), based on
  main `037256d`, published head `700d116` when the question was asked.
- Maintainer action: choose the verbatim collision exception or specify the
  timing-first rendering and the second comment's position. The question is
  pending; elapsed time does not supply approval.

## D-H17 — independent full-review corrections

- Date and timezone: 2026-10-02, Europe/Paris.
- Status: implementation choice within existing comment and invariant rules.
- Decision: retain a required newline when the last header token before `{` is a
  line or documentation comment. Clear trailing-comment eligibility when a
  separator comma follows a newline, and preserve existing source blank lines
  while removing separator-only lines.
- Reason and alternatives considered: trimming the header newline makes `{` part
  of the comment. Resetting all newline state at a comma attaches later comments
  to an earlier member and loses blank lines. Ignoring commas entirely would
  instead create a blank line for an ordinary separator-only line. The shared
  collector and brace renderer continue to own these rules.
- Authority: design section 5, comment-token preservation, source blank-line
  rules and invariant reparsing. No grammar or parser change is needed.
- Affected files and behavior: `crates/ridl-fmt/src/lib.rs` and the living wip
  index. Seven new exact-output tests cover the two fixes and four coverage
  findings. Renderer comments now describe attributes and service shape lists.
- Verification: three new tests failed on the published implementation; four
  coverage controls passed. The corrected formatter suite passed. Deliberate
  mutations removing assignment/value comment guards, ensure-only block forcing,
  subtree fallback scope and multiline-comment protection each fail their new
  assertion with exit 101. Local logs are under
  `fmt-ridl-rsdl-evidence/pr-634-*.log`.
- Commit and PR: independent correction commit for PR #634; exact commit and
  gate evidence are recorded in the review report and PR evidence.
- Maintainer action: resolve D-H16 before the remaining correction and pass 2.

## D-H18 — QUICK coverage follow-up and pending-review checkpoint

- Date and timezone: 2026-10-02, Europe/Paris.
- Status: implementation choice; D-H16 remains awaiting approval.
- Decision: add a TYPL struct header line-comment control and combine separator
  blank-line cases with a leading comment, as the test reviewer suggested.
  Publish the independent corrections and checkpoint the unresolved rendering.
- Reason and alternatives considered: the brace and container helpers are
  shared, so the controls constrain their behavior beyond the original RIDL
  reproductions. Repeating QUICK over its own corrections is excluded by the
  review workflow; full pass 2 waits until all pass 1 fixes are authorized.
- Authority: design section 5, invariant preservation, QUICK advisory review and
  handoff checkpoint requirements.
- Affected files and behavior: tests and review records. No additional
  production or rendering change.
- Verification: QUICK docs/tests used actual Terra/high; the fresh bug wrapper
  used Terra/high and native Terra/medium. Scope was the five-file correction
  diff `700d116` to `82b761f`. Bug review found nothing; code-to-prose review
  found no falsified statement. The extra same-line comma suggestion predates
  this correction and is outside that docs direction. Both test suggestions are
  applied; the focused suite passes. `just verify` passed on `82b761f`.
- Commit and PR: follow-up coverage and checkpoint commit in PR #634, based on
  main `037256d`. Actual-head push gates run before publication.
- Maintainer action: answer D-H16. Then implement its regression and approved
  rendering, run full pass 2 over the fix diff, and resume Tasks 9–13 in order.

## D-H19 — approved verbatim exception for colliding line comments

- Date and timezone: 2026-10-02, Europe/Paris.
- Status: explicit maintainer approval; supersedes the pending D-H16 question.
- Question: how to retain two comment tokens when moving timing would place an
  annotation line comment before a trailing member comment on the same line.
- Decision: preserve the whole member verbatim only for this collision. The
  maintainer answered "yes" to the recommended narrow exception. D-H14 remains
  unchanged for noncolliding comments, including the approved `/* note */` case.
- Reason and alternatives considered: this keeps the original attachment and
  both comment tokens. Moving the second comment to a new line was offered but
  was not chosen. The same collision guard handles documentation line comments
  and a trailing block comment, whose opener would otherwise be swallowed.
- Authority: explicit maintainer answer and amended design section 3.2.
- Affected files and behavior: the existing container uses its trailing-comment
  context to preserve the member when an attribute-first line comment would
  move. The member renderer and width loop remain shared; no parser change.
- Verification: after correcting a fixture that initially put `}` inside a line
  comment, the valid exact-output regression failed with exit 101 because two
  comments merged. Formatter/LSP acceptance passes after the guard. Tests cover
  line/doc comments, single/multiline trailing block comments, predicate
  attributes and noncolliding block-comment/timing-first controls. All cases
  check structure, comment identity, reparsing and fixed-point output. Logs:
  `pr-634-collision-red.log` and `pr-634-collision-green.log`.
- Commit and PR: final pass 1 correction for PR #634; separate from earlier
  `82b761f` and `6317767` corrections. Full pass 2 follows publication over only
  changes after `700d116`.
- Maintainer action: review PR #634 after the remaining reviews and CI. No
  rendering approval remains pending; Tasks 9–13 resume after pass 2.

## D-H20 — distinguish consuming line comments from documentation blocks

- Date and timezone: 2026-10-02, Europe/Paris.
- Status: implementation correction within approved D-H19.
- Observed case: `query q(): T [persist] /** attribute */ @10ms // member` was
  retained verbatim because both `///` and `/** ... */` have the lexer kind
  `DocComment`. The block form cannot consume the next comment.
- Decision: restrict the new collision guard to comment tokens whose text begins
  with `//`. Keep documentation block comments on the canonical noncolliding
  path. Extend collision tests to all five interaction kinds and widths 100, 60
  and 40.
- Reason and alternatives considered: token kind alone does not distinguish the
  two documentation-comment forms. Changing the lexer is unnecessary and outside
  the formatter's scope; source spelling is sufficient for this guard.
- Authority: approved narrow collision exception, design sections 3.2 and 5;
  QUICK docs/tests findings.
- Affected files and behavior: formatter collision guard, exact-output controls
  and review evidence. No parser or grammar change; no recursive QUICK pass.
- Verification: documentation-block regression failed before correction;
  formatter/LSP acceptance passes afterward. Evidence: PR #634 evidence and
  `pr-634-doc-block-{red,green}.log`. Full gates passed on prior `6578c3e`;
  actual-head push gates follow for the correction commit.
- Commit and PR: QUICK correction in PR #634 before full pass 2.
- Maintainer action: none; no rendering choice changed.

## D-H21 — use one line-comment classification in affected renderers

- Date and timezone: 2026-10-02, Europe/Paris.
- Status: root-cause correction of the D-H20 QUICK finding.
- Observed case: the shared `DocComment` kind also caused `/** header */` to
  move the opening brace to a new line and `/** timing */` to force a newline
  before attributes. Both valid exact-output controls failed before correction.
- Decision: use one `is_line_comment` helper based on comment kind and `//`
  spelling in header newline protection, annotation newline emission and the
  collision guard. Block documentation comments retain their inline form.
- Reason and alternatives considered: fixing only the new guard left the same
  classification error in two affected rendering paths. The shared helper avoids
  inconsistent treatment without changing the lexer or comment text.
- Authority: design sections 3.2 and 5, approved preceding-annotation attachment
  and the narrow D-H19 collision exception.
- Affected files and behavior: formatter helper and exact-output controls only;
  no new rendering choice and no recursive QUICK review.
- Verification: both controls exited 101 before correction; formatter/LSP
  acceptance passes afterward. Logs are listed in PR #634 evidence. All eight
  push gates passed on prior `c8b8086`; final actual-head gates follow.
- Commit and PR: final root-cause correction in PR #634 before full pass 2.
- Maintainer action: none.
