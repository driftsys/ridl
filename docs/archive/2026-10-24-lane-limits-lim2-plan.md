# Payload limits in the IR and in `ridl diff` (LIM2, #1058) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** record each interaction's declared payload limit in the IR, so the
catalog hash covers it, and report every limit change as breaking in
`ridl diff`.

**Architecture:** the IR's interaction `Decl` gains `optional uint32 limit`. The
checker in `ridl-sem` cannot see the manifest, so `ridlc` sets the field in a
pass over the checked IR right after `check_unit_limits`, reusing the resolution
of `design_lints/limits.rs` (Task 1). `ridl-diff` compares the field on any pair
of interactions and reports a new category, `limit_changed`, as breaking (Task
2). The descriptor column is LIM3's (driver R-16).

**Tech Stack:** Rust, prost and pbjson over
`crates/ridl-ir/proto/ridl/ir/v2/ir.proto`.

**Spec:** `docs/decisions/ADR-0030-transfer-and-bound-limits.md` decision 13,
and `docs/wip/2026-10-24-lane-limits-driver.md` (rulings R-8, R-12, R-16).

## Global Constraints

- The field is set only when a `[limits]` entry gives the interaction its limit
  (`resolve` returns `Some`). A unit with no `[limits]` table, or an interaction
  no entry reaches, has `limit` unset, and its catalog hash is unchanged.
- The key and resolution rules are LIM1's (R-8, R-12): reuse `resolve`,
  `kind_of`, `short_name` and the unit-wide short-name ambiguity logic of
  `crates/ridlc/src/design_lints/limits.rs`; do not reimplement them.
- Every limit change is breaking: added, removed, raised or lowered.
- No change to `ridl-descriptor`, `ridl-transport-ws` or the codegen `Model`.
- `freeze-check` must pass with no approved break.
- Plain, literal English. No story ids in code.

## Review Focus

1. A unit with no `[limits]` table keeps the exact catalog hash it had before
   this change (the pinned corpus hash). Test in Task 1.
2. An interaction whose short name is ambiguous in its unit (R-8) gets the kind
   or default limit, not a name entry. Test in Task 1.
3. A payload that draws RIDL-151 still gets its limit recorded, or the build
   stops on the error first; whichever holds, a test states it. Test in Task 1.
4. `ridl diff` between two snapshots where only the limit changed reports one
   `limit_changed`, breaking, and nothing else. Test in Task 2.
5. A limit added to an interaction that had none is breaking, not compatible.
   Test in Task 2.

---

### Task 1: The limit in the IR and the catalog hash

**Files:**

- Modify: `crates/ridl-ir/proto/ridl/ir/v2/ir.proto` (`Decl`:
  `optional uint32
  limit = 31;`, with a comment: set only when a `[limits]`
  entry applies, ADR-0030 decision 13)
- Modify: every `v2::Decl { .. }` literal the compiler reports (ridl-sem
  lowering, ridl-ir, ridl-diff tests, ridl-backend-rust `clauses.rs`): add
  `limit: None`
- Modify: `crates/ridlc/src/design_lints/limits.rs` (`annotate_limits`),
  `crates/ridlc/src/lib.rs` (`check_loaded`: call it after `check_unit_limits`)
- Test: `crates/ridl-ir/src/catalog_hash.rs` `mod tests`; a new test file
  `crates/ridlc/tests/limit_in_ir.rs` on the helpers of
  `crates/ridlc/tests/payload_limits.rs`

**Interfaces:**

- Produces: `v2::Decl.limit: Option<u32>`;
  `pub(crate) fn annotate_limits(checked: &mut [CheckedPackage], units: &[String], limits: &BTreeMap<String, UnitLimits>)`.

