# Toolchain Pin Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enforce a workspace's RIDL release pin, install releases side by side,
and select the compatible binary directly in VS Code.

**Architecture:** `ridl-core` owns release parsing, compatibility and the
root-manifest refusal, with the running release supplied by callers.
`crates/ridl` owns installation and selection; both installer scripts use its
directory layout, and the extension consumes `toolchain which --json` to manage
one direct language-server process per RIDL root. The six tasks below retain the
requested split and order; mechanical loader-call migrations in Task 1 keep all
dependents compiling before Task 2 changes server behavior.

**Tech Stack:** Rust 1.98.1, clap, toml with spanned values, serde/serde_json,
ureq 3, sha2, tar, flate2, standard-library file locks; Bash, PowerShell;
TypeScript, Node's test runner, vscode-languageclient 10.1.0.

**Spec:** [Approved design](2026-11-25-toolchain-pin-design.md). Read it
together with this plan.

## Global Constraints

- The minimum pinnable release is `0.23.0`; a pin is a string
  `MAJOR.MINOR.PATCH`, three decimal numbers, no prefix, no pre-release or build
  suffix.
- Compatibility requires identical `MAJOR.MINOR` and running `PATCH >=` pinned
  `PATCH`; only the root manifest returned by `find_root` pins.
- MANI-024, MANI-025 and MANI-026 suppress all other workspace diagnostics and
  analysis and cause CLI exit `2`; a member pin is MANI-027, a warning, and is
  ignored. Invalid TOML skips pin checking and remains MANI-001.
- Manifest absence preserves today's behavior; no repository example, book
  sample workspace or permanent corpus acquires a pin. Tests generate pinned
  manifests and inject a running release.
- Origins and automatic-install policy never come from a manifest or workspace
  setting; production downloads use
  `https://github.com/driftsys/ridl/releases/download/editor-v<version>/`.
- Artifacts are flat `ridl-<target>.tar.gz` archives with matching `.sha256`
  files, containing `ridl` or `ridl.exe`, `LICENSE` and
  `THIRD-PARTY-NOTICES.txt`; supported targets are `x86_64-unknown-linux-gnu`,
  `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin` and
  `x86_64-pc-windows-msvc`.
- Roots are `RIDL_TOOLCHAIN_DIR`, otherwise Linux `$XDG_DATA_HOME/ridl` or
  `~/.local/share/ridl`, macOS `~/Library/Application Support/ridl`, Windows
  `%LOCALAPPDATA%\ridl`; releases are `<root>/versions/<version>/` and require
  an empty `.complete` file.
- Installation locks `<root>/versions/.<version>.lock`, rechecks, verifies
  SHA-256 before extraction, stages in `.tmp-<version>-<pid>` on the same
  filesystem, writes `.complete` last and renames. Existing version directories
  are never written; damaged directories are reported and preserved.
- Toolchain implementation stays in `crates/ridl`; add flate2 directly there and
  preserve packaged runtime/compiler compatibility boundaries. Use the existing
  workspace dependencies for ureq, sha2 and tar.
- `RIDL_VERSION` accepts bare or `editor-v` form; scripts copy `ridl` to the
  existing PATH destination. `RIDL_INSTALL_BINARY=ridlc` remains a flat install
  and does not touch the versions directory.
- Extension `ridl.toolchain.autoInstall` is boolean, default `true`, scope
  `application`; keep the absence of `untrustedWorkspaces` capability. Preserve
  the explicit `ridl.serverPath` override.
- No terminal hand-off, terminal automatic install, uninstall, artifact signing,
  plugin pinning or automatic pin update. The driving session files follow-ups
  `driftsys/ridl#TBD-deferred` and `driftsys/ridl#TBD-minimum-ci`.
- Frozen surfaces only gain lines: generate dumps, preserve every baseline ref
  in `freeze/surfaces.toml`, and do not refresh generated crates or unrelated
  dumps.
- During execution, use the existing `CARGO_TARGET_DIR`; run focused tests with
  `cargo test --locked -p <crate>`. Do not bind TCP sockets in the Codex
  sandbox. The named HTTP and PowerShell HTTP checks run at the normal gate
  outside it.
- Each test cycle must compile before its red run and fail on an assertion about
  behavior. For a new interface, first add the minimum compiling declaration and
  inert result in the named implementation file; a missing symbol, panic stub or
  missing fixture is not the required red result. Record and check the named
  mutations, restore them, then rerun the owning tests.
- This document authorizes no implementation in the planning session. Execution
  commits below are proposed messages; a sandbox that cannot write the index
  leaves changes and reports those messages.

## Review Focus

1. A refused root reached through a member, with broken sources, invalid
   overlays and remote imports, yields only the root refusal and performs no
   writes or fetches — Task 1,
   `release_refusal_precedes_members_overlays_and_remotes`.
2. An unsaved pin change after a successful LSP analysis clears stale source
   results, blocks navigation, and recovers without restarting — Task 2,
   `release_refusal_replaces_analysis_and_recovers_from_manifest_buffer`.
3. Two installers contend for one release, or an archive fails validation: one
   complete directory is published, no existing directory is modified, and no
   temporary directory counts as installed — Task 3,
   `toolchain_install_waits_rechecks_and_preserves_damage` and
   `toolchain_install_rejects_checksum_and_unsafe_archive`.
4. Installer roots containing spaces and a second run without available assets
   preserve the PATH copy and versions directory; `ridlc` never creates a
   version entry — Task 4, `install_reuses_complete_release_without_download`
   and `install_ridlc_remains_flat`.
5. Concurrent document opens and a pin change during selection cannot start
   duplicate clients or start an obsolete selection; malformed or future-schema
   JSON safely falls back — Task 5,
   `concurrent_opens_and_pin_changes_use_latest_selection` and
   `unknown_schema_and_malformed_results_fall_back_once`.

---

## File Structure

The files below are the boundaries; task lists identify all edits. New
`ridl-core/src/release.rs` holds pure release/pin types and rules;
`workspace.rs` owns filesystem preflight. New `ridlc/src/release.rs` supplies
the normalized build release without a global mutable override. New
`ridl/src/lib.rs` exposes `toolchain/{mod,paths,install,selection}.rs` for
file-based integration tests; the binary remains the command adapter. New
extension `toolchainSelection.ts` owns async selection through injected effects,
and `workspaceClients.ts` owns root/client routing, reusing the existing
`ClientLifecycle`. Existing files retain their other responsibilities.

The book currently has no manifest reference chapter. Ruling (driving session):
this change does not write one for every key. Task 1 creates
`docs/book/release-pin.md`, a chapter on pinning the ridl release (the key, the
compatibility rule, the four codes; later tasks add the toolchain directory,
`ridl toolchain`, the install scripts and the editor behaviour), and adds it to
`SUMMARY.md`; diagnostics remain owned by the family overview and lint book.
Task 6 adds ADR-0036, the next number after ADR-0035 at planning time, and
`docs/design/toolchain-directory.md`.

Test assertions below are assertion clauses, not proposed function bodies.
Bindings named in a clause are the observed values from the fixture described
immediately above it; each named test constructs that fixture using its crate's
existing temporary-directory/process or LSP in-memory connection helpers.
Additional fixture helpers stay private to the owning test module.

### Task 1: Parse and enforce the root pin across compiler and CLI paths

**Files:**

- Create: `crates/ridl-core/src/release.rs`, `crates/ridlc/src/release.rs`,
  `crates/ridlc/build.rs`, `crates/ridlc/build_support/version.rs`,
  `crates/ridl/build_support/version.rs`, `crates/ridlc/tests/release_pin.rs`,
  `crates/ridl/tests/release_pin.rs`, `crates/ridl/tests/build_version.rs`.
- Modify: `crates/ridl-core/src/{lib,manifest,workspace,diag}.rs`,
  `crates/ridlc/src/{lib,diff_side,main}.rs`, `crates/ridl/build.rs`,
  `crates/ridl/src/{main,lock,vendor,property}.rs`.
- Modify: direct `load_workspace*` call sites and their existing tests in
  `crates/ridl-lsp/src/server.rs`, `crates/ridl-core/src/` and `crates/ridlc/`;
  discover the complete list with
  `rg -n 'load_workspace(_with|_with_remotes)?\(' crates xtask`. This is a
  parameter migration only for server callers.
- Test: inline modules in `release.rs`, `manifest.rs`, `workspace.rs`,
  `diag.rs`; `xtask/src/freeze/{manifest,diagnostics}.rs`.
- Modify: `freeze/{manifest,diagnostics}.txt`,
  `docs/book/{SUMMARY,lints,cli-reference}.md`,
  `docs/specification/ridl-family-overview.md` §7.2,
  `docs/decisions/ADR-0010-cli-conventions.md`.
- Create: `docs/book/release-pin.md`.
- Read-only: `freeze/surfaces.toml`, `crates/ridlc/Cargo.toml` (Cargo discovers
  the new `build.rs` automatically).

