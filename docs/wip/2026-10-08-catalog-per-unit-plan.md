# One Catalog per Unit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** `ridl build --emit catalog` writes one catalog per unit (one
`[package]` manifest and its directory tree), with one numbering space, one
`interfaces.lock` and one hash per unit, and every consumer of the catalog name
(the hash, the system regions, `ridl diff`, the generated `CATALOG`) reads the
unit instead of the source package.

**Architecture:** The loader records the unit of every source package
(`ridl_core::package::Package.unit`) and reads one lock per unit; the IR carries
the unit (`v2::Package.unit`, a new field under ADR-0014's rule); the checker
numbers interfaces once per unit through a unit-level salsa query folded into
each package's IR; the hash, the descriptor, the rsdl regions and `ridl diff`
group packages by `unit`. Two new loader diagnostics enforce the unit rules:
MANI-013 (a nested manifest) and MANI-014 (a source package claimed by two
units).

**Tech Stack:** Rust (the pinned toolchain of `rust-toolchain.toml`), salsa
queries in `ridl-core`/`ridl-sem`, prost/pbjson for the IR, planus for the
descriptor, insta snapshots in `ridlc`, `ridl` and `ridl-sem`.

**Spec:** `docs/wip/2026-10-08-catalog-per-unit-design.md` (approved). The plan
argues from the spec; an executor reads both. The spec's decisions are not
reopened here.

## Global Constraints

- A unit is one `ridl.toml` with a `[package]` table plus every source package
  in its directory tree; the unit name is the `[package] name`, which is also
  the root source package's name and the catalog name (spec, Terms and rule 1).
- The manifest does not change shape: no new table, no new field (spec, The
  manifest).
- MANI-013: a `ridl.toml` in a subdirectory of a unit's tree (rule 2). MANI-014:
  a source package declared by two units of one build (rule 3). Both are errors.
- A catalog file is `<unit name>.catalog.binfb`, one per unit with at least one
  interface shape; its `name` is the unit name.
- A declared interface's catalog name is its source package path relative to the
  unit plus its name (`cluster.SpeedDisplay`); an interface of the root source
  package keeps its short name (`Session`); an inline shape keeps the service's
  full dotted name, marked inline.
- The lock key of a declared interface is its catalog name; the key of an inline
  shape stays `service:<full dotted name>`. Numbers are 1-based, one space per
  unit, provisional numbers from `next` upward in byte order of the key.
- The hash is SHA-256 over the reduced unit: one IR `Package` named after the
  unit, holding every interface shape of the unit under its catalog name, sorted
  by (number, catalog name), and every reached declaration of any unit under its
  full canonical name (`com.example.hmi.cluster.Foo`); doc strings, `labels` and
  `deprecated` blanked (amends ADR-0014 decision 15).
- `v2::Package` gets a `unit` field, filled for every loaded package; a new
  field follows ADR-0014's rule: appended at the next free number, outside every
  `oneof` tag range, never a `map<>`.
- `Package.retired` in the IR keeps short names; the descriptor's `retired` and
  the codegen model's `Catalog.retired` carry the unit's whole list under
  catalog names.
- Baselines stay one `.ridl/baseline/<pkg>.ir.json` per source package;
  `ridl diff` groups the snapshots by unit and matches numbers within a unit
  (spec, decision 1).
- `Catalog.package` in `model.proto` keeps its name and number; its comment says
  it carries the unit name, which is `CatalogRef.name` (decision 2).
- The root source package can be empty (decision 3).
- Records under `docs/` and rustdoc never name a story, an epic or a task number
  (`just story-id-check`); prose is plain, literal English with no idioms; the
  name of no private consumer project appears anywhere.
- This is a breaking change: every commit that changes a number, a catalog name,
  a lock key or a hash carries a `BREAKING CHANGE:` footer; the release note of
  Task 17 is the consolidated text.
- A snapshot that a task predicts to change is updated with
  `INSTA_UPDATE=always cargo test -p <crate> --test <test>` (or the inline
  module), and the implementer reads the snapshot diff before committing: every
  changed line is one the task names, and an unpredicted change is a defect to
  report, not to accept.
- Commit scopes come from `.git-std.toml`; a change under `evals/` uses the
  scope of the last commit that touched `evals/`
  (`git log --format=%s -- evals`).

## Review Focus

Inputs the spec implies but no task's tests would exercise without the lines
below; each line's test is added to the owning task.

1. **One unit name a prefix of another in one workspace without a tree overlap**
   (`com.example` at `base/`, `com.example.hmi` at `hmi/`, no `hmi/`
   subdirectory under `base/`): both load, no MANI-014, and the hash of
   `com.example` reaches none of `com.example.hmi`'s shapes. Test in Task 11.
2. **A baseline snapshot written before the `unit` field existed** (`unit`
   empty) on the old side of `ridl diff`: it is its own unit, and the diff
   reports exactly what it reported before. Test in Task 14.
3. **A lock entry spelled with the unit prefix**
   (`com.example.hmi.cluster.Speed` in unit `com.example.hmi`'s lock): it names
   no declaration, so it is RIDL-409, and the declaration `cluster.Speed` stays
   provisional. Test in Task 9.
4. **A retired entry whose source package no longer exists**
   (`cluster.Old 3
   retired` after `cluster/` was deleted): the number stays
   retired, the entry is carried by the unit's anchor package, and `ridl diff`
   still reports `InterfaceRetired`. Tests in Tasks 9 and 14.
5. **Two source packages of one unit declaring the same short interface name**
   (`Foo` in `u.a` and `u.b`): two keys `a.Foo` and `b.Foo`, two numbers, two
   catalog entries, two reduced declarations. Tests in Tasks 9 and 11.

## Decisions the plan takes

The spec leaves these to the implementation. Each is recorded here so that the
implementer and the reviewer read the same choice.

- **The anchor package of a unit** is the source package named like the unit
  when it exists, else the first package of the unit in byte order of name. It
  is the package whose check reports the unit's RIDL-409 orphans, and the
  package that carries a retired `service:` entry and a retired interface entry
  whose source package is not in the unit.
- **After MANI-013**, the nested directory and its tree are not loaded as part
  of the unit (the loader skips the directory as it does today); the error is
  the only output for it.
- **MANI-014** reports the second claim in load order (members in `members`
  order, a unit's tree in name order) on the `name` line of the second unit's
  manifest, and does not load the second claimant's package.
- **An `interfaces.lock` in a subdirectory of a unit** is not read. It draws a
  warning on the file, saying that only the lock beside the unit's `ridl.toml`
  is read (author's answer 1). The code is the next free code in the RIDL-4xx
  range, with a lint name, added as ADR-0024 and `docs/book/lints.md` require.
- **The vss corpus directory of a member is its package name**
  (`evals/corpus/vss/vehicle.cabin.hvac.station.row1.driver/`); the book harness
  uses the same rule (`<root>/<package name>/`).
- **The checker split**: `check_package` keeps its signature and its callers;
  the per-package lowering without numbering becomes the inner query
  `lower_package`, and the unit fold is the new query `unit_numbering`. The
  inner query never calls `check_package`, so no salsa cycle exists.
- **Cross-package matching in `ridl diff`**: a frozen number found in another
  package of the same unit on the new side is a match, reported as
  `InterfaceRenamed` (the catalog name changed), and its body is compared as any
  other pair.

## File map

| Path                                                                                                                         | Change                                                                                |
| ---------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------- |
| `crates/ridl-core/src/package.rs`                                                                                            | `Package.unit` input field                                                            |
| `crates/ridl-core/src/workspace.rs`                                                                                          | fill `unit`; `LoadedWorkspace.units`; one lock per unit; MANI-013; MANI-014           |
| `crates/ridl-core/src/diag.rs`                                                                                               | MANI-013 and MANI-014 catalogue rows                                                  |
| `crates/ridl-core/src/interface_lock.rs`                                                                                     | dotted interface keys                                                                 |
| `crates/ridl-ir/proto/ridl/ir/v2/ir.proto`                                                                                   | `Package.unit = 6`; comments                                                          |
| `crates/ridl-ir/src/lib.rs` (module `v2`)                                                                                    | `relative_name`, `Package::catalog_name`, `unit_of`, `unit_retired`                   |
| `crates/ridl-ir/src/catalog_hash.rs`                                                                                         | `reduced_unit`, `catalog_hash(unit, packages)`, `reachable_decls(unit, packages)`     |
| `crates/ridl-ir/src/codegen/lower.rs`                                                                                        | `Catalog { package: unit, hash, retired }`                                            |
| `crates/ridl-ir/proto/ridl/codegen/v1/model.proto`                                                                           | `Catalog.package` comment                                                             |
| `crates/ridl-ir/proto/ridl/ir/v2/system.proto`                                                                               | `Region.catalog` comment                                                              |
| `crates/ridl-sem/src/check.rs`                                                                                               | `lower_package`, `unit_numbering`, the fold in `check_package`                        |
| `crates/ridl-sem/src/rsdl/lower.rs`                                                                                          | regions and routes keyed by unit, catalog names                                       |
| `crates/ridl-descriptor/src/lower.rs`, `src/hash.rs`                                                                         | `lower(unit, packages)`; re-exports                                                   |
| `crates/ridl-descriptor/schema/catalog.fbs`                                                                                  | comments of `Catalog.name`, `Interface.name`, `RetiredInterface`                      |
| `crates/ridlc/src/lib.rs`                                                                                                    | `write_catalogs` per unit; `embed_catalog_hashes` by unit; `--emit catalog` help text |
| `crates/ridl/src/lock.rs`                                                                                                    | `LockedUnit`, `locked_units`, allocation and edits per unit                           |
| `crates/ridl/src/main.rs`                                                                                                    | `shape_key` callers; RIDL-412 lookup by unit                                          |
| `crates/ridl-diff/src/lib.rs`, `src/walk.rs`                                                                                 | grouping by unit, cross-package number match, retired union                           |
| `crates/ridl-rt/src/contract.rs`                                                                                             | `CatalogRef` doc comment                                                              |
| `crates/ridl/tests/book_examples.rs`                                                                                         | flat staging layout                                                                   |
| `evals/corpus/vss/**`, `evals/calibration/*.toml`                                                                            | re-cut, regenerated                                                                   |
| `docs/decisions/ADR-0002`, `-0014`, `-0015`, `-0016`, `-0022`                                                                | amendments                                                                            |
| `docs/design/catalog-descriptor.md`, `interaction-face.md`, `codegen-plugins.md`                                             | amendments                                                                            |
| `docs/specification/ridl-language-reference.md` §11, `rsdl-language-reference.md` §13 and V-16, `ridl-family-overview.md` §7 | amendments                                                                            |
| `docs/book/rsdl.md`, `getting-started.md`, `codegen-plugins.md`, `cli-reference.md`                                          | amendments                                                                            |
| `docs/wip/catalog-per-unit-handoff.md`                                                                                       | the Kotlin heads-up draft and the release note                                        |

## Task order and parallelism

```text
T1 loader unit ─┬─ T4 MANI-013 (needs T3) ── T5 MANI-014 ── T6 lock per unit ─┐
                │                                                              │
T2 book flat    │   (parallel with T1)                                         ├─ T9 numbering ── T10 ridl lock ── T14 diff
T3 vss re-cut   │   (parallel with T1; before T4)                              │        │
T8 lock keys ───┘   (parallel with T1..T7)                                     │        └─ T11 hash ── T12 descriptor ── T13 regions
T7 IR unit field (after T1) ───────────────────────────────────────────────────┘
T15 records, T16 book, T17 handoff: after T13 and T14, parallel with each other.
```

Parallel groups (separate worktrees, each reviewed before the next task that
depends on it starts): {T1, T2, T3, T8}; then T4, T5, T6, T7 in that order
(T4..T6 edit `workspace.rs`, so they are sequential; T7 may run beside T6); T9;
T10; then {T11 → T12 → T13} beside T14; then {T15, T16, T17}.

Every task passes `just fmt-check`, `just compile`, `just test`, `just lint` and
`just check`. A task that touches `ridl-core`, `ridl-ir`, `ridl-sem` or
`ridl-syntax` also passes `just wasm-check`. A task that touches `ridlc`'s emits
or `examples/cabin` also passes `just demo`. A task that touches a Markdown file
also passes `just link-check`, `just doc-path-check`, `just story-id-check` and
`just book-check`. Task 13 and Task 17 end with the full gate `just build`.

---

### Task 1: The loader records the unit of every source package

**Files:**

- Modify: `crates/ridl-core/src/package.rs:63-86` (`Package`)
- Modify: `crates/ridl-core/src/workspace.rs` (`LoadedWorkspace`, `Loader`,
  `load_root`, `load_member`, `load_package_tree`, `load_single_file`)
- Modify: `crates/ridl-core/src/std_lib.rs`,
  `crates/ridl-lsp/src/server.rs:582`, `crates/ridlc/src/lib.rs`,
  `crates/ridlc/src/design_lints/sites.rs`, and the test-only `Package::new`
  sites in `ridl-sem`, `ridlc/tests`, `ridl-sem/tests`
- Test: `crates/ridl-core/src/workspace.rs` (inline `tests` module)

**Interfaces:**

- Produces: `Package.unit: String` (salsa input field, declared right after
  `name`, `#[returns(ref)]`); `LoadedWorkspace.units: BTreeMap<String, PathBuf>`
  (unit name to manifest directory, in the path form of the file paths of
  `sources`; in single-file mode, the file's directory).
- Values: a package of a `[package]` tree has `unit` = the manifest's `name`;
  the synthetic single-file package has `unit` = its own name; `ridl.std` has
  `unit` = `"ridl.std"`; the language server's overlay package (`server.rs:582`)
  has `unit` = its own name.

- [ ] **Step 1: Write the failing tests** in the `tests` module of
      `workspace.rs`, using the module's `TempDir` helper:

```rust
#[test]
fn every_package_of_a_tree_carries_the_manifest_name_as_its_unit() {
    // ridl.toml: [package] name = "veh.hmi"; hmi.ridl at the root;
    // cluster/speed.ridl declaring `package veh.hmi.cluster`.
    // assert: both loaded packages have unit(db) == "veh.hmi"
    // assert: loaded.units == {"veh.hmi" => <root dir>}
}

#[test]
fn a_workspace_member_is_a_unit_and_the_root_is_not() {
    // [workspace] members = ["a", "b"]; a/ridl.toml name "x.a"; b/ridl.toml name "x.b"
    // assert: units keys == ["x.a", "x.b"], each mapped to its member directory
}

#[test]
fn a_single_file_is_its_own_unit() {
    // load `p.ridl` with `package p` and no manifest
    // assert: the one package has unit == "p"; units == {"p" => <file's dir>}
}
```

- [ ] **Step 2: Run them**: `cargo test -p ridl-core --lib workspace::tests` —
      expected: FAIL, "no method named `unit`" is a compile error, so first make
      the field exist with an empty default fill, run again, and expect the
      assertion `unit(db) == "veh.hmi"` to fail with `""`.
- [ ] **Step 3: Implement.** Add the field to `Package`; thread `unit: &str`
      through `load_package_tree(db, dir, unit, name, imports, defaults)`;
      record `self.units.insert(unit, dir)` in `load_root` (package mode) and
      `load_member`; in `load_single_file` set `unit = name` and record the
      directory; copy `Loader.units` into `LoadedWorkspace.units`. Update every
      `Package::new` call site (`grep -rn "Package::new(" crates`): `ridl.std`
      passes `"ridl.std"`, every other site passes its package name.
- [ ] **Step 4: Run** `cargo test -p ridl-core` and `just compile` — expected:
      PASS.
- [ ] **Step 5: Gate**: `just fmt-check`, `just test`, `just lint`,
      `just wasm-check`, `just check`.
- [ ] **Step 6: Commit**:
      `feat(ridl-core): record the unit of every loaded package`.

---

### Task 2: The book harness stages one flat member per package

**Files:**

- Modify: `crates/ridl/tests/book_examples.rs:562-607` (`stage`)
- Test: same file

**Interfaces:**

- Produces: `stage` writes `<root>/<package name>/ridl.toml` (the directory name
  is the dotted package name, not a path) and
  `members = ["<package name>", ...]`.

- [ ] **Step 1: Write the failing test** in `book_examples.rs`:

```rust
#[test]
fn stage_writes_one_flat_member_per_package() {
    // two Example values declaring `package x` and `package x.y`
    // after stage(&examples, root):
    assert!(root.join("x/ridl.toml").is_file());
    assert!(root.join("x.y/ridl.toml").is_file());
    assert!(!root.join("x/y").exists());
    let manifest = std::fs::read_to_string(root.join("ridl.toml")).unwrap();
    assert_eq!(manifest, "[workspace]\nmembers = [\"x\", \"x.y\"]\n");
}
```

- [ ] **Step 2: Run**
      `cargo test -p ridl-cli --test book_examples
      stage_writes_one_flat_member_per_package`
      — expected: FAIL on `root.join("x.y/ridl.toml").is_file()`.
- [ ] **Step 3: Implement**: replace both `package.replace('.', "/")` with
      `package` in `stage`.
- [ ] **Step 4: Run** `cargo test -p ridl-cli --test book_examples` — expected:
      PASS (every book fence still compiles).
- [ ] **Step 5: Gate** (the five common recipes).
- [ ] **Step 6: Commit**:
      `test(ridl): stage every book package as a flat workspace member`.

---

### Task 3: Re-cut the vss corpus and regenerate the calibration records

**Files:**

- Move: `evals/corpus/vss/Vehicle/Cabin/HVAC/**` to
  `evals/corpus/vss/<package name>/` (14 members)
- Modify: `evals/corpus/vss/ridl.toml`, `evals/corpus/vss/PROVENANCE.md`
- Modify: `evals/calibration/low-cohesion-interface.toml`,
  `duplicate-shape.toml`, `recall.toml` and any other file under
  `evals/calibration/` that cites a `Vehicle/Cabin/HVAC` path
- Test: `crates/ridl/tests/eval_corpus.rs` (existing),
  `cargo xtask calibrate derive`

- [ ] **Step 1: Write the failing test** in `crates/ridl/tests/eval_corpus.rs`:

```rust
#[test]
fn the_vss_workspace_has_no_manifest_below_a_member() {
    // walk evals/corpus/vss; for every ridl.toml other than the root's,
    // assert its parent is a direct child of evals/corpus/vss
}
```

- [ ] **Step 2: Run** it — expected: FAIL, naming
      `Vehicle/Cabin/HVAC/Station/ridl.toml`.
- [ ] **Step 3: Move each member** with `git mv` to `evals/corpus/vss/<name>/`
      where `<name>` is the member's `[package] name` (`vehicle.cabin.hvac`,
      `vehicle.cabin.hvac.station`, `vehicle.cabin.hvac.station.row1`, ...,
      `vehicle.cabin.hvac.station.row4.passenger`); the files inside keep their
      names and their `package` lines. Rewrite `members` in the root manifest as
      the 14 names in byte order. Remove the empty `Vehicle/` tree.
- [ ] **Step 4: Regenerate the calibration records.** Run
      `cargo xtask calibrate dump <scratch dir>`. For each `[[finding]]` of each
      label file, rewrite `id` and `location` by the path map
      (`Vehicle/Cabin/HVAC/Station/Row1/Driver/contract.ridl` becomes
      `vehicle.cabin.hvac.station.row1.driver/contract.ridl`, and the same for
      the `# Sampled:` comments), keep every label and reason, and reorder the
      records as the dump orders them. Assert by script that the set of ids in
      the dump equals the set of rewritten ids. Then
      `cargo xtask calibrate derive --write` and confirm
      `git diff
      evals/calibration/summary.md` shows no change to a count
      or a threshold.
- [ ] **Step 5: Record the layout** in `PROVENANCE.md`: a new section "Layout"
      stating that each translated VSS branch is one top-level workspace member
      named after its package, because a unit's tree holds no second manifest
      (one sentence, with a link to ADR-0002 §4 after Task 15 amends it; until
      then cite the rule in prose).
- [ ] **Step 6: Run** `cargo test -p ridl-cli --test eval_corpus` and
      `cargo test -p ridl-cli --test lints` — expected: PASS.
- [ ] **Step 7: Gate** (the five common recipes plus `just link-check`,
      `just doc-path-check`, `just story-id-check`).
- [ ] **Step 8: Commit** (two commits: the move, then the calibration):
      `chore(repo): re-cut the vss corpus as flat workspace members` and
      `chore(repo): regenerate the calibration records for the flat vss layout`.

---

### Task 4: MANI-013 — a manifest inside a unit's tree

**Files:**

- Modify: `crates/ridl-core/src/diag.rs:1307` (append `MANI_013`)
- Modify: `crates/ridl-core/src/workspace.rs:652` (`load_package_tree`)
- Modify: `docs/specification/ridl-family-overview.md:335` (the MANI table)
- Test: `workspace.rs` inline tests; `crates/ridl/tests/check_json.rs`

**Interfaces:**

- Produces: `DiagCode::MANI_013`, `Error`, catalogue text "nested manifest — a
  `ridl.toml` inside a unit's directory tree". Message:
  ``"`{path}` is a `ridl.toml` inside the tree of unit `{unit}`; a unit holds one manifest. Move the directory beside the unit, or delete the manifest"``.
  Primary span: the nested manifest's own file (interned with its text) at range
  0..0.

- [ ] **Step 1: Write the failing tests** in `workspace.rs`:

```rust
#[test]
fn a_manifest_below_a_unit_is_mani_013_and_its_tree_is_not_loaded() {
    // ridl.toml name "veh.hmi"; hmi.ridl; cluster/ridl.toml (any [package]); cluster/x.ridl
    // assert: codes(&loaded.diagnostics) == ["MANI-013"]
    // assert: the diagnostic's primary file is "<root>/cluster/ridl.toml"
    // assert: exactly one package is loaded, named "veh.hmi"
}

#[test]
fn a_member_whose_tree_holds_another_member_is_mani_013() {
    // [workspace] members = ["a", "a/b"]; a/ridl.toml; a/b/ridl.toml
    // assert: codes contain "MANI-013" exactly once
}
```

- [ ] **Step 2: Run** them — expected: FAIL, `codes == []`.
- [ ] **Step 3: Implement** in `load_package_tree`: where a subdirectory with
      its own `ridl.toml` is skipped today, push the MANI-013 diagnostic first
      (intern the nested manifest's text with `self.sources.file_id`), then skip
      as today. Add the catalogue row and the overview table row.
- [ ] **Step 4: Add the CLI test** in `crates/ridl/tests/check_json.rs`:
      `ridl check --format json` over the Task-4 fixture exits 1 and the JSON
      holds one diagnostic with `"code": "MANI-013"`.
- [ ] **Step 5: Run** `cargo test -p ridl-core` and
      `cargo test -p ridl-cli --test check_json` — expected: PASS.
- [ ] **Step 6: Gate** (the five common recipes, `just wasm-check`, and the four
      Markdown recipes).
- [ ] **Step 7: Commit**:
      `feat(ridl-core): refuse a manifest inside a unit's tree (MANI-013)`.

---

### Task 5: MANI-014 — a source package claimed by two units

**Files:**

- Modify: `crates/ridl-core/src/diag.rs` (append `MANI_014`)
- Modify: `crates/ridl-core/src/workspace.rs` (`Loader`, `load_package_tree`, a
  helper `package_name_range(text: &str) -> TextRange` beside
  `member_entry_range`)
- Modify: `docs/specification/ridl-family-overview.md` (the MANI table)
- Test: `workspace.rs` inline tests

**Interfaces:**

- Produces: `DiagCode::MANI_014`, `Error`, catalogue text "a source package is
  declared by two units". `Loader.claims: BTreeMap<String, (String, PathBuf)>`
  (source package name to the claiming unit and its manifest directory).
  Message:
  ``"source package `{pkg}` is already declared by unit `{first}` (`{first dir}`); unit `{second}` declares it too, in `{dir}`. A source package belongs to one unit"``.
  Primary span: the `name` value of the second unit's manifest.

- [ ] **Step 1: Write the failing tests** in `workspace.rs`:

```rust
#[test]
fn a_root_package_already_claimed_by_a_sibling_tree_is_mani_014() {
    // members = ["base", "hmi"]; base/ridl.toml name "com.example" with base/hmi/x.ridl
    //   (package com.example.hmi); hmi/ridl.toml name "com.example.hmi" with hmi/y.ridl
    // assert: codes == ["MANI-014"]; its primary file is "<root>/hmi/ridl.toml"
    // assert: package_of(ws, "com.example.hmi") is the one loaded from base/hmi (unit "com.example")
}

#[test]
fn two_units_with_a_shared_prefix_and_no_overlap_both_load() {
    // members = ["base", "hmi"]; base: name "com.example", base/ids.typl only;
    // hmi: name "com.example.hmi", hmi/y.ridl
    // assert: diagnostics empty; units == {"com.example", "com.example.hmi"}
}
```

- [ ] **Step 2: Run** — expected: the first FAILS with `codes == []`.
- [ ] **Step 3: Implement**: in `load_package_tree`, before
      `self.packages.push(...)`, consult `claims`: a name already claimed by a
      different unit pushes MANI-014 against the current unit's manifest and
      returns without pushing the package (its subdirectories are still walked);
      otherwise record the claim.
- [ ] **Step 4: Run** `cargo test -p ridl-core` — expected: PASS.
- [ ] **Step 5: Gate** (as Task 4).
- [ ] **Step 6: Commit**:
      `feat(ridl-core): refuse a source package claimed by two units (MANI-014)`.

---

### Task 6: One `interfaces.lock` per unit, read at the manifest directory

**Files:**

- Modify: `crates/ridl-core/src/workspace.rs` (`load_package_tree`, `load_root`,
  `load_member`, `read_lock`)
- Modify: `crates/ridl-core/src/package.rs:78-85` (the doc of `Package.lock`)
- Test: `workspace.rs` inline tests

**Interfaces:**

- Produces: `Package.lock` is the unit's lock, read once from the manifest
  directory and attached (cloned) to every package of the unit; a package of a
  unit whose manifest directory has no `interfaces.lock` has `lock == None`.
  `PackageLock.path` is the manifest directory's lock path for every package of
  the unit. Single-file mode is unchanged (the file's directory).

- [ ] **Step 1: Write the failing tests**:

```rust
#[test]
fn every_package_of_a_unit_carries_the_lock_of_the_manifest_directory() {
    // name "veh.hmi"; interfaces.lock "next 1\n" at the root; cluster/speed.ridl
    // assert: both packages' lock(db) are Some and carry path "<root>/interfaces.lock"
}

#[test]
fn a_lock_in_a_subdirectory_is_not_read() {
    // name "veh.hmi"; cluster/interfaces.lock holding malformed text "x"
    // assert: no RIDL-410; the cluster package's lock(db) is None;
    // exactly one warning with the new RIDL-4xx code, on cluster/interfaces.lock
}
```

- [ ] **Step 2: Run** — expected: FAIL (the cluster package's lock is `None` in
      the first test; RIDL-410 is drawn in the second).
- [ ] **Step 3: Implement**: read the lock in `load_root`/`load_member` for the
      unit's directory and pass `lock: &Option<PackageLock>` down
      `load_package_tree`; remove the per-directory `read_lock` call there, and
      report the new warning for an `interfaces.lock` found in a subdirectory.
      Add the code to the catalogue in `crates/ridl-core/src/diag.rs`, the ridl
      reference's diagnostics table, and `docs/book/lints.md` (ADR-0024). Update
      the `Package.lock` doc comment: "the unit's `interfaces.lock`, read by the
      loader from the manifest directory".
- [ ] **Step 4: Run** `cargo test -p ridl-core` and `just test` — expected:
      PASS. `crates/ridl/tests/lock_cli.rs` still passes because its fixtures
      keep the lock beside the manifest.
- [ ] **Step 5: Gate** (the five common recipes, `just wasm-check`).
- [ ] **Step 6: Commit**: `feat(ridl-core): read one interfaces.lock per unit`,
      footer
      `BREAKING CHANGE: an interfaces.lock in a subdirectory of a unit is no longer read; the unit's lock is the one beside its ridl.toml.`

---

### Task 7: The IR records the unit

**Files:**

- Modify: `crates/ridl-ir/proto/ridl/ir/v2/ir.proto:41-59` (`Package`)
- Modify: `crates/ridl-ir/src/lib.rs` (module `v2`, beside `InterfaceShape`)
- Modify: `crates/ridl-sem/src/check.rs:270-280` (`CheckedPackage { ir }`)
- Test: `crates/ridl-ir/src/lib.rs` inline tests; `crates/ridlc/tests/corpus.rs`
  snapshots (`corpus__ir@*.snap`), `crates/ridlc/tests/ir_canonical.rs`,
  `crates/ridl-sem/tests/snapshots/*`

**Interfaces:**

- Produces in `ir.proto`: `string unit = 6;` with the comment "The unit this
  package belongs to: the `name` of the `[package]` manifest whose tree holds
  the package. Equal to `name` for a root source package and for a single file.
  Empty in a snapshot written before the field existed." and the trailer "7–15
  remain open for later profiles."
- Produces in `ridl_ir::v2`:
  - `pub fn relative_name(unit: &str, package: &str, name: &str) -> String` —
    `name` when `package == unit`, else `"{package[unit.len()+1..]}.{name}"`;
    `debug_assert!` that `package` is `unit` or starts with `"{unit}."`.
  - `impl Package { pub fn catalog_name(&self, shape: &InterfaceShape<'_>) -> String }`
    — `shape.name` for an inline shape,
    `relative_name(&self.unit, &self.name,
    shape.name)` otherwise.
  - `pub fn unit_of(package: &Package) -> &str` — `unit` when non-empty, else
    `name`.
- The checker fills `unit: pkg.unit(db).clone()`.

- [ ] **Step 1: Write the failing tests** in `lib.rs`'s `v2` tests:

```rust
#[test]
fn relative_name_strips_the_unit_prefix() {
    assert_eq!(relative_name("u", "u", "Session"), "Session");
    assert_eq!(relative_name("u", "u.cluster", "Speed"), "cluster.Speed");
    assert_eq!(relative_name("com.example.hmi", "com.example.hmi.cluster.front", "A"), "cluster.front.A");
}

#[test]
fn an_inline_shape_keeps_its_global_name() {
    // Package { unit: "u", name: "u.cluster", services: [service "veh.hvac.cabin" with an inline shape] }
    // assert: catalog_name(&shape) == "veh.hvac.cabin"
}

#[test]
fn unit_of_falls_back_to_the_name() {
    // Package { name: "p", unit: "" } -> "p"; Package { name: "u.a", unit: "u" } -> "u"
}
```

- [ ] **Step 2: Run** `cargo test -p ridl-ir` — expected: FAIL to compile on the
      missing field; add the proto field, run again, expected: FAIL on the
      missing functions.
- [ ] **Step 3: Implement** the three functions and the checker fill.
- [ ] **Step 4: Update the snapshots** that carry IR JSON: every
      `corpus__ir@*.snap` and `corpus__codegen@*.snap` in
      `crates/ridlc/tests/snapshots`, `docs_in_source__docs_fixture_ir.snap`,
      and any `.ir.json` fixture a test compares byte for byte
      (`grep -rl '"retired"' crates/*/tests`). The predicted change is one new
      line `"unit": "<unit>"` per package and nothing else.
- [ ] **Step 5: Run** `just test` — expected: PASS.
- [ ] **Step 6: Gate** (the five common recipes, `just wasm-check`).
- [ ] **Step 7: Commit**:
      `feat(ridl-ir): record the unit of a package in the IR`.

---

### Task 8: Dotted interface keys in `interfaces.lock`

**Files:**

- Modify: `crates/ridl-core/src/interface_lock.rs:14-30` (module doc), `:95-125`
  (`LockKey::from_str`, `is_ident`)
- Test: `interface_lock.rs` inline tests; `crates/ridl/tests/lock_merge.rs`

**Interfaces:**

- Produces: `LockKey::Interface(String)` holds a relative dotted name: one or
  more identifiers (`[A-Za-z][A-Za-z0-9_]*`) joined by `.`. The `service:` form
  is unchanged. `Display` prints the name as held. The refusal message for an
  interface key becomes
  ``"`{text}` is not a lock key: an interface key is a dotted name of identifiers"``.

- [ ] **Step 1: Write the failing tests**:

```rust
#[test]
fn an_interface_key_is_a_dotted_relative_name() {
    assert_eq!(key("cluster.SpeedDisplay"), LockKey::Interface("cluster.SpeedDisplay".into()));
    assert_eq!(key("Session"), LockKey::Interface("Session".into()));
    for bad in ["cluster..Speed", ".Speed", "cluster.", "a-b.Speed"] {
        assert!(bad.parse::<LockKey>().is_err(), "{bad}");
    }
}

#[test]
fn a_dotted_key_round_trips_through_parse_and_render() {
    let lock = parse("next 3\ncluster.Speed 1\nSession 2\n").unwrap();
    assert_eq!(lock.render(), "next 3\ncluster.Speed 1\nSession 2\n");
}
```

And in `lock_merge.rs`: `merge_keeps_a_dotted_key` — base
`next 2\ncluster.Speed 1\n`, ours adds `climate.Climate 2`, theirs unchanged;
expect exit 0 and OURS holding both lines.

- [ ] **Step 2: Run** `cargo test -p ridl-core --lib interface_lock` — expected:
      FAIL on `key("cluster.SpeedDisplay")` (parse error).
- [ ] **Step 3: Implement** the grammar in `from_str`; keep `is_ident` for each
      segment. The existing test that lists invalid keys (around line 883) keeps
      `service:a..b` and `service:a.b.` invalid.
- [ ] **Step 4: Run** `cargo test -p ridl-core` and
      `cargo test -p ridl-cli --test lock_merge --test lock_protocol` —
      expected: PASS.
- [ ] **Step 5: Gate** (the five common recipes, `just wasm-check`).
- [ ] **Step 6: Commit**:
      `feat(ridl-core): accept a dotted relative name as an interface lock key`.

---

### Task 9: Numbering runs once per unit

**Files:**

- Modify: `crates/ridl-sem/src/check.rs:65-120`, `:240-400` (`check_package`,
  `Numbering`, `number_interfaces`, `orphan_entry_message`, `package_dir`)
- Test: `check.rs` inline tests (around line 14759, the `PackageLock` fixture
  helper and `provisional_numbers_follow_byte_order_from_next`);
  `crates/ridlc/tests/corpus.rs` snapshots

**Interfaces:**

- Consumes: `Package.unit`, `Package.lock` (Task 6), `relative_name`, `LockKey`
  (Task 8).
- Produces in `ridl_sem::check`:
  - `pub(crate) fn lower_package(db, ws: Workspace, pkg: Package, std: Package) -> CheckedPackage`
    (`#[salsa::tracked(returns(clone))]`): today's `check_package` body minus
    the identity fold; every shape has `number: 0, provisional: true`, `retired`
    empty, no RIDL-409.
  - `pub fn unit_numbering(db, ws: Workspace, unit: String, std: Package) -> UnitNumbering`
    (`#[salsa::tracked(returns(clone))]`) over `lower_package` of every package
    of `ws` whose `unit` is `unit`.
  - `#[derive(Clone, Debug, PartialEq, Eq)] pub struct UnitNumbering { pub numbers: BTreeMap<LockKey, (u32, bool)>, pub retired: Vec<(LockKey, u32)>, pub orphans: Vec<LockEntry>, pub any_provisional: bool, pub anchor: String }`
    — `numbers` maps every shape key of the unit to (number, provisional);
    `retired` is the lock's retired entries in number order; `anchor` is the
    anchor package name (Decisions).
  - `pub fn check_package(...)` keeps its signature: it calls `lower_package`,
    then `unit_numbering(unit)`, sets each shape's `number`/`provisional` from
    `numbers[key]` where `key` is
    `LockKey::Interface(relative_name(unit,
    pkg, name))` or
    `LockKey::Service(service name)`, fills `retired` with the entries that name
    this package (an interface key whose directory part is this package's
    relative path, under its short name; the `service:` and orphan-package
    entries when this package is the anchor), and pushes one RIDL-409 per orphan
    when this package is the anchor.
  - `orphan_entry_message` names `ridl lock <unit dir>` (rename `package_dir` to
    `unit_dir`; the text "in package" becomes "in unit").

- [ ] **Step 1: Write the failing tests** in `check.rs` tests (extend the lock
      fixture helper to build a two-package unit `u` with `u` holding
      `interface Session {}` and `u.cluster` holding `interface Speed {}`):

```rust
#[test]
fn provisional_numbers_run_over_the_unit_in_key_byte_order() {
    // no lock: keys "Session" < "cluster.Speed" in bytes
    assert_eq!(numbers("u"), [("Session", 1, true)]);
    assert_eq!(numbers("u.cluster"), [("cluster.Speed", 2, true)]);
}

#[test]
fn a_live_entry_freezes_a_subpackage_shape_under_its_relative_key() {
    // lock "next 3\ncluster.Speed 1\nSession 2\n"
    assert_eq!(numbers("u.cluster"), [("cluster.Speed", 1, false)]);
    assert_eq!(numbers("u"), [("Session", 2, false)]);
}

#[test]
fn a_retired_entry_lands_on_the_package_its_relative_name_names() {
    // lock "next 4\ncluster.Speed 1\nSession 2\ncluster.Old 3 retired\n"
    assert_eq!(ir("u.cluster").retired, [RetiredInterface { name: "Old", number: 3 }]);
    assert!(ir("u").retired.is_empty());
}

#[test]
fn a_retired_entry_of_a_missing_package_and_a_service_land_on_the_anchor() {
    // lock "next 5\n... gone.Old 3 retired\nservice:veh.x 4 retired\n", no package u.gone
    // assert: ir("u").retired names "gone.Old" 3 and "service:veh.x" 4; ir("u.cluster").retired is empty
}

#[test]
fn an_orphan_is_ridl_409_once_from_the_anchor_package() {
    // lock "next 3\ncluster.Gone 1\n"
    // assert: codes(check("u")) == ["RIDL-409"], codes(check("u.cluster")) == []
    // assert: the message holds "ridl lock <unit dir>"
}

#[test]
fn a_key_spelled_with_the_unit_prefix_names_nothing() {
    // lock "next 2\nu.cluster.Speed 1\n"
    // assert: RIDL-409 from "u"; numbers("u.cluster") == [("cluster.Speed", 2, true)]
}

#[test]
fn the_same_short_name_in_two_packages_gets_two_numbers() {
    // u.a and u.b each declare `interface Foo {}`; no root package
    // assert: a.Foo = 1, b.Foo = 2; anchor is "u.a"
}

#[test]
fn an_empty_root_still_numbers_its_subpackages() {
    // unit u with packages u.a only; lock beside the manifest "next 2\na.Foo 1\n"
    // assert: numbers("u.a") == [("a.Foo", 1, false)]
}
```

- [ ] **Step 2: Run** `cargo test -p ridl-sem --lib check::tests` — expected:
      FAIL on the first test (`cluster.Speed` is numbered 1, not 2).
- [ ] **Step 3: Implement** the split and the fold as the Interfaces block
      states. Provisional order stays `provisional_order` (byte order of the
      key's name, interface before inline shape).
- [ ] **Step 4: Update the snapshots** whose numbers change:
      `corpus__ir@rsdl-appendix-a.snap`, `corpus__codegen@rsdl-appendix-a.snap`,
      `corpus__rust@rsdl-appendix-a.snap`,
      `corpus__system@rsdl-appendix-a.snap`,
      `corpus__typescript@rsdl-appendix-a.snap` (the appendix's `adas` and
      `diag` packages numbered in one space: every `adas.*` key before every
      `diag.*` key), plus any `ridl-sem` snapshot that carries a number. Read
      `NOTES` of the appendix corpus and update its numbering sentence.
- [ ] **Step 5: Run** `just test` — expected: PASS.
      `crates/ridl/tests/lock_cli.rs` may fail on
      `workspace_output_prefixes_each_package_path` and
      `rename_over_more_than_one_package_exits_two`; that is Task 10's
      deliverable: mark those two
      `#[ignore = "per-unit lock lands in the next task"]` in this task and
      remove the attribute in Task 10.
- [ ] **Step 6: Gate** (the five common recipes, `just wasm-check`).
- [ ] **Step 7: Commit**: `feat(ridl-sem): number interfaces once per unit`,
      footer
      `BREAKING CHANGE: interface numbers are one space per unit and a declared interface's lock key is its source package path relative to the unit plus its name (cluster.Speed). Delete the per-package interfaces.lock files, run ridl lock, and publish a new baseline.`

---

### Task 10: `ridl lock` writes the lock of a unit

**Files:**

- Modify: `crates/ridl/src/lock.rs` (module doc, `LockedPackage` to
  `LockedUnit`, `locked_packages` to `locked_units`, `allocate`, `edit`,
  `write`, `shape_key`, `is_declared`, `is_provisional`)
- Modify: `crates/ridl/src/main.rs:985-1045` (`provisional_number_message`,
  `dropped_number_message`: "in unit")
- Modify: `docs/book/cli-reference.md` (the `ridl lock` entry, as
  `crates/ridl/tests/cli_reference.rs` checks it against `--help`)
- Test: `crates/ridl/tests/lock_cli.rs`

**Interfaces:**

- Consumes: `LoadedWorkspace.units`, `Package.unit`,
  `v2::Package::catalog_name`.
- Produces:
  - `struct LockedUnit { name: String, dir: PathBuf, lock: InterfaceLock, packages: Vec<v2::Package> }`
  - `fn locked_units(entry: &Path, irs: Vec<v2::Package>, scope: Option<&Path>) -> io::Result<Vec<LockedUnit>>`
    — groups the checked IRs by `unit`, takes `dir` from `loaded.units`, keeps
    the units whose `dir` starts with `scope`.
  - `pub(crate) fn shape_key(package: &v2::Package, shape: &v2::InterfaceShape<'_>) -> LockKey`
    — `LockKey::Service(shape.name)` for an inline shape, else
    `LockKey::Interface(package.catalog_name(shape))`.
  - `allocate` walks every provisional shape of every package of the unit,
    sorted by (number, key); the output prefix is the unit directory relative to
    `entry` when the run holds more than one unit.
  - `edit` requires exactly one unit; the exit-2 message says "edit one unit's
    `interfaces.lock`, but `{entry}` holds {n} units; name the unit directory".

- [ ] **Step 1: Write the failing tests** in `lock_cli.rs` (the helpers
      `package_workspace` and `two_member_workspace` exist; add
      `unit_with_subpackage(dir, root_source, sub_source)` writing `ridl.toml`
      name `veh.hmi`, `hmi.ridl`, and `cluster/speed.ridl`):

```rust
#[test]
fn plain_lock_writes_one_file_at_the_unit_root_with_relative_keys() {
    // root: interface Session; cluster: interface Speed
    // expect exit 0; stdout "allocated Session 1\nallocated cluster.Speed 2\n"
    // expect read_lock(root) == "next 3\nSession 1\ncluster.Speed 2\n"
    // expect no root/cluster/interfaces.lock
}

#[test]
fn rename_across_packages_of_one_unit_keeps_the_number() {
    // lock "next 2\ncluster.Speed 1\n"; move the interface to climate/climate.ridl as Climate
    // ridl lock --rename cluster.Speed=climate.Climate -> exit 0,
    // stdout "renamed cluster.Speed climate.Climate 1"
}

#[test]
fn retire_over_a_workspace_of_two_units_exits_two() {
    // two_member_workspace; ridl lock root --retire Hvac -> exit 2, stderr mentions "name the unit directory"
}

#[test]
fn lock_on_a_member_allocates_in_that_unit_only() { /* re-point the existing member test at units */ }

#[test]
fn workspace_output_prefixes_each_unit_directory() { /* the existing prefix test, over two units */ }
```

Remove the two `#[ignore]` attributes Task 9 added and re-target those tests at
units.

- [ ] **Step 2: Run** `cargo test -p ridl-cli --test lock_cli` — expected: FAIL:
      the first test finds `root/cluster/interfaces.lock` written and the root
      lock holding `Speed 1`.
- [ ] **Step 3: Implement** as the Interfaces block states. Update the module
      doc ("writes a unit's `interfaces.lock`") and the `ridl lock` help text
      and its `cli-reference.md` entry (`--rename`/`--retire` "edit one unit's
      file; PATH must resolve to one unit").
- [ ] **Step 4: Run** `cargo test -p ridl-cli` — expected: PASS, including
      `cli_reference`.
- [ ] **Step 5: Gate** (the five common recipes and the four Markdown recipes).
- [ ] **Step 6: Commit**:
      `feat(ridl): write one interfaces.lock per unit with ridl lock`.

---

### Task 11: The reduced unit and the codegen model's `Catalog`

**Files:**

- Modify: `crates/ridl-ir/src/catalog_hash.rs` (module doc, `reachable_decls`,
  `reduced_package` to `reduced_unit`, `catalog_hash`, `Index`)
- Modify: `crates/ridl-ir/src/lib.rs` (module `v2`: `unit_retired`)
- Modify: `crates/ridl-ir/src/codegen/lower.rs:1087-1100` (`catalog`)
- Modify: `crates/ridl-ir/proto/ridl/codegen/v1/model.proto:734-742`
  (`Catalog.package` comment)
- Modify: `crates/ridl-descriptor/src/hash.rs` (the re-export names),
  `crates/ridl-descriptor/src/lower.rs:60-62` (call
  `catalog_hash(&package.unit, others)` — the full signature change of `lower`
  is Task 12), `crates/ridlc/src/lib.rs:1466-1478` (`embed_catalog_hashes`: find
  the region's package by name as today, hash its `unit`)
- Modify: `crates/ridl-backend-rust/src/descriptors.rs:110-145` (`CATALOG.name`
  reads `Catalog.package`, if it reads `Model.name` today)
- Modify: `crates/ridl-rt/src/contract.rs:29-40` (`CatalogRef` doc)
- Test: `catalog_hash.rs` inline tests;
  `crates/ridl-descriptor/tests/golden_hash.rs`;
  `crates/ridlc/tests/snapshots/corpus__{codegen,rust,system}@*.snap`;
  `crates/ridl/tests/snapshots/describe_cli__corpus_catalog.snap`

**Interfaces:**

- Consumes: `Package.unit`, `Package::catalog_name`, `relative_name`.
- Produces:
  - `pub fn reduced_unit(unit: &str, packages: &[&Package]) -> Package` —
    `name: unit, unit: unit`; `interfaces`: every shape of every package with
    `package.unit == unit`, `interface.name = package.catalog_name(&shape)`,
    visibility from the shape, sorted by `(number, name)`; `decls`: the closure
    under full canonical names (`Index::canonical` always returns
    `"{pkg}.{bare}"`, for the unit's own packages too); doc blanking as today.
  - `pub fn catalog_hash(unit: &str, packages: &[&Package]) -> [u8; 32]`.
  - `pub fn reachable_decls<'a>(unit: &str, packages: &[&'a Package]) -> BTreeMap<String, &'a Decl>`.
  - `pub fn unit_retired(unit: &str, packages: &[&Package]) -> Vec<RetiredInterface>`
    in `v2`: every `retired` entry of every package of the unit, an interface
    entry under `relative_name(unit, package.name, entry.name)`, a `service:`
    entry and a dotted entry (an anchor-carried one) as spelled, in number
    order.
  - `Lowering::catalog` returns
    `Catalog { package: unit, hash: catalog_hash(unit, scope.package + scope.others), retired: unit_retired(unit, ...) }`.
  - `Catalog.package` comment: "`CatalogRef.name`: the name of the unit the
    package belongs to (`Package.unit`), which names the one catalog of that
    unit. A plugin finds its region in the deployment section by this value."
  - `CatalogRef` doc: "A catalog: the interfaces of one unit — one package
    manifest and the source packages in its directory tree." and the `name`
    field doc "The unit name."

- [ ] **Step 1: Write the failing tests** in `catalog_hash.rs` tests (the
      fixture helpers `struct_decl`, `signal`, `inline_service` exist):

```rust
#[test]
fn the_reduced_unit_holds_every_package_of_the_unit_under_catalog_names() {
    // packages: u (Session), u.cluster (Speed, reaches struct Pos), unit "u" both; v (unit "v")
    let reduced = reduced_unit("u", &[&u, &cluster, &v]);
    assert_eq!(reduced.name, "u");
    assert_eq!(names(&reduced.interfaces), ["Session", "cluster.Speed"]);  // (number, name) order
    assert_eq!(decl_names(&reduced), ["u.cluster.Pos"]);                   // full canonical name
}

#[test]
fn a_change_in_one_package_moves_the_hash_of_the_unit() {
    // add a field to Pos in u.cluster; assert catalog_hash("u", ..) differs
}

#[test]
fn a_sibling_unit_with_a_prefix_name_is_not_in_the_hash() {
    // units "u" and "u.x" (two units); assert catalog_hash("u", all) == catalog_hash("u", [u's packages only])
}

#[test]
fn the_same_short_name_in_two_packages_gives_two_reduced_declarations() {
    // u.a and u.b each declare struct Foo reached by an interface; assert decl names "u.a.Foo" and "u.b.Foo"
}

#[test]
fn unit_retired_qualifies_an_interface_entry_and_keeps_a_service_entry() {
    // u.cluster.retired = [Old 3]; u.retired = [service:veh.x 4]
    // assert unit_retired("u", ..) == [("cluster.Old", 3), ("service:veh.x", 4)]
}
```

- [ ] **Step 2: Run** `cargo test -p ridl-ir` — expected: FAIL to compile on
      `reduced_unit`; after the signature exists, FAIL on the interface names.
- [ ] **Step 3: Implement** as the Interfaces block states; the existing hash
      tests are rewritten against `reduced_unit` (the bare-name expectations
      become full canonical names).
- [ ] **Step 4: Update the golden hash** in `golden_hash.rs` and the snapshots
      that carry a hash or `Catalog.package`: `corpus__codegen@*.snap`,
      `corpus__rust@*.snap`, `corpus__system@rsdl-appendix-a.snap`,
      `describe_cli__corpus_catalog.snap`. Predicted: every hash byte array
      changes; `"package"` under `catalog` becomes the unit name; nothing else.
- [ ] **Step 5: Run** `just test` and `just demo` — expected: PASS.
- [ ] **Step 6: Gate** (the five common recipes, `just wasm-check`,
      `just compat-check`, `just demo`).
- [ ] **Step 7: Commit**:
      `feat(ridl-ir): hash the reduced unit and name the catalog after the unit`,
      footer
      `BREAKING CHANGE: every catalog hash changes; the reduced package is now the reduced unit and CATALOG.name is the unit name.`

---

### Task 12: One catalog descriptor per unit

**Files:**

- Modify: `crates/ridl-descriptor/src/lower.rs` (`lower`),
  `crates/ridl-descriptor/src/number.rs` (`numbered_shapes` takes the unit's
  packages), `crates/ridl-descriptor/schema/catalog.fbs:74-98` (comments only:
  `Interface.name` "the catalog name: the source package path relative to the
  unit plus the interface name, or the service's full dotted name for an inline
  shape"; `Catalog.name` "the unit name")
- Modify: `crates/ridlc/src/lib.rs:544-548` (the `Emit::Catalog` doc and help
  text), `:2005-2035` (move the `Emit::Catalog` arm out of the per-package
  writer into
  `fn write_catalogs(out_dir: &Path, packages: &[&v2::Package], others: &[&v2::Package]) -> io::Result<()>`
  called once per build after the per-package loop)
- Modify: `docs/book/cli-reference.md:561`, `:1935` (the `catalog` emit help
  line, checked by `cli_reference.rs`)
- Test: `crates/ridl-descriptor/tests/lower.rs`; `crates/ridlc/tests/cli.rs`;
  `crates/ridl/tests/describe_cli.rs`

**Interfaces:**

- Consumes: `catalog_hash(unit, packages)`, `unit_retired`,
  `Package::catalog_name`.
- Produces:
  - `pub fn lower(unit: &str, packages: &[&Package]) -> Result<Vec<u8>, LowerError>`
    — `packages` is every checked package of the build plus `ridl.std` when
    referenced; the unit's packages are those with `unit == unit`; the
    descriptor's `name` is `unit`, its interfaces are every shape of those
    packages under `catalog_name`, in (number, name) order, its `retired` is
    `unit_retired`.
  - `write_catalogs` writes `<unit>.catalog.binfb` for every unit (in name
    order) that has at least one shape, and nothing for a unit with none.
  - `--emit catalog` help: "The catalog descriptor an engine reads, written to
    `<unit>.catalog.binfb` for every unit that declares an interface or a
    service with an inline body: a FlatBuffers file of the unit's interfaces,
    their members and their catalog hash".

- [ ] **Step 1: Write the failing tests**:

In `crates/ridl-descriptor/tests/lower.rs`:

```rust
#[test]
fn a_unit_of_two_packages_lowers_to_one_descriptor_with_qualified_names() {
    // u: Session (number 2); u.cluster: Speed (number 1), retired Old 3
    let bytes = lower("u", &[&u, &cluster]).unwrap();
    // read back: name == "u"; interface names == ["cluster.Speed", "Session"] (number order);
    // retired == [("cluster.Old", 3)]
}
```

In `crates/ridlc/tests/cli.rs`, over the `rsdl-appendix-a` corpus:

```rust
#[test]
fn emit_catalog_writes_one_file_per_unit() {
    // ridlc build --emit catalog --out-dir <tmp> crates/ridlc/tests/corpus/rsdl-appendix-a
    // assert: the only *.catalog.binfb in <tmp> is "veh.catalog.binfb"
    // assert: its interfaces are named "adas.<...>" and "diag.<...>"
}
```

- [ ] **Step 2: Run** them — expected: FAIL (two files, `veh.adas.catalog.binfb`
      and `veh.diag.catalog.binfb`).
- [ ] **Step 3: Implement** as the Interfaces block states. `numbered_shapes`
      becomes
      `pub fn numbered_shapes(unit: &str, packages: &[&Package]) -> Result<Vec<Numbered>, ZeroNumber>`:
      it walks the packages whose `unit == unit` and returns
      `Numbered { name: <catalog name>, number, provisional }` in (number, name)
      order.
- [ ] **Step 4: Update** `describe_cli__corpus_catalog.snap` if the corpus
      package's interface names change (it is a root package: they do not; only
      Task 11's hash changed) and `cli-reference.md`.
- [ ] **Step 5: Run** `just test` — expected: PASS.
- [ ] **Step 6: Gate** (the five common recipes, `just demo`, the four Markdown
      recipes).
- [ ] **Step 7: Commit**:
      `feat(ridl-descriptor): lower one catalog per unit with qualified interface names`,
      footer
      `BREAKING CHANGE: ridl build --emit catalog writes <unit>.catalog.binfb, one file per unit; interface names in the descriptor are qualified by the source package path relative to the unit.`

---

### Task 13: System regions keyed by unit

**Files:**

- Modify: `crates/ridl-sem/src/rsdl/lower.rs:80-107` (`interface_ref`,
  `interface_ir`), `:443-475` (`regions`), `:330-370` (`routes`)
- Modify: `crates/ridl-ir/proto/ridl/ir/v2/system.proto:228-233`
  (`Region.catalog` comment: "The catalog's name — the unit name (vocabulary
  V-16)"; `RegionInterface.name`: "the catalog name")
- Modify: `crates/ridlc/src/lib.rs:1466-1478` (`embed_catalog_hashes`: a
  region's catalog is a unit; find any package with `unit == region.catalog` and
  hash `region.catalog` over `others`)
- Modify: `crates/ridlc/tests/corpus/rsdl-appendix-a/NOTES` (one region, `veh`)
- Test: `lower.rs` inline tests
  (`a_region_is_the_catalog_that_declares_the_interface`,
  `appendix_a_lowers_its_routes_regions_and_grants`, `region_rows`);
  `corpus__system@rsdl-appendix-a.snap`; `crates/ridlc/tests/layout.rs`

**Interfaces:**

- Consumes: `Package.unit`, `Package::catalog_name`, `catalog_hash(unit, ..)`.
- Produces:
  `InterfaceRef { catalog: <unit of the declaring package>, name: <catalog name>, inline }`;
  `interface_ir` finds the shape among the packages whose `unit == catalog` by
  catalog name and `inline`; `Region.catalog` is the unit, one region per unit,
  `RegionInterface.name` and `Route.interface` are catalog names;
  `Grant.regions` are unit names.

- [ ] **Step 1: Write the failing test** in `lower.rs` tests (the fixture
      helpers build a workspace from source strings; give the two interface
      packages one unit):

```rust
#[test]
fn two_source_packages_of_one_unit_share_one_region() {
    // unit "veh": veh.adas (interface Cruise), veh.diag (interface Diag); a system that lists both
    assert_eq!(
        region_rows(&system),
        [("veh", "adas.Cruise", false, 1, true, "..."), ("veh", "diag.Diag", false, 2, true, "...")]
    );
}
```

- [ ] **Step 2: Run** `cargo test -p ridl-sem --lib rsdl::lower::tests` —
      expected: FAIL with two regions `veh.adas` and `veh.diag`.
- [ ] **Step 3: Implement** as the Interfaces block states; rewrite the existing
      region test's expectation to the unit; update `embed_catalog_hashes`.
- [ ] **Step 4: Update** `corpus__system@rsdl-appendix-a.snap` (predicted: one
      region `veh` with the four interfaces under `adas.`/`diag.` names; the
      routes' `catalog` is `veh` and `interface` is the catalog name; the grants
      list `veh`) and the corpus `NOTES`.
- [ ] **Step 5: Run** `just test` — expected: PASS, including `layout.rs`
      (`examples/cabin` is one unit, so its layout is unchanged).
- [ ] **Step 6: Gate**: the full `just build`.
- [ ] **Step 7: Commit**: `feat(ridl-sem): key the system regions on the unit`,
      footer
      `BREAKING CHANGE: a region of the system artifact is one unit; region interface names are catalog names (cluster.Speed), and a plugin finds its region by Catalog.package.`

---

### Task 14: `ridl diff` and the baseline gate group packages by unit

**Files:**

- Modify: `crates/ridl-diff/src/lib.rs:351` (`diff_sets_in`),
  `crates/ridl-diff/src/walk.rs:340-430` (`diff_interfaces`)
- Modify: `crates/ridl/src/main.rs:1006-1031` (`dropped_number`)
- Test: `crates/ridl-diff/src/tests.rs`; `crates/ridl/tests/baseline_gate.rs`;
  `crates/ridl/tests/diff_cli.rs`

**Interfaces:**

- Consumes: `unit_of`.
- Produces:
  - `diff_sets_in` builds, over the new side, an index of frozen numbers per
    unit: `(unit_of(package), number) -> (package name, shape key)`, and the
    union of retired numbers per unit, and passes both to `diff_interfaces`.
  - `diff_interfaces` pass 1 looks a frozen old number up in the old package's
    unit; a hit in another package of the unit is a pair: the change is
    `InterfaceRenamed` with path `{new package}/{new name}`, and the body
    comparison runs on the pair. Pass 3's `sanctioned` reads the unit's retired
    union.
  - `dropped_number` reads the retired union of the fresh packages whose
    `unit_of` equals the published package's `unit_of`.
  - A snapshot with an empty `unit` is its own unit (`unit_of`).

- [ ] **Step 1: Write the failing tests** in `ridl-diff/src/tests.rs`:

```rust
#[test]
fn a_frozen_number_moved_to_a_sibling_package_is_a_rename() {
    // old: u.cluster { Speed number 1 frozen }; new: u.climate { Speed number 1 frozen }, u.cluster empty
    let report = diff_sets(&[old_cluster], &[new_cluster, new_climate]);
    assert_eq!(categories(&report), [Category::InterfaceRenamed]);
    assert_eq!(report.changes[0].path, "u.climate/Speed");
    assert_eq!(report.verdict, Verdict::Compatible);
}

#[test]
fn a_retired_number_carried_by_the_anchor_sanctions_a_removal_in_a_sibling() {
    // old: u.cluster { Old number 3 frozen }; new: u.cluster without Old, u.retired = [("cluster.Old", 3)]
    assert_eq!(categories(&report), [Category::InterfaceRetired]);
}

#[test]
fn a_snapshot_without_a_unit_is_its_own_unit() {
    // old: { name "p", unit "" , Foo 1 frozen }; new: { name "p", unit "p", Foo 1 frozen }
    assert_eq!(report.verdict, Verdict::Identical);
}
```

In `baseline_gate.rs`: `a_number_moved_within_the_unit_is_not_ridl_412` —
publish a baseline with `cluster.Speed 1`, move the interface to `climate` with
`ridl lock --rename`, run `ridl baseline` again: exit 0.

- [ ] **Step 2: Run** `cargo test -p ridl-diff` — expected: FAIL: the first test
      reports `DeclRemoved` and `DeclAdded`.
- [ ] **Step 3: Implement** as the Interfaces block states; `diff_packages` (one
      pair, no context) keeps its behaviour by building the index over the one
      new package.
- [ ] **Step 4: Run** `just test` — expected: PASS (`diff_cli.rs`,
      `diff_gate.rs`, `diff_member_reorder.rs`, `baseline_desk.rs` included).
- [ ] **Step 5: Gate** (the five common recipes).
- [ ] **Step 6: Commit**:
      `feat(ridl-diff): match interface numbers within a unit`.

---

### Task 15: Amend the records

**Files:**

- Modify: `docs/decisions/ADR-0002-module-system.md` §1 (a unit holds a tree of
  source packages; a source package belongs to one unit, MANI-014) and §4 (a
  `ridl.toml` below a unit is MANI-013, not a separate package root)
- Modify: `docs/decisions/ADR-0014-*.md` decision 15 (the reduced unit: the
  input, the interface names, the full canonical names) with a dated amendment
  line in `## Status`
- Modify: `docs/decisions/ADR-0015-*.md:46,272,417,448,479,511` and
  `docs/decisions/ADR-0016-*.md:397` ("its package's `interfaces.lock`" to "its
  unit's `interfaces.lock`"), each with a dated amendment line in `## Status`
- Modify: `docs/decisions/ADR-0022-*.md` decisions 6 and 7 ("the catalog of the
  package that declares it" to "the catalog of the unit that declares it"), with
  a dated amendment line
- Modify: `docs/design/catalog-descriptor.md` ("The artifact", "What a catalog
  contains", "The catalog hash", and the D-1, D-4, D-10 rows)
- Modify: `docs/design/interaction-face.md` ("The catalog check": the name is
  the unit name; line 83's "reduced package")
- Modify: `docs/design/codegen-plugins.md` ("The deployment section": a plugin
  finds its region by `Catalog.package`; region interface names are catalog
  names while `Model.interfaces[i].name` stays short)
- Modify: `docs/specification/ridl-language-reference.md` §11 (the lock is per
  unit, in the manifest directory; the key grammar; the number scope) and line
  1933 ("a per-package artifact" to "a per-unit artifact")
- Modify: `docs/specification/rsdl-language-reference.md` §13 and V-16 (a
  catalog is a unit's interfaces; "its package name" to "its unit name"), lines
  65, 97, 698, 727
- Modify: `docs/specification/ridl-family-overview.md` (its footer's list of
  sections to update when a reference changes: confirm nothing else is owed)
- Test: `just link-check`, `just doc-path-check`, `just story-id-check`,
  `just book-check`, `just check`

- [ ] **Step 1: Check the gate before editing**: `just story-id-check` passes on
      the branch (the baseline).
- [ ] **Step 2: Edit each record** at the lines the file map names. State a fact
      once in the record that owns it (ADR-0002 §1 owns the unit; ridl §11 owns
      the lock; ADR-0014 decision 15 owns the hash) and link to it from the
      others. Every amended ADR gets one line under `## Status`:
      `Amended 2026-10-08 — decision N: one sentence`.
- [ ] **Step 3: Run**
      `grep -rn "per package\|its package name\|package's interfaces.lock" docs/decisions docs/design docs/specification`
      and confirm every remaining match is about a baseline snapshot, a
      generated module, a codegen request or a plugin run (which stay per
      package), not about a catalog, a lock, a number or a region.
- [ ] **Step 4: Gate**: `just check`, `just link-check`, `just doc-path-check`,
      `just story-id-check`, `just book-check`.
- [ ] **Step 5: Commit**:
      `docs(adr): record one catalog per unit in the decision records` and
      `docs(ridl): state the unit in the language references and design records`.

---

### Task 16: Amend the book

**Files:**

- Modify: `docs/book/rsdl.md:228-250` (regions and routes are per unit; region
  interface names are catalog names)
- Modify: `docs/book/getting-started.md:794-817` (the `catalog` row:
  `<unit>.catalog.binfb`, one per unit; the sentence on `ridl.std`), `:882`
- Modify: `docs/book/codegen-plugins.md:301,327` (the hash is over the unit; the
  catalog is the unit's)
- Modify: `docs/book/cli-reference.md` (the `ridl lock`, `ridl baseline` and
  `ridl diff` entries: the lock per unit and its key form; baselines per
  package, compared per unit; the catalog emit line, if Task 12 did not already
  change both copies)
- Test: `crates/ridl/tests/book_examples.rs`,
  `crates/ridl/tests/cli_reference.rs`, `just book-check`

- [ ] **Step 1: Edit** the chapters. A `ridl`/`rsdl` fence added or changed must
      compile under the harness (declare its own `package`; the book is one
      workspace with one `system`).
- [ ] **Step 2: Run**
      `cargo test -p ridl-cli --test book_examples --test cli_reference` —
      expected: PASS.
- [ ] **Step 3: Gate**: `just check`, `just link-check`, `just doc-path-check`,
      `just story-id-check`, `just book-check`.
- [ ] **Step 4: Commit**:
      `docs(docs): describe one catalog per unit in the book`.

---

### Task 17: The Kotlin heads-up draft and the release note

**Files:**

- Create: `docs/wip/catalog-per-unit-handoff.md`
- Test: `just check`, `just link-check`, `just story-id-check`

The driver posts the issue and runs the release; this task only writes the
texts.

- [ ] **Step 1: Write the handoff file** with three sections:
  1. **Kotlin plugin heads-up (issue draft)** — title "One catalog per unit:
     region lookup by `Catalog.package`, catalog names in regions". Body: the
     two changes of spec decision 2 (find the region in the deployment section
     by `Catalog.package`, not `Model.name`; region interface names are catalog
     names such as `cluster.SpeedDisplay` while `Model.interfaces[i].name` stays
     short), the unchanged facts (generated packages stay per source package;
     `Catalog.package` keeps its name and number; `Catalog.retired` now carries
     the unit's whole list under catalog names; `NUMBER` is the unit's number),
     the breaking effects (every hash changes; numbers are reassigned), and the
     ridl version that ships it (left as `<version>`).
  2. **Release note** — the consolidated `BREAKING CHANGE` text for the
     changelog: one catalog per unit (`<unit>.catalog.binfb`); one
     `interfaces.lock` per unit beside `ridl.toml`, keys qualified by the source
     package path relative to the unit; numbers reassigned per unit; every
     catalog hash changes; regions per unit; MANI-013 and MANI-014. The
     migration in three steps, verbatim from the spec: delete the per-package
     `interfaces.lock` files, run `ridl lock`, publish a new baseline with
     `ridl baseline`. One sentence for the vss corpus and the book harness.
  3. **Gardening pointers** — the records Task 15 and Task 16 amended, for the
     `sdd-gardening` pass that archives the spec and this plan.
- [ ] **Step 2: Run** `prim fmt docs/wip/catalog-per-unit-handoff.md`, then
      `just check`, `just link-check`, `just story-id-check`.
- [ ] **Step 3: Run the full gate** `just build` on the branch head.
- [ ] **Step 4: Commit**:
      `docs(docs): draft the Kotlin heads-up and the release note for one catalog per unit`.

---

## Self-review notes

- Spec coverage: rules 1 (Task 1, by construction of the loader), 2 (Task 4), 3
  (Task 5), 4 (Task 12: a unit with no shape writes no catalog; codegen
  unchanged), 5 and 7 (unchanged code, stated in Task 15's records), 6
  (unchanged); the catalog file, name, interface names, inline names, numbers
  and hash (Tasks 11, 12); the lock (Tasks 6, 8, 9, 10); the IR unit and the
  baselines (Tasks 7, 14); single-file mode (Task 1); the runtime identity (Task
  11); the migration and release note (Task 17); the impact table rows for
  `ridl-backend-rust` (Task 11), `ridl-rt` (Task 11), the system artifact (Task
  13), plugins (Tasks 11, 17), `ridl-mcp`/`ridl-lsp` (Tasks 1 and 4: both render
  loader diagnostics and load the synthetic package through the same code); the
  fixtures (Tasks 3, 9, 12, 13 and the MCP fixtures, which hold a type-only
  subpackage and so change nothing but their IR `unit`); the records (Tasks 15,
  16).
- Type consistency: `Package.unit` (ridl-core input) and `v2::Package.unit` (IR)
  are two fields with one value; `relative_name`, `catalog_name`, `unit_of`,
  `unit_retired` live in `ridl_ir::v2`; `catalog_hash(unit,
  packages)` and
  `reduced_unit(unit, packages)` in `ridl_ir::catalog_hash`;
  `lower(unit, packages)` in `ridl_descriptor`; `shape_key(package, shape)` in
  `ridl::lock`.

## Answers from the author (2026-10-08)

1. **A stale `interfaces.lock` in a subdirectory** draws a warning (Task 6).
2. **The anchor package** is confirmed: the root source package, or the first
   source package by name when the root is empty. It carries retired `service:`
   entries, retired entries of a source package that no longer exists, and
   RIDL-409 positions that are not on a lock line. It is needed because
   baselines stay per source package (spec decision 1) and every retired number
   must be in some snapshot.
3. **A moved interface in `ridl diff`** is reported as `InterfaceRenamed`. No
   new category.
