# Task 8 evidence

Base: Task 7 approved correction `b8fbe9c` on `feat/387-fmt-ridl`, based on main
`037256d5068a6222599030bef96105f831640a3a` after the maintainer merged #630.

## Tests first

`cargo test -p ridl-fmt --lib service --locked` exited 101 before
implementation: five layout tests failed with unchanged service text, while the
commented-list verbatim control passed. The failures were expected-output
failures on valid input, not parser errors. Local log: `task-08-red.log`.

## Acceptance

`cargo test -p ridl-fmt --locked` passed after implementation. It includes
service exact-output tests, golden pairs, the parser corpus at widths 100, 60
and 40, and the new RIDL Appendix A harness at those widths. Exact-output tests
compare reparsing, structure, content including comments and a second pass.
Local log: `task-08-green.log`.

The section 3.3 golden covers the two service forms. Other cases cover required
commas, optional trailing commas, width 40 broken shapes, trailing comments,
inline body comments, header/brace comments, a comment inside a dotted path, and
block/line comments in a named shape list. Exact-width and off checks cover
inline versus broken lists without letting long trailing comments cause breaks.
No parser, checker, diagnostic or configuration change is made.

QUICK review, full grouped PR review and gates follow the Task 8 commit.

## Exact head and QUICK review

Task 8 commit: `46ab1f56011637dfd13b4bcbbd07126df82d2842`.
`cargo test -p ridl-fmt --locked` exited 0: 77 unit tests, 2 goldens, 9
configuration tests, 6 properties, 1 RIDL reference test and 2 rsdl reference
tests. `cargo clippy -p ridl-fmt --all-targets --locked -- -D warnings`
exited 0. `just check` exited 0.

Fresh QUICK tests/docs/general-purpose bugs wrapper ran actual `gpt-5.6-terra`
at high effort. The bugs wrapper invoked the native built-in review on this
single commit at explicit medium effort. Startup metadata confirms that child
model/effort. All seats returned no findings. Their git scope output contains
the same seven changed paths; two narrative summaries miscounted those paths, so
the count was checked against git rather than copied. No refuters, ledger or
GitHub comments were used. Final native artifact was inspected.

`just verify` on `46ab1f5` exited 0, including every required push gate and the
full build members. Local log: `tasks-06-08-verify.log`. The report records
these results in a later documentation commit; its actual-head push gates
follow. The group remains unpublished and full review/CI follow PR creation.

## Final acceptance audit

Plan section 4 requires a broken-input control for each new declaration. The
audit added an explicit named/inline service control for missing required
commas, a missing shape and an unclosed inline body. It compares the original
parse diagnostics with the formatter's `ParseErrors`; no production behavior
changes. These cases were not used as the Task 8 layout RED. The focused test
passed. This acceptance correction follows the completed QUICK review and is
covered by the grouped full review rather than a recursive QUICK pass.

`just verify` also passed on report head `b7fa1ba`; log:
`tasks-06-08-publication-verify.log`. The final test correction receives the
required actual-head gates before publication.
