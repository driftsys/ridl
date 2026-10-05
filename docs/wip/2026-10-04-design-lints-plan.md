# Design lints and the eval seed — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port three public interface sets into RIDL as a corpus, implement five
design checks, set each check's default level and threshold from labelled corpus
findings, serve the metrics through `ridl_metrics`, and seed ten eval tasks for
piece 1c.

**Architecture:** The checks run as one shared workspace-level pass at the end
of `ridlc`'s `check_loaded` and from the language server's analysis path, over
checked IR with the current database inputs. They locate their diagnostics
through the AST because the IR carries no spans. The corpus and the eval tasks
live under a top-level `evals/`, outside `crates/`. A `cargo xtask calibrate`
command runs the `ridl` binary over the corpus and turns the labels into levels
and thresholds.

**Tech Stack:** Rust (the pinned toolchain), the `ridlc`, `ridl-core`,
`ridl-sem`, `ridl-mcp` and `ridl` crates, `xtask`, TOML, GPT Sol 6.1 through
`codex exec`.

**Spec:**
[`2026-10-04-design-lints-design.md`](2026-10-04-design-lints-design.md). Read
it with this plan; section numbers below (§n) refer to it.

## Global Constraints

- Codes TYPL-406 to TYPL-410 and the record number ADR-0026 are not used (held
  by piece 2a). TYPL-221 is not used (held by the portable-patterns work).
- Corpus budget: 6,000 lines of ported source in total; at most 2,500 lines for
  `ros2`, 2,500 for `mavlink`, 1,000 for `vss` (D-2).
- Nothing outside `evals/` names a domain; the corpus directory names are
  `ros2`, `mavlink` and `vss`.
- Precision bar 80 %, floor 50 % (D-5). At least the bar: Warning. From the
  floor to the bar: Info. Below the floor: not a lint. Fewer than ten findings
  at the chosen threshold: Info at most (§11).
- Every check emits at its catalogue severity; levels are applied only by the
  existing `apply_lint_levels` callers (ADR-0024 decision 6).
- Every lint that ships adds, in the same change: its name on the `diag_codes!`
  row, a row in the table of `docs/book/lints.md`, and a pair in the `expected`
  set of `lint_names_are_present_exactly_on_warnings_and_infos_and_unique`
  (`crates/ridl-core/src/diag.rs`) (ADR-0024 decision 7).
- `ridl_metrics` is read-only and offline, and its addition to
  `crates/ridl-mcp/tests/tools.json` is additive (ADR-0025 decisions 6 and 9).
- No new `ridl` subcommand or flag.
- Prose is plain, literal English. Commits are Conventional Commits with the
  scopes of `.git-std.toml`; a new top-level directory `evals/` takes the `repo`
  scope.
- Types of `ridl.std` are excluded from every check (§4).

## Review Focus

1. **A fixture with no applicable findings**, including a one-package workspace
   of `check_source` and MCP source mode: every check reports nothing and does
   not panic. Absence of units or interfaces, or having one package, does not
   disable other checks: duplicate shapes and abbreviation pairs can occur
   there. Test silence and a positive `check_source` case in Task 6.
2. **A unit type reached through an alias, an `optional`, or an inline scalar**:
   the unit is the resolved unit. An array, map or stream of a unit type is
   skipped. Test in Task 6.
3. **Identifiers with acronyms and digits** (`GPSFix`, `HTTP2Port`,
   `battery_SoC`): the word split is `gps fix`, `http 2 port` → `http port`,
   `battery soc`. Test in Task 7.
4. **Two shapes whose field types have the same simple name in different
   packages** (`a.Pose` and `b.Pose`): they are different types and the shapes
   are not duplicates. Test in Task 8.
5. **A package spread over several files**: `package-fan-out` reports on the
   `package` line of the first file in path order, and the diagnostic is
   reported once. Test in Task 10.

---

## File structure

```text
evals/                                  new, Tasks 1-4, 12-14
  README.md
  corpus/{ros2,mavlink,vss}/            ported workspaces + PROVENANCE.md + LICENSE
  calibration/<lint>.toml, summary.md, expected-counts.toml
  tasks/<id>/{task.toml,prompt.md,rubric.md}
crates/ridlc/src/deps.rs                new, Task 5: package dependency edges
crates/ridlc/src/design_lints/mod.rs    new, Task 6: the pass, run from check_loaded
crates/ridlc/src/design_lints/sites.rs  new, Task 6: AST index of spans
crates/ridlc/src/design_lints/units.rs  new, Task 6
crates/ridlc/src/design_lints/words.rs  new, Task 7
crates/ridlc/src/design_lints/shapes.rs new, Task 8
crates/ridlc/src/design_lints/cohesion.rs new, Task 9
crates/ridlc/src/design_lints/fan_out.rs new, Task 10
crates/ridlc/tests/design_lints.rs      new, Tasks 6-10
crates/ridl-mcp/src/metrics.rs          new, Task 11
crates/ridl/tests/eval_corpus.rs        new, Tasks 1, 4, 14
xtask/src/calibrate.rs                  new, Task 12
```

Modified: `crates/ridlc/src/lib.rs` (call and export the shared pass;
`pub mod deps`), `crates/ridl-lsp/src/server.rs`, `crates/ridl-lsp/Cargo.toml`,
`crates/ridl-lsp/tests/server.rs`, `crates/ridl-mcp/src/refs.rs` and `lib.rs`,
`crates/ridl-core/src/diag.rs`, `docs/book/lints.md`, `xtask/src/main.rs`,
`.primignore`, `THIRD-PARTY-NOTICES.txt`, `.github/workflows/ci.yml`,
`.git-std.toml` if a scope is missing, `crates/ridl-mcp/README.md`,
`docs/design/mcp-workspace-tools.md`.

## Driving Sol

Tasks 1 to 4 and 13 hand work to GPT Sol 6.1. The driver (the session executing
this plan) runs it so:

```bash
codex exec -m gpt-6.1-sol -C <worktree> -s workspace-write - < <brief-file>
```

The driver first clones each upstream at the pinned revision into the scratchpad
(`git clone --filter=blob:none` then `git checkout <rev>`), because the sandbox
may have no network; the brief names that local path. Every brief lives in the
scratchpad, is self-contained, and ends with the exact files Sol must write. The
driver reviews Sol's output before any commit; Sol never commits.

---

### Task 1: The `evals/` scaffold, the corpus guard, and the `ros2` port

**Files:**

- Create: `evals/README.md`, `evals/corpus/ros2/**`
- Create: `crates/ridl/tests/eval_corpus.rs`
- Modify: `.primignore`, `THIRD-PARTY-NOTICES.txt`, `.github/workflows/ci.yml`

**Interfaces:**

- Produces: `evals/corpus/<set>/` as a `ridl check`-able workspace with
  `PROVENANCE.md` and `LICENSE`; the test helpers
  `corpus_dirs() -> Vec<PathBuf>` and `source_lines(dir: &Path) -> usize`
  (counts lines of `.typl`, `.ridl`, `.rsdl` files) in `eval_corpus.rs`, reused
  by Tasks 2, 3, 4 and 14.

- [ ] **Step 1: Write the failing tests** in `crates/ridl/tests/eval_corpus.rs`,
      with the corpus root at `env!("CARGO_MANIFEST_DIR")/../../evals/corpus`:
  - `every_corpus_workspace_checks_without_an_error`: for each directory of
    `corpus_dirs()`, run `ridl check --format json <dir>` (the binary through
    `env!("CARGO_BIN_EXE_ridl")`, as `crates/ridl/tests/lints.rs` does), parse
    the array, assert no element has `"severity": "error"`; assert
    `corpus_dirs()` is not empty.
  - `the_corpus_stays_inside_its_budget`: assert `source_lines` of `ros2` ≤
    2,500, of `mavlink` ≤ 2,500, of `vss` ≤ 1,000 (each only if the directory
    exists), and the sum ≤ 6,000.
  - `every_corpus_workspace_records_its_provenance`: each directory has a
    non-empty `PROVENANCE.md` and `LICENSE`, and `PROVENANCE.md` contains the
    lines starting `Upstream:`, `Revision:`, `Licence:` and `Kind rule:`.
    Directory-specific checks are conditional only while Tasks 1–3 add ports;
    Task 3 makes the expected-set assertion unconditional.
- [ ] **Step 2: Run** `cargo test -p ridl-cli --test eval_corpus` — expect FAIL
      (no corpus directory).
- [ ] **Step 3: Port `ros2` through Sol.** Verify the licences of
      `ros2/common_interfaces` and `ros-navigation/navigation2` first (expected
      Apache-2.0); stop and report to Sebastien if either is not. The brief is
      §3.2's `ros2` row and §3.3 rules 1 to 7 verbatim, plus: the
      `PROVENANCE.md` header lines the test reads, and "write only under
      `evals/corpus/ros2/`".
- [ ] **Step 4: Review the port** as §3.4 says: every declaration of the subset
      present; 20 declarations compared field by field with upstream; every
      listed deviation justified. Send findings back to Sol until the review
      passes.
