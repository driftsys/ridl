# Lane G stage G5 plan: gates, CI, xtask and CLI messages

Worktree `.claude/worktrees/lane-g-g5`, branch `ci/lane-g-g5-gates`, from
`origin/main` at 82e83c1a. Each task starts from a red test whose red state is
an assertion failure. Implementers: Sonnet. Task reviewers: Opus, each returning
"meets the brief" and "code is sound" and applying the named mutation.

## Task 1 - #426 gate-parity cannot see a removed member

Files: `justfile` (recipe `gate-parity`), new test
`crates/ridl/tests/gate_parity.rs`.

Fix: `gate-parity` holds an explicit list of the expected members of `build` and
compares it with the list it reads from the `build:` line, in both directions,
naming every removed and every added member. A member is stated twice (the
`build:` line and the list); no new recipe, no CI change.

Red test: copy `justfile` and `.github/workflows/ci.yml` into a temp dir (the
shape of `vsix_packaging_passes_the_target_and_output_arguments`), remove one
member from the copied `build:` line, run `just gate-parity`, assert a non-zero
exit and that stderr names the member. One case per member, read from the real
`build:` line. A second test adds a member to the copied line and a matching
`run: just` line in the copied workflow and asserts failure naming it. A third
asserts the unmodified copy passes.

Mutation for the reviewer: change the comparison so only the "removed" direction
is checked, or delete one name from the expected list.

## Task 2 - #729 two timing-flaky plugin tests

Files: `crates/ridlc/src/plugin.rs` (tests only).

Reproduce first under CPU load (spawn about twice as many busy loops as cores,
run `cargo test -p ridlc --lib plugin::tests::with_a_child` in a loop) and
record the measured cause. Fix: the tests that do not test the timeout use a
named constant of 120 seconds instead of 10; a test returns as soon as its child
exits, so the longer bound costs no time on a pass. The tests of the timeout
keep their short timeout. If the measured cause is not the bound, fix the cause
and say so.

Red test: a test that fails today for the reason found (for example, a child
that takes longer than 10 seconds to exit because of a deliberate `sleep 11`
before `exit 3`, asserting the non-zero exit is reported, not a timeout). The
reviewer applies the mutation of putting the 10-second bound back.

## Task 3 - #687 test of the CI changes-job filter and the `if:` gates

Files: new test `crates/ridl/tests/ci_workflow.rs`. No change to `ci.yml` unless
a case shows a defect in it (then a ruling, not a silent edit).

Test: extract the `run:` script of the `filter` step from `ci.yml`, run it with
`bash` in a temp git repository whose `HEAD^1..HEAD` diff holds a fixed file
list, with `GITHUB_OUTPUT` pointing to a file, and assert the exact `rust=` and
`markdown=` lines. Cases: docs-only, Rust-only, mixed, a rename out of a Rust
path, `.editorconfig`, `LICENSE`, an empty diff, a list over 64 KiB, and a
non-pull-request event. A second test asserts that `rust`, `wasm` and `markdown`
carry `needs: changes` and the matching `if:`, that `book` has no `if:`, that
`ci.needs` contains `changes`, and that the `changes` checkout has
`fetch-depth: 2`.

Mutations for the reviewer (from the issue): `if: false` on `rust`; the filter
always writing `rust=false`; `crates/` removed from the filter; the markdown
condition inverted; `if: false` on `book`; `changes` dropped from `ci.needs`;
`fetch-depth: 1`. Each is applied to a copy of `ci.yml` read by the test through
one helper, or the reviewer edits the real file and restores it.

## Task 4 - #739 calibrate dump writes `[lints]` for dropped lints

Files: `xtask/src/calibrate.rs`, `xtask/tests/calibrate_cli.rs`. ADR-0024 binds
the lint names.

Fix: `dump` writes a `[lints]` entry only for a check the built `ridl` ships.
Source of truth: the binary or the catalogue, not a second list in xtask, unless
no query exists; if a list is unavoidable, escalate. The findings arrays for the
dropped checks stay written (the records still carry them), but they are empty.

Red test: a dump over a fixture corpus must produce a manifest copy that
`ridl check` accepts without MANI-010.

## Task 5 - #196 an unreadable path is reported with the wrong cause

Files: `crates/ridl-core/src/workspace.rs` (small, G3 also edits the loader:
keep the edit small, expect a rebase), and the CLI message path only if the
cause is lost after the loader. Tests in `crates/ridl/tests` (unix only,
`chmod 000`, skipped when the test runs as root).

Fix: (1) a workspace root that holds `ridl.toml` but cannot be read reports the
OS error and the path, not "no `ridl.toml` found". (2) an unreadable
subdirectory inside a readable workspace names that subdirectory and the OS
error. Covers `ridl check`, `ridlc check`, `ridl build`, `ridlc build`,
`ridl baseline`, `ridl test`. No new diagnostic code (exit 2 stays).

## Task 6 - #734, #646, #645 catalogue versus reference drift guards

Files: `crates/ridlc/tests/corpus.rs` (the helper `reference_diagnostic_rows`,
the test `typl_and_ridl_catalogue_entries_match_the_reference_tables`, the test
`rsdl_profile_codes_match_the_reference_table`), and the reference tables only
where a row must change punctuation.

The three share one table reader and one comparison, so they are one task:

- #646: the table-only rows (a row with no catalogue entry) are compared with an
  explicit list of the deferred and retired codes; a table-only code not on the
  list fails, and a listed code with no row fails.
- #645: a clause is separated from the summary by `,` only inside the summary.
  The extension a row adds after the catalogue summary may begin only with `(`
  or `-`; a `,` continuation fails, so a catalogue summary that loses a clause
  is detected. A row that relies on `,` as an explanation gets `-` in the
  reference.
- #734: the rsdl §16.1 table's Rule column gets the same comparison against
  `RSDL_CATALOG` summaries.

Red tests: the issue mutations as unit tests over the comparison function with
in-memory rows (RIDL-408 lost clause, TYPL-107 renamed to TYPL-170, an RSDL
summary altered), so the check does not need to mutate the files.

## Pre-flight read

- Task 2: a red test that sleeps 11 seconds on every run costs 11 seconds
  forever. Replace it with a test that pins the bound: the tests that do not
  test the timeout take their timeout from one constant, and one test asserts
  the constant is at least 60 seconds (the default `--plugin-timeout`). The
  reviewer proves it by restoring 10.
- Task 5: `find_root` returns `Option` and `is_file()` reports an unreadable
  directory as "not a file", so the cause is lost before `load_workspace_with`
  builds its message. The fix has to probe `ridl.toml` with `metadata` and keep
  the error kind, not only reword the message.
- Task 6: the rsdl §16.1 table has four columns (code, rule, severity, section);
  `reference_diagnostic_rows` expects five cells, so the rsdl table needs its
  own cell pattern.