**Interfaces:**

- Consumes: existing
  `parse_manifest(file_id: FileId, text: &str) -> (Option<Manifest>, Vec<Diagnostic>)`;
  `find_root(dir: &Path) -> Option<PathBuf>` remains unchanged; existing
  `LoadedWorkspace`, `Overlay`, `RemoteAccess<'_>` and `LoadError`.
- Produces in `ridl_core::release`:
  `pub struct Release { pub major: u64, pub minor: u64, pub patch: u64 }` with
  numeric `Ord`, `Copy`, `Eq` and `Display`;
  `pub fn parse_release(text: &str) -> Option<Release>`;
  `pub const MIN_PINNABLE_RELEASE: Release` = `0.23.0`;
  `pub fn compatible(running: Release, pin: Release) -> bool`.
- Produces:
  `pub struct PinValue { pub text: Option<String>, pub span: Range<usize> }`;
  `pub struct ReleasePin { pub release: Release, pub span: Range<usize> }`;
  `pub fn pin_value(text: &str) -> Option<PinValue>`;
  `pub fn parse_release_pin(file_id: FileId, text: &str) -> Result<Option<ReleasePin>, Diagnostic>`;
  `pub fn check_release_pin(file_id: FileId, text: &str, running: Release) -> Option<Diagnostic>`;
  `pub fn is_release_refusal(diagnostic: &Diagnostic) -> bool`. Export the
  predicate at the crate root too.
- Produces in `workspace.rs`:
  `pub struct RootRelease { pub manifest: Option<PathBuf>, pub text: Option<String>, pub pin: Option<ReleasePin>, pub diagnostics: Vec<Diagnostic>, pub sources: SourceMap }`;
  `pub fn read_root_release(entry: &Path, manifest_overlay: Option<&Overlay>) -> io::Result<RootRelease>`;
  this resolves the root and reads only its manifest, without comparing a
  running release or loading members.
- Changes loader signatures to
  `pub fn load_workspace(db: &mut RidlDatabase, entry: &Path, running: Release) -> io::Result<LoadedWorkspace>`,
  `pub fn load_workspace_with(db: &mut RidlDatabase, entry: &Path, overlays: &[Overlay], running: Release) -> Result<LoadedWorkspace, LoadError>`,
  and
  `pub fn load_workspace_with_remotes(db: &mut RidlDatabase, entry: &Path, overlays: &[Overlay], access: RemoteAccess<'_>, running: Release) -> Result<LoadedWorkspace, LoadError>`.
- Produces in `ridlc/src/release.rs`, reexported from `lib.rs`:
  `pub fn running_release() -> ridl_core::release::Release`. Normal value is
  `env!("RIDL_BUILD_VERSION")`; only under `cfg(debug_assertions)` accept
  `RIDL_TEST_RUNNING_RELEASE`. An invalid test override is ignored and uses the
  actual build release, never an implicit bypass. No core environment access.
  Existing public `ridlc::run_*` and `compile_workspace*` signatures stay
  unchanged; their shared internal loader passes this release. CLI direct loader
  calls pass the same function's result.
- Produces `pub fn normalise_version(value: &str) -> &str` in each crate-local
  `build_support/version.rs`, included by that crate's build script and both
  included by the stamp integration test. Keep the two small pure normalizers
  behaviorally identical so each published crate builds without a sibling source
  directory; trim whitespace then remove one leading `editor-v` or `v`, in that
  order; blank input falls back to `CARGO_PKG_VERSION`. Keep
  `rerun-if-env-changed=RIDL_BUILD_VERSION`. Both binaries' clap versions use
  the resulting `RIDL_BUILD_VERSION`.
- Produces
  `fn release_preflight(entries: &[PathBuf]) -> io::Result<Option<(Vec<Diagnostic>, SourceMap)>>`
  in `crates/ridl/src/main.rs`: shared pin-only preflight for commands that do
  not load a workspace, notably formatting and descriptor inspection. Existing
  `fn exit_code(run: &CliRun) -> ExitCode` and `ridlc`'s
  `fn finish(run: io::Result<CliRun>) -> ExitCode` check the refusal predicate
  first.

- [ ] **Step 1: Write the failing pin/parser/loader tests.** In `release.rs`,
      name `release_pin_accepts_only_three_decimal_components`,
      `release_compatibility_uses_major_minor_and_patch`,
      `release_pin_reports_value_spans_and_floor`; in `workspace.rs`, name
      `release_refusal_precedes_members_overlays_and_remotes`,
      `member_pin_is_warning_even_when_its_value_is_invalid`, and
      `invalid_toml_keeps_mani_001`. Pin `0.23.0` against running `0.23.2`; also
      test `[package]` and `[workspace]`. Assert the following, using the actual
      span to slice the original text:

  ```rust
  // release_pin_accepts_only_three_decimal_components
  assert_eq!(accepted, ["0.23.0", "1.2.3", "00.23.0"]);
  assert_eq!(rejected, ["v0.23.0", "editor-v0.23.0", "0.23", "0.23.0-rc.1", "0.23.0+1", " 0.23.0", "-1.23.0", "1.2.18446744073709551616"]);
  // release_compatibility_uses_major_minor_and_patch; pin = 0.23.2
  assert_eq!(compatible_running, ["0.23.2", "0.23.4"]);
  assert_eq!(incompatible_running, ["0.23.1", "0.22.9", "0.24.0", "1.23.2"]);
  // release_pin_reports_value_spans_and_floor
  assert_eq!(codes_for_wrong_type_bad_string_below_floor_mismatch, ["MANI-024", "MANI-024", "MANI-025", "MANI-026"]);
  assert_eq!(value_slice, "\"0.23.0\"");
  assert!(mismatch_message.contains("0.23.0"));
  assert!(mismatch_message.contains("0.24.0"));
  assert!(mismatch_message.contains("ridl toolchain install 0.23.0"));
  // release_refusal_precedes_members_overlays_and_remotes
  assert_eq!(diagnostic_codes, ["MANI-026"]);
  assert!(loaded.workspace.packages(&db).is_empty());
  assert_eq!(loaded.sources.iter_files().count(), 1);
  assert_eq!(filesystem_after, filesystem_before);
  // member_pin_is_warning_even_when_its_value_is_invalid
  assert_eq!(pin_diagnostic_codes, ["MANI-027"]);
  assert_eq!(pin_severity, Severity::Warning);
  // invalid_toml_keeps_mani_001
  assert_eq!(diagnostic_codes, ["MANI-001"]);
  ```

  The refusal fixture has a nonexistent member, an unknown root key, malformed
  `instances.lock`, a nonexistent header, a missing remote import under
  `Frozen::No`, a broken source and an outside-workspace source overlay. It must
  return the refusal rather than an overlay error, import error or member error.
  Repeat through all three loader entry points and from a member entry. Invalid
  TOML means broken TOML syntax, not merely a wrong typed field. Member pins
  with `42`, `"bad"`, `"0.22.0"` and `"9.0.0"` each produce only MANI-027 for
  the pin; remote unit pins never constrain the consumer. A workspace without a
  pin retains its existing diagnostics and files.

- [ ] **Step 2: Run tests to verify they fail.** Run
      `cargo test --locked -p ridl-core release_`,
      `cargo test --locked -p ridl-core member_pin_`, and
      `cargo test --locked -p ridl-core invalid_toml_keeps_mani_001`. Expected:
      assertion failures showing ignored pins, extra diagnostics or loaded
      members; the unchanged TOML control may already pass.

- [ ] **Step 3: Implement the pure interfaces in `release.rs` and the loader
      signatures in `workspace.rs`.** Read a generic spanned TOML value map
      before typed manifest validation, so a non-string pin becomes MANI-024
      rather than MANI-001. Accept ASCII decimal components fitting `u64`,
      including leading zeros, and display canonical decimal components. For
      syntactically invalid TOML, return no pin from the prepass and use today's
      parser diagnostics. When both mode tables exist, inspect `[workspace]`
      first, falling back to a pin in `[package]` if it has none; a refusal
      precedes MANI-002, otherwise ordinary mode validation still rejects the
      manifest. `pin_value` preserves wrong-type presence for member warnings
      and code actions. Add only the two known-key rows
      `package.ridl-version string` and `workspace.ridl-version string`; do not
      make a typed string field reject wrong-type values prematurely.

  The root preflight must precede overlay validation, members, source parsing,
  codegen headers, remotes, all lockfile reads and lint directives. A refused
  `LoadedWorkspace` has an empty workspace, only the root source and the single
  refusal, empty other collections, no report scope and no instance lock. Never
  call `load_remotes` after refusal. Permit one root `ridl.toml` overlay for LSP
  reuse; root discovery still uses on-disk `find_root`, and other manifest
  overlays remain unsupported. Consume this overlay before source-overlay
  validation. `read_root_release` returns an unpinned result for an existing
  path without a root, returns the root's MANI-001 for malformed TOML, and
  propagates path/read errors; its diagnostics use its own `sources` ids.

  In standalone `parse_manifest`, validate pin grammar/floor with
  `parse_release_pin` but never compare a running release. Member and remote
  loading remove these pin-only parse findings before extending diagnostics:
  local members replace them with MANI-027, remote pins do not constrain the
  consumer. Adapt `known_keys_drive_mani_005` and
  `known_key_types_are_the_types_the_reader_accepts` to use `"0.23.0"` for this
  key and recognize MANI-024 as its type error, preserving their checks for
  every other key. Register MANI-024/-025/-026 as errors and MANI-027 as warning
  with lint name `member-ridl-version` and default `warn` in the `diag_codes!`
  MANI catalogue. Update the explicit lint-name expectation and MANI-warning
  whitelist in `diag.rs` tests; do not exempt the new warning from registry
  checks. `is_release_refusal` recognizes exactly 024–026, even when severity
  was otherwise altered; MANI-027 raised to deny remains ordinary exit `1`.

