# Task 7 evidence

Base: `4a65b45e4cd0c8afc8a0dbe0f4f3f935b03a14ca` on `feat/387-fmt-ridl`; grouped
PR after Task 8, based on PR #632.

## Tests before layout implementation

- `cargo test -p ridl-fmt --test properties annotation_normalization --locked`
  exited 101 before normalization and 0 after the narrow test-only helper.
- `cargo test -p ridl-fmt --lib attribute --locked` exited 101: 14 expected
  layout failures and one malformed-input control passed. No invalid fixture
  provided the failure. Log: `task-07-red-layout.log`.
- `cargo test -p ridl-fmt --lib ridl_ --locked` exited 101, including predicate
  operator spacing and annotation order. Log: `task-07-red-all-ridl.log`.
- The complete attribute golden was also run against Task 6 production, with
  Task 7 tests and the test-only normalizer:
  `cargo test -p ridl-fmt --lib
  ridl_attributes_golden --locked` exited 101.
  Production was restored before acceptance. Log: `task-07-red-golden.log`.

## Acceptance and mutation

`cargo test -p ridl-fmt --locked` exited 0: 67 unit tests, 2 golden tests, 9
EditorConfig tests, 5 properties and 2 rsdl-reference tests. The corpus
invariants run at widths 100, 60 and 40. Every layout unit compares exact
output, reparsing, structure, content including comments, and a second
formatting pass. The new shared helper normalizes only sibling timing/attribute
order.

Normalization controls detect either annotation comment dropped, a missing
member, a changed duration and reordered members. A deliberate mutation that
excluded comments from the normalized content stream made
`annotation_normalization_preserves_comments_members_and_literal_identity` fail
(exit 101); the helper was restored and acceptance passed again. Log:
`task-07-mutation-comment.log`.

Width tests cover 99/100/101 and 59/60/61 for inline attributes and value lists,
width 40 nested lists, width 1, unlimited width and the complete width-60 worked
example. Trailing attribute comments do not force value-list breaks. The
dotted-key unit renders a parsed rsdl attribute node without advancing rsdl body
routing before Task 9.

`just check` initially exited 1 because the appended decision entry needed prim
formatting. It is repaired before commit. No parser or checker changes. QUICK
review and exact commit verification follow.
