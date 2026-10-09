# Review residuals of #777 (issue #778) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close driftsys/ridl#778: pin the test gaps the branch review of #777
left open, fix the doc statements it found false, and take the small code
follow-ups it listed.

**Architecture:** No new component. Each task touches one crate (or one doc
area), adds the tests the review named, and makes the smallest code change the
issue asks for. A test that pins existing behaviour is proven by a named
semantic mutation: apply the mutation, the test fails; restore, it passes.

**Tech Stack:** Rust workspace, `cargo test -p <crate>`, insta snapshots in
`crates/ridlc`, `just` gate.

**Spec:** <https://github.com/driftsys/ridl/issues/778> (the issue text is the
spec). Background: the review ledgers
`~/.claude/review-ledgers/driftsys-ridl/pr777-pass{1,2}.md` on the driver's
machine, and `docs/design/catalog-descriptor.md` for the as-built
one-catalog-per-unit behaviour.

## Global Constraints

- Branch `fix/778-review-residuals`, worktree `../ridl-778`, one pull request,
  one commit per task.
- Conventional Commits; the scope is the crate name (`ridl`, `ridl-sem`,
  `ridl-ir`, `ridl-descriptor`, `ridl-core`, `ridlc`, `xtask`) or `docs`. Every
  commit ends with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- Prose (comments, tests' doc comments, commit messages) is plain, literal
  English. No story or stage ids in shipped files (`just story-id-check`).
- A test added to pin existing behaviour must be shown to fail under the
  mutation its step names, then pass with the mutation reverted. The implementer
  reports both runs.
- Do not change which `provisional` numbers `ridl lock` allocates (Task 3 pins
  the rule the code has; it does not change the rule).
- `just build` must be green at the end of Task 10 (the last code task); each
  task runs the crate's own tests.

## Decisions taken by this plan (report to Sebastien)

- D-1 A member listed twice in `[workspace] members` is loaded once with no
  diagnostic (Task 6). A warning would be a new code under ADR-0024's lint
  process, which this plan does not open.
- D-2 The codegen model's `catalog()` keeps rebuilding the unit hash per
  package. `codegen::lower(package, others)` is the per-package API every
  backend and plugin reads (ADR-0020 decision 9); a cross-package cache would
  change that API for a cost that is linear in the unit's size per package.
  Recorded on the issue at close, not done.
- D-3 "Deleting a whole unit's last package without retiring its numbers is not
  refused" is a change to the baseline gate's rule (lock design §7, §8) and is
  left on the issue for Sebastien, not done here.
- D-4 The `rename_labels` hint for a payload written bare in the baseline and
  qualified in the new package is done by comparing canonical type names (Task
  2), with the canonicalization the catalog hash already uses.

## Review Focus

1. A legacy (unit-less) snapshot of a subpackage interface whose number is lost:
   the RIDL-412 message must spell the key relative to the unit
   (`cluster.Speed`), because that is the line the user restores in
   `interfaces.lock` (Task 1).
2. A package moved to a different unit between two baselines: the gate must
   compare it in the unit the baseline recorded, else a dropped number passes
   (Task 1).
3. A workspace whose root package has no interface and whose only interfaces are
   in a subpackage: the RIDL-409 rename hint must still appear (Task 2).
4. A duplicate `members` entry: the package must not be loaded twice, which
   doubles every diagnostic (Task 6).
5. The codegen snapshot of a multi-package unit in the corpus: the hash must be
   the one `ridl build` writes, so a snapshot change means a real hash change
   (Task 9).

---

### Task 1: Baseline-gate pins in `crates/ridl/tests/baseline_gate.rs`

**Files:**

- Modify: `crates/ridl/tests/baseline_gate.rs` (near
  `the_migration_to_one_lock_per_unit_republishes_a_legacy_baseline`, ~1366, and
  the helpers `legacy_per_package_baseline` and `migrate`)
- Read: `crates/ridl/src/main.rs` `published_unit` (~1011), `dropped_number`
  (~1029), `dropped_number_message` (~1064)

**Interfaces:**

- Consumes: the existing fixtures `SESSION`,
  `legacy_per_package_baseline(dir, root_source, lock_text, edits)`,
  `migrate(root) -> (lock, code, stderr)` in the same file;
  `ridl_ir::v2::relative_name`, `lock::shape_key`.
- Produces: nothing other tasks use.

- [ ] **Step 1: Write
      `a_lost_subpackage_number_of_a_legacy_snapshot_is_named_relative_to_the_unit`**

Build on
`a_legacy_number_the_new_numbering_does_not_reach_is_republished_from_an_empty_baseline`:
the lost number belongs to the subpackage interface (`veh.hmi.cluster` /
`Speed`), the snapshot has no `unit`. Assert exit 1, and that stderr contains
exactly
``"`cluster.Speed` holds interface number <N> in the baseline being replaced, in unit `veh.hmi`"``
and ``"Restore the line `cluster.Speed <N>`"``, with `<N>` the number the
fixture gives it.

- [ ] **Step 2: Write
      `a_lost_inline_service_number_of_a_legacy_snapshot_keeps_its_service_key`**

The lost number belongs to an inline `service zone` of the root package (an
inline shape; key `service:zone` per `lock::shape_key`). Assert exit 1 and that
stderr contains ``"`service:zone` holds interface number"`` and
``"Restore the line `service:zone <N>`"``.

- [ ] **Step 3: Prove both tests by mutation**

Run `cargo test -p ridl-cli --test baseline_gate` three times with these
mutations of `dropped_number_message` applied one at a time, then reverted: (a)
always use `lock::shape_key(package, shape)`; (b) pass
`ridl_ir::v2::unit_of(package)` as `unit` at the call site; (c) spell the inline
key without the `service:` prefix. Expected: at least one of the two new tests
fails under each mutation; all pass with the code restored.

- [ ] **Step 4: Write
      `a_package_moved_to_another_unit_is_compared_in_the_unit_the_baseline_recorded`**

Baseline: unit `veh.hmi` with root `Session 1` and subpackage `veh.hmi.cluster`
holding `cluster.Speed 2`, snapshots carrying `"unit": "veh.hmi"`. Fresh
workspace: `veh.hmi.cluster` promoted to its own workspace member (own
`ridl.toml`, own lock) declaring two interfaces so that `Speed` is numbered 2 in
its new unit, while unit `veh.hmi` no longer holds number 2. Assert: exit 1 and
stderr contains `RIDL-412` and ``"in unit `veh.hmi`"``.

- [ ] **Step 5: Prove it by mutation**

Delete the `package.unit.is_empty() &&` guard of `published_unit`; run the test;
expected: FAIL (exit 0, no RIDL-412). Restore; expected: PASS. If no fixture
flips under this mutation, change the fixture until one does; do not weaken the
assertion.

- [ ] **Step 6: Write
      `a_migration_of_a_mixed_baseline_reads_each_snapshot_in_its_own_unit`**

Same shape as
`the_migration_to_one_lock_per_unit_republishes_a_legacy_baseline`, but only the
subpackage snapshot is legacy (no `unit`), while the root snapshot already
carries `"unit": "veh.hmi"`. Assert exit 0, no RIDL-412, and the republished
subpackage snapshot carries `"unit": "veh.hmi"`.

- [ ] **Step 7: Run the file and commit**

Run: `cargo test -p ridl-cli --test baseline_gate`. Expected: all pass.

```bash
git add crates/ridl/tests/baseline_gate.rs
git commit -m "test(ridl): pin the legacy-snapshot RIDL-412 key and the recorded unit"
```

---

### Task 2: `rename_labels` finds the unit and compares canonical shapes

**Files:**

- Modify: `crates/ridl/src/main.rs` `rename_labels` (~1280-1340) and
  `same_shape` (~1365)
- Modify: `crates/ridl/tests/lock_protocol.rs` (near
  `one_same_shape_candidate_adds_the_rename_label`, ~229)
- Read: `crates/ridl-core/src/workspace.rs` `units` map (~598), `DeclIndex`
  (`crates/ridl-sem` or `ridl-core`, find `fn package_of_dir`),
  `crates/ridl-ir/src/catalog_hash.rs` `canonicalize`

**Interfaces:**

- Consumes: `DeclIndex::package_of_dir(&str) -> Option<String>`; the loader's
  `units: BTreeMap<String, PathBuf>`.
- Produces: `DeclIndex::unit_of_dir(&self, dir: &str) -> Option<&str>` (or the
  existing accessor if one maps a manifest directory to its unit; search before
  adding), used only by `rename_labels`.

- [ ] **Step 1: Write
      `the_rename_hint_appears_when_the_unit_root_declares_no_interface`**

In `lock_protocol.rs`: a unit whose root package `veh.hmi` declares only a type,
and whose subpackage `veh.hmi.cluster` declares `interface Speed`; lock and
baseline it; rename `Speed` to `Velocity` in the subpackage; run `ridl check`.
Assert the RIDL-409 diagnostic carries the label naming
`ridl lock <unit dir> --rename cluster.Speed=cluster.Velocity`. Run: expected
FAIL (no label).

- [ ] **Step 2: Write `the_rename_hint_searches_every_package_of_the_unit`**

A unit of two packages where the orphaned entry's shape was in the root and the
same-shape candidate is declared in the subpackage. Assert the label is present.
Run; expected FAIL under the mutation of Step 4 only, so write it now and prove
it there.

- [ ] **Step 3: Write
      `the_rename_hint_matches_a_bare_payload_against_its_qualified_spelling`**

The baseline interface's signal payload is written bare (`Level`, declared in
the same package); the renamed interface in another package of the unit names it
qualified (`veh.hmi.Level`). Assert the label is present. Run: expected FAIL.

