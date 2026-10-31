# Inline lint directives Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build driftsys/ridl#1124: a
`// lint: allow <name>[, <name>] -- <reason>` comment allows lints on one
declaration, and TYPL-411 and TYPL-412 report directive mistakes.

**Architecture:** A new module `ridl_core::lint::directive` reads directives
from a file's CST and resolves each one's scope. `apply_lint_levels` keeps its
signature and also removes a lint diagnostic that a directive allows, which
keeps it idempotent. A new function, `report_lint_directives`, raises TYPL-411
and TYPL-412. Each entry point calls it exactly once, over its raw list, after
its last diagnostic is raised and before the levels are applied. The carrier
predicate moves from `ridl-sem` to `ridl-syntax`, so the doc lints and the
directives use one list.

**Tech Stack:** Rust, rowan CST (`ridl-syntax`), `ridl-core` catalogue, `ridlc`,
`ridl` CLI, `ridl-lsp`, `ridl-mcp`, `ridl-fmt`.

**Spec:** [ADR-0024](../decisions/ADR-0024-lint-registry-and-levels.md) decision
2 (as changed on 2026-10-30), decisions 4, 6, 8, 17, 18 and 19, and its
2026-10-30 Status paragraph. The work list is issue driftsys/ridl#1124. Read
both before starting a task.

## Global Constraints

- Spelling: the text after `//` and optional whitespace starts with `lint:`;
  then `allow`, one or more comma-separated lint names, `--`, and a reason that
  is not empty. Only a `SyntaxKind::LineComment` is read, never a `DocComment`
  or a `BlockComment`.
- Names are `[lints]` names (`lint_by_name`), never codes.
- A directive wins over every `[lints]` level, `deny` included. An Error code is
  never removed.
- TYPL-411 `unused-lint-directive` and TYPL-412 `malformed-lint-directive` are
  both `Info`, with no `default = allow`. A directive never removes either one.
- `apply_lint_levels` stays idempotent (decision 6). TYPL-411 is computed once
  per report, over the list before any level is applied (decision 19).
- Only the entry points that apply levels (decision 8) read directives. The
  grammar does not change.
- Prose rules from AGENTS.md: plain, literal English. No story id or plan name
  in `crates/` or `docs/book/` (`just story-id-check`).
- Implementers run focused crate tests only. CI runs the full gate.

## Review Focus

1. **CRLF sources.** A file with `\r\n` line endings gives the same scopes as
   the same file with `\n`, including the no-blank-line rule. Test in Task 1.
2. **Stray commas and empty names.** `allow missing-docs, -- r` and
   `allow , missing-docs -- r` draw TYPL-412 for the empty item, and
   `missing-docs` still applies. Test in Task 1.
3. **A directive among doc comment lines.** `/// a` + directive + `/// b` +
   declaration applies to the declaration, and the scope starts at `/// a`. Test
   in Task 1.
4. **An end-of-line directive on a one-line nested declaration.**
   `struct Pose { x: m, y: m } // lint: allow …` targets `Pose`, not a field.
   Test in Task 1.
5. **The formatter breaks an overlong declaration that carries an end-of-line
   directive.** After formatting, the directive still targets the same
   declaration. Test in Task 6.

---

### Task 1: Read directives and resolve their scope

**Files:**

- Modify: `crates/ridl-syntax/src/ast.rs` (add `is_doc_carrier`)
- Modify: `crates/ridl-sem/src/doc_lint.rs:92-125` (delete `is_carrier`, call
  `ridl_syntax::ast::is_doc_carrier`)
- Create: `crates/ridl-core/src/lint/directive.rs`
- Modify: `crates/ridl-core/src/lint.rs` (`mod directive;` and re-exports)

**Interfaces:**