- [ ] **Step 4: Write failing compiler, CLI and stamp tests.** In the two
      `tests/release_pin.rs` files, name
      `release_refusal_exits_two_and_preserves_outputs`,
      `release_refusal_in_json_and_sarif_is_the_only_diagnostic`,
      `release_refusal_does_not_change_unpinned_or_non_source_commands`, and
      `release_override_is_debug_only`. In `build_version.rs`, name
      `release_stamp_normalises_both_binary_tags`. Run child processes with the
      debug override rather than changing process-global environment in
      concurrent Rust tests. Parameterize the refusal tests over
      MANI-024/-025/-026 and the following source paths: `check`, `build`,
      `fmt`, `fmt --check`, `lock`, `baseline`, `test`, `vendor`,
      `vendor --check`, both source sides of `diff`, and `ridlc check/build`.

  ```rust
  assert_eq!(exit_status, Some(2));
  assert_eq!(reported_codes, [expected_refusal]);
  assert_eq!(workspace_and_output_bytes_after, workspace_and_output_bytes_before);
  assert_eq!(json_diagnostic_count, 1);
  assert_eq!(sarif_result_count, 1);
  assert_eq!(unprefixed_versions, ["ridl 0.23.0", "ridlc 0.23.0"]);
  assert_eq!(release_binary_with_override_exit, Some(2));
  ```

  `fmt` also gets `release_fmt_preflights_all_roots_before_writing`: format a
  directory containing two independent RIDL roots, with a format-worthy source
  in the first and a refusing pin in the second; bytes in both remain unchanged.
  Its existing bare-file and manifestless-directory cases still work. Add
  `release_describe_checks_input_root_without_analysing_sources`: a valid
  descriptor inside a refusing workspace exits `2` with only the pin diagnostic,
  even when its sources are broken. A standalone descriptor outside any root
  still succeeds when cwd has a refusing manifest; preflight follows the input
  path, not cwd. Snapshot-only diff, `init`, `new` and lock's merge-driver mode
  retain their current contracts. Stamping tests include both crate-local
  build-support modules, asserting plain, `v` and `editor-v` inputs; the child
  CLI tests assert debug override behavior. Add an ignored test
  `release_override_is_ignored_by_release_binaries` that reads
  `RIDL_RELEASE_BIN` and `RIDLC_RELEASE_BIN`, runs both release binaries and
  proves `RIDL_TEST_RUNNING_RELEASE=0.24.0` cannot make pin `0.24.0` pass on
  binaries stamped `0.23.0`.

- [ ] **Step 5: Run tests to verify they fail.** Run
      `cargo test --locked -p ridl-cli --test release_pin`,
      `cargo test --locked -p ridlc --test release_pin`, and
      `cargo test --locked -p ridl-cli --test build_version`. Expected: exit
      `1`/`0` instead of `2`, writes under `fmt`, or prefixed stamps; not a
      build failure.

- [ ] **Step 6: Implement `ridlc::running_release()`, both version normalizers
      and the refusal exit paths in the files above.** Keep the `ridlc` public
      compilation API signatures; its shared `load_and_check` passes
      `running_release()` to core. Short-circuit its subsequent workspace
      checks, lint/directive reporting, backend work and baseline-context
      computations on refusal. Every direct core loader caller passes the same
      release. `release_preflight` uses the shared root reader and checker,
      interns/remaps its diagnostics into its returned source map, and scans all
      distinct roots before formatting writes; it does not introduce ordinary
      semantic validation into `fmt`. Preflight the requested entry before
      collecting sources, then all discovered roots before any write.
      Source-workspace command preflight also precedes
      baseline/publication-context validation and output writes. Keep the root
      refusal visible when callers filter to a member report scope. Check
      refusal before CLI-specific ordinary-error mappings, notably `baseline`,
      `lock` and both vendor paths; `vendor --check` must not append lock/vendor
      diagnostics. `ridl lsp/mcp` still start servers: Task 2 handles their
      per-workspace refusal. For `fn run_describe(path: &Path) -> ExitCode`, run
      pin-only preflight against the descriptor input root before
      reading/verifying bytes, as the design explicitly includes this command;
      do not compile sources. Keep standalone descriptors and snapshot-only diff
      independent of cwd pins.

- [ ] **Step 7: Write the failing frozen-surface tests.** Extend the existing
      xtask manifest/diagnostic dump tests. Assert:

  ```rust
  assert!(manifest_lines.contains(&"package.ridl-version string".into()));
  assert!(manifest_lines.contains(&"workspace.ridl-version string".into()));
  assert!(diagnostic_lines.contains(&"MANI-024 error".into()));
  assert!(diagnostic_lines.contains(&"MANI-025 error".into()));
  assert!(diagnostic_lines.contains(&"MANI-026 error".into()));
  assert!(diagnostic_lines.contains(&"MANI-027 warning lint=member-ridl-version default=warn".into()));
  assert!(baseline_lines.iter().all(|line| current_lines.contains(line)));
  assert!(new_lines.iter().all(|line| committed_dump_lines.contains(line)));
  ```

- [ ] **Step 8: Run the frozen-surface tests to verify failure before updating
      dumps.** Run `cargo test --locked -p xtask freeze::manifest` and
      `cargo test --locked -p xtask freeze::diagnostics` against both generated
      and committed lines before dump regeneration, then `just freeze-check`
      against the implemented but stale dumps. Expected: the committed-line
      assertion and stale-dump check fail even if generated catalogue lines
      already pass. Reuse the existing generator; no new freeze API or baseline
      is needed.

- [ ] **Step 9: Document the key, diagnostics and release-refusal exit case, and
      regenerate the two dumps.** Add the manifest chapter and SUMMARY entry,
      document root-only behavior, minimum, compatibility and exact codes; add
      `member-ridl-version` to `lints.md` and MANI-024..027 to overview §7.2.
      Amend ADR-0010's existing command rows and cli-reference exit table so
      refusal is `2`, and describe normalized versions. Generate with
      `cargo xtask freeze update`; inspect that only manifest/diagnostics dumps
      changed. If another output changes, investigate it and preserve any
      preexisting user change rather than reverting it. No pinned manifest
      examples are added to compiled book fixtures: show the new key as an
      isolated TOML fragment.

- [ ] **Step 10: Run tests to verify they pass.** Run
      `cargo test --locked -p ridl-core`,
      `cargo test --locked -p ridlc --test release_pin`,
      `cargo test --locked -p ridl-cli --test release_pin`,
      `cargo test --locked -p ridl-cli --test build_version`,
      `cargo test --locked -p ridl-cli --test cli_reference`,
      `cargo test --locked -p ridlc --test cli_reference`,
      `cargo test --locked -p xtask freeze::manifest`,
      `cargo test --locked -p xtask freeze::diagnostics`, `just freeze-check`,
      `just link-check`, and `just doc-path-check`. Expected: all pass and no
      baseline line disappears. Outside the sandbox's focused run, build with
      `RIDL_BUILD_VERSION=editor-v0.23.0 cargo build --locked --release -p ridl-cli -p ridlc`,
      then run
      `RIDL_RELEASE_BIN="$CARGO_TARGET_DIR/release/ridl" RIDLC_RELEASE_BIN="$CARGO_TARGET_DIR/release/ridlc" cargo test --locked -p ridl-cli --test release_pin release_override_is_ignored_by_release_binaries -- --ignored`
      (use `.exe` on Windows). Mutation checks: remove the early return, change
      `>=` to `==`, include MANI-027 in refusal, or remove the debug cfg; the
      corresponding tests must fail before restoration.

- [ ] **Step 11: Commit.** Proposed message:
      `feat(ridl-core): enforce root manifest release pins`. Stage only this
      task's files and the mechanical loader-call changes; the driving session
      commits if this sandbox cannot write the index.