- [ ] **Step 1: Write the failing tests**
  - `catalog_hash.rs`: `a_limit_changes_the_catalog_hash` (the same unit with
    `limit: Some(64)` on one interaction hashes differently from `None`) and
    `an_unset_limit_leaves_the_hash_unchanged` (two units equal but for
    `limit: None` vs the field absent hash equal — construct both the same way
    and compare to the hash computed before the field existed, using the pinned
    value of an existing hash test).
  - `limit_in_ir.rs`: `a_name_entry_is_recorded_in_the_ir`
    (`[limits.interaction]
    "Iface.sig" = 64` → that decl's `limit` is
    `Some(64)`, other decls `None`); `kind_and_default_are_recorded`;
    `no_table_records_nothing` (every decl `None`, and `ridl build`'s catalog
    hash equals the hash with no table);
    `an_ambiguous_short_name_falls_back_to_kind` (Review Focus 2); and a test
    for Review Focus 3 stating which of the two behaviours holds.
- [ ] **Step 2: Run** `cargo test -p ridl-ir catalog_hash` and
      `cargo test -p ridlc --test limit_in_ir` — fail on assertions once the
      field exists (add the proto field and `limit: None` literals first so they
      compile).
- [ ] **Step 3: Implement `annotate_limits`** by walking units and shapes as
      `limits::check` does and setting `decl.limit = resolve(..)`; skip entirely
      when `limits` is empty. Call it in `check_loaded` after
      `check_unit_limits`.
- [ ] **Step 4: Run** `cargo test -p ridl-ir`, `cargo test -p ridl-sem`,
      `cargo test -p ridlc`, `cargo test -p ridl-descriptor --test golden_hash`,
      `cargo xtask freeze check` — pass, with no freeze dump changed.
- [ ] **Step 5: Commit**
      `feat(ridl-ir): record a declared payload limit on the interaction`.

### Task 2: `limit_changed` in `ridl diff`

**Files:**

- Modify: `crates/ridl-diff/src/lib.rs` (`Category::LimitChanged`,
  `category_word` → `"limit_changed"`, `heading`)
- Modify: `crates/ridl-diff/src/classify.rs` (verdict `Breaking`; `explain`
  text: "the payload limit of the interaction changed; every change of a limit
  is breaking, ADR-0030 decision 13")
- Modify: `crates/ridl-diff/src/walk.rs` (compare `limit` once for any pair of
  interactions of the same kind, outside the per-kind arms; values `"none"` or
  the number)
- Modify: `crates/ridl/tests/diff_gate.rs` (the category list, in enum order),
  `crates/ridl/src/main.rs` (`--explain` help if it lists categories),
  `docs/book/cli-reference.md` (category list, enum order),
  `docs/book/evolving-an-interface.md` (breaking list), `crates/ridl-mcp` tests
  if they list categories
- Test: `crates/ridl-diff/src/classify/classify_tests.rs`,
  `crates/ridl-diff/src/tests.rs`

**Interfaces:**

- Consumes: `v2::Decl.limit` (Task 1).
- Produces: `Category::LimitChanged`, word `limit_changed`.

- [ ] **Step 1: Write the failing tests** in `classify_tests.rs`:
      `a_raised_limit_is_breaking`, `a_lowered_limit_is_breaking`,
      `a_limit_added_is_breaking`, `a_limit_removed_is_breaking`, and
      `an_unchanged_limit_reports_nothing`; in `tests.rs`, a walk test where
      only the limit differs and exactly one change with category
      `LimitChanged`, old `"none"`, new `"64"` is reported.
- [ ] **Step 2: Run** `cargo test -p ridl-diff` — fail on assertions (add the
      enum variant first so they compile).
- [ ] **Step 3: Implement** the variant in every exhaustive match, the verdict,
      the explanation and the comparison in `walk.rs`.
- [ ] **Step 4: Update** the category lists in `diff_gate.rs`, `main.rs`, the
      two book chapters and the MCP tests
      (`grep -rn rpc_bound_changed crates docs` finds each list).
- [ ] **Step 5: Run** `cargo test -p ridl-diff`,
      `cargo test -p ridl --test
  diff_gate`, `cargo test -p ridl-mcp`,
      `cargo xtask freeze update` then `cargo xtask freeze check` — pass; a
      changed dump only adds lines.
- [ ] **Step 6: Commit**
      `feat(ridl-diff): report a changed payload limit as breaking`.