- [ ] **Step 5: Write `evals/README.md`**: what `evals/` holds, the corpus rules
      (link to the spec's §3.3), the task format (§8.1), and the calibration
      procedure (§7) in one paragraph each.
- [ ] **Step 6: Add** `evals/corpus/*/LICENSE` to `.primignore`; add one entry
      for the `ros2` port to `THIRD-PARTY-NOTICES.txt` (upstream projects,
      licence, path); add `evals/**` to the path filter that decides whether the
      Rust jobs run in `.github/workflows/ci.yml` (#675), so a corpus change
      runs the guard.
- [ ] **Step 7: Run** `cargo test -p ridl-cli --test eval_corpus` — expect PASS;
      `just check` and `just link-check` — expect PASS.
- [ ] **Step 8: Commit** —
      `feat(repo): add the evals corpus with the ROS 2 port`.

### Task 2: The `mavlink` port

**Files:** Create `evals/corpus/mavlink/**`; modify `THIRD-PARTY-NOTICES.txt`.

- [ ] **Step 1: Verify the licence** of `mavlink/mavlink`'s
      `message_definitions/v1.0/common.xml`. If it is not MIT, BSD, Apache-2.0
      or MPL-2.0, port the AOSP sensors and power HAL AIDL interfaces into
      `evals/corpus/aosp-hal/` instead, with the same budget, and record the
      switch in `evals/README.md`, the guard's budget and expected-directory
      tests, task corpus references, provenance and calibration workspace keys.
      The replacement takes the original set's place and 2,500-line budget; it
      does not become a fourth corpus set.
- [ ] **Step 2: Port through Sol** with the `mavlink` row of §3.2 and §3.3.
- [ ] **Step 3: Review** as in Task 1 Step 4.
- [ ] **Step 4: Add** the `THIRD-PARTY-NOTICES.txt` entry.
- [ ] **Step 5: Run** `cargo test -p ridl-cli --test eval_corpus` — expect PASS.
- [ ] **Step 6: Commit** —
      `feat(repo): add the MAVLink port to the evals corpus`.

### Task 3: The `vss` port

**Files:** Create `evals/corpus/vss/**`; modify `THIRD-PARTY-NOTICES.txt`.

- [ ] **Step 1: Verify the licence** (expected MPL-2.0).
      `evals/corpus/vss/LICENSE` is the MPL-2.0 text, and `PROVENANCE.md` states
      that the files under `evals/corpus/vss/` are MPL-2.0.
- [ ] **Step 2: Port through Sol**: `Vehicle.Cabin.HVAC` first;
      `Vehicle.Powertrain.TractionBattery` only while the total stays ≤ 1,000
      lines.
- [ ] **Step 3: Review** as in Task 1 Step 4.
- [ ] **Step 4: Add** the `THIRD-PARTY-NOTICES.txt` entry.
- [ ] **Step 5: Complete the corpus guard**: add an unconditional assertion that
      `corpus_dirs()` contains exactly the three selected directories, using the
      documented Task 2 replacement if needed. Removing any selected set or
      adding an unexpected one fails the guard. Run
      `cargo test -p ridl-cli --test eval_corpus` — expect PASS.
- [ ] **Step 6: Commit** — `feat(repo): add the VSS port to the evals corpus`.

### Task 4: The eval seed

Must be merged before Task 13 starts (D-9). No candidate check may be run over
the corpus by anyone writing a rubric before this task is committed.

**Files:**

- Create: `evals/tasks/<id>/{task.toml,prompt.md,rubric.md}` × 10
- Modify: `crates/ridl/tests/eval_corpus.rs`

**Interfaces:**

- Produces: the task format of §8.1; ids `review-0001` to `review-0005`,
  `evolve-0001`, `evolve-0002`, `design-0001` to `design-0003`.

- [ ] **Step 1: Write the failing test** `every_eval_task_is_well_formed`: for
      each directory of `evals/tasks/`, `task.toml` parses; `id` equals the
      directory name; `kind` is `review`, `evolve` or `design`; `corpus` is
      present for `review` and `evolve` and names a directory of
      `evals/corpus/`, and is absent for `design`; each name in `expect.lints`
      is a lint name in `ridl_core::diag::ALL_CATALOGS`; review expectations
      stay empty until Task 14, so no candidate-name exemption is needed;
      `expect.diff` is present only for `evolve` and names a `ridl_diff` verdict
      (for example, `compatible`, rather than a change category); `prompt.md`
      and `rubric.md` exist and are not empty; every rubric item line starts
      `N. **must**`, `N. **should**` or `N. **must not**`. Also assert there are
      at least 10 tasks and that tasks with `corpus = "vss"` are at most a
      third.
- [ ] **Step 2: Run** it — expect FAIL.
- [ ] **Step 3: Draft the ten tasks** (§8.2): five review tasks, at least one
      per corpus workspace; two evolve tasks; three design tasks from
      requirements paraphrased from upstream public documentation. Review
      rubrics list the real design issues of the workspace and are written from
      the corpus and its upstream only. Keep item numbers stable after the
      rubric is committed: `<task-id>:<item-number>` supplies the recall item ID
      without changing the authored text. `expect.lints` stays empty in review
      tasks until Task 14.
- [ ] **Step 4: Have Sol review the tasks** independently (a brief with the
      tasks and the corpus; output: one list of objections per task). Resolve or
      record each objection.
- [ ] **Step 5: Ask Sebastien to approve the set.** Wait for the answer.
- [ ] **Step 6: Run** the test — expect PASS. **Commit** —
      `feat(repo): seed the eval tasks for piece 1c`.

### Task 5: Package dependency edges in `ridlc`

**Files:**

- Create: `crates/ridlc/src/deps.rs`
- Modify: `crates/ridlc/src/lib.rs` (`pub mod deps;`),
  `crates/ridl-mcp/src/refs.rs` (`dependencies` calls the new function)

**Interfaces:**

- Produces:
  `pub fn package_edges(checked: &[CheckedPackage], system: Option<&System>) -> BTreeMap<String, BTreeSet<String>>`
  — for each checked workspace package, all dependency targets, including
  external qualifiers, with `ridl.std` and the package itself excluded. The
  edges are exactly those `ridl_dependencies` reports today: imports used by
  references, and rsdl component uses (ADR-0025 decision 8).
  `pub fn workspace_package_edges(edges: &BTreeMap<String, BTreeSet<String>>) -> BTreeMap<String, BTreeSet<String>>`
  filters targets to the graph's workspace-package keys. Fan-in, fan-out,
  instability and metrics `dependsOn` consume this filtered view; MCP
  `ridl_dependencies` consumes the complete graph.

- [ ] **Step 1: Pin today's behaviour.** Run `cargo test -p ridl-mcp` and record
      that the `ridl_dependencies` tests pass; they are the regression oracle
      and must not change.
- [ ] **Step 2: Write the failing test** in `crates/ridlc/src/deps.rs`:
      `package_edges_matches_imports_and_component_uses`, over a two-package
      workspace where `b` references a type of `a`, plus one rsdl component in
      `c` requiring an interface of `a`: expect `{a: {}, b: {a}, c: {a}}`. Add
      an external-qualifier case: complete edges retain `ext` and
      `foreign.deep`, while `workspace_package_edges` removes both and keeps the
      internal target `a`.
- [ ] **Step 3: Extract** the shared edge logic of `refs::dependencies`
      (`references_of`, `component_requires` and `system_member_packages`) into
      `deps.rs`, retaining the reference-detail helpers MCP also uses.
      `refs::dependencies` passes `output.checked` and `output.system`, builds
      `depends_on` from complete `package_edges`, and derives `dependents` by
      inverting it. Do not filter external targets from the existing tool.
- [ ] **Step 4: Run** `cargo test -p ridlc -p ridl-mcp` — expect PASS with the
      `ridl-mcp` tests unchanged.
- [ ] **Step 5: Commit** —
      `refactor(ridlc): compute package dependency edges in ridlc`.

### Task 6: The pass, the site index, and `inconsistent-unit`

**Files:**

- Create: `crates/ridlc/src/design_lints/{mod.rs,sites.rs,units.rs}`,
  `crates/ridlc/tests/design_lints.rs`
- Modify: `crates/ridlc/src/lib.rs` (call the pass at the end of `check_loaded`,
  after `unclaimed_backend_keys`, before `Compiled { .. }`),
  `crates/ridl-lsp/src/server.rs`, `crates/ridl-lsp/Cargo.toml` (use the shared
  `ridlc` pass), `crates/ridl-lsp/tests/server.rs`,
  `crates/ridl-core/src/diag.rs`, `docs/book/lints.md`

**Interfaces:**

- Produces:
  - `pub(crate) fn run(ctx: &Ctx<'_>) -> Vec<Diagnostic>` in `mod.rs`, where
    `Ctx` holds `&RidlDatabase`, `&[CheckedPackage]`, `&[Resolution]`,
    `&std_ir`, `Option<&System>`, `&mut SourceMap` access for `file_id`, and the
    workspace-only package edges of Task 5. Each later check adds one
    `fn check(ctx) -> Vec<Diagnostic>` and one line in `run`. Export a shared
    pass entry point accepting the current database, package inputs, their
    checked IR/resolutions, standard IR, optional system and render source map;
    it constructs the internal `Ctx` and `SiteIndex`. The LSP calls it after
    semantic checks and before applying levels. It passes loaded workspace
    inputs including unsaved changes as one set, then calls it separately for
    each standalone overlay's one-package set. Source IDs and spans use the same
    render map as the existing LSP diagnostics; do not call a disk-loading
    compile entry point from analysis.
  - `sites.rs`: `pub(crate) struct SiteIndex` built once per run from the AST
    (`parse_file`), with lookups returning `Span`:
    `field(pkg, struct_name, field)`, `member(pkg, iface, member)`,
    `param(pkg, iface, member, param)`, `variant(pkg, enum_name, variant)`,
    `decl(pkg, name)` (from `Resolution.symbols`), `package_line(pkg)` (the
    `package` line of the first file in path order).
  - `units.rs`: `fn unit_of(ctx, pkg, &FieldType) -> Option<String>` returning
    the canonical UCUM unit; `named` resolves through `type_def` aliases across
    packages; `optional` unwraps; `inline_scalar` reads its own `Backing`;
    array, map, stream, tuple and primitives return `None`.
  - The code allocation for all five checks, made here once: the first free
    TYPL-2xx numbers after TYPL-221 for `inconsistent-unit`,
    `inconsistent-abbreviation`, `duplicate-shape`, and the first free RIDL-4xx
    numbers after RIDL-413 for `low-cohesion-interface`, `package-fan-out`.
    Before taking them, `gh pr list --state open` and grep their diffs for the
    candidate numbers. Record the five codes in the commit message.

- [ ] **Step 1: Write the failing tests** in
      `crates/ridlc/tests/design_lints.rs` (workspaces written to a temp dir,
      compiled with `ridlc::compile_workspace`, diagnostics filtered by code):
  - `inconsistent_unit_reports_the_minority_site`: package `a` has
    `type Speed : km/h`, `type SpeedMs : m/s`, structs with fields
    `speed: Speed` (twice) and `speed: SpeedMs` (once); expect one diagnostic,
    at the `SpeedMs` field, message
    ``"`speed` uses `m/s` here; elsewhere `speed` uses `km/h`"``, with a label
    at one `km/h` site.
  - `inconsistent_unit_reports_every_site_on_a_tie`: one `km/h`, one `m/s` — two
    diagnostics.
  - `inconsistent_unit_follows_aliases_optional_and_inline_scalars` (Review
    Focus 2): `speed: Alias` where `type Alias = Speed`,
    `speed: optional
    SpeedMs`, `speed: float km/h [0.0..1.0 step 0.1]`
    resolve to their units; `speed: array SpeedMs` is skipped.
  - `inconsistent_unit_covers_parameters_and_signals`: a `command` parameter and
    a `signal` payload named `speed` take part.
  - `design_lints_are_silent_without_applicable_findings` (Review Focus 1): a
    one-package fixture of plain structs with distinct shapes and no
    abbreviation pair, through workspace compile and `ridlc::check_source` — no
    diagnostic of the five codes. This fixture does not justify disabling a
    check because the workspace has one package or no units/interfaces.
  - `check_source_reports_inconsistent_unit`: pass one source containing two
    `speed: Speed` sites and one `speed: SpeedMs` site to `ridlc::check_source`;
    assert the one minority diagnostic, catalogue severity and field span match
    the workspace fixture above. This test fails if source-mode diagnostics are
    dropped. Use the syntax of `docs/specification/typl-language-reference.md`
    §5.1 for unit types; adjust literal syntax to what the parser accepts, not
    the assertions.
  - In `crates/ridl-lsp/tests/server.rs`, add a workspace fixture with the same
    unit conflict: initial published code, span and level match CLI/MCP path
    checks. An unsaved edit that creates or removes the conflict updates the
    published diagnostics; `allow` removes it and `deny` promotes it. Also open
    a conflicting standalone source outside the workspace: it reports the
    source-mode finding at its own URI without affecting workspace counts.
- [ ] **Step 2: Run** `cargo test -p ridlc --test design_lints` — expect FAIL.
- [ ] **Step 3: Add the catalogue row** for `inconsistent-unit` at **Info**
      ("one field name used with different units"), its pair in the `expected`
      set, and its row in `docs/book/lints.md`.
- [ ] **Step 4: Implement** `sites.rs`, `units.rs`, the `inconsistent-unit`
      check (§4.1: group sites by exact name; most frequent unit wins; ties
      report all), `run`, its shared entry point, and calls in `check_loaded`
      and LSP analysis. Sites in `ridl.std` are skipped. Preserve the existing
      entry points' application of lint levels; do not apply them inside the
      shared pass.
- [ ] **Step 5: Run**
      `cargo test -p ridlc -p ridl-core -p ridl-cli -p ridl-lsp -p ridl-mcp` —
      expect PASS (`book_lints` and editor/server parity included).
- [ ] **Step 6: Commit** —
      `feat(ridlc): add the design lint pass and inconsistent-unit`.

### Task 7: `inconsistent-abbreviation`

**Files:** Create `crates/ridlc/src/design_lints/words.rs`; modify `mod.rs`,
`diag.rs`, `docs/book/lints.md`, `crates/ridlc/tests/design_lints.rs`.

**Interfaces:** Produces `pub(crate) fn words(ident: &str) -> Vec<String>`.

- [ ] **Step 1: Write the failing tests**:
  - `words_split_case_digits_and_underscores` (Review Focus 3):
    `words("GPSFix") == ["gps","fix"]`, `words("HTTP2Port") == ["http","port"]`,
    `words("battery_SoC") == ["battery","soc"]`,
    `words("maxSpeed") == ["max","speed"]`. Digits split words and are dropped.
  - `abbreviation_is_reported_where_the_short_form_is_used`: identifiers
    `tempLimit` and `temperature`; one diagnostic at `tempLimit`, message
    ``"`temp` in `tempLimit` abbreviates `temperature`, used in `temperature`"``.
  - `abbreviation_needs_three_letters_and_two_more`: `id`/`identity` and
    `pos`/`post` produce nothing; `pos`/`position` does.
  - `abbreviation_variant_uses_its_own_span`: a variant `Temp` and identifier
    `Temperature` in one package report at the abbreviated variant token, rather
    than its enum declaration; assert the primary byte range.
- [ ] **Step 2: Run** — expect FAIL.
- [ ] **Step 3: Add the Info row**, the `expected` pair and the book row.
- [ ] **Step 4: Implement** §4.2 over every declared identifier the `SiteIndex`
      can locate (types, fields, enum values, interfaces, members, parameters).
- [ ] **Step 5: Run** `cargo test -p ridlc -p ridl-core -p ridl-cli` — expect
      PASS.
- [ ] **Step 6: Commit** —
      `feat(ridlc): add the inconsistent-abbreviation lint`.

### Task 8: `duplicate-shape`

**Files:** Create `shapes.rs`; modify `mod.rs`, `diag.rs`, `docs/book/lints.md`,
the test file.

**Interfaces:** Produces the constants `DUPLICATE_SHAPE_MIN_FIELDS: usize = 2`
and `DUPLICATE_SHAPE_MIN_VARIANTS: usize = 2` (the search start; Task 14 sets
the final values).

- [ ] **Step 1: Write the failing tests**:
  - `duplicate_struct_is_reported_on_the_later_declaration`:
    `a.Point3 {x,y,z: float}` and `b.Vec3 {x,y,z: float}`; one diagnostic at
    `b.Vec3`, message ``"`b.Vec3` has the same 3 fields as `a.Point3`"``, label
    at `a.Point3`.
  - `duplicate_enum_is_reported`: two enums with variants `{Low, High}`; message
    `"... has the same 2 variants as ..."`.
  - `same_simple_type_name_in_two_packages_is_not_a_duplicate` (Review Focus 4):
    two structs with fields `{pose: a.Pose, index: integer}` and
    `{pose: b.Pose, index: integer}` — no diagnostic for either struct. Both are
    at the two-field search-start threshold; declare `a.Pose` and `b.Pose` with
    different valid shapes so the fixture adds no unrelated duplicate.
  - `same_variant_count_with_different_names_is_not_a_duplicate`: enums
    `{Low, High}` and `{Cold, Hot}` both reach the two-variant threshold but
    produce no duplicate diagnostic.
  - `shapes_below_the_threshold_are_not_reported`: two one-field structs — none.
- [ ] **Step 2: Run** — expect FAIL.
- [ ] **Step 3: Add the Info row**, the `expected` pair and the book row.
- [ ] **Step 4: Implement** §4.3: key each struct by its sorted (name, canonical
      qualified type) list, each enum by its sorted variant names; report every
      declaration after the first of a key, ordered by package name then source
      order. The number in the message is the field or variant count.
- [ ] **Step 5: Run** — expect PASS. **Step 6: Commit** —
      `feat(ridlc): add the duplicate-shape lint`.

### Task 9: `low-cohesion-interface`

**Files:** Create `cohesion.rs`; modify `mod.rs`, `diag.rs`,
`docs/book/lints.md`, the test file.

**Interfaces:**

- Produces
  `pub fn cohesion_groups(pkg: &Package, iface: &Interface) -> Vec<Vec<String>>`
  (public: Task 11 uses it), groups of member names, each group sorted, groups
  ordered by their earliest member in source order; members with no named
  non-`ridl.std` type are left out. Constants
  `LOW_COHESION_MIN_GROUPS: usize = 2`,
  `LOW_COHESION_MIN_GROUP_SIZE: usize = 1`.

- [ ] **Step 1: Write the failing tests**:
  - `cohesion_groups_link_members_that_share_a_type`: members `a(X)`, `b(X, Y)`,
    `c(Z)`, `reset()`; groups `[[a,b],[c]]`.
  - `cohesion_groups_follow_first_member_source_order`: members declared in the
    order `z(X)`, `y(X)`, `a(Z)`, `reset()` produce `[[y,z],[a]]`. Members
    within a group are sorted, but group order follows its earliest member in
    source, rather than the lexical order of its sorted first name.
  - `low_cohesion_interface_is_reported_with_its_groups`: message
    ``"interface `I` splits into 2 groups of members that share no type: [a, b], [c]"``,
    at the interface's declaration.
  - `a_cohesive_interface_is_not_reported`.
- [ ] **Step 2: Run** — expect FAIL.
- [ ] **Step 3: Add the Info row**, the `expected` pair and the book row.
- [ ] **Step 4: Implement** §4.4 with a union-find over the types each member
      references directly (payload, parameters, return value, fallible ok and
      err).
- [ ] **Step 5: Run** — expect PASS. **Step 6: Commit** —
      `feat(ridlc): add the low-cohesion-interface lint`.

### Task 10: `package-fan-out`

**Files:** Create `fan_out.rs`; modify `mod.rs`, `diag.rs`,
`docs/book/lints.md`, the test file.

**Interfaces:** Consumes the `ridlc::deps::workspace_package_edges` view of
`package_edges`. Produces `PACKAGE_FAN_OUT_MAX: usize = 3`.

- [ ] **Step 1: Write the failing tests**:
  - `fan_out_above_the_maximum_is_reported`: package `e` depends on `a`, `b`,
    `c`, `d`; one diagnostic, message
    ``"package `e` depends on 4 workspace packages: a, b, c, d"``.
  - `fan_out_is_reported_once_on_the_first_file` (Review Focus 5): `e` spread
    over `e/one.ridl` and `e/two.ridl`; the diagnostic is on the `package` line
    of `e/one.ridl`, and there is exactly one.
  - `fan_out_at_the_maximum_is_not_reported`.
- [ ] **Step 2: Run** — expect FAIL.
- [ ] **Step 3: Add the Info row**, the `expected` pair and the book row.
- [ ] **Step 4: Implement** §4.5.
- [ ] **Step 5: Run** — expect PASS. **Step 6: Commit** —
      `feat(ridlc): add the package-fan-out lint`.

### Task 11: `ridl_metrics`

**Files:** Create `crates/ridl-mcp/src/metrics.rs`; modify
`crates/ridl-mcp/src/lib.rs`, `crates/ridl-mcp/tests/tools.json`,
`crates/ridl-mcp/README.md`, `docs/design/mcp-workspace-tools.md`.

**Interfaces:**

- Consumes `ridlc::deps::workspace_package_edges` applied to the complete
  `package_edges`, `ridlc::design_lints::cohesion_groups` (re-exported from
  `ridlc` for this use).
- Produces
  `pub fn metrics(snap: &Snapshot, input: &MetricsInput) -> Result<MetricsOutput, ToolError>`;
  `MetricsInput { path }`; the output shape of §6 with camelCase fields:
  `packages: [{name, fanIn, fanOut, instability: Option<f64>, dependsOn}]`,
  `interfaces: [{name, members, groups}]`, `workspace`.

- [ ] **Step 1: Write the failing tests** in the `ridl-mcp` test layout the
      other tools use: `metrics_reports_fan_in_fan_out_and_instability` over the
      `ws` fixture (expected numbers derived from `ridl_dependencies` on the
      same fixture after retaining workspace targets only; a package with
      neither edge has `instability: null`);
      `metrics_excludes_external_dependency_targets`: the existing dependency
      tool still lists external qualifiers, while metrics `dependsOn`, fan-in,
      fan-out and instability use workspace edges only;
      `metrics_reports_cohesion_groups` over a fixture with one split interface;
      `metrics_writes_nothing` following the existing read-only test.
- [ ] **Step 2: Run** `cargo test -p ridl-mcp` — expect FAIL.
- [ ] **Step 3: Implement** the tool with the `#[tool(...)]` pattern of
      `ridl_dependencies`; add its line to the server `instructions`, its name
      to `the_tool_list_is_pinned`, and the new entry to `tests/tools.json`
      (copy from the test's printed output; confirm the diff only adds).
- [ ] **Step 4: Document** the tool in the crate README and in
      `docs/design/mcp-workspace-tools.md`.
- [ ] **Step 5: Run** `cargo test -p ridl-mcp` and `just link-check` — expect
      PASS. **Step 6: Commit** — `feat(ridl-mcp): add the ridl_metrics tool`.

### Task 12: `cargo xtask calibrate`

**Files:** Create `xtask/src/calibrate.rs`; modify `xtask/src/main.rs` (usage
string: `cargo xtask <codegen|descriptor-codegen|calibrate>`).

**Interfaces:**

- `cargo xtask calibrate dump <out-dir>`: builds `ridl`
  (`cargo build -p ridl-cli`), copies each `evals/corpus/<set>` to a temp dir,
  appends a `[lints]` table setting the five names to `warn`, runs
  `ridl check --format json`, and writes `<out-dir>/<lint-name>.json`: an array
  of `{id, workspace, location, message, metric}` where `location` is
  `path:line` relative to the workspace. Construct stable IDs from the check
  name, workspace, relative source path, primary byte range and deterministic
  occurrence index; do not include temporary paths or message text. Parse the
  typed `metric` from the message: shape `{kind: "struct" | "enum", count}` from
  `fields`/`variants`; cohesion `{kind: "cohesion", groups, min_group_size}`
  from the declared group count and listed groups; fan-out
  `{kind: "fan-out",
  count}` from the count. Unit and abbreviation records
  have no metric. Fail on missing/malformed metadata or a cohesion count
  inconsistent with its listed groups. Preserve these records unchanged through
  labels and merge.
- `cargo xtask calibrate derive` (alias: `cargo xtask calibrate --derive`):
  reads `evals/calibration/<lint-name>.toml` (§7.3 format), the unchanged review
  rubrics and `evals/calibration/recall.toml` (§7.4). Print, per check and
  candidate threshold, findings, accepted, precision, detected applicable
  issues, total applicable issues, recall (or `not applicable` for zero
  denominator), and the level and threshold D-5 and §11 give. `--write` also
  writes `evals/calibration/summary.md`. Help and README document the canonical
  subcommand and its alias; both accept `--write`. Reject invalid/missing final
  labels, finding IDs, metric coordinates, issue IDs or applicability rows.
- xtask keeps no dependency on `ridlc` or `ridl-core`
  (`xtask/tests/oracle_boundary.rs`); it adds `serde_json` and `toml` as normal
  dependencies if they are not present.

- [ ] **Step 1: Write the failing tests** in `xtask/src/calibrate.rs`:
      `metric_is_parsed_from_each_message_form` (separate struct/enum messages
      with the same count retain distinct kinds; two cohesion messages with the
      same group count but different smallest sizes retain both coordinates;
      also cover fan-out and reject malformed metadata);
      `derive_picks_the_least_strict_threshold_meeting_the_bar`: 16 distinct
      fan-out findings: four dismissals at metric 4, two dismissals at 5, eight
      accepts and two dismissals at 6. Precision is 8/16 = 50 % at >3, 8/12 at
      >4, and 8/10 = 80 % at >5: expect maximum 5 and Warning, with ten
      surviving findings; >6 retains no findings and cannot qualify;
      `derive_caps_a_check_with_fewer_than_ten_findings_at_info`;
      `derive_drops_a_check_below_the_floor`;
      `derive_preserves_both_shape_thresholds_and_cohesion_coordinates`;
      `recall_counts_distinct_applicable_issues`: two findings mapped to one
      issue count once, an applicable issue with no finding stays in the
      denominator, inapplicable issues are excluded, and filtering a finding by
      threshold changes only the numerator; zero applicable issues prints
      `not applicable`; `derive_alias_matches_the_subcommand`.
- [ ] **Step 2: Run** `cargo test -p xtask` — expect FAIL.
- [ ] **Step 3: Implement** both subcommands and the derive alias. Keep struct
      and enum counts separate when searching their two shape-size thresholds;
      both contribute to the single check's precision and level. A cohesion
      threshold is a pair (groups, size); "least strict" is the pair reporting
      the most findings, ties broken by the lower group count, then the lower
      group size. If several shape-threshold pairs retain the same number of
      findings, prefer the lower field threshold, then the lower variant
      threshold. Apply D-5 and the under-ten Info cap to the retained labelled
      sample; zero findings have undefined precision and cannot qualify for a
      shipped level. Recall uses the reviewed mapping of §7.4 and never selects
      or gates a threshold.
- [ ] **Step 4: Run** `cargo test -p xtask` and
      `cargo xtask calibrate dump
  <scratch>` — expect PASS and five JSON
      files.
- [ ] **Step 5: Commit** — `feat(repo): add cargo xtask calibrate`. (Use the
      `xtask` scope instead if `.git-std.toml` lists one.)

### Task 13: Labelling and adjudication

No code. Needs Tasks 4 and 12. The driver must not show one labeller the other's
labels, the spec, the rubrics or the recall mapping. Do not construct the recall
mapping until blind labelling and adjudication are complete.

- [ ] **Step 1: Dump** with `cargo xtask calibrate dump <scratch>/findings`.
- [ ] **Step 2: Claude labels**: a fresh subagent (model: fable), given only the
      findings files, the corpus path, and the question of §7.3, writes
      `<scratch>/claude/<lint>.toml` with `claude` and `claude_reason` per
      finding, copying its ID and typed metric unchanged.
- [ ] **Step 3: Sol labels**: a `codex exec` brief with the same inputs writes
      `<scratch>/sol/<lint>.toml`, preserving the same IDs and metadata.
- [ ] **Step 4: Merge** into `evals/calibration/<lint>.toml` (§7.3), setting
      `final` where the two agree.
- [ ] **Step 5: Ask Sebastien** to decide every disagreement and to look at ten
      agreed labels per check (sampled with a fixed seed recorded in the file).
      Wait for the answers; write them into `final`. If fewer than ten agreed
      findings exist, show all available agreements and record the shortfall.
      After adjudication, create `evals/calibration/recall.toml`: inventory
      rubric issue IDs, deduplicate repeated issues with explicit aliases, and
      record each check's applicability and finding-ID mapping for every
      canonical issue, including unfound issues. Preserve rubric text and record
      reasons for exclusions; review the mapping with the calibration summary in
      Task 14, without another approval gate.
- [ ] **Step 6: Commit** —
      `feat(repo): label the design lint findings on the evals corpus`.

### Task 14: Levels, thresholds, guard, and docs

**Files:** Modify `crates/ridl-core/src/diag.rs`, the threshold constants,
`docs/book/lints.md`, `evals/tasks/review-*/task.toml` (`expect.lints`),
`crates/ridl/tests/eval_corpus.rs`; create `evals/calibration/summary.md`,
`evals/calibration/expected-counts.toml`.

- [ ] **Step 1: Run** `cargo xtask calibrate derive --write`; commit nothing
      yet.
- [ ] **Step 2: Apply** the result: each check's catalogue severity (Warning or
      Info) and threshold constants. A dropped check loses its row, its
      `expected` pair, its book row, its code, its diagnostic emitter and its
      registration in the shared pass in the same change. Remove or update tests
      that filter the deleted code. Keep the metric computation and API used by
      `ridl_metrics`. The final task validator accepts only catalogue lint
      names, with no candidate-name exemption; assert that an unknown or dropped
      name fails validation. Show Sebastien the summary, including recall
      mappings, and obtain the existing summary approval before this step's
      commit.
- [ ] **Step 3: Write the failing test**
      `design_lint_counts_on_the_corpus_are_pinned`: run
      `ridl check --format json` at default levels on each corpus workspace and
      compare, per shipped design lint, the count with `expected-counts.toml`
      (`[<workspace>] <lint-name> = <count>`); on mismatch print the actual
      table and the instruction to update the file deliberately.
- [ ] **Step 4: Run** — expect FAIL; write `expected-counts.toml` from the
      printed table; run — expect PASS.
- [ ] **Step 5: Set `expect.lints`** in each review task to the shipped lints
      whose accepted findings fall in that task's workspace and match a rubric
      item.
- [ ] **Step 6: Document**: in `docs/book/lints.md`, a short section "How the
      design lints' levels were set", linking `evals/calibration/summary.md`; in
      `evals/README.md`, the results.
- [ ] **Step 7: Run** `just build` — expect PASS.
- [ ] **Step 8: Commit** —
      `feat(ridlc): set the design lint levels from the corpus calibration`.

## Decisions taken during execution

The first seven implementation choices were approved by the user on 2026-10-04
and applied to this plan following that approval. They complete the reviewed
interfaces and tests while keeping D-1 to D-9, the reserved codes, corpus
budgets, four PRs and approval stages. Later entries record choices made during
execution.

1. **Preserve the complete graph and derive a workspace-only view** (Tasks 5,
   10–11). External qualifiers remain part of the released MCP output. Metrics
   filter targets using the shared graph's workspace keys. If wrong, dependency
   output would regress or metrics would count packages outside their scope; the
   unchanged dependency tests and exclusion fixture must detect this.
2. **Call the shared pass from LSP analysis using current inputs** (Task 6). The
   editor's semantic-query path does not run `check_loaded`; it must call the
   same pass with its existing source map, loaded workspace and separate
   standalone overlays. If wrong, unsaved buffers, spans or scoped levels would
   differ from CLI/MCP results; parity and edit tests cover this cost.
3. **Keep typed calibration metadata parsed from existing messages** (Task 12).
   Shape kind/count and both cohesion coordinates survive dump, labels and
   merge. This preserves the existing CLI invocation and xtask dependency
   boundary. If wrong, re-derivation would mix shape thresholds or lose the
   group-size threshold; schema and parser tests must fail rather than guess.
4. **Record a reviewed recall join after adjudication** (Tasks 4, 12–14). Stable
   task/item IDs, explicit issue aliases, complete per-check applicability and
   finding IDs define a reproducible denominator without exposing rubrics to
   labellers or changing their text. If wrong, recall would double-count issues
   or omit misses; it remains reported and ungated, and the mapping is
   reviewable with the already required summary approval.
5. **Use deterministic tie breaks and undefined values** (Task 12). For equal
   retained counts, cohesion prefers fewer groups then smaller minimum size;
   shape pairs prefer fewer fields then fewer variants. Zero findings cannot
   qualify by precision; zero applicable issues report `not applicable` recall.
   If wrong, thresholds would be reproducible but selected differently from
   maintainer intent; changing the tie break re-runs derivation over existing
   labels, without new labelling or configurable thresholds.
6. **Keep intermediate guards conditional only while ports are incomplete**
   (Tasks 1–3). Completion pins the exact selected set, including the approved
   licence replacement, and final task validation is catalogue-only. Dropped
   lint emitters are removed while metric APIs remain (Task 14). If wrong,
   corpus deletion or retired lint references could pass unnoticed, or the
   build/tool contract could break; final guard and validator tests detect it.
7. **Use `calibrate derive` as canonical syntax with a `--derive` alias** (Task
   12), and call `compatible` a diff verdict (Task 4). If wrong, help, docs and
   internal callers would disagree; alias parity prevents this with no new
   `ridl` subcommand or flag.

### Corpus and evaluation lane

8. **Pin the ROS 2 port and its complete selected inventory** (Task 1).
   `ros2/common_interfaces` is pinned to
   `d8dde22160f26cf4fd8f1f8dcd819637b1b88405`, `ros-navigation/navigation2` to
   `d7bf2ac06fe778c21c6141eb49b4d3e2c0c82d3a`, and the required dependency
   `ros2/rcl_interfaces` to `99aea442813391cc20344c5b4c79e5191bf7f2c7`. The
   subset includes all 105 definitions in `std_msgs`, `geometry_msgs`,
   `sensor_msgs`, `nav_msgs` and `std_srvs`; ten `nav2_msgs` messages
   (`BehaviorTreeLog`, `BehaviorTreeStatusChange`, `Costmap`, `CostmapMetaData`,
   `Particle`, `ParticleCloud`, `SpeedLimit`, `TrackingFeedback`, `VoxelGrid`,
   `WaypointStatus`); six services (`ClearEntireCostmap`, `GetCostmap`,
   `IsPathValid`, `LoadMap`, `ManageLifecycleNodes`, `SaveMap`); seven actions
   (`AssistedTeleop`, `BackUp`, `FollowPath`, `FollowWaypoints`,
   `NavigateToPose`, `Spin`, `Wait`); and `builtin_interfaces.Time` and
   `Duration` as required dependency types. The 130 definitions contain 1,672
   upstream physical lines, leaving room for syntax translation within the
   2,500-line source budget. The selected package licences are Apache-2.0;
   navigation2's root licence index routes to `nav2_msgs/package.xml`, and no
   selected definition has an override. A complete source-to-output mapping in
   the provenance makes completeness review reproducible. If this subset exceeds
   the budget, change it explicitly before porting further; deleting comments or
   stubbing types would invalidate the evidence.

9. **Retain whitespace within upstream comments** (Task 1). Six comment lines in
   the ROS 2 port retain their upstream trailing spaces: two in
   `builtin_interfaces/messages.typl` and four in `nav_msgs/messages.typl`.
   Porting rule 1 requires verbatim comment preservation; independent Claude
   review confirmed the source comments, and a direct comparison matched all six
   lines after changing only the comment delimiter. `git diff --check` reports
   them and exits 2. The mandated corpus guard, `just check` and
   `just link-check` pass; no build gate is omitted. Removing those spaces would
   change the reviewed upstream text, so they remain part of the corpus.

10. **Freeze the second port and its protocol evidence** (Task 2). The
    definitions input is pinned to `527637cb39cb4e52293bea40441810b53f23ff25`;
    official protocol and licence documentation is pinned to
    `7412790c2a38162a3f31fa1c2fdac9263d65a1d3`. The frozen subset contains 27
    message declarations and 16 required enums, 43 declarations in total, with
    1,130 selected upstream physical lines. One large enum retains five selected
    entries; its other 165 entries are explicitly outside the subset. Every
    other selected enum is complete, including a parameter dependency reached
    through selected enum entries. Original include layers remain separate
    packages, and five interfaces follow documented protocol boundaries. The
    [corpus provenance](../../evals/corpus/mavlink/PROVENANCE.md) records exact
    names, declaration mappings, protocol choices and representation limits.
    Selected XML definitions are MIT; generator code is excluded. The full
    upstream COPYING is retained byte-exactly as licence evidence. Protocol
    documentation is cited without translating its separately licensed prose. If
    the frozen subset cannot fit the 2,500-line budget, change it explicitly
    before continuing; removing comments, stubbing dependencies or inventing
    protocol boundaries would invalidate the evidence. No candidate design check
    runs on the corpus before the Task 4 rubrics are committed.

11. **Complete the independent port review before committing** (Task 2). The
    external reviewer checked all 43 selected declarations and 20 detailed
    samples. Its advisory correction removed one translator-created duplicate
    comment while preserving the single upstream comment, and documented a
    frozen import used only by parameter metadata. The corrected provenance
    records 1,695 physical source lines and all 165 omitted enum entries. A
    second external review checked the correction and the omitted-entry list
    exhaustively. Other port sources and the licence remain byte-identical to
    the previously reviewed files. Upstream whitespace remains verbatim. The
    [corpus provenance](../../evals/corpus/mavlink/PROVENANCE.md) records the
    exact sites and representation choices. If this review boundary is wrong,
    the corpus would require another preservation review.

12. **Freeze the third port before translation** (Task 3). The definitions are
    pinned to `923692329b46bd70cda88137030b662af2765770`. The mandatory selected
    subtree includes its entire include closure and all eight declared station
    instances. Every selected input carries MPL-2.0; the port remains MPL-2.0
    inside this repository. The optional complete second subtree adds 608
    upstream physical lines before translation, so it is excluded under the
    1,000-line budget forecast rather than partially selected or stubbed. The
    complete source-to-output mapping and representation limits are recorded in
    the third port's provenance. If this forecast is too conservative, the cost
    is a smaller third corpus, which may be extended only by an explicit subset
    decision and another independent preservation review. No candidate check
    runs on any actual corpus before the Task 4 rubrics are committed.

13. **Complete the third port's infrastructure and preservation review** (Task
    3). The initial output allowlist omitted the 14 member manifests the
    workspace loader requires. The controller corrected the allowlist to 31
    exact files, preserving the frozen 14 branch packages and source subset. The
    external porter completed that corrected brief. Independent review checked
    all 31 signals, 30 setter commands and 14 interfaces, sampled at least 20
    declarations field by field, and accepted every recorded deviation. The
    controller accepted that review before guard completion. The reviewed source
    contains 759 physical lines; the combined corpus contains 4,845. The guard
    enforces compilation, provenance, exact selected directories and
    physical-line budgets. Field preservation is established by independent
    review, not by a new golden copy of every declaration. If this boundary is
    wrong, another preservation review is required before changing the port.

14. **Draft the seed independently from candidate checks** (Task 4). The ten
    drafts use five review tasks, two documentation-only evolution tasks and
    three distinct design requirements drawn from already pinned public protocol
    documentation. Review item numbers remain stable identifiers for later
    recall joins. Every lint expectation is empty until Task 14; the validator
    accepts catalogue names only and uses diff verdicts rather than change
    categories. Public documentation supplies independently paraphrased factual
    requirements with revision-specific citations only; no documentation prose
    is copied or translated. The complete task set, including design
    requirements, contains one task from the third source set out of ten. The
    validator adds the existing workspace TOML dependency to the CLI test
    target, with the corresponding lockfile dependency entry and no dependency
    version change. Its red run fails because the task directory does not yet
    exist. Formatting may wrap a rubric item, so the validator checks the
    initial line of each item paragraph rather than treating an indented
    continuation as a separate item. If these boundaries are wrong, independent
    review and user approval must revise the drafts before commitment; candidate
    checks remain excluded until that commitment.

15. **Resolve independent seed review against the local public sources** (Task
    4). The reviewer could not fetch revision-specific public pages, so the
    correction review receives exact local pinned documentation paths and the
    licence evidence. The drafts now distinguish an enforced integer range from
    comment-only unit metadata, add separate bit-preservation and
    name-independent selector criteria, remove a review prompt's concern
    checklist, align the discovery prompt with fault suppression and both
    connection states, and cite the source project's documentation licence in
    all design prompts. Original rubric item numbers remain unchanged; new
    independent items are appended. Frozen source remains unchanged; candidate
    output and calibration data are neither changed nor read. If the corrections
    still misstate the sources, the scoped second independent review must
    identify the remaining objections before user approval.

16. **Extend structural evolution coverage after the user's approval to revise**
    (Task 4). The user accepted replacing the second documentation-only
    evolution task with a compatible additive API query while retaining the
    first and the other eight tasks. The replacement appends an index-only
    convenience query using existing payload and scalar types, with a
    nonnegative-index contract, and preserves every existing interaction
    ordinal. An ordinary compiler check on a scratch copy reports no Error; the
    actual JSON diff reports compatible with one interaction_appended change.
    This checks grammar and compatibility against the pre-candidate branch
    without running candidate design checks, metrics or calibration. A scoped
    external review receives the revised task, frozen source, pinned public
    documentation and scratch feasibility evidence. The user's approval
    authorizes this revision; approval of the revised set is still required
    before commitment. If this extension is infeasible or the rubric excludes
    another valid implementation, the scoped review must identify that before
    the revised set is presented for approval.

17. **Approve and commit the complete corrected seed** (Task 4). On 2026-10-04
    the user replied "lgtm" to the refreshed approval packet for all ten tasks,
    including the corrected review and design criteria and the structural second
    evolution task. Independent external reviews of the corrections and the
    structural replacement reported no remaining objections and were accepted.
    This approves commitment of the full set rather than only the earlier
    coverage revision. Rubric item numbers become stable task-id:item-number
    identifiers at commitment and must not be renumbered by later calibration.
    All lint expectations remain empty until Task 14. No candidate design check,
    metric dump, finding, label or calibration data was run or read before this
    commitment; final corpus compilation uses only the pre-candidate branch.

18. **Validate every authored rubric item rather than each paragraph** (Task 4
    review fix, round 1). Fresh review found that paragraph-first-line
    validation could accept adjacent numbered items with an invalid second
    marker or a duplicate item ID. Regression tests reproduce both omissions
    against the old guard. The repaired guard uses Markdown item events and
    source offsets to validate every authored number and requirement marker,
    including adjacent items without blank separators; indented wrapped text
    remains a continuation. Rubrics contain consecutive, unindented, non-nested
    numbered items with the three specified markers. This changes the guard
    only: all approved task files and committed rubric IDs remain unchanged. If
    Markdown parsing hides an item or formatting is rejected incorrectly, the
    adjacent valid, invalid marker, duplicate ID and wrapped continuation
    regressions must detect it.

19. **Exercise catalogue validation with isolated task metadata** (Task 4 review
    fix, round 2). The approved task set intentionally has empty lint
    expectations until Task 14, so its successful validation did not exercise
    catalogue membership or string-entry rejection. Three otherwise valid
    synthetic tasks now pin acceptance of one released catalogue lint and
    rejection of an unknown name and a non-string entry. Controlled mutations
    deleting the validation loop and inverting membership each make two new
    regressions fail; the guard file is restored byte for byte after each
    mutation. No candidate-name exemption or broad malformed-task suite is
    introduced, and all approved task files and stable rubric IDs remain
    unchanged. If catalogue validation is later weakened, these focused
    regressions must fail even while the seed's lint arrays remain empty.

20. **Integrate current main before the corpus PR review** (PR 2 integration).
    Merge main at `112da94d863860091be0542594c5791b039e2594` into the corpus
    branch at `9689d0a4a3ff9fd6966b66fbba6ad6e646c21339`, preserving every
    automatic upstream change. Resolve only the CLI manifest conflict by
    retaining the corpus guard's TOML dev-dependency and upstream's snapshot
    dev-dependency, descriptor runtime dependency and updated description. This
    makes the approved corpus reviewable against current main without rewriting
    its commits or changing the thirty approved task files or corpus sources.
    The lockfile merges automatically and is checked with locked metadata and
    the thirteen corpus tests. If either side's dependencies are lost, the CLI
    or corpus guard can fail to build; static checks and the focused tests must
    detect that before commitment. The integration adds no version bump and runs
    no candidate check, metric dump, finding or calibration procedure.

21. **Bound the existing upstream licence check to its notice section** (PR 2
    integration fix). The full verification run found that the distribution test
    hashed the complete suffix after the upstream licence header, including all
    newly appended corpus notices. The upstream licence bytes and their recorded
    SHA-256 remain unchanged. Extract the licence up to the next generic notice
    separator, or end of file when it is the last notice, preserving its final
    newline. Keep every notice byte and the original hash. A focused regression
    checks that modified and truncated licence text still fails that hash
    comparison; release archive checks continue to compare the complete notices
    file byte for byte. If the separator convention changes, the extraction must
    be revised and reviewed; the exact upstream hash must not be updated to
    include unrelated notices. This change adds no corpus or task edits and runs
    no candidate checks or calibration procedure.

22. **Exercise the licence check when it is the final notice** (PR 2 QUICK
    review fix). The advisory tests review found that both existing fixtures had
    later notice sections, so returning an empty string at end of file escaped
    them. Add one assertion that ends the real notices file immediately after
    its already hash-verified upstream licence, retaining the final newline, and
    compares the extracted text with the same unchanged SHA-256. A scratch
    mutation replacing only the end-of-file fallback with an empty string must
    fail this assertion. The helper, all notice bytes, corpus sources and tasks
    remain unchanged. This fixes one untested branch without another review
    cycle over the QUICK fix or another full gate run.

23. **Preserve the approved seed and exercise diff expectations** (PR 707
    full-review fix wave). Keep the ten approved seed IDs and their five review,
    two evolve and three design kinds, and require a review for every selected
    corpus. Additional valid tasks remain permitted under the original minimum
    count and maximum share rules. The whole-set guard checks these constraints
    after each task passes the existing metadata validator. Focused regressions
    replace a seed review with an extra design, change seed kinds without
    renaming, remove review coverage and accept a future addition. Isolated
    mutations must fail when seed or coverage checks are removed. Diff fixtures
    accept all three recognized evolve verdicts and reject missing or unknown
    verdicts and a verdict on either other kind, with literal failure messages.
    Removing either rejection must fail the corresponding fixtures. A monotonic
    fixture counter prevents concurrent tests from reusing a timestamp-based
    directory. Correct only the provenance aggregate sentence to describe the
    4,845 physical source lines after all three ports; the earlier test handoff
    remains a historical statement. All thirty approved task files, ported
    sources and licence bytes remain unchanged. If the seed identity policy
    changes later, it requires a deliberate guard update; adding tasks alone
    does not. This wave runs focused tests, isolated mutations, Clippy and
    static checks, with no candidate checks, new review, commit or push by the
    implementer. The controller inspects the output before committing, pushing
    and requesting fresh pass 2 and CI.

24. **Run configured commit checks without the shared stash wrapper** (PR 707
    fix commit). The current pre-commit configuration contains only `prim .`;
    commit-msg contains only `git std lint --file {msg}`. Run both exact
    commands before staging the three authorized files, save the checked tree,
    and commit with `GIT_STD_SKIP_HOOKS=1` because the shared fix-mode stash
    wrapper previously failed when another worktree changed the stash stack. No
    other worktree or stash is touched, and no configured check is omitted. If
    the hook configuration changes, the controller must run any added checks
    first.

25. **Test replacement of every approved seed identity** (PR 707 final pass-2
    correction). The second review confirmed that reducing the required evolve
    and design counts leaves the existing fixture tests passing. Keep the
    correct validator unchanged and add an independently enumerated fixture for
    each of the ten approved seed IDs. Replace only its ID with a fresh ID of
    the same kind, retaining all other metadata, the total count, corpus share
    and review coverage. Each replacement must fail with the precise
    missing-seed message, including the later evolve and design IDs. The
    existing future-addition acceptance test remains. Isolated mutations that
    reduce both counts, or the design count alone, must now fail the new test;
    restore and verify each experiment. This final correction adds test
    maintenance for ten fixed identities and requires a deliberate fixture
    update if the approved seed changes. It does not alter tasks or expand the
    validator policy. Under the two-pass review policy, the controller commits
    and pushes the correction and requires fresh CI, without a third review
    pass. Run focused validation tests with the corpus compiler test excluded,
    Clippy and static checks; no corpus checks, rubric authoring, full gate or
    review dispatch occurs in this correction.

### Checks and metrics lane

8. **Task 5: retain the canonical JSON reference walker in `ridlc`** (approved
   ownership extension, 2026-10-04). Promote `serde` from a development
   dependency and add `serde_json` as a workspace dependency of `ridlc`,
   updating its lockfile entry. This preserves the released reference traversal
   instead of introducing an unrelated typed-walker rewrite. The cost is two
   direct library dependencies; both already exist in the workspace dependency
   graph.

9. **Task 6: allocate candidate codes once** (2026-10-04). The catalogue and
   every open pull request diff were checked before allocation. Reserve TYPL-222
   for `inconsistent-unit`, TYPL-223 for `inconsistent-abbreviation`, TYPL-224
   for `duplicate-shape`, RIDL-414 for `low-cohesion-interface`, and RIDL-415
   for `package-fan-out`. Task 6 registers only `inconsistent-unit`, at Info;
   later tasks register their own rows. The allocation respects the separately
   reserved codes. A conflicting allocation would make catalogue identity
   ambiguous; the catalogue uniqueness guard and the pre-allocation audit cover
   that risk.
10. **Task 6: synchronize the language server lockfile entry** (approved
    ownership extension, 2026-10-04). Adding the prescribed `ridlc` dependency
    to `crates/ridl-lsp/Cargo.toml` also adds it to that package's dependency
    list in `Cargo.lock`. No other lockfile entry changes. Omitting this would
    fail the locked dependency gate.
11. **Task 6: test resolved units using the current grammar and IR**
    (2026-10-04). The grammar has import aliases, not `type Alias = Speed`; the
    alias fixture therefore imports a unit type from another package under an
    alias, which the checker canonicalizes. Optionality is the `?` suffix and an
    IR flag; arrays and maps use bracket syntax. The source grammar admits
    inline constrained primitives but no inline unit scalar. A checked-IR
    fixture sets an inline scalar's backing to a unit, then exercises the public
    shared pass and its rendered finding. This covers that IR branch without
    adding grammar or IR variants. If wrong, aliases or inline scalar units
    would be omitted from the finding counts; the positive tests cover both.
12. **Task 6: intern spans while constructing the shared context** (2026-10-04).
    The exported pass accepts mutable access to the caller's source map;
    `SiteIndex` interns source spans before the immutable `Ctx` is passed to
    checks. Checks need no interior mutability or second source map. A
    mismatched source map would report the wrong file or range; shared-map and
    unsaved-overlay tests check both.

13. **Task 6: synchronize the SARIF catalogue snapshot** (approved ownership
    extension, 2026-10-04). The existing `sarif_shape` snapshot lists the entire
    diagnostic catalogue. Registering TYPL-222 adds its rule and shifts the
    later rule indices; only that addition and its derived indices are updated.
    The first covering test run detected the stale snapshot. The silence fixture
    filters by registered candidate lint names, so later candidates join the
    assertion when registered without introducing uncatalogued code literals
    into Rust sources.

14. **Task 6: synchronize the language reference catalogue** (approved ownership
    extension, 2026-10-04). The existing compiler corpus test checks every TYPL
    and RIDL catalogue row against its language reference's §16 table. Add
    TYPL-222 there with the same summary and Info severity, and update the
    family overview's diagnostic index as its footer requires. This is catalogue
    synchronization, with no language-surface change. Later Tasks 7 to 10 must
    update the matching reference table and SARIF snapshot when registering
    their rows; Task 14 must maintain those same gates when changing severities
    or removing candidates.

15. **Task 6 review fixes: cover the compiler and CLI reporting boundary**
    (approved ownership extension, 2026-10-04). Add an integration test to
    `crates/ridl/tests/lints.rs` for TYPL-222 at `info`, `allow` and `deny`. The
    test checks catalogue severity on raw compilation, effective severity from
    the command driver, and the binary's JSON report and exit code. Without this
    boundary check, editor tests could pass while the command line dropped
    findings or failed to apply levels. Existing CLI tests remain unchanged.

16. **Task 7: enumerate identifiers from the existing site index** (approved
    ownership extension, 2026-10-04). Add a read-only `identifiers` method to
    `sites.rs`, returning `IdSite` values with package, qualified identity,
    token name and span. This preserves the lookups and standard-package
    exclusion rather than duplicating AST or IR traversal. Only the token name
    is split into words; package and owner names establish deterministic site
    order. A changed identity or span could misreport the abbreviated site;
    category and exact-span integration tests cover that risk.
17. **Task 7: preserve a final single capital in a word** (2026-10-04). ASCII
    case splitting keeps a final single capital with its preceding word, so
    `SoC` becomes `soc`, as the reviewed test requires. Acronym-to-title
    boundaries split `GPSFix` into `gps` and `fix`; digits and underscores
    separate words and are dropped. This general rule uses no dictionary or
    exception list. The cost is that a final single capital is not a separate
    word; the shared helper's explicit examples pin this interpretation.
18. **Task 7: emit one finding per site and matching word pair** (2026-10-04).
    Repeated occurrences of one word in an identifier are deduplicated. Distinct
    longer matches each produce a finding, in lexical word order, with the
    representative identifier chosen by qualified-site order. This makes
    findings reproducible and preserves every qualifying prefix pair. The cost
    is multiple findings when a short word has multiple expansions; the
    repeated-word and multiple-expansion test covers that behavior.
19. **Task 7: synchronize the catalogue's derived records** (approved ownership
    extension, 2026-10-04). TYPL-223 is registered at provisional Info in the
    catalogue, expected lint set, book table and typl reference table. The SARIF
    snapshot adds only that rule and adjusts its later indices. The family
    overview's existing typl §16.3 and book pointer already covers this row; its
    required sections were checked and need no additional edit. Stale derived
    records would fail the existing catalogue guards.
