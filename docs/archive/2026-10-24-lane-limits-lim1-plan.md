# Per-interaction payload limits (LIM1, #1057) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** read a `[limits]` table from a unit's `ridl.toml`, resolve each
interaction's payload limit, and report an error for a payload above a declared
limit.

**Architecture:** `ridl-core`'s manifest parser gains a `Limits` value (Task 1),
the workspace loader carries each unit's limits and the manifest's `FileId` to
`ridlc` (Task 2), and a new design-lints module next to the RIDL-150 check
resolves the limits and reports RIDL-151 and the unmatched-name error (Task 3).

**Tech Stack:** Rust, `toml` with `Spanned`, the `diag_codes!` catalogue.

**Spec:** `docs/decisions/ADR-0030-transfer-and-bound-limits.md` decisions 12
and 14, and the lane driver `docs/wip/2026-10-24-lane-limits-driver.md`.

## Global Constraints

- The payload ceiling is `ridl_core::limits::PAYLOAD_CEILING` (16,256). No limit
  may be above it.
- Kind names are exactly `signal`, `fixed`, `event`, `command`, `query`.
- Resolution order: `[limits.interaction]` entry, then `[limits.kind]` entry,
  then `default`, then the ceiling.
- A `[limits.interaction]` key is `Interface.member`, the interface (or inline
  service) name as the IR records it, without the package. No other form is
  accepted.
- A unit's table applies to that unit only. A workspace root's `[limits]` does
  not apply to its members.
- RIDL-151 is an Error row with no lint name (ADR-0024 decision 1). It fires
  only when a `[limits]` entry gives the interaction its limit; where the limit
  is the ceiling because no entry applies, only RIDL-150 runs, unchanged.
- A unit with no `[limits]` table produces exactly the diagnostics it produces
  today.
- Plain, literal English in comments, messages and docs. No story ids in code.

## Review Focus

1. A `[limits]` table with only `default` on a package with a `fixed`: the fixed
   is sized and checked (ADR decision 14), though RIDL-150 skips it. Test in
   Task 3.
2. A limit exactly equal to the payload bound is accepted; one byte lower is
   RIDL-151. Test in Task 3.
3. A key naming an interface of another package, or written `pkg.Iface.member`,
   is the unmatched-name error, not silently ignored. Test in Task 3.
4. A non-integer, negative or table value (`default = "4k"`, `signal = -1`)
   gives a MANI error and the rest of the manifest still parses. Test in Task 1.
5. A workspace whose root declares `[limits]` and whose member does not: the
   member is not checked against the root's values. Test in Task 3.

---

### Task 1: Parse `[limits]` in the manifest

**Files:**

- Modify: `crates/ridl-core/src/manifest.rs` (the `Manifest` struct, the
  `KNOWN_KEYS` `""` row, a new `collect_limits` called next to `collect_lints`,
  tests in `mod tests`)
- Modify: `crates/ridl-core/src/diag.rs` (`MANI_CATALOG`: two new Error rows)
- Modify: `docs/specification/ridl-family-overview.md` (MANI table rows),
  `docs/decisions/ADR-0002-module-system.md` (a dated note: the `[limits]`
  table)

**Interfaces:**

- Produces, in `ridl_core::manifest`:
  - `pub struct Limits { pub default: Option<Spanned<u32>>, pub kinds: BTreeMap<LimitKind, Spanned<u32>>, pub interactions: Vec<LimitEntry> }`
  - `pub enum LimitKind { Signal, Fixed, Event, Command, Query }` (`Ord`)
  - `pub struct LimitEntry { pub name: String, pub key: Range<usize>, pub value: u32 }`
  - `Manifest.limits: Limits` (empty when the table is absent)
  - `Spanned<u32>` here is a value plus its byte range in `ridl.toml`; reuse an
    existing span-carrying pair if the module has one.

- [ ] **Step 1: Write the failing tests** in `manifest.rs` `mod tests`, in the
      style of `lints_table_is_read`:
  - `limits_table_is_read`: `default = 4096`, `[limits.kind] signal = 256`,
    `[limits.interaction] "Climate.temperature" = 64` → no diagnostic; `default`
    value 4096, `kinds[Signal]` 256, one entry with name `Climate.temperature`
    and value 64, and `spanned_text` of its key is `"Climate.temperature"`.
  - `absent_limits_table_is_empty`: `PACKAGE_HEAD` only → `limits` is empty, no
    diagnostic, and no MANI-005 for `limits` when the table is present.
  - `unknown_limit_kind_is_an_error`: `[limits.kind] stream = 1` → the new
    unknown-kind code, span is the key `stream`.
  - `limit_above_the_ceiling_is_an_error`: `default = 16257` → the new
    above-ceiling code, span is the value; `default = 16256` → no diagnostic.
  - `malformed_limit_value_is_an_error_and_the_manifest_parses`:
    `default = "4k"` and `[limits.kind] signal = -1` → one error each (the
    above-ceiling code is not used for these; use the existing type-error code
    the module uses for a wrong value type, or the unknown-kind code's sibling
    if none fits — record the choice), and `manifest` is `Some`.