### Task 2: Publish refusals and pin actions in LSP and MCP

**Files:**

- Modify: `crates/ridl-lsp/src/server.rs`, `crates/ridl-lsp/tests/server.rs`,
  `crates/ridl-mcp/src/{snapshot,lib,diff}.rs` and their inline tests.
- Modify: `crates/ridl-mcp/README.md`, `docs/design/mcp-workspace-tools.md`,
  `docs/book/cli-reference.md` LSP/MCP sections, `docs/book/release-pin.md`
  editor section.

**Interfaces:**

- Consumes Task 1:
  `check_release_pin(file_id: FileId, text: &str, running: Release) -> Option<Diagnostic>`,
  `is_release_refusal(&Diagnostic) -> bool`,
  `pin_value(&str) -> Option<PinValue>`, and
  `load_workspace_with_remotes(db: &mut RidlDatabase, entry: &Path, overlays: &[Overlay], access: RemoteAccess<'_>, running: Release) -> Result<LoadedWorkspace, LoadError>`.
- Retains
  `pub fn run_with_version(connection: Connection, version: Option<&str>) -> Result<SessionEnd, Error>`;
  store its parsed release (default `ridlc::running_release()`) in `ServerState`
  for every reload. Keep existing
  `fn load(&mut self, dir: &Path) -> io::Result<()>` as the disk-load adapter.
- Produces in `server.rs`:
  `fn reload_workspace(&mut self, dir: &Path, manifest_overlay: Option<ridl_core::Overlay>) -> io::Result<()>`
  and
  `fn pin_workspace_action(&self, params: &lt::CodeActionParams) -> Option<lt::CodeAction>`.
  Store manifest buffers separately from source `InputFile`s; reuse existing
  `open/change/close`, notification dispatch, diagnostic conversion and
  publication APIs.
- Retains
  `pub fn snapshot(path: &str, overlays: &[OverlayInput]) -> Result<Snapshot, ToolError>`
  and
  `pub fn lookup_snapshot(path: &str, overlays: &[OverlayInput]) -> Result<Snapshot, ToolError>`.
  Add private
  `fn release_refusal_error(output: &ridlc::WorkspaceOutput) -> Option<ToolError>`
  in `snapshot.rs`; use existing `ToolError::Request(String)` for lookup
  refusal, preserving frozen MCP schemas. Check/diff keep their existing
  diagnostic-bearing output/error paths with the root span and code.

- [ ] **Step 1: Write the failing LSP/MCP tests.** In LSP's existing in-memory
      transport tests, name `release_refusal_is_published_on_manifest_value`,
      `release_refusal_replaces_analysis_and_recovers_from_manifest_buffer`,
      `root_manifest_disk_change_reloads_workspace`, and
      `pin_workspace_action_preserves_toml_and_is_idempotent`. Initialize with
      running `0.23.2`; load a healthy workspace, open a source, then change
      root pin in an open TOML buffer to `0.24.0`; do not write it to disk.
      Request hover, completion, symbols and code actions. Fix the buffer back
      to `0.23.0`, close it, then change the disk manifest and send
      `workspace/didChangeWatchedFiles`.

  ```rust
  assert_eq!(manifest_codes, ["MANI-026"]);
  assert_eq!(diagnostic_value_text, "\"0.24.0\"");
  assert!(cleared_source_diagnostics.is_empty());
  assert!(hover.is_none());
  assert!(completion_items.is_empty());
  assert!(source_symbols.is_empty());
  assert!(recovered_hover.is_some());
  assert_eq!(action.title, "Pin this workspace to ridl 0.23.2");
  assert_eq!(inserted_line, "ridl-version = \"0.23.2\"");
  assert_eq!(second_pin_actions.len(), 0);
  assert_eq!(comments_and_other_bytes_after, comments_and_other_bytes_before);
  ```

  Repeat action tests for standalone `[package]`, `[workspace]`, inline
  `workspace = { members = [] }`, dotted `workspace.members = []`, CRLF,
  trailing inline table-header comments, and a following table. Offer on the
  root manifest without any diagnostic, not on member manifests, invalid TOML,
  an existing invalid pin, or a running release below `0.23.0`. Preserve the
  original line endings. On refusal, newly opened sources must not become
  analyzable standalone overlays; formatting, rename, references, definition,
  inlay hints and workspace symbols must also return empty/no result and no
  edit.

  In MCP inline tests name
  `release_refusal_is_reported_by_every_workspace_tool`: invoke `ridl_check`,
  metrics, all five name/dependency lookup tools and source-side `ridl_diff` on
  the refusing root and a member path; assert check has exactly MANI-026, the
  other tools return their existing error form naming MANI-026 and
  `ridl toolchain install 0.24.0`, and none returns a normal symbol/type result.
  Assert `ridl_explain` and source-text-only `ridl_check` still work. No new
  output property or tool is added.

- [ ] **Step 2: Run tests to verify they fail.** Run
      `cargo test --locked -p ridl-lsp --test server release_`,
      `cargo test --locked -p ridl-lsp --test server root_manifest_`,
      `cargo test --locked -p ridl-lsp --test server pin_workspace_`, and
      `cargo test --locked -p ridl-mcp release_refusal_`. Expected: missing
      manifest publication, stale navigation, ignored watched-file events or
      lookup returning a normal result.

- [ ] **Step 3: Implement `reload_workspace`, `pin_workspace_action` and
      `release_refusal_error` with the exact signatures above.** Bypass the
      existing empty-workspace-to-I/O-error conversion specifically for release
      refusals, retain their manifest diagnostics, clear prior
      workspace/source/overlay indexes and fixes, and stop analysis while
      refused. Preserve open source buffer text separately for recovery;
      manifest buffers never pass through `profile_of_path` or become source
      overlays. Route TOML open/change/close and disk watched-file
      create/change/delete through reload; disk events use the current unsaved
      root buffer if present. Rebuild rather than append state and publish empty
      lists for previously diagnosed files. Keep the server alive and avoid a
      second showMessage for the refusal.

  Insert immediately after the owning table header using a `WorkspaceEdit`
  (inside the table value before its closing brace for inline tables, or as a
  top-level dotted key before other tables for an implicit/dotted table), after
  validating exactly one manifest mode. Integrate this action before the current
  `fixes.get(path)` early return; absence of diagnostics must not prevent it.
  Advertise its action kind alongside quick fixes and respect `context.only`.
  All workspace MCP lookup handlers stop at the common refusal guard; check
  exposes the diagnostic itself and source-side diff retains its existing
  failure route. Keep all existing MCP schemas and tool names.

- [ ] **Step 4: Document and run tests to verify they pass.** Update the owning
      book/MCP records with publication, reload and action behavior, including
      root-manifest overlays and source-only tool exemptions. Run
      `cargo test --locked -p ridl-lsp --test server`,
      `cargo test --locked -p ridl-mcp`, `just freeze-check`, `just link-check`,
      `just doc-path-check`. Expected: pass; no MCP dump changes. Mutation
      controls: restore the empty-workspace error conversion, skip clearing old
      overlays, remove watched-file dispatch or remove lookup refusal guard;
      each named test must fail.

- [ ] **Step 5: Commit.** Proposed message:
      `feat(ridl-lsp): publish release refusals and workspace pin actions`.

### Task 3: Install and select toolchains through the CLI

**Files:**

- Create: `crates/ridl/src/lib.rs`,
  `crates/ridl/src/toolchain/{mod,paths,install,selection}.rs`,
  `crates/ridl/tests/{toolchain,toolchain_http}.rs`.
- Modify: `crates/ridl/src/main.rs`, `crates/ridl/build.rs`,
  `crates/ridl/Cargo.toml`, `Cargo.toml`, `Cargo.lock`,
  `THIRD-PARTY-NOTICES.txt` if required by the dependency notice inventory.
- Modify: `xtask/src/freeze/cli.rs`, `freeze/cli.txt`,
  `docs/decisions/ADR-0010-cli-conventions.md`, `docs/book/cli-reference.md` and
  its existing CLI-reference tests/support as needed for nested subcommand
  coverage.
- Modify: `justfile` `install-check` only to invoke the explicitly ignored
  local-HTTP test outside sandbox runs. Task 4 extends this same recipe, not a
  second gate.

**Interfaces:**

- Consumes Task 1: `Release`, `parse_release(&str) -> Option<Release>`,
  `compatible(Release, Release) -> bool`, `MIN_PINNABLE_RELEASE`,
  `read_root_release(&Path, Option<&Overlay>) -> io::Result<RootRelease>`.
  Selection reads the pin without comparing against the running binary first.
- Produces under `ridl_cli::toolchain`:
  `pub const RELEASE_BASE_URL: &str = "https://github.com/driftsys/ridl/releases/download"`;
  `pub fn toolchain_root(platform: &str, env: &BTreeMap<String, OsString>, home: Option<&Path>) -> io::Result<PathBuf>`;
  `pub fn release_target(target: &str) -> io::Result<&str>`.
