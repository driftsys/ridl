# Lint foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every Warning and Info diagnostic code becomes a named lint whose
level a `[lints]` table in `ridl.toml` sets, with the same result in
`ridl check` (text, JSON, and a new SARIF format), `ridl build`, the language
server and the MCP server.

**Architecture:** The catalogue rows in `ridl-core` carry the lint names. The
workspace loader reads `[lints]` from each manifest and produces a `LintScopes`
value, which maps each package directory to its effective levels. One function,
`ridl_core::lint::apply_lint_levels`, rewrites or removes lint diagnostics. The
shared compile (`check_loaded`) does not call it; it carries the scopes out on
`CliRun.lints` and `WorkspaceOutput.lints`. Only the entry points that report
diagnostics call it (spec D-8, §6.2): `ridlc`'s `run_check`, `run_build_with`
when asked, and `front_end` (for `check_source` and `compile`); the CLI after
the `--baseline` desk check; `ridl-mcp`'s `ridl_check` in path mode; and
`ridl-lsp` (in `load()` for loader diagnostics and in `analyze`).

**Tech Stack:** Rust (workspace pin in `rust-toolchain.toml`), `toml` 1.1 with
`Spanned`, `serde`, `codespan-reporting` (text rendering), `insta` (snapshots),
`jsonschema` (new dev-dependency, SARIF validation only).

**Spec:**
[`2026-10-03-lint-foundation-design.md`](2026-10-03-lint-foundation-design.md) —
read it before Task 1. Section numbers below (§n) refer to it.