20. **Task 7: correct the pipeline comments** (approved documentation-only
    extension, 2026-10-04). The semantic workspace module and compiler pipeline
    documentation now distinguish semantic workspace passes from the shared
    design lint pass. Incorrect comments could mislead later integration; this
    correction changes no behavior and does not alter the deferred ADR-0008
    prose.

21. **Task 7: allow observed findings on exact book fences** (approved scope
    extension, 2026-10-04). The covering CLI suite found 47 TYPL-223 diagnostics
    across the existing shared book workspace. Add this code only to the
    affected fence markers in `getting-started.md` (lines 217, 322, 359, 397,
    434, 466, 574, 627, 647, 842, 872, 1013, 1118 and 1230) and `rsdl.md` (line
    25), preserving example identifiers and the harness's bidirectional
    allowance checks. The full book harness verifies that each marker is
    necessary. No global suppression is added. Task 14 must remove these precise
    added allowances if calibration drops the abbreviation lint.

22. **Task 7: synchronize the existing MCP server expectations** (approved scope
    extension, 2026-10-04). The existing server fixture's `readSpeed` query
    abbreviates `Reading`, so the compiler now reports TYPL-223 there. Update
    the exact code list in the structured compile-error diff test, and assert
    the precise code, lint, severity, message and source span in the read-only
    tool test. Preserve the fixture and tree metadata equality, plus every
    existing lookup and diff assertion. Ignoring the additional diagnostic would
    weaken the reporting contract; exact assertions preserve it.