- Produces:

  ```rust
  // ridl_syntax::ast
  /// A carrier of a doc comment (typl reference §14.4), the declarations a lint
  /// directive applies to (ADR-0024 decision 2).
  pub fn is_doc_carrier(kind: SyntaxKind) -> bool; // exactly the list now in doc_lint::is_carrier

  // ridl_core::lint (re-exported from lint::directive)
  pub struct LintDirective {
      /// The range of the `//` comment token.
      pub range: TextRange,
      /// The lint rows of the valid names, each with the range of its name text.
      pub allows: Vec<(&'static CatalogEntry, TextRange)>,
      /// The declaration node's range, or `None` when no declaration applies.
      pub target: Option<TextRange>,
      /// `target` widened to start at the first doc comment of the declaration.
      pub scope: Option<TextRange>,
      pub problems: Vec<DirectiveProblem>,
  }
  pub enum DirectiveProblem {
      NotAllow { word: String },               // first word after `lint:` is not `allow`
      NoNames,                                 // no non-empty item before `--`
      UnknownName { name: String, range: TextRange }, // includes a code and an empty item
      DirectiveLintName { name: String, range: TextRange }, // names TYPL-411 or TYPL-412
      NoReason,                                // no `--`, or only whitespace after it
      NoDeclaration,
  }
  impl LintDirective {
      /// False when `problems` holds NotAllow, NoNames, NoReason or NoDeclaration.
      pub fn is_effective(&self) -> bool;
  }
  pub fn directives_in(root: &SyntaxNode) -> Vec<LintDirective>; // source order
  ```

Scope resolution (the signature leaves this choice open, so it is fixed here):

- A directive is **own-line** when only whitespace lies between the previous
  line break (or the start of the file) and the comment. Otherwise it is
  **end-of-line**.
- For an own-line directive, walk the tokens forward. Whitespace adds its `\n`
  count to a running count, which resets at each non-whitespace token. A count
  of 2 or more is a blank line, which gives `NoDeclaration`. A `DocComment` or a
  directive `LineComment` is skipped. Any other comment gives `NoDeclaration`.
  At the first other token `k`, the target is the outermost ancestor of `k` that
  is a carrier and whose first non-trivia token is `k`. If there is none, the
  result is `NoDeclaration`.
- For an end-of-line directive, the target is the outermost carrier whose first
  non-trivia token starts on the comment's line, before the comment. If there is
  none (for example after a closing `}`), the result is `NoDeclaration`.
- `scope` = `doc_comments_before(target).first()` start (or the target start) to
  the target end.
- Line breaks are counted as `\n`, so CRLF needs no special case.
- Reason split: the text after the first `--`. Names: the text between the level
  word and that `--`, split on `,`, each item trimmed.
- Ruling R-1 (not in the ADR; report it to the owner): naming
  `unused-lint-directive` or `malformed-lint-directive` is `DirectiveLintName`.
  The name is ignored, as an unknown name is. Decision 18 forbids a directive
  from allowing either lint, and a mistake to fix at once is TYPL-412's role.

- [ ] **Step 1: Move the carrier predicate.** Add `is_doc_carrier` to
      `ridl_syntax::ast` and use it in `doc_lint.rs`. Run
      `cargo test -p ridl-sem doc_lint`. Expected: PASS, with no change in
      behaviour.
- [ ] **Step 2: Write the failing tests** in `directive.rs` `mod tests`. Each
      test parses a source with `ridl_syntax::parse(text, profile)` and asserts
      on `directives_in`:
  - `own_line_directive_targets_the_next_declaration`: `target` is the `TypeDef`
    range, and `allows` holds `unbounded-float`.
  - `scope_starts_at_the_doc_comment`: `/// a` + directive + `/// b` +
    `type T: float` gives a `scope` start equal to the start of `/// a`.
  - `blank_line_after_directive_is_no_declaration`.
  - `plain_comment_between_is_no_declaration`.
  - `end_of_line_on_one_line_struct_targets_the_struct` (Review Focus 4).
  - `end_of_line_on_a_field_line_targets_the_field`.
  - `end_of_line_after_closing_brace_is_no_declaration`.
  - `before_package_or_import_is_no_declaration`.
  - `crlf_gives_the_same_scopes_as_lf` (Review Focus 1).
  - `doc_and_block_comments_are_not_directives`: `/// lint: allow …` and
    `/* lint: allow … */` give an empty list.
  - `word_other_than_allow_is_not_allow`: `lint: deny missing-docs -- r`.
  - `code_and_unknown_name_are_unknown_and_others_still_apply`:
    `allow RIDL-100, missing-docs -- r` gives one `UnknownName` and
    `missing-docs` in `allows`.
  - `empty_item_is_unknown_name` (Review Focus 2).
  - `missing_or_empty_reason_is_no_reason`: no `--`, and `--`.
  - `naming_a_directive_lint_is_directive_lint_name` (R-1).
  - `rsdl_member_line_is_a_target` (`.rsdl` profile, an `offers` line).
  - `interaction_parameter_is_a_target` (a `command` parameter on its own line).
