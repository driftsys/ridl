# Formatter maintainer review report

Updated: 2026-10-02, Europe/Paris. Follow [the plan](fmt-ridl-rsdl-plan.md) and
[the design](fmt-ridl-rsdl-layout.md).

Tasks 1–4 are merged in [PR #626](https://github.com/driftsys/ridl/pull/626) and
[PR #628](https://github.com/driftsys/ridl/pull/628).
[PR #630](https://github.com/driftsys/ridl/pull/630) is merged into main at
`037256d5068a6222599030bef96105f831640a3a`, including Task 5 from merged
[PR #632](https://github.com/driftsys/ridl/pull/632) and the CI scheduling fix.
The maintainer performed both merges.

| Task | State                                     | Commit and PR/base                              | Acceptance and review                                                       |
| ---- | ----------------------------------------- | ----------------------------------------------- | --------------------------------------------------------------------------- |
| 5    | Merged                                    | PR #632 into #630 into main `037256d`           | Caller tests, full reviews and CI passed; coverage debt #633                |
| 6    | Merged                                    | `faab6bb`, docs `bd6c6d7`; PR #634 / main       | Formatter/LSP acceptance passed; QUICK docs corrected                       |
| 7    | Merged; collision follow-up               | `e27a3dd`, correction `b8fbe9c`; PR #634 / main | Formatter, Clippy and comment-loss mutation passed; QUICK finding corrected |
| 8    | Merged                                    | `46ab1f5`; PR #634 / main                       | Formatter/reference acceptance passed; all QUICK seats found nothing        |
| 9    | Pending, including deferred rsdl CLI test | None                                            | Not run                                                                     |
| 10   | Pending                                   | None                                            | Not run                                                                     |
| 11   | Pending                                   | None                                            | Not run                                                                     |
| 12   | Pending                                   | None                                            | Not run                                                                     |
| 13   | Pending                                   | None                                            | Not run                                                                     |

Decisions are recorded in [the append-only log](fmt-ridl-rsdl-decisions.md).
D-H1 preserves the approved design. D-H2 records explicit approval of Task 5
test sequencing: use a typl tuple now, retain rsdl reader coverage, add a valid
rsdl CLI width test in Task 9. D-H3 records the branch dependency and test
recovery. There is no unanswered Task 5 sequencing proposal.

Task 5 resolves CLI width per file and LSP width from the document path, while
formatting the current buffer. Indentation stays two spaces. Implemented
compiler profiles are typl, ridl and rsdl; the five-extension book example is an
editor configuration example. See
[Task 5 evidence](fmt-ridl-rsdl-evidence/task-05.md) for baseline, expected
failures, acceptance, mutation and gate results.

[Issue #625](https://github.com/driftsys/ridl/issues/625) was completed by
merged PR #634; its correction is implemented in restacked Task 6 commit
`faab6bb`. The old formatter exited 0 while changing `a: <T>` to `a:` followed
by one space, and `b: [<T>; 1..2]` to `b: [; 1..2]`. The corrected formatter
retains both stream types. Its regression tests pass reparsing,
structure/comment comparison and second formatting; the rebuilt CLI also passes
a second `--check`. See [Task 6 evidence](fmt-ridl-rsdl-evidence/task-06.md).
The correction is published in
[PR #634](https://github.com/driftsys/ridl/pull/634), which contains the
permitted issue #625 closing reference.

Existing debts remain open in milestone E1 — typl + Tooling Spine:
[#627](https://github.com/driftsys/ridl/issues/627) (breaking-loop performance),
[#629](https://github.com/driftsys/ridl/issues/629) (Task 4 coverage), and
[#631](https://github.com/driftsys/ridl/issues/631) (configurable indentation).
The four pass 1 coverage findings are corrected in `d31f4b9` and
mutation-verified. Pass 2 retained two further LSP coverage gaps, deferred under
the two-pass cap to [#633](https://github.com/driftsys/ridl/issues/633),
milestone E1 — typl + Tooling Spine. No production failure was identified. The
docs finding about committed logs was refuted after inspection of the local
logs. [PR #632](https://github.com/driftsys/ridl/pull/632) was opened after
`just verify` and the enabled pre-push hook passed on
`f887c780c0d4b58fdcddd3a94f82117b6be8616e`. Its correction head
`d31f4b9062bf77139c8d4053d9f91156ba12d588` passed `just verify`, enabled
pre-push hooks, and all CI checks. The evidence file records both heads.

Tasks 6–8 formed merged PR #634 with separate task commits and evidence. The
remaining comment fixes are on the correction branch based on main `c9c7c0e`.
Later task PRs will be stacked on their immediate unmerged predecessor. No merge
is authorized.

Work is now on `fix/387-fmt-annotation-comments` in the sibling
`ridl-fmt-remaining` worktree. The original book branch and untracked handoff
are preserved. The owned Task 5 stash
`8bd2716627d07aa342d9a98b17a8dba539ed6523`, message
`On feat/387-fmt-editorconfig: Task 5 caller tests and width plumbing pending rsdl test sequencing`,
remains intact, as do all other stashes. Task 5 is complete with recorded review
debt. Task 6 acceptance and QUICK review are complete; current book and
test-module claims are corrected. Task 7 attribute layout and invariant
acceptance pass, and all three QUICK seats are complete. D-H14 records the
maintainer's approved timing-first comment attachment and supersedes D-H13.

## Current resume point

Tasks 6–8 were merged by the maintainer in
[PR #634](https://github.com/driftsys/ridl/pull/634), at `6317767`, producing
main `c9c7c0e`. Both published heads `700d116` and `6317767` passed their local
gates, enabled hooks and all CI checks. The approved regression renders
`query q(): T [persist] /* note */ @10ms` as
`query q(): T @10ms [ persist ] /* note */`.

Full review pass 1 retained eleven findings. All eleven findings are corrected.
Ten independent corrections address: two production comment fixes, four coverage
regressions and four prose updates. Three new layout assertions reproduced the
failures before the fixes; the formatter suite then passed, and all four
specified coverage mutations failed. See
[PR evidence](fmt-ridl-rsdl-evidence/pr-634.md).

The independent correction commit is `82b761f`; formatter/LSP acceptance and
`just verify` passed on that exact head. QUICK review completed with actual
Terra models and the correct scope. Its two test suggestions are applied; the
bug seat found nothing and the restricted docs seat found no falsified prose.
Full pass 2 follows publication of the approved collision correction.

D-H19 records the maintainer's explicit approval of D-H16's narrow verbatim
exception. The collision regression failed on valid input before correction;
formatter/LSP acceptance now passes. A moved annotation line comment with any
trailing member comment retains the whole source member. Noncolliding cases
continue to use timing first. No rendering approval remains pending.

D-H22 records the maintainer merge and restacking of the remaining correction
onto main `c9c7c0e` in `fix/387-fmt-annotation-comments`. The original `91d1e6d`
correction tree is preserved at `a41cddd`. Open a correction PR after
actual-head gates, then run the remaining full pass 2 on that open PR with BASE
`700d116`, preserving the original fix diff and two-pass cap. Tasks 9–13 follow
in order. Original worktree, untracked handoff and all stashes are preserved.