23. **Task 7 review fix: include all named composite children** (2026-10-04).
    The normative §4.2 inventory includes every declared identifier, including
    enumset bits and union arms. The shared source index now records those child
    tokens in a separate map and includes them in identifier enumeration,
    preserving the existing enum-only `variant` lookup and qualified ordering.
    The positive fixture asserts both exact token ranges; its negative fixture
    asserts no findings when no expansion exists. Omitting those categories
    would silently miss legitimate findings; the new test failed before this fix
    and passes afterward. The book harness then observed three newly covered
    findings on the existing `AccessFlags` fence at `getting-started.md:421`.
    Add only its exact TYPL-223 allowance, preserving source and harness
    behavior. Task 14 must remove this allowance too if calibration drops the
    lint.
24. **Task 7 review fix: pin exclusion and representative choice** (2026-10-04).
    Controlled standard-package fixtures test both standard expansion/user
    abbreviation and standard abbreviation/user expansion, alongside a user-only
    positive control. Deleting the standard-package guard makes the test fail.
    Two expansion owners `a.Z.temperatureEarly` and `b.A.temperatureLate` test
    the first qualified representative with reversed source-set input order, an
    exact message and primary range, and the existing empty label list.
    Replacing first-wins insertion with overwrite makes this test fail. This
    adds coverage without changing the label contract.