- [ ] **Step 2: Run** `cargo test -p ridl-core manifest` — expect the new tests
      to fail on assertions (add `limits: Limits::default()` first so they
      compile).
- [ ] **Step 3: Add the two `MANI_CATALOG` Error rows** — the next free numbers
      that `form_and_mani_severities_follow_their_namespace_rule` allows for a
      parse error: "unknown kind in `[limits.kind]`" and "payload limit above
      the payload ceiling of 16,256 bytes". Add both rows to the MANI table of
      `docs/specification/ridl-family-overview.md`.
- [ ] **Step 4: Implement
      `collect_limits(file_id, text, &mut diags) -> Limits`** on the `[lints]`
      pattern (its own raw struct, so a malformed `[limits]` does not stop the
      rest of the manifest), and add `("limits", "table", "")` to the
      `KNOWN_KEYS` `""` row. Update both destructurings of `Manifest` in
      `crates/ridl-core/src/workspace.rs` to bind `limits` (unused until Task
      2).
- [ ] **Step 5: Run** `cargo test -p ridl-core --lib` — all pass, including the
      catalogue guards and
      `the_overview_form_and_mani_tables_mirror_the_catalogues`.
- [ ] **Step 6: Add the ADR-0002 note** (dated 2026-10-24, one paragraph: the
      table, its keys, unit scope, pointing to ADR-0030 decision 12).
- [ ] **Step 7: Commit**
      `feat(ridl-core): read the [limits] table of the manifest`.

### Task 2: Carry each unit's limits to `ridlc`

**Files:**

- Modify: `crates/ridl-core/src/workspace.rs` (root and member manifest loading;
  the loaded workspace exposes the limits per unit)
- Modify: `crates/ridlc/src/lib.rs` (`Compiled` gains the limits;
  `run_build_inner` and the check path pass them on)
- Test: `crates/ridl-core/src/workspace.rs` `mod tests`

**Interfaces:**

- Consumes: `Limits`, `LimitEntry`, `LimitKind` from Task 1.
- Produces: on the loaded workspace,
  `pub fn unit_limits(&self) -> &BTreeMap<String, UnitLimits>` keyed by package
  name, with
  `pub struct UnitLimits { pub manifest: FileId, pub limits: Limits }`. A unit
  with no `[limits]` table has no entry.
  `Compiled.limits:
  BTreeMap<String, UnitLimits>`.

- [ ] **Step 1: Write the failing tests** in `workspace.rs`:
  - `a_package_limits_table_is_loaded`: a single-package workspace with
    `[limits] default = 1024` → `unit_limits()["<pkg>"].limits.default` is 1024
    and `manifest` is that `ridl.toml`'s `FileId`.
  - `a_member_limits_table_belongs_to_the_member_only`: a workspace root with
    `[limits] default = 100` and one member with `[limits] default = 200` → the
    member's package has 200; no package gets 100 from the root.
  - `no_limits_table_no_entry`: no `[limits]` anywhere → `unit_limits()` is
    empty.
- [ ] **Step 2: Run** `cargo test -p ridl-core workspace` — the new tests fail.
- [ ] **Step 3: Implement** storing the limits per package in both manifest
      paths; a member's `[limits]` is allowed (unlike `[backend]`, no MANI-016).
      Thread `unit_limits` into `Compiled` in `crates/ridlc/src/lib.rs` next to
      `backends`.
- [ ] **Step 4: Run** `cargo test -p ridl-core workspace` and
      `cargo test -p ridlc --lib` — pass.
- [ ] **Step 5: Commit**
      `feat(ridl-core): carry each unit's payload limits to ridlc`.

### Task 3: Resolve the limits and report RIDL-151

**Files:**

- Create: `crates/ridlc/src/design_lints/limits.rs`
- Modify: `crates/ridlc/src/design_lints/ceiling.rs` (make `payloads`, `entry`,
  `Size`, `checked_encodings` `pub(super)`; add sizing for `fixed` as a separate
  function so RIDL-150 keeps skipping it)
