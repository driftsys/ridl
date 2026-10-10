# Catalog compatibility (option A) — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** `ridl baseline` records the chain of compatible catalog hashes per
unit, `ridl build` emits that chain as `Catalog.compatible` in the catalog
descriptor and in the codegen model, and the descriptor stays readable by an
engine built against the schema as it is today.

**Architecture:** A per-unit verdict function over `ridl diff`'s report
(`ridl-diff`), a line-oriented history file beside the baseline snapshots
(`ridl-core`), one appended descriptor field and one appended model field
(`ridl-descriptor`, `ridl-ir`), the chain writer in `ridl baseline` (`ridl`) and
the chain reader in `ridlc::run_build_with` (`ridlc`), then the as-built
records. No runtime, face or lockfile changes shape.

**Tech Stack:** Rust 2024 at the pinned toolchain; FlatBuffers through planus
(toolchain only); protobuf through prost; `insta` snapshots.

**Spec:**
[`2026-10-09-catalog-compat-design.md`](2026-10-09-catalog-compat-design.md) —
rulings R-H2-1 to R-H2-14 bind every task. The normative text is ADR-0014
decision 15 as amended on 2026-10-09 and frame specification §6.1.

## Global Constraints

- The descriptor's `SCHEMA_VERSION` stays `1`; the new field is appended at the
  end of `table Catalog` and is not `required` (R-H2-7, R-H2-8).
- `ridl diff`'s report, text and JSON, and its exit code are unchanged
  (R-H2-13).
- The generated Rust face, `ridl-rt`, `interfaces.lock` and the baseline
  snapshots' shape are unchanged (R-H2-7).
- `ridlc` reads no baseline: the `ridl` facade computes the per-unit lists and
  passes them to `ridlc::run_build_with`; `ridlc build` writes an empty list
  (R-4, decided by the main session; ADR-0008 decisions 9 and 14).
- A `Breaking` unit verdict resets the chain to the one new hash at publication
  and emits an empty list at build time (R-H2-3).
- Every earlier hash is read from the `<unit>.catalogs` file, never recomputed
  from a snapshot (R-H2-5).
- Every test's red state is an assertion failure, not a compile error: a task
  that adds a function first adds it with the body the step names, then writes
  the test, then replaces the body.
- Each commit follows Conventional Commits with a scope from `.git-std.toml` and
  ends with the attribution trailer the session carries. Prose is plain and
  literal. No story id in a shipped file (`just story-id-check`).
- `just fmt` before every commit that touches Markdown; `just verify` before the
  pull request.

## Review Focus

Inputs the spec implies and no task's tests exercise as written; each line names
the owning task, whose steps add the test.

1. A baseline directory that exists and holds no `.ir.json` file at all: the
   build must emit an empty list and no error, as `ridl check` treats it (Task
   6, test `an_empty_baseline_directory_is_no_baseline`).
2. A `<unit>.catalogs` file whose first hash equals the current catalog's hash
   (the tree is the published baseline, `Identical`): the list must hold the
   earlier hashes and not the current one (Task 6, test
   `the_current_hash_is_never_listed`).
3. A hash listed twice across the replaced file and the new publication: the
   file must hold it once (Task 5, test `a_hash_is_never_written_twice`).
4. A breaking change in another unit that this unit does **not** reach: this
   unit's chain must continue (Task 1, test
   `a_break_in_an_unreached_unit_does_not_concern_this_unit`).
5. A `.catalogs` file left by a unit that no longer has a shape: publication
   must remove it (Task 5, test `a_stale_catalogs_file_is_removed`).

---

### Task 1: The per-unit verdict in `ridl-diff`

**Files:**

- Create: `crates/ridl-diff/src/unit_verdict.rs`
- Modify: `crates/ridl-diff/src/lib.rs` (declare the module, re-export the
  function beside `diff_workspaces`)

**Interfaces:**

- Consumes: `ridl_diff::{DiffReport, Change, Verdict}`;
  `ridl_ir::catalog_hash::reachable_decls(unit: &str, packages: &[&Package]) -> BTreeMap<String, &Decl>`;
  `ridl_ir::v2::unit_of(&Package) -> &str`.
