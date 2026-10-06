# Default RPC Response Bound Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every `command` and `query` resolves to a response bound — from its
annotation, the package or workspace `[defaults] command_timing` /
`query_timing`, or the built-in `[..1s]` / `[..3s]`.

**Architecture:** `ridl-core` carries the three raw `[defaults]` strings in one
`TimingDefaults` value from the manifest through the workspace merge into the
`Package` salsa input. `ridl-sem` parses them once per package into three
resolved `TimingSpec`s and hands `resolve_timing` the one that matches the
member's kind; `resolve_timing` gains the RPC defaulting arms. `ridl-diff`
accepts a `default_applied` flip on an RPC as compatible. `ridl-rt` is
unchanged.

**Tech Stack:** Rust (pinned toolchain), salsa, `toml`/`serde`, `just` gates.

**Spec:** `docs/archive/2026-10-06-rpc-default-response-bound-design.md` — read
it before any task. Section numbers below (§3, §4, …) refer to it.

## Global Constraints

- Built-in defaults: `command` → `[..1s]` (min absent, max 1 000 000 µs);
  `query` → `[..3s]` (min absent, max 3 000 000 µs). Signal/event built-in
  `[100ms..1000ms]` is unchanged.
- Manifest keys, exactly: `[defaults] command_timing`,
  `[defaults]
  query_timing`. Accepted forms `[..max]` and `[min..max]`;
  `[min..]` rejected.
- Precedence per key, independently: member annotation > package > workspace >
  built-in.
- A default fills `max` only on an annotated member; a default's `min` applies
  only to a member with no annotation (§3).
- RIDL-112 stays `Warning`, lint name `missing-response-bound`, default level
  warn. No new diagnostic codes.
- MANI-009 message:
  ``invalid `[defaults].<key>` in the package manifest:
  <reason>`` where
  `<key>` is `timing`, `command_timing` or `query_timing`.
- `ridl-rt` source is unchanged except the `call_deadline` rustdoc text.
- No story ids or plan names in `crates/`, `docs/book/`, `docs/design/`,
  `docs/technotes/` (`just story-id-check`). Prose is plain and literal.
- Conventional Commits; scopes from `.git-std.toml`. The commit that changes
  resolution (Task 4) carries a `BREAKING CHANGE:` footer naming the catalog
  hash change and the new call deadlines.
