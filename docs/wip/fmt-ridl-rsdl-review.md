# Formatter maintainer review report

Updated: 2026-10-02, Europe/Paris. Follow [the plan](fmt-ridl-rsdl-plan.md) and
[the design](fmt-ridl-rsdl-layout.md).

Tasks 1–4 are merged in [PR #626](https://github.com/driftsys/ridl/pull/626) and
[PR #628](https://github.com/driftsys/ridl/pull/628).
[PR #630](https://github.com/driftsys/ridl/pull/630) is merged into main at
`037256d5068a6222599030bef96105f831640a3a`, including Task 5 from merged
[PR #632](https://github.com/driftsys/ridl/pull/632) and the CI scheduling fix.
The maintainer performed both merges.

| Task | Implementation                                  | Commit and PR/base                                                              | Acceptance                                   | Review and CI                                                                              |
| ---- | ----------------------------------------------- | ------------------------------------------------------------------------------- | -------------------------------------------- | ------------------------------------------------------------------------------------------ |
| 5    | Merged with PR #630                             | `e515b2e`, `f887c78`, `d31f4b9`, `8b85871`; PR #632 / `docs/387-fmt-book-width` | CLI, LSP and formatter tests passed          | Two full passes complete; remaining coverage debt #633; CI passed at `8b85871`             |
| 6    | Implemented locally; #625 tests pass            | `3e6e9bf`; grouped PR forthcoming / main                                        | Formatter and LSP acceptance passed          | QUICK complete, docs corrected; grouped full review after Task 8                           |
| 7    | Implemented locally; approval applied           | `e27a3dd` plus correction; grouped PR pending / main                            | Exact-head formatter tests and Clippy passed | QUICK tests/bugs raised annotation-comment case; D-H14 approved and corrected; no group CI |
| 8    | Pending                                         | None                                                                            | Not run                                      | Not run                                                                                    |
| 9    | Pending, including deferred rsdl CLI width test | None                                                                            | Not run                                      | Not run                                                                                    |
| 10   | Pending                                         | None                                                                            | Not run                                      | Not run                                                                                    |
| 11   | Pending                                         | None                                                                            | Not run                                      | Not run                                                                                    |
| 12   | Pending                                         | None                                                                            | Not run                                      | Not run                                                                                    |
| 13   | Pending                                         | None                                                                            | Not run                                      | Not run                                                                                    |

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

[Issue #625](https://github.com/driftsys/ridl/issues/625) remains open; its
correction is implemented in Task 6 commit `3e6e9bf`. The old formatter exited 0
while changing `a: <T>` to `a:` followed by one space, and `b: [<T>; 1..2]` to
`b: [; 1..2]`. The corrected formatter retains both stream types. Its regression
tests pass reparsing, structure/comment comparison and second formatting; the
rebuilt CLI also passes a second `--check`. See
[Task 6 evidence](fmt-ridl-rsdl-evidence/task-06.md). The grouped PR is not yet
open and will contain the permitted issue #625 closing reference.

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

Tasks 6–8 form one consecutive PR with separate task commits and evidence, now
based on main after restacking. Later task PRs will be stacked on their
immediate unmerged predecessor. No merge is authorized.

Work is now on `feat/387-fmt-ridl` in the sibling `ridl-fmt-remaining` worktree.
The original book branch and untracked handoff are preserved. The owned Task 5
stash `8bd2716627d07aa342d9a98b17a8dba539ed6523`, message
`On feat/387-fmt-editorconfig: Task 5 caller tests and width plumbing pending rsdl test sequencing`,
remains intact, as do all other stashes. Task 5 is complete with recorded review
debt. Task 6 acceptance and QUICK review are complete; current book and
test-module claims are corrected. Task 7 attribute layout and invariant
acceptance pass, and all three QUICK seats are complete. D-H14 records the
maintainer's approved timing-first comment attachment and supersedes D-H13.

## Current resume point

Tasks 6 and 7 are implemented locally on `feat/387-fmt-ridl`, restacked onto
main `037256d`. They are not published or CI-verified. The approved regression
renders `query q(): T [persist] /* note */ @10ms` as
`query q(): T @10ms [ persist ] /* note */`. Inline comments between annotations
stay with their preceding annotation; line comments retain their newline. Other
direct comments retain the verbatim path. Exact output, reparsing, structure,
comment content and second formatting pass are checked. A deliberate
comment-loss mutation fails the invariant control.

Task 8 is next. Tasks 9–13 remain pending. The grouped PR, its full reviews,
full gate and CI remain pending. Original worktree, handoff and all stashes are
preserved. See [Task 7 evidence](fmt-ridl-rsdl-evidence/task-07.md).
