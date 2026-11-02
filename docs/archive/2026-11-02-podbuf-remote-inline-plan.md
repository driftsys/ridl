# Podbuf Remote Inline Compatibility Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Detect remote struct head growth in consumer inline uses and give
source, baseline, build and publication judgments consistent historical
evidence.

**Architecture:** Keep local compared packages separate from side-specific
remote context. Publish a versioned context artifact containing reached remote
IR and verified content-pin provenance beside local snapshots. Missing
historical evidence produces explicit uncertainty; recovery either reconstructs
the old context or publishes a new baseline with a fresh catalog chain.

**Tech Stack:** Rust workspace, existing podbuf projection, canonical IR JSON,
serde, sha2, clap, and the existing CLI/MCP integration harnesses.

**Spec:** [Approved design](2026-11-02-podbuf-remote-inline-design.md),
including the owner's answers in section 7.

## Global constraints

- Work in the existing isolated worktree
  `/Users/sebastientasson/Workspace/driftsys/ridl-1163`; preserve both design
  commits and all unrelated work.
- Release source judgment, baseline enrichment and recovery together (R2).
- Target a declared pre-1.0 minor release, 0.15.0 if this follows 0.14; do not
  change release versions or publish in this task.
- Preserve canonical emitted catalog hash inputs and the existing IR package
  schema.
- Keep old shared-context diff entry points available with their existing
  signatures; route CLI and MCP through the new checked entry point.
- Diff and MCP never fetch, write a lockfile, modify snapshots or perform
  recovery.
- Exit 0 means compatible or identical, 1 means a proved break, and 2 means an
  explicit comparison could not establish compatibility or encountered an input
  error.
- Restart only affected podbuf histories; proved breaks and missing evidence
  take precedence over unchanged hashes.
- Repair requires exact historical local snapshots and verified historical
  remote pins; reset preserves the prior baseline and claims no earlier
  compatibility.
- Do not implement equal-size remote layout changes, shrinkage rules, general
  dependency semantic diff, system snapshots or historical chain certification.
- Do not push, open a PR, run `git stash`, release or alter unrelated frozen
  surfaces.
- New identifiers, comments, documentation and commits use plain international
  English.

## Review focus

- A legacy baseline cannot establish an unchanged pin: require explicit
  uncertainty only for relevant inline uses, including snapshot-to-snapshot
  comparisons.
- Nested remote types and induced interaction heads must use old-side inline
  semantics; direct roots and descriptor boundaries alone must not produce
  growth reports.
- Context must be bound to its local snapshot set: stale, mixed, malformed or
  unsupported context must never fall back to current definitions.
- Recovery failure must leave the old baseline and its catalog files intact,
  including when a backup destination already exists.
- Multi-unit catalog chains must use complete hash scopes and isolate the
  affected unit even when its local declarations did not change.

## Task 1: Separate contexts and detect remote inline growth

**Files:** Modify `crates/ridl-diff/src/{lib,walk,classify}.rs`; create
`crates/ridl-diff/src/remote_inline.rs`; extend `crates/ridl-diff/src/tests.rs`.

**Interfaces:**

- Add
  `diff_refs_with_contexts(old: &[&Package], new: &[&Package], old_context: &[&Package], new_context: &[&Package], encodings: &[&str]) -> Result<DiffReport, ContextError>`.
- Add public
  `ContextIssue { path: String, declaration: String, side: ContextSide }`,
  `ContextSide::{Old, New}` and `ContextError { issues: Vec<ContextIssue> }`,
  with deterministic ordering and Display/Error implementations. Issues identify
  consumer ownership rather than remote ownership.
- Add `Category::PodbufRemoteInlineGrowth`, word `podbuf_remote_inline_growth`,
  encoding classification, heading and explanation.
- Retain `diff_refs_with` and other existing entry points as shared-context
  compatibility wrappers; the new checked function never substitutes the
  opposite side's remote definitions.

- [ ] Write `remote_inline_growth_without_local_append_is_breaking`: identical
      local Sample with Reading then u64 sequence; remote Reading grows from one
      u64 to two; assert consumer path `app.api/Sample/reading`, category word,
      before/after sizes 8/16 and Breaking with podbuf.
- [ ] Run
      `cargo test --locked -p ridl-diff remote_inline_growth_without_local_append_is_breaking`;
      record the failing assertion before changing production code.
- [ ] Implement separate scopes and a remote-growth pass independent of local
      `decl_added` lines. Use the existing projection law and old-holder
      semantics. Follow nested remote inline declarations, terminate traversal
      at producer-size boundaries, deduplicate by consumer use and remote cause,
      and retain stable ordering.
