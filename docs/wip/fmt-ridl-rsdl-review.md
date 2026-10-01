# Formatter maintainer review report

Updated: 2026-10-01, Europe/Paris. Follow [the plan](fmt-ridl-rsdl-plan.md) and
[the design](fmt-ridl-rsdl-layout.md).

Tasks 1–4 are merged in [PR #626](https://github.com/driftsys/ridl/pull/626) and
[PR #628](https://github.com/driftsys/ridl/pull/628).
[PR #630](https://github.com/driftsys/ridl/pull/630) remains open at `769b541`,
with all required CI checks successful at the initial remote inspection.

| Task | Implementation                                  | Commit and PR/base                                                        | Acceptance                          | Review and CI                                                                        |
| ---- | ----------------------------------------------- | ------------------------------------------------------------------------- | ----------------------------------- | ------------------------------------------------------------------------------------ |
| 5    | Caller changes in open PR                       | `e515b2e`, docs correction `f887c78`; PR #632 / `docs/387-fmt-book-width` | CLI, LSP and formatter tests passed | Pass 1: four coverage gaps corrected locally; pass 2 pending; CI passed at `f887c78` |
| 6    | Pending, including #625                         | None                                                                      | Not run                             | Not run                                                                              |
| 7    | Pending                                         | None                                                                      | Not run                             | Not run                                                                              |
| 8    | Pending                                         | None                                                                      | Not run                             | Not run                                                                              |
| 9    | Pending, including deferred rsdl CLI width test | None                                                                      | Not run                             | Not run                                                                              |
| 10   | Pending                                         | None                                                                      | Not run                             | Not run                                                                              |
| 11   | Pending                                         | None                                                                      | Not run                             | Not run                                                                              |
| 12   | Pending                                         | None                                                                      | Not run                             | Not run                                                                              |
| 13   | Pending                                         | None                                                                      | Not run                             | Not run                                                                              |

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

[Issue #625](https://github.com/driftsys/ridl/issues/625) remains pending
Task 6. The handoff's before/after stream evidence has not yet been reproduced
in this execution; no preservation or completion claim is made here.

Existing debts remain open in milestone E1 — typl + Tooling Spine:
[#627](https://github.com/driftsys/ridl/issues/627) (breaking-loop performance),
[#629](https://github.com/driftsys/ridl/issues/629) (Task 4 coverage), and
[#631](https://github.com/driftsys/ridl/issues/631) (configurable indentation).
No review finding has been deferred. All four retained pass 1 findings concern
caller test coverage; corrections are implemented and mutation-verified.
[PR #632](https://github.com/driftsys/ridl/pull/632) was opened after
`just verify` and the enabled pre-push hook passed on
`f887c780c0d4b58fdcddd3a94f82117b6be8616e`.

Suggested merge order: PR #630, then Task 5, then each subsequent task PR in
order. Later tasks will be stacked on their immediate unmerged predecessor.
Nothing in this execution authorizes merging any PR.

Work is on `feat/387-fmt-callers` in the sibling `ridl-fmt-remaining` worktree.
The original book branch and untracked handoff are preserved. The owned Task 5
stash `8bd2716627d07aa342d9a98b17a8dba539ed6523`, message
`On feat/387-fmt-editorconfig: Task 5 caller tests and width plumbing pending rsdl test sequencing`,
remains intact, as do all other stashes. Resume with Task 5 correction commit,
gates and full pass 2.