- [ ] **Step 4: Implement**

In `rename_labels`, resolve the unit from the lock directory through
`index.unit_of_dir(&dir)` (build the accessor from the loader's `units` map if
none exists), and drop the
`baseline.iter().find(|candidate| candidate.name == package)` lookup. In
`same_shape`, canonicalize every type-reference string of each `Decl` against
its own package before comparing: a bare name becomes `<package>.<name>`; use
the canonicalization `catalog_hash` already has (export it from `ridl_ir` if it
is private). `same_shape` takes the owning package of each interface; update its
two call sites. Update the comment above the lookup (~1296-1298).

Prove Step 2 by mutation: restrict the candidate search to the lock directory's
package; expected FAIL; restore; PASS.

- [ ] **Step 5: Run and commit**

Run: `cargo test -p ridl-cli --test lock_protocol && cargo test -p ridl-ir`.
Expected: all pass.

```bash
git add crates/ridl/src/main.rs crates/ridl/tests/lock_protocol.rs crates/ridl-ir/src crates/ridl-core/src
git commit -m "fix(ridl): find the rename hint's unit from the lock directory and compare canonical payloads"
```

---

### Task 3: Byte-order tests that tell the rule apart, and `unit_numbering` by reference

**Files:**

- Modify: `crates/ridl-sem/src/check.rs`
  `provisional_numbers_follow_key_byte_order_not_package_order` (~15384) and its
  doc comment; `unit_numbering` (~409) and its callers; the comment at ~471
