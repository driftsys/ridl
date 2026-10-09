# Lane G, stage G3 plan: loader and checker diagnostics

Worktree `.claude/worktrees/lane-g-g3`, branch `fix/loader-checker-diagnostics`.
Order: #770, #765, #643. One implementer and one task review per task.

## Task 1: #770, `.rxdl` file skipped without a diagnostic

Ruling R-10: new warning RIDL-417, lint name `unsupported-source-file`.

Files: `crates/ridl-core/src/workspace.rs` (`load_package_tree`, disk path and
overlay path), `crates/ridl-core/src/diag.rs` (declaration, lint name table,
catalogue tests), `docs/book/lints.md`, `docs/book/cli-reference.md` (near the
RIDL-416 text), `docs/specification/ridl-language-reference.md` (catalogue table
next to RIDL-416), `crates/ridlc/tests/corpus.rs` (the RIDL-416 list),
`crates/ridl/tests/lints.rs` (a RIDL-417 level test like RIDL-416's). Update
every place `grep RIDL-416` finds, except where it does not apply.

Red test: a package directory with a valid `.ridl` file and a `.rxdl` file
loaded through the workspace loader yields exactly one RIDL-417 warning per
`.rxdl` file, anchored on the file; a second test for the overlay path (an
overlay keyed to a `.rxdl` path); a `ridl check` test on the directory. Red =
assertion failure (no warning today). Acceptance: the tests pass; the `.rxdl`
file is still not compiled; the `[lints]` table can set the level;
`cargo test -p ridl-core -p ridl -p ridlc`. Mutation for the task reviewer: drop
the overlay-path emission; emit once per directory instead of once per file.

## Task 2: #765, false RIDL-101/RIDL-108 on an unreadable RPC annotation

Files: `crates/ridl-sem/src/timing.rs` (~264 fill branch; ~181 rustdoc; tests
~1379 and `ridl_112_warns_on_undeclared_bounds_and_not_on_unreadable_ones`),
`crates/ridl-core/src/diag.rs` (RIDL-108 catalogue doc ~686),
`crates/ridl-core/src/package.rs` (~71 ADR citation). Red tests:
`query ... @[5s 10s]` and `command ... @[5s..10xs]` draw FORM-101 and no
RIDL-101/RIDL-108. The branch fills `max` from the default only when the range
parsed whole (`range_parsed_whole`); the unreadable case keeps the existing rule
that the max is taken but no RIDL-101/108 is drawn from the filled bound. Decide
the exact shape against the code and the test named in the issue; if the fix
merges the two fallbacks, update that test. Test gaps from the issue: add a row
`@[5s..10xs]` (or remove the claim in the comment); assert `min_us` for
`@[20ms..50xs]` and `@[20ms 50ms]` in the RIDL-112 test, and state the case in
the `resolve_timing` rustdoc. Drift: RIDL-108 catalogue doc covers command and
query with a default maximum; verify the ADR-0002 section cited at `package.rs`
~71 against the ADR and fix the citation. The `(E2 task 9)` references belong to
#757 and are left to it. Mutation: remove the new `range_parsed_whole` guard;
drop the kept `min`.

## Task 3: #643, qualified internal type in a foreign package view

Files: `crates/ridl-sem/src/check.rs` (`lookup_path_in`, ~1011, and callers that
pass a foreign `Resolution`), tests in `crates/ridl-sem`. The internal guard
compares `symbol.package` with `self.package_name`; it must compare with the
package whose view the path is interpreted in. Red test: the two-package
reproduction of the issue (spelling `veh.common.Hidden` and bare `Hidden` in
`MAX`) both give TYPL-108 for `USE: Visible = MAX` when checking `app`. No new
code. Mutation: restore the comparison with `self.package_name`.

## Pre-flight read

Checked against the driver (G3 files only: ridl-core loader, ridl-sem timing.rs
and visibility guard; no `rsdl/` lowering, `ridl-fmt`, `ridl-lsp`,
`ridl-syntax`, `crates/ridl/src/main.rs`, the Rust backend or the descriptor)
and against `origin/main` 376aa927. Defects found: `Resolution` carries no
package name, so Task 3 must add the viewing package to the lookup (parameter or
a field); the implementer picks the smaller change. `(E2 task 9)` is owned by
#757.