- Produces:
  `pub fn unit_verdict(report: &DiffReport, unit: &str, old: &[Package], new: &[Package]) -> Verdict`.
  A change concerns `unit` when the first `/`-segment of `change.path` names a
  package whose `unit_of` is `unit` in `old` or in `new`, or when the first two
  segments joined by `.` are a key of `reachable_decls(unit, old)` or of
  `reachable_decls(unit, new)`. The result is the maximum `verdict` over the
  concerning changes, `Verdict::Identical` when none concerns the unit.

- [ ] **Step 1: Add the function with the workspace verdict as its body**

In `unit_verdict.rs`, define the signature above with the body `report.verdict`,
and declare `pub mod unit_verdict;` plus `pub use unit_verdict::unit_verdict;`
in `lib.rs`. This compiles and is wrong for every test below that expects a
verdict lower than the report's.

- [ ] **Step 2: Write the failing tests**

In `unit_verdict.rs`, a `#[cfg(test)] mod tests` that builds packages by hand
with `ridl_ir::v2` types (as `crates/ridl-descriptor/tests/lower.rs` does): unit
`a` (package `a`, one interface with a signal whose payload is the named type
`b.S`), unit `b` (package `b`, `struct S { x: i32 }`), unit `c` (package `c`,
`struct T { y: i32 }`). Each test builds `old` and `new`, runs
`diff_sets(&old, &new)`, and asserts:

```rust
#[test]
fn a_break_in_a_reached_declaration_of_another_unit_concerns_this_unit() {
    // new: b.S.x becomes i64
    assert_eq!(unit_verdict(&report, "a", &old, &new), Verdict::Breaking);
    assert_eq!(unit_verdict(&report, "c", &old, &new), Verdict::Identical);
}

#[test]
fn a_break_in_an_unreached_unit_does_not_concern_this_unit() {
    // new: c.T.y becomes i64
    assert_eq!(unit_verdict(&report, "a", &old, &new), Verdict::Identical);
    assert_eq!(unit_verdict(&report, "c", &old, &new), Verdict::Breaking);
}

#[test]
fn an_appended_interaction_is_compatible_for_its_own_unit() {
    // new: unit a's interface gains an event at the end
    assert_eq!(unit_verdict(&report, "a", &old, &new), Verdict::Compatible);
    assert_eq!(unit_verdict(&report, "b", &old, &new), Verdict::Identical);
}

#[test]
fn a_package_added_to_a_unit_concerns_that_unit() {
    // new: package `a.sub` with unit `a`, holding a struct
    assert_eq!(unit_verdict(&report, "a", &old, &new), Verdict::Compatible);
    assert_eq!(unit_verdict(&report, "b", &old, &new), Verdict::Identical);
}

#[test]
fn a_declaration_reached_only_on_the_new_side_concerns_the_unit() {
    // new: unit a's signal payload changes from b.S to c.T (breaking for a),
    // and c.T.y also changes width (breaking in c)
    assert_eq!(unit_verdict(&report, "a", &old, &new), Verdict::Breaking);
}
```

- [ ] **Step 3: Run the tests and confirm each fails on an assertion**

Run: `cargo test -p ridl-diff unit_verdict` Expected: FAIL; every failure is an
`assertion failed` on a `Verdict` (for example `Breaking` where `Identical` was
expected), never a compile error.

- [ ] **Step 4: Implement the filter**