- Commit trailer: `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. A workspace that sets `command_timing` and a member that sets only
   `query_timing`: the member must inherit the workspace `command_timing` (per
   key merge, not per table). Test in Task 1.
2. `@[min..]` on a command where the default's `max` is _below_ the written
   `min`: resolution must not panic and must not silently reorder; it yields
   `min > max` and draws RIDL-101 like a written reversed range. Test in Task 4.
3. An invalid `command_timing` must not disturb `timing` or `query_timing`: only
   the command default falls back to built-in, and exactly one MANI-009 names
   `command_timing`. Test in Task 4.
4. A package with `missing-response-bound = "deny"`: an unannotated call is an
   error, while an annotated `@[..max]` call is clean. Test in Task 4.
5. A runtime call that legitimately takes longer than 1 s or 3 s in
   `ridl-loopback`, `ridl-rt-conformance` or `examples/cabin` (`just demo`) must
   not start failing. Checked in Task 4, step "runtime sweep".

---

### Task 1: `TimingDefaults` through manifest, workspace and `Package`

**Files:**

- Modify: `crates/ridl-core/src/manifest.rs` (`Manifest.default_timing` l.68,
  parse l.141-145, `RawDefaults` l.203, allowed keys l.325, tests l.643-690)
- Modify: `crates/ridl-core/src/package.rs:75` (`Package.default_timing`)
- Modify: `crates/ridl-core/src/workspace.rs` (`workspace_default_timing` l.385,
  merge l.516-517, tests l.1503-1580)
- Modify: every `Package::new(` caller (28 sites across `ridl-core`, `ridl-sem`,
  `ridlc`, `ridl-lsp`; `grep -rn "Package::new(" crates`)
- Modify: `crates/ridl-sem/src/check.rs:107` (read `pkg.defaults(db).timing`)

**Interfaces:**

- Produces, in `ridl-core::manifest` (re-exported where `Manifest` is):

  ```rust
  #[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
  pub struct TimingDefaults {
      pub timing: Option<String>,
      pub command_timing: Option<String>,
      pub query_timing: Option<String>,
  }
  impl TimingDefaults {
      /// Per key: `self`'s value, else `fallback`'s.
      pub fn or(self, fallback: &TimingDefaults) -> TimingDefaults;
  }
  ```

- `Manifest.default_timing: Option<String>` becomes
  `Manifest.defaults: TimingDefaults`.
- `Package.default_timing` (salsa input field) becomes
  `#[returns(ref)] pub defaults: TimingDefaults`. `Package::new` takes a
  `TimingDefaults` in the same position; callers that passed `None` pass
  `TimingDefaults::default()`, callers that passed `Some(s)` pass
  `TimingDefaults { timing: Some(s), ..Default::default() }`.
- Behaviour is unchanged after this task: the checker still reads only
  `.timing`.

- [ ] **Step 1: Write the failing tests**

  In `manifest.rs` tests: `defaults_command_and_query_timing_parse` — a manifest
  with `[defaults]` `timing = "[100ms..1s]"`, `command_timing = "[..1s]"`,
  `query_timing = "[..3s]"` yields
  `manifest.defaults == TimingDefaults { timing: Some("[100ms..1s]"),
  command_timing: Some("[..1s]"), query_timing: Some("[..3s]") }`
  with no diagnostics. Extend the existing unknown-`[defaults]`-key test so
  `command_timing` and `query_timing` draw no MANI-005 and `bogus` still does.

  In `workspace.rs` tests: `defaults_precedence_is_per_key` — workspace sets
  `command_timing = "[..2s]"` and `query_timing = "[..4s]"`; member sets only
  `query_timing = "[..5s]"`; the member package's `defaults(&db)` has
  `command_timing == Some("[..2s]")` and `query_timing == Some("[..5s]")`.

- [ ] **Step 2: Run them to verify they fail**

  Run: `cargo test -p ridl-core defaults_ --locked` Expected: compile error (no
  `defaults` field / `TimingDefaults`).

- [ ] **Step 3: Implement**

  Add the two `Option<String>` fields to `RawDefaults`, the two keys to the
  `"defaults"` allowed-keys list, `TimingDefaults` and `or`. Replace the
  single-string merge at `workspace.rs:516` with
  `defaults.or(&self.workspace_defaults)`. Update the `Package` doc comment to
  describe the three keys. Update all `Package::new` callers and the existing
  `default_timing` tests to the new field.

- [ ] **Step 4: Run the tests**

  Run:
  `cargo test --workspace --locked 2>&1 | grep -E "^test result|FAILED|panicked" | tail -20`
  Expected: all `test result: ok`.

- [ ] **Step 5: Commit**

  `refactor(ridl-core): carry command and query timing defaults through the manifest`

---

### Task 2: RPC default parsing and built-ins in `timing.rs`

**Files:**

- Modify: `crates/ridl-sem/src/timing.rs` (near `builtin_default_timing` l.79
  and `parse_default_timing` l.90; tests module)

**Interfaces:**

- Produces:

  ```rust
  pub fn builtin_command_timing() -> TimingSpec; // Range, min None, max 1_000_000 µs, default_applied false
  pub fn builtin_query_timing() -> TimingSpec;   // Range, min None, max 3_000_000 µs, default_applied false
  /// Parses a `[defaults].command_timing` or `query_timing` string. Accepts
  /// `[..max]` and `[min..max]`; rejects `[min..]` ("a response-bound default
  /// must set a maximum"). All other rejections match `parse_default_timing`.
  pub fn parse_rpc_default_timing(text: &str) -> Result<TimingSpec, String>;
  ```

- No caller yet; behaviour is unchanged after this task.

- [ ] **Step 1: Write the failing tests**

  - `rpc_default_accepts_max_only`: `parse_rpc_default_timing("[..1s]")` →
    `min_us == None`, `max_us == Some(us("1s"))`, `mode == Range`.
  - `rpc_default_accepts_both_bounds`: `"[10ms..1s]"` → both set.
  - `rpc_default_rejects_min_only`: `"[10ms..]"` → `Err` containing `"maximum"`.
  - `rpc_default_rejects_what_the_signal_default_rejects`: for each of `"1s"`,
    `"[1s]"`, `"[..1.5s]"`, `"[..0ms]"`, `"[2s..1s]"` → `Err`.
  - `builtin_rpc_defaults_are_one_and_three_seconds`:
    `builtin_command_timing().max_us == Some(us("1s"))`,
    `builtin_query_timing().max_us == Some(us("3s"))`, both `min_us == None`.

- [ ] **Step 2: Run to verify they fail**

  Run: `cargo test -p ridl-sem --lib timing::tests::rpc_default --locked`
  Expected: compile error (functions not defined).

- [ ] **Step 3: Implement** — factor the shared bracket/`..`/bound parsing out
      of `parse_default_timing` so both functions call it;
      `parse_default_timing` keeps requiring both bounds,
      `parse_rpc_default_timing` requires `max`. The built-ins call
      `parse_rpc_default_timing` with `.expect`, like `builtin_default_timing`.

- [ ] **Step 4: Run the tests**

  Run: `cargo test -p ridl-sem --lib timing --locked 2>&1 | tail -3` Expected:
  `test result: ok`.

- [ ] **Step 5: Commit** —
      `feat(ridl-sem): parse response-bound defaults for commands and queries`

---

### Task 3: `ridl-diff` accepts a `default_applied` flip on an RPC

**Files:**

- Modify: `crates/ridl-diff/src/classify.rs:723-770` (`rpc_bound`) and its tests
- Modify: `crates/ridl-ir/src/rules.rs:~385-395` (comment only, if it states the
  old invariant)

**Interfaces:** none new. `rpc_bound` keeps its signature.

- [ ] **Step 1: Write the failing tests** (in `classify.rs` tests, beside the
      existing `rpc_bound` tests, using their fixture helpers)

  - `rpc_default_made_explicit_is_compatible`: old command timing `[..1s]` with
    `default_applied = true`, new `[..1s]` with `default_applied = false` →
    `Verdict::Compatible`.
  - `rpc_explicit_bound_replaced_by_equal_default_is_compatible`: the reverse
    flip → `Verdict::Compatible`.
  - `rpc_defaulted_bound_added_is_breaking`: old timing absent, new `[..1s]`
    with `default_applied = true` → `Verdict::Breaking`.
  - `rpc_default_flip_with_a_raised_max_is_breaking`: old `[..1s]` defaulted,
    new `[..2s]` explicit → `Verdict::Breaking`.

- [ ] **Step 2: Run to verify** — `cargo test -p ridl-diff rpc_ --locked`;
      expected: the two `compatible` tests FAIL (verdict `Breaking`).

- [ ] **Step 3: Implement** — delete the `default_applied` early return at
      l.754-762 and its comment; the bound comparison that follows already
      decides. Mirror the `timing()` comment at l.701 ("a default made
      explicit"). Update any comment in `rules.rs` that says an RPC bound is
      never defaulted.

- [ ] **Step 4: Run** — `cargo test -p ridl-diff --locked 2>&1 | tail -3`;
      expected ok.

- [ ] **Step 5: Commit** —
      `feat(ridl-diff): classify a defaulted RPC bound made explicit as compatible`

---

### Task 4: Resolve every command and query to a bound

**Files:**

- Modify: `crates/ridl-sem/src/timing.rs` — `resolve_timing` l.157 (range arm
  l.250-276, degenerate-node path), `untimed` l.360, `missing_response_bound`
  l.395, `TimingSpec`/`InteractionKind` doc comments l.50-70
- Modify: `crates/ridl-sem/src/check.rs` — default resolution l.98-146,
  `Checker.default_timing` l.512, `resolve_member_timing` l.4450-4467, tests
  (`rpc_bounds_lower_into_the_ir_and_absent_stays_absent` l.13407, `mani_009_*`
  l.13332-13400, helper `ridl_package_with_default` l.10710)
- Modify: `crates/ridl-core/src/diag.rs` — RIDL-112 row l.715, MANI-009 row
  l.1284
- Modify: `crates/ridl/tests/lints.rs`
- Re-bless: corpus snapshots (`crates/ridlc/tests/corpus*`), IR/descriptor
  goldens (`crates/ridlc/tests/golden.rs`, `ir_canonical.rs`,
  `codegen_model.rs`, `crates/ridl-descriptor/tests/golden_hash.rs`), generated
  backend files (`RIDL_UPDATE_GENERATED=1`). Find each suite's update mechanism
  in `CONTRIBUTING.md` or the test file header; never edit a snapshot by hand.

**Interfaces:**

- Consumes: `TimingDefaults` (Task 1); `builtin_command_timing`,
  `builtin_query_timing`, `parse_rpc_default_timing` (Task 2).
- `resolve_timing` keeps its signature; `default` is now the default for `kind`.
- `Checker.default_timing: TimingSpec` becomes three fields: `default_timing`,
  `default_command_timing`, `default_query_timing` (all `TimingSpec`).
  `resolve_member_timing` passes the one matching `kind` (`Fixed` passes
  `default_timing`; it is unused).
- `missing_response_bound(kind, spec: &TimingSpec, file, anchor)` — takes the
  resolved spec so the message names the applied bounds.

- [ ] **Step 1: Write the failing `timing.rs` tests** (one per §3 row, each for
      `Command` and `Query` via a two-element loop; `D = [..1s]` built with
      `parse_rpc_default_timing`)

  - `untimed_rpc_takes_the_default`: no annotation → `Some(spec)`,
    `spec.max_us == Some(us("1s"))`, `spec.min_us == None`, `default_applied`,
    codes `["RIDL-112"]`.
  - `untimed_rpc_takes_the_default_min_too`: `D = [10ms..1s]`, no annotation →
    `min_us == Some(us("10ms"))`.
  - `half_open_min_on_rpc_takes_max_from_the_default`: `@[20ms..]` →
    `min_us == Some(us("20ms"))`, `max_us == Some(us("1s"))`, `default_applied`,
    codes `["RIDL-112"]`.
  - `max_only_on_rpc_ignores_the_default`: `D = [10ms..1s]`, `@[..5s]` →
    `min_us == None`, `max_us == Some(us("5s"))`, `!default_applied`, no codes.
  - `full_range_on_rpc_ignores_the_default`: `@[1ms..20ms]` → as written,
    `!default_applied`.
  - `half_open_min_above_the_default_max_is_ridl_101` (Review Focus 2):
    `@[2s..]` with `D = [..1s]` → codes contain `"RIDL-101"` and `"RIDL-112"`,
    no panic.
  - Existing signal/event tests stay unchanged and green.

- [ ] **Step 2: Run** — `cargo test -p ridl-sem --lib timing --locked`; expected
      the new tests FAIL (`None` returned / `max_us == None`).

- [ ] **Step 3: Implement `resolve_timing` changes**

  - `untimed`: `Command | Query` → `Some(applied_default(default))` plus
    `missing_response_bound(kind, &spec, …)`.
  - Range arm: for `Command | Query`, when the resolved `max` is `None`, set
    `max_us = default.max_us.clone()` and `default_applied = true`, then run the
    existing `min > max` check (RIDL-101) on the completed range. Keep the
    RIDL-112 condition at l.265-270 as it is.
  - Degenerate timing node: `Command | Query` take `applied_default(default)`
    with no new diagnostic (the parse error already fails the package).
  - RIDL-112 message:
    ``"{kind} without a declared response bound — the
    default `@{bounds}` is applied: the provider must settle {responding}
    within the bound. Write `@[..max]` to declare the bound this {kind} really
    promises (ridl §9.3)"``.
  - Update the `TimingSpec`, `InteractionKind`, `untimed` and `resolve_timing`
    doc comments: commands and queries are defaulted.

- [ ] **Step 4: Run** —
      `cargo test -p ridl-sem --lib timing --locked 2>&1 | tail -3`; expected
      ok.

- [ ] **Step 5: Write the failing check-level tests** (`check.rs`)

  - Rename and rewrite `rpc_bounds_lower_into_the_ir_and_absent_stays_absent` →
    `rpc_bounds_lower_into_the_ir_and_absent_takes_the_default`: an unannotated
    command lowers `timing.max_us == Some("1000000")`,
    `default_applied == true`; an unannotated query `"3000000"`.
  - `package_command_and_query_defaults_apply`: package defaults
    `command_timing = "[..250ms]"`, `query_timing = "[..20ms]"` → those values
    in the IR. Extend `ridl_package_with_default` to take a `TimingDefaults`.
  - `mani_009_on_a_malformed_command_timing` (Review Focus 3):
    `command_timing = "[10ms..]"`, valid `query_timing = "[..20ms]"` → exactly
    one MANI-009 whose message contains `` `[defaults].command_timing` ``; the
    command lowers the built-in 1 s; the query lowers 20 ms; signals use
    `timing`.
  - `mani_009_on_a_malformed_query_timing`: symmetric.
  - Existing `mani_009_*` tests assert the message names
    `` `[defaults].timing` ``.

- [ ] **Step 6: Implement the checker wiring** — parse the three strings from
      `pkg.defaults(db)` (`parse_default_timing` for `timing`,
      `parse_rpc_default_timing` for the two others), one MANI-009 per invalid
      key with the per-key message, each falling back to its own built-in.
      Update the MANI-009 catalog title to ``invalid `[defaults]` timing value``
      and the RIDL-112 catalog title to
      ``"`command` or `query` takes the default
  response bound"``.

- [ ] **Step 7: Add the deny-level test** (Review Focus 4) in
      `crates/ridl/tests/lints.rs`: a package with
      `[lints] missing-response-bound = "deny"` → an unannotated command reports
      RIDL-112 as an error and the run exits 1; the same package with the
      command annotated `@[..1s]` exits 0.

- [ ] **Step 8: Run the workspace tests** —
      `cargo test --workspace --locked 2>&1 | grep -E "FAILED|panicked|test result: FAILED" | head -30`.
      Expected: only snapshot/golden failures caused by the new bounds. Read
      each failing diff; every change must be an RPC `timing` appearing with
      `default_applied = true`, a catalog hash, or RIDL-112's new message. Any
      other change is a bug — stop and report it.

- [ ] **Step 9: Re-bless** the snapshots, goldens and generated files, then
      rerun `cargo test --workspace --locked`; expected all ok.

- [ ] **Step 10: Runtime sweep** (Review Focus 5) — run
      `cargo test -p ridl-loopback -p ridl-rt -p ridl-rt-conformance --locked`
      and `just demo`. Grep their sources for a `sleep`, a delay or a handler
      that waits longer than 1 s on a command or 3 s on a query. If one exists,
      give that member an explicit annotation in its `.ridl` source, with a
      comment stating why; never raise the built-in.

- [ ] **Step 11: Commit**

  ```text
  feat(ridl-sem)!: default the response bound of untimed commands and queries

  <body: the precedence chain and the built-ins>

  BREAKING CHANGE: every command and query now carries a response bound in
  the IR. An untimed command lapses after 1 s and an untimed query after 3 s
  unless [defaults] command_timing or query_timing says otherwise. Catalogs
  with untimed commands or queries get a new catalog hash, so a provider and
  its clients must be rebuilt together.
  ```

---

### Task 5: Records and book

**Files:**

- Modify: `docs/decisions/ADR-0015-qos-absorption-and-rpc-bounds.md` —
  `## Status` (l.3-12), decision 4 (l.151-163), decision 7 (l.193-198)
- Modify: `docs/specification/ridl-language-reference.md` — §9.1 (l.895-935:
  precedence sentence and the "never defaulted either" paragraph), §9.3 (l.965+,
  l.992), the §16 table row for RIDL-112 (~l.1711), the glossary entry
  (~l.2361), any `[defaults]` example
- Modify: `docs/book/lints.md` (RIDL-112 row; l.78 `[defaults]` sentence names
  the new keys), `docs/book/getting-started.md:389` (`[defaults]` example),
  `docs/book/cli-reference.md:118`
- Modify: `crates/ridl-rt/src/contract.rs:150-172` (`call_deadline` rustdoc: the
  bound is always present for a command or query from a catalog built by this
  version; `None` remains for older catalogs)

**Interfaces:** none.

- [ ] **Step 1: Amend ADR-0015 in place** as §5.1 of the spec says: decision 4
      heading "Warned, and defaulted", the new rationale, the
      `missing-response-
  bound = "deny"` route; decision 7 drops the
      "`default_applied` always false" clause; `## Status` gains "Amended
      2026-10-06: decisions 4 and 7 — commands and queries take a default
      response bound (driftsys/ridl#741)."
- [ ] **Step 2: Update the reference and the book** as listed. Precedence
      sentence: package `[defaults]` shadows workspace `[defaults]`, which
      shadows the built-in — `[100ms..1000ms]` for `timing`, `[..1s]` for
      `command_timing`, `[..3s]` for `query_timing`, each key resolved on its
      own.
- [ ] **Step 3: Run the doc gates** —
      `just book-check link-check doc-path-check story-id-check check 2>&1 | tail -15`
      and
      `cargo test -p ridl-cli --test book_lints --test book_examples --locked 2>&1 | tail -3`.
      Expected: all pass. (`crates/ridl/` is package `ridl-cli`.)
- [ ] **Step 4: Commit** —
      `docs(adr): amend ADR-0015 so commands and queries take a default response bound`
      (split into a second `docs(docs)` commit for the reference and book if the
      scope list requires).

---

### After the last task

- [ ] `just verify` green (record the tail in the PR body).
- [ ] Branch-level review: `/review` passes 1 and 2.
- [ ] File the follow-up issue: ceiling and floor lints on the resolved response
      bound, with threshold syntax (spec §2 item 4), linking #741.
- [ ] Garden `docs/wip/2026-10-06-rpc-default-response-bound-*` with
      `sdd-gardening` (spec → ADR-0015 amendment already made; archive both
      files), then open the PR closing #741.
