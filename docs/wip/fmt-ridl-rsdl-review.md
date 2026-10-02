# Formatter completion review

The final branch is `feat/387-fmt-completion`, based on main `0bc48da`. Tasks
11–13 are implemented. Final actual-head gates, publication and full review
remain before this work is reported complete. No rendering approval is pending
and no merge is authorized.

## Status and evidence

| Tasks               | State                                                                   | Evidence                                                                                  |
| ------------------- | ----------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| 1–4                 | Merged before this handoff                                              | PRs #626 and #628                                                                         |
| 5                   | Merged through PR #630                                                  | [Task 5](fmt-ridl-rsdl-evidence/task-05.md)                                               |
| 6–8                 | Merged in PR #634                                                       | [PR #634](fmt-ridl-rsdl-evidence/pr-634.md)                                               |
| Comment corrections | Merged in PR #635; both reviews and CI complete                         | [PR #635](fmt-ridl-rsdl-evidence/pr-635.md)                                               |
| 9–10                | PR #637 merged into its former base; recovered in final branch for main | [Task 9](fmt-ridl-rsdl-evidence/task-09.md), [Task 10](fmt-ridl-rsdl-evidence/task-10.md) |
| 11                  | Committed `8b135ba`; acceptance and QUICK passed                        | [Task 11](fmt-ridl-rsdl-evidence/task-11.md)                                              |
| 12                  | Committed `ad877b4`, corrected `9a2d52a`; acceptance and QUICK complete | [Task 12](fmt-ridl-rsdl-evidence/task-12.md)                                              |
| 13                  | Documentation and final coverage implemented; acceptance passed         | [Task 13](fmt-ridl-rsdl-evidence/task-13.md)                                              |

## Main integration and reviews

The maintainer merged PR #635 into main, then PR #637 into its original stacked
base `fix/387-fmt-annotation-comments`, at `c87c3f3`. The final branch restores
Tasks 9–10 with four distinct cherry-picked commits and adds separate final task
commits. Its PR targets main directly; no predecessor merge is needed.

PR #637's head `a5a17bf` passed `just verify`, enabled push hooks and all CI.
Its full review was interrupted by the Codex usage limit. The completed tests
seat's three coverage suggestions are addressed by new assertions and four
failing mutations; this does not make that interrupted review complete. The
final PR receives fresh full review. Task 12's bug and restricted docs QUICK
seats completed without findings. Its tests retry completed after a full-disk
output failure; the accepted path-width finding is corrected in `9a2d52a`, with
an observed RED assertion and 41 passing book tests. Actual model metadata is
retained locally.

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
final-branch CI is pending publication. The final PR is the only required merge,
and must be reviewed against main.

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
(service preservation coverage), all in milestone E1 — typl + Tooling Spine. No
deferred work is added without need for the current contract. The decision log
is append-only; earlier implementation/review details remain in task and PR
evidence files.

## Preservation and final publication

The original checkout, untracked handoff, original branches and all five
pre-existing stashes remain intact. The owned Task 5 stash remains
`8bd2716627d07aa342d9a98b17a8dba539ed6523`; no stash was popped. Disposable
compiler caches were cleaned to resolve disk pressure. Sources and review
records were preserved. WIP gardening and archiving remain a separate maintainer
action.

The final PR will contain `Part of #387` and the final completion reference
after `just verify` passes. It will be published with enabled hooks, reviewed
through the prescribed two-pass workflow and monitored through CI. No automatic
merging will be enabled.
