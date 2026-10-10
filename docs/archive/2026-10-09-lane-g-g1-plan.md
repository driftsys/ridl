# Lane G stage G1 — the diff gate (plan)

Worktree `.claude/worktrees/lane-g-g1`, branch `fix/diff-gate`, from
`origin/main` c4c3a9aa. Files: `crates/ridl-diff`, `crates/ridl/src/main.rs`,
`crates/ridl/tests/`, `docs/book/cli-reference.md`, the typl reference.

## Task 1 — #397: enum and enumset members compared by explicit value

Files: `crates/ridl-diff/src/walk.rs` (`positions`, `diff_composite`, the
`EnumDef` and `EnumSetDef` arms of `diff_decl`), `crates/ridl-diff/src/lib.rs`
and `classify.rs` (the `MemberReordered` text), `crates/ridl-diff/src/tests.rs`,
`crates/ridl/tests/diff_member_reorder.rs`, the "As built" note in
`docs/specification/typl-language-reference.md` §8, `docs/book/cli-reference.md`
if it describes the position wording.

Red tests: (1) a pure textual reorder of an enum and of an enumset with every
value unchanged reports nothing (identical, exit 0); (2) a renumber reports
`member_reordered ... value 1 -> value 2` (`bit` for an enumset), breaking; (3)
a reorder arriving with an added member still reports the addition alone; (4) a
reordered `reserved` list alone stays `constraint_changed` (existing test
`a_reordered_enum_reserved_list_is_not_identical`).

Fix: `positions` returns each member's explicit number. In `diff_composite`,
when no slot moved and the bodies differ only in the order of the live members
(a new parameter, evaluated only then; struct and union pass `|| false`), report
nothing. Slot labels become `value` and `bit`. Update the three stale `§17.14`
comments and the existing tests that pin the position wording.

Acceptance: `cargo test -p ridl-diff -p ridl`; mutation by the reviewer: make
`positions` return the 1-based position again, and the red tests must fail.

## Task 2 — #700: a frozen or moved interface number is a diff change

Blocked on a ruling (category and verdict); see the escalation.

## Task 3 — #809: a service deleted beside a kept interface of the same name

Cause (read from `dropped_numbers` in `crates/ridl/src/main.rs`): the walk
reports the removed inline shape of `service doors` at path `pkg/doors` with
`before = interface`, the same path and marker as a declared `interface doors`.
`dropped_numbers` resolves that change with
`package.shapes().filter(|s| s.name == name).take(1)`, which returns the
declared interface (number kept) and never the inline shape (number lost).

Red test: `crates/ridl/tests/baseline_gate.rs`, a package with `interface doors`
and `service doors` published, then `service doors` deleted and its number not
retired: `ridl baseline` must refuse with RIDL-412 naming number 2. Second test:
both removed gives one refusal per number, not two.

Fix: an interface-level change loses every published shape of that name whose
number is lost, and the caller deduplicates by (package, number).

Acceptance: `cargo test -p ridl --test baseline_gate`; mutation: restore
`.take(1)`, and the red test must fail.

## Pre-flight read

Checked against the driver (G1 brief, R-23, R-35) and the code at c4c3a9aa.
Defects found:

- #397 says a renumber is reported "rather than as the container's
  `constraint_changed`". The code reports both today (the existing test
  `an_enum_reorder_with_a_changed_value_reports_constraint_changed` pins the
  pair) because a changed value is content. Kept as is; the issue text is
  inaccurate.
- A naive value slot would make a pure reorder emit `constraint_changed`
  (`!moved` is true, so the container line fires). The fix needs the new
  reorder-only check above.
- The enum `reserved` list reorder test forbids simply treating all order as
  neutral, hence the reorder-only comparison keeps the reserved order.