- Produces
  `pub struct InstalledRelease { pub release: Release, pub directory: PathBuf, pub complete: bool }`;
  `pub fn list(root: &Path) -> io::Result<Vec<InstalledRelease>>`;
  `pub fn install(root: &Path, release: Release, target: &str, base_url: &str) -> io::Result<PathBuf>`.
  Returned path is the binary, not its directory. The production adapter always
  passes `RELEASE_BASE_URL`; only library tests pass
  `file://<absolute fixture directory>` or localhost HTTP. No CLI base URL flag
  or environment override is introduced.
- Produces serde-serializable
  `pub struct WhichReport { pub schema: u32, pub manifest: Option<PathBuf>, pub pin: Option<String>, pub running: String, pub status: WhichStatus, pub path: Option<PathBuf>, pub source: Option<WhichSource>, pub diagnostic: Option<WhichDiagnostic> }`,
  `pub struct WhichDiagnostic { pub code: String, pub message: String }`,
  `pub enum WhichStatus { Compatible, Missing, Unpinned, Invalid }` and
  `pub enum WhichSource { Installed, Running }`, with lowercase serialized enums
  and explicit JSON nulls.
- Produces
  `pub fn which(entry: &Path, root: &Path, running: Release, running_exe: &Path, target: &str) -> io::Result<WhichReport>`
  and `impl WhichReport { pub fn exit_code(&self) -> u8; }`. `schema` is always
  `1`. Use canonical absolute manifest/binary paths; nonexistent version roots
  count as empty. No manifest is `unpinned`; an unreadable/malformed manifest is
  an I/O/input error, not an invalid pin.
- Main adds nested
  `ToolchainCommand::{Install { version: Option<String> }, List, Which { path: PathBuf, json: bool }}`
  under `Command::Toolchain`; `Which.path` defaults to `.`. Add
  `fn run_toolchain(command: ToolchainCommand) -> ExitCode`. `build.rs` also
  exports `RIDL_BUILD_TARGET` from Cargo's `TARGET`; do not use host runtime
  architecture or `std::env::consts` as a target triple.

- [ ] **Step 1: Write failing directory/selection/schema tests.** In
      `tests/toolchain.rs`, name `toolchain_roots_follow_platform_precedence`,
      `toolchain_which_selects_highest_compatible_including_running`,
      `toolchain_which_schema_one_covers_all_statuses`,
      `toolchain_list_sorts_numerically_and_marks_damage`, and
      `toolchain_target_rejects_unpublished_triples`. Test roots for Linux
      XDG/home, Darwin, Windows LOCALAPPDATA, and an override with spaces;
      missing required home/data directory variables return a named error rather
      than defaulting to cwd. Empty overrides count as unset. Installed
      candidates `0.23.0`, `0.23.4`, `0.23.10`, `0.24.0`; pin `0.23.2`; running
      `0.23.2`. Ignore `.tmp-*`, lock files, files named as versions and
      unrelated names. A `.complete` directory is not a marker file.

  ```rust
  assert_eq!(selected.pin.as_deref(), Some("0.23.2"));
  assert_eq!(selected.path.as_deref(), Some(expected_0_23_10_binary.as_path()));
  assert_eq!(selected.source, Some(WhichSource::Installed));
  assert_eq!(report_json_keys, ["diagnostic", "manifest", "path", "pin", "running", "schema", "source", "status"]);
  assert_eq!(schema_value, 1);
  assert_eq!(status_exit_pairs, [("compatible", 0), ("unpinned", 0), ("missing", 1), ("invalid", 1)]);
  assert_eq!(listed_versions, ["0.23.0", "0.23.4", "0.23.10", "0.24.0"]);
  assert!(damaged_line.ends_with("(damaged)"));
  assert!(unsupported_message.contains("x86_64-unknown-linux-musl"));
  ```

  The eight JSON keys are checked as a sorted set. Check every null/non-null
  combination: `missing` has a valid pin and null path/source/diagnostic;
  `invalid` has MANI-024 or MANI-025 and null path/source; `unpinned` has null
  pin and **the running path/source** (the design's explicit unpinned
  exception). An invalid string is `pin: null`; a parsed below-floor pin is its
  canonical string. A missing manifest is null; an existing unpinned manifest
  remains absolute. A missing entry, unreadable manifest, invalid TOML and
  wrong-type manifest table exit `2`, with no successful JSON on stdout.
  MANI-026 is never a `which` invalid diagnostic. Prefer the running binary on
  an equal-version tie, so no unnecessary installed copy is selected. If running
  `0.23.12`, it beats installed `0.23.10`; damaged directories never win. Read
  permission/type errors are not converted into an empty installed set.

- [ ] **Step 2: Run tests to verify they fail.** Run
      `cargo test --locked -p ridl-cli --test toolchain toolchain_which`,
      `cargo test --locked -p ridl-cli --test toolchain toolchain_roots`,
      `cargo test --locked -p ridl-cli --test toolchain toolchain_list`, and
      `cargo test --locked -p ridl-cli --test toolchain toolchain_target`.
      Expected: failed value/status/path assertions against compiling inert
      adapters, not missing-interface errors.

- [ ] **Step 3: Implement the directory, selection and CLI interfaces above in
      their owning modules.** Numeric sorting uses `Release`; canonical
      directory names use its display form. `toolchain_root` treats a relative
      explicit environment path as relative to the caller's current directory
      and returns an absolute path; platform defaults require their stated
      variables/home. `which` uses `read_root_release`, filters complete
      compatible installations and includes `current_exe`; skip a complete entry
      whose binary is missing and list it as damaged, preserving its files. For
      complete installed candidates on an unsupported build target, return a
      named target error; an unpinned or compatible running binary remains
      usable without downloading. Keep progress/errors off stdout. `install`
      without a version resolves the current root pin without enforcing the
      current binary's compatibility; no pin, malformed pin, below-floor pin or
      unreadable TOML exits `2`.

- [ ] **Step 4: Write failing installation tests.** In `tests/toolchain.rs`,
      name `toolchain_install_verifies_and_publishes_complete_release`,
      `toolchain_install_waits_rechecks_and_preserves_damage`,
      `toolchain_install_rejects_checksum_and_unsafe_archive`, and
      `toolchain_install_missing_asset_cleans_only_its_stage`. Build fixture
      assets under `editor-v0.23.0/` in a local directory, retaining the real
      gzip/tar/checksum path. The fake binary, license and notices have distinct
      known bytes. Test malformed checksum, wrong filename, missing asset,
      corrupt gzip, missing binary, nonempty marker, absolute/parent paths and
      symbolic/hard links. Do not permit archive entries to create `.complete`
      or escape staging.

  ```rust
  assert_eq!(installed_binary_bytes, fixture_binary_bytes);
  assert_eq!(installed_license_bytes, fixture_license_bytes);
  assert_eq!(installed_notices_bytes, fixture_notices_bytes);
  assert_eq!(complete_marker_bytes, b"");
  assert!(own_stage_paths.is_empty());
  assert_eq!(second_install_path, first_install_path);
  assert_eq!(existing_directory_bytes_after, existing_directory_bytes_before);
  assert!(damaged_error.contains(damaged_directory.to_str().unwrap()));
  assert!(!failed_version_directory.exists());
  assert_eq!(outside_sentinel_bytes, b"unchanged");
  ```

  For the lock test, the parent holds a real `File::lock` on `.0.23.0.lock`,
  spawns a child requesting the release, publishes a valid complete fixture
  under the held lock and makes assets unavailable, then releases the lock.
  Assert the child waited and then returned the existing path successfully
  without a download. Also race two independent install children against the
  file source; both return the same complete path and no nested/stale stage is
  published. A failed install must not remove another version, a preexisting
  damaged directory, or a preexisting same-name stage it did not create. The
  completed-install repeat removes fixture assets entirely to prove the fast
  path is offline.

  In `tests/toolchain_http.rs`, name
  `toolchain_ureq_path_verifies_and_installs`, mark it
  `#[ignore = "requires a local TCP listener; run by just install-check outside the sandbox"]`,
  serve a bounded local fixture server and call the same library `install` with
  its HTTP base URL. Assert requested archive/checksum paths, the verified
  bytes, a checksum refusal and cleanup. This is the sole network-path control,
  not a replacement for file-based tests.

- [ ] **Step 5: Run installation tests to verify they fail.** Run
      `cargo test --locked -p ridl-cli --test toolchain toolchain_install_`.
      Expected: assertions on absent marker/bytes, missing locking/recheck or
      unsafe extraction fail. Outside the sandbox run
      `cargo test --locked -p ridl-cli --test toolchain_http -- --ignored`;
      expected red failure from the same unimplemented install behavior. Do not
      attempt to bind inside the sandbox.