- [ ] **Step 3: Run** `cargo test -p ridl-core lint::directive`. Expected: FAIL
      (does not compile: `directives_in` is not defined).
- [ ] **Step 4: Implement** `directives_in` and `is_effective` as specified.
- [ ] **Step 5: Run** `cargo test -p ridl-core lint::directive` and
      `cargo test -p ridl-syntax`. Expected: PASS.
- [ ] **Step 6: Commit**
      `feat(ridl-core): read lint directives and resolve
      their scope`.

### Task 2: A directive removes the diagnostics it allows

**Files:**

- Modify: `crates/ridl-core/src/lint.rs` (`apply_lint_levels` and its tests)

**Interfaces:**

- Consumes: `directives_in`, `LintDirective::is_effective`, `allows`, `scope`
  (Task 1).
- Produces: `apply_lint_levels(diagnostics, sources, scopes)`, with the same
  signature and the new behaviour below. It also provides one private helper
  that Task 3 reuses:

  ```rust
  /// The directives of every file of `sources` whose path ends in `.typl`,
  /// `.ridl` or `.rsdl` and whose text contains `lint:`, keyed by file id.
  /// The file is parsed with `crate::db::profile_of_path`.
  fn directives_by_file(sources: &SourceMap) -> BTreeMap<FileId, Vec<LintDirective>>;
  ```

Behaviour: a diagnostic is removed before its level is looked up when all of
these hold:

- its code is a lint (`lint_of`) other than TYPL-411 or TYPL-412;
- an effective directive of its file allows that lint;
- the directive's `scope` contains `primary.range`.

The check uses the code, not the severity, so a diagnostic that an earlier
application raised to `deny` is still removed on a second application.

- [ ] **Step 1: Write the failing tests** in `lint.rs` `mod tests`. They build a
      `SourceMap` with a `.ridl` text and diagnostics at given ranges:
  - `directive_removes_its_lint_inside_scope`.
  - `directive_wins_over_deny`: the scope has `missing-docs = deny`, and the
    diagnostic is removed.
  - `directive_leaves_a_diagnostic_outside_scope`.
  - `directive_leaves_another_lint`.
  - `directive_never_removes_an_error_code`: a diagnostic with an Error code
    inside the scope stays.
  - `ineffective_directive_removes_nothing`: one with no reason.
  - `directive_never_removes_typl_411_or_412`.
  - `directives_are_idempotent`: two applications give the same list.
  - `toml_file_is_not_scanned`: a `ridl.toml` whose text contains
    `// lint: allow` leaves its MANI-010 diagnostic in place.
- [ ] **Step 2: Run** `cargo test -p ridl-core lint::tests`. Expected: the new
      tests FAIL on their assertions.
- [ ] **Step 3: Implement** the removal in `apply_lint_levels`. Update its doc
      comment and the module doc to name the directive.
- [ ] **Step 4: Run** `cargo test -p ridl-core`. Expected: PASS.
- [ ] **Step 5: Commit**
      `feat(ridl-core): remove a lint diagnostic that a
      directive allows`.

### Task 3: TYPL-411 and TYPL-412

**Files:**

- Modify: `crates/ridl-core/src/diag.rs` (two rows after TYPL-410 at `:606`; the
  expected list at `:2438`)
- Modify: `crates/ridl-core/src/lint.rs` (`report_lint_directives`)
- Modify:
  `crates/ridl-core/src/diag/snapshots/ridl_core__diag__sarif__tests__sarif_shape.snap`
  (regenerate)
- Modify: `freeze/diagnostics.txt` (regenerate with `cargo xtask freeze update`)
- Modify: `docs/book/lints.md` (two table rows, checked by
  `crates/ridl/tests/book_lints.rs`)
- Modify: `docs/specification/typl-language-reference.md` §16.5 (two rows after
  TYPL-410 at `:1468`)
- Modify: `docs/specification/rsdl-language-reference.md:844` and
  `docs/specification/ridl-family-overview.md:294` ("TYPL-404 to TYPL-410"
  becomes "TYPL-404 to TYPL-412")

**Interfaces:**

