# Formatter completion review

The final branch is `feat/387-fmt-completion`, based on main `0bc48da`. Tasks
11–13 are implemented. [PR #638](https://github.com/driftsys/ridl/pull/638) is
open against main. The reviewed code head `aaf383a` passed local gates and CI;
both full review passes are complete. One Minor evidence-timing finding is
deferred in [#639](https://github.com/driftsys/ridl/issues/639), milestone E1.
The final status-only publication will run the same gates on its own head. No
rendering approval is pending. The maintainer merges the final PR.

## Status and evidence

| Task | State and commit                         | PR/base                   | Acceptance, review and latest CI                                                                                                                         |
| ---- | ---------------------------------------- | ------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 5    | Merged through `037256d`                 | #632 into #630, then main | Caller acceptance, full review and CI passed; [evidence](fmt-ridl-rsdl-evidence/task-05.md)                                                              |
| 6    | Merged `faab6bb`                         | #634 / main               | Stream preservation, structure/idempotence and CI passed; [evidence](fmt-ridl-rsdl-evidence/task-06.md)                                                  |
| 7    | Merged `e27a3dd`, corrected `b8fbe9c`    | #634 / main               | Attributes, annotation comments, mutations and CI passed; [evidence](fmt-ridl-rsdl-evidence/task-07.md)                                                  |
| 8    | Merged `46ab1f5`                         | #634 / main               | Service/reference acceptance, QUICK and CI passed; [evidence](fmt-ridl-rsdl-evidence/task-08.md)                                                         |
| 9    | Recovered `6c515bc`, corrected `a76b499` | #638 / main               | Container, CLI/LSP acceptance, both full reviews and latest CI passed; [evidence](fmt-ridl-rsdl-evidence/task-09.md)                                     |
| 10   | Recovered `fa5a4c5`, corrected `a6c6f5b` | #638 / main               | Machine acceptance, pass-1 correction, pass 2 and latest CI passed; [evidence](fmt-ridl-rsdl-evidence/task-10.md)                                        |
| 11   | Committed `8b135ba`                      | #638 / main               | Acceptance, QUICK, full reviews and latest CI passed; [evidence](fmt-ridl-rsdl-evidence/task-11.md)                                                      |
| 12   | Committed `ad877b4`, corrected `9a2d52a` | #638 / main               | 41 book, 49 desk, 32 gate tests; QUICK, full reviews and latest CI passed; [evidence](fmt-ridl-rsdl-evidence/task-12.md)                                 |
| 13   | Committed `7f0aa60`                      | #638 / main               | Documentation checks, QUICK, full local gate and latest CI passed; full reviews complete, Minor debt #639; [evidence](fmt-ridl-rsdl-evidence/task-13.md) |

Tasks 1–4 were merged in PRs #626 and #628. Annotation-comment corrections are
merged in PR #635; [its evidence](fmt-ridl-rsdl-evidence/pr-635.md) records both
full reviews and CI. The initial PR #638 head is `7f0aa60`; its five retained
pass-1 findings are fixed in `ac52315`. QUICK follow-ups are in `aaf383a`. Full
pass 2 covers `7f0aa60..aaf383a`; its one retained Minor is deferred as #639
under the two-pass limit. The [decision log](fmt-ridl-rsdl-decisions.md) records
approved renderings, implementation choices and final dispositions.

## Main integration and reviews

The maintainer merged PR #635 into main, then PR #637 into its original stacked
base `fix/387-fmt-annotation-comments`, at `c87c3f3`. The final branch restores
Tasks 9–10 with four distinct cherry-picked commits and adds separate final task
commits. Its PR targets main directly; no predecessor merge is needed.

PR #637's head `a5a17bf` passed `just verify`, enabled push hooks and all CI.
Its full review was interrupted by the Codex usage limit. The completed tests
seat's three coverage suggestions are addressed by new assertions and four
failing mutations; this does not make that interrupted review complete. The
final PR received fresh full review: pass 1 retained five findings, all fixed;
pass 2 retained one Minor documentation finding, filed as #639. Task 12's bug
and restricted docs QUICK seats completed without findings. Its tests retry
completed after a full-disk output failure; the accepted path-width finding is
corrected in `9a2d52a`, with an observed RED assertion and 41 passing book
tests. Actual model metadata is retained locally.

## Individual task commits and earlier verification

Task 5 is published through PR #632 into #630 and main `037256d`; caller tests,
full review and CI passed. Tasks 6, 7 and 8 use separate commits `faab6bb`,
`e27a3dd` and `46ab1f5` in merged PR #634 against main. Their acceptance, QUICK
corrections, full reviews and final CI pass are recorded in the
[PR #634 evidence](fmt-ridl-rsdl-evidence/pr-634.md).

Tasks 9 and 10 are recovered as `6c515bc` and `fa5a4c5`, with corrections
`a76b499` and `a6c6f5b`, in the final main-targeted branch. Their original PR
#637 head passed local gates and CI; that PR's full review remains incomplete.
Tasks 11, 12 and 13 have separate commits and linked acceptance evidence above;
initial final-branch CI passed on `7f0aa60`; reviewed correction-head CI passed
on `aaf383a` (run 37007230085). The final PR is the only required merge, and
must be reviewed against main.

Issue #625 is completed in merged PR #634. The old formatter changed `a: <T>` to
`a:` plus a space and `b: [<T>; 1..2]` to `b: [; 1..2]`.
`ridl_struct_preserves_direct_and_array_stream_types` reproduced this before the
fix; both stream types now survive reparsing, structure/comment comparison and a
second formatting pass. See [Task 6](fmt-ridl-rsdl-evidence/task-06.md).

## Approved behavior and remaining debt

The approved rendering is `query q(): T @10ms [ persist ] /* note */` for source
`query q(): T [persist] /* note */ @10ms`. Comments stay with the preceding
source annotation. When a moved annotation line comment would consume another
trailing member comment, the whole member remains verbatim. The narrow exception
is implemented and tested; no question remains pending.

CLI options resolve per file; LSP options resolve from the document path while
formatting the current buffer. Indentation remains two spaces; only
`max_line_length` is read. Implemented compiler profiles are typl, ridl and
rsdl; the five-extension book example configures editors. Expression breaking,
configurable indentation, additional compiler profiles and the
reference/general-form sweep remain outside this implementation.

Earlier review debt remains in #627 (breaking-loop performance), #629 (reader
coverage), #631 (configurable indentation), #633 (LSP coverage) and #636
(service preservation coverage). Pass 2 adds
[#639](https://github.com/driftsys/ridl/issues/639) for time-qualified review
evidence. All six issues are in milestone E1 — typl + Tooling Spine. No deferred
work is added without need for the current contract. The decision log is
append-only; earlier implementation/review details remain in task and PR
evidence files.

## Preservation and final publication

The original checkout, untracked handoff, original branches and all five
pre-existing stashes remain intact. The owned Task 5 stash remains
`8bd2716627d07aa342d9a98b17a8dba539ed6523`; no stash was popped. Disposable
compiler caches were cleaned to resolve disk pressure. Sources and review
records were preserved. WIP gardening and archiving remain a separate maintainer
action.

The final PR contains `Part of #387` and `Closes #387`, was published after
`just verify` with enabled hooks, and targets main. Both prescribed full passes
are complete. Five pass-1 findings are fixed; the pass-2 Minor is filed as E1
debt. The cap is a limit on review rounds, not a claim that no defects remain.
CI is monitored on each published head. The maintainer reviews and merges PR
#638 directly into main; no stacked dependency remains. No automatic merging
will be enabled.

## PR 638 correction evidence

The machine-gap regression fails on valid source without an optional comma, then
passes when comments after machine blocks are collected in the deployment body.
Empty/populated machines and block/line/doc comments are covered, including
comment runs after the last machine. Bare dotted-reference comments in system,
distribution and machine members and nested 39/40/41-column boundaries now have
exact assertions, structure/content checks and second passes. Both described
coverage mutations fail; the restored formatter suite passes 110 unit and 20
integration tests. The token-scope descriptions and baseline-gate count are
corrected. See [PR evidence](fmt-ridl-rsdl-evidence/pr-638.md).

The correction QUICK also checks multiple adjacent comments between machines and
at the end of the deployment. Its first-comment-only mutation is rejected; the
two generic module statements now name the machine exception. Full pass 2
verified these follow-ups with the original correction and repeated all four
mutations, each rejected by its targeted regression. The restored formatter
suite passes 110 unit and 20 integration tests.

`CARGO_INCREMENTAL=0 just verify` and enabled push hooks pass on exact
`aaf383afa6291913cb135463ad90151f91c43899`. CI run 37007230085 succeeds on that
same head; all applicable checks succeed and Pages is skipped. Logs and review
metadata are recorded in the linked PR evidence. Final status-only changes are
verified again before publication; the
[live PR checks](https://github.com/driftsys/ridl/pull/638/checks) identify the
current published head.