- [ ] **Step 6: Implement
      `install(root: &Path, release: Release, target: &str, base_url: &str) -> io::Result<PathBuf>`
      in `toolchain/install.rs`.** Add direct dependencies `flate2 = "1"`,
      `ureq.workspace = true`, `sha2.workspace = true`, `tar.workspace = true`,
      `serde.workspace = true`; move any duplicate dev-only declarations to the
      production section and update `Cargo.lock`. Verify the full checksum
      record names the requested archive and contains 64 hex digits, then
      stream/decompress the verified bytes through flate2 and tar. Accept only
      the three root-level regular files specified by the design; reject links,
      nested paths and duplicate names. Set executable permissions on Unix. Use
      `create_dir` to claim staging exclusively, an RAII cleanup owner only
      after that succeeds, and `File::lock` with the file handle alive until
      after publication. Recheck complete/damaged state under the lock before
      downloading. Write an empty marker only after validating all files, rename
      the directory, and release the lock. Return a diagnostic naming a damaged
      path; never repair or delete it. Lock files may persist and never count as
      releases. Explicit version arguments below `0.23.0` or not strict bare
      versions are usage failures (`2`). Production cannot access the library's
      alternate-origin seam from flags, environment or TOML.

- [ ] **Step 7: Write and run the failing CLI-surface tests.** Add
      `toolchain_cli_streams_and_exit_codes` to `tests/toolchain.rs` and
      `cli_dump_lists_toolchain_commands` to `xtask/src/freeze/cli.rs`. Run the
      former with complete/offline fixtures, then the latter before updating the
      dump. Assert `which --json` missing exits `1`, install missing/no pin
      exits `2`, list empty exits `0`, already-installed install prints exactly
      one absolute binary path plus newline to stdout and no download progress,
      text `which` prints only the path on success and a sentence only on stderr
      for negative statuses. Assert frozen lines include:

  ```rust
  assert!(lines.contains(&"ridl toolchain install".into()));
  assert!(lines.contains(&"ridl toolchain list".into()));
  assert!(lines.contains(&"ridl toolchain which".into()));
  assert!(lines.contains(&"ridl toolchain which --json switch".into()));
  assert!(lines.contains(&"ridl toolchain which arg 1 <PATH> optional".into()));
  ```

  Run `cargo test --locked -p ridl-cli --test toolchain toolchain_cli_` and
  `cargo test --locked -p xtask cli_dump_lists_toolchain_commands`. Expected
  before their adapter/dump update: assertion failures; assert the same new
  command lines occur in committed `freeze/cli.txt` so this remains red before
  regeneration even if clap traversal already finds them. Extend the existing
  CLI dumper with `which-json /<key>` pointer lines derived from an unpinned
  fixture plus invalid-pin diagnostic fixture, so all schema-1 object keys,
  including `/diagnostic/code` and `/diagnostic/message`, are frozen additively.
  Define `fn which_report_pointers(root: &Path) -> Result<Vec<String>, String>`
  in `xtask/src/freeze/cli.rs` for this traversal. Add
  `cli_dump_covers_which_json_fields` asserting those generated and committed
  lines; leave existing generic `json /...` report lines unchanged.

- [ ] **Step 8: Finish the adapter, documentation and gate integration.**
      Implement `run_toolchain(command: ToolchainCommand) -> ExitCode` in
      `main.rs`; use the status mapping, handle stdout failure as `2` and keep
      stderr failure from panicking. Add all nested help transcripts and
      exit-table rows to cli-reference, and install/list/which rows to ADR-0010.
      The existing `real_commands(exe: &Path) -> BTreeSet<String>` already
      traverses nested commands; retain it and document the toolchain parent and
      all three leaves without exemptions. Add the ignored HTTP test's exact
      invocation to `just install-check`; no new build member or gate-parity
      list is needed because this recipe already belongs to build. Generate
      `freeze/cli.txt` using `cargo xtask freeze update`; keep only intended
      additive lines. Confirm flate2's existing notice covers the direct
      dependency and add a notice only if absent.

- [ ] **Step 9: Run tests to verify they pass.** Run
      `cargo test --locked -p ridl-cli --test toolchain`,
      `cargo test --locked -p ridl-cli --test cli_reference`,
      `cargo test --locked -p xtask freeze::cli`, `just freeze-check`,
      `just link-check`, `just doc-path-check`. Outside the sandbox run the
      ignored HTTP command and `just install-check`. Expected: pass, schema `1`,
      no removed dump lines. Mutation controls: skip SHA-256, omit the post-lock
      recheck, treat any directory as complete, or compare versions lexically;
      each corresponding test fails, then passes after restoration.

- [ ] **Step 10: Commit.** Proposed message:
      `feat(ridl): install and select versioned toolchains`.

### Task 4: Install script releases into the shared directory

**Files:**

- Modify: `install.sh`, `install.ps1`, `justfile` `install-check` recipe,
  `docs/book/getting-started.md` installation section.
- Test: the existing fixture-release cases inside `justfile` `install-check`;
  retain the Deno ZIP verification cases at the end of that recipe.

**Interfaces:**

- Consumes: Task 3's directory/marker/protocol and target asset naming, not its
  Rust library at runtime. Existing script entry points stay `main()` in Bash
  and `function Main` in PowerShell; preserve existing `get_version()` /
  `function Get-Version` discovery and `RIDL_INSTALL_DRY_RUN` output.
- Produces Bash functions `normalise_version()` (one string argument, prints
  bare version, nonzero on invalid input) and `toolchain_root()` (no arguments,
  prints resolved root); PowerShell
  `function ConvertTo-ReleaseVersion { param([string]$Value) }` returning
  `[string]` and `function Get-ToolchainRoot` returning `[string]`. Both share
  the design's exact platform precedence.
- Environment contract remains `RIDL_VERSION`, `RIDL_INSTALL_BINARY`,
  `RIDL_INSTALL_DIR`, `RIDL_INSTALL_BASE_URL`, `RIDL_INSTALL_DRY_RUN`, with new
  `RIDL_TOOLCHAIN_DIR` and platform data variables. Keep the scripts'
  preexisting test origin seam; do not introduce it to the CLI. PATH copies are
  files, never symlinks. A successful dry run prints the normalized editor-tag
  URL without creating directories or downloading.

- [ ] **Step 1: Write the failing install-check cases.** Name the recipe cases
      `install_bare_and_tag_versions_share_layout`,
      `install_reuses_complete_release_without_download`,
      `install_preserves_damaged_version`,
      `install_tamper_leaves_no_version_or_path_copy`,
      `install_ridlc_remains_flat`, and `install_platform_roots_and_spaces`.
      Replace the current `editor-v0.0.0-install-check` fixture tag with
      `editor-v0.23.0`, and the below-floor PowerShell dry-run tag with
      `editor-v0.23.0`; fixture binaries print `ridl 0.23.0` / `ridlc 0.23.0`.
      Include LICENSE and notices. Keep all fixture/cache/PATH roots under the
      recipe's scratch directory, including LOCALAPPDATA and HOME/XDG overrides
      for platform tests.

  ```bash
  test -f "$toolchain_root/versions/0.23.0/.complete"
  test ! -s "$toolchain_root/versions/0.23.0/.complete"
  cmp "$toolchain_root/versions/0.23.0/ridl" "$install_dir/ridl"
  test ! -L "$install_dir/ridl"
  test ! -e "$tamper_root/versions/0.23.0"
  test ! -e "$tamper_install/ridl"
  test ! -e "$ridlc_root/versions"
  ```

  Use equivalent `Test-Path`, `Get-FileHash` and content assertions for
  `ridl.exe` in the PowerShell branch. After first install make the release
  source unavailable and delete the PATH copy; the second run must recreate that
  copy from the complete directory. Tamper cases need a fresh root, so the
  complete-directory fast path cannot mask corruption. Seed a damaged version
  with a sentinel, assert refusal names its path and leaves all bytes intact.
  Existing bad-binary selection, archive extraction failure, dry-run and Deno
  verification controls remain. Test Bash platform defaults by a fixture `uname`
  on PATH; test PowerShell defaults with isolated LOCALAPPDATA. Preserve paths
  containing spaces.

- [ ] **Step 2: Run test to verify it fails.** Run `just install-check` outside
      the sandbox. Expected: assertions for absent `versions/0.23.0/.complete`
      or failure to normalize bare versions. The Bash file-based subset can be
      exercised inside the sandbox, but do not claim the PowerShell/HTTP or ureq
      gate passed there. If `pwsh` is absent, report its cases as skipped.