25. **Task 8: compare canonical nominal type identities** (2026-10-04). Checked
    references already resolve import aliases, but local references can remain
    bare. Qualify every named reference with its owning package, recursively
    through tuples, arrays, maps and streams, before using the existing IR JSON
    serialization as a type key. Optionality and container bounds remain part of
    the key; field ordinals, initial values and docs do not. Named types retain
    nominal identity rather than being expanded into their definitions. Losing
    qualification would merge distinct types from different packages; the
    same-simple-name negative fixture and imported alias positive fixture detect
    that error.
26. **Task 8: preserve source order within sorted packages** (2026-10-04). Sort
    checked packages by name and traverse each package's checked declarations in
    their existing source order, retaining the first shape as the
    representative. All later matches label that first declaration. Compare
    sorted field pairs and sorted variant names, ignoring enum values.
    Reordering declarations by their names would select the wrong
    representative; the `Z`, `A`, `B` enum fixture detects that error.
27. **Task 8: synchronize the catalogue's derived records** (approved ownership
    extension, 2026-10-04). Register TYPL-224 at provisional Info, with both
    search-start thresholds at 2, in the catalogue, expected lint set, book
    table and typl reference table. Add its SARIF rule and adjust only the two
    derived result indices. The family overview's required sections were
    checked; its existing typl §16.3 and book pointer already covers this row.

