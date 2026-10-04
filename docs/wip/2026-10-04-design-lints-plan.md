# Design lints and the eval seed — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port three public interface sets into RIDL as a corpus, implement five
design checks, set each check's default level and threshold from labelled corpus
findings, serve the metrics through `ridl_metrics`, and seed ten eval tasks for
piece 1c.

**Architecture:** The checks run as one workspace-level pass at the end of
`ridlc`'s `check_loaded`, over the checked IR, and locate their diagnostics
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

1. **A workspace with no unit types, no interfaces, or one package** (including
   the one-package workspace of `check_source` and MCP source mode): every check
   reports nothing and does not panic. Test in Task 6.
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

Modified: `crates/ridlc/src/lib.rs` (call the pass; `pub mod deps`),
`crates/ridl-mcp/src/refs.rs` and `lib.rs`, `crates/ridl-core/src/diag.rs`,
`docs/book/lints.md`, `xtask/src/main.rs`, `.primignore`,
`THIRD-PARTY-NOTICES.txt`, `.github/workflows/ci.yml`, `.git-std.toml` if a
scope is missing, `crates/ridl-mcp/README.md`,
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
      switch in `evals/README.md` and the guard's budget test.
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
- [ ] **Step 5: Run** `cargo test -p ridl-cli --test eval_corpus` — expect PASS.
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
      is a lint name in `ridl_core::diag::ALL_CATALOGS` or one of the five names
      of §4; `expect.diff` is present only for `evolve`; `prompt.md` and
      `rubric.md` exist and are not empty; every rubric item line starts
      `N. **must**`, `N. **should**` or `N. **must not**`. Also assert there are
      at least 10 tasks and that tasks with `corpus = "vss"` are at most a
      third.
- [ ] **Step 2: Run** it — expect FAIL.
- [ ] **Step 3: Draft the ten tasks** (§8.2): five review tasks, at least one
      per corpus workspace; two evolve tasks; three design tasks from
      requirements paraphrased from upstream public documentation. Review
      rubrics list the real design issues of the workspace and are written from
      the corpus and its upstream only. `expect.lints` stays empty in review
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
  `pub fn package_edges(output: &WorkspaceOutput) -> BTreeMap<String, BTreeSet<String>>`
  — for each workspace package, the workspace packages it depends on, with
  `ridl.std` and the package itself excluded. The edges are exactly those
  `ridl_dependencies` reports today: imports used by references, and rsdl
  component uses (ADR-0025 decision 8).

- [ ] **Step 1: Pin today's behaviour.** Run `cargo test -p ridl-mcp` and record
      that the `ridl_dependencies` tests pass; they are the regression oracle
      and must not change.
- [ ] **Step 2: Write the failing test** in `crates/ridlc/src/deps.rs`:
      `package_edges_matches_imports_and_component_uses`, over a two-package
      workspace where `b` references a type of `a`, plus one rsdl component in
      `c` requiring an interface of `a`: expect `{a: {}, b: {a}, c: {a}}`.
- [ ] **Step 3: Move** the edge logic of `refs::dependencies` (including
      `references_of`, `component_requires` and `system_member_packages`) into
      `deps.rs`; `refs::dependencies` builds `depends_on` from `package_edges`
      and derives `dependents` by inverting it.
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
  `crates/ridl-core/src/diag.rs`, `docs/book/lints.md`

**Interfaces:**

- Produces:
  - `pub(crate) fn run(ctx: &Ctx<'_>) -> Vec<Diagnostic>` in `mod.rs`, where
    `Ctx` holds `&RidlDatabase`, `&[CheckedPackage]`, `&[Resolution]`,
    `&std_ir`, `Option<&System>`, `&mut SourceMap` access for `file_id`, and the
    package edges of Task 5. Each later check adds one
    `fn check(ctx) -> Vec<Diagnostic>` and one line in `run`.
  - `sites.rs`: `pub(crate) struct SiteIndex` built once per run from the AST
    (`parse_file`), with lookups returning `Span`:
    `field(pkg, struct_name, field)`, `member(pkg, iface, member)`,
    `param(pkg, iface, member, param)`, `decl(pkg, name)` (from
    `Resolution.symbols`), `package_line(pkg)` (the `package` line of the first
    file in path order).
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
  - `design_lints_are_silent_without_units_or_interfaces` (Review Focus 1): a
    one-package workspace of plain structs and the `ridlc::check_source` path —
    no diagnostic of the five codes. Use the syntax of
    `docs/specification/typl-language-reference.md` §5.1 for unit types; adjust
    literal syntax to what the parser accepts, not the assertions.
- [ ] **Step 2: Run** `cargo test -p ridlc --test design_lints` — expect FAIL.
- [ ] **Step 3: Add the catalogue row** for `inconsistent-unit` at **Info**
      ("one field name used with different units"), its pair in the `expected`
      set, and its row in `docs/book/lints.md`.
- [ ] **Step 4: Implement** `sites.rs`, `units.rs`, the `inconsistent-unit`
      check (§4.1: group sites by exact name; most frequent unit wins; ties
      report all), `run`, and the call in `check_loaded`. Sites in `ridl.std`
      are skipped.