- [ ] **Step 3: Implement the functions above and the script install paths.**
      Normalize a bare version or one `editor-v` prefix to canonical decimal
      components; reject suffixes and versions below `0.23.0` for `ridl`, and
      keep historical `ridlc` tag installation behavior. Resolve the version
      root before any download. Reuse a complete directory, refuse damage,
      verify checksum and all required extracted files, stage under
      `versions/.tmp-<version>-<pid>`, write empty `.complete`, then rename. Do
      not publish tarball/checksum files as release contents. Clean only the
      invocation's claimed staging directory on error/signals. Recheck
      destination state before publication and preserve any preexisting
      directory. Keep Bash's atomic temporary-file rename for the PATH copy, now
      copying from the versioned binary; do not move the versioned file out.
      PowerShell copies from its versioned binary and retains preference/TLS
      restoration in `finally`. `ridlc` follows the existing flat path and never
      creates/inspects the toolchain root. Do not add a Python/flock
      installation prerequisite.

- [ ] **Step 4: Update getting-started and run tests to verify they pass.**
      Document version roots, `.complete`, side-by-side releases, PATH-copy
      behavior, bare/tag `RIDL_VERSION`, explicit-version installation for CI,
      and flat `ridlc`. Link to the manifest chapter for pin semantics rather
      than duplicating them. Run `bash -n install.sh`, `just install-check`,
      `just link-check`, `just doc-path-check`; outside the sandbox run the
      complete gate including Task 3's HTTP test and the optional PowerShell
      cases. Expected: all available cases pass; repeated installs work with
      unavailable assets. Mutation controls: always download on second install,
      move rather than copy the binary, omit the marker, or reuse the good cache
      during tamper testing; the named cases must fail.

- [ ] **Step 5: Commit.** Proposed message:
      `feat(repo): install releases into the shared toolchain directory`.

### Task 5: Select binaries and manage one VS Code client per RIDL root

**Files:**

- Create:
  `editors/vscode/src/{toolchainSelection,toolchainSelection.test,workspaceClients,workspaceClients.test}.ts`.
- Modify:
  `editors/vscode/src/{binaryResolution,binaryResolution.test,clientLifecycle,clientLifecycle.test,extension,contributions.test}.ts`,
  `editors/vscode/package.json`, `editors/vscode/README.md`.
- Reuse: `editors/vscode/src/{runOnce,installToPath}.ts`; preserve their current
  tests and PATH-copy command behavior.

**Interfaces:**

- Consumes Task 3 subprocess API: `ridl toolchain which --json <path>` (schema
  `1`, exits `0/1/2`) and `ridl toolchain install <pin>` (stdout binary path,
  success `0`). Call the original bundled/PATH resolver binary for both
  operations, never a binary returned by an unvalidated JSON response.
- Retains
  `resolveBinary(input: ResolveInput): { command: string; source: BinarySource }`
  for configured/bundled/PATH precedence and
  `ClientLifecycle<C extends LifecycleClient>` with
  `startIfNeeded(): Promise<boolean>`, `restart(): Promise<boolean>`,
  `stop(): Promise<void>`, `client: C | undefined`.
- Produces
  `export interface BinaryRunner { run(command: string, args: readonly string[], cwd: string): Promise<{ exitCode: number; stdout: string; stderr: string }> }`
  and
  `export function parseWhichReport(value: unknown): WhichReport | undefined` in
  `toolchainSelection.ts`. TypeScript `WhichReport` is the schema-1
  discriminated union of Task 3's four serialized states, with the unpinned
  exception and nullability enforced; do not accept only the `schema` field and
  cast the rest.
- Produces
  `export interface Selection { readonly key: string; readonly manifest: string | null; readonly root: string | null; readonly command: string }`;
  `export interface SelectionEffects { progress<T>(title: string, work: () => Promise<T>): Promise<T>; offerInstall(pin: string): Promise<boolean>; errorOnce(key: string, message: string): void; log(message: string): void }`;
  `export interface SelectInput { readonly resolver: { command: string; source: BinarySource }; readonly directory: string; readonly autoInstall: boolean }`;
  `export class ToolchainSelector { constructor(runner: BinaryRunner, effects: SelectionEffects); select(input: SelectInput): Promise<Selection>; }`.
- Produces
  `export class WorkspaceClients<C extends LifecycleClient> { constructor(select: (directory: string) => Promise<Selection>, create: (selection: Selection) => C); open(directory: string): Promise<void>; refresh(manifest: string): Promise<boolean>; stop(): Promise<void>; }`
  in `workspaceClients.ts`. Keys are normalized absolute `which.manifest`, with
  `"<no-manifest>"` for the shared manifestless client. Keep a mapping from
  opened document directories to resolved keys; use real returned roots, not VS
  Code folder names or manual ancestor walks. `refresh` invalidates in-flight
  selections for that manifest and resolves true only if the selected command
  changed and a client was replaced.
- Changes
  `clientDocumentSelector(root?: string, excludedRoots?: readonly string[]): { scheme: string; language: string; pattern?: string }[]`
  in `binaryResolution.ts`; directory selectors use escaped absolute globs.
  `excludedRoots` describe nested roots to filter through routing middleware in
  `extension.ts`, because a VS Code glob alone cannot express the exclusion;
  keep manifests separately selected for Task 2's TOML actions/reload. Add
  `selectedCommand?: string` to existing `McpDefinitionInput`;
  `resolveMcpDefinition(input: McpDefinitionInput)` uses it when supplied and
  otherwise retains current resolution.

- [ ] **Step 1: Write failing selection/schema tests using the injected fake
      runner.** Name `explicit_server_path_skips_toolchain_commands`,
      `compatible_and_unpinned_start_selected_binary`,
      `missing_installs_requeries_then_starts`,
      `auto_install_off_uses_install_button`,
      `install_failure_uses_bundled_and_reports_once`, and
      `unknown_schema_and_malformed_results_fall_back_once`. Include exit `1`
      with valid JSON, nonzero spawn failure and unsupported command from an
      older development PATH binary; the runner must return stdout even for exit
      `1`. No real network or binary is used.

  ```typescript
  assert.deepEqual(calls, [
    [bundled, ["toolchain", "which", "--json", directory]],
    [bundled, ["toolchain", "install", "0.23.0"]],
    [bundled, ["toolchain", "which", "--json", directory]],
  ]);
  assert.equal(selection.command, installedBinary);
  assert.equal(selection.key, manifestPath);
  assert.equal(errorNotifications.length, 1);
  assert.equal(fallback.command, bundled);
  assert.equal(parseWhichReport({ ...validReport, schema: 2 }), undefined);
  assert.equal(parseWhichReport({ ...validReport, path: 17 }), undefined);
  assert.equal(parseWhichReport({ ...validReport, manifest: "relative/ridl.toml" }), undefined);
  ```

  Parameterize malformed report tests over missing required keys, unknown
  status/source, missing/null pin for missing, null path for compatible,
  inconsistent code, relative binary path and exit/status disagreement. Treat an
  absolute Windows drive/UNC path correctly on Windows. Unknown schema logs a
  warning and uses fallback without installing. Invalid pin starts fallback
  without installation so its LSP reports 024/025. A successful install followed
  by another `missing` response is a failure/fallback, not an install loop.
  Auto-install-off decline makes no install call; acceptance runs install and a
  fresh which. Concurrent missing roots for one pin share one in-flight
  installation/progress; a later explicit retry is possible after failure.
  Notifications deduplicate by release/error for the activation.

- [ ] **Step 2: Run test to verify it fails.** From `editors/vscode`, run
      `npm test` after adding the two new test outputs to its test script.
      Expected: behavioral assertions in the new tests fail against compiling
      inert selectors; existing resolver/lifecycle tests remain runnable.

- [ ] **Step 3: Implement `parseWhichReport` and `ToolchainSelector` with the
      exact interfaces above.** Keep `binaryResolution.ts` pure and retain the
      three existing resolver tiers. For setting source, return it without
      probing. Otherwise probe the resolver, validate schema/state/path fields,
      install only a `missing` valid pin and only through the application-level
      policy or accepted Install button, requery once, then return direct LSP
      binary selection. Normalize fallback notifications/logging and catch
      spawn/download/requery errors. Use `execFile` with argument arrays and
      bounded execution, not shell interpolation. Deduplicate installs by pin
      within this extension activation. A configured-path override uses the
      enclosing VS Code folder boundary without running which, preserving its
      explicit bypass. An unparseable discovery result uses the enclosing VS
      Code folder only as a temporary fallback routing boundary; it is replaced
      by the returned manifest as soon as a valid probe is available. Never
      infer a pin from a workspace setting or download URL from a result.