28. **Task 8: allow the observed duplicate enum on its book fence** (approved
    scope extension, 2026-10-04). The covering CLI suite reports TYPL-224 on
    `veh.powertrain.GearPosition` at `getting-started.md:1039`, matching
    `veh.common.GearPosition`. Add only TYPL-224 to the fence starting at line
    1013, preserving its RIDL-406 and TYPL-223 allowances and all source text.
    The book harness verifies that the allowance is necessary. Task 14 must
    revalidate and remove this precise added allowance if calibration drops
    duplicate-shape or its chosen variant threshold suppresses this finding.

29. **Task 8: synchronize an existing backend test's Task 7 finding** (approved
    test-only scope extension, 2026-10-04). The broad workspace suite found six
    FlatBuffers tests whose shared `cruise_package` helper requires no
    diagnostics. Its source now legitimately reports one TYPL-223 Info on
    `setTarget`. Preserve the source and assert exactly that code, lint
    identity, severity, message, fixture path, byte range 1768..1777, token, and
    empty labels and fixits. All other diagnostics remain rejected, and the
    separate cross-package helper still requires an empty list. The affected
    tests rerun against these assertions. Task 14 must remove this specific
    expectation if calibration drops the abbreviation check, or update its exact
    provisional Info assertion if calibration changes the catalogue severity.

30. **Task 8: synchronize the same finding in backend integration helpers**
    (approved test-only scope extension, 2026-10-04). The resumed workspace
    suite passes the FlatBuffers unit tests and reaches the equivalent
    no-diagnostic assertion in
    `crates/ridl-backend-flatbuffers/tests/corpus.rs`. Its counterpart in
    `crates/ridl-backend-proto/tests/corpus.rs` shares the fixture. Both helpers
    now assert the exact TYPL-223 finding of decision 29 only for `cruise.ridl`;
    all other fixture diagnostics must still be empty. Source and
    generated-output snapshots remain unchanged. Task 14 must synchronize these
    two exact expectations if the check drops or its severity changes.

31. **Task 8: synchronize the remaining shared-fixture model tests** (approved
    bounded test-only scope extension, 2026-10-04). Inspection of every Rust
    reference to `cruise.ridl` found three remaining no-diagnostic helpers:
    `crates/ridl-backend-flatbuffers/tests/model_drift.rs`,
    `crates/ridl-backend-proto/tests/model_drift.rs`, and
    `crates/ridl-backend-ts/tests/model_drift.rs`. Apply decision 29's exact
    known finding only to that fixture, retaining empty diagnostics for every
    other fixture and preserving source, backend production and snapshots.
    Together with decisions 29 and 30 this synchronizes six helper files. Task
    14 must remove these exact expectations if the abbreviation check drops or
    update their provisional Info severity if its catalogue level changes. The
    affected suites and one final workspace run verify the batch.

32. **Task 8 review fix: pin type association and nested nominal identity**
    (2026-10-04). Add a swapped field/type-pair negative and reordered positive,
    plus a negative where differently qualified same-named types have identical
    one-field definitions below the shape threshold. Local/import-alias
    positives cover tuple children, map keys, map values and stream elements.
    The stream fixture uses the existing checked-IR variant because streams are
    interaction-only in source. A metadata fixture changes source field order
    and initial values, sets distinct checked-IR documentation, and asserts all
    three metadata differences while preserving the field/type set. Independent
    name/type sorting, removal of each nested qualification branch, and
    expansion of nominal references into definitions must fail their exact
    targeted tests. These fixtures close the review's three important coverage
    gaps without changing production behavior.
33. **Task 8 review fix: exercise inline pattern references in checked IR**
    (2026-10-04). Inline string scalar fields are forbidden in source, so copy
    two valid named string type definitions using one regex constant into the
    existing inline-scalar IR variant. Set the controlled references explicitly
    to local and canonical qualified forms of that one constant, independently
    of the spelling retained by the checker. This isolates the inline-scalar
    qualification branch, whose removal must fail the positive duplicate
    assertion. No grammar or production code changes. The shared test setup
    accepts an IR amendment after checking, then runs the public pass with the
    same source-indexed sites and render map.

34. **Controller: route the remaining Task 7 MCP expectations to Task 11**
    (2026-10-04). The existing exact code arrays in `ridl-mcp/src/diff.rs` and
    `ridl-mcp/src/lib.rs` still omit the known TYPL-223 finding. Task 11 owns
    their precise synchronization while preserving source, spans and all other
    assertions. Task 9 leaves these files unchanged and does not claim a green
    workspace suite. The final PR still requires `just verify`.
35. **Task 9: expose the shared cohesion metric at the crate boundary**
    (approved ownership extension, 2026-10-04). Re-export `cohesion_groups` from
    `ridlc/src/lib.rs` beside `check_design_lints`, with the prescribed
    `(&Package, &Interface) -> Vec<Vec<String>>` signature over checked IR. Task
    11 can consume it without duplicating grouping or adding hidden resolution
    context. An integration test calls this public API directly.
36. **Task 9: share nominal qualification and preserve source group order**
    (2026-10-04). Move Task 8's unchanged local-name qualification helper into
    the shared module and use it in both checks. Visit references inside
    anonymous containers but never expand named definitions or treat pattern
    constants as types. Exclude exactly the `ridl.std` owner. Union-find roots
    retain the earliest member index, then each group's member names are sorted.
    Missing transitive links, lost qualification or lexical group ordering would
    change the metric; direct, alias and ordering fixtures detect those errors.
    Map-key, map-value and query-parameter traversal each have a mutation check:
    removing one branch fails the public API fixture's hand-written group
    assertion, and restoring it passes.
37. **Task 9: synchronize the candidate's catalogue records** (approved
    ownership extension, 2026-10-04). Register RIDL-414 at provisional Info,
    with search-start thresholds of two groups and one member in the smallest
    group. Add the catalogue pair, book row, ridl reference row and SARIF rule,
    and extend the overview's diagnostic index to ridl §16.4. Task 14 must
    synchronize these records with the selected severity or remove the lint
    emitter and records if calibration drops it, retaining the public metric.

38. **Task 9: synchronize the diagnostic coverage index and observed book
    findings** (approved bounded extension, 2026-10-04). Add RIDL-414 to
    `RIDL_PROFILE_CODES` as `Elsewhere`, pointing at
    `crates/ridlc/tests/design_lints.rs` and its exact positive test. The
    existing showcase remains unchanged. The book harness observed eleven
    RIDL-414 findings. Add only their allowances to `getting-started.md` fence
    starts 322, 450, 466, 499, 673, 842, 952, 1013, 1118 and 1230, and `rsdl.md`
    fence start 25. Preserve every other allowance, example source and the
    bidirectional harness. Task 14 must revalidate these exact markers and
    remove any whose finding disappears after threshold selection or lint
    removal; selected severity must also be synchronized with the catalogue,
    reference, book row, SARIF rule and precise provisional Info test.

39. **Task 9: preserve the baseline tests' exact output contract** (approved
    test-only extension, 2026-10-04). The full CLI suite finds RIDL-414 in the
    unchanged `BASE` and `REORDERED` fixtures of `baseline_desk.rs`.
    `check_without_a_baseline_is_unchanged` and
    `auto_discovery_of_an_empty_baseline_directory_stays_silent` retain their
    commands, success status and empty stdout. Replace their empty stderr
    expectation with the complete single rendered Info note: the code, groups,
    exact fixture path, declaration line and column, token underline and lint
    hint. Any extra diagnostic or baseline report still fails. Task 14 must
    restore empty stderr if the lint drops or its chosen thresholds suppress
    this finding, or update this exact note if its severity changes.

40. **Task 9: synchronize the CLI server fixture's cohesion finding** (approved
    test-only extension, 2026-10-04). In `crates/ridl/tests/servers.rs`, add
    RIDL-414 to the exact compile-error code array and add its complete Info
    diagnostic to the read-only tool test. It names `Status`, five groups and
    `b/b.ridl` line 16, columns 11 to 17, with empty labels and fixes. Preserve
    the existing TYPL-223 expectation, fixture source, tree metadata equality
    and every lookup/diff assertion. Task 14 must remove these additions if the
    lint drops or its thresholds suppress the finding, or synchronize the exact
    severity if its final level changes.

41. **Task 9 review fix: distinguish package exclusion from type exclusion**
    (2026-10-04). Amend the controlled standard-package interface to reference
    two nonstandard nominal types and assert its public metric has exactly two
    singleton groups. The shared diagnostic pass must still exclude that
    package. Removing only its package filter survives: `SiteIndex` also omits
    standard-package spans, so the emitter cannot report that interface. The
    original review's single-filter mutation claim therefore does not identify
    an observable defect. A second fixture retains and connects types owned by
    `ridl.std.extra`, while exact `ridl.std.Duration` references are excluded.
    Replacing exact owner equality with a prefix check fails that group's
    literal assertion. Production is restored byte for byte after mutation
    checks; the committed fix changes only tests and this record.

42. **Task 10: report the shared graph's workspace-only fan-out** (2026-10-04).
    Register RIDL-415 at provisional Info and set `PACKAGE_FAN_OUT_MAX` to the
    search start of three. Iterate the shared ordered workspace graph once,
    count its distinct dependency targets, and report only counts greater than
    three. Use `SiteIndex::package_line` for the first file in path order, even
    when only a later file contains imports. The tests present packages and
    imports in nonlexical order, repeat a target reference, and reference a
    standard type: the exact message still lists four distinct workspace targets
    in lexical order. A separate boundary test retains three targets without a
    finding. The shared graph's existing tests cover external-target filtering
    and component-use edges; this task does not duplicate or change that
    computation.
43. **Task 10: synchronize the candidate's exact diagnostic records** (approved
    bounded extension, 2026-10-04). Add the catalogue pair, book row, ridl
    reference row, SARIF rule and derived rule index, and diagnostic coverage
    index entry for RIDL-415. The overview now names package coupling alongside
    interface design lints at ridl §16.4. Preserve all fixture sources and
    unrelated expectations. Task 14 must synchronize the final level across
    these records and the exact provisional Info assertion, or remove the lint
    records and emitter if calibration drops it. Threshold changes must
    revalidate the exact fan-out fixtures and any later counts or allowances.

44. **Task 10 review fix: pin workspace filtering at the lint consumer**
    (2026-10-04). The original three fan-out fixtures contain no external
    targets, so assigning the complete graph to the shared context survives
    them. Add a controlled shared-pass fixture with three workspace targets and
    the qualified external reference `foreign.deep.Remote`. Its complete graph
    has the literal targets `a`, `b`, `c` and `foreign.deep`; its workspace
    graph has only `a`, `b` and `c`. RIDL-415 must remain absent. Supply the
    external reference in checked IR after checking valid fixture sources,
    because the external package is unavailable for normal resolution. A
    temporary-copy mutation that passes the complete graph to the consumer must
    fail the diagnostic assertion. Actual production files remain byte exact.
    Task 14 must revalidate this threshold fixture during calibration; no corpus
    checks or rubric access are part of this correction.

