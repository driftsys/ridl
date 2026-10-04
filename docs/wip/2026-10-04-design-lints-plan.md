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

- [x] **Step 1: Write the failing tests**:
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
- [x] **Step 2: Run** — expect FAIL.
- [x] **Step 3: Add the Info row**, the `expected` pair and the book row.
- [x] **Step 4: Implement** §4.2 over every declared identifier the `SiteIndex`
      can locate (types, fields, enum values, interfaces, members, parameters).
- [x] **Step 5: Run** `cargo test -p ridlc -p ridl-core -p ridl-cli` — expect
      PASS.
- [x] **Step 6: Commit** —
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

These implementation choices were approved by the user on 2026-10-04 and applied
to this plan following that approval. Implementation has not started. They
complete the reviewed interfaces and tests while keeping D-1 to D-9, the
reserved codes, corpus budgets, four PRs and approval stages.

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

## Pull requests

Four PRs, each reviewed before merge:

1. **The spec and this plan** (this branch).
2. **Corpus and seed**: Tasks 1 to 4.
3. **Checks and tool**: Tasks 5 to 12. Can be developed in parallel with PR 2;
   nobody writing a rubric runs a check on the corpus.
4. **Calibration**: Tasks 13 and 14, after PRs 2 and 3.
