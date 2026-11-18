# Task 10 evidence

Branch: `feat/387-fmt-rsdl`, stacked on PR #635's branch at `0249fd1`. Task 9
commit: `c2602dc`; its QUICK documentation follow-up: `3c094a7`. The existing
worktree is bootstrapped. The two-task PR grouping is D-H27; Task 10's
implementation choices are D-H31.

## Tests first

Before Task 10 production changes:

- `cargo test -p ridl-fmt --lib rsdl_ --locked`: exit 101, six selected layout
  and golden failures, seven passing controls. Log: `task-10-red-unit.log`.
  Valid deployments still stay verbatim; the expected nested bodies, canonical
  relation clause, source gaps and updated attribute-position golden differ.
- `cargo test -p ridl-fmt --test corpus --locked`: exit 101; the deployment and
  nested attribute-position goldens are not reached. Log:
  `task-10-red-corpus.log`.

The broken-deployment control returns parse errors as required. No valid fixture
failed parsing; the intended failures are exact-output assertions.

## Acceptance and scope

`cargo test -p ridl-fmt --locked` exits 0: 102 unit and 20 integration tests.
Log: `task-10-green-fmt.log`. This includes syntax-structure, content/comment
and fixed-point checks at widths 100, 60 and 40 over the parser corpus and seven
RSDL reference examples. `just fmt-check` and `just lint` pass; lint log:
`task-10-lint.log`.

New assertions cover:

- The section 4.3 deployment and its three machines; the 41-column machine
  header breaks at width 40, with its brace on the attribute closer's line.
- Qualified `for` references before attributes; a 61-column deployment header
  stays inline at 61 and breaks at 60. Empty deployment bodies remain `{}`.
- Consecutive empty machines with no gap, a source blank line, a between-machine
  comment, and blank lines before and after a leading documentation comment.
- Direct deployment/machine header comments, consuming line-comment newlines,
  and a commented reference subtree with independently formatted attributes.
- Malformed deployment input returned unformatted.
- `all_seven_declaration_kinds_have_canonical_dispatch`: noncanonical ordinary
  input for every RSDL kind, interface, and named/inline service reaches exact
  canonical output. Returning any of those declarations through the verbatim
  fallback would fail the relevant assertion.

The new deployment golden comes from the reference's worked deployment. The
existing attribute-position golden now removes nested machine member separator
commas. Parser inputs remain unchanged. All three RSDL golden pairs also pass
exact-output, reparsing, structure/content and second-pass assertions.

The CLI reference now describes all five RSDL declaration layouts and the nested
machine/source-gap behavior. The fallback comment describes only unspecified
nodes, without naming implemented declarations. Final crate/architecture
publication documentation remains Task 13.

Task commit, QUICK results and actual-head PR gates follow the committed change.
Logs remain local and ignored.

## QUICK review and correction

All three seats completed on `4603d73`, against parent `3c094a7`. Native
execution metadata verifies Terra/high for docs/tests and Terra/medium for the
built-in bug review. All report the same nine changed paths. Restricted docs
review found no statements falsified by the change. The bug seat found a comment
after a machine separator attaching to the preceding machine. The test seat
identified missing coverage for nested machine comments and blank lines.

The new separator regression fails with exit 101 on valid input before the
production fix (`task-10-quick-red.log`). The shared collector now clears
trailing-comment eligibility on a machine separator. An initial broader change
failed the existing reserved-member and block-attribute comment assertions; it
was narrowed to machine separators to preserve those settled layouts. This keeps
subsequent machine comments between members. The nested machine assertion
requires canonical members, opening-brace and trailing comments, a leading
comment, a retained blank line, structure/content preservation and a second
formatting pass at widths 100, 60 and 40. Both suggestions are accepted. QUICK
does not repeat over its own corrections.

Author diff inspection also corrected the review report's stale resume
paragraph; it had still described Task 10 as pending. This is an author
correction, not a reviewer finding. `just verify` passed on `4603d73` with exit
0; final correction head gates follow before the grouped PR is opened. Logs
remain local and ignored.

The restored correction passes `cargo test -p ridl-fmt --locked`: 104 unit and
20 integration tests, exit 0 (`task-10-quick-restored.log`). A mutation
returning comment-bearing machines verbatim makes the new nested-body assertion
fail with exit 101 (`task-10-quick-mutation.log`); production source was
restored before the acceptance run. The earlier broad separator attempt's two
failures remain in `task-10-quick-green.log` for an accurate record.