45. **Execution transport after collaboration thread exhaustion** (2026-10-04).
    Retained completed collaboration threads blocked fresh spawns and followup
    to the original worker. Fresh Codex CLI processes run implementers and
    reviewers with the same briefs or the exact installed specialist developer
    instructions. Review coverage and remote/canonical ledger preflight remain
    explicit prerequisites. If this transport is wrong, the risk is incomplete
    review; it does not authorize skipping a review seat. Root inspects and
    commits this prepared correction, then obtains a fresh scoped review before
    Task 11. There is no repeat QUICK over this QUICK fix.

46. **Recover from the commit hook's stash rejection** (2026-10-04). The
    formatter passed, but the fix-mode hook rejected the commit because its
    stash was no longer at the top of the shared stack. Keep the prepared
    changes and leave other stashes untouched. Run the required formatter and
    commit-message checks directly, then commit the same checked files with
    automatic hooks disabled for that invocation. The fresh scoped reviewer
    checks the resulting correction and this record. If the checks differ from
    the hook's configured commands, this recovery could omit a required check;
    compare the commands before committing.

47. **Task 11: consume the existing public re-export** (2026-10-04).
    `design_lints` is private and `cohesion_groups` is already re-exported at
    `ridlc::cohesion_groups`. Use that public entry point with each declared
    `Package.interfaces` entry, matching the shared cohesion check. Do not
    include service-inline shapes as declared interfaces. Compute package
    metrics from `workspace_package_edges(package_edges(...))`, retaining the
    complete graph in the dependency tool. Sort packages and canonical interface
    names; preserve the shared function's group ordering. A mistaken distinction
    between declared and inline interfaces would change the published metric
    inventory.

48. **Task 11: retain the approved path-only input** (2026-10-04). The approved
    section 6 and Task 11 interface specify `MetricsInput { path }`. Follow that
    interface through the common snapshot loader with no overlays, and document
    this exception to the existing path tools' overlay support. Return the
    common workspace status, including errors and warnings, rather than reducing
    it to the illustrative root and notes fields. Pin the schema by adding only
    the new tool; compare every pre-existing entry for equality. If overlays are
    required later, they can be added as an optional field under ADR-0025
    decision 9.

49. **Task 11: synchronize the authorized MCP diagnostic expectations**
    (2026-10-04). The actual diagnostic sequence in the unchanged fixture is
    TYPL-103, TYPL-011, TYPL-223 and RIDL-414. Update
    `path_mode_check_matches_to_json` and the existing diff test
    `diff_with_a_side_that_does_not_compile_carries_diagnostics` (the brief
    names it `compile_errors_preserve_structured_diagnostics`) to pin the
    complete code, message, severity, lint, span, labels, fixes and ordering.
    Preserve the fixtures, structured-error and side-message assertions. Task 14
    must remove candidate expectations if calibration drops or suppresses those
    findings, or synchronize their exact final severity, message and ordering.

50. **Task 11: report the sandbox verification limit** (2026-10-04). The full
    focused MCP suite reaches
    `snapshot::tests::a_remote_import_is_reported_and_not_fetched`, whose
    loopback `TcpListener::bind` is rejected with `PermissionDenied` and
    `Operation not permitted` by this execution sandbox. Preserve that test
    unchanged, record the unfiltered failure, and run the remaining MCP suite
    with that single test excluded. Root must rerun the unfiltered suite in an
    environment that permits the listener before claiming its offline regression
    passed. This exclusion does not verify the listener-based no-fetch
    assertion.

51. **Task 11: make the no-findings test a single-package fixture**
    (2026-10-04). A source-file path within a package manifest loads that
    package and its subpackages under the existing discovery rule. The copied
    fixture's `a/sub` therefore made the initial one-package assertion fail.
    Remove that directory only in the temporary copy before loading the source
    file; preserve all tracked fixtures and the production loader. The resulting
    assertion pins empty interfaces, zero edges and null instability with no
    diagnostics.

52. **Run configured commit checks without the shared stash operation**
    (2026-10-04). For prepared CLI output, run the configured `prim .` formatter
    and `git std lint --file` check directly before committing. Disable the
    automatic hook invocation only after both commands pass. This avoids the
    stash rejection recorded in decision 46 without omitting a required check.
    Compare the hook configuration each time; if it gains another command, that
    command must also run before this procedure is used.

53. **Task 11 fix round 1: synchronize the CLI inventory and coverage**
    (2026-10-04). Reproduce the stale exact CLI tool-name assertion, then add
    only `ridl_metrics`, preserving the eight original names. Add its
    saved-source call to the end-to-end read-only enumeration, with complete
    package, interface and workspace expectations and text/structured equality.
    Update the three current tool inventories and the MCP design's nine-tool
    test description. Repair that record's pre-existing unused-import
    description to name the already implemented lint. No schema or producer
    behavior changes.

54. **Task 11 fix round 1: verify the QUICK test claims with mutations**
    (2026-10-04). In a source copy under this plan's absolute scratch directory,
    with a separate target directory outside the production cache, all seven
    alleged mutations survive the original metrics tests. Strengthen assertions
    against literal public output and demonstrate that the mutations fail them.
    Independently remove component requirements and system-member contributions
    in the copied producer; the new system-only edge test must fail for each.
    Never mutate the assigned worktree's production sources. Restore each copied
    source and wait for every mutation process before final green validation.

55. **Task 11 fix round 1: use existing fixtures for the public contracts**
    (2026-10-04). Extend the temporary mixed-edge fixture with another referring
    subpackage so fan-in two and fan-out one give instability one third. Extend
    the cohesion fixture with a single connected group and exercise its actual
    handler, including the member-path workspace note. Use the existing
    diagnostic fixture for nonzero error and warning counts, and the timing
    fixture under allow and deny to prove unchanged metrics while `ridl_check`
    applies levels. The full saved-source handler inventory also excludes the
    existing inline service. These protect public behavior without new APIs or
    tracked fixture edits. Task 14 must synchronize literal workspace diagnostic
    counts if final candidate severities change them; edge counts, interface
    groups and membership remain independent of candidate thresholds and levels.

56. **Task 11 fix round 1: keep verification focused** (2026-10-04). Run the
    metrics and CLI server tests and scoped static checks. Preserve the
    loopback-listener test and report any sandbox failure in the unfiltered MCP
    suite for root's unrestricted rerun. No full workspace gate, repeat QUICK,
    reviewer dispatch, merge or commit belongs to this implementer fix round.
    Root inspects and commits the prepared changes and obtains the fresh scoped
    review. The original normative specification and producer contracts remain
    unchanged.

### Main integration (checks and metrics lane, continued)

57. **Separate root Git metadata operations from worker resolution**
    (2026-10-04). The restricted worker cannot write the shared worktree Git
    metadata. Root initialized the exact no-commit merge of
    `d694fba2720fa6c307561d28b790e1fd2ac1e584` into
    `b91965de3d57ddd9c840e8685b419f4add5295a5` in its unrestricted session. This
    worker resolves authorized working-tree files and runs focused checks; root
    inspects and stages those files, supplies the staged tree and metadata
    evidence, and obtains the fresh integration review. No worker Git metadata
    mutation, commit, shared stash operation or reviewer dispatch is authorized.
    If this transport is wrong, unresolved index entries could be mistaken for a
    prepared merge; root must verify the staged tree and exact MERGE_HEAD before
    committing. Sandbox-blocked loopback tests require root's unchanged
    unrestricted rerun.

58. **Preserve both execution lanes and the additive integration contracts**
    (2026-10-04). Use main's exact approved prefix before this execution section
    and retain common decisions 1 to 7 once. Preserve corpus and evaluation
    decisions 8 to 25 and checks and metrics decisions 8 to 56 in separate
    labelled lanes with their original numbers and full text. The lane records
    retain completion evidence when main's prefix has unchecked historical task
    boxes. Preserve the Pull requests section unchanged. Merge the compiler
    manifest additively: retain descriptor runtime support, normal serde and
    serde_json dependencies, inherited insta JSON support and the main
    pulldown-cmark test dependency; remove the redundant development serde
    declaration because the normal dependency already supplies it. Keep the
    automatic lockfile. Retain main's removal of shipped story IDs in the design
    index together with the nine-tool count and metrics category. Mechanical
    comparison confirms the automatically merged MCP schema preserves all eight
    main entries exactly and adds the branch's unchanged metrics entry, so no
    schema correction is needed. If wrong, execution references, dependency
    availability or released tool semantics could regress; byte-level record and
    eval preservation checks, locked metadata, focused tests and static gates
    provide the required evidence. Task12 and candidate corpus dumps remain
    outside this integration.

### Checks and metrics lane continuation

59. **Task 12: use the public CLI JSON contract without compiler dependencies**
    (2026-10-04). Add normal `serde`, `serde_json` and `toml` dependencies to
    xtask, using typed finding and metric records. Build the CLI with the locked
    dependency graph into an output-local target directory, copy each corpus
    workspace into an output-local temporary directory, append the five `warn`
    settings, and remove the copies when the command returns. Refuse an existing
    corpus lint table rather than replacing it. If wrong, a dependency boundary
    or corpus input could change; resolved dependency inspection, the existing
    oracle tests and an unchanged-evals comparison provide evidence.

60. **Task 12: reconstruct stable primary byte ranges from source positions**
    (2026-10-04). The existing JSON diagnostic contract exposes one-based line
    and Unicode character columns rather than byte ranges. Convert both ends
    using the copied UTF-8 source, reject invalid positions and outside paths,
    and assign zero-based occurrence indices in deterministic source order.
    Normalize temporary workspace prefixes in diagnostic messages. If wrong,
    labels could attach to different findings; the UTF-8 range test and two
    byte-identical actual dumps test this transport without changing the CLI.

61. **Task 12: specify the reviewed recall join without creating actual labels**
    (2026-10-04). Document `[[item]]` inventory rows with `issue`, `alias` and
    `excluded` kinds, workspace and reason; aliases name their canonical issue.
    Document one `[[check]]` per check with a complete set of `[[check.issue]]`
    applicability rows, reasons and matching finding IDs. Validate every
    numbered review item, canonical alias ordering, finding workspaces, final
    labels, retained metric metadata and all applicability rows. Actual labels
    and the actual recall mapping remain later work. If wrong, the later mapping
    would fail validation or recall would be miscounted; synthetic input and
    rejection tests pin the format without editing committed rubrics.

62. **Task 12: search only boundaries that change retained findings**
    (2026-10-04). Include search-start values and observed coordinate
    boundaries, retaining independent struct and enum thresholds and both
    cohesion coordinates. Select the highest qualifying level, then the pair
    retaining the most findings, with the approved coordinate tie breaks. Apply
    the under-ten Info cap to the retained sample. Recall counts distinct
    applicable issues independently of labels and never affects selection. If
    wrong, thresholds or levels would differ on the same labels; the
    hand-counted precision fixture, paired searches and recall tests verify the
    procedure.

63. **Task 12: preserve authorized output and complete the actual dump**
    (2026-10-04). Keep build targets, temporary fixtures, command logs, the five
    actual finding arrays and the task report under this plan's scratch
    directory. Validate each array against this worktree's source and repeat the
    dump with new temporary roots. Both actual dumps are byte-identical and the
    tracked eval files remain unchanged. No labels, production thresholds,
    levels or calibration records are written. Root inspects and commits the
    prepared output; this worker creates no reviewers or Git metadata changes.
    If wrong, the evidence could violate its independent review barrier;
    output-path and unchanged-input checks provide the required evidence.

64. **Task 12: preserve the separate integration guard repair** (2026-10-04).
    All calibration and code generation unit tests pass, as do the runnable
    oracle boundary tests and focused clippy. The first complete xtask test run
    failed `every_direct_interfaces_read_is_justified`: the integrated cohesion,
    metrics and design-lint tests were absent from the guard's table. Running
    the unchanged HEAD guard reproduced that failure. A separate lane then
    updated `xtask/tests/shape_walk.rs` during handoff; this worker preserved
    that edit without writing the file. The complete locked package rerun now
    passes nineteen tests, with the generated-crate oracle explicitly ignored
    because it requires demo output. If ownership is confused, the worker could
    claim another lane's repair or revert it; the report distinguishes the six
    worker files, baseline failure evidence and the separate guard change.

65. **Register intentional declared-interface reads in the shape-walk guard**
    (integration compatibility repair). The approved check and metrics scope
    excludes inline service shapes. Register the cohesion helper's one read, the
    metrics tool's one read and seven fixture reads with their precise reasons.
    Keep every existing entry and the exact inventory assertions. A separate
    worker owns only this guard file; the calibration worker preserves its
    change. The focused guards pass, and an isolated additional fixture read
    fails at eight against seven. If the declared-interface scope is wrong, an
    allowance could hide an omitted inline shape; the fresh review must compare
    each reason with approved sections 4.4 and 6.