- Consumes: `directives_by_file`, `LintDirective`, `DirectiveProblem`.
- Produces:

  ```rust
  /// Raises TYPL-412 for each problem of each directive, and TYPL-411 for each
  /// name of an effective directive whose lint raises no diagnostic of
  /// `diagnostics` inside the directive's scope. `not_raised` lists the lint
  /// codes this entry point never raises; a name of one of them is never
  /// unused. Call once per report, after the last diagnostic is raised and
  /// before `apply_lint_levels` (ADR-0024 decision 19).
  pub fn report_lint_directives(
      diagnostics: &mut Vec<Diagnostic>,
      sources: &SourceMap,
      not_raised: &[DiagCode],
  );
  ```

Catalogue rows (both `Info`):

- `TYPL_411`: catalogue message "lint directive allows a lint that no diagnostic
  in its scope raises", `lint = "unused-lint-directive"`.
- `TYPL_412`: catalogue message "malformed lint directive",
  `lint = "malformed-lint-directive"`.

Diagnostic messages, with primary spans:

- TYPL-411, on the name's range:
  ``"`{name}` is allowed here, but no `{name}`
  diagnostic is raised in this declaration"``.
- TYPL-412, on the comment's range, or on the name's range for the two name
  problems:
  - `NotAllow`: ``"a lint directive only accepts `allow`, not `{word}`"``
  - `NoNames`: "the lint directive names no lint"
  - `UnknownName`:
    ``"`{name}` is not a lint name; a directive names a lint by
    its `[lints]` name, not by its code"``
  - `DirectiveLintName`:
    ``"a directive cannot allow `{name}`; set its level in
    `[lints]`"``
  - `NoReason`: "the lint directive has no reason after `--`"
  - `NoDeclaration`: "the lint directive applies to no declaration; place it on
    the line before one, with no blank line, or at the end of the line where one
    starts"

TYPL-411 counts a diagnostic in `diagnostics` that exists when the function is
called, whatever its severity. A TYPL-411 or TYPL-412 added by the same call
does not count.

- [ ] **Step 1: Write the failing tests** in `lint.rs`:
  - `unused_name_draws_typl_411_on_the_name`.
  - `used_name_draws_nothing`.
  - `allow_by_default_lint_raised_counts_as_used`: a TYPL-410 diagnostic at
    Warning inside the scope means no TYPL-411.
  - `not_raised_name_is_never_unused`: `not_raised = [RIDL_407]` and a directive
    naming RIDL-407's lint name give no TYPL-411.
  - `each_problem_draws_one_typl_412` (one assertion per `DirectiveProblem`
    variant, on code, span and message).
  - `report_then_apply_twice_keeps_one_report`: `report_lint_directives`, then
    `apply_lint_levels` twice. The list holds exactly one TYPL-411 and no
    allowed diagnostic.
- [ ] **Step 2: Run** `cargo test -p ridl-core lint`. Expected: FAIL to compile.
- [ ] **Step 3: Add the rows and the function.** Add both names to the expected
      list at `diag.rs:2438`.
- [ ] **Step 4: Regenerate** with
      `INSTA_UPDATE=always cargo test -p ridl-core sarif` and
      `cargo xtask freeze update`. Check that the diff of each regenerated file
      adds only the two rows.
- [ ] **Step 5: Docs rows.** Add the two rows to `docs/book/lints.md`, to typl
      §16.5, and to the two code ranges. Run
      `cargo test -p ridl --test book_lints` and `just freeze-check`. Expected:
      PASS.
- [ ] **Step 6: Run** `cargo test -p ridl-core`. Expected: PASS.
- [ ] **Step 7: Commit**
      `feat(ridl-core): add TYPL-411 and TYPL-412 for lint
      directives`.

### Task 4: The command line reports directives once

**Files:**

- Modify: `crates/ridlc/src/lib.rs`: `front_end` (`:196`), `run_check` (`:921`),
  `run_build_with` (`:1305`)
- Modify: `crates/ridl/src/main.rs` (`run_check`, `:710-748`)
- Test: `crates/ridl/tests/lints.rs`

**Interfaces:**

- Consumes: `report_lint_directives`, `apply_lint_levels`.
- Produces:

  ```rust
  /// `run_check` without `report_lint_directives` and without the levels:
  /// the raw list, after `retain_in_report_scope`.
  pub fn run_check_unlevelled(entry: &Path, frozen: Frozen) -> std::io::Result<CliRun>;
  ```

  `ridlc::run_check` becomes `run_check_unlevelled`, then
  `report_lint_directives(.., &[DiagCode::RIDL_407])`, then `apply_lint_levels`.

