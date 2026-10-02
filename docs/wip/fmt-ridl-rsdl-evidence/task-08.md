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