- Modify: `crates/ridl/tests/lock_cli.rs`
  `plain_lock_allocates_in_key_byte_order_over_the_unit` (~676)

**Interfaces:**

- Consumes: `provisional_order(&LockKey) -> (&str, bool)` (~528): the rule is
  byte order of the name, the interface before the inline shape of the same
  name. The `service:` prefix is not part of the order.
- Produces: `unit_numbering` as `#[salsa::tracked(returns(ref))]`; callers
  borrow.

- [ ] **Step 1: Rename the root's inline service to `alpha` in both tests**

Expected numbering under the name rule: `service:alpha 1`, `cluster.Speed 2`
(`alpha` < `cluster.Speed` by name; by whole key `service:alpha` would sort
after). Assert exactly that in both tests, and reword both doc comments to state
the name rule and why `alpha` separates it from the whole-key rule.

- [ ] **Step 2: Prove by mutation**

Replace the sort with `keys.sort_by_key(|k| k.to_string())`; run
`cargo test -p ridl-sem provisional_numbers && cargo test -p ridl-cli --test lock_cli byte_order`;
expected: both new assertions FAIL. Restore; PASS.

- [ ] **Step 3: `unit_numbering` returns a reference**

Change `returns(clone)` to `returns(ref)` and adjust every caller (grep
`unit_numbering(`) to borrow. Run `cargo test -p ridl-sem` and
`cargo clippy -p ridl-sem --all-targets -- -D warnings`.

- [ ] **Step 4: Commit**

```bash
git add crates/ridl-sem/src/check.rs crates/ridl/tests/lock_cli.rs
git commit -m "test(ridl-sem): pin provisional numbering by name order and return the unit numbering by reference"
```

---

### Task 4: `unit_retired` dedupe pinned, `ir.proto` comment

**Files:**

- Modify: `crates/ridl-ir/src/lib.rs` test
  `unit_retired_reads_a_package_named_twice_once` (~2031)
- Modify: `crates/ridl-ir/proto/ridl/ir/v2/ir.proto` `Package.retired` comment
  (~47-56)

- [ ] **Step 1: Strengthen the dedupe test**

Two distinct `Package` values named `u.cluster`, the first retiring
`cluster.Old 3`, the second retiring `cluster.Older 4`. Assert
`unit_retired("u", &[&first, &second])` equals `[cluster.Old 3]` only. Mutation:
remove the `seen` filter; expected FAIL (two entries). Mutation: keep the last
package of a name; expected FAIL (`cluster.Older 4`). Restore; PASS.