Placement of the report, at each call site:

- `front_end` and `run_build_with` (only under `ApplyLints::Yes`, on
  `diagnostics` and `outside_scope`): immediately before the existing
  `apply_lint_levels`, with `not_raised = &[DiagCode::RIDL_407]`.
- `run_check` in `ridlc`: call `report_lint_directives` before
  `retain_in_report_scope`, so that a directive in a file outside the report
  scope is checked against that file's diagnostics, and the result is then
  filtered with them. Move the retain call after the report.
- `ridl` CLI `run_check`: call `ridlc::run_check_unlevelled`. After the desk
  check and the padding check, call `report_lint_directives` once, then
  `apply_lint_levels` once. `not_raised` is `&[]` when `desk_check` ran, and
  `&[DiagCode::RIDL_407]` otherwise. Replace the comment that says the levels
  are applied a second time.
- RIDL-150: if PR #1130 has retired RIDL-150 when this task runs, nothing
  changes here. Otherwise the CLI paths raise it (`load_and_check`, `:2416`), so
  it is not in `not_raised`.

- [ ] **Step 1: Write the failing tests** in `crates/ridl/tests/lints.rs`. Reuse
      that file's workspace helpers.
  - `directive_allows_a_lint_at_deny_and_check_exits_0`: `[lints]` sets
    `unbounded-float = "deny"`, and a directive on the type allows it.
  - `unused_directive_reports_typl_411_in_check_json`.
  - `malformed_directive_reports_typl_412_and_allows_nothing`: no reason; the
    lint is still reported.
  - `baseline_desk_check_reports_a_directive_once`: with a baseline present, the
    output holds exactly one TYPL-411 for an unused name.
  - `build_applies_directives`: a directive on a lint at `deny` lets
    `ridl build` write its artifacts.
  - `diff_ignores_directives`: `ridl diff` keeps the emitted severity of an
    allowed lint (decision 8).
- [ ] **Step 2: Run** `cargo test -p ridl --test lints directive`. Expected:
      FAIL on assertions.
- [ ] **Step 3: Implement** the placements above.
- [ ] **Step 4: Run** `cargo test -p ridlc` and
      `cargo test -p ridl --test lints`. Expected: PASS.
- [ ] **Step 5: Commit**
      `feat(ridlc): apply lint directives in check and
      build`.

### Task 5: The language server and the MCP tool, with parity

**Files:**

- Modify: `crates/ridl-lsp/src/server.rs:899` (`analyze`)
- Modify: `crates/ridl-mcp/src/lib.rs:326` (`ridl_check` path mode)
- Test: `crates/ridl-lsp/tests/server.rs`, `crates/ridl-mcp/src/lib.rs` tests,
  and a parity test in `crates/ridl/tests/servers.rs`

**Interfaces:**

- Consumes: `report_lint_directives`, `apply_lint_levels`.

Placement:

- MCP path mode: `report_lint_directives` before `apply_lint_levels`, on
  `snap.output.diagnostics`. `snapshot` already ran `retain_in_report_scope`, so
  use the order of Task 4's `ridlc::run_check` instead. If `snapshot` cannot
  report before it retains without changing what the lookup tools see, report on
  the retained list, and add a test that a directive in a file outside the
  report scope draws nothing. `not_raised = &[DiagCode::RIDL_407]`.