66. **Task 12 fix round 1: own the executable calibration tests** (2026-10-04).
    Place actual caller tests in `xtask/tests/calibrate_cli.rs`. They invoke the
    built xtask executable against isolated synthetic workspaces and a small
    offline compiler protocol fixture, exercising dispatch, alias and write
    behavior, copying, manifest edits, target selection, refusals, cleanup and
    delayed publication. Calibration reads the caller's current workspace
    directory, as the README's root invocation requires, rather than the path
    embedded when xtask was built. No test-only flag or environment override is
    added to production. This worker owns the new caller test file and preserves
    the other workers' test edits. If the runtime-root choice is wrong, callers
    invoking calibration from a subdirectory must move to the documented root;
    the executable tests pin the explicit root-directory behavior.

67. **Task 12 fix round 1: reject contained destinations before side effects**
    (2026-10-04). Resolve existing path components and symlink ancestors without
    creating directories; handle missing components followed by parent steps
    before checking containment against the canonical corpus directory. Reject
    the corpus itself and every descendant before directory creation or Cargo.
    Require the first cohesion group opening bracket, preserving the existing
    closing-bracket, member and group-count validation. The docs observation is
    the same parser defect and is fixed once. If wrong, a dump could change its
    own source or accept malformed metadata; synthetic helper and executable
    tests assert strict rejection and unchanged source trees.

68. **Task 12 fix round 1: pin the reviewed numerical and validation
    boundaries** (2026-10-04). Use same-line multibyte span starts and
    endpoints, exact 50% precision and nine-finding cap cases,
    recall-independent selection with full, partial and zero recall, matching
    below-start messages, conflicting shape-pair ties, omitted excluded
    inventory items, reversed/cross-workspace aliases and consistent numeric
    occurrence gaps. Cohesion size zero is already rejected by the nonempty
    group parser; the valid one-group message tests its independent group-count
    search start. If wrong, tests could fail at an earlier unrelated guard while
    leaving the reviewed behavior unpinned; the cases supply otherwise valid
    inputs and isolate the relevant guard.

69. **Task 12 fix round 1: preserve the actual finding evidence** (2026-10-04).
    Run mutation probes only on scratch copies of xtask, with isolated targets
    and synthetic inputs, then restore those copied controls. Never mutate live
    source for an experiment, run actual corpus checks again, or change retained
    arrays, rubric text, labels, levels or thresholds. Keep focused tests,
    clippy, formatting, mutation logs and per-finding responses in the plan
    scratch. Root owns review and commits. If wrong, the fixes could invalidate
    independent evidence or overwrite another lane; hash/input comparisons and
    the exact worker file list distinguish this round's changes from concurrent
    edits.

70. **Preserve precise backend diagnostic expectations** (2026-10-04). The
    unchanged shared fixtures legitimately emit the new cohesion Info. Update
    the five affected backend test helpers to assert the complete diagnostic
    inventory, code, level, lint name, message, source path, byte range, source
    slice and empty labels and fixes. Preserve the abbreviation diagnostic and
    reject every unexpected extra diagnostic. Incoming main removed a fixture
    comment, so use byte coordinates from the current unchanged fixture. Replace
    obsolete baseline silence with its exact sole cohesion diagnostic. If wrong,
    assertions could conceal a compiler defect; the independent review must
    check the groups against the source and spec. Both affected backend packages
    passed all 110 tests without fixture edits.

71. **Restart only the Task 12 handoff step** (2026-10-04). The original
    implementer stopped producing output after the final green tests. Interrupt
    only its identified CLI process and resume the same session to write the
    missing reports, using medium effort for this administrative step. Reuse the
    completed test and isolated mutation logs; do not repeat checks or change
    code. If wrong, an incomplete operation could be mistaken for success;
    require exact commands, results and remaining issues in the handoff and
    fresh scoped review before completing the task.

72. **Synchronize the remaining backend fixture assertion** (2026-10-04). Full
    verification exposed the same stale cruise diagnostic expectation in the
    TypeScript model-drift test. Apply the same precise ordered two-Info
    expectation as the independently reviewed backend repair, retaining source
    identity, byte ranges, messages, lint names and empty labels and fixes. Keep
    the fixture and production code unchanged. The affected package passed all
    30 tests. A bounded search found no other stale helper. If wrong, the
    updated assertion could conceal a compiler defect; require a fresh scoped
    review before relying on the full verification result.

73. **Repair all ten confirmed PR 712 findings in one owned wave** (2026-10-04).
    The fresh primary finding and refuter evidence is normative. Shadow
    measurement information isolation was imperfect; preserve its original
    artifact, but do not use it as normative review evidence. Keep the approved
    spec, task text, prior decisions, corpus, labels and original finding arrays
    unchanged. Use only the authorized scratch directory and no other worktree.
    If wrong, the repair could invalidate independent calibration evidence or
    overwrite another contributor's work; compare preserved hashes and the exact
    tracked file list before handoff.

74. **Validate every existing dump publication and build destination before side
    effects** (2026-10-04, F01). Keep the outer containment check and inspect
    each output JSON file and the full existing target tree without following
    links. Reject symlinks and nonregular destinations; on Unix reject hard
    links with aliases outside the validated tree. Cargo-created hard links
    entirely within its target are safe and required for target reuse. Stage
    arrays in the private temporary copy directory and rename them into place.
    Keep the reusable target and the README's corpus isolation contract. If too
    strict, an intentionally linked build cache needs a real directory; if too
    weak, Cargo or publication could overwrite sources. Synthetic executable
    fixtures pin unchanged corpus and rejection before Cargo writes.

75. **Include indented numbered rubric items in the complete recall inventory**
    (2026-10-04, F02). Remove leading whitespace before parsing the unchanged
    numbered item syntax; retain integer, strength, duplicate ID and complete
    inventory/applicability validation. If wrong, omitted items can inflate
    recall or invalid inventories can be accepted. Test indented valid items,
    omitted classifications, omitted applicability and malformed item records.

76. **Pin recall columns, occurrence order and cross-workspace joins directly**
    (2026-10-04, F04-F06). Assert recall numerator, denominator, ratio and the
    exact alias candidate row. Use distinct same-span messages in both compiler
    orders with exact occurrence IDs across temporary roots. Reject an otherwise
    valid, unique finding from another workspace with the exact join error.
    Production calibration ordering and joins remain unchanged. If wrong, the
    tests could pass on precision or an unrelated validation error; isolated
    copied-source mutations check those specific failure paths.

77. **Index each named tuple-field occurrence through supported nested types**
    (2026-10-04, F09). Walk typed tuple-field AST nodes below winning
    definitions and interaction members, retain their owner path and exact
    name-token span, and keep repeated names in separate tuples as separate
    sites. Preserve all existing identifier collections and messages. Tuple
    fields nested inside a stream are not accepted source syntax; cover
    supported tuples, optional nesting, arrays, map values, fixed payloads and
    query returns. If wrong, abbreviation findings would be missing, duplicated
    or attached to an outer declaration; exact span and repeated-name
    regressions pin the inventory.

78. **Exclude standard unit provenance at user-owned sites** (2026-10-04, F10).
    Check the resolved defining package of the named IR type before reading its
    unit. Optionality and import aliases retain that canonical package. A local
    type named Duration remains eligible. Disk loading makes standard names
    implicit and rejects an explicit standard-package import; test alias
    resolution with an otherwise valid controlled standard package in the shared
    pass, and test implicit/qualified standard references through workspace
    compilation. If wrong, standard sites change unit majorities or legitimate
    user findings disappear; the two regressions distinguish both outcomes.

79. **Exercise the real reporting callers without changing working wiring**
    (2026-10-04, F07-F08). Add above-threshold component-requires and
    system-member fan-out assertions through compiler compilation and LSP
    initialization, and a positive unit diagnostic through the public MCP source
    wrapper. Pin URI/path, span, message and level. Both system callers and the
    wrapper already report the required findings, so leave the three
    conditionally owned production files unchanged. If wrong, helper-only
    coverage could conceal a dropped system argument or source diagnostic;
    copied caller mutations must fail.

80. **Explain the book's intentional diagnostic allowances locally**
    (2026-10-04, F03). Explain the retained temp/Temperature abbreviation near
    the vocabulary example and the abbreviation and three disconnected type
    groups near Sampling. Keep the source blocks and allowances unchanged. If
    wrong, the book would allow a finding its prose does not explain; inspect
    the examples and run only their compiled-example harness.

81. **Report the sandbox-denied socket test without changing its assertions**
    (2026-10-04). The full MCP library run passes 87 tests, including the new
    source-wrapper regression; `a_remote_import_is_reported_and_not_fetched`
    fails at binding `127.0.0.1:0` with PermissionDenied. Leave that test intact
    for the primary driver to rerun where the bind is permitted. Focused caller,
    compiler, calibration, executable, book and Clippy checks provide the repair
    evidence. If the failure has another cause, the primary rerun must expose
    it; do not claim a green full MCP library suite or weaken the runtime check.

82. **Ruling: preserve both diagnostic contracts in the main integration**
    (2026-10-05). Keep every incoming documentation change outside the twelve
    conflict regions. Combine the independent LSP and CLI test additions, lint
    catalogue expectations and book rows. Retain the exact ordered design
    diagnostic expectations in backend and CLI fixtures while excluding only the
    incoming `missing-docs` allowance by its registered lint name; do not change
    source fixtures, lint levels or thresholds. Derive SARIF result indices from
    the combined rule list, giving 69 and 155. This preserves the branch's
    design checks and main's documentation behavior without allocating a
    reserved code or investigating the separate feature. If wrong, an unexpected
    design diagnostic or incoming regression could be concealed; exact
    diagnostic inventories, catalogue guards, protocol tests and byte
    preservation comparisons provide evidence. Root owns staging, fresh review,
    unrestricted socket reruns and the final gate.

83. **Ruling: retain the complete metrics warning count after integration**
    (2026-10-05). The first CLI server run fails the exact metrics object in
    `every_tool_leaves_the_tree_unchanged`: the unchanged fixture now reports 22
    incoming documentation warnings, while the previous expectation is zero.
    Update only that literal count in the owned server test. Preserve package
    metrics, interface groups, all other workspace fields, fixture bytes and
    read-only assertions. The separate exact design diagnostic assertion still
    rejects every unexpected non-documentation diagnostic. If wrong, the metrics
    status could report an incorrect warning count; the complete object
    assertion and the focused server rerun must detect it. No producer, lint
    level or threshold changes are justified.

84. **Ruling: synchronize the authorized remaining integration expectations**
    (2026-10-05). The user extended ownership to metrics test expectations and
    documentation-example fence allowances and explanations after the first
    integration exposed four stale metrics assertions and four unallowed book
    diagnostics. Pin the complete metrics objects against observed workspace
    discovery, warning counts and sibling interfaces. Replace the empty
    diagnostic assertion with the exact sole documentation warning, including
    its source span and fix. Preserve every fixture source and the final
    read-only comparison. Add only the observed unit, abbreviation and cohesion
    allowance codes to the three affected whole-file fences, with literal local
    explanations. Keep every source block and all incoming feature content. If
    wrong, an incorrect status or unrelated diagnostic could be accepted;
    complete object and diagnostic inventories, the bidirectional book harness
    and byte-level source preservation checks must expose that error. Production
    behavior, lint levels and thresholds remain unchanged; root owns the full
    gate and unrestricted socket verification.

85. **Ruling: align exact MCP diagnostic comparisons with the combined output**
    (2026-10-05). After the completed integration review, root's full gate and
    both focused reproductions fail two MCP literal-array comparisons. Their
    four original diagnostic objects still match exactly; the output also has 23
    incoming documentation warnings already allowed by the adjacent code
    inventory. Preserve those four objects unchanged. Pin the complete ordered
    27-code inventory and the documentation warnings' default severity, and
    compare the full arrays with the compiler's JSON projection on both paths.
    Apply the existing named fixture allowance only to the separate four-object
    comparison. Keep every message, span, path, label and fix assertion and the
    positive source-wrapper repair. If wrong, an extra diagnostic or incorrect
    ordering could be concealed; full-array parity, the complete code inventory
    and unchanged literal objects must expose it. Only test assertion code is
    changed, with no production, fixture, lint-level or threshold edits. Root
    owns the fresh scoped review and full gate rerun.

86. **Ruling: address both remaining Minor findings** (2026-10-05). Correct the
    scalar vocabulary's factual abbreviation explanation and add a focused
    successful dump regression that retains an open handle to the previous
    regular output file. This pins destination-file replacement, exact output
    and containment without changing production behavior or existing tests.
    Address both findings rather than defer debt because the prose is incorrect
    and in-place publication must fail the regression. If wrong, the added test
    may constrain portable filesystem behavior; scope the retained-handle and
    file-identity assertions to Unix, where replacement of an open file is
    supported. Root owns the post-pass-2 QUICK tests and documentation review;
    no third numbered review is requested.

## Pull requests

Four PRs, each reviewed before merge:

1. **The spec and this plan** (this branch).
2. **Corpus and seed**: Tasks 1 to 4.
3. **Checks and tool**: Tasks 5 to 12. Can be developed in parallel with PR 2;
   nobody writing a rubric runs a check on the corpus.
4. **Calibration**: Tasks 13 and 14, after PRs 2 and 3.