- [ ] **Step 5: Run** `cargo test -p ridlc -p ridl-core -p ridl-cli` — expect
      PASS (`book_lints` included).
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
    `{pose: a.Pose}` and `{pose: b.Pose}` — no diagnostic.
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
  ordered by their first member's source order; members with no named
  non-`ridl.std` type are left out. Constants
  `LOW_COHESION_MIN_GROUPS: usize = 2`,
  `LOW_COHESION_MIN_GROUP_SIZE: usize = 1`.

- [ ] **Step 1: Write the failing tests**:
  - `cohesion_groups_link_members_that_share_a_type`: members `a(X)`, `b(X, Y)`,
    `c(Z)`, `reset()`; groups `[[a,b],[c]]`.
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

**Interfaces:** Consumes `ridlc::deps::package_edges`. Produces
`PACKAGE_FAN_OUT_MAX: usize = 3`.

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

- Consumes `ridlc::deps::package_edges`, `ridlc::design_lints::cohesion_groups`
  (re-exported from `ridlc` for this use).
- Produces
  `pub fn metrics(snap: &Snapshot, input: &MetricsInput) -> Result<MetricsOutput, ToolError>`;
  `MetricsInput { path }`; the output shape of §6 with camelCase fields:
  `packages: [{name, fanIn, fanOut, instability: Option<f64>, dependsOn}]`,
  `interfaces: [{name, members, groups}]`, `workspace`.

- [ ] **Step 1: Write the failing tests** in the `ridl-mcp` test layout the
      other tools use: `metrics_reports_fan_in_fan_out_and_instability` over the
      `ws` fixture (expected numbers derived from `ridl_dependencies` on the
      same fixture; a package with neither edge has `instability: null`);
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
  of `{workspace, location, message, metric}` where `location` is `path:line`
  relative to the workspace and `metric` is parsed from the message (§4.3 count,
  §4.4 group count and smallest group size, §4.5 count; absent for findings).
- `cargo xtask calibrate derive`: reads `evals/calibration/<lint-name>.toml`
  (§7.3 format) and the review rubrics, and prints, per check and per candidate
  threshold, findings, accepted, precision, and the level and threshold D-5 and
  §11 give; `--write` also writes `evals/calibration/summary.md`.
- xtask keeps no dependency on `ridlc` or `ridl-core`
  (`xtask/tests/oracle_boundary.rs`); it adds `serde_json` and `toml` as normal
  dependencies if they are not present.

- [ ] **Step 1: Write the failing tests** in `xtask/src/calibrate.rs`:
      `metric_is_parsed_from_each_message_form` (one message per §4.3-4.5 form
      from Tasks 8-10);
      `derive_picks_the_least_strict_threshold_meeting_the_bar` (findings with
      metrics 4,5,6,7 accepted/dismissed so that precision is 50 % at >3, 80 %
      at >5: expect threshold 5 and Warning);
      `derive_caps_a_check_with_fewer_than_ten_findings_at_info`;
      `derive_drops_a_check_below_the_floor`.
- [ ] **Step 2: Run** `cargo test -p xtask` — expect FAIL.
- [ ] **Step 3: Implement** both subcommands. A cohesion threshold is a pair
      (groups, size); "least strict" is the pair reporting the most findings,
      ties broken by the lower group count.
- [ ] **Step 4: Run** `cargo test -p xtask` and
      `cargo xtask calibrate dump
  <scratch>` — expect PASS and five JSON
      files.
- [ ] **Step 5: Commit** — `feat(repo): add cargo xtask calibrate`. (Use the
      `xtask` scope instead if `.git-std.toml` lists one.)

### Task 13: Labelling and adjudication

No code. Needs Tasks 4 and 12. The driver must not show one labeller the other's
labels, the spec, or the rubrics.

- [ ] **Step 1: Dump** with `cargo xtask calibrate dump <scratch>/findings`.
- [ ] **Step 2: Claude labels**: a fresh subagent (model: fable), given only the
      findings files, the corpus path, and the question of §7.3, writes
      `<scratch>/claude/<lint>.toml` with `claude` and `claude_reason` per
      finding.
- [ ] **Step 3: Sol labels**: a `codex exec` brief with the same inputs writes
      `<scratch>/sol/<lint>.toml`.
- [ ] **Step 4: Merge** into `evals/calibration/<lint>.toml` (§7.3), setting
      `final` where the two agree.
- [ ] **Step 5: Ask Sebastien** to decide every disagreement and to look at ten
      agreed labels per check (sampled with a fixed seed recorded in the file).
      Wait for the answers; write them into `final`.
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
      `expected` pair, its book row and its code in the same change; its metric
      stays in `ridl_metrics`. Show Sebastien the summary before this step's
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

## Pull requests

Four PRs, each reviewed before merge:

1. **The spec and this plan** (this branch).
2. **Corpus and seed**: Tasks 1 to 4.
3. **Checks and tool**: Tasks 5 to 12. Can be developed in parallel with PR 2;
   nobody writing a rubric runs a check on the corpus.
4. **Calibration**: Tasks 13 and 14, after PRs 2 and 3.