- [ ] **Step 4: Write failing root routing, restart and manifest contribution
      tests.** In `workspaceClients.test.ts`, name
      `two_ridl_roots_inside_one_folder_get_two_clients`,
      `documents_with_same_manifest_share_one_client`,
      `manifestless_documents_share_one_client`,
      `concurrent_opens_and_pin_changes_use_latest_selection`, and
      `pin_change_restarts_only_its_root_when_binary_changes`. Reuse fake-client
      state/queue cases from `clientLifecycle.test.ts`; add tests for a failed
      start retry and stopping all roots on deactivation. In
      `binaryResolution.test.ts`, name `root_selectors_exclude_nested_clients`
      and `mcp_uses_first_folder_selected_binary`; test the ordered first
      workspace folder even when its document opens second. In
      `contributions.test.ts`, name
      `auto_install_is_application_scoped_and_trust_capability_is_absent` and
      `manifest_workspace_activation_enables_pin_actions`.

  ```typescript
  assert.equal(createdClientsForSameManifest.length, 1);
  assert.equal(createdClientsForTwoRoots.length, 2);
  assert.equal(staleSelectionStarts, 0);
  assert.equal(unaffectedRootStops, 0);
  assert.equal(unchangedCommandRestarts, 0);
  assert.equal(mcpDefinition.command, firstFolderSelectedBinary);
  assert.deepEqual(mcpDefinition.args, ["mcp"]);
  assert.equal(autoInstall.type, "boolean");
  assert.equal(autoInstall.default, true);
  assert.equal(autoInstall.scope, "application");
  assert.equal(manifest.capabilities?.untrustedWorkspaces, undefined);
  assert.ok(manifest.activationEvents.includes("workspaceContains:**/ridl.toml"));
  ```

  The concurrency fixture pauses first selection, refreshes the root to another
  pin, resolves the newer selection first, then releases the older one; only the
  newer binary may start. Route a nested root's open/change/diagnostic traffic
  to exactly one client, and verify the parent client cannot publish diagnostics
  or edits for that root. Keep the manifestless client from covering documents
  assigned to a manifest client. Test root create/delete and workspace-folder
  removal, with dropped clients stopped and routing recomputed. The TOML root
  document belongs to the same client for pin actions even though its language
  id is `toml`. Changing a member manifest does not restart a client selected by
  a different root manifest.

- [ ] **Step 5: Run tests to verify they fail.** From `editors/vscode`, run
      `npm test`. Expected: duplicate-client/routing/restart assertions or
      missing application-scope property fail; no missing `vscode` runtime
      import in the pure test modules.

- [ ] **Step 6: Implement `WorkspaceClients`, selector wiring and contribution
      changes.** Reuse one `ClientLifecycle` per key and serialize operations
      with per-root generations so obsolete asynchronous results cannot
      create/restart clients. Do not discard its existing
      starting/start-failed/retry behavior. On RIDL document open resolve its
      directory, then start the selected binary with `lsp` directly. Supply each
      client's RIDL root as `workspaceFolders`/initialization root; a nested
      client must not initialize at its enclosing VS Code folder. Apply selector
      routing and middleware exclusions to keep nested and manifestless clients
      from claiming each other's documents or publishing overlapping
      diagnostics/edits. Add TOML manifest synchronization for Task 2's code
      action and unsaved reload. Add activation event
      `workspaceContains:**/ridl.toml` and select/start a client for an opened
      `ridl.toml` as well as a RIDL source, so the pin action works before
      opening a source. Other TOML files do not start a client.

  Register `**/ridl.toml` create/change/delete watching; send the corresponding
  LSP watched-file notification for reanalysis, rerun which for the owning root
  on disk pin change, and replace the client only if its binary changes. Unsaved
  manifest changes reload the running server without forcing a disk-based
  selection. Refresh affected mappings when a root appears/disappears or folder
  changes. Stop every client on deactivate. The MCP provider resolves selection
  for the first workspace folder independently of document-open order and emits
  definition changes after that selection changes; one definition per window.
  Read `ridl.serverPath` with the applicable folder resource, but read
  `ridl.toolchain.autoInstall` only as an application setting. Keep trust
  capability absent. Keep the existing install-to-PATH/stale-copy command, and
  show its offer once for the activation as before.

- [ ] **Step 7: Document and run tests to verify they pass.** Update extension
      README with pin selection, application setting, Install button,
      fallback/error, explicit override, trust behavior, multi-root rule and
      first-folder MCP limitation. From `editors/vscode` run `npm test`; from
      the repository run `just link-check` and `just doc-path-check`. Expected:
      all existing and new unit tests pass, including fake runner and lifecycle
      races. At the normal editor gate run `just vscode-verify` for both generic
      and target VSIX packaging; smoke-test trusted/untrusted activation and two
      pinned roots with the packaged extension outside the sandbox. Mutation
      controls: accept arbitrary schema, use one global client, omit generation
      checks, or use the last-opened binary for MCP; the corresponding tests
      fail after each mutation.

- [ ] **Step 8: Commit.** Proposed message:
      `feat(ridl): select pinned toolchains per editor workspace`.

### Task 6: Record decisions and the as-built protocol

**Files:**

- Create: `docs/decisions/ADR-0036-workspace-toolchain-pins.md`,
  `docs/design/toolchain-directory.md`.
- Modify: `docs/decisions/README.md`, `docs/decisions/ADR-0002-module-system.md`
  §4 and status amendment, `docs/specification/ridl-family-overview.md` §§2/5/6,
  `docs/ROADMAP.md`, `docs/technotes/walking-skeleton-architecture.md`, and
  cross-links in `docs/book/{release-pin,cli-reference,getting-started}.md` and
  `editors/vscode/README.md`.
- Test: existing link/path/book checks and the behavior tests owned by Tasks
  1–5; no new application test or documentation parser is warranted for this
  task.

**Interfaces:**

- Consumes the exact Task 1 release/loader contracts, Task 3 schema-1
  `WhichReport` and `install` protocol, and Task 5 subprocess/selection
  behavior. No runtime interface changes.
- Produces stable documentation targets
  `docs/decisions/ADR-0036-workspace-toolchain-pins.md` and
  `docs/design/toolchain-directory.md`; the latter owns directory layout,
  schema/nullability/exit mapping and install transaction ordering. Other pages
  link to it instead of duplicating its schema.
- Produces two follow-up references for the driving session to replace:
  `driftsys/ridl#TBD-deferred` and `driftsys/ridl#TBD-minimum-ci`. This task
  does not file issues or invent issue numbers.

- [ ] **Step 1: Establish the failing documentation check.** Add the new record
      links in the ADR index, overview and owning book pages before creating the
      files. Run `just link-check` and `just doc-path-check`. Expected: failures
      naming the two missing record paths; this is a real unresolved-link
      assertion, not a missing command or build failure. Do not add an
      application test solely to test prose.

- [ ] **Step 2: Write the ADR and protocol record.** Follow ADR-0035's `Status`,
      `Context`, numbered `Decisions`, consequences and references structure.
      Record the approved design date `2026-11-25`, scope and acceptance
      provenance without claiming a new maintainer approval. State root-only
      pin, exact compatibility/floor, exclusive refusal precedence, MANI codes
      and warning lint, version stamp normalization, old releases accepting the
      unknown key, version roots and damaged-directory preservation, constant
      origin/checksum limits, editor direct spawn and trust rule, explicit
      override, MCP first-folder limitation and deferred work.

  The design record specifies schema `1` field types and every
  status/nullability combination, including the unpinned running-path exception,
  normalized absolute paths and deterministic equal-version preference. Describe
  target detection, lock/recheck, verified extraction, same-filesystem staging,
  marker/rename, cleanup ownership, scripts' versioned installation/PATH copy
  and flat `ridlc`. Identify tests by their names from Tasks 1–5; do not
  introduce a second normative compatibility rule. Link the two follow-ups with
  their exact placeholders and state that the minimum-release extension CI job
  is due with the first release after `0.23.0`.

- [ ] **Step 3: Update the owning indexes and status records.** Amend ADR-0002
      §4 with a dated link to ADR-0036, append the ADR index summary, add the
      new record to overview §2 and a decision-ledger row in §5, and inspect §6
      for resolved/new questions (do not invent an open question). Overview §7
      was updated in Task 1. Add a bounded non-epic toolchain-pin row to ROADMAP
      with the actual implementation status and the two deferred references;
      preserve unrelated roadmap rows. Update the architecture map with
      ownership of `release.rs`, toolchain modules and editor per-root clients.
      Keep task docs with their owning tasks; this records the final behavior
      rather than postponing their documentation until this commit.

- [ ] **Step 4: Run checks to verify they pass and review the records against
      behavior.** Run `just link-check`, `just doc-path-check`,
      `just story-id-check`, `just book-check`, and `just check`. Expected: all
      pass and no unresolved new local link. Compare schema examples with
      `toolchain_which_schema_one_covers_all_statuses`, refusal claims with Task
      1/2 tests, and editor claims with Task 5's fake-runner tests. The driving
      session replaces issue placeholders before publication; no placeholder is
      treated as a real GitHub issue in this planning session. Review
      `git diff --check` and verify all frozen changes remain additions; no
      further build or mutation is required by this documentation-only task.

- [ ] **Step 5: Commit.** Proposed message:
      `docs(adr): record workspace toolchain pins and installation protocol`.