- [ ] **Step 2: Fix the proto comment**

State what `check_package` does (`crates/ridl-sem/src/check.rs` ~323-345): the
anchor package carries every root-owned entry, every `service:` entry, and every
entry whose owning package is outside the unit; match the `unit_retired` rustdoc
in `lib.rs`. Regenerate nothing: the comment is in the `.proto` only; run
`cargo build -p ridl-ir` to confirm the build script is content.

- [ ] **Step 3: Run and commit**

Run: `cargo test -p ridl-ir`. Expected: pass.

```bash
git add crates/ridl-ir/src/lib.rs crates/ridl-ir/proto/ridl/ir/v2/ir.proto
git commit -m "test(ridl-ir): pin unit_retired's first-package rule and state what the anchor carries"
```

---

### Task 5: Descriptor lowering sizes each shape in its package, once per package

**Files:**

- Modify: `crates/ridl-descriptor/src/lower.rs` `lower` (~62-95)
- Modify: `crates/ridl-descriptor/tests/lower.rs`
  `a_subpackage_payload_is_sized_in_its_own_package` (~940)
- Read: `crates/ridl-descriptor/src/number.rs` `numbered_shapes`, `unit_shapes`

- [ ] **Step 1: Strengthen the payload test**

Give the root package `u` its own `Point` (a struct of a different member list,
e.g. one `integer` field) and a root interface `Session` whose signal payload is
`Point`. Assert the root row sizes to the root's `Point` and the subpackage row
to the subpackage's `Point`. Mutation:
`Ctx::new(packages[packages.len() - 1], packages)` for every shape; expected
FAIL on the root row. Mutation: `Ctx::new(packages[0], packages)`; expected FAIL
on the subpackage row. Restore; PASS.

- [ ] **Step 2: One `Ctx` per package, one `unit_shapes` walk**

In `lower`, build the shape list once with `unit_shapes`, validate numbers from
that list (so `numbered_shapes` is not called by `lower`; keep `numbered_shapes`
if another caller uses it, else remove it), and build
`Ctx::new(package, packages)` once per distinct package name, reused across the
package's shapes.

- [ ] **Step 3: Run and commit**

Run:
`cargo test -p ridl-descriptor && cargo clippy -p ridl-descriptor --all-targets -- -D warnings`.
Expected: pass.

```bash
git add crates/ridl-descriptor
git commit -m "fix(ridl-descriptor): size each shape in its own package with one context per package"
```

---

### Task 6: Workspace loader: duplicate member, MANI-014 message, manifest span, lock docs

**Files:**

- Modify: `crates/ridl-core/src/workspace.rs` member loop (~578-605),
  `package_name_range` (~985, remove), tests (~2899-2990)
- Modify: `crates/ridl-core/src/interface_lock.rs` module doc (1-5) and
  `FILE_NAME` doc
- Modify: `docs/archive/README.md` entry at ~498-506
- Read: `crates/ridl-core/src/manifest.rs` (find the parsed `[package] name`
  span, `parse_manifest`)

- [ ] **Step 1: Pin the MANI-014 message order**

In `a_root_package_already_claimed_by_a_sibling_tree_is_mani_014` and
`two_members_with_one_name`, assert the message contains
``"already declared by the unit in `<first dir>`; the unit in `<second dir>` declares it too"``
with the concrete directories, so the two arguments cannot be swapped. Mutation:
swap them; expected FAIL; restore; PASS.

- [ ] **Step 2: Write `a_member_listed_twice_is_loaded_once`**

`members = ["a", "a"]`, `a/ridl.toml` naming package `x`, `a/x.ridl` with one
undocumented interface. Assert no diagnostic has code `MANI-014`, exactly one
package named `x` is loaded, and no diagnostic appears twice (compare the
diagnostics vector to its dedup). Run: expected FAIL (package loaded twice).

- [ ] **Step 3: Implement**

When `self.units.get(&name)` is `Some(first_dir)` and
`*first_dir == member_dir`, return `Ok(())` without loading (decision D-1);
extend the comment above and the `load_package_tree` doc.

- [ ] **Step 4: Replace `package_name_range`**

Use the span the manifest parser already records for `[package] name` (add it to
the parsed manifest if `parse_manifest` does not keep it), drop the hand-written
scanner and the manifest re-read on the MANI-014 path. The two MANI-014 tests
already assert the range covers `"com.example.hmi"` / `"x"`.