**Start point:** `main` at edeec6e1, which includes 1a (#668). Line numbers in
this plan were taken before 1a merged and may be off by up to about 200 lines;
locate each function by name. Run `./bootstrap` in the worktree.

## Deviations from the spec

One. The spec put the SARIF projection beside `to_json` in
`crates/ridl-core/src/diag.rs`; the spec now names its own module,
`crates/ridl-core/src/diag/sarif.rs`, and the plan creates that file (Task 6).
The pass-1 review of PR #671 updated the spec so that the two agree: the
directory scopes (§6.1), where the levels are applied (D-8, §6.2), the desk
check gate (§6.2), `lint_of` in `ridl_core::lint` returning `&CatalogEntry`
(§4.1), the validated keys (§5.1), and the language server's two call sites
(§6.2). The spec leaves to this plan the item names, the exact messages, the
`ApplyLints` parameter (Task 4), and the tests.

## Global Constraints

- No new non-dev dependency. `jsonschema` is a dev-dependency of `crates/ridl`
  only, with `default-features = false`.
- Level strings, exactly: `allow`, `info`, `warn`, `deny`.
- The lint names and codes are the table in spec §4.2, verbatim. MANI-010 is the
  only new code: `MANI_010 = "MANI-010", Warning,` summary
  ``"`[lints]` entry names no lint, or its value is not a level"``, lint name
  `unknown-lint`.
- Lint name pattern: `^[a-z][a-z0-9]*(-[a-z0-9]+)*$`.
- JSON field: `"lint"`, omitted when absent
  (`#[serde(skip_serializing_if = "Option::is_none")]`).
- Text note string, exactly:
  ``lint: `<name>` (set its level in `[lints]` in ridl.toml)``. Rendered, for a
  file whose line numbers have one digit, the line is exactly two spaces of
  gutter followed by
  ``= lint: `missing-timing` (set its level in `[lints]` in ridl.toml)``, in the
  form of the existing fix-it note `suggestion: replace with ...`
  (`crates/ridl-core/src/diag/render.rs`).
- SARIF: `version` `"2.1.0"`; `$schema`
  `"https://json.schemastore.org/sarif-2.1.0.json"`; `columnKind`
  `"unicodeCodePoints"`; level mapping Error→`error`, Warning→`warning`,
  Info→`note`; no `helpUri`; no `fixes`.
- Exit codes are unchanged (ADR-0010 decision 1): 1 when any Error is present,
  including, in `ridl check`, `ridl build`, `ridlc check` and `ridlc build`, a
  lint at `deny`; 2 when the check could not run. No other command applies
  levels (spec D-8), so a lint at `deny` never changes the result of
  `ridl diff`, `ridl test`, `ridl baseline`, `ridl lock`, MCP `ridl_diff` or the
  MCP lookup tools.
- Commit scopes come from `.git-std.toml` (`ridl-core`, `ridlc`, `ridl`,
  `ridl-lsp`, `ridl-mcp`, `docs`, `adr`). Prose in comments and docs is plain
  and literal.
- Run `just verify` before the PR. It must pass.

## Review Focus

1. **A lint at `deny` must not hide the baseline check.**
   `ridl check
   --baseline` runs its desk check only when
   `lock::only_lock_orphans` sees no other error. A lint raised to `deny` must
   not count as such an error, so a project gets both its denied lint and its
   RIDL-407 findings in one run. The filter is at the `ridl check` call site
   only, so `ridl lock` is unchanged. Test in Task 5.
2. **`ordinal-changed = "allow"` must silence RIDL-407.** The CLI raises it
   after `ridlc` returns, with spans interned later. Test in Task 5.
3. **A file the editor creates after load uses its member's levels**, not the
   defaults. The directory lookup covers this. Test in Task 7.
4. **A code written as a key** (`RIDL-100 = "deny"`), a level with the wrong
   case (`"Deny"`), and an Error code's catalogue name are each MANI-010, never
   silently accepted. Test in Task 3.
5. **A lint at `deny` must stop `ridl build` before any artifact is written**,
   not only change the exit code. Test in Task 4.
6. **A lint at `deny` must not change any other command** (spec D-8):
   `ridl diff`, `ridl baseline` and `compile_workspace` keep the emitted
   severities. Test in Tasks 4 and 5.

---

### Task 1: Lint names in the catalogue

**Files:**

- Modify: `crates/ridl-core/src/diag.rs` (macro `diag_codes!` at 107-150, the 23
  Warning and Info rows, `CatalogEntry` at 1475, MANI catalogue, tests module
  at 1540)
- Create: `crates/ridl-core/src/lint.rs`
- Modify: `crates/ridl-core/src/lib.rs` (add `pub mod lint;`)

**Interfaces:**

- Produces:
  - `ridl_core::diag::CatalogEntry { code, severity, summary, lint: Option<&'static str> }`
  - `DiagCode::MANI_010`
  - `ridl_core::lint::LintLevel { Allow, Info, Warn, Deny }`, deriving
    `Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash`
  - `LintLevel::parse(&str) -> Option<LintLevel>` (exact lowercase match only)
  - `LintLevel::as_str(self) -> &'static str`
  - `LintLevel::severity(self) -> Option<Severity>` (`Allow` → `None`)
  - `ridl_core::lint::lint_by_name(name: &str) -> Option<&'static CatalogEntry>`
  - `ridl_core::lint::lint_of(code: DiagCode) -> Option<&'static CatalogEntry>`
    (only rows that have a lint name)
  - `ridl_core::lint::default_level(entry: &CatalogEntry) -> LintLevel` (Warning
    → `Warn`, Info → `Info`)

- [ ] **Step 1: Write the failing guard test** in the `diag.rs` tests module,
      named `lint_names_are_present_exactly_on_warnings_and_infos_and_unique`.
      It loops over `ALL_CATALOGS` and asserts:
  - a row with `Severity::Warning` or `Severity::Info` has `lint: Some(_)`;
  - a row with `Severity::Error` has `lint: None`;
  - every name matches `^[a-z][a-z0-9]*(-[a-z0-9]+)*$` (check by hand with
    `chars()`, no regex crate);
  - no two rows share a name (collect into a `BTreeSet`);
  - the set of `(code, name)` pairs equals the 24 pairs in spec §4.2, written
    out in the test.

- [ ] **Step 2: Run it.**
      `cargo test -p ridl-core --locked lint_names_are_present`. Expected:
      compile error (`CatalogEntry` has no field `lint`).

- [ ] **Step 3: Extend the macro and the rows.** Add an optional trailing
      `lint = $lint:literal` to the row pattern, so a row reads:

  ```rust
  RIDL_404 = "RIDL-404", Warning,
      "query named like a mutation", lint = "query-named-like-mutation";
  ```

  The macro expands it to `lint: Some($lint)`, and to `lint: None` when it is
  absent. Add the name to each of the 23 rows in §4.2. Add the `MANI_010` row
  after `MANI_009` with the summary and name from Global Constraints, and a doc
  comment that cites spec §5.3. Check with
  `grep -rn "CatalogEntry {"
      crates/` that no other site constructs
  `diag::CatalogEntry`. Note that `ridl_core::CatalogEntry` is a different type
  (`package::CatalogEntry`).

- [ ] **Step 4: Write `crates/ridl-core/src/lint.rs`** with the items in
      Produces, and unit tests:
  - `LintLevel::parse` accepts the four strings and rejects `"Deny"`, `""` and
    `"forbid"`;
  - `lint_by_name("missing-timing")` is RIDL-100;
  - `lint_by_name("RIDL-100")` is `None`;
  - `lint_of(DiagCode::RIDL_101)`, an Error code, is `None`;
  - `default_level` of RIDL-405 is `Info`.

- [ ] **Step 5: Run.** `cargo test -p ridl-core --locked`. Expected: all pass,
      including the existing catalogue guards.

- [ ] **Step 6: Commit.**
      `feat(ridl-core): name every warning and info code as a lint`

### Task 2: Levels, scopes, and `apply_lint_levels`

**Files:**

- Modify: `crates/ridl-core/src/lint.rs`

**Interfaces:**

- Consumes: Task 1's `LintLevel`, `lint_of`, `default_level`.
- Produces:
  - `pub type LintTable = BTreeMap<&'static str, LintLevel>`. The keys are
    registered lint names; the manifest parser only inserts valid ones.
  - `#[derive(Debug, Clone, Default, PartialEq, Eq)] pub struct LintLevels { overrides: LintTable }`,
    with these methods:
    - `fn overlay(&mut self, table: &LintTable)`, where a later key wins;
    - `fn level(&self, entry: &CatalogEntry) -> LintLevel`, which returns the
      override or `default_level`.
  - `#[derive(Debug, Clone, Default)] pub struct LintScopes { scopes: Vec<(PathBuf, LintLevels)> }`,
    with these methods:
    - `fn insert(&mut self, dir: PathBuf, levels: LintLevels)`;
    - `fn for_path(&self, path: &Path) -> Option<&LintLevels>`, which returns
      the levels of the longest `dir` for which `path.starts_with(dir)` holds,
      compared component by component.
  - `pub fn apply_lint_levels(diagnostics: &mut Vec<Diagnostic>, sources: &SourceMap, scopes: &LintScopes)`.
    It looks up `sources.path(d.primary.file)`. A missing path, a detached file,
    or a path in no scope gets the default levels. The function is idempotent.

- [ ] **Step 1: Write the failing tests** in `lint.rs`:
  - `overlay_later_table_wins`: overlay `{missing-timing: Deny}`, then
    `{missing-timing: Allow}`; `level` is `Allow`.
  - `longest_scope_wins`: scopes `/ws` → Warn, `/ws/a` → Deny for
    `missing-timing`. `/ws/a/x.ridl` gets Deny, `/ws/b/x.ridl` gets Warn, and
    `/other/x.ridl` gets `None`.
  - `scope_match_is_by_component`: `/ws/a` does not match `/ws/ab/x.ridl`.
  - `apply_rewrites_and_removes`: four diagnostics in a file under one scope:
    - RIDL-100, set to Deny there, becomes `Severity::Error`;
    - RIDL-405, set to Allow, is removed;
    - an Error code (`RIDL_101`) is unchanged;
    - `DiagCode::NONE` with Warning is unchanged.
  - `apply_uses_defaults_outside_scopes`: RIDL-100 emitted as `Severity::Error`
    in a file outside every scope is rewritten to Warning.
  - `apply_is_idempotent`: applying twice gives the same vector.

- [ ] **Step 2: Run.** `cargo test -p ridl-core --locked lint::`. Expected: fail
      (items undefined).

- [ ] **Step 3: Implement** the items in Produces. Use `Vec::retain` for
      removal.

- [ ] **Step 4: Run.** `cargo test -p ridl-core --locked`. Expected: pass. Also
      run `just wasm-check`: `lint.rs` must not need the `fs` feature.

- [ ] **Step 5: Commit.**
      `feat(ridl-core): resolve lint levels by directory and apply them`

### Task 3: The `[lints]` table and the loader

**Files:**

- Modify: `crates/ridl-core/src/manifest.rs` (`Manifest` 55, `RawManifest` 180,
  `check_unknown_keys` 235-280, the `warning` helper doc at 322)
- Modify: `crates/ridl-core/src/workspace.rs` (`LoadedWorkspace` 45, `Loader`
  117, `load_root` and the member load near 138 and 194, the `default_timing`
  inheritance at 222)

**Interfaces:**

- Consumes: `LintLevel::parse`, `lint_by_name`, `LintTable`, `LintLevels`,
  `LintScopes`.
- Produces:
  - `Manifest.lints: LintTable`, holding only valid entries.
  - `LoadedWorkspace.lints: LintScopes`. It holds one scope for the root
    directory (defaults overlaid with the root table), and one scope per member
    directory (the root levels overlaid with the member table). A standalone
    package gets one scope for its directory. Single-file mode gets none.

- [ ] **Step 1: Write the failing manifest tests** in `manifest.rs`:
  - `lints_table_is_read`: `[lints]` with `missing-timing = "deny"` gives
    `lints["missing-timing"] == Deny`, with no diagnostics. Test this in both a
    `[package]` and a `[workspace]` manifest.
  - `lints_key_is_known`: no MANI-005 is raised for `lints`.
  - `unknown_lint_name`: `nope = "deny"` raises one MANI-010 with the message
    `` unknown lint `nope` in `[lints]` ``, and the entry is not in `lints`.
  - `code_as_key`: `"RIDL-100" = "deny"` raises MANI-010 (an unknown lint).
  - `bad_level`: `missing-timing = "Deny"` and `missing-timing = 3` each raise
    MANI-010 with the message
    `` `[lints].missing-timing` must be one of "allow", "info", "warn", "deny" ``.
  - `lints_not_a_table`: `lints = 1` raises one MANI-010 with the message
    `` `[lints]` must be a table ``, whose primary span covers the text `1`. No
    MANI-001 is raised, and the manifest is returned with its `[package]` name.
  - `mani_010_span_is_the_key`: the primary span of `unknown_lint_name`'s
    diagnostic covers the text `nope`.

- [ ] **Step 2: Run.** `cargo test -p ridl-core --locked manifest::`. Expected:
      fail.

- [ ] **Step 3: Implement.** Do not declare `lints` in `RawManifest`. A typed
      field would make `lints = 1` fail the one typed parse, which is MANI-001
      with no manifest. `RawManifest` has no `deny_unknown_fields`, so serde
      ignores the key. Read the table with a second `toml::from_str` into a
      small struct,
      `struct RawLints { #[serde(default)] lints: Option<BTreeMap<Spanned<String>, Spanned<toml::Value>>> }`,
      so each key span is available for the per-entry MANI-010s. toml 1.x
      deserializes `Spanned` map keys. If that parse fails because `lints` is
      not a table, raise one MANI-010 (`` `[lints]` must be a table ``) at the
      span of the `lints` value, taken from the untyped
      `BTreeMap<String, Spanned<toml::Value>>` that `check_unknown_keys` already
      parses. Add `"lints" => {}` in `check_unknown_keys`. Build each MANI-010
      with the existing `warning()` helper, and update that helper's doc comment
      ("MANI-005 and MANI-010").

- [ ] **Step 4: Write the failing loader test** in `workspace.rs`, named
      `lint_scopes_follow_root_then_member`. Use a temp workspace:
  - The root sets `missing-timing = "deny"` and `shared-error-type = "allow"`.
  - Member `a` sets `missing-timing = "warn"`. Member `b` sets nothing.
  - Assert, through `loaded.lints.for_path` on a file in each member:
    - in `a`, `missing-timing` is Warn and `shared-error-type` is Allow;
    - in `b`, `missing-timing` is Deny;
    - the root `ridl.toml` path gets the root levels;
    - `a/ridl.toml` gets `a`'s levels.

- [ ] **Step 5: Implement** in `Loader`. Add a `workspace_lints: LintLevels`
      field, set when the root manifest is read, following the
      `workspace_default_timing` pattern. Insert a scope per directory into a
      `lints: LintScopes` field, and move it into `LoadedWorkspace`. Update
      every `LoadedWorkspace { .. }` constructor
      (`grep -rn "LoadedWorkspace {"
      crates/`).

- [ ] **Step 6: Run.** `cargo test -p ridl-core --locked --all-features`.
      Expected: pass.

- [ ] **Step 7: Commit.**
      `feat(ridl-core): read the [lints] table into per-directory lint scopes`

### Task 4: Apply the levels in `ridlc`

**Files:**

- Modify: `crates/ridlc/src/lib.rs`:
  - `front_end`;
  - `WorkspaceOutput` and `compile_workspace_with`;
  - `CliRun`;
  - `check_loaded`;
  - `run_check`;
  - `run_build` and `run_build_with`, whose `succeeded` gate is computed before
    it writes any artifact. `materialize_and_lock` runs earlier and writes
    `ridl.lock`, so a denied lint does not stop the lockfile write; the lockfile
    is not an artifact;
  - the `Compiled` struct.
- Modify: `crates/ridlc/src/main.rs` (`Command::Build`) and
  `crates/ridl/src/main.rs` (`Command::Build`, and `run_baseline`), the callers
  of `run_build_with` and `run_build`.
- Modify: `crates/ridlc-gen-model/tests/parity.rs` and
  `crates/ridlc-gen-rust/tests/parity.rs`, which call `run_build_with` directly
  (lines 161 and 170 in each) and pass `ApplyLints::Yes`. No other file calls
  `run_build_with`; every other test calls `run_build`, whose signature does not
  change.
- Create: `crates/ridlc/tests/lint_levels.rs`

**Interfaces:**

- Consumes: `LoadedWorkspace.lints`, `apply_lint_levels`.
- Produces:
  - `Compiled.lints: LintScopes` and `WorkspaceOutput.lints: LintScopes`,
    carrying the loaded scopes. `check_loaded` does not apply them (spec D-8).
  - `CliRun.lints: LintScopes`. It is empty for `check_source`, and carries the
    loaded scopes for `run_check` and `run_build_with`.
  - `pub enum ApplyLints { Yes, No }`, a new last parameter of `run_build_with`.
    `ridl build` and `ridlc build` pass `Yes`. `run_build` keeps its signature
    and passes `Yes`. `ridl baseline` changes its call from `run_build` to
    `run_build_with` with no plugins, the default timeout and `No`, so a lint at
    `deny` does not block a baseline publication.

- [ ] **Step 1: Write the failing tests** in
      `crates/ridlc/tests/lint_levels.rs`. The fixture is a temp workspace with
      one member, whose `.ridl` file declares a `signal` without timing
      (RIDL-100). The tests:
  - `deny_turns_a_lint_into_an_error`: the member sets
    `missing-timing = "deny"`. `ridlc::run_check` gives a RIDL-100 with
    `Severity::Error`, and `has_error()` is true.
  - `allow_removes_a_lint`: with `"allow"`, no RIDL-100 is present.
  - `defaults_normalise_severity`: with no `[lints]`, RIDL-100 is a Warning.
  - `deny_blocks_build`: `ridlc::run_build` with `missing-timing = "deny"` into
    a temp out dir has `has_error()`, and the out dir holds no generated file.
  - `check_source_uses_defaults`: `ridlc::check_source` on the same text gives a
    RIDL-100 Warning.
  - `compile_workspace_keeps_emitted_severities`: with
    `missing-timing =
    "allow"`, `ridlc::compile_workspace` still reports
    RIDL-100 as a Warning, and `WorkspaceOutput.lints` gives `Allow` for the
    member's file.
  - `build_without_levels_writes_artifacts`:
    `run_build_with(...,
    ApplyLints::No)` with `missing-timing = "deny"` has
    no error and writes the IR file.

- [ ] **Step 2: Run.** `cargo test -p ridlc --locked --test lint_levels`.
      Expected: fail.

- [ ] **Step 3: Implement.**
  - In `check_loaded`, move `loaded.lints` into `Compiled.lints`. Do not apply
    it there.
  - In `compile_workspace_with`, move `Compiled.lints` into
    `WorkspaceOutput.lints`, unapplied. `ridl diff`, `ridl test`, `ridl lock`,
    `load_diff_side` and the MCP lookup tools therefore see the emitted
    severities.
  - In `run_check`, apply `Compiled.lints` after `materialize_and_lock`, and
    return the scopes in `CliRun.lints`.
  - In `run_build_with`, when `ApplyLints::Yes`, apply them after the plugin
    resolution and before `succeeded` is computed, so before `write_crate_files`
    and every other artifact write. `materialize_and_lock` has already written
    `ridl.lock`; the lockfile is not an artifact.
  - In `front_end`, which only `check_source` and `compile` call, apply
    `LintScopes::default()` to the result of `check_loaded`.
  - Update every `CliRun { .. }` and `WorkspaceOutput { .. }` constructor, and
    the callers of `run_build_with`.

- [ ] **Step 4: Run.** `cargo test -p ridlc --locked` and
      `cargo test -p ridlc-gen-model -p ridlc-gen-rust --locked`. Expected:
      pass. If an existing insta snapshot changes, check that the only change is
      a lint severity going back to its catalogue default. Review it with
      `cargo insta review` and accept it only in that case.

- [ ] **Step 5: Commit.**
      `feat(ridlc): apply lint levels from [lints] in check and build`

### Task 5: CLI — JSON field, text note, baseline path

**Files:**

- Modify: `crates/ridl-core/src/diag.rs` (`JsonDiagnostic` 1383, `to_json` 1412)
- Modify: `crates/ridl-core/src/diag/render.rs` (`render` at 29)
- Modify: `crates/ridl/src/main.rs`:
  - `run_check`, including its call of `lock::only_lock_orphans` (the function
    is in `crates/ridl/src/lock.rs` and is not changed, because `ridl lock` also
    uses it);
  - the RIDL-407 emission in `desk_check`.
- Create: `crates/ridl/tests/lints.rs`

**Interfaces:**

- Consumes: `lint_of`, `CliRun.lints`, `apply_lint_levels`.
- Produces: `JsonDiagnostic.lint: Option<String>`.

- [ ] **Step 1: Write the failing tests** in `crates/ridl/tests/lints.rs`. Run
      the binary with `Command::new(env!("CARGO_BIN_EXE_ridl"))`, and copy the
      `TempDir` helper from `check_json.rs`. Reuse the RIDL-100 fixture from
      Task 4. The tests:
  - `json_carries_lint_field_and_deny_exits_1`: with `deny`,
    `ridl check --format json` exits 1. The RIDL-100 element has
    `"severity": "error"` and `"lint": "missing-timing"`. An Error-code element
    has no `lint` key.
  - `text_deny_exits_1`: with `deny`, `ridl check` (text) exits 1, and stderr
    contains `error[RIDL-100]` and no `warning[RIDL-100]`.
  - `build_fails_on_deny`: with `deny`, `ridl build --out-dir <tmp>` exits 1,
    and the out dir holds no generated file.
  - `diff_ignores_deny`: two copies of the fixture with
    `missing-timing = "deny"`; `ridl diff <copy1> <copy2>` exits 0 (spec D-8).
  - `baseline_ignores_deny`: with `missing-timing = "deny"`, `ridl baseline`
    exits 0 and writes the snapshot (spec D-8).
  - `allow_is_absent_in_text_and_json`: exit 0, and `RIDL-100` appears in
    neither stderr (text) nor stdout (json).
  - `text_shows_lint_note`: with the default level, stderr contains a line
    exactly equal to the rendered note line in Global Constraints. The fixture
    file must have fewer than ten lines, so the gutter is two spaces wide.
  - `ordinal_changed_allow_silences_baseline`: reuse the baseline fixture setup
    from `crates/ridl/tests/baseline_desk.rs`, with an ordinal change. With
    `ordinal-changed = "allow"`, `ridl check --baseline <b>` has no RIDL-407.
    Without it, RIDL-407 is present.
  - `deny_lint_does_not_skip_desk_check`: the same baseline fixture, plus a
    RIDL-100 set to `deny`. Both RIDL-100 (error) and RIDL-407 are reported, and
    the exit code is 1.

- [ ] **Step 2: Run.** `cargo test -p ridl --locked --test lints`. Expected:
      fail.

- [ ] **Step 3: Implement.**
  - **JSON:** add
    `#[serde(skip_serializing_if = "Option::is_none")] lint: Option<String>` to
    `JsonDiagnostic`, filled from
    `lint_of(d.code).and_then(|entry| entry.lint)`.
  - **Text:** in `render`, append the note line from Global Constraints to a
    diagnostic whose code has a lint name.
  - **Baseline:** in `run_check` in `main.rs`, make the desk-check gate ignore
    diagnostics whose code has a lint name: pass `only_lock_orphans` the
    diagnostics filtered on `lint_of(d.code).is_none()` (collect them into a
    `Vec` at the call site). Do not change `only_lock_orphans` itself, so
    `ridl lock --rename/--retire` is unchanged. After `desk_check` appends its
    diagnostics, call
    `apply_lint_levels(&mut run.diagnostics, &run.sources, &run.lints)`.

- [ ] **Step 4: Run.** `cargo test -p ridl --locked`,
      `cargo test -p ridl-core --locked` and `cargo test -p ridlc --locked`.
      Expected: pass. The ridl-core `render` snapshot holds only Error codes
      (FORM-101 and TYPL-302), so it does not change. The lint note appears in
      the `crates/ridlc/tests/snapshots/corpus__diagnostics@*.snap` files that
      hold Warning or Info codes: `diag-showcase`, `ridl-diag-showcase`,
      `rsdl-appendix-a`, `rsdl-diag-showcase` and `veh-cluster`. Add a new
      `render` unit test in `crates/ridl-core/src/diag/render.rs` with a lint
      Warning that snapshots the note line. Review the snapshots with
      `cargo insta review` and accept only that change. Update `check_json.rs`
      if it pins the exact key set.

- [ ] **Step 5: Commit.**
      `feat(ridl): show lint names in check output and apply levels after the desk check`

### Task 6: SARIF output

**Files:**

- Create: `crates/ridl-core/src/diag/sarif.rs` (declare it in `diag.rs` next to
  `render`)
- Modify: `crates/ridl/src/main.rs` (`CheckFormat` 245, the `--format` doc at
  92-95, `finish_check` 2655)
- Create: `crates/ridl/tests/fixtures/sarif-2.1.0.json`. This is the official
  schema, downloaded from `https://json.schemastore.org/sarif-2.1.0.json`. Add
  it to `.primignore`, so the formatter keeps it byte-exact.
- Modify: `Cargo.toml` (workspace) and `crates/ridl/Cargo.toml` (dev-dependency
  `jsonschema`, `default-features = false`), and `Cargo.lock`
- Test: `crates/ridl/tests/lints.rs`, `crates/ridl-core/src/diag/sarif.rs`

**Interfaces:**

- Consumes: `ALL_CATALOGS`, `lint_of`, `default_level`, `SourceMap`.
- Produces:
  `pub fn to_sarif(diagnostics: &[Diagnostic], sources: &SourceMap, root: &Path, tool_version: &str) -> SarifLog`,
  where `SarifLog: Serialize`. Each artifact URI is the source path with `root`
  stripped, using `/` separators. A path outside `root` stays as it is.

- [ ] **Step 1: Write the failing unit test** in `sarif.rs`, named
      `sarif_shape`. Use one RIDL-100 Warning with a label, and one uncoded
      Error. Snapshot the serialized log with `insta::assert_json_snapshot!`,
      and assert:
  - `runs[0].columnKind == "unicodeCodePoints"`;
  - the rule count equals the total row count of `ALL_CATALOGS`;
  - the RIDL-100 rule has `name: "missing-timing"` and
    `defaultConfiguration.level: "warning"`;
  - the RIDL-100 result has `level: "warning"`, the right `ruleIndex`, and one
    `relatedLocation`;
  - the uncoded result has no `ruleId`;
  - a third diagnostic, a MANI-101 Error whose primary span is
    `FileId::DETACHED`, gives a result with no `locations` key (spec §7.3);
  - no rule has a `helpUri`, and no result has `fixes`.

- [ ] **Step 2: Write the failing CLI test** in `lints.rs`, named
      `sarif_validates_and_denies`. Run `ridl check --format sarif` on Task 4's
      fixture with `deny`. Assert:
  - the exit code is 1;
  - stdout parses as JSON and validates against the vendored schema with
    `jsonschema::validator_for`;
  - the RIDL-100 result has `level: "error"`;
  - with `allow`, the RIDL-100 result is absent.

- [ ] **Step 3: Run both.** Expected: fail.

- [ ] **Step 4: Implement** `to_sarif` and the `Sarif` arm of `finish_check`.
      Print it with `serde_json::to_string_pretty` to stdout, and then return
      `exit_code(&run)`. `root` is the checked path when it is a directory,
      otherwise its parent. `tool_version` is `env!("CARGO_PKG_VERSION")`.
      Update the `--format` doc comment to list `sarif`.

- [ ] **Step 5: Run.** `cargo test -p ridl-core --locked` and
      `cargo test -p ridl --locked --test lints`. Also run `just check` (it
      confirms `.primignore`). Expected: pass.

- [ ] **Step 6: Commit.** `feat(ridl): add ridl check --format sarif`

### Task 7: Language server

**Files:**

- Modify: `crates/ridl-lsp/src/server.rs`:
  - `load()`, about 292-330;
  - `convert_loader_diagnostics`;
  - `analyze` at 606, before `batch(all, &table)` at 691.
- Test: `crates/ridl/tests/servers.rs`. It already drives `ridl lsp` over stdio;
  reuse its helpers.

**Interfaces:**

- Consumes: `LoadedWorkspace.lints`, `apply_lint_levels`.
- Produces: a server field `lints: LintScopes`, replaced on each `load()`.

- [ ] **Step 1: Write the failing tests** in `servers.rs`:
  - `lsp_matches_check_with_lints`: the fixture workspace sets
    `missing-timing = "deny"` and `shared-error-type = "allow"`, and also has an
    unknown `[lints]` key (MANI-010). Collect the `(code, severity)` pairs from
    `ridl check --format json`, and from the language server's
    `publishDiagnostics` for every file after `initialize`. The two sets are
    equal, with `deny` mapping to LSP severity 1.
  - `lsp_new_file_uses_member_levels`: open a file that did not exist at load
    (`didOpen` of a new path under member `a`) that triggers RIDL-100. Its
    severity follows `a`'s `[lints]`.

- [ ] **Step 2: Run.** `cargo test -p ridl --locked --test servers`. Expected:
      fail.

- [ ] **Step 3: Implement.** In `load()`, keep `loaded.lints` on the server, and
      apply the levels to the loader diagnostics before
      `convert_loader_diagnostics`. Build a `SourceMap` view if that function
      needs one; it already receives `&sources`. In `analyze`, call
      `apply_lint_levels(&mut all, <its source map>, &self.lints)` before
      `batch`. Use the source map `analyze` already builds for `table`. If none
      exists, resolve through a `SourceMap` built from `table`'s paths.

- [ ] **Step 4: Run.** `cargo test -p ridl-lsp --locked` and
      `cargo test -p ridl --locked --test servers`. Expected: pass.

- [ ] **Step 5: Commit.**
      `feat(ridl-lsp): apply lint levels to published diagnostics`

### Task 8: MCP server

**Files:**

- Modify: `crates/ridl-mcp/src/lib.rs` (the `ridl_check` handler, its path-mode
  arm after `snapshot`)
- Modify: `crates/ridl-mcp/src/explain.rs` (`ExplainOutput::Diagnostic`,
  `explain`)
- Modify: `crates/ridl-mcp/tests/tools.json` (the `ridl_explain` output schema
  gains two optional fields)
- Create: `crates/ridl-mcp/tests/fixtures/ws-lints/` (a workspace root with
  `[lints]` setting `missing-timing = "deny"`, and one member whose `.ridl` file
  declares a `signal` without timing)

**Interfaces:**

- Consumes: `WorkspaceOutput.lints`, `apply_lint_levels`, `default_level`.
- Produces: `ExplainOutput::Diagnostic` gains
  `#[serde(skip_serializing_if = "Option::is_none")] lint: Option<String>` and
  `#[serde(skip_serializing_if = "Option::is_none")] default_level: Option<String>`.

- [ ] **Step 1: Write the failing tests** in the test modules of the two files,
      beside `path_mode_check_matches_to_json` and `explain_a_diagnostic_code`:
  - `path_mode_check_applies_lint_levels`: `ridl_check` with
    `{"path": <ws-lints>}`. The RIDL-100 element has `"severity": "error"` and
    `"lint": "missing-timing"`, and `workspace.errors` counts it.
  - `explain_a_lint_code`: `ridl_explain` on `RIDL-100` returns
    `lint: "missing-timing"` and `default_level: "warn"`; on `RIDL-101` both
    fields are absent.
  - `path_mode_check_matches_to_json` compares with `ridlc::compile_workspace`,
    which does not apply levels. Its `ws-diag` fixture has no `[lints]`, so the
    only possible difference is a lint severity going back to its catalogue
    default. If the test fails for that reason, apply `output.lints` to the
    expected value in the test as well.

- [ ] **Step 2: Run.** `cargo test -p ridl-mcp --locked`. Expected: fail.

- [ ] **Step 3: Implement.**
  - In the path-mode arm of `ridl_check`, after `snapshot` returns, call
    `apply_lint_levels(&mut snap.output.diagnostics, &snap.output.sources, &snap.output.lints)`
    before `to_json` and before `snap.status()`. Do not apply the levels in
    `snapshot`, which the lookup tools also use (spec D-8).
  - In `explain`, fill `lint` from `entry.lint`, and `default_level` from
    `default_level(entry).as_str()` only when `entry.lint` is present.
  - Replace `tests/tools.json` with the new tool list, as the contract test's
    message says for a change that only adds.

- [ ] **Step 4: Run.** `cargo test -p ridl-mcp --locked`. Expected: pass.

- [ ] **Step 5: Commit.**
      `feat(ridl-mcp): apply lint levels in ridl_check and name lints in ridl_explain`

### Task 9: Book page, CLI reference, ADR amendments

**Files:**

- Create: `docs/book/lints.md`
- Modify: `docs/book/SUMMARY.md` (add `- [Lints](lints.md)` after the CLI
  reference entry, line 7)
- Modify: `docs/book/cli-reference.md` (the `check` section near 167-200, and
  the rendered examples that show a lint diagnostic: the two RIDL-407 warnings
  near 273 and 279, and the TYPL-102 warning near 826)
- Modify: `docs/decisions/ADR-0002-module-system.md` §4 (136-181)
- Modify: `docs/decisions/ADR-0010-cli-conventions.md` decision 1 (59-181)
- Modify: `crates/ridl-sem/src/lint.rs` header (lines 5-9)
- Create: `crates/ridl/tests/book_lints.rs`

**Interfaces:**

- Consumes: `ALL_CATALOGS`, `default_level`.

- [ ] **Step 1: Write the failing test**
      `book_lints.rs::lints_page_matches_catalogue`. It reads
      `docs/book/lints.md` through
      `Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/book/lints.md")`.
      It parses the one Markdown table whose header is
      `| Lint | Code | Default | Summary |`, and asserts that its rows equal the
      `(name, code, default, summary)` set from `ALL_CATALOGS`. The comparison
      is order-free, and the backticks around the name are stripped.

- [ ] **Step 2: Run.** `cargo test -p ridl --locked --test book_lints`.
      Expected: fail (no file).

- [ ] **Step 3: Write the docs.**
  - **`lints.md`** covers: what a lint is (spec D-1); the levels (D-4); the
    `[lints]` table with a `toml` example; resolution root then member (§5.2);
    the commands that apply levels and those that do not (D-8); MANI-010; the
    table; and one sentence under the table saying that setting
    `ordinal-changed` to `allow` makes `ridl check --baseline` silent for
    ordinal changes. It also states the member entry point (D-9): running
    `ridl check` on a workspace member or on a file inside one, opening an
    editor on a member, or passing a member as the MCP `path` loads the member
    as a standalone package, so the root `[lints]` does not apply; check from
    the workspace root to get the root's levels. If it cites the language server
    gap, write "#529 stays open", never a closing keyword before the number.
  - **`cli-reference.md`** gets `--format sarif`, the `lint` JSON field, and a
    link to `lints.md`. In each rendered example near 273, 279 and 826, add the
    note line from Global Constraints after the snippet, with the gutter width
    of that example.
  - **ADR-0002 §4** gets a paragraph on `[lints]` (both manifest kinds, and the
    resolution order). Its `## Status` holds only "Accepted." and no amendment
    line, so add this paragraph after it, in the form ADR-0005 uses ("Amended
    2026-10-03 by the workspace-aware MCP tools design (piece 1a): ..."), with
    the date of the commit: "Amended <date> by the lint foundation design (spec
    0): §4 gains the `[lints]` table, which both manifest kinds accept, and its
    resolution order."
  - **ADR-0010 decision 1** gets one sentence: "In `ridl check`, `ridl build`,
    `ridlc check` and `ridlc build`, a lint raised to `deny` in `[lints]` is a
    diagnostic error, so it exits 1; the other subcommands of both binaries do
    not apply lint levels." Its `## Status` has no amendment line either, so add
    this paragraph after the existing status paragraph, in the same form:
    "Amended <date> by the lint foundation design (spec 0): decision 1 states
    that a lint raised to `deny` exits 1 in `ridl check`, `ridl build`,
    `ridlc check` and `ridlc build`."
  - **`ridl-sem/src/lint.rs`:** replace "There is deliberately no lint driver
    and no configuration surface in E2" with a sentence pointing at
    `ridl_core::lint` and the `[lints]` table.

- [ ] **Step 4: Run.**
      `cargo test -p ridl --locked --test book_lints
      --test book_examples`,
      then `just book-check`, `just link-check`, `just doc-path-check` and
      `just check`. Expected: all pass.

- [ ] **Step 5: Commit** in two commits:
  - `docs: document lints, the [lints] table and SARIF output` (book and the
    `lint.rs` header);
  - `docs(adr): record [lints] in ADR-0002 and deny in ADR-0010`.

### Task 10: Full gate and PR

- [ ] **Step 1:** Run `just verify`. Expected: every member passes.
- [ ] **Step 2:** Open the PR against `main`.
