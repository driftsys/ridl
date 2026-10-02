# Task 13 evidence

Branch: `feat/387-fmt-completion`, parent Task 12 correction `9a2d52a`, based on
main `0bc48da`. D-H33 records the recovery of Tasks 9–10 and final PR grouping;
D-H35 records coverage suggestions and D-H36 the documentation audit. D-H37
records the effective-width test correction.

## Documentation scope

Updated the architecture formatter entry, CLI module comment, crate description
and formatter module's profile/width descriptions. The brace helper lists all
implemented declarations; the dispatch fallback still describes only nodes
without a layout rule. The book describes all breakable constructs and the
approved preceding-annotation comment attachment with the narrow verbatim member
exception. Its wider five-extension editor example remains unchanged; the
repository section names exactly typl, ridl and rsdl.

The book audit checks every `ridl fmt`, width and indentation statement, plus
inline colon quotations against the reformatted fences. Reference wrappers and
the general-form/specification files remain outside the sweep. No WIP file is
archived or gardened. The review report is updated to the final branch state.

## Final coverage audit

Three assertions address the completed tests seat's advisory coverage gaps from
PR #637's interrupted full review. Existing behavior is tested; no production
layout changes are made. Four mutations independently fail with exit 101 on the
new assertions:

- Limit the machine separator rule to empty bodies.
- Disable width breaking for system/distribution headers.
- Disable width breaking for component member lines.
- Return distributions with direct body comments verbatim.

Local logs: `final-coverage-mutation-populated-machine.log`,
`final-coverage-mutation-sibling-header-width.log`,
`final-coverage-mutation-component-line-width.log` and
`final-coverage-mutation-distribution-comment.log`. Source was restored after
all mutations. `CARGO_INCREMENTAL=0 cargo test -p ridl-fmt --locked` then exits
0: 107 unit and 20 integration tests, `final-coverage-restored-success.log`. The
first restored run failed for disk space, recorded separately in
`final-coverage-restored.log`; it is not reported as a behavior failure.

## Done criteria

- All seven declaration kinds have exact-output canonical dispatch tests; the
  named and inline service forms are both covered. Empty, nested and commented
  bodies, header/member attributes and source gaps are tested.
- Each breakable construct is covered at the default width and explicit widths,
  including boundaries, Unicode indentation, trailing comments, `off`,
  unbreakable text and repeated measurement. Three-profile corpus and reference
  invariants run at widths 100, 60 and 40.
- CLI per-file and LSP buffer/path configuration tests pass in prior task
  acceptance. Only width is read; the repository now has the normative section.
  Wasm without default features remains a member of the final gate.
- The D-10 (b) sweep is complete and held by the three new fixed-point tests;
  book compilation, baseline span expectations and the cabin demo pass.
- Required Task 13 commands and the final `just verify` run follow the task
  commit. The final main-targeted PR references the issue only after those gates
  pass. Review results and CI will be recorded without claiming an interrupted
  review as complete. No merge or automatic merge is authorized.

Logs remain local and ignored. Required documentation commands are
`just link-check`, `just doc-path-check`, `just check` and
`cargo doc -p ridl-fmt --no-deps --locked`.

Task 13 documentation acceptance passes, all exit 0:
`CARGO_INCREMENTAL=0 cargo doc -p ridl-fmt --no-deps --locked`,
`just link-check`, `just doc-path-check` and `just check`. Logs are
`task-13-cargo-doc.log`, `task-13-link-check.log`, `task-13-doc-path-check.log`
and `task-13-check.log`. Disabling incremental caching changes build storage
only, not formatter behavior or gate commands.

Task 13 is committed in `7f0aa60`; all three QUICK seats complete with no
findings at verified Terra/medium (built-in bugs) and Terra/high (restricted
docs/tests). `just verify`, enabled push hooks and initial PR #638 CI all pass
on that exact head. Full pass 1 completes with five retained findings; their
corrections and pass-2 verification are recorded in [PR evidence](pr-638.md).