- [ ] **Step 5: Fix the two lock docs and the archive note**

`interface_lock.rs`: the lock lives in the unit's manifest directory beside
`ridl.toml`, or, for a bare source file compiled without a manifest, in the
file's directory. `docs/archive/README.md`: say the design, the plan and the
execution handoff are archived verbatim, and that `catalog-per-unit-handoff.md`
was edited after archiving to add the release note's migration step.

- [ ] **Step 6: Run and commit**

Run:
`cargo test -p ridl-core && cargo clippy -p ridl-core --all-targets -- -D warnings && just link-check`.
Expected: pass.

```bash
git add crates/ridl-core docs/archive/README.md
git commit -m "fix(ridl-core): load a member listed twice once and take the manifest name span from the parser"
```

---

### Task 7: `load_package_tree` takes a parameter struct

**Files:**

- Modify: `crates/ridl-core/src/workspace.rs` `load_package_tree` (~631) and its
  call sites

- [ ] **Step 1: Introduce
      `struct TreeScope<'a> { unit_dir: &'a Path, unit: &'a str, imports: &'a BTreeMap<String, String>, defaults: &'a TimingDefaults, lock: &'a Option<PackageLock> }`**

`load_package_tree(&mut self, db, dir, name, scope: &TreeScope<'_>)`; drop the
`#[allow(clippy::too_many_arguments)]`. Behaviour unchanged.

- [ ] **Step 2: Run and commit**

Run:
`cargo test -p ridl-core && cargo clippy -p ridl-core --all-targets -- -D warnings`.
Expected: pass, same test count as before.

```bash
git add crates/ridl-core/src/workspace.rs
git commit -m "refactor(ridl-core): pass the package tree's unit scope as one struct"
```

---

### Task 9: The corpus harness lowers each codegen model over its unit

**Files:**

- Modify: `crates/ridlc/tests/corpus.rs` (~264-270)
- Modify: the `codegen` snapshots under `crates/ridlc/tests/snapshots/` that
  change
- Read: `crates/ridlc/src/lib.rs`, the call site of `codegen_request` (how
  `others` is built for a build)

- [ ] **Step 1: Build `others` the way `ridl build` does**

For each package, `others` is every other checked package of the entry (the same
scope the build passes). Mirror the call site; do not invent a narrower scope.

- [ ] **Step 2: Review the snapshot change**

Run `cargo insta test -p ridlc --test corpus` (or `cargo test` with
`INSTA_UPDATE=new` then review). Expected: only `catalog.hash` and
`catalog.retired` fields of multi-package entries (rsdl-appendix-a) change; the
hash of a one-package entry is unchanged. Accept with `cargo insta accept` once
that holds; a change outside those fields is a defect to report, not accept.

- [ ] **Step 3: Commit**

```bash
git add crates/ridlc/tests
git commit -m "test(ridlc): lower each corpus codegen model over its unit"
```

---

### Task 10: A test runs `calibrate derive` over the real calibration records

**Files:**

- Modify: `xtask/tests/calibrate_cli.rs` (or the tests module of
  `xtask/src/calibrate.rs`, whichever already reaches `run_at`)
- Read: `xtask/src/calibrate.rs` `run_at`, `Action::Derive { write }`, what
  `--write` touches; `evals/calibration/`

- [ ] **Step 1: Write `derive_over_the_real_records_matches_what_is_committed`**

Copy `evals/calibration/` (and whatever else `--write` touches, found by reading
`run_at`) into a temp dir laid out as the repo root expects; run derive with
`--write` there; assert the run succeeds and every written file is byte-equal to
the committed one. Mutation: change one derived threshold in the committed file
inside the copy before comparing; expected FAIL. Restore; PASS.

- [ ] **Step 2: Run and commit**

Run: `cargo test -p xtask --test calibrate_cli`. Expected: pass.

```bash
git add xtask/tests/calibrate_cli.rs
git commit -m "test(xtask): derive the calibration thresholds from the real records in the suite"
```

---

### Task 11: Gate and hand-off

- [ ] **Step 1: Run `just build` in `../ridl-778`.** Expected: green. Fix
      anything it names in a `fix(<scope>)` commit.
- [ ] **Step 2: Garden** with the `sdd-gardening` skill: move this plan to
      `docs/archive/`, add its `docs/archive/README.md` entry.
- [ ] **Step 3: Open the pull request** (`Closes #778` is wrong: D-2 and D-3
      stay open; write `Refs #778`), run the branch-level review (`/review`
      passes 1 and 2), fix what survives, and report.
