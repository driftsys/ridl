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

## Exact head, QUICK review and checkpoint

Implemented Task 7 head: `ae3ddbf405301a7126e0355ae799a3c713408062`.
`cargo test -p ridl-fmt --locked` exited 0 again on this exact head; log:
`task-07-exact-head.log`.
`cargo clippy -p ridl-fmt --all-targets --locked --
-D warnings` exited 0; log:
`task-07-clippy.log`. `just check` exited 0 after prim formatting repaired the
decision entry.

All three fresh QUICK seats completed successfully over
`4a65b45e4cd0c8afc8a0dbe0f4f3f935b03a14ca..ae3ddbf405301a7126e0355ae799a3c713408062`.
Tests, code-to-prose-only docs and the general-purpose bugs wrapper actually ran
`gpt-5.6-terra` with high effort. The wrapper invoked the supported native
built-in review with `gpt-5.6-terra` and explicit medium effort; its startup
metadata confirms both. This is the supported native fallback, not a literal
`/code-review medium` invocation. Each seat reported the nine exact changed
paths; no refuters, ledger or GitHub review comments were used.

Docs found no falsified code-to-prose statement. Tests and bugs independently
reported the direct-comment guard preventing timing-first order for
`query q(): T [persist] /* note */ @10ms`. Their shared issue and the missing
exact-output coverage are **awaiting the maintainer's rendering decision**
(D-H13), not marked fixed or dismissed. A temporary unit probe confirmed the
input parses, the current output keeps attribute-first order, and current
comment/tree preservation and fixed point hold. The probe was removed after
verification. No production correction is applied pending approval.

The branch has not been pushed and the grouped Tasks 6–8 PR has not been opened.
Full review, full PR gate and CI for that group are pending. Task 8 has not
started; Tasks 9–13 remain pending. Checkpoint changes are documentation only.

Checkpoint documentation checks: `just book-check`, `just link-check`,
`just doc-path-check` and `just check` each exited 0. Logs:
`task-07-checkpoint-<recipe>.log`. These later edits change zero executable
lines; QUICK fixes and checkpoint records receive no QUICK pass of their own.

## Approved correction, 2026-10-02

The maintainer approved timing first with intervening inline comments attached
to their preceding source annotation (D-H14). This supersedes the earlier
pending checkpoint. PR #630 and #632 are merged; the work is restacked onto main
`037256d`. Task 7 implementation is now `e27a3dd`; historical hashes above
remain the original review evidence.

- `cargo test -p ridl-fmt --lib ridl_inline_annotation_comments --locked` failed
  with exit 101 before correction, specifically attribute-first output against
  the approved timing-first output. The original guard was reinstated
  temporarily to save the same failing evidence, then restored.
- `cargo test -p ridl-fmt --locked` exited 0 after correction: 70 unit tests, 2
  goldens, 9 configuration tests, 6 properties and 2 rsdl reference tests.
- `cargo clippy -p ridl-fmt --all-targets --locked -- -D warnings` exited 0.
- Comment-loss mutation: excluding comment tokens from the invariant helper made
  `annotation_normalization_detects_a_dropped_intervening_comment` fail, exit
  101. The helper was restored and formatter acceptance passed again.
- Local logs: `task-07-approved-comment-{red,green,mutation}.log`.

New cases cover both annotation orders with block and line comments, a predicate
block with its comment after the closer, a long trailing comment at width 40,
and unrelated direct/standalone comment verbatim controls. They compare exact
output, reparse, structure, comment content and fixed point. The test normalizer
needed no broader reordering. This is a QUICK correction and receives no QUICK
review of its own; full grouped PR review follows Task 8.