- [ ] Add failing tests, then support: equal-size pad-fit append; unrelated
      remote changes; direct named root; bounded-array/map/union boundary;
      nested inline member inside an element head; fixed array in a head;
      induced request/reply tuple; new-only holder; multiple consumers; nested
      remote growth; missing or removed declarations. Without podbuf, report
      known growth as Compatible under the podbuf heading; missing evidence only
      blocks a requested podbuf judgment.
- [ ] Pin source-side resolution for existing local inline append rules: old
      remote size differs from new, with opposite pad-fit outcomes under the two
      contexts.
- [ ] Run `cargo test --locked -p ridl-diff` and commit
      `feat(ridl-diff): detect remote podbuf inline growth` with `Refs #1163`.

## Task 2: Versioned and validated baseline context

**Files:** Create `crates/ridlc/src/baseline_context.rs`; modify
`crates/ridlc/src/{lib,diff_side}.rs` and `crates/ridlc/Cargo.toml`; add
`crates/ridlc/tests/baseline_context.rs`.

**Interfaces:**

- Artifact name: `context.json`, beside flat local `.ir.json` files; it is never
  a package snapshot.
- Envelope version 1 has `version`, `local_packages` (canonical package name to
  SHA-256 of canonical IR JSON), and `remote_units` (sorted unit identity, URL,
  verified tree hash and reached Package snapshots). The verified tree hash is
  the content-pin identity; machine-local cache paths are excluded.
- Add
  `RemoteContext { packages: Vec<Package>, pins: BTreeMap<String, RemotePin> }`
  and `RemotePin { url: String, tree: String }`.
- Add
  `capture_context(own: &[&Package], remotes: &[&Package], units: &BTreeMap<String, MaterializedUnit>) -> Result<BaselineContext, ContextArtifactError>`;
  validate every included remote against its owning materialized unit.
- Add
  `read_context(dir: &Path, own: &[Package]) -> Result<Option<BaselineContext>, ContextArtifactError>`
  and
  `write_context(dir: &Path, context: &BaselineContext) -> Result<(), ContextArtifactError>`.
- Add optional context evidence to `DiffSide`; source sides retain verified
  remote provenance from `WorkspaceOutput::remote_units`. Directory snapshots
  read and validate their sibling artifact; a single snapshot file remains a
  partial standalone input with no assumed sibling evidence.
- Use existing `ridl_ir::v2` canonical writers/readers and their validation
  guards; add the existing workspace `sha2` dependency to ridlc if needed,
  without changing the IR protocol.

- [ ] Write `context_round_trip_keeps_ownership_and_pin_evidence`: local
      consumer, two transitively reached remote packages and one unrelated
      remote; assert exact reached closure, complete local fingerprints, stable
      bytes and remote context excluded from local compared packages.
- [ ] Run `cargo test --locked -p ridlc --test baseline_context`; establish
      failure before implementing serialization or loading.
- [ ] Implement closure starting from every local package declaration and
      interaction, adding all referenced remote packages transitively. For each
      included remote package retain its whole IR and follow references from the
      whole stored package so it remains resolvable offline.
- [ ] Add negative tests for duplicate package/unit identities, local/remote
      collisions, bad or absent pin provenance, unsupported version, malformed
      nested IR, missing local fingerprint, mismatched local snapshot and
      missing transitive remote context. Reject enriched malformed input instead
      of treating it as legacy.
- [ ] Prove directory copy works after removing vendor/cache access; prove a
      legacy directory still loads with absent evidence and no invented pin.
- [ ] Run `cargo test --locked -p ridlc --test baseline_context` and the
      existing `diff_side` tests; commit
      `feat(ridlc): retain versioned remote baseline context`.

## Task 3: Checked CLI and MCP comparisons

**Files:** Modify `crates/ridl/src/main.rs`, `crates/ridl-mcp/src/diff.rs`,
`crates/ridl-mcp/tests/tools.json` only if the catalog carries the updated
description; extend `crates/ridl/tests/diff_cli.rs` and the existing MCP diff
integration tests.

**Interfaces:**

- Add `ridlc::diff_side_context(side: &DiffSide) -> Vec<Package>` returning
  standard context and that side's remotes only; preserve `diff_context` for
  existing callers.
- Add
  `ridl_diff::diff_workspaces_with_contexts(old: &[Package], old_system: Option<&System>, new: &[Package], new_system: Option<&System>, old_context: &[Package], new_context: &[Package], encodings: &[&str]) -> Result<DiffReport, ContextError>`.