- Modify: `crates/ridlc/src/design_lints/mod.rs` (`check_payload_limits`)
- Modify: `crates/ridlc/src/lib.rs` (call it next to `check_payload_ceiling`,
  with the same `payload_ceiling_blocked` guard)
- Modify: `crates/ridl-core/src/diag.rs` (`RIDL_151`, `RIDL_152` Error rows
  after `RIDL_150`)
- Modify: `docs/specification/ridl-language-reference.md` (rows for RIDL-151 and
  RIDL-152 after RIDL-150), `crates/ridlc/tests/corpus.rs` (code list, if the
  test requires every catalogued code)
- Test: create `crates/ridlc/tests/payload_limits.rs` on the helpers of
  `crates/ridlc/tests/payload_ceiling.rs` (`check(source, extra_manifest)`)

**Interfaces:**

- Consumes: `UnitLimits` (Task 2);
  `ceiling::{payloads, entry, Size,
  checked_encodings}`.
- Produces:
  `pub(crate) fn check_payload_limits(..same inputs as
  check_payload_ceiling.., limits: &BTreeMap<String, UnitLimits>) -> Vec<Diagnostic>`;
  `fn resolve(limits: &Limits, kind: LimitKind, owner: &str, member: &str) -> Option<u32>`
  returning `None` when no entry applies.
- Catalogue rows:
  `RIDL_151 = "RIDL-151", Error, "payload above its declared
  limit or with no finite bound"`;
  `RIDL_152 = "RIDL-152", Error, "a [limits]
  entry names no interaction of its unit"`.

- [ ] **Step 1: Write the failing tests** in `payload_limits.rs`:
  - `no_limits_table_reports_nothing_new`: a payload above the ceiling with no
    `[limits]` → only RIDL-150, no RIDL-151.
  - `the_declared_limit_boundary_is_exact`: a signal whose FlatBuffers bound is
    N bytes, `[limits.interaction] "Iface.sig" = N` → no RIDL-151; `= N-1` → one
    RIDL-151 on the member, message names the member, the role, N and the limit.
  - `resolution_order_is_name_then_kind_then_default`: `default = 1`,
    `[limits.kind] signal = 10000`, and a name entry of 10000 for one event →
    the signal and that event pass, the other event is RIDL-151.
  - `a_query_checks_request_and_reply_against_one_limit`: reply above, request
    below the limit → one RIDL-151 with role `reply`.
  - `an_unbounded_payload_under_a_declared_limit_is_an_error`: `string` with no
    finite bound in an enabled encoding plus `default = 16256` → RIDL-151.
  - `a_fixed_is_checked`: a `fixed` above `default` → RIDL-151; and RIDL-150
    still does not report a fixed above the ceiling.
  - `an_unmatched_name_is_ridl_152`: keys `"Nope.x"`, `"Iface.nope"` and
    `"pkg.Iface.sig"` → three RIDL-152, each spanned on its key in `ridl.toml`.
  - `a_root_table_does_not_apply_to_members`: root `default = 1`, member with no
    table → no RIDL-151 in the member.
  - `only_enabled_encodings_are_checked`: a payload over the limit only in
    proto3 and no backend enabling proto3 → nothing; with
    `[backend.rust] encodings = ["proto3"]` → RIDL-151.
- [ ] **Step 2: Run** `cargo test -p ridlc --test payload_limits` — fail on
      assertions.
- [ ] **Step 3: Add the catalogue rows and the reference rows.** Run
      `cargo test -p ridl-core diag` — pass.
- [ ] **Step 4: Implement** `limits.rs`: for each subject package with an entry
      in `limits`, walk shapes and interactions as `ceiling::check` does, map
      the interaction kind to `LimitKind`, `resolve`, and when it returns
      `Some(limit)` compare each enabled encoding's `Size` per role: `Bytes(n)`
      with `n > limit`, `NoBound` or `Overflow` is RIDL-151 on
      `ctx.sites.member(..)`. Size a `fixed` from its declared type with
      `size::size_state` (the function `fallible_entry` uses). Collect
      `(shape.name, decl.name)` pairs and report each `LimitEntry` whose `name`
      matches none as RIDL-152 on `Span { file: manifest, range: key }`.
- [ ] **Step 5: Run** `cargo test -p ridlc --test payload_limits`,
      `cargo test -p ridlc --test payload_ceiling`,
      `cargo test -p ridlc --lib
  design_lints` and
      `cargo test -p ridlc --test corpus` — pass.
- [ ] **Step 6: Commit**
      `feat(ridlc): report RIDL-151 for a payload above its declared limit`.
