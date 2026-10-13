# Freeze check and stability policy — implementation plan (lane 0)

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** `just freeze-check` fails when a frozen surface loses a fact that its
baseline had, and ADR-0029 states the stability policy that the check enforces.

**Architecture:** `cargo xtask freeze` writes each surface as a sorted text file
under `freeze/` (one fact per line) and checks two things: the committed file
equals what the current code produces, and every line of the file at the
surface's baseline ref (read with `git show`) is still present. Rust APIs are
checked with `cargo semver-checks` against the baseline ref; the generated
crates are committed under `freeze/generated/` so that their baseline can be
read from git the same way.

**Tech Stack:** Rust (xtask, standalone binary with hand-rolled dispatch),
`git`, `cargo-semver-checks`, `protox` (already a workspace dependency of
`ridl-ir`'s build), the `just` gate (ADR-0009).

**Spec:**
[`2026-10-11-release-0.10-design.md`](../wip/2026-10-11-release-0.10-design.md),
section 4. Lanes:
[`2026-10-11-release-0.10-lanes-plan.md`](../wip/2026-10-11-release-0.10-lanes-plan.md).

## Deviation from the spec (approved with this plan)

Spec 4.2 says the check "produces the dump at HEAD and at the baseline ref".
Producing a dump at an old ref means building the old toolchain. This plan
commits each dump instead and reads the baseline's copy with
`git show <ref>:freeze/<surface>.txt`. The pull request under check cannot edit
a file at an older ref, so the guarantee is the same, and a second check
(committed file equals regenerated file) keeps the committed dump honest. A
consequence: a surface's row is added in a pull request **after** the one that
added its dump file, because the baseline ref must be a commit on `main` that
contains the file.

## Global Constraints

- The gate is one recipe, `freeze-check`, in `build`, in the `expected` list of
  `gate-parity`, and as `- run: just freeze-check` in `.github/workflows/ci.yml`
  (ADR-0009; the three edits are checked by `just gate-parity`).
- A line missing at HEAD that is present at the baseline fails the check. An
  added line passes.
- Not dumped (spec section 3, "Not frozen"): size and budget values, the text of
  a message or a doc, and the unpublished crates `xtask`, `ridlc-gen-model`,
  `ridlc-gen-rust`.
- Frozen minimum: Rust 1.83, edition 2021, for `ridl-rt` and
  `ridl-rt-conformance` (lane W extends this to `ridl-loopback` and
  `ridl-descriptor`).
- Commit scopes come from `.git-std.toml`: use `xtask`, `ci`, `ridl-core`, `adr`
  and `docs`. No new scope.
- Prose is plain and literal. Shipped docs name no story ids
  (`just story-id-check`).

## Review Focus

1. A surface with a row whose baseline ref has no `freeze/<surface>.txt`: the
   check must fail with a message naming the ref and the file, not pass
   silently.
2. A baseline ref that does not exist in the clone (a shallow CI checkout): the
   check must fail and say to fetch it; CI's checkout must fetch enough history
   (`fetch-depth: 0`).
3. A dump that changes with the machine (path separators, `HashMap` order,
   absolute paths, the host's `cargo` version): every dumper sorts its lines and
   writes paths relative to the repository root with `/`.
4. A duplicate line in a dump (two keywords with the same text under one
   profile): comparison is by set, and the dumper deduplicates, so a removed
   duplicate is not reported as a break.
5. A gate that is on but whose dumper was deleted: a row in
   `freeze/surfaces.toml` whose surface name has no dumper is an error.

Each item has a test in Task 2 or Task 3.

---

## Before Task 1: release 0.9.0

This is the maintainer's release procedure (no pull request), not a reviewed
task. On `main` at `bc58189e` or later:

1. `git std bump` (`just release`). It must produce 0.9.0 (#851 is breaking); if
   it does not, use `--release-as 0.9.0`.
2. In the same release commit, bump: `crates/ridl-sem/Cargo.toml`'s `ridl-core`
   literal; `RIDL_RT_REQUIREMENT` in `crates/ridlc/src/lib.rs` (`"0.8"` →
   `"0.9"`); `examples/cabin/consumer/Cargo.toml`'s `ridl-rt` requirement and
   the comment in `examples/cabin/Cargo.toml`; regenerate
   `examples/cabin/generated` with the new `ridl`; run
   `cargo update -w --offline` at the root and in `examples/cabin`.
3. `just build` passes.
4. Hand Sebastien `git push --follow-tags`. The editor extension skips 0.9.0
   unless he asks for it.
5. Post the 0.9.0 upgrade note to the xflux consumers (what to add to
   `ridl.toml`: `[backend.rust] encodings = [...]`).

### Task 1: Prove that cargo-semver-checks works with the pinned toolchain

**Files:**

- Modify: `bootstrap` (add `cargo-semver-checks` to the `require` list with an
  install hint), `.github/workflows/ci.yml` (install step in the gate job)
- Create: `docs/archive/freeze-check-semver-spike.md` (the result, three or four
  lines; it is gardened with this plan)

- [ ] **Step 1:** Install the newest `cargo-semver-checks` release:
      `cargo install cargo-semver-checks --locked`.
- [ ] **Step 2:** Run
      `cargo semver-checks --package ridl-rt --baseline-rev bc58189e~1`.
      Expected: it builds both rustdoc JSON files with toolchain 1.98.1 and
      reports the `ridl-rt` break that #851 made (the size table per codec). If
      it reports a rustdoc format error instead, record the error and use the
      fallback in Task 5.
- [ ] **Step 3:** Record the version, the command and the result in
      `docs/archive/freeze-check-semver-spike.md`. Add the `require` entry to
      `bootstrap` and a pinned install step to the CI gate job (the version from
      step 1, with `--locked`).
- [ ] **Step 4:** Commit:
      `ci: install cargo-semver-checks for the freeze check`.

### Task 2: The freeze harness

**Files:**

- Create: `xtask/src/freeze.rs` (the harness), `xtask/src/freeze/` for the
  dumpers of Tasks 3–5, `freeze/surfaces.toml` (empty table with a header
  comment), `freeze/README.md` (what the directory is, how to update a dump, how
  a break is approved; it links ADR-0029)
- Modify: `xtask/src/main.rs` (a `Some("freeze")` arm, the usage string, the
  task list in the doc comment), `justfile` (recipe `freeze-check`, the `build:`
  line, the `expected` list of `gate-parity`), `.github/workflows/ci.yml`
  (`- run: just freeze-check`, and `fetch-depth: 0` on that job's checkout),
  `AGENTS.md` (the recipe in the Commands list)
- Test: `xtask/tests/freeze.rs`

**Interfaces:**

- Produces, in `xtask/src/freeze.rs`:
  - `pub struct Row { pub surface: String, pub baseline: String }`
  - `pub fn parse_surfaces(toml: &str) -> Result<Vec<Row>, String>` — the file
    is `[[surface]]` tables with `name` and `baseline`.
  - `pub fn missing_lines(baseline: &str, head: &str) -> Vec<String>` — the
    lines of `baseline` that are not in `head`, compared as sets, sorted.
  - `pub type Dumper = fn(&Path) -> Result<Vec<String>, String>` — takes the
    repository root, returns lines (the harness sorts and deduplicates).
  - `pub fn dumpers() -> &'static [(&'static str, Dumper)]` — the registry;
    Tasks 3–4 add entries.
  - `pub fn run(args: &[String]) -> Result<(), String>` with subcommands
    `update` (write every `freeze/<surface>.txt`) and `check` (the two checks
    below, over every dumper).
- `check` does, for every dumper: (a) regenerate and compare with the committed
  `freeze/<surface>.txt`; a difference fails with "run
  `cargo xtask
  freeze update` and commit the result"; (b) if the surface has a
  row, read `git show <baseline>:freeze/<surface>.txt` and fail on any
  `missing_lines`, printing each line and the row. It also fails on a row whose
  surface has no dumper.

- [ ] **Step 1: Write the failing tests** in `xtask/tests/freeze.rs`:
  - `missing_lines_reports_a_removed_line`:
    `missing_lines("a\nb\n", "a\n") == vec!["b"]`.
  - `missing_lines_accepts_an_added_line`:
    `missing_lines("a\n", "a\nb\n").is_empty()`.
  - `missing_lines_reports_a_changed_line_as_removed`:
    `missing_lines("k = 1\n", "k = 2\n") == vec!["k = 1"]`.
  - `missing_lines_ignores_duplicates`:
    `missing_lines("a\na\n", "a\n").is_empty()`.
  - `parse_surfaces_reads_rows`: two `[[surface]]` tables give two `Row`s in
    file order; a table without `baseline` is an error that names the surface.
  - `check_fails_on_a_row_without_a_dumper`,
    `check_fails_when_the_baseline_has_no_dump`,
    `check_fails_when_the_baseline_ref_is_unknown`: each builds a temporary git
    repository (clear `GIT_DIR` and `GIT_INDEX_FILE` in the child environment,
    see the hooks lesson in `CONTRIBUTING.md`), commits a `freeze/` tree, runs
    `cargo xtask freeze check` with `--root <tmp>` and asserts a non-zero exit
    and the named ref or file in stderr. Use a test-only dumper name registered
    through `--dumper-fixture <name>=<file>`, which reads its lines from a file,
    so that the test does not depend on the real surfaces.
- [ ] **Step 2:** Run `cargo test -p xtask --test freeze`. Expected: FAIL
      (module not found).
- [ ] **Step 3:** Implement `xtask/src/freeze.rs` and the `main.rs` arm. Add
      `--root <dir>` (default: the repository root) and the test-only
      `--dumper-fixture` flag.
- [ ] **Step 4:** Run `cargo test -p xtask --test freeze`. Expected: PASS.
- [ ] **Step 5:** Add the recipe and wire it: `freeze-check: toolchain-check`
      running `cargo xtask freeze check`; add `freeze-check` to `build:` and to
      the `expected` list; add the CI step. Run `just gate-parity` and
      `just freeze-check`. Expected: both pass (no dumper yet, no row).
- [ ] **Step 6:** Commit: `feat(xtask): add the freeze check harness`.

### Task 3: Dumpers that read library data

**Files:**

- Modify: `xtask/Cargo.toml` (path dependencies on `ridl-syntax`, `ridl-core`,
  and `protox` from the workspace), `xtask/src/freeze.rs` (registry entries)
- Create: `xtask/src/freeze/{keywords,diagnostics,mcp,msrv,protocol}.rs`, and
  the five files `freeze/{keywords,diagnostics,mcp,msrv,codegen-protocol}.txt`
  written by `cargo xtask freeze update`
- Test: `xtask/tests/freeze.rs` (one test per dumper)

**Interfaces:** each file exports
`pub fn dump(root: &Path) -> Result<Vec<String>, String>`.

| Surface            | Line format                                                                                                                                                                                                              | Source                                    |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------- |
| `keywords`         | `<profile> <word>` for each word of `ridl_syntax::keywords::FAMILY_RESERVED` and each `Profile` where `keyword_in(profile, word)` is `Some`; `family <word>` for a word reserved in no profile                           | `crates/ridl-syntax/src/keywords.rs`      |
| `diagnostics`      | `<CODE> <severity>` for every entry of `ridl_core::diag::ALL_CATALOGS`; for an entry with a lint, `<CODE> <severity> lint=<name> default=<level>` with `ridl_core::lint::default_level`. The summary text is not dumped. | `crates/ridl-core/src/diag.rs`, `lint.rs` |
| `mcp`              | `<tool>` and `<tool> input <json-pointer> <type>` / `<tool> output <json-pointer> <type>` for every property path of the schemas, with `required` marked as `<path> required`                                            | `crates/ridl-mcp/tests/tools.json`        |
| `msrv`             | `<crate> rust-version=<v> edition=<e>` for `ridl-rt`, `ridl-rt-conformance`, `ridl-loopback`, `ridl-descriptor`, read from each `Cargo.toml` with the workspace values resolved                                          | the crate manifests                       |
| `codegen-protocol` | `<package>.<Message>.<field> = <number> <label> <type>` and `<package>.<Enum>.<VALUE> = <number>` for every message and enum under `crates/ridl-ir/proto/ridl/ir/v2/` and `.../codegen/v1/`, compiled with `protox`      | the `.proto` files                        |

A change of a field's JSON name breaks the codegen JSON; the canonical JSON name
derives from the field name, so the field line covers it.

- [ ] **Step 1: Write the failing tests:**
  - `keywords_dump_lists_a_ridl_keyword`: the dump contains `ridl interface`.
  - `diagnostics_dump_carries_lint_and_level`: the dump contains a line for
    `RSDL-807` with `lint=link-encoding-not-emitted`, and no line contains the
    summary text of that entry.
  - `mcp_dump_lists_every_tool`: nine lines with no space, equal to the nine
    names of `tools.json`.
  - `msrv_dump_reads_ridl_rt`: contains
    `ridl-rt rust-version=1.83 edition=2021`.
  - `protocol_dump_lists_the_package_root`: contains a line that starts with
    `ridl.ir.v2.Package.`.
  - `dumps_are_sorted_and_relative`: for every registered dumper, the output of
    `run update` is sorted, has no duplicate, and contains neither the absolute
    root path nor a `\`.
- [ ] **Step 2:** Run `cargo test -p xtask --test freeze`. Expected: the new
      tests FAIL.
- [ ] **Step 3:** Implement the five dumpers and register them.
- [ ] **Step 4:** Run the tests (PASS), then `cargo xtask freeze update` and
      `just freeze-check` (PASS). Check that `just compile` still builds with
      the new xtask dependencies under `--locked` (update `Cargo.lock` in this
      commit).
- [ ] **Step 5:** Commit:
      `feat(xtask): dump keywords, diagnostics, MCP tools, MSRV and the codegen protocol`.

### Task 4: Dumpers that run the CLI

**Files:**

- Modify: `crates/ridl-core/src/manifest.rs` (extract the inline key lists of
  the MANI-005 check, about lines 641–762, into
  `pub const KNOWN_KEYS: &[(&str, &[&str])]` — section name to allowed keys,
  with `""` for the top level — and use it in the check)
- Create: `xtask/src/freeze/{cli,manifest,sources,wire}.rs`, and
  `freeze/{cli,manifest,accepted-sources,wire}.txt`
- Test: `xtask/tests/freeze.rs`; `crates/ridl-core` unit test for the extraction

| Surface            | Line format                                                                                                                                                                                                                                                                                                                                                                                                                    | Source                                                                                                                    |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------- |
| `cli`              | `ridl <path…>` for each subcommand, `ridl <path…> <flag> <value-kind>` for each flag (`<value-kind>` is the placeholder clap prints, or `switch`), for both `ridl` and `ridlc`; `exit <n>` for the codes ADR-0010 decision 1 lists; `json <pointer>` and `sarif <pointer>` for every key path of `ridl check --format json` and `--format sarif` run on `xtask/tests/freeze-fixtures/warn/` (a package that draws one warning) | the built binaries' `--help`, walked recursively as `crates/ridl/tests/cli_reference_support/mod.rs` `real_commands` does |
| `manifest`         | `<section>.<key>` for every entry of `KNOWN_KEYS`                                                                                                                                                                                                                                                                                                                                                                              | `ridl_core::manifest::KNOWN_KEYS`                                                                                         |
| `accepted-sources` | `<relative path> ok` for every package directory under `examples/` and `evals/corpus/` that `ridl check` accepts with exit 0                                                                                                                                                                                                                                                                                                   | the built `ridl`                                                                                                          |
| `wire`             | `encoding <name>` for each name the manifest accepts in `encodings`; `proto <relative file> <line>` and `fbs <relative file> <line>` for each non-blank, non-comment line that `ridl build --emit` writes for `evals/corpus/` (the emit names `ridl build --help` lists for proto3 and FlatBuffers); `catalog-hash <unit> <hex>` for each unit that `ridl describe` reports for `examples/cabin`                               | the built `ridl`                                                                                                          |

Frame vectors do not exist yet; lane W adds them to the `wire` dumper with the
WebSocket binding document.

- [ ] **Step 1: Write the failing tests:**
  - `known_keys_drive_mani_005` (in `ridl-core`): every key of `KNOWN_KEYS` is
    accepted without MANI-005, and the key `package.nmae` draws MANI-005.
  - `cli_dump_lists_ridl_build_out_dir`: the dump contains a line that starts
    with `ridl build --out-dir`.
  - `cli_dump_lists_json_code_key`: contains `json` followed by a pointer that
    ends with `/code`.
  - `manifest_dump_lists_backend_encodings`: contains `backend.rust.encodings`
    (or the section form the loader uses for `[backend.<name>]`; write the test
    against the real key).
  - `sources_dump_accepts_cabin`: contains `examples/cabin ok`.
  - `wire_dump_lists_encodings`: contains `encoding flatbuffers`.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Extract `KNOWN_KEYS`; implement the four dumpers. They build
      `ridl` and `ridlc` with `cargo build --locked -p ridl-cli -p ridlc` and
      use `${CARGO_TARGET_DIR:-target}/debug/`.
- [ ] **Step 4:** Run the tests and `cargo test -p ridl-core` (PASS);
      `cargo xtask freeze update`; `just freeze-check` (PASS).
- [ ] **Step 5:** Commit in two commits:
      `refactor(ridl-core): name the manifest keys that MANI-005 accepts` and
      `feat(xtask): dump the CLI, the manifest, accepted sources and the wire mapping`.

### Task 5: The Rust API checks

**Files:**

- Create: `xtask/src/freeze/rust_api.rs`, `freeze/generated/cabin/` and
  `freeze/generated/veh-cluster/` (the crates `ridl build --emit rust` writes,
  with every encoding named in a manifest under `xtask/tests/freeze-fixtures/`)
- Modify: `xtask/src/freeze.rs` (a second registry for API surfaces),
  `.primignore` if prim rewrites anything under `freeze/generated/`
- Test: `xtask/tests/freeze.rs`

**Interfaces:**

- `runtime-crates` surface: for a row, run
  `cargo semver-checks --package <crate> --baseline-rev <baseline>` for
  `ridl-rt`, `ridl-rt-conformance`, `ridl-loopback`, `ridl-descriptor`.
- `generated-crate` surface: (a) regenerating into a temporary directory must
  equal `freeze/generated/`; (b) for a row,
  `git archive <baseline>
  freeze/generated` into a temporary directory and run
  `cargo semver-checks --manifest-path <head crate>/Cargo.toml --baseline-root
  <baseline crate>`
  for each crate.
- If Task 1 found that `cargo-semver-checks` cannot read the toolchain's rustdoc
  JSON, replace both with a line dump of public items:
  `cargo +1.98.1 rustdoc -- -Zunstable-options --output-format json` is not
  available on stable, so the fallback is `pub` item paths collected with `syn`
  from the sources (`<crate>::<path> <kind>`). Record the choice in ADR-0029.

- [ ] **Step 1: Write the failing tests:**
  - `generated_dump_is_current`: after `run update`, regeneration equals the
    committed tree.
  - `semver_check_flags_a_removed_item` (marked `#[ignore]` unless
    `cargo-semver-checks` is on `PATH`; CI has it): in a temporary copy of a
    tiny fixture crate under `xtask/tests/freeze-fixtures/api/`, commit, remove
    a `pub fn`, commit, and assert the check fails naming the function.
- [ ] **Step 2:** Run. Expected: FAIL.
- [ ] **Step 3:** Implement `rust_api.rs`.
- [ ] **Step 4:** Run the tests, `cargo xtask freeze update`,
      `just freeze-check` (PASS). Run `just build` once in full.
- [ ] **Step 5:** Commit:
      `feat(xtask): check the runtime and generated crate APIs with cargo-semver-checks`.

### Task 6: ADR-0029 and the Stability chapter

**Files:**

- Create: `docs/decisions/ADR-0029-stability-policy.md` (headings as ADR-0027:
  Status, Context, Decision, Alternatives considered, Consequences, References),
  `docs/book/stability.md`
- Modify: `docs/decisions/README.md` (index entry), `docs/book/SUMMARY.md`,
  `CONTRIBUTING.md` (a section "Changing a frozen surface"), and the records
  that hold a stability statement today, each with a dated note "Amended
  2026-10-xx by ADR-0029": the IR specification §6–§7, ADR-0005 §7, ADR-0008
  decision 9, ADR-0010, ADR-0021 decision 10, ADR-0025

ADR-0029's decisions, from the spec: (1) the frozen surfaces of spec section 3;
(2) the not-frozen list; (3) the gate and its two checks, as built; (4) the
break rules of spec 4.4; (5) the minimum Rust version is frozen; (6) the table
of gates with an empty "baseline" column, which lanes fill. The ADR is numbered
0029 because spec 2b takes 0028; if 2b has not merged when this task runs, keep
0029 anyway.

ADR-0029 is the record that owns stability; the amended records link to it
instead of restating it.

- [ ] **Step 1:** Write the ADR, the book chapter (for users: what will not
      change in 0.10.x and what may), the index entry, the SUMMARY entry and the
      CONTRIBUTING section.
- [ ] **Step 2:** Add the amendment notes.
- [ ] **Step 3:** Run `just book-check`, `just link-check`,
      `just doc-path-check`, `just story-id-check`, `just check`. Expected:
      PASS.
- [ ] **Step 4:** Commit: `docs(adr): add ADR-0029, the stability policy`.

### After the pull request merges: the `mcp` row

In a follow-up pull request: add the `mcp` row to `freeze/surfaces.toml` with
the merge commit as its `baseline`, after lane B has reported that #347 holds no
pending MCP schema break. Update the gate table in ADR-0029 and in the lanes
plan.
