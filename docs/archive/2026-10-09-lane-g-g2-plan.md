# Lane G stage G2 plan: the baseline gate, the provisional-order sentence

Issue: driftsys/ridl#778 items D-3 and the specification drift; rulings R-1,
R-2, R-3.

## Task 1 — RIDL-412 for a package or a whole unit gone from the fresh set

- Files: `crates/ridl/src/main.rs` (`interface_refusals`, `dropped_numbers`),
  `crates/ridl/tests/baseline_gate.rs`, `docs/book/cli-reference.md`.
- Red tests: a whole unit removed from a two-unit workspace is refused (exit 1,
  one RIDL-412); a package removed from a unit that remains names the override;
  deleting the unit's snapshot publishes. The whole-unit test fails on an
  assertion (exit 0) before the fix.
- Fix: `dropped_numbers` also takes the package-level `DeclRemoved` change and
  loses every shape the published package holds whose number the fresh unit does
  not keep; the message names the override when the package is gone.
- Existing tests that renamed a package (leaving its unit) now meet the gate;
  they rewrite the snapshot as a pre-lock one (number 0, PD-9).
- Acceptance: `cargo test -p ridl-cli`.

## Task 2 — the provisional-order sentence (R-3)

- File: `docs/specification/ridl-language-reference.md`.
- Cites the tests in `crates/ridl-sem/src/check.rs`: byte order from next, whole
  unit, name not whole key, name not package order. The interface-before-inline
  tie-break is pinned only by its observable order (the CLI test in Task 1's
  file).

## Pre-flight

- Defect found: the brief does not list `baseline_desk.rs` among the files to
  change; two existing tests need the pre-lock rewrite. Recorded and done.