Replace the body: collect the unit's package names from `old` and `new`
(`unit_of(pkg) == unit`), compute the two `reachable_decls` maps once, then fold
the report's changes with the rule in the Interfaces block. Document the path
grammar the function reads (`crates/ridl-diff/src/walk.rs`, `emit`, and
`Change::path`'s doc) in the rustdoc.

- [ ] **Step 5: Run the tests and the crate's suite**

Run: `cargo test -p ridl-diff` Expected: PASS, every existing test included.

- [ ] **Step 6: Commit**

```bash
git add crates/ridl-diff
git commit -m "feat(ridl-diff): add the per-unit verdict over a report"
```

---

### Task 2: The `<unit>.catalogs` history file in `ridl-core`

**Files:**

- Create: `crates/ridl-core/src/catalog_history.rs`
- Modify: `crates/ridl-core/src/lib.rs` (declare `pub mod catalog_history;`)

**Interfaces:**

- Produces:
  - `pub const FILE_SUFFIX: &str = ".catalogs";`
  - `pub const HEADER: &str = "# catalogs of this unit's published baselines, newest first, back to the last breaking change";`
  - `pub struct CatalogHistory { pub hashes: Vec<[u8; 32]> }` — newest first, no
    duplicate.
  - `pub struct HistoryError { pub line: usize, pub message: String }`
    (`Display` prints `line N: message`).
  - `pub fn parse(text: &str) -> Result<CatalogHistory, HistoryError>` — skips
    blank lines and lines starting with `#`; every other line is exactly 64
    lowercase hex characters, else `HistoryError`; a hash that repeats is
    `HistoryError`.
  - `impl CatalogHistory { pub fn render(&self) -> String }` — `HEADER`, a
    newline, then one lowercase hex line per hash, each newline-terminated.
  - `impl CatalogHistory { pub fn push_front(&mut self, hash: [u8; 32]) }` —
    inserts at the front and removes any later copy of the same hash.

- [ ] **Step 1: Add the module with placeholder bodies**

`parse` returns `Ok(CatalogHistory { hashes: Vec::new() })`; `render` returns
`HEADER` plus a newline; `push_front` pushes at the back.

- [ ] **Step 2: Write the failing tests**

In the module's `#[cfg(test)] mod tests`:

```rust
#[test]
fn a_rendered_history_parses_back_in_order() {
    let history = CatalogHistory { hashes: vec![[1; 32], [2; 32]] };
    let parsed = parse(&history.render()).unwrap();
    assert_eq!(parsed.hashes, vec![[1; 32], [2; 32]]);
}

#[test]
fn render_writes_the_header_then_lowercase_hex() {
    let text = CatalogHistory { hashes: vec![[0xab; 32]] }.render();
    assert_eq!(text, format!("{HEADER}\n{}\n", "ab".repeat(32)));
}

#[test]
fn blank_and_comment_lines_are_skipped() {
    let text = format!("{HEADER}\n\n# note\n{}\n", "00".repeat(32));
    assert_eq!(parse(&text).unwrap().hashes, vec![[0; 32]]);
}

#[test]
fn a_line_that_is_not_64_hex_characters_is_refused_with_its_line_number() {
    let text = format!("{HEADER}\n{}\nnot-a-hash\n", "00".repeat(32));
    let err = parse(&text).unwrap_err();
    assert_eq!(err.line, 3);
}

#[test]
fn uppercase_hex_is_refused() {
    assert!(parse(&format!("{HEADER}\n{}\n", "AB".repeat(32))).is_err());
}

#[test]
fn a_repeated_hash_is_refused() {
    let line = "00".repeat(32);
    assert!(parse(&format!("{HEADER}\n{line}\n{line}\n")).is_err());
}

#[test]
fn push_front_moves_an_existing_hash_to_the_front() {
    let mut history = CatalogHistory { hashes: vec![[1; 32], [2; 32]] };
    history.push_front([2; 32]);
    assert_eq!(history.hashes, vec![[2; 32], [1; 32]]);
}
```

- [ ] **Step 3: Run the tests and confirm each fails on an assertion**

Run: `cargo test -p ridl-core catalog_history` Expected: FAIL on assertions
(`parses back` sees an empty vector; the refusal tests see `Ok`).

- [ ] **Step 4: Implement `parse`, `render` and `push_front` as specified**

- [ ] **Step 5: Run the tests**

Run: `cargo test -p ridl-core catalog_history` Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/ridl-core
git commit -m "feat(ridl-core): add the catalog history file reader and writer"
```

---

### Task 3: The descriptor's `compatible` field

**Files:**

- Modify: `crates/ridl-descriptor/schema/catalog.fbs` (append
  `table EarlierCatalog { hash: [ubyte] (required); }` before `table Catalog`,
  and `compatible: [EarlierCatalog];` as the last field of `Catalog`, with a
  comment naming ADR-0014 decision 15's 2026-10-09 amendment and the
  not-required rule)
- Regenerate: `crates/ridl-descriptor/src/generated.rs` with
  `cargo xtask descriptor-codegen`
- Modify: `crates/ridl-descriptor/src/lower.rs` (`lower` gains a parameter),
  `crates/ridl-descriptor/src/lib.rs` (`walk` touches the field; the crate-level
  doc's field list), `crates/ridl-descriptor/src/describe.rs` (the JSON view)
- Modify callers: `crates/ridlc/src/lib.rs` (`write_catalogs` passes `&[]` for
  now), `crates/ridl-descriptor/tests/lower.rs` (`lower_one` passes `&[]`)
- Test: `crates/ridl-descriptor/tests/lower.rs`,
  `crates/ridl-descriptor/tests/verify.rs`, the unit tests in `describe.rs`
- Update snapshot:
  `crates/ridl/tests/snapshots/describe_cli__corpus_catalog.snap` (gains
  `"compatible": []`)

**Interfaces:**

- Produces:
  `pub fn lower(unit: &str, packages: &[&Package], compatible: &[[u8; 32]]) -> Result<Vec<u8>, LowerError>`
  — writes `compatible` always, one `EarlierCatalog` per hash in the given
  order, empty when the slice is empty. The generated reader
  `CatalogRef::compatible(&self) -> planus::Result<Option<planus::Vector<'a, planus::Result<EarlierCatalogRef<'a>>>>>`
  (the exact type is what planus generates). `describe::to_json` emits
  `"compatible"` as an array of byte arrays, `[]` when the field is absent.

- [ ] **Step 1: Append the schema field and regenerate the accessors**

Run: `cargo xtask descriptor-codegen`, then
`cargo test -p xtask committed_generated_accessors_match_the_schema`. Expected:
PASS.

- [ ] **Step 2: Add the `compatible` parameter to `lower` and ignore it**

Change the signature, fix the two callers with `&[]`, and leave the body writing
no `compatible` entry (`compatible_as_null` or an empty vector — pick the empty
vector, which Step 5 keeps).

- [ ] **Step 3: Write the failing tests**

In `tests/lower.rs`:

```rust
#[test]
fn the_compatible_catalogs_are_written_in_order() {
    let bytes = lower(unit_of(&package), &[&package], &[[1; 32], [2; 32]]).unwrap();
    let catalog = verify(&bytes).unwrap();
    let listed: Vec<Vec<u8>> = catalog.compatible().unwrap().unwrap()
        .iter().map(|entry| entry.unwrap().hash().unwrap().to_vec()).collect();
    assert_eq!(listed, vec![vec![1; 32], vec![2; 32]]);
}

#[test]
fn an_empty_list_is_written_as_an_empty_vector() {
    let bytes = lower_one(&package).unwrap();
    let catalog = verify(&bytes).unwrap();
    assert_eq!(catalog.compatible().unwrap().unwrap().len(), 0);
}
```

In `tests/verify.rs`, a test that builds a buffer with the planus builder
**without** the field (as the file was written before this change) and asserts
`verify` accepts it and `compatible()` reads `None`:

```rust
#[test]
fn a_file_without_the_compatible_field_still_verifies() {
    let catalog = verify(&buffer_without_compatible()).unwrap();
    assert!(catalog.compatible().unwrap().is_none());
}
```

In `src/describe.rs`'s tests: `to_json` of a buffer with two entries yields
`"compatible": [[1,1,…],[2,2,…]]`, and of a buffer without the field yields
`"compatible": []`.

- [ ] **Step 4: Run the tests and confirm the failures are assertions**

Run: `cargo test -p ridl-descriptor` Expected:
`the_compatible_catalogs_are_written_in_order` fails comparing an empty list
with two hashes; the describe tests fail on the missing key; the verify test
passes already (it pins the lenient reader).

- [ ] **Step 5: Implement**

`lower` writes the entries; `walk` in `lib.rs` iterates `catalog.compatible()?`
when `Some` and touches each entry's `hash()?`; `describe` adds the key. Update
the crate-level doc's list of `Catalog` fields.

- [ ] **Step 6: Run the suite with and without the `std` feature, then update
      the describe snapshot**

Run:
`cargo test -p ridl-descriptor && cargo test -p ridl-descriptor --no-default-features`
Expected: PASS. Run: `cargo insta test -p ridl-cli --test describe_cli --accept`
(or `INSTA_UPDATE=always cargo test -p ridl-cli --test describe_cli`), then read
the snapshot diff: the only change is a `"compatible": []` key.

- [ ] **Step 7: Commit**

```bash
git add crates/ridl-descriptor crates/ridlc/src/lib.rs crates/ridl/tests/snapshots
git commit -m "feat(ridl-descriptor)!: append the compatible catalogs to the descriptor"
```

(The `!` marks the `lower` signature change for crates.io consumers of the `std`
half; the file format is append-compatible.)

---

### Task 4: The codegen model's `Catalog.compatible`

**Files:**

- Modify: `crates/ridl-ir/proto/ridl/codegen/v1/model.proto`
  (`repeated bytes compatible = 4;` in `message Catalog`, commented as the
  descriptor's list, same order)
- Modify: `crates/ridl-ir/src/codegen/lower.rs` (`lower_with`; `catalog` fills
  the field), `crates/ridlc/src/lib.rs` (`codegen_request` gains the parameter;
  the one internal caller passes `&[]` for now)
- Modify callers: `crates/ridlc-gen-rust/tests/parity.rs:98`,
  `crates/ridlc-gen-model/tests/parity.rs:91`,
  `crates/ridlc/tests/layout.rs:287`, `crates/ridlc/tests/codegen_model.rs:561`
  and `:589` (pass `&[]`)
- Test: `crates/ridlc/tests/codegen_model.rs`
- Update snapshots: `crates/ridlc/tests/snapshots/corpus__codegen@*.snap` if the
  canonical JSON writes the empty field

**Interfaces:**

- Produces:
  `pub fn lower_with(package: &v2::Package, others: &[&v2::Package], compatible: &[[u8; 32]]) -> v1::Model`
  in `ridl_ir::codegen`, with `lower(package, others)` kept as
  `lower_with(package, others, &[])`;
  `pub fn codegen_request(base: &str, package: &Package, others: &[&Package], compatible: &[[u8; 32]], options: Vec<BackendOption>, deployment: Option<Deployment>, header: Option<&str>) -> CodegenRequest`
  in `ridlc`.

- [ ] **Step 1: Add the proto field and the two signatures, filling nothing**

`lower_with` ignores `compatible`; `codegen_request` passes it to `lower_with`.
Fix every caller with `&[]`.

- [ ] **Step 2: Write the failing test**

In `crates/ridlc/tests/codegen_model.rs`, beside the existing request tests:

```rust
#[test]
fn the_request_carries_the_compatible_catalogs_in_order() {
    let request = ridlc::codegen_request("x", &package, &[], &[[1; 32], [2; 32]], Vec::new(), None, None);
    let catalog = request.model.unwrap().catalog.unwrap();
    assert_eq!(catalog.compatible, vec![vec![1u8; 32], vec![2u8; 32]]);
}
```

- [ ] **Step 3: Run it and confirm the assertion fails**

Run:
`cargo test -p ridlc --test codegen_model the_request_carries_the_compatible_catalogs_in_order`
Expected: FAIL with an empty `compatible`.

- [ ] **Step 4: Fill the field in `catalog()`**

- [ ] **Step 5: Run the workspace tests that read the model; accept snapshot
      changes that add only the new key**

Run: `cargo test -p ridlc -p ridl-ir -p ridlc-gen-rust -p ridlc-gen-model`
Expected: PASS, after `cargo insta accept` for snapshots whose only change is
`"compatible": []`.

- [ ] **Step 6: Commit**

```bash
git add crates/ridl-ir crates/ridlc crates/ridlc-gen-rust crates/ridlc-gen-model
git commit -m "feat(ridl-ir): carry the compatible catalogs in the codegen model"
```

---

### Task 5: `ridl baseline` records the chain

**Files:**

- Create: `crates/ridl/src/catalogs.rs`
- Modify: `crates/ridl/src/main.rs` (`mod catalogs;`; `run_baseline` calls the
  writer after both gates pass and before `publish_baseline`; `publish_baseline`
  publishes `*.catalogs` files wholesale as it does `*.ir.json`, removing stale
  ones)
- Test: create `crates/ridl/tests/catalog_chain.rs` (copy the `TempDir`, `ridl`
  and workspace helpers of `tests/baseline_gate.rs`; add a
  `describe_hash(out: &Path, unit: &str) -> String` helper that runs
  `ridl build --emit catalog` into `out`, then `ridl describe` on
  `<unit>.catalog.binfb`, and returns the `hash` array as lowercase hex)

**Interfaces:**

- Consumes: `ridl_core::catalog_history::{parse, CatalogHistory, FILE_SUFFIX}`;
  `ridl_diff::{diff_sets_in, unit_verdict, Verdict}`;
  `ridlc::{load_diff_side, catalog_scope, std_ir}`;
  `ridl_descriptor::hash::catalog_hash`.
- Produces:
  `pub(crate) fn write_catalog_histories(db: &mut RidlDatabase, staging: &Path, published: &Path) -> Result<(), ExitCode>`
  — loads the staging snapshots and, when `published` holds at least one
  `.ir.json`, the published ones; computes the report
  `diff_sets_in(&published, &staging, &[std_ir()])` once; for every unit of the
  staging packages that has a shape (`Package::shapes()`), computes the new hash
  over `catalog_scope(staging refs, Some(&std))`, reads
  `published/<unit>.catalogs` when present, and writes
  `staging/<unit>.catalogs`: the new hash first, then the earlier hashes when
  `unit_verdict(&report, unit, …)` is `Compatible` or `Identical`, through
  `push_front`. An unreadable published history file is an error (exit 2,
  nothing published).

- [ ] **Step 1: Add the writer with a body that writes a file holding the new
      hash only, and wire it into `run_baseline` and `publish_baseline`**

This compiles and is wrong for the chain tests.

- [ ] **Step 2: Write the failing tests**

In `tests/catalog_chain.rs`, each over a one-unit workspace with an interface
(`ridl lock` run first, as `baseline_gate.rs` does):

```rust
#[test]
fn the_first_publication_records_the_catalog_hash() {
    // publish; read .ridl/baseline/<unit>.catalogs
    assert_eq!(lines, vec![describe_hash(&out, unit)]);
}

#[test]
fn a_compatible_publication_carries_the_earlier_hash_over() {
    // publish; append an event; publish again
    assert_eq!(lines, vec![new_hash, old_hash]);
}

#[test]
fn a_breaking_publication_restarts_the_chain() {
    // publish; change a signal's payload type; publish again
    assert_eq!(lines, vec![new_hash]);
}

#[test]
fn a_hash_is_never_written_twice() {
    // publish; publish again with no change (Identical)
    assert_eq!(lines, vec![hash]);
}

#[test]
fn a_replaced_baseline_without_a_history_file_starts_a_chain() {
    // publish; delete <unit>.catalogs; append an event; publish
    assert_eq!(lines, vec![new_hash]);
}

#[test]
fn a_unit_without_a_shape_gets_no_file() {
    // a types-only package: publish; assert no *.catalogs under the baseline
}

#[test]
fn a_stale_catalogs_file_is_removed() {
    // publish; write .ridl/baseline/gone.catalogs by hand; publish again
    assert!(!baseline.join("gone.catalogs").exists());
}

#[test]
fn a_refused_publication_writes_no_history() {
    // publish; drop an interaction with no tombstone (RIDL-408); the file is byte-identical
}

#[test]
fn an_unreadable_history_file_refuses_the_publication() {
    // publish; overwrite <unit>.catalogs with "garbage"; publish -> exit 2, snapshots unchanged
}
```

- [ ] **Step 3: Run them and confirm the chain tests fail on assertions**

Run: `cargo test -p ridl-cli --test catalog_chain` Expected:
`a_compatible_publication_carries_the_earlier_hash_over` fails with one line
instead of two; `a_stale_catalogs_file_is_removed` and
`an_unreadable_history_file_refuses_the_publication` fail on their assertions;
the rest may already pass.

- [ ] **Step 4: Implement the writer as the Interfaces block states, and the
      wholesale publication of `*.catalogs`**

- [ ] **Step 5: Run the `ridl` crate's tests**

Run: `cargo test -p ridl-cli` Expected: PASS, `baseline_gate.rs` and
`baseline_desk.rs` included.

- [ ] **Step 6: Commit**

```bash
git add crates/ridl
git commit -m "feat(ridl): record the chain of compatible catalogs at publication"
```

---

### Task 6: `ridl build` computes the list and `ridlc` writes it

**Files:**

- Modify: `crates/ridlc/src/lib.rs` (`run_build_with` gains a parameter
  `compatible: &BTreeMap<String, Vec<[u8; 32]>>`, keyed by unit name, and passes
  each unit's list to `write_catalogs` and to `codegen_request`;
  `write_catalogs` gains the same parameter; `run_build` and every other caller
  pass an empty map)
- Modify: `crates/ridlc/src/main.rs` (`ridlc build` passes an empty map)
- Modify: `crates/ridl/src/catalogs.rs` (Task 5's module gains the reader
  below), `crates/ridl/src/main.rs` (`run_build` computes the map before it
  calls `ridlc::run_build_with`, only when the build writes a catalog or
  generates code)
- Test: `crates/ridl/tests/catalog_chain.rs` (extend),
  `crates/ridlc/tests/cli.rs` (one test)

**Interfaces:**

- Consumes: Task 1's `unit_verdict`; Task 2's `parse` and `FILE_SUFFIX`;
  `ridlc::{compile_workspace, load_diff_side, catalog_scope, std_ir}`;
  `default_baseline_dir` in `crates/ridl/src/main.rs`.
- Produces, in `crates/ridl/src/catalogs.rs`:
  `pub(crate) fn compatible_catalogs(db: &mut RidlDatabase, entry: &Path) -> Result<BTreeMap<String, Vec<[u8; 32]>>, ExitCode>`
  — the baseline directory is `default_baseline_dir(entry)`. An absent
  directory, or one with no `.ir.json` file directly inside, yields an empty map
  without compiling anything. Otherwise the snapshots are loaded with
  `load_diff_side` (a load error is reported as `ridl check` reports it, exit
  2), the workspace is compiled with `compile_workspace` (a compile error is
  left to `run_build_with`, which reports it: return an empty map), and the
  report is `diff_sets_in(&baseline, &current, &[std_ir()])`. For each unit of
  the compiled packages with a shape: when `<unit>.catalogs` is absent, no
  entry; when present and `unit_verdict` is `Breaking`, an empty list; otherwise
  the file's hashes minus the current catalog hash
  (`catalog_hash(unit, catalog_scope(current refs, Some(&std)))`). A malformed
  history file is exit 2 with its line named.
- Produces, in `ridlc`:
  `pub fn run_build_with(entry, out_dir, emits, plugins, plugin_timeout, frozen, apply_lints, deployment, compatible: &BTreeMap<String, Vec<[u8; 32]>>) -> io::Result<CliRun>`
  — writes each unit's list into the descriptor (Task 3's `lower`) and the
  codegen request (Task 4's `codegen_request`); a unit absent from the map gets
  an empty list. `ridlc` reads no baseline in this task or any other (ADR-0008
  decisions 9 and 14, ruling R-4).

- [ ] **Step 1: Thread the map through `ridlc` (every caller passes an empty
      map), add `compatible_catalogs` returning an empty map, and call it from
      `run_build`**

- [ ] **Step 2: Write the failing tests**

In `tests/catalog_chain.rs`, with a `compatible_of(out, unit) -> Vec<String>`
helper over `ridl describe`'s `compatible` key, and a
`model_compatible_of(out, pkg) -> Vec<String>` helper over the
`--emit codegen-model` JSON:

```rust
#[test]
fn a_build_after_a_compatible_change_lists_the_baseline_hash() {
    // publish; append an event; build --emit catalog,codegen-model
    assert_eq!(compatible_of(&out, unit), vec![baseline_hash.clone()]);
    assert_eq!(model_compatible_of(&out, pkg), vec![baseline_hash]);
}

#[test]
fn a_build_after_a_breaking_change_lists_nothing() {
    // publish twice (chain of two), then change a payload type; build
    assert_eq!(compatible_of(&out, unit), Vec::<String>::new());
}

#[test]
fn a_build_with_no_baseline_lists_nothing() { … }

#[test]
fn an_empty_baseline_directory_is_no_baseline() {
    // mkdir .ridl/baseline with nothing in it; build exits 0 with an empty list
}

#[test]
fn the_current_hash_is_never_listed() {
    // publish; publish a compatible change (chain of two); build the published tree
    assert_eq!(compatible_of(&out, unit), vec![older_hash]);
}

#[test]
fn a_baseline_that_cannot_be_loaded_fails_the_build() {
    // publish; overwrite one .ir.json with "{"; build -> exit 2, stderr names the file
}

#[test]
fn a_malformed_history_file_fails_the_build() {
    // publish; overwrite <unit>.catalogs with "garbage"; build -> exit 2
}
```

In `crates/ridlc/tests/cli.rs`:

```rust
#[test]
fn ridlc_build_reads_no_baseline() {
    // publish with ridl baseline; append an event; build with ridlc:
    // the descriptor's compatible list is empty although the chain exists
    assert_eq!(compatible_of(&out, unit), Vec::<String>::new());
}
```

- [ ] **Step 3: Run them and confirm the assertion failures**

Run:
`cargo test -p ridl-cli --test catalog_chain && cargo test -p ridlc --test cli ridlc_build_reads_no_baseline`
Expected: the `lists_the_baseline_hash`, `the_current_hash_is_never_listed` and
the two failure-path tests fail on assertions.

- [ ] **Step 4: Implement `compatible_catalogs` as the Interfaces block states**

- [ ] **Step 5: Run the whole gate**

Run: `just verify 2>&1 | tail -30` Expected: every member passes, `just demo`
included (`examples/cabin` has no baseline, so its descriptor is unchanged but
for the empty field).

- [ ] **Step 6: Commit**

```bash
git add crates/ridlc crates/ridl
git commit -m "feat(ridlc): emit the compatible catalogs from the published chain"
```

---

### Task 7: The records and the book, as built

**Files:**

- Modify: `docs/design/catalog-descriptor.md` ("What a catalog contains" gains
  `compatible`; "The catalog hash" gains a paragraph on the list, the chain
  file, the per-unit verdict and the reset, citing ADR-0014 decision 15's
  2026-10-09 amendment; the "Where the code is" and "Tests" tables gain the new
  rows; the decisions table's D-4 row names the field)
- Modify: `docs/book/catalog-descriptor.md` (the field list and the JSON excerpt
  gain `compatible`)
- Modify: `docs/book/cli-reference.md` (`ridl baseline`: the `<unit>.catalogs`
  file, its format, the chain rule and the reset; `ridl build`: the list is
  emitted from it; `ridl describe`: the key; the sentence "nothing else in that
  directory is touched" now excepts `*.catalogs`)
- Modify: `docs/design/codegen-plugins.md:393` (the sentence on `Catalog.hash`
  and `Catalog.retired` names `Catalog.compatible`)
- Modify: `docs/specification/frame-specification.md` (no rule change; the "As
  built" note gains one sentence: the descriptor carries the list since this
  change, and no runtime in this workspace reads it at `attach`)
- Verify: `docs/wip/README.md` lists the design and the plan (added by the H2
  pull request); gardening is lane H stage H5's.

- [ ] **Step 1: Write the record and book changes**

Every sentence describes what Tasks 1 to 6 built; none describes a planned
behaviour. A `ridl`/`typl`/`rsdl` fence in `docs/book/` follows the harness
rules in `AGENTS.md`.

- [ ] **Step 2: Run the documentation gates**

Run:
`just fmt && just check && just book-check && just link-check && just doc-path-check && just story-id-check && cargo test -p ridl-cli --test cli_reference`
Expected: every command exits 0.

- [ ] **Step 3: Commit**

```bash
git add docs
git commit -m "docs: describe the compatible catalogs as built"
```

---

## Models

| Task | Work                                              | Model  | Why                                                                                       |
| ---- | ------------------------------------------------- | ------ | ----------------------------------------------------------------------------------------- |
| 1    | `unit_verdict`                                    | Opus   | The filter decides when a chain resets; its tests must cover the reached-declaration rule |
| 2    | the history file                                  | Sonnet | A line format with a parser and a writer                                                  |
| 3    | the descriptor field                              | Sonnet | Schema, regeneration, a parameter and a walk, under an existing pattern (`retired`)       |
| 4    | the model field                                   | Sonnet | One proto field and a parameter threaded through callers                                  |
| 5    | `ridl baseline` writes the chain                  | Opus   | Staging, publication and two gates interact                                               |
| 6    | `ridl build` computes the list, `ridlc` writes it | Fable  | The rule at the heart of the design; every input class in Review Focus lands here         |
| 7    | records and book                                  | Sonnet | As-built prose over a finished behaviour                                                  |

Each task has a reviewer with both verdicts (the brief is met; the code is
sound) before the next task's implementer starts, then `/review` over the pull
request. Tasks 1 and 2 are independent and may run in either order; Task 3 and
Task 4 are independent of each other and of Tasks 1 and 2; Task 5 needs 1 and 2;
Task 6 needs 1 to 5; Task 7 needs 6.

## After the plan

- A heads-up to the Kotlin codegen plugin's maintainers for `Catalog.compatible`
  in the model is the main session's to file, as the `retired` and `package`
  changes were.
- driftsys/ridl#700 is unchanged by this plan.