- CLI maps ContextError to exit 2 with consumer paths and concrete repair/reset
  command guidance. MCP maps it into the existing ToolError mechanism without a
  new output schema or verdict variant.

- [ ] Write offline CLI regressions using vendored, hash-pinned fixtures:
      source-to-source and enriched-baseline-to-source pin-only growth exit 1
      with the consumer category; legacy inline baseline exits 2; unchanged
      enriched baseline exits 0.
- [ ] Run `cargo test --locked -p ridl-cli --test diff_cli`; establish the
      missing behavior before routing callers.
- [ ] Route CLI and MCP through side-specific checked entry points; preserve
      normal JSON/text rendering for complete reports and the existing
      two-source-only system comparison.
- [ ] Test baseline-to-baseline missing evidence, no remote inline use, no
      podbuf selection, overlays on the candidate, no writes to
      inputs/locks/cache and matching MCP results. Missing evidence must name
      the affected consumer use and distinguish repair from reset.
- [ ] Run focused CLI and MCP suites; commit
      `fix(ridl): compare remote inline heads using each side context`.

## Task 4: Publish context and correct per-unit catalog history

**Files:** Modify `crates/ridlc/src/lib.rs`,
`crates/ridl/src/{main,catalogs}.rs`; create `crates/ridl/src/baseline.rs` for
publication coordination; extend
`crates/ridl/tests/{catalog_chain,baseline_gate}.rs`.

**Interfaces:**

- Keep the existing `CompatibleCatalogsFn` API available. Add a
  build-computation variant whose callback receives checked local packages,
  standard context and a `RemoteContext` captured from the same compile; avoid a
  second compilation or lockfile reread.
- Stage local snapshots, `context.json` and catalog files together. Validate
  their pairing before publication; include context in stale-file cleanup and
  replacement.
- Compare old and current complete scopes for verdicts and catalog hashes.
  Per-unit proved podbuf breaks and missing evidence override unchanged-hash
  carry. Do not alter `ridl_ir::catalog_hash` canonical inputs.

- [ ] Write `remote_inline_pin_growth_ends_only_affected_catalog_chain`: publish
      a two-unit fixture, move a remote pin, build, inspect descriptors and
      republish; assert affected compatible list is empty and its new chain
      contains only the current canonical emitted hash while the other chain
      remains carried.
- [ ] Run
      `cargo test --locked -p ridl-cli --test catalog_chain remote_inline_pin_growth_ends_only_affected_catalog_chain`;
      record the pre-fix failure.
- [ ] Publish versioned context from the compile that emits the local snapshots.
      Enriched unchanged baselines retain valid history; remote-only growth must
      not pass through an unchanged local-only hash shortcut.
- [ ] Test that published chain head equals the emitted descriptor's hash with
      remote types, including unchanged remote context; malformed enriched
      context refuses the operation; legacy missing evidence warns and
      suppresses affected build compatibility rather than claiming
      compatibility.
- [ ] Normal publication encountering missing historical inline evidence must
      require explicit reset or repair, preserving the previous baseline. Known
      proved breaks can still publish a fresh chain under existing publication
      rules, preserving the prior baseline as required by the approved design.
- [ ] Test multi-unit isolation, identical local declarations, root-manifest
      encodings, zero-interface units, and failed publication with the original
      baseline untouched.
- [ ] Run
      `cargo test --locked -p ridl-cli --test catalog_chain --test baseline_gate`;
      commit `fix(ridl): bind catalog history to historical remote context`.

## Task 5: Explicit repair and reset recovery

**Files:** Modify `crates/ridl/src/{main,baseline}.rs`; create
`crates/ridl/tests/baseline_recovery.rs`; use `crates/ridl/tests/remote_support`
fixture helpers.

**Interfaces:**

- Add mutually exclusive optional flags
  `ridl baseline [SOURCE] --repair-context [--out DIR]` and
  `ridl baseline [SOURCE] --reset-history [--out DIR]`.
- Repair compiles the supplied historical source read-only with frozen imports
  and validates the complete canonical local package set against DIR before
  writing any evidence. It does not overwrite local snapshots with new sources.
  Recompute the repaired baseline's current canonical hash and discard
  uncertified earlier chain entries; preserve the prior baseline.
- Reset compiles the current source under normal baseline rules, preserves the
  old baseline, and publishes current enriched snapshots with fresh chains.
  Missing old evidence does not block explicit reset; ordinary compile and
  publication refusal rules remain active.