- LSP `analyze`: `report_lint_directives` on `all` after the remote filter and
  before `apply_lint_levels`. `load()` keeps its `apply_lint_levels` call, which
  now also removes allowed diagnostics. The loader's list is levelled at load
  time, against the text on disk, so `not_raised` in `analyze` is `RIDL_407`,
  `RIDL_150` (decision 18; skip it if #1130 retired the code), and every lint
  code that `ridl_core::load_workspace` raises with a span in a `.typl`, `.ridl`
  or `.rsdl` file. Find those codes by reading the emit sites in
  `crates/ridl-core/src/workspace.rs`, and list them in a comment at the call.

- [ ] **Step 1: Write the failing tests:**
  - LSP: `directive_allowed_lint_is_not_published` and
    `unused_directive_is_published_as_typl_411`, using the existing
    `[lints]`-based test at `server.rs:5942` as the model.
  - LSP: `unsaved_edit_moves_the_directive_scope`. An overlay adds a line above
    the directive. The allowed lint stays unpublished, and nothing else changes.
  - MCP: `ridl_check_path_mode_applies_directives`.
  - Parity, in `crates/ridl/tests/servers.rs`:
    `directives_give_the_same_diagnostics_in_check_lsp_and_mcp`. One workspace
    holds one used directive, one unused directive and one malformed directive.
    The set of `(code, file, line)` from `ridl check --format json`, from the
    `ridl lsp` publishDiagnostics, and from the MCP `ridl_check` path mode is
    equal.
- [ ] **Step 2: Run** `cargo test -p ridl-lsp directive`,
      `cargo test -p ridl-mcp directive` and
      `cargo test -p ridl --test servers directive`. Expected: FAIL on
      assertions.
- [ ] **Step 3: Implement** the placements.
- [ ] **Step 4: Run** the three commands of Step 2. Expected: PASS.
- [ ] **Step 5: Commit**
      `feat(ridl-lsp): apply lint directives in the language
      server and the MCP tool`.

### Task 6: The formatter keeps every directive on its declaration

**Files:**

- Test: `crates/ridl/tests/lint_directives_fmt.rs` (the facade depends on both
  `ridl-fmt` and `ridl-core`; `ridl-fmt` does not depend on `ridl-core`)
- Modify: `crates/ridl-fmt/src/lib.rs` only if a test fails

**Interfaces:**

- Consumes: `directives_in` (`target`), `ridl_fmt`'s pure format entry point,
  and the ok corpus that `crates/ridl-fmt/tests/properties.rs` reads.

Identity of a target: the pair (directive text with its trailing whitespace
trimmed, preorder index of the target node among the file's carrier nodes, or
`None`). Formatting changes whitespace and separator commas, but not the order
of the nodes.

- [ ] **Step 1: Write the failing tests:**
  - `own_line_directive_stays_directly_above`: `format` output has the directive
    on the line just before the declaration.
  - `end_of_line_directive_stays_on_its_line`.
  - `overlong_declaration_keeps_its_end_of_line_directive` (Review Focus 5): a
    `command` whose parameter list is over 100 characters, with an end-of-line
    directive. The identity is unchanged after formatting.
  - `directive_targets_survive_formatting_over_the_corpus`: for every ok corpus
    file, with a directive inserted on its own line before each carrier and at
    the end of each carrier's first line, the identity list is unchanged after
    formatting.
- [ ] **Step 2: Run** `cargo test -p ridl --test lint_directives_fmt`. Record
      which tests fail. If none fail, the formatter already meets the
      constraint, and the tests pin it.
- [ ] **Step 3: Fix the formatter** for each failing case. For an overlong
      declaration, the end-of-line comment stays on the line where the
      declaration starts.
- [ ] **Step 4: Run** the test file and `cargo test -p ridl-fmt`. Expected:
      PASS.
- [ ] **Step 5: Commit**
      `test(ridl-fmt): pin lint directive placement through
      formatting`
      (use `fix(ridl-fmt)` if Step 3 changed code).

### Task 7: Document the directive as built

**Files:**

- Modify: `docs/book/lints.md`. Replace the sentence that says no source
  attribute changes a level. Add a section, "Allowing a lint on one
  declaration", with the spelling, the scope rules, the precedence, and TYPL-411
  and TYPL-412. Include one verified `` ```ridl `` block that compiles with no
  diagnostic: it declares its own package, and a directive allows a lint the
  block would otherwise draw.
- Modify: `docs/specification/typl-language-reference.md` §13. Add one sentence:
  a `//` comment that starts with `lint:` is read by the checker as a lint
  directive, ADR-0024 decision 2.
- Modify: `docs/decisions/ADR-0024-lint-registry-and-levels.md` Status. Replace
  "Not built: driftsys/ridl#1124. Until it is built, …" with the as-built
  statement, and record ruling R-1 there.

- [ ] **Step 1: Write the docs.**
- [ ] **Step 2: Run** `cargo test -p ridl --test book_examples`,
      `cargo test -p ridl --test book_lints`, `just link-check`,
      `just doc-path-check`, `just story-id-check` and `just check`. Expected:
      PASS.
- [ ] **Step 3: Commit** `docs(book): describe lint directives`.

---

## After the last task

The whole-branch `/review` (passes 1 and 2). Then garden this plan with
`sdd-gardening`: archive it to `docs/archive/`, so that `docs/wip/` holds none
of this work. Then open the PR with "Closes #1124".