- Preserve old baseline directories at unused sibling names
  `<baseline-name>.previous.N`, selecting the first positive unused N without
  overwriting an existing entry. Print the backup path. Existing symlinks and
  non-directories are not overwritten or followed as backup targets.
- Build and validate staging completely before replacing anything. Use a
  recoverable rename sequence; if replacement fails, restore the old baseline
  and report recovery paths. Do not claim cross-platform atomic multi-directory
  replacement.

- [ ] Write `legacy_baseline_reset_unblocks_diff_and_preserves_prior_bytes` and
      `historical_repair_refuses_new_pin_and_preserves_baseline`; assert exact
      exit codes, backup contents, context loading and subsequent diff behavior.
- [ ] Run `cargo test --locked -p ridl-cli --test baseline_recovery`; record the
      missing-command failure before adding flags.
- [ ] Implement repair, reset and backup coordination; diagnostics emitted by
      Task 3 must use these exact command spellings and explain which SOURCE
      represents the historical input.
- [ ] Add positive historical repair using matching local snapshots and old
      vendor tree; prove moved consumer snapshots are rejected and historical
      pins are loaded frozen. A remote-only pin substitution cannot be detected
      from legacy local IR alone: repair explicitly relies on the operator
      supplying the original historical source checkout and lockfile, and must
      say so rather than claim cryptographic proof of historical pin provenance.
- [ ] Test mutually exclusive flags, pre-existing backup names, write/rename
      failure recovery, malformed context repair refusal, compile errors,
      provisional identities and untombstoned removals. None may delete the old
      baseline.
- [ ] Run recovery, desk-check and publication suites; commit
      `feat(ridl): add baseline context repair and history reset`.

## Task 6: Records, stability evidence and full verification

**Files:** Amend `docs/decisions/ADR-0031-podbuf-projection-rules.md`,
`docs/design/podbuf-codec.md`, `docs/book/{cli-reference,stability}.md` and
`docs/technotes/walking-skeleton-architecture.md`. Review ADR-0002, ADR-0022 and
ADR-0029; amend only their affected statements. Update existing CLI snapshots
and generated freeze dumps only where the new optional flags require additions.
Do not alter frozen wire layouts, IR protocol or unrelated baselines.

**Interfaces:** The shipped CLI reference owns exact artifact version/shape,
context validation, error contract and recovery usage; ADR-0031 owns
inline-growth and history decisions. Link rather than duplicate normative rules.

- [ ] Add or extend meaningful CLI reference tests for the new category and
      recovery examples. Confirm the old reference lacks the entries before
      writing the matching documentation.
- [ ] Record dated amendments explaining both pin-only growth and correction of
      historical contexts. Replace the known-gap claim with the bounded
      implemented coverage; retain deferred equal-size/shrinkage/semantic gaps.
- [ ] Record the owner's explicit stability approval, pre-1.0 minor release
      requirement and minimum tool version for enriched decisions. Keep code
      version at its current value; release preparation is a separate action. An
      added optional flag may add freeze lines; never remove a row merely to
      make a check pass.
- [ ] Run focused crates:
      `cargo test --locked -p ridl-diff -p ridlc -p ridl-cli -p ridl-mcp` and
      relevant clippy checks. Fix regressions without accepting unrelated
      snapshot drift.
- [ ] Run `just verify` from this worktree, plus `git diff --check`; record all
      results and any unavailable tool/environment checks. Keep Cargo output in
      a task log; use a separate target directory if needed for concurrent
      workspace activity. No network fetch is authorized by this plan.
- [ ] Perform a fresh independent branch review against the approved design,
      including a negative control where a pin-only regression is deliberately
      reintroduced and the regression test fails. Restore the implementation and
      rerun affected checks.
- [ ] Apply the repository's SDD gardening workflow once implementation tests
      are green, retaining the owner's design decisions in durable records and
      moving completed working memory to the appropriate archive. Commit the
      reviewed records and test evidence.

## Plan self-review

All approved owner answers map to tasks: A and provenance to Task 2;
source/baseline classification and category to Tasks 1–3; history clearing and
hash consistency to Task 4; repair/reset and preservation to Task 5; R2 and
minor-release constraints to Task 6 and the global constraints. All five
review-focus classes have explicit test cases in their owning tasks. Deferred
semantic changes and release publication remain outside scope.

The repair trust boundary is explicit: a legacy baseline cannot independently
prove an old remote pin that it never recorded. Repair validates the supplied
local snapshots and pins, while the operator supplies historical provenance.
Reset is available when that provenance cannot be recovered.
